using System;
using System.Drawing;
using System.Windows.Forms;

namespace GlasPen2
{
    internal static class Program
    {
        private static OverlayForm _overlay;
        private static InputWindow _inputWin;
        private static PenInterceptor _interceptor;
        private static NotifyIcon _trayIcon;
        private static bool _drawingEnabled = true;

        public static void OnPointerDown(int x, int y, uint pressure)
        {
            if (_drawingEnabled && _overlay != null)
            {
                _overlay.SetPressure(pressure);
                _overlay.StartDrawing();
            }
        }
        public static void OnPointerUp()
        {
            if (_overlay != null) _overlay.StopDrawing();
        }
        public static void OnPointerPressure(uint p)
        {
            if (_overlay != null) _overlay.SetPressure(p);
        }

        private static readonly Color[] PresetColors =
        {
            Color.FromArgb(220, 30, 30),   // Red
            Color.FromArgb(30, 120, 220),   // Blue
            Color.FromArgb(30, 180, 60),    // Green
            Color.FromArgb(240, 160, 20),   // Orange
            Color.FromArgb(160, 80, 220),   // Purple
            Color.FromArgb(20, 20, 20),     // Black
            Color.FromArgb(255, 255, 255),  // White
        };

        [STAThread]
        public static void Main()
        {
            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);

            // ── Initialize Rust core (DB + modeler) via FFI ──
            try
            {
                var bounds = SystemInformation.VirtualScreen;
                GlaspenNative.glaspen2_init_db(bounds.Width, bounds.Height);
                Console.WriteLine("[Main] Rust DB initialized via FFI");

                // Load saved settings
                double sr, sg, sb, sw;
                if (GlaspenNative.glaspen2_load_settings_parts(out sr, out sg, out sb, out sw) != 0)
                {
                    Console.WriteLine("[Main] Loaded settings: r={0:F2} g={1:F2} b={2:F2} w={3:F2}", sr, sg, sb, sw);
                }
            }
            catch (Exception ex)
            {
                Console.WriteLine("[Main] WARNING: Failed to load glaspen2.dll: {0}", ex.Message);
                Console.WriteLine("[Main] Rust FFI features (DB, modeler, export) will be unavailable.");
            }

            // ── System tray icon ──
            _trayIcon = new NotifyIcon
            {
                Text = "GlasPen2 — 屏幕涂鸦",
                Visible = true,
                ContextMenuStrip = BuildTrayMenu()
            };
            _trayIcon.Icon = CreateTrayIcon();

            // ── Input window (behind overlay) ──
            _inputWin = new InputWindow();
            _inputWin.Show();

            // ── Create the transparent overlay ──
            _overlay = new OverlayForm();
            _overlay.DrawingEnabled = _drawingEnabled;
            _overlay.ProbeRustModeler();
            _overlay.Show();

            // ── Install the mouse hook (suppresses pen mouse events + detects touch) ──
            _interceptor = new PenInterceptor();
            _interceptor.PenDown += (x, y) =>
            {
                if (_drawingEnabled && _overlay != null)
                    _overlay.OnPenDown(x, y);
            };
            _interceptor.PenMove += (x, y) =>
            {
                // Hook move — raw input handles drawing
            };
            _interceptor.PenUp += (x, y) =>
            {
                if (_overlay != null)
                    _overlay.OnPenUp(x, y);
            };
            _interceptor.Install();

            // ── Watch for display changes ──
            Microsoft.Win32.SystemEvents.DisplaySettingsChanged += (s, e) =>
            {
                if (_overlay != null) _overlay.RefreshScreenBounds();
            };

            // ── Run the message loop ──
            Application.ApplicationExit += OnApplicationExit;
            Application.Run();
        }

