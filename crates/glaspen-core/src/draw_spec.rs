//! 绘制指令规范(DrawPlan;D9a 第一步,**本切片不接线两端**)。
//!
//! 目的:macOS 的整页重建走 core export/mod.rs:638 的 paint_strokes_into
//! (CairoRenderer 直画),Windows 覆盖层则在 overlay/pen.rs:56 fill_stroke_path +
//! overlay/canvas.rs 里另写了一份「笔迹 → 阴影/描边/墨迹」的绘制 pass。两端
//! 几何、宽度映射、脏矩形、实时/提交语义大多相同,但像素级细节又各有真实差异
//! (见下方审计)。本模块把两端「一致的部分」收敛成平台无关的纯函数 plan(),
//! 把「真实不同的部分」显式做成参数(PassOrder / PlanScope / ShadowParams /
//! OutlineParams),留给 D9c 让 adapter 选择 —— **不静默统一、不改变任何像素**。
//!
//! 本切片(D9a)只产出:类型 + plan() 纯函数 + 单元/表驱动/特征化测试。两端
//! adapter、FFI、像素常量一律未动;D9b(模型器单源)、D9c(adapter 消费 DrawPlan)、
//! D9d(真机视觉/性能回归)的方案见 docs/duplication-backlog.md。
//!
//! # 模型
//!
//! - [Vec2] / [Rect]:表面像素坐标(原点左上、y 向下,与 cairo ARGB32 表面和
//!   Windows DIB 一致)与轴对齐矩形;脏区/裁剪都用它。
//! - [ViewTransform]:画布坐标 → 表面像素的「平移 + 等比缩放」变换。两端公式
//!   一致:surface = (canvas − pan) × (zoom × scale × page_scale) + page_origin × scale。
//!   macOS 的 scale = Retina backingScaleFactor、page_scale = 页快照 scale-to-fit;
//!   Windows 两者恒为 1。
//! - [PaintPass]:Shadow(软阴影/halo)→ Outline(marching ants 描边)→ Fill(墨迹
//!   本体);Erase 只出现在 macOS 实时橡皮的 CLEAR 指令里。
//! - [DrawPlan]:layers(从下到上的绘制批;层内按数组顺序画) + points(命令引用的
//!   点表,已变换到表面像素、宽度已是墨迹直径) + dirty/clear/clip。
//! - [plan]:纯函数;不读全局、不碰 I/O,同一输入必得同一输出。
//!
//! # 两端现状审计(读码结论;行号以本提交时的 main 为准)
//!
//! ## 已经一致、可以单源的部分(plan 直接承载)
//!
//! 1. pass 语义与笔画内顺序:软阴影 → 描边 → 墨迹。
//!    - Windows overlay/pen.rs:67 / :78 / :93(fill_stroke_path 内);
//!    - macOS 实时 src/macos/parts/panel.m:701 / :738 / :758(raw_draw_dot /
//!      raw_draw_segment 内);
//!    - macOS 整页重建 core export/mod.rs:663 / :705 / :734(paint_strokes_into)。
//! 2. 软阴影三档常量完全同值:(外扩, alpha) = (1.5, 0.20) / (3.0, 0.13) /
//!    (5.0, 0.07),颜色黑。Windows pen.rs:68-69 与 macOS panel.m:702-703、core
//!    export/mod.rs:664-665 逐字一致。
//! 3. 描边是黑白相间 1 (逻辑)px 虚线(marching ants):黑偶相位、白奇相位各描
//!    一遍,相位按整笔累计弧长连续,平头(圆帽会吞掉 1px 段)。Windows
//!    canvas.rs:177-208、macOS panel.m:758-771、core export/mod.rs:705-733、
//!    pagerender.rs:396-470 同规则。
//! 4. 描边宽度 = 墨迹宽度 + 2 × OUTLINE_PAD,OUTLINE_PAD = 1.0
//!    (core export/mod.rs:159、pagerender.rs:392;canvas.rs:194 的 DASH = 1.0 同区)。
//! 5. 墨迹本体:圆头线段 + 首点实心圆点;宽度是「直径」,中心线点列来自模型器。
//!    两端以及核心导出路径的绘制语义一致(pagerender.rs:236-290 是同一套的
//!    软件实现)。
//! 6. 压力 → 笔宽公式已单源 core modeler::pressure_to_width(modeler.rs:218);
//!    Windows pen.rs:2 width_r 只是「直径公式 → 半径」的包装,不再是第二份公式。
//! 7. 坐标管线一致(见上);像素颜色量化规则一致((v × 255) as u8)。
//! 8. 绘制层级:网格/分栏先画、笔迹后画(macOS run.m:168 drawRect 先画网格再贴
//!    表面图;Windows actions.rs:413 redraw_from_strokes 先 draw_grid 再画笔迹)。
//!
//! ## 真实差异(plan 用参数显式化,绝不静默统一)
//!
//! 差异 A 阴影的分层位置(会改变像素):macOS 整页重建把**全部笔迹的三档阴影
//! 先画完**,再逐笔「描边 + 墨迹」(core export/mod.rs:663-698,档位外层/笔迹
//! 内层);Windows 是**逐笔**画「三档阴影 → 描边 → 墨迹」(pen.rs:56-106)。后果:
//! 后画笔迹的阴影在 Windows 会盖在先画笔迹的墨迹/描边边缘上,macOS 不会。
//! -> [PassOrder::MacosCommitted] / [PassOrder::WindowsPerStroke]。
//!
//! 差异 B 阴影外扩是否随视图缩放:macOS 用 (w + spread×2) × zoom×scale,spread
//! 跟着镜头/DPI 缩放(export/mod.rs:682);Windows 的 spread 固定视图像素,只随
//! 笔宽缩放、不随 zoom 缩放(pen.rs:74-76 的 spread 直接加在视图像素上)。
//! -> [ShadowParams::macos] 按 view.factor() 放大 spread;[ShadowParams::windows]
//! 原值使用。
//!
//! 差异 C 描边加宽/虚线是否随视图缩放:macOS 整页重建 pad × zoom×scale、虚线
//! 段长 = max(zoom×scale, 1)(export/mod.rs:719-721 的 d);Windows pad 固定 1 视口
//! 像素、虚线固定 1.0(canvas.rs:194)。-> [OutlineParams::macos_committed] /
//! [OutlineParams::windows] / [OutlineParams::macos_live]。
//!
//! 差异 D 描边取宽规则:macOS 整页重建与 pagerender 用当前点的宽度
//! (export/mod.rs:717、pagerender.rs:434);Windows 用相邻两点较粗者
//! (pen.rs:82 的 r0.max(r1))。plan 统一给「每点宽度」与 width_delta,由光栅器
//! 决定取当前点还是取相邻最大值(D9c 若要逐像素对齐 Windows 需保留后者)。
//!
//! 差异 E 实时描边相位:macOS 实时跨事件接续累计弧长(g_raw_path_len,panel.m:817,
//! 下一段的 offset 从上一段末尾继续);Windows 实时每次重填整条路径、相位从 0
//! 起算(pen.rs:78-91 每次 fill_stroke_path 都从 cum=0 循环)。-> plan 的
//! [OutlineParams::phase_start](调用方按 scale 换算);Windows 传 0。
//!
//! 差异 F 墨迹光栅化方式(视觉细节):macOS 整页重建是「逐段圆头描线 + 首点圆点」
//! (export/mod.rs:734-754),Windows 是「按每点法线生成可变宽闭合轮廓 + 一次填充 +
//! 两端圆点」(pen.rs:12-40 build_outline、:93-105)。等宽直线段两者一致,急转弯
//! 与宽度突变处的接缝/外侧形状不同。plan 只描述中心线点列 + 每点宽度,不规定
//! 光栅化算法;D9c 必须保留各自的算法才能宣称「像素不变」。
//!
//! 差异 G 实时笔迹的输入源:macOS 实时画**原始未平滑**的笔触点,模型器平滑结果
//! 只在 commit 后进 STROKES,且抬笔不重建(panel.m:694-826 画 raw px/py;
//! run.m:2741 注释 P0: no rebuild);Windows 实时画**模型器平滑后**的点
//! (pen.rs:147 handle_point 直接用 results)。-> plan 通过 [LiveStroke] 只取最终
//! 点列,「用原始还是平滑点」由 adapter 决定(D9b/D9c 风险点)。
//!
//! 差异 H 橡皮:仅 macOS。macOS 实时用 cairo CAIRO_OPERATOR_CLEAR 擦像素
//! (panel.m:696-698),不画阴影/描边;抬笔按圆-点相交整笔删除(core export/mod.rs:
//! 981-1013 erase_finish + :1016 stroke_intersects_eraser)。Windows 全仓 0 处
//! 橡皮实现(无实时擦除、无整笔删除)。-> [PaintPass::Erase] + PlanParams::eraser;
//! 只允许增量计划,整页重建计划里被擦笔迹已不存在。
//!
//! 差异 I 整体不透明度:macOS 有 alpha 参数(时光隧道卡片渐显/渐隐)并走独立的
//! 软件合成路径 paint_strokes_soft(export/mod.rs:761,内部用 pagerender,且该路径
//! 是「全部阴影 → 全部描边 → 全部墨迹」的分层,与不透明快路径的「逐笔」不同);
//! Windows 画布路径恒 alpha = 1(翻页是直接切页,无渐显)。-> alpha 参数统一乘到
//! 每条命令上;分层差异记录在此,D9c 若接 alpha < 1 必须按 pagerender 顺序。
//!
//! 差异 J 视图/DPI:macOS 表面像素 = 逻辑点 × backingScaleFactor,并额外有页
//! 快照的 page_ox/page_oy/page_scale(run.m:14 rebuild 传 g_scale/页视图变换);
//! Windows DIB 就是虚拟屏幕物理像素,无独立 DPI 倍率、无页 fit。-> 由
//! [ViewTransform] 参数表达,两端各填各的。
//!
//! 差异 K Windows 的墨迹最小宽度:实时路径把半径钳到 0.5px(pen.rs:219 的
//! .max(0.5)),即直径最小 1px;macOS 不钳。plan 存原始宽度,不改写(D9c 由
//! Windows adapter 在入参前钳,或作为显式参数补进 D9c)。
//!
//! 差异 L 脏矩形:macOS 实时逐点并集,外扩 = 半宽 + (1.5 AA + 软阴影 5)×scale,
//! 在视图坐标记录、drawRect 时再 ×scale 映射(glaspen2.m:374-393
//! dirty_include_surface_point + panel.m:725);Windows 实时只取新增点,外扩 =
//! ceil(半径) + 1 + 描边(0/1) + 软阴影 5,逐点截断到画布(pen.rs:111-145)。
//! 整页重建两端都是全表面刷新。-> [PlanScope::MacosLive] / [PlanScope::WindowsLive]
//! / [PlanScope::Rebuild]。
//!
//! 差异 M 跨页与叠层:macOS 整页重建先画当前页,再把上一页/下一页按
//! y_shift = ±页高 各画一遍(export/mod.rs:429-507),最后画页号文本;Windows
//! 没有邻页、页号。plan 只画「一批笔迹」(一个 y_shift/视图);多页由调用方用不同
//! ViewTransform 多次 plan 后在 adapter 里按页序拼接。
//!
//! 差异 N 反色:macOS 墨迹主体可换成背景反相 pattern source,并支持脏区裁剪
//! (export/mod.rs:700-704、INVERT_DIRTY);Windows 只持久化设置、不消费。
//! -> plan 以 clip 暴露裁剪;pattern source 不属于几何/颜色指令,留 adapter。
//!
//! 差异 O 无 cairo 回退:Windows cairo 加载失败时用自绘 draw_soft_line 近似
//! (canvas.rs:273,软阴影直接跳过、描边退化成实线);macOS 必须 cairo 可用。
//! plan 假定光栅器具备向量描边 + alpha 合成,D9c 不要用 plan 驱动回退路径。
//!
//! ## plan 不建模的(adapter 责任 / D9c 风险)
//!
//! - **接缝重绘**:macOS 实时路径每段后重画上一段墨迹(panel.m:797-806),因为
//!   描边带会覆盖已干墨迹边缘;plan 是语义指令表,不含这条补丁。D9c 若用 plan
//!   驱动 macOS 实时路径必须保留该步骤,否则弯道内侧会出现描边压墨迹的缝。
//! - **虚线相位在整笔路径内的连续推进**:plan 每个 Outline 命令给起始相位,光栅器
//!   必须沿路径累计弧长继续(D9c 实现注意)。
//! - **网格/分栏/minimap/HUD/页号文本/十字光标/反色 pattern**:属于 adapter 的
//!   UI 层,不是笔迹绘制指令。
//! - **背景清空与窗口合成**:Windows 的 BG_BLOCK alpha 块、磨砂玻璃、ULW 合成;
//!   macOS 的 cairo 表面清空与 CGImage 贴图。
//! - **模型器平滑策略**(D9b):plan 只吃最终点列,平滑器用哪份实现、实时画原始
//!   还是平滑点,属于输入侧。
//!
//! # 测试与特征化
//!
//! 单测覆盖:分层顺序(两端两种 order)、阴影/描边宽度随平台规则、渲染变换、
//! 单点/空输入、实时笔迹标记与增量脏区(两端外扩公式)、橡皮、alpha 钳制、
//! 非法输入拒绝、NaN/Inf 点跳过、开关组合表驱动;另有两端命令序列的**特征化
//! 快照**(characterization test):按当前两端真实顺序锁死命令流,D9b/D9c 改动后
//! 可直接 diff 判断像素序列是否被意外改变。

