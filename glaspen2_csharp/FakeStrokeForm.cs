using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Windows.Forms;

namespace GlasPen2
{
    public class FakeStrokeForm : Form
    {
        private Bitmap _canvas;
        private Graphics _g;
        private Color _penColor = Color.Lime;
        private float _currentWidth;
        private bool _isDrawing;
        private Point _lastDirectPoint;
        private Point _lastCrosshair = new Point(-1, -1);
        private const int CROSSHAIR_RADIUS = 10;

        public FakeStrokeForm(Rectangle bounds)
        {
            this.StartPosition = FormStartPosition.Manual;
            this.Location = bounds.Location;
            this.Size = bounds.Size;
            this.FormBorderStyle = FormBorderStyle.None;
            this.ShowInTaskbar = false;
            this.TopMost = true;
            this.ShowIcon = false;
            this.BackColor = Color.Fuchsia;
            this.TransparencyKey = Color.Fuchsia;
            this.DoubleBuffered = false;

            // Canvas: transparent background, used as backing store for crosshair clearing.
            // Transparent pixels won't overwrite Fuchsia when drawn via DrawImage.
            _canvas = new Bitmap(bounds.Width, bounds.Height, PixelFormat.Format32bppArgb);
            _g = Graphics.FromImage(_canvas);
            _g.SmoothingMode = SmoothingMode.None;
            _g.CompositingQuality = CompositingQuality.Default;
            _g.InterpolationMode = InterpolationMode.NearestNeighbor;
            _g.PixelOffsetMode = PixelOffsetMode.None;
            _g.Clear(Color.Transparent);
        }

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

        private IntPtr GetWindowDC()
        {
            return NativeMethods.GetDC(this.Handle);
        }

        private void ReleaseWindowDC(IntPtr hdc)
        {
            NativeMethods.ReleaseDC(this.Handle, hdc);
        }

        public void BeginStroke(int x, int y, float width)
        {
            _currentWidth = width;
            _isDrawing = true;
            var pt = new Point(x, y);
            _lastDirectPoint = pt;

            // Draw to canvas (backing store)
            using (var pen = new Pen(_penColor, _currentWidth))
            {
                pen.StartCap = LineCap.Round;
                pen.EndCap = LineCap.Round;
                _g.DrawEllipse(pen, x - _currentWidth / 2, y - _currentWidth / 2, _currentWidth, _currentWidth);
            }
            // Draw to window DC directly (real-time)
            IntPtr hdc = GetWindowDC();
            if (hdc != IntPtr.Zero)
            {
                using (var g = Graphics.FromHdc(hdc))
                {
                    g.SmoothingMode = SmoothingMode.None;
                    using (var pen = new Pen(_penColor, _currentWidth))
                    {
                        pen.StartCap = LineCap.Round;
                        pen.EndCap = LineCap.Round;
                        g.DrawEllipse(pen, x - _currentWidth / 2, y - _currentWidth / 2, _currentWidth, _currentWidth);
                    }
                }
                ReleaseWindowDC(hdc);
            }
        }

        public void AddPoint(int x, int y, float width)
        {
            if (!_isDrawing) return;
            _currentWidth = width;
            var pt = new Point(x, y);

            // Draw to canvas
            using (var pen = new Pen(_penColor, _currentWidth))
            {
                pen.StartCap = LineCap.Round;
                pen.EndCap = LineCap.Round;
                pen.LineJoin = LineJoin.Round;
                _g.DrawLine(pen, _lastDirectPoint, pt);
            }
            // Draw to window DC
            IntPtr hdc = GetWindowDC();
            if (hdc != IntPtr.Zero)
            {
                using (var g = Graphics.FromHdc(hdc))
                {
                    g.SmoothingMode = SmoothingMode.None;
                    using (var pen = new Pen(_penColor, _currentWidth))
                    {
                        pen.StartCap = LineCap.Round;
                        pen.EndCap = LineCap.Round;
                        pen.LineJoin = LineJoin.Round;
                        g.DrawLine(pen, _lastDirectPoint, pt);
                    }
                }
                ReleaseWindowDC(hdc);
            }
            _lastDirectPoint = pt;
        }

