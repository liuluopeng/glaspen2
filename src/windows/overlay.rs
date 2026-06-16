//! Overlay window (WS_EX_LAYERED|WS_EX_TRANSPARENT, TOPMOST) + pen-input window (non-layered, TOPMOST).
//!
//! Z-order (top to bottom):
//!   overlay (TOPMOST, LAYERED+TRANSPARENT) — display via UpdateLayeredWindow, mouse passthrough
//!   pen-input (TOPMOST, non-layered)       — receives WM_POINTER through the transparent overlay
//!   desktop apps

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use std::sync::Mutex;

use windows::core::PCWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Graphics::Dwm::DwmExtendFrameIntoClientArea;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Input::Pointer::*;

use crate::cairo::{Format, ImageSurface};

use super::render;

pub const WM_TRAY_COMMAND: u32 = WM_USER + 1;
const WM_REFRESH: u32 = WM_USER + 10;

pub const CMD_SELECT_COLOR: usize = 100;
pub const CMD_SELECT_WIDTH: usize = 200;
pub const CMD_SAVE_WITH_BG: usize = 300;
pub const CMD_SAVE_DRAWING: usize = 301;
pub const CMD_SAVE_XOJ: usize = 302;
pub const CMD_CLEAR_SCREEN: usize = 400;
pub const CMD_TOGGLE_RAINBOW: usize = 500;
pub const CMD_TOGGLE_ENABLED: usize = 600;
pub const CMD_TOGGLE_LANG: usize = 700;
pub const CMD_TOGGLE_OUTLINE: usize = 650;
pub const CMD_TOGGLE_INVERSE: usize = 651;
pub const CMD_QUIT: usize = 999;

pub const COLOR_PRESETS: [(f64, f64, f64); 10] = [
    (1.0, 0.0, 0.0), (1.0, 0.5, 0.0), (1.0, 1.0, 0.0), (0.0, 0.8, 0.0), (0.0, 0.8, 0.8),
    (0.0, 0.4, 1.0), (0.6, 0.0, 0.8), (1.0, 0.4, 0.7), (1.0, 1.0, 1.0), (0.0, 0.0, 0.0),
];
pub const COLOR_NAMES_ZH: [&str; 10] = ["红", "橙", "黄", "绿", "青", "蓝", "紫", "粉", "白", "黑"];
pub const WIDTH_PRESETS: [f64; 5] = [0.3, 0.6, 1.0, 1.5, 2.5];
pub const WIDTH_NAMES_ZH: [&str; 5] = ["极细", "细", "中", "粗", "极粗"];

pub struct DrawState {
    pub pen_r: f64, pub pen_g: f64, pub pen_b: f64,
    pub width_scale: f64,
    pub selected_color: usize, pub selected_width: usize,
    pub enabled: bool, pub show_rainbow: bool,
    pub outline_enabled: bool, pub inverse_enabled: bool,
    pub lang: i32,
}

pub static OVERLAY_HWND: Mutex<isize> = Mutex::new(0);

pub struct OverlayData {
    pub state: Mutex<DrawState>,
    pub surface: ImageSurface,
    hdc_mem: HDC,
    hbitmap: HBITMAP,
    dib_bits: *mut u8,
    dib_stride: usize,
    screen_w: i32,
    screen_h: i32,
    overlay_hwnd: HWND,
}

// Thread-local pointer for the input window to access shared data.
static mut SHARED_DATA_PTR: *mut OverlayData = ptr::null_mut();

pub fn get_overlay_data() -> Option<&'static mut OverlayData> {
    unsafe {
        if SHARED_DATA_PTR.is_null() { None } else { Some(&mut *SHARED_DATA_PTR) }
    }
}

