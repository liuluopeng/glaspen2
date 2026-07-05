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
        private Color _penColor = Color.FromArgb(0xDC, 0x1E, 0x1E); // default: red (matches Flutter UI index 0)
        private float _currentWidth;
        private bool _isDrawing;
        private Point _lastDirectPoint;
        private Point _lastCrosshair = new Point(-1, -1);
        private const int CROSSHAIR_RADIUS = 10;

        // Catmull-Rom spline smoothing buffer
        private readonly List<PointF> _pointBuffer = new List<PointF>();
        private int _unprocessedIndex;

        // On-screen notification (hotkey feedback)
        private string _notification;
        private System.Windows.Forms.Timer _notificationTimer;
        private Rectangle _notificationRect;

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

            // Pre-warm GDI resources to avoid first-stroke latency
            if (this.IsHandleCreated)
            {
                IntPtr hdc = NativeMethods.GetDC(this.Handle);
                if (hdc != IntPtr.Zero)
                {
                    using (var g = Graphics.FromHdc(hdc))
                    {
                        g.Clear(Color.Fuchsia);
                    }
                    NativeMethods.ReleaseDC(this.Handle, hdc);
                }
            }

            // Notification timer: clears notification after 1 second
            _notificationTimer = new System.Windows.Forms.Timer { Interval = 1000 };
            _notificationTimer.Tick += (s, e) =>
            {
                _notificationTimer.Stop();
                ClearNotification();
            };
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

        /// <summary>
        /// Returns the backing canvas bitmap for export.
        /// </summary>
        public Bitmap GetCanvas() { return _canvas; }

        /// <summary>
        /// Show a centered notification on screen for ~1 second.
        /// </summary>
        public void ShowNotification(string text)
        {
            _notification = text;
            _notificationTimer.Stop();

            // Draw notification to window DC
            DrawNotification();

            // Auto-clear after 1 second
            _notificationTimer.Start();
        }

        private void DrawNotification()
        {
            if (string.IsNullOrEmpty(_notification) || !this.IsHandleCreated) return;

            IntPtr hdc = GetWindowDC();
            if (hdc == IntPtr.Zero) return;
            try
            {
                using (var g = Graphics.FromHdc(hdc))
                {
                    g.SmoothingMode = SmoothingMode.AntiAlias;
                    g.TextRenderingHint = System.Drawing.Text.TextRenderingHint.AntiAlias;

                    using (var font = new Font("Consolas", 36f, FontStyle.Bold))
                    {
                        var textSize = g.MeasureString(_notification, font);
                        float x = (this.Width - textSize.Width) / 2f;
                        float y = (this.Height - textSize.Height) / 2f;

                        // Save rect for clearing
                        _notificationRect = new Rectangle(
                            (int)x - 4, (int)y - 4,
                            (int)textSize.Width + 8, (int)textSize.Height + 8);

                        // Shadow
                        using (var brush = new SolidBrush(Color.FromArgb(200, 0, 0, 0)))
                        {
                            g.DrawString(_notification, font, brush, x + 2, y + 2);
                        }
                        // Text
                        using (var brush = new SolidBrush(Color.FromArgb(240, 255, 255, 255)))
                        {
                            g.DrawString(_notification, font, brush, x, y);
                        }
                    }
                }
            }
            finally
            {
                ReleaseWindowDC(hdc);
            }
        }

        private void ClearNotification()
        {
            if (_notificationRect.IsEmpty || !this.IsHandleCreated) return;
            _notification = null;

            // Clear notification area: Fuchsia + restore canvas
            IntPtr hdc = GetWindowDC();
            if (hdc != IntPtr.Zero)
            {
                using (var g = Graphics.FromHdc(hdc))
                {
                    g.FillRectangle(Brushes.Fuchsia, _notificationRect);
                    g.DrawImage(_canvas, _notificationRect, _notificationRect, GraphicsUnit.Pixel);
                }
                ReleaseWindowDC(hdc);
            }
            _notificationRect = Rectangle.Empty;
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

        public void BeginStroke(float x, float y, float width)
        {
            _currentWidth = width;
            _isDrawing = true;
            _pointBuffer.Clear();
            _unprocessedIndex = 0;
            var pt = new PointF(x, y);
            _pointBuffer.Add(pt);
            _lastDirectPoint = new Point(x, y);
            DrawDot(pt, _currentWidth);

            // Save to Rust DB
            GlaspenNative.glaspen2_begin_stroke(
                _penColor.R / 255.0, _penColor.G / 255.0, _penColor.B / 255.0,
                _currentWidth);
            GlaspenNative.glaspen2_add_point(x, y, _currentWidth);
        }

        public void AddPoint(float x, float y, float width)
        {
            if (!_isDrawing) return;
            _currentWidth = width;
            var pt = new PointF(x, y);
            _pointBuffer.Add(pt);

            // Save to Rust DB
            GlaspenNative.glaspen2_add_point(x, y, width);

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

            // Save stroke to Rust DB
            GlaspenNative.glaspen2_end_stroke();
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

            // Tell Rust to save current screen and start a new one
            GlaspenNative.glaspen2_clear_strokes(_canvas.Width, _canvas.Height);

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
        /// Load strokes from Rust DB for a given screen and replay them.
        /// Called by page navigation hotkeys (Ctrl+Alt+J / Ctrl+Alt+K).
        /// </summary>
        public void LoadAndReplayFromNative(long screenId)
        {
            GlaspenNative.glaspen2_load_strokes_for_screen(screenId);
            GlaspenNative.glaspen2_smooth_loaded_strokes();

            int count = GlaspenNative.glaspen2_stroke_count();

            // Clear canvas bitmap
            _g.Clear(Color.Transparent);

            // Clear window DC (actual screen)
            IntPtr hdc = GetWindowDC();
            Graphics winG = (hdc != IntPtr.Zero) ? Graphics.FromHdc(hdc) : null;
            if (winG != null) winG.Clear(Color.Fuchsia);
            try
            {
                if (winG != null) winG.SmoothingMode = SmoothingMode.None;

                for (int i = 0; i < count; i++)
                {
                    int ptCount = GlaspenNative.glaspen2_get_stroke_point_count(i);
                    if (ptCount < 1) continue;

                    double r, g, b;
                    GlaspenNative.glaspen2_get_stroke_color(i, out r, out g, out b);
                    double avgW = GlaspenNative.glaspen2_get_stroke_avg_width(i);
                    var color = Color.FromArgb((int)(r * 255), (int)(g * 255), (int)(b * 255));
                    float width = (float)avgW;

                    if (ptCount == 1)
                    {
                        double px, py;
                        GlaspenNative.glaspen2_get_stroke_point(i, 0, out px, out py);
                        float rad = width / 2f;
                        using (var pen = new Pen(color, width))
                        {
                            pen.StartCap = LineCap.Round;
                            pen.EndCap = LineCap.Round;
                            _g.DrawEllipse(pen, (float)px - rad, (float)py - rad, width, width);
                        }
                        if (winG != null)
                        {
                            using (var pen = new Pen(color, width))
                            {
                                pen.StartCap = LineCap.Round;
                                pen.EndCap = LineCap.Round;
                                winG.DrawEllipse(pen, (float)px - rad, (float)py - rad, width, width);
                            }
                        }
                    }
                    else
                    {
                        // Collect points
                        var pts = new PointF[ptCount];
                        for (int j = 0; j < ptCount; j++)
                        {
                            double px, py;
                            GlaspenNative.glaspen2_get_stroke_point(i, j, out px, out py);
                            pts[j] = new PointF((float)px, (float)py);
                        }

                        // Draw as connected line segments (simplified — no Catmull-Rom for replay)
                        using (var pen = new Pen(color, width))
                        {
                            pen.StartCap = LineCap.Round;
                            pen.EndCap = LineCap.Round;
                            pen.LineJoin = LineJoin.Round;
                            _g.DrawLines(pen, pts);
                        }
                        if (winG != null)
                        {
                            using (var pen = new Pen(color, width))
                            {
                                pen.StartCap = LineCap.Round;
                                pen.EndCap = LineCap.Round;
                                pen.LineJoin = LineJoin.Round;
                                winG.DrawLines(pen, pts);
                            }
                        }
                    }
                }
            }
            finally
            {
                if (winG != null) winG.Dispose();
                if (hdc != IntPtr.Zero) ReleaseWindowDC(hdc);
            }

            ShowNotification(count > 0
                ? string.Format("第 {0} 页 ({1} 笔)", screenId, count)
                : string.Format("第 {0} 页为空", screenId));
        }

        public void DrawCrosshair(float x, float y)
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