use std::fmt;
use std::ops::Range;

use crate::Stroke;

// ---------------------------------------------------------------------------
// 基础几何
// ---------------------------------------------------------------------------

/// 表面像素坐标点(原点左上、y 向下)。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    pub fn distance_to(self, other: Self) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}

/// 轴对齐矩形(表面像素;半开区间 [x, x+w) × [y, y+h))。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub const EMPTY: Self = Self {
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
    };

    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    pub fn from_points(a: Vec2, b: Vec2) -> Self {
        let x = a.x.min(b.x);
        let y = a.y.min(b.y);
        Self {
            x,
            y,
            w: (a.x.max(b.x) - x).abs(),
            h: (a.y.max(b.y) - y).abs(),
        }
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.w.is_finite() && self.h.is_finite()
    }

    pub fn is_empty(self) -> bool {
        self.w <= 0.0 || self.h <= 0.0
    }

    /// 并集(空矩形是单位元)。
    pub fn union(self, other: Self) -> Self {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = (self.x + self.w).max(other.x + other.w);
        let bottom = (self.y + self.h).max(other.y + other.h);
        Self::new(x, y, right - x, bottom - y)
    }

    /// 交集(不相交返回空矩形)。
    pub fn intersect(self, other: Self) -> Self {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = (self.x + self.w).min(other.x + other.w);
        let bottom = (self.y + self.h).min(other.y + other.h);
        Self::new(x, y, (right - x).max(0.0), (bottom - y).max(0.0))
    }
}

/// 颜色(0..=1,与 Stroke/设置里的笔色同域)。量化到 u8 由各光栅器做,
/// 两端规则一致:(v × 255) as u8。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

impl Color {
    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
    };
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
    };

    pub const fn new(r: f64, g: f64, b: f64) -> Self {
        Self { r, g, b }
    }

    /// 单通道钳制;非有限值按 0 处理(与两端 as u8 饱和/NaN→0 的实际行为一致)。
    fn channel(v: f64) -> f64 {
        if v.is_finite() {
            v.clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    fn from_stroke(s: &Stroke) -> Self {
        Self {
            r: Self::channel(s.r),
            g: Self::channel(s.g),
            b: Self::channel(s.b),
        }
    }

    /// 量化到 8 位 RGB(两端光栅器同一规则:(v × 255) as u8;非有限通道按 0)。
    ///
    /// D9c:macOS 整页重建与 Windows 覆盖层的画法原语都从这里取颜色,
    /// 不再各写一份量化(取值与旧两端 `(r.clamp(0.0,1.0) * 255.0) as u8`
    /// / `(r * 255.0) as u8` 对合法笔色逐位一致)。
    pub fn to_rgb8(self) -> (u8, u8, u8) {
        fn channel_u8(v: f64) -> u8 {
            (Color::channel(v) * 255.0) as u8
        }
        (channel_u8(self.r), channel_u8(self.g), channel_u8(self.b))
    }
}

/// 画布坐标 → 表面像素的仿射变换(只有平移 + 等比缩放,两端公式一致):
/// surface = (canvas − pan) × (zoom × scale × page_scale) + page_origin × scale。
///
/// - scale:表面倍率(macOS Retina backingScaleFactor;Windows 恒 1)
/// - page_scale / page_origin:页快照 scale-to-fit(macOS 快照观看;Windows 恒 1 / 0)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewTransform {
    /// 平移(无限画布镜头 pan;翻页模式为 0)。
    pub pan: Vec2,
    /// 视图缩放(0.05..=1.0;翻页模式 1.0)。
    pub zoom: f64,
    /// 表面倍率(macOS Retina;Windows 1.0)。
    pub scale: f64,
    /// 页 fit 比例(macOS 快照观看;Windows 1.0)。
    pub page_scale: f64,
    /// 页左上角在视口中的偏移(逻辑点;Windows 0)。
    pub page_origin: Vec2,
}

impl ViewTransform {
    pub const IDENTITY: Self = Self {
        pan: Vec2::ZERO,
        zoom: 1.0,
        scale: 1.0,
        page_scale: 1.0,
        page_origin: Vec2::ZERO,
    };

    pub const fn new(pan: Vec2, zoom: f64, scale: f64) -> Self {
        Self {
            pan,
            zoom,
            scale,
            page_scale: 1.0,
            page_origin: Vec2::ZERO,
        }
    }

