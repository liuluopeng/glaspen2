//! FFI export functions — `#[unsafe(no_mangle)] extern "C"` API callable from ObjC/C#.
//! Extracted from lib.rs to keep the crate root focused on types and modules.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_double, c_int, c_uchar};
use std::path::PathBuf;
use std::slice;
use std::sync::Arc;

use crate::{
    RAW_STROKE_START, STROKES, Stroke, db, desktop_path, modeler, pressure_to_width, runtime,
    state, timestamped_name, timestamped_path,
};

// ---------------------------------------------------------------------------
// Drawing FFI (legacy, non-modeler path)
// ---------------------------------------------------------------------------

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

/// Start a new canvas: a new page is created only when the notebook's last
/// page has strokes — 空白页之后不能再造空白页(要画就画在那页空白页上,
/// 不在末页时自动跳过去复用)。Returns 1 if a new page was created, 0 if
/// the existing blank tail page was reused (possibly after navigating to it).
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_clear_strokes(screen_w: c_int, screen_h: c_int) -> c_int {
    runtime().block_on(db::end_stroke()); // flush before checking — must block
    // 无限画布:全局只有一个画布,清空内容即可,不新建页。
    // 返回 1 表示"清掉了东西",0 表示本来就是空的。
    if state::canvas_kind() == state::CanvasKind::Infinite {
        let had = runtime().block_on(db::infinite_canvas_has_strokes());
        runtime().block_on(db::clear_infinite_canvas());
        STROKES.lock().unwrap().clear();
        return if had { 1 } else { 0 };
    }
    let current = state::current_screen_id();
    let last = runtime().block_on(async {
        match db::last_screen_id().await {
            Some(id) => Some((id, db::screen_has_strokes(id).await)),
            None => None,
        }
    });
    let (create, reuse) = plan_new_page(last);
    let mut created = 0;
    if create {
        runtime().block_on(db::new_screen(screen_w, screen_h));
        created = 1;
    } else if let Some(id) = reuse {
        // 末页已是空白:复用它,不再新建;不在那页就跳过去
        if id != current {
            glaspen2_load_strokes_for_screen(id);
        }
    }
    let mut strokes = STROKES.lock().unwrap();
    strokes.clear();
    created
}

/// 描边(轮廓)渲染开关 —— 纯渲染设置,只在内存,不落库、重启即恢复关闭。
static STROKE_OUTLINE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// 描边比笔迹宽出的半径(px)。描边层 = 同路径加宽 2×OUTLINE 后置于笔迹之下。
const OUTLINE_PAD: f64 = 1.0;

/// 按笔色亮度选对比描边色(与 Windows contrast_color 同参数:BT.601,阈值 128)。
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
    let Some(r) = crate::cairo_dl::CairoRenderer::from_surface(surface_ptr) else {
        return;
    };
    r.clear();
    let (pan_x, pan_y, zoom) = view_transform();
    let outline = STROKE_OUTLINE.load(std::sync::atomic::Ordering::SeqCst);
    let strokes = STROKES.lock().unwrap();

    // 主页笔迹
    paint_strokes(&r, &strokes, pan_x, pan_y, zoom, scale, 0.0, outline);

    // 活页本跨页显示:相邻两页的笔迹画在本页上下(视口滑出页界时可见)。
    // 仅翻页模式(无限画布全局只有一张,无邻页)。
    let cur = crate::state::current_screen_id();
    if cur > 0 && crate::state::canvas_kind() != crate::state::CanvasKind::Infinite {
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
            paint_strokes(&r, group, pan_x, pan_y, zoom, scale, *dy * stride, outline);
        }
        // 页号跟随:各页区域顶部标注页号(滑动跨页时知道自己在哪)
        if let Some(info) = runtime().block_on(db::page_info(cur)) {
            let cur_ord = info.2; // 全局位置(1 起)
            let label = |shift: f64, text: String| {
                r.draw_text(
                    (20.0 - pan_x) * zoom * scale,
                    (44.0 + shift - pan_y) * zoom * scale,
                    22.0 * zoom * scale,
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

/// 把一组笔迹画到 renderer 上(支持跨页 y 偏移与描边层)。
fn paint_strokes(
    r: &crate::cairo_dl::CairoRenderer,
    strokes: &[Stroke],
    pan_x: f64,
    pan_y: f64,
    zoom: f64,
    scale: f64,
    y_shift: f64,
    outline: bool,
) {
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
                let (px, py, _pw, _pt) = pts[i - 1];
                r.stroke_line(
                    ((px - pan_x) * zoom * scale) as f32,
                    ((py + y_shift - pan_y) * zoom * scale) as f32,
                    ((x - pan_x) * zoom * scale) as f32,
                    ((y + y_shift - pan_y) * zoom * scale) as f32,
                    (w * zoom * scale) as f32,
                    color,
                );
            }
        }
    }
}

/// Undo the last stroke: remove from both STROKES (memory) and DB.
/// Returns the number of remaining strokes, or -1 if there was nothing to undo.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_undo_last_stroke() -> c_int {
    let id = {
        let mut strokes = STROKES.lock().unwrap();
        if strokes.is_empty() {
            return -1;
        }
        strokes.pop().map(|s| s.id).unwrap_or(0)
    };
    if id > 0 {
        if state::canvas_kind() == state::CanvasKind::Infinite {
            runtime().block_on(db::delete_infinite_stroke_by_id(id));
        } else {
            runtime().block_on(db::delete_stroke_by_id(id));
        }
    }
    STROKES.lock().unwrap().len() as c_int
}

/// Initialize the database and create the first screen record. Call once at app start.
/// 沿用活页本末页作为当前页(不再每次启动新建一页——那会积累大量空白页);
/// 只有空库才创建第一页。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_init_db(screen_w: c_int, screen_h: c_int) {
    runtime().block_on(db::init());
    match runtime().block_on(db::last_screen_id()) {
        Some(id) => state::set_current_screen_id(id),
        None => runtime().block_on(db::new_screen(screen_w, screen_h)),
    }
    warm_thumbnail_cache();
}

