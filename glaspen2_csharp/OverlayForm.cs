using System;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Windows.Forms;

namespace GlasPen2
{
    /// <summary>
    /// Full-screen transparent overlay using WinForms TransparencyKey
    /// instead of UpdateLayeredWindow (which isn't rendering on this system).
    /// All Fuchsia pixels are transparent; ink is drawn in other colors.
    /// </summary>
    public class OverlayForm : Form
    {
        private Bitmap _canvas;
        private Graphics _canvasGraphics;

        private bool _isDrawing;
        private Point _lastPoint;
        private readonly List<StrokeRecord> _completedStrokes = new List<StrokeRecord>();

        private Color _penColor = Color.Red;
        private float _penWidth = 0.3f;

        private readonly List<Point> _smoothBuffer = new List<Point>();
        private const int SmoothDistance = 2;

        // Stroke smoothing via Rust ink-stroke-modeler (FFI)
        private bool _smoothEnabled = true;
        private bool _rustModelerAvailable = false;

        // C# fallback smoothing fields (used when Rust modeler unavailable)
        private readonly List<PointF> _smoothHistory = new List<PointF>();
        private const int SmoothWindow = 5;
        private PointF _lastSmoothPoint;

        // Pen color as normalized doubles for Rust modeler FFI
        private double _penR = 1.0, _penG = 0.0, _penB = 0.0;
        private double _widthScale = 1.0;

        /// <summary>Enable/disable stroke smoothing (jitter/wobble reduction).</summary>
        public bool SmoothEnabled
        {
            get { return _smoothEnabled; }
            set { _smoothEnabled = value; if (!value) _smoothHistory.Clear(); }
        }

        /// <summary>Set pen color as normalized RGB (0.0-1.0) for Rust modeler.</summary>
        public void SetPenColorNorm(double r, double g, double b)
        {
            _penR = r; _penG = g; _penB = b;
        }

        /// <summary>Set width scale for Rust modeler.</summary>
        public double WidthScale
        {
            get { return _widthScale; }
            set { _widthScale = Math.Max(0.1, Math.Min(10.0, value)); }
        }

        // Shared state for PenInterceptor suppression
        public static DateTime LastPenEventUtc = DateTime.MinValue;
        public static bool HidTipDown = false;
        public static int PointerX = -1, PointerY = -1;

        /// <summary>Apply WM_POINTER position (correct screen coords).</summary>
        private void ApplyPointerPos()
        {
            if (PointerX < 0) return;
            int sx = PointerX, sy = PointerY;
            PointerX = -1; // consume
            _lastPenPos = new Point(sx, sy);
            _penCursorPos = _lastPenPos;
            _showPenCursor = true;
            if (_isDrawing)
            {
                OnPenMove(_lastPoint.X, _lastPoint.Y, sx, sy);
            }
        }

        public bool DrawingEnabled { get; set; }

        // Last known pen position from raw input
        private Point _lastPenPos;

        // Pen cursor visibility
        private bool _showPenCursor;
        private Point _penCursorPos;

        // Transparent cursor to hide system pen cursor
        private IntPtr _transparentCursor = IntPtr.Zero;
        private IntPtr _originalCursor = IntPtr.Zero;

        private void CreateTransparentCursor()
        {
            // 1x1 fully transparent cursor
            byte[] andPlane = new byte[] { 0xFF }; // all bits AND mask (transparent)
            byte[] xorPlane = new byte[] { 0x00 }; // all bits XOR mask
            _transparentCursor = NativeMethods.CreateCursor(
                IntPtr.Zero, 0, 0, 1, 1, andPlane, xorPlane);
        }

        // Tip state from HID
        private bool _hidTipDown;
        private DateTime _hidLastReportUtc = DateTime.MinValue;
        private Timer _liftTimer;

        // Coordinate inversion for 180° rotated tablets
        public bool InvertX = false;
        public bool InvertY = false;

        private int _rawMouseCount, _rawHidCount, _penAbsCount;
        private int _drawCount, _paintCount;
        private int _moveLogCount;
        private int _pointerLogCount;

        private static void Log(string msg) { Program.Log(msg); }
        private static void Log(string fmt, params object[] args) { Program.Log(fmt, args); }

        public Color PenColor
        {
            get { return _penColor; }
            set
            {
                _penColor = value;
                // Sync normalized color for Rust modeler
                _penR = value.R / 255.0;
                _penG = value.G / 255.0;
                _penB = value.B / 255.0;
            }
        }

        public float PenWidth
        {
            get { return _penWidth; }
            set { _penWidth = Math.Max(0.5f, Math.Min(20f, value)); }
        }

        /// <summary>Check if Rust modeler DLL is available.</summary>
        public void ProbeRustModeler()
        {
            try
            {
                // Try a harmless FFI call to see if the DLL loads
                GlaspenNative.glaspen2_now_secs();
                _rustModelerAvailable = true;
                Log("[Overlay] Rust modeler DLL loaded — using ink-stroke-modeler");
            }
            catch
            {
                _rustModelerAvailable = false;
                Log("[Overlay] Rust modeler DLL not found — using C# fallback smoothing");
            }
        }