pub fn run(screen_w: i32, screen_h: i32) {
    let hmodule = unsafe { GetModuleHandleW(None).unwrap() };
    let instance = HINSTANCE(hmodule.0);

    // ── Register window classes ──
    let ol_class = wide_string("Glaspen2Overlay");
    let wc_ol = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(overlay_wnd_proc),
        hInstance: instance,
        lpszClassName: PCWSTR(ol_class.as_ptr()),
        ..Default::default()
    };
    unsafe { RegisterClassExW(&wc_ol) };

    let pen_class = wide_string("Glaspen2Pen");
    let wc_pen = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(pen_wnd_proc),
        hInstance: instance,
        lpszClassName: PCWSTR(pen_class.as_ptr()),
        ..Default::default()
    };
    unsafe { RegisterClassExW(&wc_pen) };

    // ── Create pen-input window FIRST (below overlay in Z-order) ──
    let pen_ex = WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
    let pen_hwnd = unsafe {
        CreateWindowExW(
            pen_ex,
            PCWSTR(pen_class.as_ptr()),
            PCWSTR(wide_string("glaspen2_pen").as_ptr()),
            WS_POPUP,
            0, 0, screen_w, screen_h,
            None, None, instance, None,
        ).unwrap()
    };

    // ── Create overlay window SECOND (on top of pen window) ──
    let ol_ex = WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW;
    let overlay_hwnd = unsafe {
        CreateWindowExW(
            ol_ex,
            PCWSTR(ol_class.as_ptr()),
            PCWSTR(wide_string("glaspen2").as_ptr()),
            WS_POPUP,
            0, 0, screen_w, screen_h,
            None, None, instance, None,
        ).unwrap()
    };

    // DWM transparency for the overlay
    unsafe {
        let margins = MARGINS { cxLeftWidth: -1, cxRightWidth: -1, cyTopHeight: -1, cyBottomHeight: -1 };
        let _ = DwmExtendFrameIntoClientArea(overlay_hwnd, &margins);
    }

    { let mut h = OVERLAY_HWND.lock().unwrap(); *h = overlay_hwnd.0 as isize; }

    // ── Shared state ──
    let surface = ImageSurface::create(Format::ARGB32, screen_w, screen_h).unwrap();
    let (hdc_mem, hbitmap, dib_bits, dib_stride) = create_dib(screen_w, screen_h);

    let mut pen_r = 1.0; let mut pen_g = 0.0; let mut pen_b = 0.0; let mut width_scale = 1.0;
    crate::glaspen2_load_settings_parts(&mut pen_r, &mut pen_g, &mut pen_b, &mut width_scale);

    let outline_enabled = crate::db::load_setting("outline_enabled").and_then(|v| v.parse::<i32>().ok()).unwrap_or(0) != 0;
    let inverse_enabled = crate::db::load_setting("inverse_enabled").and_then(|v| v.parse::<i32>().ok()).unwrap_or(0) != 0;

    let state = DrawState {
        pen_r, pen_g, pen_b, width_scale,
        selected_color: 0, selected_width: 2,
        enabled: true, show_rainbow: false,
        outline_enabled, inverse_enabled, lang: 0,
    };

    let data = Box::new(OverlayData {
        state: Mutex::new(state),
        surface,
        hdc_mem, hbitmap, dib_bits, dib_stride,
        screen_w, screen_h, overlay_hwnd,
    });
    let ptr = Box::into_raw(data);
    unsafe { SHARED_DATA_PTR = ptr; }

    // Hotkeys on overlay
    unsafe {
        let mods = HOT_KEY_MODIFIERS(MOD_CONTROL.0 | MOD_ALT.0);
        RegisterHotKey(overlay_hwnd, 1, mods, 'C' as u32).ok();
        RegisterHotKey(overlay_hwnd, 2, mods, 'V' as u32).ok();
    }

    // Show both windows
    unsafe {
        ShowWindow(pen_hwnd, SW_SHOW);
        ShowWindow(overlay_hwnd, SW_SHOW);
    }

    // Initial render
    update_overlay();

    // Message loop — services both windows (same thread)
    let mut msg = MSG::default();
    loop {
        let result = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if result.0 == 0 { break; }
        unsafe { TranslateMessage(&msg); DispatchMessageW(&msg); }
    }

    // Cleanup
    unsafe {
        DeleteObject(hbitmap);
        DeleteDC(hdc_mem);
        DestroyWindow(pen_hwnd).ok();
        if !ptr.is_null() { drop(Box::from_raw(ptr)); }
    }
}

