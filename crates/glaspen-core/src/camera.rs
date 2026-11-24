//! 无限画布镜头数学的唯一实现(D5a: pan/zoom/锚点/限幅/居中/持久化格式)。
//!
//! 背景: 镜头**状态**仍由两端壳各自持有(macOS `g_pan_x/g_pan_y/g_zoom`,
//! `src/macos/glaspen2.m:416-418`;Windows `CAM_PAN_X/CAM_PAN_Y/CAM_ZOOM` 原子量,
//! `crates/glaspen-windows/src/overlay/state.rs:53-78`), 但同一套**数学**曾各写
//! 一份(`docs/duplication-backlog.md` D5):
//!
//! - macOS: `src/macos/parts/run.m:461-468` `canvas_pan_by`、`:507-535`
//!   `canvas_zoom_at`、`:1450-1464` 持久化/载入;视图↔画布换算在
//!   `src/macos/glaspen2.m:427-440`,`run.m:109-121`(网格绘制)消费变换;
//!   居中内容在 `src/macos/parts/menu.m:1254-1261`。
//! - Windows: `crates/glaspen-windows/src/overlay/actions.rs:639`
//!   `persist_camera`、`:659-668` `canvas_pan_by`、`:672-713`
//!   `canvas_zoom_at`; `overlay/state.rs:63-102` `cam/set_cam` +
//!   `canvas_from_view/view_from_canvas`; `overlay/hotkeys.rs:814-824`
//!   `pan_step/zoom_step`; `overlay/mouse.rs:166-201` 鼠标抓手;
//!   `overlay/actions.rs:283-304` 回原点/居中内容。
//!
//! 坐标约定(两端一致, 也是本模块全部公式的前提):
//!
//! ```text
//! view   = (canvas − pan) × zoom   // 画布 → 视口(屏幕逻辑点), 两壳绘制/命中同款
//! canvas = view / zoom + pan       // 视口 → 画布, 缩放锚点/笔输入同款
//! ```
//!
//! `zoom` 1.0 = 100%(上限), 下限 0.05 = 5%;翻页模式下镜头恒为原点 + 100%
//! (macOS `canvas_reset_lens`, `run.m:1466-1474` / Windows
//! `apply_infinite_canvas(false)`, `actions.rs:744-752`)。
//!
//! 边界: 本切片(D5a)只提供**纯数学**, 不接线两端 —— 镜头状态、重绘/持久化
//! 节流、提示文案与节流、模式门控(仅无限画布 / 书写中忽略)都留在 adapter。
//! 两端**真实差异**只记录、不静默统一:
//!
//! 1. **键盘平移方向符号相反**: macOS 按"视野移动"(`Left` → `dx = +step`,
//!    `run.m:1786-1800`), Windows 按"内容跟随按键"(`Left` → `dx = -step`,
//!    `hotkeys.rs:788-795`, 2026-11 依用户反馈取反)。[Camera::pan_by] 与两壳
//!    的 `canvas_pan_by` 同款(`pan -= d`), 方向符号仍留在 adapter(见
//!    `crate::action_effect::CameraPan` 的文档)。
//! 2. **最小缩放提示只有 Windows 有**: 到 100% 上限两端都会提示("已达最大
//!    缩放 100%"), 但 5% 下限提示("已达最小缩放 5%")只有 Windows
//!    (`actions.rs:691-706`);macOS 到下限静默(`run.m:513-527` 只有 max 分支)。
//!    触发条件也略有差别: macOS 只要本次请求 `nz > 1.0` 就提示(即使镜头没变),
//!    Windows 在"钳制后没变化"时提示。[ZoomOutcome] 同时给出
//!    `hit_max/hit_min/changed`, 让两端各自保留自己的提示规则。
//! 3. **非法 zoom 的兜底不统一**: macOS 网格绘制把 `zoom <= 0.05` 兜底成
//!    0.05(`run.m:113-121`);笔迹绘制走 core `view_transform()`, 与 Windows
//!    `cam()` 一样是 `zoom > 0 ? zoom : 1.0`(`export/mod.rs:255-265` /
//!    `state.rs:63-71`);而键盘步长/鼠标拖动/居中公式两端都用 "> 0.05 否则
//!    1.0"。[Camera::step_zoom] 采用后者(两端步长公式同款), 绘制兜底留在
//!    adapter, 不在本层统一。
//! 4. **持久化载入不钳上限**: 两端载入都只做 `zoom > 0 ? zoom : 1.0`
//!    (`run.m:1462` / `actions.rs:743`), 不把历史脏值(如 2.5)钳回 1.0;
//!    [Camera::from_db_values] 照做, [Camera::clamp] 由调用方显式调。
//! 5. **居中时 zoom 回写不同**: Windows `CMD_CANVAS_CENTER` 把兜底后的 zoom
//!    一起 `set_cam` 写回(`actions.rs:294-300`), macOS 只改 pan 不动 `g_zoom`
//!    (`menu.m:1254-1261`)。[Camera::centered_on] 只改 pan 并返回本次用的
//!    zoom, 写回与否留给 adapter。
//! 6. **滚轮倍率/锚点来源属 adapter**: macOS 触控板 `exp(-dy*0.0015)`、行滚
//!    1.1(`run.m:2126-2132`), Windows 行滚恒 1.1(`mouse.rs:211-219`);键盘
//!    缩放锚点两端都是视口中心([Camera::zoom_at_center]), 滚轮锚点是鼠标位置。
//!
//! D5b: 两端已接线 —— Windows 同进程直接调 [Camera](`overlay/state.rs::camera()`),
//! macOS 经 `export::glaspen2_camera_*` FFI 拿 JSON(与 D1b/D3b 同形),
//! 镜头状态/门控/提示节流/持久化节流仍留在 adapter。

