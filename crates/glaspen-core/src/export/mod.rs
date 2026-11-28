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

use crate::camera::{Camera, ZoomOutcome};
use crate::draw_spec::{
    Color, Dash, DrawCommand, DrawPlan, LiveStroke, OUTLINE_COLORS, OutlineParams, PaintPass,
    PassOrder, PlanError, PlanParams, PlanScope, Rect, ShadowParams, Vec2, ViewTransform,
    plan as draw_plan,
};
use crate::grid::{GridLines, GridParams, Segment};
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

pub mod bubbles;
mod chat_glue;
mod media;
pub mod nas_backup;
mod pages;
#[cfg(target_os = "macos")]
mod pen_settings;
#[cfg(target_os = "macos")]
mod render_state;
pub(crate) mod thumbs;
#[cfg(target_os = "macos")]
mod toggle_state;
#[cfg(target_os = "macos")]
mod view_state;
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
// hook 注入器单独导出(pages 模块私有, 函数经 * 已可用但路径更明确)
pub use pages::{set_glass_refresh_hook, set_infinite_refresh_hook};
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

/// 抽稀最后一笔的点列。Windows 壳在抬笔后调用(它用自己的 StrokeModeler,
/// 不经过 modeler_commit_to_strokes); macOS 在 commit 内部已经抽稀。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_decimate_last_stroke() {
    ffi_guard((), || {
        let mut strokes = STROKES.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(last) = strokes.last_mut() {
            last.points = decimate(&last.points);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_begin_stroke(
    r: c_double,
    g: c_double,
    b: c_double,
    width_scale: c_double,
) {
    crate::state::mark_stroke_activity();
    let id = runtime().block_on(db::begin_stroke(r, g, b, width_scale));
    let mut strokes = STROKES.lock().unwrap_or_else(|e| e.into_inner());
    strokes.push(Stroke {
        id,
        r,
        g,
        b,
        points: Vec::new(),
    });
    *RAW_STROKE_START.lock().unwrap_or_else(|e| e.into_inner()) = None;
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
    let mut strokes = STROKES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(stroke) = strokes.last_mut() {
        stroke.points.push((x, y, width, t));
    }
    state::buffer_point(x, y, width, t); // sync
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_end_stroke() {
    crate::state::mark_stroke_activity();
    db::end_stroke_spawned();
}

/// 描边(轮廓)渲染开关 —— 纯渲染设置,只在内存,不落库、重启即恢复关闭。
static STROKE_OUTLINE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

// 描边比笔迹宽出的半径不再是本文件的常量:pad = 1.0 逻辑 px 已单源在
// core draw_spec::OutlineParams::macos_committed(随视图缩放,差异 C),
// 本文件只按计划光栅化。pagerender 的软件路径暂时保留自己的 OUTLINE_PAD。

/// 按笔色亮度选对比描边色(BT.601, 阈值 0.5)。marching ants 虚线描边
/// 实验期间未接线, 保留备用(若回退"自适应对比色实线描边"直接接回)。
#[allow(dead_code)]
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

/// 软阴影开关(独立于描边, 可叠加): 笔迹下方三档加宽递减 alpha 的黑影。
static STROKE_SHADOW: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_set_soft_shadow(enabled: c_int) {
    STROKE_SHADOW.store(enabled != 0, std::sync::atomic::Ordering::SeqCst);
}

/// 反色突出(实验): 墨迹主体以捕获的背景反相位图为 source, 逐像素取
/// "下方背景的反色"。背景位图由 ObjC 侧经 SCScreenshotManager 持续刷新
/// (排除自身窗口), 以 cairo image surface 指针传入。
static STROKE_INVERT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static INVERT_BG_SURFACE: std::sync::atomic::AtomicPtr<std::ffi::c_void> =
    std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_set_stroke_invert(enabled: c_int) {
    STROKE_INVERT.store(enabled != 0, std::sync::atomic::Ordering::SeqCst);
}

/// 传入反色背景的 cairo image surface(与 g_surface 同尺寸同坐标, 1:1);
/// None 传空指针。ObjC 侧保活该 surface, 清除前必须先置空这边。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_set_invert_background(surface: *mut std::ffi::c_void) {
    INVERT_BG_SURFACE.store(surface, std::sync::atomic::Ordering::SeqCst);
}

/// 反色局部重绘的脏区(表面像素坐标)。ObjC 在触发重绘前设置; 取用即清
/// (w<0 = 全量)。让只有背景真正变化的区域被清空+重画, 其余笔迹像素
/// 一字不动 —— 微动画不再让全屏涂鸦陪闪。
static INVERT_DIRTY: [std::sync::atomic::AtomicI32; 4] = [
    std::sync::atomic::AtomicI32::new(-1),
    std::sync::atomic::AtomicI32::new(-1),
    std::sync::atomic::AtomicI32::new(-1),
    std::sync::atomic::AtomicI32::new(-1),
];

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_set_invert_dirty(x: c_int, y: c_int, w: c_int, h: c_int) {
    INVERT_DIRTY[0].store(x, std::sync::atomic::Ordering::SeqCst);
    INVERT_DIRTY[1].store(y, std::sync::atomic::Ordering::SeqCst);
    INVERT_DIRTY[2].store(w, std::sync::atomic::Ordering::SeqCst);
    INVERT_DIRTY[3].store(h, std::sync::atomic::Ordering::SeqCst);
}

fn take_invert_dirty() -> Option<(c_double, c_double, c_double, c_double)> {
    let dw = INVERT_DIRTY[2].swap(-1, std::sync::atomic::Ordering::SeqCst);
    if dw <= 0 {
        return None;
    }
    let dx = INVERT_DIRTY[0].load(std::sync::atomic::Ordering::SeqCst);
    let dy = INVERT_DIRTY[1].load(std::sync::atomic::Ordering::SeqCst);
    let dh = INVERT_DIRTY[3].load(std::sync::atomic::Ordering::SeqCst);
    Some((
        dx as c_double,
        dy as c_double,
        dw as c_double,
        dh as c_double,
    ))
}

/// 当前是否有笔迹(反色捕获循环的空转判断用)
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_has_strokes() -> c_int {
    let n = STROKES.lock().map(|s| s.len()).unwrap_or(0);
    (n > 0) as c_int
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
    let mut strokes = STROKES.lock().unwrap_or_else(|e| e.into_inner());
    strokes.clear();
    for s in data {
        let points = crate::sanitize_loaded_points(s.points);
        if points.is_empty() {
            continue;
        }
        strokes.push(Stroke {
            id: s.id,
            r: s.r,
            g: s.g,
            b: s.b,
            points,
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
///
/// D5b: zoom 在 FFI 边界按 [Camera::from_stored](与 `from_db_values` 同款)
/// 兜底 —— `zoom > 0 ? zoom : 1.0`(含 NaN), **不钳 100% 上限**;两端载入
/// (macOS `canvas_infinite_load` / Windows `apply_infinite_canvas`、启动恢复)
/// 因此不再各写一份兜底公式。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_get_infinite_transform(
    x: *mut c_double,
    y: *mut c_double,
    z: *mut c_double,
) {
    let cam = runtime()
        .block_on(db::get_infinite_transform())
        .map(|(pan_x, pan_y, zoom)| Camera::from_stored(pan_x, pan_y, zoom))
        .unwrap_or(Camera::HOME);
    unsafe {
        *x = cam.pan_x;
        *y = cam.pan_y;
        *z = cam.zoom;
    }
}

/// Re-render every stroke from STROKES onto the ObjC-owned cairo surface.
/// FFI 边界 panic 护栏。
///
/// `extern "C"` 函数里发生 unwind 是未定义行为, 且实战中默认直接 abort。
/// 输入事件/渲染/导出都从 ObjC、Win32 直接调进来, 一次索引越界不该让
/// 用户丢掉整个应用。这里把 panic 接住, 返回调用方约定的默认值, 并留下
/// error 级日志(panic hook 另有 backtrace 落盘)。
pub(crate) fn ffi_guard<T>(default: T, f: impl FnOnce() -> T) -> T {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(v) => v,
        Err(_) => {
            tracing::error!("FFI panic 已拦截, 返回默认值(详见崩溃日志/backtrace)");
            default
        }
    }
}

/// 退出前收尾: 把排队中的点写完, 并把 WAL 合并回主库。
///
/// ObjC 的 `exit(0)` / `NSApp terminate` 不会跑 Rust 析构, 不 checkpoint
/// 的话更新器拷贝的 `.db` 快照会漏掉 WAL 里的最近笔迹。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_shutdown() {
    runtime().block_on(db::end_stroke());
    // 退出前把未完结的气泡段结掉并落库, 否则最后一条消息只在内存里。
    bubbles::flush_now();
    runtime().block_on(db::checkpoint());
}

/// Used on undo, page navigation and display changes.
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
    ffi_guard((), || {
        draw_rebuild_impl(surface_ptr, scale, page_ox, page_oy, page_scale);
    });
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
    // 反色局部重绘: 有脏区则只清+只画该区域(裁剪), 其余像素保持原样
    let clip = take_invert_dirty();
    match clip {
        Some((cx, cy, cw, ch)) => {
            r.clear_rect(cx as f32, cy as f32, cw as f32, ch as f32);
            r.clip_rect(cx as f32, cy as f32, cw as f32, ch as f32);
        }
        None => r.clear(),
    }
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
    let invert = if STROKE_INVERT.load(std::sync::atomic::Ordering::SeqCst) {
        crate::cairo_dl::CairoPattern::from_surface(
            INVERT_BG_SURFACE.load(std::sync::atomic::Ordering::SeqCst),
        )
    } else {
        None
    };
    // 主页笔迹: 只在需要绘制时持锁, 画完立刻释放。以前锁会一直持有到
    // 后面的邻页 DB 查询, 落笔线程抢锁时要等一整轮 SQLite IO。
    {
        let strokes = STROKES.lock().unwrap_or_else(|e| e.into_inner());
        paint_strokes_into(
            &r,
            strokes.as_slice(),
            pan_x_eff,
            pan_y_eff,
            zoom_eff,
            scale,
            0.0,
            STROKE_SHADOW.load(std::sync::atomic::Ordering::SeqCst),
            outline,
            1.0,
            invert.as_ref(),
        );
    }

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
                group.as_slice(),
                pan_x_eff,
                pan_y_eff,
                zoom_eff,
                scale,
                *dy * stride,
                STROKE_SHADOW.load(std::sync::atomic::Ordering::SeqCst),
                outline,
                1.0,
                invert.as_ref(),
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

    r.reset_clip(); // 反色局部重绘收尾(每次调用都是新 context, 双保险)
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
            false, // 导出(白底/缩略)不带阴影
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

/// macOS 整页重建的计划参数(D9c 第二步):阴影**全局先画**,之后逐笔
/// 「描边 → 墨迹」——D9a 审计差异 A 的 macOS 侧;pad/虚线随视图缩放
/// (差异 C)。计划本身仍是平台无关的 core 纯函数,这里只选平台参数。
fn macos_paint_plan(
    strokes: &[Stroke],
    view: ViewTransform,
    viewport: Rect,
    shadow: bool,
    outline: bool,
) -> Result<DrawPlan, PlanError> {
    let mut params = PlanParams::new(view, viewport);
    params.order = PassOrder::MacosCommitted;
    params.scope = PlanScope::Rebuild;
    if shadow {
        params.shadow = Some(ShadowParams::macos(&view));
    }
    if outline {
        params.outline = Some(OutlineParams::macos_committed(&view));
    }
    draw_plan(strokes, &params)
}

/// 把一组笔迹画到 renderer 上(支持跨页 y 偏移、描边层与整体不透明度)。
///
/// D9c 第二步:**pass 顺序 / 每点宽度 / 透明度 / 虚线 / 颜色全部来自
/// core `draw_spec::plan`**(平台参数见 [macos_paint_plan]);本函数只填
/// macOS 视图参数(Retina scale、页 fit 折叠后的 pan/zoom)并光栅化。
///
/// 坐标管线:像素 = (画布坐标 − pan) × zoom × scale + y_shift。y_shift 是
/// 画布坐标里的页偏移(随 zoom 缩放),ViewTransform 没有对应字段 → 折进
/// pan.y: `y + shift − pan ≡ y − (pan − shift)`(见 D9a 差异 M:邻页由调用方
/// 用不同视图多次 plan)。与原式相比只有 f64 舍入级差异(ULL 级),不影响像素。
///
/// `alpha < 1` 时仍走旧的软件合成路径 [paint_strokes_soft](未迁移:该路径是
/// pagerender 的「全部阴影 → 全部描边 → 全部墨迹」分层,与不透明快路径不同,
/// D9a 差异 I;本次保持原实现)。plan 的 clear/dirty 不在这里使用:整页重建
/// 的清屏/裁剪由 draw_rebuild_impl 事先完成(cairo clear/clip_rect)。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // 仅 macOS 渲染/导出路径消费; Windows 用覆盖层自己的实现
pub(crate) fn paint_strokes_into(
    r: &crate::cairo_dl::CairoRenderer,
    strokes: &[Stroke],
    pan_x: f64,
    pan_y: f64,
    zoom: f64,
    scale: f64,
    y_shift: f64,
    shadow: bool,
    outline: bool,
    alpha: f64,
    invert: Option<&crate::cairo_dl::CairoPattern>,
) {
    let alpha = alpha.clamp(0.0, 1.0);
    if alpha <= 0.0 {
        return;
    }
    // 整体不透明度 ≠ 1: 走软件合成(独立缓冲 + blit), 保证渐显/渐隐均匀。
    // (软路径不支持 pattern source, 反色在渐显帧回落笔色)
    if alpha < 1.0 {
        paint_strokes_soft(
            r, strokes, pan_x, pan_y, zoom, scale, y_shift, shadow, outline, alpha,
        );
        return;
    }
    let view = ViewTransform::new(Vec2::new(pan_x, pan_y - y_shift), zoom, scale);
    let viewport = Rect::new(0.0, 0.0, r.w.max(0) as f64, r.h.max(0) as f64);
    let Ok(plan) = macos_paint_plan(strokes, view, viewport, shadow, outline) else {
        return;
    };
    rasterize_macos_plan(r, &plan, invert);
}

/// 按 core DrawPlan 光栅化 macOS 整页重建。
///
/// 光栅化算法保持旧实现(逐段圆头描线 + 首点实心圆点;差异 F:不是 Windows
/// 的多边形轮廓),这里只消费计划给的顺序/点列/宽度/透明度/颜色/虚线。
fn rasterize_macos_plan(
    r: &crate::cairo_dl::CairoRenderer,
    plan: &DrawPlan,
    invert: Option<&crate::cairo_dl::CairoPattern>,
) {
    for layer in &plan.layers {
        // 反色 pattern 只在「逐笔层」(描边/墨迹)挂:旧实现就是每笔在描边前
        // 重新挂 pattern(阴影层是实体黑,不消费 pattern)。
        let per_stroke = layer.iter().any(|c| c.pass != PaintPass::Shadow);
        if per_stroke && let Some(p) = invert {
            r.set_source_pattern(p);
        }
        for cmd in layer {
            rasterize_macos_command(r, plan, cmd, invert.is_some());
        }
    }
}

/// 执行一条计划命令(只调 cairo;不决定任何顺序/宽度规则)。
fn rasterize_macos_command(
    r: &crate::cairo_dl::CairoRenderer,
    plan: &DrawPlan,
    cmd: &DrawCommand,
    invert_on: bool,
) {
    let pts = &plan.points[cmd.points.clone()];
    match cmd.pass {
        // 软阴影层:全部笔迹先画(macOS 分层,差异 A);宽度 = 计划给的
        // 每点墨迹直径 + 该档 width_delta(= 2 × spread × factor,差异 B)。
        PaintPass::Shadow => {
            for i in 1..pts.len() {
                let (a, b) = (&pts[i - 1], &pts[i]);
                r.stroke_line_alpha(
                    a.pos.x as f32,
                    a.pos.y as f32,
                    b.pos.x as f32,
                    b.pos.y as f32,
                    (b.width + cmd.width_delta) as f32,
                    (0, 0, 0),
                    cmd.alpha,
                );
            }
        }
        // 描边层:黑白相间虚线(颜色 = OUTLINE_COLORS,core 单源常量),
        // 相位沿整笔累计弧长连续;平头避免 1px 虚段被圆帽吞掉。
        PaintPass::Outline => {
            if pts.len() < 2 {
                return;
            }
            let dash = cmd.dash.unwrap_or(Dash {
                len: 1.0,
                phase: 0.0,
            });
            r.set_line_cap_butt();
            let mut cum = dash.phase;
            for i in 1..pts.len() {
                let (a, b) = (&pts[i - 1], &pts[i]);
                let (x0, y0) = (a.pos.x as f32, a.pos.y as f32);
                let (x1, y1) = (b.pos.x as f32, b.pos.y as f32);
                // 差异 D:macOS 整页重建的描边宽度取**当前点**宽度
                // (Windows 取相邻较粗者,见 paint_plan 适配层)。
                let segw = (b.width + cmd.width_delta) as f32;
                let off = cum.rem_euclid(2.0 * dash.len);
                r.set_dash(dash.len, off);
                r.stroke_line(x0, y0, x1, y1, segw, OUTLINE_COLORS[0].to_rgb8());
                r.set_dash(dash.len, (off + dash.len).rem_euclid(2.0 * dash.len));
                r.stroke_line(x0, y0, x1, y1, segw, OUTLINE_COLORS[1].to_rgb8());
                let (dx, dy) = (x1 - x0, y1 - y0);
                cum += ((dx * dx + dy * dy) as f64).sqrt();
            }
            r.set_dash(0.0, 0.0);
            r.set_line_cap_round();
        }
        // 墨迹本体:逐段圆头描线 + 首点实心圆点(差异 F:macOS 光栅化算法)。
        PaintPass::Fill => {
            let color = cmd.color.to_rgb8();
            for i in 0..pts.len() {
                let p = &pts[i];
                if i == 0 {
                    r.fill_circle_opt(
                        p.pos.x as f32,
                        p.pos.y as f32,
                        (p.width * 0.5) as f32,
                        (!invert_on).then_some(color),
                    );
                } else {
                    let prev = &pts[i - 1];
                    r.stroke_line_opt(
                        prev.pos.x as f32,
                        prev.pos.y as f32,
                        p.pos.x as f32,
                        p.pos.y as f32,
                        p.width as f32,
                        (!invert_on).then_some(color),
                    );
                }
            }
        }
        // 整页重建计划里不会出现 Erase(被擦除的笔迹此时已从 STROKES 删除)。
        PaintPass::Erase => {}
    }
}

// ── macOS 实时绘制计划 FFI(D9c 剩余:core 出计划, ObjC 光栅化) ────────
//
// macOS 实时路径(panel.m raw_draw_dot/raw_draw_segment)目前逐段自绘;D9c 把
// 它接到 core `draw_spec::plan(MacosLive)` 需要一条 JSON 边界。本函数是该边界
// 的 **core 侧半成品**:只产出计划(命令 / 脏区 / 接缝几何),不接线 ObjC、
// 不改像素;光栅化与接缝重画仍由 adapter 执行(见 draw_spec 模块文档
// 「plan 不建模的」第 1 条)。Windows 同进程直调 core,不经此 JSON。
//
// 输入(单个 UTF-8 JSON 字符串;NULL / 非法 UTF-8 / 非法 JSON / 字段不合法
// 都回 `{"error":"..."}`):
//
// ```text
// {
//   "points": [[x, y, width], ...],  // 必填;本次要画的实时点列(画布/视图逻辑
//                                    // 坐标 + 墨迹直径,逻辑点)。也接受
//                                    // [x,y,width,t] 四元组(t 本层忽略);
//                                    // width 必须有限且 >= 0。
//   "seam": [x, y],                  // 可选;上上采样点 P_{i-2}(逻辑坐标),配合
//                                    // points 首点 P_{i-1} 产出接缝重绘几何
//                                    // (见输出 seam)。不传时从点列自身推导。
//   "params": {                      // 必填对象(含必填 viewport)
//     "viewport": [w, h],            // 必填;表面尺寸(表面像素),脏区上限
//     "scale": 2.0,                  // 表面倍率(backingScaleFactor;> 0,缺省 1)
//     "zoom": 1.0,                   // 视图缩放(> 0;缺省 1;实时路径通常 1)
//     "pan": [0.0, 0.0],             // 视图平移(逻辑点;缺省 [0, 0])
//     "shadow": true,                // 软阴影三档(缺省 false)
//     "outline": true,               // marching ants 描边(缺省 false)
//     "eraser": false,               // 实时橡皮(缺省 false)
//     "alpha": 1.0,                  // 整体不透明度(缺省 1;plan 内钳到 0..=1)
//     "arc_len_start": 7.5,          // 整笔已画弧长(逻辑点;× scale = 描边起始
//                                    // 相位;缺省 0)
//     "color": [0.1, 0.2, 0.3]       // 落笔色 0..1(缺省黑;逐通道钳制)
//   }
// }
// ```
//
// 输出(单行 JSON,永不 NULL):
//
// ```text
// {
//   "commands": [                    // DrawPlan::layers 按层展开;数组顺序即绘制
//                                    // 顺序(层边界见 PassOrder::MacosCommitted)
//     {"layer": 0, "pass": "shadow|outline|fill|erase",
//      "stroke_index": 0, "stroke_id": 0, "live": true,
//      "color": [r, g, b], "alpha": 0.2, "width_delta": 3.0,
//      "dash": null,                // 仅 outline:{"len": 2.0, "phase": 15.0}
//      "points": [[x, y, 有效宽度], ...]}   // 表面像素;有效宽度 = 点宽 +
//                                           // width_delta(Shadow/Outline 已含)
//   ],
//   "dirty": {"x": 0.0, "y": 0.0, "w": 0.0, "h": 0.0},  // 表面像素;MacosLive
//                                    // 增量脏区(plan 单源公式,已并集 + 截到视口)
//   "seam": null | {"from": [x, y], "to": [x, y], "width": w}
//                                    // 表面像素;接缝重绘(panel.m:797-806):
//                                    // 用墨迹色把 from→to(宽 width)重画一遍,
//                                    // 盖回描边压在已干墨迹上的痕迹。
//                                    // 仅描边开启且存在上一段时给出。
// }
// ```

/// 实时计划输入(平台 adapter 可绕过 JSON 直接构造;字段语义同 FFI schema)。
pub struct MacosLiveInput {
    pub points: Vec<(f64, f64, f64, f64)>,
    pub seam: Option<(f64, f64)>,
    pub view: ViewTransform,
    pub viewport: Rect,
    pub shadow: bool,
    pub outline: bool,
    pub eraser: bool,
    pub alpha: f64,
    pub arc_len_start: f64,
    pub color: Color,
}

/// 接缝重绘几何(plan 不建模)。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MacosLiveSeam {
    pub from: Vec2,
    pub to: Vec2,
    pub width: f64,
}

/// 实时计划 + 接缝的重绘载荷(同进程 adapter 可直接消费, 无需 JSON)。
#[derive(Clone, Debug)]
pub struct MacosLivePlan {
    pub plan: DrawPlan,
    pub seam: Option<MacosLiveSeam>,
}

fn macos_live_seam(plan: &DrawPlan, parsed: &MacosLiveInput) -> Option<MacosLiveSeam> {
    if parsed.eraser || !parsed.outline {
        return None;
    }
    let points = &plan.points;
    let pair = match parsed.seam {
        Some((x, y)) if points.len() >= 2 => {
            let from = parsed.view.map(Vec2::new(x, y));
            from.is_finite().then(|| (from, points[0]))
        }
        None if points.len() >= 3 => Some((points[points.len() - 3].pos, points[points.len() - 2])),
        _ => None,
    };
    let (from, to) = pair?;
    Some(MacosLiveSeam {
        from,
        to: to.pos,
        width: to.width,
    })
}

pub fn build_macos_live_plan(input: MacosLiveInput) -> Result<MacosLivePlan, String> {
    let parsed = input;
    let mut params = PlanParams::new(parsed.view, parsed.viewport);
    params.order = PassOrder::MacosCommitted;
    params.scope = PlanScope::MacosLive;
    params.alpha = parsed.alpha;
    params.eraser = parsed.eraser;
    if parsed.shadow {
        params.shadow = Some(ShadowParams::macos(&parsed.view));
    }
    if parsed.outline {
        params.outline = Some(OutlineParams::macos_live(
            parsed.view.scale,
            parsed.arc_len_start,
        ));
    }
    params.live = Some(LiveStroke {
        points: &parsed.points,
        color: parsed.color,
    });
    let plan = draw_plan(&[], &params).map_err(|err| format!("计划生成失败: {err}"))?;
    let seam = macos_live_seam(&plan, &parsed);
    Ok(MacosLivePlan { plan, seam })
}

/// 输入 JSON → MacosLive 计划单行 JSON;失败返回 error 文本描述。
/// 整体不透明度合成:笔迹画进独立缓冲(逐段软件 alpha 混合), 再 blit 到
/// 目标表面。描边层同样按 alpha 合成, 保持与不透明渲染一致的层次。
///
/// D9c 未迁移:该路径走 pagerender 的软件光栅化(平台无关),与不透明快路径
/// 的「全局阴影 → 逐笔描边/墨迹」分层不同(D9a 差异 I);保持原实现以免改变
/// 渐显/隧道卡片的像素。
fn paint_strokes_soft(
    r: &crate::cairo_dl::CairoRenderer,
    strokes: &[Stroke],
    pan_x: f64,
    pan_y: f64,
    zoom: f64,
    scale: f64,
    y_shift: f64,
    shadow: bool,
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
            points: &s.points,
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
            shadow,
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
    ffi_guard((), || {
        state::mark_stroke_activity();
        modeler::begin_stroke(x, y, pressure, timestamp, width_scale);
        *RAW_STROKE_START.lock().unwrap_or_else(|e| e.into_inner()) = Some(timestamp);
        // Start DB stroke with correct color
        let id = runtime().block_on(db::begin_stroke(r, g, b, width_scale));
        state::buffer_point(x, y, pressure_to_width(pressure, width_scale), 0.0); // sync
        // Start STROKES entry
        let mut strokes = STROKES.lock().unwrap_or_else(|e| e.into_inner());
        strokes.push(Stroke {
            id,
            r,
            g,
            b,
            points: Vec::new(),
        });
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
    ffi_guard((), || {
        modeler::pen_move(x, y, pressure, timestamp, width_scale);
        let start = RAW_STROKE_START
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .unwrap_or(timestamp);
        state::buffer_point(
            x,
            y,
            pressure_to_width(pressure, width_scale),
            timestamp - start,
        );
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_modeler_end(
    x: c_double,
    y: c_double,
    pressure: c_double,
    timestamp: c_double,
    width_scale: c_double,
) {
    ffi_guard((), || {
        modeler::end_stroke(x, y, pressure, timestamp, width_scale);
        let start = RAW_STROKE_START
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .unwrap_or(timestamp);
        let rel_t = timestamp - start;
        if rel_t.is_finite() && rel_t > 0.0 {
            state::buffer_point(x, y, pressure_to_width(pressure, width_scale), rel_t); // sync
        }
        db::end_stroke_spawned();
    });
}

/// Commit the modeler buffer into STROKES. Call after drawing the buffer.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_modeler_commit_to_strokes(r: c_double, g: c_double, b: c_double) {
    ffi_guard((), || {
        crate::state::mark_stroke_activity();
        {
            let smoothed = modeler::take_buffer();
            let mut strokes = STROKES.lock().unwrap_or_else(|e| e.into_inner());
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
        bubbles::on_stroke_committed();
        crate::ocr::on_stroke_committed(crate::state::current_screen_id());
    });
}

/// Windows 直写路径(begin_stroke/add_point_t/end_stroke)的抬笔钩子扇出。
/// macOS 走 modeler_commit_to_strokes(内含同一组钩子);Windows 的平滑在
/// 覆盖层自己的 StrokeModeler 实例里完成,核心缓冲为空,故抬笔后单独触发:
/// 草稿通道推送 / 共享画布上行 / OCR 闲时登记,与 macOS 语义一致。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_notify_stroke_committed() {
    crate::state::mark_stroke_activity();
    ink_draft_on_stroke_committed();
    ink_share_on_stroke_committed();
    bubbles::on_stroke_committed();
    crate::ocr::on_stroke_committed(crate::state::current_screen_id());
}

/// 气泡消息「冷启动」分段的笔状态喂入(三个符号, macOS event tap 与
/// Windows HID 报告共用同一语义):
/// - `glaspen2_bubble_pen_activity`: 笔活动心跳(悬空移动/落笔/拖动/抬笔),
///   悬停构思期间持续刷新, 消息不被"静默超时"切断;
/// - `glaspen2_bubble_pen_back`: 笔回到感应区(macOS proximity enter),
///   距离开 <300ms 视为笔↔橡皮切换, 撤销冷标记;
/// - `glaspen2_bubble_pen_gone`: 笔离开感应区(macOS proximity exit /
///   无 proximity 平板的真鼠标移动回退), 下一次落笔必开新消息。
///
/// 气泡捕获总开关(壳层启动与「气泡消息」tab 开关变化时调用)。
/// 关 = 笔事件钩子立即返回、不落库、不读历史 —— 关闭的功能零成本。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_bubble_set_capture(on: c_int) {
    bubbles::set_capture_enabled(on != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_bubble_pen_activity() {
    bubbles::on_pen_activity();
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_bubble_pen_back() {
    bubbles::on_pen_back();
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_bubble_pen_gone() {
    bubbles::on_pen_gone();
}

/// 「写完自动新建一页」: 落笔瞬间(笔画尚未开始)由 shell 调用。若这一笔
/// 将开新消息(冷启动/静默超时, 设置 bubbleAutoNewPage × 气泡 tab 开启,
/// 仅翻页模式), 建新页并切换 current id, 返回新页 id 供 shell 翻页载入;
/// 0 = 无动作。详见 bubbles::auto_page_if_due。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_bubble_auto_page_if_due() -> i64 {
    bubbles::auto_page_if_due()
}

/// Eraser: remove strokes overlapped by the just-finished eraser stroke.
/// The eraser stroke itself produced no DB points; its pending DB row is
/// deleted here along with any hit strokes (memory + DB).
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_modeler_erase_finish() {
    let eraser_pts = modeler::take_buffer();

    let removed_ids: Vec<i64> = {
        let mut strokes = STROKES.lock().unwrap_or_else(|e| e.into_inner());
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
    STROKES.lock().unwrap_or_else(|e| e.into_inner()).len() as c_int
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_get_stroke_point_count(idx: c_int) -> c_int {
    let strokes = STROKES.lock().unwrap_or_else(|e| e.into_inner());
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
    let strokes = STROKES.lock().unwrap_or_else(|e| e.into_inner());
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
    let strokes = STROKES.lock().unwrap_or_else(|e| e.into_inner());
    strokes.get(idx as usize).map_or(1.0, |s| s.avg_width())
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_get_stroke_point(
    idx: c_int,
    pidx: c_int,
    x: *mut c_double,
    y: *mut c_double,
) {
    let strokes = STROKES.lock().unwrap_or_else(|e| e.into_inner());
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
    let strokes = STROKES.lock().unwrap_or_else(|e| e.into_inner());
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
// setSetting effect 桥(D1 单源;只服务 macOS ObjC, Windows 直接调 Rust)
// ---------------------------------------------------------------------------

/// 解析 + 钳制 + 落库一次完成, 返回平台无关的 effect JSON 字符串。
///
/// 形状: `{"kind":"<变体名>", "bool"|"int"|"float"|"str"|"json": ...}`;
/// 失败: `{"error":"..."}`(未知/只读键、值不合法、value_json 不是 JSON)。
/// ObjC 侧只按 `kind` 做本平台运行时副作用, 不再自己解析标量/钳制/落库。
/// 返回值用 `glaspen2_free_c_string` 释放(NULL = 参数为空)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_settings_apply(
    key: *const c_char,
    value_json: *const c_char,
) -> *mut c_char {
    let error = |msg: String| return_c_string(serde_json::json!({ "error": msg }).to_string());
    let Some(key) = (unsafe { borrow_cstr(key) }) else {
        return error("setSetting 缺少 key".to_string());
    };
    let Some(raw) = (unsafe { borrow_cstr(value_json) }) else {
        return error("setSetting 缺少 value_json".to_string());
    };
    let value: serde_json::Value = match serde_json::from_str(raw) {
        Ok(value) => value,
        Err(e) => return error(format!("value_json 不是合法 JSON: {e}")),
    };
    match crate::settings_apply::apply_setting(key, &value) {
        Ok(effect) => return_c_string(effect.to_json().to_string()),
        Err(e) => error(e),
    }
}

// ---------------------------------------------------------------------------
// 面板设置快照桥(D12 单源;只服务 macOS ObjC, Windows 同进程直接调 Rust)
// ---------------------------------------------------------------------------

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
                    tracing::error!("launch agent write failed: {}", e);
                    0
                }
            }
        } else {
            match std::fs::remove_file(&plist_path) {
                Ok(_) => 1,
                Err(e) => {
                    tracing::error!("launch agent remove failed: {}", e);
                    0
                }
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        let Some(exe) = std::env::current_exe()
            .ok()
            .map(|p| p.to_string_lossy().to_string())
        else {
            tracing::error!("launch-at-login: current_exe 不可用");
            return 0;
        };
        const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
        let ok = if enable != 0 {
            reg_ok(&[
                "add", RUN_KEY, "/v", "Glaspen2", "/t", "REG_SZ", "/d", &exe, "/f",
            ])
        } else {
            // 值不存在时 reg delete 返回非零, 也算"已处于关闭态"= 成功
            reg_ok(&["delete", RUN_KEY, "/v", "Glaspen2", "/f"])
                || !reg_ok(&["query", RUN_KEY, "/v", "Glaspen2"])
        };
        if !ok {
            tracing::error!("launch-at-login: reg.exe 写注册表失败");
        }
        if ok { 1 } else { 0 }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        0
    }
}

/// reg.exe 子进程封装(避免为此引 windows crate 依赖; reg 在所有 Windows 上常驻)。
#[cfg(target_os = "windows")]
fn reg_ok(args: &[&str]) -> bool {
    match std::process::Command::new("reg").args(args).output() {
        Ok(o) => o.status.success(),
        Err(e) => {
            tracing::error!("launch-at-login: reg {:?} 失败: {e}", args.first());
            false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_is_launch_at_login() -> c_int {
    #[cfg(target_os = "macos")]
    {
        if launch_agent_plist().exists() { 1 } else { 0 }
    }
    #[cfg(target_os = "windows")]
    {
        if reg_ok(&[
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            "/v",
            "Glaspen2",
        ]) {
            1
        } else {
            0
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
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

// ── chord 单源 FFI(D2): macOS 壳经此使用 core::chord; Windows 直接调 Rust ──

unsafe fn borrow_cstr<'a>(p: *const c_char) -> Option<&'a str> {
    if p.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(p) }.to_str().ok()
}

fn return_c_string(s: String) -> *mut c_char {
    match CString::new(s) {
        Ok(cs) => cs.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// 规范化 chord; null = 非法, 空串 = 未绑定。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_chord_normalize(raw: *const c_char) -> *mut c_char {
    let Some(raw) = (unsafe { borrow_cstr(raw) }) else {
        return std::ptr::null_mut();
    };
    match crate::chord::normalize(raw) {
        Some(s) => return_c_string(s),
        None => std::ptr::null_mut(),
    }
}

/// 旧默认迁移; null = 无需迁移(保持原值)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_chord_migrate_value(
    action: *const c_char,
    raw: *const c_char,
) -> *mut c_char {
    let (Some(action), Some(raw)) = (unsafe { borrow_cstr(action) }, unsafe { borrow_cstr(raw) })
    else {
        return std::ptr::null_mut();
    };
    match crate::chord::migrate_value(crate::ui::Platform::Macos, action, raw) {
        Some(s) => return_c_string(s),
        None => std::ptr::null_mut(),
    }
}

/// 该 chord 是否为 macOS 保留组合。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_chord_reserved(chord: *const c_char) -> c_int {
    let Some(chord) = (unsafe { borrow_cstr(chord) }) else {
        return 0;
    };
    crate::chord::is_reserved(crate::ui::Platform::Macos, chord) as c_int
}

// ── 镜头数学单源 FFI(D5b): macOS 壳经此使用 core::camera ──
//
// 与 D1b/D3b 同形: 输入是 ObjC 侧持有的镜头三值(pan_x/pan_y/zoom)与本次操作
// 参数, 输出是 core 算好的新三值 JSON(ObjC 解析后写回 `g_pan_x/g_pan_y/g_zoom`)。
// 镜头**状态**仍归 adapter;这里只做数学。缩放结果额外带 `hit_max/hit_min/
// changed` 三个标记, 让两端各自保留本平台的提示/早退规则(见 camera.rs 模块
// 文档差异 2)。JSON 一律单行(ObjC 用 NSJSONSerialization 解析), 永不返回
// NULL: 非有限数输入回 `{"error": ...}` 而不是 panic/UB。
// Windows 同进程直接调 `Camera`, 不经这些 JSON 入口。

/// 镜头三值 JSON;非有限数(被除零/NaN 输入污染)回 error JSON, 永不上抛 panic。
///
/// `zoom_outcome` 为 Some 时附带 `hit_max/hit_min/changed`(`glaspen2_camera_zoom_at`
/// / `zoom_at_center`);pan/centered 传 None。
fn camera_result_json(
    pan_x: f64,
    pan_y: f64,
    zoom: f64,
    zoom_outcome: Option<ZoomOutcome>,
) -> *mut c_char {
    if !pan_x.is_finite() || !pan_y.is_finite() || !zoom.is_finite() {
        return return_c_string(
            serde_json::json!({ "error": "镜头结果非法(需要有限数)" }).to_string(),
        );
    }
    let mut value = serde_json::json!({
        "pan_x": pan_x,
        "pan_y": pan_y,
        "zoom": zoom,
    });
    if let Some(out) = zoom_outcome {
        value["hit_max"] = serde_json::Value::Bool(out.hit_max);
        value["hit_min"] = serde_json::Value::Bool(out.hit_min);
        value["changed"] = serde_json::Value::Bool(out.changed());
    }
    return_c_string(value.to_string())
}

/// 平移镜头(两壳 `canvas_pan_by` 的数学): 返回 `{"pan_x","pan_y","zoom"}`。
///
/// `dx/dy` 是画布单位增量(调用方已按 zoom 折算: 键盘 `80/z`、鼠标拖动
/// `Δ/z`);正 dx = 内容向右。书写中忽略/仅无限画布生效的门控留在 adapter。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_camera_pan(
    pan_x: c_double,
    pan_y: c_double,
    zoom: c_double,
    dx: c_double,
    dy: c_double,
) -> *mut c_char {
    let mut cam = Camera::new(pan_x, pan_y, zoom);
    cam.pan_by(dx, dy);
    camera_result_json(cam.pan_x, cam.pan_y, cam.zoom, None)
}

/// 以视口点 `(vx, vy)` 为锚缩放(滚轮/触控板, 锚点 = 鼠标位置)。
/// 返回 `{"pan_x","pan_y","zoom","hit_max","hit_min","changed"}`。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_camera_zoom_at(
    pan_x: c_double,
    pan_y: c_double,
    zoom: c_double,
    factor: c_double,
    vx: c_double,
    vy: c_double,
) -> *mut c_char {
    let mut cam = Camera::new(pan_x, pan_y, zoom);
    let out = cam.zoom_at(factor, vx, vy);
    camera_result_json(cam.pan_x, cam.pan_y, cam.zoom, Some(out))
}

/// 以视口中心为锚缩放(键盘镜头缩放两端同款;macOS 传 `g_screen_w/h`)。
/// 返回形状同 [glaspen2_camera_zoom_at]。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_camera_zoom_at_center(
    pan_x: c_double,
    pan_y: c_double,
    zoom: c_double,
    factor: c_double,
    viewport_w: c_double,
    viewport_h: c_double,
) -> *mut c_char {
    let mut cam = Camera::new(pan_x, pan_y, zoom);
    let out = cam.zoom_at_center(factor, viewport_w, viewport_h);
    camera_result_json(cam.pan_x, cam.pan_y, cam.zoom, Some(out))
}

/// 镜头居中到内容包围盒(面板「居中内容」/ `canvasCenter`)。
/// 返回 `{"pan_x","pan_y","zoom"}`, 其中 zoom = **公式实际用的 z**
/// ([Camera::centered_on] 的 `used_zoom`, 越界 zoom 会兜底成 1.0);
/// 只改 pan, 是否把 zoom 写回由 adapter 决定(macOS 不写, Windows 写)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_camera_centered(
    bx: c_double,
    by: c_double,
    bx2: c_double,
    by2: c_double,
    viewport_w: c_double,
    viewport_h: c_double,
    zoom: c_double,
) -> *mut c_char {
    let mut cam = Camera::new(0.0, 0.0, zoom);
    let used_zoom = cam.centered_on((bx, by, bx2, by2), (viewport_w, viewport_h));
    camera_result_json(cam.pan_x, cam.pan_y, used_zoom, None)
}

/// 交互公式用的 zoom 兜底: `zoom > 0.05 ? zoom : 1.0`([Camera::step_zoom])。
///
/// macOS 鼠标抓手每帧把屏幕位移折算成画布增量(`Δ / z`)时用;标量返回避免
/// 高频拖动走 JSON/堆分配。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_camera_step_zoom(zoom: c_double) -> c_double {
    Camera::new(0.0, 0.0, zoom).step_zoom()
}

/// 键盘镜头平移每步的画布增量 = `80 / step_zoom`([Camera::keyboard_pan_step])。
///
/// 只有 macOS 用(Windows 同进程直接调);方向符号(哪边是正)留在 adapter:
/// macOS 是"视野移动"(Left → +step), Windows 是"内容跟随"(Left → -step)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_camera_keyboard_pan_step(zoom: c_double) -> c_double {
    Camera::new(0.0, 0.0, zoom).keyboard_pan_step()
}

/// 镜头回原点 + 100%(`canvas_reset_lens` / 菜单「回到原点」「新建画布」)。
///
/// out 参数可传 NULL(跳过该字段);不分配、无返回值。数学常量唯一来源是
/// [Camera::HOME]。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_camera_home(
    pan_x: *mut c_double,
    pan_y: *mut c_double,
    zoom: *mut c_double,
) {
    let home = Camera::HOME;
    unsafe {
        if !pan_x.is_null() {
            *pan_x = home.pan_x;
        }
        if !pan_y.is_null() {
            *pan_y = home.pan_y;
        }
        if !zoom.is_null() {
            *zoom = home.zoom;
        }
    }
}

// ── 录制状态机桥单源 FFI(D8b): macOS ObjC 不能持 Rust 结构体 ──
//
// 两个槽位(adapter 在 **main runloop 线程** 调用, 后台结果 dispatch 回主线程
// 再喂回 —— 状态机不是 Sync, 也不能跨线程共享):
//   0 = GIF(⌘⌃R);
//   1 = 手写消息(⌘⌃2 草稿 / ⌘⌃3 直发, 同一机器内互斥, 受总开关门控)。
// Windows 同进程直接持两个 recording::RecordingMachine, 不经这些 JSON 入口。
//
// 事件/effects JSON 形状见 crate::recording 的 "adapter JSON 编解码" 注释
// (单行, ObjC 用 NSJSONSerialization 解析); 永不返回 NULL, 非法槽位/坏 JSON
// 一律回 {"error": ...}。返回值用 glaspen2_free_c_string 释放。

static RECORDING_MACHINES: std::sync::LazyLock<
    std::sync::Mutex<[Option<crate::recording::RecordingMachine>; 2]>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new([None, None]));

/// adapter 直接喂 typed 事件(macOS Rust shell 用; 不经过事件 JSON)。
pub fn recording_handle_typed(
    slot: c_int,
    event: crate::recording::RecordingEvent,
) -> Vec<crate::recording::RecordingEffect> {
    let Some(idx) = recording_slot(slot) else {
        return Vec::new();
    };
    let mut machines = RECORDING_MACHINES.lock().unwrap_or_else(|e| e.into_inner());
    let machine = machines[idx].get_or_insert_with(Default::default);
    machine.handle(event)
}

/// typed 复位(保留总开关镜像)。
pub fn recording_reset_typed(slot: c_int) {
    let Some(idx) = recording_slot(slot) else {
        return;
    };
    let mut machines = RECORDING_MACHINES.lock().unwrap_or_else(|e| e.into_inner());
    let chat_integration = machines[idx]
        .as_ref()
        .is_some_and(crate::recording::RecordingMachine::chat_integration);
    machines[idx] = Some(crate::recording::RecordingMachine::new(chat_integration));
}

fn recording_slot(slot: c_int) -> Option<usize> {
    match slot {
        0 => Some(0),
        1 => Some(1),
        _ => None,
    }
}

fn recording_error_json(message: String) -> *mut c_char {
    return_c_string(serde_json::json!({ "error": message }).to_string())
}

/// 初始化(或重建)一个槽位的状态机; chat_integration = 手写消息总开关当前值
/// (槽 1 用, GIF 槽忽略)。启动时调用一次即可; 事件入口也允许懒初始化。
/// 返回 1 = 成功, 0 = 非法槽位。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_recording_init(slot: c_int, chat_integration: c_int) -> c_int {
    let Some(idx) = recording_slot(slot) else {
        return 0;
    };
    let mut machines = RECORDING_MACHINES.lock().unwrap_or_else(|e| e.into_inner());
    machines[idx] = Some(crate::recording::RecordingMachine::new(
        chat_integration != 0,
    ));
    1
}

/// 复位一个槽位到空闲(保留手写消息总开关镜像; 退出/异常收尾用)。
/// 返回 1 = 成功, 0 = 非法槽位。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_recording_reset(slot: c_int) -> c_int {
    let Some(idx) = recording_slot(slot) else {
        return 0;
    };
    let mut machines = RECORDING_MACHINES.lock().unwrap_or_else(|e| e.into_inner());
    let chat_integration = machines[idx]
        .as_ref()
        .is_some_and(crate::recording::RecordingMachine::chat_integration);
    machines[idx] = Some(crate::recording::RecordingMachine::new(chat_integration));
    1
}

/// 当前录制模式: 0 = 空闲, 1 = GIF, 2 = 草稿, 3 = 直发, -1 = 非法槽位。
///
/// ObjC 的 key-up 分支靠它判断"这次抬起是否属于本槽位"(避免把普通 r/2/3
/// 抬键吞掉), 等价于两端旧标志位只做状态查询的那部分。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_recording_mode(slot: c_int) -> c_int {
    use crate::recording::RecordingMode;
    let Some(idx) = recording_slot(slot) else {
        return -1;
    };
    let machines = RECORDING_MACHINES.lock().unwrap_or_else(|e| e.into_inner());
    match machines[idx]
        .as_ref()
        .and_then(|machine| machine.state().mode())
    {
        None => 0,
        Some(RecordingMode::Gif) => 1,
        Some(RecordingMode::Draft) => 2,
        Some(RecordingMode::Send) => 3,
    }
}

// ── 文案表 FFI(D10a): macOS ObjC 经此查 core::i18n, Windows 同进程直接调 ──
//
// 单行纯文本, **永不返回 NULL**: 未知 key 回显 key 本身(便于发现漏登记/
// adapter 拼错), null/非法 UTF-8 的 key 回 "?"; lang 缺省/未知一律中文
// (与两端现状一致)。返回值用 glaspen2_free_c_string 释放。
// Windows 侧不经过这个 FFI(同进程直接 crate::i18n::text), 与 D6/D7/D8 同形。

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_i18n(msg_key: *const c_char, lang: *const c_char) -> *mut c_char {
    use crate::i18n::{Lang, by_key};
    let lang = unsafe { borrow_cstr(lang) }.map_or(Lang::Zh, Lang::from_code);
    let text = match unsafe { borrow_cstr(msg_key) } {
        Some(key) => by_key(key).map_or_else(|| key.to_string(), |msg| msg.text(lang).to_string()),
        None => "?".to_string(),
    };
    return_c_string(text)
}

// ── 时光隧道几何(Swift shell → Rust, R1 首个迁移切片) ──

/// 由页距计算卡片位置/尺寸/透明度; 写 out[0..5] 并返回 1, 不可见返回 0。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_tunnel_place(
    zc: c_double,
    k: c_double,
    travel: c_double,
    surf_w: c_double,
    surf_h: c_double,
    out: *mut c_double,
) -> c_int {
    if out.is_null() {
        return 0;
    }
    match crate::tunnel::place(zc, k, travel, surf_w, surf_h) {
        Some(p) => {
            let out = unsafe { std::slice::from_raw_parts_mut(out, 5) };
            out[0] = p.cx;
            out[1] = p.cy;
            out[2] = p.w;
            out[3] = p.h;
            out[4] = p.alpha;
            1
        }
        None => 0,
    }
}

/// 隧道最前页距(唯一来源; Swift 动画循环用它做可见性门控)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_tunnel_front() -> c_double {
    crate::tunnel::FRONT
}

