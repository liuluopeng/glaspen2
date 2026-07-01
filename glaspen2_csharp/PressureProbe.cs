using System;
using System.Runtime.InteropServices;
using System.Threading;
using System.Windows.Forms;

namespace GlasPen2
{
    /// <summary>
    /// Standalone window to probe for pressure data via multiple methods.
    /// Run this to discover which approach works on this system.
    /// </summary>
    public class PressureProbe : Form
    {
        private int _wmPointerCount;
        private int _wmInputCount;
        private int _hookCount;

        public PressureProbe()
        {
            this.Text = "Pressure Probe";
            this.Size = new System.Drawing.Size(400, 300);
            this.StartPosition = FormStartPosition.CenterScreen;
            this.TopMost = true;
        }

        protected override void OnHandleCreated(EventArgs e)
        {
            base.OnHandleCreated(e);

            Program.Log("[Probe] Window created. Testing pressure APIs...");

            // Test 1: EnableMouseInPointer to get WM_POINTER
            bool empResult = NativeMethods.EnableMouseInPointer(true);
            Program.Log("[Probe] EnableMouseInPointer: {0}", empResult);

            // Test 2: Register for raw input (digitizer)
            RegisterRawInputProbe();

            // Test 3: Try HID devices with various access flags
            ProbeHidDevices();
        }

        private void RegisterRawInputProbe()
        {
            var devices = new NativeMethods.RAWINPUTDEVICE[2];
            devices[0].usUsagePage = 0x000D;
            devices[0].usUsage = 0x0002; // Pen
            devices[0].dwFlags = NativeMethods.RIDEV_INPUTSINK;
            devices[0].hwndTarget = this.Handle;

            devices[1].usUsagePage = 0x0001;
            devices[1].usUsage = 0x0002; // Mouse
            devices[1].dwFlags = NativeMethods.RIDEV_INPUTSINK;
            devices[1].hwndTarget = this.Handle;

            uint cbSize = (uint)Marshal.SizeOf(typeof(NativeMethods.RAWINPUTDEVICE));
            bool ok = NativeMethods.RegisterRawInputDevices(devices, 2, cbSize);
            Program.Log("[Probe] RegisterRawInput: {0} err={1}", ok, Marshal.GetLastWin32Error());
        }

        protected override void WndProc(ref Message m)
        {
            if (m.Msg == NativeMethods.WM_POINTERDOWN ||
                m.Msg == NativeMethods.WM_POINTERUPDATE ||
                m.Msg == NativeMethods.WM_POINTERUP)
            {
                _wmPointerCount++;
                uint pointerId = (uint)m.WParam.ToInt64();
                uint pointerType;
                NativeMethods.GetPointerType(pointerId, out pointerType);

                // Always try GetPointerPenInfo regardless of type
                var penInfo = new NativeMethods.POINTER_PEN_INFO();
                bool penOk = NativeMethods.GetPointerPenInfo(pointerId, ref penInfo);
                Program.Log("[Probe] WM_POINTER msg=0x{0:X4} type={1} penInfo={2} pressure={3} pos=({4},{5})",
                    m.Msg, pointerType, penOk, penInfo.pressure,
                    penInfo.pointerInfo.ptPixelLocation.X,
                    penInfo.pointerInfo.ptPixelLocation.Y);

                // Also try GetPointerInfo + GetPointerTouchInfo
                var ptrInfo = new NativeMethods.POINTER_INFO();
                bool ptrOk = NativeMethods.GetPointerInfo(pointerId, ref ptrInfo);
                if (ptrOk)
                {
                    Program.Log("[Probe]   PointerInfo: type={0} flags=0x{1:X8} ptPixel=({2},{3})",
                        ptrInfo.pointerType, ptrInfo.pointerFlags,
                        ptrInfo.ptPixelLocation.X, ptrInfo.ptPixelLocation.Y);
                }
            }
            else if (m.Msg == NativeMethods.WM_INPUT)
            {
                _wmInputCount++;
                ProcessRawInputProbe(m.LParam);
            }
            base.WndProc(ref m);
        }

