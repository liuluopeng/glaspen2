#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::path::PathBuf;
use std::slice;
use std::os::raw::{c_int, c_double, c_uchar, c_char};
use std::ffi::CStr;
use std::sync::Mutex;

// Cairo: real crate when available, stub for cross-compilation / Windows
#[cfg(all(feature = "cairo_real", not(target_os = "windows")))]
extern crate cairo;
#[cfg(any(not(feature = "cairo_real"), target_os = "windows"))]
#[path = "cairo_stub.rs"]
pub mod cairo;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

pub mod db;
pub mod modeler;

// --- Stroke recording for Xournal export ---

pub struct Stroke {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub points: Vec<(f64, f64, f64)>, // (x, y, width)
    pub point_colors: Option<Vec<(f64, f64, f64)>>, // per-point color for inverse mode (memory only)
}

impl Stroke {
    pub fn avg_width(&self) -> f64 {
        if self.points.is_empty() { return 1.0; }
        self.points.iter().map(|p| p.2).sum::<f64>() / self.points.len() as f64
    }
}

pub static STROKES: Mutex<Vec<Stroke>> = Mutex::new(Vec::new());

#[no_mangle]
pub extern "C" fn glaspen2_begin_stroke(r: c_double, g: c_double, b: c_double, width_scale: c_double) {
    let mut strokes = STROKES.lock().unwrap();
    strokes.push(Stroke { r, g, b, points: Vec::new(), point_colors: None });
    db::begin_stroke(r, g, b, width_scale);
}

#[no_mangle]
pub extern "C" fn glaspen2_add_point(x: c_double, y: c_double, width: c_double) {
    let mut strokes = STROKES.lock().unwrap();
    if let Some(stroke) = strokes.last_mut() {
        stroke.points.push((x, y, width));
    }
    db::add_point(x, y, width);
}

#[no_mangle]
pub extern "C" fn glaspen2_end_stroke() {
    db::end_stroke();
}

#[no_mangle]
pub extern "C" fn glaspen2_clear_strokes(screen_w: c_int, screen_h: c_int) {
    db::end_stroke(); // flush pending before checking
    let current = db::current_screen();
    if db::screen_has_strokes(current) {
        db::new_screen(screen_w, screen_h);
    }
    let mut strokes = STROKES.lock().unwrap();
    strokes.clear();
}

/// Initialize the database and create the first screen record. Call once at app start.
#[no_mangle]
pub extern "C" fn glaspen2_init_db(screen_w: c_int, screen_h: c_int) {
    db::init();
    db::new_screen(screen_w, screen_h);
}

// --- Modeler FFI ---

#[no_mangle]
pub extern "C" fn glaspen2_modeler_begin(r: c_double, g: c_double, b: c_double, x: c_double, y: c_double, pressure: c_double, timestamp: c_double, width_scale: c_double) {
    modeler::begin_stroke(x, y, pressure, timestamp, width_scale);
    // Start DB stroke with correct color
    db::begin_stroke(r, g, b, width_scale);
    db::add_point(x, y, pressure_to_width(pressure, width_scale));
    // Start STROKES entry
    let mut strokes = STROKES.lock().unwrap();
    strokes.push(Stroke { r, g, b, points: Vec::new(), point_colors: None });
}

#[no_mangle]
pub extern "C" fn glaspen2_modeler_move(x: c_double, y: c_double, pressure: c_double, timestamp: c_double, width_scale: c_double) {
    modeler::pen_move(x, y, pressure, timestamp, width_scale);
    db::add_point(x, y, pressure_to_width(pressure, width_scale));
}

#[no_mangle]
pub extern "C" fn glaspen2_modeler_end(x: c_double, y: c_double, pressure: c_double, timestamp: c_double, width_scale: c_double) {
    modeler::end_stroke(x, y, pressure, timestamp, width_scale);
    db::add_point(x, y, pressure_to_width(pressure, width_scale));
    db::end_stroke();
}