// ── Pen window proc (receives WM_POINTER through transparent overlay) ──

static mut PEN_ACTIVE: bool = false;
static mut LAST_X: f64 = 0.0;
static mut LAST_Y: f64 = 0.0;
static mut HAS_LAST: bool = false;
static mut STROKE_COLOR: (f64, f64, f64) = (1.0, 0.0, 0.0);

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

unsafe extern "system" fn pen_wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_NCHITTEST => {
            // Let legacy mouse events pass through to windows below.
            // WM_POINTER (pen) is NOT affected by this — it's routed separately.
            return LRESULT(HTTRANSPARENT as isize);
        }
        0x0246 | 0x0245 | 0x0247 => {
            let pid = (wparam.0 & 0xFFFF) as u32;
            if !is_pen(pid) { return DefWindowProcW(hwnd, msg, wparam, lparam); }

            let x = (lparam.0 as i32 & 0xFFFF) as f64;
            let y = ((lparam.0 as i32 >> 16) & 0xFFFF) as f64;
            let pressure = get_pressure(pid);

            static mut N: u32 = 0;
            N += 1;
            if N <= 5 { eprintln!("[pen] #{} msg={:#x} x={:.0} y={:.0} p={:.3}", N, msg, x, y, pressure); }

            let data = match get_overlay_data() { Some(d) => d, None => return LRESULT(0) };
            let state = data.state.lock().unwrap();
            if !state.enabled { return LRESULT(0); }
            let ws = state.width_scale;
            let (pr, pg, pb) = (state.pen_r, state.pen_g, state.pen_b);
            let inv = state.inverse_enabled;
            drop(state);

            let is_down = msg == 0x0246;
            let is_up = msg == 0x0247;

            if is_down && !PEN_ACTIVE {
                PEN_ACTIVE = true; HAS_LAST = false;
                STROKE_COLOR = if inv { sample_inverse(x, y) } else { (pr, pg, pb) };
                let w = pressure_to_width(pressure, ws);
                draw_point(&data.surface, x, y, w, STROKE_COLOR, false);
                let r = (w * 0.5 + 2.0) as i32;
                update_overlay_rect(x as i32 - r, y as i32 - r, x as i32 + r, y as i32 + r);
            } else if !is_down && !is_up && PEN_ACTIVE {
                let w = pressure_to_width(pressure, ws);
                draw_point(&data.surface, x, y, w, STROKE_COLOR, true);
                let r = (w * 0.5 + 2.0) as i32;
                let lx = LAST_X as i32;
                let ly = LAST_Y as i32;
                let cx = x as i32;
                let cy = y as i32;
                update_overlay_rect(
                    lx.min(cx) - r, ly.min(cy) - r,
                    lx.max(cx) + r, ly.max(cy) + r,
                );
            } else if is_up && PEN_ACTIVE {
                PEN_ACTIVE = false; HAS_LAST = false;
            }
            LRESULT(0)
        }
        WM_DESTROY => LRESULT(0),
        _ => DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

fn sample_inverse(x: f64, y: f64) -> (f64, f64, f64) {
    unsafe {
        let hdc = GetDC(None);
        if hdc.is_invalid() { return (1.0, 1.0, 1.0); }
        let c = GetPixel(hdc, x as i32, y as i32);
        ReleaseDC(None, hdc);
        if c.0 == 0xFFFFFFFF { return (1.0, 1.0, 1.0); }
        let r = (c.0 & 0xFF) as f64 / 255.0;
        let g = ((c.0 >> 8) & 0xFF) as f64 / 255.0;
        let b = ((c.0 >> 16) & 0xFF) as f64 / 255.0;
        (1.0 - r, 1.0 - g, 1.0 - b)
    }
}

fn pressure_to_width(p: f64, scale: f64) -> f64 {
    if p > 0.01 { (0.3 + p * p * 7.7) * scale } else { 1.0 * scale }
}

// ── Overlay window proc ──