        public OverlayForm()
        {
            DrawingEnabled = true;

            var bounds = SystemInformation.VirtualScreen;
            this.StartPosition = FormStartPosition.Manual;
            this.Location = bounds.Location;
            this.Size = bounds.Size;
            this.FormBorderStyle = FormBorderStyle.None;
            this.ShowInTaskbar = false;
            this.TopMost = true;
            this.ShowIcon = false;

            // Use TransparencyKey instead of WS_EX_LAYERED
            this.BackColor = Color.Fuchsia;
            this.TransparencyKey = Color.Fuchsia;

            // Double-buffering for smooth rendering
            this.DoubleBuffered = true;

            // Create off-screen bitmap for ink
            _canvas = new Bitmap(bounds.Width, bounds.Height, PixelFormat.Format32bppArgb);
            _canvasGraphics = Graphics.FromImage(_canvas);
            _canvasGraphics.SmoothingMode = SmoothingMode.AntiAlias;
            _canvasGraphics.Clear(Color.Transparent);

            CreateTransparentCursor();

            Log("[Overlay] Canvas: {0}x{1}, using TransparencyKey=Fuchsia, BackColor=Fuchsia",
                bounds.Width, bounds.Height);
        }

        protected override void OnHandleCreated(EventArgs e)
        {
            base.OnHandleCreated(e);
            Log("[Overlay] HWND=0x{0:X}, Pos=({1},{2}), Size={3}x{4}",
                this.Handle.ToInt64(), this.Left, this.Top, this.Width, this.Height);

            // NOT calling EnableMouseInPointer — it converts pen to WM_POINTER
            // which bypasses our mouse hook and ClipCursor lock.
            Log("[Overlay] Ready.");

            // Force window to topmost visible
            NativeMethods.SetWindowPos(
                this.Handle, NativeMethods.HWND_TOPMOST,
                this.Left, this.Top, this.Width, this.Height,
                NativeMethods.SWP_NOACTIVATE | NativeMethods.SWP_SHOWWINDOW);

            // Register for raw input
            RegisterRawInput();

            // Register global hotkeys
            // Ctrl+Alt+C — clear screen
            NativeMethods.RegisterHotKey(this.Handle, 1,
                NativeMethods.MOD_CONTROL | NativeMethods.MOD_ALT, (uint)Keys.C);
            // Ctrl+Alt+Z — undo last stroke
            NativeMethods.RegisterHotKey(this.Handle, 2,
                NativeMethods.MOD_CONTROL | NativeMethods.MOD_ALT, (uint)Keys.Z);

            // Safety auto-lift timer (only when HID not providing tip data)
            _liftTimer = new Timer { Interval = 200 };
            _liftTimer.Tick += (s, args) =>
            {
                if (_isDrawing && !HidTipDown
                    && (DateTime.UtcNow - LastPenEventUtc).TotalMilliseconds > 2000)
                {
                    Log("[Draw] SAFETY LIFT (no input for 2s)");
                    StopCursorLock();
                    StopDrawing();
                }
            };
            _liftTimer.Start();
        }

        private void RegisterRawInput()
        {
            // Try multiple registration strategies for HID digitizer
            // Strategy 1: RIDEV_INPUTSINK (standard background)
            // Strategy 2: RIDEV_EXINPUTSINK (extended background, Vista+)
            // Strategy 3: No INPUTSINK at all (foreground only, but might work)

            var devices = new NativeMethods.RAWINPUTDEVICE[6];
            int idx = 0;
            IntPtr hwnd = this.Handle;

            // Mouse
            devices[idx].usUsagePage = 0x0001; devices[idx].usUsage = 0x0002;
            devices[idx].dwFlags = NativeMethods.RIDEV_INPUTSINK; devices[idx].hwndTarget = hwnd;
            idx++;

            // Digitizer — try different flag combinations
            const uint EXSINK = 0x00001000; // RIDEV_EXINPUTSINK
            uint[] flags = { NativeMethods.RIDEV_INPUTSINK, EXSINK, (uint)0, NativeMethods.RIDEV_INPUTSINK | EXSINK };

            foreach (uint flag in flags)
            {
                devices[idx].usUsagePage = 0x000D;
                devices[idx].usUsage = 0x0002; // Pen
                devices[idx].dwFlags = flag;
                devices[idx].hwndTarget = hwnd;
                idx++;
                if (idx >= devices.Length) break;
            }

            // Also try digitizer stylus with INPUTSINK
            if (idx < devices.Length)
            {
                devices[idx].usUsagePage = 0x000D;
                devices[idx].usUsage = 0x0001; // Stylus
                devices[idx].dwFlags = NativeMethods.RIDEV_INPUTSINK;
                devices[idx].hwndTarget = hwnd;
                idx++;
            }

            uint cbSize = (uint)Marshal.SizeOf(typeof(NativeMethods.RAWINPUTDEVICE));
            bool ok = NativeMethods.RegisterRawInputDevices(devices, (uint)idx, cbSize);
            int err = Marshal.GetLastWin32Error();
            Log("[Overlay] RegisterRawInputDevices ({0} entries): {1} (err={2})",
                idx, ok ? "OK" : "FAILED", err);
        }

