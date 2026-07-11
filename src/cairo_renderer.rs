mod cairo_renderer {
    use std::os::raw::{c_uchar};
    use std::f64::consts::PI;

    use crate::{STROKES, modeler};

    /// Uses real Cairo (via cairo_dl) if DLL loaded, falls back to stub.
    enum CairoBackend {
        Real(crate::windows::cairo_dl::CairoRealSurface),
        Stub(crate::cairo::ImageSurface),
    }

    pub struct CairoRenderer {
        surface: CairoBackend,
        width: i32,
        height: i32,
    }

    impl CairoRenderer {
        pub fn new(width: i32, height: i32) -> Option<Self> {
            let _ = crate::windows::cairo_dl::cairo_init();
            let surface = if crate::windows::cairo_dl::is_cairo_loaded() {
                let s = crate::windows::cairo_dl::CairoRealSurface::create(width, height)?;
                CairoBackend::Real(s)
            } else {
                let s = crate::cairo::ImageSurface::create(crate::cairo::Format::ARGB32, width, height).ok()?;
                CairoBackend::Stub(s)
            };
            // Initialize to fully transparent
            let mut r = Self { surface, width, height };
            r.clear();
            Some(r)
        }

        pub fn clear(&mut self) {
            match &self.surface {
                CairoBackend::Real(s) => {
                    if let Some(cr) = crate::windows::cairo_dl::CairoRealContext::new(s) {
                        cr.set_operator_clear();
                        cr.paint();
                        cr.set_operator_over();
                    }
                }
                CairoBackend::Stub(s) => {
                    use crate::cairo::{Context, Operator};
                    if let Ok(cr) = Context::new(s) {
                        cr.set_operator(Operator::Clear);
                        cr.paint().ok();
                    }
                }
            }
        }

        pub fn draw_line(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, width: f64, r: f64, g: f64, b: f64) {
            match &self.surface {
                CairoBackend::Real(s) => {
                    if let Some(cr) = crate::windows::cairo_dl::CairoRealContext::new(s) {
                        cr.set_source_rgba(r, g, b, 1.0);
                        cr.set_line_width(width);
                        cr.set_line_cap_round();
                        cr.set_line_join_round();
                        cr.move_to(x0, y0);
                        cr.line_to(x1, y1);
                        cr.stroke();
                    }
                }
                CairoBackend::Stub(s) => {
                    use crate::cairo::{Context, LineCap, LineJoin};
                    if let Ok(cr) = Context::new(s) {
                        cr.set_source_rgba(r, g, b, 1.0);
                        cr.set_line_width(width);
                        cr.set_line_cap(LineCap::Round);
                        cr.set_line_join(LineJoin::Round);
                        cr.move_to(x0, y0);
                        cr.line_to(x1, y1);
                        cr.stroke().ok();
                    }
                }
            }
        }

        pub fn draw_dot(&mut self, x: f64, y: f64, width: f64, r: f64, g: f64, b: f64) {
            match &self.surface {
                CairoBackend::Real(s) => {
                    if let Some(cr) = crate::windows::cairo_dl::CairoRealContext::new(s) {
                        cr.set_source_rgba(r, g, b, 1.0);
                        cr.arc(x, y, width * 0.5, 0.0, 2.0 * PI);
                        cr.fill();
                    }
                }
                CairoBackend::Stub(s) => {
                    use crate::cairo::Context;
                    if let Ok(cr) = Context::new(s) {
                        cr.set_source_rgba(r, g, b, 1.0);
                        cr.arc(x, y, width * 0.5, 0.0, 2.0 * PI);
                        cr.fill().ok();
                    }
                }
            }
        }

        pub fn surface_data(&self) -> *const c_uchar {
            match &self.surface {
                CairoBackend::Real(s) => {
                    s.flush();
                    s.data_ptr()
                }
                CairoBackend::Stub(s) => {
                    s.data().map(|d| d.as_ptr()).unwrap_or(std::ptr::null())
                }
            }
        }

        pub fn surface_data_mut(&self) -> *mut c_uchar {
            match &self.surface {
                CairoBackend::Real(s) => {
                    s.flush();
                    s.data_ptr_mut()
                }
                CairoBackend::Stub(s) => {
                    s.pixels_mut().as_mut_ptr()
                }
            }
        }

        pub fn surface_size(&self) -> (i32, i32, i32) {
            let stride = match &self.surface {
                CairoBackend::Real(s) => s.stride(),
                CairoBackend::Stub(s) => s.stride(),
            };
            (self.width, self.height, stride)
        }

        pub fn mark_dirty(&self) {
            match &self.surface {
                CairoBackend::Real(s) => s.mark_dirty(),
                CairoBackend::Stub(_) => {}
            }
        }

        pub fn draw_modeler_buffer(&mut self, r: f64, g: f64, b: f64) {
            let count = modeler::buffer_len();
            if count < 1 { return; }

            if let Some((x0, y0, w0, _t0)) = modeler::get_buffer_point(0) {
                self.draw_dot(x0, y0, w0, r, g, b);
                let mut prev_x = x0;
                let mut prev_y = y0;
                for i in 1..count {
                    if let Some((px, py, pw, _pt)) = modeler::get_buffer_point(i) {
                        self.draw_line(prev_x, prev_y, px, py, pw, r, g, b);
                        prev_x = px;
                        prev_y = py;
                    }
                }
            }
        }

        pub fn replay_strokes(&mut self) {
            self.clear();
            let strokes = STROKES.lock().unwrap();
            for stroke in strokes.iter() {
                if stroke.points.is_empty() { continue; }
                let (x0, y0, w0, _t0) = stroke.points[0];
                self.draw_dot(x0, y0, w0, stroke.r, stroke.g, stroke.b);
                for w in stroke.points.windows(2) {
                    let (x1, y1, w1, _t1) = w[0];
                    let (x2, y2, w2, _t2) = w[1];
                    self.draw_line(x1, y1, x2, y2, w2, stroke.r, stroke.g, stroke.b);
                }
            }
        }
    }
}