unsafe extern "system" fn overlay_wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_REFRESH => { update_overlay(); LRESULT(0) }
        WM_HOTKEY => {
            let data = match get_overlay_data() { Some(d) => d, None => return LRESULT(0) };
            let mut state = data.state.lock().unwrap();
            match wparam.0 as i32 {
                1 => clear_screen(&mut state, data),
                2 => toggle_enabled(&mut state),
                _ => {}
            }
            LRESULT(0)
        }
        WM_TRAY_COMMAND => {
            let data = match get_overlay_data() { Some(d) => d, None => return LRESULT(0) };
            let mut state = data.state.lock().unwrap();
            handle_command(&mut state, data, wparam.0, lparam.0 as usize);
            LRESULT(0)
        }
        WM_DESTROY => { PostQuitMessage(0); LRESULT(0) }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

// ── Drawing ──

fn draw_point(surface: &ImageSurface, x: f64, y: f64, w: f64,
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
            draw_line(px, sw, sh, stride, LAST_X, LAST_Y, x, y, radius, cr, cg, cb, 255);
        } else {
            draw_circle(px, sw, sh, stride, x, y, radius, cr, cg, cb, 255);
        }
        LAST_X = x; LAST_Y = y; HAS_LAST = true;
    }
}

fn draw_circle(pixels: &mut [u8], sw: usize, sh: usize, stride: usize,
               cx: f64, cy: f64, radius: f64, r: u32, g: u32, b: u32, a: u32) {
    if radius < 0.5 { return; }
    let r_sq = radius * radius;
    let r_inner_sq = (radius - 0.5).max(0.0).powi(2);
    let x0 = (cx - radius - 1.0).floor() as i32;
    let x1 = (cx + radius + 1.0).ceil() as i32;
    let y0 = (cy - radius - 1.0).floor() as i32;
    let y1 = (cy + radius + 1.0).ceil() as i32;
    for py in y0..=y1 {
        for px in x0..=x1 {
            let dx = px as f64 + 0.5 - cx;
            let dy = py as f64 + 0.5 - cy;
            let dsq = dx * dx + dy * dy;
            if dsq < r_inner_sq {
                set_pixel(pixels, stride, sw, sh, px, py, r, g, b, a);
            } else if dsq < r_sq {
                let alpha = (a as f64 * (0.5 + radius - dsq.sqrt())).min(255.0) as u32;
                if alpha > 0 { set_pixel(pixels, stride, sw, sh, px, py, r, g, b, alpha); }
            }
        }
    }
}

fn draw_line(pixels: &mut [u8], sw: usize, sh: usize, stride: usize,
             x0: f64, y0: f64, x1: f64, y1: f64, radius: f64,
             r: u32, g: u32, b: u32, a: u32) {
    let d = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt();
    let n = (d / (radius * 0.5).max(1.0)).ceil() as i32;
    for i in 0..=n {
        let t = if n == 0 { 0.0 } else { i as f64 / n as f64 };
        draw_circle(pixels, sw, sh, stride, x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, radius, r, g, b, a);
    }
}

#[inline]
fn set_pixel(pixels: &mut [u8], stride: usize, sw: usize, sh: usize,
              x: i32, y: i32, r: u32, g: u32, b: u32, a: u32) {
    if x < 0 || y < 0 || x as usize >= sw || y as usize >= sh { return; }
    let off = y as usize * stride + x as usize * 4;
    if off + 3 >= pixels.len() { return; }
    let sa = a as f32 / 255.0;
    let da = pixels[off + 3] as f32 / 255.0;
    let inv = 1.0 - sa;
    let out_a = sa + da * inv;
    if out_a < 0.004 { return; }
    pixels[off] = (b as f32 * sa + pixels[off] as f32 * inv * da) as u8;
    pixels[off + 1] = (g as f32 * sa + pixels[off + 1] as f32 * inv * da) as u8;
    pixels[off + 2] = (r as f32 * sa + pixels[off + 2] as f32 * inv * da) as u8;
    pixels[off + 3] = (out_a * 255.0).min(255.0) as u8;
}

// ── UpdateLayeredWindow (incremental) ──