/// Background-fill the thumbnail cache so the 活页本 grid opens instantly.
/// Renders at the settings panel's size (280) only; pages already cached or
/// drawn later are filled on demand. Yields between pages to stay out of the
/// way of live drawing.
fn warm_thumbnail_cache() {
    std::thread::Builder::new()
        .name("thumb-warm".into())
        .spawn(|| {
            std::thread::sleep(std::time::Duration::from_secs(5));
            let outline = STROKE_OUTLINE.load(std::sync::atomic::Ordering::SeqCst);
            for (id, _w, _h) in runtime().block_on(db::list_screens()) {
                let (count, max_id) = runtime().block_on(db::screen_stroke_version(id));
                if count == 0 {
                    continue;
                }
                let cached =
                    runtime().block_on(db::thumbnail_lookup(id, 280, count, max_id, outline));
                if cached.is_some() {
                    continue;
                }
                let mut len: c_int = 0;
                let ptr = glaspen2_render_thumbnail(id, 0, 0, 280, &mut len);
                if !ptr.is_null() {
                    glaspen2_free_rust_bytes(ptr, len);
                }
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
        })
        .ok();
}

/// Called when the display size/arrangement changed. Only starts a new page
/// when the current page already has strokes; otherwise the current page is
/// kept (avoiding silent page switches from resolution changes).
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_on_display_change(screen_w: c_int, screen_h: c_int) {
    let current = state::current_screen_id();
    if !runtime().block_on(db::screen_has_strokes(current)) {
        return;
    }
    let last = runtime().block_on(async {
        match db::last_screen_id().await {
            Some(id) => Some((id, db::screen_has_strokes(id).await)),
            None => None,
        }
    });
    let (create, _) = plan_new_page(last);
    if create {
        runtime().block_on(db::new_screen(screen_w, screen_h));
    }
}

/// 新建页守卫的纯决策:活页本末页(未删除页中最新的一页)已有笔迹 →
/// 允许新建;**末页空白 → 不允许**(空白页之后不能再造空白页,要画就画
/// 在那页上);空库 → 新建第一页。返回 (是否新建, 可复用的末页 id)。
fn plan_new_page(last: Option<(i64, bool)>) -> (bool, Option<i64>) {
    match last {
        Some((id, true)) => (true, None),
        Some((id, false)) => (false, Some(id)),
        None => (true, None),
    }
}

// ---------------------------------------------------------------------------
// Modeler FFI
// ---------------------------------------------------------------------------

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
pub extern "C" fn glaspen2_load_strokes_for_screen(screen_id: i64) -> c_int {
    // Flush any stroke whose pen-up was still queued, so page navigation
    // never races the async point flush.
    runtime().block_on(db::end_stroke());
    let data = runtime().block_on(db::strokes_for_screen(screen_id));
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
    // Update current screen in DB
    state::set_current_screen_id(screen_id);
    count
}

/// Smooth all loaded strokes in STROKES through the modeler. Call after loading.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_smooth_loaded_strokes() {
    // Snapshot under lock; run CPU-heavy smoothing outside the lock.
    let snapshot: Vec<(f64, f64, f64, Vec<(f64, f64, f64)>)> = {
        let strokes = STROKES.lock().unwrap();
        strokes
            .iter()
            .map(|s| {
                (
                    s.r,
                    s.g,
                    s.b,
                    s.points.iter().map(|&(x, y, w, _)| (x, y, w)).collect(),
                )
            })
            .collect()
    };

    let mut smoothed_all: Vec<Vec<(f64, f64, f64, f64)>> = Vec::with_capacity(snapshot.len());
    for (_, _, _, raw) in snapshot.iter() {
        let smoothed = modeler::smooth_points(raw);
        smoothed_all.push(smoothed);
    }

    let mut strokes = STROKES.lock().unwrap();
    for (stroke, smoothed) in strokes.iter_mut().zip(smoothed_all) {
        if !smoothed.is_empty() {
            stroke.points = decimate(&smoothed);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_prev_screen_id() -> i64 {
    runtime()
        .block_on(db::prev_screen(state::current_screen_id()))
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_next_screen_id() -> i64 {
    runtime()
        .block_on(db::next_screen(state::current_screen_id()))
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_get_current_screen_id() -> i64 {
    state::current_screen_id()
}

/// Delete a screen (page) and all its data (strokes, points).
/// Returns 1 on success, 0 on failure.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_delete_screen(screen_id: i64) -> c_int {
    let ok = runtime().block_on(db::delete_screen(screen_id));
    // 只有翻页模式才把"被删的是当前页"反映到内存;无限画布与页存储无关。
    if ok
        && state::canvas_kind() == state::CanvasKind::Page
        && screen_id == state::current_screen_id()
    {
        state::set_current_screen_id(0);
        STROKES.lock().unwrap().clear();
    }
    ok as c_int
}

// ---------------------------------------------------------------------------
// Stroke introspection
// ---------------------------------------------------------------------------

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

fn xoj_timestamped_path() -> PathBuf {
    let desktop = desktop_path();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600 + 8) % 24;
    let days = secs / 86400;
    let y = 1970 + days / 365;
    let d = days % 365;
    let filename = format!("glaspen2_{:04}-{:03}_{:02}-{:02}-{:02}.xoj", y, d, h, m, s);
    desktop.join(filename)
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_save_xoj() {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::fmt::Write as _;
    use std::io::Write;

    // Snapshot strokes under lock, then encode/write without holding it.
    let snapshot: Vec<(f64, f64, f64, Vec<(f64, f64, f64)>)> = {
        let strokes = STROKES.lock().unwrap();
        strokes
            .iter()
            .map(|s| {
                (
                    s.r,
                    s.g,
                    s.b,
                    s.points.iter().map(|&(x, y, w, _)| (x, y, w)).collect(),
                )
            })
            .collect()
    };

    // Get screen dimensions from the first point bounds, or use defaults
    let (mut max_x, mut max_y) = (1920.0f64, 1080.0f64);
    for (_, _, _, points) in snapshot.iter() {
        for &(x, y, _) in points {
            if x > max_x {
                max_x = x;
            }
            if y > max_y {
                max_y = y;
            }
        }
    }
    let page_w = (max_x + 10.0).ceil() as i32;
    let page_h = (max_y + 10.0).ceil() as i32;

    // Build XML — Xournal 0.4 spec: <stroke tool="pen" color="#rrggbb">
    // followed by "x y width" triples in the element body.
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" standalone=\"no\"?>\n");
    xml.push_str("<xournal version=\"0.4\" fileversion=\"4\">\n");
    xml.push_str(&format!(
        "  <page width=\"{}\" height=\"{}\">\n",
        page_w, page_h
    ));
    xml.push_str("    <layer>\n");

    for (r, g, b, points) in snapshot.iter() {
        if points.is_empty() {
            continue;
        }
        let color_hex = format!(
            "#{:02x}{:02x}{:02x}",
            (r * 255.0) as u8,
            (g * 255.0) as u8,
            (b * 255.0) as u8
        );
        xml.push_str(&format!(
            "      <stroke tool=\"pen\" color=\"{}\">\n        ",
            color_hex
        ));
        for &(x, y, w) in points {
            write!(xml, "{:.2} {:.2} {:.2} ", x, y, w).ok();
        }
        xml.push_str("\n      </stroke>\n");
    }

    xml.push_str("    </layer>\n");
    xml.push_str("  </page>\n");
    xml.push_str("</xournal>\n");

    // Gzip compress
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    if encoder.write_all(xml.as_bytes()).is_err() {
        return;
    }
    let Ok(compressed) = encoder.finish() else {
        return;
    };

    // Write to file
    let path = xoj_timestamped_path();
    match std::fs::write(&path, &compressed) {
        Ok(_) => println!("[glaspen2] Saved Xournal to {}", path.display()),
        Err(e) => eprintln!("[glaspen2] Xournal save failed: {}", e),
    }
}

// ---------------------------------------------------------------------------
// Settings
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

#[unsafe(no_mangle)]
// `enable` is only used in the macOS branch; other platforms ignore it.
#[allow(unused_variables)]
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

#[cfg(target_os = "macos")]
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// ---------------------------------------------------------------------------
// Drawing save (PNG — transparent)
// ---------------------------------------------------------------------------

/// Save drawing only (transparent background)
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_save_drawing(
    data: *const c_uchar,
    width: c_int,
    height: c_int,
    stride: c_int,
) {
    if data.is_null() || width <= 0 || height <= 0 || stride < width * 4 {
        return;
    }
    let w = width as u32;
    let h = height as u32;
    let s = stride as usize;
    let raw = unsafe { slice::from_raw_parts(data, s * h as usize) };

    let mut img = image::RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let offset = y as usize * s + x as usize * 4;
            if offset + 3 < raw.len() {
                // Cairo ARGB32 on little-endian: [B, G, R, A]
                let b = raw[offset];
                let g = raw[offset + 1];
                let r = raw[offset + 2];
                let a = raw[offset + 3];
                img.put_pixel(x, y, image::Rgba([r, g, b, a]));
            }
        }
    }

    let path = timestamped_path();
    match img.save(&path) {
        Ok(_) => println!("[glaspen2] Saved (drawing only) to {}", path.display()),
        Err(e) => eprintln!("[glaspen2] Save failed: {}", e),
    }
}

/// Save drawing composited on top of a background screenshot
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_save_with_background(
    drawing_data: *const c_uchar,
    drawing_width: c_int,
    drawing_height: c_int,
    drawing_stride: c_int,
    bg_data: *const c_uchar,
    bg_width: c_int,
    bg_height: c_int,
    bg_stride: c_int,
) {
    if drawing_data.is_null()
        || drawing_width <= 0
        || drawing_height <= 0
        || bg_data.is_null()
        || bg_width <= 0
        || bg_height <= 0
    {
        return;
    }
    if drawing_stride < drawing_width * 4 || bg_stride < bg_width * 4 {
        return;
    }
    let dw = drawing_width as u32;
    let dh = drawing_height as u32;
    let ds = drawing_stride as usize;
    let draw_raw = unsafe { slice::from_raw_parts(drawing_data, ds * dh as usize) };

    let bw = bg_width as u32;
    let bh = bg_height as u32;
    let bs = bg_stride as usize;
    let bg_raw = unsafe { slice::from_raw_parts(bg_data, bs * bh as usize) };

    // Create background image from BGRA pixel data
    let mut img = image::RgbaImage::new(bw, bh);
    for y in 0..bh {
        for x in 0..bw {
            let offset = y as usize * bs + x as usize * 4;
            if offset + 3 < bg_raw.len() {
                let b = bg_raw[offset];
                let g = bg_raw[offset + 1];
                let r = bg_raw[offset + 2];
                let a = bg_raw[offset + 3];
                img.put_pixel(x, y, image::Rgba([r, g, b, a]));
            }
        }
    }

    // Composite drawing on top with alpha blending
    for y in 0..dh.min(bh) {
        for x in 0..dw.min(bw) {
            let d_offset = y as usize * ds + x as usize * 4;
            if d_offset + 3 < draw_raw.len() {
                let db = draw_raw[d_offset] as f32;
                let dg = draw_raw[d_offset + 1] as f32;
                let dr = draw_raw[d_offset + 2] as f32;
                let da = draw_raw[d_offset + 3] as f32 / 255.0;

                if da > 0.01 {
                    let bg_pixel = img.get_pixel(x, y);
                    let br = bg_pixel[0] as f32;
                    let bg_g = bg_pixel[1] as f32;
                    let bb = bg_pixel[2] as f32;

                    let r = (dr * da + br * (1.0 - da)) as u8;
                    let g = (dg * da + bg_g * (1.0 - da)) as u8;
                    let b = (db * da + bb * (1.0 - da)) as u8;
                    img.put_pixel(x, y, image::Rgba([r, g, b, 255]));
                }
            }
        }
    }

    let path = timestamped_path();
    match img.save(&path) {
        Ok(_) => println!("[glaspen2] Saved (with background) to {}", path.display()),
        Err(e) => eprintln!("[glaspen2] Save failed: {}", e),
    }
}

// ---------------------------------------------------------------------------
// Bounding box + SVG
// ---------------------------------------------------------------------------

/// Compute bounding box of all strokes. Returns 1 if there are strokes, 0 otherwise.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_stroke_bbox(
    x_min: *mut c_double,
    y_min: *mut c_double,
    x_max: *mut c_double,
    y_max: *mut c_double,
) -> c_int {
    let strokes = STROKES.lock().unwrap();
    if strokes.is_empty() {
        return 0;
    }
    let mut bx_min = f64::MAX;
    let mut by_min = f64::MAX;
    let mut bx_max = f64::MIN;
    let mut by_max = f64::MIN;
    for s in strokes.iter() {
        for &(x, y, _, _) in &s.points {
            if x < bx_min {
                bx_min = x;
            }
            if y < by_min {
                by_min = y;
            }
            if x > bx_max {
                bx_max = x;
            }
            if y > by_max {
                by_max = y;
            }
        }
    }
    let padding = 10.0;
    unsafe {
        *x_min = bx_min - padding;
        *y_min = by_min - padding;
        *x_max = bx_max + padding;
        *y_max = by_max + padding;
    }
    1
}

