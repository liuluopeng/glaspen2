//! 网格/分栏几何的唯一实现(D6: 细网格 + 屏幕分界线 + 分栏参考线)。
//!
//! 背景: 两端壳曾经各写一份同一套画线公式(`docs/duplication-backlog.md» D6):
//!
//! - macOS: `src/macos/parts/run.m:105-230»(`GlaspenDrawView drawRect» 内联的
//!   细网格/屏幕分界线/分栏参考线三段几何), 状态与设置键在
//!   `src/macos/glaspen2.m:410-417,495-500»(`g_grid_size» / `g_grid_divider» /
//!   `g_show_grid» / `g_grid_follow_strokes»),启动恢复在 `run.m:3069-3082,3128-3136»。
//! - Windows: `crates/glaspen-windows/src/overlay/canvas.rs:319-506»
//!   `OverlayCanvas::draw_grid»(同一套公式 + 像素直写), 状态在
//!   `overlay/state.rs:14-23,183-209»,设置分派在 `overlay/actions.rs:106-163»,
//!   启动恢复在 `overlay/run.rs:69-125»。
//!
//! 本模块只算**几何**(线段表, 视图坐标); 颜色/线宽/alpha/像素取整/抗锯齿等
//! 绘制样式仍留在各平台 adapter(见 [GridLines] 的三层定义与两端样式源码)。
//!
//! 坐标约定: 视图坐标 = 屏幕逻辑点(原点左上, **y 向下**), 与
//! [`crate::camera::Camera»] 的 `view = (canvas − pan) × zoom» 完全一致
//! (Windows 直接消费; macOS 的 CGContext 是 y 向上, adapter 按
//! `cg_y = view_h − view_y» 翻转后描边)。
//!
//! # 公式(逐字来自两端旧实现)
//!
//! ```text
//! 细网格(间距 grid_size)在画布坐标的整数格点: k * grid_size
//!   竖线 k: view_x = (k * grid_size − pan_x) * zoom, y ∈ [0, view_h]
//!   横线 k: view_y = (k * grid_size − pan_y) * zoom, x ∈ [0, view_w]
//!
//! 屏幕分界线(间距 screen_w / screen_h = "每屏一条参考线"):
//!   与细网格同款, 只是间距换成屏幕单位(不吸附, 不取整)。
//!
//! 分栏参考线(独立于"显示网格"开关, 纯视觉):
//!   每屏单位的固定比例位置吸附到**最近的网格线**(lround, 半值远离零):
//!     k = round((i * screen + screen * fraction) / grid_size)
//!     view_x = (k * grid_size − pan_x) * zoom   (竖线; 横线把 x/pan_x 换成 y/pan_y)
//!   比例表(与两端 `if divider == 1/2/4 else» 分支一致):
//!     1 左右两栏 = 竖线 1/2
//!     2 上下两栏 = 横线 1/2
//!     3 九宫格   = 竖线 + 横线 1/3、2/3
//!     4 康奈尔   = 竖线 30%(线索栏右缘) + 横线 75%(总结栏上缘)
//!     其它正值   = 与 3 同款(两端口径都是 `else» 分支; 设置层已钳 0..=4)
//!
//! 扫描范围(旧实现逐字, ±1 是两端的边界余量):
//!   k_min = floor((pan_x + clip_x0 / zoom) / spacing) − 1
//!   k_max = floor((pan_x + clip_x1 / zoom) / spacing) + 1
//!   全视口重绘时 clip = (0, 0, view_w, view_h); macOS 的脏矩形重绘传墨迹脏区,
//!   只生成与之相交的线(旧实现同样的省遍历优化, 见 [GridParams::clip])。
//! ```
//!
//! # 两端真实差异(只显式化, 不静默统一)
//!
//! 1. **屏幕分界线只在 Windows 无限画布出现**: macOS 只要"显示网格"开着就在
//!    两种画布模式都画(`run.m:159-174» 在 `if (g_show_grid)» 内, 不受
//!    `g_infinite_canvas» 门控); Windows 只在无限画布分支画(`canvas.rs:392-424»
//!    的 `if infinite»), 翻页模式不画。→ [GridParams::screen_lines]。
//! 2. **屏幕单位的来源/兜底**: macOS 用 `g_screen_w/g_screen_h», 且 `<= 0» 时
//!    退回 `grid_size * 4»(`run.m:135-136»); Windows 用画布(DIB/虚拟屏幕)尺寸
//!    `w/h», 恒为正。→ [GridParams::screen_w] / [GridParams::screen_h](`<= 0»
//!    或非有限时按 `grid_size * 4» 兜底, 与 macOS 现状同款)。
//! 3. **绘制样式**: macOS 细网格 0.5pt / 分界线 1.0pt / 分栏 1.5pt,
//!    `colorWithWhite:0.5» alpha 0.15 / 0.55 / 0.65, CG 抗锯齿、亚像素坐标;
//!    Windows 细网格/分界线 1px、分栏主列 + 邻列半 alpha(约 2px),
//!    全部 `.round()» 到整像素直写预乘 BGRA(`canvas.rs:331-341,444-484»)。
//!    样式不进 core, 两端各自保留。
//! 4. **非法 zoom 的绘制兜底**: macOS 网格把 `zoom <= 0.05» 兜底成 0.05
//!    (`run.m:113-121»), Windows 用 `camera()»(`> 0» 否则 1.0)。这是 adapter
//!    的兜底, 不属几何: core 只要求有限正 zoom, 不做平台兜底(见 camera.rs
//!    模块文档差异 3)。
//! 5. **脏矩形**: macOS 只算与 `clipRect» 相交的线(笔迹脏区只有十几像素);
//!    Windows 每次全画布重算。→ [GridParams::clip](Windows 传 `None»)。
//!
//! # 异常输入(边界)
//!
//! - 「视口/pan/zoom/grid_size」非有限或非正 → [compute_checked] 回 [GridError],
//!   [compute] 回空表: 不 panic、绝不产出 NaN/Inf。两端设置层已做钳制/兜底
//!   (macOS 网格 zoom 0.05 兜底、Windows `grid_size()` 40 兜底), 正常运行时不可达;
//!   旧实现遇到局部变量被污染时会画出随机线(甚至 C long 转换 UB), 这里选择不画。
//! - 线数超过防御上限(单轴 10 万条, 只可能来自非法 view/zoom 组合)→ 同上,
//!   防止一次绘制扫出百万条线卡死进程。
//!
//! # "网格跟随涂鸦"(grid follows strokes)
//!
//! 两端**都没有几何影响**: 它只是可见性门控 —— 飘渺画布隐藏笔迹时网格是否
//! 一起隐藏(macOS `run.m:108-109» 的 `(g_strokes_visible || !g_grid_follow_strokes)»,
//! Windows `actions.rs:424/602» 的 `if !grid_follow_strokes»)。
//! 因此 core 的几何输入里**没有**该开关; macOS FFI 保留它只为签名对齐与回显
//! (几何相同), 测试断言传 0/1 得到完全相同的线段表。
//!
//! D6 接线: Windows 同进程直接调 [compute](`overlay/canvas.rs»), macOS 经
//! `export::glaspen2_grid_json» / `glaspen2_grid_json_clipped» FFI 拿单行 JSON
//! (与 D1b/D3b/D5b 同形), ObjC 只做 CGContext 描边。

