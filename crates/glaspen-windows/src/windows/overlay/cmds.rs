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
pub const COLOR_PRESETS: [(f64, f64, f64); 10] = [
    // 对齐 rnote 实测色板:全部 S=100% 全饱和,鲜艳度优先(浅色场景配描边)
    (0.839, 0.000, 0.227), // Red    #D6003A
    (1.000, 0.302, 0.000), // Orange #FF4D00
    (0.988, 0.718, 0.000), // Yellow #FCB700
    (0.000, 0.694, 0.431), // Green  #00B16E
    (0.431, 0.769, 0.957), // Cyan   #6EC4F4
    (0.000, 0.439, 0.741), // Blue   #0070BD
    (0.541, 0.000, 0.902), // Purple #8A00E6
    (1.000, 0.000, 0.502), // Pink   #FF0080
    (1.0, 1.0, 1.0),       // White
    (0.0, 0.0, 0.0),       // Black
];
pub const COLOR_NAMES_ZH: [&str; 10] = ["红", "橙", "黄", "绿", "青", "蓝", "紫", "粉", "白", "黑"];
// 8 档线宽倍率,与 Flutter 设置 UI 的 8 档一一对应
pub const WIDTH_PRESETS: [f64; 8] = [0.15, 0.3, 0.6, 1.0, 1.5, 2.0, 2.5, 3.5];
pub const WIDTH_NAMES_ZH: [&str; 8] = ["极细", "很细", "细", "中", "粗", "很粗", "超粗", "极粗"];

