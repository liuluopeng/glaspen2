//! WH_MOUSE_LL hook + Raw Input overlay (GlasPen3 approach, ported to Rust).
//!
//! Architecture:
//!   Single WS_EX_LAYERED+TRANSPARENT+TOPMOST window for display.
//!   WH_MOUSE_LL global hook intercepts ALL mouse events:
//!     - Pen events → suppressed (return 1), ink drawn on overlay
//!     - Mouse events → pass through via CallNextHookEx
//!   Raw Input (RegisterRawInputDevices) detects pen vs mouse:
//!     - MOUSE_MOVE_ABSOLUTE flag → pen (tablet)
//!     - MOUSE_MOVE_RELATIVE flag → real mouse
//!   HID digitizer provides pressure when driver reports it (Ink ON).
//!   Works with Ink OFF — no WM_POINTER dependency.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::time::Instant;

use windows::core::PCWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Graphics::Dwm::DwmExtendFrameIntoClientArea;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;

use crate::cairo::{Format, ImageSurface};

use super::render;

// ── Raw Input types/consts (declared manually for precise control) ──

const RIM_TYPEMOUSE: u32 = 0;
const RIM_TYPEHID: u32 = 2;
const RID_INPUT: u32 = 0x10000003;
const RIDEV_INPUTSINK: u32 = 0x00000100;

const MOUSE_MOVE_ABSOLUTE: u16 = 0x0001;
const RI_MOUSE_LEFT_BUTTON_DOWN: u16 = 0x0001;
const RI_MOUSE_LEFT_BUTTON_UP: u16 = 0x0002;

#[repr(C)]
struct RawInputDevice {
    us_usage_page: u16,
    us_usage: u16,
    dw_flags: u32,
    hwnd_target: isize,
}

#[repr(C)]
struct RawInputHeader {
    dw_type: u32,
    dw_size: u32,
    h_device: isize,
    w_param: usize,
}

#[repr(C)]
struct RawMouse {
    us_flags: u16,
    _pad: u16, // alignment
    us_button_flags: u16,
    us_button_data: u16,
    ul_raw_buttons: u32,
    l_last_x: i32,
    l_last_y: i32,
    ul_extra_information: u32,
}

extern "system" {
    fn RegisterRawInputDevices(
        p_raw_input_devices: *const RawInputDevice,
        ui_num_devices: u32,
        cb_size: u32,
    ) -> BOOL;

    fn GetRawInputData(
        h_raw_input: isize,
        ui_command: u32,
        p_data: *mut u8,
        pcb_size: *mut u32,
        cb_size_header: u32,
    ) -> u32;
}

// ── Custom messages ──
pub const WM_TRAY_COMMAND: u32 = WM_USER + 1;

// ── Command IDs (match overlay.rs for tray compatibility) ──
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

// ── Color & width presets ──
pub const COLOR_PRESETS: [(f64, f64, f64); 10] = [
    (1.0, 0.0, 0.0), (1.0, 0.5, 0.0), (1.0, 1.0, 0.0), (0.0, 0.8, 0.0), (0.0, 0.8, 0.8),
    (0.0, 0.4, 1.0), (0.6, 0.0, 0.8), (1.0, 0.4, 0.7), (1.0, 1.0, 1.0), (0.0, 0.0, 0.0),
];
pub const COLOR_NAMES_ZH: [&str; 10] = ["红", "橙", "黄", "绿", "青", "蓝", "紫", "粉", "白", "黑"];
pub const WIDTH_PRESETS: [f64; 5] = [0.3, 0.6, 1.0, 1.5, 2.5];
pub const WIDTH_NAMES_ZH: [&str; 5] = ["极细", "细", "中", "粗", "极粗"];

