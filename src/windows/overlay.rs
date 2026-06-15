use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use std::sync::Mutex;

use windows::core::PCWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;

use crate::cairo::{Format, ImageSurface};

use super::render;
use super::input;

pub const WM_TRAY_COMMAND: u32 = WM_USER + 1;
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
    pub last_x: f64, pub last_y: f64, pub has_last: bool,
    pub cursor_x: f64, pub cursor_y: f64, pub cursor_visible: bool,
    pub notification: Option<String>, pub notification_time: Option<std::time::Instant>,
}

pub static OVERLAY_HWND: Mutex<isize> = Mutex::new(0);

pub fn run(screen_w: i32, screen_h: i32) {
    let hmodule = unsafe { GetModuleHandleW(None).unwrap() };
    let instance = HINSTANCE(hmodule.0);
    let class_name = wide_string("Glaspen2Overlay");

    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(wnd_proc),
        hInstance: instance,
        lpszClassName: PCWSTR(class_name.as_ptr()),
        ..Default::default()
    };
    unsafe { RegisterClassExW(&wc) };

    // WS_EX_LAYERED overlay: transparent to input, uses UpdateLayeredWindow for rendering
    let ex_style = WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW;
    let style = WS_POPUP;

    let window_name = wide_string("glaspen2");
    let hwnd = unsafe {
        CreateWindowExW(
            ex_style,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(window_name.as_ptr()),
            style,
            0, 0, screen_w, screen_h,
            None,
            None,
            instance,
            None,
        ).unwrap()
    };

    { let mut h = OVERLAY_HWND.lock().unwrap(); *h = hwnd.0 as isize; }

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
        last_x: -1.0, last_y: -1.0, has_last: false,
        cursor_x: -100.0, cursor_y: -100.0, cursor_visible: false,
        notification: None, notification_time: None,
    };

    let state_box = Box::new(OverlayData {
        state: Mutex::new(state), surface, hdc_mem, hbitmap,
        dib_bits, dib_stride, screen_w, screen_h,
    });
    let state_ptr = Box::into_raw(state_box);
    unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize) };

    input::install_hook(hwnd);

    unsafe {
        let mods = HOT_KEY_MODIFIERS(MOD_CONTROL.0 | MOD_ALT.0);
        RegisterHotKey(hwnd, 1, mods, 'C' as u32).ok();
        RegisterHotKey(hwnd, 2, mods, 'V' as u32).ok();
        RegisterHotKey(hwnd, 3, mods, 'J' as u32).ok();
        RegisterHotKey(hwnd, 4, mods, 'K' as u32).ok();
    }

    update_overlay(hwnd);
    unsafe { ShowWindow(hwnd, SW_SHOW) };

    let mut msg = MSG::default();
    loop {
        let result = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if result.0 == 0 { break; }
        unsafe { TranslateMessage(&msg); DispatchMessageW(&msg); }
    }

    input::uninstall_hook();
    unsafe {
        DeleteObject(hbitmap);
        DeleteDC(hdc_mem);
        let ptr = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *mut OverlayData;
        if !ptr.is_null() { drop(Box::from_raw(ptr)); }
    }
}

