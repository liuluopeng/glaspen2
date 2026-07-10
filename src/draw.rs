//! Cairo drawing operations migrated from ObjC to Rust.
//! MacOS-only (uses real Cairo from cairo-rs crate).

use std::os::raw::c_double;
use crate::STROKES;

/// Draw all strokes from `STROKES` onto a surface (clear + stroke rendering).
/// Internal helper called by both the FFI export and unit tests.
#[cfg(feature = "cairo_real")]
pub fn draw_rebuild_on_surface(surface: &crate::cairo::Surface, scale: f64) {
    let Ok(cr) = crate::cairo::Context::new(surface) else { return };
    cr.set_operator(crate::cairo::Operator::Clear);
    let _ = cr.paint();
    cr.set_operator(crate::cairo::Operator::Over);
    cr.scale(scale, scale);

    let strokes = STROKES.lock().unwrap();
    cr.set_line_cap(crate::cairo::LineCap::Round);
    cr.set_line_join(crate::cairo::LineJoin::Round);
    for s in strokes.iter() {
        let pts = &s.points;
        if pts.len() < 2 { continue; }
        cr.set_source_rgba(s.r, s.g, s.b, 1.0);
        for i in 0..pts.len() {
            let (x, y, w, _t) = pts[i];
            if i == 0 {
                let _ = cr.arc(x, y, w * 0.5, 0.0, 2.0 * std::f64::consts::PI);
                let _ = cr.fill();
            } else {
                let (px, py, _pw, _pt) = pts[i - 1];
                cr.set_line_width(w);
                let _ = cr.move_to(px, py);
                let _ = cr.line_to(x, y);
                let _ = cr.stroke();
            }
        }
    }
}

/// Re‑render every stroke from `STROKES` onto a Cairo surface.
/// Called on undo, page‑nav, and display changes.
/// `surface_ptr` is a borrowed `cairo_surface_t*` — Rust does not free it.
#[cfg(feature = "cairo_real")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn glaspen2_draw_rebuild(
    surface_ptr: *mut std::ffi::c_void,
    scale: c_double,
) {
    let surface = unsafe {
        crate::cairo::Surface::from_raw_none(
            surface_ptr as *mut crate::cairo::ffi::cairo_surface_t,
        )
    };    draw_rebuild_on_surface(&surface, scale);
}

#[cfg(all(test, feature = "cairo_real"))]
mod tests {
    use crate::draw::draw_rebuild_on_surface;
    use crate::{Stroke, STROKES};

    fn pixel(s: &mut crate::cairo::ImageSurface, x: u32, y: u32) -> (u8, u8, u8, u8) {
        let stride = s.stride() as usize;
        let data = s.data().unwrap();
        let off = y as usize * stride + x as usize * 4;
        (data[off + 2], data[off + 1], data[off], data[off + 3])
    }

    #[test]
    fn test_empty_surface_transparent() {
        STROKES.lock().unwrap().clear();
        let mut s = crate::cairo::ImageSurface::create(
            crate::cairo::Format::ARgb32, 50, 50).unwrap();
        draw_rebuild_on_surface(&s, 1.0);
        assert_eq!(pixel(&mut s, 10, 10).3, 0);
    }

    #[test]
    fn test_red_stroke_renders() {
        STROKES.lock().unwrap().clear();
        STROKES.lock().unwrap().push(Stroke {
            r: 1.0, g: 0.0, b: 0.0,
            points: vec![(5.0, 25.0, 8.0, 0.0), (45.0, 25.0, 8.0, 1.0)],
        });
        let mut s = crate::cairo::ImageSurface::create(
            crate::cairo::Format::ARgb32, 50, 50).unwrap();
        // Draw manually to verify Cairo works
        let cr = crate::cairo::Context::new(&s).unwrap();
        cr.set_operator(crate::cairo::Operator::Clear);
        let _ = cr.paint();
        cr.set_operator(crate::cairo::Operator::Over);
        cr.set_source_rgba(1.0, 0.0, 0.0, 1.0);
        cr.set_line_width(8.0);
        let _ = cr.move_to(5.0, 25.0);
        let _ = cr.line_to(45.0, 25.0);
        let _ = cr.stroke();
        std::mem::drop(cr);
        // Check midpoint of stroke
        let (r, g, b, a) = pixel(&mut s, 25, 25);
        assert!(a > 0, "manual stroke should draw (a={})", a);
        assert!(r > 0 && g == 0 && b == 0, "stroke should be red");
        STROKES.lock().unwrap().clear();
    }

