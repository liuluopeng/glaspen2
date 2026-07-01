using System;
using System.Runtime.InteropServices;
using System.Threading;

namespace GlasPen2
{
    /// <summary>
    /// Opens the pen digitizer HID device directly via CreateFile + ReadFile,
    /// bypassing Windows Ink and Raw Input entirely. Works with Ink OFF.
    /// </summary>
    public class HidReader : IDisposable
    {
        private IntPtr _deviceHandle = IntPtr.Zero;
        private Thread _readThread;
        private bool _running;

        public event Action<uint, uint, uint, uint> PacketReceived; // x, y, pressure, switches

        public bool IsOpen { get { return _deviceHandle != IntPtr.Zero && _deviceHandle != new IntPtr(-1); } }

        public bool Open()
        {
            // Enumerate HID digitizer devices
            Guid hidGuid;
            NativeMethods.HidD_GetHidGuid(out hidGuid);
            Console.WriteLine("[HidReader] HID Guid = {0}", hidGuid);

            IntPtr devInfo = NativeMethods.SetupDiGetClassDevs(ref hidGuid, null, IntPtr.Zero,
                NativeMethods.DIGCF_PRESENT | NativeMethods.DIGCF_DEVICEINTERFACE);

            if (devInfo == new IntPtr(-1))
            {
                Console.WriteLine("[HidReader] SetupDiGetClassDevs FAILED. err={0}",
                    Marshal.GetLastWin32Error());
                return false;
            }
            Console.WriteLine("[HidReader] SetupDiGetClassDevs OK. devInfo=0x{0:X}", devInfo.ToInt64());

            int enumCount = 0;
            try
            {
                var iface = new NativeMethods.SP_DEVICE_INTERFACE_DATA();
                iface.cbSize = Marshal.SizeOf(iface);

                for (uint i = 0; NativeMethods.SetupDiEnumDeviceInterfaces(devInfo, IntPtr.Zero, ref hidGuid, i, ref iface); i++)
                {
                    enumCount++;

                    // Get required buffer size
                    uint required = 0;
                    bool sizeOk = NativeMethods.SetupDiGetDeviceInterfaceDetail(devInfo, ref iface, IntPtr.Zero, 0, ref required, IntPtr.Zero);
                    int sizeErr = Marshal.GetLastWin32Error();
                    if (i < 3)
                        Console.WriteLine("[HidReader #{0}] sizeReq={1} sizeOk={2} sizeErr={3}",
                            i, required, sizeOk, sizeErr);
                    if (required == 0) continue;

                    IntPtr detailBuf = Marshal.AllocHGlobal((int)required);
                    try
                    {
                        // SP_DEVICE_INTERFACE_DETAIL_DATA: DWORD cbSize, WCHAR DevicePath[]
                        // cbSize = 8 on x64, 6 on x86 (includes sizeof(DWORD)+sizeof(WCHAR) with packing)
                        int cbSize = IntPtr.Size == 8 ? 8 : 6;
                        Marshal.WriteInt32(detailBuf, cbSize);

                        if (!NativeMethods.SetupDiGetDeviceInterfaceDetail(devInfo, ref iface, detailBuf, required, ref required, IntPtr.Zero))
                        {
                            int err = Marshal.GetLastWin32Error();
                            if (i < 3)
                                Console.WriteLine("[HidReader #{0}] 2nd call FAILED err={1} cbSize={2} req={3}", i, err, cbSize, required);
                            continue;
                        }

                        string devicePath = Marshal.PtrToStringUni(detailBuf + 4); // skip DWORD cbSize

                        // Try to open and get HID caps
                        IntPtr dev = NativeMethods.CreateFile(devicePath,
                            NativeMethods.GENERIC_READ,
                            NativeMethods.FILE_SHARE_READ | NativeMethods.FILE_SHARE_WRITE,
                            IntPtr.Zero, NativeMethods.OPEN_EXISTING,
                            NativeMethods.FILE_ATTRIBUTE_NORMAL | NativeMethods.FILE_FLAG_OVERLAPPED,
                            IntPtr.Zero);

                        if (dev == new IntPtr(-1) || dev == IntPtr.Zero)
                        {
                            if (i < 5)
                                Console.WriteLine("[HidReader #{0}] CreateFile FAILED err={1}",
                                    i, Marshal.GetLastWin32Error());
                            continue;
                        }

                        IntPtr preparsed;
                        bool isPen = false;
                        if (NativeMethods.HidD_GetPreparsedData(dev, out preparsed))
                        {
                            var caps = new NativeMethods.HIDP_CAPS();
                            if (NativeMethods.HidP_GetCaps(preparsed, ref caps) == 0) // HIDP_STATUS_SUCCESS
                            {
                                Console.WriteLine("[HidReader #{0}] UsagePage=0x{1:X4} Usage=0x{2:X4} InLen={3}",
                                    i, caps.UsagePage, caps.Usage, caps.InputReportByteLength);

                                if (caps.UsagePage == 0x000D && (caps.Usage == 0x0001 || caps.Usage == 0x0002))
                                {
                                    isPen = true;
                                    Console.WriteLine("[HidReader] -> PEN DIGITIZER! Opening...");
                                    _deviceHandle = dev;
                                    Console.WriteLine("[HidReader]   InputReportLen={0}", caps.InputReportByteLength);
                                }
                            }
                            NativeMethods.HidD_FreePreparsedData(preparsed);
                        }

                        if (!isPen)
                            NativeMethods.CloseHandle(dev);
                        else
                            return true; // found and opened
                    }
                    finally
                    {
                        Marshal.FreeHGlobal(detailBuf);
                    }
                }
            }
            finally
            {
                Console.WriteLine("[HidReader] Enumerated {0} devices.", enumCount);
                NativeMethods.SetupDiDestroyDeviceInfoList(devInfo);
            }

            Console.WriteLine("[HidReader] No pen digitizer found among {0} HID devices.", enumCount);
            return false;
        }