/// 镜头缩放下限 5%(两壳 `ZOOM_MIN = 0.05`: macOS `run.m:525-526`,
/// Windows `state.rs:57`)。
pub const MIN_ZOOM: f64 = 0.05;

/// 镜头缩放上限 100%(两壳 `ZOOM_MAX = 1.0`: macOS `run.m:513-514`,
/// Windows `state.rs:58`)。上限的产品原因: 防"蚂蚁大小涂鸦"。
pub const MAX_ZOOM: f64 = 1.0;

/// 键盘镜头平移一步的屏幕像素步长(两壳都写 80.0: macOS `run.m:1791`,
/// Windows `hotkeys.rs:817`)。
pub const KEY_PAN_STEP_PX: f64 = 80.0;

/// 键盘镜头缩放一步的倍率(两壳都写 1.15: macOS `run.m:1806`,
/// Windows `hotkeys.rs:796-797`)。
pub const ZOOM_STEP_FACTOR: f64 = 1.15;

/// 无限画布镜头: 视口左上角在画布坐标系中的位置 + 缩放。
///
/// `zoom` 1.0 = 100%;合法区间 0.05..=1.0(见 [MIN_ZOOM]/[MAX_ZOOM]), 但
/// 载入历史数据时刻意不自动钳制(见 [Camera::from_db_values])。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// 视口左上角在画布坐标系中的 X(画布单位)。
    pub pan_x: f64,
    /// 视口左上角在画布坐标系中的 Y(画布单位)。
    pub pan_y: f64,
    /// 缩放, 1.0 = 100%。
    pub zoom: f64,
}

impl Camera {
    /// 回原点 + 100%(两壳初始值与 `canvas_reset_lens` 同款)。
    pub const HOME: Self = Self {
        pan_x: 0.0,
        pan_y: 0.0,
        zoom: 1.0,
    };

    /// 直接构造(用于载入/测试;不做任何钳制)。
    pub const fn new(pan_x: f64, pan_y: f64, zoom: f64) -> Self {
        Self { pan_x, pan_y, zoom }
    }

    /// 回原点 + 100%(macOS `canvas_reset_lens` `run.m:1466-1474`;
    /// Windows 翻页分支 `set_cam(0,0,1)` `actions.rs:744-752` / 面板
    /// 「回到原点」`CMD_CANVAS_HOME` `actions.rs:283-288`)。
    pub fn home(&mut self) {
        *self = Self::HOME;
    }

    /// 平移镜头: 把**内容**在屏幕上移动 `(dx*zoom, dy*zoom)` 逻辑点。
    ///
    /// `dx/dy` 是画布单位增量, 调用方已按 zoom 折算(键盘 `80/z`、鼠标拖动
    /// `Δ/z`);实现 = 两壳 `canvas_pan_by` 的 `pan -= d`(macOS
    /// `run.m:464-465` `g_pan_x -= dx`;Windows `actions.rs:663-664`
    /// `set_cam(px - dx, py - dy, z)`)。正 dx = 内容向右。
    ///
    /// 书写中忽略 / 仅无限画布生效的门控留在 adapter。
    pub fn pan_by(&mut self, dx: f64, dy: f64) {
        self.pan_x -= dx;
        self.pan_y -= dy;
    }

    /// 以视口点 `(viewport_x, viewport_y)` 为锚缩放: 锚点下的**画布点**保持
    /// 不动。
    ///
    /// 公式逐行来自两壳 `canvas_zoom_at`(macOS `run.m:507-535`, Windows
    /// `actions.rs:672-713`):
    ///
    /// ```text
    /// cc   = view / zoom + pan                 // 锚点的画布坐标(旧 zoom)
    /// nz   = clamp(zoom * factor, 0.05, 1.0)
    /// zoom = nz;  pan = cc - view / nz
    /// ```
    ///
    /// 返回 [ZoomOutcome]: 两端提示/早退规则不同(模块文档差异 2), 由 adapter
    /// 决定是否重绘/peek/弹提示。重绘、持久化节流、飘渺 peek 仍留在 adapter。
    pub fn zoom_at(&mut self, factor: f64, viewport_x: f64, viewport_y: f64) -> ZoomOutcome {
        let previous_zoom = self.zoom;
        let canvas_x = viewport_x / self.zoom + self.pan_x;
        let canvas_y = viewport_y / self.zoom + self.pan_y;
        let mut new_zoom = self.zoom * factor;
        let hit_max = new_zoom > MAX_ZOOM;
        let hit_min = new_zoom < MIN_ZOOM;
        new_zoom = new_zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        self.zoom = new_zoom;
        self.pan_x = canvas_x - viewport_x / new_zoom;
        self.pan_y = canvas_y - viewport_y / new_zoom;
        ZoomOutcome {
            previous_zoom,
            zoom: new_zoom,
            hit_max,
            hit_min,
        }
    }

