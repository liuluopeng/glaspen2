const HUD_NOTIF_W: i32 = 460;
const HUD_NOTIF_H: i32 = 48;
const HUD_PM_W: i32 = 240;
const HUD_PM_H: i32 = 32;
const HUD_NOTIF_TIMER: usize = 10;
const HUD_NOTIF_MS: u32 = 1200;

/// 日期标签:今天 / 昨天 / YYYY-MM-DD(本地时区,chrono 计算)
fn date_label(unix_secs: u64) -> String {
    use chrono::{Local, TimeZone};
    let now = Local::now();
    let ts = Local
        .timestamp_opt(unix_secs as i64, 0)
        .single()
        .unwrap_or(now);
    if ts.date_naive() == now.date_naive() {
        "今天".to_string()
    } else if now
        .date_naive()
        .signed_duration_since(ts.date_naive())
        .num_days()
        == 1
    {
        "昨天".to_string()
    } else {
        ts.format("%Y-%m-%d").to_string()
    }
}

// ── GDI+ 文字渲染(灰度抗锯齿,消除 GDI 文本的颗粒感) ──

use libloading::{Library, Symbol};

const TEXT_RENDERING_HINT_ANTIALIAS: i32 = 4; // AntiAlias
const STRING_ALIGNMENT_CENTER: i32 = 1;
const STATUS_OK: i32 = 0;
const UNIT_PIXEL: i32 = 3;

#[repr(C)]
struct GdiplusStartupInput {
    gdiplus_version: u32,
    debug_event_callback: *mut std::ffi::c_void,
    suppress_background_thread: i32,
    suppress_external_codecs: i32,
}

#[repr(C)]
struct GdiplusStartupOutput {
    notification_hook: *mut std::ffi::c_void,
    notification_unhook: *mut std::ffi::c_void,
}

