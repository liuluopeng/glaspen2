use std::path::PathBuf;
use std::slice;
use std::os::raw::{c_int, c_double, c_uchar};
use std::sync::Mutex;

// --- Stroke recording for Xournal export ---

struct Stroke {
    r: f64,
    g: f64,
    b: f64,
    points: Vec<(f64, f64, f64)>, // (x, y, width)
}

static STROKES: Mutex<Vec<Stroke>> = Mutex::new(Vec::new());

#[no_mangle]
pub extern "C" fn glaspen2_begin_stroke(r: c_double, g: c_double, b: c_double) {
    let mut strokes = STROKES.lock().unwrap();
    strokes.push(Stroke { r, g, b, points: Vec::new() });
}

#[no_mangle]
pub extern "C" fn glaspen2_add_point(x: c_double, y: c_double, width: c_double) {
    let mut strokes = STROKES.lock().unwrap();
    if let Some(stroke) = strokes.last_mut() {
        stroke.points.push((x, y, width));
    }
}

#[no_mangle]
pub extern "C" fn glaspen2_end_stroke() {
    // Stroke boundary — nothing to do, strokes are separated by begin/end
}

#[no_mangle]
pub extern "C" fn glaspen2_clear_strokes() {
    let mut strokes = STROKES.lock().unwrap();
    strokes.clear();
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
        Ok(_) => println!("[glaspen2-rust] Saved Xournal to {}", path.display()),
        Err(e) => eprintln!("[glaspen2-rust] Xournal save failed: {}", e),
    }
}

fn xoj_timestamped_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let desktop = PathBuf::from(home).join("Desktop");
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

fn main() {
    #[cfg(target_os = "macos")]
    macos_run();

    #[cfg(not(target_os = "macos"))]
    println!("Only macOS supported");
}

#[cfg(target_os = "macos")]
fn macos_run() {
    extern "C" {
        fn glaspen2_run();
    }
    unsafe { glaspen2_run(); }
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
        Ok(_) => println!("[glaspen2-rust] Saved (drawing only) to {}", path.display()),
        Err(e) => eprintln!("[glaspen2-rust] Save failed: {}", e),
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
        Ok(_) => println!("[glaspen2-rust] Saved (with background) to {}", path.display()),
        Err(e) => eprintln!("[glaspen2-rust] Save failed: {}", e),
    }
}

fn timestamped_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let desktop = PathBuf::from(home).join("Desktop");
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