        protected override void WndProc(ref Message m)
        {
            if (m.Msg == NativeMethods.WM_TABLET_QUERYSYSTEMGESTURESTATUS)
            {
                // Disable ALL Windows INK visual feedback on our overlay.
                m.Result = (IntPtr)NativeMethods.TABLET_DISABLE_ALL;
                return;
            }
            else if (m.Msg == NativeMethods.WM_SETCURSOR)
            {
                // Replace system cursor with transparent one to hide the
                // Windows INK pen cursor (we draw our own crosshair).
                int hitTest = (int)(m.LParam.ToInt64() & 0xFFFF);
                if (hitTest == NativeMethods.HTCLIENT && _transparentCursor != IntPtr.Zero)
                {
                    NativeMethods.SetCursor(_transparentCursor);
                    m.Result = (IntPtr)1;
                    return;
                }
            }

            // Without WS_EX_TRANSPARENT, pen events reach our overlay directly.
            // No need to suppress pen-originated mouse events — we handle them.

            if (m.Msg == NativeMethods.WM_LBUTTONDOWN)
            {
                bool isPen = IsPenSourceMessage();
                int cx = (int)(m.LParam.ToInt64() & 0xFFFF);
                int cy = (int)((m.LParam.ToInt64() >> 16) & 0xFFFF);
                var pt = new NativeMethods.POINT(cx, cy);
                NativeMethods.ClientToScreen(this.Handle, ref pt);
                Log("[WndProc] WM_LBUTTONDOWN client=({0},{1}) screen=({2},{3}) isPen={4} pressure={5}",
                    cx, cy, pt.X, pt.Y, isPen, _lastPointerPressure);
                if (DrawingEnabled && isPen)
                {
                    OnPenDown(pt.X, pt.Y);
                    return;
                }
            }
            else if (m.Msg == NativeMethods.WM_LBUTTONUP)
            {
                bool isPen = IsPenSourceMessage();
                Log("[WndProc] WM_LBUTTONUP isPen={0} drawing={1}", isPen, _isDrawing);
                if (_isDrawing && isPen)
                {
                    OnPenUp(_lastPenPos.X, _lastPenPos.Y);
                    return;
                }
            }
            else if (m.Msg == NativeMethods.WM_MOUSEMOVE)
            {
                bool isPen = IsPenSourceMessage();
                if (isPen)
                {
                    int cx = (int)(m.LParam.ToInt64() & 0xFFFF);
                    int cy = (int)((m.LParam.ToInt64() >> 16) & 0xFFFF);
                    var pt = new NativeMethods.POINT(cx, cy);
                    NativeMethods.ClientToScreen(this.Handle, ref pt);
                    if (_moveLogCount < 50 || _moveLogCount % 100 == 0)
                        Log("[WndProc] WM_MOUSEMOVE(pen) screen=({0},{1}) pressure={2} drawing={3}",
                            pt.X, pt.Y, _lastPointerPressure, _isDrawing);
                    _moveLogCount++;
                    _lastPenPos = new Point(pt.X, pt.Y);
                    if (_isDrawing) OnPenMoveRaw(pt.X, pt.Y);
                    return;
                }
            }

            if (m.Msg == NativeMethods.WM_INPUT)
            {
                ProcessRawInput(m.LParam);
            }
            else if (m.Msg == NativeMethods.WM_POINTERDOWN ||
                     m.Msg == NativeMethods.WM_POINTERUPDATE ||
                     m.Msg == NativeMethods.WM_POINTERUP)
            {
                if (_pointerLogCount < 20)
                {
                    uint pid = (uint)m.WParam.ToInt64();
                    uint ptype;
                    NativeMethods.GetPointerType(pid, out ptype);
                    Log("[WndProc] WM_POINTER 0x{0:X4} pointerId={1} type={2}",
                        m.Msg, pid, ptype);
                }
                _pointerLogCount++;
                ProcessPointerMsg((uint)m.WParam.ToInt64(), m.Msg);
            }
            else if (m.Msg == NativeMethods.WM_HOTKEY)
            {
                int id = (int)m.WParam;
                if (id == 1) ClearAll();       // Ctrl+Alt+C
                else if (id == 2) UndoLast();  // Ctrl+Alt+Z
            }
            base.WndProc(ref m);
        }

        /// <summary>Check if the current message was generated by a pen (not a real mouse).
        /// Uses GetMessageExtraInfo — pen driver sets PEN_SIGNATURE bits.</summary>
        private static bool IsPenSourceMessage()
        {
            try
            {
                IntPtr extra = NativeMethods.GetMessageExtraInfo();
                ulong val = (ulong)extra.ToInt64();
                return (val & NativeMethods.PEN_SIGNATURE_MASK) == NativeMethods.PEN_SIGNATURE;
            }
            catch { return false; }
        }

        private void ProcessPointerMsg(uint pointerId, int msg)
        {
            uint pointerType;
            if (!NativeMethods.GetPointerType(pointerId, out pointerType)) return;
            if (pointerType != NativeMethods.PT_PEN) return; // only pen

            var penInfo = new NativeMethods.POINTER_PEN_INFO();
            if (!NativeMethods.GetPointerPenInfo(pointerId, ref penInfo)) return;

            uint pressure = penInfo.pressure; // 0-1024
            int scrX = penInfo.pointerInfo.ptPixelLocation.X;
            int scrY = penInfo.pointerInfo.ptPixelLocation.Y;

            _lastPointerPressure = pressure; // update on EVERY event

            if (msg == NativeMethods.WM_POINTERDOWN)
            {
                if (!_isDrawing && DrawingEnabled)
                {
                    Log("[Pointer] PEN DOWN pressure={0} pos=({1},{2})", pressure, scrX, scrY);
                    OnPenDown(scrX, scrY);
                }
            }
            else if (msg == NativeMethods.WM_POINTERUP)
            {
                if (_isDrawing)
                {
                    Log("[Pointer] PEN UP pressure={0}", pressure);
                    OnPenUp(scrX, scrY);
                }
            }
            else if (msg == NativeMethods.WM_POINTERUPDATE && _isDrawing)
            {
                OnPenMove(_lastPoint.X, _lastPoint.Y, scrX, scrY);
                _lastPenPos = new Point(scrX, scrY);
                _lastPoint = _lastPenPos;
            }
        }

