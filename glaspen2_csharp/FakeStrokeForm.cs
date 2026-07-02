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
        private float _currentWidth;
        private bool _isDrawing;
        private Point _lastDirectPoint;
        private Point _lastCrosshair = new Point(-1, -1);
        private const int CROSSHAIR_RADIUS = 10;

        // Catmull-Rom spline smoothing buffer
        private readonly List<PointF> _pointBuffer = new List<PointF>();
        private int _unprocessedIndex;

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

        public void SetColor(Color color)
        {
            _penColor = color;
        }

        private IntPtr GetWindowDC()
        {
            return NativeMethods.GetDC(this.Handle);
        }

        private void ReleaseWindowDC(IntPtr hdc)
        {
            NativeMethods.ReleaseDC(this.Handle, hdc);
        }

        /// <summary>
        /// Catmull-Rom to cubic Bezier conversion (ref rnote).
        /// Given 4 points, returns control points for the segment p1→p2.
        /// </summary>
        private static void CatmullRomToBezier(PointF p0, PointF p1, PointF p2, PointF p3,
            out PointF cp1, out PointF cp2)
        {
            cp1 = new PointF(p1.X + (p2.X - p0.X) / 6f, p1.Y + (p2.Y - p0.Y) / 6f);
            cp2 = new PointF(p2.X - (p3.X - p1.X) / 6f, p2.Y - (p3.Y - p1.Y) / 6f);
        }

        private void DrawDot(PointF center, float width)
        {
            float r = width / 2f;
            using (var pen = new Pen(_penColor, width))
            {
                pen.StartCap = LineCap.Round;
                pen.EndCap = LineCap.Round;
                _g.DrawEllipse(pen, center.X - r, center.Y - r, width, width);
            }
            IntPtr hdc = GetWindowDC();
            if (hdc != IntPtr.Zero)
            {
                using (var g = Graphics.FromHdc(hdc))
                {
                    g.SmoothingMode = SmoothingMode.None;
                    using (var pen = new Pen(_penColor, width))
                    {
                        pen.StartCap = LineCap.Round;
                        pen.EndCap = LineCap.Round;
                        g.DrawEllipse(pen, center.X - r, center.Y - r, width, width);
                    }
                }
                ReleaseWindowDC(hdc);
            }
        }

        public void BeginStroke(int x, int y, float width)
        {
            _currentWidth = width;
            _isDrawing = true;
            _pointBuffer.Clear();
            _unprocessedIndex = 0;
            var pt = new PointF(x, y);
            _pointBuffer.Add(pt);
            _lastDirectPoint = new Point(x, y);
            DrawDot(pt, _currentWidth);
        }

        public void AddPoint(int x, int y, float width)
        {
            if (!_isDrawing) return;
            _currentWidth = width;
            var pt = new PointF(x, y);
            _pointBuffer.Add(pt);

            // Process all available Catmull-Rom segments, drawing to ONE window DC
            if (_unprocessedIndex + 3 < _pointBuffer.Count)
            {
                IntPtr hdc = GetWindowDC();
                try
                {
                    Graphics winG = (hdc != IntPtr.Zero) ? Graphics.FromHdc(hdc) : null;
                    try
                    {
                        if (winG != null) winG.SmoothingMode = SmoothingMode.None;

                        // Reuse pen objects
                        using (var canvasPen = new Pen(_penColor, _currentWidth))
                        using (var winPen = (winG != null) ? new Pen(_penColor, _currentWidth) : null)
                        {
                            canvasPen.StartCap = LineCap.Round;
                            canvasPen.EndCap = LineCap.Round;
                            canvasPen.LineJoin = LineJoin.Round;
                            if (winPen != null)
                            {
                                winPen.StartCap = LineCap.Round;
                                winPen.EndCap = LineCap.Round;
                                winPen.LineJoin = LineJoin.Round;
                            }

                            while (_unprocessedIndex + 3 < _pointBuffer.Count)
                            {
                                PointF p0 = _pointBuffer[_unprocessedIndex];
                                PointF p1 = _pointBuffer[_unprocessedIndex + 1];
                                PointF p2 = _pointBuffer[_unprocessedIndex + 2];
                                PointF p3 = _pointBuffer[_unprocessedIndex + 3];

                                PointF cp1, cp2;
                                CatmullRomToBezier(p0, p1, p2, p3, out cp1, out cp2);

                                try { _g.DrawBezier(canvasPen, p1, cp1, cp2, p2); }
                                catch { _g.DrawLine(canvasPen, p1, p2); } // fallback

                                if (winG != null && winPen != null)
                                {
                                    try { winG.DrawBezier(winPen, p1, cp1, cp2, p2); }
                                    catch { winG.DrawLine(winPen, p1, p2); } // fallback
                                }

                                _unprocessedIndex++;
                            }
                        }
                    }
                    finally
                    {
                        if (winG != null) winG.Dispose();
                    }
                }
                finally
                {
                    if (hdc != IntPtr.Zero) ReleaseWindowDC(hdc);
                }
            }

            _lastDirectPoint = new Point(x, y);
        }

        public void EndStroke()
        {
            // Draw remaining segments as straight lines
            if (_unprocessedIndex + 1 < _pointBuffer.Count)
            {
                IntPtr hdc = GetWindowDC();
                try
                {
                    using (var winG = (hdc != IntPtr.Zero) ? Graphics.FromHdc(hdc) : null)
                    {
                        if (winG != null) winG.SmoothingMode = SmoothingMode.None;

                        while (_unprocessedIndex + 1 < _pointBuffer.Count)
                        {
                            PointF from = _pointBuffer[_unprocessedIndex];
                            PointF to = _pointBuffer[_unprocessedIndex + 1];

                            using (var pen = new Pen(_penColor, _currentWidth))
                            {
                                pen.StartCap = LineCap.Round;
                                pen.EndCap = LineCap.Round;
                                _g.DrawLine(pen, from, to);
                            }
                            if (winG != null)
                            {
                                using (var pen = new Pen(_penColor, _currentWidth))
                                {
                                    pen.StartCap = LineCap.Round;
                                    pen.EndCap = LineCap.Round;
                                    winG.DrawLine(pen, from, to);
                                }
                            }
                            _unprocessedIndex++;
                        }
                    }
                }
                finally
                {
                    if (hdc != IntPtr.Zero) ReleaseWindowDC(hdc);
                }
            }

            _pointBuffer.Clear();
            _unprocessedIndex = 0;
            _isDrawing = false;
        }

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
                        g.FillRectangle(Brushes.Fuchsia, rect);
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
            _pointBuffer.Clear();
            _unprocessedIndex = 0;
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

                    if (_lastCrosshair.X >= 0)
                    {
                        var oldRect = new Rectangle(
                            _lastCrosshair.X - r - pad, _lastCrosshair.Y - r - pad,
                            r * 2 + pad * 2, r * 2 + pad * 2);
                        g.FillRectangle(Brushes.Fuchsia, oldRect);
                        g.DrawImage(_canvas, oldRect, oldRect, GraphicsUnit.Pixel);
                    }

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

        protected override void OnPaint(PaintEventArgs e) { }

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
