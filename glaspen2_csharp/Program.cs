using System;
using System.IO;
using System.IO.Pipes;
using System.Windows.Forms;

namespace GlasPen2
{
    internal static class Program
    {
        private static OverlayForm _overlay;
        private static InputWindow _inputWin;
        private static PenInterceptor _interceptor;
        private static bool _drawingEnabled = true;

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

        [STAThread]
        public static void Main()
        {
            NativeMethods.SetProcessDPIAware();

            // Log pipe — Rust connects and prints to its stderr
            try
            {
                _pipe = new NamedPipeServerStream("glaspen2_log",
                    PipeDirection.Out, 1,
                    PipeTransmissionMode.Byte,
                    PipeOptions.Asynchronous);
                _pipe.WaitForConnectionAsync().ContinueWith(t => { });
            }
            catch { }

            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);

            Log("[Main] Starting GlasPen2...");

            // ── Input window (behind overlay) ──
            _inputWin = new InputWindow();
            _inputWin.Show();

            // ── Create the transparent overlay ──
            _overlay = new OverlayForm();
            _overlay.DrawingEnabled = _drawingEnabled;
            _overlay.Show();

            // ── Install the mouse hook (suppresses pen mouse events) ──
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

            Log("[Main] Overlay shown. DPI-aware, pen hook active.");

            // ── Watch for display changes ──
            Microsoft.Win32.SystemEvents.DisplaySettingsChanged += (s, e) =>
            {
                if (_overlay != null) _overlay.RefreshScreenBounds();
            };

            // ── Run the message loop ──
            Application.ApplicationExit += OnApplicationExit;
            Application.Run();
        }

        private static void OnApplicationExit(object sender, EventArgs e)
        {
            Log("[Exit] Cleaning up...");
            if (_interceptor != null) { _interceptor.Uninstall(); _interceptor = null; }
            if (_overlay != null) { _overlay.Close(); _overlay.Dispose(); _overlay = null; }
            if (_inputWin != null) { _inputWin.Close(); _inputWin.Dispose(); _inputWin = null; }
        }
    }
}