pub struct OverlayData {
    pub state: Mutex<DrawState>,
    pub surface: ImageSurface,
    hdc_mem: HDC,
    hbitmap: HBITMAP,
    dib_bits: *mut u8,
    dib_stride: usize,
    screen_w: i32,
    screen_h: i32,
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_HOTKEY => {
            let data = get_overlay_data(hwnd);
            if let Some(data) = data {
                let mut state = data.state.lock().unwrap();
                match wparam.0 as i32 {
                    1 => clear_screen(&mut state, hwnd, data),
                    2 => toggle_enabled(&mut state, hwnd),
                    3 => navigate_prev(&mut state, hwnd, data),
                    4 => navigate_next(&mut state, hwnd, data),
                    _ => {}
                }
            }
            LRESULT(0)
        }
        WM_TRAY_COMMAND => {
            let data = get_overlay_data(hwnd);
            if let Some(data) = data {
                let mut state = data.state.lock().unwrap();
                let cmd = wparam.0;
                let param = lparam.0 as usize;
                handle_command(&mut state, hwnd, data, cmd, param);
            }
            LRESULT(0)
        }
        WM_USER_NOTIFICATION => {
            let data = get_overlay_data(hwnd);
            if let Some(data) = data {
                let mut state = data.state.lock().unwrap();
                let text_ptr = lparam.0 as *mut String;
                if !text_ptr.is_null() {
                    let text = Box::from_raw(text_ptr);
                    state.notification = Some(*text);
                    state.notification_time = Some(std::time::Instant::now());
                }
                update_overlay(hwnd);
            }
            LRESULT(0)
        }
        WM_TIMER => {
            let data = get_overlay_data(hwnd);
            if let Some(data) = data {
                let mut state = data.state.lock().unwrap();
                if let Some(t) = state.notification_time {
                    if t.elapsed().as_secs() >= 1 {
                        state.notification = None;
                        state.notification_time = None;
                    }
                }
                update_overlay(hwnd);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

const WM_USER_NOTIFICATION: u32 = WM_USER + 2;

pub fn get_overlay_data(hwnd: HWND) -> Option<&'static mut OverlayData> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut OverlayData;
        if ptr.is_null() { None } else { Some(&mut *ptr) }
    }
}

fn handle_command(state: &mut DrawState, hwnd: HWND, data: &OverlayData, cmd: usize, _param: usize) {
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
            x if x == CMD_SAVE_WITH_BG => save_with_background(state, data),
            x if x == CMD_SAVE_DRAWING => save_drawing_only(data),
            x if x == CMD_SAVE_XOJ => { crate::glaspen2_save_xoj(); show_notification(state, hwnd, "Notes saved"); }
            x if x == CMD_CLEAR_SCREEN => clear_screen(state, hwnd, data),
            x if x == CMD_TOGGLE_RAINBOW => {
                state.show_rainbow = !state.show_rainbow;
                super::tray::update_rainbow_checkmark(state.show_rainbow);
                if state.show_rainbow { render::draw_rainbow_indicator(&data.surface); update_overlay(hwnd); }
                else { clear_screen(state, hwnd, data); }
            }
            x if x == CMD_TOGGLE_OUTLINE => {
                state.outline_enabled = !state.outline_enabled;
                crate::db::save_setting("outline_enabled", if state.outline_enabled { "1" } else { "0" });
                super::tray::update_outline_checkmark(state.outline_enabled);
                rebuild_surface(state, data);
                update_overlay(hwnd);
            }
            x if x == CMD_TOGGLE_INVERSE => {
                state.inverse_enabled = !state.inverse_enabled;
                crate::db::save_setting("inverse_enabled", if state.inverse_enabled { "1" } else { "0" });
                super::tray::update_inverse_checkmark(state.inverse_enabled);
            }
            x if x == CMD_TOGGLE_ENABLED => toggle_enabled(state, hwnd),
            x if x == CMD_TOGGLE_LANG => { state.lang = 1 - state.lang; super::tray::update_menu_texts(state); }
            x if x == CMD_QUIT => {
                unsafe { input::uninstall_hook(); ShowCursor(TRUE); DestroyWindow(hwnd).ok(); }
            }
            _ => {}
        }
    }
}

fn clear_screen(state: &mut DrawState, hwnd: HWND, data: &OverlayData) {
    render::clear_screen(&data.surface);
    state.has_last = false;
    crate::glaspen2_clear_strokes(data.screen_w, data.screen_h);
    if state.show_rainbow { render::draw_rainbow_indicator(&data.surface); }
    update_overlay(hwnd);
    show_notification(state, hwnd, "Screen cleared");
}

fn navigate_prev(state: &mut DrawState, hwnd: HWND, data: &OverlayData) {
    let target = crate::glaspen2_prev_screen_id();
    if target == 0 { show_notification(state, hwnd, "No previous page"); return; }
    goto_screen(state, hwnd, data, target);
}

fn navigate_next(state: &mut DrawState, hwnd: HWND, data: &OverlayData) {
    let target = crate::glaspen2_next_screen_id();
    if target == 0 { show_notification(state, hwnd, "No next page"); return; }
    goto_screen(state, hwnd, data, target);
}

fn goto_screen(state: &mut DrawState, hwnd: HWND, data: &OverlayData, screen_id: i64) {
    render::clear_screen(&data.surface);
    state.has_last = false;
    crate::glaspen2_load_strokes_for_screen(screen_id);
    crate::glaspen2_smooth_loaded_strokes();
    replay_strokes(data);
    if state.show_rainbow { render::draw_rainbow_indicator(&data.surface); }
    update_overlay(hwnd);
}

fn replay_strokes(data: &OverlayData) {
    let strokes = crate::STROKES.lock().unwrap();
    for stroke in strokes.iter() {
        if stroke.points.len() < 2 { continue; }
        for i in 1..stroke.points.len() {
            let (x0, y0, _) = stroke.points[i - 1];
            let (x1, y1, pw) = stroke.points[i];
            render::pen_draw(&data.surface, x1, y1, pw, stroke.r, stroke.g, stroke.b, x0, y0, true);
        }
    }
}

fn rebuild_surface(state: &DrawState, data: &OverlayData) {
    render::clear_screen(&data.surface);
    if state.show_rainbow { render::draw_rainbow_indicator(&data.surface); }
    let strokes = crate::STROKES.lock().unwrap();
    for stroke in strokes.iter() {
        if stroke.points.is_empty() { continue; }
        for i in 0..stroke.points.len() {
            let (x, y, pw) = stroke.points[i];
            if i == 0 { render::draw_dot(&data.surface, x, y, pw, stroke.r, stroke.g, stroke.b); }
            else { let (px, py, _) = stroke.points[i - 1]; render::pen_draw(&data.surface, x, y, pw, stroke.r, stroke.g, stroke.b, px, py, true); }
        }
    }
}

