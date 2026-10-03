//! 文件导出:XOJ 笔记、PNG 截图、SVG、GIF 动画、PDF。

use super::*;

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
                // Cairo ARGB32 on little-endian: [B, G, R, A] — 预乘 alpha,
                // 需反预乘成直线 alpha, 否则半透明边缘在 PNG 里发暗(黑边)。
                let b = raw[offset];
                let g = raw[offset + 1];
                let r = raw[offset + 2];
                let a = raw[offset + 3] as u32;
                let un = |c: u8| ((c as u32 * 255 + a / 2) / a.max(1)) as u8;
                img.put_pixel(x, y, image::Rgba([un(r), un(g), un(b), a as u8]));
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
                // cairo ARGB32 是预乘值: out = d_premul + bg × (1-α)
                // (旧代码把预乘值又乘了一次 α, 笔迹边缘发暗)
                let db = draw_raw[d_offset] as f32;
                let dg = draw_raw[d_offset + 1] as f32;
                let dr = draw_raw[d_offset + 2] as f32;
                let da = draw_raw[d_offset + 3] as f32 / 255.0;

                if da > 0.01 {
                    let bg_pixel = img.get_pixel(x, y);
                    let br = bg_pixel[0] as f32;
                    let bg_g = bg_pixel[1] as f32;
                    let bb = bg_pixel[2] as f32;

                    let r = (dr + br * (1.0 - da)) as u8;
                    let g = (dg + bg_g * (1.0 - da)) as u8;
                    let b = (db + bb * (1.0 - da)) as u8;
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
pub(crate) fn snapshot_from_stroke_data(
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
pub(crate) fn quantize_width(w: f64) -> f64 {
    (w * 4.0).round() / 4.0
}

/// Build an SVG cropped to the content bbox. Emits ONE `<path>` per run of
/// consecutive points that share a quantized width (plus one `<circle>` for the
/// first point) instead of one element per point, so a canvas with tens of
/// thousands of points stays a few thousand nodes and opens quickly.
pub(crate) fn build_svg_from(snapshot: &[(f64, f64, f64, Vec<(f64, f64, f64)>)]) -> Option<String> {
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

/// 导出单页 PNG(白底, 页原生分辨率 ×2 采样)到桌面。返回 1 成功。
/// 把某页笔迹画进外部 cairo 表面(scale = 采样倍率)。白底可选。
/// 单页 PNG 导出与翻页"时光隧道"动画的页快照共用。
pub(crate) fn paint_page_into_surface(
    r: &crate::cairo_dl::CairoRenderer,
    screen_id: i64,
    scale: f64,
    white_bg: bool,
) -> c_int {
    let strokes = runtime().block_on(db::strokes_for_screen(screen_id));
    r.clear();
    if white_bg {
        // 白底尺寸给足(覆盖任意表面): fill_rect 不做越界裁剪也安全
        r.fill_rect(-1e5, -1e5, 2e5, 2e5, (255, 255, 255));
    }
    if strokes.is_empty() {
        r.flush();
        return 1; // 空页 = 纯白/纯透明, 仍算成功
    }
    let outline = STROKE_OUTLINE.load(std::sync::atomic::Ordering::SeqCst);
    let views: Vec<crate::Stroke> = strokes
        .into_iter()
        .map(|sd| crate::Stroke {
            id: sd.id,
            r: sd.r,
            g: sd.g,
            b: sd.b,
            points: sd.points,
        })
        .collect();
    // 与玻璃渲染同一条管线: pan=0, zoom=1
    super::paint_strokes(&r, &views, 0.0, 0.0, 1.0, scale, 0.0, outline);
    r.flush();
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_paint_page_into_surface(
    surface_ptr: *mut std::ffi::c_void,
    screen_id: i64,
    scale: c_double,
    white_bg: c_int,
) -> c_int {
    let Some(r) = crate::cairo_dl::CairoRenderer::from_surface(surface_ptr) else {
        return 0;
    };
    paint_page_into_surface(&r, screen_id, scale, white_bg != 0)
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_export_page_png(screen_id: i64) -> c_int {
    let Some((w, h)) = runtime().block_on(db::screen_dims(screen_id)) else {
        return 0;
    };
    let scale: f64 = 2.0; // 2x 采样: 观感与玻璃上的 retina 渲染一致
    let pw = (w as f64 * scale) as usize;
    let ph = (h as f64 * scale) as usize;

    let r = match crate::cairo_dl::CairoRenderer::create_owned(pw as i32, ph as i32) {
        Some(r) => r,
        None => return 0,
    };
    if paint_page_into_surface(&r, screen_id, scale, true) == 0 {
        return 0;
    }

    let data = unsafe { std::slice::from_raw_parts(r.bits(), pw * ph * 4) };
    let Some(png) = encode_png_rgba(data, pw as u32, ph as u32) else {
        return 0;
    };
    let path = desktop_path().join(format!(
        "glaspen2_p{}_{}",
        screen_id,
        timestamped_name("png")
    ));
    match std::fs::write(&path, &png) {
        Ok(()) => {
            eprintln!("[export] 页 {screen_id} PNG → {}", path.display());
            1
        }
        Err(e) => {
            eprintln!("[export] 页 PNG 写入失败: {e}");
            0
        }
    }
}

/// 导出单页 SVG(按内容包围盒裁剪)到桌面。返回 1 成功。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_export_page_svg(screen_id: i64) -> c_int {
    let strokes = runtime().block_on(db::strokes_for_screen(screen_id));
    if strokes.is_empty() {
        return 0;
    }
    let snap = snapshot_from_stroke_data(&strokes);
    let Some(svg) = build_svg_from(&snap) else {
        return 0;
    };
    let path = desktop_path().join(format!(
        "glaspen2_p{}_{}",
        screen_id,
        timestamped_name("svg")
    ));
    match std::fs::write(&path, &svg) {
        Ok(()) => {
            eprintln!("[export] 页 {screen_id} SVG → {}", path.display());
            1
        }
        Err(e) => {
            eprintln!("[export] 页 SVG 写入失败: {e}");
            0
        }
    }
}

/// 导出勾选的页为单个 PDF(ids_json = "[3,7,9]"; 空数组 = 全部页)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_export_pages_pdf_json(ids_json: *const c_char) -> c_int {
    let ids: Vec<i64> = if ids_json.is_null() {
        Vec::new()
    } else {
        let Ok(js) = unsafe { CStr::from_ptr(ids_json) }.to_str() else {
            return 0;
        };
        serde_json::from_str::<Vec<i64>>(js).unwrap_or_default()
    };
    match crate::pdf::export_pages_by_ids(&ids) {
        Some(p) => {
            eprintln!(
                "[export] {} 页 PDF → {p}",
                if ids.is_empty() {
                    "全部".to_string()
                } else {
                    ids.len().to_string()
                }
            );
            1
        }
        None => 0,
    }
}

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
pub(crate) struct GifStroke {
    pub(crate) r: f64,
    pub(crate) g: f64,
    pub(crate) b: f64,
    pub(crate) points: Vec<(f64, f64, f64, f64)>, // (x, y, width, relative_time)
}

/// Clone a slice of strokes into the render-only GifStroke representation.
pub(crate) fn gif_strokes_from(strokes: &[Stroke]) -> Vec<GifStroke> {
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
pub(crate) fn encode_animated_gif(
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
pub(crate) fn render_gif_frame(
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
                let a = *bits.add(off + 3);
                if a == 0 {
                    // 全透明背景: 保持 (0,0,0,0), 编码时映射到保留透明色
                    flat.extend_from_slice(&[0, 0, 0, 0]);
                    continue;
                }
                // 反预乘: 半透明的抗锯齿边缘像素否则会偏暗(黑边)
                let a32 = a as u32;
                let un = |c: u8| ((c as u32 * 255 + a32 / 2) / a32) as u8;
                flat.push(un(*bits.add(off + 2))); // R
                flat.push(un(*bits.add(off + 1))); // G
                flat.push(un(*bits.add(off))); // B
                flat.push(a); // A
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
pub extern "C" fn glaspen2_export_pdf() -> c_int {
    match crate::pdf::export_all_pages() {
        Some(_) => 1,
        None => 0,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn red_stroke() -> GifStroke {
        GifStroke {
            r: 0.84,
            g: 0.0,
            b: 0.23,
            points: (0..20)
                .map(|i| {
                    let t = i as f64;
                    (t * 4.0, 20.0 + (t * 0.7).sin() * 6.0, 2.5, t * 0.05)
                })
                .collect(),
        }
    }

    fn padded_bbox(strokes: &[GifStroke]) -> (i32, i32, f64, f64) {
        let mut x0 = f64::MAX;
        let mut y0 = f64::MAX;
        let mut x1 = f64::MIN;
        let mut y1 = f64::MIN;
        for s in strokes {
            for &(x, y, w, _) in &s.points {
                let h = w * 0.5;
                x0 = x0.min(x - h);
                y0 = y0.min(y - h);
                x1 = x1.max(x + h);
                y1 = y1.max(y + h);
            }
        }
        let pad = 10.0;
        (
            ((x1 - x0) + pad * 2.0).ceil() as i32,
            ((y1 - y0) + pad * 2.0).ceil() as i32,
            x0 - pad,
            y0 - pad,
        )
    }

    /// 回归:GIF 帧不得有"黑色描边"。cairo ARGB32 是预乘 alpha, 若不反预乘,
    /// 半透明的抗锯齿边缘像素会以暗色不透明进入 GIF —— 笔迹四周一圈黑边。
    #[test]
    fn gif_frame_edges_not_dark() {
        let strokes = vec![red_stroke()];
        let (bw, bh, bx_min, by_min) = padded_bbox(&strokes);
        let seg_offset = vec![(0usize, 0.0, strokes[0].points.last().unwrap().3)];
        let (flat, ok) = render_gif_frame(
            &strokes,
            &seg_offset,
            bw,
            bh,
            bx_min,
            by_min,
            120,
            90,
            0,
            false,
            f64::MAX,
            0,
        );
        assert!(ok);

        let mut opaque = 0;
        let mut dark = 0;
        for px in flat.as_chunks::<4>().0 {
            let [r, g, b, a] = *px;
            if a == 0 {
                continue; // 背景: 透明
            }
            opaque += 1;
            // 未反预乘时, 半透明边缘像素 = 本色×α, 会明显偏暗
            if r < 190 {
                dark += 1;
            }
            let _ = (g, b);
        }
        assert!(opaque > 50, "应渲染出笔迹像素: {opaque}");
        assert_eq!(dark, 0, "存在暗色描边像素: {dark}");
    }
}
