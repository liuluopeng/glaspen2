//! Pen-input window (non-layered, fullscreen, BELOW the overlay).
//! Receives WM_POINTER events when pen passes through the transparent overlay.
//! Uses Pointer API for pen detection and pressure — no dwExtraInfo dependency.

use std::sync::Mutex;
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::UI::Input::Pointer::*;

use super::overlay;
use super::render;

pub static PEN_HWND: Mutex<isize> = Mutex::new(0);

static mut PEN_ACTIVE: bool = false;
static mut LAST_X: f64 = 0.0;
static mut LAST_Y: f64 = 0.0;
static mut HAS_LAST: bool = false;
static mut STROKE_COLOR: (f64, f64, f64) = (1.0, 0.0, 0.0);

pub fn run(screen_w: i32, screen_h: i32) {
    let hmodule = unsafe { GetModuleHandleW(None).unwrap() };
    let instance = HINSTANCE(hmodule.0);
    let class_name = wide_string("Glaspen2Input");

    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(pen_wnd_proc),
        hInstance: instance,
        lpszClassName: PCWSTR(class_name.as_ptr()),
        ..Default::default()
    };
    unsafe { RegisterClassExW(&wc) };

    // Non-layered, TOPMOST, but created BEFORE the overlay so it's below it.
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(wide_string("glaspen2_pen").as_ptr()),
            WS_POPUP,
            0, 0, screen_w, screen_h,
            None, None, instance, None,
        ).unwrap()
    };

    { let mut h = PEN_HWND.lock().unwrap(); *h = hwnd.0 as isize; }

    unsafe { ShowWindow(hwnd, SW_SHOW); }

    // Message loop
    let mut msg = MSG::default();
    loop {
        let result = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if result.0 == 0 { break; }
        unsafe { TranslateMessage(&msg); DispatchMessageW(&msg); }
    }
}

fn is_pen(pid: u32) -> bool {
    let mut pt = POINTER_INPUT_TYPE::default();
    unsafe { GetPointerType(pid, &mut pt).is_ok() && pt == PT_PEN }
}

fn get_pressure(pid: u32) -> f64 {
    let mut info = POINTER_PEN_INFO::default();
    unsafe {
        if GetPointerPenInfo(pid, &mut info).is_ok() {
            return (info.pressure as f64 / 1024.0).clamp(0.0, 1.0);
        }
    }
    0.5
}

fn pressure_to_width(p: f64, scale: f64) -> f64 {
    if p > 0.01 { (0.3 + p * p * 7.7) * scale } else { 1.0 * scale }
}

