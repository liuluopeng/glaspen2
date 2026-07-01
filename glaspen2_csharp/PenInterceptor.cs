using System;
using System.Runtime.InteropServices;

namespace GlasPen2
{
    /// <summary>
    /// Global mouse hook that:
    /// 1. Detects WM_LBUTTONDOWN/UP from the pen → signals draw start/stop
    /// 2. Suppresses WM_MOUSEMOVE from the pen → cursor doesn't move
    /// All based on timing correlation with OverlayForm.LastPenEventUtc.
    /// </summary>
    public class PenInterceptor : IDisposable
    {
        private IntPtr _hookId = IntPtr.Zero;
        private NativeMethods.LowLevelMouseProc _proc;

        /// <summary>Mouse events within this window after a raw pen event are considered pen input.</summary>
        private const double SuppressWindowMs = 80.0;

        public bool Enabled { get; set; }

        /// <summary>Fired when pen touches tablet (WM_LBUTTONDOWN from pen).</summary>
        public event Action<int, int> PenDown;

        /// <summary>Fired when pen moves while touching (WM_MOUSEMOVE from pen).</summary>
        public event Action<int, int> PenMove;

        /// <summary>Fired when pen lifts (WM_LBUTTONUP from pen).</summary>
        public event Action<int, int> PenUp;

        public PenInterceptor()
        {
            _proc = HookCallback;
            Enabled = true;
        }

        public void Install()
        {
            using (var curProcess = System.Diagnostics.Process.GetCurrentProcess())
            using (var curModule = curProcess.MainModule)
            {
                string moduleName = (curModule != null) ? curModule.ModuleName : "";
                _hookId = NativeMethods.SetWindowsHookEx(
                    NativeMethods.WH_MOUSE_LL,
                    _proc,
                    NativeMethods.GetModuleHandle(moduleName),
                    0);
            }
            if (_hookId == IntPtr.Zero)
            {
                int err = Marshal.GetLastWin32Error();
                Console.WriteLine("[Hook] SetWindowsHookEx FAILED! err={0}", err);
                throw new System.ComponentModel.Win32Exception(err);
            }
            Console.WriteLine("[Hook] Installed OK. Window={0}ms", SuppressWindowMs);
        }

        public void Uninstall()
        {
            if (_hookId != IntPtr.Zero)
            {
                NativeMethods.UnhookWindowsHookEx(_hookId);
                _hookId = IntPtr.Zero;
            }
        }

        private int _suppressCount, _passCount, _downCount, _upCount;
        private int _lastPenX = -1, _lastPenY = -1;
        private bool _penJustDown;

        private IntPtr HookCallback(int nCode, IntPtr wParam, IntPtr lParam)
        {
            if (nCode < 0)
                return NativeMethods.CallNextHookEx(_hookId, nCode, wParam, lParam);

            if (!Enabled)
                return NativeMethods.CallNextHookEx(_hookId, nCode, wParam, lParam);

            double msSincePen = (DateTime.UtcNow - OverlayForm.LastPenEventUtc).TotalMilliseconds;
            bool isFromPen = msSincePen < SuppressWindowMs || OverlayForm.HidTipDown;

            if (!isFromPen)
            {
                _passCount++;
                return NativeMethods.CallNextHookEx(_hookId, nCode, wParam, lParam);
            }

            int msg = (int)wParam;
            var hookStruct = (NativeMethods.MSLLHOOKSTRUCT)Marshal.PtrToStructure(
                lParam, typeof(NativeMethods.MSLLHOOKSTRUCT));
            int x = hookStruct.pt.X;
            int y = hookStruct.pt.Y;

            switch (msg)
            {
                case NativeMethods.WM_MOUSEMOVE:
                    _suppressCount++;
                    _lastPenX = x; _lastPenY = y;
                    if (_suppressCount <= 3)
                        Console.WriteLine("[Hook] SUPPRESS MOVE #{0} at ({1},{2})", _suppressCount, x, y);
                    if (_penJustDown)
                    {
                        _penJustDown = false;
                        Console.WriteLine("[Hook] → PenDown at ({0},{1})", x, y);
                        if (PenDown != null) PenDown(x, y);
                    }
                    if (PenMove != null) PenMove(x, y);
                    return (IntPtr)1;

                case NativeMethods.WM_LBUTTONDOWN:
                    _downCount++;
                    _penJustDown = true; // delay until first MOVE with actual position
                    Console.WriteLine("[Hook] PEN DOWN #{0} (deferred)", _downCount);
                    return (IntPtr)1;

                case NativeMethods.WM_LBUTTONUP:
                    _upCount++;
                    _penJustDown = false;
                    Console.WriteLine("[Hook] PEN UP #{0} at ({1},{2})", _upCount, x, y);
                    if (PenUp != null) PenUp(x, y);
                    return (IntPtr)1;

                case NativeMethods.WM_RBUTTONDOWN:
                case NativeMethods.WM_RBUTTONUP:
                case NativeMethods.WM_MBUTTONDOWN:
                case NativeMethods.WM_MBUTTONUP:
                    return (IntPtr)1;
            }

            return NativeMethods.CallNextHookEx(_hookId, nCode, wParam, lParam);
        }

        public void Dispose()
        {
            Uninstall();
        }
    }
}
