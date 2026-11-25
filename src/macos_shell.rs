//! macOS AppKit 壳的 Rust 预研(R14): 先用最外层状态栏项创建验证 objc2
//! 在真实 app 内可工作; 默认仍走 Swift, 由 `GLASPEN2_RUST_STATUS_ITEM=1` 切换。
#![cfg(target_os = "macos")]

use std::ffi::c_void;

use objc2::rc::Retained;
use objc2::{AnyThread, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::NSStatusBar;
use objc2_foundation::MainThreadMarker;

/// 创建方形 NSStatusItem, 返回 +1 持有的裸指针(由 Swift 侧接管引用;
/// 本项目 app 生命周期 = 进程生命周期, 不额外实现释放)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_status_item_create() -> *mut c_void {
    let Some(_mtm) = MainThreadMarker::new() else {
        tracing::error!("rust-shell: status item create 不在主线程");
        return std::ptr::null_mut();
    };
    let bar = NSStatusBar::systemStatusBar();
    // NSSquareStatusItemLength = -2.0; 常量未在 objc2 绑定里暴露。
    let item = bar.statusItemWithLength(-2.0);
    tracing::info!("rust-shell: NSStatusItem created by Rust (objc2)");
    Retained::into_raw(item) as *mut c_void
}

/// R16: 创建覆盖层内容容器(NSView, layer-backed)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_overlay_content_view_create(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> *mut c_void {
    use objc2_app_kit::NSView;
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    let Some(mtm) = MainThreadMarker::new() else {
        return std::ptr::null_mut();
    };
    let frame = NSRect::new(NSPoint::new(x, y), NSSize::new(width, height));
    let view = NSView::initWithFrame(NSView::alloc(mtm), frame);
    view.setWantsLayer(true);
    Retained::into_raw(view) as *mut c_void
}

/// R16: 创建磨砂玻璃 NSVisualEffectView(behind-window/light/active)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_overlay_glass_view_create(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    enabled: i32,
    opacity: f64,
) -> *mut c_void {
    use objc2_app_kit::{
        NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView,
    };
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    let Some(mtm) = MainThreadMarker::new() else {
        return std::ptr::null_mut();
    };
    let frame = NSRect::new(NSPoint::new(x, y), NSSize::new(width, height));
    let view = NSVisualEffectView::initWithFrame(NSVisualEffectView::alloc(mtm), frame);
    let enabled = enabled != 0;
    view.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    view.setMaterial(NSVisualEffectMaterial::Light);
    view.setState(NSVisualEffectState::Active);
    view.setAlphaValue(if enabled { opacity * 2.0 } else { 0.0 });
    view.setHidden(!enabled);
    Retained::into_raw(view) as *mut c_void
}

/// R15: 用 Rust/objc2 创建覆盖层主窗口(层级/透明/穿透/集合行为),
/// 返回 +1 持有指针; Swift 仅在 `GLASPEN2_RUST_OVERLAY=1` 时使用。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_overlay_window_create(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> *mut c_void {
    use objc2_app_kit::{
        NSBackingStoreType, NSColor, NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask,
    };
    use objc2_foundation::{NSPoint, NSRect, NSSize, ns_string};

    let Some(mtm) = MainThreadMarker::new() else {
        tracing::error!("rust-shell: overlay window create 不在主线程");
        return std::ptr::null_mut();
    };
    let frame = NSRect::new(NSPoint::new(x, y), NSSize::new(width, height));
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            frame,
            NSWindowStyleMask::Borderless,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    unsafe { window.setReleasedWhenClosed(false) };
    window.setLevel(objc2_core_graphics::kCGMaximumWindowLevel as isize);
    window.setOpaque(false);
    window.setBackgroundColor(Some(&NSColor::clearColor()));
    window.setTitle(ns_string!("glaspen2"));
    window.setAcceptsMouseMovedEvents(true);
    window.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces | NSWindowCollectionBehavior::Stationary,
    );
    window.setIgnoresMouseEvents(true);
    tracing::info!("rust-shell: overlay NSWindow created by Rust (objc2)");
    Retained::into_raw(window) as *mut c_void
}

// ── R17: 状态栏图标渲染(objc2 移植 Swift StatusIconRenderer) ─────────────

