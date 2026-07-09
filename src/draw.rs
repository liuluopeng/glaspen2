//! Cairo drawing operations migrated from ObjC to Rust.
//! MacOS-only (uses real Cairo from cairo-rs crate).

use std::os::raw::c_double;
use crate::STROKES;

/// Re‑render every stroke from `STROKES` onto a Cairo surface.
/// Called on undo, page‑nav, and display changes.
/// `surface_ptr` is a borrowed `cairo_surface_t*` — Rust does not free it.
#[cfg(feature = "cairo_real")]
#[no_mangle]
pub unsafe extern "C" fn glaspen2_draw_rebuild(
    surface_ptr: *mut std::ffi::c_void,
    scale: c_double,
) {
    let surface = crate::cairo::Surface::from_raw_none(
        surface_ptr as *mut crate::cairo::ffi::cairo_surface_t,
    );
    let Ok(cr) = crate::cairo::Context::new(&surface) else { return };
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
