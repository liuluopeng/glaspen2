//! GTK4 overlay for pen drawing on Windows.
//! Uses GDK-Win32 EventControllerLegacy for pen events (no magic numbers).
//! Architecture: single GTK4 window on top of Win32 LAYERED+TRANSPARENT overlay.

use std::cell::RefCell;
use std::ptr;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use gtk4::prelude::*;
use gtk4::{gdk, glib};
use gtk4::cairo::{self, ImageSurface, Format};

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::DwmExtendFrameIntoClientArea;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::Console::SetConsoleCtrlHandler;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::WindowsAndMessaging::*;

// ── Pen suppression state (driven by GTK pen events, read by WH_MOUSE_LL hook) ──
static PEN_ACTIVE: AtomicBool = AtomicBool::new(false);
static PEN_UP_TIME: AtomicU64 = AtomicU64::new(0);
static mut HHOOK_HANDLE: HHOOK = HHOOK(ptr::null_mut());
static mut BLANK_CURSOR: HCURSOR = HCURSOR(ptr::null_mut());

struct StrokeState {
    pub pen_r: f64, pub pen_g: f64, pub pen_b: f64, pub width_scale: f64,
    pub enabled: bool, pub inverse_enabled: bool,
    pub active: bool, pub last_x: f64, pub last_y: f64, pub has_last: bool,
    pub stroke_color: (f64, f64, f64),
    pub last_pressure: f64,
}

struct OverlayCtx {
    pub stroke: RefCell<StrokeState>,
    pub gtk_surface: RefCell<ImageSurface>,
    pub overlay_hwnd: HWND,
    pub hdc_mem: HDC,
    pub hbitmap: HBITMAP,
    pub dib_bits: *mut u8,
    pub dib_stride: usize,
    pub screen_w: i32,
    pub screen_h: i32,
}

// ── WH_MOUSE_LL hook: suppress pen-generated mouse events ──

/// Grace period (ms) after pen-up: residual mouse messages from the pen are still suppressed.
const PEN_UP_GRACE_MS: u64 = 50;

unsafe extern "system" fn low_level_mouse_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 {
        let pen_active = PEN_ACTIVE.load(Ordering::SeqCst);
        let suppress = if pen_active {
            true
        } else {
            let up_time = PEN_UP_TIME.load(Ordering::SeqCst);
            up_time > 0 && GetTickCount64().saturating_sub(up_time) < PEN_UP_GRACE_MS
        };
        if suppress {
            match w_param.0 as u32 {
                WM_LBUTTONDOWN | WM_LBUTTONUP | WM_MOUSEMOVE
                | WM_RBUTTONDOWN | WM_RBUTTONUP
                | WM_MBUTTONDOWN | WM_MBUTTONUP => return LRESULT(1),
                _ => {}
            }
        }
    }
    CallNextHookEx(None, n_code, w_param, l_param)
}

// ── System cursor hide / restore ──

fn hide_system_cursor() {
    unsafe {
        let blank = BLANK_CURSOR;
        if blank.is_invalid() { return; }
        // Replace common cursor shapes so the cursor is invisible while drawing
        SetSystemCursor(blank, OCR_NORMAL).ok();
        SetSystemCursor(blank, OCR_IBEAM).ok();
        SetSystemCursor(blank, OCR_CROSS).ok();
        SetSystemCursor(blank, OCR_UP).ok();
        SetSystemCursor(blank, OCR_SIZEALL).ok();
        SetSystemCursor(blank, OCR_SIZENWSE).ok();
        SetSystemCursor(blank, OCR_SIZENESW).ok();
        SetSystemCursor(blank, OCR_SIZEWE).ok();
        SetSystemCursor(blank, OCR_SIZENS).ok();
    }
}

fn restore_system_cursor() {
    unsafe {
        // SPI_SETCURSORS restores all system cursors to their defaults
        SystemParametersInfoW(SPI_SETCURSORS, 0, None, SPIF_SENDCHANGE).ok();
    }
}

// ── Console Ctrl handler for crash safety ──

unsafe extern "system" fn console_ctrl_handler(_ctrl_type: u32) -> BOOL {
    restore_system_cursor();
    // Don't handle the signal — let the default handler proceed
    FALSE
}

