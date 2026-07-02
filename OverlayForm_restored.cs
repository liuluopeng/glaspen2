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
    /// Full-screen transparent overlay for pen drawing.
    ///
    /// Architecture (INK-only):
    ///   - WS_EX_TRANSPARENT: pen events pass through to Windows INK
    ///   - Raw Input HID: receives pen coordinates + pressure (RIDEV_INPUTSINK)
    ///   - HID tipDown directly drives OnPenDown/OnPenMove/OnPenUp
    ///   - No hook, no WM_POINTER dependency
    ///   - DPI-aware: canvas = physical screen pixels
    /// </summary>
    public class OverlayForm : Form
    {
        // ── Canvas ──
        private Bitmap _canvas;
        private Graphics _canvasGraphics;

        // ── Drawing state ──
        private bool _isDrawing;
        private Point _lastPoint;
        private readonly List<StrokeRecord> _completedStrokes = new List<StrokeRecord>();
        private Color _penColor = Color.Red;
        private float _penWidth = 2.0f;

        // ── Pen state from HID ──
        private bool _hidTipDown;
        private uint _hidPressure;
        private int _hidX, _hidY;
        private DateTime _hidLastReportUtc = DateTime.MinValue;

        // ── Pressure-driven width ──
        private float _currentWidth;
        private double _penR = 1.0, _penG = 0.0, _penB = 0.0;
        private double _widthScale = 1.0;

        // ── Rust modeler ──
        private bool _rustModelerAvailable;

        // ── Smoothing (C# fallback) ──
        private bool _smoothEnabled = true;
        private readonly List<PointF> _smoothHistory = new List<PointF>();
        private const int SmoothWindow = 5;
        private PointF _lastSmoothPoint;

        // ── Pen cursor ──
        private bool _showPenCursor;
        private Point _penCursorPos;
        private IntPtr _transparentCursor = IntPtr.Zero;

        // ── Safety lift timer ──
        private Timer _liftTimer;

        // ── Logging ──
        private int _hidCount;
        private int _paintCount;

        private static void Log(string msg) { Program.Log(msg); }
        private static void Log(string fmt, params object[] args) { Program.Log(fmt, args); }

        // ── Properties ──
        public Color PenColor
        {
            get { return _penColor; }
            set
            {
                _penColor = value;
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

        public double WidthScale
        {
            get { return _widthScale; }
            set { _widthScale = Math.Max(0.1, Math.Min(10.0, value)); }
        }

        public bool SmoothEnabled
        {
            get { return _smoothEnabled; }
            set { _smoothEnabled = value; if (!value) _smoothHistory.Clear(); }
        }

        // ── Window style: transparent + topmost + no-activate ──
        protected override CreateParams CreateParams
        {
            get
            {
                var cp = base.CreateParams;
                cp.ExStyle |= NativeMethods.WS_EX_TRANSPARENT  // input passes through
                           | NativeMethods.WS_EX_NOACTIVATE    // no focus steal
                           | NativeMethods.WS_EX_TOOLWINDOW    // no taskbar
                           | NativeMethods.WS_EX_TOPMOST;      // always on top
                return cp;
            }
        }

        protected override bool ShowWithoutActivation { get { return true; } }

        // ── Constructor ──
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

            // TransparencyKey: Fuchsia pixels are transparent
            this.BackColor = Color.Fuchsia;
            this.TransparencyKey = Color.Fuchsia;
            this.DoubleBuffered = true;

            // Canvas at physical screen resolution
            _canvas = new Bitmap(bounds.Width, bounds.Height, PixelFormat.Format32bppArgb);
            _canvasGraphics = Graphics.FromImage(_canvas);
            _canvasGraphics.SmoothingMode = SmoothingMode.AntiAlias;
            _canvasGraphics.Clear(Color.Transparent);

            Log("[Overlay] Canvas: {0}x{1}, Location=({2},{3})",
                bounds.Width, bounds.Height, bounds.Left, bounds.Top);

            // Transparent cursor
            CreateTransparentCursor();

            // Probe Rust modeler
            try
            {
                GlaspenNative.glaspen2_now_secs();
                _rustModelerAvailable = true;
                Log("[Overlay] Rust modeler available");
            }
            catch
            {
                _rustModelerAvailable = false;
                Log("[Overlay] Rust modeler NOT available — using C# fallback");
            }
        }

        // ── Handle created ──
        protected override void OnHandleCreated(EventArgs e)
        {
            base.OnHandleCreated(e);

            // Register Raw Input (background: RIDEV_INPUTSINK)
            RegisterRawInput();

            // Set topmost
            NativeMethods.SetWindowPos(this.Handle, NativeMethods.HWND_TOPMOST,
                this.Left, this.Top, this.Width, this.Height,
                NativeMethods.SWP_NOACTIVATE | NativeMethods.SWP_SHOWWINDOW);

            // Safety lift timer
            _liftTimer = new Timer { Interval = 200 };
            _liftTimer.Tick += (s, args) =>
            {
                if (_isDrawing && !_hidTipDown
                    && (DateTime.UtcNow - _hidLastReportUtc).TotalMilliseconds > 2000)
                {
                    Log("[Draw] SAFETY LIFT (no HID for 2s)");
                    StopDrawing();
                }
            };
            _liftTimer.Start();

            Log("[Overlay] Handle=0x{0:X}, Raw Input registered", this.Handle.ToInt64());
        }

        // ── Register Raw Input ──
        private void RegisterRawInput()
        {
            var devices = new NativeMethods.RAWINPUTDEVICE[3];

            // Mouse (for absolute positioning fallback)
            devices[0].usUsagePage = 0x0001;
            devices[0].usUsage = 0x0002;
            devices[0].dwFlags = NativeMethods.RIDEV_INPUTSINK;
            devices[0].hwndTarget = this.Handle;

            // Pen (Digitizer)
            devices[1].usUsagePage = 0x000D;
            devices[1].usUsage = 0x0002;
            devices[1].dwFlags = NativeMethods.RIDEV_INPUTSINK;
            devices[1].hwndTarget = this.Handle;

            // Stylus (Digitizer)
            devices[2].usUsagePage = 0x000D;
            devices[2].usUsage = 0x0001;
            devices[2].dwFlags = NativeMethods.RIDEV_INPUTSINK;
            devices[2].hwndTarget = this.Handle;

            uint cbSize = (uint)Marshal.SizeOf(typeof(NativeMethods.RAWINPUTDEVICE));
            bool ok = NativeMethods.RegisterRawInputDevices(devices, 3, cbSize);
            int err = Marshal.GetLastWin32Error();
            Log("[Overlay] RegisterRawInputDevices: {0} (err={1})", ok ? "OK" : "FAIL", err);
        }

        // ── WndProc ──
        protected override void WndProc(ref Message m)
        {
            // Disable INK visual feedback on our overlay
            if (m.Msg == NativeMethods.WM_TABLET_QUERYSYSTEMGESTURESTATUS)
            {
                m.Result = (IntPtr)NativeMethods.TABLET_DISABLE_ALL;
                return;
            }

            // Hide system pen cursor
            if (m.Msg == NativeMethods.WM_SETCURSOR)
            {
                int hitTest = (int)(m.LParam.ToInt64() & 0xFFFF);
                if (hitTest == NativeMethods.HTCLIENT && _transparentCursor != IntPtr.Zero)
                {
                    NativeMethods.SetCursor(_transparentCursor);
                    m.Result = (IntPtr)1;
                    return;
                }
            }

            // Raw Input: the ONLY source of pen data
            if (m.Msg == NativeMethods.WM_INPUT)
            {
                ProcessRawInput(m.LParam);
            }

            base.WndProc(ref m);
        }

        // ── Process Raw Input ──
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

        // ── Process HID Input: the core pen data source ──
        // Format: [reportId:1][switches:1][X:2 LE][Y:2 LE][pressure:2 LE]
        private void ProcessHidInput(IntPtr buffer, int offset, int dataLen)
        {
            _hidCount++;
            if (dataLen < 8)
            {
                if (_hidCount <= 5)
                    Log("[HID #{0}] dataLen={1} (too short)", _hidCount, dataLen);
                return;
            }

            int baseOff = offset + 8; // skip dwSizeHid(4) + dwCount(4)
            byte switches = Marshal.ReadByte(buffer, baseOff + 1);
            uint x = (uint)Marshal.ReadByte(buffer, baseOff + 2)
                  | ((uint)Marshal.ReadByte(buffer, baseOff + 3) << 8);
            uint y = (uint)Marshal.ReadByte(buffer, baseOff + 4)
                  | ((uint)Marshal.ReadByte(buffer, baseOff + 5) << 8);
            uint pressure = (uint)Marshal.ReadByte(buffer, baseOff + 6)
                         | ((uint)Marshal.ReadByte(buffer, baseOff + 7) << 8);

            bool tipDown = (switches & 0x01) != 0;
            bool inRange = (switches & 0x10) != 0;

            // Update HID state
            _hidTipDown = tipDown;
            _hidPressure = pressure;
            _hidLastReportUtc = DateTime.UtcNow;

            // Map HID logical coords (0-65535) → screen physical pixels
            var sb = SystemInformation.VirtualScreen;
            int screenX = sb.Left + (int)((long)x * sb.Width / 65536);
            int screenY = sb.Top + (int)((long)y * sb.Height / 65536);
            _hidX = screenX;
            _hidY = screenY;

            // Update pen cursor position
            int dxc = screenX - _penCursorPos.X;
            int dyc = screenY - _penCursorPos.Y;
            if (dxc * dxc + dyc * dyc >= 4)
            {
                _penCursorPos = new Point(screenX, screenY);
                _showPenCursor = true;
                this.Invalidate(new Rectangle(
                    ClampX(screenX - this.Left) - 15,
                    ClampY(screenY - this.Top) - 15, 30, 30));
            }

            // Log first few HID reports and every 100th
            if (_hidCount <= 20 || _hidCount % 200 == 0)
                Log("[HID #{0}] x={1} y={2} screen=({3},{4}) pressure={5} tip={6} range={7}",
                    _hidCount, x, y, screenX, screenY, pressure, tipDown, inRange);

            // Drive drawing from HID
            if (!Program.DrawingEnabled) return;

            if (tipDown && pressure > 0)
            {
                SetPressure(pressure);
                if (!_isDrawing)
                {
                    Log("[HID] TIP DOWN at ({0},{1}) pressure={2}", screenX, screenY, pressure);
                    OnPenDown(screenX, screenY);
                }
                else
                {
                    OnPenMove(_lastPoint.X, _lastPoint.Y, screenX, screenY);
                }
            }
            else if (!tipDown && _isDrawing)
            {
                Log("[HID] TIP UP at ({0},{1})", screenX, screenY);
                OnPenUp(screenX, screenY);
            }
        }

        // ── Pressure → width ──
        private void SetPressure(uint pressure)
        {
            float factor = 0.3f + (pressure / 1024f) * 1.7f;
            _currentWidth = _penWidth * factor;
        }

        // ── Drawing ──
        private void OnPenDown(int screenX, int screenY)
        {
            if (_isDrawing) return;
            _isDrawing = true;
            _lastPoint = new Point(screenX, screenY);
            _lastSmoothPoint = new PointF(screenX, screenY);
            _smoothHistory.Clear();

            float w = _currentWidth > 0 ? _currentWidth : _penWidth;
            int x = ClampX(screenX - this.Left);
            int y = ClampY(screenY - this.Top);

            using (var pen = new Pen(_penColor, w))
            {
                pen.StartCap = LineCap.Round;
                pen.EndCap = LineCap.Round;
                _canvasGraphics.DrawEllipse(pen, x, y, w, w);
            }

            // Init Rust modeler
            if (_rustModelerAvailable && _smoothEnabled)
            {
                GlaspenNative.glaspen2_modeler_clear_buffer();
                double p = (_hidPressure > 0) ? _hidPressure / 1024.0 : 0.5;
                double ts = GlaspenNative.glaspen2_now_secs();
                GlaspenNative.glaspen2_modeler_begin(
                    _penR, _penG, _penB, screenX, screenY, p, ts, _widthScale);
                GlaspenNative.glaspen2_modeler_clear_buffer();
            }

            this.Invalidate(new Rectangle(x - (int)w - 2, y - (int)w - 2, (int)w * 2 + 4, (int)w * 2 + 4));
        }

        private void OnPenMove(int fromX, int fromY, int toX, int toY)
        {
            if (!_isDrawing) return;
            int dx = toX - _lastPoint.X;
            int dy = toY - _lastPoint.Y;
            if (dx * dx + dy * dy < 4) return;

            if (_rustModelerAvailable && _smoothEnabled)
            {
                double p = (_hidPressure > 0) ? _hidPressure / 1024.0 : 0.5;
                double ts = GlaspenNative.glaspen2_now_secs();
                GlaspenNative.glaspen2_modeler_move(toX, toY, p, ts, _widthScale);
                float lineW = _currentWidth > 0 ? _currentWidth : _penWidth;
                DrawModelerBuffer(lineW);
            }
            else
            {
                // C# fallback: weighted moving average
                float useFromX, useFromY, useToX, useToY;
                if (_smoothEnabled)
                {
                    _smoothHistory.Add(new PointF(toX, toY));
                    if (_smoothHistory.Count > SmoothWindow) _smoothHistory.RemoveAt(0);
                    float tw = 0, sx = 0, sy = 0;
                    for (int i = 0; i < _smoothHistory.Count; i++)
                    {
                        float w = (float)(i + 1) / _smoothHistory.Count;
                        sx += _smoothHistory[i].X * w;
                        sy += _smoothHistory[i].Y * w;
                        tw += w;
                    }
                    var smoothed = new PointF(sx / tw, sy / tw);
                    useFromX = _lastSmoothPoint.X; useFromY = _lastSmoothPoint.Y;
                    useToX = smoothed.X; useToY = smoothed.Y;
                    _lastSmoothPoint = smoothed;
                }
                else
                {
                    useFromX = fromX; useFromY = fromY; useToX = toX; useToY = toY;
                }

                int fx = ClampX((int)useFromX - this.Left);
                int fy = ClampY((int)useFromY - this.Top);
                int tx = ClampX((int)useToX - this.Left);
                int ty = ClampY((int)useToY - this.Top);

                float lw = _currentWidth > 0 ? _currentWidth : _penWidth;
                using (var pen = new Pen(_penColor, lw))
                {
                    pen.StartCap = LineCap.Round;
                    pen.EndCap = LineCap.Round;
                    pen.LineJoin = LineJoin.Round;
                    _canvasGraphics.DrawLine(pen, fx, fy, tx, ty);
                }
            }

            _lastPoint = new Point(toX, toY);
            // Invalidate only dirty region
            int minX = Math.Min(ClampX(fromX - this.Left), ClampX(toX - this.Left));
            int minY = Math.Min(ClampY(fromY - this.Top), ClampY(toY - this.Top));
            int maxX = Math.Max(ClampX(fromX - this.Left), ClampX(toX - this.Left));
            int maxY = Math.Max(ClampY(fromY - this.Top), ClampY(toY - this.Top));
            int pad = (int)(_penWidth * 3) + 4;
            this.Invalidate(new Rectangle(minX - pad, minY - pad, maxX - minX + pad * 2, maxY - minY + pad * 2));
        }

        private void OnPenUp(int screenX, int screenY)
        {
            if (!_isDrawing) return;
            _isDrawing = false;

            if (_rustModelerAvailable && _smoothEnabled)
            {
                double p = (_hidPressure > 0) ? _hidPressure / 1024.0 : 0.5;
                double ts = GlaspenNative.glaspen2_now_secs();
                GlaspenNative.glaspen2_modeler_end(screenX, screenY, p, ts, _widthScale);
                GlaspenNative.glaspen2_modeler_clear_buffer();
                GlaspenNative.glaspen2_modeler_commit_to_strokes(_penR, _penG, _penB, IntPtr.Zero, 0);
            }

            _smoothHistory.Clear();
            this.Invalidate();
        }

        // ── Draw smoothed points from Rust modeler ──
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

        // ── Paint ──
        protected override void OnPaint(PaintEventArgs e)
        {
            _paintCount++;
            if (_canvas != null) e.Graphics.DrawImage(_canvas, 0, 0);

            // Pen cursor crosshair
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

        // ── Clear all ──
        public void ClearAll()
        {
            _completedStrokes.Clear();
            _isDrawing = false;
            _canvasGraphics.Clear(Color.Transparent);
            if (_rustModelerAvailable)
                GlaspenNative.glaspen2_clear_strokes(this.Width, this.Height);
            this.Invalidate();
            Log("[Overlay] Cleared");
        }

        // ── Helpers ──
        private int ClampX(int x) { return x < 0 ? 0 : (x >= _canvas.Width ? _canvas.Width - 1 : x); }
        private int ClampY(int y) { return y < 0 ? 0 : (y >= _canvas.Height ? _canvas.Height - 1 : y); }

        private void CreateTransparentCursor()
        {
            byte[] andPlane = new byte[] { 0xFF };
            byte[] xorPlane = new byte[] { 0x00 };
            _transparentCursor = NativeMethods.CreateCursor(IntPtr.Zero, 0, 0, 1, 1, andPlane, xorPlane);
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing)
            {
                if (_liftTimer != null) { _liftTimer.Stop(); _liftTimer.Dispose(); }
                if (_canvasGraphics != null) _canvasGraphics.Dispose();
                if (_canvas != null) _canvas.Dispose();
                if (_transparentCursor != IntPtr.Zero)
                    NativeMethods.DestroyCursor(_transparentCursor);
            }
            base.Dispose(disposing);
        }

        private class StrokeRecord
        {
            public List<Point> Points;
            public Color Color;
            public float Width;
        }
    }
}