/// Build cropped SVG string from current STROKES. Returns None if no strokes.
pub(crate) fn build_cropped_svg() -> Option<String> {
    // Snapshot under lock, then build the string without holding it.
    let snapshot: Vec<(f64, f64, f64, Vec<(f64, f64, f64)>)> = {
        let strokes = STROKES.lock().unwrap();
        strokes
            .iter()
            .map(|s| {
                (
                    s.r,
                    s.g,
                    s.b,
                    s.points.iter().map(|&(x, y, w, _)| (x, y, w)).collect(),
                )
            })
            .collect()
    };
    build_svg_from(&snapshot)
}

/// Snapshot DB stroke rows into the plain `(r, g, b, points)` form.
fn snapshot_from_stroke_data(
    data: &[db::StrokeData],
) -> Vec<(f64, f64, f64, Vec<(f64, f64, f64)>)> {
    data.iter()
        .map(|s| {
            (
                s.r,
                s.g,
                s.b,
                s.points.iter().map(|&(x, y, w, _)| (x, y, w)).collect(),
            )
        })
        .collect()
}

/// Round a stroke width to a 0.25px bucket so consecutive points that differ
/// only by sub-pixel pressure share one `<path>` run.
fn quantize_width(w: f64) -> f64 {
    (w * 4.0).round() / 4.0
}

/// Build an SVG cropped to the content bbox. Emits ONE `<path>` per run of
/// consecutive points that share a quantized width (plus one `<circle>` for the
/// first point) instead of one element per point, so a canvas with tens of
/// thousands of points stays a few thousand nodes and opens quickly.
fn build_svg_from(snapshot: &[(f64, f64, f64, Vec<(f64, f64, f64)>)]) -> Option<String> {
    let mut bx_min = f64::MAX;
    let mut by_min = f64::MAX;
    let mut bx_max = f64::MIN;
    let mut by_max = f64::MIN;
    for (_, _, _, points) in snapshot.iter() {
        for &(x, y, _) in points {
            bx_min = bx_min.min(x);
            by_min = by_min.min(y);
            bx_max = bx_max.max(x);
            by_max = by_max.max(y);
        }
    }
    if snapshot.is_empty() || bx_min > bx_max || by_min > by_max {
        return None;
    }
    let pad = 10.0;
    bx_min -= pad;
    by_min -= pad;
    bx_max += pad;
    by_max += pad;
    let bw = bx_max - bx_min;
    let bh = by_max - by_min;

    let mut svg = String::new();
    svg.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {:.1} {:.1}\" width=\"{:.1}\" height=\"{:.1}\">\n",
        bw, bh, bw, bh
    ));
    for (r, g, b, points) in snapshot.iter() {
        if points.is_empty() {
            continue;
        }
        let color_hex = format!(
            "#{:02x}{:02x}{:02x}",
            (r * 255.0) as u8,
            (g * 255.0) as u8,
            (b * 255.0) as u8
        );
        let (x0, y0, w0) = points[0];
        // First point: filled dot (round cap of a zero-length stroke).
        svg.push_str(&format!(
            "  <circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.2}\" fill=\"{}\"/>\n",
            x0 - bx_min,
            y0 - by_min,
            w0 * 0.5,
            color_hex
        ));
        // Segment i is drawn with points[i]'s width; merge equal-width runs.
        let n = points.len();
        let mut i = 1;
        while i < n {
            let wq = quantize_width(points[i].2);
            let mut d = format!(
                "M {:.1} {:.1}",
                points[i - 1].0 - bx_min,
                points[i - 1].1 - by_min
            );
            let mut j = i;
            while j < n && quantize_width(points[j].2) == wq {
                d.push_str(&format!(
                    " L {:.1} {:.1}",
                    points[j].0 - bx_min,
                    points[j].1 - by_min
                ));
                j += 1;
            }
            svg.push_str(&format!(
                "  <path d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{:.2}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>\n",
                d, color_hex, wq
            ));
            i = j;
        }
    }
    svg.push_str("</svg>\n");
    Some(svg)
}

/// Export the whole infinite canvas as one SVG (content bbox, no lens). 1 on success.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_export_infinite_svg() -> c_int {
    let data = runtime().block_on(db::load_infinite_strokes());
    let snapshot = snapshot_from_stroke_data(&data);
    match build_svg_from(&snapshot) {
        Some(svg) => {
            let path = desktop_path().join(timestamped_name("svg"));
            match std::fs::write(&path, &svg) {
                Ok(_) => {
                    println!(
                        "[glaspen2] Saved infinite-canvas SVG to {} ({} bytes)",
                        path.display(),
                        svg.len()
                    );
                    1
                }
                Err(e) => {
                    eprintln!("[glaspen2] SVG save failed: {}", e);
                    0
                }
            }
        }
        None => 0,
    }
}

/// Export the whole infinite canvas as a PDF split into screen-sized pages.
/// 1 on success, 0 on failure / nothing to export.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_export_infinite_pdf_paged(page_w: c_int, page_h: c_int) -> c_int {
    match crate::pdf::export_infinite_paged(page_w, page_h) {
        Some(_) => 1,
        None => 0,
    }
}

/// Save strokes as SVG to desktop (cropped to bbox).
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_save_svg() {
    if let Some(svg) = build_cropped_svg() {
        let path = desktop_path().join(timestamped_name("svg"));
        if let Err(e) = std::fs::write(&path, &svg) {
            eprintln!("[glaspen2] SVG save failed: {}", e);
        } else {
            println!("[glaspen2] Saved SVG to {}", path.display());
        }
    }
}

/// Generate cropped SVG as a C string. Caller must free with glaspen2_free_c_string.
/// Returns NULL if no strokes.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_get_cropped_svg() -> *mut c_char {
    match build_cropped_svg() {
        Some(svg) => match CString::new(svg) {
            Ok(cs) => cs.into_raw(),
            Err(_) => std::ptr::null_mut(),
        },
        None => std::ptr::null_mut(),
    }
}

/// Free a string returned by glaspen2_get_cropped_svg.
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
pub extern "C" fn glaspen2_save_gif_cropped(
    surface_data: *const c_uchar,
    surface_w: c_int,
    surface_h: c_int,
    surface_stride: c_int,
    surface_scale: c_double,
) -> c_int {
    if surface_data.is_null() || surface_w <= 0 || surface_h <= 0 {
        return 0;
    }
    if surface_stride < surface_w * 4 {
        return 0;
    }
    let w = surface_w as u32;
    let h = surface_h as u32;
    let scale = surface_scale.clamp(0.5, 4.0);
    let stride = surface_stride as usize;
    let raw = unsafe { slice::from_raw_parts(surface_data, stride * h as usize) };

    // Compute bbox under lock (cheap), then drop the lock before encoding.
    let (bx_min_u, by_min_u, bx_max_u, by_max_u) = {
        let strokes = STROKES.lock().unwrap();
        if strokes.is_empty() {
            return 0;
        }
        let mut bx_min = f64::MAX;
        let mut by_min = f64::MAX;
        let mut bx_max = f64::MIN;
        let mut by_max = f64::MIN;
        for s in strokes.iter() {
            for &(x, y, _, _) in &s.points {
                if x < bx_min {
                    bx_min = x;
                }
                if y < by_min {
                    by_min = y;
                }
                if x > bx_max {
                    bx_max = x;
                }
                if y > by_max {
                    by_max = y;
                }
            }
        }
        // Scale to physical surface coordinates
        bx_min = (bx_min * scale).floor();
        by_min = (by_min * scale).floor();
        bx_max = (bx_max * scale).ceil();
        by_max = (by_max * scale).ceil();
        let pad = (5.0 * scale).ceil() as u32;
        let bx_min_u = (bx_min as u32).saturating_sub(pad);
        let by_min_u = (by_min as u32).saturating_sub(pad);
        let bx_max_u = ((bx_max as u32) + pad).min(w.saturating_sub(1));
        let by_max_u = ((by_max as u32) + pad).min(h.saturating_sub(1));
        (bx_min_u, by_min_u, bx_max_u, by_max_u)
    };
    let crop_w = if bx_max_u > bx_min_u {
        bx_max_u - bx_min_u + 1
    } else {
        1
    };
    let crop_h = if by_max_u > by_min_u {
        by_max_u - by_min_u + 1
    } else {
        1
    };

    let Some(crop_bytes) = (crop_w as usize)
        .checked_mul(crop_h as usize)
        .and_then(|v| v.checked_mul(4))
    else {
        return 0;
    };
    let mut flat: Vec<u8> = Vec::with_capacity(crop_bytes);
    for cy in 0..crop_h {
        let sy = (by_min_u + cy) as usize;
        for cx in 0..crop_w {
            let sx = (bx_min_u + cx) as usize;
            let off = sy * stride + sx * 4;
            if off + 3 < raw.len() {
                let b = raw[off];
                let g = raw[off + 1];
                let r = raw[off + 2];
                let a = raw[off + 3];
                if a == 0 {
                    flat.extend_from_slice(&[0, 0, 0, 0]);
                } else {
                    flat.extend_from_slice(&[r, g, b, a]);
                }
            }
        }
    }
    // Downscale to 50% for smaller GIF (ceil so no edge column/row is dropped)
    let gif_w = crop_w.div_ceil(2).max(1);
    let gif_h = crop_h.div_ceil(2).max(1);
    let Some(gif_bytes) = (gif_w as usize)
        .checked_mul(gif_h as usize)
        .and_then(|v| v.checked_mul(4))
    else {
        return 0;
    };
    let mut gif_pixels: Vec<u8> = Vec::with_capacity(gif_bytes);
    for gy in 0..gif_h {
        for gx in 0..gif_w {
            let sx = gx * 2;
            let sy = gy * 2;
            let off = (sy * crop_w + sx) as usize * 4;
            if off + 3 < flat.len() {
                gif_pixels.extend_from_slice(&flat[off..off + 4]);
            }
        }
    }

    // Train the palette on opaque pixels only and reserve the last palette
    // index for transparency, so the transparent background (0,0,0,0) never
    // shares a palette entry with dark ink (which rendered the bg black).
    const PALETTE_BITS: usize = 7; // 2^7 = 128 colors
    const PALETTE_SIZE: usize = 1 << PALETTE_BITS;
    const TRANSPARENT_IDX: usize = PALETTE_SIZE - 1; // reserved, never quantized
    let train: Vec<u8> = gif_pixels
        .chunks(4)
        .filter(|p| p[3] > 0)
        .flat_map(|p| [p[0], p[1], p[2], 255u8])
        .collect();
    let quantizer = if train.is_empty() {
        color_quant::NeuQuant::new(30, PALETTE_SIZE - 1, &[0u8, 0, 0, 255])
    } else {
        color_quant::NeuQuant::new(30, PALETTE_SIZE - 1, &train)
    };
    let indices: Vec<u8> = gif_pixels
        .chunks(4)
        .map(|p| {
            if p[3] == 0 {
                TRANSPARENT_IDX as u8
            } else {
                quantizer.index_of(&[p[0], p[1], p[2], 255]) as u8
            }
        })
        .collect();
    // A single fixed transparent index that no opaque pixel ever maps to.
    let transparent = Some(TRANSPARENT_IDX as u8);
    let palette = quantizer.color_map_rgba();
    let gif_palette: Vec<u8> = (0..PALETTE_SIZE)
        .flat_map(|i| {
            if i == TRANSPARENT_IDX {
                [0u8, 0u8, 0u8]
            } else {
                [palette[i * 4], palette[i * 4 + 1], palette[i * 4 + 2]]
            }
        })
        .collect();
    let mut gif_data = Vec::new();
    {
        let mut enc =
            gif::Encoder::new(&mut gif_data, gif_w as u16, gif_h as u16, &gif_palette).unwrap();
        let frame = gif::Frame {
            width: gif_w as u16,
            height: gif_h as u16,
            buffer: std::borrow::Cow::Owned(indices),
            transparent,
            ..gif::Frame::default()
        };
        if let Err(e) = enc.write_frame(&frame) {
            eprintln!("[glaspen2] GIF encode failed: {}", e);
            return 0;
        }
    }
    let path = desktop_path().join(timestamped_name("gif"));
    match std::fs::write(&path, &gif_data) {
        Ok(_) => {
            println!("[glaspen2] Saved GIF to {}", path.display());
            1
        }
        Err(e) => {
            eprintln!("[glaspen2] GIF write failed: {}", e);
            0
        }
    }
}

