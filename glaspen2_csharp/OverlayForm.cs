using System;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Windows.Forms;

namespace GlasPen2
{
    public class OverlayForm : Form
    {
        private Bitmap _canvas;
        private Graphics _g;

        // HID pen state
        private int _hidCount;
        private bool _tipDown;
        private bool _inRange;
        private uint _pressure;
        private int _screenX, _screenY;

        // HID coordinate range from device descriptor
        private int _hidMinX, _hidMaxX;
        private int _hidMinY, _hidMaxY;
        private bool _rangeFound;

        // Drawing state
        private bool _isDrawing;
        private Point _lastPoint;
        private Color _penColor = Color.Red;
        private float _penWidth = 2.5f;
        private float _currentWidth;

        // Smooth curve: collect recent points for spline interpolation
        private readonly List<Point> _recentPoints = new List<Point>();
        private const int MAX_RECENT = 8; // rolling window for curve smoothing

        // Cursor
        private IntPtr _transparentCursor = IntPtr.Zero;
        private bool _showCursor;

        // Block mode: toggle WS_EX_TRANSPARENT to block/allow pen+mouse passthrough
        private bool _isBlocking = false; // start in transparent (pass-through) mode

        private static void Log(string msg) { Program.Log(msg); }
        private static void Log(string fmt, params object[] args) { Program.Log(fmt, args); }

        protected override CreateParams CreateParams
        {
            get
            {
                var cp = base.CreateParams;
                // Start in BLOCKING mode (no WS_EX_TRANSPARENT) — pen draws on overlay
                cp.ExStyle |= NativeMethods.WS_EX_NOACTIVATE
                           | NativeMethods.WS_EX_TOOLWINDOW
                           | NativeMethods.WS_EX_TOPMOST;
                return cp;
            }
        }

        protected override bool ShowWithoutActivation { get { return true; } }

        public OverlayForm()
        {
            var bounds = SystemInformation.VirtualScreen;
            this.StartPosition = FormStartPosition.Manual;
            this.Location = bounds.Location;
            this.Size = bounds.Size;
            this.FormBorderStyle = FormBorderStyle.None;
            this.ShowInTaskbar = false;
            this.TopMost = true;
            this.ShowIcon = false;
            this.BackColor = Color.Black;
            this.Opacity = 0.01; // nearly invisible but blocks input
            this.DoubleBuffered = true;

            _canvas = new Bitmap(bounds.Width, bounds.Height, PixelFormat.Format32bppArgb);
            _g = Graphics.FromImage(_canvas);
            _g.SmoothingMode = SmoothingMode.AntiAlias;
            _g.CompositingQuality = CompositingQuality.HighQuality;
            _g.InterpolationMode = InterpolationMode.HighQualityBicubic;
            _g.PixelOffsetMode = PixelOffsetMode.HighQuality;
            _g.Clear(Color.Transparent);

            byte[] andPlane = { 0xFF };
            byte[] xorPlane = { 0x00 };
            _transparentCursor = NativeMethods.CreateCursor(IntPtr.Zero, 0, 0, 1, 1, andPlane, xorPlane);
            while (NativeMethods.ShowCursor(false) >= 0) { }

            Log("[Overlay] Canvas: {0}x{1}, Location=({2},{3}), penWidth={4}",
                bounds.Width, bounds.Height, bounds.Left, bounds.Top, _penWidth);
        }

        protected override void OnHandleCreated(EventArgs e)
        {
            base.OnHandleCreated(e);

            // Probe HID digitizer devices for coordinate range (after pipe is connected)
            ProbeDigitizerDevices();

            RegisterRawInput();
            NativeMethods.SetWindowPos(this.Handle, NativeMethods.HWND_TOPMOST,
                this.Left, this.Top, this.Width, this.Height,
                NativeMethods.SWP_NOACTIVATE | NativeMethods.SWP_SHOWWINDOW);
            NativeMethods.RegisterHotKey(this.Handle, 1,
                NativeMethods.MOD_CONTROL | NativeMethods.MOD_ALT, (uint)Keys.C);
            NativeMethods.RegisterHotKey(this.Handle, 2,
                NativeMethods.MOD_CONTROL | NativeMethods.MOD_ALT, (uint)Keys.Q);
            NativeMethods.RegisterHotKey(this.Handle, 3,
                NativeMethods.MOD_CONTROL | NativeMethods.MOD_ALT, (uint)Keys.B);
            Log("[Overlay] Ready. Handle=0x{0:X}, Mode=BLOCKING (no WS_EX_TRANSPARENT)", this.Handle.ToInt64());
        }