use crate::camera::Camera;

/// 网格尺寸默认值(两端默认 40: macOS `glaspen2.m:411», Windows
/// `overlay/state.rs:192»;面板可调 10..200, 钳制在 `presets» 单源)。
pub const DEFAULT_GRID_SIZE: f64 = 40.0;

/// 屏幕单位兜底倍数: 屏幕尺寸 <= 0 时用 `grid_size * 4»(macOS
/// `run.m:135-136» 的 `gs * 4.0»)。
pub const SCREEN_FALLBACK_MULTIPLIER: f64 = 4.0;

/// 单轴最多生成多少条线(防御性上限: 非法 view/zoom 组合不把进程卡死)。
/// 真实上限远小于此: 6K 屏 + 最小缩放 5% + 最小网格 10 时单轴约 1.2 万条。
const MAX_LINES_PER_AXIS: u64 = 100_000;

/// 轴对齐线段(视图坐标: 原点左上, y 向下; 端点按旧实现画满整屏,
/// 竖线 `x0 == x1»、`y0 = 0, y1 = view_h»;横线 `y0 == y1»、`x0 = 0, x1 = view_w»)。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Segment {
    /// 竖线(画满视口高度)。
    pub const fn vertical(x: f64, view_h: f64) -> Self {
        Self {
            x0: x,
            y0: 0.0,
            x1: x,
            y1: view_h,
        }
    }

    /// 横线(画满视口宽度)。
    pub const fn horizontal(y: f64, view_w: f64) -> Self {
        Self {
            x0: 0.0,
            y0: y,
            x1: view_w,
            y1: y,
        }
    }

    /// 竖线(`x0 == x1»;两端绘制分支用)。
    pub fn is_vertical(self) -> bool {
        self.x0 == self.x1
    }

    /// 横线(`y0 == y1»;两端绘制分支用)。
    pub fn is_horizontal(self) -> bool {
        self.y0 == self.y1
    }
}

/// 网格三层线段(绘制顺序 = 字段顺序: `minor» → `screen» → `divider»,
/// 后画的覆盖先画的;两端旧实现同序)。
///
/// - `minor»: 细网格(间距 `grid_size»)—— 只在 `show_grid» 时生成。
/// - `screen»: 屏幕分界线(间距 = 屏幕单位, 加深加粗层)—— 只在
///   `show_grid && screen_lines» 时生成(Windows 仅无限画布; macOS 两种模式)。
/// - `divider»: 分栏参考线(纯视觉, 独立于"显示网格"开关)—— `divider > 0»
///   时生成, 与 `show_grid» 无关。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GridLines {
    pub minor: Vec<Segment>,
    pub screen: Vec<Segment>,
    pub divider: Vec<Segment>,
}

impl GridLines {
    /// 三层都为空(非法输入 / 两个开关都关)。
    pub fn is_empty(&self) -> bool {
        self.minor.is_empty() && self.screen.is_empty() && self.divider.is_empty()
    }

    /// 三层线段总数(测试/日志用)。
    pub fn len(&self) -> usize {
        self.minor.len() + self.screen.len() + self.divider.len()
    }
}

/// 视口矩形(视图坐标: 原点左上, y 向下), 只用于 [GridParams::clip]。
///
/// macOS adapter 把 AppKit 的非翻转 `clipRect» 换算成 y 向下后传入:
/// `x0 = NSMinX, x1 = NSMaxX, y0 = view_h − NSMaxY, y1 = view_h − NSMinY»。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewRect {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl ViewRect {
    pub const fn new(x0: f64, y0: f64, x1: f64, y1: f64) -> Self {
        Self { x0, y0, x1, y1 }
    }

    /// 端点顺序归一(脏矩形可能带反向坐标)。
    fn normalized(self) -> Self {
        Self {
            x0: self.x0.min(self.x1),
            y0: self.y0.min(self.y1),
            x1: self.x0.max(self.x1),
            y1: self.y0.max(self.y1),
        }
    }

    fn is_finite(self) -> bool {
        self.x0.is_finite() && self.y0.is_finite() && self.x1.is_finite() && self.y1.is_finite()
    }
}

