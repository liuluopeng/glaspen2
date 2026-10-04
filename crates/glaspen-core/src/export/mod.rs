//! FFI 大本营:ObjC / Windows 覆盖层 / 面板(FRB)调用的一切入口。
//!
//! 体量原因拆分为子模块(全部在本 crate 内,FFI 符号名不变):
//! - [`pages`]      活页本:页管理/新建守卫/导航/列表
//! - [`chat_glue`]  手写消息:⌘⌃2 草稿、⌘⌃3 直发、共享上行、涂鸦身份
//! - [`media`]      文件导出:XOJ / PNG / SVG / GIF / PDF
//! - [`thumbs`]     缩略图与画布总览渲染
//!
//! 本文件保留:绘图与模型器 FFI、描边/镜头状态、设置持久化 FFI、
//! 开机自启、公共小工具(free_c_string / leak_png 等)。

pub(crate) use crate::{
    RAW_STROKE_START, Stroke, desktop_path, modeler, pressure_to_width, runtime, state,
    timestamped_name, timestamped_path,
};
pub use crate::{STROKES, db};
pub use std::ffi::{CStr, CString};
pub use std::os::raw::{c_char, c_double, c_int, c_uchar};
pub(crate) use std::path::PathBuf;
pub use std::slice;
pub use std::sync::Arc;

mod chat_glue;
mod media;
mod pages;
pub(crate) mod thumbs;
pub(crate) use chat_glue::ink_draft_on_stroke_committed;
pub(crate) use chat_glue::ink_share_on_stroke_committed;
pub use chat_glue::*;
#[cfg(test)]
pub(crate) use chat_glue::{CHAT_NOTEBOOK, stroke_to_chat_message};
pub use media::*;
#[cfg(test)]
pub(crate) use media::{GifStroke, build_svg_from, encode_animated_gif};
#[cfg(test)]
pub(crate) use pages::plan_new_page;
pub use pages::*;
pub use thumbs::*;
pub use thumbs::{THUMB_BLOB_MAGIC, encode_thumb_blob, page_thumbnails_blob, warm_thumbnail_cache};

/// 批量补全所有缺 OCR 结果的页面(阻塞, 逐页调 axum 服务)。
/// 供面板/菜单后续接入; 无调用方时保持 FFI 导出以便调试。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_ocr_backfill_all() -> c_int {
    crate::ocr::backfill_missing() as c_int
}

// ---------------------------------------------------------------------------
// Drawing FFI (legacy, non-modeler path)
// ---------------------------------------------------------------------------

/// cairo ARGB32 是**预乘 alpha**(内存序 BGRA): 半透明的抗锯齿边缘像素
/// 若直接当不透明色使用会偏暗 —— 表现为笔迹四周一圈"黑色描边"。
/// 就地反预乘(直线 alpha), 使边缘像素呈现笔迹本色; alpha 本身不变。
pub(crate) fn unpremultiply_rgba(buf: &mut [u8]) {
    for px in buf.as_chunks_mut::<4>().0 {
        let a = px[3] as u32;
        if (1..255).contains(&a) {
            for c in &mut px[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a) as u8;
            }
        }
    }
}

/// Keep points whose distance from the last kept point exceeds `min_dist`,
/// or whose width changed by more than `width_ratio` from the last kept width.
/// Preserves stroke shape while bounding point count in long-running sessions.
pub(crate) fn decimate(points: &[(f64, f64, f64, f64)]) -> Vec<(f64, f64, f64, f64)> {
    if points.len() <= 4 {
        return points.to_vec();
    }
    let min_dist = 0.8f64;
    let width_ratio = 0.12f64;
    let mut out: Vec<(f64, f64, f64, f64)> = Vec::with_capacity(points.len() / 2 + 2);
    let mut last: (f64, f64, f64, f64) = points[0];
    out.push(last);
    for &p in points.iter().skip(1) {
        let dx = p.0 - last.0;
        let dy = p.1 - last.1;
        let dw = (p.2 - last.2).abs() / last.2.max(1e-6);
        if dx * dx + dy * dy >= min_dist * min_dist || dw > width_ratio {
            out.push(p);
            last = p;
        }
    }
    if *out.last().unwrap() != *points.last().unwrap() {
        out.push(*points.last().unwrap());
    }
    out
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_begin_stroke(
    r: c_double,
    g: c_double,
    b: c_double,
    width_scale: c_double,
) {
    let id = runtime().block_on(db::begin_stroke(r, g, b, width_scale));
    let mut strokes = STROKES.lock().unwrap();
    strokes.push(Stroke {
        id,
        r,
        g,
        b,
        points: Vec::new(),
    });
    *RAW_STROKE_START.lock().unwrap() = None;
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_add_point(x: c_double, y: c_double, width: c_double) {
    glaspen2_add_point_t(x, y, width, 0.0);
}

/// Variant with the point's relative time (seconds from stroke start).
/// The animated GIF replay builds its timeline from these times — points
/// recorded with t=0 all collapse to zero-duration segments and the export
/// comes out empty, so drawing paths must use this variant.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_add_point_t(x: c_double, y: c_double, width: c_double, t: c_double) {
    let mut strokes = STROKES.lock().unwrap();
    if let Some(stroke) = strokes.last_mut() {
        stroke.points.push((x, y, width, t));
    }
    state::buffer_point(x, y, width, t); // sync
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_end_stroke() {
    db::end_stroke_spawned();
}

/// 描边(轮廓)渲染开关 —— 纯渲染设置,只在内存,不落库、重启即恢复关闭。
static STROKE_OUTLINE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// 描边比笔迹宽出的半径(px)。描边层 = 同路径加宽 2×OUTLINE 后置于笔迹之下。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // 仅 macOS 渲染/导出路径消费; Windows 用覆盖层自己的实现
const OUTLINE_PAD: f64 = 1.0;

/// 按笔色亮度选对比描边色(与 Windows contrast_color 同参数:BT.601,阈值 128)。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // 仅 macOS 渲染/导出路径消费; Windows 用覆盖层自己的实现
fn outline_contrast_color(r: f64, g: f64, b: f64) -> (u8, u8, u8) {
    let lum = 0.299 * r + 0.587 * g + 0.114 * b;
    if lum > 0.5 {
        (0, 0, 0)
    } else {
        (255, 255, 255)
    }
}

/// 开关描边渲染(仅当前会话生效)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_set_stroke_outline(enabled: c_int) {
    STROKE_OUTLINE.store(enabled != 0, std::sync::atomic::Ordering::SeqCst);
}

// ── 无限画布:视口变换 ──
// 视图 = (画布坐标 − pan) × zoom。笔迹以画布坐标存储(可为负/超界),
// 渲染时减 pan 乘 zoom。zoom ∈ (0,1],上限 100% 防蚂蚁大小涂鸦。
// 翻页模式下 pan=0、zoom=1,行为与从前一致。
static VIEW_PAN_X: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static VIEW_PAN_Y: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static VIEW_ZOOM: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // 仅 macOS 渲染/导出路径消费; Windows 用覆盖层自己的实现
fn view_transform() -> (f64, f64, f64) {
    use std::sync::atomic::Ordering;
    (
        f64::from_bits(VIEW_PAN_X.load(Ordering::SeqCst)),
        f64::from_bits(VIEW_PAN_Y.load(Ordering::SeqCst)),
        {
            let z = f64::from_bits(VIEW_ZOOM.load(Ordering::SeqCst));
            if z > 0.0 { z } else { 1.0 }
        },
    )
}

/// 设置渲染视口变换(macOS rebuild 用)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_set_view_transform(pan_x: c_double, pan_y: c_double, zoom: c_double) {
    use std::sync::atomic::Ordering;
    VIEW_PAN_X.store(pan_x.to_bits(), Ordering::SeqCst);
    VIEW_PAN_Y.store(pan_y.to_bits(), Ordering::SeqCst);
    VIEW_ZOOM.store(zoom.to_bits(), Ordering::SeqCst);
}

