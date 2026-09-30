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
    pub frosted: bool,
    /// 飘渺画布涂鸦模式:悬空/落笔显示笔迹,笔离开后隐藏
    pub ethereal: bool,
    /// 网格跟随涂鸦:飘渺模式下隐藏笔迹时网格也隐藏
    pub grid_follow_strokes: bool,
    /// 压力监控 HUD 是否开启
    pub pressure_monitor: bool,
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
    GIF_FPS.store(fps.clamp(1, 50), Ordering::SeqCst);
    GIF_RESOLUTION.store(resolution.clamp(0.05, 1.0).to_bits(), Ordering::SeqCst);
    GIF_SPEED.store(speed.clamp(0.5, 20.0).to_bits(), Ordering::SeqCst);
    GIF_END_MODE.store(end_mode.clamp(0, 2), Ordering::SeqCst);
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

struct OverlayState {
    canvas: OverlayCanvas,
    draw: DrawState,