// ── DrawState ──
pub struct DrawState {
    pub pen_r: f64, pub pen_g: f64, pub pen_b: f64,
    pub width_scale: f64,
    pub selected_color: usize, pub selected_width: usize,
    pub enabled: bool, pub show_rainbow: bool,
    pub outline_enabled: bool, pub inverse_enabled: bool,
    pub lang: i32,
}

// ── Shared state ──
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

static mut SHARED_DATA_PTR: *mut OverlayData = ptr::null_mut();

pub fn get_overlay_data() -> Option<&'static mut OverlayData> {
    unsafe {
        if SHARED_DATA_PTR.is_null() { None } else { Some(&mut *SHARED_DATA_PTR) }
    }
}

// ── Pen tracking (shared with hook callback via atomics) ──
static LAST_PEN_TICK: AtomicI32 = AtomicI32::new(i32::MIN);
static HID_TIP_DOWN: AtomicBool = AtomicBool::new(false);
static PEN_ACTIVE: AtomicBool = AtomicBool::new(false);
static mut LAST_DRAW_X: f64 = 0.0;
static mut LAST_DRAW_Y: f64 = 0.0;
static mut HAS_LAST_DRAW: bool = false;

const SUPPRESS_WINDOW_MS: i32 = 80;

// We use an Instant-based tick counter to avoid needing GetTickCount.
// Store the Instant in a static, compare elapsed ms.
static PEN_EVENT_INSTANT: std::sync::Mutex<Option<Instant>> = Mutex::new(None);

fn mark_pen_event() {
    let mut lock = PEN_EVENT_INSTANT.lock().unwrap();
    *lock = Some(Instant::now());
}

fn is_pen_event() -> bool {
    let lock = PEN_EVENT_INSTANT.lock().unwrap();
    if let Some(t) = *lock {
        t.elapsed().as_millis() < SUPPRESS_WINDOW_MS as u128
    } else {
        HID_TIP_DOWN.load(Ordering::SeqCst)
    }
}

// ── WH_MOUSE_LL Hook callback ──

unsafe extern "system" fn hook_proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if n_code < 0 {
        return CallNextHookEx(None, n_code, w_param, l_param);
    }

    if !is_pen_event() {
        return CallNextHookEx(None, n_code, w_param, l_param);
    }

    let msg = w_param.0 as u32;
    let _hook_struct = &*(l_param.0 as *const MSLLHOOKSTRUCT);

    match msg {
        WM_MOUSEMOVE => LRESULT(1),
        WM_LBUTTONDOWN | WM_LBUTTONUP => LRESULT(1),
        WM_RBUTTONDOWN | WM_RBUTTONUP | WM_MBUTTONDOWN | WM_MBUTTONUP => LRESULT(1),
        _ => CallNextHookEx(None, n_code, w_param, l_param),
    }
}

// ── Run entry point ──