        private int _pointerCount;
        private uint _lastPointerPressure;

        // Paint the ink bitmap + pen cursor onto the form
        protected override void OnPaint(PaintEventArgs e)
        {
            base.OnPaint(e);
            _paintCount++;
            if (_canvas != null)
            {
                e.Graphics.DrawImage(_canvas, 0, 0);
            }

            // Draw pen cursor (crosshair at last known pen position)
            if (_showPenCursor && _penCursorPos.X > 0)
            {
                int cx = ClampX(_penCursorPos.X - this.Left);
                int cy = ClampY(_penCursorPos.Y - this.Top);
                int r = 10;
                using (var cp = new Pen(Color.FromArgb(200, 255, 80, 30), 2f))
                {
                    e.Graphics.DrawLine(cp, cx - r, cy, cx + r, cy);
                    e.Graphics.DrawLine(cp, cx, cy - r, cx, cy + r);
                    e.Graphics.DrawEllipse(cp, cx - r, cy - r, r * 2, r * 2);
                }
            }

            if (_paintCount <= 3)
                Log("[Paint #{0}] Form painted", _paintCount);
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
                else if (header.dwType == NativeMethods.RIM_TYPEMOUSE)
                {
                    ProcessMouseInput(buffer, headerBytes);
                }
            }
            finally
            {
                Marshal.FreeHGlobal(buffer);
            }
        }

        private void ProcessHidInput(IntPtr buffer, int offset, int dataLen)
        {
            _rawHidCount++;
            // Standard digitizer HID: [rptId:1][switches:1][X:2 LE][Y:2 LE][press:2 LE]...
            if (dataLen < 8)
            {
                if (_rawHidCount <= 10)
                    Log("[HID #{0}] dataLen={1} (too short, skipped)", _rawHidCount, dataLen);
                return;
            }

            int baseOff = offset + 8; // skip dwSizeHid(4)+dwCount(4)
            byte reportId = Marshal.ReadByte(buffer, baseOff);
            byte switches = Marshal.ReadByte(buffer, baseOff + 1);
            uint x = (uint)Marshal.ReadByte(buffer, baseOff + 2)
                  | ((uint)Marshal.ReadByte(buffer, baseOff + 3) << 8);
            uint y = (uint)Marshal.ReadByte(buffer, baseOff + 4)
                  | ((uint)Marshal.ReadByte(buffer, baseOff + 5) << 8);
            uint pressure = (uint)Marshal.ReadByte(buffer, baseOff + 6)
                         | ((uint)Marshal.ReadByte(buffer, baseOff + 7) << 8);

            bool tipDown = (switches & 0x01) != 0;
            _hidTipDown = tipDown;
            HidTipDown = tipDown; // share with hook
            _hidLastReportUtc = DateTime.UtcNow;
            _lastPointerPressure = pressure; // feed pressure to modeler

            bool logIt = _isDrawing || _rawHidCount <= 10 || (_rawHidCount % 100 == 0);
            if (logIt)
            {
                Console.Write("[HID #{0}] x={1} y={2} sw=0x{3:X2} press={4} tip={5}",
                    _rawHidCount, x, y, switches, pressure, tipDown);
                if (tipDown && _isDrawing)
                    Console.Write(" drawing");
                Log();
            }

            // HID logical coords → screen via 0-65535 (Windows pointer standard)
            if (x > 0 && y > 0 && x < 100000 && y < 100000)
            {
                var sb = SystemInformation.VirtualScreen;
                int sx = sb.Left + (int)((long)x * sb.Width / 65536);
                int sy = sb.Top  + (int)((long)y * sb.Height / 65536);
                _lastPenPos = new Point(sx, sy);
                _penCursorPos = _lastPenPos;
                _showPenCursor = true;
                LastPenEventUtc = DateTime.UtcNow;
            }

            // HID controls drawing when hook can't detect WM_LBUTTONDOWN (Ink ON).
            // Hook coordinates from WM_MOUSEMOVE are tracked in _lastPenPos.
            if (tipDown && pressure > 0)
            {
                SetPressure(pressure);
                if (!_isDrawing && DrawingEnabled)
                {
                    Log("[HID] TIP TOUCH → OnPenDown");
                    OnPenDown(_lastPenPos.X, _lastPenPos.Y);
                }
            }
            else if (!tipDown && _isDrawing)
            {
                Log("[HID] TIP LIFT → OnPenUp");
                OnPenUp(_lastPenPos.X, _lastPenPos.Y);
            }
        }

