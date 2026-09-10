//! FFI export functions — `#[unsafe(no_mangle)] extern "C"` API callable from ObjC/C#.
//! Extracted from lib.rs to keep the crate root focused on types and modules.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_double, c_int, c_uchar};
use std::path::PathBuf;
use std::slice;

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
    let mut strokes = STROKES.lock().unwrap();
    if let Some(stroke) = strokes.last_mut() {
        stroke.points.push((x, y, width, 0.0));
    }
    state::buffer_point(x, y, width, 0.0); // sync
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_end_stroke() {
    db::end_stroke_spawned();
}

/// Start a new canvas: only when the current canvas was ever edited
/// (a blank canvas cannot spawn another blank canvas). Returns 1 if a new
/// screen was created, 0 if it was blocked (current canvas never edited).
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_clear_strokes(screen_w: c_int, screen_h: c_int) -> c_int {
    runtime().block_on(db::end_stroke()); // flush before checking — must block
    let current = state::current_screen_id();
    let mut created = 0;
    if runtime().block_on(db::screen_edited(current)) {
        runtime().block_on(db::new_screen(screen_w, screen_h));
        created = 1;
    }
    let mut strokes = STROKES.lock().unwrap();
    strokes.clear();
    created
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
            if i == 0 {
                // 起点实心圆点(圆帽)
                r.fill_circle(
                    (x * scale) as f32,
                    (y * scale) as f32,
                    (w * 0.5 * scale) as f32,
                    color,
                );
            } else {
                let (px, py, _pw, _pt) = pts[i - 1];
                r.stroke_line(
                    (px * scale) as f32,
                    (py * scale) as f32,
                    (x * scale) as f32,
                    (y * scale) as f32,
                    (w * scale) as f32,
                    color,
                );
            }
        }
    }
    drop(strokes);
    r.flush();
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
        runtime().block_on(db::delete_stroke_by_id(id));
    }
    STROKES.lock().unwrap().len() as c_int
}

/// Initialize the database and create the first screen record. Call once at app start.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_init_db(screen_w: c_int, screen_h: c_int) {
    runtime().block_on(db::init());
    runtime().block_on(db::new_screen(screen_w, screen_h));
}