// ---------------------------------------------------------------------------
// Animated GIF
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct GifStroke {
    r: f64,
    g: f64,
    b: f64,
    points: Vec<(f64, f64, f64, f64)>, // (x, y, width, relative_time)
}

/// Clone a slice of strokes into the render-only GifStroke representation.
fn gif_strokes_from(strokes: &[Stroke]) -> Vec<GifStroke> {
    strokes
        .iter()
        .map(|s| GifStroke {
            r: s.r,
            g: s.g,
            b: s.b,
            points: s.points.clone(),
        })
        .collect()
}

/// Encode an animated GIF that replays `gif_strokes` in drawing order at
/// (roughly) real speed. Caller owns the slice — no lock is taken here, so it
/// is safe to run on a background thread once the data has been cloned.
/// Returns raw GIF bytes, or None on any failure.
///
/// `fps` is the GIF frame rate (1..=50), `resolution` the size multiplier in
/// (0.0, 1.0] (1.0 = full size), `speed` the playback speed multiplier
/// (higher = faster replay). `end_mode` controls the ending:
///   0 = play once and stop on the last stroke,
///   1 = hold the last stroke ~1s then loop,
///   2 = loop immediately after the last stroke.
fn encode_animated_gif(
    gif_strokes: &[GifStroke],
    fps: i32,
    resolution: f64,
    speed: f64,
    end_mode: i32,
) -> Option<Vec<u8>> {
    if gif_strokes.is_empty() {
        return None;
    }

    let fps = fps.clamp(1, 50) as f64;
    let res = resolution.clamp(0.1, 1.0);
    let speed = speed.clamp(0.25, 20.0);

    // ── Bounding box ──
    let mut bx_min = f64::MAX;
    let mut by_min = f64::MAX;
    let mut bx_max = f64::MIN;
    let mut by_max = f64::MIN;
    for s in gif_strokes.iter() {
        for &(x, y, _, _) in &s.points {
            if x < bx_min {
                bx_min = x;
            }
            if y < by_min {
                by_min = y;
            }
            if x > bx_max {
                bx_max = x;
            }
            if y > by_max {
                by_max = y;
            }
        }
    }
    let pad = 10.0;
    bx_min -= pad;
    by_min -= pad;
    bx_max += pad;
    by_max += pad;
    let bw = (bx_max - bx_min).ceil() as i32;
    let bh = (by_max - by_min).ceil() as i32;
    if bw < 4 || bh < 4 {
        return None;
    }
    let gif_w = ((bw as f64) * res).round().clamp(1.0, 10000.0) as u16;
    let gif_h = ((bh as f64) * res).round().clamp(1.0, 10000.0) as u16;

    // ── Compressed timeline ──
    struct Seg {
        si: usize,
        dur: f64,
    }
    let segments: Vec<Seg> = gif_strokes
        .iter()
        .enumerate()
        .filter_map(|(si, s)| {
            if s.points.len() < 2 {
                return None;
            }
            let dur = s.points[s.points.len() - 1].3 - s.points[0].3;
            if dur <= 0.0 {
                None
            } else {
                Some(Seg { si, dur })
            }
        })
        .collect();
    if segments.is_empty() {
        return None;
    }

    // Small floor so very fast playback (up to 20x) still shows each stroke
    // instead of collapsing to zero duration; low enough that high speeds differ.
    const MIN_SEG: f64 = 0.01;
    let total_active: f64 = segments
        .iter()
        .map(|seg| (seg.dur / speed).max(MIN_SEG))
        .sum();
    if total_active < 0.01 {
        return None;
    }

    let seg_offset: Vec<(usize, f64, f64)> = {
        let mut v = Vec::new();
        let mut cur = 0.0;
        for seg in &segments {
            let adj = (seg.dur / speed).max(MIN_SEG);
            v.push((seg.si, cur, cur + adj));
            cur += adj;
        }
        v
    };

    // Frame count and delay follow the requested fps: the GIF plays the
    // (speed-compressed) timeline at `fps` frames/second. Long clips are capped
    // at MAX_DRAW frames; the delay is clamped to >=2cs (GIF/decoder minimum).
    // Cap the frame count so a long/high-fps recording never spins up 240+
    // full frames (which dominates generation time). ~150 frames still plays
    // smoothly (effective fps adjusts via draw_delay).
    const MAX_DRAW: usize = 150;
    // Ending hold frames: 0 = stop on last frame / loop immediately,
    // 1 = a single 1s (100cs) hold frame before looping.
    let n_hold: usize = match end_mode {
        1 => 1,
        _ => 0,
    };
    let play_time = total_active.clamp(0.5, 5.0);
    let n_draw = ((play_time * fps).round() as usize).clamp(1, MAX_DRAW);
    let draw_delay = ((play_time / n_draw as f64) * 100.0)
        .round()
        .clamp(2.0, 100.0) as u16;
    let n_frames = n_draw + n_hold;

    // ── Parallel frame rendering (rayon global thread pool) ──
    use rayon::prelude::*;

    let n_threads = rayon::current_num_threads();
    eprintln!(
        "[glaspen2] animated GIF: rayon threads={}, n_frames={}",
        n_threads, n_frames
    );

    let mut frame_results: Vec<(usize, Vec<u8>, u16)> = (0..n_frames)
        .into_par_iter()
        .map(|fi| {
            let is_hold = fi >= n_draw;
            let cutoff = (fi.min(n_draw - 1) as f64 / n_draw as f64) * total_active;
            let delay = if is_hold { 100u16 } else { draw_delay };

            let (flat, _ok) = render_gif_frame(
                gif_strokes,
                &seg_offset,
                bw,
                bh,
                bx_min,
                by_min,
                gif_w,
                gif_h,
                fi,
                is_hold,
                cutoff,
                delay,
            );
            (fi, flat, delay)
        })
        .collect();

    frame_results.sort_by_key(|&(fi, _, _)| fi);
    let frame_pixels: Vec<(Vec<u8>, u16)> = frame_results
        .into_iter()
        .map(|(_, px, d)| (px, d))
        .collect();

    // ── Palette ──
    // Train the palette on OPAQUE pixels only (alpha > 0) and reserve the last
    // palette index as the transparent index. This stops the transparent
    // background (0,0,0,0) from sharing a palette entry with dark ink, which
    // used to flash the background black once a frame got busy.
    const PALETTE_BITS: usize = 6; // 2^6 = 64 colors
    const PALETTE_SIZE: usize = 1 << PALETTE_BITS;
    const TRANSPARENT_IDX: usize = PALETTE_SIZE - 1; // reserved, never quantized
    // Uniform alpha (255) so NeuQuant's alpha distance can't bias matching
    // toward the low-alpha background. Built in parallel across frames.
    let train: Vec<u8> = frame_pixels
        .par_iter()
        .flat_map_iter(|(px, _)| {
            px.chunks(4)
                .filter(|p| p[3] > 0)
                .flat_map(|p| [p[0], p[1], p[2], 255u8])
        })
        .collect();
    let quantizer = if train.is_empty() {
        color_quant::NeuQuant::new(30, PALETTE_SIZE - 1, &[0u8, 0, 0, 255])
    } else {
        color_quant::NeuQuant::new(30, PALETTE_SIZE - 1, &train)
    };
    let palette = quantizer.color_map_rgba();
    let gif_palette: Vec<u8> = (0..PALETTE_SIZE)
        .flat_map(|i| {
            if i == TRANSPARENT_IDX {
                [0u8, 0u8, 0u8] // unused (transparent) — value does not matter
            } else {
                [palette[i * 4], palette[i * 4 + 1], palette[i * 4 + 2]]
            }
        })
        .collect();

    // Per-frame palette indices: background pixels always use the reserved
    // transparent index; opaque (or antialiased) pixels quantize to 0..(N-2).
    // Computed in parallel across frames (the pixel-per-index match dominates).
    let frame_indices: Vec<Vec<u8>> = frame_pixels
        .par_iter()
        .map(|(pixels, _)| {
            pixels
                .chunks(4)
                .map(|p| {
                    if p[3] == 0 {
                        TRANSPARENT_IDX as u8
                    } else {
                        quantizer.index_of(&[p[0], p[1], p[2], 255]) as u8
                    }
                })
                .collect()
        })
        .collect();

    // A single fixed transparent index that no opaque pixel ever maps to.
    let transparent = Some(TRANSPARENT_IDX as u8);

    // ── Encode GIF ──
    // Turn each frame's palette indices into a `gif::Frame`, then LZW-compress
    // them independently IN PARALLEL (the gif crate explicitly supports this:
    // frames can be compressed separately from the Encoder). This step was the
    // single-core bottleneck, so parallelizing it scales across cores. The final
    // sequential `write_lzw_pre_encoded_frame` only copies the compressed bytes.
    let mut frames: Vec<gif::Frame<'static>> = frame_pixels
        .iter()
        .zip(frame_indices.iter())
        .map(|((_, delay), indices)| gif::Frame {
            width: gif_w,
            height: gif_h,
            buffer: std::borrow::Cow::Owned(indices.clone()),
            delay: *delay,
            transparent,
            ..gif::Frame::default()
        })
        .collect();
    frames.par_iter_mut().for_each(|f| f.make_lzw_pre_encoded());

    let mut gif_data = Vec::new();
    {
        let mut enc = match gif::Encoder::new(&mut gif_data, gif_w, gif_h, &gif_palette) {
            Ok(e) => e,
            Err(_) => return None,
        };
        // Repeat::Finite(0) writes no Netscape loop extension, so viewers play
        // the GIF once and hold the last frame; Infinite makes it loop.
        let repeat = if end_mode == 0 {
            gif::Repeat::Finite(0)
        } else {
            gif::Repeat::Infinite
        };
        enc.set_repeat(repeat).ok();

        for f in &frames {
            if enc.write_lzw_pre_encoded_frame(f).is_err() {
                return None;
            }
        }
    }

    Some(gif_data)
}

