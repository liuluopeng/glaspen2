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

            // Overlay
            var overlay = new OverlayForm();
            overlay.Show();

            Log("[Main] Overlay shown. DPI-aware, waiting for pen input...");
            Application.Run();
        }

    }
}