use objc2_app_kit::{
    NSBezierPath, NSBitmapImageRep, NSColor, NSGraphicsContext, NSImage, NSLineCapStyle,
};
use objc2_foundation::{NSAffineTransform, NSPoint, NSRect, NSSize};

fn mark_transform() -> Retained<NSAffineTransform> {
    let t = NSAffineTransform::transform();
    t.translateXBy_yBy(9.0, 9.0);
    t.rotateByDegrees(45.0);
    t.translateXBy_yBy(-9.0, -9.0);
    t
}

fn stroke_inked(path: &NSBezierPath, width: f64, halo: &NSColor, ink: &NSColor) {
    path.setLineWidth(width + 1.2);
    halo.setStroke();
    path.stroke();
    path.setLineWidth(width);
    ink.setStroke();
    path.stroke();
}

fn render_status_icon(
    eraser: bool,
    enabled: bool,
    ethereal: bool,
    infinite: bool,
    width_scale: f64,
    pen_r: f64,
    pen_g: f64,
    pen_b: f64,
) -> Option<Retained<NSImage>> {
    let mtm = MainThreadMarker::new()?;
    let size = 18.0f64;
    let px = 72isize;
    let rep = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(), px, px, 8, 4, true, false,
            unsafe { objc2_app_kit::NSCalibratedRGBColorSpace },
            0, 0,
        )
    }?;
    unsafe { rep.setSize(NSSize::new(size, size)) };
    let ctx = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)?;
    NSGraphicsContext::saveGraphicsState_class();
    NSGraphicsContext::setCurrentContext(Some(&ctx));

    let ink = NSColor::colorWithWhite_alpha(0.0, if enabled { 0.78 } else { 0.78 * 0.35 });
    let halo = NSColor::colorWithWhite_alpha(1.0, if enabled { 0.88 } else { 0.88 * 0.35 });
    let pen = NSColor::colorWithDeviceRed_green_blue_alpha(
        pen_r,
        pen_g,
        pen_b,
        if enabled { 1.0 } else { 0.35 },
    );
    let dim = if enabled { 1.0 } else { 0.35 };
    let ink_dim = NSColor::colorWithWhite_alpha(0.0, dim);
    let halo_dim = NSColor::colorWithWhite_alpha(1.0, dim);
    let _ = &ink;
    let _ = &halo;

    let t = mark_transform();
    if eraser && enabled {
        let body = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
            NSRect::new(NSPoint::new(3.5, 6.1), NSSize::new(11.2, 5.8)),
            1.5,
            1.5,
        );
        body.transformUsingAffineTransform(&t);
        let band = NSBezierPath::bezierPathWithRect(NSRect::new(
            NSPoint::new(10.2, 6.1),
            NSSize::new(4.5, 5.8),
        ));
        band.transformUsingAffineTransform(&t);
        body.setLineWidth(0.8 + 1.2);
        halo_dim.setStroke();
        body.stroke();
        NSColor::colorWithWhite_alpha(0.92, dim).setFill();
        body.fill();
        NSColor::colorWithWhite_alpha(0.25, dim).setFill();
        band.fill();
        body.setLineWidth(0.8);
        ink_dim.setStroke();
        body.stroke();
    } else {
        let th = (2.4 + width_scale.sqrt() * 1.9).clamp(2.6, 6.0);
        let body = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
            NSRect::new(NSPoint::new(5.5, 9.0 - th / 2.0), NSSize::new(9.0, th)),
            th.min(3.2) / 2.0,
            th.min(3.2) / 2.0,
        );
        let tip = NSBezierPath::bezierPath();
        tip.moveToPoint(NSPoint::new(5.5, 9.0 - th / 2.0));
        tip.lineToPoint(NSPoint::new(3.0, 9.0));
        tip.lineToPoint(NSPoint::new(5.5, 9.0 + th / 2.0));
        tip.closePath();
        let sil = NSBezierPath::bezierPath();
        sil.appendBezierPath(&body);
        sil.appendBezierPath(&tip);
        sil.transformUsingAffineTransform(&t);
        tip.transformUsingAffineTransform(&t);
        sil.setLineWidth(0.9 + 1.2);
        halo_dim.setStroke();
        sil.stroke();
        pen.setFill();
        sil.fill();
        ink_dim.setFill();
        tip.fill();
        sil.setLineWidth(0.9);
        ink_dim.setStroke();
        sil.stroke();
    }

    if !enabled {
        let slash = NSBezierPath::bezierPath();
        slash.moveToPoint(NSPoint::new(2.6, 15.4));
        slash.lineToPoint(NSPoint::new(15.4, 2.6));
        slash.setLineCapStyle(NSLineCapStyle::Round);
        slash.setLineWidth(2.8);
        NSColor::colorWithWhite_alpha(1.0, 0.88).setStroke();
        slash.stroke();
        slash.setLineWidth(1.6);
        NSColor::colorWithWhite_alpha(0.0, 0.78).setStroke();
        slash.stroke();
    }

    if ethereal {
        let dot = NSBezierPath::bezierPathWithOvalInRect(NSRect::new(
            NSPoint::new(1.9, 13.1),
            NSSize::new(3.0, 3.0),
        ));
        stroke_inked(
            &dot,
            1.0,
            &NSColor::colorWithWhite_alpha(1.0, 0.88),
            &NSColor::colorWithWhite_alpha(0.0, 0.78),
        );
    }
    if infinite {
        let inf = NSBezierPath::bezierPath();
        inf.appendBezierPathWithOvalInRect(NSRect::new(
            NSPoint::new(12.9, 1.6),
            NSSize::new(2.4, 2.4),
        ));
        inf.appendBezierPathWithOvalInRect(NSRect::new(
            NSPoint::new(15.1, 1.6),
            NSSize::new(2.4, 2.4),
        ));
        stroke_inked(
            &inf,
            1.0,
            &NSColor::colorWithWhite_alpha(1.0, 0.88),
            &NSColor::colorWithWhite_alpha(0.0, 0.78),
        );
    }

    NSGraphicsContext::restoreGraphicsState_class();
    let image = NSImage::initWithSize(NSImage::alloc(), NSSize::new(size, size));
    image.addRepresentation(&rep);
    Some(image)
}

