//! PDF export — render all pages with handwriting image + invisible selectable text.

use std::path::PathBuf;

use printpdf::*;

use crate::{db, runtime};

/// Export all pages to a PDF on the desktop.  Returns the file path.
pub fn export_all_pages() -> Option<String> {
    let rt = runtime();

    // Collect all screens
    let screens: Vec<(i64, i32, i32)> = {
        let pool = rt.block_on(async {
            sqlx::SqlitePool::connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(db::db_path())
                    .read_only(true),
            ).await
        });
        let pool = match pool { Ok(p) => p, Err(e) => { eprintln!("[pdf] DB: {e}"); return None; } };
        let rows = rt.block_on(async {
            sqlx::query_as(
                "SELECT s.id, s.screen_w, s.screen_h FROM screens s \
                 WHERE EXISTS (SELECT 1 FROM strokes WHERE screen_id = s.id) \
                 ORDER BY s.id"
            ).fetch_all(&pool).await.unwrap_or_default()
        });
        let _ = rt.block_on(pool.close());
        rows
    };

    if screens.is_empty() { eprintln!("[pdf] No pages"); return None; }
    eprintln!("[pdf] Exporting {} pages", screens.len());

    let mut doc = PdfDocument::new("glaspen2");
    let font_id = load_cjk_font(&mut doc);

    for (screen_id, sw, sh) in &screens {
        eprintln!("[pdf] Page {}: {}x{}", screen_id, sw, sh);
        let strokes = rt.block_on(db::strokes_for_screen(*screen_id));
        if strokes.is_empty() { continue; }

        // Render strokes to Cairo surface
        let mut surface = match crate::cairo::ImageSurface::create(
            crate::cairo::Format::ARgb32, *sw, *sh,
        ) { Ok(s) => s, Err(_) => continue };

        {
            let Ok(cr) = crate::cairo::Context::new(&surface) else { continue };
            cr.set_operator(crate::cairo::Operator::Clear);
            let _ = cr.paint();
            cr.set_operator(crate::cairo::Operator::Over);
            cr.set_line_cap(crate::cairo::LineCap::Round);
            cr.set_line_join(crate::cairo::LineJoin::Round);
            for s in &strokes {
                if s.points.len() < 2 { continue; }
                cr.set_source_rgba(s.r, s.g, s.b, 1.0);
                for i in 0..s.points.len() {
                    let (x, y, w, _t) = s.points[i];
                    if i == 0 {
                        let _ = cr.arc(x, y, w * 0.5, 0.0, 2.0 * std::f64::consts::PI);
                        let _ = cr.fill();
                    } else {
                        let (px, py, _pw, _pt) = s.points[i - 1];
                        cr.set_line_width(w);
                        let _ = cr.move_to(px, py);
                        let _ = cr.line_to(x, y);
                        let _ = cr.stroke();
                    }
                }
            }
        }

        // Convert BGRA (Cairo) → RGBA raw pixels
        let stride = surface.stride() as usize;
        let surf_w = surface.width() as u32;
        let surf_h = surface.height() as u32;
        let d = surface.data().unwrap();
        let mut rgba = vec![0u8; (surf_w * surf_h * 4) as usize];
        for y in 0..surf_h {
            for x in 0..surf_w {
                let off = y as usize * stride + x as usize * 4;
                let pi = (y * surf_w + x) as usize * 4;
                rgba[pi] = d[off + 2];
                rgba[pi + 1] = d[off + 1];
                rgba[pi + 2] = d[off];
                rgba[pi + 3] = d[off + 3];
            }
        }
        std::mem::drop(d);

        // Add image to PDF — use raw RGBA pixels
        let raw_img = RawImage {
            pixels: RawImageData::U8(rgba),
            width: surf_w as usize,
            height: surf_h as usize,
            data_format: RawImageFormat::RGBA8,
            tag: vec![],
        };
        let img_id = doc.add_image(&raw_img);

        // Page dimensions in mm (72 pt/inch, 25.4 mm/inch)
        let mm_w = *sw as f32 * 25.4 / 72.0;
        let mm_h = *sh as f32 * 25.4 / 72.0;

        // Operations
        let mut ops = Vec::new();

        // Place the image at the page origin
        ops.push(Op::SaveGraphicsState);
        ops.push(Op::UseXobject {
            id: img_id,
            transform: XObjectTransform {
                translate_x: None,
                translate_y: None,
                rotate: None,
                scale_x: None,
                scale_y: None,
                dpi: Some(72.0), // match screen resolution 1:1
            },
        });
        ops.push(Op::RestoreGraphicsState);

        // Invisible selectable text layer
        if let Some(ocr) = rt.block_on(db::load_latest_ocr(*screen_id)) {
            if !ocr.boxes.is_empty() {
                ops.push(Op::StartTextSection);
                for ob in &ocr.boxes {
                    // PDF origin is bottom-left; screen origin is top-left
                    let pdf_x = ob.x as f32 * 25.4 / 72.0; // in mm
                    let pdf_y = (*sh as f32 - ob.y as f32 - ob.h as f32) * 25.4 / 72.0;
                    let font_size = Pt((ob.h as f32 * 0.8).max(4.0));

                    if let Some(ref fid) = font_id {
                        ops.push(Op::SetFont {
                            font: PdfFontHandle::External(fid.clone()),
                            size: font_size,
                        });
                    }
                    ops.push(Op::SetTextRenderingMode {
                        mode: TextRenderingMode::Invisible,
                    });
                    ops.push(Op::SetTextCursor {
                        pos: Point { x: Pt(pdf_x * 72.0 / 25.4), y: Pt(pdf_y * 72.0 / 25.4) },
                    });
                    ops.push(Op::ShowText {
                        items: vec![TextItem::Text(ob.text.clone())],
                    });
                }
                ops.push(Op::EndTextSection);
            }
        }

        doc.pages.push(PdfPage::new(Mm(mm_w), Mm(mm_h), ops));
    }

    // Save
    let desktop = desktop_path();
    let path = desktop.join(timestamped_name("pdf"));
    let opts = PdfSaveOptions::default();
    let mut warnings = Vec::new();
    let pdf_bytes = doc.save(&opts, &mut warnings);
    match std::fs::write(&path, &pdf_bytes) {
        Ok(_) => { eprintln!("[pdf] Saved to {}", path.display()); Some(path.to_string_lossy().to_string()) }
        Err(e) => { eprintln!("[pdf] Save failed: {e}"); None }
    }
}

fn load_cjk_font(doc: &mut PdfDocument) -> Option<FontId> {
    for path in &[
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/System/Library/Fonts/Helvetica.ttc",
    ] {
        if !std::path::Path::new(path).exists() { continue; }
        if let Ok(bytes) = std::fs::read(path) {
            let mut warns = Vec::new();
            if let Some(parsed) = ParsedFont::from_bytes(&bytes, 0, &mut warns) {
                eprintln!("[pdf] Font: {path}");
                return Some(doc.add_font(&parsed));
            }
        }
    }
    None
}

fn desktop_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    { PathBuf::from(std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string())).join("Desktop") }
    #[cfg(not(target_os = "windows"))]
    { PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string())).join("Desktop") }
}

fn timestamped_name(ext: &str) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let secs = now.as_secs();
    let s = secs % 60; let m = (secs / 60) % 60; let h = (secs / 3600 + 8) % 24;
    let days = secs / 86400; let y = 1970 + days / 365; let d = days % 365;
    format!("glaspen2_{:04}-{:03}_{:02}-{:02}-{:02}.{}", y, d, h, m, s, ext)
}