// ── 预设显示名(R2: Swift MenuNames 表迁 Rust) ──

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_color_name(index: c_int, lang: c_int) -> *mut c_char {
    let lang = if lang == 1 {
        crate::i18n::Lang::En
    } else {
        crate::i18n::Lang::Zh
    };
    let text = if index < 0 {
        ""
    } else {
        crate::i18n::color_name(index as usize, lang)
    };
    return_c_string(text.to_string())
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_width_name(index: c_int, lang: c_int) -> *mut c_char {
    let lang = if lang == 1 {
        crate::i18n::Lang::En
    } else {
        crate::i18n::Lang::Zh
    };
    let text = if index < 0 {
        ""
    } else {
        crate::i18n::width_name(index as usize, lang)
    };
    return_c_string(text.to_string())
}

// ── 壳全局状态(R6: Swift AppState → Rust) ──

#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_screen_w() -> c_double {
    crate::shell_state::screen_w() as f64
}

#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_screen_h() -> c_double {
    crate::shell_state::screen_h() as f64
}

#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_screen_size(w: c_int, h: c_int) {
    crate::shell_state::set_screen_size(w, h);
}

#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_lang() -> c_int {
    crate::shell_state::lang()
}

#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_lang(value: c_int) {
    crate::shell_state::set_lang(value);
}

