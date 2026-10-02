// ── 自定义消息与命令 ID(Flutter 设置管道用) ──

pub const WM_TRAY_COMMAND: u32 = WM_USER + 1;
/// 录制完成 → 消息循环:wparam 1=已复制剪贴板, 2=没有笔迹/失败
const WM_RECORD_DONE: u32 = WM_USER + 3;

pub const CMD_SELECT_COLOR: usize = 100;
pub const CMD_SELECT_WIDTH: usize = 200;
pub const CMD_SAVE_WITH_BG: usize = 300;
pub const CMD_SAVE_DRAWING: usize = 301;
pub const CMD_SAVE_XOJ: usize = 302;
pub const CMD_CLEAR_SCREEN: usize = 400;
pub const CMD_TOGGLE_RAINBOW: usize = 500;
pub const CMD_TOGGLE_ENABLED: usize = 600;
pub const CMD_TOGGLE_OUTLINE: usize = 650;
pub const CMD_TOGGLE_GRID: usize = 652;
pub const CMD_TOGGLE_FROSTED: usize = 653;
pub const CMD_TOGGLE_ETHEREAL: usize = 654;
pub const CMD_TOGGLE_PRESSURE_MONITOR: usize = 655;
pub const CMD_SET_GRID_DIVIDER: usize = 656; // 网格分栏 0..3(面板「分栏」设置)
pub const CMD_TOGGLE_CHAT_INTEGRATION: usize = 657; // 手写消息集成总开关(面板)
pub const CMD_SET_GRID_SIZE: usize = 658; // 网格尺寸 20/40/80(面板)
pub const CMD_TOGGLE_MINIMAP: usize = 659; // 页面缩略图条开关(面板)
pub const CMD_MINIMAP_REFRESH: usize = 660; // 后台缩略图渲染完成 → 重绘条带
pub const CMD_NAVIGATE_TO_PAGE: usize = 810;
pub const CMD_PAGE_PREV: usize = 720;
pub const CMD_PAGE_NEXT: usize = 721;
pub const CMD_EXPORT_SVG_GIF: usize = 722;
pub const CMD_CANVAS_HOME: usize = 730; // 无限画布:镜头回原点 + 100%
pub const CMD_CANVAS_CENTER: usize = 731; // 无限画布:镜头居中内容包围盒
pub const CMD_CANVAS_NEW: usize = 732; // 无限画布:清空内容 + 镜头回原点
pub const CMD_TOGGLE_INFINITE_CANVAS: usize = 733; // 活页本 ↔ 无限画布
pub const CMD_UNDO: usize = 800;
pub const CMD_QUIT: usize = 999;

// ── 颜色 & 线宽预设 ──
// 数值单源在 glaspen-core presets(macOS 同一实现);这里的常量是本平台
// UI 字符串(中文 HUD/菜单名)。
pub use glaspen_core::presets::{COLOR_PRESETS, WIDTH_PRESETS};
pub const COLOR_NAMES_ZH: [&str; 10] = ["红", "橙", "黄", "绿", "青", "蓝", "紫", "粉", "白", "黑"];
pub const WIDTH_NAMES_ZH: [&str; 8] = ["极细", "很细", "细", "中", "粗", "很粗", "超粗", "极粗"];

