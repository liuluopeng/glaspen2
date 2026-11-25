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

/// 实时计划输入解析结果(JSON → core 类型;不把 serde 渗进 draw_spec)。
struct MacosLiveInput {
    points: Vec<(f64, f64, f64, f64)>,
    seam: Option<(f64, f64)>,
    view: ViewTransform,
    viewport: Rect,
    shadow: bool,
    outline: bool,
    eraser: bool,
    alpha: f64,
    arc_len_start: f64,
    color: Color,
}

fn live_plan_number(value: &serde_json::Value, what: &str) -> Result<f64, String> {
    value
        .as_f64()
        .filter(|number| number.is_finite())
        .ok_or_else(|| format!("{what} 必须是有限数字"))
}

fn live_plan_pair(value: &serde_json::Value, what: &str) -> Result<(f64, f64), String> {
    let array = value
        .as_array()
        .filter(|array| array.len() == 2)
        .ok_or_else(|| format!("{what} 必须是 [x, y] 二元数组"))?;
    Ok((
        live_plan_number(&array[0], &format!("{what}[0]"))?,
        live_plan_number(&array[1], &format!("{what}[1]"))?,
    ))
}

fn live_plan_bool(
    params: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<bool, String> {
    match params.get(key) {
        None => Ok(false),
        Some(value) => value
            .as_bool()
            .ok_or_else(|| format!("params.{key} 必须是布尔值")),
    }
}

/// 把 FFI 输入 JSON 解析为 [MacosLiveInput];schema 见 [glaspen2_live_plan_json] 注释。
fn parse_macos_live_input(input: &str) -> Result<MacosLiveInput, String> {
    let root: serde_json::Value =
        serde_json::from_str(input).map_err(|err| format!("输入不是合法 JSON: {err}"))?;
    let object = root
        .as_object()
        .ok_or_else(|| "输入必须是 JSON 对象".to_string())?;

    let points = object
        .get("points")
        .ok_or_else(|| "缺少 points 点列".to_string())?
        .as_array()
        .ok_or_else(|| "points 必须是数组".to_string())?
        .iter()
        .enumerate()
        .map(|(index, point)| {
            let array = point
                .as_array()
                .filter(|array| array.len() == 3 || array.len() == 4)
                .ok_or_else(|| format!("points[{index}] 必须是 [x,y,width] 或 [x,y,width,t]"))?;
            let width = live_plan_number(&array[2], &format!("points[{index}].width"))?;
            if width < 0.0 {
                return Err(format!("points[{index}].width 不能为负"));
            }
            let time = match array.get(3) {
                Some(value) => live_plan_number(value, &format!("points[{index}].t"))?,
                None => 0.0,
            };
            Ok((
                live_plan_number(&array[0], &format!("points[{index}].x"))?,
                live_plan_number(&array[1], &format!("points[{index}].y"))?,
                width,
                time,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;

    let seam = match object.get("seam") {
        None | Some(serde_json::Value::Null) => None,
        Some(value) => Some(live_plan_pair(value, "seam")?),
    };

    let params = object
        .get("params")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "缺少 params 对象(至少要有 viewport)".to_string())?;
    let viewport = params
        .get("viewport")
        .ok_or_else(|| "缺少 params.viewport 表面尺寸".to_string())?;
    let (viewport_w, viewport_h) = live_plan_pair(viewport, "params.viewport")?;
    if viewport_w < 0.0 || viewport_h < 0.0 {
        return Err("params.viewport 尺寸不能为负".to_string());
    }

    let number = |key: &str, default: f64| -> Result<f64, String> {
        match params.get(key) {
            None => Ok(default),
            Some(value) => live_plan_number(value, &format!("params.{key}")),
        }
    };
    let scale = number("scale", 1.0)?;
    let zoom = number("zoom", 1.0)?;
    if scale <= 0.0 {
        return Err("params.scale 必须 > 0".to_string());
    }
    if zoom <= 0.0 {
        return Err("params.zoom 必须 > 0".to_string());
    }
    let pan = match params.get("pan") {
        None => (0.0, 0.0),
        Some(value) => live_plan_pair(value, "params.pan")?,
    };
    let alpha = number("alpha", 1.0)?;
    let arc_len_start = number("arc_len_start", 0.0)?;
    let color = match params.get("color") {
        None => Color::BLACK,
        Some(value) => {
            let array = value
                .as_array()
                .filter(|array| array.len() == 3)
                .ok_or_else(|| "params.color 必须是 [r, g, b] 三元数组".to_string())?;
            let channel = |index: usize, name: &str| -> Result<f64, String> {
                Ok(
                    live_plan_number(&array[index], &format!("params.color.{name}"))?
                        .clamp(0.0, 1.0),
                )
            };
            Color::new(channel(0, "r")?, channel(1, "g")?, channel(2, "b")?)
        }
    };

    Ok(MacosLiveInput {
        points,
        seam,
        view: ViewTransform::new(Vec2::new(pan.0, pan.1), zoom, scale),
        viewport: Rect::new(0.0, 0.0, viewport_w, viewport_h),
        shadow: live_plan_bool(params, "shadow")?,
        outline: live_plan_bool(params, "outline")?,
        eraser: live_plan_bool(params, "eraser")?,
        alpha,
        arc_len_start,
        color,
    })
}

fn macos_live_pass_name(pass: PaintPass) -> &'static str {
    match pass {
        PaintPass::Shadow => "shadow",
        PaintPass::Outline => "outline",
        PaintPass::Fill => "fill",
        PaintPass::Erase => "erase",
    }
}

/// 接缝重绘几何(plan 不建模,见 draw_spec 模块文档「plan 不建模的」第 1 条)。
///
/// 语义 = panel.m:797-806:每画完一段,把**上一段**墨迹按该段终点宽度重画一遍,
/// 恢复「描边垫底、墨迹在上」的层级。显式 seam(P_{i-2})时上一段 = seam→points[0]
/// (本次点列至少 2 点,即画的是线段而不是落笔圆点);否则点列自身为完整笔迹时
/// 取 points[n-3]→points[n-2]。输出表面像素。
fn macos_live_seam_json(plan: &DrawPlan, parsed: &MacosLiveInput) -> serde_json::Value {
    if parsed.eraser || !parsed.outline {
        return serde_json::Value::Null;
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
    let Some((from, to)) = pair else {
        return serde_json::Value::Null;
    };
    serde_json::json!({
        "from": [from.x, from.y],
        "to": [to.pos.x, to.pos.y],
        "width": to.width,
    })
}

/// DrawPlan → 单行 JSON(命令流展开 / 脏区 / 接缝几何;schema 见 FFI 注释)。
fn macos_live_plan_json(plan: &DrawPlan, parsed: &MacosLiveInput) -> String {
    let mut commands = Vec::with_capacity(plan.command_count());
    for (layer, layer_commands) in plan.layers.iter().enumerate() {
        for command in layer_commands {
            let points: Vec<serde_json::Value> = plan.points[command.points.clone()]
                .iter()
                .map(|point| {
                    serde_json::json!([point.pos.x, point.pos.y, point.width + command.width_delta])
                })
                .collect();
            commands.push(serde_json::json!({
                "layer": layer,
                "pass": macos_live_pass_name(command.pass),
                "stroke_index": command.stroke_index,
                "stroke_id": command.stroke_id,
                "live": command.live,
                "color": [command.color.r, command.color.g, command.color.b],
                "alpha": command.alpha,
                "width_delta": command.width_delta,
                "dash": match command.dash {
                    Some(dash) => serde_json::json!({ "len": dash.len, "phase": dash.phase }),
                    None => serde_json::Value::Null,
                },
                "points": points,
            }));
        }
    }
    serde_json::json!({
        "commands": commands,
        "dirty": {
            "x": plan.dirty.x,
            "y": plan.dirty.y,
            "w": plan.dirty.w,
            "h": plan.dirty.h,
        },
        "seam": macos_live_seam_json(plan, parsed),
    })
    .to_string()
}

/// 输入 JSON → MacosLive 计划单行 JSON;失败返回 error 文本描述。
fn macos_live_plan_text(input: &str) -> Result<String, String> {
    let parsed = parse_macos_live_input(input)?;
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
    Ok(macos_live_plan_json(&plan, &parsed))
}

/// 返回 JSON 字符串的 C 指针;serde_json 输出不含 NUL,真正失败时回静态 error。
fn return_json_c_string(text: String) -> *mut c_char {
    match CString::new(text) {
        Ok(string) => string.into_raw(),
        Err(_) => CString::new(r#"{"error":"实时绘制计划响应编码失败"}"#)
            .expect("静态 error JSON 不含 NUL")
            .into_raw(),
    }
}

/// macOS 实时绘制计划:输入 JSON → 单行计划 JSON(schema 见上方注释)。
///
/// 单 C 参数,永不返回 NULL:NULL / 非法 UTF-8 / 非法 JSON / 非法字段 / plan
/// 拒绝(如 scale 非正)一律回 `{"error":"..."}`。返回值用
/// [glaspen2_free_c_string] 释放;ObjC 侧接线: `raw_draw_segment`(d0924a5)与
/// `raw_draw_dot`(单点圆点)都已消费(命令流 + 接缝重画 + 脏区;失败回退旧自绘)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_live_plan_json(input: *const c_char) -> *mut c_char {
    let Some(input) = (unsafe { borrow_cstr(input) }) else {
        return return_json_c_string(
            serde_json::json!({ "error": "实时绘制计划缺少 JSON 输入(或不是合法 UTF-8)" })
                .to_string(),
        );
    };
    match macos_live_plan_text(input) {
        Ok(text) => return_json_c_string(text),
        Err(message) => return_json_c_string(serde_json::json!({ "error": message }).to_string()),
    }
}

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
        state::buffer_point(
            x,
            y,
            pressure_to_width(pressure, width_scale),
            timestamp - start,
        ); // sync
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

/// 构建面板设置快照(字段集合/顺序/类型/缺省值/DB 回退单源在
/// crate::settings_snapshot)。
///
/// 参数 = 运行时专属值 JSON 对象(macOS 必须含 launchAtLogin/shortcuts;
/// 可选 flipEffect/language;见 crate::settings_snapshot::RUNTIME_ONLY_KEYS)。
/// 返回完整快照单行 JSON;失败返回 {"error":"..."};永不 NULL。
/// 返回值用 glaspen2_free_c_string 释放。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_settings_snapshot_json(overrides_json: *const c_char) -> *mut c_char {
    let error = |msg: String| return_c_string(serde_json::json!({ "error": msg }).to_string());
    let Some(raw) = (unsafe { borrow_cstr(overrides_json) }) else {
        return error("settings_snapshot 缺少 overrides_json".to_string());
    };
    let overrides = match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(serde_json::Value::Object(map)) => map,
        Ok(other) => {
            return error(format!(
                "overrides_json 必须是 JSON 对象, 收到 {}",
                if other.is_null() { "null" } else { "非对象" }
            ));
        }
        Err(e) => return error(format!("overrides_json 不是合法 JSON: {e}")),
    };
    match crate::settings_snapshot::snapshot_json(crate::ui::Platform::Macos, None, &overrides) {
        Ok(json) => return_c_string(json),
        Err(e) => error(e.to_string()),
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

/// 默认 chord 表(JSON 对象, action -> chord)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_chord_defaults_json() -> *mut c_char {
    return_c_string(crate::chord::defaults_json(crate::ui::Platform::Macos))
}

/// 动作 id 列表(JSON 数组, 顺序 = 面板显示顺序)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_chord_actions_json() -> *mut c_char {
    return_c_string(crate::ui::action_ids_json())
}

// ── 动作 effect 单源 FFI(D3b): macOS 壳经此使用 core::action_effect ──

/// 动作 id → 平台无关 effect JSON(只服务 macOS ObjC 适配器)。
///
/// 形状: `{"kind":"<variant_name>"}`。镜头平移/缩放的变体名已带方向
/// (`CameraPanLeft` / `CameraZoomIn` 等), 因此没有额外载荷字段; 其余
/// 无载荷变体直接是变体名(`Undo`、`OpenSettings`、`Quit`……)。
/// 失败: `{"error":"未知动作 id: ..."}`(空/未登记 id), 不返回 NULL。
/// Windows 同进程直接 `match ActionEffect`, 不经此 JSON。
/// 返回值用 `glaspen2_free_c_string` 释放。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_action_effect_json(action: *const c_char) -> *mut c_char {
    let error = |msg: String| return_c_string(serde_json::json!({ "error": msg }).to_string());
    let Some(action) = (unsafe { borrow_cstr(action) }) else {
        return error("动作 effect 缺少 action id".to_string());
    };
    match crate::action_effect::effect_for_action(action) {
        Ok(effect) => {
            return_c_string(serde_json::json!({ "kind": effect.variant_name() }).to_string())
        }
        Err(e) => error(e),
    }
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

// ── 网格/分栏几何单源 FFI(D6): macOS 壳经此使用 core::grid ──
//
// 与 D1b/D3b/D5b 同形: ObjC 侧持有视图尺寸/镜头/网格设置, 输出是 core 算好的
// 线段表 JSON(单行, NSJSONSerialization 解析); macOS 只按本平台既有样式描边
// (CGContext 线宽/颜色/alpha/CG 抗锯齿不变), 不再算任何几何。
// 永不返回 NULL: 非法/非有限输入回 {"error": ...}(ObjC 记日志并跳过本帧)。
// Windows 同进程直接调 grid::compute, 不经这些 JSON 入口。
//
// 两个入口:
// - glaspen2_grid_json: 规范入口(D6 约定签名), 全视口重绘用。屏幕单位 = 视口
//   尺寸(macOS 视图 = 屏幕 frame, 与旧 g_screen_w/h 一致), 屏幕分界线两种画布
//   模式都生成(macOS 现状), 细网格恒生成(调用方只在"显示网格"时走此入口)。
//   follow 只回显: 网格跟随涂鸦是可见性门控, 不参与几何(见 grid.rs 模块文档)。
// - glaspen2_grid_json_clipped: macOS 笔迹脏矩形热路径(旧实现只算与 clipRect
//   相交的线, 这里保留同一优化); 额外带 show_grid 开关, 只开分栏时不算细网格。

fn grid_segment_json(seg: &Segment) -> serde_json::Value {
    serde_json::json!({
        "x0": seg.x0,
        "y0": seg.y0,
        "x1": seg.x1,
        "y1": seg.y1,
    })
}

fn grid_lines_json(lines: &GridLines) -> serde_json::Value {
    let mut value = serde_json::json!({
        "minor": lines.minor.iter().map(grid_segment_json).collect::<Vec<_>>(),
        "screen": lines.screen.iter().map(grid_segment_json).collect::<Vec<_>>(),
        "divider": lines.divider.iter().map(grid_segment_json).collect::<Vec<_>>(),
    });
    value["follow"] = serde_json::Value::Bool(false);
    value
}

fn grid_error_json(err: &crate::grid::GridError) -> *mut c_char {
    return_c_string(serde_json::json!({ "error": err.to_string() }).to_string())
}

/// 网格线段表 JSON(规范入口): 形状
/// `{"minor":[{"x0","y0","x1","y1"},...],"screen":[...],"divider":[...],"follow":bool}»。
///
/// - 参数顺序 = D6 约定: 视图宽高、镜头三值、网格尺寸、分栏模式、网格跟随涂鸦;
/// - 线段是视图坐标(原点左上, y 向下; macOS adapter 描边时翻转 y),
///   竖线 x0==x1、y0=0/y1=view_h, 横线 y0==y1、x0=0/x1=view_w;
/// - 绘制顺序 = minor → screen → divider(两端旧实现同序);
/// - follow 仅回显(几何与它无关, 两端现状都只是可见性门控);
/// - 非法/非有限输入 → {"error": "..."};永不 NULL。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_grid_json(
    view_w: c_double,
    view_h: c_double,
    pan_x: c_double,
    pan_y: c_double,
    zoom: c_double,
    grid_size: c_double,
    divider: c_int,
    follow: c_int,
) -> *mut c_char {
    let params = GridParams {
        view_w,
        view_h,
        camera: Camera::new(pan_x, pan_y, zoom),
        grid_size,
        divider,
        show_grid: true,
        screen_w: view_w,
        screen_h: view_h,
        screen_lines: true,
        clip: None,
    };
    match crate::grid::compute_checked(&params) {
        Ok(lines) => {
            let mut value = grid_lines_json(&lines);
            value["follow"] = serde_json::Value::Bool(follow != 0);
            return_c_string(value.to_string())
        }
        Err(err) => grid_error_json(&err),
    }
}

/// 网格线段表 JSON(脏矩形热路径): 同 [glaspen2_grid_json] 形状, 但只生成与
/// `(clip_x0, clip_y0, clip_x1, clip_y1)»(视图坐标, y 向下)相交的线。
///
/// `show_grid != 0» 才生成细网格 + 屏幕分界线; 分栏线独立生成(与"显示网格"
/// 开关无关, 两端同款)。屏幕单位仍 = 视口尺寸(不是脏区尺寸)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_grid_json_clipped(
    view_w: c_double,
    view_h: c_double,
    pan_x: c_double,
    pan_y: c_double,
    zoom: c_double,
    grid_size: c_double,
    divider: c_int,
    show_grid: c_int,
    clip_x0: c_double,
    clip_y0: c_double,
    clip_x1: c_double,
    clip_y1: c_double,
) -> *mut c_char {
    let params = GridParams {
        view_w,
        view_h,
        camera: Camera::new(pan_x, pan_y, zoom),
        grid_size,
        divider,
        show_grid: show_grid != 0,
        screen_w: view_w,
        screen_h: view_h,
        screen_lines: true,
        clip: Some(crate::grid::ViewRect::new(
            clip_x0, clip_y0, clip_x1, clip_y1,
        )),
    };
    match crate::grid::compute_checked(&params) {
        Ok(lines) => return_c_string(grid_lines_json(&lines).to_string()),
        Err(err) => grid_error_json(&err),
    }
}

