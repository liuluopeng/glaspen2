//! macOS AppKit 壳的 Rust 预研(R14): 先用最外层状态栏项创建验证 objc2
//! 在真实 app 内可工作; 默认仍走 Swift, 由 `GLASPEN2_RUST_STATUS_ITEM=1` 切换。
#![cfg(target_os = "macos")]

use std::ffi::c_void;

use objc2::rc::Retained;
use objc2::{AnyThread, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::NSStatusBar;
use objc2_app_kit::NSWindowDelegate;
use objc2_foundation::MainThreadMarker;
use objc2_foundation::NSObjectProtocol;

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

/// R25: 按索引更新颜色/粗细菜单勾选。返回 1 成功, 0 失败(Swift 回退)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_menu_update_checkmarks(
    menu: *mut c_void,
    color_count: i32,
    width_count: i32,
    color_index: i32,
    width_index: i32,
) -> i32 {
    use objc2_app_kit::{NSControlStateValueOn, NSMenu};
    if menu.is_null() {
        return 0;
    }
    let menu = unsafe { &*(menu as *const NSMenu) };
    for i in 0..color_count.max(0) {
        if let Some(item) = menu.itemAtIndex(i as isize) {
            item.setState(if i == color_index {
                NSControlStateValueOn
            } else {
                0
            });
        }
    }
    let width_offset = color_count.max(0) + 1;
    for i in 0..width_count.max(0) {
        if let Some(item) = menu.itemAtIndex((width_offset + i) as isize) {
            item.setState(if i == width_index {
                NSControlStateValueOn
            } else {
                0
            });
        }
    }
    tracing::info!("rust-shell: menu checkmarks updated by Rust (objc2)");
    1
}

/// R26: 菜单动态文案刷新(颜色/粗细富文本 + 顶层/导出/更多 + tag 778/888)。
/// 数组长度显式传入; 失败返回 0(Swift 回退)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_menu_update_texts(
    menu: *mut c_void,
    color_titles: *const *mut c_void,
    color_count: i32,
    width_titles: *const *mut c_void,
    width_count: i32,
    top_titles_json: *const std::os::raw::c_char,
    export_titles_json: *const std::os::raw::c_char,
    more_titles_json: *const std::os::raw::c_char,
    mode_title: *const std::os::raw::c_char,
    toggle_title: *const std::os::raw::c_char,
    mode_item: *mut c_void,
    toggle_item: *mut c_void,
) -> i32 {
    use objc2_app_kit::{NSMenu, NSMenuItem};
    use objc2_foundation::{NSAttributedString, NSString};

    if menu.is_null() {
        return 0;
    }
    let cstr = |p: *const std::os::raw::c_char| -> Option<String> {
        if p.is_null() {
            None
        } else {
            Some(
                unsafe { std::ffi::CStr::from_ptr(p) }
                    .to_string_lossy()
                    .into_owned(),
            )
        }
    };
    let parse_list = |p: *const std::os::raw::c_char| -> Vec<String> {
        cstr(p)
            .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
            .unwrap_or_default()
    };
    let menu = unsafe { &*(menu as *const NSMenu) };
    let color_count = color_count.max(0);
    let width_count = width_count.max(0);

    for i in 0..color_count {
        if color_titles.is_null() {
            break;
        }
        let p = unsafe { *color_titles.add(i as usize) };
        if p.is_null() {
            continue;
        }
        if let Some(item) = menu.itemAtIndex(i as isize) {
            let title = unsafe { &*(p as *const NSAttributedString) };
            item.setAttributedTitle(Some(title));
        }
    }
    let width_base = color_count + 1;
    for i in 0..width_count {
        if width_titles.is_null() {
            break;
        }
        let p = unsafe { *width_titles.add(i as usize) };
        if p.is_null() {
            continue;
        }
        if let Some(item) = menu.itemAtIndex((width_base + i) as isize) {
            let title = unsafe { &*(p as *const NSAttributedString) };
            item.setAttributedTitle(Some(title));
        }
    }
    let base = width_base + width_count + 1;
    let top = parse_list(top_titles_json);
    for (offset, title) in top.iter().enumerate() {
        if title.is_empty() {
            continue;
        }
        if let Some(item) = menu.itemAtIndex((base + offset as i32) as isize) {
            item.setTitle(&NSString::from_str(title));
        }
    }
    if let Some(export) = menu
        .itemAtIndex((base + 3) as isize)
        .and_then(|i| i.submenu())
    {
        for (i, title) in parse_list(export_titles_json).iter().enumerate() {
            if let Some(item) = export.itemAtIndex(i as isize) {
                item.setTitle(&NSString::from_str(title));
            }
        }
    }
    if let Some(more) = menu
        .itemAtIndex((base + 7) as isize)
        .and_then(|i| i.submenu())
    {
        for (i, title) in parse_list(more_titles_json).iter().enumerate() {
            if let Some(item) = more.itemAtIndex(i as isize) {
                item.setTitle(&NSString::from_str(title));
            }
        }
    }
    if let Some(title) = cstr(mode_title) {
        if !mode_item.is_null() {
            let item = unsafe { &*(mode_item as *const NSMenuItem) };
            item.setTitle(&NSString::from_str(&title));
        }
    }
    if let Some(title) = cstr(toggle_title) {
        if !toggle_item.is_null() {
            let item = unsafe { &*(toggle_item as *const NSMenuItem) };
            item.setTitle(&NSString::from_str(&title));
        }
    }
    tracing::info!("rust-shell: menu texts updated by Rust (objc2)");
    1
}

/// R27: 递归查找菜单项(顶层 + 一层子菜单, 与旧 Swift MenuLookup 等价)。
/// 返回非持有指针(菜单仍持有); 找不到返回 null。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_menu_item_with_tag(menu: *mut c_void, tag: isize) -> *mut c_void {
    use objc2_app_kit::NSMenu;
    if menu.is_null() {
        return std::ptr::null_mut();
    }
    let menu = unsafe { &*(menu as *const NSMenu) };
    if let Some(item) = menu.itemWithTag(tag) {
        return Retained::as_ptr(&item) as *mut c_void;
    }
    for i in 0..menu.numberOfItems() {
        if let Some(inner) = menu.itemAtIndex(i).and_then(|item| item.submenu()) {
            if let Some(item) = inner.itemWithTag(tag) {
                return Retained::as_ptr(&item) as *mut c_void;
            }
        }
    }
    std::ptr::null_mut()
}

/// R27: 查找并设置菜单项标题; 找不到返回 0。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_menu_set_item_title(
    menu: *mut c_void,
    tag: isize,
    title: *const std::os::raw::c_char,
) -> i32 {
    use objc2_app_kit::NSMenuItem;
    use objc2_foundation::NSString;
    if menu.is_null() || title.is_null() {
        return 0;
    }
    let p = glaspen2_rust_menu_item_with_tag(menu, tag);
    if p.is_null() {
        return 0;
    }
    let item = unsafe { &*(p as *const NSMenuItem) };
    let title = unsafe { std::ffi::CStr::from_ptr(title) }.to_string_lossy();
    item.setTitle(&NSString::from_str(&title));
    1
}

/// R27: 查找并读取菜单项 state(未找到返回 0, 与旧实现默认一致)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_menu_item_state(menu: *mut c_void, tag: isize) -> i32 {
    use objc2_app_kit::NSMenuItem;
    let p = glaspen2_rust_menu_item_with_tag(menu, tag);
    if p.is_null() {
        return 0;
    }
    let item = unsafe { &*(p as *const NSMenuItem) };
    item.state() as i32
}

// ── R28: 光标壳(空白/箭头 NSCursor + 光标环绘制) ──────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_blank_cursor_create() -> *mut c_void {
    use objc2_app_kit::{NSBezierPath, NSColor, NSCursor, NSImage};
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    let mtm = MainThreadMarker::new();
    let Some(mtm) = mtm else {
        return std::ptr::null_mut();
    };
    let image = NSImage::initWithSize(NSImage::alloc(), NSSize::new(1.0, 1.0));
    image.lockFocus();
    NSColor::clearColor().setFill();
    NSBezierPath::bezierPathWithRect(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1.0, 1.0)))
        .fill();
    image.unlockFocus();
    let cursor = NSCursor::initWithImage_hotSpot(NSCursor::alloc(), &image, NSPoint::new(0.0, 0.0));
    let _ = mtm;
    Retained::into_raw(cursor) as *mut c_void
}

/// R28: 系统箭头光标(旧 makeArrowCursor)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_arrow_cursor_create() -> *mut c_void {
    use objc2_app_kit::NSCursor;
    Retained::into_raw(NSCursor::arrowCursor()) as *mut c_void
}

/// R28: 光标环/点/十字线绘制(旧 OverlayCursorRenderer.drawCursor)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_draw_cursor(
    ctx: *mut c_void,
    visible: i32,
    x: f64,
    y: f64,
    eraser: i32,
) {
    use objc2_app_kit::NSColor;
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use objc2_core_graphics::CGContext;
    if visible == 0 || ctx.is_null() {
        return;
    }
    let ctx = unsafe { &*(ctx as *const CGContext) };
    let radius = 8.0;
    let eraser = eraser != 0;
    let ring = if eraser {
        NSColor::colorWithCalibratedRed_green_blue_alpha(1.0, 0.25, 0.2, 0.95)
    } else {
        NSColor::colorWithWhite_alpha(1.0, 0.8)
    };
    let dot = if eraser {
        NSColor::colorWithCalibratedRed_green_blue_alpha(1.0, 0.25, 0.2, 0.95)
    } else {
        NSColor::colorWithWhite_alpha(1.0, 0.9)
    };
    CGContext::set_stroke_color_with_color(Some(ctx), Some(&ring.CGColor()));
    CGContext::set_line_width(Some(ctx), 1.5);
    CGContext::stroke_ellipse_in_rect(
        Some(ctx),
        CGRect::new(
            CGPoint::new(x - radius, y - radius),
            CGSize::new(radius * 2.0, radius * 2.0),
        ),
    );
    CGContext::set_fill_color_with_color(Some(ctx), Some(&dot.CGColor()));
    CGContext::fill_ellipse_in_rect(
        Some(ctx),
        CGRect::new(CGPoint::new(x - 1.5, y - 1.5), CGSize::new(3.0, 3.0)),
    );
    let gap = 3.0;
    CGContext::set_stroke_color_with_color(
        Some(ctx),
        Some(&NSColor::colorWithWhite_alpha(0.0, 0.5).CGColor()),
    );
    CGContext::set_line_width(Some(ctx), 1.0);
    CGContext::move_to_point(Some(ctx), x, y - radius - 2.0);
    CGContext::add_line_to_point(Some(ctx), x, y - gap);
    CGContext::move_to_point(Some(ctx), x, y + gap);
    CGContext::add_line_to_point(Some(ctx), x, y + radius + 2.0);
    CGContext::move_to_point(Some(ctx), x - radius - 2.0, y);
    CGContext::add_line_to_point(Some(ctx), x - gap, y);
    CGContext::move_to_point(Some(ctx), x + gap, y);
    CGContext::add_line_to_point(Some(ctx), x + radius + 2.0, y);
    CGContext::stroke_path(Some(ctx));
}

/// R29: 创建设置面板窗口(标题/尺寸/样式/内容 VC/激活/居中); delegate 仍由 Swift 设置。
/// 返回 +1 持有 NSWindow; GLASPEN2_SWIFT_SETTINGS_PANEL=1 时 Swift 回退。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_settings_window_create(
    title: *const std::os::raw::c_char,
    view_controller: *mut c_void,
    width: f64,
    height: f64,
) -> *mut c_void {
    use objc2_app_kit::{
        NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSViewController,
        NSWindow, NSWindowStyleMask,
    };
    use objc2_foundation::{NSPoint, NSRect, NSSize, NSString, ns_string};

    if title.is_null() || view_controller.is_null() {
        return std::ptr::null_mut();
    }
    let Some(mtm) = MainThreadMarker::new() else {
        return std::ptr::null_mut();
    };
    let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height));
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            frame,
            NSWindowStyleMask::Titled | NSWindowStyleMask::Closable | NSWindowStyleMask::Resizable,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    let title = unsafe { std::ffi::CStr::from_ptr(title) }.to_string_lossy();
    window.setTitle(&NSString::from_str(&title));
    window.setMinSize(NSSize::new(600.0, 800.0));
    unsafe { window.setReleasedWhenClosed(false) };
    let vc = unsafe { &*(view_controller as *const NSViewController) };
    window.setContentViewController(Some(vc));
    install_settings_delegate(&window, mtm);
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);
    window.center();
    let _ = ns_string!("");
    tracing::info!("rust-shell: settings window created by Rust (objc2)");
    Retained::into_raw(window) as *mut c_void
}

// ── R30: 笔迹 surface 合成(小脏矩形裁子图; Swift 仍负责 CGImage 缓存) ──

unsafe extern "C" {
    fn CGImageCreateWithImageInRect(
        image: *const c_void,
        rect: objc2_core_foundation::CGRect,
    ) -> *mut c_void;
    fn CGImageRelease(image: *mut c_void);
}

/// R30: 把笔迹 surface 的 CGImage 绘到当前 overlay context。
/// 返回 1 成功, 0 失败(Swift 回退)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_draw_surface(
    ctx: *mut c_void,
    image: *mut c_void,
    rx: f64,
    ry: f64,
    rw: f64,
    rh: f64,
    bx: f64,
    by: f64,
    bw: f64,
    bh: f64,
    scale: f64,
) -> i32 {
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use objc2_core_graphics::{CGContext, CGImage};
    if ctx.is_null() || image.is_null() || bw <= 0.0 || bh <= 0.0 {
        return 0;
    }
    let ctx = unsafe { &*(ctx as *const CGContext) };
    let image = unsafe { &*(image as *const CGImage) };
    let same_rect = (rx - bx).abs() < 0.5
        && (ry - by).abs() < 0.5
        && (rw - bw).abs() < 0.5
        && (rh - bh).abs() < 0.5;
    let small_dirty = !same_rect && (rw < bw || rh < bh);
    let full = CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(bw, bh));
    if small_dirty {
        let sx = rx * scale;
        let sy = (bh - (ry + rh)) * scale;
        let phys = CGRect::new(CGPoint::new(sx, sy), CGSize::new(rw * scale, rh * scale));
        let sub = unsafe { CGImageCreateWithImageInRect(image as *const _ as *const c_void, phys) };
        if !sub.is_null() {
            let sub = unsafe { &*(sub as *const CGImage) };
            let rect = CGRect::new(CGPoint::new(rx, ry), CGSize::new(rw, rh));
            CGContext::draw_image(Some(ctx), rect, Some(sub));
            unsafe { CGImageRelease(sub as *const _ as *mut c_void) };
        } else {
            CGContext::draw_image(Some(ctx), full, Some(image));
        }
    } else {
        CGContext::draw_image(Some(ctx), full, Some(image));
    }
    1
}

// ── R31: 网格三段线绘制(core::grid 直接计算, CGContext 描边) ──

fn stroke_grid_layer(
    ctx: &objc2_core_graphics::CGContext,
    segments: &[glaspen_core::grid::Segment],
    view_h: f64,
    white: f64,
    alpha: f64,
    line_width: f64,
) {
    use objc2_app_kit::NSColor;
    use objc2_core_graphics::CGContext;
    if segments.is_empty() {
        return;
    }
    let color = NSColor::colorWithWhite_alpha(white, alpha);
    CGContext::set_stroke_color_with_color(Some(ctx), Some(&color.CGColor()));
    CGContext::set_line_width(Some(ctx), line_width);
    for seg in segments {
        CGContext::move_to_point(Some(ctx), seg.x0, view_h - seg.y0);
        CGContext::add_line_to_point(Some(ctx), seg.x1, view_h - seg.y1);
    }
    CGContext::stroke_path(Some(ctx));
}

/// R31: 绘制网格(细网格/屏幕分界/分栏线); 返回 1 成功, 0 失败(Swift 回退)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_draw_grid(
    ctx: *mut c_void,
    view_w: f64,
    view_h: f64,
    pan_x: f64,
    pan_y: f64,
    zoom: f64,
    grid_size: f64,
    divider: i32,
    show_grid: i32,
    screen_w: f64,
    screen_h: f64,
    clip_x0: f64,
    clip_y0: f64,
    clip_x1: f64,
    clip_y1: f64,
) -> i32 {
    use glaspen_core::camera::Camera;
    use glaspen_core::grid::{self, GridParams, ViewRect};
    use objc2_core_graphics::CGContext;
    if ctx.is_null() {
        return 0;
    }
    let ctx = unsafe { &*(ctx as *const CGContext) };
    let params = GridParams {
        view_w,
        view_h,
        camera: Camera::new(pan_x, pan_y, zoom),
        grid_size,
        divider,
        show_grid: show_grid != 0,
        screen_w,
        screen_h,
        screen_lines: true,
        clip: Some(ViewRect::new(clip_x0, clip_y0, clip_x1, clip_y1)),
    };
    let Ok(lines) = grid::compute_checked(&params) else {
        return 0;
    };
    stroke_grid_layer(ctx, &lines.minor, view_h, 0.5, 0.15, 0.5);
    stroke_grid_layer(ctx, &lines.screen, view_h, 0.5, 0.55, 1.0);
    stroke_grid_layer(ctx, &lines.divider, view_h, 0.5, 0.65, 1.5);
    1
}