pub fn run_gtk() {
    unsafe {
        let (sw, sh) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));

        // ── Create Win32 overlay (LAYERED+TRANSPARENT for display & mouse passthrough) ──
        let hmodule = GetModuleHandleW(None).unwrap();
        let oclass = wide("Glaspen2OL");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(overlay_wnd_proc),
            hInstance: HINSTANCE(hmodule.0),
            lpszClassName: windows::core::PCWSTR(oclass.as_ptr()),
            ..Default::default()
        };
        RegisterClassExW(&wc);

        let ol_hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            windows::core::PCWSTR(oclass.as_ptr()),
            windows::core::PCWSTR(wide("glaspen2_ol").as_ptr()),
            WS_POPUP, 0, 0, sw, sh,
            None, None, HINSTANCE(hmodule.0), None,
        ).expect("overlay window");

        let margins = MARGINS { cxLeftWidth: -1, cxRightWidth: -1, cyTopHeight: -1, cyBottomHeight: -1 };
        DwmExtendFrameIntoClientArea(ol_hwnd, &margins).ok();
        ShowWindow(ol_hwnd, SW_SHOW);

        let (hdc_mem, hbitmap, dib_bits, dib_stride) = create_dib(sw, sh);
        let gtk_surface = RefCell::new(ImageSurface::create(Format::ARgb32, sw, sh).unwrap());

        let ctx = Rc::new(OverlayCtx {
            stroke: RefCell::new(StrokeState {
                pen_r: 1.0, pen_g: 0.0, pen_b: 0.0, width_scale: 1.0,
                enabled: true, inverse_enabled: false,
                active: false, last_x: 0.0, last_y: 0.0, has_last: false,
                stroke_color: (1.0, 0.0, 0.0), last_pressure: 0.5,
            }),
            gtk_surface, overlay_hwnd: ol_hwnd, hdc_mem, hbitmap,
            dib_bits, dib_stride, screen_w: sw, screen_h: sh,
        });

        // ── GTK4 application (pen events) ──
        let app = gtk4::Application::new(
            Some("com.glaspen2.pen"), gtk4::gio::ApplicationFlags::default(),
        );
        let c = ctx.clone();
        app.connect_activate(move |app| build_pen_window(app, c.clone()));

        // ── Install WH_MOUSE_LL hook to suppress pen-generated mouse events ──
        HHOOK_HANDLE = SetWindowsHookExW(
            WH_MOUSE_LL,
            Some(low_level_mouse_proc),
            HINSTANCE(hmodule.0),
            0,
        ).expect("WH_MOUSE_LL hook installation failed");
        eprintln!("[gtk] WH_MOUSE_LL hook installed");

        // ── Create blank cursor for system-wide hiding during pen drawing ──
        let and_plane: [u8; 4] = [0xFF; 4]; // AND mask: all 1s = transparent
        let xor_plane: [u8; 4] = [0x00; 4]; // XOR mask: all 0s = no inversion
        BLANK_CURSOR = CreateCursor(
            HINSTANCE(hmodule.0),
            0, 0, 1, 1,
            and_plane.as_ptr() as *const std::ffi::c_void,
            xor_plane.as_ptr() as *const std::ffi::c_void,
        ).expect("CreateCursor failed");
        eprintln!("[gtk] blank cursor created");

        // ── Crash safety: restore cursor on panic or Ctrl+C ──
        let default_panic = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore_system_cursor();
            (default_panic)(info);
        }));
        SetConsoleCtrlHandler(Some(console_ctrl_handler), true).ok();

        app.run();

        // ── Cleanup ──
        UnhookWindowsHookEx(HHOOK_HANDLE).ok();
        restore_system_cursor();
        if !BLANK_CURSOR.is_invalid() {
            DestroyCursor(BLANK_CURSOR).ok();
        }
        DeleteObject(hbitmap).ok();
        DeleteDC(hdc_mem).ok();
        DestroyWindow(ol_hwnd).ok();
    }
}

fn build_pen_window(app: &gtk4::Application, ctx: Rc<OverlayCtx>) {
    let w = ctx.screen_w;
    let h = ctx.screen_h;

    let window = gtk4::Window::new();
    window.set_application(Some(app));
    window.set_default_size(w, h);
    window.set_decorated(false);
    window.set_resizable(false);
    window.set_title(Some("glaspen2-gtk"));

    let da = gtk4::DrawingArea::new();
    da.set_size_request(w, h);
    window.set_child(Some(&da));

    // ── Pen event controller ──
    let ctrl = gtk4::EventControllerLegacy::new();
    ctrl.set_propagation_phase(gtk4::PropagationPhase::Capture);
    let c = ctx.clone();
    ctrl.connect_event(move |_ctrl, event| {
        handle_pen_event(event, &c);
        glib::Propagation::Proceed
    });
    window.add_controller(ctrl);

    // CSS: nearly invisible background
    let css = gtk4::CssProvider::new();
    css.load_from_string("window { background-color: rgba(0,0,0,0.01); }");
    if let Some(dpy) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &dpy, &css, gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    window.present();

    // Make the GTK window TOPMOST so it stays above all apps
    make_gtk_topmost(&window);

    // Try to make mouse events pass through GDK's input region
    if let Some(surf) = window.surface() {
        let region = cairo::Region::create();
        surf.set_input_region(&region);
        eprintln!("[gtk] pen window ready {}x{} topmost=true input_region=empty", w, h);
    }
}

fn make_gtk_topmost(window: &gtk4::Window) {
    // Find GTK window HWND by title
    let title: Vec<u16> = "glaspen2-gtk"
        .encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        if let Ok(hwnd) = FindWindowW(None, windows::core::PCWSTR(title.as_ptr())) {
            SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE).ok();
            eprintln!("[gtk] set HWND {:?} to TOPMOST", hwnd);
        }
    }
}

