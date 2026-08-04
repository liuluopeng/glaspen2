// PenEventProbe.cs — 在 Windows 上观察笔合成鼠标事件的"签名痕迹"。
//
// 目的: 本机笔驱动把 GetMessageExtraInfo 写成 0x0 (无 0xFF515700 签名),
// 导致 WH_MOUSE_LL 钩子无法区分笔与真鼠标。本探针记录每个鼠标事件的:
//   - dwExtraInfo 完整值 (hex)          ← 也许不是 0, 而是某个别的常数
//   - MSLLHOOKSTRUCT.flags 的 INJECTED 位 ← 很多驱动给笔事件打"注入"标记
// 对比"真鼠标移动"与"笔悬停/落笔"的日志差异, 找出可用的替代标志。
//
// 编译 (Windows, 管理员不需要, 普通权限即可):
//   csc /out:PenEventProbe.exe PenEventProbe.cs
// 运行:
//   PenEventProbe.exe > probe.log
//   然后: 用真鼠标来回移动几秒 → 用笔悬停/落笔几秒 → Ctrl+C 结束
//   查看 probe.log 中两类事件 dwExtraInfo / flags 的差异。

using System;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;

class PenEventProbe
{
    delegate IntPtr LowLevelMouseProc(int nCode, IntPtr wParam, IntPtr lParam);

    [StructLayout(LayoutKind.Sequential)]
    struct MSLLHOOKSTRUCT
    {
        public int ptX, ptY;
        public uint mouseData;
        public uint flags;      // LLMHF_INJECTED = 0x00000001
        public uint time;
        public UIntPtr dwExtraInfo;
    }

    [DllImport("user32.dll", SetLastError = true)]
    static extern IntPtr SetWindowsHookEx(int idHook, LowLevelMouseProc lpfn,
        IntPtr hMod, uint dwThreadId);
    [DllImport("user32.dll")]
    static extern bool UnhookWindowsHookEx(IntPtr hhk);
    [DllImport("user32.dll")]
    static extern IntPtr CallNextHookEx(IntPtr hhk, int nCode, IntPtr wParam, IntPtr lParam);
    [DllImport("kernel32.dll", CharSet = CharSet.Auto)]
    static extern IntPtr GetModuleHandle(string lpModuleName);

    const int WH_MOUSE_LL = 14;
    const uint LLMHF_INJECTED = 0x00000001;

    static IntPtr _hookId = IntPtr.Zero;
    static long _lastLog = 0;
    static long _count = 0;

    static IntPtr HookCallback(int nCode, IntPtr wParam, IntPtr lParam)
    {
        if (nCode >= 0)
        {
            var ms = (MSLLHOOKSTRUCT)Marshal.PtrToStructure(lParam, typeof(MSLLHOOKSTRUCT));
            _count++;
            // 每秒最多打一行, 避免刷屏 (用秒级去重)
            long now = Environment.TickCount64;
            if (now - _lastLog >= 1000)
            {
                _lastLog = now;
                string msg = wParam.ToInt64() switch
                {
                    0x0200 => "MOVE",
                    0x0201 => "LDOWN",
                    0x0202 => "LUP",
                    0x0204 => "RDOWN",
                    0x0205 => "RUP",
                    0x0206 => "MDOWN",
                    0x0207 => "MUP",
                    0x020A => "WHEEL",
                    _ => $"0x{wParam.ToInt64():X4}",
                };
                bool injected = (ms.flags & LLMHF_INJECTED) != 0;
                Console.WriteLine($"[{_count,6}] {msg,-6} flags=0x{ms.flags:X8}{(injected ? " INJECTED" : "")} " +
                                  $"extra=0x{ms.dwExtraInfo.ToUInt64():X16} pos=({ms.ptX},{ms.ptY})");
            }
        }
        return CallNextHookEx(_hookId, nCode, wParam, lParam);
    }

    static void Main()
    {
        Console.WriteLine("PenEventProbe — 观察鼠标事件签名痕迹");
        Console.WriteLine("步骤: 1) 真鼠标移动几秒  2) 笔悬停/落笔几秒  3) Ctrl+C 退出");
        Console.WriteLine("注意: 所有事件都 pass through (不拦截), 不影响系统。");
        Console.WriteLine("----");
        using (var proc = Process.GetCurrentProcess())
        using (var module = proc.MainModule)
        {
            _hookId = SetWindowsHookEx(WH_MOUSE_LL, HookCallback,
                GetModuleHandle(module.ModuleName), 0);
        }
        if (_hookId == IntPtr.Zero)
        {
            Console.WriteLine($"SetWindowsHookEx 失败: {Marshal.GetLastWin32Error()}");
            return;
        }
        Console.WriteLine("钩子已安装, 等待事件...");
        // 钩子回调在本线程的消息循环中执行, 必须泵消息
        while (true)
        {
            System.Threading.Thread.Sleep(50);
        }
    }
}