/// 切换当前画布使用的存储:0 = 翻页模式,1 = 无限画布。
/// 调用方负责在切换前后 flush 笔画并重新载入对应画布的笔迹。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_set_canvas_kind(infinite: c_int) {
    state::set_canvas_kind(if infinite != 0 {
        state::CanvasKind::Infinite
    } else {
        state::CanvasKind::Page
    });
}

/// 载入全局唯一的无限画布笔迹到 STROKES。返回笔迹数。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_load_infinite_strokes() -> c_int {
    // 先把仍在排队的抬笔冲刷掉,避免与异步落点竞争。
    runtime().block_on(db::end_stroke());
    let data = runtime().block_on(db::load_infinite_strokes());
    let count = data.len() as c_int;
    let mut strokes = STROKES.lock().unwrap();
    strokes.clear();
    for s in data {
        strokes.push(Stroke {
            id: s.id,
            r: s.r,
            g: s.g,
            b: s.b,
            points: s.points,
        });
    }
    count
}

/// 保存无限画布镜头变换(全局唯一,存 user_settings)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_set_infinite_transform(
    pan_x: c_double,
    pan_y: c_double,
    zoom: c_double,
) {
    runtime().block_on(db::set_infinite_transform(pan_x, pan_y, zoom));
}

/// 读取无限画布镜头变换。未设置时回原点 + 100%。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_get_infinite_transform(
    x: *mut c_double,
    y: *mut c_double,
    z: *mut c_double,
) {
    let (px, py, pz) = runtime()
        .block_on(db::get_infinite_transform())
        .unwrap_or((0.0, 0.0, 1.0));
    unsafe {
        *x = px;
        *y = py;
        *z = pz;
    }
}

/// Re-render every stroke from STROKES onto the ObjC-owned cairo surface.
/// Used on undo, page navigation, display changes and the rainbow toggle.
/// Coordinates are scaled by `scale` (retina factor); the surface is an
/// external cairo image surface owned by ObjC.
#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_draw_rebuild(surface_ptr: *mut std::ffi::c_void, scale: c_double) {
    draw_rebuild_impl(surface_ptr, scale, 0.0, 0.0, 1.0);
}

/// scale-to-fit 观看:页几何 ≠ 当前屏幕时, 壳层按等比缩放+居中把页画进
/// 视口。`page_scale/ox/oy` 是"页像素 → 屏幕逻辑点"的变换; fit 模式下
/// (pscale ≠ 1)跨页邻接与页号标注停用 —— 那是滑动翻页时代的视觉,
/// 快照观看时一页就是一页。
#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_draw_rebuild_view(
    surface_ptr: *mut std::ffi::c_void,
    scale: c_double,
    page_ox: c_double,
    page_oy: c_double,
    page_scale: c_double,
) {
    draw_rebuild_impl(surface_ptr, scale, page_ox, page_oy, page_scale);
}

#[cfg(target_os = "macos")]
fn draw_rebuild_impl(
    surface_ptr: *mut std::ffi::c_void,
    scale: c_double,
    page_ox: c_double,
    page_oy: c_double,
    page_scale: c_double,
) {
    let Some(r) = crate::cairo_dl::CairoRenderer::from_surface(surface_ptr) else {
        return;
    };
    r.clear();
    let (pan_x, pan_y, zoom) = view_transform();
    // 页视图变换折叠进既有 pan/zoom 管线:
    //   目标 screen = (canvas - pan)*zoom*pscale + off
    //   paint 给出  = (canvas - pan_eff)*zoom_eff*scale
    //   → zoom_eff = zoom*pscale, pan_eff = pan - off/(zoom*pscale)
    let fit = page_scale != 1.0;
    let zoom_eff = zoom * page_scale;
    let pan_x_eff = pan_x - page_ox / (zoom * page_scale);
    let pan_y_eff = pan_y - page_oy / (zoom * page_scale);
    let outline = STROKE_OUTLINE.load(std::sync::atomic::Ordering::SeqCst);
    let strokes = STROKES.lock().unwrap();

    // 主页笔迹
    paint_strokes_into(
        &r,
        &strokes.iter().map(stroke_points).collect::<Vec<_>>(),
        pan_x_eff,
        pan_y_eff,
        zoom_eff,
        scale,
        0.0,
        outline,
        1.0,
    );

    // 活页本跨页显示:相邻两页的笔迹画在本页上下(视口滑出页界时可见)。
    // 仅翻页模式(无限画布全局只有一张,无邻页);fit 观看时停用。
    let cur = crate::state::current_screen_id();
    if !fit && cur > 0 && crate::state::canvas_kind() != crate::state::CanvasKind::Infinite {
        let neighbors = runtime().block_on(async {
            let mut out: Vec<(f64, Vec<Stroke>)> = Vec::new();
            for (dy, id) in [
                (-1.0f64, db::prev_screen(cur).await),
                (1.0f64, db::next_screen(cur).await),
            ] {
                if let Some(id) = id {
                    let sts = db::strokes_for_screen(id).await;
                    if !sts.is_empty() {
                        out.push((
                            dy,
                            sts.into_iter()
                                .map(|s| Stroke {
                                    id: s.id,
                                    r: s.r,
                                    g: s.g,
                                    b: s.b,
                                    points: s.points,
                                })
                                .collect(),
                        ));
                    }
                }
            }
            out
        });
        let stride = runtime().block_on(db::page_height(cur)).unwrap_or(0.0);
        for (dy, group) in &neighbors {
            paint_strokes_into(
                &r,
                &group.iter().map(stroke_points).collect::<Vec<_>>(),
                pan_x_eff,
                pan_y_eff,
                zoom_eff,
                scale,
                *dy * stride,
                outline,
                1.0,
            );
        }
        // 页号跟随:各页区域顶部标注页号(滑动跨页时知道自己在哪)
        if let Some(info) = runtime().block_on(db::page_info(cur)) {
            let cur_ord = info.2; // 全局位置(1 起)
            let label = |shift: f64, text: String| {
                r.draw_text(
                    (20.0 - pan_x_eff) * zoom_eff * scale,
                    (44.0 + shift - pan_y_eff) * zoom_eff * scale,
                    22.0 * zoom_eff * scale,
                    (150, 146, 138),
                    &text,
                );
            };
            if neighbors.iter().any(|(dy, _)| *dy < 0.0) {
                let n = format!("第 {} 页", cur_ord - 1);
                label(-stride + 60.0 * zoom * scale, n);
            }
            label(0.0, format!("第 {} 页", cur_ord));
            if neighbors.iter().any(|(dy, _)| *dy > 0.0) {
                let n = format!("第 {} 页", cur_ord + 1);
                label(stride + 60.0 * zoom * scale, n);
            }
        }
    }

    drop(strokes);
    r.flush();
}