pub fn run() {
    let (sw, sh);
    unsafe {
        sw = GetSystemMetrics(SM_CXSCREEN);
        sh = GetSystemMetrics(SM_CYSCREEN);
    }

    // Init DB
    crate::glaspen2_init_db(sw, sh);

    let hmodule = unsafe { GetModuleHandleW(None).unwrap() };
    let instance = HINSTANCE(hmodule.0);

    // ── Register window class ──
    let ol_class = wide_string("Glaspen2HookOL");
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(overlay_wnd_proc),
        hInstance: instance,
        lpszClassName: PCWSTR(ol_class.as_ptr()),
        ..Default::default()
    };
    unsafe { RegisterClassExW(&wc) };

    // ── Create overlay window ──
    let ol_ex = WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
    let overlay_hwnd = unsafe {
        CreateWindowExW(
            ol_ex,
            PCWSTR(ol_class.as_ptr()),
            PCWSTR(wide_string("glaspen2_hol").as_ptr()),
            WS_POPUP,
            0, 0, sw, sh,
            None, None, instance, None,
        ).unwrap()
    };

    unsafe {
        let margins = MARGINS { cxLeftWidth: -1, cxRightWidth: -1, cyTopHeight: -1, cyBottomHeight: -1 };
        let _ = DwmExtendFrameIntoClientArea(overlay_hwnd, &margins);
    }

    { let mut h = OVERLAY_HWND.lock().unwrap(); *h = overlay_hwnd.0 as isize; }

    // ── Create drawing surface + DIB ──
    let surface = ImageSurface::create(Format::ARGB32, sw, sh).unwrap();
    let (hdc_mem, hbitmap, dib_bits, dib_stride) = create_dib(sw, sh);

    // Load settings
    let mut pen_r = 1.0; let mut pen_g = 0.0; let mut pen_b = 0.0; let mut width_scale = 0.3;
    crate::glaspen2_load_settings_parts(&mut pen_r, &mut pen_g, &mut pen_b, &mut width_scale);

    let outline_enabled = crate::db::load_setting("outline_enabled")
        .and_then(|v| v.parse::<i32>().ok()).unwrap_or(0) != 0;
    let inverse_enabled = crate::db::load_setting("inverse_enabled")
        .and_then(|v| v.parse::<i32>().ok()).unwrap_or(0) != 0;

    let state = DrawState {
        pen_r, pen_g, pen_b, width_scale,
        selected_color: 0, selected_width: 0,
        enabled: true, show_rainbow: false,
        outline_enabled, inverse_enabled, lang: 0,
    };

    let data = Box::new(OverlayData {
        state: Mutex::new(state),
        surface,
        hdc_mem, hbitmap, dib_bits, dib_stride,
        screen_w: sw, screen_h: sh, overlay_hwnd,
    });
    let ptr = Box::into_raw(data);
    unsafe { SHARED_DATA_PTR = ptr; }

    // ── Register Raw Input devices ──
    register_raw_input(overlay_hwnd);

    // ── Install WH_MOUSE_LL hook ──
    let hook = unsafe {
        SetWindowsHookExW(
            WH_MOUSE_LL,
            Some(hook_proc),
            HINSTANCE(std::ptr::null_mut()),
            0,
        )
    };
    match &hook {
        Ok(_) => eprintln!("[hook] WH_MOUSE_LL installed OK"),
        Err(e) => eprintln!("[hook] SetWindowsHookExW FAILED: {:?}", e),
    }

    // ── Register hotkeys ──
    unsafe {
        let mods = HOT_KEY_MODIFIERS(MOD_CONTROL.0 | MOD_ALT.0);
        RegisterHotKey(overlay_hwnd, 1, mods, 'C' as u32).ok();
        RegisterHotKey(overlay_hwnd, 2, mods, 'V' as u32).ok();
    }

    // ── Show window ──
    unsafe { ShowWindow(overlay_hwnd, SW_SHOW); }
    update_overlay();

    // ── Spawn tray thread ──
    {
        let tray_hwnd = overlay_hwnd.0 as isize;
        std::thread::spawn(move || {
            super::tray::run(tray_hwnd);
        });
    }

    // ── Main message loop ──
    let mut msg = MSG::default();
    loop {
        let result = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if result.0 == 0 { break; }
        unsafe { TranslateMessage(&msg); DispatchMessageW(&msg); }
    }

    // ── Cleanup ──
    if let Ok(h) = hook {
        unsafe { UnhookWindowsHookEx(h).ok(); }
    }
    unsafe {
        DeleteObject(hbitmap).ok();
        DeleteDC(hdc_mem).ok();
        if !ptr.is_null() { drop(Box::from_raw(ptr)); }
    }
}

// ── Raw Input registration ──