        public void StartReading()
        {
            if (!IsOpen) return;
            _running = true;
            _readThread = new Thread(ReadLoop);
            _readThread.IsBackground = true;
            _readThread.Start();
            Console.WriteLine("[HidReader] Read thread started.");
        }

        private void ReadLoop()
        {
            var overlapped = new NativeOverlapped();
            var readyEvent = new ManualResetEvent(false);
            overlapped.EventHandle = readyEvent.SafeWaitHandle.DangerousGetHandle();

            IntPtr buffer = Marshal.AllocHGlobal(256);
            try
            {
                while (_running)
                {
                    uint bytesRead = 0;
                    readyEvent.Reset();

                    bool ok = NativeMethods.ReadFile(_deviceHandle, buffer, 256, ref bytesRead, ref overlapped);
                    int err = Marshal.GetLastWin32Error();

                    if (!ok && err == NativeMethods.ERROR_IO_PENDING)
                    {
                        // Wait for data with timeout
                        if (readyEvent.WaitOne(500))
                        {
                            NativeMethods.GetOverlappedResult(_deviceHandle, ref overlapped, ref bytesRead, false);
                        }
                        else
                        {
                            NativeMethods.CancelIo(_deviceHandle);
                            continue;
                        }
                    }
                    else if (!ok)
                    {
                        Console.WriteLine("[HidReader] ReadFile error: {0}", err);
                        break;
                    }

                    if (bytesRead >= 8)
                    {
                        ParseReport(buffer, (int)bytesRead);
                    }
                }
            }
            finally
            {
                Marshal.FreeHGlobal(buffer);
                readyEvent.Dispose();
            }
        }

        private int _packetCount;
        private void ParseReport(IntPtr buffer, int len)
        {
            if (len < 8) return;

            byte reportId = Marshal.ReadByte(buffer);
            byte switches = Marshal.ReadByte(buffer, 1);
            uint x = (uint)Marshal.ReadByte(buffer, 2) | ((uint)Marshal.ReadByte(buffer, 3) << 8);
            uint y = (uint)Marshal.ReadByte(buffer, 4) | ((uint)Marshal.ReadByte(buffer, 5) << 8);
            uint pressure = (uint)Marshal.ReadByte(buffer, 6) | ((uint)Marshal.ReadByte(buffer, 7) << 8);

            _packetCount++;
            if (_packetCount <= 10 || _packetCount % 100 == 0)
                Console.WriteLine("[HidReader #{0}] x={1} y={2} press={3} sw=0x{4:X2}",
                    _packetCount, x, y, pressure, switches);

            if (PacketReceived != null)
                PacketReceived(x, y, pressure, switches);
        }

        public void Stop()
        {
            _running = false;
            if (_readThread != null && _readThread.IsAlive)
                _readThread.Join(1000);
        }

        public void Dispose()
        {
            Stop();
            if (IsOpen)
            {
                NativeMethods.CloseHandle(_deviceHandle);
                _deviceHandle = IntPtr.Zero;
                Console.WriteLine("[HidReader] Device closed.");
            }
        }
    }
}