#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_startup_set_lang(lang: c_int) {
    crate::shell_state::startup_set_lang(lang);
}

#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
pub extern "C" fn sw_app_screen_w() -> c_int {
    crate::shell_state::screen_w()
}

#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
pub extern "C" fn sw_app_screen_h() -> c_int {
    crate::shell_state::screen_h()
}

// ── 性能日志(R3: Swift PerfLog → Rust) ──

#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_perf_init_from_env() {
    crate::perf::init_from_env();
}

#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_perf_is_enabled() -> c_int {
    i32::from(crate::perf::is_enabled())
}

#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_perf_elapsed_us(since: u64) -> u64 {
    crate::perf::elapsed_us(since)
}

#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_perf_record(name: *const c_char, dur_us: u64, notes: *const c_char) {
    let name = if name.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(name) }
            .to_string_lossy()
            .into_owned()
    };
    let notes = if notes.is_null() {
        None
    } else {
        Some(
            unsafe { CStr::from_ptr(notes) }
                .to_string_lossy()
                .into_owned(),
        )
    };
    crate::perf::record(&name, dur_us, notes.as_deref());
}

#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_perf_summary() {
    crate::perf::summary();
}

// ── 快捷键生效表状态(R8: Swift ShortcutStore → Rust) ──

#[cfg(target_os = "macos")]
fn parse_shortcut_map_pb(ptr: *const u8, len: usize) -> std::collections::BTreeMap<String, String> {
    if ptr.is_null() || len == 0 {
        return std::collections::BTreeMap::new();
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    glaspen_ipc::decode_string_map(bytes).unwrap_or_default()
}

#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_shortcut_store_install(
    values_ptr: *const u8,
    values_len: usize,
    lookup_ptr: *const u8,
    lookup_len: usize,
) {
    crate::shortcut_state::install(
        parse_shortcut_map_pb(values_ptr, values_len),
        parse_shortcut_map_pb(lookup_ptr, lookup_len),
    );
}