// ── R32: 时光隧道 CGImage 缓存 + 合成(替代 Swift OverlayTunnelRenderer) ──

#[derive(Default)]
struct TunnelCache {
    data: *mut c_void,
    width: i32,
    height: i32,
    stride: i32,
    image: *mut c_void,
}

unsafe impl Send for TunnelCache {}

static TUNNEL_CACHE: std::sync::OnceLock<std::sync::Mutex<TunnelCache>> =
    std::sync::OnceLock::new();

fn tunnel_cache() -> &'static std::sync::Mutex<TunnelCache> {
    TUNNEL_CACHE.get_or_init(|| std::sync::Mutex::new(TunnelCache::default()))
}

unsafe extern "C" {
    fn CGDataProviderCreateWithData(
        info: *mut c_void,
        data: *const c_void,
        size: usize,
        release_data: *const c_void,
    ) -> *mut c_void;
    fn CGColorSpaceCreateDeviceRGB() -> *mut c_void;
    fn CGColorSpaceRelease(space: *mut c_void);
    fn CGImageCreate(
        width: usize,
        height: usize,
        bits_per_component: usize,
        bits_per_pixel: usize,
        bytes_per_row: usize,
        space: *mut c_void,
        bitmap_info: u32,
        provider: *mut c_void,
        decode: *const f64,
        should_interpolate: bool,
        intent: i32,
    ) -> *mut c_void;
    fn CFRelease(cf: *const c_void);
}

fn tunnel_image(data: *mut c_void, width: i32, height: i32, stride: i32) -> *mut c_void {
    let mut cache = tunnel_cache().lock().unwrap_or_else(|e| e.into_inner());
    if cache.image.is_null()
        || cache.data != data
        || cache.width != width
        || cache.height != height
        || cache.stride != stride
    {
        if !cache.image.is_null() {
            unsafe { CGImageRelease(cache.image) };
        }
        let size = height.max(0) as usize * stride.max(0) as usize;
        let provider = unsafe {
            CGDataProviderCreateWithData(
                std::ptr::null_mut(),
                data as *const c_void,
                size,
                std::ptr::null(),
            )
        };
        let space = unsafe { CGColorSpaceCreateDeviceRGB() };
        // premultipliedFirst(2) | byteOrder32Little(2 << 12)
        let image = if provider.is_null() || space.is_null() {
            std::ptr::null_mut()
        } else {
            unsafe {
                CGImageCreate(
                    width.max(0) as usize,
                    height.max(0) as usize,
                    8,
                    32,
                    stride.max(0) as usize,
                    space,
                    2 | (2 << 12),
                    provider,
                    std::ptr::null(),
                    false,
                    0,
                )
            }
        };
        unsafe {
            if !provider.is_null() {
                CFRelease(provider)
            }
            if !space.is_null() {
                CGColorSpaceRelease(space)
            }
        }
        cache.data = data;
        cache.width = width;
        cache.height = height;
        cache.stride = stride;
        cache.image = image;
    }
    cache.image
}

/// R32: 时光隧道 surface → CGImage 缓存并绘制; 返回 1 成功, 0 失败(Swift 回退)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_draw_tunnel(
    ctx: *mut c_void,
    data: *mut c_void,
    width: i32,
    height: i32,
    stride: i32,
    bw: f64,
    bh: f64,
) -> i32 {
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use objc2_core_graphics::{CGContext, CGImage};
    if ctx.is_null() || data.is_null() || width <= 0 || height <= 0 || stride <= 0 {
        return 0;
    }
    let image = tunnel_image(data, width, height, stride);
    if image.is_null() {
        return 0;
    }
    let ctx = unsafe { &*(ctx as *const CGContext) };
    let image = unsafe { &*(image as *const CGImage) };
    CGContext::draw_image(
        Some(ctx),
        CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(bw, bh)),
        Some(image),
    );
    1
}

/// R32: 清除隧道 CGImage 缓存。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_clear_tunnel_cache() {
    let mut cache = tunnel_cache().lock().unwrap_or_else(|e| e.into_inner());
    if !cache.image.is_null() {
        unsafe { CGImageRelease(cache.image) };
    }
    *cache = TunnelCache::default();
}

/// R33: 通知文字绘制(36pt 等宽 + 白字 + 阴影 + 视图内居中)。
/// 返回 1 成功, 0 失败(Swift 回退)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_draw_notification(
    text: *const std::os::raw::c_char,
    view_w: f64,
    view_h: f64,
) -> i32 {
    use objc2::runtime::{AnyObject, ProtocolObject};
    use objc2_app_kit::{
        NSFont, NSFontAttributeName, NSFontWeightMedium, NSForegroundColorAttributeName, NSShadow,
        NSShadowAttributeName, NSStringDrawing,
    };
    use objc2_core_foundation::{CGPoint, CGSize};
    use objc2_foundation::{NSAttributedStringKey, NSMutableDictionary, NSString};

    if text.is_null() || view_w <= 0.0 || view_h <= 0.0 {
        return 0;
    }
    let text = unsafe { std::ffi::CStr::from_ptr(text) }.to_string_lossy();
    if text.is_empty() {
        return 0;
    }
    let shadow = NSShadow::new();
    shadow.setShadowColor(Some(&objc2_app_kit::NSColor::colorWithWhite_alpha(
        0.0, 0.8,
    )));
    shadow.setShadowOffset(objc2_foundation::NSSize::new(2.0, -2.0));
    shadow.setShadowBlurRadius(4.0);
    let font = NSFont::monospacedSystemFontOfSize_weight(36.0, unsafe { NSFontWeightMedium });
    let attrs: objc2::rc::Retained<NSMutableDictionary<NSAttributedStringKey, AnyObject>> =
        NSMutableDictionary::new();
    unsafe {
        let font_any: &AnyObject = &*(&*font as *const NSFont as *const AnyObject);
        let shadow_any: &AnyObject = &*(&*shadow as *const NSShadow as *const AnyObject);
        let white = objc2_app_kit::NSColor::whiteColor();
        let white_any: &AnyObject =
            &*(&*white as *const objc2_app_kit::NSColor as *const AnyObject);
        attrs.setObject_forKey(font_any, ProtocolObject::from_ref(&*NSFontAttributeName));
        attrs.setObject_forKey(
            white_any,
            ProtocolObject::from_ref(&*NSForegroundColorAttributeName),
        );
        attrs.setObject_forKey(
            shadow_any,
            ProtocolObject::from_ref(&*NSShadowAttributeName),
        );
    }
    let ns = NSString::from_str(&text);
    let size: CGSize = unsafe { ns.sizeWithAttributes(Some(&attrs)) };
    let x = (view_w - size.width) / 2.0;
    let y = (view_h - size.height) / 2.0;
    unsafe { ns.drawAtPoint_withAttributes(CGPoint::new(x, y), Some(&attrs)) };
    1
}

// ── R34: 笔迹 surface CGImage 缓存(替代 Swift OverlaySurfaceImageCache) ──

#[derive(Default)]
struct SurfaceCache {
    data: *mut c_void,
    width: i32,
    height: i32,
    stride: i32,
    image: *mut c_void,
}

unsafe impl Send for SurfaceCache {}

static SURFACE_CACHE: std::sync::OnceLock<std::sync::Mutex<SurfaceCache>> =
    std::sync::OnceLock::new();

fn surface_cache() -> &'static std::sync::Mutex<SurfaceCache> {
    SURFACE_CACHE.get_or_init(|| std::sync::Mutex::new(SurfaceCache::default()))
}

fn surface_image(data: *mut c_void, width: i32, height: i32, stride: i32) -> *mut c_void {
    let mut cache = surface_cache().lock().unwrap_or_else(|e| e.into_inner());
    if cache.image.is_null()
        || cache.data != data
        || cache.width != width
        || cache.height != height
        || cache.stride != stride
    {
        if !cache.image.is_null() {
            unsafe { CGImageRelease(cache.image) };
        }
        let size = height.max(0) as usize * stride.max(0) as usize;
        let provider = unsafe {
            CGDataProviderCreateWithData(
                std::ptr::null_mut(),
                data as *const c_void,
                size,
                std::ptr::null(),
            )
        };
        let space = unsafe { CGColorSpaceCreateDeviceRGB() };
        let image = if provider.is_null() || space.is_null() {
            std::ptr::null_mut()
        } else {
            unsafe {
                CGImageCreate(
                    width.max(0) as usize,
                    height.max(0) as usize,
                    8,
                    32,
                    stride.max(0) as usize,
                    space,
                    2 | (2 << 12),
                    provider,
                    std::ptr::null(),
                    false,
                    0,
                )
            }
        };
        unsafe {
            if !provider.is_null() {
                CFRelease(provider)
            }
            if !space.is_null() {
                CGColorSpaceRelease(space)
            }
        }
        cache.data = data;
        cache.width = width;
        cache.height = height;
        cache.stride = stride;
        cache.image = image;
    }
    cache.image
}

/// R34: 由 cairo surface 指针直接缓存 CGImage 并绘制; 返回 1 成功, 0 失败。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_draw_surface_data(
    ctx: *mut c_void,
    data: *mut c_void,
    width: i32,
    height: i32,
    stride: i32,
    rx: f64,
    ry: f64,
    rw: f64,
    rh: f64,
    bx: f64,
    by: f64,
    bw: f64,
    bh: f64,
    scale: f64,
) -> i32 {
    if data.is_null() || width <= 0 || height <= 0 || stride <= 0 {
        return 0;
    }
    let image = surface_image(data, width, height, stride);
    if image.is_null() {
        return 0;
    }
    glaspen2_rust_draw_surface(ctx, image, rx, ry, rw, rh, bx, by, bw, bh, scale)
}

// ── R35: 设置窗口 delegate(窗口关闭后回到 Accessory + deactivate) ──

#[derive(Default)]
struct RustSettingsDelegateIvars {}

define_class!(
    #[unsafe(super = objc2_foundation::NSObject)]
    #[thread_kind = MainThreadOnly]
    #[name = "GlaspenRustSettingsDelegate"]
    #[ivars = RustSettingsDelegateIvars]
    struct RustSettingsDelegate;

    unsafe impl NSObjectProtocol for RustSettingsDelegate {}

    unsafe impl NSWindowDelegate for RustSettingsDelegate {
        #[unsafe(method(windowWillClose:))]
        fn window_will_close(&self, _notification: &objc2_foundation::NSNotification) {
            use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
            if let Some(mtm) = MainThreadMarker::new() {
                let app = NSApplication::sharedApplication(mtm);
                app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
                app.deactivate();
            }
            tracing::info!("rust-shell: settings window will close");
        }
    }
);

impl RustSettingsDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(RustSettingsDelegateIvars::default());
        unsafe { msg_send![super(this), init] }
    }
}

thread_local! {
    static SETTINGS_DELEGATE: std::cell::RefCell<Option<Retained<RustSettingsDelegate>>> =
        const { std::cell::RefCell::new(None) };
}

fn install_settings_delegate(window: &objc2_app_kit::NSWindow, mtm: MainThreadMarker) {
    use objc2::runtime::ProtocolObject;
    let delegate = RustSettingsDelegate::new(mtm);
    window.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    SETTINGS_DELEGATE.with(|d| *d.borrow_mut() = Some(delegate));
    tracing::info!("rust-shell: settings delegate installed by Rust (objc2)");
}

// ── R36: 颜色/粗细选择状态机(书写中暂存, 否则应用+刷新) ──

unsafe extern "C" {
    fn glaspen2_router_stroke_active() -> i32;
    fn glaspen2_router_set_pending_color(idx: i32);
    fn glaspen2_router_selected_color() -> i32;
    fn glaspen2_router_set_prev_color(idx: i32);
    fn glaspen2_router_apply_color(idx: i32);
    fn glaspen2_router_save_pen_settings();
    fn glaspen2_router_update_status_icon_state();
    fn glaspen2_router_sync_settings_panel();
    fn glaspen2_router_set_pending_width(idx: i32);
    fn glaspen2_router_selected_width() -> i32;
    fn glaspen2_router_set_prev_width(idx: i32);
    fn glaspen2_router_apply_width(idx: i32);
    fn glaspen2_router_pending_color() -> i32;
    fn glaspen2_router_clear_pending_color();
    fn glaspen2_router_pending_width() -> i32;
    fn glaspen2_router_clear_pending_width();
}

fn refresh_after_pen_change() {
    unsafe {
        glaspen2_router_save_pen_settings();
        glaspen2_router_update_checkmarks();
        glaspen2_router_update_status_icon_state();
        glaspen2_router_sync_settings_panel();
    }
}

/// R36: 选择颜色; 返回 1 已处理, 0 非法/失败(Swift 回退)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_select_color(index: i32) -> i32 {
    if index < 0 || index as usize >= glaspen_core::presets::COLOR_PRESETS.len() {
        return 0;
    }
    unsafe {
        if glaspen2_router_stroke_active() != 0 {
            glaspen2_router_set_pending_color(index);
            return 1;
        }
        let selected = glaspen2_router_selected_color();
        if index != selected {
            glaspen2_router_set_prev_color(selected);
        }
        glaspen2_router_apply_color(index);
    }
    refresh_after_pen_change();
    1
}

/// R36: 选择粗细; 返回 1 已处理, 0 非法/失败(Swift 回退)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_select_width(index: i32) -> i32 {
    if index < 0 || index as usize >= glaspen_core::presets::WIDTH_PRESETS.len() {
        return 0;
    }
    unsafe {
        if glaspen2_router_stroke_active() != 0 {
            glaspen2_router_set_pending_width(index);
            return 1;
        }
        let selected = glaspen2_router_selected_width();
        if index != selected {
            glaspen2_router_set_prev_width(selected);
        }
        glaspen2_router_apply_width(index);
    }
    refresh_after_pen_change();
    1
}

/// R36: 抬笔后应用暂存颜色/粗细。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_apply_pending_color_width() {
    unsafe {
        if glaspen2_router_stroke_active() != 0 {
            return;
        }
        let color = glaspen2_router_pending_color();
        if color >= 0 {
            glaspen2_router_clear_pending_color();
            let _ = glaspen2_rust_select_color(color);
        }
        let width = glaspen2_router_pending_width();
        if width >= 0 {
            glaspen2_router_clear_pending_width();
            let _ = glaspen2_rust_select_width(width);
        }
    }
}

// ── R37: 功能开关 setter(聊天/压力/网格/自启/玻璃) ──

unsafe extern "C" {
    fn glaspen2_router_set_chat_integration(on: i32);
    fn glaspen2_save_bool_setting(key: *const std::os::raw::c_char, value: i32);
    fn glaspen2_router_rec_chat_integration_changed(on: i32);
    fn glaspen2_share_ink_set_active(on: i32);
    fn glaspen2_load_bool_setting(key: *const std::os::raw::c_char) -> i32;
    fn glaspen2_router_set_pressure_monitor(on: i32);
    fn glaspen2_router_pm_show();
    fn glaspen2_router_pm_refresh();
    fn glaspen2_router_pm_hide();
    fn glaspen2_router_set_grid_state(on: i32);
    fn glaspen2_set_launch_at_login(on: i32);
    fn glaspen2_router_set_glass_state(on: i32);
}

fn bool_setting(key: &str, value: bool) {
    let key = std::ffi::CString::new(key).unwrap();
    unsafe { glaspen2_save_bool_setting(key.as_ptr(), i32::from(value)) };
}