        private void LockCursor(bool locked)
        {
            if (locked)
            {
                // Lock cursor to current position to prevent drift
                NativeMethods.POINT pt;
                NativeMethods.GetCursorPos(out pt);
                var r = new NativeMethods.RECT(pt.X, pt.Y, pt.X + 1, pt.Y + 1);
                if (!NativeMethods.ClipCursor(ref r))
                    Log("[Lock] ClipCursor FAILED! err={0}", Marshal.GetLastWin32Error());
            }
            else
            {
                if (!NativeMethods.ClipCursor(IntPtr.Zero))
                    Log("[Lock] ClipCursor release FAILED! err={0}", Marshal.GetLastWin32Error());
            }
        }

        private Timer _lockTimer;
        private void StartCursorLock()
        {
            LockCursor(true);
            if (_lockTimer == null)
            {
                _lockTimer = new Timer { Interval = 30 };
                _lockTimer.Tick += (s, args) => { if (HidTipDown) LockCursor(true); };
            }
            _lockTimer.Start();
        }
        private void StopCursorLock()
        {
            if (_lockTimer != null) _lockTimer.Stop();
            LockCursor(false);
        }

        private void ProcessMouseInput(IntPtr buffer, int offset)
        {
            _rawMouseCount++;
            var mouse = (NativeMethods.RAWMOUSE)Marshal.PtrToStructure(
                buffer + offset, typeof(NativeMethods.RAWMOUSE));

            bool isAbsolute = (mouse.usFlags & NativeMethods.MOUSE_MOVE_ABSOLUTE) != 0;

            if (_rawMouseCount <= 10)
                Log("[Raw #{0}] flags=0x{1:X4} abs={2} lX={3} lY={4} ulRawBtns=0x{5:X8}",
                    _rawMouseCount, mouse.usFlags, isAbsolute, mouse.lLastX, mouse.lLastY, mouse.ulRawButtons);

            if (!isAbsolute) return;
            _penAbsCount++;
            if (!DrawingEnabled) return;

            LastPenEventUtc = DateTime.UtcNow;

            var bounds = SystemInformation.VirtualScreen;
            int screenX = (int)((long)mouse.lLastX * bounds.Width / 65536) + bounds.Left;
            int screenY = (int)((long)mouse.lLastY * bounds.Height / 65536) + bounds.Top;

            // Ignore near-zero — tablet reports this when pen goes out of range
            if (screenX <= 2 && screenY <= 2) return;

            // Always update last known pen position and cursor
            _lastPenPos = new Point(screenX, screenY);
            int dx = screenX - _penCursorPos.X;
            int dy = screenY - _penCursorPos.Y;
            if (dx * dx + dy * dy >= 4) // move > 2px → refresh cursor
            {
                _penCursorPos = _lastPenPos;
                _showPenCursor = true;
                this.Invalidate();
            }

            if (_penAbsCount % 50 == 0 || _penAbsCount <= 5)
                Log("[Pen #{0}] scr=({1},{2}) drawing={3}",
                    _penAbsCount, screenX, screenY, _isDrawing);

            // Draw from raw input (correct screen coords). Hook handles cursor suppression.
            if (_isDrawing)
            {
                OnPenMove(_lastPoint.X, _lastPoint.Y, screenX, screenY);
            }
        }

        #region Pen drawing

        /// <summary>Start drawing at the last known raw-input pen position.</summary>
        public void StartDrawing()
        {
            if (_isDrawing) return;
            ApplyPointerPos(); // use WM_POINTER position if available
            _isDrawing = true;
            _lastPoint = _lastPenPos;
            _smoothBuffer.Clear();
            _smoothBuffer.Add(_lastPoint);

            int x = ClampX(_lastPoint.X - this.Left);
            int y = ClampY(_lastPoint.Y - this.Top);

            float w = _currentWidth > 0 ? _currentWidth : _penWidth;
            using (var pen = new Pen(_penColor, w))
            {
                pen.StartCap = LineCap.Round;
                pen.EndCap = LineCap.Round;
                _canvasGraphics.DrawEllipse(pen, x, y, w, w);
            }

            // Initialize modeler for this stroke (same as OnPenDown)
            if (_rustModelerAvailable && _smoothEnabled)
            {
                GlaspenNative.glaspen2_modeler_clear_buffer();
                double pressure = (_lastPointerPressure > 0) ? _lastPointerPressure / 1024.0 : 0.5;
                double ts = GlaspenNative.glaspen2_now_secs();
                GlaspenNative.glaspen2_modeler_begin(
                    _penR, _penG, _penB,
                    _lastPoint.X, _lastPoint.Y, pressure, ts, _widthScale);
                GlaspenNative.glaspen2_modeler_clear_buffer();
            }

            Log("[Draw] DOWN at ({0},{1})", _lastPoint.X, _lastPoint.Y);
            this.Invalidate();
        }

