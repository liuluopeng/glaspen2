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
    /// Visible stroke rendering form using Cairo (via Rust FFI) for anti-aliased strokes.
    /// Uses UpdateLayeredWindow + AC_SRC_ALPHA for per-pixel alpha blending.
    /// </summary>
    public class FakeStrokeForm : Form
    {
        private IntPtr _renderer;
        private IntPtr _dibHdc;
        private IntPtr _dibBitmap;
        private IntPtr _dibBits;
        private int _dibStride;
        private Color _penColor = Color.FromArgb(0xDC, 0x1E, 0x1E);
        private float _currentWidth;
        private bool _isDrawing;
        private float _lastX, _lastY;

        // Crosshair (drawn to DIB via GDI+)
        private Point _lastCrosshair = new Point(-1, -1);
        private const int CROSSHAIR_RADIUS = 10;

        // On-screen notification
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

            // Create Cairo renderer via Rust FFI
            _renderer = GlaspenNative.glaspen2_cairo_renderer_create(bounds.Width, bounds.Height);

            // Create persistent DIB section for blitting
            CreateDib(bounds.Width, bounds.Height);

            // Initial blit happens in OnHandleCreated (handle not ready yet)

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
                           | NativeMethods.WS_EX_TOPMOST
                           | NativeMethods.WS_EX_LAYERED; // required for UpdateLayeredWindow
                return cp;
            }
        }

        protected override bool ShowWithoutActivation { get { return true; } }

        protected override void OnHandleCreated(EventArgs e)
        {
            base.OnHandleCreated(e);
            // Initial present: transparent Cairo surface → window
            CopyCairoToDib();
            PresentDib(true);
        }

        // ── DIB setup ──

        private void CreateDib(int w, int h)
        {
            IntPtr screenDc = NativeMethods.GetDC(IntPtr.Zero);
            _dibHdc = NativeMethods.CreateCompatibleDC(screenDc);
            NativeMethods.ReleaseDC(IntPtr.Zero, screenDc);

            _dibStride = (w * 4 + 3) & ~3;

            var bmi = new BITMAPINFO
            {
                bmiHeader = new BITMAPINFOHEADER
                {
                    biSize = (uint)Marshal.SizeOf<BITMAPINFOHEADER>(),
                    biWidth = w,
                    biHeight = -h,
                    biPlanes = 1,
                    biBitCount = 32,
                    biCompression = BI_RGB
                }
            };

            IntPtr bitsPtr;
            _dibBitmap = CreateDIBSection(_dibHdc, ref bmi, DIB_RGB_COLORS, out bitsPtr, IntPtr.Zero, 0);
            _dibBits = bitsPtr;
            SelectObject(_dibHdc, _dibBitmap);
        }

        private void DestroyDib()
        {
            if (_dibBitmap != IntPtr.Zero) { NativeMethods.DeleteObject(_dibBitmap); _dibBitmap = IntPtr.Zero; }
            if (_dibHdc != IntPtr.Zero) { NativeMethods.DeleteDC(_dibHdc); _dibHdc = IntPtr.Zero; }
            _dibBits = IntPtr.Zero;
        }

        // ── Blit pipeline: 1) CopyCairoToDib  2) [draw overlays]  3) PresentDib ──

        private int _lastPresentTick;
        private const int PRESENT_THROTTLE_MS = 10; // ~100 FPS max, responsive

        /// <summary>
        /// Copy entire Cairo surface pixels to DIB using a single CopyMemory call (when strides match).
        /// </summary>
        private void CopyCairoToDib()
        {
            if (_renderer == IntPtr.Zero) return;
            IntPtr srcPixels = GlaspenNative.glaspen2_cairo_surface_data(_renderer);
            if (srcPixels == IntPtr.Zero) return;
            int surfW, surfH, stride;
            GlaspenNative.glaspen2_cairo_surface_size(_renderer, out surfW, out surfH, out stride);
            if (surfW <= 0 || surfH <= 0 || _dibBits == IntPtr.Zero) return;

            int rowBytes = surfW * 4;
            // When strides match, use a single fast copy
            if (stride == _dibStride)
            {
                NativeMethods.CopyMemory(_dibBits, srcPixels, (uint)(rowBytes * surfH));
            }
            else
            {
                for (int y = 0; y < surfH; y++)
                {
                    IntPtr srcRow = IntPtr.Add(srcPixels, y * stride);
                    IntPtr dstRow = IntPtr.Add(_dibBits, y * _dibStride);
                    NativeMethods.CopyMemory(dstRow, srcRow, (uint)rowBytes);
                }
            }
        }

        /// <summary>
        /// Copy a sub-rect of Cairo surface to DIB. Used for partial updates during drawing.
        /// </summary>
        private void CopyCairoRectToDib(int bx, int by, int bw, int bh)
        {
            if (_renderer == IntPtr.Zero) return;
            IntPtr srcPixels = GlaspenNative.glaspen2_cairo_surface_data(_renderer);
            if (srcPixels == IntPtr.Zero) return;
            int surfW, surfH, stride;
            GlaspenNative.glaspen2_cairo_surface_size(_renderer, out surfW, out surfH, out stride);
            if (surfW <= 0 || surfH <= 0 || _dibBits == IntPtr.Zero) return;

            // Clamp
            if (bx < 0) { bw += bx; bx = 0; }
            if (by < 0) { bh += by; by = 0; }
            if (bx + bw > surfW) bw = surfW - bx;
            if (by + bh > surfH) bh = surfH - by;
            if (bw <= 0 || bh <= 0) return;

            int rowBytes = bw * 4;
            for (int y = 0; y < bh; y++)
            {
                IntPtr srcRow = IntPtr.Add(srcPixels, (by + y) * stride + bx * 4);
                IntPtr dstRow = IntPtr.Add(_dibBits, (by + y) * _dibStride + bx * 4);
                NativeMethods.CopyMemory(dstRow, srcRow, (uint)rowBytes);
            }
        }

        /// <summary>
        /// Present the DIB to the window via full-screen UpdateLayeredWindow.
        /// Throttled unless force=true.
        /// </summary>
        private void PresentDib(bool force)
        {
            if (!this.IsHandleCreated) return;

            int now = Environment.TickCount;
            if (!force && unchecked(now - _lastPresentTick) < PRESENT_THROTTLE_MS) return;
            _lastPresentTick = now;

            var ptDst = new NativeMethods.POINT(this.Left, this.Top);
            var sz = new NativeMethods.SIZE(this.Width, this.Height);
            var ptSrc = new NativeMethods.POINT(0, 0);
            var blend = new NativeMethods.BLENDFUNCTION(
                NativeMethods.AC_SRC_OVER, 0, 255, NativeMethods.AC_SRC_ALPHA);

            NativeMethods.UpdateLayeredWindow(
                this.Handle, IntPtr.Zero, ref ptDst, ref sz,
                _dibHdc, ref ptSrc, 0, ref blend, NativeMethods.ULW_ALPHA);
        }

        // Convenience: throttled full blit
        private void BlitCairoToWindow() { CopyCairoToDib(); PresentDib(false); }

        /// <summary>
        /// Compute bounding box for a line segment with width margin.
        /// </summary>
        private static void LineRect(float x0, float y0, float x1, float y1, float w,
            out int bx, out int by, out int bw, out int bh)
        {
            float pad = w / 2f + 2f;
            float minX = Math.Min(x0, x1) - pad, minY = Math.Min(y0, y1) - pad;
            float maxX = Math.Max(x0, x1) + pad, maxY = Math.Max(y0, y1) + pad;
            bx = (int)Math.Floor(minX); by = (int)Math.Floor(minY);
            bw = (int)Math.Ceiling(maxX) - bx + 1;
            bh = (int)Math.Ceiling(maxY) - by + 1;
        }

        private static void DotRect(float x, float y, float w,
            out int bx, out int by, out int bw, out int bh)
        {
            float pad = w / 2f + 2f;
            bx = (int)(x - pad); by = (int)(y - pad);
            bw = (int)(pad * 2) + 1; bh = (int)(pad * 2) + 1;
        }

        // ── Crosshair / Notification (drawn to DIB via GDI+) ──

        private void DrawCrosshairToDib()
        {
            if (_dibHdc == IntPtr.Zero) return;
            using (var g = Graphics.FromHdc(_dibHdc))
            {
                g.SmoothingMode = SmoothingMode.AntiAlias;
                using (var pen = new Pen(Color.FromArgb(200, 0, 255, 0), 2f))
                {
                    g.DrawLine(pen,
                        _lastCrosshair.X - CROSSHAIR_RADIUS, _lastCrosshair.Y,
                        _lastCrosshair.X + CROSSHAIR_RADIUS, _lastCrosshair.Y);
                    g.DrawLine(pen,
                        _lastCrosshair.X, _lastCrosshair.Y - CROSSHAIR_RADIUS,
                        _lastCrosshair.X, _lastCrosshair.Y + CROSSHAIR_RADIUS);
                    g.DrawEllipse(pen,
                        _lastCrosshair.X - CROSSHAIR_RADIUS, _lastCrosshair.Y - CROSSHAIR_RADIUS,
                        CROSSHAIR_RADIUS * 2, CROSSHAIR_RADIUS * 2);
                }
            }
        }

        private void DrawNotificationToDib()
        {
            if (string.IsNullOrEmpty(_notification) || _dibHdc == IntPtr.Zero) return;
            using (var g = Graphics.FromHdc(_dibHdc))
            {
                g.SmoothingMode = SmoothingMode.AntiAlias;
                g.TextRenderingHint = System.Drawing.Text.TextRenderingHint.AntiAlias;
                using (var font = new Font("Consolas", 36f, FontStyle.Bold))
                {
                    var textSize = g.MeasureString(_notification, font);
                    float x = (this.Width - textSize.Width) / 2f;
                    float y = (this.Height - textSize.Height) / 2f;
                    _notificationRect = new Rectangle(
                        (int)x - 4, (int)y - 4,
                        (int)textSize.Width + 8, (int)textSize.Height + 8);
                    using (var brush = new SolidBrush(Color.FromArgb(200, 0, 0, 0)))
                        g.DrawString(_notification, font, brush, x + 2, y + 2);
                    using (var brush = new SolidBrush(Color.FromArgb(240, 255, 255, 255)))
                        g.DrawString(_notification, font, brush, x, y);
                }
            }
        }

        // ── Public API ──

        public void SetColor(Color color) { _penColor = color; }

        public Bitmap GetCanvas()
        {
            if (_renderer == IntPtr.Zero) return null;
            int w, h, stride;
            GlaspenNative.glaspen2_cairo_surface_size(_renderer, out w, out h, out stride);
            if (w <= 0 || h <= 0) return null;

            IntPtr pixels = GlaspenNative.glaspen2_cairo_surface_data(_renderer);
            if (pixels == IntPtr.Zero) return null;

            var bmp = new Bitmap(w, h, PixelFormat.Format32bppArgb);
            var bmpData = bmp.LockBits(new Rectangle(0, 0, w, h), ImageLockMode.WriteOnly, PixelFormat.Format32bppArgb);
            for (int y = 0; y < h; y++)
            {
                IntPtr srcRow = IntPtr.Add(pixels, y * stride);
                IntPtr dstRow = IntPtr.Add(bmpData.Scan0, y * bmpData.Stride);
                NativeMethods.CopyMemory(dstRow, srcRow, (uint)(w * 4));
            }
            bmp.UnlockBits(bmpData);
            return bmp;
        }

        public void ShowNotification(string text)
        {
            _notification = text;
            _notificationTimer.Stop();
            _notificationTimer.Start();
            // Copy Cairo → DIB first, then draw notification on top, then present
            CopyCairoToDib();
            DrawNotificationToDib();
            PresentDib(true); // NOT BlitCairoToWindow — would overwrite notification
        }

        private void ClearNotification()
        {
            _notification = null;
            _notificationRect = Rectangle.Empty;
            // Re-blit: copy Cairo → DIB (without notification) and present
            CopyCairoToDib();
            PresentDib(true);
        }

        // ── Stroke drawing via Cairo FFI ──

        public void BeginStroke(float x, float y, float width)
        {
            _currentWidth = width;
            _isDrawing = true;
            _lastX = x; _lastY = y;

            double r = _penColor.R / 255.0;
            double g = _penColor.G / 255.0;
            double b = _penColor.B / 255.0;

            GlaspenNative.glaspen2_cairo_draw_dot(_renderer, x, y, width, r, g, b);

            // Write to Rust DB for page navigation/export
            GlaspenNative.glaspen2_begin_stroke(r, g, b, width);
            GlaspenNative.glaspen2_add_point(x, y, width);

            int bx, by, bw, bh;
            DotRect(x, y, width, out bx, out by, out bw, out bh);
            CopyCairoRectToDib(bx, by, bw, bh);
            PresentDib(true); // force immediate on pen-down
        }

        public void AddPoint(float x, float y, float width)
        {
            if (!_isDrawing) return;
            _currentWidth = width;

            double r = _penColor.R / 255.0;
            double g = _penColor.G / 255.0;
            double b = _penColor.B / 255.0;

            GlaspenNative.glaspen2_cairo_draw_line(_renderer, _lastX, _lastY, x, y, width, r, g, b);
            float lx = _lastX, ly = _lastY;
            _lastX = x; _lastY = y;

            GlaspenNative.glaspen2_add_point(x, y, width);

            // Copy only the dirty rect from Cairo surface to DIB
            int bx, by, bw, bh;
            LineRect(lx, ly, x, y, width, out bx, out by, out bw, out bh);
            CopyCairoRectToDib(bx, by, bw, bh);
            PresentDib(false); // throttled
        }

        public void EndStroke()
        {
            _isDrawing = false;
            GlaspenNative.glaspen2_end_stroke();
            CopyCairoToDib();  // full copy for final state
            PresentDib(true);  // force immediate on pen-up
        }

        public void ClearCrosshair()
        {
            if (_lastCrosshair.X >= 0)
            {
                _lastCrosshair = new Point(-1, -1);
                CopyCairoToDib();
                PresentDib(true);
            }
        }

        public void ClearAll()
        {
            _isDrawing = false;
            _lastCrosshair = new Point(-1, -1);

            if (_renderer != IntPtr.Zero)
            {
                GlaspenNative.glaspen2_clear_strokes(this.Width, this.Height);
                GlaspenNative.glaspen2_cairo_clear(_renderer);
            }

            CopyCairoToDib();
            PresentDib(true);
        }

        public void LoadAndReplayFromNative(long screenId)
        {
            int count = GlaspenNative.glaspen2_load_strokes_for_screen(screenId);
            if (count > 0)
                GlaspenNative.glaspen2_smooth_loaded_strokes();

            if (_renderer != IntPtr.Zero)
                GlaspenNative.glaspen2_cairo_replay_strokes(_renderer);

            CopyCairoToDib();
            PresentDib(true);

            ShowNotification(count > 0
                ? string.Format("第 {0} 页 ({1} 笔)", screenId, count)
                : string.Format("第 {0} 页为空", screenId));
        }

        public void DrawCrosshair(float x, float y)
        {
            _lastCrosshair = new Point((int)x, (int)y);
            // Copy Cairo → DIB first, then draw crosshair on top, then present
            CopyCairoToDib();
            DrawCrosshairToDib();
            PresentDib(true); // NOT BlitCairoToWindow — would overwrite crosshair
        }

        protected override void OnPaint(PaintEventArgs e) { }

        protected override void Dispose(bool disposing)
        {
            if (disposing)
            {
                if (_notificationTimer != null) _notificationTimer.Dispose();
                if (_renderer != IntPtr.Zero)
                {
                    GlaspenNative.glaspen2_cairo_renderer_destroy(_renderer);
                    _renderer = IntPtr.Zero;
                }
                DestroyDib();
            }
            base.Dispose(disposing);
        }

        // ── DIB Win32 P/Invoke ──

        private const uint DIB_RGB_COLORS = 0;
        private const uint BI_RGB = 0;

        [StructLayout(LayoutKind.Sequential)]
        private struct BITMAPINFOHEADER
        {
            public uint biSize;
            public int biWidth;
            public int biHeight;
            public ushort biPlanes;
            public ushort biBitCount;
            public uint biCompression;
            public uint biSizeImage;
            public int biXPelsPerMeter;
            public int biYPelsPerMeter;
            public uint biClrUsed;
            public uint biClrImportant;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct BITMAPINFO
        {
            public BITMAPINFOHEADER bmiHeader;
        }

        [DllImport("gdi32.dll", SetLastError = true)]
        private static extern IntPtr CreateDIBSection(
            IntPtr hdc, [In] ref BITMAPINFO pbmi, uint iUsage,
            out IntPtr ppvBits, IntPtr hSection, uint dwOffset);

        [DllImport("gdi32.dll")]
        private static extern IntPtr SelectObject(IntPtr hdc, IntPtr hgdiobj);
    }
}