        private static ContextMenuStrip BuildTrayMenu()
        {
            var menu = new ContextMenuStrip();

            // Toggle drawing
            var toggleItem = new ToolStripMenuItem("✏️ 启用涂鸦 (绘图)")
            {
                Checked = _drawingEnabled,
                CheckOnClick = true
            };
            toggleItem.Click += (s, e) =>
            {
                _drawingEnabled = toggleItem.Checked;
                if (_overlay != null) _overlay.DrawingEnabled = _drawingEnabled;
                if (_interceptor != null) _interceptor.Enabled = _drawingEnabled;
                string state = _drawingEnabled ? "已启用" : "已暂停";
                _trayIcon.ShowBalloonTip(800, "GlasPen2",
                    string.Format("涂鸦功能{0}", state), ToolTipIcon.Info);
            };
            menu.Items.Add(toggleItem);

            menu.Items.Add(new ToolStripSeparator());

            // Color sub-menu
            var colorMenu = new ToolStripMenuItem("🎨 笔颜色");
            string[] colorNames = { "红色", "蓝色", "绿色", "橙色", "紫色", "黑色", "白色" };
            for (int i = 0; i < PresetColors.Length; i++)
            {
                var colorItem = new ToolStripMenuItem(colorNames[i]);
                var color = PresetColors[i];
                colorItem.Click += (s, e) =>
                {
                    _overlay.PenColor = color;
                    // Save to Rust DB
                    try { GlaspenNative.glaspen2_save_settings(color.R / 255.0, color.G / 255.0, color.B / 255.0, _overlay.WidthScale); } catch { }
                };
                colorMenu.DropDownItems.Add(colorItem);
            }
            menu.Items.Add(colorMenu);

            // Pen width sub-menu
            var widthMenu = new ToolStripMenuItem("📏 笔粗细");
            float[] widths = { 0.3f, 0.5f, 0.8f, 1.2f, 2f, 3.5f, 5f, 8f };
            string[] widthNames = { "极细 (0.3)", "很细 (0.5)", "细 (0.8)", "中 (1.2)", "粗 (2.0)", "很粗 (3.5)", "超粗 (5.0)", "极粗 (8.0)" };
            for (int i = 0; i < widths.Length; i++)
            {
                var widthItem = new ToolStripMenuItem(widthNames[i]);
                var w = widths[i];
                widthItem.Click += (s, e) =>
                {
                    _overlay.PenWidth = w;
                    _overlay.WidthScale = w;
                    // Save to Rust DB
                    try { GlaspenNative.glaspen2_save_settings(_overlay.PenColor.R / 255.0, _overlay.PenColor.G / 255.0, _overlay.PenColor.B / 255.0, w); } catch { }
                };
                widthMenu.DropDownItems.Add(widthItem);
            }
            menu.Items.Add(widthMenu);

            menu.Items.Add(new ToolStripSeparator());

            // Clear (Ctrl+Alt+C)
            var clearItem = new ToolStripMenuItem("🗑️ 清除涂鸦  Ctrl+Alt+C");
            clearItem.Click += (s, e) => _overlay.ClearAll();
            menu.Items.Add(clearItem);

            // Undo (Ctrl+Alt+Z)
            var undoItem = new ToolStripMenuItem("↩️ 撤销上一笔  Ctrl+Alt+Z");
            undoItem.Click += (s, e) => _overlay.UndoLast();
            menu.Items.Add(undoItem);

            menu.Items.Add(new ToolStripSeparator());

            // Stroke smoothing toggle (like macOS ink-stroke-modeler)
            var smoothItem = new ToolStripMenuItem("✨ 笔迹美化 (去抖)")
            {
                Checked = true,
                CheckOnClick = true
            };
            smoothItem.Click += (s, e) =>
            {
                if (_overlay != null) _overlay.SmoothEnabled = smoothItem.Checked;
            };
            menu.Items.Add(smoothItem);

            // 180° rotation toggle
            var invertItem = new ToolStripMenuItem("🔄 坐标翻转 (180°)")
            {
                Checked = false,
                CheckOnClick = true
            };
            invertItem.Click += (s, e) =>
            {
                bool inv = invertItem.Checked;
                if (_overlay != null) { _overlay.InvertX = inv; _overlay.InvertY = inv; }
            };
            menu.Items.Add(invertItem);

            menu.Items.Add(new ToolStripSeparator());

            // Export sub-menu (uses Rust FFI)
            var exportMenu = new ToolStripMenuItem("💾 导出");
            var svgItem = new ToolStripMenuItem("📄 导出 SVG");
            svgItem.Click += (s, e) =>
            {
                try { GlaspenNative.glaspen2_save_svg(); _trayIcon.ShowBalloonTip(1000, "GlasPen2", "SVG 已保存到桌面", ToolTipIcon.Info); }
                catch (Exception ex) { Console.WriteLine("[Export] SVG failed: " + ex.Message); }
            };
            exportMenu.DropDownItems.Add(svgItem);

            var xojItem = new ToolStripMenuItem("📝 导出 Xournal");
            xojItem.Click += (s, e) =>
            {
                try { GlaspenNative.glaspen2_save_xoj(); _trayIcon.ShowBalloonTip(1000, "GlasPen2", "Xournal 已保存到桌面", ToolTipIcon.Info); }
                catch (Exception ex) { Console.WriteLine("[Export] Xournal failed: " + ex.Message); }
            };
            exportMenu.DropDownItems.Add(xojItem);
            menu.Items.Add(exportMenu);

            menu.Items.Add(new ToolStripSeparator());

            // Exit
            var exitItem = new ToolStripMenuItem("❌ 退出");
            exitItem.Click += (s, e) =>
            {
                _trayIcon.Visible = false;
                Application.Exit();
            };
            menu.Items.Add(exitItem);

            return menu;
        }

        private static Icon CreateTrayIcon()
        {
            var bmp = new Bitmap(16, 16);
            using (var g = Graphics.FromImage(bmp))
            {
                g.Clear(Color.Transparent);
                using (var brush = new SolidBrush(Color.FromArgb(220, 50, 50)))
                {
                    g.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.AntiAlias;
                    g.FillEllipse(brush, 3, 3, 10, 10);
                }
                using (var brush = new SolidBrush(Color.FromArgb(255, 120, 100)))
                {
                    g.FillEllipse(brush, 6, 5, 3, 3);
                }
            }
            return Icon.FromHandle(bmp.GetHicon());
        }

        private static void OnApplicationExit(object sender, EventArgs e)
        {
            Console.WriteLine("[Exit] Cleaning up...");
            if (_interceptor != null) { _interceptor.Uninstall(); _interceptor = null; }
            if (_trayIcon != null)
            {
                _trayIcon.Visible = false;
                _trayIcon.Icon = null;
                _trayIcon.Dispose();
                _trayIcon = null;
            }
            if (_overlay != null) { _overlay.Close(); _overlay.Dispose(); _overlay = null; }
            if (_inputWin != null) { _inputWin.Close(); _inputWin.Dispose(); _inputWin = null; }
        }
    }
}