    pub const fn with_page_fit(mut self, page_origin: Vec2, page_scale: f64) -> Self {
        self.page_origin = page_origin;
        self.page_scale = page_scale;
        self
    }

    /// 画布单位 → 表面像素的总倍率。
    pub fn factor(&self) -> f64 {
        self.zoom * self.scale * self.page_scale
    }

    /// 页 fit 偏置(表面像素)。
    pub fn offset(&self) -> Vec2 {
        Vec2::new(
            self.page_origin.x * self.scale,
            self.page_origin.y * self.scale,
        )
    }

    pub fn map(&self, p: Vec2) -> Vec2 {
        let f = self.factor();
        let off = self.offset();
        Vec2::new(
            (p.x - self.pan.x) * f + off.x,
            (p.y - self.pan.y) * f + off.y,
        )
    }

    /// 长度(墨迹直径/半径/外扩)随视图缩放。
    pub fn map_width(&self, w: f64) -> f64 {
        w * self.factor()
    }

    pub fn is_valid(&self) -> bool {
        self.pan.is_finite()
            && self.zoom.is_finite()
            && self.zoom > 0.0
            && self.scale.is_finite()
            && self.scale > 0.0
            && self.page_scale.is_finite()
            && self.page_scale > 0.0
            && self.page_origin.is_finite()
    }
}

// ---------------------------------------------------------------------------
// 绘制 pass / 指令 / 计划
// ---------------------------------------------------------------------------

/// 绘制 pass。
///
/// 语义顺序(从下到上):Shadow(软阴影/halo)→ Outline(marching ants 描边)→
/// Fill(墨迹本体)。Erase 是 macOS 实时橡皮的 CLEAR 指令,只出现在增量计划里,
/// 不参与整页重建(被擦除的笔迹此时已从 STROKES 删除)。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintPass {
    Shadow,
    Outline,
    Fill,
    Erase,
}

/// 描边虚线段参数(marching ants;黑白交替固定约定,见 [OUTLINE_COLORS])。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dash {
    /// 虚线段长(表面像素,> 0)。
    pub len: f64,
    /// 本命令起点处的相位(表面像素;两端都是整笔累计弧长 mod 2×len)。
    pub phase: f64,
}

/// 描边两色:偶相位黑、奇相位白(两端一致)。
pub const OUTLINE_COLORS: [Color; 2] = [Color::BLACK, Color::WHITE];

/// 软阴影一档:外扩半径(表面像素)+ 透明度。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowBand {
    pub spread: f64,
    pub alpha: f64,
}

/// 两端源码里的三档软阴影常量(画布单位):(外扩半径, 透明度)。
pub const SHADOW_BANDS: [(f64, f64); 3] = [(1.5, 0.20), (3.0, 0.13), (5.0, 0.07)];

/// 软阴影参数(三档,spread 已是表面像素)。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowParams {
    pub bands: [ShadowBand; 3],
}

impl ShadowParams {
    /// macOS:spread 随 view.factor()(zoom × scale × page_scale)缩放
    /// (core export/mod.rs:682 的 (w + spread × 2) × zoom × scale)。
    pub fn macos(view: &ViewTransform) -> Self {
        let f = view.factor();
        Self {
            bands: [
                ShadowBand {
                    spread: SHADOW_BANDS[0].0 * f,
                    alpha: SHADOW_BANDS[0].1,
                },
                ShadowBand {
                    spread: SHADOW_BANDS[1].0 * f,
                    alpha: SHADOW_BANDS[1].1,
                },
                ShadowBand {
                    spread: SHADOW_BANDS[2].0 * f,
                    alpha: SHADOW_BANDS[2].1,
                },
            ],
        }
    }

    /// Windows:spread 固定在视图像素空间,不随镜头 zoom 缩放
    /// (pen.rs:67-76 的 spread 直接加在视图像素宽度上)。
    pub fn windows() -> Self {
        Self {
            bands: [
                ShadowBand {
                    spread: SHADOW_BANDS[0].0,
                    alpha: SHADOW_BANDS[0].1,
                },
                ShadowBand {
                    spread: SHADOW_BANDS[1].0,
                    alpha: SHADOW_BANDS[1].1,
                },
                ShadowBand {
                    spread: SHADOW_BANDS[2].0,
                    alpha: SHADOW_BANDS[2].1,
                },
            ],
        }
    }
}

/// marching-ants 描边参数(全部表面像素)。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutlineParams {
    /// 描边比墨迹宽出的半径(命令 width_delta = 2 × pad)。
    pub pad: f64,
    /// 虚线段长(> 0;黑白各一段)。
    pub dash_len: f64,
    /// 实时笔迹的起始相位(表面像素;整页重建的已提交笔迹恒从 0 起算)。
    pub phase_start: f64,
}

impl OutlineParams {
    /// macOS 整页重建:pad = 1.0 × view.factor(),虚线段长 = max(factor, 1.0),
    /// 相位 0(core export/mod.rs:159 + :705-733 的 d = (zoom × scale).max(1.0))。
    pub fn macos_committed(view: &ViewTransform) -> Self {
        let f = view.factor();
        Self {
            pad: f * 1.0,
            dash_len: f.max(1.0),
            phase_start: 0.0,
        }
    }

    /// macOS 实时:实时绘制在视图坐标系(不经过镜头),pad/线段长 = 1.0 × scale,
    /// 相位接续已画弧长(panel.m:689、:758-760、:817;逻辑点 × backingScaleFactor)。
    pub fn macos_live(scale: f64, arc_len_start: f64) -> Self {
        Self {
            pad: scale,
            dash_len: scale.max(1.0),
            phase_start: arc_len_start * scale,
        }
    }

    /// Windows:pad = 1.0、虚线段长 = 1.0,固定视图像素(pen.rs:82、
    /// canvas.rs:194 的 DASH = 1.0)。
    pub fn windows() -> Self {
        Self {
            pad: 1.0,
            dash_len: 1.0,
            phase_start: 0.0,
        }
    }
}

/// 分层顺序(两端真实差异,显式选择;见模块审计差异 A)。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassOrder {
    /// macOS 整页重建(core export/mod.rs:638 paint_strokes_into):
    /// 层 0 = 全部软阴影(档位外层、笔迹内层),之后每笔一层(描边 → 墨迹)。
    MacosCommitted,
    /// Windows 整页重建/实时(overlay/pen.rs:56 fill_stroke_path):
    /// 每笔一层(三档阴影 → 描边 → 墨迹)。
    WindowsPerStroke,
}

/// 计划范围 + 脏矩形规则(两端真实差异,显式选择;见模块审计差异 L)。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanScope {
    /// 整页重建:清空视口(或裁剪区)后按 layers 全量重画;脏区 = 视口 ∩ clip。
    Rebuild,
    /// macOS 实时:只画在写笔迹;脏区 = 新点按「半宽 + (AA 1.5 + 软阴影 5) × scale」
    /// 外扩后并集(glaspen2.m:385 dirty_include_surface_point + panel.m:725)。
    MacosLive,
    /// Windows 实时:只画在写路径;脏区 = 新点按「ceil(半径) + 1 + 描边(0/1)
    /// + 软阴影 5」外扩、截断到画布(pen.rs:111-145)。
    WindowsLive,
}

impl PlanScope {
    pub fn is_incremental(self) -> bool {
        !matches!(self, Self::Rebuild)
    }
}

/// 在写笔迹(尚未提交进 STROKES)。
#[derive(Clone, Copy, Debug)]
pub struct LiveStroke<'a> {
    /// 采样点(画布坐标 + 墨迹直径 + 相对时间;t 本层忽略)。与 Stroke::points
    /// 同形状,便于 adapter 直接把模型器输出喂进来。
    pub points: &'a [(f64, f64, f64, f64)],
    /// 落笔时的笔色(实时路径尚未提交,颜色不在 Stroke 里)。
    pub color: Color,
}

/// 点表里的一项:表面坐标 + 墨迹直径(表面像素)。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanPoint {
    pub pos: Vec2,
    pub width: f64,
}

/// 一条绘制指令。
#[derive(Clone, Debug, PartialEq)]
pub struct DrawCommand {
    pub pass: PaintPass,
    /// 来源:已提交笔迹 = strokes 下标;在写笔迹 = strokes.len()。
    pub stroke_index: usize,
    /// STROKES 行 id(在写笔迹 = 0)。
    pub stroke_id: i64,
    /// 是否来自在写笔迹(增量计划的脏区只覆盖 live 命令覆盖的点)。
    pub live: bool,
    /// 引用 [DrawPlan::points] 的区间。
    pub points: Range<usize>,
    /// 每点有效宽度 = PlanPoint::width + width_delta。
    /// Shadow = 2 × spread;Outline = 2 × pad;Fill / Erase = 0。
    pub width_delta: f64,
    /// 颜色(Shadow/Outline 固定黑;Outline 的白段按 [OUTLINE_COLORS] 交替)。
    pub color: Color,
    /// 透明度(已乘整体 alpha 与档位 alpha)。
    pub alpha: f64,
    /// 仅 Outline:虚线参数;其余 pass 为 None。
    pub dash: Option<Dash>,
}