/// 网格几何输入。
///
/// 所有平台差异都显式化在这里(见模块文档"两端真实差异"), 不静默统一:
/// 各 adapter 按本平台现状填值, core 只做同一套公式。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridParams {
    /// 视口宽/高(逻辑点或像素): macOS `[self bounds].size»(非翻转视图,
    /// 窗口 = 屏幕 frame, 与 `g_screen_w/h» 一致); Windows `OverlayCanvas::w/h»
    /// (DIB/虚拟屏幕尺寸)。
    pub view_w: f64,
    pub view_h: f64,
    /// 镜头: `view = (canvas − pan) × zoom»。macOS 无限画布传
    /// `Camera::new(g_pan_x, g_pan_y, (g_zoom > 0.05) ? g_zoom : 0.05)»(zoom
    /// 兜底是绘制差异, 见 camera.rs 模块文档差异 3), 翻页模式传
    /// `Camera::new(-g_page_off_x, -g_page_off_y, 1.0)»; Windows 无限画布传
    /// `camera()», 翻页模式传 `Camera::HOME»。
    pub camera: Camera,
    /// 细网格间距(画布单位; 两端 10..200 钳制, 默认 [DEFAULT_GRID_SIZE])。
    pub grid_size: f64,
    /// 分栏模式: 0=无 1=左右两栏 2=上下两栏 3=九宫格 4=康奈尔;
    /// 其它正值按 3 处理(两端 `else» 分支), `<= 0» 不画。
    pub divider: i32,
    /// "显示网格"开关(细网格 + 屏幕分界线两层); 分栏线不受此开关影响。
    pub show_grid: bool,
    /// 屏幕单位宽/高(屏幕分界线间距 + 分栏吸附单位)。macOS 传
    /// `g_screen_w/g_screen_h», Windows 传视口尺寸; `<= 0» 或非有限时按
    /// `grid_size * 4» 兜底(macOS 旧公式同款)。
    pub screen_w: f64,
    pub screen_h: f64,
    /// 是否生成屏幕分界线: macOS 两种画布模式都 `true»; Windows 仅无限画布
    /// `true»(`infinite_on()»)。
    pub screen_lines: bool,
    /// 只生成与该视口矩形相交的线(`None» = 全视口)。macOS 传脏矩形
    /// (`drawRect» 的 clipRect); Windows 传 `None»(全画布重绘)。
    pub clip: Option<ViewRect>,
}

impl GridParams {
    /// 常用构造: 全视口 + 屏幕单位 = 视口尺寸 + macOS 口味(两种模式都画
    /// 屏幕分界线)。测试与 macOS 全屏重绘入口用; Windows 显式改
    /// `screen_lines» / `clip»。
    pub fn view(view_w: f64, view_h: f64, camera: Camera, grid_size: f64) -> Self {
        Self {
            view_w,
            view_h,
            camera,
            grid_size,
            divider: 0,
            show_grid: true,
            screen_w: view_w,
            screen_h: view_h,
            screen_lines: true,
            clip: None,
        }
    }
}

/// 几何输入非法(两端 adapter 传入前已做设置钳制/兜底;正常运行时不会出现)。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridError {
    /// 视口宽/高不是有限正数。
    ViewSize,
    /// 镜头 pan 不是有限数。
    CameraPan,
    /// 镜头 zoom 不是有限正数。
    CameraZoom,
    /// 网格尺寸不是有限正数。
    GridSize,
    /// 线太多(非法 view/zoom 组合;防御性上限 [MAX_LINES_PER_AXIS])。
    TooManyLines,
}

impl GridError {
    pub fn message(self) -> &'static str {
        match self {
            GridError::ViewSize => "网格视图尺寸非法(需要有限正数)",
            GridError::CameraPan => "网格镜头 pan 非法(需要有限数)",
            GridError::CameraZoom => "网格镜头 zoom 非法(需要有限正数)",
            GridError::GridSize => "网格尺寸非法(需要有限正数)",
            GridError::TooManyLines => "网格线过多(视图/缩放组合非法)",
        }
    }
}

impl std::fmt::Display for GridError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for GridError {}

