//! Windows 覆盖层的绘制计划适配层(D9c 第一步):**Windows adapter 消费
//! core `draw_spec::plan` 的入口**。
//!
//! 背景(docs/duplication-backlog.md D9c):D9a 已在 core 产出平台无关的
//! [DrawPlan](glaspen_core::draw_spec::DrawPlan)(pass 顺序 / 每点宽度 / 透明度 /
//! 颜色 / 虚线 / 脏区),但 Windows 覆盖层仍在 overlay/pen.rs 与
//! overlay/actions.rs 里自己写了一份「三档阴影 → 描边 → 墨迹」的 pass 循环。
//! 本模块把「Windows 参数」与「计划 → 光栅器输入」的**纯映射**集中在这里,
//! 让 overlay 的执行层只调用画法原语(cairo / 像素直写),不再决定 pass 顺序。
//!
//! 本模块**不依赖 Win32**:它只吃 `Stroke` / `Camera` / 画布尺寸,产
//! `DrawPlan` 或纯几何值,所以在 macOS 开发机上也能编译并跑单测
//! (`cargo test -p glaspen-windows paint_plan::`)——「两端同 strokes → 同命令
//! 序列」的契约在非 Windows 机器上也能验证。
//!
//! # 平台参数(差异全部显式取自 core,不静默统一)
//!
//! - 视图:[view] 用 `Camera::canvas_to_view` 同款公式
//!   (`view = (canvas − pan) × zoom`);Windows 表面就是虚拟屏幕物理像素,
//!   所以 `scale = 1.0`、无页 fit;翻页模式镜头恒 HOME。
//! - 分层顺序:[`PassOrder::WindowsPerStroke`](逐笔「三档阴影 → 描边 → 墨迹」)
//!   是 D9a 审计差异 A 的 Windows 侧;macOS 整页重建的
//!   [`PassOrder::MacosCommitted`] 不在这里使用。
//! - 阴影:[`ShadowParams::windows`](spread 固定视图像素、不随 zoom 缩放,差异 B)
//!   + [`OutlineParams::windows`](pad/dash 固定 1 视图像素,差异 C)。
//! - 脏区:整页重建用 [`PlanScope::Rebuild`];实时笔迹用
//!   [`PlanScope::WindowsLive`](ceil 半径 + 1 + 描边 + 软阴影 5,截断到画布,
//!   差异 L)。实时绘制仍然重填整条在写路径(旧行为),但**脏区只覆盖新增段** ——
//!   这段「只取新增段」的切片由 adapter 决定,公式本身还是 core 的。
//!
//! # 保留在 adapter 的 Windows 光栅化差异(D9a 审计 D/F/K)
//!
//! - 差异 D 描边取宽:取相邻两点**较粗者**(core 计划给每点宽度 +
//!   [outline_segment_width] 取 max);macOS 整页重建取当前点。
//! - 差异 F 墨迹光栅化:仍是 `build_outline` 法线轮廓 + 一次填充 + 两端圆点
//!   (不被 plan 规定;plan 只给中心线点列与每点直径)。
//! - 差异 K 墨迹最小宽度:计划存原始直径,Windows 在光栅化前把**半径**
//!   钳到 0.5px([command_path])。
//! - 无 cairo 回退(差异 O):plan 不驱动回退路径,回退仍走画法原语内部近似。

use glaspen_core::Stroke;
use glaspen_core::camera::Camera;
use glaspen_core::draw_spec::{
    Color, Dash, DrawCommand, DrawPlan, LiveStroke, OutlineParams, PassOrder, PlanError,
    PlanParams, PlanScope, Rect, ShadowParams, Vec2, ViewTransform, plan,
};

/// Windows 光栅化半径下限(旧 overlay/pen.rs 的 `.max(0.5)` 收窄,差异 K)。
pub const MIN_VIEW_RADIUS: f64 = 0.5;

/// 描边虚线回退段长(plan 的 Outline 命令恒带 dash;仅防御性回退)。
const FALLBACK_DASH_LEN: f64 = 1.0;

