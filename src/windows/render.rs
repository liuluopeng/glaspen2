use std::f64::consts::PI;
use crate::cairo::{Context, ImageSurface, Operator, LineCap, LineJoin, FontSlant, FontWeight};

/// Draw a pen stroke segment on the Cairo surface.
pub fn pen_draw(
    surface: &ImageSurface,
    x: f64,
    y: f64,
    width: f64,
    r: f64,
    g: f64,
    b: f64,
    last_x: f64,
    last_y: f64,
    has_last: bool,
) {
    let cr = Context::new(surface).unwrap();
    cr.set_source_rgba(r, g, b, 1.0);
    cr.set_line_width(width);
    cr.set_line_cap(LineCap::Round);
    cr.set_line_join(LineJoin::Round);

    if has_last {
        cr.move_to(last_x, last_y);
        cr.line_to(x, y);
        cr.stroke().unwrap();
    } else {
        cr.arc(x, y, width * 0.5, 0.0, 2.0 * PI);
        cr.fill().unwrap();
    }
}

/// Compute contrast color (black or white) for outline based on luminance.
pub fn contrast_color(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let lum = 0.299 * r + 0.587 * g + 0.114 * b;
    if lum > 0.5 { (0.0, 0.0, 0.0) } else { (1.0, 1.0, 1.0) }
}

/// Draw a filled dot at (x, y) with given width and color.
pub fn draw_dot(surface: &ImageSurface, x: f64, y: f64, width: f64, r: f64, g: f64, b: f64) {
    let cr = Context::new(surface).unwrap();
    cr.set_source_rgba(r, g, b, 1.0);
    cr.arc(x, y, width * 0.5, 0.0, 2.0 * PI);
    cr.fill().unwrap();
}

/// Clear the entire Cairo surface to Fuchsia (transparent via color key).
pub fn clear_screen(surface: &ImageSurface) {
    // Fill with Fuchsia — this is our transparent color (LWA_COLORKEY)
    let stride = surface.stride() as usize;
    let w = surface.width() as usize;
    let h = surface.height() as usize;
    let pixels = surface.pixels_mut();
    for y in 0..h {
        for x in 0..w {
            let off = y * stride + x * 4;
            if off + 3 < pixels.len() {
                pixels[off] = 255;     // B (Fuchsia = 255,0,255)
                pixels[off + 1] = 0;   // G
                pixels[off + 2] = 255; // R
                pixels[off + 3] = 255; // A (opaque, color-key makes it transparent)
            }
        }
    }
}

/// Draw the rainbow indicator bar at top-left corner.
pub fn draw_rainbow_indicator(surface: &ImageSurface) {
    let cr = Context::new(surface).unwrap();
    cr.set_operator(Operator::Over);

    for col in 0..14 {
        let h = col as f64 / 14.0;
        let (r, g, b) = hsv_to_rgb(h);
        cr.set_source_rgba(r, g, b, 1.0);
        cr.rectangle(col as f64 * 2.0, 0.0, 2.0, 4.0);
        cr.fill().unwrap();
    }
}

/// Draw the crosshair cursor overlay.
pub fn draw_crosshair(surface: &ImageSurface, cx: f64, cy: f64) {
    let cr = Context::new(surface).unwrap();
    let radius = 8.0;

    // Outer circle
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.8);
    cr.set_line_width(1.5);
    cr.arc(cx, cy, radius, 0.0, 2.0 * PI);
    cr.stroke().unwrap();

    // Center dot
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.9);
    cr.arc(cx, cy, 1.5, 0.0, 2.0 * PI);
    cr.fill().unwrap();

    // Crosshair lines (black with gap)
    let gap = 3.0;
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.5);
    cr.set_line_width(1.0);

    // Top
    cr.move_to(cx, cy - radius - 2.0);
    cr.line_to(cx, cy - gap);
    // Bottom
    cr.move_to(cx, cy + gap);
    cr.line_to(cx, cy + radius + 2.0);
    // Left
    cr.move_to(cx - radius - 2.0, cy);
    cr.line_to(cx - gap, cy);
    // Right
    cr.move_to(cx + gap, cy);
    cr.line_to(cx + radius + 2.0, cy);
    cr.stroke().unwrap();
}

/// Draw centered notification text with shadow.
pub fn draw_notification(surface: &ImageSurface, text: &str) {
    let w = surface.width() as f64;
    let h = surface.height() as f64;
    let cr = Context::new(surface).unwrap();

    // Use a simple monospace font
    cr.select_font_face("monospace", FontSlant::Normal, FontWeight::Normal);
    cr.set_font_size(36.0);

    let extents = cr.text_extents(text);
    let tx = (w - extents.width()) / 2.0 - extents.x_bearing();
    let ty = (h - extents.height()) / 2.0 - extents.y_bearing();

    // Shadow
    cr.move_to(tx + 2.0, ty + 2.0);
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.8);
    cr.show_text(text).unwrap();

    // Main text
    cr.move_to(tx, ty);
    cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    cr.show_text(text).unwrap();
}

/// Copy Cairo surface pixels to a raw BGRA buffer (for UpdateLayeredWindow).
pub fn copy_surface_to_bgra(surface: &ImageSurface, dest: &mut [u8]) {
    let w = surface.width() as usize;
    let h = surface.height() as usize;
    let stride = surface.stride() as usize;
    let data = surface.data().unwrap();

    for y in 0..h {
        for x in 0..w {
            let src_off = y * stride + x * 4;
            let dst_off = y * w * 4 + x * 4;
            if src_off + 3 < data.len() && dst_off + 3 < dest.len() {
                // Cairo ARGB32 premultiplied on LE: [B, G, R, A]
                // UpdateLayeredWindow expects BGRA: same order
                dest[dst_off] = data[src_off];         // B
                dest[dst_off + 1] = data[src_off + 1]; // G
                dest[dst_off + 2] = data[src_off + 2]; // R
                dest[dst_off + 3] = data[src_off + 3]; // A
            }
        }
    }
}

fn hsv_to_rgb(h: f64) -> (f64, f64, f64) {
    let i = (h * 6.0) as i32;
    let f = h * 6.0 - i as f64;
    let q = 1.0 - f;
    match i % 6 {
        0 => (1.0, f, 0.0),
        1 => (q, 1.0, 0.0),
        2 => (0.0, 1.0, f),
        3 => (0.0, q, 1.0),
        4 => (f, 0.0, 1.0),
        5 => (1.0, 0.0, q),
        _ => (0.0, 0.0, 0.0),
    }
}
