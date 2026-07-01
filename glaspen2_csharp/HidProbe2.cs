using System;
using System.Runtime.InteropServices;
using System.Threading;
using System.Windows.Forms;

namespace GlasPen2
{
    /// <summary>
    /// Aggressive HID probe: registers ALL digitizer usages with RIDEV_EXINPUTSINK,
    /// logs every WM_INPUT and WM_POINTER with full detail.
    /// </summary>
    public class HidProbe2 : Form
    {
        private int _wmInputCount;
        private int _wmPointerCount;
        private int _hidReportCount;
        private int _mouseReportCount;

        public HidProbe2()
        {
            this.Text = "HID Probe 2";
            this.Size = new System.Drawing.Size(500, 400);
            this.StartPosition = FormStartPosition.CenterScreen;
            this.TopMost = true;
        }

        protected override void OnHandleCreated(EventArgs e)
        {
            base.OnHandleCreated(e);
            Program.Log("[Probe2] Window created. Registering ALL digitizer usages...");

            // Register for EVERY digitizer usage with RIDEV_EXINPUTSINK
            RegisterAllDigitizerUsages();

            // Also try direct HID device probing with corrected parsing
            ProbeHidDevicesFixed();
        }

        private void RegisterAllDigitizerUsages()
        {
            // All possible digitizer usages from HID Usage Tables
            ushort[] digitizerUsages = {
                0x0001, // Digitizer
                0x0002, // Pen
                0x0003, // Light Pen
                0x0004, // Touch Screen
                0x0005, // Touch Pad
                0x0006, // Whiteboard
                0x0007, // Coordinate Measuring Machine
                0x0008, // 3D Digitizer
                0x0009, // Stereo Plotter
                0x000A, // Articulated Arm
                0x000B, // Armature
                0x000C, // Multiple Point Digitizer
                0x000D, // Free Space Wand
                0x000E, // Device Configuration
                0x000F, // Capacitive Heat Map Digitizer
                0x0020, // Stylus (sub-usage)
                0x0021, // Finger (sub-usage)
                0x0022, // Device Settings (sub-usage)
            };

            uint EXSINK = 0x00001000; // RIDEV_EXINPUTSINK
            uint INPUTSINK = NativeMethods.RIDEV_INPUTSINK;

            var devices = new NativeMethods.RAWINPUTDEVICE[digitizerUsages.Length * 2 + 2];
            int idx = 0;

            // Mouse (always needed)
            devices[idx].usUsagePage = 0x0001;
            devices[idx].usUsage = 0x0002;
            devices[idx].dwFlags = INPUTSINK;
            devices[idx].hwndTarget = this.Handle;
            idx++;

            // All digitizer usages with EXINPUTSINK
            foreach (ushort usage in digitizerUsages)
            {
                // With RIDEV_EXINPUTSINK
                devices[idx].usUsagePage = 0x000D;
                devices[idx].usUsage = usage;
                devices[idx].dwFlags = EXSINK;
                devices[idx].hwndTarget = this.Handle;
                idx++;

                // Also with RIDEV_INPUTSINK (some devices respond to one but not the other)
                devices[idx].usUsagePage = 0x000D;
                devices[idx].usUsage = usage;
                devices[idx].dwFlags = INPUTSINK;
                devices[idx].hwndTarget = this.Handle;
                idx++;
            }

            uint cbSize = (uint)Marshal.SizeOf(typeof(NativeMethods.RAWINPUTDEVICE));
            bool ok = NativeMethods.RegisterRawInputDevices(devices, (uint)idx, cbSize);
            int err = Marshal.GetLastWin32Error();
            Program.Log("[Probe2] RegisterRawInputDevices ({0} entries): {1} err={2}", idx, ok ? "OK" : "FAILED", err);
        }

        protected override void WndProc(ref Message m)
        {
            if (m.Msg == NativeMethods.WM_INPUT)
            {
                _wmInputCount++;
                ProcessRawInputDetailed(m.LParam);
            }
            else if (m.Msg == NativeMethods.WM_POINTERDOWN ||
                     m.Msg == NativeMethods.WM_POINTERUPDATE ||
                     m.Msg == NativeMethods.WM_POINTERUP)
            {
                _wmPointerCount++;
                uint pointerId = (uint)m.WParam.ToInt64();

                // Try GetPointerInfo first
                var ptrInfo = new NativeMethods.POINTER_INFO();
                bool ptrOk = NativeMethods.GetPointerInfo(pointerId, ref ptrInfo);

                // Try GetPointerPenInfo
                var penInfo = new NativeMethods.POINTER_PEN_INFO();
                bool penOk = NativeMethods.GetPointerPenInfo(pointerId, ref penInfo);

                if (_wmPointerCount <= 50)
                {
                    Program.Log("[Probe2] WM_POINTER #{0} msg=0x{1:X4} ptrId={2} ptrOk={3} type={4} penOk={5} press={6} pt=({7},{8})",
                        _wmPointerCount, m.Msg, pointerId,
                        ptrOk, ptrOk ? ptrInfo.pointerType : 0,
                        penOk, penInfo.pressure,
                        penInfo.pointerInfo.ptPixelLocation.X,
                        penInfo.pointerInfo.ptPixelLocation.Y);
                }
            }
            base.WndProc(ref m);
        }