        private void ProcessRawInputProbe(IntPtr hRawInput)
        {
            uint dwSize = 0;
            uint headerSize = (uint)Marshal.SizeOf(typeof(NativeMethods.RAWINPUTHEADER));
            NativeMethods.GetRawInputData(hRawInput, NativeMethods.RID_INPUT,
                IntPtr.Zero, ref dwSize, headerSize);
            if (dwSize == 0) return;

            IntPtr buffer = Marshal.AllocHGlobal((int)dwSize);
            try
            {
                uint bytesRead = NativeMethods.GetRawInputData(hRawInput, NativeMethods.RID_INPUT,
                    buffer, ref dwSize, headerSize);
                if (bytesRead != dwSize) return;

                var header = (NativeMethods.RAWINPUTHEADER)Marshal.PtrToStructure(
                    buffer, typeof(NativeMethods.RAWINPUTHEADER));

                if (_wmInputCount <= 20)
                {
                    Program.Log("[Probe] WM_INPUT #{0} type={1} size={2}",
                        _wmInputCount, header.dwType, dwSize);
                }

                if (header.dwType == NativeMethods.RIM_TYPEHID)
                {
                    int offset = Marshal.SizeOf(typeof(NativeMethods.RAWINPUTHEADER));
                    int dataLen = (int)(dwSize - offset);
                    if (dataLen >= 12)
                    {
                        uint dwSizeHid = (uint)Marshal.ReadInt32(buffer, offset);
                        uint dwCount = (uint)Marshal.ReadInt32(buffer, offset + 4);
                        int baseOff = offset + 8;
                        byte reportId = Marshal.ReadByte(buffer, baseOff);
                        byte switches = Marshal.ReadByte(buffer, baseOff + 1);
                        uint x = (uint)Marshal.ReadByte(buffer, baseOff + 2) | ((uint)Marshal.ReadByte(buffer, baseOff + 3) << 8);
                        uint y = (uint)Marshal.ReadByte(buffer, baseOff + 4) | ((uint)Marshal.ReadByte(buffer, baseOff + 5) << 8);
                        uint pressure = (uint)Marshal.ReadByte(buffer, baseOff + 6) | ((uint)Marshal.ReadByte(buffer, baseOff + 7) << 8);
                        bool tipDown = (switches & 0x01) != 0;

                        Program.Log("[Probe] HID rpt={0} sw=0x{1:X2} x={2} y={3} press={4} tip={5}",
                            reportId, switches, x, y, pressure, tipDown);
                    }
                }
            }
            finally
            {
                Marshal.FreeHGlobal(buffer);
            }
        }

        private void ProbeHidDevices()
        {
            Guid hidGuid;
            NativeMethods.HidD_GetHidGuid(out hidGuid);

            IntPtr devInfo = NativeMethods.SetupDiGetClassDevs(
                ref hidGuid, null, IntPtr.Zero,
                NativeMethods.DIGCF_PRESENT | NativeMethods.DIGCF_DEVICEINTERFACE);

            if (devInfo == IntPtr.Zero || devInfo == new IntPtr(-1))
            {
                Program.Log("[Probe] SetupDiGetClassDevs FAILED");
                return;
            }

            try
            {
                int structSize = Marshal.SizeOf(typeof(NativeMethods.SP_DEVICE_INTERFACE_DATA));
                uint index = 0;

                while (true)
                {
                    IntPtr ifDataPtr = Marshal.AllocHGlobal(structSize);
                    Marshal.Copy(new byte[structSize], 0, ifDataPtr, structSize);
                    Marshal.WriteInt32(ifDataPtr, structSize);

                    var ifData = (NativeMethods.SP_DEVICE_INTERFACE_DATA)Marshal.PtrToStructure(
                        ifDataPtr, typeof(NativeMethods.SP_DEVICE_INTERFACE_DATA));

                    bool ok = NativeMethods.SetupDiEnumDeviceInterfaces(
                        devInfo, IntPtr.Zero, ref hidGuid, index, ref ifData);

                    if (!ok) { Marshal.FreeHGlobal(ifDataPtr); break; }

                    // Get device path
                    uint requiredSize = 0;
                    NativeMethods.SetupDiGetDeviceInterfaceDetail(
                        devInfo, ref ifData, IntPtr.Zero, 0, ref requiredSize, IntPtr.Zero);

                    string devicePath = null;
                    if (requiredSize > 0)
                    {
                        IntPtr detailData = Marshal.AllocHGlobal((int)requiredSize);
                        if (IntPtr.Size == 8)
                            Marshal.WriteInt32(detailData, 8);
                        else
                            Marshal.WriteInt32(detailData, 6);

                        if (NativeMethods.SetupDiGetDeviceInterfaceDetail(
                            devInfo, ref ifData, detailData, requiredSize, ref requiredSize, IntPtr.Zero))
                        {
                            devicePath = Marshal.PtrToStringUni(detailData + 4);
                        }
                        Marshal.FreeHGlobal(detailData);
                    }
                    Marshal.FreeHGlobal(ifDataPtr);

                    if (devicePath != null)
                    {
                        TryOpenWithAllFlags(devicePath, index);
                    }

                    index++;
                }
                Program.Log("[Probe] Enumerated {0} devices", index);
            }
            finally
            {
                NativeMethods.SetupDiDestroyDeviceInfoList(devInfo);
            }
        }

