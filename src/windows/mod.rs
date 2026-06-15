use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, DrawingArea, gdk, glib};
use gdk::prelude::*;
use cairo;
use std::sync::{Arc, Mutex};
use std::cell::UnsafeCell;

use crate::db;
use crate::modeler;

struct SharedSurface {
    inner: UnsafeCell<cairo::ImageSurface>,
}

// Safety: GTK4 is single-threaded
unsafe impl Send for SharedSurface {}
unsafe impl Sync for SharedSurface {}

impl SharedSurface {
    fn new(surface: cairo::ImageSurface) -> Self {
        Self { inner: UnsafeCell::new(surface) }
    }
    fn get(&self) -> &cairo::ImageSurface {
        unsafe { &*self.inner.get() }
    }
    fn get_mut(&self) -> &mut cairo::ImageSurface {
        unsafe { &mut *self.inner.get() }
    }
}

struct AppState {
    surface: SharedSurface,
    pen_r: f64,
    pen_g: f64,
    pen_b: f64,
    width_scale: f64,
    stroke_active: bool,
    last_x: f64,
    last_y: f64,
    has_last: bool,
    enabled: bool,
    inverse_enabled: bool,
    screen_w: i32,
    screen_h: i32,
}

fn pressure_to_width(pressure: f64, width_scale: f64) -> f64 {
    if pressure > 0.01 { (0.3 + pressure * pressure * 7.7) * width_scale }
    else { 1.0 * width_scale }
}

fn draw_pen_dot(surface: &cairo::ImageSurface, x: f64, y: f64, radius: f64, color: (f64, f64, f64)) {
    let cr = cairo::Context::new(surface).unwrap();
    cr.set_source_rgba(color.0, color.1, color.2, 1.0);
    cr.arc(x, y, radius, 0.0, std::f64::consts::PI * 2.0);
    let _ = cr.fill();
}

fn draw_pen_segment(surface: &cairo::ImageSurface, x0: f64, y0: f64, x1: f64, y1: f64, w: f64, color: (f64, f64, f64)) {
    let cr = cairo::Context::new(surface).unwrap();
    cr.set_source_rgba(color.0, color.1, color.2, 1.0);
    cr.set_line_width(w);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.set_line_join(cairo::LineJoin::Round);
    cr.move_to(x0, y0);
    cr.line_to(x1, y1);
    let _ = cr.stroke();
}

fn flush_surface_to_drawing_area(surface: &cairo::ImageSurface, area: &DrawingArea) {
    area.queue_draw();
}