/// R17: 状态栏图标(18pt/72px), 返回 +1 持有 NSImage; Swift 在
/// `GLASPEN2_RUST_STATUS_ICON=1` 时使用, 默认 Swift/失败回退。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_status_icon_render(
    eraser: i32,
    enabled: i32,
    ethereal: i32,
    infinite: i32,
    width_scale: f64,
    pen_r: f64,
    pen_g: f64,
    pen_b: f64,
) -> *mut c_void {
    let out = render_status_icon(
        eraser != 0,
        enabled != 0,
        ethereal != 0,
        infinite != 0,
        width_scale,
        pen_r,
        pen_g,
        pen_b,
    );
    tracing::info!("rust-shell: status icon render ok={}", out.is_some());
    match out {
        Some(image) => Retained::into_raw(image) as *mut c_void,
        None => std::ptr::null_mut(),
    }
}

/// R18: 覆盖层内容组装(玻璃在下/绘制视图在上 + layer 透明 + attach/orderFront)。
/// 指针来自 Swift 已创建的 NSView/NSWindow; 仅 opt-in 路径调用。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_overlay_assemble_content(
    content: *mut c_void,
    glass: *mut c_void,
    draw_view: *mut c_void,
    window: *mut c_void,
) {
    use objc2_app_kit::{NSColor, NSView, NSWindow};
    if content.is_null() || glass.is_null() || draw_view.is_null() || window.is_null() {
        return;
    }
    let content = unsafe { &*(content as *const NSView) };
    let glass = unsafe { &*(glass as *const NSView) };
    let draw_view = unsafe { &*(draw_view as *const NSView) };
    let window = unsafe { &*(window as *const NSWindow) };
    content.addSubview(glass);
    draw_view.setWantsLayer(true);
    if let Some(layer) = draw_view.layer() {
        layer.setOpaque(false);
        layer.setBackgroundColor(Some(&NSColor::clearColor().CGColor()));
    }
    content.addSubview(draw_view);
    window.setContentView(Some(content));
    window.orderFront(None);
    tracing::info!("rust-shell: overlay content assembled by Rust (objc2)");
}

// ── R19: Rust 绘制视图(NSView 子类, drawRect 回调 Swift 绘制桥) ──────────

#[derive(Default)]
struct RustDrawViewIvars {}