// ── 缩略图条(minimap)页窗/布局单源 FFI(D7): macOS 壳经此使用 core::minimap ──
//
// 与 D1b/D3b/D5b/D6 同形: ObjC 只持有当前页/视图尺寸/几何组过滤宽度, 输出是
// core 算好的条带布局 JSON(单行, NSJSONSerialization 解析);缩略图渲染/缓存/
// 后台队列/条带合成/描边/页号字体仍归 adapter。
// 永不返回 NULL: 视图尺寸非法回 {"error": ...};当前页不在活页列表(新建的空白页
// /空库)是合法状态 → items 为空(adapter 直接跳过绘制)。
// Windows 同进程直接调 minimap::page_window/layout, 不经此 JSON 入口。

/// 页列表行 + core 页窗/布局 → FFI JSON 值(单行形状)。
///
/// 抽成独立函数便于测试: 不碰全局 DB、不分配 C 字符串。
fn minimap_layout_json_value(
    rows: &[(i64, i32, i32)],
    current: i64,
    view_w: f64,
    view_h: f64,
    filter: crate::minimap::PageFilter,
) -> Result<serde_json::Value, crate::minimap::MinimapError> {
    use crate::minimap::{MACOS_METRICS, WINDOW_COUNT, WINDOW_HALF, layout, page_window_from_rows};
    let window = page_window_from_rows(rows, current, filter, WINDOW_HALF, WINDOW_COUNT);
    let (ids, start): (&[i64], usize) = match window.as_ref() {
        Some(window) => (window.ids.as_slice(), window.start),
        None => (&[], 0),
    };
    let layout = layout(view_w, view_h, ids, start, current, &MACOS_METRICS)?;
    Ok(serde_json::json!({
        "strip_x": layout.strip_x,
        "strip_y": layout.strip_y,
        "strip_w": layout.strip_w,
        "strip_h": layout.strip_h,
        "items": layout
            .items
            .iter()
            .map(|item| {
                serde_json::json!({
                    "screen_id": item.screen_id,
                    "index": item.index,
                    "y": item.y,
                    "h": item.h,
                    "label": item.label,
                    "current": item.current,
                })
            })
            .collect::<Vec<_>>(),
    }))
}

