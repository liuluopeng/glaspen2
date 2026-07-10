//! PDF export — render strokes as vector paths + invisible selectable text.
//! Uses printpdf for both vector rendering and text overlay.

use std::path::PathBuf;

use printpdf::*;

use crate::{db, ocr, runtime};

/// Export all pages to a vector PDF on the desktop. Returns the file path.
pub fn export_all_pages() -> Option<String> {
    let rt = runtime();

    let db_path = std::env::var("GLASPEN2_DB")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| db::db_path());

    // Open DB pool
    let pool = rt.block_on(async {
        sqlx::SqlitePool::connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&db_path)
                .read_only(true),
        ).await
    });
    let pool = match pool { Ok(p) => p, Err(e) => { eprintln!("[pdf] DB: {e}"); return None; } };

    // Collect all screens
    let screens: Vec<(i64, i32, i32)> = rt.block_on(async {
        sqlx::query_as(
            "SELECT s.id, s.screen_w, s.screen_h FROM screens s \
             WHERE EXISTS (SELECT 1 FROM strokes WHERE screen_id = s.id) \
             ORDER BY s.id"
        ).fetch_all(&pool).await.unwrap_or_default()
    });

    if screens.is_empty() { eprintln!("[pdf] No pages"); return None; }
    eprintln!("[pdf] Exporting {} pages", screens.len());

    let mut doc = PdfDocument::new("glaspen2");
    let font_id = load_cjk_font(&mut doc);

    for (screen_id, sw, sh) in &screens {
        // Load strokes directly (not via db::strokes_for_screen which uses the app's global pool)
        let strokes: Vec<db::StrokeData> = rt.block_on(async {
            let rows: Vec<(i64, f64, f64, f64, f64)> = sqlx::query_as(
                "SELECT id, color_r, color_g, color_b, width_scale FROM strokes WHERE screen_id = ?1 ORDER BY id"
            ).bind(screen_id).fetch_all(&pool).await.unwrap_or_default();
            let mut result = Vec::new();
            for (sid, r, g, b, ws) in rows {
                let pts: Vec<(f64,f64,f64,f64)> = sqlx::query_as(
                    "SELECT x, y, width, t FROM points WHERE stroke_id = ?1 ORDER BY seq"
                ).bind(sid).fetch_all(&pool).await.unwrap_or_default();
                result.push(db::StrokeData { r, g, b, width_scale: ws, points: pts });
            }
            result
        });
        eprintln!("[pdf] Page {}: {}x{} ({} strokes)", screen_id, sw, sh, strokes.len());

        // Page dimensions in mm (72 pt/inch → 25.4 mm/inch)
        let mm_w = *sw as f32 * 25.4 / 72.0;
        let mm_h = *sh as f32 * 25.4 / 72.0;

        let mut ops: Vec<Op> = Vec::new();

        // Render each stroke as vector paths
        for s in &strokes {
            let pts = &s.points;
            if pts.len() < 2 { continue; }

            // Convert color to PDF Rgb
            let color = Color::Rgb(Rgb {
                r: s.r as f32, g: s.g as f32, b: s.b as f32,
                icc_profile: None,
            });

            for i in 0..pts.len() {
                let (x, y, w, _t) = pts[i];
                // PDF origin is bottom-left
                let px = Pt(x as f32);
                let py = Pt(*sh as f32 - y as f32);

                if i == 0 {
                    // First point: filled circle (approximated with horizontal line w=0)
                    ops.push(Op::SaveGraphicsState);
                    ops.push(Op::SetFillColor { col: color.clone() });
                    ops.push(Op::SetOutlineThickness { pt: Pt(w as f32) });
                    ops.push(Op::SetLineCapStyle { cap: LineCapStyle::Round });
                    // Draw a short line segment at the same point → round cap creates a dot
                    ops.push(Op::DrawLine {
                        line: Line {
                            points: vec![
                                LinePoint { p: Point { x: px, y: py }, bezier: false },
                                LinePoint { p: Point { x: px, y: py }, bezier: false },
                            ],
                            is_closed: false,
                        },
                    });
                    ops.push(Op::RestoreGraphicsState);
                } else {
                    // Line segment
                    let (px_prev, py_prev, _pw, _pt) = pts[i - 1];
                    let ppx = Pt(px_prev as f32);
                    let ppy = Pt(*sh as f32 - py_prev as f32);

                    ops.push(Op::SaveGraphicsState);
                    ops.push(Op::SetOutlineColor { col: color.clone() });
                    ops.push(Op::SetOutlineThickness { pt: Pt(w as f32) });
                    ops.push(Op::SetLineCapStyle { cap: LineCapStyle::Round });
                    ops.push(Op::SetLineJoinStyle { join: LineJoinStyle::Round });
                    ops.push(Op::DrawLine {
                        line: Line {
                            points: vec![
                                LinePoint { p: Point { x: ppx, y: ppy }, bezier: false },
                                LinePoint { p: Point { x: px, y: py }, bezier: false },
                            ],
                            is_closed: false,
                        },
                    });
                    ops.push(Op::RestoreGraphicsState);
                }
            }
        }

        // Load OCR data from DB
        let ocr_boxes: Vec<db::OcrBox> = rt.block_on(async {
            let row = sqlx::query_as::<_, (i64, String, f64)>(
                "SELECT id, full_text, created_at FROM ocr_results WHERE screen_id = ?1 ORDER BY id DESC LIMIT 1"
            ).bind(screen_id).fetch_optional(&pool).await.ok().flatten();
            let Some((rid, _full_text, _created_at)) = row else { return Vec::new() };
            let boxes: Vec<(i64, String, f64, f64, f64, f64, f64)> = sqlx::query_as(
                "SELECT box_index, text, x, y, w, h, confidence FROM ocr_boxes WHERE result_id = ?1 ORDER BY box_index"
            ).bind(rid).fetch_all(&pool).await.unwrap_or_default();
            boxes.into_iter().map(|(_bi, t, x, y, w, h, c)| {
                db::OcrBox { text: t, x, y, w, h, confidence: c as f32 }
            }).collect()
        });
        if !ocr_boxes.is_empty() {
            eprintln!("[pdf] Page {}: {} OCR chars", screen_id, ocr_boxes.len());
        }

        // Add invisible selectable text — group per-character boxes into lines
        if !ocr_boxes.is_empty() {
            // Group by y-position (line tolerance = half average char height)
            let avg_h: f32 = ocr_boxes.iter().map(|b| b.h as f32).sum::<f32>() / ocr_boxes.len() as f32;
            let line_tol = (avg_h * 1.5).max(8.0);

            let mut lines: Vec<Vec<&db::OcrBox>> = Vec::new();
            for ob in &ocr_boxes {
                let ob_y = ob.y as f32;
                let mut placed = false;
                for line in &mut lines {
                    let first_y = line[0].y as f32;
                    if (ob_y - first_y).abs() < line_tol {
                        line.push(ob);
                        placed = true;
                        break;
                    }
                }
                if !placed {
                    lines.push(vec![ob]);
                }
            }

            // Sort each line by x, then build text
            ops.push(Op::StartTextSection);
            for line in &lines {
                let mut sorted = line.clone();
                sorted.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());

                let line_text: String = sorted.iter().map(|b| b.text.clone()).collect::<Vec<_>>().join("");
                if line_text.is_empty() { continue; }

                let avg_y: f32 = sorted.iter().map(|b| b.y as f32).sum::<f32>() / sorted.len() as f32;
                let min_x = sorted.iter().map(|b| b.x as f32).reduce(f32::min).unwrap_or(0.0);
                let avg_h_line: f32 = sorted.iter().map(|b| b.h as f32).sum::<f32>() / sorted.len() as f32;

                let pdf_x = min_x;
                let pdf_y = *sh as f32 - avg_y - avg_h_line;
                let font_size = Pt(avg_h_line.max(4.0));

                let pdf_font = match font_id {
                    Some(ref fid) => PdfFontHandle::External(fid.clone()),
                    None => PdfFontHandle::Builtin(BuiltinFont::Helvetica),
                };
                ops.push(Op::SetFont { font: pdf_font, size: font_size });
                ops.push(Op::SetFillColor {
                    col: Color::Rgb(Rgb { r: 1.0, g: 1.0, b: 1.0, icc_profile: None }),
                });
                ops.push(Op::SetTextCursor {
                    pos: Point { x: Pt(pdf_x), y: Pt(pdf_y) },
                });
                ops.push(Op::ShowText {
                    items: vec![TextItem::Text(line_text)],
                });
            }
            ops.push(Op::EndTextSection);
        }

        doc.pages.push(PdfPage::new(Mm(mm_w), Mm(mm_h), ops));
    }

    // Close DB
    rt.block_on(pool.close());

    // Save
    let desktop = desktop_path();
    let path = desktop.join(timestamped_name("pdf"));
    let opts = PdfSaveOptions::default();
    let mut warnings = Vec::new();
    let pdf_bytes = doc.save(&opts, &mut warnings);
    match std::fs::write(&path, &pdf_bytes) {
        Ok(_) => {
            eprintln!("[pdf] Saved vector PDF to {}", path.display());
            Some(path.to_string_lossy().to_string())
        }
        Err(e) => { eprintln!("[pdf] Save failed: {e}"); None }
    }
}

