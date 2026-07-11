//! FFI export functions — `#[unsafe(no_mangle)] extern "C"` API callable from ObjC/C#.
//! Extracted from lib.rs to keep the crate root focused on types and modules.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_double, c_int, c_uchar};
use std::path::PathBuf;
use std::slice;

use crate::{
    db, desktop_path, modeler, ocr, pressure_to_width, runtime, state, timestamped_name,
    timestamped_path, db::OcrBox, RAW_STROKE_START, Stroke, STROKES,
};

// ---------------------------------------------------------------------------
// Drawing FFI (legacy, non-modeler path)
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_begin_stroke(
    r: c_double,
    g: c_double,
    b: c_double,
    width_scale: c_double,
) {
    let mut strokes = STROKES.lock().unwrap();
    strokes.push(Stroke {
        r,
        g,
        b,
        points: Vec::new(),
    });
    *RAW_STROKE_START.lock().unwrap() = None;
    runtime().block_on(db::begin_stroke(r, g, b, width_scale));
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

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_clear_strokes(screen_w: c_int, screen_h: c_int) {
    runtime().block_on(db::end_stroke()); // flush before checking — must block
    let current = state::current_screen_id();
    if runtime().block_on(db::screen_has_strokes(current)) {
        runtime().block_on(db::new_screen(screen_w, screen_h));
    }
    let mut strokes = STROKES.lock().unwrap();
    strokes.clear();
}

/// Undo the last stroke: remove from both STROKES (memory) and DB.
/// Returns the number of remaining strokes, or -1 if there was nothing to undo.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_undo_last_stroke() -> c_int {
    let mut strokes = STROKES.lock().unwrap();
    if strokes.is_empty() {
        return -1;
    }
    strokes.pop();
    runtime().block_on(db::delete_last_stroke());
    strokes.len() as c_int
}

