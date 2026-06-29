using System;
using System.Runtime.InteropServices;

namespace GlasPen2
{
    /// <summary>
    /// Global mouse hook that:
    /// 1. Detects pen-originated mouse events via GetMessageExtraInfo (PEN_SIGNATURE)
    /// 2. Suppresses ALL pen mouse events → no click-through to underlying windows
    /// 3. Signals PenDown/PenMove/PenUp for our drawing logic
    /// 4. Falls back to timing-based detection when GetMessageExtraInfo is unavailable
    /// </summary>
    public class PenInterceptor : IDisposable
    {
        private IntPtr _hookId = IntPtr.Zero;
        private NativeMethods.LowLevelMouseProc _proc;

        /// <summary>Mouse events within this window after a raw pen event are considered pen input.</summary>
        private const double SuppressWindowMs = 80.0;

        public bool Enabled { get; set; }

        /// <summary>Fired when pen touches tablet (from pen, not mouse).</summary>
        public event Action<int, int> PenDown;

        /// <summary>Fired when pen moves while touching.</summary>
        public event Action<int, int> PenMove;

        /// <summary>Fired when pen lifts.</summary>
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
            Console.WriteLine("[Hook] Installed OK. Pen detection: GetMessageExtraInfo + timing fallback");
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

            int msg = (int)wParam;
            var hookStruct = (NativeMethods.MSLLHOOKSTRUCT)Marshal.PtrToStructure(
                lParam, typeof(NativeMethods.MSLLHOOKSTRUCT));
            int x = hookStruct.pt.X;
            int y = hookStruct.pt.Y;

            // ── Primary detection: dwExtraInfo from hook struct ──
            // Windows INK sets PEN_SIGNATURE in dwExtraInfo when it injects
            // pen-as-mouse events. Use the hook struct field, NOT GetMessageExtraInfo()
            // (which only works for messages in a thread queue, not hook callbacks).
            ulong extraVal = (ulong)hookStruct.dwExtraInfo.ToInt64();
            bool isFromPen = (extraVal & NativeMethods.PEN_SIGNATURE_MASK) == NativeMethods.PEN_SIGNATURE;

            // Debug: log first few events to verify detection
            if (_passCount + _suppressCount < 20)
            {
                Console.WriteLine("[Hook] msg=0x{0:X4} pos=({1},{2}) extraInfo=0x{3:X16} isPen={4}",
                    msg, x, y, extraVal, isFromPen);
            }

            // ── Fallback: timing-based detection ──
            // For systems where GetMessageExtraInfo doesn't work.
            if (!isFromPen)
            {
                double msSincePen = (DateTime.UtcNow - OverlayForm.LastPenEventUtc).TotalMilliseconds;
                isFromPen = msSincePen < SuppressWindowMs || OverlayForm.HidTipDown;
            }

            // Not from pen → real mouse event → pass through
            if (!isFromPen)
            {
                _passCount++;
                return NativeMethods.CallNextHookEx(_hookId, nCode, wParam, lParam);
            }

            // ── This is a pen event — suppress it (don't let it reach any window) ──
            switch (msg)
            {
                case NativeMethods.WM_MOUSEMOVE:
                    _suppressCount++;
                    _lastPenX = x; _lastPenY = y;
                    if (_suppressCount <= 3)
                        Console.WriteLine("[Hook] PEN MOVE #{0} at ({1},{2}) [suppressed]", _suppressCount, x, y);
                    if (_penJustDown)
                    {
                        _penJustDown = false;
                        Console.WriteLine("[Hook] → PenDown at ({0},{1})", x, y);
                        if (PenDown != null) PenDown(x, y);
                    }
                    if (PenMove != null) PenMove(x, y);
                    return (IntPtr)1; // suppress — don't pass to any window

                case NativeMethods.WM_LBUTTONDOWN:
                    _downCount++;
                    _penJustDown = true;
                    Console.WriteLine("[Hook] PEN DOWN #{0} at ({1},{2}) [suppressed]", _downCount, x, y);
                    return (IntPtr)1; // suppress — no click-through

                case NativeMethods.WM_LBUTTONUP:
                    _upCount++;
                    _penJustDown = false;
                    Console.WriteLine("[Hook] PEN UP #{0} at ({1},{2}) [suppressed]", _upCount, x, y);
                    if (PenUp != null) PenUp(x, y);
                    return (IntPtr)1; // suppress — no click-through

                case NativeMethods.WM_LBUTTONDBLCLK:
                    Console.WriteLine("[Hook] PEN DBLCLK [suppressed]");
                    return (IntPtr)1; // suppress — no double-click selection

                case NativeMethods.WM_RBUTTONDOWN:
                case NativeMethods.WM_RBUTTONUP:
                case NativeMethods.WM_MBUTTONDOWN:
                case NativeMethods.WM_MBUTTONUP:
                    return (IntPtr)1; // suppress barrel button events
            }

            return NativeMethods.CallNextHookEx(_hookId, nCode, wParam, lParam);
        }

        public void Dispose()
        {
            Uninstall();
        }
    }
}