    /// 以视口中心为锚缩放(键盘镜头缩放两端同款: macOS `run.m:1806-1807`
    /// `g_screen_w/h * 0.5`, Windows `hotkeys.rs:821-824` `canvas.w/h * 0.5`)。
    pub fn zoom_at_center(&mut self, factor: f64, viewport_w: f64, viewport_h: f64) -> ZoomOutcome {
        self.zoom_at(factor, viewport_w * 0.5, viewport_h * 0.5)
    }

    /// 把 zoom 钳到 [MIN_ZOOM, MAX_ZOOM](两端 `canvas_zoom_at` 内联的上下限,
    /// macOS `run.m:513-526` / Windows `actions.rs:681-688`)。只动 zoom 不动
    /// pan;NaN 原样保留(两壳内联 if 对 NaN 同样不生效), 非法值兜底见
    /// [Camera::from_db_values]。
    pub fn clamp(&mut self) {
        self.zoom = self.zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    }

    /// 交互公式用的 zoom 兜底: `zoom > 0.05 ? zoom : 1.0`。
    ///
    /// 两壳键盘步长/鼠标拖动 `/z`/居中公式同款: macOS `run.m:1790`、
    /// `run.m:2274`、`menu.m:1257`;Windows `hotkeys.rs:816`、
    /// `mouse.rs:177`、`actions.rs:294`。
    ///
    /// 注意 strict `>`: 恰好 0.05 也按 1.0 算(既有行为, D5a 照抄不改)。
    pub fn step_zoom(&self) -> f64 {
        if self.zoom > MIN_ZOOM { self.zoom } else { 1.0 }
    }

    /// 键盘镜头平移每步的画布增量 = [KEY_PAN_STEP_PX] / [Camera::step_zoom]
    /// (两壳 `step = 80.0 / z` 同款, macOS `run.m:1790-1791` / Windows
    /// `hotkeys.rs:814-818`)。方向符号(哪边是正)留在 adapter。
    pub fn keyboard_pan_step(&self) -> f64 {
        KEY_PAN_STEP_PX / self.step_zoom()
    }

    /// 画布坐标 → 视口坐标: `(canvas − pan) × zoom`。
    ///
    /// 两壳绘制/命中同款(macOS `run.m:111-121`、`glaspen2.m:428`;
    /// Windows `state.rs:94-102` `view_from_canvas`)。非法 zoom 的兜底两端
    /// 不同(模块文档差异 3), 需要时 adapter 先自行兜底;本函数按给定 zoom 直算。
    pub fn canvas_to_view(&self, canvas_x: f64, canvas_y: f64) -> (f64, f64) {
        (
            (canvas_x - self.pan_x) * self.zoom,
            (canvas_y - self.pan_y) * self.zoom,
        )
    }

    /// 视口坐标 → 画布坐标: `view / zoom + pan`。
    ///
    /// 两壳笔输入/缩放锚点同款(macOS `glaspen2.m:427-440`;Windows
    /// `state.rs:79-92` `canvas_from_view`)。与 [Camera::canvas_to_view] 互逆。
    pub fn view_to_canvas(&self, view_x: f64, view_y: f64) -> (f64, f64) {
        (
            view_x / self.zoom + self.pan_x,
            view_y / self.zoom + self.pan_y,
        )
    }

    /// 镜头居中到内容包围盒(两端 `canvasCenter` / 面板「居中内容」的同款公式)。
    ///
    /// `bounds` = 笔迹包围盒 `(min_x, min_y, max_x, max_y)`(画布坐标;
    /// `glaspen2_stroke_bbox` 的四个 out 参数),`viewport` = `(w, h)` 当前视口
    /// 尺寸(macOS `g_screen_w/h`, Windows `state.canvas.w/h`)。
    ///
    /// 公式(两壳逐字符一致, macOS `menu.m:1256-1259` / Windows
    /// `actions.rs:293-300`):
    ///
    /// ```text
    /// z   = zoom > 0.05 ? zoom : 1.0          // step_zoom, 注意 strict >
    /// pan = 包围盒中心 − 视口尺寸 * 0.5 / z
    /// ```
    ///
    /// 只改 pan, 不改 zoom;返回公式实际用的 z。Windows 旧代码会把 z 一并写回
    /// 镜头(越界 zoom 因此被"修"成 1.0), macOS 不写 —— 是否写回由 adapter 决定。
    pub fn centered_on(&mut self, bounds: (f64, f64, f64, f64), viewport: (f64, f64)) -> f64 {
        let (min_x, min_y, max_x, max_y) = bounds;
        let (viewport_w, viewport_h) = viewport;
        let zoom = self.step_zoom();
        self.pan_x = (min_x + max_x) * 0.5 - viewport_w * 0.5 / zoom;
        self.pan_y = (min_y + max_y) * 0.5 - viewport_h * 0.5 / zoom;
        zoom
    }