#[repr(C)]
struct RectF {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

struct Gdiplus {
    _lib: Library,
    token: usize,
    create_from_hdc: unsafe extern "C" fn(HDC, *mut *mut std::ffi::c_void) -> i32,
    delete_graphics: unsafe extern "C" fn(*mut std::ffi::c_void) -> i32,
    set_text_rendering_hint: unsafe extern "C" fn(*mut std::ffi::c_void, i32) -> i32,
    new_private_font_collection: unsafe extern "C" fn(*mut *mut std::ffi::c_void) -> i32,
    private_add_font_file: unsafe extern "C" fn(*mut std::ffi::c_void, *const u16) -> i32,
    create_font_family_from_name:
        unsafe extern "C" fn(*const u16, *mut std::ffi::c_void, *mut *mut std::ffi::c_void) -> i32,
    delete_font_family: unsafe extern "C" fn(*mut std::ffi::c_void) -> i32,
    create_font: unsafe extern "C" fn(
        *mut std::ffi::c_void,
        f32,
        i32,
        i32,
        *mut *mut std::ffi::c_void,
    ) -> i32,
    delete_font: unsafe extern "C" fn(*mut std::ffi::c_void) -> i32,
    create_solid_fill: unsafe extern "C" fn(u32, *mut *mut std::ffi::c_void) -> i32,
    delete_brush: unsafe extern "C" fn(*mut std::ffi::c_void) -> i32,
    create_string_format: unsafe extern "C" fn(i32, u16, *mut *mut std::ffi::c_void) -> i32,
    set_string_align: unsafe extern "C" fn(*mut std::ffi::c_void, i32) -> i32,
    set_line_align: unsafe extern "C" fn(*mut std::ffi::c_void, i32) -> i32,
    delete_string_format: unsafe extern "C" fn(*mut std::ffi::c_void) -> i32,
    measure_string: unsafe extern "C" fn(
        *mut std::ffi::c_void,
        *const u16,
        i32,
        *mut std::ffi::c_void,
        *const RectF,
        *mut std::ffi::c_void,
        *mut RectF,
        *mut i32,
        *mut i32,
    ) -> i32,
    draw_string: unsafe extern "C" fn(
        *mut std::ffi::c_void,
        *const u16,
        i32,
        *mut std::ffi::c_void,
        *const RectF,
        *mut std::ffi::c_void,
        *mut std::ffi::c_void,
    ) -> i32,
}

fn gdiplus() -> Option<&'static Gdiplus> {
    use std::sync::OnceLock;
    static GP: OnceLock<Option<Gdiplus>> = OnceLock::new();
    GP.get_or_init(|| {
        let lib = unsafe { libloading::Library::new("gdiplus.dll") }.ok()?;
        unsafe fn sym<T: Copy>(lib: &Library, name: &[u8]) -> Option<T> {
            let s: Symbol<T> = lib.get(name).ok()?;
            Some(*s)
        }
        let startup: unsafe extern "C" fn(
            *mut usize,
            *const GdiplusStartupInput,
            *mut GdiplusStartupOutput,
        ) -> i32 = unsafe { sym(&lib, b"GdiplusStartup") }?;
        let shutdown: unsafe extern "C" fn(usize) = unsafe { sym(&lib, b"GdiplusShutdown") }?;
        let _ = shutdown;

        let mut input = GdiplusStartupInput {
            gdiplus_version: 1,
            debug_event_callback: std::ptr::null_mut(),
            suppress_background_thread: 0,
            suppress_external_codecs: 0,
        };
        let mut token: usize = 0;
        let status = unsafe { startup(&mut token, &input, std::ptr::null_mut()) };
        if status != STATUS_OK {
            return None;
        }
        let create_from_hdc = unsafe { sym(&lib, b"GdipCreateFromHDC") }?;
        let delete_graphics = unsafe { sym(&lib, b"GdipDeleteGraphics") }?;
        let set_text_rendering_hint = unsafe { sym(&lib, b"GdipSetTextRenderingHint") }?;
        let new_private_font_collection = unsafe { sym(&lib, b"GdipNewPrivateFontCollection") }?;
        let private_add_font_file = unsafe { sym(&lib, b"GdipPrivateAddFontFile") }?;
        let create_font_family_from_name = unsafe { sym(&lib, b"GdipCreateFontFamilyFromName") }?;
        let delete_font_family = unsafe { sym(&lib, b"GdipDeleteFontFamily") }?;
        let create_font = unsafe { sym(&lib, b"GdipCreateFont") }?;
        let delete_font = unsafe { sym(&lib, b"GdipDeleteFont") }?;
        let create_solid_fill = unsafe { sym(&lib, b"GdipCreateSolidFill") }?;
        let delete_brush = unsafe { sym(&lib, b"GdipDeleteBrush") }?;
        let create_string_format = unsafe { sym(&lib, b"GdipCreateStringFormat") }?;
        let set_string_align = unsafe { sym(&lib, b"GdipSetStringFormatAlign") }?;
        let set_line_align = unsafe { sym(&lib, b"GdipSetStringFormatLineAlign") }?;
        let delete_string_format = unsafe { sym(&lib, b"GdipDeleteStringFormat") }?;
        let measure_string = unsafe { sym(&lib, b"GdipMeasureString") }?;
        let draw_string = unsafe { sym(&lib, b"GdipDrawString") }?;
        Some(Gdiplus {
            _lib: lib,
            token,
            create_from_hdc,
            delete_graphics,
            set_text_rendering_hint,
            new_private_font_collection,
            private_add_font_file,
            create_font_family_from_name,
            delete_font_family,
            create_font,
            delete_font,
            create_solid_fill,
            delete_brush,
            create_string_format,
            set_string_align,
            set_line_align,
            delete_string_format,
            measure_string,
            draw_string,
        })
    })
    .as_ref()
}