        // Enumerate HID digitizer devices and read their coordinate ranges
        private void ProbeDigitizerDevices()
        {
            Log("[Probe] Starting device enumeration...");
            // Try to enumerate devices via SetupAPI
            try
            {
                Guid hidGuid;
                NativeMethods.HidD_GetHidGuid(out hidGuid);
                Log("[Probe] HID GUID: {0}", hidGuid);

                IntPtr devInfoSet = NativeMethods.SetupDiGetClassDevs(
                    ref hidGuid, null, IntPtr.Zero,
                    NativeMethods.DIGCF_PRESENT | NativeMethods.DIGCF_DEVICEINTERFACE);

                if (devInfoSet == IntPtr.Zero || devInfoSet == new IntPtr(-1))
                {
                    Log("[Probe] SetupDiGetClassDevs failed");
                    return;
                }

                try
                {
                    var ifaceData = new NativeMethods.SP_DEVICE_INTERFACE_DATA();
                    ifaceData.cbSize = Marshal.SizeOf(typeof(NativeMethods.SP_DEVICE_INTERFACE_DATA));

                    for (uint i = 0; NativeMethods.SetupDiEnumDeviceInterfaces(
                        devInfoSet, IntPtr.Zero, ref hidGuid, i, ref ifaceData); i++)
                    {
                        // Get required size for detail data
                        uint detailSize = 0;
                        NativeMethods.SetupDiGetDeviceInterfaceDetail(
                            devInfoSet, ref ifaceData, IntPtr.Zero, 0, ref detailSize, IntPtr.Zero);
                        if (detailSize == 0) continue;

                        IntPtr detailBuf = Marshal.AllocHGlobal((int)detailSize);
                        try
                        {
                            // First 4 bytes (or 8 on x64) are cbSize
                            if (IntPtr.Size == 8)
                                Marshal.WriteInt32(detailBuf, 8);
                            else
                                Marshal.WriteInt32(detailBuf, 4 + Marshal.SizeOf(typeof(char)));

                            if (!NativeMethods.SetupDiGetDeviceInterfaceDetail(
                                devInfoSet, ref ifaceData, detailBuf, detailSize, ref detailSize, IntPtr.Zero))
                                continue;

                            string devicePath = Marshal.PtrToStringUni(detailBuf + IntPtr.Size) ?? "";
                            if (string.IsNullOrEmpty(devicePath)) continue;

                            // Only process digitizer devices (UsagePage 0x000D)
                            if (!devicePath.Contains("vid_") && !devicePath.Contains("VID_"))
                                continue;

                            TryReadDeviceRange(devicePath);
                            if (_rangeFound) break;
                        }
                        finally
                        {
                            Marshal.FreeHGlobal(detailBuf);
                        }
                    }
                }
                finally
                {
                    NativeMethods.SetupDiDestroyDeviceInfoList(devInfoSet);
                }
            }
            catch (Exception ex)
            {
                Log("[Probe] EXCEPTION: {0} ({1})", ex.Message, ex.GetType().Name);
            }

            Log("[Probe] Done. rangeFound={0}", _rangeFound);

            if (!_rangeFound)
            {
                Log("[Probe] Could not find digitizer range — using fallback 0-32767");
                _hidMinX = 0; _hidMaxX = 32767;
                _hidMinY = 0; _hidMaxY = 32767;
                _rangeFound = true;
            }
        }