/// 把 `glaspen2_load_page_preview` 载入的页画进外部 cairo 表面。
///
/// 与画布渲染同一条坐标管线:笔迹(页几何像素)→ `pan/zoom` → scale-to-fit
/// (`ox/oy/pscale`)→ 表面像素(× `scale`)。`alpha` 是整页不透明度
/// (隧道卡片的渐显/渐隐),`white_bg` 供导出用。返回 1 成功, 0 = 绑定失败。
pub(crate) fn render_page_into(
    r: &crate::cairo_dl::CairoRenderer,
    strokes: &[db::StrokeData],
    scale: f64,
    ox: f64,
    oy: f64,
    pscale: f64,
    page_w: f64,
    page_h: f64,
    alpha: f64,
    white_bg: bool,
    depth: f64,
) -> c_int {
    if scale <= 0.0 {
        return 0;
    }
    let alpha = alpha.clamp(0.0, 1.0);
    let views: Vec<crate::pagerender::PageStroke<'_>> = strokes
        .iter()
        .map(|s| crate::pagerender::PageStroke {
            r: s.r,
            g: s.g,
            b: s.b,
            points: &s.points,
        })
        .collect();
    let (pan_x, pan_y, zoom) = view_transform();
    // 目标像素 = (页像素 × pscale + off) × scale;折叠成布局:
    // 缩放 = zoom×pscale×scale, 平移 = (−pan×zoom×pscale + off)×scale。
    let lay = crate::pagerender::PageLayout {
        scale: zoom * pscale * scale,
        ox: (-pan_x * zoom * pscale + ox) * scale,
        oy: (-pan_y * zoom * pscale + oy) * scale,
        w: 0.0,
        h: 0.0,
    };
    let mut data = vec![0u8; (r.w.max(0) as usize) * (r.h.max(0) as usize) * 4];
    let stride = r.w.max(0) as usize * 4;
    {
        let mut surf = crate::pagerender::PageSurface {
            data: &mut data,
            w: r.w,
            h: r.h,
            stride,
        };
        // scale-to-fit 时页外压暗(与 pageview_frame_draw 同一视觉):
        // 快照卡片看着就是"贴在深色卡纸上的一页纸"。页几何(点)× pscale
        // = 页在视口里的尺寸, 与壳层 pageview_update 的定义一致。
        if pscale != 1.0 && page_w > 0.0 && page_h > 0.0 {
            surf.dim_outside(
                ox * scale,
                oy * scale,
                page_w * pscale * scale,
                page_h * pscale * scale,
                0.42,
            );
        }
        crate::pagerender::render_strokes_with_outline(
            &mut surf,
            &lay,
            &views,
            0.0,
            0.0,
            1.0,
            &crate::pagerender::RenderOpts { alpha, white_bg },
            STROKE_OUTLINE.load(std::sync::atomic::Ordering::SeqCst),
        );
        // 磨砂玻璃化(depth >= 0): 页快照变成"磨砂玻璃上的墨迹",
        // 越深越暗 → 隧道卡片的层次/透视一眼可读。depth < 0 = 不磨砂。
        // 磨砂已按用户要求停用: prev/next 玻璃片 = 纯半透明玻璃底
        // (无模糊), 墨迹原样浮在其上。frost_and_tint 保留备用。
        let _ = depth;
    }
    r.blit_bgra(&data, r.w, r.h, stride);
    1
}

/// 一条笔迹的可渲染视图:(颜色, 点列)。点列 = (x, y, width, t)。
/// 用 &[(f64,f64,f64,f64)] 切片做统一源 —— STROKES / 邻页 / 页预览缓冲
/// 三种来源都能零拷贝或一次收集后走同一条绘制管线。
pub(crate) struct StrokeView<'a> {
    pub(crate) r: f64,
    pub(crate) g: f64,
    pub(crate) b: f64,
    pub(crate) points: &'a [(f64, f64, f64, f64)],
}

/// `Stroke` → [`StrokeView`](零拷贝)。
pub(crate) fn stroke_points(s: &Stroke) -> StrokeView<'_> {
    StrokeView {
        r: s.r,
        g: s.g,
        b: s.b,
        points: &s.points,
    }
}