fn toggle_enabled(state: &mut DrawState, hwnd: HWND) {
    state.enabled = !state.enabled;
    super::tray::update_tray_icon(state);
    super::tray::update_enabled_item(state.enabled, state.lang);
    let msg = if state.enabled { "Drawing enabled" } else { "Drawing disabled" };
    show_notification(state, hwnd, msg);
}

fn show_notification(state: &mut DrawState, hwnd: HWND, text: &str) {
    state.notification = Some(text.to_string());
    state.notification_time = Some(std::time::Instant::now());
    unsafe { SetTimer(hwnd, 1, 1000, None) };
    update_overlay(hwnd);
}

fn save_drawing_only(data: &OverlayData) {
    let w = data.surface.width();
    let h = data.surface.height();
    let stride = data.surface.stride();
    let surface_data = data.surface.data().unwrap();
    crate::glaspen2_save_drawing(surface_data.as_ptr(), w, h, stride);
}

fn save_with_background(_state: &DrawState, data: &OverlayData) {
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

pub fn update_overlay(hwnd: HWND) {
    let data = match get_overlay_data(hwnd) { Some(d) => d, None => return };
    render::copy_surface_to_bgra(&data.surface, unsafe {
        std::slice::from_raw_parts_mut(data.dib_bits, data.dib_stride * data.screen_h as usize)
    });
    let (cursor_visible, cursor_x, cursor_y, notification) = {
        let state = data.state.lock().unwrap();
        (state.cursor_visible, state.cursor_x, state.cursor_y, state.notification.clone())
    };
    if cursor_visible || notification.is_some() {
        let overlay_surface = ImageSurface::create(Format::ARGB32, data.screen_w, data.screen_h).unwrap();
        if cursor_visible && cursor_x >= 0.0 { render::draw_crosshair(&overlay_surface, cursor_x, cursor_y); }
        if let Some(ref text) = notification { render::draw_notification(&overlay_surface, text); }
        let overlay_data = overlay_surface.data().unwrap();
        let overlay_stride = overlay_surface.stride() as usize;
        let dib = unsafe { std::slice::from_raw_parts_mut(data.dib_bits, data.dib_stride * data.screen_h as usize) };
        for y in 0..data.screen_h as usize {
            for x in 0..data.screen_w as usize {
                let o_off = y * overlay_stride + x * 4;
                let d_off = y * data.dib_stride + x * 4;
                if o_off + 3 < overlay_data.len() && d_off + 3 < dib.len() {
                    let alpha = overlay_data[o_off + 3] as f32 / 255.0;
                    if alpha > 0.01 {
                        dib[d_off] = (overlay_data[o_off] as f32 + dib[d_off] as f32 * (1.0 - alpha)) as u8;
                        dib[d_off + 1] = (overlay_data[o_off + 1] as f32 + dib[d_off + 1] as f32 * (1.0 - alpha)) as u8;
                        dib[d_off + 2] = (overlay_data[o_off + 2] as f32 + dib[d_off + 2] as f32 * (1.0 - alpha)) as u8;
                        dib[d_off + 3] = ((overlay_data[o_off + 3] as f32) + dib[d_off + 3] as f32 * (1.0 - alpha)).min(255.0) as u8;
                    }
                }
            }
        }
    }
    let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
    let point = POINT { x: 0, y: 0 };
    let size = SIZE { cx: data.screen_w, cy: data.screen_h };
    let point_src = POINT { x: 0, y: 0 };
    unsafe { UpdateLayeredWindow(hwnd, None, Some(&point as *const POINT), Some(&size as *const SIZE), data.hdc_mem, Some(&point_src as *const POINT), COLORREF(0), Some(&blend as *const BLENDFUNCTION), ULW_ALPHA).ok(); }
}

fn create_dib(w: i32, h: i32) -> (HDC, HBITMAP, *mut u8, usize) {
    unsafe {
        let hdc_screen = GetDC(None);
        let hdc_mem = CreateCompatibleDC(hdc_screen);
        ReleaseDC(None, hdc_screen);
        let stride = (w as usize * 4 + 3) & !3;
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w, biHeight: -h, biPlanes: 1, biBitCount: 32,
                biCompression: BI_RGB.0, ..Default::default()
            }, ..Default::default()
        };
        let mut bits: *mut std::ffi::c_void = ptr::null_mut();
        let hbitmap = CreateDIBSection(hdc_mem, &bmi, DIB_RGB_COLORS, &mut bits, None, 0).unwrap();
        SelectObject(hdc_mem, HGDIOBJ(hbitmap.0 as _));
        (hdc_mem, hbitmap, bits as *mut u8, stride)
    }
}

fn wide_string(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}
