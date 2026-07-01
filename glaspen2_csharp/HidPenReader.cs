using System;
using System.Runtime.InteropServices;
using System.Threading;

namespace GlasPen2
{
    /// <summary>
    /// Reads pen pressure from HID digitizer device directly.
    /// Works when INK is OFF (pen events are mouse, not WM_POINTER).
    /// </summary>
    public class HidPenReader : IDisposable
    {
        private IntPtr _deviceHandle = IntPtr.Zero;
        private Thread _readThread;
        private volatile bool _running;
        private IntPtr _preparsedData = IntPtr.Zero;
        private ushort _inputReportByteLength;

        // Pressure range from HID report
        private uint _pressureMin;
        private uint _pressureMax = 16000;

        // Latest values from HID
        public uint LastX { get; private set; }
        public uint LastY { get; private set; }
        public uint LastPressure { get; private set; }
        public bool TipDown { get; private set; }
        public bool HasData { get; private set; }

        // Coordinate ranges (dynamic, for mapping to screen)
        public uint MinX { get; private set; }
        public uint MaxX { get; private set; }
        public uint MinY { get; private set; }
        public uint MaxY { get; private set; }

        public HidPenReader()
        {
            MaxX = 65535;
            MaxY = 65535;
        }

        // Events
        public event Action<uint, uint, uint, bool> PenReport; // x, y, pressure, tipDown

        public bool Open()
        {
            Program.Log("[HidPen] Open() called");
            Guid hidGuid;
            NativeMethods.HidD_GetHidGuid(out hidGuid);
            Program.Log("[HidPen] HID GUID: {0}", hidGuid);

            IntPtr devInfo = NativeMethods.SetupDiGetClassDevs(
                ref hidGuid, null, IntPtr.Zero,
                NativeMethods.DIGCF_PRESENT | NativeMethods.DIGCF_DEVICEINTERFACE);
            Program.Log("[HidPen] SetupDiGetClassDevs returned: 0x{0:X}", devInfo.ToInt64());

            if (devInfo == IntPtr.Zero || devInfo == new IntPtr(-1))
            {
                Program.Log("[HidPen] SetupDiGetClassDevs FAILED err={0}", Marshal.GetLastWin32Error());
                return false;
            }

            try
            {
                return EnumerateDevices(devInfo, hidGuid);
            }
            finally
            {
                NativeMethods.SetupDiDestroyDeviceInfoList(devInfo);
            }
        }

        private bool EnumerateDevices(IntPtr devInfo, Guid hidGuid)
        {
            // SP_DEVICE_INTERFACE_DATA: cbSize=4, Guid=16, Flags=4, Reserved(IntPtr)=8 → 28 on x64
            int structSize = Marshal.SizeOf(typeof(NativeMethods.SP_DEVICE_INTERFACE_DATA));
            Program.Log("[HidPen] SP_DEVICE_INTERFACE_DATA size={0}", structSize);

            uint index = 0;
            while (true)
            {
                // Allocate and zero-init, then set cbSize
                IntPtr ifDataPtr = Marshal.AllocHGlobal(structSize);
                Marshal.Copy(new byte[structSize], 0, ifDataPtr, structSize);
                // cbSize = structSize (4 + 16 + 4 + IntPtr.Size)
                Marshal.WriteInt32(ifDataPtr, structSize);

                var ifData = (NativeMethods.SP_DEVICE_INTERFACE_DATA)Marshal.PtrToStructure(
                    ifDataPtr, typeof(NativeMethods.SP_DEVICE_INTERFACE_DATA));

                bool ok = NativeMethods.SetupDiEnumDeviceInterfaces(
                    devInfo, IntPtr.Zero, ref hidGuid, index, ref ifData);

                if (!ok)
                {
                    Marshal.FreeHGlobal(ifDataPtr);
                    int err = Marshal.GetLastWin32Error();
                    if (index == 0)
                        Program.Log("[HidPen] No HID devices found (err={0})", err);
                    else
                        Program.Log("[HidPen] Enumerated {0} HID devices", index);
                    break;
                }

                // Write back the updated struct
                Marshal.StructureToPtr(ifData, ifDataPtr, false);

                string devicePath = GetDevicePath(devInfo, ref ifData);
                Marshal.FreeHGlobal(ifDataPtr);

                if (devicePath != null)
                {
                    Program.Log("[HidPen] Device #{0}: {1}", index,
                        devicePath.Length > 80 ? devicePath.Substring(0, 80) + "..." : devicePath);

                    if (TryOpenDevice(devicePath))
                        return true;
                }

                index++;
            }
            return false;
        }