        public void OnPenDown(int screenX, int screenY)
        {
            if (_isDrawing) return;
            // Use raw input position if available (correct screen coords)
            int useX = (_lastPenPos.X > 0 || _lastPenPos.Y > 0) ? _lastPenPos.X : screenX;
            int useY = (_lastPenPos.X > 0 || _lastPenPos.Y > 0) ? _lastPenPos.Y : screenY;
            _isDrawing = true;
            _lastPoint = new Point(useX, useY);
            _lastPenPos = _lastPoint;
            _smoothBuffer.Clear();
            _smoothBuffer.Add(_lastPoint);

            float w = _currentWidth > 0 ? _currentWidth : _penWidth;

            // Always draw a dot at pen-down position (both modeler and fallback paths)
            int x = ClampX(useX - this.Left);
            int y = ClampY(useY - this.Top);
            using (var pen = new Pen(_penColor, w))
            {
                pen.StartCap = LineCap.Round;
                pen.EndCap = LineCap.Round;
                _canvasGraphics.DrawEllipse(pen, x, y, w, w);
            }

            if (_rustModelerAvailable && _smoothEnabled)
            {
                // Clear any residual modeler state, then begin new stroke.
                // Do NOT call DrawModelerBuffer here — the modeler may return
                // stale points from the previous stroke. Drawing starts on OnPenMove.
                GlaspenNative.glaspen2_modeler_clear_buffer();
                double pressure = (_lastPointerPressure > 0) ? _lastPointerPressure / 1024.0 : 0.5;
                double ts = GlaspenNative.glaspen2_now_secs();
                GlaspenNative.glaspen2_modeler_begin(
                    _penR, _penG, _penB,
                    useX, useY, pressure, ts, _widthScale);
                GlaspenNative.glaspen2_modeler_clear_buffer();
            }
            else
            {
                // C# fallback: reset smoothing state
                _smoothHistory.Clear();
                _lastSmoothPoint = new PointF(useX, useY);
                if (_smoothEnabled)
                    _smoothHistory.Add(_lastSmoothPoint);
            }
            Log("[Draw] DOWN at ({0},{1}) modeler={2}", useX, useY, _rustModelerAvailable);
            this.Invalidate();
        }

        /// <summary>Draw from hook WM_MOUSEMOVE (screen coordinates).</summary>
        public void OnPenMoveRaw(int screenX, int screenY)
        {
            if (!_isDrawing) return;
            // Ignore near-zero — tablet sends these when pen goes out of range
            if (screenX <= 2 && screenY <= 2) return;
            OnPenMove(_lastPoint.X, _lastPoint.Y, screenX, screenY);
        }

        /// <summary>Read smoothed points from the Rust modeler buffer and draw them.</summary>
        private void DrawModelerBuffer(float baseWidth)
        {
            int count = GlaspenNative.glaspen2_modeler_point_count();
            if (count < 1) return;

            double prevX = 0, prevY = 0, prevW = 0;
            bool hasPrev = false;

            for (int i = 0; i < count; i++)
            {
                double px, py, pw;
                GlaspenNative.glaspen2_modeler_get_point(i, out px, out py, out pw);

                int sx = ClampX((int)px - this.Left);
                int sy = ClampY((int)py - this.Top);
                float drawW = (float)pw > 0 ? (float)pw : baseWidth;

                if (hasPrev)
                {
                    int fx = ClampX((int)prevX - this.Left);
                    int fy = ClampY((int)prevY - this.Top);
                    using (var pen = new Pen(_penColor, drawW))
                    {
                        pen.StartCap = LineCap.Round;
                        pen.EndCap = LineCap.Round;
                        pen.LineJoin = LineJoin.Round;
                        _canvasGraphics.DrawLine(pen, fx, fy, sx, sy);
                    }
                }
                else
                {
                    using (var pen = new Pen(_penColor, drawW))
                    {
                        pen.StartCap = LineCap.Round;
                        _canvasGraphics.DrawEllipse(pen, sx, sy, drawW, drawW);
                    }
                }

                prevX = px; prevY = py; prevW = pw;
                hasPrev = true;
            }
        }

        public void OnPenMove(int fromSX, int fromSY, int toSX, int toSY)
        {
            if (!_isDrawing) return;

            int dx = toSX - _lastPoint.X;
            int dy = toSY - _lastPoint.Y;
            if (dx * dx + dy * dy < SmoothDistance * SmoothDistance) return;

            _smoothBuffer.Add(new Point(toSX, toSY));

            if (_rustModelerAvailable && _smoothEnabled)
            {
                // Use Rust modeler with real-time pressure
                double pressure = QueryCurrentPressure();
                double ts = GlaspenNative.glaspen2_now_secs();
                GlaspenNative.glaspen2_modeler_move(toSX, toSY, pressure, ts, _widthScale);

                float lineW = _currentWidth > 0 ? _currentWidth : _penWidth;
                DrawModelerBuffer(lineW);
            }
            else
            {
                // C# fallback: weighted moving average
                float useFromX, useFromY, useToX, useToY;
                if (_smoothEnabled)
                {
                    var rawTo = new PointF(toSX, toSY);
                    _smoothHistory.Add(rawTo);
                    if (_smoothHistory.Count > SmoothWindow)
                        _smoothHistory.RemoveAt(0);

                    float totalW = 0, sumX = 0, sumY = 0;
                    for (int i = 0; i < _smoothHistory.Count; i++)
                    {
                        float w = (float)(i + 1) / _smoothHistory.Count;
                        sumX += _smoothHistory[i].X * w;
                        sumY += _smoothHistory[i].Y * w;
                        totalW += w;
                    }
                    var smoothedTo = new PointF(sumX / totalW, sumY / totalW);

                    useFromX = _lastSmoothPoint.X;
                    useFromY = _lastSmoothPoint.Y;
                    useToX = smoothedTo.X;
                    useToY = smoothedTo.Y;
                    _lastSmoothPoint = smoothedTo;
                }
                else
                {
                    useFromX = fromSX;
                    useFromY = fromSY;
                    useToX = toSX;
                    useToY = toSY;
                }

                int fx = ClampX((int)useFromX - this.Left);
                int fy = ClampY((int)useFromY - this.Top);
                int tx = ClampX((int)useToX - this.Left);
                int ty = ClampY((int)useToY - this.Top);

                if (_smoothEnabled && (fx - tx) * (fx - tx) + (fy - ty) * (fy - ty) < 1) return;

                float lineW = _currentWidth > 0 ? _currentWidth : _penWidth;
                using (var pen = new Pen(_penColor, lineW))
                {
                    pen.StartCap = LineCap.Round;
                    pen.EndCap = LineCap.Round;
                    pen.LineJoin = LineJoin.Round;
                    _canvasGraphics.DrawLine(pen, fx, fy, tx, ty);
                }
            }

            _lastPoint = new Point(toSX, toSY);
            // Only invalidate the dirty region, not the entire form
            int minX = Math.Min(ClampX(fromSX - this.Left), ClampX(toSX - this.Left));
            int minY = Math.Min(ClampY(fromSY - this.Top), ClampY(toSY - this.Top));
            int maxX = Math.Max(ClampX(fromSX - this.Left), ClampX(toSX - this.Left));
            int maxY = Math.Max(ClampY(fromSY - this.Top), ClampY(toSY - this.Top));
            int pad = (int)(_penWidth * 3) + 4;
            this.Invalidate(new Rectangle(minX - pad, minY - pad, maxX - minX + pad * 2, maxY - minY + pad * 2));
        }