#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_shortcut_store_chord(action: *const c_char) -> *mut c_char {
    if action.is_null() {
        return std::ptr::null_mut();
    }
    let action = unsafe { CStr::from_ptr(action) }.to_string_lossy();
    match crate::shortcut_state::chord(&action) {
        Some(chord) => return_c_string(chord),
        None => std::ptr::null_mut(),
    }
}

#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_shortcut_store_action_for_chord(chord: *const c_char) -> *mut c_char {
    if chord.is_null() {
        return std::ptr::null_mut();
    }
    let chord = unsafe { CStr::from_ptr(chord) }.to_string_lossy();
    match crate::shortcut_state::action_for_chord(&chord) {
        Some(action) => return_c_string(action),
        None => std::ptr::null_mut(),
    }
}

#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_shortcut_store_values_pb(out_len: *mut usize) -> *mut u8 {
    let values = crate::shortcut_state::values();
    let bytes = glaspen_ipc::encode_string_map(&values);
    if !out_len.is_null() {
        unsafe { *out_len = bytes.len() };
    }
    if bytes.is_empty() {
        return std::ptr::null_mut();
    }
    let ptr = unsafe { libc::malloc(bytes.len()) as *mut u8 };
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len()) };
    ptr
}

#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_shortcut_store_is_empty() -> c_int {
    i32::from(crate::shortcut_state::is_empty())
}