pub fn win_main() {
    let app = Application::builder()
        .application_id("com.glaspen2")
        .build();

    app.connect_activate(move |app| {
        let display = gdk::Display::default().expect("no display");
        let monitors = display.monitors();
        let monitor = monitors.item(0).expect("no monitor").downcast::<gdk::Monitor>().expect("not a monitor");
        let geo = monitor.geometry();
        let screen_w = geo.width();
        let screen_h = geo.height();

        db::init();
        db::new_screen(screen_w, screen_h);

        let mut pen_r = 1.0; let mut pen_g = 0.0; let mut pen_b = 0.0; let mut width_scale = 1.0;
        crate::glaspen2_load_settings_parts(&mut pen_r, &mut pen_g, &mut pen_b, &mut width_scale);
        let inverse_enabled = db::load_setting("inverse_enabled")
            .and_then(|v| v.parse::<i32>().ok()).unwrap_or(0) != 0;

        let surface = SharedSurface::new(
            cairo::ImageSurface::create(cairo::Format::ARgb32, screen_w, screen_h)
                .expect("failed to create surface")
        );

        let state = Arc::new(Mutex::new(AppState {
            surface,
            pen_r, pen_g, pen_b,
            width_scale,
            stroke_active: false,
            last_x: 0.0, last_y: 0.0,
            has_last: false,
            enabled: true,
            inverse_enabled,
            screen_w, screen_h,
        }));

        let window = ApplicationWindow::builder()
            .application(app)
            .title("glaspen2")
            .default_width(screen_w)
            .default_height(screen_h)
            .decorated(false)
            .build();

        window.set_decorated(false);

        // Make window background transparent via CSS
        let css_provider = gtk::CssProvider::new();
        css_provider.load_from_data("window { background-color: transparent; }");
        gtk::StyleContext::add_provider_for_display(
            &gdk::Display::default().expect("no display"),
            &css_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        // Drawing area
        let drawing_area = DrawingArea::new();
        drawing_area.set_size_request(screen_w as i32, screen_h as i32);
        // Make drawing area input-transparent (mouse/keyboard pass through)
        drawing_area.set_focus_on_click(false);

        let state_clone = state.clone();
        drawing_area.set_draw_func(move |_area, cr, _w, _h| {
            let s = state_clone.lock().unwrap();
            let surface = s.surface.get();
            let w = surface.width();
            let h = surface.height();
            let stride = surface.stride();
            // Copy pixel data while holding lock
            let mut pixel_data = vec![0u8; (stride * h) as usize];
            {
                let surface_mut = s.surface.get_mut();
                cairo::Surface::flush(surface_mut);
                if let Ok(data) = surface_mut.data() {
                    pixel_data.copy_from_slice(&data);
                }
            }
            drop(s); // Release lock
            if let Ok(tmp) = cairo::ImageSurface::create_for_data(
                pixel_data,
                cairo::Format::ARgb32,
                w,
                h,
                stride,
            ) {
                cr.set_source_surface(&tmp, 0.0, 0.0);
                let _ = cr.paint();
            }
        });

        window.set_child(Some(&drawing_area));

        // Keyboard hotkeys (Ctrl+Alt+C = clear, Ctrl+Alt+V = toggle)
        {
            let state_c = state.clone();
            let da = drawing_area.clone();
            let ctrl = gtk::EventControllerKey::new();
            ctrl.connect_key_pressed(move |_ctrl, key, _code, mods| {
                if mods.contains(gdk::ModifierType::CONTROL_MASK) && mods.contains(gdk::ModifierType::ALT_MASK) {
                    match key {
                        gdk::Key::C => {
                            let mut s = state_c.lock().unwrap();
                            crate::glaspen2_clear_strokes(s.screen_w, s.screen_h);
                            let surface = s.surface.get_mut();
                            let cr = cairo::Context::new(surface).unwrap();
                            cr.set_operator(cairo::Operator::Clear);
                            let _ = cr.paint();
                            cr.set_operator(cairo::Operator::Over);
                            da.queue_draw();
                            glib::Propagation::Stop
                        }
                        gdk::Key::V => {
                            let mut s = state_c.lock().unwrap();
                            s.enabled = !s.enabled;
                            glib::Propagation::Stop
                        }
                        _ => glib::Propagation::Proceed,
                    }
                } else {
                    glib::Propagation::Proceed
                }
            });
            window.add_controller(ctrl);
        }

        // Motion events
        {
            let state_c = state.clone();
            let da = drawing_area.clone();
            let ctrl = gtk::EventControllerMotion::new();
            ctrl.connect_motion(move |_ctrl, x, y| {
                let mut s = state_c.lock().unwrap();
                if s.stroke_active && s.enabled {
                    let pressure = 0.5;
                    let w = pressure_to_width(pressure, s.width_scale);
                    let color = (s.pen_r, s.pen_g, s.pen_b);
                    draw_pen_segment(s.surface.get_mut(), s.last_x, s.last_y, x, y, w, color);
                    modeler::pen_move(x, y, pressure, 0.0, s.width_scale);
                    s.last_x = x;
                    s.last_y = y;
                    s.has_last = true;
                    da.queue_draw();
                }
            });
            drawing_area.add_controller(ctrl);
        }

        // Button press/release
        {
            let state_c = state.clone();
            let da = drawing_area.clone();
            let ctrl = gtk::GestureClick::new();
            {
                let state_c2 = state_c.clone();
                let da2 = da.clone();
                ctrl.connect_pressed(move |_gesture, _n_press, x, y| {
                    let mut s = state_c2.lock().unwrap();
                    if s.enabled {
                        s.stroke_active = true;
                        s.has_last = false;
                        s.last_x = x;
                        s.last_y = y;
                        let pressure = 0.5;
                        let w = pressure_to_width(pressure, s.width_scale);
                        let color = (s.pen_r, s.pen_g, s.pen_b);
                        draw_pen_dot(s.surface.get_mut(), x, y, w * 0.5, color);
                        modeler::begin_stroke(x, y, pressure, 0.0, s.width_scale);
                        da2.queue_draw();
                    }
                });
            }
            {
                let state_c2 = state_c.clone();
                ctrl.connect_released(move |_gesture, _n_press, _x, _y| {
                    let mut s = state_c2.lock().unwrap();
                    if s.stroke_active {
                        s.stroke_active = false;
                        s.has_last = false;
                    }
                });
            }
            drawing_area.add_controller(ctrl);
        }

        window.show();
        window.present();

        // After window is realized, find HWND by title and set transparency
        {
            let title = "glaspen2";
            let title_wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
            unsafe {
                use windows::Win32::Foundation::HWND;
                use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, GetWindowLongW, SetWindowLongW, GWL_EXSTYLE, WS_EX_TRANSPARENT};
                std::thread::sleep(std::time::Duration::from_millis(100));
                let hwnd = FindWindowW(None, windows::core::PCWSTR(title_wide.as_ptr()));
                if let Ok(hwnd) = hwnd {
                    let style = GetWindowLongW(hwnd, GWL_EXSTYLE);
                    SetWindowLongW(hwnd, GWL_EXSTYLE, style | WS_EX_TRANSPARENT.0 as i32);
                }
            }
        }

        // Main loop for GTK4
    });

    app.run_with_args(&[] as &[&str]);
}