/// Commit the modeler buffer into STROKES. Call after drawing the buffer.
/// If inv_colors is non-null and inv_count > 0, uses per-point inverse colors.
/// inv_colors is a flat array: [r0,g0,b0, r1,g1,b1, ...] — one per modeler output point.
/// DB stores the original (r,g,b) color; point_colors is memory-only for rendering.
#[no_mangle]
pub extern "C" fn glaspen2_modeler_commit_to_strokes(
    r: c_double, g: c_double, b: c_double,
    inv_colors: *const c_double, inv_count: c_int,
) {
    let smoothed = modeler::take_buffer();
    let n_smoothed = smoothed.len();
    let mut strokes = STROKES.lock().unwrap();
    if let Some(last) = strokes.last_mut() {
        last.r = r;
        last.g = g;
        last.b = b;

        // Per-point inverse colors (1:1 with modeler output)
        let point_colors = if !inv_colors.is_null() && inv_count > 0 {
            let inv = unsafe { std::slice::from_raw_parts(inv_colors, (inv_count * 3) as usize) };
            let n = n_smoothed.min(inv_count as usize);
            let mut colors = Vec::with_capacity(n);
            for i in 0..n {
                let ci = i * 3;
                colors.push((inv[ci], inv[ci + 1], inv[ci + 2]));
            }
            Some(colors)
        } else {
            None
        };

        for (sx, sy, sw) in smoothed {
            last.points.push((sx, sy, sw));
        }
        last.point_colors = point_colors;
    }
}

/// Get the number of smoothed points available after the last modeler call.
#[no_mangle]
pub extern "C" fn glaspen2_modeler_point_count() -> c_int {
    modeler::buffer_len() as c_int
}

/// Get a smoothed point by index (for macOS ObjC to read back).
#[no_mangle]
pub extern "C" fn glaspen2_modeler_get_point(idx: c_int, x: *mut c_double, y: *mut c_double, w: *mut c_double) {
    if let Some((px, py, pw)) = modeler::get_buffer_point(idx as usize) {
        unsafe { *x = px; *y = py; *w = pw; }
    }
}

/// Clear the modeler buffer (call after platform has read and drawn all points).
#[no_mangle]
pub extern "C" fn glaspen2_modeler_clear_buffer() {
    modeler::clear_buffer();
}

fn pressure_to_width(pressure: f64, width_scale: f64) -> f64 {
    if pressure > 0.01 {
        (0.3 + pressure * pressure * 7.7) * width_scale
    } else {
        1.0 * width_scale
    }
}

// --- Page navigation ---

/// Load strokes from DB into STROKES for a given screen. Returns stroke count.
#[no_mangle]
pub extern "C" fn glaspen2_load_strokes_for_screen(screen_id: i64) -> c_int {
    let data = db::strokes_for_screen(screen_id);
    let count = data.len() as c_int;
    let mut strokes = STROKES.lock().unwrap();
    strokes.clear();
    for s in data {
        strokes.push(Stroke { r: s.r, g: s.g, b: s.b, points: s.points, point_colors: None });
    }
    // Update current screen in DB
    db::set_current_screen(screen_id);
    count
}

/// Smooth all loaded strokes in STROKES through the modeler. Call after loading.
#[no_mangle]
pub extern "C" fn glaspen2_smooth_loaded_strokes() {
    let mut strokes = STROKES.lock().unwrap();
    for stroke in strokes.iter_mut() {
        let smoothed = modeler::smooth_points(&stroke.points);
        if !smoothed.is_empty() {
            stroke.points = smoothed;
        }
    }
}