define_class!(
    #[unsafe(super = objc2_app_kit::NSView)]
    #[thread_kind = MainThreadOnly]
    #[name = "GlaspenRustDrawView"]
    #[ivars = RustDrawViewIvars]
    struct RustDrawView;

    impl RustDrawView {
        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, rect: objc2_foundation::NSRect) {
            unsafe extern "C" {
                fn glaspen2_swift_draw_overlay_bridge(
                    view: *mut c_void, x: f64, y: f64, w: f64, h: f64,
                );
            }
            let view = self as *const Self as *mut c_void;
            unsafe {
                glaspen2_swift_draw_overlay_bridge(
                    view, rect.origin.x, rect.origin.y, rect.size.width, rect.size.height,
                );
            }
        }
    }
);

impl RustDrawView {
    fn new(mtm: MainThreadMarker, frame: objc2_foundation::NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(RustDrawViewIvars::default());
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

/// R19: 创建 Rust NSView 子类绘制视图; drawRect 通过 C 桥回调 Swift OverlayDrawing。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_draw_view_create(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> *mut c_void {
    let Some(mtm) = MainThreadMarker::new() else {
        return std::ptr::null_mut();
    };
    let frame = objc2_foundation::NSRect::new(
        objc2_foundation::NSPoint::new(x, y),
        objc2_foundation::NSSize::new(width, height),
    );
    let view = RustDrawView::new(mtm, frame);
    tracing::info!("rust-shell: draw NSView created by Rust (objc2)");
    Retained::into_raw(view) as *mut c_void
}

/// R22: 用 Rust/objc2 创建状态栏主菜单对象(菜单项仍由 Swift 构建)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_menu_create() -> *mut c_void {
    use objc2_app_kit::NSMenu;
    let Some(mtm) = MainThreadMarker::new() else {
        return std::ptr::null_mut();
    };
    let menu = NSMenu::new(mtm);
    tracing::info!("rust-shell: NSMenu created by Rust (objc2)");
    Retained::into_raw(menu) as *mut c_void
}

/// R22: Rust 把菜单挂到状态栏项(statusItem.menu = menu)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_status_item_set_menu(item: *mut c_void, menu: *mut c_void) {
    use objc2_app_kit::{NSMenu, NSStatusItem};
    if item.is_null() || menu.is_null() {
        return;
    }
    let item = unsafe { &*(item as *const NSStatusItem) };
    let menu = unsafe { &*(menu as *const NSMenu) };
    item.setMenu(Some(menu));
    tracing::info!("rust-shell: status item menu attached by Rust (objc2)");
}

/// R23: 批量追加菜单项(颜色/粗细预设); 富文本标题由 Swift 预构建, Rust 负责
/// NSMenuItem 创建/attributedTitle/target/action/tag/追加。返回 1 成功, 0 失败。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_menu_add_items(
    menu: *mut c_void,
    handler: *mut c_void,
    count: i32,
    titles: *const *mut c_void,
    title_count: i32,
    action: *const std::os::raw::c_char,
) -> i32 {
    use objc2::runtime::{AnyObject, Sel};
    use objc2_app_kit::{NSMenu, NSMenuItem};
    use objc2_foundation::NSAttributedString;

    if menu.is_null() || titles.is_null() || action.is_null() {
        return 0;
    }
    let Some(mtm) = MainThreadMarker::new() else {
        return 0;
    };
    let menu = unsafe { &*(menu as *const NSMenu) };
    let action = unsafe { std::ffi::CStr::from_ptr(action) };
    let sel = Sel::register(action);
    let n = count.max(0).min(title_count.max(0));
    for i in 0..n {
        let title = unsafe { *titles.add(i as usize) };
        if title.is_null() {
            continue;
        }
        let title = unsafe { &*(title as *const NSAttributedString) };
        let item = NSMenuItem::new(mtm);
        item.setAttributedTitle(Some(title));
        if !handler.is_null() {
            let handler = unsafe { &*(handler as *const AnyObject) };
            unsafe { item.setTarget(Some(handler)) };
        }
        unsafe { item.setAction(Some(sel)) };
        item.setTag(i as isize);
        menu.addItem(&item);
    }
    tracing::info!("rust-shell: {n} menu items added by Rust (objc2)");
    1
}

