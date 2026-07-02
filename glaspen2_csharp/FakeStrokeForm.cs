using System;
using System.Collections.Generic;
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
        private float _penWidth = 2.5f;
        private float _currentWidth;
        private bool _isDrawing;
        private readonly List<Point> _recentPoints = new List<Point>();
        private const int MAX_RECENT = 8;
        private Point _lastDirectPoint;

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
            this.DoubleBuffered = true;

            _canvas = new Bitmap(bounds.Width, bounds.Height, PixelFormat.Format32bppArgb);
            _g = Graphics.FromImage(_canvas);
            _g.SmoothingMode = SmoothingMode.AntiAlias;
            _g.CompositingQuality = CompositingQuality.HighQuality;
            _g.InterpolationMode = InterpolationMode.HighQualityBicubic;
            _g.PixelOffsetMode = PixelOffsetMode.HighQuality;
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

        /// <summary>
        /// Draw just one line segment directly to window DC — fast, no full-bitmap copy.
        /// </summary>
        private void DirectDrawLine(Point from, Point to, float width)
        {
            if (!this.IsHandleCreated) return;
            IntPtr hdc = NativeMethods.GetDC(this.Handle);
            if (hdc == IntPtr.Zero) return;
            try
            {
                using (var g = Graphics.FromHdc(hdc))
                {
                    g.SmoothingMode = SmoothingMode.AntiAlias;
                    using (var pen = new Pen(_penColor, width))
                    {
                        pen.StartCap = LineCap.Round;
                        pen.EndCap = LineCap.Round;
                        pen.LineJoin = LineJoin.Round;
                        g.DrawLine(pen, from, to);
                    }
                }
            }
            finally
            {
                NativeMethods.ReleaseDC(this.Handle, hdc);
            }
        }

        private void DirectDrawEllipse(Point center, float width)
        {
            if (!this.IsHandleCreated) return;
            IntPtr hdc = NativeMethods.GetDC(this.Handle);
            if (hdc == IntPtr.Zero) return;
            try
            {
                using (var g = Graphics.FromHdc(hdc))
                {
                    g.SmoothingMode = SmoothingMode.AntiAlias;
                    using (var pen = new Pen(_penColor, width))
                    {
                        pen.StartCap = LineCap.Round;
                        pen.EndCap = LineCap.Round;
                        g.DrawEllipse(pen, center.X - width / 2, center.Y - width / 2, width, width);
                    }
                }
            }
            finally
            {
                NativeMethods.ReleaseDC(this.Handle, hdc);
            }
        }

        public void BeginStroke(int x, int y, float width)
        {
            _currentWidth = width;
            _isDrawing = true;
            _recentPoints.Clear();
            var pt = new Point(x, y);
            _recentPoints.Add(pt);
            _lastDirectPoint = pt;

            // Draw to persistent canvas
            using (var pen = new Pen(_penColor, _currentWidth))
            {
                pen.StartCap = LineCap.Round;
                pen.EndCap = LineCap.Round;
                _g.DrawEllipse(pen, x - _currentWidth / 2, y - _currentWidth / 2, _currentWidth, _currentWidth);
            }
            // Draw directly to screen
            DirectDrawEllipse(pt, _currentWidth);
        }

        public void AddPoint(int x, int y)
        {
            if (!_isDrawing) return;
            var pt = new Point(x, y);
            _recentPoints.Add(pt);
            if (_recentPoints.Count > MAX_RECENT)
                _recentPoints.RemoveAt(0);

            // Draw to persistent canvas
            using (var pen = new Pen(_penColor, _currentWidth))
            {
                pen.StartCap = LineCap.Round;
                pen.EndCap = LineCap.Round;
                pen.LineJoin = LineJoin.Round;

                if (_recentPoints.Count >= 3)
                    _g.DrawCurve(pen, _recentPoints.ToArray(), 0.5f);
                else if (_recentPoints.Count == 2)
                    _g.DrawLine(pen, _recentPoints[0], _recentPoints[1]);
            }
            // Draw just the new segment directly to screen
            DirectDrawLine(_lastDirectPoint, pt, _currentWidth);
            _lastDirectPoint = pt;
        }

        public void EndStroke()
        {
            _recentPoints.Clear();
            _isDrawing = false;
        }

        public void ClearAll()
        {
            _isDrawing = false;
            _g.Clear(Color.Transparent);
            // Full repaint needed for clear
            if (this.IsHandleCreated)
                this.Invalidate();
        }

        /// <summary>
        /// Draw a green crosshair directly to window DC.
        /// </summary>
        public void DrawCrosshair(int x, int y)
        {
            if (!this.IsHandleCreated) return;
            IntPtr hdc = NativeMethods.GetDC(this.Handle);
            if (hdc == IntPtr.Zero) return;
            try
            {
                using (var g = Graphics.FromHdc(hdc))
                {
                    int r = 10;
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
                NativeMethods.ReleaseDC(this.Handle, hdc);
            }
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            if (_canvas != null) e.Graphics.DrawImage(_canvas, 0, 0);
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
