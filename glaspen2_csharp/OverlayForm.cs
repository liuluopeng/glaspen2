using System;
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

        // Dynamic HID coordinate range
        private uint _hidMinX = 100, _hidMaxX = 25000;
        private uint _hidMinY = 100, _hidMaxY = 16000;

        // Drawing state
        private bool _isDrawing;
        private Point _lastPoint;
        private Color _penColor = Color.Red;
        private float _penWidth = 1.5f;
        private float _currentWidth;

        // Cursor
        private IntPtr _transparentCursor = IntPtr.Zero;
        private Point _cursorPos;
        private bool _showCursor;

        private static void Log(string msg) { Program.Log(msg); }
        private static void Log(string fmt, params object[] args) { Program.Log(fmt, args); }

        protected override CreateParams CreateParams
        {
            get
            {
                var cp = base.CreateParams;
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
            this.BackColor = Color.Fuchsia;
            this.TransparencyKey = Color.Fuchsia;
            this.DoubleBuffered = true;

            _canvas = new Bitmap(bounds.Width, bounds.Height, PixelFormat.Format32bppArgb);
            _g = Graphics.FromImage(_canvas);
            _g.SmoothingMode = SmoothingMode.AntiAlias;
            _g.Clear(Color.Transparent);

            // Transparent cursor (hide system pen cursor, draw our own)
            byte[] andPlane = { 0xFF };
            byte[] xorPlane = { 0x00 };
            _transparentCursor = NativeMethods.CreateCursor(IntPtr.Zero, 0, 0, 1, 1, andPlane, xorPlane);

            Log("[Overlay] Canvas: {0}x{1}, Location=({2},{3}), penWidth={4}",
                bounds.Width, bounds.Height, bounds.Left, bounds.Top, _penWidth);
        }

        protected override void OnHandleCreated(EventArgs e)
        {
            base.OnHandleCreated(e);
            RegisterRawInput();
            NativeMethods.SetWindowPos(this.Handle, NativeMethods.HWND_TOPMOST,
                this.Left, this.Top, this.Width, this.Height,
                NativeMethods.SWP_NOACTIVATE | NativeMethods.SWP_SHOWWINDOW);
            Log("[Overlay] Ready. Handle=0x{0:X}", this.Handle.ToInt64());
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

        // HID report: [reportId:1][switches:1][X:2 LE][Y:2 LE][pressure:2 LE]
        private void ProcessHidInput(IntPtr buffer, int offset, int dataLen)
        {
            _hidCount++;
            if (dataLen < 8) return;

            int b = offset + 8;
            byte switches = Marshal.ReadByte(buffer, b + 1);
            uint rawX = (uint)Marshal.ReadByte(buffer, b + 2) | ((uint)Marshal.ReadByte(buffer, b + 3) << 8);
            uint rawY = (uint)Marshal.ReadByte(buffer, b + 4) | ((uint)Marshal.ReadByte(buffer, b + 5) << 8);
            uint press = (uint)Marshal.ReadByte(buffer, b + 6) | ((uint)Marshal.ReadByte(buffer, b + 7) << 8);

            bool tipDown = (switches & 0x01) != 0;
            bool inRange = (switches & 0x10) != 0;

            // Track HID range
            if (rawX > 100 && rawX < 65535) { if (rawX < _hidMinX) _hidMinX = rawX; if (rawX > _hidMaxX) _hidMaxX = rawX; }
            if (rawY > 100 && rawY < 65535) { if (rawY < _hidMinY) _hidMinY = rawY; if (rawY > _hidMaxY) _hidMaxY = rawY; }

            // Map to screen
            long rangeX = _hidMaxX - _hidMinX;
            long rangeY = _hidMaxY - _hidMinY;
            var sb = SystemInformation.VirtualScreen;
            int sx = (rangeX > 0) ? sb.Left + (int)((long)(rawX - _hidMinX) * sb.Width / rangeX) : (int)rawX;
            int sy = (rangeY > 0) ? sb.Top + (int)((long)(rawY - _hidMinY) * sb.Height / rangeY) : (int)rawY;

            bool tipChanged = tipDown != _tipDown;
            bool rangeChanged = inRange != _inRange;

            _tipDown = tipDown;
            _inRange = inRange;
            _pressure = press;
            _screenX = sx;
            _screenY = sy;

            // Log
            if (_hidCount <= 50 || tipChanged || rangeChanged || _hidCount % 100 == 0)
                Log("[HID #{0}] raw=({1},{2}) screen=({3},{4}) pressure={5} tip={6} range={7}",
                    _hidCount, rawX, rawY, sx, sy, press,
                    tipDown ? "DOWN" : "UP",
                    inRange ? "YES" : "NO");

            // Update cursor
            int dx = sx - _cursorPos.X;
            int dy = sy - _cursorPos.Y;
            if (dx * dx + dy * dy >= 4)
            {
                _cursorPos = new Point(sx, sy);
                _showCursor = true;
                this.Invalidate(new Rectangle(
                    ClampX(sx - this.Left) - 15,
                    ClampY(sy - this.Top) - 15, 30, 30));
            }

            // Drive drawing
            if (tipDown && press > 0)
            {
                _currentWidth = _penWidth * (0.3f + (press / 16000f) * 1.7f);
                if (!_isDrawing)
                {
                    _isDrawing = true;
                    _lastPoint = new Point(sx, sy);
                    int cx = ClampX(sx - this.Left);
                    int cy = ClampY(sy - this.Top);
                    using (var pen = new Pen(_penColor, _currentWidth))
                    {
                        pen.StartCap = LineCap.Round;
                        _g.DrawEllipse(pen, cx, cy, _currentWidth, _currentWidth);
                    }
                    this.Invalidate(new Rectangle(cx - 10, cy - 10, 20, 20));
                }
                else
                {
                    int fx = ClampX(_lastPoint.X - this.Left);
                    int fy = ClampY(_lastPoint.Y - this.Top);
                    int tx = ClampX(sx - this.Left);
                    int ty = ClampY(sy - this.Top);
                    using (var pen = new Pen(_penColor, _currentWidth))
                    {
                        pen.StartCap = LineCap.Round;
                        pen.EndCap = LineCap.Round;
                        _g.DrawLine(pen, fx, fy, tx, ty);
                    }
                    int minX = Math.Min(fx, tx) - (int)_currentWidth - 2;
                    int minY = Math.Min(fy, ty) - (int)_currentWidth - 2;
                    int maxX = Math.Max(fx, tx) + (int)_currentWidth + 2;
                    int maxY = Math.Max(fy, ty) + (int)_currentWidth + 2;
                    this.Invalidate(new Rectangle(minX, minY, maxX - minX, maxY - minY));
                    _lastPoint = new Point(sx, sy);
                }
            }
            else if (!tipDown && _isDrawing)
            {
                _isDrawing = false;
                this.Invalidate();
            }
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            if (_canvas != null) e.Graphics.DrawImage(_canvas, 0, 0);

            if (_showCursor && _cursorPos.X > 0)
            {
                int cx = ClampX(_cursorPos.X - this.Left);
                int cy = ClampY(_cursorPos.Y - this.Top);
                int r = 10;
                using (var pen = new Pen(Color.FromArgb(200, 255, 80, 30), 2f))
                {
                    e.Graphics.DrawLine(pen, cx - r, cy, cx + r, cy);
                    e.Graphics.DrawLine(pen, cx, cy - r, cx, cy + r);
                    e.Graphics.DrawEllipse(pen, cx - r, cy - r, r * 2, r * 2);
                }
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
                if (_g != null) _g.Dispose();
                if (_canvas != null) _canvas.Dispose();
                if (_transparentCursor != IntPtr.Zero)
                    NativeMethods.DestroyCursor(_transparentCursor);
            }
            base.Dispose(disposing);
        }
    }
}
