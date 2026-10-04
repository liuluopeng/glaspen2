pub struct DrawState {
    pub pen_r: f64,
    pub pen_g: f64,
    pub pen_b: f64,
    pub width_scale: f64,
    pub selected_color: usize,
    pub selected_width: usize,
    pub enabled: bool,
    pub show_rainbow: bool,
    pub outline_enabled: bool,
    pub show_grid: bool,
    /// 网格分栏(纯视觉):0=无 1=左右两栏 2=上下两栏 3=九宫格(macOS 同键)
    pub grid_divider: i32,
    pub frosted: bool,
    /// 飘渺画布涂鸦模式:悬空/落笔显示笔迹,笔离开后隐藏
    pub ethereal: bool,
    /// 网格跟随涂鸦:飘渺模式下隐藏笔迹时网格也隐藏
    pub grid_follow_strokes: bool,
    /// 磨砂玻璃跟随涂鸦:飘渺模式下隐藏笔迹时磨砂背景一起关(默认开=macOS 同款)
    pub glass_follow_strokes: bool,
    /// 压力监控 HUD 是否开启
    pub pressure_monitor: bool,
    /// 手写消息集成总开关(⌘⌃2/⌘⌃3 → Ctrl+Alt+2/3 的守门;面板同键)
    pub chat_integration: bool,
}

// ── 共享状态(仅消息循环线程访问) ──

pub static OVERLAY_HWND: std::sync::Mutex<isize> = std::sync::Mutex::new(0);

/// 笔迹描边(渲染设置):仅在内存,不落库,重启恢复关闭。
/// 管道线程(getSettings)与消息循环共享此值。
pub static OUTLINE_ENABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

// ── 无限画布:模式开关 + 镜头(macOS 同款) ──
// 视图 = (画布 − pan) × zoom,zoom ∈ (0.05, 1](上限 100% 防蚂蚁大小涂鸦)。
// 笔迹以画布坐标存储(可为负/超屏),输入侧 视图→画布,渲染侧 画布→视图。
// 开关与镜头用原子量:消息循环线程独占改写,管道线程(getSettings /
// 画布总览)只读。翻页模式下两者不起作用(pan=0、zoom=1)。
pub static INFINITE_CANVAS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

static CAM_PAN_X: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static CAM_PAN_Y: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static CAM_ZOOM: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

const ZOOM_MIN: f64 = 0.05;
const ZOOM_MAX: f64 = 1.0;
const ZOOM_HINT_MS: u64 = 1500; // "已达最大缩放"提示节流
const CAM_SAVE_INTERVAL_MS: u64 = 500; // 镜头持久化节流(与 macOS 一致)

/// 当前镜头 (pan_x, pan_y, zoom)
fn cam() -> (f64, f64, f64) {
    use std::sync::atomic::Ordering;
    let z = f64::from_bits(CAM_ZOOM.load(Ordering::SeqCst));
    (
        f64::from_bits(CAM_PAN_X.load(Ordering::SeqCst)),
        f64::from_bits(CAM_PAN_Y.load(Ordering::SeqCst)),
        if z > 0.0 { z } else { 1.0 },
    )
}

fn set_cam(px: f64, py: f64, z: f64) {
    use std::sync::atomic::Ordering;
    CAM_PAN_X.store(px.to_bits(), Ordering::SeqCst);
    CAM_PAN_Y.store(py.to_bits(), Ordering::SeqCst);
    CAM_ZOOM.store(z.to_bits(), Ordering::SeqCst);
}

fn infinite_on() -> bool {
    INFINITE_CANVAS.load(std::sync::atomic::Ordering::SeqCst)
}

/// 视图坐标 → 画布坐标(笔输入;视图 = 屏幕像素坐标)
fn canvas_from_view(vx: f64, vy: f64) -> (f64, f64) {
    if infinite_on() {
        let (px, py, z) = cam();
        (vx / z + px, vy / z + py)
    } else {
        (vx, vy)
    }
}

