//! PDF export — render strokes as vector paths.
//! Uses printpdf for vector rendering, lopdf for saving.

use std::path::PathBuf;

use printpdf::*;

use crate::{db, runtime};

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
        )
        .await
    });
    let pool = match pool {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[pdf] DB: {e}");
            return None;
        }
    };

    // Collect all screens
    let screens: Vec<(i64, i32, i32)> = rt.block_on(async {
        sqlx::query_as(
            "SELECT s.id, s.screen_w, s.screen_h FROM screens s \
             WHERE EXISTS (SELECT 1 FROM strokes WHERE screen_id = s.id) \
             ORDER BY s.id",
        )
        .fetch_all(&pool)
        .await
        .unwrap_or_default()
    });

    if screens.is_empty() {
        eprintln!("[pdf] No pages");
        return None;
    }
    eprintln!("[pdf] Exporting {} pages", screens.len());

    let mut doc = PdfDocument::new("glaspen2");

    for (screen_id, sw, sh) in &screens {
        // Load strokes directly (single JOIN query, no N+1)
        let strokes: Vec<db::StrokeData> = rt.block_on(db::strokes_for_screen(*screen_id));
        eprintln!(
            "[pdf] Page {}: {}x{} ({} strokes)",
            screen_id,
            sw,
            sh,
            strokes.len()
        );

        // Page dimensions in mm (72 pt/inch → 25.4 mm/inch)
        let mm_w = *sw as f32 * 25.4 / 72.0;
        let mm_h = *sh as f32 * 25.4 / 72.0;

        let mut ops: Vec<Op> = Vec::new();
        for s in &strokes {
            push_stroke(&mut ops, s, 0.0, 0.0, *sh as f64);
        }

        doc.pages.push(PdfPage::new(Mm(mm_w), Mm(mm_h), ops));
    }

    // Close DB
    rt.block_on(pool.close());

    let opts = PdfSaveOptions::default();
    let mut warnings = Vec::new();
    let mut lopdf_doc = doc.to_lopdf_document(&opts, &mut warnings);

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

/// Export the single infinite canvas to a PDF, split into screen-sized pages.
///
/// The canvas content is tiled into a grid of `page_w` x `page_h` (points)
/// rectangles; each tile becomes one PDF page. Strokes that cross a tile edge
/// are drawn on every tile they touch and clipped by the page's MediaBox.
/// Returns the file path, or None when there is nothing to export / too many
/// pages would be produced.
pub fn export_infinite_paged(page_w: i32, page_h: i32) -> Option<String> {
    let rt = runtime();

    let db_path = std::env::var("GLASPEN2_DB")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| db::db_path());
    let pool = rt.block_on(async {
        sqlx::SqlitePool::connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&db_path)
                .read_only(true),
        )
        .await
    });
    let pool = match pool {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[pdf] DB: {e}");
            return None;
        }
    };
    let strokes = rt.block_on(db::load_infinite_strokes_with(&pool));
    rt.block_on(pool.close());

    if strokes.is_empty() {
        eprintln!("[pdf] Infinite canvas is empty");
        return None;
    }

    // Content bounding box.
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for s in &strokes {
        for &(x, y, w, _) in &s.points {
            let r = w * 0.5;
            x0 = x0.min(x - r);
            y0 = y0.min(y - r);
            x1 = x1.max(x + r);
            y1 = y1.max(y + r);
        }
    }
    if x0 > x1 || y0 > y1 {
        return None;
    }
    let pad = 10.0;
    x0 -= pad;
    y0 -= pad;
    x1 += pad;
    y1 += pad;

    let pw = page_w.max(64) as f64;
    let ph = page_h.max(64) as f64;
    let cols = (((x1 - x0) / pw).ceil() as i64).max(1);
    let rows = (((y1 - y0) / ph).ceil() as i64).max(1);
    const MAX_PAGES: i64 = 400;
    if cols * rows > MAX_PAGES {
        eprintln!(
            "[pdf] Infinite canvas needs {} pages (> {}) — reduce content or page size",
            cols * rows,
            MAX_PAGES
        );
        return None;
    }
    eprintln!(
        "[pdf] Infinite canvas: {} strokes, bbox {:.0}x{:.0}, {}x{} = {} pages",
        strokes.len(),
        x1 - x0,
        y1 - y0,
        cols,
        rows,
        cols * rows
    );

    let mm_w = pw as f32 * 25.4 / 72.0;
    let mm_h = ph as f32 * 25.4 / 72.0;
    let mut doc = PdfDocument::new("glaspen2-infinite");

    for row in 0..rows {
        for col in 0..cols {
            let ox = x0 + col as f64 * pw;
            let oy = y0 + row as f64 * ph;
            let mut ops: Vec<Op> = Vec::new();
            for s in &strokes {
                if stroke_intersects_rect(s, ox, oy, pw, ph) {
                    push_stroke(&mut ops, s, ox, oy, ph);
                }
            }
            doc.pages.push(PdfPage::new(Mm(mm_w), Mm(mm_h), ops));
        }
    }

    let opts = PdfSaveOptions::default();
    let mut warnings = Vec::new();
    let mut lopdf_doc = doc.to_lopdf_document(&opts, &mut warnings);
    let path = desktop_path().join(timestamped_name("pdf"));
    let mut pdf_bytes = Vec::new();
    match lopdf_doc.save_to(&mut pdf_bytes) {
        Ok(()) => {
            std::fs::write(&path, &pdf_bytes).ok();
            if path.exists() {
                eprintln!("[pdf] Saved infinite-canvas PDF to {}", path.display());
                return Some(path.to_string_lossy().to_string());
            }
        }
        Err(e) => eprintln!("[pdf] Save error: {e}"),
    }
    None
}