/// 查找 Flutter 打包的字体文件路径(exe 旁 → 开发构建目录 → 源码 assets)
fn find_font_file() -> Option<std::path::PathBuf> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    // 1) exe 旁(打包分发:data/flutter_assets/assets/)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(
                dir.join("data")
                    .join("flutter_assets")
                    .join("assets")
                    .join("LXGWWenKaiMono-Regular.ttf"),
            );
        }
    }
    // 2) 开发构建目录 / 源码 assets(CARGO_MANIFEST_DIR 仅编译期可用)
    if let Some(manifest) = option_env!("CARGO_MANIFEST_DIR") {
        let m = std::path::Path::new(manifest);
        for sub in [
            "flutter_settings/build/windows/x64/runner/Release/data/flutter_assets/assets",
            "flutter_settings/build/windows/x64/runner/Debug/data/flutter_assets/assets",
            "flutter_settings/assets",
        ] {
            candidates.push(m.join(sub).join("LXGWWenKaiMono-Regular.ttf"));
        }
    }
    candidates.into_iter().find(|p| p.exists())
}

/// GDI+ 私人字体集合(通知渲染用,系统字体不经过 GDI 注册表),仅主线程访问
static mut GP_FONT_COLLECTION: *mut std::ffi::c_void = std::ptr::null_mut();

/// 把 Flutter 字体文件加入 GDI+ 私人字体集合。
/// System.Drawing 的 PrivateFontCollection 同路径(该字体经
/// AddFontResourceExW + CreateFontFromLogfont 会报 NotTrueTypeFont)。
unsafe fn gdiplus_load_font_file() -> bool {
    if !GP_FONT_COLLECTION.is_null() {
        return true;
    }
    let gp = match gdiplus() {
        Some(g) => g,
        None => return false,
    };
    let mut col: *mut std::ffi::c_void = std::ptr::null_mut();
    if (gp.new_private_font_collection)(&mut col) != STATUS_OK || col.is_null() {
        return false;
    }
    let Some(path) = find_font_file() else {
        return false;
    };
    let wide: Vec<u16> = path
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let st = (gp.private_add_font_file)(col, wide.as_ptr());
    if st == STATUS_OK {
        GP_FONT_COLLECTION = col;
        eprintln!("[overlay] GDI+ 私人字体已加载: {}", path.display());
        true
    } else {
        false
    }
}

/// 缓存的 GDI+ 通知字体(48px,创建一次全程复用)。
/// 不能每次渲染创建/删除:GDI+ 对同一 collection 重复
/// create_font_family_from_name 会返回已释放的同一地址(悬垂),
/// 第二次渲染即崩溃。
static mut GP_NOTIF_FONT: *mut std::ffi::c_void = std::ptr::null_mut();
static mut GP_NOTIF_FAMILY: *mut std::ffi::c_void = std::ptr::null_mut();

/// 获取缓存的 GDI+ 通知字体(48px,霞鹜文楷等宽;失败回退微软雅黑)
unsafe fn gdiplus_get_notif_font(gp: &Gdiplus) -> *mut std::ffi::c_void {
    if !GP_NOTIF_FONT.is_null() {
        return GP_NOTIF_FONT;
    }
    let collection = if gdiplus_load_font_file() {
        GP_FONT_COLLECTION
    } else {
        std::ptr::null_mut()
    };
    let names: [&str; 2] = ["霞鹜文楷等宽", "Microsoft YaHei UI"];
    for name in names {
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let mut family: *mut std::ffi::c_void = std::ptr::null_mut();
        let st = (gp.create_font_family_from_name)(wide.as_ptr(), collection, &mut family);
        if st != STATUS_OK || family.is_null() {
            continue;
        }
        let mut font: *mut std::ffi::c_void = std::ptr::null_mut();
        let st2 = (gp.create_font)(family, 48.0, 0, UNIT_PIXEL, &mut font);
        if st2 == STATUS_OK && !font.is_null() {
            GP_NOTIF_FAMILY = family;
            GP_NOTIF_FONT = font;

            return font;
        }
        let _ = (gp.delete_font_family)(family);
    }
    GP_NOTIF_FONT
}