// ── chord 修饰键掩码(R7: Swift ShortcutMask → Rust) ──

#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_chord_modifier_mask(chord: *const c_char) -> u64 {
    if chord.is_null() {
        return 0;
    }
    let chord = unsafe { CStr::from_ptr(chord) }.to_string_lossy();
    crate::chord::appkit_modifier_mask(&chord)
}

// ── 镜头结果直出(R5: 去掉 Swift JSON 解析) ──

fn write_camera_out(
    pan_x: f64,
    pan_y: f64,
    zoom: f64,
    out_x: *mut c_double,
    out_y: *mut c_double,
    out_z: *mut c_double,
) -> bool {
    if out_x.is_null() || out_y.is_null() || out_z.is_null() {
        return false;
    }
    if !pan_x.is_finite() || !pan_y.is_finite() || !zoom.is_finite() {
        return false;
    }
    unsafe {
        *out_x = pan_x;
        *out_y = pan_y;
        *out_z = zoom;
    }
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_camera_pan_values(
    pan_x: c_double,
    pan_y: c_double,
    zoom: c_double,
    dx: c_double,
    dy: c_double,
    out_x: *mut c_double,
    out_y: *mut c_double,
    out_z: *mut c_double,
) -> c_int {
    let mut cam = Camera::new(pan_x, pan_y, zoom);
    cam.pan_by(dx, dy);
    i32::from(write_camera_out(
        cam.pan_x, cam.pan_y, cam.zoom, out_x, out_y, out_z,
    ))
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_camera_zoom_at_values(
    pan_x: c_double,
    pan_y: c_double,
    zoom: c_double,
    factor: c_double,
    vx: c_double,
    vy: c_double,
    out_x: *mut c_double,
    out_y: *mut c_double,
    out_z: *mut c_double,
    out_hit_max: *mut c_int,
    out_hit_min: *mut c_int,
    out_changed: *mut c_int,
) -> c_int {
    let mut cam = Camera::new(pan_x, pan_y, zoom);
    let out = cam.zoom_at(factor, vx, vy);
    if !write_camera_out(cam.pan_x, cam.pan_y, cam.zoom, out_x, out_y, out_z) {
        return 0;
    }
    if !out_hit_max.is_null() {
        unsafe { *out_hit_max = i32::from(out.hit_max) };
    }
    if !out_hit_min.is_null() {
        unsafe { *out_hit_min = i32::from(out.hit_min) };
    }
    if !out_changed.is_null() {
        unsafe { *out_changed = i32::from(out.changed()) };
    }
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_camera_zoom_at_center_values(
    pan_x: c_double,
    pan_y: c_double,
    zoom: c_double,
    factor: c_double,
    viewport_w: c_double,
    viewport_h: c_double,
    out_x: *mut c_double,
    out_y: *mut c_double,
    out_z: *mut c_double,
    out_hit_max: *mut c_int,
    out_hit_min: *mut c_int,
    out_changed: *mut c_int,
) -> c_int {
    let mut cam = Camera::new(pan_x, pan_y, zoom);
    let out = cam.zoom_at_center(factor, viewport_w, viewport_h);
    if !write_camera_out(cam.pan_x, cam.pan_y, cam.zoom, out_x, out_y, out_z) {
        return 0;
    }
    if !out_hit_max.is_null() {
        unsafe { *out_hit_max = i32::from(out.hit_max) };
    }
    if !out_hit_min.is_null() {
        unsafe { *out_hit_min = i32::from(out.hit_min) };
    }
    if !out_changed.is_null() {
        unsafe { *out_changed = i32::from(out.changed()) };
    }
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_camera_centered_values(
    bx: c_double,
    by: c_double,
    bx2: c_double,
    by2: c_double,
    viewport_w: c_double,
    viewport_h: c_double,
    zoom: c_double,
    out_x: *mut c_double,
    out_y: *mut c_double,
    out_used_zoom: *mut c_double,
) -> c_int {
    let mut cam = Camera::new(0.0, 0.0, zoom);
    let used_zoom = cam.centered_on((bx, by, bx2, by2), (viewport_w, viewport_h));
    i32::from(write_camera_out(
        cam.pan_x,
        cam.pan_y,
        used_zoom,
        out_x,
        out_y,
        out_used_zoom,
    ))
}

// ── 画布输入变换(R4: Swift canvas_input_x/y → Rust) ──

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_canvas_input(
    view_x: c_double,
    view_y: c_double,
    infinite: c_int,
    zoom: c_double,
    pan_x: c_double,
    pan_y: c_double,
    page_off_x: c_double,
    page_off_y: c_double,
    is_fit: c_int,
    paper_ox: c_double,
    paper_oy: c_double,
    paper_scale: c_double,
    out_x: *mut c_double,
    out_y: *mut c_double,
) -> c_int {
    if out_x.is_null() || out_y.is_null() {
        return 0;
    }
    let (x, y) = if infinite != 0 {
        (
            crate::canvas_input::infinite_input(view_x, zoom, pan_x),
            crate::canvas_input::infinite_input(view_y, zoom, pan_y),
        )
    } else {
        let fit = is_fit != 0;
        (
            crate::canvas_input::page_input(view_x, page_off_x, fit, paper_ox, paper_scale),
            crate::canvas_input::page_input(view_y, page_off_y, fit, paper_oy, paper_scale),
        )
    };
    unsafe {
        *out_x = x;
        *out_y = y;
    }
    1
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
        let _g = crate::tests::TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        assert_eq!(glaspen2_ink_draft_start(1920, 1080), 1);
        assert_eq!(
            glaspen2_ink_draft_start(1920, 1080),
            0,
            "会话进行中应拒绝二连开"
        );
        STROKES
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(Stroke {
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
        STROKES.lock().unwrap_or_else(|e| e.into_inner()).pop();
    }

    /// 描边对比色:亮色(黄/白)配黑边,暗色(蓝/黑)配白边。
    #[test]
    fn outline_contrast_follows_luminance() {
        assert_eq!(outline_contrast_color(1.0, 1.0, 0.0), (0, 0, 0)); // 黄
        assert_eq!(outline_contrast_color(1.0, 1.0, 1.0), (0, 0, 0)); // 白
        assert_eq!(outline_contrast_color(0.11, 0.44, 0.85), (255, 255, 255)); // 蓝
        assert_eq!(outline_contrast_color(0.0, 0.0, 0.0), (255, 255, 255)); // 黑
    }

    // ------------------------------------------------------------------
    // D9c 第二步像素回归:plan 光栅化 vs 旧实现(测试内 oracle)
    // ------------------------------------------------------------------

    /// 旧 paint_strokes_into 的不透明快路径(D9c 前的实现,逐字复刻;
    /// 仅把 StrokeView 换成 Stroke、OUTLINE_PAD 内联为 1.0)。作为像素基准,
    /// 防止之后有人"顺手统一"掉 macOS 的分层/取宽规则。
    fn legacy_paint_strokes_into(
        r: &crate::cairo_dl::CairoRenderer,
        strokes: &[Stroke],
        pan_x: f64,
        pan_y: f64,
        zoom: f64,
        scale: f64,
        y_shift: f64,
        shadow: bool,
        outline: bool,
    ) {
        const OUTLINE_PAD: f64 = 1.0;
        if shadow {
            const STEPS: [(f64, f64); 3] = [(1.5, 0.20), (3.0, 0.13), (5.0, 0.07)];
            for (spread, sa) in STEPS {
                for s in strokes {
                    let pts = &s.points;
                    if pts.len() < 2 {
                        continue;
                    }
                    for i in 1..pts.len() {
                        let (x, y, w, _t) = pts[i];
                        let (px, py, _pw, _pt) = pts[i - 1];
                        r.stroke_line_alpha(
                            ((px - pan_x) * zoom * scale) as f32,
                            ((py + y_shift - pan_y) * zoom * scale) as f32,
                            ((x - pan_x) * zoom * scale) as f32,
                            ((y + y_shift - pan_y) * zoom * scale) as f32,
                            ((w + spread * 2.0) * zoom * scale) as f32,
                            (0, 0, 0),
                            sa,
                        );
                    }
                }
            }
        }
        for s in strokes {
            let pts = &s.points;
            if pts.len() < 2 {
                continue;
            }
            let color = (
                (s.r.clamp(0.0, 1.0) * 255.0) as u8,
                (s.g.clamp(0.0, 1.0) * 255.0) as u8,
                (s.b.clamp(0.0, 1.0) * 255.0) as u8,
            );
            if outline {
                const OL_BLACK: (u8, u8, u8) = (0, 0, 0);
                const OL_WHITE: (u8, u8, u8) = (255, 255, 255);
                let d = (zoom * scale).max(1.0);
                r.set_line_cap_butt();
                let mut cum = 0.0f64;
                for i in 1..pts.len() {
                    let (x, y, w, _t) = pts[i];
                    let (px, py, _pw, _pt) = pts[i - 1];
                    let x0 = ((px - pan_x) * zoom * scale) as f32;
                    let y0 = ((py + y_shift - pan_y) * zoom * scale) as f32;
                    let x1 = ((x - pan_x) * zoom * scale) as f32;
                    let y1 = ((y + y_shift - pan_y) * zoom * scale) as f32;
                    let segw = ((w + OUTLINE_PAD * 2.0) * zoom * scale) as f32;
                    let off = cum.rem_euclid(2.0 * d);
                    r.set_dash(d, off);
                    r.stroke_line(x0, y0, x1, y1, segw, OL_BLACK);
                    r.set_dash(d, (off + d).rem_euclid(2.0 * d));
                    r.stroke_line(x0, y0, x1, y1, segw, OL_WHITE);
                    let (dx, dy) = (x1 - x0, y1 - y0);
                    cum += ((dx * dx + dy * dy) as f64).sqrt();
                }
                r.set_dash(0.0, 0.0);
                r.set_line_cap_round();
            }
            for i in 0..pts.len() {
                let (x, y, w, _t) = pts[i];
                if i == 0 {
                    r.fill_circle_opt(
                        ((x - pan_x) * zoom * scale) as f32,
                        ((y + y_shift - pan_y) * zoom * scale) as f32,
                        (w * 0.5 * zoom * scale) as f32,
                        Some(color),
                    );
                } else {
                    let (px, py, _pw, _pt) = pts[i - 1];
                    r.stroke_line_opt(
                        ((px - pan_x) * zoom * scale) as f32,
                        ((py + y_shift - pan_y) * zoom * scale) as f32,
                        ((x - pan_x) * zoom * scale) as f32,
                        ((y + y_shift - pan_y) * zoom * scale) as f32,
                        (w * zoom * scale) as f32,
                        Some(color),
                    );
                }
            }
        }
    }

    fn surface_bytes(r: &crate::cairo_dl::CairoRenderer) -> Vec<u8> {
        r.flush();
        unsafe { std::slice::from_raw_parts(r.bits(), (r.w * r.h * 4) as usize).to_vec() }
    }

    fn pixel_fixture_strokes() -> Vec<Stroke> {
        vec![
            Stroke {
                id: 1,
                r: 0.0,
                g: 0.0,
                b: 0.0,
                points: vec![
                    (60.0, 80.0, 4.0, 0.0),
                    (90.0, 96.0, 10.0, 0.05),
                    (130.0, 110.0, 2.5, 0.1),
                    (160.0, 120.0, 14.0, 0.15),
                ],
            },
            Stroke {
                id: 2,
                r: 1.0,
                g: 0.0,
                b: 0.0,
                points: vec![
                    (70.0, 130.0, 6.0, 0.2),
                    (110.0, 140.0, 3.0, 0.25),
                    (170.0, 100.0, 18.0, 0.3),
                ],
            },
            Stroke {
                id: 3,
                r: 0.2,
                g: 0.6,
                b: 1.0,
                points: vec![(200.0, 200.0, 12.0, 0.0), (210.0, 210.0, 12.0, 0.02)],
            },
            // 单点笔迹:两端整页重建都不画(计划跳过 < 2 点)。
            Stroke {
                id: 4,
                r: 0.0,
                g: 1.0,
                b: 0.0,
                points: vec![(40.0, 40.0, 8.0, 0.0)],
            },
        ]
    }

    /// 同一组笔迹走旧实现与 plan 光栅化,离屏表面逐字节比对。
    ///
    /// 无 cairo 的动态加载环境(非 macOS CI/裸机)自动跳过:与两端渲染入口
    /// 的 cairo 缺失降级一致。
    #[test]
    fn plan_rasterization_matches_legacy_pixels() {
        use crate::cairo_dl::CairoRenderer;
        const W: i32 = 320;
        const H: i32 = 400;
        let strokes = pixel_fixture_strokes();
        let fixtures: [(&str, f64, f64, f64, f64, f64, bool, bool); 5] = [
            // (名称, pan_x, pan_y, zoom, scale, y_shift, shadow, outline)
            ("retina+全部 pass", 30.0, 40.0, 1.0, 2.0, 0.0, true, true),
            ("zoom<1", 12.5, 22.25, 0.7, 2.0, 0.0, true, true),
            ("仅描边", 30.0, 40.0, 1.0, 2.0, 0.0, false, true),
            ("仅阴影", 30.0, 40.0, 1.0, 2.0, 0.0, true, false),
            ("邻页 y_shift", 30.0, 40.0, 1.0, 2.0, 150.0, true, true),
        ];
        for (name, pan_x, pan_y, zoom, scale, y_shift, shadow, outline) in fixtures {
            let Some(legacy) = CairoRenderer::create_owned(W, H) else {
                // 无 cairo 的动态加载环境(裸机/Windows CI)自动跳过 —— 与两端
                // 真实渲染入口在 cairo 缺失时的降级一致。
                return;
            };
            legacy_paint_strokes_into(
                &legacy, &strokes, pan_x, pan_y, zoom, scale, y_shift, shadow, outline,
            );
            let want = surface_bytes(&legacy);
            let Some(planned) = CairoRenderer::create_owned(W, H) else {
                return;
            };
            paint_strokes_into(
                &planned, &strokes, pan_x, pan_y, zoom, scale, y_shift, shadow, outline, 1.0, None,
            );
            let got = surface_bytes(&planned);
            assert_eq!(want.len(), got.len(), "{name}: 缓冲尺寸");
            let diff = want.iter().zip(got.iter()).filter(|(a, b)| a != b).count();
            assert_eq!(
                diff,
                0,
                "{name}: plan 光栅化与旧实现出现 {diff} 字节像素差异({}/{})",
                diff,
                want.len()
            );
        }
    }

    /// D9c 第二步:macOS 整页重建的生产参数([macos_paint_plan])必须复现
    /// D9a 锁定的 macOS 命令序列(阴影层全局先行 → 逐笔「描边 → 墨迹」),
    /// 即本文件 paint_strokes_into 现在执行的计划。
    #[test]
    fn macos_paint_plan_matches_d9a_characterization_snapshot() {
        let strokes = [
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
        ];
        // 与 draw_spec 特征化测试同一视图:zoom 0.5 × scale 2 → factor 1、pan (10,20)。
        let view = ViewTransform::new(Vec2::new(10.0, 20.0), 0.5, 2.0);
        let viewport = Rect::new(0.0, 0.0, 200.0, 100.0);
        let plan = macos_paint_plan(&strokes, view, viewport, true, true).expect("plan");
        let mut got = Vec::new();
        for (li, layer) in plan.layers.iter().enumerate() {
            for c in layer {
                let dash = match &c.dash {
                    None => "-".to_string(),
                    Some(d) => format!("{:.2}/{:.2}", d.len, d.phase),
                };
                got.push(format!(
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
        // 与 draw_spec::tests::characterization_macos_committed_command_sequence
        // 逐字相同(D9a 快照;D9c 不允许改变命令序列)。
        assert_eq!(
            got,
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
        // 相邻页/页偏移:y_shift 折进视图 pan,映射结果等于旧公式。
        let shifted = macos_paint_plan(
            &[strokes[0].clone()],
            ViewTransform::new(Vec2::new(5.0, 7.0 - 300.0), 0.5, 2.0),
            viewport,
            false,
            false,
        )
        .expect("plan");
        // 旧公式:(y + 300 − 7) × 0.5 × 2 = y + 293
        assert!((shifted.points[0].pos.y - 313.0).abs() < 1e-9);
        assert!((shifted.points[1].pos.y - 323.0).abs() < 1e-9);
        assert!((shifted.points[0].pos.x - 5.0).abs() < 1e-9);
    }

    /// 整页重建计划跳过不足 2 点的笔迹(旧 paint_strokes_into 的 len()<2 早退)。
    #[test]
    fn macos_paint_plan_skips_single_point_strokes() {
        let strokes = [Stroke {
            id: 1,
            r: 1.0,
            g: 0.0,
            b: 0.0,
            points: vec![(5.0, 5.0, 2.0, 0.0)],
        }];
        let plan = macos_paint_plan(
            &strokes,
            ViewTransform::IDENTITY,
            Rect::new(0.0, 0.0, 100.0, 100.0),
            true,
            true,
        )
        .unwrap();
        assert!(plan.layers.is_empty());
        assert!(plan.points.is_empty());
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
        tracing::info!(
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
            tracing::info!(
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
                tracing::info!(
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
        let _g = crate::tests::TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
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
            STROKES.lock().unwrap_or_else(|e| e.into_inner()).clear();
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
            STROKES
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(Stroke {
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

            tracing::info!(
                "iter {}: begin={:?}  {} 个 move = {:?} ({:.1} µs/事件)  pen-up 收敛+提交={:?}",
                iter,
                begin_cost,
                n_events,
                move_cost,
                move_cost.as_micros() as f64 / n_events as f64,
                up_cost,
            );
        }

        STROKES.lock().unwrap_or_else(|e| e.into_inner()).clear();
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

    /// D1 effect 桥 FFI: macOS ObjC 依赖的 JSON 形状(成功 = kind+类型字段,
    /// 失败 = error)。这里直接调 FFI, 不依赖 ObjC 宿主(测试环境 DB 未初始化,
    /// 落库是 no-op)。
    #[test]
    fn settings_apply_ffi_roundtrip() {
        let call = |key: &str, value: &str| {
            let key = CString::new(key).unwrap();
            let value = CString::new(value).unwrap();
            let ptr = glaspen2_settings_apply(key.as_ptr(), value.as_ptr());
            assert!(!ptr.is_null(), "FFI 应返回 JSON 字符串");
            let text = unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned();
            glaspen2_free_c_string(ptr);
            serde_json::from_str::<serde_json::Value>(&text)
                .unwrap_or_else(|e| panic!("effect JSON 不合法: {e}: {text}"))
        };

        let effect = call("gifFps", "15");
        assert_eq!(effect["kind"], "GifFps");
        assert_eq!(effect["int"], 15);
        let effect = call("gridSize", "40.0");
        assert_eq!(effect["kind"], "GridSize");
        assert_eq!(effect["float"], 40.0);
        let effect = call("frostedGlass", "true");
        assert_eq!(effect["kind"], "FrostedGlass");
        assert_eq!(effect["bool"], true);
        let effect = call("chatUser", "\"alice\"");
        assert_eq!(effect["kind"], "ChatUser");
        assert_eq!(effect["str"], "alice");
        // shortcuts 载荷是 JSON 对象(不是字符串)
        let effect = call("shortcuts", "{\"prevPage\":\"cmd+ctrl+`\"}");
        assert_eq!(effect["kind"], "Shortcuts");
        assert_eq!(effect["json"]["prevPage"], "cmd+ctrl+`");

        // 失败:未知键 / 非法值 → {"error": "..."}
        assert!(call("noSuchSetting", "1")["error"].is_string());
        assert!(call("grid", "\"yes\"")["error"].is_string());
        assert!(call("invertFps", "25")["error"].is_string());
        // null 指针: 安全返回 error JSON, 不崩
        let ptr = glaspen2_settings_apply(std::ptr::null(), std::ptr::null());
        assert!(!ptr.is_null());
        glaspen2_free_c_string(ptr);
    }

    /// D5b 镜头数学 FFI: macOS ObjC 依赖的 JSON 形状 + 数值语义。
    ///
    /// - pan/zoom/centered 与 core [Camera] 同一条公式(已知值 + 与 core 对比);
    /// - zoom 结果带 hit_max/hit_min/changed(两端提示/早退规则不同, 见 camera.rs);
    /// - 输出必须单行合法 JSON(NSJSONSerialization);永不 NULL / panic;
    /// - 非有限数输入回 {"error":...}(防止 ObjC 侧写出 NaN).
    #[test]
    fn camera_ffi_roundtrip() {
        fn take(ptr: *mut c_char) -> serde_json::Value {
            assert!(!ptr.is_null(), "镜头 FFI 必须返回 JSON 字符串, 不能 NULL");
            let text = unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned();
            glaspen2_free_c_string(ptr);
            assert!(!text.contains('\n'), "镜头 JSON 必须单行: {text}");
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("镜头 JSON 不合法: {e}: {text}"))
        }

        // 平移: 与 Camera::pan_by 同款(pan -= d)。
        let v = take(glaspen2_camera_pan(120.0, -45.0, 0.5, 30.0, -10.0));
        assert_eq!(
            v.as_object().map(|obj| obj.len()),
            Some(3),
            "pan 只回三值: {v}"
        );
        assert_eq!(v["pan_x"], 90.0);
        assert_eq!(v["pan_y"], -35.0);
        assert_eq!(v["zoom"], 0.5);

        // 缩放锚点不变: 与 camera.rs 的已知向量逐值一致。
        let mut expected = Camera::new(120.0, -45.0, 0.5);
        let expected_out = expected.zoom_at(2.0, 300.0, 200.0);
        let v = take(glaspen2_camera_zoom_at(
            120.0, -45.0, 0.5, 2.0, 300.0, 200.0,
        ));
        assert_eq!(
            v.as_object().map(|obj| obj.len()),
            Some(6),
            "zoom 结果带三个标记: {v}"
        );
        assert_eq!(v["pan_x"], expected.pan_x);
        assert_eq!(v["pan_y"], expected.pan_y);
        assert_eq!(v["zoom"], expected_out.zoom);
        assert_eq!(v["hit_max"], false);
        assert_eq!(v["hit_min"], false);
        assert_eq!(v["changed"], true);

        // 100% 上限: 镜头不变但 hit_max(两端都据此提示)。
        let v = take(glaspen2_camera_zoom_at(0.0, 0.0, 1.0, 1.15, 300.0, 200.0));
        assert_eq!(v["zoom"], 1.0);
        assert_eq!(v["changed"], false);
        assert_eq!(v["hit_max"], true);
        assert_eq!(v["hit_min"], false);
        // 5% 下限: changed + hit_min(只有 Windows 弹提示)。
        let v = take(glaspen2_camera_zoom_at(0.0, 0.0, 0.06, 0.5, 0.0, 0.0));
        assert_eq!(v["zoom"], 0.05);
        assert_eq!(v["changed"], true);
        assert_eq!(v["hit_min"], true);
        assert_eq!(v["hit_max"], false);

        // 键盘缩放以视口中心为锚: 与 Camera::zoom_at_center 一致
        // (JSON 数字按最短往返字符串输出, 浮点比较用 1e-12 容差)。
        let close = |actual: &serde_json::Value, expected: f64| {
            let actual = actual
                .as_f64()
                .unwrap_or_else(|| panic!("不是数字: {actual}"));
            assert!(
                (actual - expected).abs() < 1e-12,
                "actual={actual} expected={expected}"
            );
        };
        let mut c = Camera::new(-40.0, 23.0, 0.8);
        let out = c.zoom_at_center(1.15, 1024.0, 768.0);
        let v = take(glaspen2_camera_zoom_at_center(
            -40.0, 23.0, 0.8, 1.15, 1024.0, 768.0,
        ));
        close(&v["pan_x"], c.pan_x);
        close(&v["pan_y"], c.pan_y);
        close(&v["zoom"], out.zoom);
        assert_eq!(v["changed"], true);

        // 居中: zoom 字段 = 公式实际用的 z(越界 zoom 兜底成 1.0, Windows 会写回)。
        let v = take(glaspen2_camera_centered(
            100.0, 50.0, 300.0, 150.0, 800.0, 600.0, 0.5,
        ));
        assert_eq!(v["pan_x"], -600.0);
        assert_eq!(v["pan_y"], -500.0);
        assert_eq!(v["zoom"], 0.5);
        let v = take(glaspen2_camera_centered(
            0.0, 0.0, 200.0, 100.0, 800.0, 600.0, 0.05,
        ));
        assert_eq!(v["zoom"], 1.0, "居中公式的 zoom 兜底是 step_zoom(strict >)");
        assert_eq!(v["pan_x"], -300.0);
        assert_eq!(v["pan_y"], -250.0);

        // 标量: 交互公式的 zoom 兜底 / 键盘步长(80 / step_zoom)。
        assert_eq!(glaspen2_camera_step_zoom(1.0), 1.0);
        assert_eq!(glaspen2_camera_step_zoom(0.05), 1.0);
        assert_eq!(glaspen2_camera_step_zoom(0.4), 0.4);
        assert_eq!(glaspen2_camera_keyboard_pan_step(1.0), 80.0);
        assert_eq!(glaspen2_camera_keyboard_pan_step(0.4), 200.0);

        // 非有限数输入: 回 error JSON, 不 panic(否则 ObjC 侧会写出 NaN/崩溃)。
        for v in [
            take(glaspen2_camera_pan(0.0, 0.0, 1.0, f64::NAN, 0.0)),
            take(glaspen2_camera_zoom_at(0.0, 0.0, 0.5, f64::NAN, 0.0, 0.0)),
            take(glaspen2_camera_zoom_at_center(
                0.0,
                0.0,
                f64::NAN,
                1.0,
                100.0,
                100.0,
            )),
            take(glaspen2_camera_centered(
                f64::NAN,
                0.0,
                1.0,
                1.0,
                100.0,
                100.0,
                1.0,
            )),
        ] {
            assert!(v["error"].is_string(), "非有限输入应回 error: {v}");
        }

        // home: out 参数写回 HOME;空指针安全(不 crash)。
        let (mut hx, mut hy, mut hz) = (9.0f64, 9.0f64, 9.0f64);
        glaspen2_camera_home(&mut hx, &mut hy, &mut hz);
        assert_eq!((hx, hy, hz), (0.0, 0.0, 1.0));
        glaspen2_camera_home(
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );

        // 载入 FFI: zoom 已在边界兜底(> 0, 不钳上限), 输出必须有限。
        let (mut px, mut py, mut pz) = (f64::NAN, f64::NAN, f64::NAN);
        glaspen2_get_infinite_transform(&mut px, &mut py, &mut pz);
        assert!(
            px.is_finite() && py.is_finite() && pz.is_finite(),
            "载入输出必须是有限数"
        );
        assert!(pz > 0.0, "载入 zoom 必须 > 0(FFI 已做 from_db_values 兜底)");
    }

    /// D10a 文案表 FFI: 全表 key×lang 往返、未知 key 回显、null/坏 UTF-8 兜底、
    /// Windows 同进程路径(crate::i18n::text)与 FFI 一致。
    #[test]
    fn i18n_ffi_roundtrip() {
        use crate::i18n::{Lang, Msg};

        fn take(ptr: *mut c_char) -> String {
            assert!(!ptr.is_null(), "i18n FFI 必须返回字符串, 不能 NULL");
            let text = unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned();
            glaspen2_free_c_string(ptr);
            assert!(!text.contains('\n'), "i18n 文案必须单行: {text}");
            text
        }

        // 全表: 每条 key × 两种语言都和 core 表逐字一致。
        for msg in Msg::ALL {
            let key = CString::new(msg.key()).expect("key 不含 NUL");
            for (code, lang) in [("zh", Lang::Zh), ("en", Lang::En)] {
                let code_c = CString::new(code).expect("lang 不含 NUL");
                let text = take(glaspen2_i18n(key.as_ptr(), code_c.as_ptr()));
                assert_eq!(text, msg.text(lang), "key={} lang={code}", msg.key());
                // Windows 同进程路径: 与 FFI 走同一张表。
                assert_eq!(crate::i18n::text(lang, msg), msg.text(lang));
            }
            // lang 缺省 → 中文(与两端默认一致)。
            assert_eq!(
                take(glaspen2_i18n(key.as_ptr(), std::ptr::null())),
                msg.zh()
            );
        }

        // 未知 key → 回显 key 本身(便于定位 adapter 拼错的 key), 不是 NULL/"?"。
        let unknown = CString::new("no.such.key").unwrap();
        let lang_en = CString::new("en").unwrap();
        assert_eq!(
            take(glaspen2_i18n(unknown.as_ptr(), lang_en.as_ptr())),
            "no.such.key"
        );
        // 中文 UTF-8 往返(macOS 侧 NSString stringWithUTF8String 依赖合法 UTF-8)。
        let gif = CString::new("recording.gif_copied").unwrap();
        let text = take(glaspen2_i18n(gif.as_ptr(), lang_en.as_ptr()));
        assert_eq!(text, "GIF copied to clipboard");
        // null key / 非法 UTF-8 key → "?", 永不 NULL。
        assert_eq!(take(glaspen2_i18n(std::ptr::null(), std::ptr::null())), "?");
        let bad = CString::new(vec![0xFF_u8]).expect("非 UTF-8 但无 NUL, 仍是合法 CString");
        assert_eq!(take(glaspen2_i18n(bad.as_ptr(), std::ptr::null())), "?");
    }
}