/// 把一组笔迹画到 renderer 上(支持跨页 y 偏移、描边层与整体不透明度)。
///
/// 坐标管线:像素 = (画布坐标 − pan) × zoom × scale + y_shift。
/// `alpha < 1` 时逐段软件合成(cairo 的 set_source_rgba 在同一表面的多次
/// 绘制间不累积 alpha, 隧道卡片的渐显/渐隐只能在像素上做)。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // 仅 macOS 渲染/导出路径消费; Windows 用覆盖层自己的实现
pub(crate) fn paint_strokes_into(
    r: &crate::cairo_dl::CairoRenderer,
    strokes: &[StrokeView<'_>],
    pan_x: f64,
    pan_y: f64,
    zoom: f64,
    scale: f64,
    y_shift: f64,
    outline: bool,
    alpha: f64,
) {
    let alpha = alpha.clamp(0.0, 1.0);
    if alpha <= 0.0 {
        return;
    }
    // 整体不透明度 ≠ 1: 走软件合成(独立缓冲 + blit), 保证渐显/渐隐均匀。
    if alpha < 1.0 {
        paint_strokes_soft(
            r, strokes, pan_x, pan_y, zoom, scale, y_shift, outline, alpha,
        );
        return;
    }
    for s in strokes {
        let pts = s.points;
        if pts.len() < 2 {
            continue;
        }
        let color = (
            (s.r.clamp(0.0, 1.0) * 255.0) as u8,
            (s.g.clamp(0.0, 1.0) * 255.0) as u8,
            (s.b.clamp(0.0, 1.0) * 255.0) as u8,
        );
        // 描边层:同路径加宽 + 对比色,先画(垫在笔迹之下)
        if outline {
            let ol = outline_contrast_color(s.r, s.g, s.b);
            for i in 0..pts.len() {
                let (x, y, w, _t) = pts[i];
                if i == 0 {
                    r.fill_circle(
                        ((x - pan_x) * zoom * scale) as f32,
                        ((y + y_shift - pan_y) * zoom * scale) as f32,
                        ((w * 0.5 + OUTLINE_PAD) * zoom * scale) as f32,
                        ol,
                    );
                } else {
                    let (px, py, _pw, _pt) = pts[i - 1];
                    r.stroke_line(
                        ((px - pan_x) * zoom * scale) as f32,
                        ((py + y_shift - pan_y) * zoom * scale) as f32,
                        ((x - pan_x) * zoom * scale) as f32,
                        ((y + y_shift - pan_y) * zoom * scale) as f32,
                        ((w + OUTLINE_PAD * 2.0) * zoom * scale) as f32,
                        ol,
                    );
                }
            }
        }
        for i in 0..pts.len() {
            let (x, y, w, _t) = pts[i];
            if i == 0 {
                // 起点实心圆点(圆帽)
                r.fill_circle(
                    ((x - pan_x) * zoom * scale) as f32,
                    ((y + y_shift - pan_y) * zoom * scale) as f32,
                    (w * 0.5 * zoom * scale) as f32,
                    color,
                );
            } else {
                let (px, _py, _pw, _pt) = pts[i - 1];
                r.stroke_line(
                    ((px - pan_x) * zoom * scale) as f32,
                    ((y + y_shift - pan_y) * zoom * scale) as f32,
                    ((x - pan_x) * zoom * scale) as f32,
                    ((y + y_shift - pan_y) * zoom * scale) as f32,
                    (w * zoom * scale) as f32,
                    color,
                );
            }
        }
    }
}

/// 整体不透明度合成:笔迹画进独立缓冲(逐段软件 alpha 混合), 再 blit 到
/// 目标表面。描边层同样按 alpha 合成, 保持与不透明渲染一致的层次。
fn paint_strokes_soft(
    r: &crate::cairo_dl::CairoRenderer,
    strokes: &[StrokeView<'_>],
    pan_x: f64,
    pan_y: f64,
    zoom: f64,
    scale: f64,
    y_shift: f64,
    outline: bool,
    alpha: f64,
) {
    let w = r.w.max(0) as usize;
    let h = r.h.max(0) as usize;
    let stride = w * 4;
    let mut data = vec![0u8; stride * h];
    let views: Vec<crate::pagerender::PageStroke<'_>> = strokes
        .iter()
        .map(|s| crate::pagerender::PageStroke {
            r: s.r,
            g: s.g,
            b: s.b,
            points: s.points,
        })
        .collect();
    {
        let mut surf = crate::pagerender::PageSurface {
            data: &mut data,
            w: r.w,
            h: r.h,
            stride,
        };
        // 布局把"页坐标 → 表面像素"整条管线折叠进来: 缩放量 = zoom×scale,
        // 平移量 = −pan×zoom×scale + y_shift×scale。
        let lay = crate::pagerender::PageLayout {
            scale: zoom * scale,
            ox: -pan_x * zoom * scale,
            oy: (-pan_y + y_shift) * zoom * scale,
            w: 0.0,
            h: 0.0,
        };
        crate::pagerender::render_strokes_with_outline(
            &mut surf,
            &lay,
            &views,
            0.0,
            0.0,
            1.0,
            &crate::pagerender::RenderOpts {
                alpha,
                white_bg: false,
            },
            outline,
        );
    }
    r.blit_bgra(&data, r.w, r.h, stride);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_modeler_begin(
    r: c_double,
    g: c_double,
    b: c_double,
    x: c_double,
    y: c_double,
    pressure: c_double,
    timestamp: c_double,
    width_scale: c_double,
) {
    modeler::begin_stroke(x, y, pressure, timestamp, width_scale);
    *RAW_STROKE_START.lock().unwrap() = Some(timestamp);
    // Start DB stroke with correct color
    let id = runtime().block_on(db::begin_stroke(r, g, b, width_scale));
    state::buffer_point(x, y, pressure_to_width(pressure, width_scale), 0.0); // sync
    // Start STROKES entry
    let mut strokes = STROKES.lock().unwrap();
    strokes.push(Stroke {
        id,
        r,
        g,
        b,
        points: Vec::new(),
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_modeler_move(
    x: c_double,
    y: c_double,
    pressure: c_double,
    timestamp: c_double,
    width_scale: c_double,
) {
    modeler::pen_move(x, y, pressure, timestamp, width_scale);
    let start = RAW_STROKE_START.lock().unwrap().unwrap_or(timestamp);
    state::buffer_point(
        x,
        y,
        pressure_to_width(pressure, width_scale),
        timestamp - start,
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_modeler_end(
    x: c_double,
    y: c_double,
    pressure: c_double,
    timestamp: c_double,
    width_scale: c_double,
) {
    modeler::end_stroke(x, y, pressure, timestamp, width_scale);
    let start = RAW_STROKE_START.lock().unwrap().unwrap_or(timestamp);
    state::buffer_point(
        x,
        y,
        pressure_to_width(pressure, width_scale),
        timestamp - start,
    ); // sync
    db::end_stroke_spawned();
}

/// Commit the modeler buffer into STROKES. Call after drawing the buffer.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_modeler_commit_to_strokes(r: c_double, g: c_double, b: c_double) {
    {
        let smoothed = modeler::take_buffer();
        let mut strokes = STROKES.lock().unwrap();
        if let Some(last) = strokes.last_mut() {
            last.r = r;
            last.g = g;
            last.b = b;

            for (sx, sy, sw, st) in smoothed {
                last.points.push((sx, sy, sw, st));
            }
            // Bound point count for long-running sessions.
            last.points = decimate(&last.points);
        }
    } // 先放掉 STROKES 锁再走草稿钩子(钩子内部要重新拿锁)
    ink_draft_on_stroke_committed();
    ink_share_on_stroke_committed();
    crate::ocr::on_stroke_committed(crate::state::current_screen_id());
}

/// Windows 直写路径(begin_stroke/add_point_t/end_stroke)的抬笔钩子扇出。
/// macOS 走 modeler_commit_to_strokes(内含同一组钩子);Windows 的平滑在
/// 覆盖层自己的 StrokeModeler 实例里完成,核心缓冲为空,故抬笔后单独触发:
/// 草稿通道推送 / 共享画布上行 / OCR 闲时登记,与 macOS 语义一致。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_notify_stroke_committed() {
    ink_draft_on_stroke_committed();
    ink_share_on_stroke_committed();
    crate::ocr::on_stroke_committed(crate::state::current_screen_id());
}

/// Eraser: remove strokes overlapped by the just-finished eraser stroke.
/// The eraser stroke itself produced no DB points; its pending DB row is
/// deleted here along with any hit strokes (memory + DB).
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_modeler_erase_finish() {
    let eraser_pts = modeler::take_buffer();

    let removed_ids: Vec<i64> = {
        let mut strokes = STROKES.lock().unwrap();
        let mut removed = Vec::new();
        strokes.retain(|s| {
            let hit = if eraser_pts.is_empty() {
                false
            } else {
                stroke_intersects_eraser(s, &eraser_pts)
            };
            if hit {
                removed.push(s.id);
            }
            !hit
        });
        removed
    };

    // Discard the eraser stroke's own DB row (it has no points).
    // Deletes run synchronously so an immediate page-nav/undo can't see
    // strokes that should already be gone.
    if let Some(id) = state::take_pending_stroke_id() {
        // clear any buffered points so they never flush under a stale id
        state::take_pending();
        runtime().block_on(db::delete_stroke_by_id(id));
    }

    for id in removed_ids {
        runtime().block_on(db::delete_stroke_by_id(id));
    }
}

/// True if the eraser path (points with width) touches the stroke.
fn stroke_intersects_eraser(stroke: &Stroke, eraser_pts: &[(f64, f64, f64, f64)]) -> bool {
    for &(ex, ey, ew, _) in eraser_pts {
        for &(px, py, pw, _) in &stroke.points {
            let dx = px - ex;
            let dy = py - ey;
            let r = (ew + pw) * 0.5;
            if dx * dx + dy * dy <= r * r {
                return true;
            }
        }
    }
    false
}

/// Get the number of smoothed points available after the last modeler call.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_modeler_point_count() -> c_int {
    modeler::buffer_len() as c_int
}

/// Get a smoothed point by index (for macOS ObjC to read back).
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_modeler_get_point(
    idx: c_int,
    x: *mut c_double,
    y: *mut c_double,
    w: *mut c_double,
) {
    if let Some((px, py, pw, _pt)) = modeler::get_buffer_point(idx as usize) {
        unsafe {
            *x = px;
            *y = py;
            *w = pw;
        }
    }
}

/// Clear the modeler buffer (call after platform has read and drawn all points).
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_modeler_clear_buffer() {
    modeler::clear_buffer();
}

// ---------------------------------------------------------------------------
// Page navigation
// ---------------------------------------------------------------------------

/// Load strokes from DB into STROKES for a given screen. Returns stroke count.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_stroke_count() -> c_int {
    STROKES.lock().unwrap().len() as c_int
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_get_stroke_point_count(idx: c_int) -> c_int {
    let strokes = STROKES.lock().unwrap();
    strokes
        .get(idx as usize)
        .map_or(0, |s| s.points.len() as c_int)
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_get_stroke_color(
    idx: c_int,
    r: *mut c_double,
    g: *mut c_double,
    b: *mut c_double,
) {
    let strokes = STROKES.lock().unwrap();
    if let Some(s) = strokes.get(idx as usize) {
        unsafe {
            *r = s.r;
            *g = s.g;
            *b = s.b;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_get_stroke_avg_width(idx: c_int) -> c_double {
    let strokes = STROKES.lock().unwrap();
    strokes.get(idx as usize).map_or(1.0, |s| s.avg_width())
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_get_stroke_point(
    idx: c_int,
    pidx: c_int,
    x: *mut c_double,
    y: *mut c_double,
) {
    let strokes = STROKES.lock().unwrap();
    if let Some(s) = strokes.get(idx as usize)
        && let Some(&(px, py, _, _)) = s.points.get(pidx as usize)
    {
        unsafe {
            *x = px;
            *y = py;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_get_stroke_point_width(idx: c_int, pidx: c_int) -> c_double {
    let strokes = STROKES.lock().unwrap();
    strokes
        .get(idx as usize)
        .and_then(|s| s.points.get(pidx as usize))
        .map_or(1.0, |p| p.2)
}

// ---------------------------------------------------------------------------
// Xournal save (.xoj)
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_save_settings(
    r: c_double,
    g: c_double,
    b: c_double,
    width_scale: c_double,
) {
    runtime().block_on(db::save_settings(r, g, b, width_scale));
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_load_settings_parts(
    r: *mut c_double,
    g: *mut c_double,
    b: *mut c_double,
    w: *mut c_double,
) -> c_int {
    match runtime().block_on(db::load_settings()) {
        Some((rr, gg, bb, ww)) => {
            unsafe {
                *r = rr;
                *g = gg;
                *b = bb;
                *w = ww;
            }
            1
        }
        None => 0,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_save_bool_setting(key: *const c_char, val: c_int) {
    if key.is_null() {
        return;
    }
    let k = unsafe { CStr::from_ptr(key) }.to_str().unwrap_or("");
    runtime().block_on(db::save_setting(k, if val != 0 { "1" } else { "0" }));
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_load_bool_setting(key: *const c_char) -> c_int {
    if key.is_null() {
        return 0;
    }
    let k = unsafe { CStr::from_ptr(key) }.to_str().unwrap_or("");
    runtime()
        .block_on(db::load_setting(k))
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0)
}

/// Persist an arbitrary string setting (used for GIF quality/speed values).
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_save_string_setting(key: *const c_char, value: *const c_char) {
    if key.is_null() || value.is_null() {
        return;
    }
    let k = unsafe { CStr::from_ptr(key) }.to_str().unwrap_or("");
    let v = unsafe { CStr::from_ptr(value) }.to_str().unwrap_or("");
    runtime().block_on(db::save_setting(k, v));
}

/// Load a stored string setting. Returns a C string the caller must free with
/// glaspen2_free_c_string, or NULL when unset.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_load_string_setting(key: *const c_char) -> *mut c_char {
    if key.is_null() {
        return std::ptr::null_mut();
    }
    let k = unsafe { CStr::from_ptr(key) }.to_str().unwrap_or("");
    match runtime().block_on(db::load_setting(k)) {
        Some(v) => match CString::new(v) {
            Ok(cs) => cs.into_raw(),
            Err(_) => std::ptr::null_mut(),
        },
        None => std::ptr::null_mut(),
    }
}

// ---------------------------------------------------------------------------
// 笔预设与设置数值约束(presets 模块的 FFI 面;macOS/Windows 共用单一事实源)
// ---------------------------------------------------------------------------

/// 落笔即时反馈的原始笔宽(与 modeler 平滑宽度同公式)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_pressure_raw_width(
    pressure: c_double,
    width_scale: c_double,
) -> c_double {
    crate::presets::pressure_raw_width(pressure, width_scale)
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_color_preset_count() -> c_int {
    crate::presets::COLOR_PRESETS.len() as c_int
}

/// 取第 i 个颜色预设的 RGB(越界时不写目标,安全返回)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_color_preset_rgb(
    i: c_int,
    r: *mut c_double,
    g: *mut c_double,
    b: *mut c_double,
) {
    let idx = i as usize;
    if idx >= crate::presets::COLOR_PRESETS.len() {
        return;
    }
    let (cr, cg, cb) = crate::presets::COLOR_PRESETS[idx];
    unsafe {
        if !r.is_null() {
            *r = cr;
        }
        if !g.is_null() {
            *g = cg;
        }
        if !b.is_null() {
            *b = cb;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_width_preset_count() -> c_int {
    crate::presets::WIDTH_PRESETS.len() as c_int
}

/// 取第 i 档粗细倍率(越界返回 -1)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_width_preset_value(i: c_int) -> c_double {
    crate::presets::WIDTH_PRESETS
        .get(i as usize)
        .copied()
        .unwrap_or(-1.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_nearest_color_index(r: c_double, g: c_double, b: c_double) -> c_int {
    crate::presets::nearest_color_index(r, g, b) as c_int
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_nearest_width_index(w: c_double) -> c_int {
    crate::presets::nearest_width_index(w) as c_int
}

/// 钳制 double 型设置值(键名用面板/FFI 的驼峰键;未知键原样返回)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_clamp_setting_double(key: *const c_char, v: c_double) -> c_double {
    if key.is_null() {
        return v;
    }
    let k = unsafe { CStr::from_ptr(key) }.to_str().unwrap_or("");
    crate::presets::clamp_setting_double(k, v)
}

/// 钳制 int 型设置值(未知键原样返回)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_clamp_setting_int(key: *const c_char, v: c_int) -> c_int {
    if key.is_null() {
        return v;
    }
    let k = unsafe { CStr::from_ptr(key) }.to_str().unwrap_or("");
    crate::presets::clamp_setting_int(k, v)
}

// ---------------------------------------------------------------------------
// Launch at login (macOS LaunchAgent)
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn launch_agent_plist() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home)
        .join("Library")
        .join("LaunchAgents")
        .join("com.glaspen2.plist")
}

#[cfg(target_os = "macos")]
fn launch_agent_program() -> String {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "/Applications/glaspen2.app/Contents/MacOS/glaspen2".to_string())
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // 仅 macOS 渲染/导出路径消费; Windows 用覆盖层自己的实现
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// `enable` is only used in the macOS branch; other platforms ignore it.
#[allow(unused_variables)]
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_set_launch_at_login(enable: c_int) -> c_int {
    #[cfg(target_os = "macos")]
    {
        let plist_path = launch_agent_plist();
        if enable != 0 {
            let parent = plist_path.parent().unwrap();
            std::fs::create_dir_all(parent).ok();
            let program = launch_agent_program();
            let plist = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
                 <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
                 \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
                 <plist version=\"1.0\">\n\
                 <dict>\n\
                 \t<key>Label</key>\n\
                 \t<string>com.glaspen2</string>\n\
                 \t<key>Program</key>\n\
                 \t<string>{program}</string>\n\
                 \t<key>RunAtLoad</key>\n\
                 \t<true/>\n\
                 </dict>\n\
                 </plist>\n",
                program = xml_escape(&program)
            );
            match std::fs::write(&plist_path, &plist) {
                Ok(_) => 1,
                Err(e) => {
                    eprintln!("[glaspen2] launch agent write failed: {}", e);
                    0
                }
            }
        } else {
            match std::fs::remove_file(&plist_path) {
                Ok(_) => 1,
                Err(e) => {
                    eprintln!("[glaspen2] launch agent remove failed: {}", e);
                    0
                }
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_is_launch_at_login() -> c_int {
    #[cfg(target_os = "macos")]
    {
        if launch_agent_plist().exists() { 1 } else { 0 }
    }
    #[cfg(not(target_os = "macos"))]
    {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_free_c_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            drop(CString::from_raw(ptr));
        }
    }
}

// ---------------------------------------------------------------------------
// GIF save (cropped)
// ---------------------------------------------------------------------------

/// Save cropped drawing as GIF to desktop. Returns 1 on success, 0 on failure.
/// `surface_scale` is the backing scale factor (1.0 = non-Retina, 2.0 = Retina).
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_now_secs() -> c_double {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// Get the time component of a single stroke point. Used by Windows Flutter overlay.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_get_stroke_point_time(idx: c_int, pidx: c_int) -> c_double {
    let strokes = STROKES.lock().unwrap();
    strokes
        .get(idx as usize)
        .and_then(|s| s.points.get(pidx as usize))
        .map_or(0.0, |p| p.3)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 新建页守卫:末页有笔迹才准建;末页空白 → 复用那页;空库 → 建第一页。
    #[test]
    fn plan_new_page_guard() {
        // 末页有笔迹:允许新建
        assert_eq!(plan_new_page(Some((7, true))), (true, None));
        // 末页空白:不新建,复用末页(空白页之后不能再造空白页)
        assert_eq!(plan_new_page(Some((7, false))), (false, Some(7)));
        // 空库:建第一页
        assert_eq!(plan_new_page(None), (true, None));
    }

    /// ⌘⌃2 全流程(默认 mock 通道):start → 提交笔迹(钩子实时推帧)→
    /// stop,回执"已发送 1 笔";重复 start 被拒;关掉后再 stop 报无会话。
    #[test]
    fn ink_draft_session_mock_roundtrip() {
        // 测试依赖默认 mock 模式;显式要求真连的环境下跳过。
        if std::env::var("GLASPEN_CHAT_MOCK").is_ok_and(|v| v == "0") {
            return;
        }
        let _g = crate::tests::TEST_LOCK.lock().unwrap();
        assert_eq!(glaspen2_ink_draft_start(1920, 1080), 1);
        assert_eq!(
            glaspen2_ink_draft_start(1920, 1080),
            0,
            "会话进行中应拒绝二连开"
        );
        STROKES.lock().unwrap().push(Stroke {
            id: 0,
            r: 1.0,
            g: 0.0,
            b: 0.0,
            points: vec![(1.0, 2.0, 2.5, 0.0), (30.0, 40.0, 3.5, 0.12)],
        });
        ink_draft_on_stroke_committed();
        assert_eq!(
            glaspen2_ink_draft_stop(),
            1,
            "mock 通道应回执 sent/accepted=1"
        );
        assert_eq!(glaspen2_ink_draft_stop(), -1, "会话已关,再 stop 报无会话");
        STROKES.lock().unwrap().pop();
    }

    /// 描边对比色:亮色(黄/白)配黑边,暗色(蓝/黑)配白边。
    #[test]
    fn outline_contrast_follows_luminance() {
        assert_eq!(outline_contrast_color(1.0, 1.0, 0.0), (0, 0, 0)); // 黄
        assert_eq!(outline_contrast_color(1.0, 1.0, 1.0), (0, 0, 0)); // 白
        assert_eq!(outline_contrast_color(0.11, 0.44, 0.85), (255, 255, 255)); // 蓝
        assert_eq!(outline_contrast_color(0.0, 0.0, 0.0), (255, 255, 255)); // 黑
    }

    /// 笔迹 → STROKE 消息:颜色转 0xRRGGBB,点列保序,不带作者/设备。
    #[test]
    fn stroke_to_chat_message_mapping() {
        let s = Stroke {
            id: 0,
            r: 1.0,
            g: 0.0,
            b: 0.0,
            points: vec![(0.0, 0.0, 2.0, 0.0), (10.0, 10.0, 3.0, 0.05)],
        };
        let m = stroke_to_chat_message(7, &s);
        assert_eq!(m.seq, 7);
        assert_eq!(m.r#type, glaspen_chat::pb::MsgType::Stroke as i32);
        assert_eq!(m.author, "");
        assert_eq!(m.device, "");
        assert_eq!(m.notebook_id, CHAT_NOTEBOOK);
        let Some(glaspen_chat::pb::chat_message::Payload::Stroke(sc)) = m.payload else {
            panic!("wrong payload");
        };
        assert_eq!(sc.color_rgb, 0xFF0000);
        assert_eq!(sc.points.len(), 2);
        assert_eq!(
            (
                sc.points[1].x,
                sc.points[1].y,
                sc.points[1].width,
                sc.points[1].t_rel
            ),
            (10.0, 10.0, 3.0, 0.05)
        );
    }

    fn pts(data: &[(f64, f64, f64)]) -> Vec<(f64, f64, f64, f64)> {
        data.iter().map(|&(x, y, w)| (x, y, w, 0.0)).collect()
    }

    #[test]
    fn test_decimate_short_list_unchanged() {
        let input = pts(&[
            (0.0, 0.0, 2.0),
            (1.0, 1.0, 3.0),
            (2.0, 0.0, 2.0),
            (3.0, 1.0, 3.0),
        ]);
        assert_eq!(decimate(&input), input);
    }

    #[test]
    fn test_decimate_single_point_unchanged() {
        let input = pts(&[(5.0, 5.0, 2.0)]);
        assert_eq!(decimate(&input), input);
    }

    #[test]
    fn test_decimate_drops_close_points() {
        // 0.3/0.6 away from (0,0) with same width → dropped; 10.0/20.0 kept
        let input = pts(&[
            (0.0, 0.0, 2.0),
            (0.3, 0.0, 2.0),
            (0.6, 0.0, 2.0),
            (10.0, 0.0, 2.0),
            (20.0, 0.0, 2.0),
        ]);
        let out = decimate(&input);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0], input[0]);
        assert_eq!(out[1], input[3]);
        assert_eq!(out[2], input[4]);
    }

    #[test]
    fn test_decimate_keeps_width_changes() {
        // width jumps 1.0 → 4.0 (>12%) even at close distance → kept
        let input = pts(&[
            (0.0, 0.0, 1.0),
            (0.2, 0.0, 1.0),
            (0.4, 0.0, 4.0),
            (0.6, 0.0, 4.0),
            (10.0, 0.0, 1.0),
        ]);
        let out = decimate(&input);
        assert!(
            out.iter().any(|p| p.0 == 0.4 && p.2 == 4.0),
            "width jump must be kept: {:?}",
            out
        );
        assert_eq!(*out.last().unwrap(), *input.last().unwrap());
    }

    #[test]
    fn test_decimate_keeps_last_point() {
        // everything within threshold — first and last survive
        let input = pts(&[
            (0.0, 0.0, 2.0),
            (0.1, 0.1, 2.0),
            (0.2, 0.2, 2.0),
            (0.3, 0.3, 2.0),
            (0.4, 0.4, 2.0),
        ]);
        let out = decimate(&input);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0], input[0]);
        assert_eq!(out[1], input[4]);
    }

    #[test]
    fn test_encode_animated_gif_empty_is_none() {
        assert!(encode_animated_gif(&[], 15, 0.5, 2.0, 1).is_none());
    }

    #[test]
    fn test_encode_animated_gif_produces_gif_bytes() {
        // Two strokes with monotonic relative_time so the timeline has duration.
        let strokes = vec![
            GifStroke {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                points: vec![(0.0, 0.0, 2.0, 0.0), (20.0, 20.0, 3.0, 0.5)],
            },
            GifStroke {
                r: 0.0,
                g: 0.0,
                b: 1.0,
                points: vec![(5.0, 5.0, 2.0, 0.0), (30.0, 10.0, 2.0, 0.4)],
            },
        ];
        let bytes = encode_animated_gif(&strokes, 15, 0.5, 2.0, 1).expect("should encode");
        // GIF89a magic, plus a non-trivial payload.
        assert!(bytes.starts_with(b"GIF89a"));
        assert!(bytes.len() > 20);
    }

    #[test]
    fn test_encode_animated_gif_end_mode_loop_control() {
        let strokes = vec![GifStroke {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            points: vec![(0.0, 0.0, 2.0, 0.0), (20.0, 20.0, 3.0, 0.5)],
        }];
        // mode 0 = play once → no Netscape loop extension (stop on last frame)
        let once = encode_animated_gif(&strokes, 15, 0.5, 2.0, 0).expect("once");
        assert!(
            !contains_netescape(&once),
            "stop-on-last-frame GIF must not write the loop extension"
        );
        // mode 1/2 = loop → Netscape loop extension present
        let hold = encode_animated_gif(&strokes, 15, 0.5, 2.0, 1).expect("hold");
        let loop_now = encode_animated_gif(&strokes, 15, 0.5, 2.0, 2).expect("loop");
        assert!(contains_netescape(&hold), "hold-then-loop must loop");
        assert!(contains_netescape(&loop_now), "immediate-loop must loop");
    }

    #[test]
    fn test_gif_background_stays_transparent_with_black_ink() {
        // Black ink: under the old palette logic the transparent background
        // (0,0,0,0) shared the black palette entry and rendered as black once
        // a frame got busy. Now a reserved index is always transparent.
        let strokes = vec![GifStroke {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            points: vec![(50.0, 50.0, 3.0, 0.0), (150.0, 150.0, 3.0, 0.5)],
        }];
        let bytes = encode_animated_gif(&strokes, 15, 0.5, 2.0, 1).expect("encode");

        let mut options = gif::DecodeOptions::new();
        options.set_color_output(gif::ColorOutput::Indexed);
        let mut decoder = options.read_info(&bytes[..]).expect("decode header");
        let mut last: Option<(Vec<u8>, Option<u8>, usize, usize)> = None;
        while let Some(f) = decoder.read_next_frame().expect("frame") {
            last = Some((
                f.buffer.to_vec(),
                f.transparent,
                f.width as usize,
                f.height as usize,
            ));
        }
        let (buffer, transparent, w, h) = last.expect("must have frames");
        let transp = transparent.expect("must have a transparent index");
        // Corner pixels (outside the ink bbox + pad) must be transparent, never black.
        assert_eq!(buffer[0], transp, "top-left must be transparent");
        assert_eq!(buffer[w - 1], transp, "top-right must be transparent");
        assert_eq!(
            buffer[(h - 1) * w],
            transp,
            "bottom-left must be transparent"
        );
        assert_ne!(
            buffer[w / 2 + h / 2 * w],
            transp,
            "ink pixel must not be transparent"
        );
    }

    fn contains_netescape(bytes: &[u8]) -> bool {
        bytes.windows(11).any(|w| w == b"NETSCAPE2.0")
    }

    /// Synthetic "20 hanzi" workload for a rough speed measurement of the
    /// animated-GIF pipeline. Run with `cargo test -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn bench_animated_gif_20_hanzi() {
        fn make_strokes() -> Vec<GifStroke> {
            // 20 characters, ~5 strokes each; laid out in 2 rows of 10, spread
            // across a large area to approximate a full-canvas doodle.
            let mut out = Vec::new();
            for hi in 0..20usize {
                let cx = (hi % 10) as f64 * 150.0 + 60.0;
                let cy = (hi / 10) as f64 * 200.0 + 60.0;
                for si in 0..5usize {
                    let (x0, y0) = (cx, cy + si as f64 * 10.0);
                    let (x1, y1) = (cx + 60.0, cy + si as f64 * 10.0 + 30.0);
                    let dur = 0.2;
                    let n = 20usize;
                    let points: Vec<(f64, f64, f64, f64)> = (0..=n)
                        .map(|k| {
                            let f = k as f64 / n as f64;
                            (x0 + (x1 - x0) * f, y0 + (y1 - y0) * f, 2.0, f * dur)
                        })
                        .collect();
                    out.push(GifStroke {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        points,
                    });
                }
            }
            out
        }
        let strokes = make_strokes();
        let bbox_w = 10.0 * 150.0;
        let bbox_h = 2.0 * 200.0;
        eprintln!(
            "strokes={}, points≈{}, bbox≈{}x{}",
            strokes.len(),
            strokes.iter().map(|s| s.points.len()).sum::<usize>(),
            bbox_w,
            bbox_h
        );
        for (fps, res, speed) in [
            (15, 0.5, 2.0),
            (24, 0.5, 2.0),
            (30, 0.75, 2.0),
            (50, 1.0, 2.0),
        ] {
            let start = std::time::Instant::now();
            let bytes = encode_animated_gif(&strokes, fps, res, speed, 1);
            let el = start.elapsed();
            eprintln!(
                "fps={} res={} speed={} -> {:?} ({} bytes)",
                fps,
                res,
                speed,
                el,
                bytes.map(|b| b.len()).unwrap_or(0)
            );
        }
    }

    #[test]
    fn test_svg_merges_equal_width_runs_into_one_path() {
        // 100 points of constant width must collapse to ONE <path>, not 100
        // elements — this is what keeps large canvases small and quick to open.
        let pts: Vec<(f64, f64, f64)> = (0..100).map(|i| (i as f64, i as f64, 2.0)).collect();
        let snap = vec![(0.0, 0.0, 0.0, pts)];
        let svg = build_svg_from(&snap).expect("svg");
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("stroke-width"));
        assert!(svg.ends_with("</svg>\n"));
        assert_eq!(svg.matches("<path").count(), 1, "constant width = one path");
    }

    #[test]
    fn test_svg_splits_width_runs() {
        // Widths 1,1,4,4 -> two runs -> two <path> elements.
        let pts = vec![
            (0.0, 0.0, 1.0),
            (1.0, 0.0, 1.0),
            (2.0, 0.0, 4.0),
            (3.0, 0.0, 4.0),
        ];
        let snap = vec![(1.0, 0.0, 0.0, pts)];
        let svg = build_svg_from(&snap).expect("svg");
        assert_eq!(svg.matches("<path").count(), 2);
        assert!(svg.contains("stroke-width=\"1.00\""));
        assert!(svg.contains("stroke-width=\"4.00\""));
    }

    /// Measure SVG node count / byte size at various canvas sizes, for both
    /// smooth pressure (few width runs) and noisy pressure (worst case, one run
    /// per point). Run: cargo test --lib bench_svg_scaling -- --ignored --nocapture
    #[test]
    #[ignore]
    fn bench_svg_scaling() {
        for (n_strokes, pts_per) in [(1_000usize, 50usize), (5_000, 100), (20_000, 100)] {
            // Smooth: width varies over a slow sine -> long equal-width runs.
            let mut smooth: Vec<(f64, f64, f64, Vec<(f64, f64, f64)>)> =
                Vec::with_capacity(n_strokes);
            let mut noisy: Vec<(f64, f64, f64, Vec<(f64, f64, f64)>)> =
                Vec::with_capacity(n_strokes);
            for s in 0..n_strokes {
                let (ox, oy) = ((s % 200) as f64 * 60.0, (s / 200) as f64 * 60.0);
                let mut sp = Vec::with_capacity(pts_per);
                let mut np = Vec::with_capacity(pts_per);
                for i in 0..pts_per {
                    let f = i as f64 / pts_per as f64;
                    let x = ox + f * 40.0;
                    let y = oy + (f * 8.0).sin() * 10.0;
                    let w_smooth = 1.0 + (f * std::f64::consts::TAU).sin() * 0.5 + 2.0;
                    let w_noisy = 1.0 + ((i * 7 % 37) as f64) / 6.0; // jitters every point
                    sp.push((x, y, w_smooth));
                    np.push((x, y, w_noisy));
                }
                smooth.push((0.0, 0.0, 0.0, sp));
                noisy.push((0.0, 0.0, 0.0, np));
            }
            let total_pts = n_strokes * pts_per;
            for (label, snap) in [("smooth", &smooth), ("noisy", &noisy)] {
                let svg = build_svg_from(snap).expect("svg");
                eprintln!(
                    "strokes={} pts={} ({label}): elements={} bytes={} (~{} B/point, old per-point <line> would be ~{} elements, ~{} MB)",
                    n_strokes,
                    total_pts,
                    svg.matches("<path").count() + svg.matches("<circle").count(),
                    svg.len(),
                    svg.len() / total_pts.max(1),
                    total_pts,
                    total_pts * 130 / 1_000_000
                );
            }
        }
    }

    /// 虚拟笔迹基准:测"每次拖动事件"在 Rust 侧的真实成本 —— 复刻
    /// `glaspen2_modeler_begin/move/end/commit_to_strokes` 的调用序列
    /// (模型器预测 + 逐点缓冲 + pen-up 的 decimate/提交)。
    ///
    /// 这是涂鸦热路径里 **Rust 的那一半**;ObjC 绘制 + CA 上屏那一半要靠
    /// 虚拟 CGEvent 注入测(见 docs/debugging.md 的性能章节)。
    /// 只测每事件成本, 不含 DB(begin 的那一行 INSERT 是每笔一次, 不在热路径)。
    ///
    /// debug 与 release 各跑一次对比 —— debug 的模型器数学慢好几倍,
    /// 别用 debug 数字下结论:
    ///   cargo test bench_virtual_stroke -- --ignored --nocapture
    ///   cargo test --release bench_virtual_stroke -- --ignored --nocapture
    #[test]
    #[ignore]
    fn bench_virtual_stroke_per_event() {
        let _g = crate::tests::TEST_LOCK.lock().unwrap();
        let n_events = 2000usize; // 200Hz × 10 秒
        let dt = 1.0 / 200.0;
        // 手写弧线:横向推进 + 纵向正弦起伏 + 压感脉动
        let curve = |t: f64| -> (f64, f64, f64) {
            let x = 400.0 + t * 300.0;
            let y = 600.0 + (t * 4.0).sin() * 120.0;
            let p = 0.5 + 0.4 * (t * 6.0).sin().abs();
            (x, y, p)
        };

        for iter in 0..3 {
            STROKES.lock().unwrap().clear();
            let (x0, y0, p0) = curve(0.0);

            // begin 的等价部分(不含 DB 的那一行 INSERT)
            let t_begin = std::time::Instant::now();
            crate::modeler::begin_stroke(x0, y0, p0, 0.0, 1.0);
            crate::state::buffer_point(x0, y0, crate::pressure_to_width(p0, 1.0), 0.0);
            let begin_cost = t_begin.elapsed();

            let t_in = std::time::Instant::now();
            for i in 1..=n_events {
                let t = i as f64 * dt;
                let (x, y, p) = curve(t);
                glaspen2_modeler_move(x, y, p, t, 1.0);
            }
            let move_cost = t_in.elapsed();

            // pen-up:模型器收敛 + 提交到笔迹(含 decimate)
            STROKES.lock().unwrap().push(Stroke {
                id: 0,
                r: 1.0,
                g: 0.0,
                b: 0.0,
                points: Vec::new(),
            });
            let t_up = std::time::Instant::now();
            let (xe, ye, pe) = curve(n_events as f64 * dt);
            glaspen2_modeler_end(xe, ye, pe, n_events as f64 * dt, 1.0);
            glaspen2_modeler_commit_to_strokes(1.0, 0.0, 0.0);
            let up_cost = t_up.elapsed();

            println!(
                "iter {}: begin={:?}  {} 个 move = {:?} ({:.1} µs/事件)  pen-up 收敛+提交={:?}",
                iter,
                begin_cost,
                n_events,
                move_cost,
                move_cost.as_micros() as f64 / n_events as f64,
                up_cost,
            );
        }

        STROKES.lock().unwrap().clear();
        let _ = crate::state::take_pending();
    }

    /// Wire format of `glaspen2_page_thumbnails`, parsed exactly the way the
    /// Dart side does (`_parseThumbnailBlob`). Keeps both ends in sync.
    #[test]
    fn test_thumb_blob_layout() {
        let entries = vec![
            (7i64, vec![1u8, 2, 3]),
            (569i64, vec![]),
            (-1i64, vec![255u8]),
        ];
        let blob = encode_thumb_blob(&entries);

        assert_eq!(
            u32::from_le_bytes(blob[0..4].try_into().unwrap()),
            THUMB_BLOB_MAGIC
        );
        assert_eq!(u32::from_le_bytes(blob[4..8].try_into().unwrap()), 3);

        let mut off = 8usize;
        let mut parsed: Vec<(i64, Vec<u8>)> = Vec::new();
        while off < blob.len() {
            let id = i64::from_le_bytes(blob[off..off + 8].try_into().unwrap());
            let len = u32::from_le_bytes(blob[off + 8..off + 12].try_into().unwrap()) as usize;
            let png = blob[off + 12..off + 12 + len].to_vec();
            parsed.push((id, png));
            off += 12 + len;
        }
        assert_eq!(parsed, entries);
        assert_eq!(off, blob.len(), "blob must parse to exactly its length");

        // Empty batch still carries the magic + zero count.
        let empty = encode_thumb_blob(&[]);
        assert_eq!(empty.len(), 8);
        assert_eq!(u32::from_le_bytes(empty[4..8].try_into().unwrap()), 0);

        // The FFI entry point rejects degenerate arguments instead of reading
        // through a null pointer.
        let mut len: i32 = 123;
        assert!(glaspen2_page_thumbnails(std::ptr::null(), 4, 280, &mut len).is_null());
        assert_eq!(len, 0);
        let ids = [7i64];
        assert!(
            glaspen2_page_thumbnails(ids.as_ptr(), 1, 0, &mut len).is_null(),
            "max_size <= 0 must be rejected"
        );
        assert_eq!(len, 0);
    }
}