#[no_mangle]
pub extern "C" fn glaspen2_prev_screen_id() -> i64 {
    db::prev_screen(db::current_screen()).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn glaspen2_next_screen_id() -> i64 {
    db::next_screen(db::current_screen()).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn glaspen2_get_current_screen_id() -> i64 {
    db::current_screen()
}

#[no_mangle]
pub extern "C" fn glaspen2_stroke_count() -> c_int {
    STROKES.lock().unwrap().len() as c_int
}

#[no_mangle]
pub extern "C" fn glaspen2_get_stroke_point_count(idx: c_int) -> c_int {
    let strokes = STROKES.lock().unwrap();
    strokes.get(idx as usize).map_or(0, |s| s.points.len() as c_int)
}

#[no_mangle]
pub extern "C" fn glaspen2_get_stroke_color(idx: c_int, r: *mut c_double, g: *mut c_double, b: *mut c_double) {
    let strokes = STROKES.lock().unwrap();
    if let Some(s) = strokes.get(idx as usize) {
        unsafe { *r = s.r; *g = s.g; *b = s.b; }
    }
}

#[no_mangle]
pub extern "C" fn glaspen2_get_stroke_avg_width(idx: c_int) -> c_double {
    let strokes = STROKES.lock().unwrap();
    strokes.get(idx as usize).map_or(1.0, |s| s.avg_width())
}

#[no_mangle]
pub extern "C" fn glaspen2_get_stroke_point(idx: c_int, pidx: c_int, x: *mut c_double, y: *mut c_double) {
    let strokes = STROKES.lock().unwrap();
    if let Some(s) = strokes.get(idx as usize) {
        if let Some(&(px, py, _)) = s.points.get(pidx as usize) {
            unsafe { *x = px; *y = py; }
        }
    }
}

#[no_mangle]
pub extern "C" fn glaspen2_get_stroke_point_width(idx: c_int, pidx: c_int) -> c_double {
    let strokes = STROKES.lock().unwrap();
    strokes.get(idx as usize)
        .and_then(|s| s.points.get(pidx as usize))
        .map_or(1.0, |p| p.2)
}

/// Get per-point color for inverse mode. Returns 1 if available, 0 otherwise.
#[no_mangle]
pub extern "C" fn glaspen2_get_stroke_point_color(idx: c_int, pidx: c_int, r: *mut c_double, g: *mut c_double, b: *mut c_double) -> c_int {
    let strokes = STROKES.lock().unwrap();
    if let Some(s) = strokes.get(idx as usize) {
        if let Some(ref colors) = s.point_colors {
            if let Some(&(cr, cg, cb)) = colors.get(pidx as usize) {
                unsafe { *r = cr; *g = cg; *b = cb; }
                return 1;
            }
        }
    }
    0
}



#[no_mangle]
pub extern "C" fn glaspen2_save_xoj() {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    let strokes = STROKES.lock().unwrap();

    // Get screen dimensions from the first point bounds, or use defaults
    let (mut max_x, mut max_y) = (1920.0f64, 1080.0f64);
    for stroke in strokes.iter() {
        for &(x, y, _) in &stroke.points {
            if x > max_x { max_x = x; }
            if y > max_y { max_y = y; }
        }
    }
    let page_w = (max_x + 10.0).ceil() as i32;
    let page_h = (max_y + 10.0).ceil() as i32;

    // Build XML
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" standalone=\"no\"?>\n");
    xml.push_str(&format!("<xournal version=\"0.4\" fileversion=\"4\">\n"));
    xml.push_str(&format!("  <page width=\"{}\" height=\"{}\">\n", page_w, page_h));
    xml.push_str("    <layer>\n");

    for stroke in strokes.iter() {
        let color_hex = format!("#{:02x}{:02x}{:02x}",
            (stroke.r * 255.0) as u8,
            (stroke.g * 255.0) as u8,
            (stroke.b * 255.0) as u8);

        let widths: String = stroke.points.iter()
            .map(|&(_, _, w)| format!("{:.2}", w))
            .collect::<Vec<_>>()
            .join(" ");

        let coords: String = stroke.points.iter()
            .map(|&(x, y, _)| format!("{:.2} {:.2}", x, y))
            .collect::<Vec<_>>()
            .join(" ");

        xml.push_str(&format!(
            "      <stroke color=\"{}\" tool=\"pen\" width=\"{}\">\n        {}\n      </stroke>\n",
            color_hex, widths, coords));
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

fn desktop_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string()))
            .join("Desktop")
    }
    #[cfg(not(target_os = "windows"))]
    {
        PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
            .join("Desktop")
    }
}

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

#[no_mangle]
pub extern "C" fn glaspen2_save_settings(r: c_double, g: c_double, b: c_double, width_scale: c_double) {
    db::save_settings(r, g, b, width_scale);
}

#[no_mangle]
pub extern "C" fn glaspen2_load_settings_parts(r: *mut c_double, g: *mut c_double, b: *mut c_double, w: *mut c_double) -> c_int {
    match db::load_settings() {
        Some((rr, gg, bb, ww)) => {
            unsafe { *r = rr; *g = gg; *b = bb; *w = ww; }
            1
        }
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn glaspen2_save_bool_setting(key: *const c_char, val: c_int) {
    let k = unsafe { CStr::from_ptr(key) }.to_str().unwrap_or("");
    db::save_setting(k, if val != 0 { "1" } else { "0" });
}

#[no_mangle]
pub extern "C" fn glaspen2_load_bool_setting(key: *const c_char) -> c_int {
    let k = unsafe { CStr::from_ptr(key) }.to_str().unwrap_or("");
    db::load_setting(k).and_then(|v| v.parse::<i32>().ok()).unwrap_or(0)
}

// --- Launch at login (macOS LaunchAgent) ---

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

#[no_mangle]
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
                 \t<string>{}</string>\n\
                 \t<key>RunAtLoad</key>\n\
                 \t<true/>\n\
                 </dict>\n\
                 </plist>\n",
                xml_escape(&program)
            );
            match std::fs::write(&plist_path, &plist) {
                Ok(_) => 1,
                Err(e) => { eprintln!("[glaspen2] launch agent write failed: {}", e); 0 }
            }
        } else {
            match std::fs::remove_file(&plist_path) {
                Ok(_) => 1,
                Err(e) => { eprintln!("[glaspen2] launch agent remove failed: {}", e); 0 }
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    { 0 }
}

#[no_mangle]
pub extern "C" fn glaspen2_is_launch_at_login() -> c_int {
    #[cfg(target_os = "macos")]
    { if launch_agent_plist().exists() { 1 } else { 0 } }
    #[cfg(not(target_os = "macos"))]
    { 0 }
}

#[cfg(target_os = "macos")]
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
     .replace('\'', "&apos;")
}

