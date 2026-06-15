use std::sync::Mutex;
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

use super::overlay;

static HOOK_HANDLE: Mutex<isize> = Mutex::new(0);
static HOOK_HWND: Mutex<isize> = Mutex::new(0);

static mut LAST_PEN_X: f64 = 0.0;
static mut LAST_PEN_Y: f64 = 0.0;
static mut HAS_LAST_PEN: bool = false;
static mut PEN_ACTIVE: bool = false;
static mut STROKE_COLOR: (f64, f64, f64) = (1.0, 0.0, 0.0);

pub fn is_pen_active() -> bool { unsafe { PEN_ACTIVE } }

pub fn install_hook(hwnd: HWND) {
    let hmodule = unsafe { GetModuleHandleW(None).unwrap() };
    let instance = HINSTANCE(hmodule.0);
    let hook = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), instance, 0).unwrap() };
    let mut h = HOOK_HANDLE.lock().unwrap();
    *h = hook.0 as isize;
    let mut h2 = HOOK_HWND.lock().unwrap();
    *h2 = hwnd.0 as isize;
}

pub fn uninstall_hook() {
    let h = HOOK_HANDLE.lock().unwrap();
    if *h != 0 {
        unsafe { let hook = HHOOK(*h as *mut _); UnhookWindowsHookEx(hook).ok(); }
    }
}

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

fn pressure_to_width(pressure: f64, width_scale: f64) -> f64 {
    if pressure > 0.01 { (0.3 + pressure * pressure * 7.7) * width_scale }
    else { 1.0 * width_scale }
}

/// Handle WM_POINTER pen events from the overlay window.
pub fn handle_pointer_pen(hwnd: HWND, msg: u32, x: f64, y: f64, pressure: f64) {
    let is_down = msg == 0x0246;
    let is_move = msg == 0x0245;
    let is_up = msg == 0x0247;

    let data = match overlay::get_overlay_data(hwnd) { Some(d) => d, None => return };
    let state = data.state.lock().unwrap();
    if !state.enabled { return; }
    let width_scale = state.width_scale;

    if is_down && !unsafe { PEN_ACTIVE } {
        unsafe { PEN_ACTIVE = true; HAS_LAST_PEN = false; }
        let pen_color = if state.inverse_enabled { sample_screen_inverse(x, y) }
                        else { (state.pen_r, state.pen_g, state.pen_b) };
        unsafe { STROKE_COLOR = pen_color; }
        drop(state);

        let w = pressure_to_width(pressure, width_scale);
        draw_pen_point(&data.surface, x, y, w, pen_color);
        overlay::update_overlay(hwnd);
        unsafe { ShowCursor(FALSE); }
        return;
    }

    if is_move && unsafe { PEN_ACTIVE } {
        let pen_color = unsafe { STROKE_COLOR };
        drop(state);
        let w = pressure_to_width(pressure, width_scale);
        draw_pen_point(&data.surface, x, y, w, pen_color);
        overlay::update_overlay(hwnd);
        return;
    }

    if is_up && unsafe { PEN_ACTIVE } {
        unsafe { PEN_ACTIVE = false; HAS_LAST_PEN = false; }
        drop(state);
        unsafe { ShowCursor(TRUE); }
    }
}

fn draw_pen_point(surface: &crate::cairo::ImageSurface, x: f64, y: f64, w: f64, color: (f64, f64, f64)) {
    let sw = surface.width() as usize;
    let sh = surface.height() as usize;
    let stride = surface.stride() as usize;
    let px_vec = surface.pixels_mut();
    let px = px_vec.as_mut_slice();
    let cr = (color.0 * 255.0) as u32;
    let cg = (color.1 * 255.0) as u32;
    let cb = (color.2 * 255.0) as u32;
    let radius = w * 0.5;

    unsafe {
        if HAS_LAST_PEN {
            draw_thick_line(px, sw, sh, stride, LAST_PEN_X, LAST_PEN_Y, x, y, radius, cr, cg, cb, 255);
        } else {
            draw_filled_circle(px, sw, sh, stride, x, y, radius, cr, cg, cb, 255);
        }
        LAST_PEN_X = x;
        LAST_PEN_Y = y;
        HAS_LAST_PEN = true;
    }
}

/// WH_MOUSE_LL hook — don't swallow anything, let the overlay handle everything
unsafe extern "system" fn mouse_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    CallNextHookEx(None, code, wparam, lparam)
}

// --- Fast direct pixel drawing ---

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
fn put_pixel(pixels: &mut [u8], stride: usize, sw: usize, sh: usize, x: i32, y: i32, r: u32, g: u32, b: u32, a: u32) {
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