/// 缩略图条布局 JSON: 页窗(当前页前后各 [crate::minimap::WINDOW_HALF] 页)+
/// 条带几何(右缘竖条, 卡片 64×40 / 间距 4 / 右缘 8 / 顶端 8;公式与两端差异
/// 见 crate::minimap 模块文档)。
///
/// 形状: {"strip_x","strip_y","strip_w","strip_h",
///        "items":[{"screen_id","index","y","h","label","current"},...]}
/// 坐标 = 视图坐标(原点左上, y 向下);macOS adapter 合成条带时翻转 y。
/// geometry_width > 0 时只保留 screen_w == geometry_width 的页(macOS
/// page-view-fit 模式, 只显示当前几何组);<= 0 = 不过滤(Windows 语义)。
/// 返回值用 glaspen2_free_c_string 释放。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_minimap_layout_json(
    current_screen_id: i64,
    view_w: c_double,
    view_h: c_double,
    geometry_width: c_int,
) -> *mut c_char {
    use crate::minimap::PageFilter;
    let filter = if geometry_width > 0 {
        PageFilter::Width(geometry_width)
    } else {
        PageFilter::All
    };
    let rows = runtime().block_on(db::list_screens());
    match minimap_layout_json_value(&rows, current_screen_id, view_w, view_h, filter) {
        Ok(value) => return_c_string(value.to_string()),
        Err(err) => return_c_string(serde_json::json!({ "error": err.to_string() }).to_string()),
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

/// 喂一个事件, 返回 must-execute-in-order 的 effects JSON 数组。
///
/// 事件 JSON 例: {"type":"StartPressed","mode":"Gif","stroke_count":3}、
/// {"type":"ReleasePressed","mode":"Draft","stroke_count":7}、
/// {"type":"GifEncodeSucceeded"}、{"type":"ChatDraftFinished","code":2,"detail":""}。
/// 未初始化槽位按"默认关总开关的空闲机器"懒建(与两端启动默认一致)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_recording_event_json(
    slot: c_int,
    event_json: *const c_char,
) -> *mut c_char {
    let Some(idx) = recording_slot(slot) else {
        return recording_error_json(format!("非法录制槽位: {slot}"));
    };
    let Some(text) = (unsafe { borrow_cstr(event_json) }) else {
        return recording_error_json("录制事件缺少 JSON".to_string());
    };
    let event = match crate::recording::event_from_json(text) {
        Ok(event) => event,
        Err(err) => return recording_error_json(err),
    };
    let mut machines = RECORDING_MACHINES.lock().unwrap_or_else(|e| e.into_inner());
    let machine = machines[idx].get_or_insert_with(Default::default);
    let effects = machine.handle(event);
    return_c_string(crate::recording::effects_to_json(&effects))
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

/// 全表审计 JSON(D10a): `{"<key>":{"zh":"...","en":"..."}}`, 单行, key 字典序。
/// 供审计/测试用; 永不 NULL。返回值用 glaspen2_free_c_string 释放。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_i18n_catalog_json() -> *mut c_char {
    return_c_string(crate::i18n::catalog_json())
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

    // ------------------------------------------------------------------
    // D9c 剩余:macOS 实时计划 FFI(core 出计划;ObjC 接线仍未做)
    // ------------------------------------------------------------------

    fn live_plan_text(input: &str) -> String {
        let input = CString::new(input).expect("测试 JSON 不含 NUL");
        let pointer = glaspen2_live_plan_json(input.as_ptr());
        assert!(!pointer.is_null(), "FFI 必须永不返回 NULL");
        let text = unsafe { CStr::from_ptr(pointer) }
            .to_string_lossy()
            .into_owned();
        glaspen2_free_c_string(pointer);
        text
    }

    fn live_plan_value(input: &str) -> serde_json::Value {
        let text = live_plan_text(input);
        serde_json::from_str(&text).unwrap_or_else(|err| panic!("计划 JSON 不合法: {err}: {text}"))
    }

    fn live_plan_approx(actual: &serde_json::Value, expected: f64) {
        let actual = actual
            .as_f64()
            .unwrap_or_else(|| panic!("不是数字: {actual}"));
        assert!(
            (actual - expected).abs() < 1e-9,
            "actual={actual} expected={expected}"
        );
    }

    /// 3 点实时序列:命令流形状(层序 / pass 序 / 点表 / 有效宽度)、脏区、接缝几何。
    #[test]
    fn live_plan_ffi_roundtrip_three_point_sequence() {
        let value = live_plan_value(
            r#"{"points":[[10,10,4],[20,10,4],[30,10,4]],"params":{"viewport":[200,100],"scale":2.0,"shadow":true,"outline":true,"arc_len_start":7.5,"color":[1,0,0]}}"#,
        );
        let commands = value["commands"].as_array().expect("commands 必须是数组");
        // MacosCommitted:层 0 = 三档阴影(实时只有一笔),层 1 = 描边 + 墨迹。
        assert_eq!(commands.len(), 5);
        let passes: Vec<&str> = commands
            .iter()
            .map(|command| command["pass"].as_str().expect("pass 必须是字符串"))
            .collect();
        assert_eq!(passes, ["shadow", "shadow", "shadow", "outline", "fill"]);
        let layers: Vec<u64> = commands
            .iter()
            .map(|command| command["layer"].as_u64().expect("layer 必须是整数"))
            .collect();
        assert_eq!(layers, [0, 0, 0, 1, 1]);
        // 每个命令都是实时命令,带同一组 3 个表面点(scale 2 → 20/40/60)。
        for command in commands {
            assert_eq!(command["live"], true);
            assert_eq!(command["stroke_index"], 0);
            assert_eq!(command["stroke_id"], 0);
            let points = command["points"].as_array().expect("points 必须是数组");
            assert_eq!(points.len(), 3);
            live_plan_approx(&points[0][0], 20.0);
            live_plan_approx(&points[0][1], 20.0);
            live_plan_approx(&points[2][0], 60.0);
        }
        // 阴影:width_delta = 2 × spread(spread = 档位 × factor 2);点宽 8。
        for (command, expected) in commands[..3].iter().zip([6.0, 12.0, 20.0]) {
            live_plan_approx(&command["width_delta"], expected);
            live_plan_approx(&command["points"][0][2], 8.0 + expected);
        }
        // 脏区 = 三点按「半宽 4 + (AA 1.5 + 阴影 5) × 2 = 17」外扩后的并集。
        let dirty = &value["dirty"];
        live_plan_approx(&dirty["x"], 3.0);
        live_plan_approx(&dirty["y"], 3.0);
        live_plan_approx(&dirty["w"], 74.0);
        live_plan_approx(&dirty["h"], 34.0);
        // 墨迹:颜色透传、无虚线;描边:相位接续 = arc_len_start(7.5)× scale 2。
        let outline = &commands[3];
        live_plan_approx(&outline["width_delta"], 4.0);
        live_plan_approx(&outline["dash"]["len"], 2.0);
        live_plan_approx(&outline["dash"]["phase"], 15.0);
        let fill = &commands[4];
        assert_eq!(fill["color"], serde_json::json!([1.0, 0.0, 0.0]));
        live_plan_approx(&fill["alpha"], 1.0);
        live_plan_approx(&fill["width_delta"], 0.0);
        assert_eq!(fill["dash"], serde_json::Value::Null);
        // 接缝重绘:3 点自身 → 上一段 = P0→P1,宽 = P1 墨迹宽(表面 8)。
        let seam = &value["seam"];
        assert_eq!(seam["from"], serde_json::json!([20.0, 20.0]));
        assert_eq!(seam["to"], serde_json::json!([40.0, 20.0]));
        live_plan_approx(&seam["width"], 8.0);
    }

    /// 相位接续参数透传 + 显式 seam(P_{i-2});无描边时 seam 恒 null。
    #[test]
    fn live_plan_ffi_phase_passthrough_and_explicit_seam() {
        let value = live_plan_value(
            r#"{"points":[[10,10,4],[20,10,4]],"seam":[4,5],"params":{"viewport":[200,100],"scale":2.0,"outline":true,"arc_len_start":7.5}}"#,
        );
        let commands = value["commands"].as_array().expect("commands 必须是数组");
        assert_eq!(commands.len(), 2, "只有描边 + 墨迹两层命令");
        let outline = &commands[0];
        assert_eq!(outline["pass"], "outline");
        // pad / dash_len 都 × scale;起始相位 = 已画弧长 7.5 × scale 2。
        live_plan_approx(&outline["width_delta"], 4.0);
        live_plan_approx(&outline["dash"]["len"], 2.0);
        live_plan_approx(&outline["dash"]["phase"], 15.0);
        // 显式 seam = P_{i-2}(4,5)→表面 (8,10);to = 首点 P_{i-1},宽 = P_{i-1} 墨迹宽。
        let seam = &value["seam"];
        assert_eq!(seam["from"], serde_json::json!([8.0, 10.0]));
        assert_eq!(seam["to"], serde_json::json!([20.0, 20.0]));
        live_plan_approx(&seam["width"], 8.0);

        // 描边关闭:没有接缝重绘,只剩墨迹命令。
        let value = live_plan_value(
            r#"{"points":[[10,10,4],[20,10,4]],"seam":[4,5],"params":{"viewport":[200,100],"scale":2.0}}"#,
        );
        assert_eq!(value["commands"].as_array().expect("数组").len(), 1);
        assert_eq!(value["commands"][0]["pass"], "fill");
        assert_eq!(value["seam"], serde_json::Value::Null);
    }

    /// 非法输入一律 error JSON、永不 NULL、不 panic、单行合法 JSON。
    #[test]
    fn live_plan_ffi_invalid_input_returns_error_json() {
        let invalid_inputs = [
            "not json",
            r#"{"points":[[NaN,10,4]],"params":{"viewport":[200,100]}}"#,
            r#"{"points":[[10,Infinity,4]],"params":{"viewport":[200,100]}}"#,
            r#"{"points":[[1e400,10,4]],"params":{"viewport":[200,100]}}"#,
            r#"{"points":[[10,10,-4]],"params":{"viewport":[200,100]}}"#,
            r#"{"points":{},"params":{"viewport":[200,100]}}"#,
            r#"{"points":[[10,10]],"params":{"viewport":[200,100]}}"#,
            r#"{"points":[]}"#,
            r#"{"points":[[10,10,4]],"params":{}}"#,
            r#"{"points":[[10,10,4]],"params":{"viewport":[200,100],"scale":0}}"#,
            r#"{"points":[[10,10,4]],"params":{"viewport":[200,100],"zoom":-1}}"#,
            r#"{"points":[[10,10,4]],"params":{"viewport":[200,100],"alpha":"1"}}"#,
            "[]",
        ];
        for input in invalid_inputs {
            let text = live_plan_text(input);
            assert!(!text.contains('\n'), "error JSON 也必须单行: {text}");
            let value: serde_json::Value = serde_json::from_str(&text)
                .unwrap_or_else(|err| panic!("error JSON 非法: {err}: {text}"));
            assert!(
                value["error"].is_string(),
                "非法输入必须回 error JSON: input={input} output={text}"
            );
        }

        // NULL 指针 / 非 UTF-8 输入:同样 error JSON,不 NULL、不崩溃。
        let pointer = glaspen2_live_plan_json(std::ptr::null());
        assert!(!pointer.is_null(), "NULL 输入也必须返回 error JSON");
        let text = unsafe { CStr::from_ptr(pointer) }
            .to_string_lossy()
            .into_owned();
        glaspen2_free_c_string(pointer);
        assert!(text.contains("error"), "NULL 输入应回 error: {text}");

        let invalid_utf8 = CString::new(vec![0xff, b'{', b'}']).expect("无 NUL");
        let pointer = glaspen2_live_plan_json(invalid_utf8.as_ptr());
        assert!(!pointer.is_null(), "非 UTF-8 输入也必须返回 error JSON");
        let text = unsafe { CStr::from_ptr(pointer) }
            .to_string_lossy()
            .into_owned();
        glaspen2_free_c_string(pointer);
        assert!(text.contains("error"), "非 UTF-8 输入应回 error: {text}");
    }

    /// 成功/失败输出都是单行 JSON;空点列是合法输入(空计划、空脏区、无接缝)。
    #[test]
    fn live_plan_ffi_output_is_single_line() {
        let inputs = [
            r#"{"points":[],"params":{"viewport":[200,100]}}"#,
            r#"{"points":[[10,10,4]],"params":{"viewport":[200,100],"scale":2.0,"shadow":true,"outline":true}}"#,
            r#"{"points":[[10,10,4]],"params":{"viewport":[200,100],"eraser":true}}"#,
            "not json",
        ];
        for input in inputs {
            let text = live_plan_text(input);
            assert!(!text.contains('\n'), "必须是单行 JSON: {text}");
            let value: serde_json::Value = serde_json::from_str(&text)
                .unwrap_or_else(|err| panic!("JSON 非法: {err}: {text}"));
            assert!(value.is_object(), "输出必须是 JSON 对象: {text}");
        }

        let value = live_plan_value(r#"{"points":[],"params":{"viewport":[200,100]}}"#);
        assert_eq!(value["commands"], serde_json::json!([]));
        assert_eq!(
            value["dirty"],
            serde_json::json!({"x": 0.0, "y": 0.0, "w": 0.0, "h": 0.0})
        );
        assert_eq!(value["seam"], serde_json::Value::Null);
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

    /// D12 设置快照桥 FFI: macOS ObjC 依赖的完整快照(字段集合/类型/单行)与
    /// 错误 JSON;adapter 只提供运行时专属值 + 覆盖即可, 不再拼设置字典。
    #[test]
    fn settings_snapshot_ffi_roundtrip() {
        let call = |overrides: &str| -> String {
            let overrides = CString::new(overrides).unwrap();
            let ptr = glaspen2_settings_snapshot_json(overrides.as_ptr());
            assert!(!ptr.is_null(), "FFI 应返回 JSON 字符串(永不 NULL)");
            let text = unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned();
            glaspen2_free_c_string(ptr);
            text
        };

        // 正常路径: 完整快照 = macOS 登记表全部字段(含 flipEffect, 无 ethereal)。
        let text = call(
            r#"{"launchAtLogin":true,"shortcuts":"{\"quit\":\"cmd+ctrl+Q\"}","flipEffect":1,"language":"en"}"#,
        );
        assert!(!text.contains('\n'), "快照必须是单行: {text}");
        let value: serde_json::Value = serde_json::from_str(&text).expect("快照必须是合法 JSON");
        let object = value.as_object().expect("快照必须是 JSON 对象");
        let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
        keys.sort_unstable();
        let mut expected = crate::settings::keys_for(crate::ui::Platform::Macos);
        expected.sort_unstable();
        assert_eq!(keys, expected, "FFI 快照字段集合与登记表不一致");
        assert!(
            !object.contains_key("ethereal"),
            "macOS 快照不含 Windows 专属字段"
        );
        // 运行时覆盖生效。
        assert_eq!(object["launchAtLogin"], serde_json::json!(true));
        assert_eq!(object["flipEffect"], serde_json::json!(1));
        assert_eq!(object["language"], serde_json::json!("en"));
        assert!(
            object["shortcuts"].is_string(),
            "shortcuts 必须是 JSON 字符串"
        );
        // 类型契约(值取决于全局库是否已被其它测试初始化, 这里只锁类型)。
        assert!(object["grid"].is_boolean());
        assert!(object["gridSize"].is_number());
        assert!(object["chatHasPassword"].is_boolean());
        assert!(!text.contains("ethereal"));

        // 错误路径: 非法参数一律 {"error": ...}, 单行, 永不 NULL。
        for bad in ["{}", "not json", "[]", "null"] {
            let text = call(bad);
            assert!(!text.contains('\n'), "error JSON 必须单行: {text}");
            let value: serde_json::Value =
                serde_json::from_str(&text).expect("error 也必须是合法 JSON");
            assert!(
                value["error"].is_string(),
                "{bad} 应返回 error JSON: {text}"
            );
        }
        let ptr = glaspen2_settings_snapshot_json(std::ptr::null());
        assert!(!ptr.is_null(), "null 入参也必须返回 error JSON");
        let text = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        glaspen2_free_c_string(ptr);
        assert!(text.contains("error"));
    }

    /// D3b 动作 effect 桥 FFI: macOS ObjC 依赖的 JSON 形状。
    ///
    /// - `ui::ACTIONS` 的每个 action 都必须得到 `{"kind":"<variant_name>"}`
    ///   (与 `effect_for_action` + `variant_name` 逐条一致);
    /// - 镜头变体的 kind 已带方向(`CameraPanLeft` / `CameraZoomOut`), 无载荷字段;
    /// - 未知/空 id → `{"error":"未知动作 id: ..."}`;
    /// - 输出必须是单行合法 JSON(ObjC 用 NSJSONSerialization 解析)。
    #[test]
    fn action_effect_ffi_roundtrip() {
        let call = |action: &str| {
            let action_c = CString::new(action).unwrap();
            let ptr = glaspen2_action_effect_json(action_c.as_ptr());
            assert!(!ptr.is_null(), "FFI 应返回 JSON 字符串");
            let text = unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned();
            glaspen2_free_c_string(ptr);
            text
        };
        let parse = |text: &str| -> serde_json::Value {
            assert!(!text.contains('\n'), "effect JSON 必须是单行: {text}");
            serde_json::from_str(text).unwrap_or_else(|e| panic!("effect JSON 不合法: {e}: {text}"))
        };

        for def in crate::ui::ACTIONS {
            let value = parse(&call(def.id));
            let expected = crate::action_effect::effect_for_action(def.id)
                .unwrap_or_else(|e| panic!("动作 {} 未映射: {e}", def.id))
                .variant_name();
            assert_eq!(value["kind"], expected, "动作 {} 的 kind 漂移", def.id);
            assert_eq!(
                value.as_object().map(|obj| obj.len()),
                Some(1),
                "动作 {} 的 effect 不应带载荷字段: {value}",
                def.id
            );
        }
        // 镜头载荷: kind 直接带方向(与 ActionEffect::variant_name 一致)。
        assert_eq!(parse(&call("canvas_pan_left"))["kind"], "CameraPanLeft");
        assert_eq!(parse(&call("canvas_pan_right"))["kind"], "CameraPanRight");
        assert_eq!(parse(&call("canvas_pan_up"))["kind"], "CameraPanUp");
        assert_eq!(parse(&call("canvas_pan_down"))["kind"], "CameraPanDown");
        assert_eq!(parse(&call("canvas_zoom_in"))["kind"], "CameraZoomIn");
        assert_eq!(parse(&call("canvas_zoom_out"))["kind"], "CameraZoomOut");

        // 未知/空 id → error JSON, 错误信息说明是未知动作。
        for bad in ["", "no_such_action", "Undo"] {
            let value = parse(&call(bad));
            assert!(
                value["error"]
                    .as_str()
                    .unwrap_or("")
                    .contains("未知动作 id"),
                "{bad:?} 应返回未知动作错误: {value}"
            );
        }
        // null 指针: 安全返回 error JSON, 不崩。
        let ptr = glaspen2_action_effect_json(std::ptr::null());
        assert!(!ptr.is_null());
        let text = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        glaspen2_free_c_string(ptr);
        assert!(parse(&text)["error"].is_string(), "null 指针应返回 error");
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

    /// D6 网格几何 FFI: macOS ObjC 依赖的 JSON 形状 + 数值语义。
    ///
    /// - 线段表与 core `grid::compute» 同一公式(已知向量 + 与 core 计数对照);
    /// - 三层绘制顺序 minor → screen → divider, 线段轴对齐且画满视口;
    /// - 分栏 0/1/2/3/4 位置与 core 快照一致;脏矩形入口只回相交线;
    /// - `follow» 仅回显(网格跟随涂鸦不参与几何);
    /// - 非法输入回 {"error":...}, 永不 NULL / NaN, 输出单行合法 JSON。
    #[test]
    fn grid_ffi_roundtrip() {
        fn take(ptr: *mut c_char) -> serde_json::Value {
            assert!(!ptr.is_null(), "网格 FFI 必须返回 JSON 字符串, 不能 NULL");
            let text = unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned();
            glaspen2_free_c_string(ptr);
            assert!(!text.contains('\n'), "网格 JSON 必须单行: {text}");
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("网格 JSON 不合法: {e}: {text}"))
        }
        fn seg(value: &serde_json::Value) -> [f64; 4] {
            [
                value["x0"].as_f64().expect("x0 数字"),
                value["y0"].as_f64().expect("y0 数字"),
                value["x1"].as_f64().expect("x1 数字"),
                value["y1"].as_f64().expect("y1 数字"),
            ]
        }
        fn array_len(value: &serde_json::Value) -> usize {
            value.as_array().expect("线段数组").len()
        }

        // 回原点 800x600 / 间距 40: 细网格 23 竖 + 18 横;屏幕分界线 4+4;无分栏。
        let v = take(glaspen2_grid_json(800.0, 600.0, 0.0, 0.0, 1.0, 40.0, 0, 0));
        assert_eq!(array_len(&v["minor"]), 23 + 18);
        assert_eq!(array_len(&v["screen"]), 4 + 4);
        assert_eq!(array_len(&v["divider"]), 0);
        assert_eq!(v["follow"], false);
        assert_eq!(seg(&v["minor"][0]), [-40.0, 0.0, -40.0, 600.0]);
        assert_eq!(seg(&v["screen"][1]), [0.0, 0.0, 0.0, 600.0]);
        // 与 core 同一条公式(防止 FFI 入口另写一份)。
        let core = crate::grid::compute(&crate::grid::GridParams::view(
            800.0,
            600.0,
            Camera::HOME,
            40.0,
        ));
        assert_eq!(array_len(&v["minor"]), core.minor.len());
        assert_eq!(array_len(&v["screen"]), core.screen.len());

        // 分栏模式位置(1=左右两栏, 吸附到最近网格线)。
        let v = take(glaspen2_grid_json(800.0, 600.0, 0.0, 0.0, 1.0, 40.0, 1, 0));
        assert_eq!(array_len(&v["divider"]), 4);
        assert_eq!(seg(&v["divider"][0]), [-400.0, 0.0, -400.0, 600.0]);

        // follow 只回显: 传 0/1 的三层线段完全相同(可见性门控, 不参与几何)。
        let a = take(glaspen2_grid_json(800.0, 600.0, 0.0, 0.0, 1.0, 40.0, 3, 0));
        let b = take(glaspen2_grid_json(800.0, 600.0, 0.0, 0.0, 1.0, 40.0, 3, 1));
        assert_eq!(a["minor"], b["minor"]);
        assert_eq!(a["screen"], b["screen"]);
        assert_eq!(a["divider"], b["divider"]);
        assert_eq!(
            (a["follow"].clone(), b["follow"].clone()),
            (false.into(), true.into())
        );

        // 脏矩形入口: 只回与 clip 相交的线(macOS 旧实现的省遍历优化)。
        let v = take(glaspen2_grid_json_clipped(
            800.0, 600.0, 0.0, 0.0, 1.0, 40.0, 0, 1, 100.0, 100.0, 200.0, 200.0,
        ));
        assert_eq!(array_len(&v["minor"]), 6 + 6);
        assert_eq!(array_len(&v["screen"]), 3 + 3);
        // 反向 clip 端点归一; 非有限 clip 忽略(按全视口算), 都不报错。
        let swapped = take(glaspen2_grid_json_clipped(
            800.0, 600.0, 0.0, 0.0, 1.0, 40.0, 0, 1, 200.0, 200.0, 100.0, 100.0,
        ));
        assert_eq!(swapped["minor"], v["minor"]);
        let nan_clip = take(glaspen2_grid_json_clipped(
            800.0,
            600.0,
            0.0,
            0.0,
            1.0,
            40.0,
            0,
            1,
            f64::NAN,
            0.0,
            1.0,
            1.0,
        ));
        assert_eq!(array_len(&nan_clip["minor"]), 23 + 18);
        // 只开分栏(show_grid=0): 不算细网格/屏幕分界线, 分栏线照画。
        let v = take(glaspen2_grid_json_clipped(
            800.0, 600.0, 0.0, 0.0, 1.0, 40.0, 3, 0, 0.0, 0.0, 800.0, 600.0,
        ));
        assert_eq!(array_len(&v["minor"]), 0);
        assert_eq!(array_len(&v["screen"]), 0);
        assert_eq!(array_len(&v["divider"]), 8 + 8);

        // 非法输入 → error JSON(不 NULL、不 panic、不含 NaN)。
        for v in [
            take(glaspen2_grid_json(0.0, 600.0, 0.0, 0.0, 1.0, 40.0, 0, 0)),
            take(glaspen2_grid_json(
                800.0,
                600.0,
                f64::NAN,
                0.0,
                1.0,
                40.0,
                0,
                0,
            )),
            take(glaspen2_grid_json(800.0, 600.0, 0.0, 0.0, 0.0, 40.0, 0, 0)),
            take(glaspen2_grid_json(800.0, 600.0, 0.0, 0.0, 1.0, 0.0, 0, 0)),
            take(glaspen2_grid_json_clipped(
                800.0, 600.0, 0.0, 0.0, 1.0, -5.0, 0, 1, 0.0, 0.0, 800.0, 600.0,
            )),
        ] {
            let error = v["error"].as_str().unwrap_or_default();
            assert!(error.contains("网格"), "error JSON 应说明网格非法: {v}");
        }
    }

    /// D7: 缩略图条布局 JSON 形状 + 与 core::minimap 的往返一致性 + error JSON。
    #[test]
    fn minimap_ffi_roundtrip() {
        use crate::minimap::{
            MACOS_METRICS, MinimapError, PageFilter, WINDOW_COUNT, WINDOW_HALF, layout,
            page_window_from_rows,
        };

        fn number(value: &serde_json::Value, key: &str) -> f64 {
            value[key]
                .as_f64()
                .unwrap_or_else(|| panic!("{key} 应为数字: {value}"))
        }

        // 页列表: 5 页, 当前页 3(索引 2);1920 组两页(1/4), 3440 组三页。
        let rows = vec![
            (1, 1920, 1080),
            (2, 3440, 1440),
            (3, 3440, 1440),
            (4, 1920, 1080),
            (5, 3440, 1440),
        ];
        let value = minimap_layout_json_value(&rows, 3, 1440.0, 900.0, PageFilter::All).unwrap();
        let window =
            page_window_from_rows(&rows, 3, PageFilter::All, WINDOW_HALF, WINDOW_COUNT).unwrap();
        let core = layout(1440.0, 900.0, &window.ids, window.start, 3, &MACOS_METRICS).unwrap();
        for (key, expected) in [
            ("strip_x", core.strip_x),
            ("strip_y", core.strip_y),
            ("strip_w", core.strip_w),
            ("strip_h", core.strip_h),
        ] {
            assert_eq!(number(&value, key), expected, "{key}");
        }
        let items = value["items"].as_array().expect("items 数组");
        assert_eq!(items.len(), core.items.len());
        for (json, core_item) in items.iter().zip(&core.items) {
            assert_eq!(json["screen_id"].as_i64(), Some(core_item.screen_id));
            assert_eq!(json["index"].as_u64(), Some(core_item.index as u64));
            assert_eq!(json["label"].as_str(), Some(core_item.label.as_str()));
            assert_eq!(json["current"].as_bool(), Some(core_item.current));
            assert_eq!(number(json, "y"), core_item.y);
            assert_eq!(number(json, "h"), core_item.h);
        }
        // 当前页高亮恰好一个;页号 = 全列表 1 基序号。
        assert_eq!(items.iter().filter(|i| i["current"] == true).count(), 1);
        assert_eq!(items[2]["label"], "3");
        // 公式抽查: total_h = 5*40 + 4*4 = 216;macOS 顶端 8。
        assert_eq!(
            (number(&value, "strip_h"), number(&value, "strip_y")),
            (216.0, 8.0)
        );

        // 几何组过滤(macOS page-view-fit): 只留 1920 组 → [1, 4], 当前页 4。
        let value =
            minimap_layout_json_value(&rows, 4, 1440.0, 900.0, PageFilter::Width(1920)).unwrap();
        let items = value["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["screen_id"], 1);
        assert_eq!(items[1]["screen_id"], 4);
        assert_eq!(items[1]["label"], "2");

        // 当前页不在活页列表(新建空白页): 合法空 items, 不是 error。
        let value = minimap_layout_json_value(&rows, 99, 1440.0, 900.0, PageFilter::All).unwrap();
        assert!(value["items"].as_array().unwrap().is_empty());
        assert!(value.get("error").is_none());

        // 非法视图尺寸 → core error(FFI 包成 {"error"})。
        assert_eq!(
            minimap_layout_json_value(&rows, 3, 0.0, 900.0, PageFilter::All).unwrap_err(),
            MinimapError::ViewSize
        );

        // 真实 FFI 入口: 永不 NULL 且单行 JSON;非法尺寸回 error JSON。
        let take = |ptr: *mut c_char| -> serde_json::Value {
            assert!(
                !ptr.is_null(),
                "minimap FFI 必须返回 JSON 字符串, 不能 NULL"
            );
            let text = unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned();
            glaspen2_free_c_string(ptr);
            assert!(!text.contains('\n'), "minimap JSON 必须单行: {text}");
            serde_json::from_str(&text)
                .unwrap_or_else(|e| panic!("minimap JSON 不合法: {e}: {text}"))
        };
        let value = take(glaspen2_minimap_layout_json(3, 1440.0, 900.0, 0));
        assert!(value["strip_x"].is_number());
        assert!(value["items"].is_array());
        let err = take(glaspen2_minimap_layout_json(3, 0.0, 900.0, 0));
        let message = err["error"].as_str().unwrap_or_default();
        assert!(
            message.contains("缩略图"),
            "error JSON 应说明 minimap 非法: {err}"
        );
    }

    /// D8b 录制状态机桥 FFI: macOS ObjC 依赖的槽位语义 + JSON 形状。
    ///
    /// 槽位 0 = GIF, 1 = 手写消息(草稿/直发互斥);事件 JSON 喂入,effects JSON
    /// 数组按序返回;非法槽位/坏 JSON 回 error JSON(永不 NULL);mode 标量供
    /// ObjC 的 key-up 门控。Windows 同进程直接用枚举, 本测试只覆盖 ObjC 路径。
    #[test]
    fn recording_ffi_roundtrip() {
        fn take(ptr: *mut c_char) -> serde_json::Value {
            assert!(!ptr.is_null(), "录制 FFI 必须返回 JSON 字符串, 不能 NULL");
            let text = unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned();
            glaspen2_free_c_string(ptr);
            assert!(!text.contains('\n'), "录制 JSON 必须单行: {text}");
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("录制 JSON 不合法: {e}: {text}"))
        }
        fn feed(slot: c_int, event: &str) -> serde_json::Value {
            let event_c = CString::new(event).unwrap();
            take(glaspen2_recording_event_json(slot, event_c.as_ptr()))
        }
        fn effects(value: serde_json::Value) -> Vec<String> {
            value
                .as_array()
                .unwrap_or_else(|| panic!("effects 必须是数组: {value}"))
                .iter()
                .map(|e| e["kind"].as_str().unwrap_or("").to_string())
                .collect()
        }

        // 槽位 0: GIF 开始/结束窗口 + 结果顺序。
        assert_eq!(glaspen2_recording_init(0, 0), 1);
        assert_eq!(glaspen2_recording_mode(0), 0);
        let v = feed(
            0,
            r#"{"type":"StartPressed","mode":"Gif","stroke_count":3}"#,
        );
        assert_eq!(v[0]["kind"], "ShowHud");
        assert_eq!(v[0]["hud"], "GifStartHint");
        assert_eq!(glaspen2_recording_mode(0), 1);
        // 键重复按下被 FSM 忽略 → 空数组(ObjC 不执行任何动作)。
        assert_eq!(
            effects(feed(
                0,
                r#"{"type":"StartPressed","mode":"Gif","stroke_count":9}"#
            )),
            Vec::<String>::new()
        );
        let v = feed(
            0,
            r#"{"type":"ReleasePressed","mode":"Gif","stroke_count":8}"#,
        );
        assert_eq!(v[0]["kind"], "EncodeGif");
        assert_eq!(v[0]["start_stroke"], 3);
        assert_eq!(v[0]["end_stroke"], 8);
        assert_eq!(glaspen2_recording_mode(0), 0);
        // 编码成功 → Clipboard;剪贴板结果 → HUD(顺序由 adapter 保证)。
        assert_eq!(
            effects(feed(0, r#"{"type":"GifEncodeSucceeded"}"#)),
            vec!["Clipboard".to_string()]
        );
        assert_eq!(
            feed(0, r#"{"type":"GifClipboardSucceeded"}"#)[0]["hud"],
            "GifCopied"
        );
        assert_eq!(
            feed(0, r#"{"type":"GifEncodeFailed"}"#)[0]["hud"],
            "GifNoStrokesOrExportFailed"
        );
        assert_eq!(
            feed(0, r#"{"type":"GifClipboardFailed"}"#)[0]["hud"],
            "GifClipboardFailed"
        );
        // 空窗口在 FSM 判定, 不经过 EncodeGif。
        let _ = feed(
            0,
            r#"{"type":"StartPressed","mode":"Gif","stroke_count":4}"#,
        );
        assert_eq!(
            feed(
                0,
                r#"{"type":"ReleasePressed","mode":"Gif","stroke_count":4}"#
            )[0]["hud"],
            "GifNoStrokesOrExportFailed"
        );

        // 槽位 1: 手写消息——总开关关闭时草稿/直发不开始;打开后才生效。
        assert_eq!(glaspen2_recording_init(1, 0), 1);
        assert_eq!(
            effects(feed(
                1,
                r#"{"type":"StartPressed","mode":"Draft","stroke_count":0}"#
            )),
            Vec::<String>::new()
        );
        assert_eq!(glaspen2_recording_mode(1), 0);
        let _ = feed(1, r#"{"type":"ChatIntegrationChanged","enabled":true}"#);
        assert_eq!(
            effects(feed(
                1,
                r#"{"type":"StartPressed","mode":"Draft","stroke_count":0}"#
            )),
            vec!["StartChatDraft".to_string()]
        );
        assert_eq!(glaspen2_recording_mode(1), 2);
        assert_eq!(
            feed(1, r#"{"type":"ChatDraftStarted"}"#)[0]["hud"],
            "DraftStartHint"
        );
        assert_eq!(
            effects(feed(
                1,
                r#"{"type":"ReleasePressed","mode":"Draft","stroke_count":5}"#
            )),
            vec!["StopChatDraft".to_string()]
        );
        // 裁决 code 经 DraftOutcome::from_ffi_code 单源映射。
        let v = feed(1, r#"{"type":"ChatDraftFinished","code":2,"detail":""}"#);
        assert_eq!(v[0]["hud"], "DraftAccepted");
        assert_eq!(v[0]["strokes"], 2);
        assert_eq!(
            feed(1, r#"{"type":"ChatDraftFinished","code":0}"#)[0]["hud"],
            "DraftDeclined"
        );
        let v = feed(
            1,
            r#"{"type":"ChatDraftFinished","code":-1,"detail":"身份过期"}"#,
        );
        assert_eq!(v[0]["hud"], "DraftFailed");
        assert_eq!(v[0]["detail"], "身份过期");
        // 直发: 发送结果事件 → 带条数的 HUD。
        let _ = feed(
            1,
            r#"{"type":"StartPressed","mode":"Send","stroke_count":1}"#,
        );
        assert_eq!(
            effects(feed(
                1,
                r#"{"type":"ReleasePressed","mode":"Send","stroke_count":3}"#
            )),
            vec!["SendChatStrokes".to_string()]
        );
        let v = feed(1, r#"{"type":"ChatSendSucceeded","strokes":2}"#);
        assert_eq!(v[0]["hud"], "SendSucceeded");
        assert_eq!(v[0]["strokes"], 2);
        assert_eq!(
            feed(1, r#"{"type":"ChatSendFailed"}"#)[0]["hud"],
            "SendFailed"
        );
        // 总开关关闭时草稿收尾 = StopChatDraft(Windows 侧据此补齐提交与裁决)。
        let _ = feed(1, r#"{"type":"ChatIntegrationChanged","enabled":true}"#);
        let _ = feed(
            1,
            r#"{"type":"StartPressed","mode":"Draft","stroke_count":0}"#,
        );
        assert_eq!(
            effects(feed(
                1,
                r#"{"type":"ChatIntegrationChanged","enabled":false}"#
            )),
            vec!["StopChatDraft".to_string()]
        );
        // 关总开关时直发 = Discard(静默不发送)。
        let _ = feed(1, r#"{"type":"ChatIntegrationChanged","enabled":true}"#);
        let _ = feed(
            1,
            r#"{"type":"StartPressed","mode":"Send","stroke_count":1}"#,
        );
        assert_eq!(
            effects(feed(
                1,
                r#"{"type":"ChatIntegrationChanged","enabled":false}"#
            )),
            vec!["Discard".to_string()]
        );

        // reset: 回空闲, 保留总开关镜像(这里关掉后 reset 仍是关)。
        let _ = feed(1, r#"{"type":"ChatIntegrationChanged","enabled":false}"#);
        let _ = feed(
            1,
            r#"{"type":"StartPressed","mode":"Gif","stroke_count":0}"#,
        );
        assert_eq!(glaspen2_recording_reset(1), 1);
        assert_eq!(glaspen2_recording_mode(1), 0);

        // 非法槽位/坏 JSON/空指针: error JSON 且永不 NULL。
        assert_eq!(glaspen2_recording_init(2, 0), 0);
        assert_eq!(glaspen2_recording_reset(-1), 0);
        assert_eq!(glaspen2_recording_mode(9), -1);
        let v = feed(5, r#"{"type":"ForceStop"}"#);
        assert!(v["error"].is_string(), "非法槽位应回 error: {v}");
        let v = feed(0, r#"{"type":"NoSuch"}"#);
        assert!(v["error"].is_string(), "坏事件应回 error: {v}");
        let ptr = glaspen2_recording_event_json(0, std::ptr::null());
        assert!(!ptr.is_null());
        let text = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        glaspen2_free_c_string(ptr);
        assert!(
            serde_json::from_str::<serde_json::Value>(&text).unwrap()["error"].is_string(),
            "空指针应回 error JSON: {text}"
        );
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

    /// D10a 审计 JSON FFI: 单行、合法、覆盖全表。
    #[test]
    fn i18n_catalog_json_ffi() {
        let ptr = glaspen2_i18n_catalog_json();
        assert!(!ptr.is_null(), "审计 JSON 不能 NULL");
        let text = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        glaspen2_free_c_string(ptr);
        assert!(!text.contains('\n'), "审计 JSON 必须单行");
        let value: serde_json::Value = serde_json::from_str(&text).expect("审计 JSON 合法");
        assert_eq!(
            value.as_object().expect("对象").len(),
            crate::i18n::Msg::ALL.len()
        );
        // 与纯函数输出同一份(FFI 只是转发, 不另拼一份)。
        assert_eq!(text, crate::i18n::catalog_json());
    }
}
