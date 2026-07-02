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

        // Fake stroke form (visible green strokes below overlay)
        private FakeStrokeForm _fakeStrokeForm;
        private const int FAKE_OFFSET_X = 0;
        private const int FAKE_OFFSET_Y = 0;

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

        // Settings: preset colors and widths (matching Flutter UI)
        private static readonly Color[] PresetColors = {
            Color.FromArgb(0xDC, 0x1E, 0x1E), // 红色
            Color.FromArgb(0x1E, 0x78, 0xDC), // 蓝色
            Color.FromArgb(0x1E, 0xB4, 0x3C), // 绿色
            Color.FromArgb(0xF0, 0xA0, 0x14), // 橙色
            Color.FromArgb(0xA0, 0x50, 0xDC), // 紫色
            Color.FromArgb(0x14, 0x14, 0x14), // 黑色
            Color.FromArgb(0xFF, 0xFF, 0xFF), // 白色
        };
        private static readonly float[] PresetWidths = { 1.0f, 1.5f, 2.0f, 2.5f, 3.5f, 5.0f, 7.0f, 10.0f };
        private int _colorIndex = 0;
        private int _widthIndex = 3; // default: 中

        // Smooth curve: collect recent points for spline interpolation
        private readonly List<Point> _recentPoints = new List<Point>();
        private const int MAX_RECENT = 8; // rolling window for curve smoothing

        // Cursor
        private IntPtr _transparentCursor = IntPtr.Zero;
        private bool _showCursor;

        // Block mode: toggle WS_EX_TRANSPARENT to block/allow pen+mouse passthrough
        private bool _isBlocking = false; // start in transparent (pass-through) mode

        // Pressure display
        private PressureForm _pressureForm;

        // Auto-block delay: wait after pen lift before unblocking
        private System.Windows.Forms.Timer _unblockTimer;
        private const int UNBLOCK_DELAY_MS = 200; // 200ms delay after pen lift

        private static void Log(string msg) { Program.Log(msg); }
        private static void Log(string fmt, params object[] args) { Program.Log(fmt, args); }

        protected override CreateParams CreateParams
        {
            get
            {
                var cp = base.CreateParams;
                // Start in TRANSPARENT mode (mouse available) — auto-block on pen down
                cp.ExStyle |= NativeMethods.WS_EX_TRANSPARENT
                           | NativeMethods.WS_EX_NOACTIVATE
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
            this.Opacity = 0.01; // nearly invisible overlay
            this.DoubleBuffered = true;

            _canvas = new Bitmap(bounds.Width, bounds.Height, PixelFormat.Format32bppArgb);
            _g = Graphics.FromImage(_canvas);
            _g.SmoothingMode = SmoothingMode.AntiAlias;
            _g.CompositingQuality = CompositingQuality.HighQuality;
            _g.InterpolationMode = InterpolationMode.HighQualityBicubic;
            _g.PixelOffsetMode = PixelOffsetMode.HighQuality;
            _g.Clear(Color.Transparent);

            // Create fake stroke form (sits above overlay, WS_EX_TRANSPARENT lets input pass through)
            _fakeStrokeForm = new FakeStrokeForm(bounds);
            _fakeStrokeForm.Show();

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
            NativeMethods.RegisterHotKey(this.Handle, 4,
                NativeMethods.MOD_CONTROL | NativeMethods.MOD_ALT, (uint)Keys.G);

            // Create pressure display
            _pressureForm = new PressureForm();
            _pressureForm.Show();

            // Create unblock timer for delayed unblocking
            _unblockTimer = new System.Windows.Forms.Timer { Interval = UNBLOCK_DELAY_MS };
            _unblockTimer.Tick += (s, ev) =>
            {
                _unblockTimer.Stop();
                if (_isBlocking)
                {
                    int style = NativeMethods.GetWindowLong(this.Handle, NativeMethods.GWL_EXSTYLE);
                    style |= NativeMethods.WS_EX_TRANSPARENT;
                    NativeMethods.SetWindowLong(this.Handle, NativeMethods.GWL_EXSTYLE, style);
                    _isBlocking = false;
                    Log("[AutoBlock] OFF — delay expired, mouse available");
                }
            };

            Log("[Overlay] Ready. Handle=0x{0:X}, Mode=AUTO-BLOCK (block on pen down, {1}ms delay on pen up)", this.Handle.ToInt64(), UNBLOCK_DELAY_MS);
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
                if (id == 1) { ClearAll(); _fakeStrokeForm.ShowNotification("清屏"); }
                else if (id == 2) Application.Exit();
                else if (id == 3) { ToggleBlockMode(); _fakeStrokeForm.ShowNotification(_isBlocking ? "拦截模式" : "穿透模式"); }
                else if (id == 4) ExportGif();
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

            // Update pressure display
            if (_pressureForm != null)
            {
                _pressureForm.CurrentPressure = press;
                _pressureForm.TipDown = tipDown;
                _pressureForm.InRange = inRange;
                _pressureForm.ScreenX = sx;
                _pressureForm.ScreenY = sy;
                _pressureForm.UpdateDisplay();
                if (_hidCount <= 10 || tipChanged)
                    Log("[Pressure] Updated: P={0} tip={1} ({2},{3})", press, tipDown, sx, sy);
            }

            if (_hidCount <= 50 || tipChanged || rangeChanged || _hidCount % 100 == 0)
            {
                // Log raw bytes for debugging
                byte b0 = Marshal.ReadByte(buffer, b);
                byte b1 = Marshal.ReadByte(buffer, b + 1);
                byte b2 = Marshal.ReadByte(buffer, b + 2);
                byte b3 = Marshal.ReadByte(buffer, b + 3);
                byte b4 = Marshal.ReadByte(buffer, b + 4);
                byte b5 = Marshal.ReadByte(buffer, b + 5);
                byte b6 = Marshal.ReadByte(buffer, b + 6);
                byte b7 = Marshal.ReadByte(buffer, b + 7);
                Log("[HID #{0}] raw=({1},{2}) screen=({3},{4}) pressure={5} tip={6} range={7} bytes=[{8},{9},{10},{11},{12},{13},{14},{15}]",
                    _hidCount, rawX, rawY, sx, sy, press,
                    tipDown ? "DOWN" : "UP",
                    inRange ? "YES" : "NO",
                    b0, b1, b2, b3, b4, b5, b6, b7);
            }

            _showCursor = inRange && !tipDown; // show crosshair on hover, hide when drawing or out of range

            // Auto-block: enable blocking on hover, delay unblock when pen leaves range
            if (rangeChanged || tipChanged)
            {
                int style = NativeMethods.GetWindowLong(this.Handle, NativeMethods.GWL_EXSTYLE);
                if (inRange)
                {
                    // Pen hovering or touching — cancel any pending unblock and enable blocking
                    _unblockTimer.Stop();
                    if (!_isBlocking)
                    {
                        style &= ~NativeMethods.WS_EX_TRANSPARENT;
                        NativeMethods.SetWindowLong(this.Handle, NativeMethods.GWL_EXSTYLE, style);
                        _isBlocking = true;
                        Log("[AutoBlock] ON — pen in range, blocking lower apps");
                    }
                }
                else
                {
                    // Pen left range — clear crosshair and start delay timer before unblocking
                    _fakeStrokeForm.ClearCrosshair();
                    _unblockTimer.Start();
                    Log("[AutoBlock] PENDING — pen out of range, will unblock in {0}ms", UNBLOCK_DELAY_MS);
                }
            }

            if (tipDown && press > 0)
            {
                _currentWidth = _penWidth * (0.3f + (press / 16000f) * 1.7f);
                int cx = ClampX(sx - this.Left);
                int cy = ClampY(sy - this.Top);
                var pt = new Point(cx, cy);

                // Fake stroke offset position
                int fx = cx + FAKE_OFFSET_X;
                int fy = cy + FAKE_OFFSET_Y;

                if (!_isDrawing)
                {
                    _fakeStrokeForm.ClearCrosshair(); // remove crosshair before drawing
                    _isDrawing = true;
                    _recentPoints.Clear();
                    _recentPoints.Add(pt);
                    using (var pen = new Pen(_penColor, _currentWidth))
                    {
                        pen.StartCap = LineCap.Round;
                        pen.EndCap = LineCap.Round;
                        _g.DrawEllipse(pen, cx - _currentWidth / 2, cy - _currentWidth / 2, _currentWidth, _currentWidth);
                    }
                    // Fake stroke
                    _fakeStrokeForm.BeginStroke(fx, fy, _currentWidth);
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
                            _g.DrawCurve(pen, _recentPoints.ToArray(), 0.5f);
                        }
                        else if (_recentPoints.Count == 2)
                        {
                            _g.DrawLine(pen, _recentPoints[0], _recentPoints[1]);
                        }
                    }
                    // Fake stroke (offset)
                    _fakeStrokeForm.AddPoint(fx, fy, _currentWidth);
                }
                _lastPoint = pt;
                this.Invalidate();
            }
            else if (!tipDown && _isDrawing)
            {
                _recentPoints.Clear();
                _isDrawing = false;
                _fakeStrokeForm.EndStroke();
                this.Invalidate();
            }
            else if (_showCursor)
            {
                // Draw green crosshair on fake stroke form (with offset)
                int fx = ClampX(_screenX - this.Left) + FAKE_OFFSET_X;
                int fy = ClampY(_screenY - this.Top) + FAKE_OFFSET_Y;
                _fakeStrokeForm.DrawCrosshair(fx, fy);
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
            _fakeStrokeForm.ClearAll();
            this.Invalidate();
            Log("[Overlay] Cleared");
        }

        private int ClampX(int x) { return Math.Max(0, Math.Min(x, _canvas.Width - 1)); }
        private int ClampY(int y) { return Math.Max(0, Math.Min(y, _canvas.Height - 1)); }

        /// <summary>
        /// Export the fake stroke canvas as GIF, save to desktop, copy path to clipboard.
        /// </summary>
        private void ExportGif()
        {
            try
            {
                var canvas = _fakeStrokeForm.GetCanvas();
                if (canvas == null) { _fakeStrokeForm.ShowNotification("无画布"); return; }

                var rect = new System.Drawing.Rectangle(0, 0, canvas.Width, canvas.Height);
                var data = canvas.LockBits(rect, System.Drawing.Imaging.ImageLockMode.ReadOnly,
                    System.Drawing.Imaging.PixelFormat.Format32bppArgb);

                var pathBuf = new char[260];
                var pathPtr = Marshal.UnsafeAddrOfPinnedArrayElement(pathBuf, 0);

                int ok = NativeMethods.glaspen2_save_gif_from_pixels(
                    data.Scan0, canvas.Width, canvas.Height, data.Stride,
                    pathPtr, 260);

                canvas.UnlockBits(data);

                if (ok == 1)
                {
                    string path = new string(pathBuf).TrimEnd('\0');
                    if (!string.IsNullOrEmpty(path))
                    {
                        CopyGifToClipboard(path, canvas);
                    }
                    _fakeStrokeForm.ShowNotification("已导出 GIF");
                    Log("[Export] GIF saved: {0}", path);
                }
                else
                {
                    _fakeStrokeForm.ShowNotification("导出失败");
                }
            }
            catch (Exception ex)
            {
                _fakeStrokeForm.ShowNotification("导出错误");
                Log("[Export] Error: {0}", ex.Message);
            }
        }

        /// <summary>
        /// Copy GIF file to clipboard as CF_HDROP (file reference) + CF_DIB (bitmap preview).
        /// </summary>
        private void CopyGifToClipboard(string filePath, Bitmap canvas)
        {
            try
            {
                NativeMethods.OpenClipboard(this.Handle);
                NativeMethods.EmptyClipboard();

                // CF_HDROP: copy file reference (preserves GIF animation when pasting)
                CopyFileToClipboard(filePath);

                // CF_DIB: also put a bitmap snapshot (for apps that expect image data)
                CopyBitmapToClipboard(canvas);

                NativeMethods.CloseClipboard();
            }
            catch (Exception ex)
            {
                try { NativeMethods.CloseClipboard(); } catch { }
                Log("[Clipboard] Error: {0}", ex.Message);
            }
        }

        private void CopyFileToClipboard(string filePath)
        {
            // DROPFILES struct: { pFiles(4), pt(8), fNC(4), fWide(4) } = 20 bytes
            // Followed by double-null-terminated UTF-16 file path
            byte[] pathBytes = System.Text.Encoding.Unicode.GetBytes(filePath);
            int totalSize = 20 + pathBytes.Length + 2; // +2 for double null terminator

            IntPtr hMem = NativeMethods.GlobalAlloc(NativeMethods.GMEM_MOVEABLE, (UIntPtr)totalSize);
            if (hMem == IntPtr.Zero) return;

            IntPtr ptr = NativeMethods.GlobalLock(hMem);
            if (ptr == IntPtr.Zero) { NativeMethods.GlobalUnlock(hMem); return; }

            // DROPFILES.pFiles = 20 (offset to file list)
            System.Runtime.InteropServices.Marshal.WriteInt32(ptr, 0, 20);
            // DROPFILES.pt = (0,0)
            System.Runtime.InteropServices.Marshal.WriteInt32(ptr, 4, 0);
            System.Runtime.InteropServices.Marshal.WriteInt32(ptr, 8, 0);
            // DROPFILES.fNC = 0 (client coords)
            System.Runtime.InteropServices.Marshal.WriteInt32(ptr, 12, 0);
            // DROPFILES.fWide = TRUE (Unicode)
            System.Runtime.InteropServices.Marshal.WriteInt32(ptr, 16, 1);

            // File path (UTF-16, null terminated)
            System.Runtime.InteropServices.Marshal.Copy(pathBytes, 0, ptr + 20, pathBytes.Length);
            // Double null terminator
            System.Runtime.InteropServices.Marshal.WriteInt16(ptr, 20 + pathBytes.Length, 0);

            NativeMethods.GlobalUnlock(hMem);
            NativeMethods.SetClipboardData(NativeMethods.CF_HDROP, hMem);
        }

        private void CopyBitmapToClipboard(Bitmap canvas)
        {
            // Create a cropped copy (non-premultiplied for clipboard)
            // Find bounding box of non-transparent pixels
            int minX = canvas.Width, minY = canvas.Height, maxX = 0, maxY = 0;
            bool found = false;
            for (int y = 0; y < canvas.Height; y++)
            {
                for (int x = 0; x < canvas.Width; x++)
                {
                    var px = canvas.GetPixel(x, y);
                    if (px.A > 0)
                    {
                        if (x < minX) minX = x;
                        if (y < minY) minY = y;
                        if (x > maxX) maxX = x;
                        if (y > maxY) maxY = y;
                        found = true;
                    }
                }
            }
            if (!found) return;

            int w = maxX - minX + 1;
            int h = maxY - minY + 1;
            using (var cropped = canvas.Clone(new System.Drawing.Rectangle(minX, minY, w, h),
                System.Drawing.Imaging.PixelFormat.Format32bppArgb))
            {
                // Convert to CF_DIB
                var rect = new System.Drawing.Rectangle(0, 0, w, h);
                var bmpData = cropped.LockBits(rect,
                    System.Drawing.Imaging.ImageLockMode.ReadOnly,
                    System.Drawing.Imaging.PixelFormat.Format32bppArgb);

                int headerSize = 40; // BITMAPINFOHEADER
                int imageSize = bmpData.Stride * h;
                int totalSize = headerSize + imageSize;

                IntPtr hMem = NativeMethods.GlobalAlloc(NativeMethods.GMEM_MOVEABLE, (UIntPtr)totalSize);
                if (hMem != IntPtr.Zero)
                {
                    IntPtr ptr = NativeMethods.GlobalLock(hMem);
                    if (ptr != IntPtr.Zero)
                    {
                        // BITMAPINFOHEADER
                        System.Runtime.InteropServices.Marshal.WriteInt32(ptr, 0, headerSize);
                        System.Runtime.InteropServices.Marshal.WriteInt32(ptr, 4, w);
                        System.Runtime.InteropServices.Marshal.WriteInt32(ptr, 8, h);
                        System.Runtime.InteropServices.Marshal.WriteInt16(ptr, 12, 1); // planes
                        System.Runtime.InteropServices.Marshal.WriteInt16(ptr, 14, 32); // bpp
                        System.Runtime.InteropServices.Marshal.WriteInt32(ptr, 16, 0); // BI_RGB
                        System.Runtime.InteropServices.Marshal.WriteInt32(ptr, 20, imageSize);

                        // Pixel data (BGRA, bottom-up)
                        for (int y = 0; y < h; y++)
                        {
                            IntPtr src = bmpData.Scan0 + (h - 1 - y) * bmpData.Stride;
                            IntPtr dst = ptr + headerSize + y * bmpData.Stride;
                            NativeMethods.CopyMemory(dst, src, (uint)bmpData.Stride);
                        }

                        NativeMethods.GlobalUnlock(hMem);
                        NativeMethods.SetClipboardData(8 /* CF_DIB */, hMem); // CF_DIB = 8
                    }
                    else
                    {
                        NativeMethods.GlobalUnlock(hMem);
                    }
                }

                cropped.UnlockBits(bmpData);
            }
        }

        /// <summary>
        /// Called by SettingsPipeServer when a setting changes from Flutter UI.
        /// </summary>
        public void UpdateSetting(string key, object value)
        {
            try
            {
                int intVal = Convert.ToInt32(value);
                if (key == "color" && intVal >= 0 && intVal < PresetColors.Length)
                {
                    _colorIndex = intVal;
                    _penColor = PresetColors[_colorIndex];
                    _fakeStrokeForm.SetColor(_penColor);
                    Log("[Settings] Color={0} ({1})", _colorIndex, _penColor);
                }
                else if (key == "width" && intVal >= 0 && intVal < PresetWidths.Length)
                {
                    _widthIndex = intVal;
                    _penWidth = PresetWidths[_widthIndex];
                    Log("[Settings] Width={0} ({1})", _widthIndex, _penWidth);
                }
            }
            catch (Exception ex)
            {
                Log("[Settings] Error: {0}", ex.Message);
            }
        }

        /// <summary>
        /// Returns current settings for the Flutter UI.
        /// </summary>
        public Dictionary<string, object> GetSettings()
        {
            return new Dictionary<string, object>
            {
                { "color", _colorIndex },
                { "width", _widthIndex },
            };
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing)
            {
                if (this.IsHandleCreated)
                {
                    NativeMethods.UnregisterHotKey(this.Handle, 1);
                    NativeMethods.UnregisterHotKey(this.Handle, 2);
                    NativeMethods.UnregisterHotKey(this.Handle, 3);
                    NativeMethods.UnregisterHotKey(this.Handle, 4);
                }
                if (_unblockTimer != null) { _unblockTimer.Stop(); _unblockTimer.Dispose(); }
                if (_pressureForm != null) { _pressureForm.Close(); _pressureForm.Dispose(); }
                if (_g != null) _g.Dispose();
                if (_canvas != null) _canvas.Dispose();
                if (_fakeStrokeForm != null) { _fakeStrokeForm.Close(); _fakeStrokeForm.Dispose(); }
                if (_transparentCursor != IntPtr.Zero)
                    NativeMethods.DestroyCursor(_transparentCursor);
            }
            base.Dispose(disposing);
        }
    }
}