        private string GetDevicePath(IntPtr devInfo, ref NativeMethods.SP_DEVICE_INTERFACE_DATA ifData)
        {
            uint requiredSize = 0;
            NativeMethods.SetupDiGetDeviceInterfaceDetail(
                devInfo, ref ifData, IntPtr.Zero, 0, ref requiredSize, IntPtr.Zero);

            if (requiredSize == 0) return null;

            // Allocate buffer: cbSize (4 or 6 depending on 32/64-bit) + device path
            IntPtr detailData = Marshal.AllocHGlobal((int)requiredSize);
            try
            {
                // cbSize: on x64 it's 8 (4 + alignment), on x86 it's 4+2=6
                if (IntPtr.Size == 8)
                    Marshal.WriteInt32(detailData, 8);  // x64
                else
                    Marshal.WriteInt32(detailData, 6);  // x86

                if (!NativeMethods.SetupDiGetDeviceInterfaceDetail(
                    devInfo, ref ifData, detailData, requiredSize, ref requiredSize, IntPtr.Zero))
                {
                    int err = Marshal.GetLastWin32Error();
                    Program.Log("[HidPen] GetDeviceInterfaceDetail FAILED err={0}", err);
                    return null;
                }

                // Device path starts at offset 4 (after cbSize)
                return Marshal.PtrToStringUni(detailData + 4);
            }
            finally
            {
                Marshal.FreeHGlobal(detailData);
            }
        }

        private bool TryOpenDevice(string devicePath)
        {
            // Try multiple access modes
            uint[] accessModes = {
                NativeMethods.GENERIC_READ,
                0x00000001, // FILE_READ_DATA
                0,          // no access (for HidD_GetAttributes only)
            };

            foreach (uint access in accessModes)
            {
                IntPtr handle = NativeMethods.CreateFile(
                    devicePath,
                    access,
                    NativeMethods.FILE_SHARE_READ | NativeMethods.FILE_SHARE_WRITE,
                    IntPtr.Zero,
                    NativeMethods.OPEN_EXISTING,
                    0,
                    IntPtr.Zero);

                if (handle == IntPtr.Zero || handle == new IntPtr(-1))
                    continue;

                // Get VID/PID even if we can't read data
                var attrs = new NativeMethods.HIDD_ATTRIBUTES();
                attrs.Size = Marshal.SizeOf(typeof(NativeMethods.HIDD_ATTRIBUTES));
                NativeMethods.HidD_GetAttributes(handle, ref attrs);

                IntPtr preparsed;
                if (!NativeMethods.HidD_GetPreparsedData(handle, out preparsed))
                {
                    Program.Log("[HidPen] VID=0x{0:X4} PID=0x{1:X4} access=0x{2:X8} — no preparsed data",
                        attrs.VendorID, attrs.ProductID, access);
                    // Try direct read without preparsed data
                    if (TryDirectRead(handle))
                    {
                        _deviceHandle = handle;
                        _inputReportByteLength = 64; // default buffer size
                        StartReading();
                        return true;
                    }
                    NativeMethods.CloseHandle(handle);
                    continue;
                }

                var caps = new NativeMethods.HIDP_CAPS();
                uint status = NativeMethods.HidP_GetCaps(preparsed, ref caps);
                if (status != 0)
                {
                    Program.Log("[HidPen] VID=0x{0:X4} PID=0x{1:X4} access=0x{2:X8} — caps failed 0x{3:X8}",
                        attrs.VendorID, attrs.ProductID, access, status);
                    NativeMethods.HidD_FreePreparsedData(preparsed);
                    // Try direct read
                    if (TryDirectRead(handle))
                    {
                        _deviceHandle = handle;
                        _inputReportByteLength = 64;
                        StartReading();
                        return true;
                    }
                    NativeMethods.CloseHandle(handle);
                    continue;
                }

                Program.Log("[HidPen] VID=0x{0:X4} PID=0x{1:X4} UsagePage=0x{2:X4} Usage=0x{3:X4} ReportLen={4}",
                    attrs.VendorID, attrs.ProductID, caps.UsagePage, caps.Usage, caps.InputReportByteLength);

                if (caps.UsagePage == 0x000D && (caps.Usage == 0x02 || caps.Usage == 0x01))
                {
                    Program.Log("[HidPen] *** FOUND DIGITIZER ***");
                    _deviceHandle = handle;
                    _preparsedData = preparsed;
                    _inputReportByteLength = caps.InputReportByteLength;
                    StartReading();
                    return true;
                }

                NativeMethods.HidD_FreePreparsedData(preparsed);
                NativeMethods.CloseHandle(handle);
            }

            return false;
        }