struct HudState {
    /// 通知窗口(屏幕中央)
    notif_hwnd: HWND,
    /// 压力监控窗口(左上角,macOS 同位置)
    pm_hwnd: HWND,
    /// 短暂通知文本(1.2s 后清除)
    notif: Option<String>,
    /// 压力监控文本(常驻,开关控制)
    pm_text: String,
    pm_visible: bool,
}

static mut HUD: *mut HudState = std::ptr::null_mut();

fn hud_ref() -> &'static mut HudState {
    unsafe { &mut *HUD }
}

unsafe extern "system" fn hud_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            let hud = hud_ref();
            // 通知窗口内容由 UpdateLayeredWindow 直接呈现,这里只清无效区;
            // 压力窗口保持 GDI 绘制
            if hwnd == hud.pm_hwnd {
                // 先把无效区涂成不透明黑底:类背景刷为空、WM_ERASEBKGND 不擦除,
                // 高频刷新时旧字残影叠加会糊成一团(看起来像乱码)
                let rc = ps.rcPaint;
                let _ = PatBlt(
                    hdc,
                    rc.left,
                    rc.top,
                    rc.right - rc.left,
                    rc.bottom - rc.top,
                    BLACKNESS,
                );
                let text = if hud.pm_visible {
                    hud.pm_text.clone()
                } else {
                    String::new()
                };
                if !text.is_empty() {
                    // 固定系统雅黑:LXGW 私有字体经 GDI CreateFont 匹配会失败
                    // (NotTrueTypeFont,见 gdiplus_load_font_file 注释),
                    // 回退渲染缺字形导致乱码;数字/中文用雅黑都稳定
                    let font_name = "Microsoft YaHei UI";
                    let font = CreateFontW(
                        -13,
                        0,
                        0,
                        0,
                        400,
                        0,
                        0,
                        0,
                        DEFAULT_CHARSET,
                        OUT_DEFAULT_PRECIS,
                        CLIP_DEFAULT_PRECIS,
                        CLEARTYPE_QUALITY,
                        0x40, // FF_DONTCARE
                        PCWSTR(wide_string(font_name).as_ptr()),
                    );
                    let old_font = SelectObject(hdc, font.into());
                    let _ = SetBkMode(hdc, TRANSPARENT);
                    let _ = SetTextColor(hdc, COLORREF(0x00_FF_FF_FF));
                    let mut rc = RECT {
                        left: 4,
                        top: 0,
                        right: HUD_PM_W - 4,
                        bottom: HUD_PM_H,
                    };
                    let mut wide: Vec<u16> = text.encode_utf16().collect();
                    let _ = DrawTextW(
                        hdc,
                        wide.as_mut_slice(),
                        &mut rc,
                        DT_VCENTER | DT_SINGLELINE,
                    );
                    let _ = SelectObject(hdc, old_font);
                    let _ = DeleteObject(font.into());
                }
            }
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_TIMER => {
            if wparam.0 as usize == HUD_NOTIF_TIMER {
                let _ = KillTimer(Some(hwnd), HUD_NOTIF_TIMER);
                let hud = hud_ref();
                hud.notif = None;
                let _ = ShowWindow(hud.notif_hwnd, SW_HIDE);
            }
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_DESTROY => LRESULT(0),
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// 创建一个小 HUD 窗口。压力窗口带黑色半透明底(SetLayeredWindowAttributes),
/// 通知窗口不用(内容用 UpdateLayeredWindow per-pixel alpha 直接呈现,无背景)。
unsafe fn hud_create_window(
    class: PCWSTR,
    hinst: HINSTANCE,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    black_bg: bool,
) -> HWND {
    let hwnd = CreateWindowExW(
        WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
        class,
        PCWSTR::null(),
        WS_POPUP,
        x,
        y,
        w,
        h,
        None,
        None,
        Some(hinst),
        None,
    )
    .expect("CreateWindowExW (HUD) failed");
    if black_bg {
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 220, LWA_ALPHA);
    }
    hwnd
}