/// Save an animated GIF of every stroke currently in memory to the desktop.
/// The GIF replays the strokes in drawing order at (roughly) real speed.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_save_animated_gif(
    fps: c_int,
    resolution: c_double,
    speed: c_double,
    end_mode: c_int,
) -> c_int {
    let gif_strokes = {
        let strokes = STROKES.lock().unwrap();
        gif_strokes_from(&strokes)
    };

    let gif_data = match encode_animated_gif(&gif_strokes, fps, resolution, speed, end_mode) {
        Some(d) => d,
        None => return 0,
    };

    let path = desktop_path().join(timestamped_name("gif"));
    match std::fs::write(&path, &gif_data) {
        Ok(_) => {
            println!(
                "[glaspen2] Saved animated GIF to {} ({} bytes)",
                path.display(),
                gif_data.len()
            );
            1
        }
        Err(e) => {
            eprintln!("[glaspen2] Animated GIF write failed: {}", e);
            0
        }
    }
}

/// Encode an animated GIF of the strokes in the half-open index range
/// `[start_index, end_index)` of the in-memory stroke list. The range is
/// passed explicitly by the caller (captured at key-down and key-up) so a new
/// recording can never clobber a pending one. Returns a heap buffer the caller
/// must free with glaspen2_free_rust_bytes (len via `out_len`), or NULL on
/// failure or an empty range. `fps`, `resolution`, `speed` and `end_mode` drive
/// the encoder.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_gif_record_end(
    start_index: c_int,
    end_index: c_int,
    fps: c_int,
    resolution: c_double,
    speed: c_double,
    end_mode: c_int,
    out_len: *mut c_int,
) -> *mut c_uchar {
    if out_len.is_null() || start_index < 0 || end_index < 0 {
        unsafe {
            *out_len = 0;
        }
        return std::ptr::null_mut();
    }

    let gif_strokes = {
        let strokes = STROKES.lock().unwrap();
        let start = start_index as usize;
        let end = (end_index as usize).min(strokes.len());
        if end <= start || start >= strokes.len() {
            unsafe {
                *out_len = 0;
            }
            return std::ptr::null_mut();
        }
        gif_strokes_from(&strokes[start..end])
    };

    match encode_animated_gif(&gif_strokes, fps, resolution, speed, end_mode) {
        Some(gif_data) => {
            let len = gif_data.len() as c_int;
            let ptr = gif_data.as_ptr() as *mut c_uchar;
            std::mem::forget(gif_data);
            unsafe {
                *out_len = len;
            }
            ptr
        }
        None => {
            unsafe {
                *out_len = 0;
            }
            std::ptr::null_mut()
        }
    }
}

/// Render a single frame of the animated GIF.
/// Returns (flat RGBA pixels, success). Called from multiple threads.
#[inline]
fn render_gif_frame(
    strokes: &[GifStroke],
    seg_offset: &[(usize, f64, f64)],
    bw: i32,
    bh: i32,
    bx_min: f64,
    by_min: f64,
    gif_w: u16,
    gif_h: u16,
    _fi: usize,
    is_hold: bool,
    cutoff: f64,
    _delay: u16,
) -> (Vec<u8>, bool) {
    // 渲染统一走 crate::cairo_dl(动态加载 cairo). Render directly at the GIF
    // resolution (not the full bbox) so low-resolution GIFs do far less work.
    let scale_x = if bw > 0 {
        gif_w as f64 / bw as f64
    } else {
        1.0
    };
    let scale_y = if bh > 0 {
        gif_h as f64 / bh as f64
    } else {
        1.0
    };
    let width_scale = (scale_x + scale_y) * 0.5;
    let renderer = match crate::cairo_dl::CairoRenderer::create_owned(gif_w as i32, gif_h as i32) {
        Some(r) => r,
        None => return (Vec::new(), false),
    };
    renderer.clear();

    // Render strokes (coordinates scaled to GIF resolution)
    for &(si, seg_start, seg_end) in seg_offset {
        let s = &strokes[si];

        let pts: Vec<(f64, f64, f64)> = if is_hold || cutoff >= seg_end {
            s.points
                .iter()
                .map(|&(x, y, w, _)| {
                    (
                        (x - bx_min) * scale_x,
                        (y - by_min) * scale_y,
                        w * width_scale,
                    )
                })
                .collect()
        } else if cutoff > seg_start {
            let local_frac = (cutoff - seg_start) / (seg_end - seg_start);
            let local_cut =
                s.points[0].3 + local_frac * (s.points[s.points.len() - 1].3 - s.points[0].3);
            s.points
                .iter()
                .take_while(|&&(_, _, _, t)| t <= local_cut)
                .map(|&(x, y, w, _)| {
                    (
                        (x - bx_min) * scale_x,
                        (y - by_min) * scale_y,
                        w * width_scale,
                    )
                })
                .collect()
        } else {
            Vec::new()
        };
        if pts.is_empty() {
            continue;
        }

        let color = (
            (s.r * 255.0) as u8,
            (s.g * 255.0) as u8,
            (s.b * 255.0) as u8,
        );
        for i in 0..pts.len() {
            let (cx, cy, w) = pts[i];
            if i == 0 {
                renderer.fill_circle(cx as f32, cy as f32, (w * 0.5) as f32, color);
            } else {
                let (px, py, _) = pts[i - 1];
                renderer.stroke_line(px as f32, py as f32, cx as f32, cy as f32, w as f32, color);
            }
        }
    }
    renderer.flush();

    // Read pixels directly at GIF resolution (BGRA premultiplied, stride = gif_w*4)
    let bits = renderer.bits();
    let stride = gif_w.max(1) as u32;
    let gw = gif_w as u32;
    let gh = gif_h as u32;
    let mut flat = Vec::with_capacity((gw * gh * 4) as usize);
    unsafe {
        for y in 0..gh {
            let row = (y * stride) as usize * 4;
            for x in 0..gw {
                let off = row + x as usize * 4;
                flat.push(*bits.add(off + 2)); // R
                flat.push(*bits.add(off + 1)); // G
                flat.push(*bits.add(off)); // B
                flat.push(*bits.add(off + 3)); // A
            }
        }
    }
    (flat, true)
}

// ---------------------------------------------------------------------------
// Misc FFI
// ---------------------------------------------------------------------------

/// Get current time as seconds since Unix epoch (f64). For modeler timestamps.
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

/// Void undo — legacy callers (returns nothing).
/// The macOS equivalent glaspen2_undo_last_stroke returns remaining count.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_delete_last_stroke() {
    if state::canvas_kind() == state::CanvasKind::Infinite {
        runtime().block_on(db::delete_last_infinite_stroke());
    } else {
        runtime().block_on(db::delete_last_stroke());
    }
    STROKES.lock().unwrap().pop();
}

// ---------------------------------------------------------------------------
// PDF export
// ---------------------------------------------------------------------------

/// Export all pages to a PDF on the desktop.  Returns 1 on success, 0 on failure.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_export_pdf() -> c_int {
    match crate::pdf::export_all_pages() {
        Some(_) => 1,
        None => 0,
    }
}

