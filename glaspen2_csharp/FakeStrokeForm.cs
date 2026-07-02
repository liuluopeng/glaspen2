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
                // WS_EX_TRANSPARENT — let input pass through to overlay below
                // WS_EX_LAYERED — per-pixel alpha
                cp.ExStyle |= NativeMethods.WS_EX_TRANSPARENT
                           | NativeMethods.WS_EX_NOACTIVATE
                           | NativeMethods.WS_EX_TOOLWINDOW;
                // NOT TopMost — sits BELOW the overlay
                return cp;
            }
        }

        protected override bool ShowWithoutActivation { get { return true; } }

        public void BeginStroke(int x, int y, float width)
        {
            _currentWidth = width;
            _isDrawing = true;
            _recentPoints.Clear();
            _recentPoints.Add(new Point(x, y));
            using (var pen = new Pen(_penColor, _currentWidth))
            {
                pen.StartCap = LineCap.Round;
                pen.EndCap = LineCap.Round;
                _g.DrawEllipse(pen, x - _currentWidth / 2, y - _currentWidth / 2, _currentWidth, _currentWidth);
            }
            this.Refresh();
        }

        public void AddPoint(int x, int y)
        {
            if (!_isDrawing) return;
            var pt = new Point(x, y);
            _recentPoints.Add(pt);
            if (_recentPoints.Count > MAX_RECENT)
                _recentPoints.RemoveAt(0);

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
            this.Refresh();
        }

        public void EndStroke()
        {
            _recentPoints.Clear();
            _isDrawing = false;
            this.Refresh();
        }

        public void ClearAll()
        {
            _isDrawing = false;
            _g.Clear(Color.Transparent);
            this.Refresh();
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