unsafe extern "system" fn pen_wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        // WM_POINTER
        0x0246 | 0x0245 | 0x0247 => {
            let pid = (wparam.0 & 0xFFFF) as u32;
            if !is_pen(pid) { return DefWindowProcW(hwnd, msg, wparam, lparam); }

            let x = (lparam.0 as i32 & 0xFFFF) as f64;
            let y = ((lparam.0 as i32 >> 16) & 0xFFFF) as f64;
            let pressure = get_pressure(pid);

            static mut COUNT: u32 = 0;
            COUNT += 1;
            if COUNT <= 5 {
                eprintln!("[pen] msg={:#x} x={:.0} y={:.0} p={:.3}", msg, x, y, pressure);
            }

            let data = match overlay::get_overlay_data() {
                Some(d) => d,
                None => return LRESULT(0),
            };
            let state = data.state.lock().unwrap();
            if !state.enabled { return LRESULT(0); }
            let width_scale = state.width_scale;
            let (pr, pg, pb) = (state.pen_r, state.pen_g, state.pen_b);
            let inverse = state.inverse_enabled;
            drop(state);

            if msg == 0x0246 && !PEN_ACTIVE {
                // DOWN — start stroke
                PEN_ACTIVE = true; HAS_LAST = false;
                let color = if inverse {
                    let inv = sample_screen_inverse(x, y);
                    inv
                } else { (pr, pg, pb) };
                STROKE_COLOR = color;
                let w = pressure_to_width(pressure, width_scale);
                draw_pen_point(&data.surface, x, y, w, color, false);
                overlay::update_overlay();
            } else if msg == 0x0245 && PEN_ACTIVE {
                // MOVE — continue
                let color = STROKE_COLOR;
                let w = pressure_to_width(pressure, width_scale);
                draw_pen_point(&data.surface, x, y, w, color, true);
                overlay::update_overlay();
            } else if msg == 0x0247 && PEN_ACTIVE {
                // UP — end
                PEN_ACTIVE = false; HAS_LAST = false;
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

// ── Drawing ──

fn sample_screen_inverse(x: f64, y: f64) -> (f64, f64, f64) {
    unsafe {
        use windows::Win32::Graphics::Gdi::{GetDC, ReleaseDC, GetPixel};
        let hdc = GetDC(None);
        if hdc.is_invalid() { return (1.0, 1.0, 1.0); }
        let color = GetPixel(hdc, x as i32, y as i32);
        ReleaseDC(None, hdc);
        if color.0 == 0xFFFFFFFF { return (1.0, 1.0, 1.0); }
        let r = (color.0 & 0xFF) as f64 / 255.0;
        let g = ((color.0 >> 8) & 0xFF) as f64 / 255.0;
        let b = ((color.0 >> 16) & 0xFF) as f64 / 255.0;
        (1.0 - r, 1.0 - g, 1.0 - b)
    }
}

fn draw_pen_point(surface: &crate::cairo::ImageSurface, x: f64, y: f64, w: f64,
                  color: (f64, f64, f64), has_last: bool) {
    let sw = surface.width() as usize;
    let sh = surface.height() as usize;
    let stride = surface.stride() as usize;
    let px = surface.pixels_mut();
    let cr = (color.0 * 255.0) as u32;
    let cg = (color.1 * 255.0) as u32;
    let cb = (color.2 * 255.0) as u32;
    let radius = w * 0.5;
    unsafe {
        if has_last && HAS_LAST {
            draw_thick_line(px, sw, sh, stride, LAST_X, LAST_Y, x, y, radius, cr, cg, cb, 255);
        } else {
            draw_filled_circle(px, sw, sh, stride, x, y, radius, cr, cg, cb, 255);
        }
        LAST_X = x; LAST_Y = y; HAS_LAST = true;
    }
}

// Re-use fast pixel drawing from overlay module
fn draw_filled_circle(pixels: &mut [u8], sw: usize, sh: usize, stride: usize,
                       cx: f64, cy: f64, radius: f64, r: u32, g: u32, b: u32, a: u32) {
    if radius < 0.5 { return; }
    let r_sq = radius * radius;
    let r_inner_sq = (radius - 0.5).max(0.0).powi(2);
    let x_min = (cx - radius - 1.0).floor() as i32;
    let x_max = (cx + radius + 1.0).ceil() as i32;
    let y_min = (cy - radius - 1.0).floor() as i32;
    let y_max = (cy + radius + 1.0).ceil() as i32;
    for py in y_min..=y_max {
        for px_x in x_min..=x_max {
            let dx = px_x as f64 + 0.5 - cx;
            let dy = py as f64 + 0.5 - cy;
            let dsq = dx * dx + dy * dy;
            if dsq < r_inner_sq {
                put_pixel(pixels, stride, sw, sh, px_x, py, r, g, b, a);
            } else if dsq < r_sq {
                let alpha = (a as f64 * (0.5 + radius - dsq.sqrt())).min(255.0) as u32;
                if alpha > 0 { put_pixel(pixels, stride, sw, sh, px_x, py, r, g, b, alpha); }
            }
        }
    }
}

fn draw_thick_line(pixels: &mut [u8], sw: usize, sh: usize, stride: usize,
                    x0: f64, y0: f64, x1: f64, y1: f64, radius: f64,
                    r: u32, g: u32, b: u32, a: u32) {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let dist = (dx * dx + dy * dy).sqrt();
    let steps = (dist / (radius * 0.5).max(1.0)).ceil() as i32;
    for s in 0..=steps {
        let t = if steps == 0 { 0.0 } else { s as f64 / steps as f64 };
        draw_filled_circle(pixels, sw, sh, stride, x0 + dx * t, y0 + dy * t, radius, r, g, b, a);
    }
}

#[inline]
fn put_pixel(pixels: &mut [u8], stride: usize, sw: usize, sh: usize,
              x: i32, y: i32, r: u32, g: u32, b: u32, a: u32) {
    if x < 0 || y < 0 || x as usize >= sw || y as usize >= sh { return; }
    let off = y as usize * stride + x as usize * 4;
    if off + 3 >= pixels.len() { return; }
    let sa = a as f32;
    let da = pixels[off + 3] as f32;
    let inv = 1.0 - sa / 255.0;
    let out_a = sa + da * inv;
    if out_a < 1.0 { return; }
    pixels[off] = (b as f32 * sa / 255.0 + pixels[off] as f32 * inv * da / 255.0) as u8;
    pixels[off + 1] = (g as f32 * sa / 255.0 + pixels[off + 1] as f32 * inv * da / 255.0) as u8;
    pixels[off + 2] = (r as f32 * sa / 255.0 + pixels[off + 2] as f32 * inv * da / 255.0) as u8;
    pixels[off + 3] = out_a.min(255.0) as u8;
}

fn wide_string(s: &str) -> Vec<u16> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}
