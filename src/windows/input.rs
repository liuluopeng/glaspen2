use std::sync::Mutex;
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

use super::overlay;
use super::render;

static HOOK_HANDLE: Mutex<isize> = Mutex::new(0);
static HOOK_HWND: Mutex<isize> = Mutex::new(0);
static mut PEN_DRAWING: bool = false;
static mut STROKE_COLOR: (f64, f64, f64) = (1.0, 0.0, 0.0);

pub fn install_hook(hwnd: HWND) {
    let hmodule = unsafe { GetModuleHandleW(None).unwrap() };
    let instance = HINSTANCE(hmodule.0);
    let hook = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), instance, 0).unwrap() };
    let mut h = HOOK_HANDLE.lock().unwrap();
    *h = hook.0 as isize;
    set_hook_hwnd(hwnd);
}

pub fn uninstall_hook() {
    let h = HOOK_HANDLE.lock().unwrap();
    if *h != 0 {
        unsafe { let hook = HHOOK(*h as *mut _); UnhookWindowsHookEx(hook).ok(); }
    }
    unsafe { if PEN_DRAWING { ShowCursor(TRUE); PEN_DRAWING = false; } }
}

fn set_hook_hwnd(hwnd: HWND) {
    let mut h = HOOK_HWND.lock().unwrap();
    *h = hwnd.0 as isize;
}

fn get_hook_hwnd() -> Option<HWND> {
    let h = HOOK_HWND.lock().unwrap();
    if *h == 0 { None } else { Some(HWND(*h as *mut _)) }
}

fn is_pen_event(extra_info: usize) -> bool {
    if extra_info == 0 { return false; }
    let val = extra_info as u32;
    if val > 0x100 { return true; }
    false
}

fn estimate_pressure(_extra_info: usize) -> f64 { 0.5 }

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

/// Draw smoothed modeler points directly into the surface pixel buffer.
fn draw_modeler_buffer(surface: &crate::cairo::ImageSurface, r: f64, g: f64, b: f64) {
    let count = crate::glaspen2_modeler_point_count() as usize;
    if count < 1 { return; }

    let sw = surface.width() as usize;
    let sh = surface.height() as usize;
    let stride = surface.stride() as usize;
    let px_vec = surface.pixels_mut();
    let px = px_vec.as_mut_slice();

    let cr = (r * 255.0) as u32;
    let cg = (g * 255.0) as u32;
    let cb = (b * 255.0) as u32;

    let mut prev_x = 0.0f64;
    let mut prev_y = 0.0f64;

    for i in 0..count {
        let mut pt_x = 0.0f64;
        let mut pt_y = 0.0f64;
        let mut pw = 0.0f64;
        crate::glaspen2_modeler_get_point(i as i32, &mut pt_x, &mut pt_y, &mut pw);
        let radius = pw * 0.5;
        if i == 0 {
            draw_filled_circle(px, sw, sh, stride, pt_x, pt_y, radius, cr, cg, cb, 255);
        } else {
            draw_thick_line(px, sw, sh, stride, prev_x, prev_y, pt_x, pt_y, radius, cr, cg, cb, 255);
        }
        prev_x = pt_x;
        prev_y = pt_y;
    }

    crate::glaspen2_modeler_commit_to_strokes(r, g, b, std::ptr::null(), 0);
    crate::glaspen2_modeler_clear_buffer();
}

unsafe extern "system" fn mouse_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 { return CallNextHookEx(None, code, wparam, lparam); }

    let hwnd = match get_hook_hwnd() { Some(h) => h, None => return CallNextHookEx(None, code, wparam, lparam) };
    let data = match overlay::get_overlay_data(hwnd) { Some(d) => d, None => return CallNextHookEx(None, code, wparam, lparam) };

    let hook_struct = &*(lparam.0 as *const MSLLHOOKSTRUCT);
    let extra_info = hook_struct.dwExtraInfo;
    let is_pen = is_pen_event(extra_info);
    let msg = wparam.0 as u32;
    let x = hook_struct.pt.x as f64;
    let y = hook_struct.pt.y as f64;
    let timestamp = hook_struct.time as f64 / 1000.0;

    if !is_pen {
        return CallNextHookEx(None, code, wparam, lparam);
    }

    let mut state = data.state.lock().unwrap();
    if !state.enabled { return CallNextHookEx(None, code, wparam, lparam); }

    let width_scale = state.width_scale;
    let pressure = estimate_pressure(extra_info);

    if msg == WM_LBUTTONDOWN && !PEN_DRAWING {
        ShowCursor(FALSE);
        PEN_DRAWING = true;
        let pen_color = if state.inverse_enabled { sample_screen_inverse(x, y) }
                        else { (state.pen_r, state.pen_g, state.pen_b) };
        STROKE_COLOR = pen_color;
        drop(state);
        crate::glaspen2_modeler_begin(pen_color.0, pen_color.1, pen_color.2, x, y, pressure, timestamp, width_scale);
        return LRESULT(1);
    }

    if msg == WM_MOUSEMOVE && PEN_DRAWING {
        let pen_color = STROKE_COLOR;
        drop(state);
        crate::glaspen2_modeler_move(x, y, pressure, timestamp, width_scale);
        draw_modeler_buffer(&data.surface, pen_color.0, pen_color.1, pen_color.2);
        overlay::update_overlay(hwnd);
        return LRESULT(1);
    }

    if msg == WM_LBUTTONUP && PEN_DRAWING {
        ShowCursor(TRUE);
        PEN_DRAWING = false;
        let pen_color = STROKE_COLOR;
        drop(state);
        crate::glaspen2_modeler_end(x, y, pressure, timestamp, width_scale);
        draw_modeler_buffer(&data.surface, pen_color.0, pen_color.1, pen_color.2);
        overlay::update_overlay(hwnd);
        return LRESULT(1);
    }

    CallNextHookEx(None, code, wparam, lparam)
}

// --- Fast direct pixel drawing functions ---

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