// ---------------------------------------------------------------------------
// Content tab data (page listing)
// ---------------------------------------------------------------------------

/// List all screens as JSON.
/// Returns a C string (caller must free via glaspen2_free_c_string).
/// JSON: [{"id":1,"w":1920,"h":1080}, ...]
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_list_screens_json() -> *mut c_char {
    let rows = runtime().block_on(db::list_screens());
    let list: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|(id, w, h)| serde_json::json!({ "id": id, "w": w, "h": h }))
        .collect();
    let json = serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string());
    CString::new(json).unwrap_or_default().into_raw()
}

/// Page info JSON for the 新建画布/翻页 notification:
/// {"nth":n,"date_total":m,"pos":x,"total":y,"created":unix_ts}
/// Caller frees with glaspen2_free_c_string. NULL when the page is unknown.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_page_info_json(screen_id: i64) -> *mut c_char {
    let Some((nth, date_total, pos, total, created)) = runtime().block_on(db::page_info(screen_id))
    else {
        return std::ptr::null_mut();
    };
    let json = serde_json::json!({
        "nth": nth,
        "date_total": date_total,
        "pos": pos,
        "total": total,
        "created": created,
    })
    .to_string();
    match CString::new(json) {
        Ok(cs) => cs.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

// ---------------------------------------------------------------------------
// Thumbnail rendering (cairo_dl → scaled PNG)
// ---------------------------------------------------------------------------

/// Render a page thumbnail cropped to the content bounding box.
/// Instead of rendering the full screen and downsampling (which makes small
/// doodles illegible), we find the stroke bounding box, add a small margin,
/// and render only that region — so the thumbnail tightly fits the content.
/// Never touches the global STROKES.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_render_thumbnail(
    screen_id: i64,
    _w: c_int,
    _h: c_int,
    max_size: c_int,
    out_len: *mut c_int,
) -> *mut c_uchar {
    if max_size <= 0 || out_len.is_null() {
        if !out_len.is_null() {
            unsafe {
                *out_len = 0;
            }
        }
        return std::ptr::null_mut();
    }
    unsafe {
        *out_len = 0;
    }

    // ── 0. 缓存:内容版本(笔迹数,最大笔迹id)+渲染参数未变 → 直接返回存库 PNG ──
    let outline = STROKE_OUTLINE.load(std::sync::atomic::Ordering::SeqCst);
    let (count, max_id) = runtime().block_on(db::screen_stroke_version(screen_id));
    if count > 0
        && let Some(png) = runtime().block_on(db::thumbnail_lookup(
            screen_id, max_size, count, max_id, outline,
        ))
    {
        return leak_png(png, out_len);
    }

    match render_and_store_thumbnail(screen_id, max_size, (count, max_id), outline) {
        Some(png) => leak_png(png, out_len),
        None => std::ptr::null_mut(),
    }
}

/// Load one page's strokes, render its thumbnail and cache it under
/// `version` = (live stroke count, max stroke id). `None` when the page has
/// nothing drawable or cairo is unavailable.
fn render_and_store_thumbnail(
    screen_id: i64,
    max_size: i32,
    version: (i64, i64),
    outline: bool,
) -> Option<Vec<u8>> {
    let strokes = runtime().block_on(db::strokes_for_screen(screen_id));
    if strokes.is_empty() {
        return None;
    }
    let png = render_strokes_thumbnail(&strokes, max_size)?;
    runtime().block_on(db::thumbnail_store(
        screen_id, max_size, version.0, version.1, outline, &png,
    ));
    Some(png)
}

/// Crop to the content bounding box (padded by line width radius + margin) and
/// render directly at thumbnail resolution, transparent background.
fn render_strokes_thumbnail(strokes: &[db::StrokeData], max_size: i32) -> Option<Vec<u8>> {
    // ── 1. 内容包围盒(含线宽半径) ──
    let mut bx0 = f64::MAX;
    let mut by0 = f64::MAX;
    let mut bx1 = f64::MIN;
    let mut by1 = f64::MIN;
    for s in strokes {
        for &(x, y, wd, _) in &s.points {
            let half = wd * 0.5;
            bx0 = bx0.min(x - half);
            by0 = by0.min(y - half);
            bx1 = bx1.max(x + half);
            by1 = by1.max(y + half);
        }
    }
    // 外扩留白(不低于 16pt,防止贴边)
    let margin = (bx1 - bx0).max(by1 - by0) * 0.06 + 12.0;
    bx0 -= margin;
    by0 -= margin;
    bx1 += margin;
    by1 += margin;
    let bw = (bx1 - bx0).max(1.0);
    let bh = (by1 - by0).max(1.0);

    // 2. 适配 max_size(最长边 = max_size,保持纵横比)
    let fit = (max_size as f64 / bw.max(bh)).min(1.0);
    let ow = ((bw * fit).ceil() as i32).max(1);
    let oh = ((bh * fit).ceil() as i32).max(1);

    // 3. 渲染(坐标偏移到 bbox 起点,缩放到 fit,透明底)
    let renderer = crate::cairo_dl::CairoRenderer::create_owned(ow, oh)?;
    renderer.clear();
    for s in strokes {
        if s.points.len() < 2 {
            continue;
        }
        let color = (
            (s.r.clamp(0.0, 1.0) * 255.0) as u8,
            (s.g.clamp(0.0, 1.0) * 255.0) as u8,
            (s.b.clamp(0.0, 1.0) * 255.0) as u8,
        );
        for i in 0..s.points.len() {
            let (x, y, wd, _t) = s.points[i];
            let px = ((x - bx0) * fit) as f32;
            let py = ((y - by0) * fit) as f32;
            let pw = (wd * fit) as f32;
            if i == 0 {
                renderer.fill_circle(px, py, pw * 0.5, color);
            } else {
                let (qx, qy, _qw, _qt) = s.points[i - 1];
                renderer.stroke_line(
                    ((qx - bx0) * fit) as f32,
                    ((qy - by0) * fit) as f32,
                    px,
                    py,
                    pw,
                    color,
                );
            }
        }
    }
    renderer.flush();

    // 4. BGRA → RGBA + PNG 编码(已是目标尺寸,无需降采样)
    let bits = renderer.bits();
    let stride = ow as usize;
    let n = stride * oh as usize * 4;
    let rgba: Vec<u8> = unsafe {
        std::slice::from_raw_parts(bits, n)
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|px| [px[2], px[1], px[0], px[3]]) // BGRA → RGBA
            .collect()
    };
    encode_png_rgba(&rgba, ow as u32, oh as u32)
}

/// Magic + per-entry framing for `glaspen2_page_thumbnails` (see there).
pub(crate) const THUMB_BLOB_MAGIC: u32 = 0x3148_5447; // "GTH1"

/// Batched page thumbnails in a single self-describing blob — one FFI call and
/// one channel round trip for a whole screenful of the 活页本 grid, instead of
/// two SQL queries plus a PNG transfer per page.
///
/// Layout, little endian: magic u32, entry count u32, then per entry
/// `id i64, len u32, png bytes`. Pages with no drawable strokes are omitted, so
/// the blob may describe fewer pages than requested. Caller frees the buffer
/// with `glaspen2_free_rust_bytes`; NULL means "no thumbnails at all".
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_page_thumbnails(
    ids: *const i64,
    count: c_int,
    max_size: c_int,
    out_len: *mut c_int,
) -> *mut c_uchar {
    if out_len.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        *out_len = 0;
    }
    if ids.is_null() || count <= 0 || max_size <= 0 {
        return std::ptr::null_mut();
    }
    let ids = unsafe { std::slice::from_raw_parts(ids, count as usize) };
    let blob = page_thumbnails_blob(ids, max_size);
    if blob.is_empty() {
        return std::ptr::null_mut();
    }
    leak_png(blob, out_len)
}

/// Build the blob described by `glaspen2_page_thumbnails`: one batched version
/// query, one batched cache read, and a render only for the pages whose cache
/// entry is missing or stale.
pub(crate) fn page_thumbnails_blob(ids: &[i64], max_size: i32) -> Vec<u8> {
    let mut seen = std::collections::HashSet::with_capacity(ids.len());
    let ids: Vec<i64> = ids.iter().copied().filter(|id| seen.insert(*id)).collect();
    if ids.is_empty() {
        return Vec::new();
    }

    let outline = STROKE_OUTLINE.load(std::sync::atomic::Ordering::SeqCst);
    // 分批查询:SQLite 的绑定变量有上限,超长 IN 列表会整条失败
    const CHUNK: usize = 400;
    let mut versions = std::collections::HashMap::new();
    let mut cached = std::collections::HashMap::new();
    for chunk in ids.chunks(CHUNK) {
        versions.extend(runtime().block_on(db::stroke_versions_many(chunk)));
        cached.extend(runtime().block_on(db::thumbnails_many(chunk, max_size, outline)));
    }

    let mut entries: Vec<(i64, Vec<u8>)> = Vec::with_capacity(ids.len());
    for &id in &ids {
        let version = versions.get(&id).copied().unwrap_or((0, 0));
        // A cached PNG counts only when it was rendered from this exact version.
        let png = match cached.remove(&id) {
            Some((count, max_id, png)) if (count, max_id) == version => Some(png),
            _ if version.0 == 0 => None,
            _ => render_and_store_thumbnail(id, max_size, version, outline),
        };
        if let Some(png) = png {
            entries.push((id, png));
        }
    }

    encode_thumb_blob(&entries)
}

