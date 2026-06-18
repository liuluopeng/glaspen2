//! GTK4 overlay for pen drawing on Windows.
//! Uses GDK-Win32 EventControllerLegacy for pen events.
//! Architecture: single GTK4 window on top of Win32 LAYERED+TRANSPARENT overlay.
//!
//! Pen click suppression: dynamically switch the GTK window's input region.
//! - Pen down → input region = full screen (captures ALL input, pen clicks don't leak)
//! - Pen up   → input region = empty     (mouse events pass through to apps below)
//! This matches macOS CGEventTap behavior without needing WH_MOUSE_LL hooks.

use std::cell::RefCell;
use std::ptr;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{gdk, glib};
use gtk4::cairo::{self, ImageSurface, Format};

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::DwmExtendFrameIntoClientArea;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::WindowsAndMessaging::*;

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

// ── Input region helpers ──

/// Set the GDK surface input region to cover the full screen.
/// All input (pen + mouse) is captured by the GTK window.
fn set_input_region_full(surf: &gdk::Surface, w: i32, h: i32) {
    let rect = cairo::RectangleInt::new(0, 0, w, h);
    let region = cairo::Region::create_rectangle(&rect);
    surf.set_input_region(&region);
}

/// Set the GDK surface input region to empty.
/// All mouse input passes through to windows below.
fn set_input_region_empty(surf: &gdk::Surface) {
    let region = cairo::Region::create();
    surf.set_input_region(&region);
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

        app.run();

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

    // Make the GTK window TOPMOST + no taskbar icon + no focus stealing
    // (like macOS: no Dock icon, setIgnoresMouseEvents, never takes focus)
    make_gtk_topmost(&window);

    // Initially set empty input region: mouse events pass through to apps below.
    // When the pen touches down, we switch to full input region to capture
    // everything (preventing pen-generated clicks from leaking to other apps).
    if let Some(surf) = window.surface() {
        set_input_region_empty(&surf);
        eprintln!("[gtk] pen window ready {}x{} topmost=true input_region=empty", w, h);
    }
}

fn make_gtk_topmost(window: &gtk4::Window) {
    // Find GTK window HWND by title
    let title: Vec<u16> = "glaspen2-gtk"
        .encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        if let Ok(hwnd) = FindWindowW(None, windows::core::PCWSTR(title.as_ptr())) {
            // Remove from taskbar + prevent activation — like macOS's
            // NSApplicationActivationPolicyAccessory + setIgnoresMouseEvents
            let mut ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
            ex_style |= WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0;
            ex_style &= !WS_EX_APPWINDOW.0;
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style as isize);

            SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED).ok();
            eprintln!("[gtk] set HWND {:?} to TOPMOST+TOOLWINDOW+NOACTIVATE", hwnd);

            // Return focus to whichever window had it before
            let fg = GetForegroundWindow();
            if !fg.is_invalid() && fg != hwnd {
                SetForegroundWindow(fg).ok();
            }
        }
    }
}

fn handle_pen_event(event: &gdk::Event, ctx: &OverlayCtx) {
    if event.device_tool().is_none() { return; }

    match event.event_type() {
        gdk::EventType::ButtonPress => {
            let (x, y, p) = extract_pen(event);
            eprintln!("[gtk] STYLUS DOWN x={:.0} y={:.0} p={:.3}", x, y, p);

            // Switch to full input region: capture ALL input while pen is down.
            // This prevents pen-generated mouse clicks from reaching other apps.
            // Mouse events are also captured but ignored (device_tool is None).
            if let Some(surf) = event.surface() {
                set_input_region_full(&surf, ctx.screen_w, ctx.screen_h);
            }

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

            // Switch back to empty input region: mouse events pass through again.
            if let Some(surf) = event.surface() {
                set_input_region_empty(&surf);
            }

            // Restore focus to the previously active window in case the
            // pen down event caused a focus switch.
            unsafe {
                let fg = GetForegroundWindow();
                let title: Vec<u16> = "glaspen2-gtk"
                    .encode_utf16().chain(std::iter::once(0)).collect();
                if let Ok(gtk_hwnd) = FindWindowW(None, windows::core::PCWSTR(title.as_ptr())) {
                    if !fg.is_invalid() && fg != gtk_hwnd {
                        SetForegroundWindow(fg).ok();
                    }
                }
            }
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