/// 画布像素上的整数脏矩形(半开区间 [left, right) × [top, bottom))。
///
/// 本模块不依赖 Win32 的 `RECT`,执行层(canvas.rs)转换一次即可;
/// 单测可以直接断言纯几何结果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DirtyRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl DirtyRect {
    /// 空矩形(左/上 > 右/下)。
    pub fn is_empty(self) -> bool {
        self.right <= self.left || self.bottom <= self.top
    }
}

/// Windows 视图变换:表面像素 = (画布 − pan) × zoom(scale 恒 1)。
///
/// 翻页模式镜头恒 HOME(不消费平移/缩放),与 `state::view_from_canvas` 同款。
pub fn view(infinite: bool, cam: Camera) -> ViewTransform {
    if infinite {
        ViewTransform::new(Vec2::new(cam.pan_x, cam.pan_y), cam.zoom, 1.0)
    } else {
        ViewTransform::IDENTITY
    }
}

/// 整页重建计划(撤销/翻页/改设置后 `redraw_from_strokes` 全量重绘)。
///
/// 旧实现是每笔一次 `fill_stroke_path`:三档阴影 → 描边 → 墨迹,
/// 与 `PassOrder::WindowsPerStroke` 的计划逐条对应。整页重建的
/// `clear/dirty` = 全视口(执行层本来就会先 `canvas.clear()`,这里只取命令)。
pub fn rebuild_plan(
    strokes: &[Stroke],
    view: ViewTransform,
    viewport_w: f64,
    viewport_h: f64,
    shadow: bool,
    outline: bool,
) -> Result<DrawPlan, PlanError> {
    let mut params = PlanParams::new(
        view,
        Rect::new(0.0, 0.0, viewport_w.max(0.0), viewport_h.max(0.0)),
    );
    params.order = PassOrder::WindowsPerStroke;
    params.scope = PlanScope::Rebuild;
    if shadow {
        params.shadow = Some(ShadowParams::windows());
    }
    if outline {
        params.outline = Some(OutlineParams::windows());
    }
    plan(strokes, &params)
}

/// 实时笔迹计划(在写笔迹整条重填,旧 `redraw_pen` 行为)。
///
/// 输入点列已经是**视图坐标**(`paint_plan::view_points` 由 pen_path 半径换算),
/// 所以视图恒 IDENTITY、宽度恒等映射;plan 只负责 pass 顺序/透明度/虚线。
pub fn live_plan(
    points: &[(f64, f64, f64, f64)],
    color: (f64, f64, f64),
    viewport_w: i32,
    viewport_h: i32,
    shadow: bool,
    outline: bool,
) -> Result<DrawPlan, PlanError> {
    plan(
        &[],
        &live_params(points, color, viewport_w, viewport_h, shadow, outline),
    )
}

/// 实时笔迹的增量脏区(只覆盖 `points` 这段新点;旧 `redraw_pen` 的 ceil 公式)。
///
/// 公式由 core 的 `PlanScope::WindowsLive` 拥有:这里用同一组点生成一个
/// 增量计划,读它的 `dirty`(逐点 union + 截断到视口),adapter 只决定
/// 「传哪一段新点」。空点列/完全在视口外 → `None`。
pub fn live_dirty(
    points: &[(f64, f64, f64, f64)],
    viewport_w: i32,
    viewport_h: i32,
    shadow: bool,
    outline: bool,
) -> Option<DirtyRect> {
    if points.is_empty() {
        return None;
    }
    let params = live_params(
        points,
        (0.0, 0.0, 0.0),
        viewport_w,
        viewport_h,
        shadow,
        outline,
    );
    let plan = plan(&[], &params).ok()?;
    let dirty = plan.dirty;
    if dirty.is_empty() {
        return None;
    }
    Some(DirtyRect {
        left: dirty.x as i32,
        top: dirty.y as i32,
        right: (dirty.x + dirty.w) as i32,
        bottom: (dirty.y + dirty.h) as i32,
    })
}