/// 绘制计划:platform-neutral 的「这一帧该画什么」。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DrawPlan {
    /// 从下到上的绘制层;层内按数组顺序画。层边界本身就是像素顺序的分界
    /// (两端 order 差异见 [PassOrder])。
    pub layers: Vec<Vec<DrawCommand>>,
    /// 本计划覆盖的脏矩形(表面像素,已外扩/并集;空 = 无变化)。
    pub dirty: Rect,
    /// 绘制前需要清空的矩形(None = 增量绘制,不清空)。
    pub clear: Option<Rect>,
    /// 绘制裁剪(反色局部重绘;None = 不裁剪;光栅器负责应用)。
    pub clip: Option<Rect>,
    /// 命令引用的点表(表面坐标 + 墨迹直径)。
    pub points: Vec<PlanPoint>,
}

impl DrawPlan {
    pub fn commands(&self) -> impl Iterator<Item = &DrawCommand> {
        self.layers.iter().flatten()
    }

    pub fn command_count(&self) -> usize {
        self.layers.iter().map(Vec::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.command_count() == 0
    }

    /// 命令内第 point_index 个点的有效宽度(表面像素)。
    pub fn effective_width(&self, cmd: &DrawCommand, point_index: usize) -> Option<f64> {
        if point_index >= cmd.points.len() {
            return None;
        }
        self.points
            .get(cmd.points.start + point_index)
            .map(|p| p.width + cmd.width_delta)
    }
}

/// plan() 的输入参数。用 [PlanParams::new] 起步,再按平台/开关填字段。
#[derive(Clone, Debug)]
pub struct PlanParams<'a> {
    pub view: ViewTransform,
    /// 表面尺寸(表面像素;整页重建的清空范围与脏区都被它限制)。
    pub viewport: Rect,
    /// 整体不透明度(0..=1;隧道卡片渐显/导出;Windows 画布路径恒 1)。
    pub alpha: f64,
    /// 软阴影;None = 关闭。
    pub shadow: Option<ShadowParams>,
    /// marching ants 描边;None = 关闭。
    pub outline: Option<OutlineParams>,
    /// 分层顺序(两端真实差异)。
    pub order: PassOrder,
    /// 范围/脏区规则(整页重建 vs 两端实时)。
    pub scope: PlanScope,
    /// 在写笔迹;None = 只有已提交笔迹。
    pub live: Option<LiveStroke<'a>>,
    /// 实时橡皮(macOS 独有):只发 Erase 指令,忽略已提交笔迹与阴影/描边。
    pub eraser: bool,
    /// 绘制裁剪(反色局部重绘);None = 不裁剪。
    pub clip: Option<Rect>,
}

impl<'a> PlanParams<'a> {
    pub fn new(view: ViewTransform, viewport: Rect) -> Self {
        Self {
            view,
            viewport,
            alpha: 1.0,
            shadow: None,
            outline: None,
            order: PassOrder::MacosCommitted,
            scope: PlanScope::Rebuild,
            live: None,
            eraser: false,
            clip: None,
        }
    }
}

/// plan() 的错误。非法输入一律显式拒绝(不产出 NaN/Inf 指令)。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanError {
    /// 视图变换含 NaN/Inf,或 zoom/scale/page_scale 非正。
    InvalidView,
    /// 视口尺寸含 NaN/Inf 或为负。
    InvalidViewport,
    /// alpha 含 NaN/Inf。
    InvalidAlpha,
    /// 阴影档位含 NaN/Inf。
    InvalidShadowBand,
    /// 描边 pad/dash_len/phase 含 NaN/Inf,或 dash_len 非正。
    InvalidOutline,
    /// 裁剪矩形含 NaN/Inf 或尺寸为负。
    InvalidClip,
    /// 橡皮只存在于实时增量路径;整页重建计划不能是橡皮。
    EraserInRebuild,
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            Self::InvalidView => "视图变换非法(含 NaN/Inf 或非正的 zoom/scale)",
            Self::InvalidViewport => "视口尺寸非法",
            Self::InvalidAlpha => "整体不透明度非法",
            Self::InvalidShadowBand => "软阴影档位非法",
            Self::InvalidOutline => "描边参数非法(pad/dash_len/phase)",
            Self::InvalidClip => "裁剪矩形非法",
            Self::EraserInRebuild => "橡皮只允许增量计划,整页重建不能是橡皮",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for PlanError {}

// macOS 实时脏区外扩常量(逻辑点,乘 scale 后为表面像素)。
const MACOS_AA_PAD: f64 = 1.5;
// 两端实时脏区都按最宽一档软阴影(5)外扩。
const SHADOW_PAD: f64 = 5.0;
// Windows 实时脏区的 +1 抗锯齿余量(pen.rs:130)。
const WINDOWS_AA_PAD: f64 = 1.0;

/// 一组要画的笔迹几何(点表区间 + 来源元数据)。
struct Geom {
    stroke_index: usize,
    stroke_id: i64,
    range: Range<usize>,
    live: bool,
    color: Color,
}

/// 把 (x, y, width) 点列过滤 + 变换后追加到点表,返回本组的区间。
/// 非有限值/负宽度/变换后溢出的点被丢弃(macOS 的 cairo 与 Windows 的轮廓
/// 对 NaN 都没有健壮语义,本层统一在入口处隔离)。
fn push_points<I>(points: I, view: &ViewTransform, out: &mut Vec<PlanPoint>) -> Range<usize>
where
    I: Iterator<Item = (f64, f64, f64)>,
{
    let start = out.len();
    for (x, y, w) in points {
        if !(x.is_finite() && y.is_finite() && w.is_finite()) || w < 0.0 {
            continue;
        }
        let pos = view.map(Vec2::new(x, y));
        let width = view.map_width(w);
        if !pos.is_finite() || !width.is_finite() {
            continue;
        }
        out.push(PlanPoint { pos, width });
    }
    start..out.len()
}

fn shadow_command(g: &Geom, band: &ShadowBand, alpha: f64) -> DrawCommand {
    DrawCommand {
        pass: PaintPass::Shadow,
        stroke_index: g.stroke_index,
        stroke_id: g.stroke_id,
        live: g.live,
        points: g.range.clone(),
        width_delta: 2.0 * band.spread,
        color: Color::BLACK,
        alpha: (alpha * band.alpha).clamp(0.0, 1.0),
        dash: None,
    }
}

fn outline_command(g: &Geom, outline: &OutlineParams, alpha: f64) -> DrawCommand {
    DrawCommand {
        pass: PaintPass::Outline,
        stroke_index: g.stroke_index,
        stroke_id: g.stroke_id,
        live: g.live,
        points: g.range.clone(),
        width_delta: 2.0 * outline.pad,
        color: Color::BLACK,
        alpha,
        dash: Some(Dash {
            len: outline.dash_len,
            // 已提交笔迹每笔从 0 起算;在写笔迹接续本段起始弧长(macOS 实时)。
            phase: if g.live { outline.phase_start } else { 0.0 },
        }),
    }
}

fn fill_command(g: &Geom, alpha: f64) -> DrawCommand {
    DrawCommand {
        pass: PaintPass::Fill,
        stroke_index: g.stroke_index,
        stroke_id: g.stroke_id,
        live: g.live,
        points: g.range.clone(),
        width_delta: 0.0,
        color: g.color,
        alpha,
        dash: None,
    }
}

fn erase_command(g: &Geom) -> DrawCommand {
    DrawCommand {
        pass: PaintPass::Erase,
        stroke_index: g.stroke_index,
        stroke_id: g.stroke_id,
        live: true,
        points: g.range.clone(),
        width_delta: 0.0,
        color: Color::BLACK,
        alpha: 1.0,
        dash: None,
    }
}

fn dirty_for_point(
    p: &PlanPoint,
    scope: PlanScope,
    view: &ViewTransform,
    shadow: bool,
    outline: bool,
) -> Rect {
    match scope {
        PlanScope::Rebuild => Rect::EMPTY,
        PlanScope::MacosLive => {
            let pad =
                p.width * 0.5 + (MACOS_AA_PAD + if shadow { SHADOW_PAD } else { 0.0 }) * view.scale;
            Rect::new(p.pos.x - pad, p.pos.y - pad, 2.0 * pad, 2.0 * pad)
        }
        PlanScope::WindowsLive => {
            let rr = (p.width * 0.5).ceil()
                + WINDOWS_AA_PAD
                + if outline { 1.0 } else { 0.0 }
                + if shadow { SHADOW_PAD } else { 0.0 };
            let x = p.pos.x.trunc();
            let y = p.pos.y.trunc();
            Rect::new(x - rr, y - rr, 2.0 * rr + 1.0, 2.0 * rr + 1.0)
        }
    }
}