        private void TryReadDeviceRange(string devicePath)
        {
            IntPtr devHandle = NativeMethods.CreateFile(
                devicePath,
                0, // no access — just need preparsed data
                NativeMethods.FILE_SHARE_READ | NativeMethods.FILE_SHARE_WRITE,
                IntPtr.Zero,
                NativeMethods.OPEN_EXISTING,
                0, IntPtr.Zero);

            if (devHandle == IntPtr.Zero || devHandle == new IntPtr(-1))
                return;

            try
            {
                IntPtr preparsed;
                if (!NativeMethods.HidD_GetPreparsedData(devHandle, out preparsed))
                    return;

                try
                {
                    var caps = new NativeMethods.HIDP_CAPS();
                    uint status = NativeMethods.HidP_GetCaps(preparsed, ref caps);
                    if (status != 0) return;

                    // Only process digitizer devices (UsagePage 0x000D)
                    if (caps.UsagePage != 0x000D)
                        return;

                    Log("[Probe] Found digitizer: UsagePage=0x{0:X4} Usage=0x{1:X4} Path={2}",
                        caps.UsagePage, caps.Usage, devicePath.Substring(0, Math.Min(80, devicePath.Length)));

                    ushort numCaps = caps.NumberInputValueCaps;
                    if (numCaps > 0 && numCaps < 20)
                    {
                        int capsSize = Marshal.SizeOf(typeof(NativeMethods.HIDP_VALUE_CAPS));
                        IntPtr valueCaps = Marshal.AllocHGlobal(numCaps * capsSize);
                        try
                        {
                            status = NativeMethods.HidP_GetValueCaps(0, valueCaps, ref numCaps, preparsed);
                            if (status == 0)
                            {
                                for (int j = 0; j < numCaps; j++)
                                {
                                    var vc = (NativeMethods.HIDP_VALUE_CAPS)Marshal.PtrToStructure(
                                        valueCaps + j * capsSize, typeof(NativeMethods.HIDP_VALUE_CAPS));

                                    if (vc.UsagePage == 0x0001 && vc.LogicalMax > vc.LogicalMin)
                                    {
                                        if (vc.UsageMin == 0x30) // X
                                        {
                                            _hidMinX = (int)vc.LogicalMin;
                                            _hidMaxX = (int)vc.LogicalMax;
                                            Log("[Probe]   X: {0} - {1}", _hidMinX, _hidMaxX);
                                        }
                                        else if (vc.UsageMin == 0x31) // Y
                                        {
                                            _hidMinY = (int)vc.LogicalMin;
                                            _hidMaxY = (int)vc.LogicalMax;
                                            Log("[Probe]   Y: {0} - {1}", _hidMinY, _hidMaxY);
                                        }
                                    }
                                }
                                _rangeFound = (_hidMaxX > _hidMinX && _hidMaxY > _hidMinY);
                            }
                        }
                        finally
                        {
                            Marshal.FreeHGlobal(valueCaps);
                        }
                    }
                }
                finally
                {
                    NativeMethods.HidD_FreePreparsedData(preparsed);
                }
            }
            finally
            {
                NativeMethods.CloseHandle(devHandle);
            }
        }

        private void RegisterRawInput()
        {
            var devices = new NativeMethods.RAWINPUTDEVICE[3];
            devices[0].usUsagePage = 0x0001; devices[0].usUsage = 0x0002;
            devices[0].dwFlags = NativeMethods.RIDEV_INPUTSINK; devices[0].hwndTarget = this.Handle;
            devices[1].usUsagePage = 0x000D; devices[1].usUsage = 0x0002;
            devices[1].dwFlags = NativeMethods.RIDEV_INPUTSINK; devices[1].hwndTarget = this.Handle;
            devices[2].usUsagePage = 0x000D; devices[2].usUsage = 0x0001;
            devices[2].dwFlags = NativeMethods.RIDEV_INPUTSINK; devices[2].hwndTarget = this.Handle;

            uint cbSize = (uint)Marshal.SizeOf(typeof(NativeMethods.RAWINPUTDEVICE));
            bool ok = NativeMethods.RegisterRawInputDevices(devices, 3, cbSize);
            Log("[Overlay] RegisterRawInput: {0} (err={1})", ok ? "OK" : "FAIL", Marshal.GetLastWin32Error());
        }