/// Initialize the database and create the first screen record. Call once at app start.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_init_db(screen_w: c_int, screen_h: c_int) {
    runtime().block_on(db::init());
    runtime().block_on(db::new_screen(screen_w, screen_h));
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
    runtime().block_on(db::begin_stroke(r, g, b, width_scale));
    state::buffer_point(x, y, pressure_to_width(pressure, width_scale), 0.0); // sync
    // Start STROKES entry
    let mut strokes = STROKES.lock().unwrap();
    strokes.push(Stroke {
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
pub extern "C" fn glaspen2_modeler_commit_to_strokes(
    r: c_double,
    g: c_double,
    b: c_double,
) {
    let smoothed = modeler::take_buffer();
    let mut strokes = STROKES.lock().unwrap();
    if let Some(last) = strokes.last_mut() {
        last.r = r;
        last.g = g;
        last.b = b;

        for (sx, sy, sw, st) in smoothed {
            last.points.push((sx, sy, sw, st));
        }
    }
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
    let data = runtime().block_on(db::strokes_for_screen(screen_id));
    let count = data.len() as c_int;
    let mut strokes = STROKES.lock().unwrap();
    strokes.clear();
    for s in data {
        strokes.push(Stroke {
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
    let mut strokes = STROKES.lock().unwrap();
    for stroke in strokes.iter_mut() {
        let raw: Vec<_> = stroke.points.iter().map(|&(x, y, w, _)| (x, y, w)).collect();
        let smoothed = modeler::smooth_points(&raw);
        if !smoothed.is_empty() {
            stroke.points = smoothed;
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

/// Delete a screen (page) and all its data (strokes, points, OCR).
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
    strokes
        .get(idx as usize)
        .map_or(1.0, |s| s.avg_width())
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_get_stroke_point(
    idx: c_int,
    pidx: c_int,
    x: *mut c_double,
    y: *mut c_double,
) {
    let strokes = STROKES.lock().unwrap();
    if let Some(s) = strokes.get(idx as usize) {
        if let Some(&(px, py, _, _)) = s.points.get(pidx as usize) {
            unsafe {
                *x = px;
                *y = py;
            }
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
    let filename =
        format!("glaspen2_{:04}-{:03}_{:02}-{:02}-{:02}.xoj", y, d, h, m, s);
    desktop.join(filename)
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_save_xoj() {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    let strokes = STROKES.lock().unwrap();

    // Get screen dimensions from the first point bounds, or use defaults
    let (mut max_x, mut max_y) = (1920.0f64, 1080.0f64);
    for stroke in strokes.iter() {
        for &(x, y, _, _) in &stroke.points {
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

    // Build XML
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" standalone=\"no\"?>\n");
    xml.push_str(&format!(
        "<xournal version=\"0.4\" fileversion=\"4\">\n"
    ));
    xml.push_str(&format!(
        "  <page width=\"{}\" height=\"{}\">\n",
        page_w, page_h
    ));
    xml.push_str("    <layer>\n");

    for stroke in strokes.iter() {
        let color_hex = format!(
            "#{:02x}{:02x}{:02x}",
            (stroke.r * 255.0) as u8,
            (stroke.g * 255.0) as u8,
            (stroke.b * 255.0) as u8
        );

        let widths: String = stroke
            .points
            .iter()
            .map(|&(_, _, w, _)| format!("{:.2}", w))
            .collect::<Vec<_>>()
            .join(" ");

        let coords: String = stroke
            .points
            .iter()
            .map(|&(x, y, _, _)| format!("{:.2} {:.2}", x, y))
            .collect::<Vec<_>>()
            .join(" ");

        xml.push_str(&format!(
            "      <stroke color=\"{}\" tool=\"pen\" width=\"{}\">\n        {}\n      </stroke>\n",
            color_hex, widths, coords
        ));
    }

    xml.push_str("    </layer>\n");
    xml.push_str("  </page>\n");
    xml.push_str("</xournal>\n");

    // Gzip compress
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(xml.as_bytes()).unwrap();
    let compressed = encoder.finish().unwrap();

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
    let k = unsafe { CStr::from_ptr(key) }
        .to_str()
        .unwrap_or("");
    runtime().block_on(db::save_setting(k, if val != 0 { "1" } else { "0" }));
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_load_bool_setting(key: *const c_char) -> c_int {
    let k = unsafe { CStr::from_ptr(key) }
        .to_str()
        .unwrap_or("");
    runtime()
        .block_on(db::load_setting(k))
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0)
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
        .unwrap_or_else(|_| {
            "/Applications/glaspen2.app/Contents/MacOS/glaspen2".to_string()
        })
}

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
        if launch_agent_plist().exists() {
            1
        } else {
            0
        }
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
        Ok(_) => println!(
            "[glaspen2] Saved (with background) to {}",
            path.display()
        ),
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
    let strokes = STROKES.lock().unwrap();
    if strokes.is_empty() {
        return None;
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
    for s in strokes.iter() {
        if s.points.is_empty() {
            continue;
        }
        let color_hex = format!(
            "#{:02x}{:02x}{:02x}",
            (s.r * 255.0) as u8,
            (s.g * 255.0) as u8,
            (s.b * 255.0) as u8
        );
        for i in 0..s.points.len() {
            let (x, y, w, _t) = s.points[i];
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
                let (prev_x, prev_y, _, _) = s.points[i - 1];
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
    let w = surface_w as u32;
    let h = surface_h as u32;
    let scale = surface_scale.max(0.5).min(4.0);
    let stride = surface_stride as usize;
    let raw = unsafe { slice::from_raw_parts(surface_data, stride * h as usize) };
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
    let mut flat: Vec<u8> = Vec::with_capacity((crop_w * crop_h * 4) as usize);
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
    // Downscale to 50% for smaller GIF (clamp to minimum 1 pixel)
    let gif_w = (crop_w / 2).max(1);
    let gif_h = (crop_h / 2).max(1);
    let mut gif_pixels: Vec<u8> = Vec::with_capacity((gif_w * gif_h * 4) as usize);
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

    let quantizer = color_quant::NeuQuant::new(30, 128, &gif_pixels);
    let indices: Vec<u8> = gif_pixels
        .chunks(4)
        .map(|p| quantizer.index_of(&[p[0], p[1], p[2], p[3]]) as u8)
        .collect();
    let mut idx_counts = [0u32; 128];
    for (i, &idx) in indices.iter().enumerate() {
        if gif_pixels[i * 4 + 3] == 0 {
            idx_counts[idx as usize] += 1;
        }
    }
    let mut transparent_idx: u8 = 0;
    let mut max_count = 0u32;
    for i in 0..128 {
        if idx_counts[i] > max_count {
            max_count = idx_counts[i];
            transparent_idx = i as u8;
        }
    }
    let palette = quantizer.color_map_rgba();
    let gif_palette: Vec<u8> = (0..128)
        .flat_map(|i| {
            [palette[i * 4], palette[i * 4 + 1], palette[i * 4 + 2]]
        })
        .collect();
    let mut gif_data = Vec::new();
    {
        let mut enc =
            gif::Encoder::new(&mut gif_data, gif_w as u16, gif_h as u16, &gif_palette)
                .unwrap();
        let frame = gif::Frame {
            width: gif_w as u16,
            height: gif_h as u16,
            buffer: std::borrow::Cow::Owned(indices),
            transparent: Some(transparent_idx),
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

/// Save an animated GIF showing stroke drawing order at real speed.
/// Uses the timing data stored in each point (relative_time).
/// Frames are rendered in parallel across available CPU cores.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_save_animated_gif() -> c_int {
    // ── Phase 1: lock, extract, drop ──
    let (gif_strokes, bw, bh, gif_w, gif_h, bx_min, by_min) = {
        let strokes = STROKES.lock().unwrap();
        if strokes.is_empty() {
            return 0;
        }

        // Bounding box
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
        let pad = 10.0;
        bx_min -= pad;
        by_min -= pad;
        bx_max += pad;
        by_max += pad;
        let bw = (bx_max - bx_min).ceil() as i32;
        let bh = (by_max - by_min).ceil() as i32;
        if bw < 4 || bh < 4 {
            return 0;
        }
        let gif_w = ((bw as u32) / 2).max(1) as u16;
        let gif_h = ((bh as u32) / 2).max(1) as u16;

        // Clone stroke data so lock can be dropped
        let gif_strokes: Vec<GifStroke> = strokes
            .iter()
            .map(|s| GifStroke {
                r: s.r,
                g: s.g,
                b: s.b,
                points: s.points.clone(),
            })
            .collect();
        // lock drops here
        (gif_strokes, bw, bh, gif_w, gif_h, bx_min, by_min)
    };

    // ── Phase 2: compressed timeline (no lock needed) ──
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
        return 0;
    }

    const SPEED: f64 = 2.0;
    const MIN_SEG: f64 = 0.05;
    let total_active: f64 = segments
        .iter()
        .map(|seg| (seg.dur / SPEED).max(MIN_SEG))
        .sum();
    if total_active < 0.01 {
        return 0;
    }

    let seg_offset: Vec<(usize, f64, f64)> = {
        let mut v = Vec::new();
        let mut cur = 0.0;
        for seg in &segments {
            let adj = (seg.dur / SPEED).max(MIN_SEG);
            v.push((seg.si, cur, cur + adj));
            cur += adj;
        }
        v
    };

    const N_DRAW: usize = 60;
    const N_HOLD: usize = 5;
    let n_frames = N_DRAW + N_HOLD;
    let draw_delay =
        ((total_active.min(5.0).max(0.5) / N_DRAW as f64) * 100.0).max(2.0) as u16;

    // ── Phase 3: parallel frame rendering (rayon global thread pool) ──
    use rayon::prelude::*;

    let n_threads = rayon::current_num_threads();
    eprintln!(
        "[glaspen2] animated GIF: rayon threads={}, n_frames={}",
        n_threads, n_frames
    );

    let mut frame_results: Vec<(usize, Vec<u8>, u16)> = (0..n_frames)
        .into_par_iter()
        .map(|fi| {
            let is_hold = fi >= N_DRAW;
            let cutoff =
                (fi.min(N_DRAW - 1) as f64 / N_DRAW as f64) * total_active;
            let delay = if is_hold { 100u16 } else { draw_delay };

            let (flat, _ok) = render_gif_frame(
                &gif_strokes, &seg_offset, bw, bh, bx_min, by_min, gif_w, gif_h, fi,
                is_hold, cutoff, delay,
            );
            (fi, flat, delay)
        })
        .collect();

    frame_results.sort_by_key(|&(fi, _, _)| fi);
    let frame_pixels: Vec<(Vec<u8>, u16)> =
        frame_results.into_iter().map(|(_, px, d)| (px, d)).collect();

    // ── Phase 4: palette ──
    let all_pixels: Vec<u8> = frame_pixels
        .iter()
        .flat_map(|(px, _)| px.iter())
        .copied()
        .collect();
    if all_pixels.is_empty() {
        return 0;
    }
    let quantizer = color_quant::NeuQuant::new(30, 64, &all_pixels);
    let palette = quantizer.color_map_rgba();
    let gif_palette: Vec<u8> = (0..64)
        .flat_map(|i| {
            [palette[i * 4], palette[i * 4 + 1], palette[i * 4 + 2]]
        })
        .collect();

    // Transparent index
    let transparent_idx = {
        let mut idx_counts = [0u32; 64];
        for (px, _) in &frame_pixels {
            for ch in px.chunks(4) {
                if ch.len() == 4 && ch[3] == 0 {
                    let idx =
                        quantizer.index_of(&[ch[0], ch[1], ch[2], 0]) as u8;
                    idx_counts[idx as usize] += 1;
                }
            }
        }
        let mut best = 0u8;
        let mut max_count = 0u32;
        for i in 0..64 {
            if idx_counts[i] > max_count {
                max_count = idx_counts[i];
                best = i as u8;
            }
        }
        best
    };

    // ── Phase 5: encode GIF ──
    let mut gif_data = Vec::new();
    {
        let mut enc = match gif::Encoder::new(
            &mut gif_data,
            gif_w,
            gif_h,
            &gif_palette,
        ) {
            Ok(e) => e,
            Err(_) => return 0,
        };
        enc.set_repeat(gif::Repeat::Infinite).ok();

        for (pixels, delay) in &frame_pixels {
            let indices: Vec<u8> = pixels
                .chunks(4)
                .map(|p| quantizer.index_of(&[p[0], p[1], p[2], 0]) as u8)
                .collect();

            let frame = gif::Frame {
                width: gif_w,
                height: gif_h,
                buffer: std::borrow::Cow::Owned(indices),
                delay: *delay,
                transparent: Some(transparent_idx),
                ..gif::Frame::default()
            };
            if enc.write_frame(&frame).is_err() {
                return 0;
            }
        }
    }

    let path = desktop_path().join(timestamped_name("gif"));
    match std::fs::write(&path, &gif_data) {
        Ok(_) => {
            println!(
                "[glaspen2] Saved animated GIF to {} ({} frames, {} threads)",
                path.display(),
                n_frames,
                n_threads
            );
            1
        }
        Err(e) => {
            eprintln!("[glaspen2] Animated GIF write failed: {}", e);
            0
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
    let mut surface =
        match crate::cairo::ImageSurface::create(crate::cairo::Format::ARgb32, bw, bh)
        {
            Ok(s) => s,
            Err(_) => return (Vec::new(), false),
        };
    let stride = surface.stride() as usize;
    let rw = surface.width() as u32;
    let rh = surface.height() as u32;

    // Clear
    if let Ok(cr) = crate::cairo::Context::new(&surface) {
        cr.set_operator(crate::cairo::Operator::Clear);
        let _ = cr.paint();
    }

    // Render strokes
    {
        let cr = match crate::cairo::Context::new(&surface) {
            Ok(c) => c,
            Err(_) => return (Vec::new(), false),
        };
        for &(si, seg_start, seg_end) in seg_offset {
            let s = &strokes[si];

            let pts: Vec<(f64, f64, f64)> = if is_hold || cutoff >= seg_end {
                s.points
                    .iter()
                    .map(|&(x, y, w, _)| (x - bx_min, y - by_min, w))
                    .collect()
            } else if cutoff > seg_start {
                let local_frac = (cutoff - seg_start) / (seg_end - seg_start);
                let local_cut = s.points[0].3
                    + local_frac
                        * (s.points[s.points.len() - 1].3 - s.points[0].3);
                s.points
                    .iter()
                    .take_while(|&&(_, _, _, t)| t <= local_cut)
                    .map(|&(x, y, w, _)| (x - bx_min, y - by_min, w))
                    .collect()
            } else {
                Vec::new()
            };
            if pts.is_empty() {
                continue;
            }

            cr.set_source_rgba(s.r, s.g, s.b, 1.0);
            for i in 0..pts.len() {
                let (cx, cy, w) = pts[i];
                if i == 0 {
                    cr.arc(cx, cy, w * 0.5, 0.0, 2.0 * std::f64::consts::PI);
                    let _ = cr.fill();
                } else {
                    let (px, py, _) = pts[i - 1];
                    cr.move_to(px, py);
                    cr.line_to(cx, cy);
                    cr.set_line_width(w);
                    cr.set_line_cap(crate::cairo::LineCap::Round);
                    cr.set_line_join(crate::cairo::LineJoin::Round);
                    let _ = cr.stroke();
                }
            }
        }
    }

    // Read pixels
    let pix = &*surface.data().unwrap_or_else(|_| panic!("Surface data"));
    let mut flat = Vec::with_capacity((gif_w as u32 * gif_h as u32 * 4) as usize);
    for gy in 0..gif_h as u32 {
        for gx in 0..gif_w as u32 {
            let sx = (gx * 2).min(rw.saturating_sub(1));
            let sy = (gy * 2).min(rh.saturating_sub(1));
            let off = sy as usize * stride + sx as usize * 4;
            if off + 3 < pix.len() {
                flat.push(pix[off + 2]); // R
                flat.push(pix[off + 1]); // G
                flat.push(pix[off]); // B
                flat.push(pix[off + 3]); // A
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
pub extern "C" fn glaspen2_get_stroke_point_time(
    idx: c_int,
    pidx: c_int,
) -> c_double {
    let strokes = STROKES.lock().unwrap();
    strokes
        .get(idx as usize)
        .and_then(|s| s.points.get(pidx as usize))
        .map_or(0.0, |p| p.3)
}

/// Void undo — Windows tray menu calls this (returns nothing).
/// The macOS equivalent glaspen2_undo_last_stroke returns remaining count.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_delete_last_stroke() {
    runtime().block_on(db::delete_last_stroke());
    STROKES.lock().unwrap().pop();
}

// ---------------------------------------------------------------------------
// OCR
// ---------------------------------------------------------------------------

/// Run OCR on an RGBA pixel buffer. Returns a C string that the caller must
/// free with glaspen2_free_c_string.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_ocr_recognize(
    pixels: *const c_uchar,
    width: c_int,
    height: c_int,
) -> *mut c_char {
    if pixels.is_null() || width <= 0 || height <= 0 {
        return std::ptr::null_mut();
    }
    let w = width as u32;
    let h = height as u32;
    let len = (w * h * 4) as usize;
    let pixel_slice = unsafe { std::slice::from_raw_parts(pixels, len) };
    let text = ocr::detect_and_recognize(pixel_slice, w, h);
    match CString::new(text) {
        Ok(cs) => cs.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Run OCR on the current surface and save results to DB for this page.
/// Returns the recognized text as a C string (caller must free with
/// glaspen2_free_c_string), or NULL if no text found.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_ocr_page(
    pixels: *const c_uchar,
    width: c_int,
    height: c_int,
    screen_id: i64,
) -> *mut c_char {
    if pixels.is_null() || width <= 0 || height <= 0 {
        return std::ptr::null_mut();
    }
    let w = width as u32;
    let h = height as u32;
    let len = (w * h * 4) as usize;
    let pixel_slice = unsafe { std::slice::from_raw_parts(pixels, len) };

    // Run detection + recognition
    let boxes = ocr::det::detect_text_regions(pixel_slice, w, h);

    // Recognize each box, build OcrBox entries
    let mut ocr_boxes: Vec<OcrBox> = Vec::new();
    let mut full_text = String::new();
    for (i, tb) in boxes.iter().enumerate() {
        let pad = 4u32;
        let cx = tb.x.saturating_sub(pad);
        let cy = tb.y.saturating_sub(pad);
        let cw = (tb.w + pad * 2).min(w - cx);
        let ch = (tb.h + pad * 2).min(h - cy);
        if cw < 4 || ch < 4 { continue; }

        let crop = ocr::det::crop_pixels(pixel_slice, w, cx, cy, cw, ch);
        let text = ocr::rec::recognize(&crop, cw, ch);
        if !text.is_empty() {
            if i > 0 { full_text.push('\n'); }
            full_text.push_str(&text);

            // Per-char positions (estimate by dividing box width evenly)
            let chars: Vec<char> = text.chars().collect();
            if chars.len() > 0 {
                let char_w = tb.w as f64 / chars.len() as f64;
                for (ci, ch) in chars.iter().enumerate() {
                    ocr_boxes.push(OcrBox {
                        text: ch.to_string(),
                        x: tb.x as f64 + char_w * ci as f64,
                        y: tb.y as f64,
                        w: char_w,
                        h: tb.h as f64,
                        confidence: 0.0, // per-char confidence not available
                    });
                }
            }
        }
    }

    // Save to DB
    runtime().block_on(db::save_ocr_result(screen_id, &full_text, &ocr_boxes));

    match CString::new(full_text) {
        Ok(cs) => cs.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
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

/// Backfill OCR data for all pages that don't have it yet.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_ocr_backfill_all() {
    crate::pdf::backfill_ocr_all_pages();
}

// ---------------------------------------------------------------------------
// Content tab data (page listing, OCR, search)
// ---------------------------------------------------------------------------

/// List all screens with their OCR text as JSON.
/// Returns a C string (caller must free via glaspen2_free_c_string).
/// JSON: [{"id":1,"w":1920,"h":1080,"ocr":"text or null"}, ...]
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_list_screens_json() -> *mut c_char {
    let rows = runtime().block_on(db::list_screens_with_ocr());
    let list: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|(id, w, h, ocr)| {
            serde_json::json!({
                "id": id,
                "w": w,
                "h": h,
                "ocr": ocr,
            })
        })
        .collect();
    let json = serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string());
    CString::new(json).unwrap_or_default().into_raw()
}

/// Search OCR text across all pages.
/// Returns JSON array of matching screens with OCR text.
/// JSON: [{"id":1,"w":1920,"h":1080,"ocr":"full text"}, ...]
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_search_ocr_json(query: *const c_char) -> *mut c_char {
    let Ok(q) = unsafe { CStr::from_ptr(query) }.to_str() else {
        return CString::new("[]".to_string()).unwrap_or_default().into_raw();
    };
    if q.is_empty() {
        return glaspen2_list_screens_json();
    }
    let rows = runtime().block_on(db::search_ocr(q));
    let list: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|(id, w, h, ocr)| {
            serde_json::json!({
                "id": id,
                "w": w,
                "h": h,
                "ocr": ocr,
            })
        })
        .collect();
    let json = serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string());
    CString::new(json).unwrap_or_default().into_raw()
}

// ---------------------------------------------------------------------------
// Thumbnail rendering (pure Rust, full-resolution → scaled PNG)
// ---------------------------------------------------------------------------

/// Render a page thumbnail entirely in Rust.
#[cfg(feature = "cairo_real")]
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_render_thumbnail(
    screen_id: i64,
    w: c_int,
    h: c_int,
    max_size: c_int,
    out_len: *mut c_int,
) -> *mut c_uchar {
    eprintln!("[thumb] CALLED screen={} w={} h={} max={}", screen_id, w, h, max_size);
    if w <= 0 || h <= 0 || max_size <= 0 || out_len.is_null() {
        eprintln!("[thumb] EARLY EXIT: invalid params");
        if !out_len.is_null() { unsafe { *out_len = 0; } }
        return std::ptr::null_mut();
    }

    let scale = if w >= h {
        max_size as f64 / w as f64
    } else {
        max_size as f64 / h as f64
    };
    let tw = ((w as f64) * scale).max(1.0) as i32;
    let th = ((h as f64) * scale).max(1.0) as i32;

    let strokes = runtime().block_on(db::strokes_for_screen(screen_id));
    if strokes.is_empty() {
        unsafe { *out_len = 0; }
        return std::ptr::null_mut();
    }
    {
        let mut dst = STROKES.lock().unwrap();
        dst.clear();
        dst.extend(strokes.into_iter().map(|s| Stroke {
            r: s.r, g: s.g, b: s.b, points: s.points,
        }));
    }

    // Clamp stroke widths to minimum visible (≥1px at thumbnail scale)
    let min_w = 1.0 / scale;
    {
        let mut dst2 = STROKES.lock().unwrap();
        for s in dst2.iter_mut() {
            for p in s.points.iter_mut() {
                if p.2 < min_w { p.2 = min_w; }
            }
        }
    }

    // Render directly at thumbnail scale
    let mut thumb = match crate::cairo::ImageSurface::create(
        crate::cairo::Format::ARgb32, tw, th,
    ) {
        Ok(s) => s,
        Err(_) => { unsafe { *out_len = 0; } return std::ptr::null_mut(); }
    };
    crate::draw::draw_rebuild_on_surface(&thumb, scale);

    let stride = thumb.stride() as usize;
    let Ok(data) = thumb.data() else {
        unsafe { *out_len = 0; }
        return std::ptr::null_mut();
    };
    let (tw_u, th_u) = (tw as u32, th as u32);
    let mut rgba = Vec::with_capacity((tw_u * th_u * 4) as usize);
    for y in 0..th_u {
        for x in 0..tw_u {
            let off = y as usize * stride + x as usize * 4;
            let (b, g, r, a) = (data[off], data[off + 1], data[off + 2], data[off + 3]);
            rgba.push(r); rgba.push(g); rgba.push(b); rgba.push(a);
        }
    }
    drop(data);

    let png_bytes = match encode_png_rgba(&rgba, tw_u, th_u) {
        Some(b) => b,
        None => { unsafe { *out_len = 0; } return std::ptr::null_mut(); }
    };

    // Drop the STROKES we loaded (don't pollute global state)
    STROKES.lock().unwrap().clear();

    let len = png_bytes.len() as c_int;
    let ptr = png_bytes.as_ptr() as *mut c_uchar;
    std::mem::forget(png_bytes);
    unsafe { *out_len = len; }
    // Debug: write PNG to temp file
    if len > 0 {
        let buf = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
        let _ = std::fs::write("/tmp/glaspen2_thumb.png", buf);
    }
    eprintln!("[thumb] screen={} dim={}x{} scale={:.4} thumb={}x{} strokes={} png={}B",
              screen_id, w, h, scale, tw, th, "ok", len);
    ptr
}

#[cfg(not(feature = "cairo_real"))]
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_render_thumbnail(
    _screen_id: i64, _w: c_int, _h: c_int, _max_size: c_int,
    out_len: *mut c_int,
) -> *mut c_uchar {
    if !out_len.is_null() { unsafe { *out_len = 0; } }
    std::ptr::null_mut()
}

/// Free a buffer returned by glaspen2_render_thumbnail.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_free_rust_bytes(ptr: *mut c_uchar, len: c_int) {
    if !ptr.is_null() && len > 0 {
        unsafe { let _ = Vec::from_raw_parts(ptr, len as usize, len as usize); }
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