/// Frame `(id, png)` entries into the blob the settings panel parses.
/// Kept separate from the DB work so the wire format is unit-testable.
pub(crate) fn encode_thumb_blob(entries: &[(i64, Vec<u8>)]) -> Vec<u8> {
    let mut out =
        Vec::with_capacity(8 + entries.iter().map(|(_, png)| png.len() + 12).sum::<usize>());
    out.extend_from_slice(&THUMB_BLOB_MAGIC.to_le_bytes());
    out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for (id, png) in entries {
        out.extend_from_slice(&id.to_le_bytes());
        out.extend_from_slice(&(png.len() as u32).to_le_bytes());
        out.extend_from_slice(png);
    }
    out
}

/// Hand ownership of a PNG buffer to the caller via out_len (leak pattern).
fn leak_png(png: Vec<u8>, out_len: *mut c_int) -> *mut c_uchar {
    let len = png.len() as c_int;
    let ptr = png.as_ptr() as *mut c_uchar;
    std::mem::forget(png);
    unsafe {
        *out_len = len;
    }
    ptr
}

/// Free a buffer returned by glaspen2_render_thumbnail./// Free a buffer returned by glaspen2_render_thumbnail.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_free_rust_bytes(ptr: *mut c_uchar, len: c_int) {
    if !ptr.is_null() && len > 0 {
        unsafe {
            let _ = Vec::from_raw_parts(ptr, len as usize, len as usize);
        }
    }
}

/// 无限画布总览:把当前 STROKES 按包围盒 [bx,by,bw,bh] 适配进
/// out_w×out_h(居中、透明底),返回 PNG 字节,由 glaspen2_free_rust_bytes
/// 释放。失败返回 NULL 且 *out_len = 0。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_render_canvas_overview(
    bx: c_double,
    by: c_double,
    bw: c_double,
    bh: c_double,
    out_w: c_int,
    out_h: c_int,
    out_len: *mut c_int,
) -> *mut c_uchar {
    if out_len.is_null() || out_w <= 0 || out_h <= 0 || bw <= 0.0 || bh <= 0.0 {
        if !out_len.is_null() {
            unsafe {
                *out_len = 0;
            }
        }
        return std::ptr::null_mut();
    }
    unsafe {
        *out_len = 0;
    }
    let scale = (out_w as f64 / bw).min(out_h as f64 / bh);
    let off_x = (out_w as f64 - bw * scale) * 0.5;
    let off_y = (out_h as f64 - bh * scale) * 0.5;
    let Some(r) = crate::cairo_dl::CairoRenderer::create_owned(out_w, out_h) else {
        return std::ptr::null_mut();
    };
    r.clear();
    let strokes = STROKES.lock().unwrap();
    for s in strokes.iter() {
        let pts = &s.points;
        if pts.len() < 2 {
            continue;
        }
        let color = (
            (s.r.clamp(0.0, 1.0) * 255.0) as u8,
            (s.g.clamp(0.0, 1.0) * 255.0) as u8,
            (s.b.clamp(0.0, 1.0) * 255.0) as u8,
        );
        for i in 0..pts.len() {
            let (x, y, w, _t) = pts[i];
            let sx = ((x - bx) * scale + off_x) as f32;
            let sy = ((y - by) * scale + off_y) as f32;
            let sw = (w * scale).max(1.0) as f32;
            if i == 0 {
                r.fill_circle(sx, sy, sw * 0.5, color);
            } else {
                let (px, py, _pw, _pt) = pts[i - 1];
                r.stroke_line(
                    ((px - bx) * scale + off_x) as f32,
                    ((py - by) * scale + off_y) as f32,
                    sx,
                    sy,
                    sw,
                    color,
                );
            }
        }
    }
    drop(strokes);
    r.flush();

    let bits = r.bits();
    let n = (out_w as usize) * (out_h as usize) * 4;
    let rgba = unsafe { std::slice::from_raw_parts(bits, n).to_vec() };
    let Some(png) = encode_png_rgba(&rgba, out_w as u32, out_h as u32) else {
        return std::ptr::null_mut();
    };
    let len = png.len() as c_int;
    let ptr = png.as_ptr() as *mut c_uchar;
    std::mem::forget(png);
    unsafe {
        *out_len = len;
    }
    ptr
}

/// Encode RGBA pixel data as PNG bytes.
fn encode_png_rgba(rgba: &[u8], width: u32, height: u32) -> Option<Vec<u8>> {
    use image::ImageEncoder;
    let mut buf = Vec::new();
    image::codecs::png::PngEncoder::new(&mut buf)
        .write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
        .ok()?;
    Some(buf)
}

// ---------------------------------------------------------------------------
// 聊天流(⌘⌃3 录制的手写消息 → 本地 axum 存储)
// ---------------------------------------------------------------------------

/// 手写消息归属的流。当前形态是"本机涂鸦工具",单一流即可;
/// 服务端按 (notebook_id, seq) 去重。
const CHAT_NOTEBOOK: &str = "glaspen2-doodle";

/// 流内 seq 的高水位,持久化在 user_settings 里:应用重启后接着涨,
/// 避免服务端把重放的新消息当成重复吞掉。
static CHAT_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// 预留 n 个连续 seq,返回首个 seq。首次调用时从 DB 加载高水位。
fn reserve_chat_seqs(n: u64) -> u64 {
    use std::sync::atomic::Ordering;
    static LOADED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    LOADED.get_or_init(|| {
        let v = runtime()
            .block_on(db::load_setting("chat_seq"))
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        CHAT_SEQ.store(v, Ordering::SeqCst);
    });
    let first = CHAT_SEQ.fetch_add(n, Ordering::SeqCst) + 1;
    runtime().block_on(db::save_setting(
        "chat_seq",
        &CHAT_SEQ.load(Ordering::SeqCst).to_string(),
    ));
    first
}

/// 一条笔迹 → 一条 STROKE 消息(无作者/设备字段,涂鸦工具不携带身份)。
fn stroke_to_chat_message(seq: u64, s: &Stroke) -> glaspen_chat::pb::ChatMessage {
    let to8 = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    let color_rgb = (to8(s.r) << 16) | (to8(s.g) << 8) | to8(s.b);
    glaspen_chat::stroke_message(
        CHAT_NOTEBOOK,
        seq,
        "",
        "",
        color_rgb,
        1.0, // 线宽逐点携带在 points 里,全局倍率固定 1.0
        &s.points,
        None, // flow 布局:聊天流里按块下排
    )
}

/// 把录制窗口 [start_index, end_index) 内的笔迹打包成手写消息,发送到聊天服务。
/// 由 macOS 端 ⌘⌃3 key-up 在后台线程调用(start/end 均在主线程钉好)。
/// 返回发送的消息条数;无新笔迹返回 0;发送失败返回 -1。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_chat_send_strokes(start_index: c_int, end_index: c_int) -> c_int {
    let strokes: Vec<Stroke> = {
        let all = STROKES.lock().unwrap();
        let start = start_index.clamp(0, all.len() as i32) as usize;
        let end = (end_index.clamp(0, all.len() as i32) as usize).max(start);
        all[start..end].to_vec()
    };
    if strokes.is_empty() {
        return 0;
    }

    let first = reserve_chat_seqs(strokes.len() as u64);
    let msgs: Vec<_> = strokes
        .iter()
        .enumerate()
        .map(|(i, s)| stroke_to_chat_message(first + i as u64, s))
        .collect();

    let endpoint = glaspen_chat::endpoint_from_env();
    let send = async {
        let mut sink = glaspen_chat::connect(&endpoint)
            .await
            .map_err(|e| e.to_string())?;
        sink.append(&msgs).await
    };
    match runtime().block_on(send) {
        Ok(summary) => {
            eprintln!("[chat] sent {} strokes (seq {}..)", summary.accepted, first);
            summary.accepted as c_int
        }
        Err(e) => {
            eprintln!("[chat] send failed: {e}");
            -1
        }
    }
}

// ---------------------------------------------------------------------------
// 手写消息草稿通道(⌘⌃2 按住 → ChatStore/DraftInk gRPC 流 → axum 决定是否发送)
// 与 ⌘⌃3 直发的区别:⌘⌃3 是松开后一次性 AppendMessages;⌘⌃2 按住期间
// 笔迹实时流给 axum(可预览),松开 half-close 后由 axum 决定发或不发。
// 语义契约见 docs/ink-draft-grpc.md。
// ---------------------------------------------------------------------------

struct InkDraftSession {
    channel: glaspen_chat::draft::DraftChannel,
    /// 已推送的 STROKES 下标游标(会话开启时的 STROKES.len(),只增不减;
    /// 会话期间发生撤销导致游标越界时直接跳过,宁漏不重)。
    pushed_upto: usize,
    stroke_count: u32,
    started_at: std::time::SystemTime,
}

static INK_DRAFT: std::sync::Mutex<Option<InkDraftSession>> = std::sync::Mutex::new(None);

/// 最近一次草稿通道失败的用户可读原因(ObjC 通知用);CString 常驻,
/// 指针在下次 set 之前一直有效。
static INK_DRAFT_LAST_ERROR: std::sync::Mutex<Option<CString>> = std::sync::Mutex::new(None);

fn set_ink_draft_error(err: Option<&str>) {
    *INK_DRAFT_LAST_ERROR.lock().unwrap() =
        err.map(|s| CString::new(s).unwrap_or_default());
}

/// 通道失败原因(UTF-8),无失败时返回 NULL。供 ObjC 在 stop 返回 <0 时
/// 展示具体原因(身份过期 / 未注册路由 / 连接失败等)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_ink_draft_last_error() -> *const c_char {
    match INK_DRAFT_LAST_ERROR.lock().unwrap().as_ref() {
        Some(c) => c.as_ptr(),
        None => std::ptr::null(),
    }
}