    /// 镜头三值的落库字符串: 与 `db::set_infinite_transform` 的 `{:.6}` 逐字节
    /// 一致(`crates/glaspen-core/src/db.rs:2433-2438`)。键名
    /// (`infinite_pan_x` / `infinite_pan_y` / `infinite_zoom`)仍归 db 层。
    pub fn to_db_values(self) -> (String, String, String) {
        (
            format!("{:.6}", self.pan_x),
            format!("{:.6}", self.pan_y),
            format!("{:.6}", self.zoom),
        )
    }

    /// 解析落库三串 → 镜头;任一缺失/不可解析 → `None`(调用方按
    /// `glaspen2_get_infinite_transform` 现状回 [Camera::HOME],
    /// `export/mod.rs:318-333`)。
    ///
    /// 与两端载入同款(macOS `canvas_infinite_load` `run.m:1457-1464` /
    /// Windows `apply_infinite_canvas` `actions.rs:739-743`): pan 原样;
    /// zoom 只做 `> 0.0` 兜底为 1.0(含 NaN), **不钳 100% 上限** —— 历史脏值
    /// 原样载入, 需要时调用方显式 [Camera::clamp]。
    pub fn from_db_values(pan_x: &str, pan_y: &str, zoom: &str) -> Option<Self> {
        let pan_x = pan_x.parse::<f64>().ok()?;
        let pan_y = pan_y.parse::<f64>().ok()?;
        let zoom = zoom.parse::<f64>().ok()?;
        Some(Self::from_stored(pan_x, pan_y, zoom))
    }

    /// 载入兜底(输入已是解析后的 `f64`;与 [Camera::from_db_values] 的 zoom 规则
    /// 同款): pan 原样, zoom 只做 `> 0.0` 兜底为 1.0(含 NaN), **不钳 100% 上限**。
    ///
    /// 给已经拿到 `f64` 三值的载入路径用: `glaspen2_get_infinite_transform`
    /// (macOS/Windows 都经它载入) 在 FFI 边界做一次兜底, adapter 不再重复。
    pub fn from_stored(pan_x: f64, pan_y: f64, zoom: f64) -> Self {
        Self {
            pan_x,
            pan_y,
            zoom: if zoom > 0.0 { zoom } else { 1.0 },
        }
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self::HOME
    }
}

/// 一次 [Camera::zoom_at] 的结果。
///
/// 字段给 adapter 保留两端不同的提示/早退行为(模块文档差异 2): 核心不做提示
/// 也不提前 return, 是否重绘/peek/持久化由 adapter 决定。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZoomOutcome {
    /// 缩放前的 zoom。
    pub previous_zoom: f64,
    /// 缩放后的 zoom(已钳制)。
    pub zoom: f64,
    /// 本次请求(钳制前)越过了 100% 上限。
    pub hit_max: bool,
    /// 本次请求(钳制前)低于 5% 下限。
    pub hit_min: bool,
}