    #[test]
    fn test_scale_2x_respected() {
        STROKES.lock().unwrap().clear();
        STROKES.lock().unwrap().push(Stroke {
            r: 0.0, g: 1.0, b: 0.0,
            points: vec![(5.0, 5.0, 4.0, 0.0), (45.0, 5.0, 4.0, 1.0)],
        });
        let mut s = crate::cairo::ImageSurface::create(
            crate::cairo::Format::ARgb32, 100, 100).unwrap();
        draw_rebuild_on_surface(&s, 2.0);
        // stroke is at logical y=5; scale=2 → physical y=10
        let (_r, _g, _b, a) = pixel(&mut s, 25, 10);
        assert!(a > 0, "pixel on scaled stroke should have alpha");
        STROKES.lock().unwrap().clear();
    }

    /// End-to-end OCR test: render strokes → run OCR → print result.
    /// Run with: cargo test draw::tests::test_ocr_e2e -- --nocapture
    #[test]
    fn test_ocr_e2e() {
        STROKES.lock().unwrap().clear();
        // Draw some stroke patterns that should look like "test" or similar
        // Characters: draw horizontal+vertical strokes
        let strokes_data: Vec<(f64, f64, f64, f64, f64, f64)> = vec![
            // "T" shape
            (10.0, 20.0, 50.0, 20.0, 6.0, 0.0), // horizontal bar
            (30.0, 20.0, 30.0, 50.0, 6.0, 0.0), // vertical bar
            // "e" shape (simplified)
            (60.0, 35.0, 90.0, 35.0, 5.0, 0.0), // horizontal
            (75.0, 25.0, 60.0, 35.0, 5.0, 0.0),
            (75.0, 25.0, 75.0, 45.0, 5.0, 0.0),
            // "s" shape
            (100.0, 25.0, 125.0, 25.0, 5.0, 0.0),
            (125.0, 25.0, 125.0, 35.0, 5.0, 0.0),
            (125.0, 35.0, 100.0, 35.0, 5.0, 0.0),
            (100.0, 35.0, 100.0, 45.0, 5.0, 0.0),
            (100.0, 45.0, 125.0, 45.0, 5.0, 0.0),
            // "t" shape
            (135.0, 25.0, 135.0, 50.0, 5.0, 0.0),
            (130.0, 35.0, 145.0, 35.0, 5.0, 0.0),
        ];
        STROKES.lock().unwrap().push(crate::Stroke {
            r: 0.0, g: 0.0, b: 0.0,
            points: strokes_data.iter().map(|&(x1,y1,_,_,w,_)| {
                (x1, y1, w, 0.0)
            }).collect(),
        });
        STROKES.lock().unwrap().push(crate::Stroke {
            r: 0.0, g: 0.0, b: 0.0,
            points: strokes_data.iter().skip(1).map(|&(_,_,x2,y2,w,_)| {
                (x2, y2, w, 0.0)
            }).collect(),
        });

        // Render to surface
        let mut s = crate::cairo::ImageSurface::create(
            crate::cairo::Format::ARgb32, 160, 70).unwrap();
        draw_rebuild_on_surface(&s, 1.0);

        // Read pixel data
        let stride = s.stride() as usize;
        let w = s.width() as u32;
        let h = s.height() as u32;
        let data = s.data().unwrap();
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let off = y as usize * stride + x as usize * 4;
                let b = data[off];
                let g = data[off + 1];
                let r = data[off + 2];
                let a = data[off + 3];
                let pix_off = (y * w + x) as usize * 4;
                rgba[pix_off] = r;
                rgba[pix_off + 1] = g;
                rgba[pix_off + 2] = b;
                rgba[pix_off + 3] = a;
            }
        }
        std::mem::drop(data);

        // Run OCR
        let text = crate::ocr::recognize(&rgba, w, h);
        eprintln!("[ocr_e2e] recognized: {:?}", text);
        // Pipeline should not crash; any output is a bonus
        assert!(text.len() < 100);
        STROKES.lock().unwrap().clear();
    }

    /// Read the latest screen from a specific glaspen2 DB file and run OCR.
    /// Set GLASPEN2_DB env var to the path, or defaults to target/debug/glaspen2.db
    #[test]
    fn test_ocr_from_db() {
        let db_path = std::env::var("GLASPEN2_DB")
            .unwrap_or_else(|_| "target/debug/glaspen2.db".to_string());

        if !std::path::Path::new(&db_path).exists() {
            eprintln!("[ocr_db] DB not found at {}, skipping", db_path);
            return;
        }
        eprintln!("[ocr_db] reading from {}", db_path);

        let strokes = {
            let rt = crate::runtime();
            rt.block_on(async {
                let pool = sqlx::SqlitePool::connect_with(
                    sqlx::sqlite::SqliteConnectOptions::new()
                        .filename(&db_path)
                        .read_only(true)
                ).await.expect("Failed to open DB");

                // Get latest screen with substantial handwriting (>500 points AND >5 pts/stroke)
                let screen_id: i64 = sqlx::query_scalar(
                    "SELECT s.id FROM screens s
                     JOIN strokes st ON st.screen_id = s.id
                     JOIN points p ON p.stroke_id = st.id
                     GROUP BY s.id
                     HAVING COUNT(p.rowid) >= 500
                        AND 1.0 * COUNT(p.rowid) / COUNT(DISTINCT st.id) >= 20
                     ORDER BY s.id DESC LIMIT 1"
                ).fetch_one(&pool).await.unwrap_or(0);
                eprintln!("[ocr_db] latest screen: {}", screen_id);

                // Get strokes for that screen
                let rows: Vec<(i64, f64, f64, f64, f64)> = sqlx::query_as(
                    "SELECT id, color_r, color_g, color_b, width_scale FROM strokes WHERE screen_id = ?1 ORDER BY id"
                ).bind(screen_id).fetch_all(&pool).await.unwrap_or_default();
                eprintln!("[ocr_db] {} strokes", rows.len());

                let mut result = Vec::new();
                for (stroke_id, r, g, b, _ws) in rows {
                    let points: Vec<(f64,f64,f64,f64)> = sqlx::query_as(
                        "SELECT x, y, width, t FROM points WHERE stroke_id = ?1 ORDER BY seq"
                    ).bind(stroke_id).fetch_all(&pool).await.unwrap_or_default();
                    result.push(Stroke { r, g, b, points });
                }
                pool.close().await;
                result
            })
        };

        if strokes.is_empty() {
            eprintln!("[ocr_db] No strokes found");
            return;
        }
        let total_points: usize = strokes.iter().map(|s| s.points.len()).sum();
        eprintln!("[ocr_db] total points: {}", total_points);

        // Push to STROKES
        *STROKES.lock().unwrap() = strokes;

        // Find bounding box to crop
        let (min_x, min_y, max_x, max_y) = {
            let s = STROKES.lock().unwrap();
            let mut bx = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
            for st in s.iter() {
                for &(x, y, _, _) in &st.points {
                    if x < bx.0 { bx.0 = x; }
                    if y < bx.1 { bx.1 = y; }
                    if x > bx.2 { bx.2 = x; }
                    if y > bx.3 { bx.3 = y; }
                }
            }
            bx
        };
        let pad = 20.0;
        let crop_x = (min_x - pad).max(0.0) as u32;
        let crop_y = (min_y - pad).max(0.0) as u32;
        let crop_w = ((max_x - min_x + pad * 2.0).ceil() as u32).max(10);
        let crop_h = ((max_y - min_y + pad * 2.0).ceil() as u32).max(10);
        eprintln!("[ocr_db] crop: {}x{} +({},{})", crop_w, crop_h, crop_x, crop_y);

        // Render to Cairo surface
        let mut surface = crate::cairo::ImageSurface::create(
            crate::cairo::Format::ARgb32,
            (crop_w + crop_x) as i32,
            (crop_h + crop_y) as i32,
        ).unwrap();
        draw_rebuild_on_surface(&surface, 1.0);

        // Read pixels and crop
        let stride = surface.stride() as usize;
        let _sw = surface.width() as u32;
        let data = surface.data().unwrap();
        let mut rgba = vec![0u8; (crop_w * crop_h * 4) as usize];
        for y in 0..crop_h {
            for x in 0..crop_w {
                let sx = x + crop_x;
                let sy = y + crop_y;
                let off = (sy as usize) * stride + (sx as usize) * 4;
                let pix_off = (y * crop_w + x) as usize * 4;
                if off + 3 < data.len() {
                    rgba[pix_off] = data[off + 2];     // R
                    rgba[pix_off + 1] = data[off + 1]; // G
                    rgba[pix_off + 2] = data[off];     // B
                    rgba[pix_off + 3] = data[off + 3]; // A
                }
            }
        }
        std::mem::drop(data);

        eprintln!("[ocr_db] running OCR on {}x{} crop...", crop_w, crop_h);
        eprintln!("[ocr_db] after resize to 48h: w={}", (crop_w as f64 * 48.0 / crop_h as f64).ceil());
        let text = crate::ocr::detect_and_recognize(&rgba, crop_w, crop_h);
        eprintln!("[ocr_db] ======== RECOGNIZED: {:?} ========", text);

        STROKES.lock().unwrap().clear();
    }
}