        protected override void WndProc(ref Message m)
        {
            if (m.Msg == NativeMethods.WM_TABLET_QUERYSYSTEMGESTURESTATUS)
            {
                m.Result = (IntPtr)NativeMethods.TABLET_DISABLE_ALL;
                return;
            }
            if (m.Msg == NativeMethods.WM_SETCURSOR)
            {
                int ht = (int)(m.LParam.ToInt64() & 0xFFFF);
                if (ht == NativeMethods.HTCLIENT && _transparentCursor != IntPtr.Zero)
                {
                    NativeMethods.SetCursor(_transparentCursor);
                    m.Result = (IntPtr)1;
                    return;
                }
            }
            if (m.Msg == NativeMethods.WM_INPUT)
            {
                ProcessRawInput(m.LParam);
            }
            else if (m.Msg == NativeMethods.WM_HOTKEY)
            {
                int id = (int)m.WParam;
                if (id == 1) ClearAll();
                else if (id == 2) Application.Exit();
                else if (id == 3) ToggleBlockMode();
            }
            base.WndProc(ref m);
        }

        private void ProcessRawInput(IntPtr hRawInput)
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
                    ProcessHidInput(buffer, headerBytes, (int)(dwSize - headerBytes));
                }
            }
            finally
            {
                Marshal.FreeHGlobal(buffer);
            }
        }

        private void ProcessHidInput(IntPtr buffer, int offset, int dataLen)
        {
            _hidCount++;
            if (dataLen < 8) return;

            int b = offset + 8;
            byte switches = Marshal.ReadByte(buffer, b + 1);
            uint rawX = (uint)Marshal.ReadByte(buffer, b + 2) | ((uint)Marshal.ReadByte(buffer, b + 3) << 8);
            uint rawY = (uint)Marshal.ReadByte(buffer, b + 4) | ((uint)Marshal.ReadByte(buffer, b + 5) << 8);
            uint press = (uint)Marshal.ReadByte(buffer, b + 6) | ((uint)Marshal.ReadByte(buffer, b + 7) << 8);

            bool tipDown = (switches & 0x05) != 0;
            bool inRange = (switches & 0x10) != 0;

            // Map to screen coords
            var sb = SystemInformation.VirtualScreen;
            long rangeX = _hidMaxX - _hidMinX;
            long rangeY = _hidMaxY - _hidMinY;
            int sx = (rangeX > 0) ? sb.Left + (int)((long)(rawX - _hidMinX) * sb.Width / rangeX) : sb.Left;
            int sy = (rangeY > 0) ? sb.Top + (int)((long)(rawY - _hidMinY) * sb.Height / rangeY) : sb.Top;
            sx = Math.Max(sb.Left, Math.Min(sx, sb.Left + sb.Width - 1));
            sy = Math.Max(sb.Top, Math.Min(sy, sb.Top + sb.Height - 1));

            bool tipChanged = tipDown != _tipDown;
            bool rangeChanged = inRange != _inRange;

            _tipDown = tipDown;
            _inRange = inRange;
            _pressure = press;
            _screenX = sx;
            _screenY = sy;

            if (_hidCount <= 50 || tipChanged || rangeChanged || _hidCount % 100 == 0)
                Log("[HID #{0}] raw=({1},{2}) screen=({3},{4}) pressure={5} tip={6} range={7}",
                    _hidCount, rawX, rawY, sx, sy, press,
                    tipDown ? "DOWN" : "UP",
                    inRange ? "YES" : "NO");

            _showCursor = inRange && !tipDown; // show crosshair on hover, hide when drawing or out of range

            if (tipDown && press > 0)
            {
                _currentWidth = _penWidth * (0.3f + (press / 16000f) * 1.7f);
                int cx = ClampX(sx - this.Left);
                int cy = ClampY(sy - this.Top);
                var pt = new Point(cx, cy);

                if (!_isDrawing)
                {
                    _isDrawing = true;
                    _recentPoints.Clear();
                    _recentPoints.Add(pt);
                    using (var pen = new Pen(_penColor, _currentWidth))
                    {
                        pen.StartCap = LineCap.Round;
                        pen.EndCap = LineCap.Round;
                        _g.DrawEllipse(pen, cx - _currentWidth / 2, cy - _currentWidth / 2, _currentWidth, _currentWidth);
                    }
                }
                else
                {
                    _recentPoints.Add(pt);
                    if (_recentPoints.Count > MAX_RECENT)
                        _recentPoints.RemoveAt(0);

                    // Draw smooth curve through recent points
                    using (var pen = new Pen(_penColor, _currentWidth))
                    {
                        pen.StartCap = LineCap.Round;
                        pen.EndCap = LineCap.Round;
                        pen.LineJoin = LineJoin.Round;

                        if (_recentPoints.Count >= 3)
                        {
                            // Cardinal spline — smooth curve through all points
                            _g.DrawCurve(pen, _recentPoints.ToArray(), 0.5f);
                        }
                        else if (_recentPoints.Count == 2)
                        {
                            _g.DrawLine(pen, _recentPoints[0], _recentPoints[1]);
                        }
                    }
                }
                _lastPoint = pt;
                this.Invalidate();
            }
            else if (!tipDown && _isDrawing)
            {
                _recentPoints.Clear();
                _isDrawing = false;
                this.Invalidate();
            }
            else if (_showCursor)
            {
                this.Invalidate();
            }
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            if (_canvas != null) e.Graphics.DrawImage(_canvas, 0, 0);

            if (_showCursor && _screenX > 0)
            {
                int cx = ClampX(_screenX - this.Left);
                int cy = ClampY(_screenY - this.Top);
                int r = 10;
                using (var pen = new Pen(Color.FromArgb(200, 255, 80, 30), 2f))
                {
                    e.Graphics.DrawLine(pen, cx - r, cy, cx + r, cy);
                    e.Graphics.DrawLine(pen, cx, cy - r, cx, cy + r);
                    e.Graphics.DrawEllipse(pen, cx - r, cy - r, r * 2, r * 2);
                }
            }
        }

        public void ToggleBlockMode()
        {
            _isBlocking = !_isBlocking;
            int style = NativeMethods.GetWindowLong(this.Handle, NativeMethods.GWL_EXSTYLE);
            if (_isBlocking)
            {
                // Remove WS_EX_TRANSPARENT — block pen+mouse from reaching lower apps
                style &= ~NativeMethods.WS_EX_TRANSPARENT;
                NativeMethods.SetWindowLong(this.Handle, NativeMethods.GWL_EXSTYLE, style);
                Log("[Overlay] Mode=BLOCKING (pen+mouse intercepted, overlay visible)");
            }
            else
            {
                // Add WS_EX_TRANSPARENT — pen+mouse pass through to lower apps
                style |= NativeMethods.WS_EX_TRANSPARENT;
                NativeMethods.SetWindowLong(this.Handle, NativeMethods.GWL_EXSTYLE, style);
                Log("[Overlay] Mode=TRANSPARENT (pen+mouse pass through)");
            }
        }

        public void ClearAll()
        {
            _isDrawing = false;
            _g.Clear(Color.Transparent);
            this.Invalidate();
            Log("[Overlay] Cleared");
        }

        private int ClampX(int x) { return Math.Max(0, Math.Min(x, _canvas.Width - 1)); }
        private int ClampY(int y) { return Math.Max(0, Math.Min(y, _canvas.Height - 1)); }

        protected override void Dispose(bool disposing)
        {
            if (disposing)
            {
                if (this.IsHandleCreated)
                {
                    NativeMethods.UnregisterHotKey(this.Handle, 1);
                    NativeMethods.UnregisterHotKey(this.Handle, 2);
                    NativeMethods.UnregisterHotKey(this.Handle, 3);
                }
                if (_g != null) _g.Dispose();
                if (_canvas != null) _canvas.Dispose();
                if (_transparentCursor != IntPtr.Zero)
                    NativeMethods.DestroyCursor(_transparentCursor);
            }
            base.Dispose(disposing);
        }
    }
}