fn handle_pen_event(event: &gdk::Event, ctx: &OverlayCtx) {
    if event.device_tool().is_none() { return; }

    match event.event_type() {
        gdk::EventType::ButtonPress => {
            let (x, y, p) = extract_pen(event);
            eprintln!("[gtk] STYLUS DOWN x={:.0} y={:.0} p={:.3}", x, y, p);

            // Activate pen suppression: swallow mouse events & hide cursor
            PEN_ACTIVE.store(true, Ordering::SeqCst);
            hide_system_cursor();

            let mut st = ctx.stroke.borrow_mut();
            st.active = true; st.has_last = false;
            st.stroke_color = (st.pen_r, st.pen_g, st.pen_b);
            st.last_pressure = if p < 0.01 { 0.5 } else { p };
            let c = st.stroke_color;
            let w = pressure_to_width(st.last_pressure, st.width_scale);
            drop(st);
            pen_draw(&ctx.gtk_surface.borrow(), x, y, w, c.0, c.1, c.2, 0.0, 0.0, false);
            copy_to_overlay(ctx);
            update_layered(ctx);
        }
        gdk::EventType::MotionNotify => {
            let mut st = ctx.stroke.borrow_mut();
            if !st.active { return; }
            let (x, y, p) = extract_pen(event);
            st.last_pressure = if p > 0.01 { p } else { st.last_pressure };
            let c = st.stroke_color;
            let w = pressure_to_width(st.last_pressure, st.width_scale);
            let (lx, ly, hl) = (st.last_x, st.last_y, st.has_last);
            st.last_x = x; st.last_y = y; st.has_last = true;
            drop(st);
            pen_draw(&ctx.gtk_surface.borrow(), x, y, w, c.0, c.1, c.2, lx, ly, hl);
            copy_to_overlay(ctx);
            update_layered(ctx);
        }
        gdk::EventType::ButtonRelease => {
            ctx.stroke.borrow_mut().active = false;

            // Deactivate pen suppression: allow mouse events & restore cursor
            PEN_ACTIVE.store(false, Ordering::SeqCst);
            PEN_UP_TIME.store(unsafe { GetTickCount64() }, Ordering::SeqCst);
            restore_system_cursor();
        }
        _ => {}
    }
}

fn extract_pen(event: &gdk::Event) -> (f64, f64, f64) {
    let (x, y) = event.position().unwrap_or((0.0, 0.0));
    let p = event.axis(gdk::AxisUse::Pressure).unwrap_or(0.0);
    (x, y, p)
}

fn pressure_to_width(p: f64, scale: f64) -> f64 {
    if p > 0.01 { (0.3 + p * p * 7.7) * scale } else { 1.0 * scale }
}

// ── Real Cairo drawing ──

fn pen_draw(surf: &ImageSurface, x: f64, y: f64, w: f64,
            r: f64, g: f64, b: f64, px: f64, py: f64, has_prev: bool) {
    let cr = cairo::Context::new(surf).unwrap();
    cr.set_source_rgba(r, g, b, 1.0);
    cr.set_line_width(w);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.set_line_join(cairo::LineJoin::Round);
    if has_prev {
        cr.move_to(px, py);
        cr.line_to(x, y);
        cr.stroke().unwrap();
    } else {
        cr.arc(x, y, w * 0.5, 0.0, 2.0 * std::f64::consts::PI);
        cr.fill().unwrap();
    }
}

// ── Copy Cairo → DIB → UpdateLayeredWindow ──

fn copy_to_overlay(ctx: &OverlayCtx) {
    let sstride = ctx.gtk_surface.borrow().stride() as usize;
    let mut s = ctx.gtk_surface.borrow_mut();
    let sdata = s.data().unwrap();
    let dib = unsafe { std::slice::from_raw_parts_mut(ctx.dib_bits, ctx.dib_stride * ctx.screen_h as usize) };
    for y in 0..ctx.screen_h as usize {
        for x in 0..ctx.screen_w as usize {
            let src = y * sstride + x * 4;
            let dst = y * ctx.dib_stride + x * 4;
            if src + 3 < sdata.len() && dst + 3 < dib.len() {
                dib[dst] = sdata[src]; dib[dst+1] = sdata[src+1];
                dib[dst+2] = sdata[src+2]; dib[dst+3] = sdata[src+3];
            }
        }
    }
}

fn update_layered(ctx: &OverlayCtx) {
    let bf = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
    let sz = SIZE { cx: ctx.screen_w, cy: ctx.screen_h };
    let pt = POINT { x: 0, y: 0 };
    unsafe {
        UpdateLayeredWindow(ctx.overlay_hwnd, None, Some(&pt), Some(&sz),
            ctx.hdc_mem, Some(&pt), COLORREF(0), Some(&bf), ULW_ALPHA).ok();
    }
}

// ── Win32 overlay wnd_proc ──

unsafe extern "system" fn overlay_wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_DESTROY => { PostQuitMessage(0); LRESULT(0) }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

// ── Helpers ──

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

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