/// Called when the display size/arrangement changed. Only starts a new page
/// when the current page already has strokes; otherwise the current page is
/// kept (avoiding silent page switches from resolution changes).
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_on_display_change(screen_w: c_int, screen_h: c_int) {
    let current = state::current_screen_id();
    if runtime().block_on(db::screen_has_strokes(current)) {
        runtime().block_on(db::new_screen(screen_w, screen_h));
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
    // If deleted screen was the current one, clear STROKES and navigate
    if ok && screen_id == state::current_screen_id() {
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
        if strokes.is_empty() {
            return None;
        }
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
    let mut bx_min = f64::MAX;
    let mut by_min = f64::MAX;
    let mut bx_max = f64::MIN;
    let mut by_max = f64::MIN;
    for (_, _, _, points) in snapshot.iter() {
        for &(x, y, _) in points {
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
        for i in 0..points.len() {
            let (x, y, w) = points[i];
            let cx = x - bx_min;
            let cy = y - by_min;
            if i == 0 {
                // First point: filled circle dot
                svg.push_str(&format!(
                    "  <circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.1}\" fill=\"{}\"/>\n",
                    cx,
                    cy,
                    w * 0.5,
                    color_hex
                ));
            } else {
                let (prev_x, prev_y, _) = points[i - 1];
                // Segment with destination-point width and round caps
                svg.push_str(&format!(
                    "  <line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"{}\" stroke-width=\"{:.1}\" stroke-linecap=\"round\"/>\n",
                    prev_x - bx_min, prev_y - by_min, cx, cy, color_hex, w
                ));
            }
        }
    }
    svg.push_str("</svg>\n");
    Some(svg)
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
    runtime().block_on(db::delete_last_stroke());
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

/// Render a page thumbnail. Never touches the global STROKES.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_render_thumbnail(
    screen_id: i64,
    w: c_int,
    h: c_int,
    max_size: c_int,
    out_len: *mut c_int,
) -> *mut c_uchar {
    if w <= 0 || h <= 0 || max_size <= 0 || out_len.is_null() {
        if !out_len.is_null() {
            unsafe {
                *out_len = 0;
            }
        }
        return std::ptr::null_mut();
    }

    let scale = if w >= h {
        max_size as f64 / w as f64
    } else {
        max_size as f64 / h as f64
    };
    let tw = ((w as f64) * scale).max(1.0) as i32;
    let th = ((h as f64) * scale).max(1.0) as i32;

    // Render from a local stroke list loaded from the DB — no global state.
    let strokes = runtime().block_on(db::strokes_for_screen(screen_id));
    if strokes.is_empty() {
        unsafe {
            *out_len = 0;
        }
        return std::ptr::null_mut();
    }
    let local: Vec<Stroke> = strokes
        .into_iter()
        .map(|s| Stroke {
            id: s.id,
            r: s.r,
            g: s.g,
            b: s.b,
            points: s.points,
        })
        .collect();

    // Render full resolution → scale down (preserves stroke proportions)
    let renderer = match crate::cairo_dl::CairoRenderer::create_owned(w, h) {
        Some(r) => r,
        None => {
            unsafe {
                *out_len = 0;
            }
            return std::ptr::null_mut();
        }
    };
    renderer.clear();
    for s in &local {
        if s.points.len() < 2 {
            continue;
        }
        let color = (
            (s.r * 255.0) as u8,
            (s.g * 255.0) as u8,
            (s.b * 255.0) as u8,
        );
        for i in 0..s.points.len() {
            let (x, y, wdt, _t) = s.points[i];
            if i == 0 {
                renderer.fill_circle(x as f32, y as f32, (wdt * 0.5) as f32, color);
            } else {
                let (px, py, _pw, _pt) = s.points[i - 1];
                renderer.stroke_line(px as f32, py as f32, x as f32, y as f32, wdt as f32, color);
            }
        }
    }
    renderer.flush();

    // Downsample (box average) to tw×th and reorder BGRA → RGBA
    let bits = renderer.bits();
    let stride = w as u32;
    let (tw_u, th_u) = (tw as u32, th as u32);
    let Some(cap) = (tw_u as usize)
        .checked_mul(th_u as usize)
        .and_then(|v| v.checked_mul(4))
    else {
        unsafe {
            *out_len = 0;
        }
        return std::ptr::null_mut();
    };
    let mut rgba = Vec::with_capacity(cap);
    unsafe {
        for y in 0..th_u {
            let sy0 = (y as u64 * h as u64 / th_u as u64) as i32;
            let sy1 = (((y + 1) as u64 * h as u64 / th_u as u64) as i32).max(sy0 + 1);
            for x in 0..tw_u {
                let sx0 = (x as u64 * w as u64 / tw_u as u64) as i32;
                let sx1 = (((x + 1) as u64 * w as u64 / tw_u as u64) as i32).max(sx0 + 1);
                let mut sr = 0u32;
                let mut sg = 0u32;
                let mut sb = 0u32;
                let mut sa = 0u32;
                let mut n = 0u32;
                for py in sy0..sy1 {
                    for px in sx0..sx1 {
                        let off = ((py as u32) * stride + px as u32) as usize * 4;
                        sb += *bits.add(off) as u32;
                        sg += *bits.add(off + 1) as u32;
                        sr += *bits.add(off + 2) as u32;
                        sa += *bits.add(off + 3) as u32;
                        n += 1;
                    }
                }
                if n == 0 {
                    n = 1;
                }
                rgba.push((sr / n) as u8);
                rgba.push((sg / n) as u8);
                rgba.push((sb / n) as u8);
                rgba.push((sa / n) as u8);
            }
        }
    }

    let png_bytes = match encode_png_rgba(&rgba, tw_u, th_u) {
        Some(b) => b,
        None => {
            unsafe {
                *out_len = 0;
            }
            return std::ptr::null_mut();
        }
    };

    let len = png_bytes.len() as c_int;
    let ptr = png_bytes.as_ptr() as *mut c_uchar;
    std::mem::forget(png_bytes);
    unsafe {
        *out_len = len;
    }
    ptr
}

/// Free a buffer returned by glaspen2_render_thumbnail.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_free_rust_bytes(ptr: *mut c_uchar, len: c_int) {
    if !ptr.is_null() && len > 0 {
        unsafe {
            let _ = Vec::from_raw_parts(ptr, len as usize, len as usize);
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
