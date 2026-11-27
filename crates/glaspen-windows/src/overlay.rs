//! glaspen2 Windows overlay — 纯 Rust 实现(无 C#)。
//!
//! 架构(基于已验证原型 raw_input_trans_draw.rs):
//!   - 单层全屏 WS_EX_LAYERED 窗口 + UpdateLayeredWindowIndirect(32bit BGRA alpha) 合成
//!   - 输入:WM_INPUT(Raw Input)手工解析 HID 报告,任何窗口状态都能收到;
//!     量程按 hDevice 从 HID value caps 动态读取(驱动更新后虚拟数位板
//!     VID_00FF/BACC 归一化到 32767x32767,写死 XP-Pen 量程会笔迹偏移)
//!   - 笔迹:core modeler_session(位置/压力双平滑 + 120Hz 重采样)→ 可变宽度轮廓
//!     (法线偏移 + cairo 抗锯齿填充 + 端点圆帽);Windows 只剩薄适配层
//!     src/pen_modeler.rs(D9b)
//!   - 拦截:笔事件到达清除 WS_EX_TRANSPARENT,笔离开 500ms 后恢复穿透
//!   - 集成 glaspen2 功能:Flutter 设置管道、撤销/清屏/保存导出、热键
//!
//! 依赖版本与 wAPItry 原型一致(windows 0.62、libloading 0.9)。

#![allow(unsafe_op_in_unsafe_fn)]

use std::collections::HashMap;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::time::Instant;

use crate::pen_modeler::{self, PenModeler, PenReport};
use windows::Win32::Devices::HumanInterfaceDevice::{
    HIDP_VALUE_CAPS, HidD_GetPreparsedData, HidP_GetValueCaps, HidP_Input, PHIDP_PREPARSED_DATA,
};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Pipes::PeekNamedPipe;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN,
    RegisterHotKey, UnregisterHotKey, VK_CONTROL, VK_LCONTROL, VK_LSHIFT, VK_LWIN, VK_MENU,
    VK_RCONTROL, VK_RSHIFT, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::Input::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::PCWSTR;

use glaspen_core::cairo_dl::CairoRenderer;
use glaspen_core::camera::{Camera, ZoomOutcome};
use glaspen_core::grid::Segment;
// D10b 文案单源: 两端共享的用户可见文案只许查 core::i18n(与 macOS 同一张表)。
// Windows 覆盖层目前只有中文 HUD("language" 设置只影响独立的 Flutter 设置面板
// 进程, 覆盖层没有语言状态/切换入口), 所以各处固定 Lang::Zh; 键名占位符
// ({keys}) 由 adapter 传本平台键位(Ctrl+Alt+…)。
use glaspen_core::i18n::{self, Lang, Msg};

// ── 文件拆分(纯文本 include, 编译期等价于原单文件; 无 Windows 编译环境,
//    用 include! 保证拆分零语义变化; 错误行号仍指向真实子文件)──
include!("overlay/hid.rs");
include!("overlay/cmds.rs");
include!("overlay/state.rs");
include!("overlay/canvas.rs");
include!("overlay/pen.rs");
include!("overlay/hid_raw.rs");
include!("overlay/wndproc.rs");
include!("overlay/run.rs");
include!("overlay/mouse.rs");
include!("overlay/hud.rs");
include!("overlay/actions.rs");
include!("overlay/tray.rs");
include!("overlay/recording.rs");
include!("overlay/hotkeys.rs");
include!("overlay/pipe.rs");