fn load_bool(key: &str) -> bool {
    let key = std::ffi::CString::new(key).unwrap();
    unsafe { glaspen2_load_bool_setting(key.as_ptr()) != 0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_set_chat_integration(enabled: i32) {
    let on = enabled != 0;
    unsafe {
        glaspen2_router_set_chat_integration(i32::from(on));
        bool_setting("chat_integration", on);
        glaspen2_router_rec_chat_integration_changed(i32::from(on));
        if !on {
            glaspen2_share_ink_set_active(0);
        } else if load_bool("share_ink") {
            glaspen2_share_ink_set_active(1);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_set_pressure_monitor(enabled: i32) {
    let on = enabled != 0;
    unsafe {
        glaspen2_router_set_pressure_monitor(i32::from(on));
        bool_setting("pressure_monitor", on);
        if on {
            glaspen2_router_pm_show();
            glaspen2_router_pm_refresh();
        } else {
            glaspen2_router_pm_hide();
        }
        glaspen2_router_sync_settings_panel();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_set_grid_enabled(enabled: i32) {
    let on = enabled != 0;
    unsafe {
        glaspen2_router_set_grid_state(i32::from(on));
        bool_setting("grid", on);
        glaspen2_router_sync_settings_panel();
        glaspen2_router_save_pen_settings();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_set_launch_at_login(enabled: i32) {
    let on = enabled != 0;
    unsafe {
        glaspen2_set_launch_at_login(i32::from(on));
        glaspen2_router_update_menu_state(777, i32::from(on));
        glaspen2_router_sync_settings_panel();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_set_glass_enabled(enabled: i32) {
    let on = enabled != 0;
    unsafe {
        glaspen2_router_set_glass_state(i32::from(on));
        bool_setting("glass_enabled", on);
        glaspen2_router_glass_apply();
    }
}

// ── R38: 覆盖层绘制编排(opt-in: GLASPEN2_RUST_OVERLAY_DRAW=1) ──

unsafe extern "C" {
    fn glaspen2_swift_overlay_prepare() -> i32;
    fn glaspen2_swift_overlay_tunnel(ctx: *mut c_void, bx: f64, by: f64, bw: f64, bh: f64) -> i32;
    fn glaspen2_swift_overlay_grid(
        ctx: *mut c_void,
        bx: f64,
        by: f64,
        bw: f64,
        bh: f64,
        rx: f64,
        ry: f64,
        rw: f64,
        rh: f64,
    );
    fn glaspen2_swift_overlay_surface(
        ctx: *mut c_void,
        bx: f64,
        by: f64,
        bw: f64,
        bh: f64,
        rx: f64,
        ry: f64,
        rw: f64,
        rh: f64,
    );
    fn glaspen2_swift_overlay_minimap(ctx: *mut c_void, bx: f64, by: f64, bw: f64, bh: f64);
    fn glaspen2_swift_overlay_notification(bx: f64, by: f64, bw: f64, bh: f64);
    fn glaspen2_swift_overlay_cursor(ctx: *mut c_void);
    fn glaspen2_swift_overlay_perf(
        t0: u64,
        bx: f64,
        by: f64,
        bw: f64,
        bh: f64,
        rx: f64,
        ry: f64,
        rw: f64,
        rh: f64,
    );
}

unsafe extern "C" {
    fn mach_absolute_time() -> u64;
}

/// R38: 覆盖层绘制编排; 返回 1 成功, 0 失败(Swift 回退)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_overlay_draw(
    view: *mut c_void,
    rx: f64,
    ry: f64,
    rw: f64,
    rh: f64,
) -> i32 {
    use objc2_app_kit::{NSGraphicsContext, NSView};
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use objc2_core_graphics::CGContext;

    if view.is_null() {
        return 0;
    }
    let Some(_mtm) = MainThreadMarker::new() else {
        return 0;
    };
    let view = unsafe { &*(view as *const NSView) };
    let Some(gctx) = NSGraphicsContext::currentContext() else {
        return 0;
    };
    let cg = gctx.CGContext();
    let ctx_ptr = Retained::as_ptr(&cg) as *mut c_void;
    let bounds = view.bounds();
    let (bx, by, bw, bh) = (
        bounds.origin.x,
        bounds.origin.y,
        bounds.size.width,
        bounds.size.height,
    );
    let t0 = unsafe { mach_absolute_time() };

    CGContext::save_g_state(Some(&cg));
    CGContext::clip_to_rect(
        Some(&cg),
        CGRect::new(CGPoint::new(rx, ry), CGSize::new(rw, rh)),
    );
    if unsafe { glaspen2_swift_overlay_prepare() } == 0 {
        CGContext::restore_g_state(Some(&cg));
        return 1;
    }
    if unsafe { glaspen2_swift_overlay_tunnel(ctx_ptr, bx, by, bw, bh) } != 0 {
        CGContext::restore_g_state(Some(&cg));
        return 1;
    }
    unsafe {
        glaspen2_swift_overlay_grid(ctx_ptr, bx, by, bw, bh, rx, ry, rw, rh);
        glaspen2_swift_overlay_surface(ctx_ptr, bx, by, bw, bh, rx, ry, rw, rh);
        glaspen2_swift_overlay_minimap(ctx_ptr, bx, by, bw, bh);
        glaspen2_swift_overlay_notification(bx, by, bw, bh);
        glaspen2_swift_overlay_cursor(ctx_ptr);
    }
    CGContext::restore_g_state(Some(&cg));
    unsafe { glaspen2_swift_overlay_perf(t0, bx, by, bw, bh, rx, ry, rw, rh) };
    1
}

// ── R40: Minimap strip 合成/绘制(缩略图生成仍 Swift) ──

#[repr(C)]
pub struct MinimapItemC {
    pub screen_id: i64,
    pub current: i32,
    pub y: f64,
    pub h: f64,
    pub label: *const std::os::raw::c_char,
    pub thumb: *mut c_void,
}

/// R40: 用 Swift 提供的缩略图/布局合成 minimap strip 并绘制; 返回 1 成功, 0 失败。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_draw_minimap(
    ctx: *mut c_void,
    items: *const MinimapItemC,
    count: i32,
    strip_w: f64,
    strip_h: f64,
    strip_x: f64,
    strip_y: f64,
    view_h: f64,
) -> i32 {
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{
        NSBezierPath, NSColor, NSCompositingOperation, NSFont, NSFontAttributeName,
        NSFontWeightRegular, NSForegroundColorAttributeName, NSImage, NSStringDrawing,
    };
    use objc2_foundation::{
        NSAttributedStringKey, NSMutableDictionary, NSPoint, NSRect, NSSize, NSString,
    };

    if ctx.is_null() || items.is_null() || strip_w <= 0.0 || strip_h <= 0.0 || count <= 0 {
        return 0;
    }
    let Some(mtm) = MainThreadMarker::new() else {
        return 0;
    };
    let ctx = unsafe { &*(ctx as *const objc2_core_graphics::CGContext) };
    let strip = NSImage::initWithSize(NSImage::alloc(), NSSize::new(strip_w, strip_h));
    strip.lockFocus();
    for i in 0..count {
        let item = unsafe { &*items.add(i as usize) };
        let ly = strip_h - ((item.y - strip_y) + item.h);
        let rect = NSRect::new(NSPoint::new(0.0, ly), NSSize::new(strip_w, item.h));
        if !item.thumb.is_null() {
            let thumb = unsafe { &*(item.thumb as *const NSImage) };
            thumb.drawInRect_fromRect_operation_fraction(
                rect,
                NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
                NSCompositingOperation::SourceOver,
                1.0,
            );
        } else {
            NSColor::colorWithCalibratedWhite_alpha(0.94, 0.9).setFill();
            NSBezierPath::bezierPathWithRect(rect).fill();
        }
        let border = NSBezierPath::bezierPathWithRect(rect);
        border.setLineWidth(if item.current != 0 { 2.0 } else { 1.0 });
        if item.current != 0 {
            NSColor::systemRedColor().setStroke();
        } else {
            NSColor::systemGrayColor().setStroke();
        }
        border.stroke();
        if !item.label.is_null() {
            let label = unsafe { std::ffi::CStr::from_ptr(item.label) }.to_string_lossy();
            if !label.is_empty() {
                let font =
                    NSFont::monospacedSystemFontOfSize_weight(9.0, unsafe { NSFontWeightRegular });
                let attrs: objc2::rc::Retained<
                    NSMutableDictionary<NSAttributedStringKey, AnyObject>,
                > = NSMutableDictionary::new();
                unsafe {
                    let font_any: &AnyObject = &*(&*font as *const NSFont as *const AnyObject);
                    let color = NSColor::colorWithCalibratedWhite_alpha(0.25, 0.9);
                    let color_any: &AnyObject = &*(&*color as *const NSColor as *const AnyObject);
                    attrs.setObject_forKey(
                        font_any,
                        objc2::runtime::ProtocolObject::from_ref(&*NSFontAttributeName),
                    );
                    attrs.setObject_forKey(
                        color_any,
                        objc2::runtime::ProtocolObject::from_ref(&*NSForegroundColorAttributeName),
                    );
                }
                let ns = NSString::from_str(&label);
                unsafe {
                    ns.drawAtPoint_withAttributes(
                        objc2_core_foundation::CGPoint::new(
                            rect.origin.x + 3.0,
                            rect.origin.y + 3.0,
                        ),
                        Some(&attrs),
                    );
                }
            }
        }
    }
    strip.unlockFocus();
    let dst = NSRect::new(
        NSPoint::new(strip_x, view_h - strip_y - strip_h),
        NSSize::new(strip_w, strip_h),
    );
    strip.drawInRect_fromRect_operation_fraction(
        dst,
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
        NSCompositingOperation::SourceOver,
        1.0,
    );
    let _ = mtm;
    let _ = ctx;
    1
}

// ── R41: 脏矩形跟踪(视图点并集) ──

#[derive(Clone, Copy)]
struct DirtyRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

static DIRTY: std::sync::OnceLock<std::sync::Mutex<Option<DirtyRect>>> = std::sync::OnceLock::new();

fn dirty_state() -> &'static std::sync::Mutex<Option<DirtyRect>> {
    DIRTY.get_or_init(|| std::sync::Mutex::new(None))
}

fn dirty_include_point(x: f64, y: f64, r: f64) {
    let mut state = dirty_state().lock().unwrap_or_else(|e| e.into_inner());
    let (nx0, ny0, nx1, ny1) = (x - r, y - r, x + r, y + r);
    *state = Some(match *state {
        None => DirtyRect {
            x: nx0,
            y: ny0,
            w: nx1 - nx0,
            h: ny1 - ny0,
        },
        Some(rect) => {
            let x0 = rect.x.min(nx0);
            let y0 = rect.y.min(ny0);
            let x1 = (rect.x + rect.w).max(nx1);
            let y1 = (rect.y + rect.h).max(ny1);
            DirtyRect {
                x: x0,
                y: y0,
                w: x1 - x0,
                h: y1 - y0,
            }
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_dirty_reset() {
    *dirty_state().lock().unwrap_or_else(|e| e.into_inner()) = None;
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_dirty_include_point(x: f64, y: f64, r: f64) {
    dirty_include_point(x, y, r);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_dirty_include_surface_point(x: f64, y: f64, r: f64) {
    let view_h = glaspen_core::shell_state::screen_h() as f64;
    dirty_include_point(x, view_h - y, r);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_dirty_include_surface_rect(x: f64, y: f64, w: f64, h: f64) {
    let view_h = glaspen_core::shell_state::screen_h() as f64;
    dirty_include_point(x, view_h - (y + h), 0.0);
    dirty_include_point(x + w, view_h - y, 0.0);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_dirty_has() -> i32 {
    i32::from(
        dirty_state()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some(),
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_dirty_rect(out: *mut f64) -> i32 {
    if out.is_null() {
        return 0;
    }
    let state = *dirty_state().lock().unwrap_or_else(|e| e.into_inner());
    match state {
        Some(rect) => {
            unsafe {
                *out.add(0) = rect.x;
                *out.add(1) = rect.y;
                *out.add(2) = rect.w;
                *out.add(3) = rect.h;
            }
            1
        }
        None => 0,
    }
}

// ── R42: 磨砂玻璃应用(组合 enabled/opacity/飘渺联动) ──

unsafe extern "C" {
    fn glaspen2_router_glass_enabled() -> i32;
    fn glaspen2_router_glass_opacity() -> f64;
    fn glaspen2_router_glass_follow() -> i32;
    fn glaspen2_router_ethereal() -> i32;
    fn glaspen2_router_strokes_visible() -> i32;
    fn glaspen2_router_apply_glass_view(alpha: f64, hidden: i32);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_apply_glass() {
    unsafe {
        let enabled = glaspen2_router_glass_enabled() != 0;
        let opacity = glaspen2_router_glass_opacity();
        let follow = glaspen2_router_glass_follow() != 0;
        let ethereal = glaspen2_router_ethereal() != 0;
        let strokes_visible = glaspen2_router_strokes_visible() != 0;
        let hidden = ethereal && follow && !strokes_visible;
        glaspen2_router_apply_glass_view(
            if enabled { opacity * 2.0 } else { 0.0 },
            i32::from(!enabled || hidden),
        );
        glaspen2_router_update_menu_state(444, i32::from(enabled));
    }
}

// ── R43: 涂鸦总开关/描边开关(文案由 Swift 传入, 状态/菜单/通知动作在 Rust) ──

unsafe extern "C" {
    fn glaspen2_router_enabled() -> i32;
    fn glaspen2_router_set_enabled_state(on: i32);
    fn glaspen2_router_outline_enabled() -> i32;
    fn glaspen2_router_set_outline_state(on: i32);
    fn glaspen2_set_stroke_outline(on: i32);
    fn glaspen2_router_rebuild_surface();
    fn glaspen2_router_finish_active_stroke();
    fn glaspen2_menu_notify(text: *const c_void);
    fn sw_menu() -> *mut c_void;
}

fn menu_title(tag: i32, text: &str) {
    let menu = unsafe { sw_menu() };
    if menu.is_null() {
        return;
    }
    let c = std::ffi::CString::new(text).unwrap_or_default();
    unsafe { glaspen2_rust_menu_set_item_title(menu, tag as isize, c.as_ptr()) };
}

fn notify(text: &str) {
    let ns = objc2_foundation::NSString::from_str(text);
    unsafe { glaspen2_menu_notify(&*ns as *const objc2_foundation::NSString as *const c_void) };
}

/// R43: 涂鸦总开关; menu_title/hud_text 由 Swift 传入。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_set_draw_enabled(
    enabled: i32,
    menu_title: *const std::os::raw::c_char,
    hud_text: *const std::os::raw::c_char,
) -> i32 {
    let on = enabled != 0;
    unsafe {
        glaspen2_router_set_enabled_state(i32::from(on));
        glaspen2_router_update_menu_state(888, i32::from(on));
        glaspen2_router_update_status_icon_state();
    }
    if !menu_title.is_null() {
        let title = unsafe { std::ffi::CStr::from_ptr(menu_title) }.to_string_lossy();
        menu_title_set(888, &title);
    }
    if !hud_text.is_null() {
        let text = unsafe { std::ffi::CStr::from_ptr(hud_text) }.to_string_lossy();
        notify(&text);
    }
    1
}

fn menu_title_set(tag: i32, text: &str) {
    menu_title(tag, text);
}

/// R43: 描边开关; notify_text 由 Swift 传入。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_set_outline_enabled(
    enabled: i32,
    notify_text: *const std::os::raw::c_char,
) -> i32 {
    let on = enabled != 0;
    unsafe {
        if (glaspen2_router_outline_enabled() != 0) == on {
            return 1;
        }
        glaspen2_router_set_outline_state(i32::from(on));
        glaspen2_set_stroke_outline(i32::from(on));
        bool_setting("outline", on);
        glaspen2_router_update_menu_state(667, i32::from(on));
        glaspen2_router_finish_active_stroke();
        glaspen2_router_rebuild_surface();
    }
    if !notify_text.is_null() {
        let text = unsafe { std::ffi::CStr::from_ptr(notify_text) }.to_string_lossy();
        notify(&text);
    }
    1
}

// ── R44: 页视图 fit 状态/几何(PageViewOps) ──

static PV_FIT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static PV_SCALE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1.0f64.to_bits());
static PV_OX: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0.0f64.to_bits());
static PV_OY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0.0f64.to_bits());
static PV_PAGE_W: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
static PV_PAGE_H: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

unsafe extern "C" {
    fn glaspen2_page_dims(screen_id: i64, w: *mut i32, h: *mut i32);
    fn glaspen2_get_current_screen_id() -> i64;
}

fn pv_reset() {
    use std::sync::atomic::Ordering;
    PV_FIT.store(false, Ordering::Relaxed);
    PV_SCALE.store(1.0f64.to_bits(), Ordering::Relaxed);
    PV_OX.store(0.0f64.to_bits(), Ordering::Relaxed);
    PV_OY.store(0.0f64.to_bits(), Ordering::Relaxed);
    PV_PAGE_W.store(0, Ordering::Relaxed);
    PV_PAGE_H.store(0, Ordering::Relaxed);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pageview_update() {
    use std::sync::atomic::Ordering;
    pv_reset();
    if glaspen_core::view_state::infinite() {
        return;
    }
    let mut pw = 0i32;
    let mut ph = 0i32;
    unsafe { glaspen2_page_dims(glaspen2_get_current_screen_id(), &mut pw, &mut ph) };
    let screen_w = glaspen_core::shell_state::screen_w() as f64;
    let screen_h = glaspen_core::shell_state::screen_h() as f64;
    if pw <= 0 || ph <= 0 || (pw as f64 == screen_w && ph as f64 == screen_h) {
        return;
    }
    let scale = (screen_w / pw as f64).min(screen_h / ph as f64);
    let ox = (screen_w - pw as f64 * scale) / 2.0;
    let oy = (screen_h - ph as f64 * scale) / 2.0;
    PV_FIT.store(true, Ordering::Relaxed);
    PV_SCALE.store(scale.to_bits(), Ordering::Relaxed);
    PV_OX.store(ox.to_bits(), Ordering::Relaxed);
    PV_OY.store(oy.to_bits(), Ordering::Relaxed);
    PV_PAGE_W.store(pw, Ordering::Relaxed);
    PV_PAGE_H.store(ph, Ordering::Relaxed);
    tracing::info!(
        "rust-shell: pageview fit {pw}x{ph} -> {screen_w:.0}x{screen_h:.0} scale={scale:.3}"
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pageview_is_fit() -> i32 {
    i32::from(PV_FIT.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pageview_paper_scale() -> f64 {
    f64::from_bits(PV_SCALE.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pageview_paper_ox() -> f64 {
    f64::from_bits(PV_OX.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pageview_paper_oy() -> f64 {
    f64::from_bits(PV_OY.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pageview_page_w() -> i32 {
    PV_PAGE_W.load(std::sync::atomic::Ordering::Relaxed)
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pageview_page_h() -> i32 {
    PV_PAGE_H.load(std::sync::atomic::Ordering::Relaxed)
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pageview_geometry_width() -> i32 {
    0
}

// ── R45: 菜单 C ABI 薄桥(状态/勾选/玻璃/标题) ──

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_update_checkmarks() {
    let menu = unsafe { sw_menu() };
    if menu.is_null() {
        return;
    }
    let color_count = glaspen_core::presets::COLOR_PRESETS.len() as i32;
    let width_count = glaspen_core::presets::WIDTH_PRESETS.len() as i32;
    let (color_index, width_index) = unsafe {
        (
            glaspen2_router_selected_color(),
            glaspen2_router_selected_width(),
        )
    };
    let _ = glaspen2_rust_menu_update_checkmarks(
        menu,
        color_count,
        width_count,
        color_index,
        width_index,
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_update_menu_state(tag: i32, state: i32) {
    use objc2_app_kit::{NSControlStateValueOn, NSMenuItem};
    let menu = unsafe { sw_menu() };
    if menu.is_null() {
        return;
    }
    let p = glaspen2_rust_menu_item_with_tag(menu, tag as isize);
    if p.is_null() {
        return;
    }
    let item = unsafe { &*(p as *const NSMenuItem) };
    item.setState(if state != 0 { NSControlStateValueOn } else { 0 });
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_menu_state(tag: i32) -> i32 {
    let menu = unsafe { sw_menu() };
    if menu.is_null() {
        return 0;
    }
    glaspen2_rust_menu_item_state(menu, tag as isize)
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_glass_apply() {
    glaspen2_rust_apply_glass();
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_menu_set_title(tag: i32, title: *const std::os::raw::c_char) {
    let menu = unsafe { sw_menu() };
    if menu.is_null() || title.is_null() {
        return;
    }
    let _ = glaspen2_rust_menu_set_item_title(menu, tag as isize, title);
}

// ── R46: 镜头状态(平移/缩放/翻页偏移/相机抓手) ──

static CAM_PAN_X: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0.0f64.to_bits());
static CAM_PAN_Y: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0.0f64.to_bits());
static CAM_ZOOM: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1.0f64.to_bits());
static CAM_PAGE_OFF_X: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0.0f64.to_bits());
static CAM_PAGE_OFF_Y: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0.0f64.to_bits());
static CAM_DRAGGING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static CAM_LAST_X: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0.0f64.to_bits());
static CAM_LAST_Y: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0.0f64.to_bits());

unsafe extern "C" {
    fn glaspen2_camera_step_zoom(zoom: f64) -> f64;
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_camera(pan_x: f64, pan_y: f64, zoom: f64, assign_zoom: i32) {
    use std::sync::atomic::Ordering;
    CAM_PAN_X.store(pan_x.to_bits(), Ordering::Relaxed);
    CAM_PAN_Y.store(pan_y.to_bits(), Ordering::Relaxed);
    if assign_zoom != 0 {
        CAM_ZOOM.store(zoom.to_bits(), Ordering::Relaxed);
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_pan_x() -> f64 {
    f64::from_bits(CAM_PAN_X.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_pan_y() -> f64 {
    f64::from_bits(CAM_PAN_Y.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_zoom() -> f64 {
    f64::from_bits(CAM_ZOOM.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_reset_page_offsets() {
    use std::sync::atomic::Ordering;
    CAM_PAGE_OFF_X.store(0.0f64.to_bits(), Ordering::Relaxed);
    CAM_PAGE_OFF_Y.store(0.0f64.to_bits(), Ordering::Relaxed);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_page_off_x() -> f64 {
    f64::from_bits(CAM_PAGE_OFF_X.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_page_off_y() -> f64 {
    f64::from_bits(CAM_PAGE_OFF_Y.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_camera_dragging() -> i32 {
    i32::from(CAM_DRAGGING.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_camera_step_zoom() -> f64 {
    let zoom = glaspen2_router_zoom();
    unsafe { glaspen2_camera_step_zoom(zoom) }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_camera_set_last(x: f64, y: f64) {
    use std::sync::atomic::Ordering;
    CAM_LAST_X.store(x.to_bits(), Ordering::Relaxed);
    CAM_LAST_Y.store(y.to_bits(), Ordering::Relaxed);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_camera_last_x() -> f64 {
    f64::from_bits(CAM_LAST_X.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_camera_last_y() -> f64 {
    f64::from_bits(CAM_LAST_Y.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_camera_clear_dragging() {
    CAM_DRAGGING.store(false, std::sync::atomic::Ordering::Relaxed);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_camera_start(x: f64, y: f64) {
    use std::sync::atomic::Ordering;
    CAM_DRAGGING.store(true, Ordering::Relaxed);
    glaspen2_router_camera_set_last(x, y);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_clear_camera_dragging() {
    CAM_DRAGGING.store(false, std::sync::atomic::Ordering::Relaxed);
}

// ── R47: 画笔光标状态(位置/可见/绘制中/刷新时间) ──

static CUR_X: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new((-100.0f64).to_bits());
static CUR_Y: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new((-100.0f64).to_bits());
static CUR_VISIBLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static CUR_PEN_DRAWING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static CUR_LAST_FLUSH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_cursor_x() -> f64 {
    f64::from_bits(CUR_X.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_cursor_y() -> f64 {
    f64::from_bits(CUR_Y.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_cursor_hide() {
    CUR_VISIBLE.store(false, std::sync::atomic::Ordering::Relaxed);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_pen_cursor_visible() -> i32 {
    i32::from(CUR_VISIBLE.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn sw_cursor_set(x: f64, y: f64, visible: i32) {
    use std::sync::atomic::Ordering;
    CUR_X.store(x.to_bits(), Ordering::Relaxed);
    CUR_Y.store(y.to_bits(), Ordering::Relaxed);
    if visible != 0 {
        CUR_VISIBLE.store(true, Ordering::Relaxed);
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn sw_cursor_visible() -> i32 {
    glaspen2_router_pen_cursor_visible()
}
#[unsafe(no_mangle)]
pub extern "C" fn sw_cursor_x() -> f64 {
    glaspen2_router_cursor_x()
}
#[unsafe(no_mangle)]
pub extern "C" fn sw_cursor_y() -> f64 {
    glaspen2_router_cursor_y()
}
#[unsafe(no_mangle)]
pub extern "C" fn sw_cursor_show() {
    CUR_VISIBLE.store(true, std::sync::atomic::Ordering::Relaxed);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_pen_drawing() -> i32 {
    i32::from(CUR_PEN_DRAWING.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_pen_drawing(on: i32) {
    CUR_PEN_DRAWING.store(on != 0, std::sync::atomic::Ordering::Relaxed);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_cursor_last_flush() -> u64 {
    CUR_LAST_FLUSH.load(std::sync::atomic::Ordering::Relaxed)
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_cursor_set_last_flush(value: u64) {
    CUR_LAST_FLUSH.store(value, std::sync::atomic::Ordering::Relaxed);
}

// ── R48: 原始笔迹状态(raw last/prev/path) ──

static RAW_LAST_X: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0.0f64.to_bits());
static RAW_LAST_Y: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0.0f64.to_bits());
static RAW_LAST_W: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0.0f64.to_bits());
static RAW_PATH_LEN: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0.0f64.to_bits());
static RAW_PREV_X: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0.0f64.to_bits());
static RAW_PREV_Y: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0.0f64.to_bits());
static RAW_HAS_LAST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static RAW_HAS_PREV: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_raw_last_x() -> f64 {
    f64::from_bits(RAW_LAST_X.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_set_raw_last_x(v: f64) {
    RAW_LAST_X.store(v.to_bits(), std::sync::atomic::Ordering::Relaxed);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_raw_last_y() -> f64 {
    f64::from_bits(RAW_LAST_Y.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_set_raw_last_y(v: f64) {
    RAW_LAST_Y.store(v.to_bits(), std::sync::atomic::Ordering::Relaxed);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_raw_last_w() -> f64 {
    f64::from_bits(RAW_LAST_W.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_set_raw_last_w(v: f64) {
    RAW_LAST_W.store(v.to_bits(), std::sync::atomic::Ordering::Relaxed);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_raw_path_len() -> f64 {
    f64::from_bits(RAW_PATH_LEN.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_set_raw_path_len(v: f64) {
    RAW_PATH_LEN.store(v.to_bits(), std::sync::atomic::Ordering::Relaxed);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_raw_prev_x() -> f64 {
    f64::from_bits(RAW_PREV_X.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_set_raw_prev_x(v: f64) {
    RAW_PREV_X.store(v.to_bits(), std::sync::atomic::Ordering::Relaxed);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_raw_prev_y() -> f64 {
    f64::from_bits(RAW_PREV_Y.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_set_raw_prev_y(v: f64) {
    RAW_PREV_Y.store(v.to_bits(), std::sync::atomic::Ordering::Relaxed);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_raw_has_last() -> i32 {
    i32::from(RAW_HAS_LAST.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_set_raw_has_last(v: i32) {
    RAW_HAS_LAST.store(v != 0, std::sync::atomic::Ordering::Relaxed);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_raw_has_prev() -> i32 {
    i32::from(RAW_HAS_PREV.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_set_raw_has_prev(v: i32) {
    RAW_HAS_PREV.store(v != 0, std::sync::atomic::Ordering::Relaxed);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_clear_raw() {
    use std::sync::atomic::Ordering;
    RAW_HAS_LAST.store(false, Ordering::Relaxed);
    RAW_HAS_PREV.store(false, Ordering::Relaxed);
    RAW_PATH_LEN.store(0.0f64.to_bits(), Ordering::Relaxed);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_update_after_segment(x: f64, y: f64, width: f64) {
    use std::sync::atomic::Ordering;
    let has_last = glaspen2_rust_pen_raw_has_last() != 0;
    if has_last {
        let lx = glaspen2_rust_pen_raw_last_x();
        let ly = glaspen2_rust_pen_raw_last_y();
        let dx = x - lx;
        let dy = y - ly;
        let len = glaspen2_rust_pen_raw_path_len() + (dx * dx + dy * dy).sqrt();
        RAW_PATH_LEN.store(len.to_bits(), Ordering::Relaxed);
    }
    RAW_PREV_X.store(glaspen2_rust_pen_raw_last_x().to_bits(), Ordering::Relaxed);
    RAW_PREV_Y.store(glaspen2_rust_pen_raw_last_y().to_bits(), Ordering::Relaxed);
    RAW_HAS_PREV.store(true, Ordering::Relaxed);
    RAW_LAST_X.store(x.to_bits(), Ordering::Relaxed);
    RAW_LAST_Y.store(y.to_bits(), Ordering::Relaxed);
    RAW_LAST_W.store(width.to_bits(), Ordering::Relaxed);
    RAW_HAS_LAST.store(true, Ordering::Relaxed);
}

// ── R49: 压感监视状态(压力/坐标/接触/接近/事件类型) ──

static PM_PRESSURE: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
static PM_TIP_DOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static PM_IN_RANGE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static PM_X: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0.0f64.to_bits());
static PM_Y: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0.0f64.to_bits());
static PM_EV_TYPE: std::sync::OnceLock<std::sync::Mutex<Option<std::ffi::CString>>> =
    std::sync::OnceLock::new();

fn pm_ev_type() -> &'static std::sync::Mutex<Option<std::ffi::CString>> {
    PM_EV_TYPE.get_or_init(|| std::sync::Mutex::new(None))
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pm_update(
    in_range: i32,
    pressure: i32,
    x: f64,
    y: f64,
    tip_down: i32,
    ev_type: *const std::os::raw::c_char,
) {
    use std::sync::atomic::Ordering;
    PM_IN_RANGE.store(in_range != 0, Ordering::Relaxed);
    PM_PRESSURE.store(pressure, Ordering::Relaxed);
    PM_X.store(x.to_bits(), Ordering::Relaxed);
    PM_Y.store(y.to_bits(), Ordering::Relaxed);
    PM_TIP_DOWN.store(tip_down != 0, Ordering::Relaxed);
    let value = if ev_type.is_null() {
        None
    } else {
        Some(unsafe { std::ffi::CStr::from_ptr(ev_type) }.to_owned())
    };
    *pm_ev_type().lock().unwrap_or_else(|e| e.into_inner()) = value;
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pm_pressure() -> i32 {
    PM_PRESSURE.load(std::sync::atomic::Ordering::Relaxed)
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pm_tip_down() -> i32 {
    i32::from(PM_TIP_DOWN.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pm_in_range() -> i32 {
    i32::from(PM_IN_RANGE.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pm_x() -> f64 {
    f64::from_bits(PM_X.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pm_y() -> f64 {
    f64::from_bits(PM_Y.load(std::sync::atomic::Ordering::Relaxed))
}
/// 返回事件类型 C 字符串(调用方立即复制, 下次 update 可能失效)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pm_ev_type() -> *const std::os::raw::c_char {
    let guard = pm_ev_type().lock().unwrap_or_else(|e| e.into_inner());
    guard.as_ref().map_or(std::ptr::null(), |s| s.as_ptr())
}

// ── R50: 快捷键生效表(默认/迁移/保留/冲突解绑) ──

/// R50: 由已保存 JSON(可为空)计算并安装生效表; 返回 1 成功。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_shortcut_apply_json(saved_json: *const std::os::raw::c_char) -> i32 {
    use std::collections::BTreeMap;
    let saved: BTreeMap<String, serde_json::Value> = if saved_json.is_null() {
        BTreeMap::new()
    } else {
        let raw = unsafe { std::ffi::CStr::from_ptr(saved_json) }.to_string_lossy();
        serde_json::from_str(&raw).unwrap_or_default()
    };
    let defaults: BTreeMap<&str, &str> =
        glaspen_core::ui::defaults(glaspen_core::ui::Platform::Macos)
            .into_iter()
            .collect();
    let mut values: BTreeMap<String, String> = BTreeMap::new();
    let mut lookup: BTreeMap<String, String> = BTreeMap::new();
    for action in glaspen_core::ui::actions() {
        let action_id = action.id;
        let mut chord: Option<String> = None;
        if let Some(raw) = saved.get(action_id) {
            if let Some(s) = raw.as_str() {
                let migrated = glaspen_core::chord::migrate_value(
                    glaspen_core::ui::Platform::Macos,
                    action_id,
                    s,
                )
                .unwrap_or_else(|| s.to_string());
                chord = glaspen_core::chord::normalize(&migrated);
                if chord.is_none() && !s.trim().is_empty() {
                    tracing::warn!("hotkey: 非法组合 {action_id}={s}, 回退默认");
                }
            }
        }
        if chord.is_none() {
            chord = defaults.get(action_id).map(|s| s.to_string());
        }
        if let Some(c) = chord.clone() {
            if !c.is_empty()
                && glaspen_core::chord::is_reserved(glaspen_core::ui::Platform::Macos, &c)
            {
                tracing::warn!("hotkey: 保留组合 {action_id}={c}, 回退默认");
                chord = defaults.get(action_id).map(|s| s.to_string());
            }
        }
        if let Some(c) = chord.clone() {
            if !c.is_empty()
                && let Some(existing) = lookup.get(&c)
            {
                tracing::warn!("hotkey: 冲突 {existing} 与 {action_id} 都是 {c}; 后者解绑");
                chord = Some(String::new());
            }
        }
        let final_chord = chord.unwrap_or_default();
        if !final_chord.is_empty() {
            lookup.insert(final_chord.clone(), action_id.to_string());
        }
        values.insert(action_id.to_string(), final_chord);
    }
    glaspen_core::shortcut_state::install(values, lookup);
    1
}

// ── R51: 事件 tap 句柄/scroll 标志 ──

#[derive(Clone, Copy)]
struct EventTapPtr(*mut c_void);
unsafe impl Send for EventTapPtr {}
static EVENT_TAP: std::sync::OnceLock<std::sync::Mutex<Option<EventTapPtr>>> =
    std::sync::OnceLock::new();
static EVENT_TAP_SCROLL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn event_tap_slot() -> &'static std::sync::Mutex<Option<EventTapPtr>> {
    EVENT_TAP.get_or_init(|| std::sync::Mutex::new(None))
}

unsafe extern "C" {
    fn CFMachPortInvalidate(port: *mut c_void);
    fn CFRetain(cf: *const c_void) -> *const c_void;
    fn CGEventTapIsEnabled(tap: *mut c_void) -> bool;
    fn CGEventTapEnable(tap: *mut c_void, enable: bool);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_event_tap_store(tap: *mut c_void) {
    if !tap.is_null() {
        unsafe { CFRetain(tap as *const c_void) };
    }
    let old = {
        let mut slot = event_tap_slot().lock().unwrap_or_else(|e| e.into_inner());
        let old = slot.take();
        *slot = if tap.is_null() {
            None
        } else {
            Some(EventTapPtr(tap))
        };
        old
    };
    if let Some(old) = old {
        unsafe {
            CFMachPortInvalidate(old.0);
            CFRelease(old.0 as *const c_void);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_event_tap_note_scroll(on: i32) {
    EVENT_TAP_SCROLL.store(on != 0, std::sync::atomic::Ordering::Relaxed);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_event_tap_ok() -> i32 {
    i32::from(
        event_tap_slot()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some(),
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_event_tap_enabled() -> i32 {
    let guard = event_tap_slot().lock().unwrap_or_else(|e| e.into_inner());
    match *guard {
        Some(tap) => i32::from(unsafe { CGEventTapIsEnabled(tap.0) }),
        None => 0,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_event_tap_enable() {
    let guard = event_tap_slot().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(tap) = *guard {
        unsafe { CGEventTapEnable(tap.0, true) };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn sw_event_tap_disable() {
    let guard = event_tap_slot().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(tap) = *guard {
        unsafe { CGEventTapEnable(tap.0, false) };
    }
}

// ── R52: 设置快照 overrides JSON(启动自启/快捷键/翻页/语言) ──

unsafe extern "C" {
    fn glaspen2_is_launch_at_login() -> i32;
    fn glaspen2_shortcut_store_values_json() -> *mut std::os::raw::c_char;
    fn glaspen2_router_flip_effect() -> i32;
    fn glaspen2_router_lang() -> i32;
}

/// R52: 返回堆上 JSON(调用方 free), 失败返回 null。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_menu_snapshot_overrides_json() -> *mut std::os::raw::c_char {
    let launch = unsafe { glaspen2_is_launch_at_login() != 0 };
    let shortcuts = unsafe {
        let p = glaspen2_shortcut_store_values_json();
        if p.is_null() {
            "{}".to_string()
        } else {
            let s = std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned();
            // Rust core 的 into_raw 字符串由 glaspen2_free_c_string 释放。
            unsafe extern "C" {
                fn glaspen2_free_c_string(p: *mut std::os::raw::c_char);
            }
            unsafe { glaspen2_free_c_string(p) };
            s
        }
    };
    let flip = unsafe { glaspen2_router_flip_effect() };
    let lang = if unsafe { glaspen2_router_lang() } == 1 {
        "en"
    } else {
        "zh"
    };
    let value = serde_json::json!({
        "launchAtLogin": launch,
        "shortcuts": shortcuts,
        "flipEffect": flip,
        "language": lang,
    });
    match std::ffi::CString::new(value.to_string()) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

// ── R53: 描边 cairo context 活动槽 + 光标刷新判定 ──

#[derive(Clone, Copy)]
struct CairoCtxPtr(*mut c_void);
unsafe impl Send for CairoCtxPtr {}
static STROKE_CTX: std::sync::OnceLock<std::sync::Mutex<Option<CairoCtxPtr>>> =
    std::sync::OnceLock::new();

fn stroke_ctx_slot() -> &'static std::sync::Mutex<Option<CairoCtxPtr>> {
    STROKE_CTX.get_or_init(|| std::sync::Mutex::new(None))
}

unsafe extern "C" {
    fn cairo_create_scaled() -> *mut c_void;
    fn glaspen2_perf_elapsed_us(since: u64) -> u64;
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_acquire_cr(owned: *mut i32) -> *mut c_void {
    let guard = stroke_ctx_slot().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ctx) = *guard {
        if !owned.is_null() {
            unsafe { *owned = 0 };
        }
        ctx.0
    } else {
        if !owned.is_null() {
            unsafe { *owned = 1 };
        }
        unsafe { cairo_create_scaled() }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_active_ctx_present() -> i32 {
    i32::from(
        stroke_ctx_slot()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some(),
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_install_active_ctx(ctx: *mut c_void) {
    *stroke_ctx_slot().lock().unwrap_or_else(|e| e.into_inner()) = if ctx.is_null() {
        None
    } else {
        Some(CairoCtxPtr(ctx))
    };
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_take_active_ctx() -> *mut c_void {
    stroke_ctx_slot()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .map_or(std::ptr::null_mut(), |ctx| ctx.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_cursor_flush_due() -> i32 {
    unsafe {
        if sw_stroke_active() != 0 {
            return 0;
        }
        if glaspen2_perf_elapsed_us(glaspen2_rust_cursor_last_flush()) >= 16000 {
            glaspen2_rust_cursor_set_last_flush(mach_absolute_time());
            return 1;
        }
        0
    }
}

unsafe extern "C" {
    fn sw_stroke_active() -> i32;
}

// ── R54: 录制 HUD 语义 -> 本地化文案映射 ──

fn rec_hud_msg(hud: &str) -> Option<glaspen_core::i18n::Msg> {
    use glaspen_core::i18n::Msg;
    Some(match hud {
        "GifStartHint" => Msg::GifStartHint,
        "SendStartHint" => Msg::SendStartHint,
        "DraftStartHint" => Msg::DraftStartHint,
        "DraftStartFailed" => Msg::DraftStartFailed,
        "GifCopied" => Msg::GifCopied,
        "GifNoStrokesOrExportFailed" => Msg::GifNoStrokesOrExportFailed,
        "GifClipboardFailed" => Msg::GifClipboardFailed,
        "SendNoStrokes" => Msg::SendNoStrokes,
        "SendSucceeded" => Msg::SendSucceeded,
        "SendFailed" => Msg::SendFailed,
        "DraftAccepted" => Msg::DraftAccepted,
        "DraftDeclined" => Msg::DraftDeclined,
        "DraftFailed" => Msg::DraftFailed,
        _ => return None,
    })
}

/// R54: 返回 HUD 文案(C 字符串, 调用方 free); 未知 hud 返回 null。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_rec_hud_text(
    hud: *const std::os::raw::c_char,
    strokes: i32,
    detail: *const std::os::raw::c_char,
) -> *mut std::os::raw::c_char {
    if hud.is_null() {
        return std::ptr::null_mut();
    }
    let hud = unsafe { std::ffi::CStr::from_ptr(hud) }.to_string_lossy();
    let Some(msg) = rec_hud_msg(&hud) else {
        return std::ptr::null_mut();
    };
    let lang = if glaspen_core::shell_state::lang() == 1 {
        glaspen_core::i18n::Lang::En
    } else {
        glaspen_core::i18n::Lang::Zh
    };
    let mut text = glaspen_core::i18n::text(lang, &msg).to_string();
    if hud == "SendStartHint" {
        text = text.replace("{keys}", "⌘⌃3");
    } else if hud == "DraftStartHint" {
        text = text.replace("{keys}", "⌘⌃2");
    } else if hud == "SendSucceeded" || hud == "DraftAccepted" {
        text = text.replace("{n}", &strokes.to_string());
    } else if hud == "DraftFailed" && !detail.is_null() {
        let d = unsafe { std::ffi::CStr::from_ptr(detail) }.to_string_lossy();
        if !d.is_empty() {
            text = d.into_owned();
        }
    }
    std::ffi::CString::new(text).map_or(std::ptr::null_mut(), |s| s.into_raw())
}

// ── R55: 实时绘制计划(core plan JSON)光栅化 ──

unsafe extern "C" {
    fn cairo_save(cr: *mut c_void);
    fn cairo_restore(cr: *mut c_void);
    fn cairo_set_operator(cr: *mut c_void, op: i32);
    fn cairo_set_line_cap(cr: *mut c_void, cap: i32);
    fn cairo_set_line_join(cr: *mut c_void, join: i32);
    fn cairo_set_dash(cr: *mut c_void, dashes: *const f64, count: i32, offset: f64);
    fn cairo_set_source_rgba(cr: *mut c_void, r: f64, g: f64, b: f64, a: f64);
    fn cairo_set_source(cr: *mut c_void, pattern: *mut c_void);
    fn cairo_set_line_width(cr: *mut c_void, width: f64);
    fn cairo_move_to(cr: *mut c_void, x: f64, y: f64);
    fn cairo_line_to(cr: *mut c_void, x: f64, y: f64);
    fn cairo_arc(cr: *mut c_void, x: f64, y: f64, radius: f64, a1: f64, a2: f64);
    fn cairo_fill(cr: *mut c_void);
    fn cairo_stroke(cr: *mut c_void);
    fn cairo_pattern_destroy(pattern: *mut c_void);
    fn glaspen2_router_invert_ink_source(cr: *mut c_void) -> *mut c_void;
    fn glaspen2_router_pen_r() -> f64;
    fn glaspen2_router_pen_g() -> f64;
    fn glaspen2_router_pen_b() -> f64;
}

fn plan_num(v: &serde_json::Value, key: &str) -> f64 {
    v.get(key).and_then(|x| x.as_f64()).unwrap_or(0.0)
}
fn plan_point(v: &serde_json::Value, i: usize) -> f64 {
    v.get(i).and_then(|x| x.as_f64()).unwrap_or(0.0)
}
fn plan_color(cr: *mut c_void, cmd: &serde_json::Value, alpha: f64) {
    let (mut r, mut g, mut b) = (0.0, 0.0, 0.0);
    if let Some(arr) = cmd.get("color").and_then(|x| x.as_array()) {
        r = arr.first().and_then(|x| x.as_f64()).unwrap_or(0.0);
        g = arr.get(1).and_then(|x| x.as_f64()).unwrap_or(0.0);
        b = arr.get(2).and_then(|x| x.as_f64()).unwrap_or(0.0);
    }
    unsafe { cairo_set_source_rgba(cr, r, g, b, alpha) };
}

fn plan_command(cr: *mut c_void, cmd: &serde_json::Value, inv: f64) {
    let Some(pass) = cmd.get("pass").and_then(|x| x.as_str()) else {
        return;
    };
    let Some(points) = cmd.get("points").and_then(|x| x.as_array()) else {
        return;
    };
    if points.is_empty() {
        return;
    }
    let alpha = plan_num(cmd, "alpha");
    let width_delta = plan_num(cmd, "width_delta");
    unsafe {
        cairo_set_dash(cr, std::ptr::null(), 0, 0.0);
        cairo_set_operator(cr, 2);
        cairo_set_line_cap(cr, 1);
        cairo_set_line_join(cr, 1);
    }
    match pass {
        "shadow" => {
            plan_color(cr, cmd, alpha);
            if points.len() == 1 {
                let p = &points[0];
                if p.as_array().map_or(0, |a| a.len()) < 3 {
                    return;
                }
                unsafe {
                    cairo_arc(
                        cr,
                        plan_point(p, 0) * inv,
                        plan_point(p, 1) * inv,
                        plan_point(p, 2) * 0.5 * inv,
                        0.0,
                        std::f64::consts::TAU,
                    );
                    cairo_fill(cr);
                }
                return;
            }
            for i in 1..points.len() {
                let (a, b) = (&points[i - 1], &points[i]);
                if a.as_array().map_or(0, |x| x.len()) < 3
                    || b.as_array().map_or(0, |x| x.len()) < 3
                {
                    continue;
                }
                unsafe {
                    cairo_set_line_width(cr, (plan_point(b, 2) + width_delta) * inv);
                    cairo_move_to(cr, plan_point(a, 0) * inv, plan_point(a, 1) * inv);
                    cairo_line_to(cr, plan_point(b, 0) * inv, plan_point(b, 1) * inv);
                    cairo_stroke(cr);
                }
            }
        }
        "outline" => {
            if points.len() == 1 {
                let p = &points[0];
                if p.as_array().map_or(0, |a| a.len()) < 3 {
                    return;
                }
                unsafe {
                    cairo_set_dash(cr, std::ptr::null(), 0, 0.0);
                    cairo_set_source_rgba(cr, 0.0, 0.0, 0.0, alpha);
                    cairo_arc(
                        cr,
                        plan_point(p, 0) * inv,
                        plan_point(p, 1) * inv,
                        plan_point(p, 2) * 0.5 * inv,
                        0.0,
                        std::f64::consts::TAU,
                    );
                    cairo_fill(cr);
                }
                return;
            }
            if points.len() < 2 {
                return;
            }
            let mut dash_len = cmd
                .get("dash")
                .and_then(|d| d.get("len"))
                .and_then(|x| x.as_f64())
                .unwrap_or(1.0)
                * inv;
            if !(dash_len > 0.0) {
                dash_len = 1.0;
            }
            let mut cum = cmd
                .get("dash")
                .and_then(|d| d.get("phase"))
                .and_then(|x| x.as_f64())
                .unwrap_or(0.0)
                * inv;
            let pattern = [dash_len, dash_len];
            unsafe { cairo_set_line_cap(cr, 2) };
            for i in 1..points.len() {
                let (a, b) = (&points[i - 1], &points[i]);
                if a.as_array().map_or(0, |x| x.len()) < 3
                    || b.as_array().map_or(0, |x| x.len()) < 3
                {
                    continue;
                }
                let (x0, y0) = (plan_point(a, 0) * inv, plan_point(a, 1) * inv);
                let (x1, y1) = (plan_point(b, 0) * inv, plan_point(b, 1) * inv);
                let seg_width = (plan_point(b, 2) + width_delta) * inv;
                let mut off = cum % (2.0 * dash_len);
                if off < 0.0 {
                    off += 2.0 * dash_len;
                }
                unsafe {
                    cairo_set_line_width(cr, seg_width);
                    cairo_set_dash(cr, pattern.as_ptr(), 2, off);
                    cairo_set_source_rgba(cr, 0.0, 0.0, 0.0, alpha);
                    cairo_move_to(cr, x0, y0);
                    cairo_line_to(cr, x1, y1);
                    cairo_stroke(cr);
                    let nxt = (off + dash_len) % (2.0 * dash_len);
                    cairo_set_dash(cr, pattern.as_ptr(), 2, nxt);
                    cairo_set_source_rgba(cr, 1.0, 1.0, 1.0, alpha);
                    cairo_move_to(cr, x0, y0);
                    cairo_line_to(cr, x1, y1);
                    cairo_stroke(cr);
                }
                let dx = x1 - x0;
                let dy = y1 - y0;
                cum += (dx * dx + dy * dy).sqrt();
            }
            unsafe {
                cairo_set_dash(cr, std::ptr::null(), 0, 0.0);
                cairo_set_line_cap(cr, 1);
            }
        }
        "erase" => {
            unsafe {
                cairo_set_operator(cr, 3);
                cairo_set_source_rgba(cr, 0.0, 0.0, 0.0, 1.0);
            }
            if points.len() == 1 {
                let p = &points[0];
                if p.as_array().map_or(0, |a| a.len()) < 3 {
                    return;
                }
                unsafe {
                    cairo_arc(
                        cr,
                        plan_point(p, 0) * inv,
                        plan_point(p, 1) * inv,
                        plan_point(p, 2) * 0.5 * inv,
                        0.0,
                        std::f64::consts::TAU,
                    );
                    cairo_fill(cr);
                }
            } else {
                for i in 1..points.len() {
                    let (a, b) = (&points[i - 1], &points[i]);
                    if a.as_array().map_or(0, |x| x.len()) < 3
                        || b.as_array().map_or(0, |x| x.len()) < 3
                    {
                        continue;
                    }
                    unsafe {
                        cairo_set_line_width(cr, plan_point(b, 2) * inv);
                        cairo_move_to(cr, plan_point(a, 0) * inv, plan_point(a, 1) * inv);
                        cairo_line_to(cr, plan_point(b, 0) * inv, plan_point(b, 1) * inv);
                        cairo_stroke(cr);
                    }
                }
            }
            unsafe { cairo_set_operator(cr, 2) };
        }
        _ => {
            // fill
            let invert = unsafe { glaspen2_router_invert_ink_source(cr) };
            if !invert.is_null() {
                unsafe { cairo_set_source(cr, invert) };
            } else {
                plan_color(cr, cmd, alpha);
            }
            if points.len() == 1 {
                let p = &points[0];
                if p.as_array().map_or(0, |a| a.len()) >= 3 {
                    unsafe {
                        cairo_arc(
                            cr,
                            plan_point(p, 0) * inv,
                            plan_point(p, 1) * inv,
                            plan_point(p, 2) * 0.5 * inv,
                            0.0,
                            std::f64::consts::TAU,
                        );
                        cairo_fill(cr);
                    }
                }
            } else {
                for i in 1..points.len() {
                    let (a, b) = (&points[i - 1], &points[i]);
                    if a.as_array().map_or(0, |x| x.len()) < 3
                        || b.as_array().map_or(0, |x| x.len()) < 3
                    {
                        continue;
                    }
                    unsafe {
                        cairo_set_line_width(cr, plan_point(b, 2) * inv);
                        cairo_move_to(cr, plan_point(a, 0) * inv, plan_point(a, 1) * inv);
                        cairo_line_to(cr, plan_point(b, 0) * inv, plan_point(b, 1) * inv);
                        cairo_stroke(cr);
                    }
                }
            }
            if !invert.is_null() {
                unsafe { cairo_pattern_destroy(invert) };
            }
        }
    }
}

fn plan_seam(cr: *mut c_void, seam: &serde_json::Value, inv: f64) {
    let from = seam.get("from").and_then(|x| x.as_array());
    let to = seam.get("to").and_then(|x| x.as_array());
    let (Some(from), Some(to)) = (from, to) else {
        return;
    };
    if from.len() < 2 || to.len() < 2 {
        return;
    }
    let width = plan_num(seam, "width") * inv;
    if width <= 0.0 {
        return;
    }
    unsafe {
        cairo_set_dash(cr, std::ptr::null(), 0, 0.0);
        cairo_set_operator(cr, 2);
        cairo_set_line_cap(cr, 1);
        cairo_set_line_join(cr, 1);
        cairo_set_line_width(cr, width);
        cairo_set_source_rgba(
            cr,
            glaspen2_router_pen_r(),
            glaspen2_router_pen_g(),
            glaspen2_router_pen_b(),
            1.0,
        );
        cairo_move_to(
            cr,
            from.first().and_then(|x| x.as_f64()).unwrap_or(0.0) * inv,
            from.get(1).and_then(|x| x.as_f64()).unwrap_or(0.0) * inv,
        );
        cairo_line_to(
            cr,
            to.first().and_then(|x| x.as_f64()).unwrap_or(0.0) * inv,
            to.get(1).and_then(|x| x.as_f64()).unwrap_or(0.0) * inv,
        );
        cairo_stroke(cr);
    }
}

/// R55: 光栅化 core live plan JSON; 返回 1 成功, 0 失败(调用方回退)。
/// fallback 两点的 (x,y,pad) 通过 7 个参数传入; count<2 时忽略。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_draw_live_plan(
    cr: *mut c_void,
    plan_json: *const std::os::raw::c_char,
    inv_scale: f64,
    dot: i32,
    f1x: f64,
    f1y: f64,
    f1p: f64,
    f2x: f64,
    f2y: f64,
    f2p: f64,
    fallback_count: i32,
) -> i32 {
    if cr.is_null() || plan_json.is_null() {
        return 0;
    }
    let text = unsafe { std::ffi::CStr::from_ptr(plan_json) }.to_string_lossy();
    let root: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return 0,
    };
    if root.get("error").is_some() {
        return 0;
    }
    let Some(commands) = root.get("commands").and_then(|x| x.as_array()) else {
        return 0;
    };
    unsafe { cairo_save(cr) };
    for cmd in commands {
        plan_command(cr, cmd, inv_scale);
    }
    if dot == 0 {
        if let Some(seam) = root.get("seam") {
            plan_seam(cr, seam, inv_scale);
        }
    }
    unsafe { cairo_restore(cr) };
    if let Some(dirty) = root.get("dirty") {
        let dx = plan_num(dirty, "x") * inv_scale;
        let dy = plan_num(dirty, "y") * inv_scale;
        let dw = plan_num(dirty, "w") * inv_scale;
        let dh = plan_num(dirty, "h") * inv_scale;
        if dx.is_finite()
            && dy.is_finite()
            && dw.is_finite()
            && dh.is_finite()
            && dw > 0.0
            && dh > 0.0
        {
            glaspen2_rust_dirty_include_surface_rect(dx, dy, dw, dh);
        } else if fallback_count > 0 {
            glaspen2_rust_dirty_include_surface_point(f1x, f1y, f1p);
            if fallback_count > 1 {
                glaspen2_rust_dirty_include_surface_point(f2x, f2y, f2p);
            }
        }
    }
    1
}

// ── R56: 实时笔迹首点/拖动(计划优先 + 旧回退光栅化) ──

unsafe extern "C" {
    fn cairo_destroy(cr: *mut c_void);
    fn glaspen2_router_flush_dirty();
    fn glaspen2_router_soft_shadow() -> i32;
    fn glaspen2_router_outline_pad() -> f64;
    fn glaspen2_router_outline_dash() -> f64;
    fn glaspen2_router_eraser_mode() -> i32;
}

fn raw_dot_fallback(cr: *mut c_void, x: f64, y: f64, width: f64) {
    let eraser = unsafe { glaspen2_router_eraser_mode() != 0 };
    let shadow = unsafe { glaspen2_router_soft_shadow() != 0 };
    unsafe {
        cairo_set_operator(cr, if eraser { 3 } else { 2 });
        if shadow && !eraser {
            for (pad, alpha) in [(1.5, 0.20), (3.0, 0.13), (5.0, 0.07)] {
                cairo_set_source_rgba(cr, 0.0, 0.0, 0.0, alpha);
                cairo_arc(cr, x, y, width * 0.5 + pad, 0.0, std::f64::consts::TAU);
                cairo_fill(cr);
            }
        }
        if glaspen2_router_outline_enabled() != 0 && !eraser {
            cairo_set_source_rgba(cr, 0.0, 0.0, 0.0, 1.0);
            cairo_arc(
                cr,
                x,
                y,
                width * 0.5 + glaspen2_router_outline_pad(),
                0.0,
                std::f64::consts::TAU,
            );
            cairo_fill(cr);
        }
        let ipat = glaspen2_router_invert_ink_source(cr);
        if ipat.is_null() {
            cairo_set_source_rgba(
                cr,
                glaspen2_router_pen_r(),
                glaspen2_router_pen_g(),
                glaspen2_router_pen_b(),
                1.0,
            );
        }
        cairo_arc(cr, x, y, width * 0.5, 0.0, std::f64::consts::TAU);
        cairo_fill(cr);
        if !ipat.is_null() {
            cairo_pattern_destroy(ipat);
        }
    }
    let pad = width * 0.5 + 1.5 + if shadow { 5.0 } else { 0.0 };
    glaspen2_rust_dirty_include_surface_point(x, y, pad);
}

fn raw_segment_fallback(cr: *mut c_void, x: f64, y: f64, width: f64) {
    let has_last = glaspen2_rust_pen_raw_has_last() != 0;
    let has_prev = glaspen2_rust_pen_raw_has_prev() != 0;
    let last_x = glaspen2_rust_pen_raw_last_x();
    let last_y = glaspen2_rust_pen_raw_last_y();
    let last_w = glaspen2_rust_pen_raw_last_w();
    let path_len = glaspen2_rust_pen_raw_path_len();
    let eraser = unsafe { glaspen2_router_eraser_mode() != 0 };
    let shadow = unsafe { glaspen2_router_soft_shadow() != 0 };
    let outline = unsafe { glaspen2_router_outline_enabled() != 0 };
    let pad_const = unsafe { glaspen2_router_outline_pad() };
    let dash = unsafe { glaspen2_router_outline_dash() };
    unsafe {
        cairo_set_operator(cr, if eraser { 3 } else { 2 });
        cairo_set_line_cap(cr, 1);
        cairo_set_line_join(cr, 1);
        if shadow && !eraser {
            for (sp, alpha) in [(1.5, 0.20), (3.0, 0.13), (5.0, 0.07)] {
                cairo_set_source_rgba(cr, 0.0, 0.0, 0.0, alpha);
                cairo_set_line_width(cr, width + 2.0 * sp);
                if has_last {
                    cairo_move_to(cr, last_x, last_y);
                    cairo_line_to(cr, x, y);
                    cairo_stroke(cr);
                } else {
                    cairo_arc(cr, x, y, width * 0.5 + sp, 0.0, std::f64::consts::TAU);
                    cairo_fill(cr);
                }
            }
        }
        if outline && !eraser {
            let mut off = path_len % (2.0 * dash);
            cairo_set_line_cap(cr, 2);
            cairo_set_line_width(cr, width + 2.0 * pad_const);
            if has_last {
                let dashes = [dash, dash];
                cairo_set_dash(cr, dashes.as_ptr(), 2, off);
                cairo_set_source_rgba(cr, 0.0, 0.0, 0.0, 1.0);
                cairo_move_to(cr, last_x, last_y);
                cairo_line_to(cr, x, y);
                cairo_stroke(cr);
                off = (off + dash) % (2.0 * dash);
                cairo_set_dash(cr, std::ptr::null(), 0, 0.0);
                cairo_set_source_rgba(cr, 1.0, 1.0, 1.0, 1.0);
                cairo_move_to(cr, last_x, last_y);
                cairo_line_to(cr, x, y);
                cairo_stroke(cr);
                cairo_set_dash(cr, std::ptr::null(), 0, 0.0);
            } else {
                cairo_set_dash(cr, std::ptr::null(), 0, 0.0);
                cairo_set_source_rgba(cr, 0.0, 0.0, 0.0, 1.0);
                cairo_arc(
                    cr,
                    x,
                    y,
                    width * 0.5 + pad_const,
                    0.0,
                    std::f64::consts::TAU,
                );
                cairo_fill(cr);
            }
            cairo_set_line_cap(cr, 1);
        }
        let ipat = glaspen2_router_invert_ink_source(cr);
        if ipat.is_null() {
            cairo_set_source_rgba(
                cr,
                glaspen2_router_pen_r(),
                glaspen2_router_pen_g(),
                glaspen2_router_pen_b(),
                1.0,
            );
        }
        cairo_set_line_width(cr, width);
        if has_last {
            cairo_move_to(cr, last_x, last_y);
            cairo_line_to(cr, x, y);
            cairo_stroke(cr);
        } else {
            cairo_arc(cr, x, y, width * 0.5, 0.0, std::f64::consts::TAU);
            cairo_fill(cr);
        }
        if !ipat.is_null() {
            cairo_pattern_destroy(ipat);
        }
        if outline && !eraser && has_prev {
            cairo_set_source_rgba(
                cr,
                glaspen2_router_pen_r(),
                glaspen2_router_pen_g(),
                glaspen2_router_pen_b(),
                1.0,
            );
            cairo_set_line_width(cr, last_w);
            cairo_move_to(
                cr,
                glaspen2_rust_pen_raw_prev_x(),
                glaspen2_rust_pen_raw_prev_y(),
            );
            cairo_line_to(cr, last_x, last_y);
            cairo_stroke(cr);
        }
    }
    let pad = width * 0.5 + 1.5 + if shadow { 5.0 } else { 0.0 };
    if has_last {
        glaspen2_rust_dirty_include_surface_point(last_x, last_y, pad);
    }
    glaspen2_rust_dirty_include_surface_point(x, y, pad);
}

// ── R57: 笔状态(书写中/橡皮/接近/设备) ──

static PEN_STROKE_ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static PEN_ERASER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static PEN_TABLET_PROX: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static PEN_SEEN_PROX: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static PEN_PROX_DEVICE: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_stroke_active() -> i32 {
    i32::from(PEN_STROKE_ACTIVE.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_set_stroke_active(v: i32) {
    PEN_STROKE_ACTIVE.store(v != 0, std::sync::atomic::Ordering::Relaxed);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_eraser_mode() -> i32 {
    i32::from(PEN_ERASER.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_set_eraser_mode(v: i32) {
    PEN_ERASER.store(v != 0, std::sync::atomic::Ordering::Relaxed);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_tablet_proximity() -> i32 {
    i32::from(PEN_TABLET_PROX.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_set_tablet_proximity(v: i32) {
    PEN_TABLET_PROX.store(v != 0, std::sync::atomic::Ordering::Relaxed);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_has_seen_proximity() -> i32 {
    i32::from(PEN_SEEN_PROX.load(std::sync::atomic::Ordering::Relaxed))
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_set_has_seen_proximity(v: i32) {
    PEN_SEEN_PROX.store(v != 0, std::sync::atomic::Ordering::Relaxed);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_prox_device() -> i32 {
    PEN_PROX_DEVICE.load(std::sync::atomic::Ordering::Relaxed)
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_set_prox_device(v: i32) {
    PEN_PROX_DEVICE.store(v, std::sync::atomic::Ordering::Relaxed);
}

// ── R58: 画布模式切换(翻页 <-> 无限) ──

unsafe extern "C" {
    fn glaspen2_router_camera_pan_cancel();
    fn glaspen2_router_camera_persist_infinite();
    fn glaspen2_router_camera_load_infinite();
    fn glaspen2_router_camera_reset_lens();
    fn glaspen2_router_infinite() -> i32;
    fn glaspen2_router_set_infinite_state(on: i32);
    fn glaspen2_set_canvas_kind(kind: i32);
    fn glaspen2_load_infinite_strokes();
    fn glaspen2_load_strokes_for_screen(id: i64);
    fn glaspen2_clear_strokes(w: i32, h: i32);
    fn glaspen2_router_event_tap_reinstall();
    fn glaspen2_notify_settings_changed();
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_apply_infinite(
    on: i32,
    notify_text: *const std::os::raw::c_char,
) -> i32 {
    let want = on != 0;
    if (unsafe { glaspen2_router_infinite() } != 0) == want {
        return 1;
    }
    unsafe {
        glaspen2_router_clear_camera_dragging();
        glaspen2_router_camera_pan_cancel();
        glaspen2_router_finish_active_stroke();
        if glaspen2_router_infinite() != 0 {
            glaspen2_router_camera_persist_infinite();
        }
        glaspen2_router_set_infinite_state(i32::from(want));
        let key = std::ffi::CString::new("infinite_canvas").unwrap();
        glaspen2_save_bool_setting(key.as_ptr(), i32::from(want));
        glaspen2_router_update_menu_state(668, i32::from(want));
        glaspen2_router_update_status_icon_state();
        glaspen2_set_canvas_kind(i32::from(want));
        if want {
            glaspen2_load_infinite_strokes();
            glaspen2_router_camera_load_infinite();
        } else {
            let cur = glaspen2_get_current_screen_id();
            if cur > 0 {
                glaspen2_load_strokes_for_screen(cur);
            } else {
                glaspen2_clear_strokes(
                    glaspen_core::shell_state::screen_w(),
                    glaspen_core::shell_state::screen_h(),
                );
            }
            glaspen2_rust_pageview_update();
            glaspen2_router_camera_reset_lens();
        }
        glaspen2_router_rebuild_surface();
        glaspen2_router_event_tap_reinstall();
        glaspen2_notify_settings_changed();
    }
    if !notify_text.is_null() {
        let text = unsafe { std::ffi::CStr::from_ptr(notify_text) }.to_string_lossy();
        notify(&text);
    }
    1
}

// ── R60: 涂鸦总开关热键路径(窗口显隐/压感窗/状态/通知/菜单) ──

unsafe extern "C" {
    fn glaspen2_router_restore_system_cursor();
    fn glaspen2_router_main_window() -> *mut c_void;
    fn glaspen2_router_ensure_surface();
    fn glaspen2_router_pressure_monitor() -> i32;
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_toggle_draw(
    menu_title: *const std::os::raw::c_char,
    hud_text: *const std::os::raw::c_char,
) -> i32 {
    use objc2_app_kit::NSWindow;
    let enabled = unsafe { glaspen2_router_enabled() } == 0;
    tracing::info!("rust-shell: draw enabled={enabled}");
    unsafe {
        glaspen2_router_set_enabled_state(i32::from(enabled));
        let win_ptr = glaspen2_router_main_window();
        let window = if win_ptr.is_null() {
            None
        } else {
            Some(&*(win_ptr as *const NSWindow))
        };
        if !enabled {
            glaspen2_router_restore_system_cursor();
            glaspen2_router_finish_active_stroke();
            if let Some(window) = window {
                window.orderOut(None);
            }
            if glaspen2_router_pressure_monitor() != 0 {
                glaspen2_router_pm_hide();
            }
        } else {
            if window.is_some() {
                glaspen2_router_ensure_surface();
                if glaspen2_router_ethereal() != 0 && glaspen2_router_strokes_visible() == 0 {
                    glaspen2_router_rebuild_surface();
                    if let Some(window) = window {
                        let _: () = unsafe { objc2::msg_send![window, setIsVisible: false] };
                    }
                } else {
                    if let Some(window) = window {
                        window.orderFrontRegardless();
                    }
                    glaspen2_router_rebuild_surface();
                }
            }
            if glaspen2_router_pressure_monitor() != 0 {
                glaspen2_router_pm_show();
            }
        }
        glaspen2_router_update_status_icon_state();
        glaspen2_router_update_menu_state(888, i32::from(enabled));
    }
    if !menu_title.is_null() {
        let title = unsafe { std::ffi::CStr::from_ptr(menu_title) }.to_string_lossy();
        let menu = unsafe { sw_menu() };
        if !menu.is_null() {
            let c = std::ffi::CString::new(title.as_bytes()).unwrap_or_default();
            unsafe { glaspen2_rust_menu_set_item_title(menu, 888, c.as_ptr()) };
        }
    }
    if !hud_text.is_null() {
        let text = unsafe { std::ffi::CStr::from_ptr(hud_text) }.to_string_lossy();
        notify(&text);
    }
    1
}

// ── R61: 笔事件编排(落笔/抬笔/拖动/橡皮态) ──

unsafe extern "C" {
    fn glaspen2_router_bubble_auto_page_flip();
    fn glaspen2_router_modeler_begin(x: f64, y: f64, pressure: f64, timestamp: f64);
    fn glaspen2_router_modeler_move(x: f64, y: f64, pressure: f64, timestamp: f64);
    fn glaspen2_router_modeler_end(x: f64, y: f64, pressure: f64, timestamp: f64);
    fn glaspen2_router_modeler_erase_finish();
    fn glaspen2_router_stroke_begin();
    fn glaspen2_router_stroke_end();
    fn glaspen2_router_commit_to_strokes();
    fn glaspen2_router_pen_down_state(x: f64, y: f64, raw_width: f64);
    fn glaspen2_router_pen_up_state();
    fn glaspen2_router_set_stroke_active(on: i32);
    fn glaspen2_router_set_eraser_mode(on: i32);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_down(
    x: f64,
    y: f64,
    pressure: f64,
    timestamp: f64,
    raw_width: f64,
    scale: f64,
) {
    unsafe {
        glaspen2_router_bubble_auto_page_flip();
        glaspen2_router_modeler_begin(x, y, pressure, timestamp);
        glaspen2_router_stroke_begin();
    }
    glaspen2_rust_raw_draw_dot2(x, y, raw_width, scale);
    unsafe { glaspen2_router_pen_down_state(x, y, raw_width) };
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_up(x: f64, y: f64, pressure: f64, timestamp: f64) {
    if glaspen2_rust_pen_stroke_active() == 0 {
        return;
    }
    unsafe {
        if glaspen2_router_eraser_mode() != 0 {
            glaspen2_router_modeler_erase_finish();
        } else {
            glaspen2_router_modeler_end(x, y, pressure, timestamp);
            glaspen2_router_commit_to_strokes();
        }
        glaspen2_router_stroke_end();
        glaspen2_router_set_stroke_active(0);
    }
    glaspen2_rust_apply_pending_color_width();
    unsafe { glaspen2_router_pen_up_state() };
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_move(
    x: f64,
    y: f64,
    pressure: f64,
    timestamp: f64,
    raw_width: f64,
    scale: f64,
) {
    unsafe { glaspen2_router_modeler_move(x, y, pressure, timestamp) };
    glaspen2_rust_raw_draw_segment2(x, y, raw_width, scale);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_update_eraser(is_eraser: i32) {
    let want = is_eraser != 0;
    let current = unsafe { glaspen2_router_eraser_mode() != 0 };
    if want != current {
        unsafe {
            glaspen2_router_set_eraser_mode(i32::from(want));
            glaspen2_router_update_status_icon_state();
        }
    }
}

// ── R62: 键盘/滚轮镜头动作(平移/缩放/应用/持久化) ──

unsafe extern "C" {
    fn glaspen2_camera_pan_values(
        px: f64,
        py: f64,
        pz: f64,
        dx: f64,
        dy: f64,
        ox: *mut f64,
        oy: *mut f64,
        oz: *mut f64,
    ) -> i32;
    fn glaspen2_camera_zoom_at_values(
        px: f64,
        py: f64,
        pz: f64,
        factor: f64,
        vx: f64,
        vy: f64,
        ox: *mut f64,
        oy: *mut f64,
        oz: *mut f64,
        hit_max: *mut i32,
        hit_min: *mut i32,
        changed: *mut i32,
    ) -> i32;
    fn glaspen2_camera_zoom_at_center_values(
        px: f64,
        py: f64,
        pz: f64,
        factor: f64,
        vw: f64,
        vh: f64,
        ox: *mut f64,
        oy: *mut f64,
        oz: *mut f64,
        hit_max: *mut i32,
        hit_min: *mut i32,
        changed: *mut i32,
    ) -> i32;
    fn glaspen2_camera_home(px: *mut f64, py: *mut f64, pz: *mut f64);
    fn glaspen2_set_view_transform(px: f64, py: f64, pz: f64);
    fn glaspen2_set_infinite_transform(px: f64, py: f64, pz: f64);
    fn glaspen2_get_infinite_transform(px: *mut f64, py: *mut f64, pz: *mut f64);
    fn glaspen2_router_canvas_apply_transform();
    fn glaspen2_router_peek_lens_change();
    fn glaspen2_router_state() -> i32;
    fn glaspen2_router_screen_w() -> f64;
    fn glaspen2_router_screen_h() -> f64;
}

static CAM_LAST_SAVE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static CAM_LAST_ZOOM_HINT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn now_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn camera_persist_infinite_now() {
    unsafe {
        if glaspen2_router_state() & 4 == 0 {
            return;
        }
        glaspen2_set_infinite_transform(
            glaspen2_router_pan_x(),
            glaspen2_router_pan_y(),
            glaspen2_router_zoom(),
        );
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_camera_pan(dx: f64, dy: f64) {
    if glaspen2_rust_pen_stroke_active() != 0 {
        return;
    }
    let mut px = 0.0;
    let mut py = 0.0;
    let mut pz = 0.0;
    let ok = unsafe {
        glaspen2_camera_pan_values(
            glaspen2_router_pan_x(),
            glaspen2_router_pan_y(),
            glaspen2_router_zoom(),
            dx,
            dy,
            &mut px,
            &mut py,
            &mut pz,
        )
    };
    if ok == 0 {
        return;
    }
    unsafe {
        glaspen2_router_set_camera(px, py, pz, 1);
        glaspen2_router_canvas_apply_transform();
        glaspen2_router_peek_lens_change();
    }
}

fn camera_apply_zoom(px: f64, py: f64, pz: f64, hit_max: i32, changed: i32) {
    if hit_max != 0 {
        let now = now_secs();
        let last = f64::from_bits(CAM_LAST_ZOOM_HINT.load(std::sync::atomic::Ordering::Relaxed));
        if now - last > 1.5 {
            CAM_LAST_ZOOM_HINT.store(now.to_bits(), std::sync::atomic::Ordering::Relaxed);
            let lang = if glaspen_core::shell_state::lang() == 1 {
                glaspen_core::i18n::Lang::En
            } else {
                glaspen_core::i18n::Lang::Zh
            };
            notify(glaspen_core::i18n::text(
                lang,
                &glaspen_core::i18n::Msg::ZoomMax100,
            ));
        }
    }
    unsafe {
        glaspen2_router_set_camera(px, py, pz, 1);
        if changed != 0 {
            glaspen2_router_canvas_apply_transform();
            glaspen2_router_peek_lens_change();
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_camera_zoom_at(factor: f64, vx: f64, vy: f64) {
    if glaspen2_rust_pen_stroke_active() != 0 {
        return;
    }
    let mut px = 0.0;
    let mut py = 0.0;
    let mut pz = 0.0;
    let (mut hit_max, mut hit_min, mut changed) = (0, 0, 0);
    let ok = unsafe {
        glaspen2_camera_zoom_at_values(
            glaspen2_router_pan_x(),
            glaspen2_router_pan_y(),
            glaspen2_router_zoom(),
            factor,
            vx,
            vy,
            &mut px,
            &mut py,
            &mut pz,
            &mut hit_max,
            &mut hit_min,
            &mut changed,
        )
    };
    if ok == 0 {
        return;
    }
    camera_apply_zoom(px, py, pz, hit_max, changed);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_camera_zoom_at_center(factor: f64) {
    if glaspen2_rust_pen_stroke_active() != 0 {
        return;
    }
    let mut px = 0.0;
    let mut py = 0.0;
    let mut pz = 0.0;
    let (mut hit_max, mut hit_min, mut changed) = (0, 0, 0);
    let ok = unsafe {
        glaspen2_camera_zoom_at_center_values(
            glaspen2_router_pan_x(),
            glaspen2_router_pan_y(),
            glaspen2_router_zoom(),
            factor,
            glaspen2_router_screen_w(),
            glaspen2_router_screen_h(),
            &mut px,
            &mut py,
            &mut pz,
            &mut hit_max,
            &mut hit_min,
            &mut changed,
        )
    };
    if ok == 0 {
        return;
    }
    camera_apply_zoom(px, py, pz, hit_max, changed);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_camera_persist_infinite() {
    camera_persist_infinite_now();
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_camera_load_infinite() {
    let (mut px, mut py, mut pz) = (0.0, 0.0, 1.0);
    unsafe {
        glaspen2_get_infinite_transform(&mut px, &mut py, &mut pz);
        glaspen2_router_set_camera(px, py, pz, 1);
        glaspen2_set_view_transform(px, py, pz);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_camera_apply_transform() {
    unsafe {
        glaspen2_set_view_transform(
            glaspen2_router_pan_x(),
            glaspen2_router_pan_y(),
            glaspen2_router_zoom(),
        );
        glaspen2_router_rebuild_surface();
    }
    let now = now_secs();
    let last = f64::from_bits(CAM_LAST_SAVE.load(std::sync::atomic::Ordering::Relaxed));
    if now - last > 0.5 {
        CAM_LAST_SAVE.store(now.to_bits(), std::sync::atomic::Ordering::Relaxed);
        camera_persist_infinite_now();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_camera_home() {
    let (mut px, mut py, mut pz) = (0.0, 0.0, 1.0);
    unsafe { glaspen2_camera_home(&mut px, &mut py, &mut pz) };
    unsafe { glaspen2_router_set_camera(px, py, pz, 1) };
    glaspen2_rust_camera_apply_transform();
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_camera_reset_lens() {
    unsafe { glaspen2_router_reset_page_offsets() };
    let (mut px, mut py, mut pz) = (0.0, 0.0, 1.0);
    unsafe {
        glaspen2_camera_home(&mut px, &mut py, &mut pz);
        glaspen2_router_set_camera(px, py, pz, 1);
        glaspen2_set_view_transform(px, py, pz);
    }
}

// ── R63: 退出流程与压感标签格式化 ──

unsafe extern "C" {
    fn glaspen2_router_perf_summary();
    fn glaspen2_router_show_system_cursor();
    fn glaspen2_router_pressure_label(text: *const std::os::raw::c_char);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_quit_app() {
    use objc2_app_kit::NSApplication;
    unsafe {
        glaspen2_router_perf_summary();
        glaspen2_router_show_system_cursor();
    }
    if let Some(mtm) = MainThreadMarker::new() {
        NSApplication::sharedApplication(mtm).terminate(None);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_update_pressure_label(
    pressure: i32,
    event_type: *const std::os::raw::c_char,
    x: f64,
    y: f64,
) {
    let type_string = if event_type.is_null() {
        "---".to_string()
    } else {
        unsafe { std::ffi::CStr::from_ptr(event_type) }
            .to_string_lossy()
            .into_owned()
    };
    let text = format!(
        "P={:<5}  {}  ({},{})",
        pressure, type_string, x as i32, y as i32
    );
    if let Ok(c) = std::ffi::CString::new(text) {
        unsafe { glaspen2_router_pressure_label(c.as_ptr()) };
    }
}

// ── R64: 收尾在写笔画/恢复系统光标 ──

unsafe extern "C" {
    fn glaspen2_router_last_raw_x() -> f64;
    fn glaspen2_router_last_raw_y() -> f64;
    fn glaspen2_router_clear_raw_state();
}

#[repr(C)]
struct Timespec {
    tv_sec: i64,
    tv_nsec: i64,
}

fn monotonic_secs() -> f64 {
    let mut ts = Timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // CLOCK_MONOTONIC = 6 on Darwin.
    let rc = unsafe { clock_gettime(6, &mut ts) };
    if rc == 0 {
        ts.tv_sec as f64 + ts.tv_nsec as f64 / 1e9
    } else {
        0.0
    }
}

unsafe extern "C" {
    fn clock_gettime(clk: i32, ts: *mut Timespec) -> i32;
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_finish_active_stroke() {
    if glaspen2_rust_pen_stroke_active() == 0 {
        return;
    }
    let ts = monotonic_secs();
    unsafe {
        if glaspen2_router_eraser_mode() != 0 {
            glaspen2_router_modeler_erase_finish();
        } else {
            glaspen2_router_modeler_end(
                glaspen2_router_last_raw_x(),
                glaspen2_router_last_raw_y(),
                0.0,
                ts,
            );
            glaspen2_router_commit_to_strokes();
        }
        glaspen2_rust_update_eraser(0);
        glaspen2_router_stroke_end();
        glaspen2_router_set_stroke_active(0);
        glaspen2_rust_apply_pending_color_width();
        glaspen2_router_clear_raw_state();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_restore_system_cursor() {
    unsafe {
        if glaspen2_router_pen_drawing() != 0 {
            glaspen2_router_show_system_cursor();
            glaspen2_router_set_pen_drawing(0);
        }
    }
}

// ── R67: 实时绘制计划入参/调用/光栅化整体迁 Rust ──

unsafe extern "C" {
    fn glaspen2_live_plan_json(input: *const std::os::raw::c_char) -> *mut std::os::raw::c_char;
    fn glaspen2_free_c_string(p: *mut std::os::raw::c_char);
    fn cairo_get_target(cr: *mut c_void) -> *mut c_void;
    fn cairo_image_surface_get_width(surface: *mut c_void) -> i32;
    fn cairo_image_surface_get_height(surface: *mut c_void) -> i32;
}

fn live_plan_run(
    cr: *mut c_void,
    input: &serde_json::Value,
    inv: f64,
    dot: bool,
    fallback: &[(f64, f64, f64)],
) -> bool {
    let text = serde_json::to_string(input).unwrap_or_default();
    let Ok(cinput) = std::ffi::CString::new(text) else {
        return false;
    };
    let out = unsafe { glaspen2_live_plan_json(cinput.as_ptr()) };
    if out.is_null() {
        return false;
    }
    let s = unsafe { std::ffi::CStr::from_ptr(out) }
        .to_string_lossy()
        .into_owned();
    unsafe { glaspen2_free_c_string(out) };
    let f1 = fallback.first().copied().unwrap_or((0.0, 0.0, 0.0));
    let f2 = fallback.get(1).copied().unwrap_or(f1);
    let Ok(cplan) = std::ffi::CString::new(s) else {
        return false;
    };
    glaspen2_rust_draw_live_plan(
        cr,
        cplan.as_ptr(),
        inv,
        i32::from(dot),
        f1.0,
        f1.1,
        f1.2,
        f2.0,
        f2.1,
        f2.2,
        fallback.len() as i32,
    ) != 0
}

fn live_plan_params(surface_w: i32, surface_h: i32, scale: f64, arc_len: f64) -> serde_json::Value {
    let shadow = unsafe { glaspen2_router_soft_shadow() != 0 };
    let outline = unsafe { glaspen2_router_outline_enabled() != 0 };
    let eraser = unsafe { glaspen2_router_eraser_mode() != 0 };
    let (r, g, b) = unsafe {
        (
            glaspen2_router_pen_r(),
            glaspen2_router_pen_g(),
            glaspen2_router_pen_b(),
        )
    };
    serde_json::json!({
        "viewport": [surface_w, surface_h],
        "scale": scale,
        "zoom": 1.0,
        "pan": [0.0, 0.0],
        "shadow": shadow,
        "outline": outline,
        "eraser": eraser,
        "alpha": 1.0,
        "arc_len_start": arc_len,
        "color": [r, g, b],
    })
}

fn live_plan_dot(cr: *mut c_void, x: f64, y: f64, width: f64, scale: f64) -> bool {
    let target = unsafe { cairo_get_target(cr) };
    if target.is_null() {
        return false;
    }
    let sw = unsafe { cairo_image_surface_get_width(target) };
    let sh = unsafe { cairo_image_surface_get_height(target) };
    if sw <= 0 || sh <= 0 {
        return false;
    }
    let input = serde_json::json!({
        "points": [[x, y, width]],
        "params": live_plan_params(sw, sh, scale, 0.0),
    });
    let shadow = unsafe { glaspen2_router_soft_shadow() != 0 };
    let pad = width * 0.5 + 1.5 + if shadow { 5.0 } else { 0.0 };
    live_plan_run(cr, &input, 1.0 / scale.max(0.0001), true, &[(x, y, pad)])
}

fn live_plan_segment(cr: *mut c_void, x: f64, y: f64, width: f64, scale: f64) -> bool {
    let target = unsafe { cairo_get_target(cr) };
    if target.is_null() {
        return false;
    }
    let sw = unsafe { cairo_image_surface_get_width(target) };
    let sh = unsafe { cairo_image_surface_get_height(target) };
    if sw <= 0 || sh <= 0 {
        return false;
    }
    let has_last = glaspen2_rust_pen_raw_has_last() != 0;
    let last_x = glaspen2_rust_pen_raw_last_x();
    let last_y = glaspen2_rust_pen_raw_last_y();
    let last_w = glaspen2_rust_pen_raw_last_w();
    let path_len = glaspen2_rust_pen_raw_path_len();
    let mut input = serde_json::json!({
        "points": [[last_x, last_y, last_w], [x, y, width]],
        "params": live_plan_params(sw, sh, scale, path_len),
    });
    if glaspen2_rust_pen_raw_has_prev() != 0 {
        input["seam"] = serde_json::json!([
            glaspen2_rust_pen_raw_prev_x(),
            glaspen2_rust_pen_raw_prev_y()
        ]);
    }
    let shadow = unsafe { glaspen2_router_soft_shadow() != 0 };
    let pad = width * 0.5 + 1.5 + if shadow { 5.0 } else { 0.0 };
    let last_pad = last_w * 0.5 + 1.5 + if shadow { 5.0 } else { 0.0 };
    let _ = has_last;
    live_plan_run(
        cr,
        &input,
        1.0 / scale.max(0.0001),
        false,
        &[(x, y, pad), (last_x, last_y, last_pad)],
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_raw_draw_dot2(x: f64, y: f64, width: f64, scale: f64) {
    let mut owned = 0i32;
    let cr = glaspen2_router_acquire_cr(&mut owned);
    if cr.is_null() {
        return;
    }
    let planned = live_plan_dot(cr, x, y, width, scale);
    if !planned {
        raw_dot_fallback(cr, x, y, width);
    }
    if owned != 0 {
        unsafe { cairo_destroy(cr) };
    }
    unsafe { glaspen2_router_flush_dirty() };
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_raw_draw_segment2(x: f64, y: f64, width: f64, scale: f64) {
    let mut owned = 0i32;
    let cr = glaspen2_router_acquire_cr(&mut owned);
    if cr.is_null() {
        return;
    }
    let planned = live_plan_segment(cr, x, y, width, scale);
    if !planned {
        raw_segment_fallback(cr, x, y, width);
    }
    if owned != 0 {
        unsafe { cairo_destroy(cr) };
    }
    glaspen2_rust_pen_update_after_segment(x, y, width);
    unsafe { glaspen2_router_flush_dirty() };
}

// ── R70: 录制热键判定/事件 JSON(⌘⌃R/2/3) ──

unsafe extern "C" {
    fn glaspen2_router_rec_slot_gif() -> i32;
    fn glaspen2_router_rec_slot_chat() -> i32;
    fn glaspen2_router_rec_mode(slot: i32) -> i32;
    fn glaspen2_router_chat_integration() -> i32;
    fn glaspen2_router_rec_finish_and_count() -> i32;
    fn glaspen2_router_recording_feed(slot: i32, json: *const std::os::raw::c_char) -> i32;
}

fn recording_feed(slot: i32, kind: &str, mode: &str) {
    let strokes = unsafe { glaspen2_router_rec_finish_and_count() };
    let payload = serde_json::json!({
        "type": kind,
        "mode": mode,
        "stroke_count": strokes,
    });
    if let Ok(c) = std::ffi::CString::new(payload.to_string()) {
        unsafe { glaspen2_router_recording_feed(slot, c.as_ptr()) };
    }
}

/// R70: 录制热键; 返回 1 已处理, 0 未处理。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_recording_hotkey(
    down: i32,
    key_code: u16,
    has_cmd_ctrl: i32,
) -> i32 {
    let down = down != 0;
    let has_cmd_ctrl = has_cmd_ctrl != 0;
    let gif = unsafe { glaspen2_router_rec_slot_gif() };
    let chat = unsafe { glaspen2_router_rec_slot_chat() };
    const KEY_R: u16 = 15;
    const KEY_2: u16 = 19;
    const KEY_3: u16 = 20;
    if !down && key_code == KEY_R && unsafe { glaspen2_router_rec_mode(gif) } == 1 {
        recording_feed(gif, "ReleasePressed", "Gif");
        return 1;
    }
    if down && has_cmd_ctrl && key_code == KEY_R {
        recording_feed(gif, "StartPressed", "Gif");
        return 1;
    }
    if !down && key_code == KEY_3 && unsafe { glaspen2_router_rec_mode(chat) } == 3 {
        recording_feed(chat, "ReleasePressed", "Send");
        return 1;
    }
    if down
        && unsafe { glaspen2_router_chat_integration() } != 0
        && has_cmd_ctrl
        && key_code == KEY_3
    {
        recording_feed(chat, "StartPressed", "Send");
        return 1;
    }
    if !down && key_code == KEY_2 && unsafe { glaspen2_router_rec_mode(chat) } == 2 {
        recording_feed(chat, "ReleasePressed", "Draft");
        return 1;
    }
    if down
        && unsafe { glaspen2_router_chat_integration() } != 0
        && has_cmd_ctrl
        && key_code == KEY_2
    {
        recording_feed(chat, "StartPressed", "Draft");
        return 1;
    }
    0
}

// ── R73: 无限画布总览载荷(bbox 外扩/渲染/视口矩形) ──

unsafe extern "C" {
    fn glaspen2_stroke_bbox(bx: *mut f64, by: *mut f64, bx2: *mut f64, by2: *mut f64) -> i32;
    fn glaspen2_render_canvas_overview(
        bx: f64,
        by: f64,
        bw: f64,
        bh: f64,
        ow: i32,
        oh: i32,
        out_len: *mut i32,
    ) -> *mut u8;
    fn glaspen2_free_rust_bytes(p: *mut u8, len: i32);
}

/// R73: 返回 PNG 字节指针(NULL=失败)并写 rect[4]/bounds[4]/len;
/// 成功时调用方用 glaspen2_free_rust_bytes 释放。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_canvas_overview(
    width: f64,
    height: f64,
    rect_out: *mut f64,
    bounds_out: *mut f64,
    len_out: *mut i32,
) -> *mut u8 {
    if rect_out.is_null() || bounds_out.is_null() || len_out.is_null() {
        return std::ptr::null_mut();
    }
    let (mut bx, mut by, mut bx2, mut by2) = (0.0, 0.0, 0.0, 0.0);
    if unsafe { glaspen2_stroke_bbox(&mut bx, &mut by, &mut bx2, &mut by2) } == 0 {
        return std::ptr::null_mut();
    }
    let mut bw = bx2 - bx;
    let mut bh = by2 - by;
    if bw < 1.0 {
        bw = 1.0;
    }
    if bh < 1.0 {
        bh = 1.0;
    }
    let mx = bw * 0.05;
    let my = bh * 0.05;
    bx -= mx;
    by -= my;
    bw += mx * 2.0;
    bh += my * 2.0;
    let ow = width as i32;
    let oh = height as i32;
    let mut out_len = 0i32;
    let png = unsafe { glaspen2_render_canvas_overview(bx, by, bw, bh, ow, oh, &mut out_len) };
    if png.is_null() || out_len <= 0 {
        if !png.is_null() {
            unsafe { glaspen2_free_rust_bytes(png, out_len) };
        }
        return std::ptr::null_mut();
    }
    let ov_scale = (ow as f64 / bw).min(oh as f64 / bh);
    let ov_ox = (ow as f64 - bw * ov_scale) * 0.5;
    let ov_oy = (oh as f64 - bh * ov_scale) * 0.5;
    let z = unsafe { glaspen2_camera_step_zoom(glaspen2_router_zoom()) };
    let (screen_w, screen_h) = unsafe { (glaspen2_router_screen_w(), glaspen2_router_screen_h()) };
    let vr_w = screen_w / z * ov_scale;
    let vr_h = screen_h / z * ov_scale;
    let vx = ov_ox + (glaspen2_router_pan_x() - bx) * ov_scale;
    let vy = ov_oy + (glaspen2_router_pan_y() - by) * ov_scale;
    unsafe {
        *rect_out.add(0) = vx;
        *rect_out.add(1) = vy;
        *rect_out.add(2) = vr_w;
        *rect_out.add(3) = vr_h;
        *bounds_out.add(0) = bx;
        *bounds_out.add(1) = by;
        *bounds_out.add(2) = bw;
        *bounds_out.add(3) = bh;
        *len_out = out_len;
    }
    png
}

// ── R74: 动作 id -> effect 分派 ──

unsafe extern "C" {
    fn glaspen2_action_effect_json(
        action: *const std::os::raw::c_char,
    ) -> *mut std::os::raw::c_char;
    fn glaspen2_router_perform_hotkey(key_code: u16) -> i32;
    fn glaspen2_router_prev_color_index() -> i32;
    fn glaspen2_router_selected_color_index() -> i32;
    fn glaspen2_color_preset_count() -> i32;
    fn glaspen2_router_prev_width_index() -> i32;
    fn glaspen2_router_selected_width_index() -> i32;
    fn glaspen2_width_preset_count() -> i32;
    fn glaspen2_camera_keyboard_pan_step(zoom: f64) -> f64;
    fn glaspen2_router_apply_infinite_async(on: i32);
    fn glaspen2_router_quit_async();
}

fn action_hud_msg(msg: glaspen_core::i18n::Msg, name_key: &str, name: &str) -> String {
    let lang = if glaspen_core::shell_state::lang() == 1 {
        glaspen_core::i18n::Lang::En
    } else {
        glaspen_core::i18n::Lang::Zh
    };
    glaspen_core::i18n::text(lang, &msg).replace(&format!("{{{name_key}}}"), name)
}

/// R74: 动作 effect 分派; 返回 1 已处理, 0 失败/未实现。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_perform_action(action: *const std::os::raw::c_char) -> i32 {
    if action.is_null() {
        return 0;
    }
    let out = unsafe { glaspen2_action_effect_json(action) };
    if out.is_null() {
        return 0;
    }
    let text = unsafe { std::ffi::CStr::from_ptr(out) }
        .to_string_lossy()
        .into_owned();
    unsafe { glaspen2_free_c_string(out) };
    let Ok(effect) = serde_json::from_str::<serde_json::Value>(&text) else {
        return 0;
    };
    let Some(kind) = effect.get("kind").and_then(|k| k.as_str()) else {
        tracing::warn!("action: effect 分发失败: {text}");
        return 0;
    };
    let hotkey = |kc: u16| unsafe { glaspen2_router_perform_hotkey(kc) } != 0;
    match kind {
        "Undo" => return i32::from(hotkey(0x06)),
        "PagePrev" => return i32::from(hotkey(0x32)),
        "PageNext" => return i32::from(hotkey(0x12)),
        "ToggleDraw" => return i32::from(hotkey(0x09)),
        "Clear" => return i32::from(hotkey(0x08)),
        "Export" => return i32::from(hotkey(0x05)),
        "CopySvg" => return i32::from(hotkey(0x01)),
        "ToggleGlass" => return i32::from(hotkey(0x0B)),
        "ToggleEthereal" => return i32::from(hotkey(0x07)),
        "OpenSettings" => return i32::from(hotkey(0x02)),
        "ToggleFreeCanvas" => {
            unsafe {
                if glaspen2_router_infinite() == 0 {
                    let key = std::ffi::CString::new("show_free_canvas").unwrap();
                    if glaspen2_load_bool_setting(key.as_ptr()) == 0 {
                        glaspen2_router_apply_infinite_async(0);
                    }
                }
                let on = if glaspen2_router_infinite() != 0 {
                    0
                } else {
                    1
                };
                glaspen2_router_apply_infinite_async(on);
            }
            return 1;
        }
        "Quit" => {
            unsafe { glaspen2_router_quit_async() };
            return 1;
        }
        _ => {}
    }
    if kind.starts_with("CameraPan") {
        if unsafe { glaspen2_router_infinite() } == 0 {
            return 1;
        }
        let step = unsafe { glaspen2_camera_keyboard_pan_step(glaspen2_router_zoom()) };
        let (mut dx, mut dy) = (0.0, 0.0);
        match kind {
            "CameraPanLeft" => dx = step,
            "CameraPanRight" => dx = -step,
            "CameraPanUp" => dy = step,
            "CameraPanDown" => dy = -step,
            _ => {}
        }
        glaspen2_rust_camera_pan(dx, dy);
        return 1;
    }
    if kind == "CameraZoomIn" || kind == "CameraZoomOut" {
        if unsafe { glaspen2_router_infinite() } == 0 {
            return 1;
        }
        let factor = if kind == "CameraZoomIn" {
            1.15
        } else {
            1.0 / 1.15
        };
        glaspen2_rust_camera_zoom_at_center(factor);
        return 1;
    }
    if kind == "ColorSwap" {
        let selected = unsafe { glaspen2_router_selected_color_index() };
        let count = unsafe { glaspen2_color_preset_count() };
        let mut prev = unsafe { glaspen2_router_prev_color_index() };
        if prev < 0 || prev >= count || prev == selected {
            prev = if selected == 0 { 9 } else { 0 };
        }
        let _ = glaspen2_rust_select_color(prev);
        let lang = if glaspen_core::shell_state::lang() == 1 {
            glaspen_core::i18n::Lang::En
        } else {
            glaspen_core::i18n::Lang::Zh
        };
        let name = glaspen_core::i18n::color_name(prev as usize, lang);
        notify(&action_hud_msg(
            glaspen_core::i18n::Msg::ColorChanged,
            "color",
            name,
        ));
        return 1;
    }
    if kind == "WidthSwap" {
        let selected = unsafe { glaspen2_router_selected_width_index() };
        let count = unsafe { glaspen2_width_preset_count() };
        let mut prev = unsafe { glaspen2_router_prev_width_index() };
        if prev < 0 || prev >= count || prev == selected {
            prev = if selected <= 3 { 6 } else { 2 };
        }
        let _ = glaspen2_rust_select_width(prev);
        let lang = if glaspen_core::shell_state::lang() == 1 {
            glaspen_core::i18n::Lang::En
        } else {
            glaspen_core::i18n::Lang::Zh
        };
        let name = glaspen_core::i18n::width_name(prev as usize, lang);
        notify(&action_hud_msg(
            glaspen_core::i18n::Msg::WidthChanged,
            "width",
            name,
        ));
        return 1;
    }
    tracing::warn!("action: effect kind {kind} 未实现");
    0
}