        private void TryOpenWithAllFlags(string devicePath, uint index)
        {
            // Try many different access mode combinations
            uint[][] accessCombos = {
                new uint[] { NativeMethods.GENERIC_READ, NativeMethods.FILE_SHARE_READ | NativeMethods.FILE_SHARE_WRITE },
                new uint[] { NativeMethods.GENERIC_READ, NativeMethods.FILE_SHARE_READ | NativeMethods.FILE_SHARE_WRITE | 0x00000004 }, // FILE_SHARE_DELETE
                new uint[] { 0x00000001, NativeMethods.FILE_SHARE_READ | NativeMethods.FILE_SHARE_WRITE }, // FILE_READ_ATTRIBUTES
                new uint[] { 0x00000080, NativeMethods.FILE_SHARE_READ | NativeMethods.FILE_SHARE_WRITE }, // FILE_READ_DATA (maybe?)
                new uint[] { 0x2000000, NativeMethods.FILE_SHARE_READ | NativeMethods.FILE_SHARE_WRITE }, // FILE_GENERIC_READ
                new uint[] { 0, NativeMethods.FILE_SHARE_READ | NativeMethods.FILE_SHARE_WRITE },
            };

            foreach (var combo in accessCombos)
            {
                IntPtr handle = NativeMethods.CreateFile(
                    devicePath, combo[0], combo[1],
                    IntPtr.Zero, NativeMethods.OPEN_EXISTING, 0, IntPtr.Zero);

                if (handle != IntPtr.Zero && handle != new IntPtr(-1))
                {
                    IntPtr preparsed;
                    if (NativeMethods.HidD_GetPreparsedData(handle, out preparsed))
                    {
                        var caps = new NativeMethods.HIDP_CAPS();
                        uint status = NativeMethods.HidP_GetCaps(preparsed, ref caps);
                        if (status == 0 && caps.UsagePage == 0x000D)
                        {
                            Program.Log("[Probe] Device #{0} OPENED! access=0x{1:X8} UsagePage=0x{2:X4} Usage=0x{3:X4} ReportLen={4}",
                                index, combo[0], caps.UsagePage, caps.Usage, caps.InputReportByteLength);

                            // Try reading one report
                            TryReadReport(handle, caps.InputReportByteLength, index);
                        }
                        NativeMethods.HidD_FreePreparsedData(preparsed);
                    }
                    NativeMethods.CloseHandle(handle);
                    return; // opened successfully, no need to try more
                }
            }
        }

        private void TryReadReport(IntPtr handle, ushort reportLen, uint deviceIndex)
        {
            byte[] buffer = new byte[reportLen];
            IntPtr unmanaged = Marshal.AllocHGlobal(reportLen);
            try
            {
                uint bytesRead = 0;
                NativeOverlapped overlapped = new NativeOverlapped();
                bool ok = NativeMethods.ReadFile(handle, unmanaged, reportLen, ref bytesRead, ref overlapped);
                if (ok && bytesRead > 0)
                {
                    Marshal.Copy(unmanaged, buffer, 0, (int)bytesRead);
                    Program.Log("[Probe] Device #{0} READ {1} bytes: {2}",
                        deviceIndex, bytesRead, BitConverter.ToString(buffer, 0, Math.Min((int)bytesRead, 16)));
                }
                else
                {
                    Program.Log("[Probe] Device #{0} ReadFile failed err={1}", deviceIndex, Marshal.GetLastWin32Error());
                }
            }
            finally
            {
                Marshal.FreeHGlobal(unmanaged);
            }
        }
    }
}
