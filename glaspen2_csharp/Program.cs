using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.IO.Pipes;
using System.Windows.Forms;

namespace GlasPen2
{
    internal static class Program
    {
        private static NamedPipeServerStream _pipe;
        private static StreamWriter _writer;
        private static SettingsPipeServer _settingsServer;

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

        private static void TryLaunchSettings()
        {
            // Look for glaspen2_settings.exe in several locations
            var exeDir = Path.GetDirectoryName(typeof(Program).Assembly.Location) ?? ".";
            string[] candidates = {
                Path.Combine(exeDir, "glaspen2_settings.exe"),
                // Dev build: flutter_settings/build/windows/x64/runner/Release/
                Path.Combine(exeDir, "..", "..", "..", "flutter_settings", "build", "windows", "x64", "runner", "Release", "glaspen2_settings.exe"),
                Path.Combine(exeDir, "..", "..", "..", "flutter_settings", "build", "windows", "x64", "runner", "Debug", "glaspen2_settings.exe"),
            };

            foreach (var path in candidates)
            {
                try
                {
                    var full = Path.GetFullPath(path);
                    if (File.Exists(full))
                    {
                        Process.Start(full);
                        Log("[Main] Launched settings UI: {0}", full);
                        return;
                    }
                }
                catch { }
            }
            Log("[Main] Settings UI not found (searched {0} locations)", candidates.Length);
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

            // Initialize Rust database (creates first screen record)
            var screen = SystemInformation.VirtualScreen;
            GlaspenNative.glaspen2_init_db(screen.Width, screen.Height);

            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);

            // Overlay
            var overlay = new OverlayForm();
            overlay.Show();

            // Settings pipe server (Flutter UI communication)
            _settingsServer = new SettingsPipeServer();
            _settingsServer.GetSettings = () => overlay.GetSettings();
            _settingsServer.OnSettingChanged = (key, value) =>
            {
                overlay.UpdateSetting(key, value);
                _settingsServer.NotifySettingsChanged(overlay.GetSettings());
            };
            _settingsServer.Start();

            // Launch Flutter settings UI
            TryLaunchSettings();

            Log("[Main] Overlay shown. DPI-aware, waiting for pen input...");
            Application.Run();
        }

    }
}