/// 创建 HUD 窗口(通知:屏幕正中央透明;压力:左上角 macOS 同位置黑底)
fn hud_create() -> (HWND, HWND) {
    unsafe {
        if gdiplus().is_some() {
            eprintln!("[overlay] GDI+ 已初始化");
        } else {
            eprintln!("[overlay] GDI+ 初始化失败!");
        }
        let class_name = wide_string("Glaspen2Hud");
        let hinst: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
        let wc = WNDCLASSW {
            style: WNDCLASS_STYLES(0),
            lpfnWndProc: Some(hud_wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: HICON::default(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: HBRUSH::default(),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
        };
        let _ = RegisterClassW(&wc);

        let sw = GetSystemMetrics(SM_CXSCREEN);
        let sh = GetSystemMetrics(SM_CYSCREEN);
        let class = PCWSTR(class_name.as_ptr());
        // 通知:屏幕正中央,透明(无背景)
        let notif_hwnd = hud_create_window(
            class,
            hinst,
            (sw - HUD_NOTIF_W) / 2,
            (sh - HUD_NOTIF_H) / 2,
            HUD_NOTIF_W,
            HUD_NOTIF_H,
            false,
        );
        // 压力监控:左上角 (10, 40),macOS 为 (10, height-40),黑底
        let pm_hwnd = hud_create_window(class, hinst, 10, 40, HUD_PM_W, HUD_PM_H, true);
        (notif_hwnd, pm_hwnd)
    }
}

/// 在通知窗口渲染文字:GDI+ 抗锯齿,透明背景 + 柔和黑阴影 + 白字。
/// 与 macOS 一样文字直接浮在屏幕上,没有黑底条。
unsafe fn hud_render_notif_text(hwnd: HWND, text: &str) {
    let pad = 16;
    let mut wide: Vec<u16> = text.encode_utf16().collect();

    // 测量文字尺寸(GDI+ 测量)
    let mut layout = RectF {
        x: 0.0,
        y: 0.0,
        width: 10000.0,
        height: 10000.0,
    };
    let mut measured = RectF {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };
    let w;
    let h;
    {
        let gp = match gdiplus() {
            Some(g) => g,
            None => {
                eprintln!("[overlay] GDI+ 不可用,通知渲染失败");
                return;
            }
        };
        let (font, _family) = (gdiplus_get_notif_font(gp), GP_NOTIF_FAMILY);
        if font.is_null() {
            eprintln!("[overlay] GDI+ 字体创建失败");
            return;
        }
        let screen_dc = GetDC(None);
        let mut graphics: *mut std::ffi::c_void = std::ptr::null_mut();
        let st1 = (gp.create_from_hdc)(screen_dc, &mut graphics);
        if st1 != STATUS_OK || graphics.is_null() {
            eprintln!("[overlay] GDI+ create_from_hdc 失败: st={}", st1);
            let _ = ReleaseDC(None, screen_dc);
            return;
        }
        let _ = (gp.set_text_rendering_hint)(graphics, TEXT_RENDERING_HINT_ANTIALIAS);
        let mut format: *mut std::ffi::c_void = std::ptr::null_mut();
        let _ = (gp.create_string_format)(0, 0, &mut format);
        if !format.is_null() {
            let _ = (gp.set_string_align)(format, STRING_ALIGNMENT_CENTER);
            let _ = (gp.set_line_align)(format, STRING_ALIGNMENT_CENTER);
        }
        let mut fitted = 0i32;
        let mut lines = 0i32;
        let _ = (gp.measure_string)(
            graphics,
            wide.as_ptr(),
            wide.len() as i32,
            font,
            &layout,
            format,
            &mut measured,
            &mut fitted,
            &mut lines,
        );
        // 窗口比文字四周多出 pad*2 余量,绘制区域再留 pad 内边距,
        // 保证最后一个字符不被 GDI+ 在区域边缘裁剪
        w = (measured.width as i32) + pad * 4;
        h = (measured.height as i32) + pad * 4;
        let _ = (gp.delete_string_format)(format);
        let _ = (gp.delete_graphics)(graphics);
        let _ = ReleaseDC(None, screen_dc);
    }

    // 居中置顶
    let sw = GetSystemMetrics(SM_CXSCREEN);
    let sh = GetSystemMetrics(SM_CYSCREEN);
    let _ = SetWindowPos(
        hwnd,
        Some(HWND_TOPMOST),
        (sw - w) / 2,
        (sh - h) / 2,
        w.max(1),
        h.max(1),
        SWP_NOACTIVATE | SWP_SHOWWINDOW,
    );

    // DIB:32bpp 全透明
    let screen_dc = GetDC(None);
    let dib_dc = CreateCompatibleDC(Some(screen_dc));
    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w.max(1),
            biHeight: -h.max(1),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
    let hbmp = CreateDIBSection(Some(dib_dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0).unwrap();
    let old = SelectObject(dib_dc, hbmp.into());
    std::slice::from_raw_parts_mut(bits as *mut u8, (w as usize) * (h as usize) * 4).fill(0);

    // GDI+ 绘制:柔和黑阴影(多层偏移模拟 blur) + 白字
    {
        let gp = match gdiplus() {
            Some(g) => g,
            None => {
                let _ = SelectObject(dib_dc, old);
                let _ = DeleteObject(hbmp.into());
                let _ = DeleteDC(dib_dc);
                let _ = ReleaseDC(None, screen_dc);
                return;
            }
        };
        let font = gdiplus_get_notif_font(gp);
        if font.is_null() {
            let _ = SelectObject(dib_dc, old);
            let _ = DeleteObject(hbmp.into());
            let _ = DeleteDC(dib_dc);
            let _ = ReleaseDC(None, screen_dc);
            return;
        }
        let mut graphics: *mut std::ffi::c_void = std::ptr::null_mut();
        if (gp.create_from_hdc)(dib_dc, &mut graphics) != STATUS_OK || graphics.is_null() {
            let _ = SelectObject(dib_dc, old);
            let _ = DeleteObject(hbmp.into());
            let _ = DeleteDC(dib_dc);
            let _ = ReleaseDC(None, screen_dc);
            return;
        }
        let _ = (gp.set_text_rendering_hint)(graphics, TEXT_RENDERING_HINT_ANTIALIAS);
        let mut format: *mut std::ffi::c_void = std::ptr::null_mut();
        let _ = (gp.create_string_format)(0, 0, &mut format);
        if !format.is_null() {
            let _ = (gp.set_string_align)(format, STRING_ALIGNMENT_CENTER);
            let _ = (gp.set_line_align)(format, STRING_ALIGNMENT_CENTER);
        }
        let area = RectF {
            x: pad as f32,
            y: pad as f32,
            // 绘制区域比文字实际宽度多出 pad*2,文字居中,末字不会被裁剪
            width: (w - pad * 2) as f32,
            height: (h - pad * 2) as f32,
        };
        // 阴影:黑色半透明刷子,多层偏移模拟 blur
        let mut shadow_brush: *mut std::ffi::c_void = std::ptr::null_mut();
        let _ = (gp.create_solid_fill)(0xB0000000, &mut shadow_brush); // ARGB: alpha 176, 黑
        if !shadow_brush.is_null() {
            for (dx, dy) in [
                (2.0, 2.0),
                (1.0, 1.0),
                (2.0, 0.0),
                (0.0, 2.0),
                (1.0, 2.0),
                (2.0, 1.0),
            ] {
                let sr = RectF {
                    x: area.x + dx,
                    y: area.y + dy,
                    width: area.width,
                    height: area.height,
                };
                let _ = (gp.draw_string)(
                    graphics,
                    wide.as_ptr(),
                    wide.len() as i32,
                    font,
                    &sr,
                    format,
                    shadow_brush,
                );
            }
            let _ = (gp.delete_brush)(shadow_brush);
        }
        // 主文字:白
        let mut white_brush: *mut std::ffi::c_void = std::ptr::null_mut();
        let _ = (gp.create_solid_fill)(0xFFFFFFFF, &mut white_brush);
        if !white_brush.is_null() {
            let _ = (gp.draw_string)(
                graphics,
                wide.as_ptr(),
                wide.len() as i32,
                font,
                &area,
                format,
                white_brush,
            );
            let _ = (gp.delete_brush)(white_brush);
        }
        let _ = (gp.delete_string_format)(format);
        let _ = (gp.delete_graphics)(graphics);
    }

    // GDI+ 输出到 32bpp DIB 为非预乘 ARGB,ULW 需要预乘:RGB *= alpha/255
    {
        let n = (w as usize) * (h as usize);
        let p = bits as *mut u8;
        for i in 0..n {
            let off = i * 4;
            let a = *p.add(off + 3);
            if a != 0 && a != 255 {
                let r = (*p.add(off + 2) as u32 * a as u32 + 127) / 255;
                let g = (*p.add(off + 1) as u32 * a as u32 + 127) / 255;
                let b = (*p.add(off) as u32 * a as u32 + 127) / 255;
                *p.add(off) = b as u8;
                *p.add(off + 1) = g as u8;
                *p.add(off + 2) = r as u8;
            }
        }
    }

    // ULW 呈现(透明背景 + 文字)
    let blend = BLENDFUNCTION {
        BlendOp: 0,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: 1, // AC_SRC_ALPHA
    };
    let size = SIZE {
        cx: w.max(1),
        cy: h.max(1),
    };
    let src = POINT { x: 0, y: 0 };
    let _ = UpdateLayeredWindow(
        hwnd,
        None,
        None,
        Some(&size),
        Some(dib_dc),
        Some(&src),
        COLORREF(0),
        Some(&blend),
        ULW_ALPHA,
    );

    // 清理
    let _ = SelectObject(dib_dc, old);
    let _ = DeleteObject(hbmp.into());
    let _ = DeleteDC(dib_dc);
    let _ = ReleaseDC(None, screen_dc);
}

/// 显示短暂通知(1.2s 后自动消失)
fn hud_notify(text: &str) {
    unsafe {
        if HUD.is_null() {
            return;
        }
        let hud = hud_ref();
        hud.notif = Some(text.to_string());
        hud_render_notif_text(hud.notif_hwnd, text);
        let _ = SetTimer(Some(hud.notif_hwnd), HUD_NOTIF_TIMER, HUD_NOTIF_MS, None);
    }
}

/// 更新压力监控文本(悬空/按下事件每帧调用)
fn hud_update_pressure(pressure: i32, down: bool, x: i32, y: i32) {
    unsafe {
        if HUD.is_null() {
            return;
        }
        let hud = hud_ref();
        if !hud.pm_visible {
            return;
        }
        let evtype = if down { "按下" } else { "悬空" };
        hud.pm_text = format!("压力 P={:<5}  {}  ({},{})", pressure, evtype, x, y);
        let _ = InvalidateRect(Some(hud.pm_hwnd), None, false);
    }
}

/// 开关压力监控
fn hud_toggle_pressure(on: bool) {
    unsafe {
        if HUD.is_null() {
            return;
        }
        let hud = hud_ref();
        hud.pm_visible = on;
        if on {
            hud.pm_text = "压力 P=-----  悬空  (---,---)".to_string();
            let _ = ShowWindow(hud.pm_hwnd, SW_SHOWNOACTIVATE);
        } else {
            let _ = ShowWindow(hud.pm_hwnd, SW_HIDE);
        }
        let _ = InvalidateRect(Some(hud.pm_hwnd), None, false);
    }
}