/// R24: 构建顶层其余项 + 导出▸/更多▸ 子菜单; titles 是已本地化 JSON 对象。
/// 返回 1 成功, 0 失败(Swift 回退)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_menu_add_top_level(
    menu: *mut c_void,
    handler: *mut c_void,
    titles_json: *const std::os::raw::c_char,
    launch_at_login_on: i32,
    glass_on: i32,
) -> i32 {
    use objc2::runtime::{AnyObject, Sel};
    use objc2_app_kit::{NSControlStateValueOn, NSMenu, NSMenuItem};
    use objc2_foundation::NSString;

    if menu.is_null() || titles_json.is_null() {
        return 0;
    }
    let Some(mtm) = MainThreadMarker::new() else {
        return 0;
    };
    let titles_json = unsafe { std::ffi::CStr::from_ptr(titles_json) }.to_string_lossy();
    let Ok(titles) = serde_json::from_str::<serde_json::Value>(&titles_json) else {
        return 0;
    };
    let Some(obj) = titles.as_object() else {
        return 0;
    };
    let text = |key: &str| -> String {
        obj.get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    let menu = unsafe { &*(menu as *const NSMenu) };
    let handler = if handler.is_null() {
        None
    } else {
        Some(unsafe { &*(handler as *const AnyObject) })
    };

    let empty = NSString::from_str("");
    let make = |m: &NSMenu, title: &str, action: Option<&str>| -> Retained<NSMenuItem> {
        let title = NSString::from_str(title);
        let sel = action.map(|a| Sel::register(std::ffi::CString::new(a).unwrap().as_c_str()));
        let item = unsafe { m.addItemWithTitle_action_keyEquivalent(&title, sel, &empty) };
        if let Some(handler) = handler {
            unsafe { item.setTarget(Some(handler)) };
        }
        item
    };

    menu.addItem(&NSMenuItem::separatorItem(mtm));
    make(menu, &text("saveWithBg"), Some("saveWithBg"));
    make(menu, &text("saveDrawing"), Some("saveOnly"));
    make(menu, &text("newCanvas"), Some("clearScreen"));

    let export = make(menu, &text("export"), None);
    let export_menu = NSMenu::new(mtm);
    export_menu.setAutoenablesItems(false);
    make(&export_menu, &text("exportXournal"), Some("saveXoj"));
    make(
        &export_menu,
        &text("exportInfinitePdf"),
        Some("exportInfinitePdf"),
    );
    make(
        &export_menu,
        &text("exportInfiniteSvg"),
        Some("exportInfiniteSvg"),
    );
    export.setSubmenu(Some(&export_menu));

    menu.addItem(&NSMenuItem::separatorItem(mtm));
    let toggle = make(menu, &text("toggleDraw"), Some("toggleDraw"));
    toggle.setTag(888);
    toggle.setState(NSControlStateValueOn);
    make(menu, &text("settings"), Some("showSettingsPanel"));

    let more = make(menu, &text("more"), None);
    let more_menu = NSMenu::new(mtm);
    more_menu.setAutoenablesItems(false);
    let launch = make(&more_menu, &text("moreLaunch"), Some("toggleLaunch"));
    launch.setTag(777);
    launch.setState(if launch_at_login_on != 0 {
        NSControlStateValueOn
    } else {
        0
    });
    let glass = make(&more_menu, &text("moreGlass"), Some("toggleGlass"));
    glass.setTag(444);
    glass.setState(if glass_on != 0 {
        NSControlStateValueOn
    } else {
        0
    });
    let outline = make(&more_menu, &text("moreOutline"), Some("toggleOutline"));
    outline.setTag(667);
    outline.setState(0);
    let infinite = make(
        &more_menu,
        &text("moreCanvasInfinite"),
        Some("toggleInfiniteCanvas"),
    );
    infinite.setTag(668);
    infinite.setState(0);
    let mode = make(
        &more_menu,
        &text("moreCanvasMode"),
        Some("toggleCanvasMode"),
    );
    mode.setTag(778);
    mode.setState(0);
    more.setSubmenu(Some(&more_menu));

    menu.addItem(&NSMenuItem::separatorItem(mtm));
    make(menu, &text("language"), Some("toggleLanguage"));
    make(menu, &text("quit"), Some("quitApp"));
    tracing::info!("rust-shell: top-level/submenu items built by Rust (objc2)");
    1
}