/// 计算网格三层线段;非法输入返回 [GridError], 调用方(FFI)据此回 error JSON。
///
/// 永不 panic、永不产出 NaN/Inf: 视口/镜头/网格尺寸先校验, 单条线坐标在
/// 极端输入下若溢出也直接跳过(见 [push_segment])。
pub fn compute_checked(params: &GridParams) -> Result<GridLines, GridError> {
    // ── 输入校验(顺序: 视口 → 镜头 → 网格尺寸) ──
    if !(params.view_w.is_finite() && params.view_w > 0.0) {
        return Err(GridError::ViewSize);
    }
    if !(params.view_h.is_finite() && params.view_h > 0.0) {
        return Err(GridError::ViewSize);
    }
    if !(params.camera.pan_x.is_finite() && params.camera.pan_y.is_finite()) {
        return Err(GridError::CameraPan);
    }
    if !(params.camera.zoom.is_finite() && params.camera.zoom > 0.0) {
        return Err(GridError::CameraZoom);
    }
    if !(params.grid_size.is_finite() && params.grid_size > 0.0) {
        return Err(GridError::GridSize);
    }

    let zoom = params.camera.zoom;
    let gs = params.grid_size;
    let pan_x = params.camera.pan_x;
    let pan_y = params.camera.pan_y;
    let (view_w, view_h) = (params.view_w, params.view_h);

    // 扫描范围: 默认整个视口; 非有限 clip 直接忽略(脏矩形只会来自 AppKit)。
    let (cx0, cx1, cy0, cy1) = match params.clip {
        Some(clip) if clip.is_finite() => {
            let clip = clip.normalized();
            (clip.x0, clip.x1, clip.y0, clip.y1)
        }
        _ => (0.0, view_w, 0.0, view_h),
    };

    // 屏幕单位 <= 0 / 非有限 → grid_size * 4(macOS run.m:135-136 同款)。
    let screen_w = effective_screen(params.screen_w, gs);
    let screen_h = effective_screen(params.screen_h, gs);

    // 屏幕单位索引: 屏幕分界线与分栏吸附共用(两端旧公式同款)。
    let (jx0, jx1) = line_range(pan_x + cx0 / zoom, pan_x + cx1 / zoom, screen_w)?;
    let (jy0, jy1) = line_range(pan_y + cy0 / zoom, pan_y + cy1 / zoom, screen_h)?;

    let mut out = GridLines::default();

    // ── 细网格 + 屏幕分界线(`if (show_grid)»: 细网格在主循环, 分界线固定写 0) ──
    if params.show_grid {
        let (kx0, kx1) = line_range(pan_x + cx0 / zoom, pan_x + cx1 / zoom, gs)?;
        for k in kx0..=kx1 {
            push_segment(
                &mut out.minor,
                Segment::vertical((k as f64 * gs - pan_x) * zoom, view_h),
            );
        }
        let (ky0, ky1) = line_range(pan_y + cy0 / zoom, pan_y + cy1 / zoom, gs)?;
        for k in ky0..=ky1 {
            push_segment(
                &mut out.minor,
                Segment::horizontal((k as f64 * gs - pan_y) * zoom, view_w),
            );
        }

        if params.screen_lines {
            for k in jx0..=jx1 {
                push_segment(
                    &mut out.screen,
                    Segment::vertical((k as f64 * screen_w - pan_x) * zoom, view_h),
                );
            }
            for k in jy0..=jy1 {
                push_segment(
                    &mut out.screen,
                    Segment::horizontal((k as f64 * screen_h - pan_y) * zoom, view_w),
                );
            }
        }
    }

    // ── 分栏参考线(独立于"显示网格": 两种画布模式都画) ──
    if params.divider > 0 {
        let (fx, fy) = divider_fractions(params.divider);
        for &fraction in fx {
            for i in jx0..=jx1 {
                let k = ((i as f64 * screen_w + screen_w * fraction) / gs).round();
                if !k.is_finite() {
                    continue;
                }
                push_segment(
                    &mut out.divider,
                    Segment::vertical(((k as i64) as f64 * gs - pan_x) * zoom, view_h),
                );
            }
        }
        for &fraction in fy {
            for i in jy0..=jy1 {
                let k = ((i as f64 * screen_h + screen_h * fraction) / gs).round();
                if !k.is_finite() {
                    continue;
                }
                push_segment(
                    &mut out.divider,
                    Segment::horizontal(((k as i64) as f64 * gs - pan_y) * zoom, view_w),
                );
            }
        }
    }

    Ok(out)
}

/// 便捷入口: 非法输入返回空表(Windows adapter 用;两端设置层已保证合法值)。
/// macOS 走 FFI, 用 [compute_checked] 换 error JSON。
pub fn compute(params: &GridParams) -> GridLines {
    compute_checked(params).unwrap_or_default()
}

/// 屏幕单位兜底: 有限正数原样, 否则 `grid_size * 4»(macOS `run.m:135-136»)。
fn effective_screen(screen: f64, grid_size: f64) -> f64 {
    if screen.is_finite() && screen > 0.0 {
        screen
    } else {
        grid_size * SCREEN_FALLBACK_MULTIPLIER
    }
}

/// 一轴的线索引范围(两端旧公式逐字):
/// `floor(max_canvas / spacing) + 1» 到 `floor(min_canvas / spacing) − 1»。
///
/// `min_canvas» / `max_canvas» 是已折算到画布坐标的视口(或脏区)边界。
/// 溢出/超上限时回 [GridError::TooManyLines](防非法输入把绘制线程卡死)。
fn line_range(min_canvas: f64, max_canvas: f64, spacing: f64) -> Result<(i64, i64), GridError> {
    let first = (min_canvas / spacing).floor();
    let last = (max_canvas / spacing).floor();
    if !first.is_finite() || !last.is_finite() {
        return Err(GridError::TooManyLines);
    }
    let first = first as i64;
    let last = last as i64;
    let span = last.saturating_sub(first).saturating_add(3); // 含两侧 ±1 余量
    if span < 0 || span as u64 > MAX_LINES_PER_AXIS {
        return Err(GridError::TooManyLines);
    }
    Ok((first.saturating_sub(1), last.saturating_add(1)))
}

/// 分栏比例表(与两端 `divider == 1 / == 2 / == 4 / else» 分支一致)。
fn divider_fractions(divider: i32) -> (&'static [f64], &'static [f64]) {
    const NONE: &[f64] = &[];
    const HALF: &[f64] = &[0.5];
    const THIRDS: &[f64] = &[1.0 / 3.0, 2.0 / 3.0];
    // 康奈尔笔记法: 竖线 30% = 线索栏右缘, 横线 75% = 总结栏上缘。
    const CORNELL_X: &[f64] = &[0.3];
    const CORNELL_Y: &[f64] = &[0.75];
    match divider {
        1 => (HALF, NONE),
        2 => (NONE, HALF),
        4 => (CORNELL_X, CORNELL_Y),
        _ => (THIRDS, THIRDS),
    }
}