/// Render strokes to a temporary image, OCR, return boxes + save to DB.
fn render_and_ocr(
    strokes: &[db::StrokeData], sw: i32, sh: i32, screen_id: i64,
    rt: &tokio::runtime::Runtime,
) -> Vec<db::OcrBox> {
    // Create a small surface for OCR (downscale for speed)
    use crate::cairo;
    let scale = 0.5f64;
    let rw = (sw as f64 * scale).ceil() as i32;
    let rh = (sh as f64 * scale).ceil() as i32;
    let mut surface = match cairo::ImageSurface::create(cairo::Format::ARgb32, rw, rh) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    {
        let Ok(cr) = cairo::Context::new(&surface) else { return Vec::new() };
        cr.set_operator(cairo::Operator::Clear);
        let _ = cr.paint();
        cr.set_operator(cairo::Operator::Over);
        cr.scale(scale, scale);
        cr.set_line_cap(cairo::LineCap::Round);
        cr.set_line_join(cairo::LineJoin::Round);
        for s in strokes {
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

    // Read RGBA pixels
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

    // Detect + recognize
    let boxes = ocr::det::detect_text_regions(&rgba, surf_w, surf_h);
    let mut ocr_boxes = Vec::new();
    let mut full_text = String::new();

    for (i, tb) in boxes.iter().enumerate() {
        let pad = 4u32;
        let cx = tb.x.saturating_sub(pad);
        let cy = tb.y.saturating_sub(pad);
        let cw = (tb.w + pad * 2).min(surf_w - cx);
        let ch = (tb.h + pad * 2).min(surf_h - cy);
        if cw < 4 || ch < 4 { continue; }
        let crop = ocr::det::crop_pixels(&rgba, surf_w, cx, cy, cw, ch);
        let text = ocr::rec::recognize(&crop, cw, ch);
        if !text.is_empty() {
            if i > 0 { full_text.push('\n'); }
            full_text.push_str(&text);

            // Scale box coordinates back to full resolution
            let inv = 1.0 / scale;
            let chars: Vec<char> = text.chars().collect();
            if !chars.is_empty() {
                let char_w = tb.w as f64 / chars.len() as f64;
                for (ci, ch) in chars.iter().enumerate() {
                    ocr_boxes.push(db::OcrBox {
                        text: ch.to_string(),
                        x: (tb.x as f64 + char_w * ci as f64) * inv,
                        y: (tb.y as f64) * inv,
                        w: char_w * inv,
                        h: tb.h as f64 * inv,
                        confidence: 0.0,
                    });
                }
            }
        }
    }

    if !full_text.is_empty() {
        rt.block_on(db::save_ocr_result(screen_id, &full_text, &ocr_boxes));
        let truncated: String = full_text.chars().take(60).collect();
        eprintln!("[pdf] OCR page {}: {:?}", screen_id, truncated);
    }

    ocr_boxes
}

/// Backfill OCR data for all pages that don't have OCR results yet.
/// Set GLASPEN2_DB env var to override DB path (for tests).
pub fn backfill_ocr_all_pages() {
    let rt = runtime();

    let path = std::env::var("GLASPEN2_DB")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| db::db_path());
    let screens: Vec<(i64, i32, i32)> = {
        let pool = rt.block_on(async {
            sqlx::SqlitePool::connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(&path)
                    .read_only(true),
            ).await
        });
        let pool = match pool {
            Ok(p) => p,
            Err(e) => { eprintln!("[backfill] DB: {e}"); return; }
        };
        let rows = rt.block_on(async {
            sqlx::query_as(
                "SELECT s.id, s.screen_w, s.screen_h FROM screens s \
                 WHERE EXISTS (SELECT 1 FROM strokes WHERE screen_id = s.id) \
                 AND NOT EXISTS (SELECT 1 FROM ocr_results WHERE screen_id = s.id) \
                 ORDER BY s.id"
            ).fetch_all(&pool).await.unwrap_or_default()
        });
        let _ = rt.block_on(pool.close());
        rows
    };

    if screens.is_empty() {
        eprintln!("[backfill] All pages already have OCR data");
        return;
    }

    eprintln!("[backfill] Backfilling OCR for {} pages", screens.len());
    for (screen_id, sw, sh) in &screens {
        eprintln!("[backfill] Page {}: {}x{}", screen_id, sw, sh);
        let strokes = rt.block_on(db::strokes_for_screen(*screen_id));
        if strokes.is_empty() { continue; }
        render_and_ocr(&strokes, *sw, *sh, *screen_id, rt);
    }
    eprintln!("[backfill] Done");
}