use std::os::raw::{c_int, c_double, c_uchar};
use crate::STROKES;

#[cfg(target_os = "windows")]
pub use cairo_renderer::CairoRenderer;

// ── Cairo Renderer FFI ──

#[unsafe(no_mangle)]
#[cfg(target_os = "windows")]
pub extern "C" fn glaspen2_cairo_renderer_create(w: c_int, h: c_int) -> *mut CairoRenderer {
    match CairoRenderer::new(w, h) {
        Some(r) => Box::into_raw(Box::new(r)),
        None => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
#[cfg(target_os = "windows")]
pub extern "C" fn glaspen2_cairo_renderer_destroy(renderer: *mut CairoRenderer) {
    if !renderer.is_null() {
        unsafe { drop(Box::from_raw(renderer)); }
    }
}

#[unsafe(no_mangle)]
#[cfg(target_os = "windows")]
pub extern "C" fn glaspen2_cairo_draw_line(
    renderer: *mut CairoRenderer,
    x0: c_double, y0: c_double, x1: c_double, y1: c_double,
    width: c_double, r: c_double, g: c_double, b: c_double,
) {
    if !renderer.is_null() {
        unsafe { (*renderer).draw_line(x0, y0, x1, y1, width, r, g, b); }
    }
}

#[unsafe(no_mangle)]
#[cfg(target_os = "windows")]
pub extern "C" fn glaspen2_cairo_draw_dot(
    renderer: *mut CairoRenderer,
    x: c_double, y: c_double, width: c_double,
    r: c_double, g: c_double, b: c_double,
) {
    if !renderer.is_null() {
        unsafe { (*renderer).draw_dot(x, y, width, r, g, b); }
    }
}

#[unsafe(no_mangle)]
#[cfg(target_os = "windows")]
pub extern "C" fn glaspen2_cairo_clear(renderer: *mut CairoRenderer) {
    if !renderer.is_null() {
        unsafe { (*renderer).clear(); }
    }
}

#[unsafe(no_mangle)]
#[cfg(target_os = "windows")]
pub extern "C" fn glaspen2_cairo_surface_data(renderer: *mut CairoRenderer) -> *const c_uchar {
    if renderer.is_null() { return std::ptr::null(); }
    unsafe { (*renderer).surface_data() }
}

#[unsafe(no_mangle)]
#[cfg(target_os = "windows")]
pub extern "C" fn glaspen2_cairo_surface_data_mut(renderer: *mut CairoRenderer) -> *mut c_uchar {
    if renderer.is_null() { return std::ptr::null_mut(); }
    unsafe { (*renderer).surface_data_mut() }
}

#[unsafe(no_mangle)]
#[cfg(target_os = "windows")]
pub extern "C" fn glaspen2_cairo_undo(renderer: *mut CairoRenderer) -> c_int {
    if renderer.is_null() { return -1; }
    {
        let mut strokes = STROKES.lock().unwrap();
        strokes.pop();
    }
    crate::db::delete_last_stroke();
    unsafe { (*renderer).replay_strokes(); }
    let count = STROKES.lock().unwrap().len() as c_int;
    eprintln!("[cairo_undo] remaining strokes: {}", count);
    count
}

#[unsafe(no_mangle)]
#[cfg(target_os = "windows")]
pub extern "C" fn glaspen2_cairo_surface_size(
    renderer: *mut CairoRenderer,
    w: *mut c_int, h: *mut c_int, stride: *mut c_int,
) {
    if renderer.is_null() {
        unsafe { *w = 0; *h = 0; *stride = 0; }
        return;
    }
    let (width, height, s) = unsafe { (*renderer).surface_size() };
    unsafe { *w = width; *h = height; *stride = s; }
}

#[unsafe(no_mangle)]
#[cfg(target_os = "windows")]
pub extern "C" fn glaspen2_cairo_draw_modeler_buffer(
    renderer: *mut CairoRenderer,
    r: c_double, g: c_double, b: c_double,
) {
    if !renderer.is_null() {
        unsafe { (*renderer).draw_modeler_buffer(r, g, b); }
    }
}

#[unsafe(no_mangle)]
#[cfg(target_os = "windows")]
pub extern "C" fn glaspen2_cairo_replay_strokes(renderer: *mut CairoRenderer) {
    if !renderer.is_null() {
        unsafe { (*renderer).replay_strokes(); }
    }
}
