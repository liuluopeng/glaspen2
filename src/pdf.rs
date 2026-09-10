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

        // Render each stroke as vector paths
        for s in &strokes {
            let pts = &s.points;
            if pts.len() < 2 {
                continue;
            }

            let color = Color::Rgb(Rgb {
                r: s.r as f32,
                g: s.g as f32,
                b: s.b as f32,
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
                    let ppx = Pt(px_prev as f32);
                    let ppy = Pt(*sh as f32 - py_prev as f32);

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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
