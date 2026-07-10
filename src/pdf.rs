//! PDF export — render strokes as vector paths + invisible selectable text.
//! Uses printpdf for vector rendering, lopdf post-processing for glyphless CID font text layer.

use std::path::PathBuf;

use printpdf::*;
use lopdf::{self, Dictionary, Object, Stream};
use lopdf::dictionary;

use crate::{db, ocr, runtime};

/// Embedded glyphless TTF — all glyphs empty, CID = Unicode codepoint.
const GLYPHLESS_TTF: &[u8] = include_bytes!("../models/glyphless.ttf");

/// Identity ToUnicode CMap — maps CID 0x0000..0xFFFF → Unicode U+0000..U+FFFF.
const IDENTITY_CMAP: &[u8] = b"/CIDInit /ProcSet findresource begin
12 dict begin
begincmap
/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def
/CMapName /Adobe-Identity-UCS def
/CMapType 2 def
1 begincodespacerange
<0000><FFFF>
endcodespacerange
1 beginbfrange
<0000><FFFF><0000>
endbfrange
endcmap
CMapName currentdict /CMap defineresource pop
end
end";

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

    // Store OCR data per page for lopdf post-processing: (screen_w, screen_h, ocr_boxes)
    let mut pages_ocr: Vec<(i32, i32, Vec<db::OcrBox>)> = Vec::new();

    for (screen_id, sw, sh) in &screens {
        // Load strokes directly
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

            let color = Color::Rgb(Rgb {
                r: s.r as f32, g: s.g as f32, b: s.b as f32,
                icc_profile: None,
            });

            for i in 0..pts.len() {
                let (x, y, w, _t) = pts[i];
                let px = Pt(x as f32);
                let py = Pt(*sh as f32 - y as f32);

                if i == 0 {
                    ops.push(Op::SaveGraphicsState);
                    ops.push(Op::SetFillColor { col: color.clone() });
                    ops.push(Op::SetOutlineThickness { pt: Pt(w as f32) });
                    ops.push(Op::SetLineCapStyle { cap: LineCapStyle::Round });
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

        // Load OCR data from DB (store for lopdf post-processing, no text via printpdf)
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

        pages_ocr.push((*sw, *sh, ocr_boxes));
        doc.pages.push(PdfPage::new(Mm(mm_w), Mm(mm_h), ops));
    }

    // Close DB
    rt.block_on(pool.close());

    // Convert to lopdf for glyphless CID font text layer
    let opts = PdfSaveOptions::default();
    let mut warnings = Vec::new();
    let mut lopdf_doc = doc.to_lopdf_document(&opts, &mut warnings);

    // Post-process: add glyphless CID font + invisible Unicode text
    add_glyphless_text_layer(&mut lopdf_doc, &pages_ocr);

    // Save with lopdf
    let desktop = desktop_path();
    let path = desktop.join(timestamped_name("pdf"));
    let mut pdf_bytes = Vec::new();
    match lopdf_doc.save_to(&mut pdf_bytes) {
        Ok(()) => {
            std::fs::write(&path, &pdf_bytes).ok();
            if path.exists() {
                eprintln!("[pdf] Saved vector PDF to {}", path.display());
                return Some(path.to_string_lossy().to_string());
            }
        }
        Err(e) => eprintln!("[pdf] Save error: {e}"),
    }
    None
}

// ---------------------------------------------------------------------------
// lopdf post-processing: glyphless CID font + invisible Unicode text layer
// ---------------------------------------------------------------------------

/// Add glyphless CID font + invisible text layer to all pages with OCR data.
fn add_glyphless_text_layer(
    doc: &mut lopdf::Document,
    pages_ocr: &[(i32, i32, Vec<db::OcrBox>)],
) {
    // 1. Add font infrastructure objects
    let type0_font_id = add_glyphless_font_objects(doc);
    let font_name = b"C1";

    // 2. Collect page IDs (must collect before mutating)
    let page_ids: Vec<lopdf::ObjectId> = doc.get_pages().into_values().collect();

    for (page_num, page_id) in page_ids.iter().enumerate() {
        if page_num >= pages_ocr.len() { break; }
        let (sw, sh, ocr_boxes) = &pages_ocr[page_num];
        if ocr_boxes.is_empty() { continue; }

        // Build text content stream
        let text_content = build_text_content(*sw, *sh, ocr_boxes);

        // Get existing content bytes
        let old_content = get_page_content_bytes(doc, *page_id);

        // Combine: text ops first (underneath vector strokes), then original content
        let mut new_content = text_content;
        new_content.extend_from_slice(&old_content);

        // Create new content stream
        let new_stream_id = doc.add_object(Stream::new(Dictionary::new(), new_content));

        // Add font to page's Resources
        add_font_to_page_resources(doc, *page_id, font_name, type0_font_id);

        // Update page's Contents reference
        if let Ok(page_dict) = doc.get_dictionary_mut(*page_id) {
            page_dict.set(b"Contents", Object::Reference(new_stream_id));
        }
    }
}

/// Add the glyphless font infrastructure to the lopdf document.
/// Returns the Type0 font object ID.
fn add_glyphless_font_objects(doc: &mut lopdf::Document) -> lopdf::ObjectId {
    // 1. Font file stream (glyphless.ttf)
    let font_stream_id = doc.add_object(Stream::new(
        {
            let mut d = Dictionary::new();
            d.set("Length", GLYPHLESS_TTF.len() as i64);
            d
        },
        GLYPHLESS_TTF.to_vec(),
    ));

    // 2. Font descriptor
    let font_desc_id = doc.add_object(dictionary! {
        b"Type" => Object::Name(b"FontDescriptor".to_vec()),
        b"FontName" => Object::Name(b"GLYPHLESS+GlyphLessFont".to_vec()),
        b"Flags" => Object::Integer(4),
        b"FontBBox" => Object::Array(vec![
            Object::Integer(0), Object::Integer(0),
            Object::Integer(0), Object::Integer(0),
        ]),
        b"ItalicAngle" => Object::Integer(0),
        b"Ascent" => Object::Integer(0),
        b"Descent" => Object::Integer(0),
        b"CapHeight" => Object::Integer(0),
        b"StemV" => Object::Integer(0),
        b"FontFile2" => Object::Reference(font_stream_id),
    });

    // 3. CIDFontType2 descendant
    let mut cid_system_info = Dictionary::new();
    cid_system_info.set(b"Registry", Object::string_literal("Adobe"));
    cid_system_info.set(b"Ordering", Object::string_literal("Identity"));
    cid_system_info.set(b"Supplement", Object::Integer(0));

    let cid_font_id = doc.add_object(dictionary! {
        b"Type" => Object::Name(b"Font".to_vec()),
        b"Subtype" => Object::Name(b"CIDFontType2".to_vec()),
        b"BaseFont" => Object::Name(b"GLYPHLESS+GlyphLessFont".to_vec()),
        b"CIDSystemInfo" => Object::Dictionary(cid_system_info),
        b"DW" => Object::Integer(1000),
        b"FontDescriptor" => Object::Reference(font_desc_id),
    });

    // 4. ToUnicode CMap stream
    let cmap_stream_id = doc.add_object(Stream::new(
        Dictionary::new(),
        IDENTITY_CMAP.to_vec(),
    ));

    // 5. Type0 font (root font object)
    doc.add_object(dictionary! {
        b"Type" => Object::Name(b"Font".to_vec()),
        b"Subtype" => Object::Name(b"Type0".to_vec()),
        b"BaseFont" => Object::Name(b"GLYPHLESS+GlyphLessFont-Identity-H".to_vec()),
        b"Encoding" => Object::Name(b"Identity-H".to_vec()),
        b"DescendantFonts" => Object::Array(vec![Object::Reference(cid_font_id)]),
        b"ToUnicode" => Object::Reference(cmap_stream_id),
    })
}

/// Build PDF content stream bytes for invisible text using the glyphless CID font.
/// Text rendering mode 3 (invisible) — text is selectable/searchable but not visible.
fn build_text_content(_sw: i32, sh: i32, ocr_boxes: &[db::OcrBox]) -> Vec<u8> {
    if ocr_boxes.is_empty() {
        return Vec::new();
    }

    // Group per-character boxes into lines by y-position
    let avg_h: f32 = ocr_boxes.iter().map(|b| b.h as f32).sum::<f32>() / ocr_boxes.len() as f32;
    let line_tol = (avg_h * 1.5).max(8.0);

    let mut lines: Vec<Vec<&db::OcrBox>> = Vec::new();
    for ob in ocr_boxes {
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

    let mut content = Vec::new();

    // q = save graphics state; BT = begin text; 3 Tr = invisible text
    content.extend_from_slice(b"q\nBT\n3 Tr\n");

    for line in &lines {
        let mut sorted: Vec<&&db::OcrBox> = line.iter().collect();
        sorted.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());

        let line_text: String = sorted.iter().map(|b| b.text.clone()).collect::<Vec<_>>().join("");
        if line_text.is_empty() { continue; }

        let avg_y: f32 = sorted.iter().map(|b| b.y as f32).sum::<f32>() / sorted.len() as f32;
        let min_x = sorted.iter().map(|b| b.x as f32).reduce(f32::min).unwrap_or(0.0);
        let avg_h_line: f32 = sorted.iter().map(|b| b.h as f32).sum::<f32>() / sorted.len() as f32;

        let pdf_x = min_x;
        let pdf_y = sh as f32 - avg_y - avg_h_line;
        let font_size = avg_h_line.max(4.0);

        // Build hex-encoded CID string (UTF-16BE hex = Unicode codepoints)
        let hex_cids: String = line_text
            .encode_utf16()
            .map(|cp| format!("{:04X}", cp))
            .collect::<Vec<_>>()
            .join("");

        // /C1 font_size Tf  —  select font
        // 1 0 0 1 x y Tm  —  set absolute text matrix
        // <hex> Tj          —  show text
        use std::io::Write;
        let _ = write!(&mut content, "/C1 {font_size:.1} Tf\n");
        let _ = write!(&mut content, "1 0 0 1 {pdf_x:.1} {pdf_y:.1} Tm\n");
        let _ = write!(&mut content, "<{hex_cids}> Tj\n");
    }

    // ET = end text; Q = restore graphics state
    content.extend_from_slice(b"ET\nQ\n");
    content
}

/// Extract the decoded content bytes from a page's /Contents stream(s).
fn get_page_content_bytes(doc: &lopdf::Document, page_id: lopdf::ObjectId) -> Vec<u8> {
    let page_dict = match doc.get_dictionary(page_id) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    let contents = match page_dict.get(b"Contents") {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    match contents {
        Object::Reference(stream_id) => {
            get_stream_bytes(doc, *stream_id).unwrap_or_default()
        }
        Object::Array(refs) => {
            let mut result = Vec::new();
            for obj in refs {
                if let Ok(stream_id) = obj.as_reference() {
                    if let Ok(content) = get_stream_bytes(doc, stream_id) {
                        result.extend_from_slice(&content);
                        result.push(b'\n');
                    }
                }
            }
            result
        }
        _ => Vec::new(),
    }
}

/// Get decoded content from a stream object.
fn get_stream_bytes(doc: &lopdf::Document, stream_id: lopdf::ObjectId) -> lopdf::Result<Vec<u8>> {
    let obj = doc.get_object(stream_id)?;
    obj.as_stream()?.get_plain_content()
}

/// Add /C1 font reference to a page's Resources dictionary.
fn add_font_to_page_resources(
    doc: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
    font_name: &[u8],
    font_id: lopdf::ObjectId,
) {
    // Get the Resources reference from the page
    let res_id = {
        let page_dict = match doc.get_dictionary(page_id) {
            Ok(d) => d,
            Err(_) => return,
        };
        match page_dict.get(b"Resources") {
            Ok(Object::Reference(id)) => *id,
            _ => return,
        }
    };

    // Get Resources dict and add font
    let res_dict = match doc.get_dictionary_mut(res_id) {
        Ok(d) => d,
        Err(_) => return,
    };

    // /Font may be a dict or a reference to a dict. Handle inline dict.
    match res_dict.get_mut(b"Font") {
        Ok(Object::Dictionary(font_dict)) => {
            font_dict.set(font_name, Object::Reference(font_id));
        }
        _ => {
            // No /Font entry yet — create one
            let mut font_dict = Dictionary::new();
            font_dict.set(font_name, Object::Reference(font_id));
            res_dict.set(b"Font", Object::Dictionary(font_dict));
        }
    }
}

// ---------------------------------------------------------------------------
// OCR + backfill (unchanged)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backfill() {
        backfill_ocr_all_pages();
    }

    /// Generate PDF (alias for export_all_pages)
    #[test]
    fn test_export_pdf() {
        export_all_pages();
    }
}