/// 实时计划参数(绘制与脏区共用同一份平台参数,避免两处漂移)。
fn live_params<'a>(
    points: &'a [(f64, f64, f64, f64)],
    color: (f64, f64, f64),
    viewport_w: i32,
    viewport_h: i32,
    shadow: bool,
    outline: bool,
) -> PlanParams<'a> {
    let mut params = PlanParams::new(
        ViewTransform::IDENTITY,
        Rect::new(0.0, 0.0, viewport_w.max(0) as f64, viewport_h.max(0) as f64),
    );
    params.order = PassOrder::WindowsPerStroke;
    params.scope = PlanScope::WindowsLive;
    if shadow {
        params.shadow = Some(ShadowParams::windows());
    }
    if outline {
        params.outline = Some(OutlineParams::windows());
    }
    params.live = Some(LiveStroke {
        points,
        color: Color::new(color.0, color.1, color.2),
    });
    params
}

/// 在写笔迹的 pen_path(视图坐标 + 半径)→ plan 的点列(视图坐标 + 墨迹直径)。
///
/// 半径 → 直径是 `× 2.0`(二进制浮点精确),执行层 [command_path] 再 `÷ 2`
/// 还原,所以实时路径的半径与旧实现逐位一致(只有重建路径的宽度经 core
/// 变换后有 ulp 级重排)。
pub fn view_points(pen_path: &[(f32, f32, f32)]) -> Vec<(f64, f64, f64, f64)> {
    pen_path
        .iter()
        .map(|&(x, y, r)| (x as f64, y as f64, r as f64 * 2.0, 0.0))
        .collect()
}

/// 计划命令 → 画法原语的点列(视图坐标 + **半径**)。
///
/// plan 点存的是「中心线预览位置 + 墨迹直径(表面像素)」;Windows 的画法原语
/// (`build_outline`/`fill_dot`/`stroke_shadow_pass`)要半径,取下限 0.5px
/// (差异 K)。对实时路径是精确往返;对重建路径是 core 宽度变换后的值。
pub fn command_path(plan: &DrawPlan, cmd: &DrawCommand) -> Vec<(f32, f32, f32)> {
    plan.points[cmd.points.clone()]
        .iter()
        .map(|p| {
            (
                p.pos.x as f32,
                p.pos.y as f32,
                (p.width * 0.5).max(MIN_VIEW_RADIUS) as f32,
            )
        })
        .collect()
}

/// 软阴影一档的 spread(plan 的 Shadow 命令 width_delta = 2 × spread)。
pub fn shadow_spread(cmd: &DrawCommand) -> f64 {
    cmd.width_delta * 0.5
}

/// 描边段宽(墨迹直径 + 2 × pad),取相邻两点**较粗者**(差异 D)。
///
/// `segment` 是段下标(点 segment-1 → 点 segment);越界返回 `None`。
pub fn outline_segment_width(plan: &DrawPlan, cmd: &DrawCommand, segment: usize) -> Option<f64> {
    if segment == 0 {
        return None;
    }
    let prev = plan.effective_width(cmd, segment - 1)?;
    let cur = plan.effective_width(cmd, segment)?;
    Some(prev.max(cur))
}

/// plan 的虚线参数(段长 + 起始相位);Outline 命令恒带 dash,防御性回退 1px。
pub fn dash(cmd: &DrawCommand) -> Dash {
    cmd.dash.unwrap_or(Dash {
        len: FALLBACK_DASH_LEN,
        phase: 0.0,
    })
}