fn dirty_for_range(
    points: &[PlanPoint],
    range: Range<usize>,
    scope: PlanScope,
    view: &ViewTransform,
    shadow: bool,
    outline: bool,
) -> Rect {
    let mut dirty = Rect::EMPTY;
    for p in &points[range] {
        dirty = dirty.union(dirty_for_point(p, scope, view, shadow, outline));
    }
    dirty
}

/// 从已提交笔迹 + 可选在写笔迹,生成平台无关的绘制计划。
///
/// 纯函数:不读全局、不碰 I/O、确定性。像素不变性约束:
/// - 只生成指令,不做任何绘制;光栅器(adapter)决定具体几何算法;
/// - 非法视图/尺寸/参数返回 [PlanError];点列里的 NaN/Inf/负宽度点被跳过;
/// - 已提交笔迹少于 2 点不画(两端整页重建的现状);在写笔迹允许 1 点(落笔
///   圆点,两端实时路径都先画一个点);
/// - 增量计划(scope != Rebuild)不清屏,dirty 只覆盖 live 命令覆盖的点;
///   整页重建计划 clear = dirty = 视口 ∩ clip。
pub fn plan(strokes: &[Stroke], params: &PlanParams<'_>) -> Result<DrawPlan, PlanError> {
    // ---- 输入校验(不产出 NaN/Inf) ----
    if !params.view.is_valid() {
        return Err(PlanError::InvalidView);
    }
    if !params.viewport.is_finite() || params.viewport.w < 0.0 || params.viewport.h < 0.0 {
        return Err(PlanError::InvalidViewport);
    }
    if !params.alpha.is_finite() {
        return Err(PlanError::InvalidAlpha);
    }
    if let Some(shadow) = &params.shadow {
        for band in &shadow.bands {
            if !band.spread.is_finite() || !band.alpha.is_finite() {
                return Err(PlanError::InvalidShadowBand);
            }
        }
    }
    if let Some(outline) = &params.outline
        && (!outline.pad.is_finite()
            || !outline.dash_len.is_finite()
            || outline.dash_len <= 0.0
            || !outline.phase_start.is_finite())
    {
        return Err(PlanError::InvalidOutline);
    }
    if let Some(clip) = &params.clip
        && (!clip.is_finite() || clip.w < 0.0 || clip.h < 0.0)
    {
        return Err(PlanError::InvalidClip);
    }
    if params.eraser && params.scope == PlanScope::Rebuild {
        return Err(PlanError::EraserInRebuild);
    }

    let alpha = params.alpha.clamp(0.0, 1.0);
    let viewport = params.viewport;
    let clip = params.clip.filter(|c| !c.is_empty());

    let mut plan = DrawPlan {
        layers: Vec::new(),
        dirty: Rect::EMPTY,
        clear: None,
        clip,
        points: Vec::new(),
    };

    if params.scope == PlanScope::Rebuild {
        let dirty = clip.map_or(viewport, |c| viewport.intersect(c));
        plan.dirty = dirty;
        plan.clear = Some(dirty);
    }

    // alpha = 0:清屏照常(整页重建),但不产生绘制指令(与 core 的提前返回同义)。
    if alpha <= 0.0 {
        return Ok(plan);
    }

    let shadow_on = params.shadow.is_some();
    let outline_on = params.outline.is_some();

    // ---- 实时橡皮(macOS 独有):只发 CLEAR 指令 ----
    if params.eraser {
        if let Some(live) = &params.live {
            let range = push_points(
                live.points.iter().map(|&(x, y, w, _)| (x, y, w)),
                &params.view,
                &mut plan.points,
            );
            if !range.is_empty() {
                let geom = Geom {
                    stroke_index: strokes.len(),
                    stroke_id: 0,
                    range: range.clone(),
                    live: true,
                    color: Color::BLACK,
                };
                plan.layers.push(vec![erase_command(&geom)]);
                let dirty = dirty_for_range(
                    &plan.points,
                    range,
                    params.scope,
                    &params.view,
                    shadow_on,
                    outline_on,
                );
                plan.dirty = plan.dirty.union(dirty);
            }
        }
        return Ok(plan);
    }

    // ---- 收集几何 ----
    let mut geoms: Vec<Geom> = Vec::new();
    for (index, stroke) in strokes.iter().enumerate() {
        let range = push_points(
            stroke.points.iter().map(|&(x, y, w, _)| (x, y, w)),
            &params.view,
            &mut plan.points,
        );
        // 整页重建两端都跳过不足 2 点的笔迹(core export/mod.rs:690 与
        // pen.rs:56 的 path.len() < 2 早退)。
        if range.len() >= 2 {
            geoms.push(Geom {
                stroke_index: index,
                stroke_id: stroke.id,
                range,
                live: false,
                color: Color::from_stroke(stroke),
            });
        }
    }
    if let Some(live) = &params.live {
        let range = push_points(
            live.points.iter().map(|&(x, y, w, _)| (x, y, w)),
            &params.view,
            &mut plan.points,
        );
        if !range.is_empty() {
            geoms.push(Geom {
                stroke_index: strokes.len(),
                stroke_id: 0,
                range,
                live: true,
                color: live.color,
            });
        }
    }

    // 被跳过(点不足 2)的笔迹不应在点表里留下无人引用的尾巴。
    let used = geoms.iter().map(|g| g.range.end).max().unwrap_or(0);
    plan.points.truncate(used);

    // ---- 按平台 order 分层 ----
    match params.order {
        PassOrder::MacosCommitted => {
            if let Some(shadow) = &params.shadow {
                let mut layer = Vec::new();
                // 档位外层、笔迹内层(core export/mod.rs:663-698)。
                for band in &shadow.bands {
                    for geom in &geoms {
                        layer.push(shadow_command(geom, band, alpha));
                    }
                }
                if !layer.is_empty() {
                    plan.layers.push(layer);
                }
            }
            for geom in &geoms {
                let mut layer = Vec::new();
                if let Some(outline) = &params.outline {
                    layer.push(outline_command(geom, outline, alpha));
                }
                layer.push(fill_command(geom, alpha));
                plan.layers.push(layer);
            }
        }
        PassOrder::WindowsPerStroke => {
            for geom in &geoms {
                let mut layer = Vec::new();
                if let Some(shadow) = &params.shadow {
                    for band in &shadow.bands {
                        layer.push(shadow_command(geom, band, alpha));
                    }
                }
                if let Some(outline) = &params.outline {
                    layer.push(outline_command(geom, outline, alpha));
                }
                layer.push(fill_command(geom, alpha));
                plan.layers.push(layer);
            }
        }
    }

    // ---- 增量脏区:只覆盖 live 命令对应的点 ----
    if params.scope.is_incremental() {
        let mut dirty = Rect::EMPTY;
        for geom in geoms.iter().filter(|g| g.live) {
            dirty = dirty.union(dirty_for_range(
                &plan.points,
                geom.range.clone(),
                params.scope,
                &params.view,
                shadow_on,
                outline_on,
            ));
        }
        plan.dirty = plan.dirty.union(dirty.intersect(viewport));
    }

    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // 固定夹具(特征化测试与表驱动测试共用)
    // ------------------------------------------------------------------

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "expected {b}, got {a}");
    }

    fn rect_approx(r: Rect, x: f64, y: f64, w: f64, h: f64) {
        approx(r.x, x);
        approx(r.y, y);
        approx(r.w, w);
        approx(r.h, h);
    }

    fn stroke(id: i64, rgb: (f64, f64, f64), points: &[(f64, f64, f64, f64)]) -> Stroke {
        Stroke {
            id,
            r: rgb.0,
            g: rgb.1,
            b: rgb.2,
            points: points.to_vec(),
        }
    }

    fn viewport() -> Rect {
        Rect::new(0.0, 0.0, 200.0, 100.0)
    }

    /// factor 恰为 1.0 的视图(pan 10/20, zoom 0.5, scale 2)。
    fn unit_factor_view() -> ViewTransform {
        ViewTransform::new(Vec2::new(10.0, 20.0), 0.5, 2.0)
    }

    /// 两笔固定笔迹:映射后 s0 = (0,0)-(10,10), s1 = (20,20)-(30,30)。
    fn two_strokes() -> Vec<Stroke> {
        vec![
            stroke(
                11,
                (1.0, 0.0, 0.0),
                &[(10.0, 20.0, 4.0, 0.0), (20.0, 30.0, 6.0, 0.1)],
            ),
            stroke(
                12,
                (0.0, 0.0, 1.0),
                &[(30.0, 40.0, 2.0, 0.0), (40.0, 50.0, 2.0, 0.1)],
            ),
        ]
    }

    /// 命令序列快照(特征化测试用;格式变化视为像素序列变化,需人工确认)。
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
    // 基本/分层顺序
    // ------------------------------------------------------------------

    #[test]
    fn rebuild_plan_clears_and_marks_viewport_dirty() {
        let params = PlanParams::new(ViewTransform::IDENTITY, viewport());
        let p = plan(&[], &params).unwrap();
        assert!(p.layers.is_empty());
        assert!(p.is_empty());
        assert_eq!(p.clear, Some(viewport()));
        assert_eq!(p.dirty, viewport());
        assert!(p.clip.is_none());
    }

    #[test]
    fn clip_restricts_rebuild_dirty_and_is_kept() {
        let mut params = PlanParams::new(ViewTransform::IDENTITY, viewport());
        let clip = Rect::new(10.0, 20.0, 30.0, 40.0);
        params.clip = Some(clip);
        let p = plan(&two_strokes(), &params).unwrap();
        assert_eq!(p.clear, Some(clip));
        assert_eq!(p.dirty, clip);
        assert_eq!(p.clip, Some(clip));
        // 命令本身不裁剪,由光栅器应用 clip。
        assert!(p.command_count() > 0);
    }

    #[test]
    fn macos_committed_layers_are_shadow_then_per_stroke() {
        let strokes = two_strokes();
        let mut params = PlanParams::new(unit_factor_view(), viewport());
        let view = params.view;
        params.shadow = Some(ShadowParams::macos(&view));
        params.outline = Some(OutlineParams::macos_committed(&view));
        let p = plan(&strokes, &params).unwrap();

        assert_eq!(p.layers.len(), 3, "1 个阴影层 + 2 个逐笔层");
        assert_eq!(p.layers[0].len(), 6, "3 档 x 2 笔");
        assert!(p.layers[0].iter().all(|c| c.pass == PaintPass::Shadow));
        // 档位外层、笔迹内层:band0 s0, band0 s1, band1 s0, ...
        assert_eq!(p.layers[0][0].stroke_index, 0);
        assert_eq!(p.layers[0][1].stroke_index, 1);
        assert_eq!(p.layers[0][2].stroke_index, 0);
        approx(p.layers[0][0].width_delta, 3.0);
        approx(p.layers[0][2].width_delta, 6.0);
        // 逐笔层:描边 -> 墨迹
        for (i, layer) in p.layers[1..].iter().enumerate() {
            let passes: Vec<PaintPass> = layer.iter().map(|c| c.pass).collect();
            assert_eq!(passes, vec![PaintPass::Outline, PaintPass::Fill]);
            assert!(layer.iter().all(|c| c.stroke_index == i));
        }
        assert_eq!(p.layers[1][0].stroke_id, 11);
        assert_eq!(p.layers[2][0].stroke_id, 12);
        assert!(p.commands().all(|c| !c.live));
    }

    #[test]
    fn windows_per_stroke_layers_group_each_stroke() {
        let strokes = two_strokes();
        let mut params = PlanParams::new(unit_factor_view(), viewport());
        params.shadow = Some(ShadowParams::windows());
        params.outline = Some(OutlineParams::windows());
        params.order = PassOrder::WindowsPerStroke;
        let p = plan(&strokes, &params).unwrap();

        assert_eq!(p.layers.len(), 2);
        for (i, layer) in p.layers.iter().enumerate() {
            let passes: Vec<PaintPass> = layer.iter().map(|c| c.pass).collect();
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
            assert!(layer.iter().all(|c| c.stroke_index == i));
        }
    }

    // ------------------------------------------------------------------
    // 宽度映射(平台规则)
    // ------------------------------------------------------------------

    #[test]
    fn shadow_widths_match_platform_spread_rules() {
        let strokes = vec![stroke(
            1,
            (1.0, 0.0, 0.0),
            &[(0.0, 0.0, 2.0, 0.0), (10.0, 0.0, 2.0, 0.1)],
        )];
        // factor = 2(zoom 1 x scale 2)
        let view = ViewTransform::new(Vec2::ZERO, 1.0, 2.0);
        let mut mac = PlanParams::new(view, viewport());
        mac.shadow = Some(ShadowParams::macos(&view));
        let mac_plan = plan(&strokes, &mac).unwrap();
        let mac_deltas: Vec<f64> = mac_plan.layers[0].iter().map(|c| c.width_delta).collect();
        for (got, want) in mac_deltas.iter().zip([6.0, 12.0, 20.0]) {
            approx(*got, want);
        }
        let mac_alphas: Vec<f64> = mac_plan.layers[0].iter().map(|c| c.alpha).collect();
        for (got, want) in mac_alphas.iter().zip([0.20, 0.13, 0.07]) {
            approx(*got, want);
        }

        let mut win = PlanParams::new(view, viewport());
        win.shadow = Some(ShadowParams::windows());
        win.order = PassOrder::WindowsPerStroke;
        let win_plan = plan(&strokes, &win).unwrap();
        let win_deltas: Vec<f64> = win_plan.layers[0]
            .iter()
            .filter(|c| c.pass == PaintPass::Shadow)
            .map(|c| c.width_delta)
            .collect();
        for (got, want) in win_deltas.iter().zip([3.0, 6.0, 10.0]) {
            approx(*got, want);
        }
    }

    #[test]
    fn outline_width_and_dash_match_platform_rules() {
        let strokes = vec![stroke(
            1,
            (0.0, 0.0, 0.0),
            &[(0.0, 0.0, 4.0, 0.0), (10.0, 0.0, 4.0, 0.1)],
        )];
        let view = ViewTransform::new(Vec2::ZERO, 1.0, 2.0); // factor 2
        let mut mac = PlanParams::new(view, viewport());
        mac.outline = Some(OutlineParams::macos_committed(&view));
        let p = plan(&strokes, &mac).unwrap();
        let outline = p.layers[0]
            .iter()
            .find(|c| c.pass == PaintPass::Outline)
            .unwrap();
        approx(outline.width_delta, 4.0); // 2 x 1.0 x factor 2
        let dash = outline.dash.unwrap();
        approx(dash.len, 2.0);
        approx(dash.phase, 0.0);

        let mut win = PlanParams::new(view, viewport());
        win.outline = Some(OutlineParams::windows());
        win.order = PassOrder::WindowsPerStroke;
        let p = plan(&strokes, &win).unwrap();
        let outline = p.layers[0]
            .iter()
            .find(|c| c.pass == PaintPass::Outline)
            .unwrap();
        approx(outline.width_delta, 2.0);
        approx(outline.dash.unwrap().len, 1.0);
        assert_eq!(OUTLINE_COLORS, [Color::BLACK, Color::WHITE]);
    }

    #[test]
    fn color_quantizes_to_rgb8_with_shared_rule() {
        assert_eq!(Color::new(1.0, 0.0, 0.0).to_rgb8(), (255, 0, 0));
        assert_eq!(Color::new(0.0, 0.0, 1.0).to_rgb8(), (0, 0, 255));
        assert_eq!(Color::new(0.5, 0.5, 0.5).to_rgb8(), (127, 127, 127));
        // 越界/NaN 按既有通道规则饱和(NaN → 0),不产出未定义 u8。
        assert_eq!(Color::new(f64::NAN, -1.0, 2.0).to_rgb8(), (0, 0, 255));
    }

    #[test]
    fn fill_width_uses_view_factor() {
        let strokes = vec![stroke(
            7,
            (0.0, 0.5, 1.0),
            &[(0.0, 0.0, 4.0, 0.0), (10.0, 0.0, 8.0, 0.1)],
        )];
        let view = ViewTransform::new(Vec2::ZERO, 2.0, 2.0); // factor 4
        let params = PlanParams::new(view, viewport());
        let p = plan(&strokes, &params).unwrap();
        approx(p.points[0].width, 16.0);
        approx(p.points[1].width, 32.0);
        let fill = p.layers[0]
            .iter()
            .find(|c| c.pass == PaintPass::Fill)
            .unwrap();
        assert_eq!(fill.color, Color::new(0.0, 0.5, 1.0));
        approx(p.effective_width(fill, 0).unwrap(), 16.0);
        approx(p.effective_width(fill, 1).unwrap(), 32.0);
        assert!(p.effective_width(fill, 2).is_none());
    }

    #[test]
    fn transform_applies_pan_zoom_scale_and_page_fit() {
        let view = ViewTransform::new(Vec2::new(10.0, 20.0), 0.5, 2.0)
            .with_page_fit(Vec2::new(30.0, 40.0), 2.0);
        approx(view.factor(), 2.0);
        approx(view.offset().x, 60.0);
        approx(view.offset().y, 80.0);
        let p0 = view.map(Vec2::new(10.0, 20.0));
        approx(p0.x, 60.0);
        approx(p0.y, 80.0);
        let p1 = view.map(Vec2::new(12.0, 25.0));
        approx(p1.x, 64.0);
        approx(p1.y, 90.0);
        approx(view.map_width(4.0), 8.0);

        let strokes = vec![stroke(
            1,
            (1.0, 0.0, 0.0),
            &[(10.0, 20.0, 4.0, 0.0), (12.0, 25.0, 4.0, 0.1)],
        )];
        let params = PlanParams::new(view, viewport());
        let p = plan(&strokes, &params).unwrap();
        approx(p.points[0].pos.x, 60.0);
        approx(p.points[0].pos.y, 80.0);
        approx(p.points[1].pos.x, 64.0);
        approx(p.points[1].pos.y, 90.0);
    }

    // ------------------------------------------------------------------
    // 单点 / 实时笔迹
    // ------------------------------------------------------------------

    #[test]
    fn committed_single_point_stroke_is_skipped() {
        let strokes = vec![stroke(1, (1.0, 0.0, 0.0), &[(5.0, 5.0, 2.0, 0.0)])];
        let params = PlanParams::new(ViewTransform::IDENTITY, viewport());
        let p = plan(&strokes, &params).unwrap();
        assert!(p.layers.is_empty());
        assert!(p.points.is_empty());
        assert_eq!(p.clear, Some(viewport()));
    }

    #[test]
    fn live_single_point_emits_dot_passes() {
        let points = [(5.0, 5.0, 2.0, 0.0)];
        let mut params = PlanParams::new(ViewTransform::IDENTITY, viewport());
        params.scope = PlanScope::MacosLive;
        params.live = Some(LiveStroke {
            points: &points,
            color: Color::new(1.0, 0.0, 0.0),
        });
        let p = plan(&[], &params).unwrap();
        assert_eq!(p.command_count(), 1);
        let fill = &p.layers[0][0];
        assert_eq!(fill.pass, PaintPass::Fill);
        assert!(fill.live);
        assert_eq!(fill.points.len(), 1);
        assert_ne!(p.dirty, Rect::EMPTY);
        assert!(p.clear.is_none());
    }

    #[test]
    fn live_stroke_is_appended_last_and_flagged() {
        let strokes = two_strokes();
        let live = [(100.0, 100.0, 4.0, 0.0), (110.0, 110.0, 4.0, 0.01)];
        let mut params = PlanParams::new(unit_factor_view(), viewport());
        params.live = Some(LiveStroke {
            points: &live,
            color: Color::new(0.0, 1.0, 0.0),
        });
        let p = plan(&strokes, &params).unwrap();
        assert_eq!(p.layers.len(), 3);
        assert!(p.layers[..2].iter().flatten().all(|c| !c.live));
        assert_eq!(p.layers[2].len(), 1);
        let last = &p.layers[2][0];
        assert!(last.live);
        assert_eq!(last.stroke_index, strokes.len());
        assert_eq!(last.stroke_id, 0);
        assert_eq!(last.color, Color::new(0.0, 1.0, 0.0));
    }

    #[test]
    fn live_outline_phase_continues_from_arc_len() {
        let live = [(20.0, 20.0, 4.0, 0.0), (30.0, 20.0, 4.0, 0.01)];
        let view = ViewTransform::new(Vec2::ZERO, 1.0, 2.0);
        let mut params = PlanParams::new(view, viewport());
        params.scope = PlanScope::MacosLive;
        params.outline = Some(OutlineParams::macos_live(2.0, 7.5));
        params.live = Some(LiveStroke {
            points: &live,
            color: Color::BLACK,
        });
        let p = plan(&[], &params).unwrap();
        let outline = p.layers[0]
            .iter()
            .find(|c| c.pass == PaintPass::Outline)
            .unwrap();
        approx(outline.width_delta, 4.0); // pad = 1 x scale 2
        let dash = outline.dash.unwrap();
        approx(dash.len, 2.0); // max(scale 2, 1)
        approx(dash.phase, 15.0); // 7.5 x scale 2
    }

    // ------------------------------------------------------------------
    // 脏矩形(两端实时规则 + 整页重建)
    // ------------------------------------------------------------------

    #[test]
    fn macos_live_dirty_pads_half_width_aa_and_shadow() {
        let live = [(50.0, 25.0, 8.0, 0.0)];
        let view = ViewTransform::new(Vec2::ZERO, 1.0, 2.0); // scale 2
        let mut params = PlanParams::new(view, viewport());
        params.scope = PlanScope::MacosLive;
        params.shadow = Some(ShadowParams::macos(&view));
        params.live = Some(LiveStroke {
            points: &live,
            color: Color::BLACK,
        });
        let p = plan(&[], &params).unwrap();
        // 表面点 (100,50), 宽 16: pad = 8 + (1.5 + 5) x 2 = 21
        rect_approx(p.dirty, 79.0, 29.0, 42.0, 42.0);
        assert!(p.clear.is_none());
    }

    #[test]
    fn windows_live_dirty_uses_ceil_radius_plus_one_plus_marks() {
        let live = [(100.4, 50.2, 5.0, 0.0)];
        let mut params = PlanParams::new(ViewTransform::IDENTITY, viewport());
        params.scope = PlanScope::WindowsLive;
        params.shadow = Some(ShadowParams::windows());
        params.outline = Some(OutlineParams::windows());
        params.live = Some(LiveStroke {
            points: &live,
            color: Color::BLACK,
        });
        let p = plan(&[], &params).unwrap();
        // r = 2.5 -> ceil 3; pad = 3 + 1 + 1 + 5 = 10; 左上截断 (100, 50)
        rect_approx(p.dirty, 90.0, 40.0, 21.0, 21.0);
    }

    #[test]
    fn incremental_dirty_unions_all_live_points() {
        let live = [(50.0, 50.0, 2.0, 0.0), (150.0, 80.0, 2.0, 0.1)];
        let mut params = PlanParams::new(ViewTransform::IDENTITY, viewport());
        params.scope = PlanScope::MacosLive;
        params.live = Some(LiveStroke {
            points: &live,
            color: Color::BLACK,
        });
        let p = plan(&[], &params).unwrap();
        // pad = 1 + 1.5 = 2.5 -> 每点 5x5, 并集 (47.5,47.5)-(152.5,82.5)
        rect_approx(p.dirty, 47.5, 47.5, 105.0, 35.0);
    }

    #[test]
    fn incremental_plan_does_not_clear() {
        let strokes = two_strokes();
        let live = [(120.0, 50.0, 2.0, 0.0), (130.0, 50.0, 2.0, 0.1)];
        let mut params = PlanParams::new(unit_factor_view(), viewport());
        params.scope = PlanScope::MacosLive;
        params.live = Some(LiveStroke {
            points: &live,
            color: Color::BLACK,
        });
        let p = plan(&strokes, &params).unwrap();
        assert!(p.clear.is_none());
        assert!(!p.dirty.is_empty());
    }

    #[test]
    fn empty_incremental_plan_has_empty_dirty() {
        let mut params = PlanParams::new(ViewTransform::IDENTITY, viewport());
        params.scope = PlanScope::WindowsLive;
        let p = plan(&[], &params).unwrap();
        assert!(p.layers.is_empty());
        assert_eq!(p.dirty, Rect::EMPTY);
        assert!(p.clear.is_none());
    }

    // ------------------------------------------------------------------
    // 橡皮(macOS 独有)
    // ------------------------------------------------------------------

    #[test]
    fn eraser_plan_emits_only_clear_commands() {
        let live = [(60.0, 60.0, 6.0, 0.0), (70.0, 60.0, 6.0, 0.01)];
        let mut params = PlanParams::new(ViewTransform::IDENTITY, viewport());
        params.scope = PlanScope::MacosLive;
        params.shadow = Some(ShadowParams::windows());
        params.outline = Some(OutlineParams::windows());
        params.eraser = true;
        params.live = Some(LiveStroke {
            points: &live,
            color: Color::BLACK,
        });
        // 已提交笔迹不参与橡皮计划(macOS 实时 CLEAR 只擦像素,不重画笔迹)。
        let p = plan(&two_strokes(), &params).unwrap();
        assert_eq!(p.layers.len(), 1);
        assert_eq!(p.command_count(), 1);
        let cmd = &p.layers[0][0];
        assert_eq!(cmd.pass, PaintPass::Erase);
        assert!(cmd.live);
        assert_eq!(cmd.stroke_index, 2);
        assert_eq!(cmd.dash, None);
        approx(cmd.width_delta, 0.0);
        assert!(p.clear.is_none());
        assert!(!p.dirty.is_empty());
    }

    #[test]
    fn eraser_rejected_in_rebuild() {
        let mut params = PlanParams::new(ViewTransform::IDENTITY, viewport());
        params.eraser = true;
        assert_eq!(plan(&[], &params).unwrap_err(), PlanError::EraserInRebuild);
    }

    // ------------------------------------------------------------------
    // 非法输入 / NaN / Inf
    // ------------------------------------------------------------------

    #[test]
    fn invalid_inputs_are_rejected() {
        let base = PlanParams::new(ViewTransform::IDENTITY, viewport());

        let mut bad_view = base.clone();
        bad_view.view.zoom = f64::NAN;
        assert_eq!(plan(&[], &bad_view).unwrap_err(), PlanError::InvalidView);

        let mut zero_scale = base.clone();
        zero_scale.view.scale = 0.0;
        assert_eq!(plan(&[], &zero_scale).unwrap_err(), PlanError::InvalidView);

        let mut bad_viewport = base.clone();
        bad_viewport.viewport.w = -1.0;
        assert_eq!(
            plan(&[], &bad_viewport).unwrap_err(),
            PlanError::InvalidViewport
        );

        let mut bad_alpha = base.clone();
        bad_alpha.alpha = f64::INFINITY;
        assert_eq!(plan(&[], &bad_alpha).unwrap_err(), PlanError::InvalidAlpha);

        let mut bad_shadow = base.clone();
        bad_shadow.shadow = Some(ShadowParams {
            bands: [
                ShadowBand {
                    spread: f64::NAN,
                    alpha: 0.20,
                },
                ShadowBand {
                    spread: 3.0,
                    alpha: 0.13,
                },
                ShadowBand {
                    spread: 5.0,
                    alpha: 0.07,
                },
            ],
        });
        assert_eq!(
            plan(&[], &bad_shadow).unwrap_err(),
            PlanError::InvalidShadowBand
        );

        let mut bad_outline = base.clone();
        bad_outline.outline = Some(OutlineParams {
            pad: 1.0,
            dash_len: 0.0,
            phase_start: 0.0,
        });
        assert_eq!(
            plan(&[], &bad_outline).unwrap_err(),
            PlanError::InvalidOutline
        );

        let mut bad_clip = base.clone();
        bad_clip.clip = Some(Rect::new(f64::NAN, 0.0, 1.0, 1.0));
        assert_eq!(plan(&[], &bad_clip).unwrap_err(), PlanError::InvalidClip);
    }

    #[test]
    fn non_finite_stroke_points_are_skipped_without_nan() {
        let strokes = vec![stroke(
            1,
            (f64::NAN, 2.0, -1.0),
            &[
                (0.0, 0.0, 4.0, 0.0),
                (f64::NAN, 5.0, 4.0, 0.1),
                (10.0, 10.0, f64::INFINITY, 0.2),
                (-3.0, -4.0, 4.0, 0.3),
                (20.0, 20.0, 4.0, 0.4),
            ],
        )];
        let params = PlanParams::new(ViewTransform::IDENTITY, viewport());
        let p = plan(&strokes, &params).unwrap();
        assert_eq!(p.points.len(), 3);
        for point in &p.points {
            assert!(point.pos.is_finite());
            assert!(point.width.is_finite());
        }
        for cmd in p.commands() {
            assert!(cmd.width_delta.is_finite());
            assert!(cmd.alpha.is_finite());
        }
        let fill = p.layers[0]
            .iter()
            .find(|c| c.pass == PaintPass::Fill)
            .unwrap();
        // 颜色通道钳制:NaN -> 0, >1 -> 1, <0 -> 0
        assert_eq!(fill.color, Color::new(0.0, 1.0, 0.0));
        assert_eq!(fill.points, 0..3);
    }

    #[test]
    fn overflow_after_transform_is_skipped() {
        let strokes = vec![stroke(
            1,
            (1.0, 0.0, 0.0),
            &[(0.0, 0.0, 2.0, 0.0), (1.0e10, 1.0e10, 2.0, 0.1)],
        )];
        let mut params = PlanParams::new(ViewTransform::IDENTITY, viewport());
        params.view.zoom = 1.0e300; // 映射后溢出为 inf
        let p = plan(&strokes, &params).unwrap();
        assert!(p.points.len() <= 1);
        assert!(p.layers.is_empty());
        for point in &p.points {
            assert!(point.pos.is_finite());
        }
    }

    // ------------------------------------------------------------------
    // alpha / 开关组合表
    // ------------------------------------------------------------------

    #[test]
    fn alpha_scales_pass_opacity() {
        let strokes = vec![two_strokes().remove(0)];
        let mut params = PlanParams::new(ViewTransform::IDENTITY, viewport());
        params.shadow = Some(ShadowParams::windows());
        params.outline = Some(OutlineParams::windows());
        params.alpha = 0.5;
        let p = plan(&strokes, &params).unwrap();
        let shadow = p.layers[0]
            .iter()
            .find(|c| c.pass == PaintPass::Shadow)
            .unwrap();
        approx(shadow.alpha, 0.5 * 0.20);
        let outline = p.layers[1]
            .iter()
            .find(|c| c.pass == PaintPass::Outline)
            .unwrap();
        approx(outline.alpha, 0.5);
        let fill = p.layers[1]
            .iter()
            .find(|c| c.pass == PaintPass::Fill)
            .unwrap();
        approx(fill.alpha, 0.5);
    }

    #[test]
    fn zero_alpha_emits_no_commands_but_still_clears() {
        let mut params = PlanParams::new(ViewTransform::IDENTITY, viewport());
        params.shadow = Some(ShadowParams::windows());
        params.alpha = 0.0;
        let p = plan(&two_strokes(), &params).unwrap();
        assert!(p.layers.is_empty());
        assert_eq!(p.clear, Some(viewport()));
        assert_eq!(p.dirty, viewport());

        let mut negative = params.clone();
        negative.alpha = -3.0;
        let p = plan(&two_strokes(), &negative).unwrap();
        assert!(p.layers.is_empty());
    }

    #[test]
    fn switch_toggle_table_matches_expected_passes() {
        for (shadow, outline) in [(false, false), (true, false), (false, true), (true, true)] {
            let strokes = two_strokes();
            let mut params = PlanParams::new(unit_factor_view(), viewport());
            let view = params.view;
            if shadow {
                params.shadow = Some(ShadowParams::macos(&view));
            }
            if outline {
                params.outline = Some(OutlineParams::macos_committed(&view));
            }
            let p = plan(&strokes, &params).unwrap();

            let expected_layers = 2 + usize::from(shadow);
            assert_eq!(
                p.layers.len(),
                expected_layers,
                "shadow={shadow} outline={outline}"
            );
            let expected_commands = 2 * (1 + usize::from(outline)) + if shadow { 6 } else { 0 };
            assert_eq!(p.command_count(), expected_commands);
            assert_eq!(
                p.commands()
                    .filter(|c| c.pass == PaintPass::Outline)
                    .count(),
                2 * usize::from(outline)
            );
            assert_eq!(
                p.commands().filter(|c| c.pass == PaintPass::Fill).count(),
                2
            );
            if shadow {
                assert_eq!(p.layers[0].len(), 6);
            }
        }
    }

    // ------------------------------------------------------------------
    // 特征化快照(锁死当前 core/Windows 的真实命令序列;D9b/D9c 的回归基线)
    // ------------------------------------------------------------------

    #[test]
    fn characterization_macos_committed_command_sequence() {
        let strokes = two_strokes();
        let mut params = PlanParams::new(unit_factor_view(), viewport());
        let view = params.view;
        params.shadow = Some(ShadowParams::macos(&view));
        params.outline = Some(OutlineParams::macos_committed(&view));
        let p = plan(&strokes, &params).unwrap();

        // 依据当前 core export/mod.rs:663-754 的真实顺序:
        // 阴影层(3 档 x 2 笔, 档位外层)-> s0(描边, 墨迹)-> s1(描边, 墨迹)。
        assert_eq!(
            snapshot(&p),
            vec![
                "L0 Shadow stroke=0 live=false pts=0..2 dw=3.00 a=0.20 dash=-",
                "L0 Shadow stroke=1 live=false pts=2..4 dw=3.00 a=0.20 dash=-",
                "L0 Shadow stroke=0 live=false pts=0..2 dw=6.00 a=0.13 dash=-",
                "L0 Shadow stroke=1 live=false pts=2..4 dw=6.00 a=0.13 dash=-",
                "L0 Shadow stroke=0 live=false pts=0..2 dw=10.00 a=0.07 dash=-",
                "L0 Shadow stroke=1 live=false pts=2..4 dw=10.00 a=0.07 dash=-",
                "L1 Outline stroke=0 live=false pts=0..2 dw=2.00 a=1.00 dash=1.00/0.00",
                "L1 Fill stroke=0 live=false pts=0..2 dw=0.00 a=1.00 dash=-",
                "L2 Outline stroke=1 live=false pts=2..4 dw=2.00 a=1.00 dash=1.00/0.00",
                "L2 Fill stroke=1 live=false pts=2..4 dw=0.00 a=1.00 dash=-",
            ]
        );
    }

    #[test]
    fn characterization_windows_per_stroke_command_sequence() {
        let strokes = two_strokes();
        let mut params = PlanParams::new(unit_factor_view(), viewport());
        params.shadow = Some(ShadowParams::windows());
        params.outline = Some(OutlineParams::windows());
        params.order = PassOrder::WindowsPerStroke;
        let p = plan(&strokes, &params).unwrap();

        // 依据当前 Windows pen.rs:56-106 的真实顺序:每笔
        // 三档阴影 -> 描边 -> 墨迹。
        assert_eq!(
            snapshot(&p),
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
    }
}