/// Append one stroke's vector ops. Canvas (x,y) maps to page ((x-ox), page_h-(y-oy)).
fn push_stroke(ops: &mut Vec<Op>, s: &db::StrokeData, ox: f64, oy: f64, page_h: f64) {
    let pts = &s.points;
    if pts.len() < 2 {
        return;
    }
    let color = Color::Rgb(Rgb {
        r: s.r as f32,
        g: s.g as f32,
        b: s.b as f32,
        icc_profile: None,
    });
    let map = |x: f64, y: f64| (Pt((x - ox) as f32), Pt((page_h - (y - oy)) as f32));

    for i in 0..pts.len() {
        let (x, y, w, _t) = pts[i];
        let (px, py) = map(x, y);
        if i == 0 {
            // First point: round dot (zero-length line with round cap).
            ops.push(Op::SaveGraphicsState);
            ops.push(Op::SetFillColor { col: color.clone() });
            ops.push(Op::SetOutlineThickness { pt: Pt(w as f32) });
            ops.push(Op::SetLineCapStyle {
                cap: LineCapStyle::Round,
            });
            ops.push(Op::DrawLine {
                line: Line {
                    points: vec![
                        LinePoint {
                            p: Point { x: px, y: py },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point { x: px, y: py },
                            bezier: false,
                        },
                    ],
                    is_closed: false,
                },
            });
            ops.push(Op::RestoreGraphicsState);
        } else {
            let (px_prev, py_prev, _pw, _pt) = pts[i - 1];
            let (ppx, ppy) = map(px_prev, py_prev);
            ops.push(Op::SaveGraphicsState);
            ops.push(Op::SetOutlineColor { col: color.clone() });
            ops.push(Op::SetOutlineThickness { pt: Pt(w as f32) });
            ops.push(Op::SetLineCapStyle {
                cap: LineCapStyle::Round,
            });
            ops.push(Op::SetLineJoinStyle {
                join: LineJoinStyle::Round,
            });
            ops.push(Op::DrawLine {
                line: Line {
                    points: vec![
                        LinePoint {
                            p: Point { x: ppx, y: ppy },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point { x: px, y: py },
                            bezier: false,
                        },
                    ],
                    is_closed: false,
                },
            });
            ops.push(Op::RestoreGraphicsState);
        }
    }
}

/// Whether a stroke's bounding box (grown by its widest point) overlaps a tile.
fn stroke_intersects_rect(s: &db::StrokeData, ox: f64, oy: f64, w: f64, h: f64) -> bool {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    let mut max_w: f64 = 0.0;
    for &(x, y, pw, _) in &s.points {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
        max_w = max_w.max(pw);
    }
    if x0 > x1 {
        return false;
    }
    let r = max_w * 0.5;
    x1 + r >= ox && x0 - r <= ox + w && y1 + r >= oy && y0 - r <= oy + h
}

fn desktop_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string()))
            .join("Desktop")
    }
    #[cfg(not(target_os = "windows"))]
    {
        PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string())).join("Desktop")
    }
}

fn timestamped_name(ext: &str) -> String {
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
    format!(
        "glaspen2_{:04}-{:03}_{:02}-{:02}-{:02}.{}",
        y, d, h, m, s, ext
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Generate PDF (alias for export_all_pages)
    #[test]
    fn test_export_pdf() {
        export_all_pages();
    }
}