/// 画布坐标 → 视图坐标(渲染)
fn view_from_canvas(cx: f64, cy: f64) -> (f64, f64) {
    if infinite_on() {
        let (px, py, z) = cam();
        ((cx - px) * z, (cy - py) * z)
    } else {
        (cx, cy)
    }
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ── 快捷录制 GIF(Ctrl+Alt+R 按住):质量设置(macOS 同款键名) ──
// 消息循环线程独占改写,管道线程(getSettings)只读。
static GIF_FPS: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(15);
static GIF_RESOLUTION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static GIF_SPEED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static GIF_END_MODE: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(1);

fn gif_settings() -> (i32, f64, f64, i32) {
    use std::sync::atomic::Ordering;
    (
        GIF_FPS.load(Ordering::SeqCst),
        f64::from_bits(GIF_RESOLUTION.load(Ordering::SeqCst)),
        f64::from_bits(GIF_SPEED.load(Ordering::SeqCst)),
        GIF_END_MODE.load(Ordering::SeqCst),
    )
}

fn set_gif_settings(fps: i32, resolution: f64, speed: f64, end_mode: i32) {
    use std::sync::atomic::Ordering;
    // 区间单源在 core presets(macOS 同一表);此前本侧 0.05/0.5 与
    // macOS 的 0.1/0.25 漂移,同一库文件两侧读出的合法值不一致。
    use glaspen_core::presets::{clamp_setting_double, clamp_setting_int};
    GIF_FPS.store(clamp_setting_int("gifFps", fps), Ordering::SeqCst);
    GIF_RESOLUTION.store(
        clamp_setting_double("gifResolution", resolution).to_bits(),
        Ordering::SeqCst,
    );
    GIF_SPEED.store(clamp_setting_double("gifSpeed", speed).to_bits(), Ordering::SeqCst);
    GIF_END_MODE.store(clamp_setting_int("gifEndMode", end_mode), Ordering::SeqCst);
}

/// 从 user_settings 恢复(键名与 macOS 一致,设置数据库可互换)
fn load_gif_settings() {
    let rt = glaspen_core::runtime();
    let fps = rt
        .block_on(glaspen_core::db::load_setting("gif_fps"))
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(15);
    let resolution = rt
        .block_on(glaspen_core::db::load_setting("gif_resolution"))
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.5);
    let speed = rt
        .block_on(glaspen_core::db::load_setting("gif_speed"))
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(2.0);
    let end_mode = rt
        .block_on(glaspen_core::db::load_setting("gif_end_mode"))
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(1);
    set_gif_settings(fps, resolution, speed, end_mode);
}

fn persist_gif_settings(fps: i32, resolution: f64, speed: f64, end_mode: i32) {
    set_gif_settings(fps, resolution, speed, end_mode);
    let (fps, res, speed, end_mode) = gif_settings();
    let rt = glaspen_core::runtime();
    rt.block_on(glaspen_core::db::save_setting("gif_fps", &fps.to_string()));
    rt.block_on(glaspen_core::db::save_setting(
        "gif_resolution",
        &format!("{res:.4}"),
    ));
    rt.block_on(glaspen_core::db::save_setting("gif_speed", &format!("{speed:.4}")));
    rt.block_on(glaspen_core::db::save_setting(
        "gif_end_mode",
        &end_mode.to_string(),
    ));
}

// ── 网格尺寸(macOS 同键 gridSize,库键 grid_size,10..200 钳制) ──
// 消息循环线程独占改写,管道线程(getSettings)只读。
static GRID_SIZE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn grid_size() -> f64 {
    let v = f64::from_bits(GRID_SIZE.load(std::sync::atomic::Ordering::SeqCst));
    if v > 0.0 {
        v
    } else {
        40.0
    }
}

fn set_grid_size(v: f64) {
    use std::sync::atomic::Ordering;
    let v = glaspen_core::presets::clamp_setting_double("gridSize", v);
    let v = if v > 0.0 { v } else { 40.0 };
    GRID_SIZE.store(v.to_bits(), Ordering::SeqCst);
}

fn load_grid_size() {
    let v = glaspen_core::runtime()
        .block_on(glaspen_core::db::load_setting("grid_size"))
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(40.0);
    set_grid_size(v);
}

// ── 页面缩略图条(minimap,仅活页本模式;macOS 同键,库键 minimap) ──
pub static MINIMAP_ENABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

struct OverlayState {
    canvas: OverlayCanvas,
    draw: DrawState,
    /// 当前笔的曲线点列(像素坐标 + 半径),用于轮廓填充
    pen_path: Vec<(f32, f32, f32)>,
    /// ink-stroke-modeler(位置/压力平滑 + 120Hz 重采样)
    stroke_modeler: StrokeModeler,
    start_time: Instant,
    /// 当前是否处于笔画中(首事件必须发 Down)
    in_stroke: bool,
    /// 笔迹当前是否可见(飘渺模式)
    strokes_visible: bool,
    /// 快捷录制 GIF 进行中(Ctrl+Alt+R 按住)
    gif_recording: bool,
    /// 录制起点笔画序号(-1 = 未在录制)
    gif_record_start: i32,
    /// ⌘⌃3 直发:按住录制的起点笔画序号(-1 = 未在录制)
    msg_record_start: i32,
    /// ⌘⌃2 草稿通道进行中(按住期间实时推送)
    ink_draft_active: bool,
}