impl ZoomOutcome {
    /// zoom 是否真的变了: 两端"没变化就不重绘/不 peek/不持久化"的分支判断
    /// (macOS `run.m:531-532`, Windows `actions.rs:691-708`)。
    ///
    /// pan 是 zoom + 锚点的纯函数, 所以 zoom 不变时 pan 也不变。
    pub fn changed(self) -> bool {
        self.zoom != self.previous_zoom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "actual={actual} expected={expected}"
        );
    }

    #[test]
    fn pan_accumulates_and_moves_content_in_view_space() {
        // 两端 canvas_pan_by 都是 `pan -= d`(macOS run.m:464-465 /
        // Windows actions.rs:663-664), 多次调用 = 增量累加。
        let mut c = Camera::HOME;
        c.pan_by(30.0, -10.0);
        assert_eq!((c.pan_x, c.pan_y), (-30.0, 10.0));
        c.pan_by(5.0, 10.0);
        assert_eq!(c, Camera::new(-35.0, 0.0, 1.0));

        // 内容跟随: pan_by(dx,dy) 让画布点 (0,0) 在屏幕上移动
        // (dx*zoom, dy*zoom)。
        let mut c = Camera::new(120.0, -45.0, 0.5);
        let before = c.canvas_to_view(0.0, 0.0);
        c.pan_by(30.0, -10.0);
        let after = c.canvas_to_view(0.0, 0.0);
        assert_close(after.0 - before.0, 15.0); // 30 * 0.5
        assert_close(after.1 - before.1, -5.0); // -10 * 0.5
    }

    #[test]
    fn zoom_at_keeps_anchor_content_fixed() {
        // 两壳公式(run.m:510-530 / actions.rs:677-709):
        // cc = view/zoom + pan; nz = clamp(zoom*factor); pan = cc - view/nz。
        let mut c = Camera::new(120.0, -45.0, 0.5);
        let (vx, vy) = (300.0, 200.0);
        let anchor = c.view_to_canvas(vx, vy);
        assert_eq!(anchor, (720.0, 355.0)); // 300/0.5+120, 200/0.5-45

        let out = c.zoom_at(2.0, vx, vy);
        assert_eq!(out.previous_zoom, 0.5);
        assert_eq!(out.zoom, 1.0);
        assert!(out.changed());
        assert!(!out.hit_max && !out.hit_min);

        // 锚点下的画布点在缩放前后是同一个点, 且仍在同一视口位置。
        assert_eq!(c.view_to_canvas(vx, vy), anchor);
        assert_eq!(c.canvas_to_view(anchor.0, anchor.1), (vx, vy));
        // 具体 pan 值 = cc - view/nz = (720-300, 355-200)。
        assert_eq!(c.pan_x, 420.0);
        assert_eq!(c.pan_y, 155.0);
    }

    #[test]
    fn zoom_at_center_anchors_viewport_center() {
        // 键盘缩放锚点 = 视口中心(macOS run.m:1806-1807 /
        // Windows hotkeys.rs:821-824)。
        let (w, h) = (1024.0, 768.0);
        let mut c = Camera::new(-40.0, 23.0, 0.8);
        let anchor = c.view_to_canvas(w * 0.5, h * 0.5);
        let out = c.zoom_at_center(ZOOM_STEP_FACTOR, w, h);
        assert_close(out.zoom, 0.8 * 1.15);
        assert!(out.changed());
        let after = c.view_to_canvas(w * 0.5, h * 0.5);
        assert_close(after.0, anchor.0);
        assert_close(after.1, anchor.1);
    }

    #[test]
    fn zoom_at_clamps_bounds_and_reports_hits() {
        // 100% 上限: 已到 1.0 再放大 → 镜头不变但 hit_max 置位
        // (macOS 仍据此弹"已达最大缩放 100%", run.m:513-524)。
        let mut c = Camera::new(120.0, -45.0, 1.0);
        let out = c.zoom_at(1.15, 300.0, 200.0);
        assert!(!out.changed());
        assert!(out.hit_max);
        assert!(!out.hit_min);
        assert_eq!((out.previous_zoom, out.zoom), (1.0, 1.0));
        assert_eq!(c, Camera::new(120.0, -45.0, 1.0));

        // 5% 下限(run.m:525-526 / actions.rs:685-688)。
        let mut c = Camera::new(0.0, 0.0, 0.06);
        let out = c.zoom_at(0.5, 0.0, 0.0);
        assert!(out.changed());
        assert!(out.hit_min);
        assert!(!out.hit_max);
        assert_eq!(c.zoom, MIN_ZOOM);
        assert_eq!((c.pan_x, c.pan_y), (0.0, 0.0));

        // 正常区间不受钳制;factor = 1.0 完全无变化。
        let mut c = Camera::new(0.0, 0.0, 0.5);
        let out = c.zoom_at(0.5, 0.0, 0.0);
        assert!(out.changed());
        assert!(!out.hit_min && !out.hit_max);
        assert_eq!(c.zoom, 0.25);

        let mut c = Camera::new(120.0, -45.0, 1.0);
        let out = c.zoom_at(1.0, 300.0, 200.0);
        assert!(!out.changed() && !out.hit_max && !out.hit_min);
        assert_eq!(c, Camera::new(120.0, -45.0, 1.0));
    }

    #[test]
    fn clamp_mirrors_shell_limits() {
        // 两端 canvas_zoom_at 的内联钳制: > 1.0 → 1.0, < 0.05 → 0.05
        // (macOS run.m:513-526, Windows actions.rs:681-688)。
        let mut c = Camera::new(5.0, 7.0, 1.5);
        c.clamp();
        assert_eq!(c, Camera::new(5.0, 7.0, MAX_ZOOM));

        let mut c = Camera::new(5.0, 7.0, 0.01);
        c.clamp();
        assert_eq!(c, Camera::new(5.0, 7.0, MIN_ZOOM));

        let mut c = Camera::new(5.0, 7.0, 0.5);
        c.clamp();
        assert_eq!(c, Camera::new(5.0, 7.0, 0.5));

        // 常量本身就是两壳的字面量(state.rs:57-58 / run.m:513-526)。
        assert_eq!((MIN_ZOOM, MAX_ZOOM), (0.05, 1.0));
    }

    #[test]
    fn home_resets_to_origin_and_100_percent() {
        let mut c = Camera::new(-321.0, 654.0, 0.25);
        c.home();
        assert_eq!(c, Camera::HOME);
        assert_eq!((c.pan_x, c.pan_y, c.zoom), (0.0, 0.0, 1.0));
        assert_eq!(Camera::default(), Camera::HOME);
    }

    #[test]
    fn centered_on_known_bounds_puts_center_at_viewport_center() {
        // 两端公式(menu.m:1257-1259 / actions.rs:296-300): 包围盒中心 −
        // 视口尺寸*0.5/zoom。包围盒 (100,50)-(300,150) 中心 (200,100);
        // 视口 800x600, zoom 0.5 → pan = (200-800, 100-600) = (-600,-500)。
        let mut c = Camera::new(-999.0, 999.0, 0.5);
        let used = c.centered_on((100.0, 50.0, 300.0, 150.0), (800.0, 600.0));
        assert_eq!(used, 0.5);
        assert_eq!(c.zoom, 0.5); // 只改 pan
        assert_eq!((c.pan_x, c.pan_y), (-600.0, -500.0));
        // 内容中心落在视口中心。
        assert_eq!(c.canvas_to_view(200.0, 100.0), (400.0, 300.0));

        // zoom 1.0: pan = (200-400, 100-300) = (-200,-200)。
        let mut c = Camera::new(0.0, 0.0, 1.0);
        assert_eq!(
            c.centered_on((100.0, 50.0, 300.0, 150.0), (800.0, 600.0)),
            1.0
        );
        assert_eq!((c.pan_x, c.pan_y), (-200.0, -200.0));
    }

    #[test]
    fn centered_on_uses_step_zoom_fallback_but_keeps_stored_zoom() {
        // 两壳居中公式对 zoom 的兜底是 step_zoom(strict > 0.05, 否则 1.0),
        // 不是 clamp 下限: zoom = 0.05 恰好走 1.0 分支(既有行为)。
        let mut c = Camera::new(0.0, 0.0, MIN_ZOOM);
        let used = c.centered_on((0.0, 0.0, 200.0, 100.0), (800.0, 600.0));
        assert_eq!(used, 1.0);
        assert_eq!(c.zoom, MIN_ZOOM); // 核心不写回;Windows 旧代码会写回 1.0
        assert_eq!((c.pan_x, c.pan_y), (-300.0, -250.0));
    }

    #[test]
    fn keyboard_pan_step_is_80_over_step_zoom() {
        // 两壳: step = 80.0 / z, z = (zoom > 0.05) ? zoom : 1.0
        // (macOS run.m:1790-1791;Windows hotkeys.rs:814-818)。
        assert_eq!(Camera::new(0.0, 0.0, 1.0).keyboard_pan_step(), 80.0);
        assert_close(Camera::new(0.0, 0.0, 0.4).keyboard_pan_step(), 200.0);
        // strict `>` 的既有行为: 恰好 5% 也按 1.0 兜底(勿"顺手修")。
        assert_eq!(Camera::new(0.0, 0.0, MIN_ZOOM).keyboard_pan_step(), 80.0);
        assert_eq!(Camera::new(0.0, 0.0, 0.03).keyboard_pan_step(), 80.0);
        assert_eq!(KEY_PAN_STEP_PX, 80.0);
        assert_eq!(ZOOM_STEP_FACTOR, 1.15);
    }

    #[test]
    fn db_values_match_existing_persistence_format() {
        // 格式来自 db::set_infinite_transform 的 `format!("{:.6}")`
        // (db.rs:2433-2438)。
        let c = Camera::new(12.5, -3.25, 0.4);
        let (px, py, pz) = c.to_db_values();
        assert_eq!(
            (px.as_str(), py.as_str(), pz.as_str()),
            ("12.500000", "-3.250000", "0.400000")
        );
        // 重复小数只保证"格式化 → 解析"到 6 位精度(与 db 现状一致)。
        assert_eq!(
            Camera::new(1.0 / 3.0, -2.0 / 3.0, 1.0).to_db_values().0,
            format!("{:.6}", 1.0 / 3.0)
        );
        // 往返: 6 位内可精确表示的值解析回来完全相同。
        assert_eq!(Camera::from_db_values(&px, &py, &pz), Some(c));
        // 全零(首次启动未写过)也保持合法字符串形状。
        assert_eq!(
            Camera::HOME.to_db_values(),
            (
                "0.000000".to_string(),
                "0.000000".to_string(),
                "1.000000".to_string()
            )
        );
    }

    #[test]
    fn db_load_mirrors_shell_sanitization_without_max_clamp() {
        // 两壳载入: pan 原样, zoom = pz > 0 ? pz : 1.0;不钳 100% 上限
        // (run.m:1457-1464 / actions.rs:739-743)。
        assert_eq!(Camera::from_db_values("0", "0", "0"), Some(Camera::HOME));
        assert_eq!(Camera::from_db_values("0", "0", "-3"), Some(Camera::HOME));
        assert_eq!(Camera::from_db_values("0", "0", "NaN"), Some(Camera::HOME));
        assert_eq!(
            Camera::from_db_values("0", "0", "2.5"),
            Some(Camera::new(0.0, 0.0, 2.5))
        );
        // pan 原样(可为负), 不四舍五入。
        assert_eq!(
            Camera::from_db_values("-12.5", "33.25", "0.5"),
            Some(Camera::new(-12.5, 33.25, 0.5))
        );
        // 任一坏值 → None(调用方回 HOME, 同 FFI unwrap_or)。
        assert_eq!(Camera::from_db_values("oops", "0", "1"), None);
        assert_eq!(Camera::from_db_values("", "", ""), None);

        // from_stored = 已解析 f64 的同款兜底(FFI 边界用;不钳上限)。
        assert_eq!(Camera::from_stored(0.0, 0.0, 0.0), Camera::HOME);
        assert_eq!(Camera::from_stored(0.0, 0.0, -3.0), Camera::HOME);
        assert!(Camera::from_stored(0.0, 0.0, f64::NAN).zoom == 1.0);
        assert_eq!(
            Camera::from_stored(-12.5, 33.25, 2.5),
            Camera::new(-12.5, 33.25, 2.5)
        );
    }

    #[test]
    fn view_canvas_round_trip() {
        // view = (canvas − pan) × zoom;canvas = view / zoom + pan
        // (glaspen2.m:427-440 / state.rs:85-102)。
        let c = Camera::new(37.5, -12.25, 0.6);
        let (vx, vy) = c.canvas_to_view(123.0, 456.0);
        assert_close(vx, (123.0 - 37.5) * 0.6);
        assert_close(vy, (456.0 + 12.25) * 0.6);
        let (cx, cy) = c.view_to_canvas(vx, vy);
        assert_close(cx, 123.0);
        assert_close(cy, 456.0);
    }

    // ── D5b: 两端 adapter 镜头公式残留闸门 ──
    //
    // core 是镜头数学(pan/zoom/锚点/限幅/居中/步长/持久化格式)的唯一来源:
    // macOS 经 export::glaspen2_camera_* FFI, Windows 同进程直接调 Camera。
    // 本闸门按文本扫描两端 adapter 源文件, 两类断言:
    //
    // 1) **必须**出现 core 镜头入口 token(接线被退回/删调用即红);
    // 2) 旧行内公式的"指纹"**不得**出现(有人把公式抄回 adapter 即红)。
    //
    // 扫描范围 = 真正做镜头更新的文件:
    //   macOS  run.m / menu.m;
    //   Windows overlay/{actions,hotkeys,mouse,state,run,pipe}.rs。
    // 有意排除(不是 D5b 的重复项):
    //   - run.m 网格绘制的 zoom 兜底 0.05(网格几何, D6);
    //   - glaspen2.m 的 canvas_input_x/y(每笔事件热路径的 view→canvas 只读
    //     换算, 不更新镜头)与 run.m 笔宽 `raw_w *= g_zoom`(绘制消费);
    //   - Windows canvas.rs draw_grid / actions.rs 笔宽缩放(绘制消费, D6/D9)。
    //
    // 模式选择: 不用 "g_pan_x"/"zoom" 这类宽泛子串(会命中状态赋值/绘制),
    // 只用旧公式的独特写法, 误报为零; 命中输出 文件:行号 + 匹配文本。
    #[test]
    fn adapter_camera_math_uses_core() {
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
                            "{name}:{} 仍含行内镜头公式 {:?}",
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
                .map(|token| format!("{name} 缺少 core 镜头入口 {token:?}"))
                .collect()
        }

        let mac_run = include_str!("../../../src/macos/parts/run.m");