pub fn update_overlay() {
    update_overlay_rect(-1, -1, -1, -1); // full update
}

pub fn update_overlay_rect(x0: i32, y0: i32, x1: i32, y1: i32) {
    let data = match get_overlay_data() { Some(d) => d, None => return };
    let surf_data = data.surface.data().unwrap();
    let surf_stride = data.surface.stride() as usize;
    let dib = unsafe { std::slice::from_raw_parts_mut(data.dib_bits, data.dib_stride * data.screen_h as usize) };

    let (left, top, right, bottom) = if x0 < 0 {
        // Full copy
        for y in 0..data.screen_h as usize {
            for x in 0..data.screen_w as usize {
                let s = y * surf_stride + x * 4;
                let d = y * data.dib_stride + x * 4;
                if s + 3 < surf_data.len() && d + 3 < dib.len() {
                    dib[d] = surf_data[s]; dib[d+1] = surf_data[s+1];
                    dib[d+2] = surf_data[s+2]; dib[d+3] = surf_data[s+3];
                }
            }
        }
        (0, 0, data.screen_w, data.screen_h)
    } else {
        let l = x0.max(0) as usize;
        let t = y0.max(0) as usize;
        let r = (x1 + 1).min(data.screen_w) as usize;
        let b = (y1 + 1).min(data.screen_h) as usize;
        for y in t..b {
            for x in l..r {
                let s = y * surf_stride + x * 4;
                let d = y * data.dib_stride + x * 4;
                if s + 3 < surf_data.len() && d + 3 < dib.len() {
                    dib[d] = surf_data[s]; dib[d+1] = surf_data[s+1];
                    dib[d+2] = surf_data[s+2]; dib[d+3] = surf_data[s+3];
                }
            }
        }
        (l as i32, t as i32, r as i32, b as i32)
    };

    let blend_fn = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
    let size = SIZE { cx: right - left, cy: bottom - top };
    let pt_dst = POINT { x: left, y: top };
    let pt_src = POINT { x: left, y: top };

    unsafe {
        let _ = UpdateLayeredWindow(
            data.overlay_hwnd, None,
            Some(&pt_dst), Some(&size), data.hdc_mem, Some(&pt_src),
            COLORREF(0), Some(&blend_fn), ULW_ALPHA,
        );
    }
}

fn create_dib(w: i32, h: i32) -> (HDC, HBITMAP, *mut u8, usize) {
    unsafe {
        let hdc_s = GetDC(None);
        let hdc_m = CreateCompatibleDC(hdc_s);
        ReleaseDC(None, hdc_s);
        let stride = (w as usize * 4 + 3) & !3;
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w, biHeight: -h, biPlanes: 1, biBitCount: 32,
                biCompression: BI_RGB.0, ..Default::default()
            }, ..Default::default()
        };
        let mut bits: *mut std::ffi::c_void = ptr::null_mut();
        let hbmp = CreateDIBSection(hdc_m, &bmi, DIB_RGB_COLORS, &mut bits, None, 0).unwrap();
        SelectObject(hdc_m, HGDIOBJ(hbmp.0 as _));
        (hdc_m, hbmp, bits as *mut u8, stride)
    }
}