/// 只接受有限坐标的线段(NaN/Inf 一律丢弃: 绘制层绝不写 NaN)。
fn push_segment(out: &mut Vec<Segment>, seg: Segment) {
    if seg.x0.is_finite() && seg.y0.is_finite() && seg.x1.is_finite() && seg.y1.is_finite() {
        out.push(seg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(
        view_w: f64,
        view_h: f64,
        camera: Camera,
        grid_size: f64,
        divider: i32,
        show_grid: bool,
        screen_lines: bool,
        clip: Option<ViewRect>,
    ) -> GridParams {
        GridParams {
            view_w,
            view_h,
            camera,
            grid_size,
            divider,
            show_grid,
            screen_w: view_w,
            screen_h: view_h,
            screen_lines,
            clip,
        }
    }

    /// 线段坐标完整比较(两端公式是同一串 double 运算, 这里用精确相等;
    /// 对 round/floor 产生的整数坐标恒成立)。
    /// 只取竖线的 x(横线的 x0 恒为 0, 不参与坐标断言)。
    fn xs(segments: &[Segment]) -> Vec<f64> {
        segments
            .iter()
            .filter(|s| s.is_vertical())
            .map(|s| s.x0)
            .collect()
    }

    /// 只取横线的 y(竖线的 y0 恒为 0)。
    fn ys(segments: &[Segment]) -> Vec<f64> {
        segments
            .iter()
            .filter(|s| s.is_horizontal())
            .map(|s| s.y0)
            .collect()
    }

    fn assert_axis_aligned(lines: &GridLines, view_w: f64, view_h: f64) {
        for seg in lines
            .minor
            .iter()
            .chain(&lines.screen)
            .chain(&lines.divider)
        {
            assert!(
                seg.x0.is_finite()
                    && seg.y0.is_finite()
                    && seg.x1.is_finite()
                    && seg.y1.is_finite(),
                "线段含非有限坐标: {seg:?}"
            );
            if seg.is_vertical() {
                assert_eq!((seg.y0, seg.y1), (0.0, view_h), "竖线必须画满视口高");
            } else {
                assert!(seg.is_horizontal(), "线段必须轴对齐: {seg:?}");
                assert_eq!((seg.x0, seg.x1), (0.0, view_w), "横线必须画满视口宽");
            }
        }
    }

    #[test]
    fn home_viewport_minor_grid_positions() {
        // 已知向量: 800x600、回原点、间距 40。
        // 旧公式(run.m:128-156 / canvas.rs:359-390): 索引 floor(canvas/spacing) ±1,
        // 线坐标 (k * spacing − pan) * zoom。
        let p = params(800.0, 600.0, Camera::HOME, 40.0, 0, true, false, None);
        let lines = compute_checked(&p).expect("合法输入");
        assert_eq!(lines.minor.len(), 23 + 18);
        assert_axis_aligned(&lines, 800.0, 600.0);

        let vx: Vec<f64> = lines
            .minor
            .iter()
            .filter(|s| s.is_vertical())
            .map(|s| s.x0)
            .collect();
        assert_eq!(vx.len(), 23);
        assert_eq!(vx[0], -40.0);
        assert_eq!(vx[1], 0.0);
        assert_eq!(vx[23 - 1], 840.0);
        for pair in vx.windows(2) {
            assert_eq!(pair[1] - pair[0], 40.0, "间距必须等于 grid_size");
        }

        let hy: Vec<f64> = lines
            .minor
            .iter()
            .filter(|s| s.is_horizontal())
            .map(|s| s.y0)
            .collect();
        assert_eq!(hy.len(), 18);
        assert_eq!(hy[0], -40.0);
        assert_eq!(hy[18 - 1], 640.0);

        // 屏幕分界线单独生成(此用例 screen_lines=false 关掉)。
        assert!(lines.screen.is_empty());
        assert!(lines.divider.is_empty());
        assert!(!lines.is_empty());
        assert_eq!(lines.len(), lines.minor.len());
    }

    #[test]
    fn screen_lines_use_screen_unit_spacing() {
        // macOS 两种模式都画: 屏幕单位 = 视口尺寸, 每屏一条参考线。
        let p = params(800.0, 600.0, Camera::HOME, 40.0, 0, true, true, None);
        let lines = compute_checked(&p).expect("合法输入");
        assert_eq!(xs(&lines.screen), [-800.0, 0.0, 800.0, 1600.0]);
        assert_eq!(ys(&lines.screen), [-600.0, 0.0, 600.0, 1200.0]);
        assert_axis_aligned(&lines, 800.0, 600.0);

        // Windows 翻页模式不画屏幕分界线(差异 1 的显式参数)。
        let p = params(800.0, 600.0, Camera::HOME, 40.0, 0, true, false, None);
        let lines = compute_checked(&p).expect("合法输入");
        assert!(lines.screen.is_empty());
        assert_eq!(lines.minor.len(), 41); // 细网格不受影响
    }

    #[test]
    fn screen_unit_falls_back_to_grid_size_times_four() {
        // macOS run.m:135-136: screen_w <= 0 → gs * 4。
        let mut p = params(800.0, 600.0, Camera::HOME, 40.0, 0, true, true, None);
        p.screen_w = 0.0;
        p.screen_h = -1.0;
        let lines = compute_checked(&p).expect("合法输入");
        // 屏幕单位 160: 竖线 -160..960 步长 160(8 条), 横线 -160..640(6 条)。
        assert_eq!(
            xs(&lines.screen),
            [-160.0, 0.0, 160.0, 320.0, 480.0, 640.0, 800.0, 960.0]
        );
        assert_eq!(ys(&lines.screen), [-160.0, 0.0, 160.0, 320.0, 480.0, 640.0]);
        // NaN 同样走兜底(旧表达式 NaN > 0 为假)。
        p.screen_w = f64::NAN;
        p.screen_h = f64::NAN;
        assert_eq!(compute_checked(&p).expect("合法输入").screen.len(), 8 + 6);
        assert_eq!(SCREEN_FALLBACK_MULTIPLIER, 4.0);
    }

    #[test]
    fn divider_modes_positions() {
        // 分栏线独立于"显示网格": 这里 show_grid=false, 只剩分栏层。
        // 吸附公式: k = round((i * screen + screen * f) / gs), 半值远离零。
        let base =
            |divider: i32| params(800.0, 600.0, Camera::HOME, 40.0, divider, false, true, None);

        // 0 = 无。
        assert!(compute_checked(&base(0)).unwrap().divider.is_empty());

        // 1 = 左右两栏: 竖线 1/2 处吸附到网格线。
        let lines = compute_checked(&base(1)).unwrap();
        assert_eq!(xs(&lines.divider), [-400.0, 400.0, 1200.0, 2000.0]);
        assert!(lines.divider.iter().all(|s| s.is_vertical()));
        assert!(lines.minor.is_empty() && lines.screen.is_empty());
        assert_axis_aligned(&lines, 800.0, 600.0);

        // 2 = 上下两栏。
        let lines = compute_checked(&base(2)).unwrap();
        assert_eq!(ys(&lines.divider), [-320.0, 320.0, 920.0, 1520.0]);
        assert!(lines.divider.iter().all(|s| s.is_horizontal()));

        // 3 = 九宫格: 竖线 1/3、2/3 与横线 1/3、2/3。
        let lines = compute_checked(&base(3)).unwrap();
        // 顺序 = 两端旧实现: 比例外循环(先 1/3 的全部屏单位, 再 2/3)。
        let vx: Vec<f64> = xs(&lines.divider);
        assert_eq!(
            vx,
            [-520.0, 280.0, 1080.0, 1880.0, -280.0, 520.0, 1320.0, 2120.0]
        );
        let hy: Vec<f64> = ys(&lines.divider);
        assert_eq!(
            hy,
            [-400.0, 200.0, 800.0, 1400.0, -200.0, 400.0, 1000.0, 1600.0]
        );

        // 4 = 康奈尔: 竖线 30%, 横线 75%。
        let lines = compute_checked(&base(4)).unwrap();
        assert_eq!(xs(&lines.divider), [-560.0, 240.0, 1040.0, 1840.0]);
        assert_eq!(ys(&lines.divider), [-160.0, 440.0, 1040.0, 1640.0]);

        // 其它正值 = 两端 `else» 分支(九宫格);设置层已钳 0..=4, 这里只记录口径。
        assert_eq!(
            compute_checked(&base(5)).unwrap().divider,
            compute_checked(&base(3)).unwrap().divider
        );
    }

    #[test]
    fn pan_and_zoom_transform_lines() {
        // camera (100,50) zoom 0.5: 线坐标 = (k*40 − 100)*0.5 = 20k − 50,
        // 间距 40*0.5 = 20;索引范围随 pan/zoom 移动。
        let p = params(
            800.0,
            600.0,
            Camera::new(100.0, 50.0, 0.5),
            40.0,
            0,
            true,
            false,
            None,
        );
        let lines = compute_checked(&p).expect("合法输入");
        let vx: Vec<f64> = xs(&lines.minor);
        assert_eq!(vx.len(), 43); // k = 1..=43
        assert_eq!(vx[0], -30.0);
        assert_eq!(vx[43 - 1], 810.0);
        for pair in vx.windows(2) {
            assert_eq!(pair[1] - pair[0], 20.0, "缩放后间距 = grid_size * zoom");
        }
        let hy: Vec<f64> = ys(&lines.minor);
        assert_eq!(hy.len(), 33); // k = 0..=32
        assert_eq!(hy[0], -25.0);
        assert_eq!(hy[33 - 1], 615.0);

        // 屏幕分界线同样跟随 pan/zoom: (k*800 − 100)*0.5。
        let p = params(
            800.0,
            600.0,
            Camera::new(100.0, 50.0, 0.5),
            40.0,
            0,
            true,
            true,
            None,
        );
        let lines = compute_checked(&p).expect("合法输入");
        assert_eq!(xs(&lines.screen), [-450.0, -50.0, 350.0, 750.0, 1150.0]);
        assert_eq!(ys(&lines.screen), [-325.0, -25.0, 275.0, 575.0, 875.0]);
    }

    #[test]
    fn clip_limits_scanned_lines() {
        // 脏矩形 (100,100)-(200,200): 只剩与它相交的网格线(±1 余量, 两端旧公式)。
        let p = params(
            800.0,
            600.0,
            Camera::HOME,
            40.0,
            0,
            true,
            false,
            Some(ViewRect::new(100.0, 100.0, 200.0, 200.0)),
        );
        let lines = compute_checked(&p).expect("合法输入");
        let vx = xs(&lines.minor);
        assert_eq!(vx, [40.0, 80.0, 120.0, 160.0, 200.0, 240.0]);
        let hy = ys(&lines.minor);
        assert_eq!(hy, [40.0, 80.0, 120.0, 160.0, 200.0, 240.0]);

        // 反向端点会先归一(脏矩形不保证顺序)。
        let mut swapped = p;
        swapped.clip = Some(ViewRect::new(200.0, 200.0, 100.0, 100.0));
        assert_eq!(
            compute_checked(&swapped).unwrap(),
            compute_checked(&p).unwrap()
        );

        // 非有限 clip 忽略(按全视口算), 不产生 NaN。
        swapped.clip = Some(ViewRect::new(f64::NAN, 0.0, 1.0, 1.0));
        let full = compute_checked(&swapped).unwrap();
        assert_eq!(full.minor.len(), 41);
        assert_axis_aligned(&full, 800.0, 600.0);
    }

    #[test]
    fn page_mode_matches_windows_home_camera() {
        // Windows 翻页模式 = Camera::HOME + 不画屏幕分界线;细网格/分栏与
        // 回原点无限画布同款(macOS 翻页模式 pan=-page_off=0、zoom=1, 只是多
        // 画屏幕分界线 —— 差异 1)。
        let page = compute_checked(&params(
            800.0,
            600.0,
            Camera::HOME,
            40.0,
            3,
            true,
            false,
            None,
        ))
        .unwrap();
        let mac_page = compute_checked(&params(
            800.0,
            600.0,
            Camera::HOME,
            40.0,
            3,
            true,
            true,
            None,
        ))
        .unwrap();
        assert_eq!(page.minor, mac_page.minor);
        assert_eq!(page.divider, mac_page.divider);
        assert!(page.screen.is_empty());
        assert!(!mac_page.screen.is_empty());
    }

    #[test]
    fn invalid_inputs_report_errors_never_nan() {
        let good = GridParams::view(800.0, 600.0, Camera::HOME, 40.0);

        // 视口非有限/非正。
        for (w, h) in [(0.0, 600.0), (-1.0, 600.0), (800.0, 0.0), (f64::NAN, 600.0)] {
            let mut p = good;
            p.view_w = w;
            p.view_h = h;
            assert_eq!(compute_checked(&p), Err(GridError::ViewSize));
            assert!(compute(&p).is_empty(), "非法输入不产出线段");
        }

        // 镜头 pan 非有限。
        for (pan_x, pan_y) in [(f64::NAN, 0.0), (0.0, f64::INFINITY)] {
            let mut p = good;
            p.camera = Camera::new(pan_x, pan_y, 1.0);
            assert_eq!(compute_checked(&p), Err(GridError::CameraPan));
            assert!(compute(&p).is_empty());
        }

        // zoom <= 0 / 非有限。macOS 网格绘制的 0.05 兜底在 adapter(不在此处)。
        for zoom in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let mut p = good;
            p.camera = Camera::new(0.0, 0.0, zoom);
            assert_eq!(compute_checked(&p), Err(GridError::CameraZoom));
            assert!(compute(&p).is_empty());
        }

        // 网格尺寸 <= 0 / 非有限。两端设置层钳制 10..200, 此分支不可达。
        for grid_size in [0.0, -5.0, f64::NAN, f64::INFINITY] {
            let mut p = good;
            p.grid_size = grid_size;
            assert_eq!(compute_checked(&p), Err(GridError::GridSize));
            assert!(compute(&p).is_empty());
        }

        // 极端组合(1e-9 zoom)触发防御上限而不是卡死/爆内存。
        let mut p = good;
        p.camera = Camera::new(0.0, 0.0, 1e-9);
        assert_eq!(compute_checked(&p), Err(GridError::TooManyLines));
        assert!(compute(&p).is_empty());

        // 错误文案非空(FFI 直接下发给 ObjC 日志)。
        for err in [
            GridError::ViewSize,
            GridError::CameraPan,
            GridError::CameraZoom,
            GridError::GridSize,
            GridError::TooManyLines,
        ] {
            assert!(!err.message().is_empty());
            assert!(!err.to_string().is_empty());
        }
    }

    #[test]
    fn extreme_but_accepted_inputs_stay_finite() {
        // 巨 pan + 巨大网格尺寸: 合法(有限正数)但坐标极大;保证要么出线
        // 要么空表, 绝无 NaN/Inf。
        let mut p = GridParams::view(800.0, 600.0, Camera::new(1e17, -1e17, 1.0), 40.0);
        let lines = compute_checked(&p).expect("有限输入");
        assert_axis_aligned(&lines, 800.0, 600.0);

        // 微小但合法的 zoom(不是 1e-9 那种扫出上百万条的程度)。
        p.camera = Camera::new(0.0, 0.0, 0.02);
        p.grid_size = 200.0;
        let lines = compute_checked(&p).expect("有限输入");
        assert_axis_aligned(&lines, 800.0, 600.0);
        assert!(!lines.minor.is_empty());

        assert_eq!(DEFAULT_GRID_SIZE, 40.0);
    }

    // ── D6: 两端 adapter 网格几何残留闸门 ──
    //
    // core::grid 是网格/分栏几何(间距/屏幕单位/分栏吸附/pan+zoom 变换/脏矩形
    // 裁剪)的唯一来源: macOS 经 export::glaspen2_grid_json(*_clipped) FFI,
    // Windows 同进程直接 grid::compute。本闸门按文本扫描两端 adapter, 两类断言:
    //
    // 1) **必须**出现 core 网格入口 token(接线被退回/删调用即红);
    // 2) 旧行内几何的"指纹"**不得**出现(有人把公式抄回 adapter 即红)。
    //
    // 扫描范围 = 真正画网格/分栏/声明 FFI 的文件:
    //   macOS  glaspen2.m(FFI 声明) + parts/{run,menu,panel}.m;
    //   Windows overlay/{canvas,actions,state}.rs(canvas = 绘制, actions = 开关
    //   分派, state = 网格尺寸/设置状态)。
    // 有意排除(不是 D6 的重复项):
    //   - 网格设置键/落库/启动恢复(macOS run.m:3069-3135 / Windows state.rs,
    //     设置契约归 D1/E3), 这里只要求不出现几何公式;
    //   - "网格跟随涂鸦"可见性门控(两端 adapter 行为, 与几何无关);
    //   - 绘制样式(颜色/线宽/alpha/像素取整), 按设计留在 adapter;
    //   - minimap 条带布局的 MM_GAP(actions.rs, D7)。
    #[test]
    fn adapter_grid_geometry_uses_core() {
        // 行注释剥掉再做匹配(注释里允许出现旧公式的说明; ObjC/Rust 的 // 都适用)。
        fn code_lines(src: &str) -> String {
            src.lines()
                .map(|line| match line.find("//") {
                    Some(at) => &line[..at],
                    None => line,
                })
                .collect::<Vec<_>>()
                .join("\n")
        }

        fn violations(name: &str, src: &str, forbidden: &[&str]) -> Vec<String> {
            let code = code_lines(src);
            forbidden
                .iter()
                .filter_map(|pat| {
                    code.find(pat).map(|at| {
                        format!(
                            "{name}:{} 仍含行内网格几何 {:?}",
                            code[..at].matches('\n').count() + 1,
                            pat
                        )
                    })
                })
                .collect()
        }

        fn must_contain(name: &str, src: &str, tokens: &[&str]) -> Vec<String> {
            tokens
                .iter()
                .filter(|token| !src.contains(**token))
                .map(|token| format!("{name} 缺少 core 网格入口 {token:?}"))
                .collect()
        }

        let mac_main = include_str!("../../../src/macos/glaspen2.m");
        let mac_run = include_str!("../../../src/macos/swift/run_glue.swift");
        let mac_menu = include_str!("../../../src/macos/swift/menu_runtime.swift");
        let mac_panel = format!("{}{}",
            include_str!("../../../src/macos/swift/panel_glue.swift"),
            include_str!("../../../src/macos/swift/raw_drawing.swift"));
        // S7m: 网格层绘制已迁 Swift, 闸门须同时扫 Swift 源(几何仍走 core::grid)。
        let mac_grid_swift = include_str!("../../../src/macos/swift/overlay_grid.swift");
        let mac_drawing_swift = include_str!("../../../src/macos/swift/overlay_drawing.swift");
let mac_run_grid = format!("{mac_run}\n{mac_grid_swift}\n{mac_drawing_swift}");
        let win_canvas = include_str!("../../../crates/glaspen-windows/src/overlay/canvas.rs");
        let win_actions = include_str!("../../../crates/glaspen-windows/src/overlay/actions.rs");
        let win_state = include_str!("../../../crates/glaspen-windows/src/overlay/state.rs");
        // include_str! 指错文件/文件被清空时闸门不能静默变绿。
        for (name, src) in [
            ("glaspen2.m", mac_main),
            ("run.m", mac_run),
            ("menu.m", mac_menu),
            ("panel_glue.swift", mac_panel.as_str()),
            ("canvas.rs", win_canvas),
            ("actions.rs", win_actions),
            ("state.rs", win_state),
        ] {
            assert!(
                src.len() > 500,
                "闸门扫到的 {name} 过短, include_str! 路径可疑"
            );
        }

        // ── macOS 旧行内几何指纹(旧 run.m 内联的三段 drawRect 循环) ──
        let mac_forbidden = [
            // 索引范围: floor((pan + clip/z) / spacing) ± 1
            "floor((pan_x +",
            "floor((pan_y +",
            "double bw = (g_screen_w > 0)",
            "double bh = (g_screen_h > 0)",
            "long kx0",
            "long kx1",
            "long ky0",
            "long ky1",
            "long jx0",
            "long jx1",
            "long jy0",
            "long jy1",
            "for (long k = kx0",
            "for (long k = ky0",
            "for (long i = jx0",
            "for (long i = jy0",
            // 线坐标变换: (k * spacing − pan) * zoom
            "(k * gs - pan_x)",
            "(k * gs - pan_y)",
            "(k * bw - pan_x)",
            "(k * bh - pan_y)",
            "(k * bw + bw *",
            "(k * bh + bh *",
            // 分栏吸附 + 比例表
            "lround((i * bw",
            "lround((i * bh",
            "kHalf[",
            "kThirds[",
            "kCornellX",
            "kCornellY",
            // 屏幕单位兜底 / 绘制循环坐标
            "gs * 4.0",
            "CGFloat gx = (",
            "CGFloat gy = bounds.size.height - (",
            // 旧「网格尺寸 * 常数」公式写法
            "g_grid_size * ",
        ];

        // ── Windows 旧行内几何指纹(旧 canvas.rs draw_grid) ──
        let win_canvas_forbidden = [
            // 可见画布范围 + 索引循环
            "let cx0 = pan_x",
            "let cx1 =",
            "let cy0 = pan_y",
            "let cy1 =",
            "let GAP: f64",
            "GAP",
            "(cx0 / GAP)",
            "(cy0 / GAP)",
            "let kx0",
            "let ky0",
            "while k <= kx1",
            // 线坐标变换: (k * GAP − pan) * zoom
            "(k as f64 * GAP - pan_x)",
            "(k as f64 * GAP - pan_y)",
            // 分栏比例表 + 吸附
            "let halves",
            "let thirds",
            "cornell_x",
            "cornell_y",
            "i as f64 * bw + bw * frac",
            "i as f64 * bh + bh * frac",
            "(i as f64 * bw",
            // 旧像素写列/行闭包(几何+样式耦合; 样式现由 grid_blit_* 承担)
            "draw_col(",
            "draw_row(",
        ];
        // actions.rs / state.rs 只做开关分派与设置状态(D1/E3 已单源), 不该出现
        // 画布范围/索引/吸附这些几何量; minimap 的 MM_GAP 是 D7, 不在此列。
        let win_other_forbidden = ["cx0", "cy0", "(k as f64 * GAP", "draw_col(", "draw_row("];

        let mut problems = Vec::new();
        problems.extend(violations("glaspen2.m", mac_main, &mac_forbidden));
        problems.extend(violations("run.m", mac_run, &mac_forbidden));
        problems.extend(violations("menu.m", mac_menu, &mac_forbidden));
        problems.extend(violations("panel_glue.swift", &mac_panel, &mac_forbidden));
        problems.extend(violations("canvas.rs", win_canvas, &win_canvas_forbidden));
        problems.extend(violations("actions.rs", win_actions, &win_other_forbidden));
        problems.extend(violations("state.rs", win_state, &win_other_forbidden));

        // ── 必须接 core 网格入口(mac 经 FFI / Windows 直接调 grid::compute) ──
        for (name, src, tokens) in [
            (
                "glaspen2.m",
                mac_main,
                &[
                    "extern char* glaspen2_grid_json(",
                    "glaspen2_grid_json_clipped",
                ][..],
            ),
            (
                "run.m+overlay_grid.swift",
                mac_run_grid.as_str(),
                &[
                    "glaspen2_grid_json(",
                    "glaspen2_grid_json_clipped(",
                    "OverlayGridRenderer.drawGrid(",
                ][..],
            ),
            (
                "canvas.rs",
                win_canvas,
                &[
                    "glaspen_core::grid::compute(",
                    "GridParams",
                    "grid_blit_layer(",
                    "grid_blit_divider(",
                ][..],
            ),
            ("actions.rs", win_actions, &["draw_grid("][..]),
            (
                "state.rs",
                win_state,
                &["fn grid_size() -> f64", "pub grid_divider: i32"][..],
            ),
        ] {
            problems.extend(must_contain(name, src, tokens));
        }

        assert!(
            problems.is_empty(),
            "D6 网格几何接线闸门失败(两端必须只用 core::grid 几何):\n{}",
            problems.join("\n")
        );
    }
}