        public void EndStroke()
        {
            _isDrawing = false;
        }

        /// <summary>
        /// Clear crosshair: fill region with Fuchsia (erases direct-drawn content),
        /// then restore strokes from canvas. Transparent canvas pixels don't overwrite Fuchsia.
        /// </summary>
        public void ClearCrosshair()
        {
            if (_lastCrosshair.X >= 0 && this.IsHandleCreated)
            {
                int r = CROSSHAIR_RADIUS;
                int pad = 2;
                var rect = new Rectangle(
                    _lastCrosshair.X - r - pad, _lastCrosshair.Y - r - pad,
                    r * 2 + pad * 2, r * 2 + pad * 2);

                IntPtr hdc = GetWindowDC();
                if (hdc != IntPtr.Zero)
                {
                    using (var g = Graphics.FromHdc(hdc))
                    {
                        // Erase direct-drawn crosshair with Fuchsia
                        g.FillRectangle(Brushes.Fuchsia, rect);
                        // Restore strokes from canvas (transparent pixels = no change)
                        g.DrawImage(_canvas, rect, rect, GraphicsUnit.Pixel);
                    }
                    ReleaseWindowDC(hdc);
                }
                _lastCrosshair = new Point(-1, -1);
            }
        }

        public void ClearAll()
        {
            _isDrawing = false;
            _lastCrosshair = new Point(-1, -1);
            _g.Clear(Color.Transparent);
            if (this.IsHandleCreated)
            {
                IntPtr hdc = GetWindowDC();
                if (hdc != IntPtr.Zero)
                {
                    using (var g = Graphics.FromHdc(hdc))
                    {
                        g.Clear(Color.Fuchsia);
                    }
                    ReleaseWindowDC(hdc);
                }
            }
        }

        /// <summary>
        /// Draw crosshair directly to window DC. Canvas not involved.
        /// </summary>
        public void DrawCrosshair(int x, int y)
        {
            if (!this.IsHandleCreated) return;
            int r = CROSSHAIR_RADIUS;
            int pad = 2;

            IntPtr hdc = GetWindowDC();
            if (hdc == IntPtr.Zero) return;
            try
            {
                using (var g = Graphics.FromHdc(hdc))
                {
                    g.SmoothingMode = SmoothingMode.None;

                    // Clear old crosshair: Fuchsia + restore canvas strokes
                    if (_lastCrosshair.X >= 0)
                    {
                        var oldRect = new Rectangle(
                            _lastCrosshair.X - r - pad, _lastCrosshair.Y - r - pad,
                            r * 2 + pad * 2, r * 2 + pad * 2);
                        g.FillRectangle(Brushes.Fuchsia, oldRect);
                        g.DrawImage(_canvas, oldRect, oldRect, GraphicsUnit.Pixel);
                    }

                    // Draw new crosshair
                    using (var pen = new Pen(Color.FromArgb(200, 0, 255, 0), 2f))
                    {
                        g.DrawLine(pen, x - r, y, x + r, y);
                        g.DrawLine(pen, x, y - r, x, y + r);
                        g.DrawEllipse(pen, x - r, y - r, r * 2, r * 2);
                    }
                }
            }
            finally
            {
                ReleaseWindowDC(hdc);
            }

            _lastCrosshair = new Point(x, y);
        }

        /// <summary>
        /// No canvas drawn to window — avoids black border from DrawImage compositing.
        /// </summary>
        protected override void OnPaint(PaintEventArgs e)
        {
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing)
            {
                if (_g != null) _g.Dispose();
                if (_canvas != null) _canvas.Dispose();
            }
            base.Dispose(disposing);
        }
    }
}