/// 命令颜色 → Windows 画法原语的 (R,G,B) **u8** 量化(两端同一规则 (v×255) as u8)。
pub fn color_u8(color: Color) -> (u8, u8, u8) {
    fn channel(v: f64) -> u8 {
        if v.is_finite() {
            (v.clamp(0.0, 1.0) * 255.0) as u8
        } else {
            0
        }
    }
    (channel(color.r), channel(color.g), channel(color.b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use glaspen_core::draw_spec::PaintPass;

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "expected {b}, got {a}");
    }

    /// 与 core draw_spec 特征化测试**同一组**固定夹具(画布坐标):
    /// s0 = (10,20)-(20,30), s1 = (30,40)-(40,50)。
    fn two_strokes() -> Vec<Stroke> {
        vec![
            Stroke {
                id: 11,
                r: 1.0,
                g: 0.0,
                b: 0.0,
                points: vec![(10.0, 20.0, 4.0, 0.0), (20.0, 30.0, 6.0, 0.1)],
            },
            Stroke {
                id: 12,
                r: 0.0,
                g: 0.0,
                b: 1.0,
                points: vec![(30.0, 40.0, 2.0, 0.0), (40.0, 50.0, 2.0, 0.1)],
            },
        ]
    }

    /// 与 core draw_spec 测试同格式的命令序列快照(格式变化 = 像素序列变化)。
    fn snapshot(plan: &DrawPlan) -> Vec<String> {
        let mut out = Vec::new();
        for (li, layer) in plan.layers.iter().enumerate() {
            for c in layer {
                let dash = match &c.dash {
                    None => "-".to_string(),
                    Some(d) => format!("{:.2}/{:.2}", d.len, d.phase),
                };
                out.push(format!(
                    "L{li} {:?} stroke={} live={} pts={}..{} dw={:.2} a={:.2} dash={}",
                    c.pass,
                    c.stroke_index,
                    c.live,
                    c.points.start,
                    c.points.end,
                    c.width_delta,
                    c.alpha,
                    dash
                ));
            }
        }
        out
    }

    // ------------------------------------------------------------------
    // 命令序列特征化(必须逐条等于 D9a 的 Windows 快照)
    // ------------------------------------------------------------------

    /// D9a Windows 特征化快照(draw_spec 测试里锁死的命令序列,逐字复制):
    /// 每笔「三档阴影 → 描边 → 墨迹」。
    #[test]
    fn windows_rebuild_plan_matches_d9a_characterization_snapshot() {
        // pan=(10,20) + zoom 1 → 与 D9a 的 unit_factor_view 完全同构
        // (factor 1、映射后 s0 = (0,0)-(10,10), s1 = (20,20)-(30,30))。
        let cam = Camera::new(10.0, 20.0, 1.0);
        let plan = rebuild_plan(&two_strokes(), view(true, cam), 200.0, 100.0, true, true)
            .expect("plan 应成功");
        assert_eq!(
            snapshot(&plan),
            vec![
                "L0 Shadow stroke=0 live=false pts=0..2 dw=3.00 a=0.20 dash=-",
                "L0 Shadow stroke=0 live=false pts=0..2 dw=6.00 a=0.13 dash=-",
                "L0 Shadow stroke=0 live=false pts=0..2 dw=10.00 a=0.07 dash=-",
                "L0 Outline stroke=0 live=false pts=0..2 dw=2.00 a=1.00 dash=1.00/0.00",
                "L0 Fill stroke=0 live=false pts=0..2 dw=0.00 a=1.00 dash=-",
                "L1 Shadow stroke=1 live=false pts=2..4 dw=3.00 a=0.20 dash=-",
                "L1 Shadow stroke=1 live=false pts=2..4 dw=6.00 a=0.13 dash=-",
                "L1 Shadow stroke=1 live=false pts=2..4 dw=10.00 a=0.07 dash=-",
                "L1 Outline stroke=1 live=false pts=2..4 dw=2.00 a=1.00 dash=1.00/0.00",
                "L1 Fill stroke=1 live=false pts=2..4 dw=0.00 a=1.00 dash=-",
            ]
        );
        // 映射后的位置与 Camera::canvas_to_view 一致(Windows 无 scale/page fit)。
        approx(plan.points[0].pos.x, 0.0);
        approx(plan.points[0].pos.y, 0.0);
        approx(plan.points[1].pos.x, 10.0);
        approx(plan.points[1].pos.y, 10.0);
    }

    #[test]
    fn rebuild_plan_switches_off_passes_when_disabled() {
        let cam = Camera::new(0.0, 0.0, 1.0);
        let strokes = two_strokes();
        let p = rebuild_plan(&strokes, view(true, cam), 200.0, 100.0, false, false).unwrap();
        assert_eq!(p.layers.len(), 2);
        for layer in &p.layers {
            assert_eq!(layer.len(), 1);
            assert_eq!(layer[0].pass, PaintPass::Fill);
        }

        // WindowsPerStroke:阴影/描边/墨迹同属「这一笔」的一层。
        let p = rebuild_plan(&strokes, view(true, cam), 200.0, 100.0, true, false).unwrap();
        assert_eq!(p.layers.len(), 2);
        let passes: Vec<PaintPass> = p.layers[0].iter().map(|c| c.pass).collect();
        assert_eq!(
            passes,
            vec![
                PaintPass::Shadow,
                PaintPass::Shadow,
                PaintPass::Shadow,
                PaintPass::Fill
            ]
        );
    }

    #[test]
    fn rebuild_plan_skips_single_point_strokes_like_legacy() {
        // 旧 fill_stroke_path: path.len() < 2 直接早退 → 不画阴影/描边/墨迹。
        let strokes = vec![Stroke {
            id: 1,
            r: 1.0,
            g: 0.0,
            b: 0.0,
            points: vec![(5.0, 5.0, 2.0, 0.0)],
        }];
        let p = rebuild_plan(
            &strokes,
            view(false, Camera::HOME),
            200.0,
            100.0,
            true,
            true,
        )
        .unwrap();
        assert!(p.layers.is_empty());
        assert!(p.points.is_empty());
    }

    // ------------------------------------------------------------------
    // 实时笔迹(整条重填 + 只画新增段脏区)
    // ------------------------------------------------------------------

    #[test]
    fn live_plan_is_single_per_stroke_layer_with_full_path_passes() {
        let pen_path = [(0.0f32, 0.0f32, 2.0f32), (10.0, 0.0, 3.0)];
        let points = view_points(&pen_path);
        let p = live_plan(&points, (1.0, 0.0, 0.0), 400, 300, true, true).unwrap();
        assert_eq!(p.layers.len(), 1, "实时在写笔迹只有一笔");
        let passes: Vec<PaintPass> = p.layers[0].iter().map(|c| c.pass).collect();
        assert_eq!(
            passes,
            vec![
                PaintPass::Shadow,
                PaintPass::Shadow,
                PaintPass::Shadow,
                PaintPass::Outline,
                PaintPass::Fill
            ]
        );
        assert!(p.layers[0].iter().all(|c| c.live && c.points == (0..2)));
        // 宽度:pen_path 是半径,plan 是直径 → 2×半径。
        approx(p.points[0].width, 4.0);
        approx(p.points[1].width, 6.0);
        // 实时路径视图恒 IDENTITY:位置原样。
        approx(p.points[0].pos.x, 0.0);
        approx(p.points[1].pos.x, 10.0);
        assert!(p.clear.is_none(), "增量计划不清屏");
    }

    #[test]
    fn live_single_point_emits_initial_dot_as_fill_command() {
        // 落笔首份报告(1 点):旧实现先 fill_dot 再早退;plan 侧表现为
        // 阴影/描边命令存在但在执行层因 < 2 点跳过,只有 Fill 命令画圆点。
        let points = view_points(&[(100.0f32, 120.0f32, 2.0f32)]);
        let p = live_plan(&points, (0.0, 0.0, 0.0), 1920, 1080, true, true).unwrap();
        assert_eq!(p.command_count(), 5);
        let fills: Vec<&DrawCommand> = p
            .layers
            .iter()
            .flatten()
            .filter(|c| c.pass == PaintPass::Fill)
            .collect();
        assert_eq!(fills.len(), 1);
        assert_eq!(fills[0].points, 0..1);
        // 执行层看到的点:1 点 + 半径 2.0(不受最小半径影响)。
        let path = command_path(&p, fills[0]);
        assert_eq!(path, vec![(100.0, 120.0, 2.0)]);
    }

    #[test]
    fn live_plan_skips_empty_point_slice() {
        let p = live_plan(&[], (0.0, 0.0, 0.0), 400, 300, true, true).unwrap();
        assert!(p.layers.is_empty());
        assert!(p.command_count() == 0);
    }

    // ------------------------------------------------------------------
    // 计划 → 画法原语(纯几何;跨平台可跑)
    // ------------------------------------------------------------------

    #[test]
    fn command_path_roundtrips_pen_path_radii_exactly() {
        // 实时路径:半径 → 直径(×2)→ 计划 → 半径(÷2 + 0.5 下限)必须逐位相同,
        // 这样接 plan 不改变实时墨迹的像素。
        let pen_path = [
            (12.5f32, 30.25f32, 0.5f32),
            (13.5, 31.0, 1.0),
            (20.0, 40.0, 2.75),
            (20.0, 40.0, 37.125),
        ];
        let points = view_points(&pen_path);
        let p = live_plan(&points, (0.0, 0.0, 0.0), 100, 100, false, false).unwrap();
        let fill = p
            .layers
            .iter()
            .flatten()
            .find(|c| c.pass == PaintPass::Fill)
            .unwrap();
        let path = command_path(&p, fill);
        for (got, want) in path.iter().zip(pen_path.iter()) {
            assert_eq!(got, want);
        }
    }

    #[test]
    fn command_path_clamps_tiny_radius_to_half_pixel() {
        // 差异 K:Windows 光栅化半径下限 0.5px(计划本身不改写宽度)。
        let p = live_plan(
            &[(5.0, 5.0, 0.2, 0.0), (9.0, 5.0, 0.2, 0.0)],
            (0.0, 0.0, 0.0),
            100,
            100,
            false,
            false,
        )
        .unwrap();
        let fill = p.layers[0].last().unwrap();
        // 直接传的第三元是**墨迹直径**,计划不改写它。
        approx(plan_width(&p, fill), 0.2);
        let path = command_path(&p, fill);
        approx(path[0].2 as f64, 0.5);
        approx(path[1].2 as f64, 0.5);
    }

    fn plan_width(p: &DrawPlan, cmd: &DrawCommand) -> f64 {
        p.effective_width(cmd, 0).unwrap()
    }

    #[test]
    fn shadow_spread_and_outline_width_follow_plan_deltas() {
        let cam = Camera::new(0.0, 0.0, 1.0);
        let p = rebuild_plan(&two_strokes(), view(true, cam), 200.0, 100.0, true, true).unwrap();
        // 三档 spread = width_delta / 2(Windows 固定 1.5/3/5,不随 zoom 缩放)
        let spreads: Vec<f64> = p.layers[0].iter().map(shadow_spread).collect();
        for (got, want) in spreads.iter().zip([1.5, 3.0, 5.0]) {
            approx(*got, want);
        }
        // 描边段宽 = 相邻两点较粗者 + 2 × pad(差异 D)
        let outline = p.layers[0]
            .iter()
            .find(|c| c.pass == PaintPass::Outline)
            .unwrap();
        // s0 宽 4 / 6: 段 0→1 取 max(4, 6) + 2 = 8
        approx(outline_segment_width(&p, outline, 1).unwrap(), 8.0);
        assert!(outline_segment_width(&p, outline, 0).is_none());
        assert!(outline_segment_width(&p, outline, 2).is_none());
        // 虚线段长/相位来自计划(Windows 恒 1.0/0)
        let dash = dash(outline);
        approx(dash.len, 1.0);
        approx(dash.phase, 0.0);
    }

    #[test]
    fn outline_width_uses_adjacent_max_on_live_path() {
        let points = view_points(&[(0.0f32, 0.0f32, 1.0f32), (10.0, 0.0, 4.0)]);
        let p = live_plan(&points, (0.0, 0.0, 0.0), 100, 100, false, true).unwrap();
        let outline = p.layers[0]
            .iter()
            .find(|c| c.pass == PaintPass::Outline)
            .unwrap();
        // 直径 2 / 8 → max 8 + 2 × 1
        approx(outline_segment_width(&p, outline, 1).unwrap(), 10.0);
    }

    #[test]
    fn fill_color_quantizes_like_legacy_canvas() {
        assert_eq!(color_u8(Color::new(1.0, 0.0, 0.0)), (255, 0, 0));
        assert_eq!(color_u8(Color::new(0.0, 0.0, 0.0)), (0, 0, 0));
        assert_eq!(color_u8(Color::new(0.5, 0.5, 0.5)), (127, 127, 127));
        // 非有限/越界按既有 (v×255) as u8 饱和规则
        assert_eq!(color_u8(Color::new(f64::NAN, -1.0, 2.0)), (0, 0, 255));
    }

    // ------------------------------------------------------------------
    // 实时脏区(core PlanScope::WindowsLive 公式;旧 pen.rs 逐行等价)
    // ------------------------------------------------------------------

    #[test]
    fn live_dirty_uses_ceil_radius_plus_marks() {
        // 宽 5(直径)→ 半径 2.5 → ceil 3;pad = 3 + 1(AA)+ 1(描边)+ 5(阴影)= 10。
        let pts = [(100.4, 50.2, 5.0, 0.0)];
        let d = live_dirty(&pts, 1920, 1080, true, true).expect("应产生脏区");
        assert_eq!(
            d,
            DirtyRect {
                left: 90,
                top: 40,
                right: 111,
                bottom: 61
            }
        );
    }

    #[test]
    fn live_dirty_drops_marks_when_passes_disabled() {
        let pts = [(100.0, 50.0, 5.0, 0.0)];
        let with_marks = live_dirty(&pts, 1920, 1080, true, true).unwrap();
        let plain = live_dirty(&pts, 1920, 1080, false, false).unwrap();
        // 少 1(描边)+ 5(阴影)*2 边长
        assert_eq!(
            with_marks.right - with_marks.left,
            plain.right - plain.left + 12
        );
        assert_eq!(plain.left, 96); // trunc(100) - (ceil(2.5) + 1 AA)
    }

    #[test]
    fn live_dirty_unions_points_and_clips_to_viewport() {
        // 两点相距很远:并集覆盖两者;左上/右下截断到视口。
        let pts = [(-100.0, -100.0, 2.0, 0.0), (2000.0, 2000.0, 2.0, 0.0)];
        let d = live_dirty(&pts, 300, 200, false, false).expect("应产生脏区");
        assert_eq!(
            d,
            DirtyRect {
                left: 0,
                top: 0,
                right: 300,
                bottom: 200
            }
        );
    }

    #[test]
    fn live_dirty_empty_or_fully_outside_is_none() {
        assert!(live_dirty(&[], 300, 200, true, true).is_none());
        // 完全在视口外(视口 100x100,点 -500,-500 半径 1) → 空
        assert!(live_dirty(&[(-500.0, -500.0, 2.0, 0.0)], 100, 100, false, false).is_none());
    }

    #[test]
    fn live_dirty_first_report_includes_initial_point() {
        // 旧 redraw_pen: 首份报告 new_start=0 → 脏区包含首点(初始圆点)。
        let points = view_points(&[(320.0f32, 240.0f32, 2.0f32)]);
        let d = live_dirty(&points, 1920, 1080, false, false).unwrap();
        let (x, y) = (320, 240);
        let rr = 2 + 1; // ceil(2.0) + AA 1
        assert_eq!(
            d,
            DirtyRect {
                left: x - rr,
                top: y - rr,
                right: x + rr + 1,
                bottom: y + rr + 1
            }
        );
    }

    // ------------------------------------------------------------------
    // 视图/平台参数
    // ------------------------------------------------------------------

    #[test]
    fn view_uses_camera_only_on_infinite_canvas() {
        let cam = Camera::new(37.5, -12.25, 0.4);
        let infinite = view(true, cam);
        approx(infinite.factor(), 0.4);
        approx(infinite.map(Vec2::new(37.5, -12.25)).x, 0.0);
        approx(infinite.map(Vec2::new(47.5, -12.25)).x, 4.0);
        // 翻页模式:镜头恒 HOME(与 state::view_from_canvas 同款)
        let paged = view(false, cam);
        approx(paged.factor(), 1.0);
        approx(paged.map(Vec2::new(37.5, -12.25)).x, 37.5);
    }

    #[test]
    fn rebuild_plan_widths_scale_with_zoom() {
        let cam = Camera::new(0.0, 0.0, 0.5);
        let p = rebuild_plan(&two_strokes(), view(true, cam), 200.0, 100.0, false, false).unwrap();
        // 笔宽 4/6 × zoom 0.5 → 直径 2/3
        approx(p.points[0].width, 2.0);
        approx(p.points[1].width, 3.0);
    }

    #[test]
    fn invalid_viewport_is_reported() {
        // 防御性:非有限视口由 core 拒绝,adapter 不吞错(执行层记日志并跳过)。
        let err = rebuild_plan(
            &[],
            ViewTransform::IDENTITY,
            f64::INFINITY,
            100.0,
            true,
            true,
        )
        .unwrap_err();
        assert_eq!(err, PlanError::InvalidViewport);
    }
}