        /// <summary>Query real-time pen pressure from the system (0.0-1.0).
        /// Falls back to _lastPointerPressure if query fails.</summary>
        private double QueryCurrentPressure()
        {
            // Try WM_POINTER API first (works when pen is over the overlay)
            try
            {
                uint pointerId = 1; // primary pointer
                uint pointerType;
                if (NativeMethods.GetPointerType(pointerId, out pointerType)
                    && pointerType == NativeMethods.PT_PEN)
                {
                    var penInfo = new NativeMethods.POINTER_PEN_INFO();
                    if (NativeMethods.GetPointerPenInfo(pointerId, ref penInfo)
                        && penInfo.pressure > 0)
                    {
                        _lastPointerPressure = penInfo.pressure; // cache for fallback
                        return penInfo.pressure / 1024.0;
                    }
                }
            }
            catch { }

            // Fallback: use cached value from last WM_POINTER or HID event
            return (_lastPointerPressure > 0) ? _lastPointerPressure / 1024.0 : 0.5;
        }

        /// <summary>Set pen pressure (0-1024). Adjusts stroke width: 0.5x to 2x base width.</summary>
        public void SetPressure(uint pressure)
        {
            // Map 0-1024 to 0.3x-2x of base pen width
            float factor = 0.3f + (pressure / 1024f) * 1.7f;
            _currentWidth = _penWidth * factor;
        }

        private float _currentWidth = 0f; // 0 = use _penWidth (no pressure data yet)

        /// <summary>Stop drawing at the last known raw-input pen position.</summary>
        public void StopDrawing()
        {
            if (!_isDrawing) return;
            _isDrawing = false;
            StopCursorLock();

            float w = _currentWidth > 0 ? _currentWidth : _penWidth;

            // Finalize modeler if active (same as OnPenUp — no drawing)
            if (_rustModelerAvailable && _smoothEnabled)
            {
                double pressure = (_lastPointerPressure > 0) ? _lastPointerPressure / 1024.0 : 0.5;
                double ts = GlaspenNative.glaspen2_now_secs();
                GlaspenNative.glaspen2_modeler_end(
                    _lastPenPos.X, _lastPenPos.Y, pressure, ts, _widthScale);
                GlaspenNative.glaspen2_modeler_clear_buffer();
                GlaspenNative.glaspen2_modeler_commit_to_strokes(_penR, _penG, _penB, IntPtr.Zero, 0);
            }

            Log("[Draw] UP at ({0},{1})", _lastPenPos.X, _lastPenPos.Y);

            if (_smoothBuffer.Count > 0)
            {
                _completedStrokes.Add(new StrokeRecord
                {
                    Points = new List<Point>(_smoothBuffer),
                    Color = _penColor,
                    Width = w
                });
            }
            _smoothBuffer.Clear();
            this.Invalidate();
        }

        public void OnPenUp(int screenX, int screenY)
        {
            if (!_isDrawing) return;
            _isDrawing = false;

            float w = _currentWidth > 0 ? _currentWidth : _penWidth;

            if (_rustModelerAvailable && _smoothEnabled)
            {
                // Finalize modeler — do NOT draw its output (Up event returns
                // convergence points that would create a spurious line).
                // The last OnPenMove already drew the final visible segment.
                double pressure = (_lastPointerPressure > 0) ? _lastPointerPressure / 1024.0 : 0.5;
                double ts = GlaspenNative.glaspen2_now_secs();
                GlaspenNative.glaspen2_modeler_end(screenX, screenY, pressure, ts, _widthScale);
                GlaspenNative.glaspen2_modeler_clear_buffer();
                // Commit smoothed points into Rust STROKES (for DB persistence + export)
                GlaspenNative.glaspen2_modeler_commit_to_strokes(_penR, _penG, _penB, IntPtr.Zero, 0);
            }

            if (_smoothBuffer.Count > 0)
            {
                _completedStrokes.Add(new StrokeRecord
                {
                    Points = new List<Point>(_smoothBuffer),
                    Color = _penColor,
                    Width = w
                });
            }
            _smoothBuffer.Clear();
            _smoothHistory.Clear();
            Log("[Draw] UP at ({0},{1}) modeler={2}", screenX, screenY, _rustModelerAvailable);
            this.Invalidate();
        }

