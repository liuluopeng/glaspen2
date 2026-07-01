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

        // Stroke beautification (like macOS ink-stroke-modeler)
        private bool _smoothEnabled = true;
        private readonly List<PointF> _smoothHistory = new List<PointF>();
        private const int SmoothWindow = 5;  // moving average window size
        private PointF _lastSmoothPoint;      // previous smoothed endpoint for continuity

        /// <summary>Enable/disable stroke smoothing (jitter/wobble reduction).</summary>
        public bool SmoothEnabled
        {
            get { return _smoothEnabled; }
            set { _smoothEnabled = value; if (!value) _smoothHistory.Clear(); }
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

        // Tip state from HID
        private bool _hidTipDown;
        private DateTime _hidLastReportUtc = DateTime.MinValue;
        private Timer _liftTimer;

        // Direct HID pen reader for pressure (works when INK is OFF)
        private HidPenReader _hidReader;

        // Coordinate inversion for 180° rotated tablets
        public bool InvertX = false;
        public bool InvertY = false;

        private int _rawMouseCount, _rawHidCount, _penAbsCount;
        private int _drawCount, _paintCount;

        public Color PenColor
        {
            get { return _penColor; }
            set { _penColor = value; }
        }

        public float PenWidth
        {
            get { return _penWidth; }
            set { _penWidth = Math.Max(0.5f, Math.Min(20f, value)); }
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

            Console.WriteLine("[Overlay] Canvas: {0}x{1}, using TransparencyKey=Fuchsia, BackColor=Fuchsia",
                bounds.Width, bounds.Height);
        }

        protected override void OnHandleCreated(EventArgs e)
        {
            base.OnHandleCreated(e);
            Console.WriteLine("[Overlay] HWND=0x{0:X}, Pos=({1},{2}), Size={3}x{4}",
                this.Handle.ToInt64(), this.Left, this.Top, this.Width, this.Height);

            Console.WriteLine("[Overlay] Ready.");

            // Start HID pen reader in background (don't block UI)
            System.Threading.ThreadPool.QueueUserWorkItem(_ => StartHidPenReader());

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
            // Ctrl+Alt+Q — exit application
            NativeMethods.RegisterHotKey(this.Handle, 3,
                NativeMethods.MOD_CONTROL | NativeMethods.MOD_ALT, (uint)Keys.Q);

            // Safety auto-lift timer (only when HID not providing tip data)
            _liftTimer = new Timer { Interval = 200 };
            _liftTimer.Tick += (s, args) =>
            {
                if (_isDrawing && !HidTipDown
                    && (DateTime.UtcNow - LastPenEventUtc).TotalMilliseconds > 2000)
                {
                    Console.WriteLine("[Draw] SAFETY LIFT (no input for 2s)");
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
            Console.WriteLine("[Overlay] RegisterRawInputDevices ({0} entries): {1} (err={2})",
                idx, ok ? "OK" : "FAILED", err);
        }

        private void StartHidPenReader()
        {
            try
            {
                _hidReader = new HidPenReader();
                _hidReader.PenReport += (x, y, pressure, tipDown) =>
                {
                    // Update pressure for drawing
                    SetPressure(pressure);

                    // Track HID state
                    _hidTipDown = tipDown;
                    HidTipDown = tipDown;
                    _hidLastReportUtc = DateTime.UtcNow;
                    LastPenEventUtc = DateTime.UtcNow;

                    // Map HID coords to screen
                    var sb = SystemInformation.VirtualScreen;
                    int sx = sb.Left + (int)((long)x * sb.Width / (_hidReader.MaxX > 0 ? _hidReader.MaxX : 65536));
                    int sy = sb.Top  + (int)((long)y * sb.Height / (_hidReader.MaxY > 0 ? _hidReader.MaxY : 65536));

                    _lastPenPos = new Point(sx, sy);
                    _penCursorPos = _lastPenPos;
                    _showPenCursor = true;
                };

                if (_hidReader.Open())
                    Program.Log("[Overlay] HID pen reader started successfully");
                else
                    Program.Log("[Overlay] HID pen reader: no digitizer found (expected if INK is on)");
            }
            catch (Exception ex)
            {
                Program.Log("[Overlay] HID pen reader failed: {0}", ex.Message);
            }
        }

        protected override void WndProc(ref Message m)
        {
            if (m.Msg == NativeMethods.WM_INPUT)
            {
                ProcessRawInput(m.LParam);
            }
            else if (m.Msg == NativeMethods.WM_POINTERDOWN ||
                     m.Msg == NativeMethods.WM_POINTERUPDATE ||
                     m.Msg == NativeMethods.WM_POINTERUP)
            {
                ProcessPointerMsg((uint)m.WParam.ToInt64(), m.Msg);
            }
            else if (m.Msg == NativeMethods.WM_HOTKEY)
            {
                int id = (int)m.WParam;
                if (id == 1) ClearAll();       // Ctrl+Alt+C
                else if (id == 2) UndoLast();  // Ctrl+Alt+Z
                else if (id == 3) Application.Exit();  // Ctrl+Alt+Q
            }
            base.WndProc(ref m);
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

            if (_pointerCount <= 5)
                Console.WriteLine("[Pointer #{0}] msg=0x{1:X4} pos=({2},{3}) pressure={4}",
                    _pointerCount, msg, scrX, scrY, pressure);

            _pointerCount++;
            _lastPointerPressure = pressure;

            _lastPointerPressure = pressure;

            if (msg == NativeMethods.WM_POINTERDOWN)
                Console.WriteLine("[Pointer] PEN DOWN pressure={0} pos=({1},{2})", pressure, scrX, scrY);
            else if (msg == NativeMethods.WM_POINTERUP)
                Console.WriteLine("[Pointer] PEN UP pressure={0}", pressure);
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
                Console.WriteLine("[Paint #{0}] Form painted", _paintCount);
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
            if (dataLen < 8) return;

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

            bool logIt = _isDrawing || _rawHidCount <= 10 || (_rawHidCount % 100 == 0);
            if (logIt)
            {
                Console.Write("[HID #{0}] x={1} y={2} sw=0x{3:X2} press={4} tip={5}",
                    _rawHidCount, x, y, switches, pressure, tipDown);
                if (tipDown && _isDrawing)
                    Console.Write(" drawing");
                Console.WriteLine();
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
                    Console.WriteLine("[HID] TIP TOUCH → OnPenDown");
                    OnPenDown(_lastPenPos.X, _lastPenPos.Y);
                }
            }
            else if (!tipDown && _isDrawing)
            {
                Console.WriteLine("[HID] TIP LIFT → OnPenUp");
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
                    Console.WriteLine("[Lock] ClipCursor FAILED! err={0}", Marshal.GetLastWin32Error());
            }
            else
            {
                if (!NativeMethods.ClipCursor(IntPtr.Zero))
                    Console.WriteLine("[Lock] ClipCursor release FAILED! err={0}", Marshal.GetLastWin32Error());
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
                Console.WriteLine("[Raw #{0}] flags=0x{1:X4} abs={2} lX={3} lY={4} ulRawBtns=0x{5:X8}",
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
                Console.WriteLine("[Pen #{0}] scr=({1},{2}) drawing={3}",
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
            Console.WriteLine("[Draw] DOWN at ({0},{1})", _lastPoint.X, _lastPoint.Y);
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
            _smoothHistory.Clear();
            _lastSmoothPoint = new PointF(useX, useY);
            if (_smoothEnabled)
                _smoothHistory.Add(_lastSmoothPoint);

            int x = ClampX(useX - this.Left);
            int y = ClampY(useY - this.Top);
            float w = _currentWidth > 0 ? _currentWidth : _penWidth;

            using (var pen = new Pen(_penColor, w))
            {
                pen.StartCap = LineCap.Round;
                pen.EndCap = LineCap.Round;
                _canvasGraphics.DrawEllipse(pen, x, y, w, w);
            }
            Console.WriteLine("[Draw] DOWN at ({0},{1})", useX, useY);
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

        public void OnPenMove(int fromSX, int fromSY, int toSX, int toSY)
        {
            if (!_isDrawing) return;

            int dx = toSX - _lastPoint.X;
            int dy = toSY - _lastPoint.Y;
            if (dx * dx + dy * dy < SmoothDistance * SmoothDistance) return;

            _smoothBuffer.Add(new Point(toSX, toSY));

            float useFromX, useFromY, useToX, useToY;
            if (_smoothEnabled)
            {
                // Smooth the incoming raw point
                var rawTo = new PointF(toSX, toSY);
                _smoothHistory.Add(rawTo);
                if (_smoothHistory.Count > SmoothWindow)
                    _smoothHistory.RemoveAt(0);

                // Weighted moving average
                float totalW = 0, sumX = 0, sumY = 0;
                for (int i = 0; i < _smoothHistory.Count; i++)
                {
                    float w = (float)(i + 1) / _smoothHistory.Count;
                    sumX += _smoothHistory[i].X * w;
                    sumY += _smoothHistory[i].Y * w;
                    totalW += w;
                }
                var smoothedTo = new PointF(sumX / totalW, sumY / totalW);

                // Use last smoothed point as FROM for perfect continuity (no gaps!)
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

            // Skip if smoothed coords haven't moved enough
            if (_smoothEnabled && (fx - tx) * (fx - tx) + (fy - ty) * (fy - ty) < 1) return;

            float lineW = _currentWidth > 0 ? _currentWidth : _penWidth;
            using (var pen = new Pen(_penColor, lineW))
            {
                pen.StartCap = LineCap.Round;
                pen.EndCap = LineCap.Round;
                pen.LineJoin = LineJoin.Round;
                _canvasGraphics.DrawLine(pen, fx, fy, tx, ty);
            }

            _lastPoint = new Point(toSX, toSY);
            this.Invalidate(new Rectangle(
                Math.Min(fx, tx) - 5, Math.Min(fy, ty) - 5,
                Math.Abs(tx - fx) + 10, Math.Abs(ty - fy) + 10));
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

            Console.WriteLine("[Draw] UP at ({0},{1})", _lastPenPos.X, _lastPenPos.Y);

            if (_smoothBuffer.Count > 0)
            {
                _completedStrokes.Add(new StrokeRecord
                {
                    Points = new List<Point>(_smoothBuffer),
                    Color = _penColor,
                    Width = _currentWidth > 0 ? _currentWidth : _penWidth
                });
            }
            _smoothBuffer.Clear();
            this.Invalidate();
        }

        public void OnPenUp(int screenX, int screenY)
        {
            if (!_isDrawing) return;
            _isDrawing = false;
            if (_smoothBuffer.Count > 0)
            {
                _completedStrokes.Add(new StrokeRecord
                {
                    Points = new List<Point>(_smoothBuffer),
                    Color = _penColor,
                    Width = _currentWidth > 0 ? _currentWidth : _penWidth
                });
            }
            _smoothBuffer.Clear();
            _smoothHistory.Clear();
            Console.WriteLine("[Draw] UP at ({0},{1})", screenX, screenY);
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
            this.Invalidate();
            Console.WriteLine("[Overlay] Canvas cleared.");
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
                    NativeMethods.UnregisterHotKey(this.Handle, 3);
                }
                if (_liftTimer != null) { _liftTimer.Stop(); _liftTimer.Dispose(); }
                if (_lockTimer != null) { _lockTimer.Stop(); _lockTimer.Dispose(); }
                if (_canvasGraphics != null) _canvasGraphics.Dispose();
                if (_canvas != null) _canvas.Dispose();
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
                cp.ExStyle |= NativeMethods.WS_EX_TRANSPARENT
                           | NativeMethods.WS_EX_NOACTIVATE
                           | NativeMethods.WS_EX_TOOLWINDOW
                           | NativeMethods.WS_EX_TOPMOST;
                // NOTE: NO WS_EX_LAYERED — using TransparencyKey instead
                return cp;
            }
        }
    }
}
