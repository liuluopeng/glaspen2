using System;
using System.Runtime.InteropServices;

namespace GlasPen2
{
    /// <summary>Wintab API P/Invoke for pressure-sensitive tablet input.</summary>
    internal static class Wintab
    {
        // ── Packet request flags (standard Wintab 1.x values) ──
        public const uint PK_CONTEXT          = 0x0001;
        public const uint PK_STATUS           = 0x0002;
        public const uint PK_TIME             = 0x0004;
        public const uint PK_CHANGED          = 0x0008;
        public const uint PK_BUTTONS          = 0x0040;
        public const uint PK_X                = 0x0080;
        public const uint PK_Y                = 0x0100;
        public const uint PK_NORMAL_PRESSURE  = 0x0400;

        public const uint PK_WANT = PK_BUTTONS | PK_X | PK_Y | PK_NORMAL_PRESSURE;

        // ── Context options ──
        public const uint CXO_SYSTEM  = 0x0002; // system cursor tracking
        public const uint CXO_PEN     = 0x0004; // pen context
        public const uint CXO_MESSAGES = 0x0008; // send WT_PACKET messages

        // ── Status flags ──
        public const uint CXS_ONTOP = 0x0004;

        // ── WTInfo categories ──
        public const uint WTI_INTERFACE = 1;
        public const uint WTI_DEVICES   = 100;
        public const uint WTI_DDCTXS    = 400; // default digitizer context

        // ── Packet message range ──
        public const uint WT_DEFBASE   = 0x7FF0;
        public const uint WT_MAXOFFSET = 31;
        public const uint WT_PACKET    = 0;

        // ── Structures ──

        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Ansi)]
        public struct LOGCONTEXT
        {
            [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 40)]
            public string lcName;
            public uint lcOptions;
            public uint lcStatus;
            public uint lcLocks;
            public uint lcMsgBase;
            public uint lcDevice;
            public uint lcPktRate;
            public uint lcPktData;
            public uint lcPktMode;
            public uint lcMoveMask;
            public uint lcBtnDnMask;
            public uint lcBtnUpMask;
            public int lcInOrgX;
            public int lcInOrgY;
            public int lcInExtX;
            public int lcInExtY;
            public int lcOutOrgX;
            public int lcOutOrgY;
            public int lcOutExtX;
            public int lcOutExtY;
            public int lcSensX;    // FIX32
            public int lcSensY;
            public int lcSysMode;
            public int lcSysOrgX;
            public int lcSysOrgY;
            public int lcSysExtX;
            public int lcSysExtY;
            public int lcSysSensX;
            public int lcSysSensY;
        }

        /// <summary>Packet with PK_BUTTONS | PK_X | PK_Y | PK_NORMAL_PRESSURE</summary>
        [StructLayout(LayoutKind.Sequential)]
        public struct PACKET
        {
            public uint pkButtons;
            public int pkX;
            public int pkY;
            public uint pkNormalPressure;
        }

        // ── Functions ──

        [DllImport("wintab32.dll", CharSet = CharSet.Ansi)]
        public static extern uint WTInfoA(uint wCategory, uint nIndex, IntPtr lpOutput);

        [DllImport("wintab32.dll", CharSet = CharSet.Unicode)]
        public static extern uint WTInfoW(uint wCategory, uint nIndex, IntPtr lpOutput);

        [DllImport("wintab32.dll", CharSet = CharSet.Ansi)]
        public static extern IntPtr WTOpenA(IntPtr hWnd, ref LOGCONTEXT lpLogCtx, bool fEnable);

        [DllImport("wintab32.dll", CharSet = CharSet.Unicode)]
        public static extern IntPtr WTOpenW(IntPtr hWnd, ref LOGCONTEXT lpLogCtx, bool fEnable);

        [DllImport("wintab32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool WTClose(IntPtr hCtx);

        [DllImport("wintab32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool WTEnable(IntPtr hCtx, bool fEnable);

        [DllImport("wintab32.dll")]
        public static extern uint WTPacket(IntPtr hCtx, uint wSerial, IntPtr lpPkt);

        [DllImport("wintab32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool WTOverlap(IntPtr hCtx, bool fToTop);
    }
}
