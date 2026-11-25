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

/// R28: 1x1 透明光标(旧 makeBlankCursor)。
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