fn register_raw_input(hwnd: HWND) {
    let devices = [
        RawInputDevice {
            us_usage_page: 0x0001,
            us_usage: 0x0002,
            dw_flags: RIDEV_INPUTSINK,
            hwnd_target: hwnd.0 as isize,
        },
        RawInputDevice {
            us_usage_page: 0x000D,
            us_usage: 0x0002,
            dw_flags: RIDEV_INPUTSINK,
            hwnd_target: hwnd.0 as isize,
        },
    ];

    let cb = std::mem::size_of::<RawInputDevice>() as u32;
    let result = unsafe {
        RegisterRawInputDevices(devices.as_ptr(), devices.len() as u32, cb)
    };
    if result.as_bool() {
        eprintln!("[raw] Registered {} raw input devices OK", devices.len());
    } else {
        eprintln!("[raw] RegisterRawInputDevices FAILED. err={}", std::io::Error::last_os_error());
    }
}

// ── Window proc ──

unsafe extern "system" fn overlay_wnd_proc(
    hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM
) -> LRESULT {
    match msg {
        WM_INPUT => {
            process_raw_input(lparam);
            LRESULT(0)
        }
        WM_HOTKEY => {
            let data = match get_overlay_data() { Some(d) => d, None => return LRESULT(0) };
            let mut state = data.state.lock().unwrap();
            match wparam.0 as i32 {
                1 => clear_screen_internal(&mut state, data),
                2 => toggle_enabled_internal(&mut state),
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

// ── Process Raw Input ──

fn process_raw_input(lparam: LPARAM) {
    unsafe {
        let mut size: u32 = 0;
        let header_size = std::mem::size_of::<RawInputHeader>() as u32;
        let _ = GetRawInputData(lparam.0, RID_INPUT, ptr::null_mut(), &mut size, header_size);
        if size == 0 { return; }

        let mut buf: Vec<u8> = vec![0u8; size as usize];
        let written = GetRawInputData(lparam.0, RID_INPUT, buf.as_mut_ptr(), &mut size, header_size);
        if written != size { return; }

        let header = &*(buf.as_ptr() as *const RawInputHeader);
        let data_offset = std::mem::size_of::<RawInputHeader>();

        if header.dw_type == RIM_TYPEHID {
            let hid_data = &buf[data_offset..];
            process_hid_input(hid_data);
        } else if header.dw_type == RIM_TYPEMOUSE {
            let mouse_data = &buf[data_offset..];
            if mouse_data.len() >= std::mem::size_of::<RawMouse>() {
                process_mouse_input(&*(mouse_data.as_ptr() as *const RawMouse));
            }
        }
    }
}

fn process_hid_input(data: &[u8]) {
    // dwSizeHid(4) + dwCount(4) + HID report
    if data.len() < 16 { return; }

    let hid_size = u32::from_ne_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let hid_data = &data[8..]; // skip dwSizeHid + dwCount
    if hid_data.len() < hid_size || hid_size < 8 { return; }

    let switches = hid_data[1];
    let tip_down = (switches & 0x01) != 0;

    let x_raw = hid_data[2] as u32 | ((hid_data[3] as u32) << 8);
    let y_raw = hid_data[4] as u32 | ((hid_data[5] as u32) << 8);
    let pressure = hid_data[6] as u32 | ((hid_data[7] as u32) << 8);

    HID_TIP_DOWN.store(tip_down, Ordering::SeqCst);
    mark_pen_event();

    if x_raw > 0 && y_raw > 0 && x_raw < 100000 && y_raw < 100000 {
        let data_ref = match get_overlay_data() { Some(d) => d, None => return };
        let sx = data_ref.screen_w as f64 * x_raw as f64 / 65536.0;
        let sy = data_ref.screen_h as f64 * y_raw as f64 / 65536.0;

        unsafe { LAST_DRAW_X = sx; LAST_DRAW_Y = sy; }

        if tip_down && pressure > 0 {
            if !PEN_ACTIVE.load(Ordering::SeqCst) {
                PEN_ACTIVE.store(true, Ordering::SeqCst);
                unsafe { HAS_LAST_DRAW = false; }
                let (ws, r, g, b);
                {
                    let state = data_ref.state.lock().unwrap();
                    ws = state.width_scale;
                    r = state.pen_r; g = state.pen_g; b = state.pen_b;
                }
                let w = pressure_to_width(pressure.max(1) as f64 / 1024.0, ws);
                draw_point_on_surface(&data_ref.surface, sx, sy, w, r, g, b, false);
                update_overlay_rect(
                    (sx - w) as i32, (sy - w) as i32,
                    (sx + w) as i32, (sy + w) as i32,
                );
            } else {
                let (ws, r, g, b);
                {
                    let state = data_ref.state.lock().unwrap();
                    ws = state.width_scale;
                    r = state.pen_r; g = state.pen_g; b = state.pen_b;
                }
                let ps = pressure.max(1) as f64 / 1024.0;
                let w = pressure_to_width(ps, ws);
                let (lx, ly, hl) = unsafe { (LAST_DRAW_X, LAST_DRAW_Y, HAS_LAST_DRAW) };
                draw_point_on_surface(&data_ref.surface, sx, sy, w, r, g, b, hl);
                let ri = (w * 0.5 + 2.0) as i32;
                update_overlay_rect(
                    (lx.min(sx) - ri as f64) as i32, (ly.min(sy) - ri as f64) as i32,
                    (lx.max(sx) + ri as f64) as i32, (ly.max(sy) + ri as f64) as i32,
                );
                unsafe { LAST_DRAW_X = sx; LAST_DRAW_Y = sy; HAS_LAST_DRAW = true; }
            }
        } else if !tip_down && PEN_ACTIVE.load(Ordering::SeqCst) {
            PEN_ACTIVE.store(false, Ordering::SeqCst);
            unsafe { HAS_LAST_DRAW = false; }
        }
    }
}

fn process_mouse_input(mouse: &RawMouse) {
    let is_absolute = (mouse.us_flags & MOUSE_MOVE_ABSOLUTE) != 0;
    if !is_absolute { return; }

    mark_pen_event();

    let data_ref = match get_overlay_data() { Some(d) => d, None => return };
    let sw = data_ref.screen_w as f64;
    let sh = data_ref.screen_h as f64;

    let sx = (mouse.l_last_x as f64 / 65535.0 * sw).round();
    let sy = (mouse.l_last_y as f64 / 65535.0 * sh).round();

    if sx <= 2.0 && sy <= 2.0 { return; }

    unsafe { LAST_DRAW_X = sx; LAST_DRAW_Y = sy; }

    let left_down = (mouse.us_button_flags & RI_MOUSE_LEFT_BUTTON_DOWN) != 0;
    let left_up = (mouse.us_button_flags & RI_MOUSE_LEFT_BUTTON_UP) != 0;
    let drawing = PEN_ACTIVE.load(Ordering::SeqCst);

    if left_down && !drawing {
        PEN_ACTIVE.store(true, Ordering::SeqCst);
        unsafe { HAS_LAST_DRAW = false; }
        let (ws, r, g, b);
        {
            let state = data_ref.state.lock().unwrap();
            ws = state.width_scale;
            r = state.pen_r; g = state.pen_g; b = state.pen_b;
        }
        let w = pressure_to_width(0.0, ws);
        draw_point_on_surface(&data_ref.surface, sx, sy, w, r, g, b, false);
        update_overlay_rect((sx - w) as i32, (sy - w) as i32, (sx + w) as i32, (sy + w) as i32);
    } else if left_up && drawing {
        PEN_ACTIVE.store(false, Ordering::SeqCst);
        unsafe { HAS_LAST_DRAW = false; }
    } else if drawing {
        let (ws, r, g, b);
        {
            let state = data_ref.state.lock().unwrap();
            ws = state.width_scale;
            r = state.pen_r; g = state.pen_g; b = state.pen_b;
        }
        let w = pressure_to_width(0.0, ws);
        let (lx, ly, hl) = unsafe { (LAST_DRAW_X, LAST_DRAW_Y, HAS_LAST_DRAW) };
        draw_point_on_surface(&data_ref.surface, sx, sy, w, r, g, b, hl);
        let ri = (w * 0.5 + 2.0) as i32;
        update_overlay_rect(
            (lx.min(sx) - ri as f64) as i32, (ly.min(sy) - ri as f64) as i32,
            (lx.max(sx) + ri as f64) as i32, (ly.max(sy) + ri as f64) as i32,
        );
        unsafe { LAST_DRAW_X = sx; LAST_DRAW_Y = sy; HAS_LAST_DRAW = true; }
    }
}

// ── Drawing ──

fn draw_point_on_surface(
    surface: &ImageSurface, x: f64, y: f64, w: f64,
    r: f64, g: f64, b: f64, has_last: bool,
) {
    let sw = surface.width() as usize;
    let sh = surface.height() as usize;
    let stride = surface.stride() as usize;
    let pixels = surface.pixels_mut();
    let cr = (r * 255.0) as u32;
    let cg = (g * 255.0) as u32;
    let cb = (b * 255.0) as u32;
    let radius = w * 0.5;

    unsafe {
        if has_last && HAS_LAST_DRAW {
            draw_line_aa(pixels, sw, sh, stride, LAST_DRAW_X, LAST_DRAW_Y, x, y, radius, cr, cg, cb, 255);
        } else {
            draw_circle_aa(pixels, sw, sh, stride, x, y, radius, cr, cg, cb, 255);
        }
        LAST_DRAW_X = x; LAST_DRAW_Y = y; HAS_LAST_DRAW = true;
    }
}

fn draw_circle_aa(
    pixels: &mut [u8], sw: usize, sh: usize, stride: usize,
    cx: f64, cy: f64, radius: f64, r: u32, g: u32, b: u32, a: u32,
) {
    if radius < 0.3 { return; }
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
                set_pixel_aa(pixels, stride, sw, sh, px, py, r, g, b, a);
            } else if dsq < r_sq {
                let alpha = (a as f64 * (0.5 + radius - dsq.sqrt())).min(255.0) as u32;
                if alpha > 0 { set_pixel_aa(pixels, stride, sw, sh, px, py, r, g, b, alpha); }
            }
        }
    }
}

fn draw_line_aa(
    pixels: &mut [u8], sw: usize, sh: usize, stride: usize,
    x0: f64, y0: f64, x1: f64, y1: f64, radius: f64,
    r: u32, g: u32, b: u32, a: u32,
) {
    let d = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt();
    let step = (radius * 0.5).max(0.5);
    let n = (d / step).ceil() as i32;
    for i in 0..=n {
        let t = if n == 0 { 0.0 } else { i as f64 / n as f64 };
        draw_circle_aa(pixels, sw, sh, stride,
            x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, radius, r, g, b, a);
    }
}

#[inline]
fn set_pixel_aa(pixels: &mut [u8], stride: usize, sw: usize, sh: usize,
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

// ── UpdateLayeredWindow ──

pub fn update_overlay() {
    update_overlay_rect(-1, -1, -1, -1);
}

fn update_overlay_rect(x0: i32, y0: i32, x1: i32, y1: i32) {
    let data = match get_overlay_data() { Some(d) => d, None => return };
    let surf_data = data.surface.data().unwrap();
    let surf_stride = data.surface.stride() as usize;
    let dib = unsafe {
        std::slice::from_raw_parts_mut(data.dib_bits, data.dib_stride * data.screen_h as usize)
    };

    let (left, top, right, bottom) = if x0 < 0 {
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

    let blend_fn = BLENDFUNCTION {
        BlendOp: 0, BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: 1, // AC_SRC_ALPHA
    };
    let size = SIZE { cx: right - left, cy: bottom - top };
    let pt_dst = POINT { x: left, y: top };
    let pt_src = POINT { x: left, y: top };

    unsafe {
        let _ = UpdateLayeredWindow(
            data.overlay_hwnd, None, Some(&pt_dst), Some(&size),
            data.hdc_mem, Some(&pt_src), COLORREF(0), Some(&blend_fn), ULW_ALPHA,
        );
    }
}

fn create_dib(w: i32, h: i32) -> (HDC, HBITMAP, *mut u8, usize) {
    unsafe {
        let hdc_s = GetDC(None);
        let hdc_m = CreateCompatibleDC(hdc_s);
        ReleaseDC(None, hdc_s);
        let stride = (w as usize * 4 + 3) & !3;
        let bmi = BITMAPINFO {
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

fn pressure_to_width(p: f64, scale: f64) -> f64 {
    if p > 0.01 { (0.3 + p * p * 7.7) * scale } else { 1.0 * scale }
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
            x if x == CMD_CLEAR_SCREEN => clear_screen_internal(state, data),
            x if x == CMD_TOGGLE_RAINBOW => {
                state.show_rainbow = !state.show_rainbow;
                super::tray::update_rainbow_checkmark(state.show_rainbow);
                if state.show_rainbow { render::draw_rainbow_indicator(&data.surface); update_overlay(); }
                else { clear_screen_internal(state, data); }
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
            x if x == CMD_TOGGLE_ENABLED => toggle_enabled_internal(state),
            x if x == CMD_QUIT => { unsafe { DestroyWindow(data.overlay_hwnd).ok(); } }
            _ => {}
        }
    }
}

fn clear_screen_internal(state: &mut DrawState, data: &OverlayData) {
    render::clear_screen(&data.surface);
    crate::glaspen2_clear_strokes(data.screen_w, data.screen_h);
    if state.show_rainbow { render::draw_rainbow_indicator(&data.surface); }
    update_overlay();
}

fn toggle_enabled_internal(state: &mut DrawState) {
    state.enabled = !state.enabled;
    super::tray::update_tray_icon(state);
    super::tray::update_enabled_item(state.enabled, state.lang);
}

fn save_drawing(data: &OverlayData) {
    let w = data.surface.width(); let h = data.surface.height();
    let stride = data.surface.stride();
    let sdata = data.surface.data().unwrap();
    crate::glaspen2_save_drawing(sdata.as_ptr(), w, h, stride);
}

fn save_with_bg(data: &OverlayData) {
    let screen_dc = unsafe { GetDC(None) };
    let bw = data.screen_w; let bh = data.screen_h;
    let bg_dc = unsafe { CreateCompatibleDC(screen_dc) };
    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: bw, biHeight: -bh, biPlanes: 1, biBitCount: 32,
            biCompression: BI_RGB.0, ..Default::default()
        }, ..Default::default()
    };
    let mut bg_bits: *mut std::ffi::c_void = ptr::null_mut();
    let bg_bmp = unsafe { CreateDIBSection(bg_dc, &bmi, DIB_RGB_COLORS, &mut bg_bits, None, 0).unwrap() };
    unsafe {
        SelectObject(bg_dc, HGDIOBJ(bg_bmp.0 as _));
        BitBlt(bg_dc, 0, 0, bw, bh, screen_dc, 0, 0, SRCCOPY).unwrap();
    }
    let draw_w = data.surface.width(); let draw_h = data.surface.height();
    let draw_stride = data.surface.stride() as i32;
    let draw_data = data.surface.data().unwrap();
    unsafe {
        crate::glaspen2_save_with_background(
            draw_data.as_ptr(), draw_w, draw_h, draw_stride,
            bg_bits as *const u8, bw, bh, bw * 4,
        );
        SelectObject(bg_dc, HGDIOBJ(bg_bmp.0 as _));
        let _ = DeleteObject(bg_bmp); let _ = DeleteDC(bg_dc);
        let _ = ReleaseDC(None, screen_dc);
    }
}