fn wide_string(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

// ── Command handlers ──

fn handle_command(state: &mut DrawState, data: &OverlayData, cmd: usize, _param: usize) {
    if cmd >= CMD_SELECT_COLOR && cmd < CMD_SELECT_COLOR + 10 {
        let idx = cmd - CMD_SELECT_COLOR;
        if idx < COLOR_PRESETS.len() {
            state.pen_r = COLOR_PRESETS[idx].0;
            state.pen_g = COLOR_PRESETS[idx].1;
            state.pen_b = COLOR_PRESETS[idx].2;
            state.selected_color = idx;
            crate::glaspen2_save_settings(state.pen_r, state.pen_g, state.pen_b, state.width_scale);
            super::tray::update_tray_icon(state);
        }
    } else if cmd >= CMD_SELECT_WIDTH && cmd < CMD_SELECT_WIDTH + 5 {
        let idx = cmd - CMD_SELECT_WIDTH;
        if idx < WIDTH_PRESETS.len() {
            state.width_scale = WIDTH_PRESETS[idx];
            state.selected_width = idx;
            crate::glaspen2_save_settings(state.pen_r, state.pen_g, state.pen_b, state.width_scale);
        }
    } else {
        match cmd {
            x if x == CMD_SAVE_WITH_BG => save_with_bg(data),
            x if x == CMD_SAVE_DRAWING => save_drawing(data),
            x if x == CMD_SAVE_XOJ => crate::glaspen2_save_xoj(),
            x if x == CMD_CLEAR_SCREEN => clear_screen(state, data),
            x if x == CMD_TOGGLE_RAINBOW => {
                state.show_rainbow = !state.show_rainbow;
                super::tray::update_rainbow_checkmark(state.show_rainbow);
                if state.show_rainbow { render::draw_rainbow_indicator(&data.surface); update_overlay(); }
                else { clear_screen(state, data); }
            }
            x if x == CMD_TOGGLE_OUTLINE => {
                state.outline_enabled = !state.outline_enabled;
                crate::db::save_setting("outline_enabled", if state.outline_enabled { "1" } else { "0" });
                super::tray::update_outline_checkmark(state.outline_enabled);
            }
            x if x == CMD_TOGGLE_INVERSE => {
                state.inverse_enabled = !state.inverse_enabled;
                crate::db::save_setting("inverse_enabled", if state.inverse_enabled { "1" } else { "0" });
                super::tray::update_inverse_checkmark(state.inverse_enabled);
            }
            x if x == CMD_TOGGLE_ENABLED => toggle_enabled(state),
            x if x == CMD_QUIT => { unsafe { DestroyWindow(data.overlay_hwnd).ok(); } }
            _ => {}
        }
    }
}

fn clear_screen(state: &mut DrawState, data: &OverlayData) {
    render::clear_screen(&data.surface);
    crate::glaspen2_clear_strokes(data.screen_w, data.screen_h);
    if state.show_rainbow { render::draw_rainbow_indicator(&data.surface); }
    update_overlay();
}

fn toggle_enabled(state: &mut DrawState) {
    state.enabled = !state.enabled;
    super::tray::update_tray_icon(state);
    super::tray::update_enabled_item(state.enabled, state.lang);
}

fn save_drawing(data: &OverlayData) {
    let w = data.surface.width();
    let h = data.surface.height();
    let stride = data.surface.stride();
    let surface_data = data.surface.data().unwrap();
    crate::glaspen2_save_drawing(surface_data.as_ptr(), w, h, stride);
}

fn save_with_bg(data: &OverlayData) {
    let screen_dc = unsafe { GetDC(None) };
    let bw = data.screen_w; let bh = data.screen_h;
    let bg_dc = unsafe { CreateCompatibleDC(screen_dc) };
    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: bw, biHeight: -bh, biPlanes: 1, biBitCount: 32,
            biCompression: BI_RGB.0, ..Default::default()
        }, ..Default::default()
    };
    let mut bg_bits: *mut std::ffi::c_void = ptr::null_mut();
    let bg_bmp = unsafe { CreateDIBSection(bg_dc, &bmi, DIB_RGB_COLORS, &mut bg_bits, None, 0).unwrap() };
    unsafe { SelectObject(bg_dc, HGDIOBJ(bg_bmp.0 as _)); BitBlt(bg_dc, 0, 0, bw, bh, screen_dc, 0, 0, SRCCOPY).unwrap(); }
    let draw_w = data.surface.width(); let draw_h = data.surface.height();
    let draw_stride = data.surface.stride() as i32;
    let draw_data = data.surface.data().unwrap();
    unsafe {
        crate::glaspen2_save_with_background(draw_data.as_ptr(), draw_w, draw_h, draw_stride, bg_bits as *const u8, bw, bh, bw * 4);
        SelectObject(bg_dc, HGDIOBJ(bg_bmp.0 as _)); let _ = DeleteObject(bg_bmp); let _ = DeleteDC(bg_dc); let _ = ReleaseDC(None, screen_dc);
    }
}