        private bool TryDirectRead(IntPtr handle)
        {
            // Try reading with various buffer sizes
            int[] sizes = { 8, 16, 32, 64 };
            foreach (int size in sizes)
            {
                IntPtr buf = Marshal.AllocHGlobal(size);
                try
                {
                    uint bytesRead = 0;
                    NativeOverlapped overlapped = new NativeOverlapped();
                    bool ok = NativeMethods.ReadFile(handle, buf, (uint)size, ref bytesRead, ref overlapped);
                    if (ok && bytesRead > 0)
                    {
                        byte[] data = new byte[bytesRead];
                        Marshal.Copy(buf, data, 0, (int)bytesRead);
                        Program.Log("[HidPen] Direct read {0} bytes: [{1}]",
                            bytesRead, BitConverter.ToString(data, 0, Math.Min((int)bytesRead, 16)));
                        return true;
                    }
                }
                finally
                {
                    Marshal.FreeHGlobal(buf);
                }
            }
            return false;
        }

        private void StartReading()
        {
            _running = true;
            _readThread = new Thread(ReadLoop)
            {
                IsBackground = true,
                Name = "HidPenReader"
            };
            _readThread.Start();
        }

        private void ReadLoop()
        {
            byte[] buffer = new byte[_inputReportByteLength];
            IntPtr unmanagedBuffer = Marshal.AllocHGlobal(_inputReportByteLength);

            Program.Log("[HidPen] Read loop started, report size={0}", _inputReportByteLength);

            try
            {
                while (_running)
                {
                    // Zero the buffer
                    Marshal.Copy(new byte[_inputReportByteLength], 0, unmanagedBuffer, _inputReportByteLength);

                    uint bytesRead = 0;
                    // Synchronous ReadFile (non-overlapped)
                    bool ok = NativeMethods.ReadFile(
                        _deviceHandle,
                        unmanagedBuffer,
                        _inputReportByteLength,
                        ref bytesRead,
                        ref _overlappedDummy);

                    if (!ok || bytesRead == 0)
                    {
                        int err = Marshal.GetLastWin32Error();
                        if (_running)
                            Program.Log("[HidPen] ReadFile err={0} bytesRead={1}", err, bytesRead);
                        Thread.Sleep(10);
                        continue;
                    }

                    Marshal.Copy(unmanagedBuffer, buffer, 0, (int)bytesRead);
                    ParseReport(buffer, (int)bytesRead);
                }
            }
            finally
            {
                Marshal.FreeHGlobal(unmanagedBuffer);
                Program.Log("[HidPen] Read loop ended");
            }
        }

        // Dummy overlapped struct for synchronous read (not actually used)
        private NativeOverlapped _overlappedDummy;

        private void ParseReport(byte[] data, int length)
        {
            // Standard digitizer HID report format:
            // [reportId:1][switches:1][X:2 LE][Y:2 LE][pressure:2 LE]...
            if (length < 8) return;

            byte reportId = data[0];
            byte switches = data[1];
            uint x = (uint)data[2] | ((uint)data[3] << 8);
            uint y = (uint)data[4] | ((uint)data[5] << 8);
            uint pressure = (uint)data[6] | ((uint)data[7] << 8);

            bool tipDown = (switches & 0x01) != 0;

            // Update state
            LastX = x;
            LastY = y;
            LastPressure = pressure;
            TipDown = tipDown;
            HasData = true;

            // Track coordinate ranges for mapping
            if (x > 0 && x < 100000)
            {
                if (x < MinX || MinX == 0) MinX = x;
                if (x > MaxX) MaxX = x;
            }
            if (y > 0 && y < 100000)
            {
                if (y < MinY || MinY == 0) MinY = y;
                if (y > MaxY) MaxY = y;
            }

            // Log first few reports, then periodically
            _reportCount++;
            if (_reportCount <= 10 || _reportCount % 100 == 0)
            {
                Program.Log("[HidPen #{0}] rpt={1} sw=0x{2:X2} x={3} y={4} press={5} tip={6}",
                    _reportCount, reportId, switches, x, y, pressure, tipDown);
            }

            // Fire event
            if (PenReport != null)
                PenReport(x, y, pressure, tipDown);
        }

        private int _reportCount;

        public void Dispose()
        {
            _running = false;
            if (_preparsedData != IntPtr.Zero)
            {
                NativeMethods.HidD_FreePreparsedData(_preparsedData);
                _preparsedData = IntPtr.Zero;
            }
            if (_deviceHandle != IntPtr.Zero)
            {
                NativeMethods.CloseHandle(_deviceHandle);
                _deviceHandle = IntPtr.Zero;
            }
        }
    }
}
