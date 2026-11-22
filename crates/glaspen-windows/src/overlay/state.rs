pub struct DrawState {
    pub pen_r: f64,
    pub pen_g: f64,
    pub pen_b: f64,
    pub width_scale: f64,
    pub selected_color: usize,
    pub selected_width: usize,
    /// 换色/换粗细的"上一档"历史(可配置动作 color_swap / width_swap 用,
    /// 与 macOS g_prev_color_index / g_prev_width_index 同语义)
    pub prev_color: usize,
    pub prev_width: usize,
    pub enabled: bool,
    pub outline_enabled: bool,
    pub show_grid: bool,
    /// 网格分栏(纯视觉):0=无 1=左右两栏 2=上下两栏 3=九宫格 4=康奈尔(macOS 同键)
    pub grid_divider: i32,
    pub frosted: bool,
    /// 飘渺画布涂鸦模式:悬空/落笔显示笔迹,笔离开后隐藏
    pub ethereal: bool,
    /// 网格跟随涂鸦:飘渺模式下隐藏笔迹时网格也隐藏
    pub grid_follow_strokes: bool,
    /// 磨砂玻璃跟随涂鸦:飘渺模式下隐藏笔迹时磨砂背景一起关(默认开=macOS 同款)
    pub glass_follow_strokes: bool,
    /// 软阴影(独立于描边可叠加):笔迹下方柔和黑影,同色背景保底可见
    pub soft_shadow: bool,
    /// 反色突出(实验):设置同键持久化,覆盖层行为 macOS v1 独有,Windows 暂不消费
    pub invert_ink: bool,
    /// 反色追踪帧率上限(10/30/60/100),同键持久化(macOS 消费)
    pub invert_fps: i32,
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
//
// D5b: 镜头数学唯一来源 = core camera::Camera(锚点/限幅/居中/步长/持久化格式);
// 本文件只做状态存取(对应 macOS 的 g_pan_x/g_pan_y/g_zoom 赋值)。两端真实
// 差异(键盘平移方向符号、最小缩放提示仅 Windows、居中 zoom 回写、非法 zoom
// 绘制兜底)留在 adapter,见 camera.rs 模块文档。
pub static INFINITE_CANVAS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

static CAM_PAN_X: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static CAM_PAN_Y: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static CAM_ZOOM: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

const ZOOM_HINT_MS: u64 = 1500; // "已达最大缩放"提示节流
const CAM_SAVE_INTERVAL_MS: u64 = 500; // 镜头持久化节流(与 macOS 一致)

/// 当前镜头。读取时 zoom 经 [Camera::from_stored] 兜底(> 0 否则 1.0),
/// 与 core view_transform() / 旧 cam() 同款(渲染除零兜底, 见 camera.rs
/// 模块文档差异 3)—— 历史脏值(0/NaN)不会传播到绘制/命中数学。
fn camera() -> Camera {
    use std::sync::atomic::Ordering;
    Camera::from_stored(
        f64::from_bits(CAM_PAN_X.load(Ordering::SeqCst)),
        f64::from_bits(CAM_PAN_Y.load(Ordering::SeqCst)),
        f64::from_bits(CAM_ZOOM.load(Ordering::SeqCst)),
    )
}

/// 写回镜头(core 算完的完整三值;字段级写回与旧原子量布局/线程模型一致)。
fn set_camera(cam: Camera) {
    use std::sync::atomic::Ordering;
    CAM_PAN_X.store(cam.pan_x.to_bits(), Ordering::SeqCst);
    CAM_PAN_Y.store(cam.pan_y.to_bits(), Ordering::SeqCst);
    CAM_ZOOM.store(cam.zoom.to_bits(), Ordering::SeqCst);
}

fn infinite_on() -> bool {
    INFINITE_CANVAS.load(std::sync::atomic::Ordering::SeqCst)
}

/// 视图坐标 → 画布坐标(笔输入;视图 = 屏幕像素坐标)
fn canvas_from_view(vx: f64, vy: f64) -> (f64, f64) {
    if infinite_on() {
        camera().view_to_canvas(vx, vy)
    } else {
        (vx, vy)
    }
}

/// 画布坐标 → 视图坐标(渲染)
fn view_from_canvas(cx: f64, cy: f64) -> (f64, f64) {
    if infinite_on() {
        camera().canvas_to_view(cx, cy)
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
    /// 最近一份笔 HID 报告的时间。穿透计时器用它判断"笔还在活动":
    /// 抬笔报告丢失时(笔直接离开感应区)没有新的 in_stroke=false 报告,
    /// 超时后必须强制收笔并恢复鼠标穿透,否则全屏覆盖层会一直吃掉鼠标点击。
    last_report: Instant,
    /// 最近一次"笔真的在动"的时间(落笔,或悬空移动超过阈值)。
    /// 用于"落地前先拦截合成鼠标": 笔悬空移动时提前清 WS_EX_TRANSPARENT,
    /// 这样下一次落笔的驱动合成鼠标事件不会点穿到下层窗口(0.6.0 第一笔穿透);
    /// 而笔静止悬停/搁在板上时不刷新, 几百 ms 后恢复鼠标可用(键鼠/笔分离)。
    last_activity: Instant,
    /// 上一份悬空报告的屏幕坐标(判断悬空移动距离)
    last_hover: Option<(f32, f32)>,
    /// 当前是否处于笔画中(首事件必须发 Down)
    in_stroke: bool,
    /// 笔迹当前是否可见(飘渺模式)
    strokes_visible: bool,
    /// 飘渺 peek 计时器是否在跑(macOS g_peek_timer 同义): 镜头移动时判断
    /// 要不要重新计时; 笔正在悬停可见且没有计时器时保持可见, 不启动隐藏。
    peek_active: bool,
    /// 末页按"下一页"的首次时间: 5 秒内再按一次 = 新建一页并翻过去
    /// (macOS perform_hotkey(0x12) 同款规则)
    last_no_next: Option<Instant>,
    /// 无限画布: Ctrl+Alt + 左键拖动平移进行中(macOS g_camera_mouse_dragging)
    camera_dragging: bool,
    /// 拖动中上一次的系统光标位置(用 GetCursorPos 差值, 与 macOS
    /// locationInWindow 同语义: 含系统指针加速, 画布严格跟手)
    camera_last_cursor: (i32, i32),
    /// 镜头拖动位移合并: 累积位移 + 是否已挂 16ms 计时器
    camera_pending_dx: f64,
    camera_pending_dy: f64,
    camera_pan_timer: bool,
    /// 是否因"镜头修饰键按住"而拦截了鼠标(释放修饰键/退出模式时要还原穿透)
    camera_blocked: bool,
    /// D8b: GIF / 手写消息两个录制状态机(core recording::RecordingMachine)。
    /// 消息循环线程独占(状态机不是 Sync);后台结果经自定义 WM_* 投递回该线程
    /// 再作为结果事件喂回, 旧的 gif_recording/msg_record_start/ink_draft_active
    /// 三个会话标志位已删除。
    gif_machine: glaspen_core::recording::RecordingMachine,
    chat_machine: glaspen_core::recording::RecordingMachine,
    /// GIF 编码完成、等待窗口线程写剪贴板的字节(两步顺序:
    /// GifEncodeSucceeded → Clipboard, 见 D8b backlog ①)。
    pending_gif_bytes: Vec<u8>,
}