        #endregion

        private int ClampX(int x) { return x < 0 ? 0 : (x >= _canvas.Width ? _canvas.Width - 1 : x); }
        private int ClampY(int y) { return y < 0 ? 0 : (y >= _canvas.Height ? _canvas.Height - 1 : y); }

        public void ClearAll()
        {
            _completedStrokes.Clear();
            _smoothBuffer.Clear();
            _isDrawing = false;
            _canvasGraphics.Clear(Color.Transparent);

            // Also clear Rust-side strokes and start a new DB screen
            if (_rustModelerAvailable)
            {
                GlaspenNative.glaspen2_clear_strokes(this.Width, this.Height);
            }

            this.Invalidate();
            Log("[Overlay] Canvas cleared.");
        }

        public void UndoLast()
        {
            if (_completedStrokes.Count == 0 && !_isDrawing) return;
            _canvasGraphics.Clear(Color.Transparent);
            if (_completedStrokes.Count > 0) _completedStrokes.RemoveAt(_completedStrokes.Count - 1);
            ReplayStrokes(_completedStrokes);
            this.Invalidate();
        }

        private void ReplayStrokes(List<StrokeRecord> strokes)
        {
            foreach (var stroke in strokes)
            {
                if (stroke.Points.Count == 1)
                {
                    var p = stroke.Points[0];
                    int x = ClampX(p.X - this.Left);
                    int y = ClampY(p.Y - this.Top);
                    using (var pen = new Pen(stroke.Color, stroke.Width))
                    {
                        pen.StartCap = LineCap.Round;
                        pen.EndCap = LineCap.Round;
                        _canvasGraphics.DrawEllipse(pen, x, y, stroke.Width, stroke.Width);
                    }
                }
                else if (stroke.Points.Count > 1)
                {
                    var pts = new Point[stroke.Points.Count];
                    for (int i = 0; i < pts.Length; i++)
                        pts[i] = new Point(
                            ClampX(stroke.Points[i].X - this.Left),
                            ClampY(stroke.Points[i].Y - this.Top));
                    using (var pen = new Pen(stroke.Color, stroke.Width))
                    {
                        pen.StartCap = LineCap.Round;
                        pen.EndCap = LineCap.Round;
                        pen.LineJoin = LineJoin.Round;
                        _canvasGraphics.DrawLines(pen, pts);
                    }
                }
            }
        }

        public void RefreshScreenBounds()
        {
            var bounds = SystemInformation.VirtualScreen;
            this.Location = bounds.Location;
            this.Size = bounds.Size;
            var old = new List<StrokeRecord>(_completedStrokes);
            if (_canvasGraphics != null) _canvasGraphics.Dispose();
            if (_canvas != null) _canvas.Dispose();
            _canvas = new Bitmap(bounds.Width, bounds.Height, PixelFormat.Format32bppArgb);
            _canvasGraphics = Graphics.FromImage(_canvas);
            _canvasGraphics.SmoothingMode = SmoothingMode.AntiAlias;
            _canvasGraphics.Clear(Color.Transparent);
            ReplayStrokes(old);
            this.Invalidate();
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing)
            {
                // Unregister hotkeys
                if (this.IsHandleCreated)
                {
                    NativeMethods.UnregisterHotKey(this.Handle, 1);
                    NativeMethods.UnregisterHotKey(this.Handle, 2);
                }
                if (_liftTimer != null) { _liftTimer.Stop(); _liftTimer.Dispose(); }
                if (_lockTimer != null) { _lockTimer.Stop(); _lockTimer.Dispose(); }
                if (_canvasGraphics != null) _canvasGraphics.Dispose();
                if (_canvas != null) _canvas.Dispose();
                if (_transparentCursor != IntPtr.Zero)
                {
                    NativeMethods.DestroyCursor(_transparentCursor);
                    _transparentCursor = IntPtr.Zero;
                }
            }
            base.Dispose(disposing);
        }

        private class StrokeRecord
        {
            public List<Point> Points { get; set; }
            public Color Color { get; set; }
            public float Width { get; set; }
        }

        protected override bool ShowWithoutActivation { get { return true; } }

        protected override CreateParams CreateParams
        {
            get
            {
                var cp = base.CreateParams;
                // NO WS_EX_TRANSPARENT — overlay captures pen input directly.
                // This is the key change for Microsoft INK compatibility:
                // pen events go to our overlay (not to INK stack).
                // Keyboard/mouse pass through because of WS_EX_NOACTIVATE.
                cp.ExStyle |= NativeMethods.WS_EX_NOACTIVATE
                           | NativeMethods.WS_EX_TOOLWINDOW
                           | NativeMethods.WS_EX_TOPMOST;
                // NOTE: NO WS_EX_LAYERED — using TransparencyKey instead
                return cp;
            }
        }
    }
}