        private void ProcessRawInputDetailed(IntPtr hRawInput)
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
                int headerBytes = Marshal.SizeOf(typeof(NativeMethods.RAWINPUTHEADER));

                if (header.dwType == NativeMethods.RIM_TYPEHID)
                {
                    _hidReportCount++;
                    ParseHidRawInput(buffer, headerBytes, (int)(dwSize - headerBytes));
                }
                else if (header.dwType == NativeMethods.RIM_TYPEMOUSE)
                {
                    _mouseReportCount++;
                    if (_mouseReportCount <= 10 || _mouseReportCount % 200 == 0)
                    {
                        var mouse = (NativeMethods.RAWMOUSE)Marshal.PtrToStructure(
                            buffer + headerBytes, typeof(NativeMethods.RAWMOUSE));
                        bool isAbs = (mouse.usFlags & NativeMethods.MOUSE_MOVE_ABSOLUTE) != 0;
                        Program.Log("[Probe2] RawMouse #{0} flags=0x{1:X4} abs={2} lX={3} lY={4}",
                            _mouseReportCount, mouse.usFlags, isAbs, mouse.lLastX, mouse.lLastY);
                    }
                }
            }
            finally
            {
                Marshal.FreeHGlobal(buffer);
            }
        }

        private void ParseHidRawInput(IntPtr buffer, int offset, int dataLen)
        {
            // RAWHID header: dwSizeHid(4) + dwCount(4) = 8 bytes before raw data
            if (dataLen < 8) return;

            uint dwSizeHid = (uint)Marshal.ReadInt32(buffer, offset);
            uint dwCount = (uint)Marshal.ReadInt32(buffer, offset + 4);
            int rawDataOffset = offset + 8;
            int rawDataLen = dataLen - 8;

            if (_hidReportCount <= 20 || _hidReportCount % 100 == 0)
            {
                // Log first 16 bytes of raw HID data
                int logLen = Math.Min(rawDataLen, 16);
                byte[] preview = new byte[logLen];
                Marshal.Copy(buffer + rawDataOffset, preview, 0, logLen);
                string hex = BitConverter.ToString(preview);

                Program.Log("[Probe2] HID #{0} sizeHid={1} count={2} dataLen={3} data=[{4}]",
                    _hidReportCount, dwSizeHid, dwCount, rawDataLen, hex);
            }

            // Try parsing as digitizer report (skip reportId if present)
            if (rawDataLen >= 8)
            {
                // Standard digitizer: [switches:1][X:2 LE][Y:2 LE][pressure:2 LE]
                // With reportId prefix: [reportId:1][switches:1][X:2 LE][Y:2 LE][pressure:2 LE]
                // Try both interpretations

                ParseDigitizerData(buffer, rawDataOffset, rawDataLen, false); // no reportId
                ParseDigitizerData(buffer, rawDataOffset, rawDataLen, true);  // with reportId
            }
        }

        private void ParseDigitizerData(IntPtr buffer, int offset, int len, bool hasReportId)
        {
            int baseOff = hasReportId ? offset + 1 : offset;
            int needed = hasReportId ? 8 : 7;
            if (len < needed) return;

            byte switches = Marshal.ReadByte(buffer, baseOff);
            uint x = (uint)Marshal.ReadByte(buffer, baseOff + 1) | ((uint)Marshal.ReadByte(buffer, baseOff + 2) << 8);
            uint y = (uint)Marshal.ReadByte(buffer, baseOff + 3) | ((uint)Marshal.ReadByte(buffer, baseOff + 4) << 8);
            uint pressure = (uint)Marshal.ReadByte(buffer, baseOff + 5) | ((uint)Marshal.ReadByte(buffer, baseOff + 6) << 8);

            bool tipDown = (switches & 0x01) != 0;

            // Only log if looks like valid digitizer data
            if (x > 0 && x < 65536 && y > 0 && y < 65536)
            {
                string prefix = hasReportId ? "withRptId" : "noRptId";
                Program.Log("[Probe2] {0} sw=0x{1:X2} x={2} y={3} press={4} tip={5}",
                    prefix, switches, x, y, pressure, tipDown);
            }
        }

        private void ProbeHidDevicesFixed()
        {
            // Same as before but with corrected report ID handling
            Guid hidGuid;
            NativeMethods.HidD_GetHidGuid(out hidGuid);

            IntPtr devInfo = NativeMethods.SetupDiGetClassDevs(
                ref hidGuid, null, IntPtr.Zero,
                NativeMethods.DIGCF_PRESENT | NativeMethods.DIGCF_DEVICEINTERFACE);

            if (devInfo == IntPtr.Zero || devInfo == new IntPtr(-1))
            {
                Program.Log("[Probe2] SetupDiGetClassDevs FAILED");
                return;
            }

            try
            {
                int structSize = Marshal.SizeOf(typeof(NativeMethods.SP_DEVICE_INTERFACE_DATA));
                uint index = 0;
                int openedCount = 0;

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

                    string devicePath = GetDevicePath(devInfo, ref ifData);
                    Marshal.FreeHGlobal(ifDataPtr);

                    if (devicePath != null)
                    {
                        TryOpenAndRead(devicePath, index);
                    }

                    index++;
                }
                Program.Log("[Probe2] Enumerated {0} devices, opened {1}", index, openedCount);
            }
            finally
            {
                NativeMethods.SetupDiDestroyDeviceInfoList(devInfo);
            }
        }

        private string GetDevicePath(IntPtr devInfo, ref NativeMethods.SP_DEVICE_INTERFACE_DATA ifData)
        {
            uint requiredSize = 0;
            NativeMethods.SetupDiGetDeviceInterfaceDetail(
                devInfo, ref ifData, IntPtr.Zero, 0, ref requiredSize, IntPtr.Zero);
            if (requiredSize == 0) return null;

            IntPtr detailData = Marshal.AllocHGlobal((int)requiredSize);
            try
            {
                if (IntPtr.Size == 8) Marshal.WriteInt32(detailData, 8);
                else Marshal.WriteInt32(detailData, 6);

                if (!NativeMethods.SetupDiGetDeviceInterfaceDetail(
                    devInfo, ref ifData, detailData, requiredSize, ref requiredSize, IntPtr.Zero))
                    return null;

                return Marshal.PtrToStringUni(detailData + 4);
            }
            finally
            {
                Marshal.FreeHGlobal(detailData);
            }
        }

        private void TryOpenAndRead(string devicePath, uint index)
        {
            // Try multiple approaches
            // 1. CreateFile with various access modes
            // 2. NtCreateFile with minimal access
            // 3. DeviceIoControl for direct HID report reading

            uint[][] modes = {
                new uint[] { NativeMethods.GENERIC_READ, NativeMethods.FILE_SHARE_READ | NativeMethods.FILE_SHARE_WRITE },
                new uint[] { 0x00000001, NativeMethods.FILE_SHARE_READ | NativeMethods.FILE_SHARE_WRITE },
                new uint[] { 0, NativeMethods.FILE_SHARE_READ | NativeMethods.FILE_SHARE_WRITE },
                // Additional combinations
                new uint[] { 0x00100000, NativeMethods.FILE_SHARE_READ | NativeMethods.FILE_SHARE_WRITE }, // MAXIMUM_ALLOWED
                new uint[] { 0x00000020, NativeMethods.FILE_SHARE_READ | NativeMethods.FILE_SHARE_WRITE }, // FILE_EXECUTE
                new uint[] { 0x00120089, NativeMethods.FILE_SHARE_READ | NativeMethods.FILE_SHARE_WRITE }, // FILE_GENERIC_READ
            };

            foreach (var mode in modes)
            {
                IntPtr handle = NativeMethods.CreateFile(
                    devicePath, mode[0], mode[1],
                    IntPtr.Zero, NativeMethods.OPEN_EXISTING, 0, IntPtr.Zero);

                if (handle == IntPtr.Zero || handle == new IntPtr(-1))
                    continue;

                int err = Marshal.GetLastWin32Error();
                Program.Log("[Probe2] Device #{0} OPENED! access=0x{1:X8} err={2}", index, mode[0], err);

                // Try HidD_GetAttributes (works without preparsed data)
                TryGetAttributes(handle, index);

                // Try direct ReadFile (skip HidP_GetCaps entirely)
                TryDirectRead(handle, index);

                TryReadFromHandle(handle, index, devicePath);
                NativeMethods.CloseHandle(handle);
                return;
            }

            // All CreateFile attempts failed - try NtCreateFile
            TryNtCreateFile(devicePath, index);
        }

        private void TryNtCreateFile(string devicePath, uint index)
        {
            try
            {
                // Convert \\?\ to \??\ for NtCreateFile
                string ntPath = devicePath.Replace("\\\\?\\", "\\??\\");

                // Initialize UNICODE_STRING
                var objectName = new NativeMethods.UNICODE_STRING();
                objectName.Length = (ushort)(ntPath.Length * 2);
                objectName.MaximumLength = (ushort)((ntPath.Length + 1) * 2);
                objectName.Buffer = Marshal.StringToHGlobalUni(ntPath);

                // Initialize OBJECT_ATTRIBUTES
                var objectAttributes = new NativeMethods.OBJECT_ATTRIBUTES();
                objectAttributes.Length = Marshal.SizeOf(typeof(NativeMethods.OBJECT_ATTRIBUTES));
                objectAttributes.RootDirectory = IntPtr.Zero;
                objectAttributes.ObjectName = Marshal.AllocHGlobal(Marshal.SizeOf(typeof(NativeMethods.UNICODE_STRING)));
                Marshal.StructureToPtr(objectName, objectAttributes.ObjectName, false);
                objectAttributes.Attributes = 0x00000040; // OBJ_CASE_INSENSITIVE
                objectAttributes.SecurityDescriptor = IntPtr.Zero;
                objectAttributes.SecurityQualityOfService = IntPtr.Zero;

                var ioStatusBlock = new NativeMethods.IO_STATUS_BLOCK();

                // Try different access masks
                uint[] ntAccessModes = { 0x00100000, 0x00120089, 0x00100080, 0x00000001, 0 };

                foreach (uint access in ntAccessModes)
                {
                    IntPtr handle;
                    int status = NativeMethods.NtCreateFile(
                        out handle,
                        access,
                        ref objectAttributes,
                        ref ioStatusBlock,
                        IntPtr.Zero,
                        0x00000080, // FILE_ATTRIBUTE_NORMAL
                        NativeMethods.FILE_SHARE_READ | NativeMethods.FILE_SHARE_WRITE,
                        0x00000001, // FILE_OPEN
                        0x00000060, // FILE_SYNCHRONOUS_IO_NONALERT | FILE_NON_DIRECTORY_FILE
                        IntPtr.Zero, 0);

                    if (status >= 0) // NT_SUCCESS
                    {
                        Program.Log("[Probe2] Device #{0} NT OPENED! access=0x{1:X8} status=0x{2:X8}",
                            index, access, status);
                        TryReadFromHandle(handle, index, devicePath);
                        NativeMethods.CloseHandle(handle);
                        Marshal.FreeHGlobal(objectAttributes.ObjectName);
                        Marshal.FreeHGlobal(objectName.Buffer);
                        return;
                    }
                }

                Program.Log("[Probe2] Device #{0} all NT open attempts failed", index);
                Marshal.FreeHGlobal(objectAttributes.ObjectName);
                Marshal.FreeHGlobal(objectName.Buffer);
            }
            catch (Exception ex)
            {
                Program.Log("[Probe2] Device #{0} NtCreateFile exception: {1}", index, ex.Message);
            }
        }

        private void TryGetAttributes(IntPtr handle, uint index)
        {
            // HidD_GetAttributes works without preparsed data
            var attrs = new NativeMethods.HIDD_ATTRIBUTES();
            attrs.Size = Marshal.SizeOf(typeof(NativeMethods.HIDD_ATTRIBUTES));
            bool ok = NativeMethods.HidD_GetAttributes(handle, ref attrs);
            if (ok)
            {
                Program.Log("[Probe2] Device #{0} VID=0x{1:X4} PID=0x{2:X4} Version={3}",
                    index, attrs.VendorID, attrs.ProductID, attrs.VersionNumber);
            }
        }

        private void TryDirectRead(IntPtr handle, uint index)
        {
            // Try ReadFile directly with various buffer sizes
            // Common HID report sizes: 8, 16, 32, 64 bytes
            int[] sizes = { 8, 16, 32, 64, 128 };

            foreach (int size in sizes)
            {
                IntPtr buf = Marshal.AllocHGlobal(size);
                try
                {
                    Marshal.Copy(new byte[size], 0, buf, size);
                    uint bytesRead = 0;
                    NativeOverlapped overlapped = new NativeOverlapped();
                    bool ok = NativeMethods.ReadFile(handle, buf, (uint)size, ref bytesRead, ref overlapped);
                    if (ok && bytesRead > 0)
                    {
                        byte[] data = new byte[bytesRead];
                        Marshal.Copy(buf, data, 0, (int)bytesRead);
                        string hex = BitConverter.ToString(data, 0, Math.Min((int)bytesRead, 32));
                        Program.Log("[Probe2] Device #{0} DIRECT READ {1} bytes (buf={2}): [{3}]",
                            index, bytesRead, size, hex);
                        return; // found the right size
                    }
                }
                finally
                {
                    Marshal.FreeHGlobal(buf);
                }
            }

            Program.Log("[Probe2] Device #{0} all direct reads failed (no data or wrong size)", index);
        }

        private void TryReadFromHandle(IntPtr handle, uint index, string devicePath)
        {
            IntPtr preparsed;
            if (!NativeMethods.HidD_GetPreparsedData(handle, out preparsed))
            {
                Program.Log("[Probe2] Device #{0} HidD_GetPreparsedData FAILED", index);
                return;
            }

            var caps = new NativeMethods.HIDP_CAPS();
            uint status = NativeMethods.HidP_GetCaps(preparsed, ref caps);
            if (status != 0)
            {
                Program.Log("[Probe2] Device #{0} HidP_GetCaps FAILED 0x{1:X8}", index, status);
                NativeMethods.HidD_FreePreparsedData(preparsed);
                return;
            }

            Program.Log("[Probe2] Device #{0} UsagePage=0x{1:X4} Usage=0x{2:X4} ReportLen={3}",
                index, caps.UsagePage, caps.Usage, caps.InputReportByteLength);

            if (caps.UsagePage == 0x000D)
            {
                Program.Log("[Probe2] *** DIGITIZER Device #{0} ***", index);
                TryReadWithReportId(handle, caps.InputReportByteLength, index);

                // Also try DeviceIoControl for direct report reading
                TryDeviceIoControl(handle, caps.InputReportByteLength, index);
            }

            NativeMethods.HidD_FreePreparsedData(preparsed);
        }

        private void TryDeviceIoControl(IntPtr handle, ushort reportLen, uint index)
        {
            // IOCTL_HID_READ_REPORT = 0xB01A2
            const uint IOCTL_HID_READ_REPORT = 0x000B01A2;

            byte[] buffer = new byte[reportLen + 1]; // +1 for report ID
            IntPtr unmanaged = Marshal.AllocHGlobal(buffer.Length);
            try
            {
                uint bytesReturned = 0;
                bool ok = NativeMethods.DeviceIoControl(
                    handle,
                    IOCTL_HID_READ_REPORT,
                    IntPtr.Zero, 0,
                    unmanaged, (uint)buffer.Length,
                    ref bytesReturned,
                    IntPtr.Zero);

                if (ok && bytesReturned > 0)
                {
                    Marshal.Copy(unmanaged, buffer, 0, (int)bytesReturned);
                    string hex = BitConverter.ToString(buffer, 0, Math.Min((int)bytesReturned, 16));
                    Program.Log("[Probe2] Device #{0} IOCTL READ {1} bytes: [{2}]", index, bytesReturned, hex);
                }
                else
                {
                    Program.Log("[Probe2] Device #{0} IOCTL READ failed err={1}", index, Marshal.GetLastWin32Error());
                }
            }
            finally
            {
                Marshal.FreeHGlobal(unmanaged);
            }
        }

        private void TryReadWithReportId(IntPtr handle, ushort reportLen, uint deviceIndex)
        {
            // Read with report ID prefix
            byte[] buffer = new byte[reportLen];
            IntPtr unmanaged = Marshal.AllocHGlobal(reportLen);
            try
            {
                for (int i = 0; i < 5; i++) // try 5 reads
                {
                    Marshal.Copy(new byte[reportLen], 0, unmanaged, reportLen);
                    uint bytesRead = 0;
                    NativeOverlapped overlapped = new NativeOverlapped();
                    bool ok = NativeMethods.ReadFile(handle, unmanaged, reportLen, ref bytesRead, ref overlapped);
                    if (ok && bytesRead > 0)
                    {
                        Marshal.Copy(unmanaged, buffer, 0, (int)bytesRead);
                        string hex = BitConverter.ToString(buffer, 0, Math.Min((int)bytesRead, 16));
                        Program.Log("[Probe2] Device #{0} READ {1} bytes: [{2}]", deviceIndex, bytesRead, hex);
                    }
                    else
                    {
                        Program.Log("[Probe2] Device #{0} ReadFile err={1}", deviceIndex, Marshal.GetLastWin32Error());
                        break;
                    }
                    Thread.Sleep(50);
                }
            }
            finally
            {
                Marshal.FreeHGlobal(unmanaged);
            }
        }
    }
}
