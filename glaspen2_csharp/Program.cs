using System;
using System.IO;
using System.IO.Pipes;
using System.Windows.Forms;

namespace GlasPen2
{
    internal static class Program
    {
        private static NamedPipeServerStream _pipe;
        private static StreamWriter _writer;

        public static void Log(string msg)
        {
            try
            {
                if (_writer == null && _pipe != null && _pipe.IsConnected)
                    _writer = new StreamWriter(_pipe) { AutoFlush = true };
                if (_writer != null)
                    _writer.WriteLine("[{0:HH:mm:ss.fff}] {1}", DateTime.Now, msg);
            }
            catch { }
        }

        public static void Log(string fmt, params object[] args)
        {
            try { Log(string.Format(fmt, args)); }
            catch { }
        }

        [STAThread]
        public static void Main()
        {
            NativeMethods.SetProcessDPIAware();

            // Log pipe
            try
            {
                _pipe = new NamedPipeServerStream("glaspen2_log",
                    PipeDirection.Out, 1,
                    PipeTransmissionMode.Byte,
                    PipeOptions.Asynchronous);
                _pipe.WaitForConnectionAsync().ContinueWith(t => { });
            }
            catch { }

            Log("[Main] Starting GlasPen2...");

            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);

            // Tray icon
            var trayIcon = new NotifyIcon
            {
                Text = "GlasPen2",
                Visible = true,
                Icon = CreateTrayIcon(),
            };
            var menu = new ContextMenuStrip();
            var clearItem = new ToolStripMenuItem("Clear");
            clearItem.Click += (s, e) => { if (Overlay != null) Overlay.ClearAll(); };
            menu.Items.Add(clearItem);
            var exitItem = new ToolStripMenuItem("Exit");
            exitItem.Click += (s, e) => Application.Exit();
            menu.Items.Add(exitItem);
            trayIcon.ContextMenuStrip = menu;

            // Overlay
            Overlay = new OverlayForm();
            Overlay.Show();

            Log("[Main] Overlay shown. DPI-aware, waiting for pen input...");
            Application.ApplicationExit += (s, e) =>
            {
                trayIcon.Visible = false;
                trayIcon.Dispose();
            };
            Application.Run();
        }

        public static OverlayForm Overlay;

        private static System.Drawing.Icon CreateTrayIcon()
        {
            var bmp = new System.Drawing.Bitmap(16, 16);
            using (var g = System.Drawing.Graphics.FromImage(bmp))
            {
                g.Clear(System.Drawing.Color.Transparent);
                using (var brush = new System.Drawing.SolidBrush(System.Drawing.Color.FromArgb(220, 50, 50)))
                    g.FillEllipse(brush, 3, 3, 10, 10);
            }
            return System.Drawing.Icon.FromHandle(bmp.GetHicon());
        }
    }
}