/// ⌘⌃2 key-down:打开手写草稿通道。ObjC 侧保证先 finish_active_stroke。
/// 返回 1 = 已开启;0 = 已有会话在进行(忽略本次)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_ink_draft_start(canvas_w: c_int, canvas_h: c_int) -> c_int {
    let mut g = INK_DRAFT.lock().unwrap();
    if g.is_some() {
        return 0;
    }
    let started_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let begin = glaspen_chat::pb::DraftBegin {
        session_id: glaspen_chat::draft::new_session_id(),
        started_at_ms,
        notebook_id: CHAT_NOTEBOOK.to_owned(),
        author: String::new(),
        device: String::new(),
        canvas_w: canvas_w.max(0) as u32,
        canvas_h: canvas_h.max(0) as u32,
    };
    // 端点解析优先级:设置库(chat_grpc_endpoint,面板/Dock 启动没有
    // 环境变量时靠它落地)> 环境变量 GLASPEN_CHAT_ENDPOINT > 默认值
    let db_endpoint = runtime().block_on(db::load_setting("chat_grpc_endpoint"))
        .filter(|v| !v.trim().is_empty());
    let endpoint = db_endpoint
        .clone()
        .unwrap_or_else(glaspen_chat::endpoint_from_env);
    // mock 判定:显式 GLASPEN_CHAT_MOCK=0 → 真连,=1 → mock;
    // 未设置时:配置了登录账号或涂鸦端点即默认真连(否则老用户装完
    // 什么都不配仍走 mock)。此前未设置一律 mock,配置了登录也发不出去
    let mock_env = std::env::var("GLASPEN_CHAT_MOCK").ok();
    let auth_ready = glaspen_chat::auth::config().is_configured();
    let mock = match mock_env.as_deref() {
        Some("0") => false,
        Some(_) => true,
        None => !(auth_ready || db_endpoint.is_some()),
    };
    let channel = if mock {
        glaspen_chat::draft::DraftChannel::launch_with(endpoint.as_str(), true, begin)
    } else {
        glaspen_chat::draft::DraftChannel::launch_with_auth(endpoint.as_str(), false, begin)
    };
    *g = Some(InkDraftSession {
        channel,
        pushed_upto: STROKES.lock().unwrap().len(),
        stroke_count: 0,
        started_at: std::time::SystemTime::now(),
    });
    eprintln!("[ink-draft] session opened (canvas {canvas_w}x{canvas_h}, endpoint {endpoint})");
    1
}

/// pen-up 提交笔迹后的钩子:会话进行中时,把本次提交的笔迹实时推进草稿流。
/// 在主线程调用;push 非阻塞(连接建立前帧在通道里缓冲)。
pub(crate) fn ink_draft_on_stroke_committed() {
    let mut g = INK_DRAFT.lock().unwrap();
    let Some(sess) = g.as_mut() else { return };
    let strokes = STROKES.lock().unwrap();
    while sess.pushed_upto < strokes.len() {
        let s = &strokes[sess.pushed_upto];
        sess.pushed_upto += 1;
        if s.points.is_empty() {
            continue;
        }
        sess.stroke_count += 1;
        let msg = stroke_to_chat_message(sess.stroke_count as u64, s);
        if !sess.channel.push_stroke(msg) {
            // 通道已死(连接失败/对端断开)。帧丢弃,结束时的 stop 会拿到
            // Failed 并通知用户;这里只留日志。
            eprintln!(
                "[ink-draft] channel dead at stroke {}, remaining frames dropped",
                sess.stroke_count
            );
            break;
        }
    }
}

// ---------------------------------------------------------------------------
// 涂鸦身份设置(设置面板 ↔ chat::auth):DB 里的账号配置推给 auth 模块
// ---------------------------------------------------------------------------

/// 从 DB 读取涂鸦身份设置(chat_api_base / chat_user / chat_password),
/// 字段级合并环境变量默认值后注入 chat::auth(配置变化会自动清 token 缓存)。
pub(crate) fn sync_chat_auth_from_settings() {
    let (base, user, pass) = runtime().block_on(async {
        (
            db::load_setting("chat_api_base").await.unwrap_or_default(),
            db::load_setting("chat_user").await.unwrap_or_default(),
            db::load_setting("chat_password").await.unwrap_or_default(),
        )
    });
    let nonempty = |s: String| if s.trim().is_empty() { None } else { Some(s) };
    let from_db = glaspen_chat::auth::AuthConfig {
        api_base: nonempty(base),
        user: nonempty(user),
        password: nonempty(pass),
        direct_token: None, // 直接给 token 只走环境变量,不落盘
    };
    glaspen_chat::auth::set_config(glaspen_chat::auth::AuthConfig::from_env().merged(from_db));
}

/// ObjC 入口:设置变化处与启动恢复路径调用。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_chat_auth_reload() {
    sync_chat_auth_from_settings();
}

/// 设置面板「测试登录」:强制用当前配置登录一次(成功则缓存 token)。
/// Ok = 成功;Err = 可读失败原因(给 Flutter 显示)。
pub(crate) fn chat_auth_test_login_blocking() -> Result<(), String> {
    runtime()
        .block_on(glaspen_chat::auth::force_login())
        .map(|_| ())
}

// ---------------------------------------------------------------------------
// 共享画布上行(面板「共享画布」tab 打开期间 → ChatStore/ShareInk):
// glaspen2 只作为手写工具 —— tab 开 = 建流,抬笔即推,tab 关 = half-close。
// 接收页在 kongde(经 axum 转给该用户的 ink-route);连接成败静默,
// 不进用户界面。语义与 axum 侧实现见 docs/canvas-share-grpc.md。
// ---------------------------------------------------------------------------

struct InkShareState {
    channel: glaspen_chat::canvas::InkShareChannel,
    stroke_count: u32,
    started_at: std::time::SystemTime,
}

static INK_SHARE: std::sync::Mutex<Option<InkShareState>> = std::sync::Mutex::new(None);

/// 面板切到「共享画布」tab(active=true)/切走或面板关闭(false)。
/// 幂等:重复开是 no-op,重复关也是 no-op。
pub(crate) fn share_ink_set_active_impl(active: bool) {
    if active {
        let mut g = INK_SHARE.lock().unwrap();
        if g.is_some() {
            return;
        }
        *g = Some(InkShareState {
            channel: glaspen_chat::canvas::InkShareChannel::launch(),
            stroke_count: 0,
            started_at: std::time::SystemTime::now(),
        });
        eprintln!("[share-ink] session opened");
        return;
    }
    let Some(sess) = INK_SHARE.lock().unwrap().take() else {
        return;
    };
    let stroke_count = sess.stroke_count;
    let duration_ms = std::time::SystemTime::now()
        .duration_since(sess.started_at)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    // end 帧 + half-close 放后台;结果只进 stderr,用户无感
    runtime().spawn(async move {
        sess.channel.finish(stroke_count, duration_ms).await;
    });
}

/// pen-up 提交笔迹后的共享钩子:tab 打开期间把本笔实时推给 axum。
/// 活页本/无限画布模式都发(坐标各自成系,去向由 kongde 决定);
/// 通道未连上时帧被丢弃(静默,不打扰)。
pub(crate) fn ink_share_on_stroke_committed() {
    let mut g = INK_SHARE.lock().unwrap();
    let Some(st) = g.as_mut() else { return };
    let Some(s) = STROKES.lock().unwrap().last().cloned() else {
        return;
    };
    let to8 = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    let msg = glaspen_chat::pb::ShareStroke {
        color_rgb: (to8(s.r) << 16) | (to8(s.g) << 8) | to8(s.b),
        width_scale: 1.0,
        points: s
            .points
            .iter()
            .map(|(x, y, w, t)| glaspen_chat::pb::StrokePoint {
                x: *x,
                y: *y,
                width: *w,
                t_rel: *t,
            })
            .collect(),
    };
    if st.channel.push_stroke(msg) {
        st.stroke_count += 1;
    } else {
        eprintln!("[share-ink] 通道未连接,本笔未发送");
    }
}

/// ⌘⌃2 key-up:补 end 帧 + half-close,阻塞等待 axum 的决定。
/// 由 ObjC 侧在后台线程调用(主线程先 finish_active_stroke 保证最后一笔
/// 已经过钩子推进流)。返回:>0 = axum 已发送(接受的笔迹条数);
/// 0 = axum 丢弃了草稿(含 sent 但接受 0 条);-1 = 通道失败/无会话。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_ink_draft_stop() -> c_int {
    let Some(sess) = INK_DRAFT.lock().unwrap().take() else {
        return -1;
    };
    let stroke_count = sess.stroke_count;
    let duration_ms = std::time::SystemTime::now()
        .duration_since(sess.started_at)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let outcome = runtime().block_on(sess.channel.finish(stroke_count, duration_ms));
    eprintln!("[ink-draft] session closed after {stroke_count} strokes / {duration_ms}ms: {outcome:?}");
    match outcome {
        glaspen_chat::draft::DraftOutcome::Sent { accepted, .. } => {
            set_ink_draft_error(None);
            accepted as c_int
        }
        glaspen_chat::draft::DraftOutcome::Dropped => {
            set_ink_draft_error(None);
            0
        }
        glaspen_chat::draft::DraftOutcome::Failed(e) => {
            set_ink_draft_error(Some(&e));
            eprintln!("[ink-draft] failed: {e}");
            -1
        }
    }
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
        assert_eq!(glaspen2_ink_draft_start(1920, 1080), 0, "会话进行中应拒绝二连开");
        STROKES.lock().unwrap().push(Stroke {
            id: 0,
            r: 1.0,
            g: 0.0,
            b: 0.0,
            points: vec![(1.0, 2.0, 2.5, 0.0), (30.0, 40.0, 3.5, 0.12)],
        });
        ink_draft_on_stroke_committed();
        assert_eq!(glaspen2_ink_draft_stop(), 1, "mock 通道应回执 sent/accepted=1");
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