fn load_cjk_font(doc: &mut PdfDocument) -> Option<FontId> {
    for path in &[
        "/System/Library/Fonts/STHeiti Medium.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Run: cargo test pdf::tests::test_backfill -- --nocapture
    #[test]
    fn test_backfill() {
        backfill_ocr_all_pages();
    }

    /// Generate a minimal test PDF and dump its structure.
    /// Run: cargo test pdf::tests::test_generate_minimal -- --nocapture
    #[test]
    fn test_generate_minimal() {
        use printpdf::*;
        let mut doc = PdfDocument::new("test");

        let font_path = "/System/Library/Fonts/Helvetica.ttc";
        let bytes = std::fs::read(font_path).unwrap();
        let mut warns = Vec::new();
        let parsed = ParsedFont::from_bytes(&bytes, 0, &mut warns).unwrap();
        let fid = doc.add_font(&parsed);
        eprintln!("[test] font loaded");

        let mut ops = Vec::new();
        ops.push(Op::StartTextSection);
        ops.push(Op::SetFont {
            font: PdfFontHandle::External(fid),
            size: Pt(12.0),
        });
        ops.push(Op::SetTextRenderingMode {
            mode: TextRenderingMode::Invisible,
        });
        ops.push(Op::SetTextCursor {
            pos: Point { x: Pt(50.0), y: Pt(50.0) },
        });
        ops.push(Op::ShowText {
            items: vec![TextItem::Text("HelloWorld".to_string())],
        });
        ops.push(Op::EndTextSection);

        doc.pages.push(PdfPage::new(Mm(210.0), Mm(297.0), ops));

        let path = std::path::Path::new("/tmp/test_invisible.pdf");
        let opts = PdfSaveOptions::default();
        let mut warnings = Vec::new();
        let bytes = doc.save(&opts, &mut warnings);
        std::fs::write(path, &bytes).unwrap();
        eprintln!("[test] Wrote {}", path.display());

        // Now generate using our actual export function
        let _ = export_all_pages();
    }
}