let mac_cam_swift = include_str!("../../../src/macos/swift/canvas_camera.swift");
let mac_run_cam = format!("{mac_run}
{mac_cam_swift}");
        let mac_menu = include_str!("../../../src/macos/parts/menu.m");
        let win_actions = include_str!("../../../crates/glaspen-windows/src/overlay/actions.rs");
        let win_hotkeys = include_str!("../../../crates/glaspen-windows/src/overlay/hotkeys.rs");
        let win_mouse = include_str!("../../../crates/glaspen-windows/src/overlay/mouse.rs");
        let win_state = include_str!("../../../crates/glaspen-windows/src/overlay/state.rs");
        let win_run = include_str!("../../../crates/glaspen-windows/src/overlay/run.rs");
        let win_pipe = include_str!("../../../crates/glaspen-windows/src/overlay/pipe.rs");
        let db = include_str!("db.rs");
        // include_str! 指错文件/文件被清空时闸门不能静默变绿。
        for (name, src) in [
            ("run.m", mac_run),
            ("menu.m", mac_menu),
            ("actions.rs", win_actions),
            ("hotkeys.rs", win_hotkeys),
            ("mouse.rs", win_mouse),
            ("state.rs", win_state),
            ("run.rs", win_run),
            ("pipe.rs", win_pipe),
            ("db.rs", db),
        ] {
            assert!(
                src.len() > 500,
                "闸门扫到的 {name} 过短, include_str! 路径可疑"
            );
        }

        // ── 旧行内公式指纹(两端各自的历史写法) ──
        let mac_forbidden = [
            // 平移: g_pan_x -= dx / g_pan_y -= dy
            "g_pan_x -=",
            "g_pan_y -=",
            // 缩放: cc = v/z + pan; nz = zoom * factor; g_zoom = nz; pan = cc - v/nz
            "g_zoom * factor",
            "ccx =",
            "ccy =",
            "g_zoom = nz",
            // 居中: pan = bbox 中心 - 视口/2z
            "(bx + bx2) * 0.5 -",
            "(by + by2) * 0.5 -",
            // 键盘步长: step = 80.0 / z
            "80.0 / z",
            // 交互 zoom 兜底: (g_zoom > 0.05) ? g_zoom : 1.0
            "g_zoom > 0.05 ? g_zoom : 1.0",
            // 载入兜底: g_zoom = (pz > 0.0) ? pz : 1.0(已收进 FFI)
            "(pz > 0.0) ? pz : 1.0",
        ];
        let win_forbidden = [
            // 平移: set_cam(px - dx, py - dy, z)
            "px - dx",
            "py - dy",
            // 缩放: ccx = vx / z + px; nz = z * factor; set_cam(ccx - vx / nz, ..)
            "nz = z * factor",
            "vx / z + px",
            "vy / z + py",
            "ccx - vx / nz",
            "ccy - vy / nz",
            "if nz == z",
            // 居中: pan = bbox 中心 - 视口/2z
            "(bx + bx2) * 0.5",
            "(by + by2) * 0.5",
            // 键盘步长 / 交互 zoom 兜底
            "80.0 / z",
            "if z > ZOOM_MIN",
            // 载入兜底: if pz > 0.0 { pz } else { 1.0 }(已收进 FFI)
            "pz > 0.0 { pz } else { 1.0 }",
            // 状态层兜底: (cx - px) * z / cam()/set_cam 旧 helper
            "cam()",
            "set_cam(",
        ];

        let mut problems = Vec::new();
        problems.extend(violations("run.m", mac_run, &mac_forbidden));
        problems.extend(violations("menu.m", mac_menu, &mac_forbidden));
        for (name, src) in [
            ("actions.rs", win_actions),
            ("hotkeys.rs", win_hotkeys),
            ("mouse.rs", win_mouse),
            ("state.rs", win_state),
            ("run.rs", win_run),
            ("pipe.rs", win_pipe),
        ] {
            problems.extend(violations(name, src, &win_forbidden));
        }

        // ── 必须接 core 镜头入口(mac 经 FFI / Windows 直接调 Camera) ──
        for (name, src, tokens) in [
            (
                "run.m+canvas_camera.swift",
                mac_run_cam.as_str(),
                &[
                    "glaspen2_camera_pan(",
                    "glaspen2_camera_zoom_at(",
                    "glaspen2_camera_zoom_at_center(",
                    "glaspen2_camera_keyboard_pan_step(",
                    "glaspen2_camera_step_zoom(",
                    "glaspen2_camera_home(",
                    "camera_assign(",
                ][..],
            ),
            (
                "menu.m",
                mac_menu,
                &[
                    "glaspen2_camera_centered(",
                    "glaspen2_camera_home(",
                    "camera_assign(",
                ][..],
            ),
            (
                "actions.rs",
                win_actions,
                &[
                    "let mut cam = camera();",
                    "cam.pan_by(",
                    "cam.zoom_at(",
                    "cam.zoom_at_center(",
                    "cam.centered_on(",
                    "set_camera(",
                    "Camera::HOME",
                ][..],
            ),
            (
                "hotkeys.rs",
                win_hotkeys,
                &["keyboard_pan_step(", "canvas_zoom_at_center("][..],
            ),
            ("mouse.rs", win_mouse, &["step_zoom("][..]),
            (
                "state.rs",
                win_state,
                &[
                    "Camera::from_stored(",
                    "fn camera() -> Camera",
                    "fn set_camera(cam: Camera)",
                ][..],
            ),
            ("run.rs", win_run, &["Camera::new(", "set_camera("][..]),
            (
                "pipe.rs",
                win_pipe,
                &["Camera::from_stored(", "step_zoom()"][..],
            ),
            // 持久化 {:.6} 格式单源到 Camera::to_db_values
            ("db.rs", db, &["camera::Camera", "to_db_values()"][..]),
        ] {
            problems.extend(must_contain(name, src, tokens));
        }

        assert!(
            problems.is_empty(),
            "D5b 镜头接线闸门失败(两端必须只用 core camera 数学):\n{}",
            problems.join("\n")
        );
    }
}