/// Save drawing only (transparent background)
#[no_mangle]
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
#[no_mangle]
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
        Ok(_) => println!("[glaspen2] Saved (with background) to {}", path.display()),
        Err(e) => eprintln!("[glaspen2] Save failed: {}", e),
    }
}

fn timestamped_path() -> PathBuf {
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
    let filename = format!("glaspen2_{:04}-{:03}_{:02}-{:02}-{:02}.png", y, d, h, m, s);
    desktop.join(filename)
}

/// Compute bounding box of all strokes. Returns 1 if there are strokes, 0 otherwise.
#[no_mangle]
pub extern "C" fn glaspen2_stroke_bbox(
    x_min: *mut c_double, y_min: *mut c_double,
    x_max: *mut c_double, y_max: *mut c_double,
) -> c_int {
    let strokes = STROKES.lock().unwrap();
    if strokes.is_empty() { return 0; }
    let mut bx_min = f64::MAX;
    let mut by_min = f64::MAX;
    let mut bx_max = f64::MIN;
    let mut by_max = f64::MIN;
    for s in strokes.iter() {
        for &(x, y, _) in &s.points {
            if x < bx_min { bx_min = x; }
            if y < by_min { by_min = y; }
            if x > bx_max { bx_max = x; }
            if y > by_max { by_max = y; }
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

/// Save strokes as SVG to desktop (cropped to bbox).
#[no_mangle]
pub extern "C" fn glaspen2_save_svg() {
    let strokes = STROKES.lock().unwrap();
    if strokes.is_empty() { return; }
    let mut bx_min = f64::MAX; let mut by_min = f64::MAX;
    let mut bx_max = f64::MIN; let mut by_max = f64::MIN;
    for s in strokes.iter() {
        for &(x, y, _) in &s.points {
            if x < bx_min { bx_min = x; }
            if y < by_min { by_min = y; }
            if x > bx_max { bx_max = x; }
            if y > by_max { by_max = y; }
        }
    }
    let pad = 10.0;
    bx_min -= pad; by_min -= pad;
    bx_max += pad; by_max += pad;
    let bw = bx_max - bx_min;
    let bh = by_max - by_min;
    let mut svg = String::new();
    svg.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {:.1} {:.1}\" width=\"{:.1}\" height=\"{:.1}\">\n",
        bw, bh, bw, bh));
    for s in strokes.iter() {
        if s.points.is_empty() { continue; }
        let color_hex = format!("#{:02x}{:02x}{:02x}",
            (s.r * 255.0) as u8, (s.g * 255.0) as u8, (s.b * 255.0) as u8);
        let mut d = String::new();
        let (x0, y0, _) = s.points[0];
        d.push_str(&format!("M {:.1} {:.1}", x0 - bx_min, y0 - by_min));
        for i in 1..s.points.len() {
            let (x, y, _) = s.points[i];
            d.push_str(&format!(" L {:.1} {:.1}", x - bx_min, y - by_min));
        }
        let avg_w = s.avg_width();
        svg.push_str(&format!(
            "  <path d=\"{}\" stroke=\"{}\" stroke-width=\"{:.1}\" fill=\"none\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>\n",
            d, color_hex, avg_w));
    }
    svg.push_str("</svg>\n");
    let path = desktop_path().join(timestamped_name("svg"));
    if let Err(e) = std::fs::write(&path, &svg) {
        eprintln!("[glaspen2] SVG save failed: {}", e);
    } else {
        println!("[glaspen2] Saved SVG to {}", path.display());
    }
}

/// Save cropped drawing as GIF to desktop. Returns 1 on success, 0 on failure.
#[no_mangle]
pub extern "C" fn glaspen2_save_gif_cropped(
    surface_data: *const c_uchar, surface_w: c_int, surface_h: c_int, surface_stride: c_int,
) -> c_int {
    let w = surface_w as u32; let h = surface_h as u32;
    let stride = surface_stride as usize;
    let raw = unsafe { slice::from_raw_parts(surface_data, stride * h as usize) };
    let strokes = STROKES.lock().unwrap();
    if strokes.is_empty() { return 0; }
    let mut bx_min = u32::MAX; let mut by_min = u32::MAX;
    let mut bx_max = 0u32; let mut by_max = 0u32;
    for s in strokes.iter() {
        for &(x, y, _) in &s.points {
            let ix = x as u32; let iy = y as u32;
            if ix < bx_min { bx_min = ix; }
            if iy < by_min { by_min = iy; }
            if ix > bx_max { bx_max = ix; }
            if iy > by_max { by_max = iy; }
        }
    }
    let pad = 5u32;
    bx_min = bx_min.saturating_sub(pad);
    by_min = by_min.saturating_sub(pad);
    bx_max = (bx_max + pad).min(w - 1);
    by_max = (by_max + pad).min(h - 1);
    let crop_w = bx_max - bx_min + 1;
    let crop_h = by_max - by_min + 1;
    let mut flat: Vec<u8> = Vec::with_capacity((crop_w * crop_h * 4) as usize);
    for cy in 0..crop_h {
        let sy = (by_min + cy) as usize;
        for cx in 0..crop_w {
            let sx = (bx_min + cx) as usize;
            let off = sy * stride + sx * 4;
            if off + 3 < raw.len() {
                let b = raw[off]; let g = raw[off + 1]; let r = raw[off + 2]; let a = raw[off + 3];
                if a == 0 { flat.extend_from_slice(&[0, 0, 0, 0]); }
                else { flat.extend_from_slice(&[r, g, b, a]); }
            }
        }
    }
    // Downscale to 50% for smaller GIF
    let gif_w = crop_w / 2;
    let gif_h = crop_h / 2;
    let mut gif_pixels: Vec<u8> = Vec::with_capacity((gif_w * gif_h * 4) as usize);
    for gy in 0..gif_h {
        for gx in 0..gif_w {
            let sx = gx * 2; let sy = gy * 2;
            let off = (sy * crop_w + sx) as usize * 4;
            if off + 3 < flat.len() {
                gif_pixels.extend_from_slice(&flat[off..off+4]);
            }
        }
    }

    let mut quantizer = color_quant::NeuQuant::new(30, 128, &gif_pixels);
    let indices: Vec<u8> = gif_pixels.chunks(4).map(|p| {
        quantizer.index_of(&[p[0], p[1], p[2], p[3]]) as u8
    }).collect();
    let mut idx_counts = [0u32; 128];
    for (i, &idx) in indices.iter().enumerate() {
        if gif_pixels[i * 4 + 3] == 0 { idx_counts[idx as usize] += 1; }
    }
    let mut transparent_idx: u8 = 0;
    let mut max_count = 0u32;
    for i in 0..128 { if idx_counts[i] > max_count { max_count = idx_counts[i]; transparent_idx = i as u8; } }
    let palette = quantizer.color_map_rgba();
    let gif_palette: Vec<u8> = (0..128).flat_map(|i| {
        [palette[i * 4], palette[i * 4 + 1], palette[i * 4 + 2]]
    }).collect();
    let mut gif_data = Vec::new();
    {
        let mut enc = gif::Encoder::new(&mut gif_data, gif_w as u16, gif_h as u16, &gif_palette).unwrap();
        let frame = gif::Frame {
            width: gif_w as u16, height: gif_h as u16,
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
    if let Err(e) = std::fs::write(&path, &gif_data) {
        eprintln!("[glaspen2] GIF write failed: {}", e);
        0
    } else {
        println!("[glaspen2] Saved GIF to {}", path.display());
        1
    }
}

/// Save raw BGRA pixels as GIF. Called from C# overlay.
/// pixels: BGRA data, w/h: dimensions, stride: bytes per row.
/// Saves to desktop, returns path via out_path (wchar buffer, max 260 chars).
/// Returns 1 on success, 0 on failure.
#[no_mangle]
pub extern "C" fn glaspen2_save_gif_from_pixels(
    pixels: *const c_uchar, w: c_int, h: c_int, stride: c_int,
    out_path: *mut u16, out_path_len: c_int,
) -> c_int {
    let w = w as u32; let h = h as u32;
    let stride = stride as usize;
    let raw = unsafe { slice::from_raw_parts(pixels, stride * h as usize) };

    // Downsample 50%
    let gif_w = w / 2;
    let gif_h = h / 2;
    let mut gif_pixels: Vec<u8> = Vec::with_capacity((gif_w * gif_h * 4) as usize);
    for gy in 0..gif_h {
        for gx in 0..gif_w {
            let sx = gx * 2; let sy = gy * 2;
            let off = (sy as usize * stride + sx as usize * 4);
            if off + 3 < raw.len() {
                // BGRA -> RGBA
                gif_pixels.push(raw[off + 2]); // R
                gif_pixels.push(raw[off + 1]); // G
                gif_pixels.push(raw[off]);     // B
                gif_pixels.push(raw[off + 3]); // A
            }
        }
    }

    let mut quantizer = color_quant::NeuQuant::new(30, 128, &gif_pixels);
    let indices: Vec<u8> = gif_pixels.chunks(4).map(|p| {
        quantizer.index_of(&[p[0], p[1], p[2], p[3]]) as u8
    }).collect();
    let mut idx_counts = [0u32; 128];
    for (i, &idx) in indices.iter().enumerate() {
        if gif_pixels[i * 4 + 3] == 0 { idx_counts[idx as usize] += 1; }
    }
    let mut transparent_idx: u8 = 0;
    let mut max_count = 0u32;
    for i in 0..128 { if idx_counts[i] > max_count { max_count = idx_counts[i]; transparent_idx = i as u8; } }
    let palette = quantizer.color_map_rgba();
    let gif_palette: Vec<u8> = (0..128).flat_map(|i| {
        [palette[i * 4], palette[i * 4 + 1], palette[i * 4 + 2]]
    }).collect();
    let mut gif_data = Vec::new();
    {
        let mut enc = gif::Encoder::new(&mut gif_data, gif_w as u16, gif_h as u16, &gif_palette).unwrap();
        let frame = gif::Frame {
            width: gif_w as u16, height: gif_h as u16,
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
    if let Err(e) = std::fs::write(&path, &gif_data) {
        eprintln!("[glaspen2] GIF write failed: {}", e);
        0
    } else {
        // Copy path to out_path as UTF-16
        let path_str = path.to_string_lossy();
        let utf16: Vec<u16> = path_str.encode_utf16().collect();
        if !out_path.is_null() && out_path_len > 0 {
            let max = (out_path_len as usize).min(utf16.len());
            unsafe {
                std::ptr::copy_nonoverlapping(utf16.as_ptr(), out_path, max);
                if max < out_path_len as usize {
                    *out_path.add(max) = 0; // null terminator
                }
            }
        }
        println!("[glaspen2] Saved GIF to {}", path.display());
        1
    }
}

fn timestamped_name(ext: &str) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let secs = now.as_secs();
    let s = secs % 60; let m = (secs / 60) % 60; let h = (secs / 3600 + 8) % 24;
    let days = secs / 86400; let y = 1970 + days / 365; let d = days % 365;
    format!("glaspen2_{:04}-{:03}_{:02}-{:02}-{:02}.{}", y, d, h, m, s, ext)
}

/// Get current time as seconds since Unix epoch (f64). For modeler timestamps.
#[no_mangle]
pub extern "C" fn glaspen2_now_secs() -> c_double {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

// ── Cairo Renderer (Windows: cairo_dl, falls back to stub) ──

#[cfg(target_os = "windows")]
mod cairo_renderer {
    use std::os::raw::{c_uchar};
    use std::f64::consts::PI;

    use super::{STROKES, modeler};

    /// Uses real Cairo (via cairo_dl) if DLL loaded, falls back to stub.
    enum CairoBackend {
        Real(super::windows::cairo_dl::CairoRealSurface),
        Stub(super::cairo::ImageSurface),
    }

    pub struct CairoRenderer {
        surface: CairoBackend,
        width: i32,
        height: i32,
    }

    impl CairoRenderer {
        pub fn new(width: i32, height: i32) -> Option<Self> {
            let _ = super::windows::cairo_dl::cairo_init();
            let surface = if super::windows::cairo_dl::is_cairo_loaded() {
                let s = super::windows::cairo_dl::CairoRealSurface::create(width, height)?;
                CairoBackend::Real(s)
            } else {
                let s = super::cairo::ImageSurface::create(super::cairo::Format::ARGB32, width, height).ok()?;
                CairoBackend::Stub(s)
            };
            // Initialize to fully transparent
            let mut r = Self { surface, width, height };
            r.clear();
            Some(r)
        }

        pub fn clear(&mut self) {
            match &self.surface {
                CairoBackend::Real(ref s) => {
                    if let Some(cr) = super::windows::cairo_dl::CairoRealContext::new(s) {
                        cr.set_operator_clear();
                        cr.paint();
                        cr.set_operator_over();
                    }
                }
                CairoBackend::Stub(ref s) => {
                    use super::cairo::{Context, Operator};
                    if let Ok(cr) = Context::new(s) {
                        cr.set_operator(Operator::Clear);
                        cr.paint().ok();
                    }
                }
            }
        }

        pub fn draw_line(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, width: f64, r: f64, g: f64, b: f64) {
            match &self.surface {
                CairoBackend::Real(ref s) => {
                    if let Some(cr) = super::windows::cairo_dl::CairoRealContext::new(s) {
                        cr.set_source_rgba(r, g, b, 1.0);
                        cr.set_line_width(width);
                        cr.set_line_cap_round();
                        cr.set_line_join_round();
                        cr.move_to(x0, y0);
                        cr.line_to(x1, y1);
                        cr.stroke();
                    }
                }
                CairoBackend::Stub(ref s) => {
                    use super::cairo::{Context, LineCap, LineJoin};
                    if let Ok(cr) = Context::new(s) {
                        cr.set_source_rgba(r, g, b, 1.0);
                        cr.set_line_width(width);
                        cr.set_line_cap(LineCap::Round);
                        cr.set_line_join(LineJoin::Round);
                        cr.move_to(x0, y0);
                        cr.line_to(x1, y1);
                        cr.stroke().ok();
                    }
                }
            }
        }

        pub fn draw_dot(&mut self, x: f64, y: f64, width: f64, r: f64, g: f64, b: f64) {
            match &self.surface {
                CairoBackend::Real(ref s) => {
                    if let Some(cr) = super::windows::cairo_dl::CairoRealContext::new(s) {
                        cr.set_source_rgba(r, g, b, 1.0);
                        cr.arc(x, y, width * 0.5, 0.0, 2.0 * PI);
                        cr.fill();
                    }
                }
                CairoBackend::Stub(ref s) => {
                    use super::cairo::Context;
                    if let Ok(cr) = Context::new(s) {
                        cr.set_source_rgba(r, g, b, 1.0);
                        cr.arc(x, y, width * 0.5, 0.0, 2.0 * PI);
                        cr.fill().ok();
                    }
                }
            }
        }

        pub fn surface_data(&self) -> *const c_uchar {
            match &self.surface {
                CairoBackend::Real(ref s) => {
                    s.flush();
                    s.data_ptr()
                }
                CairoBackend::Stub(ref s) => {
                    // stub: cast &[u8] to pointer
                    s.data().map(|d| d.as_ptr()).unwrap_or(std::ptr::null())
                }
            }
        }

        pub fn surface_data_mut(&self) -> *mut c_uchar {
            match &self.surface {
                CairoBackend::Real(ref s) => {
                    s.flush();
                    s.data_ptr_mut()
                }
                CairoBackend::Stub(ref s) => {
                    s.pixels_mut().as_mut_ptr()
                }
            }
        }

        pub fn surface_size(&self) -> (i32, i32, i32) {
            let stride = match &self.surface {
                CairoBackend::Real(ref s) => s.stride(),
                CairoBackend::Stub(ref s) => s.stride(),
            };
            (self.width, self.height, stride)
        }

        /// Draw all smoothed points from the modeler buffer onto the surface.
        /// Uses per-point width from the modeler. Called after pen-up.
        pub fn draw_modeler_buffer(&mut self, r: f64, g: f64, b: f64) {
            let count = modeler::buffer_len();
            if count < 1 { return; }

            if let Some((x0, y0, w0)) = modeler::get_buffer_point(0) {
                self.draw_dot(x0, y0, w0, r, g, b);
                let mut prev_x = x0;
                let mut prev_y = y0;
                for i in 1..count {
                    if let Some((px, py, pw)) = modeler::get_buffer_point(i) {
                        self.draw_line(prev_x, prev_y, px, py, pw, r, g, b);
                        prev_x = px;
                        prev_y = py;
                    }
                }
            }
        }

        /// Replay all strokes from the STROKES vector onto the surface.
        /// Used after page navigation (load + smooth).
        pub fn replay_strokes(&mut self) {
            self.clear();
            let strokes = STROKES.lock().unwrap();
            for stroke in strokes.iter() {
                if stroke.points.is_empty() { continue; }
                let (x0, y0, w0) = stroke.points[0];
                self.draw_dot(x0, y0, w0, stroke.r, stroke.g, stroke.b);
                for w in stroke.points.windows(2) {
                    let (x1, y1, w1) = w[0];
                    let (x2, y2, w2) = w[1];
                    self.draw_line(x1, y1, x2, y2, w2, stroke.r, stroke.g, stroke.b);
                }
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub use cairo_renderer::CairoRenderer;

// ── Cairo Renderer FFI ──

/// Create a new Cairo renderer. Returns pointer or null on failure.
/// Caller must later call glaspen2_cairo_renderer_destroy to free.
#[no_mangle]
#[cfg(target_os = "windows")]
pub extern "C" fn glaspen2_cairo_renderer_create(w: c_int, h: c_int) -> *mut CairoRenderer {
    match CairoRenderer::new(w, h) {
        Some(r) => Box::into_raw(Box::new(r)),
        None => std::ptr::null_mut(),
    }
}

/// Destroy a Cairo renderer previously created with glaspen2_cairo_renderer_create.
#[no_mangle]
#[cfg(target_os = "windows")]
pub extern "C" fn glaspen2_cairo_renderer_destroy(renderer: *mut CairoRenderer) {
    if !renderer.is_null() {
        unsafe { drop(Box::from_raw(renderer)); }
    }
}

/// Draw a line segment from (x0,y0) to (x1,y1). Width and color are per-point.
#[no_mangle]
#[cfg(target_os = "windows")]
pub extern "C" fn glaspen2_cairo_draw_line(
    renderer: *mut CairoRenderer,
    x0: c_double, y0: c_double, x1: c_double, y1: c_double,
    width: c_double, r: c_double, g: c_double, b: c_double,
) {
    if !renderer.is_null() {
        unsafe { (*renderer).draw_line(x0, y0, x1, y1, width, r, g, b); }
    }
}

/// Draw a filled dot at (x,y) with given width and color.
#[no_mangle]
#[cfg(target_os = "windows")]
pub extern "C" fn glaspen2_cairo_draw_dot(
    renderer: *mut CairoRenderer,
    x: c_double, y: c_double, width: c_double,
    r: c_double, g: c_double, b: c_double,
) {
    if !renderer.is_null() {
        unsafe { (*renderer).draw_dot(x, y, width, r, g, b); }
    }
}

/// Clear the renderer surface to fully transparent.
#[no_mangle]
#[cfg(target_os = "windows")]
pub extern "C" fn glaspen2_cairo_clear(renderer: *mut CairoRenderer) {
    if !renderer.is_null() {
        unsafe { (*renderer).clear(); }
    }
}

/// Get pointer to the BGRA pixel data. Returns null if renderer is null.
#[no_mangle]
#[cfg(target_os = "windows")]
pub extern "C" fn glaspen2_cairo_surface_data(renderer: *mut CairoRenderer) -> *const c_uchar {
    if renderer.is_null() { return std::ptr::null(); }
    unsafe { (*renderer).surface_data() }
}

/// Get mutable pointer to the BGRA pixel data. Returns null if renderer is null.
#[no_mangle]
#[cfg(target_os = "windows")]
pub extern "C" fn glaspen2_cairo_surface_data_mut(renderer: *mut CairoRenderer) -> *mut c_uchar {
    if renderer.is_null() { return std::ptr::null_mut(); }
    unsafe { (*renderer).surface_data_mut() }
}

/// Get surface dimensions and stride.
#[no_mangle]
#[cfg(target_os = "windows")]
pub extern "C" fn glaspen2_cairo_surface_size(
    renderer: *mut CairoRenderer,
    w: *mut c_int, h: *mut c_int, stride: *mut c_int,
) {
    if renderer.is_null() {
        unsafe { *w = 0; *h = 0; *stride = 0; }
        return;
    }
    let (width, height, s) = unsafe { (*renderer).surface_size() };
    unsafe { *w = width; *h = height; *stride = s; }
}

/// After calling modeler_end, call this to draw all smoothed points from the modeler buffer.
/// Then call modeler_commit_to_strokes to persist.
#[no_mangle]
#[cfg(target_os = "windows")]
pub extern "C" fn glaspen2_cairo_draw_modeler_buffer(
    renderer: *mut CairoRenderer,
    r: c_double, g: c_double, b: c_double,
) {
    if !renderer.is_null() {
        unsafe { (*renderer).draw_modeler_buffer(r, g, b); }
    }
}

/// Replay all strokes from the STROKES vector onto the renderer surface.
/// Call after glaspen2_load_strokes_for_screen + glaspen2_smooth_loaded_strokes.
#[no_mangle]
#[cfg(target_os = "windows")]
pub extern "C" fn glaspen2_cairo_replay_strokes(renderer: *mut CairoRenderer) {
    if !renderer.is_null() {
        unsafe { (*renderer).replay_strokes(); }
    }
}
