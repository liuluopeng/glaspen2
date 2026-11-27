//! macOS AppKit 壳的 Rust 预研(R14): 先用最外层状态栏项创建验证 objc2
//! 在真实 app 内可工作; 默认仍走 Swift, 由 `GLASPEN2_RUST_STATUS_ITEM=1` 切换。
#![cfg(target_os = "macos")]

use std::ffi::c_void;

// cairo_operator_t: CLEAR=0, SOURCE=1, OVER=2, IN=3。
// 旧代码把 3 当 CLEAR 用, 结果清屏/橡皮变成 IN+黑色源: 已有彩色笔迹
// 被整体染黑而不是清除。
const CAIRO_OPERATOR_CLEAR: i32 = 0;
const CAIRO_OPERATOR_SOURCE: i32 = 1;
const CAIRO_OPERATOR_OVER: i32 = 2;

use glaspen_core::action_effect::CameraDir;
use glaspen_core::camera::Camera;

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
fn monospaced_font_ptr(size: f64, w: f64) -> *mut objc2_app_kit::NSFont {
    let Some(cls) = objc2::runtime::AnyClass::get(c"NSFont") else {
        return std::ptr::null_mut();
    };
    unsafe { objc2::msg_send![cls, monospacedSystemFontOfSize: size, weight: w] }
}

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
    let font_ptr = monospaced_font_ptr(36.0, unsafe { NSFontWeightMedium } as f64);
    if font_ptr.is_null() {
        return 0;
    }
    let font: *mut NSFont = font_ptr;
    let attrs: objc2::rc::Retained<NSMutableDictionary<NSAttributedStringKey, AnyObject>> =
        NSMutableDictionary::new();
    unsafe {
        let font_any: &AnyObject = &*(font as *const AnyObject);
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
    fn glaspen2_router_set_pending_color(idx: i32);
    fn glaspen2_router_selected_color() -> i32;
    fn glaspen2_router_set_prev_color(idx: i32);
    fn glaspen2_router_apply_color(idx: i32);
    fn glaspen2_router_save_pen_settings();
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
    fn glaspen2_share_ink_set_active(on: i32);
    fn glaspen2_load_bool_setting(key: *const std::os::raw::c_char) -> i32;
    fn glaspen2_router_set_pressure_monitor(on: i32);
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
    fn glaspen2_swift_overlay_minimap(ctx: *mut c_void, bx: f64, by: f64, bw: f64, bh: f64);
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
    // R162: prepare/tunnel 直接调 Rust。
    unsafe {
        let surf = glaspen2_rust_surface_current();
        if surf.is_null() {
            CGContext::restore_g_state(Some(&cg));
            return 1;
        }
        cairo_surface_flush(surf);
        let tun = glaspen2_swift_tunnel_surface();
        if !tun.is_null() {
            cairo_surface_flush(tun);
        }
        if glaspen2_rust_tunnel_get_active() != 0 && !tun.is_null() {
            let data = cairo_image_surface_get_data(tun);
            let w = cairo_image_surface_get_width(tun);
            let h = cairo_image_surface_get_height(tun);
            let stride = cairo_image_surface_get_stride(tun);
            if !data.is_null() {
                let _ =
                    glaspen2_rust_draw_tunnel(ctx_ptr, data as *mut c_void, w, h, stride, bw, bh);
                CGContext::restore_g_state(Some(&cg));
                return 1;
            }
        }
    }
    // R161: grid 直接调 Rust(门控/pan/zoom/clip 与原 stage 一致)。
    unsafe {
        let show_grid = glaspen2_router_show_grid() != 0;
        let divider = glaspen2_router_grid_divider();
        let strokes_visible = glaspen2_router_strokes_visible() != 0;
        let follow = glaspen2_router_grid_follow() != 0;
        let tun_active = glaspen2_rust_tunnel_get_active() != 0;
        if (show_grid || divider > 0)
            && !tun_active
            && (strokes_visible || !follow)
            && bw > 0.0
            && bh > 0.0
        {
            let (mut pan_x, mut pan_y, mut zoom) = (0.0f64, 0.0f64, 1.0f64);
            if glaspen2_router_infinite() != 0 {
                pan_x = glaspen2_router_pan_x();
                pan_y = glaspen2_router_pan_y();
                let z = glaspen2_router_zoom();
                zoom = if z > 0.05 { z } else { 0.05 };
            } else {
                pan_x = -glaspen2_router_page_off_x();
                pan_y = -glaspen2_router_page_off_y();
            }
            let _ = glaspen2_rust_draw_grid(
                ctx_ptr,
                bw,
                bh,
                pan_x,
                pan_y,
                zoom,
                glaspen2_router_grid_size(),
                divider,
                i32::from(show_grid),
                glaspen2_router_screen_w(),
                glaspen2_router_screen_h(),
                rx,
                bh - (ry + rh),
                rx + rw,
                bh - ry,
            );
        }
    }
    // R161: surface 直接调 Rust。
    unsafe {
        let surf = glaspen2_rust_surface_current();
        if !surf.is_null() && glaspen2_router_strokes_visible() != 0 {
            let data = cairo_image_surface_get_data(surf);
            let w = cairo_image_surface_get_width(surf);
            let h = cairo_image_surface_get_height(surf);
            let stride = cairo_image_surface_get_stride(surf);
            if !data.is_null() && w > 0 && h > 0 {
                let _ = glaspen2_rust_draw_surface_data(
                    ctx_ptr,
                    data as *mut c_void,
                    w,
                    h,
                    stride,
                    rx,
                    ry,
                    rw,
                    rh,
                    bx,
                    by,
                    bw,
                    bh,
                    glaspen2_rust_surface_scale(),
                );
            }
        }
    }
    unsafe {
        let _ = glaspen2_rust_minimap_draw(ctx_ptr, bx, by, bw, bh);
        let _ = glaspen2_rust_notification_draw(bw, bh);
        let _ = glaspen2_rust_draw_cursor(
            ctx_ptr,
            i32::from(glaspen2_router_pen_cursor_visible() != 0),
            glaspen2_router_cursor_x(),
            glaspen2_router_cursor_y(),
            glaspen2_router_eraser_mode(),
        );
    }
    CGContext::restore_g_state(Some(&cg));
    // R163: perf notes 直接写 C。
    unsafe {
        if glaspen2_router_perf_enabled() != 0 {
            let mut notes = format!("rect={:.0}x{:.0}", rw, rh);
            if (rx - bx).abs() < 0.5
                && (ry - by).abs() < 0.5
                && (rw - bw).abs() < 0.5
                && (rh - bh).abs() < 0.5
            {
                notes.push_str(" FULL");
            }
            if glaspen2_router_show_grid() != 0 {
                notes.push_str(" grid");
            }
            let infinite = glaspen2_router_infinite() != 0;
            let minimap = glaspen2_router_minimap_enabled() != 0;
            if minimap && !infinite {
                notes.push_str(" minimap");
            }
            if let (Ok(name), Ok(notes)) = (
                std::ffi::CString::new("drawrect"),
                std::ffi::CString::new(notes),
            ) {
                glaspen2_router_perf_notes(name.as_ptr(), notes.as_ptr(), t0);
            }
        }
    }
    1
}

unsafe extern "C" {
    fn glaspen2_swift_tunnel_surface() -> *mut c_void;
    fn glaspen2_router_minimap_enabled() -> i32;
    fn glaspen2_router_show_grid() -> i32;
    fn glaspen2_router_grid_divider() -> i32;
    fn glaspen2_router_grid_follow() -> i32;
    fn glaspen2_router_grid_size() -> f64;
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
                let font_ptr = monospaced_font_ptr(9.0, unsafe { NSFontWeightRegular } as f64);
                if font_ptr.is_null() {
                    return 0;
                }
                let attrs: objc2::rc::Retained<
                    NSMutableDictionary<NSAttributedStringKey, AnyObject>,
                > = NSMutableDictionary::new();
                unsafe {
                    let font_any: &AnyObject = &*(font_ptr as *const AnyObject);
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

unsafe extern "C" {}

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
/// 实时笔迹的源色必须与整页重建的 `Color::to_rgb8`(×255 截断)一致。
/// 否则刚写的笔迹是浮点色, 抬笔/重绘后变成 8 位量化色: 同一笔在
/// “实时层”和“重建层”出现两套几乎一样的颜色, 用户会看到微小变色。
fn quantize_raw_rgb(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let ch = |v: f64| {
        let clipped = if v.is_finite() {
            v.clamp(0.0, 1.0)
        } else {
            0.0
        };
        ((clipped * 255.0) as u8) as f64 / 255.0
    };
    (ch(r), ch(g), ch(b))
}

fn plan_color(cr: *mut c_void, cmd: &serde_json::Value, alpha: f64) {
    let (mut r, mut g, mut b) = (0.0, 0.0, 0.0);
    if let Some(arr) = cmd.get("color").and_then(|x| x.as_array()) {
        r = arr.first().and_then(|x| x.as_f64()).unwrap_or(0.0);
        g = arr.get(1).and_then(|x| x.as_f64()).unwrap_or(0.0);
        b = arr.get(2).and_then(|x| x.as_f64()).unwrap_or(0.0);
    }
    let (r, g, b) = quantize_raw_rgb(r, g, b);
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
        cairo_set_operator(cr, CAIRO_OPERATOR_OVER);
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
                cairo_set_operator(cr, CAIRO_OPERATOR_CLEAR);
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
            unsafe { cairo_set_operator(cr, CAIRO_OPERATOR_OVER) };
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
        cairo_set_operator(cr, CAIRO_OPERATOR_OVER);
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
    fn glaspen2_router_soft_shadow() -> i32;
}

fn raw_dot_fallback(cr: *mut c_void, x: f64, y: f64, width: f64) {
    let eraser = unsafe { glaspen2_router_eraser_mode() != 0 };
    let shadow = unsafe { glaspen2_router_soft_shadow() != 0 };
    unsafe {
        cairo_set_operator(
            cr,
            if eraser {
                CAIRO_OPERATOR_CLEAR
            } else {
                CAIRO_OPERATOR_OVER
            },
        );
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
            let (pr, pg, pb) = quantize_raw_rgb(
                glaspen2_router_pen_r(),
                glaspen2_router_pen_g(),
                glaspen2_router_pen_b(),
            );
            cairo_set_source_rgba(cr, pr, pg, pb, 1.0);
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
        cairo_set_operator(
            cr,
            if eraser {
                CAIRO_OPERATOR_CLEAR
            } else {
                CAIRO_OPERATOR_OVER
            },
        );
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
            let (pr, pg, pb) = quantize_raw_rgb(
                glaspen2_router_pen_r(),
                glaspen2_router_pen_g(),
                glaspen2_router_pen_b(),
            );
            cairo_set_source_rgba(cr, pr, pg, pb, 1.0);
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
            let (pr, pg, pb) = quantize_raw_rgb(
                glaspen2_router_pen_r(),
                glaspen2_router_pen_g(),
                glaspen2_router_pen_b(),
            );
            cairo_set_source_rgba(cr, pr, pg, pb, 1.0);
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
    fn glaspen2_router_infinite() -> i32;
    fn glaspen2_router_set_infinite_state(on: i32);
    fn glaspen2_set_canvas_kind(kind: i32);
    fn glaspen2_load_infinite_strokes();
    fn glaspen2_load_strokes_for_screen(id: i64);
    fn glaspen2_clear_strokes(w: i32, h: i32) -> i32;
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

unsafe extern "C" {}

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

unsafe extern "C" {}

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

unsafe extern "C" {}

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
    fn glaspen2_router_chat_integration() -> i32;
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

/// 录制热键的按键身份(与 autorepeat 消费共用; R 常开, 2/3 需聊天集成)。
fn recording_hotkey_key(key_code: u16, has_cmd: bool, has_ctrl: bool, chat: bool) -> bool {
    if !has_cmd || !has_ctrl {
        return false;
    }
    match key_code {
        15 => true,
        19 | 20 => chat,
        _ => false,
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
    fn glaspen2_router_prev_color_index() -> i32;
    fn glaspen2_router_selected_color_index() -> i32;
    fn glaspen2_color_preset_count() -> i32;
    fn glaspen2_router_prev_width_index() -> i32;
    fn glaspen2_router_selected_width_index() -> i32;
    fn glaspen2_width_preset_count() -> i32;
    fn glaspen2_camera_keyboard_pan_step(zoom: f64) -> f64;
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
fn camera_dir_for_kind(kind: &str) -> Option<CameraDir> {
    match kind {
        "CameraPanLeft" => Some(CameraDir::Left),
        "CameraPanRight" => Some(CameraDir::Right),
        "CameraPanUp" => Some(CameraDir::Up),
        "CameraPanDown" => Some(CameraDir::Down),
        _ => None,
    }
}

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
        let Some(dir) = camera_dir_for_kind(kind) else {
            return 1;
        };
        // 方向约定单源在 core: 按键方向 = 内容移动方向。
        let (dx, dy) = Camera::keyboard_pan_delta(dir, step);
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

// ── R77: macOS 虚拟键码 -> chord 键名 ──

fn keycode_name(key_code: u16) -> Option<&'static str> {
    Some(match key_code {
        0x00 => "A",
        0x01 => "S",
        0x02 => "D",
        0x03 => "F",
        0x04 => "H",
        0x05 => "G",
        0x06 => "Z",
        0x07 => "X",
        0x08 => "C",
        0x09 => "V",
        0x0B => "B",
        0x0C => "Q",
        0x0D => "W",
        0x0E => "E",
        0x0F => "R",
        0x10 => "Y",
        0x11 => "T",
        0x12 => "1",
        0x13 => "2",
        0x14 => "3",
        0x15 => "4",
        0x16 => "6",
        0x17 => "5",
        0x18 => "=",
        0x19 => "9",
        0x1A => "7",
        0x1B => "-",
        0x1C => "8",
        0x1D => "0",
        0x1E => "]",
        0x1F => "O",
        0x20 => "U",
        0x21 => "[",
        0x22 => "I",
        0x23 => "P",
        0x24 => "Return",
        0x25 => "L",
        0x26 => "J",
        0x27 => "'",
        0x28 => "K",
        0x29 => ";",
        0x2A => "\\",
        0x2B => ",",
        0x2C => "/",
        0x2D => "N",
        0x2E => "M",
        0x2F => ".",
        0x30 => "Tab",
        0x31 => "Space",
        0x32 => "`",
        0x33 => "Backspace",
        0x35 => "Escape",
        0x60 => "F5",
        0x61 => "F6",
        0x62 => "F7",
        0x63 => "F3",
        0x64 => "F8",
        0x65 => "F9",
        0x67 => "F11",
        0x6D => "F10",
        0x6F => "F12",
        0x73 => "Home",
        0x74 => "PageUp",
        0x75 => "Delete",
        0x76 => "F4",
        0x77 => "End",
        0x78 => "F2",
        0x79 => "PageDown",
        0x7A => "F1",
        0x7B => "Left",
        0x7C => "Right",
        0x7D => "Down",
        0x7E => "Up",
        _ => return None,
    })
}

/// R77: 返回堆上键名(调用方 free); 未知键码返回 null。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_keycode_name(key_code: u16) -> *mut std::os::raw::c_char {
    match keycode_name(key_code) {
        Some(name) => std::ffi::CString::new(name).map_or(std::ptr::null_mut(), |s| s.into_raw()),
        None => std::ptr::null_mut(),
    }
}

// ── R79: 时光隧道卡片绘制(opt-in) ──

unsafe extern "C" {
    fn cairo_rectangle(cr: *mut c_void, x: f64, y: f64, w: f64, h: f64);
    fn cairo_clip(cr: *mut c_void);
    fn cairo_translate(cr: *mut c_void, x: f64, y: f64);
    fn cairo_scale(cr: *mut c_void, sx: f64, sy: f64);
    fn cairo_set_source_surface(cr: *mut c_void, surface: *mut c_void, x: f64, y: f64);
    fn cairo_paint_with_alpha(cr: *mut c_void, alpha: f64);
    fn cairo_paint(cr: *mut c_void);
    fn cairo_pattern_create_linear(x0: f64, y0: f64, x1: f64, y1: f64) -> *mut c_void;
    fn cairo_pattern_add_color_stop_rgba(
        pattern: *mut c_void,
        offset: f64,
        r: f64,
        g: f64,
        b: f64,
        a: f64,
    );
    fn cairo_close_path(cr: *mut c_void);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_draw_tunnel_card(
    cr: *mut c_void,
    surf: *mut c_void,
    nw: i32,
    nh: i32,
    cx: f64,
    cy: f64,
    w: f64,
    h: f64,
    alpha: f64,
    zc: f64,
    surf_w: i32,
    surf_h: i32,
    thick: f64,
    pane_a: f64,
) {
    if alpha <= 0.02 || w < 1.0 || h < 1.0 || nw <= 0 || nh <= 0 || cr.is_null() || surf.is_null() {
        return;
    }
    let card_x = cx - w / 2.0;
    let card_y = cy - h / 2.0;
    unsafe {
        if zc.abs() < 0.005 {
            cairo_save(cr);
            cairo_rectangle(cr, 0.0, 0.0, surf_w as f64, surf_h as f64);
            cairo_clip(cr);
            cairo_translate(cr, card_x, card_y);
            cairo_scale(cr, w / nw as f64, h / nh as f64);
            cairo_set_source_surface(cr, surf, 0.0, 0.0);
            cairo_paint_with_alpha(cr, 1.0);
            cairo_restore(cr);
            return;
        }
        let mut glass = zc.abs() / 0.10;
        if glass > 1.0 {
            glass = 1.0;
        }
        let t = (w * thick).max(4.0);
        let soff = (w * 0.008).max(3.0);
        let mut zpos = if zc > 0.0 { zc } else { 0.0 };
        if zpos > 4.0 {
            zpos = 4.0;
        }
        let mut pane = (pane_a + 0.02 * zpos) * glass;
        if zc < 0.0 {
            pane *= alpha;
        }
        if pane > 0.94 {
            pane = 0.94;
        }
        if zc.abs() < 1.6 {
            for i in (1..=2).rev() {
                let off = i as f64 * soff * 1.6;
                cairo_set_source_rgba(cr, 0.0, 0.0, 0.0, 0.028 * alpha * glass);
                cairo_rectangle(cr, card_x + off, card_y + off, w, h);
                cairo_fill(cr);
            }
        }
        // 上侧面
        let top = cairo_pattern_create_linear(card_x, card_y, card_x + t, card_y - t);
        if !top.is_null() {
            cairo_pattern_add_color_stop_rgba(top, 0.0, 1.0, 1.0, 1.0, 0.74 * alpha * glass);
            cairo_pattern_add_color_stop_rgba(top, 1.0, 0.90, 0.91, 0.92, 0.52 * alpha * glass);
            cairo_set_source(cr, top);
            cairo_move_to(cr, card_x, card_y);
            cairo_line_to(cr, card_x + t, card_y - t);
            cairo_line_to(cr, card_x + w + t, card_y - t);
            cairo_line_to(cr, card_x + w, card_y);
            cairo_close_path(cr);
            cairo_fill(cr);
            cairo_pattern_destroy(top);
        }
        // 右侧面
        let right = cairo_pattern_create_linear(card_x + w, card_y, card_x + w + t, card_y - t);
        if !right.is_null() {
            cairo_pattern_add_color_stop_rgba(right, 0.0, 0.70, 0.72, 0.74, 0.62 * alpha * glass);
            cairo_pattern_add_color_stop_rgba(right, 1.0, 0.28, 0.30, 0.33, 0.74 * alpha * glass);
            cairo_set_source(cr, right);
            cairo_move_to(cr, card_x + w, card_y);
            cairo_line_to(cr, card_x + w + t, card_y - t);
            cairo_line_to(cr, card_x + w + t, card_y + h - t);
            cairo_line_to(cr, card_x + w, card_y + h);
            cairo_close_path(cr);
            cairo_fill(cr);
            cairo_pattern_destroy(right);
        }
        // 前脸
        cairo_save(cr);
        cairo_rectangle(cr, card_x, card_y, w, h);
        cairo_clip(cr);
        let fg = cairo_pattern_create_linear(card_x, card_y, card_x, card_y + h);
        if !fg.is_null() {
            cairo_pattern_add_color_stop_rgba(fg, 0.0, 0.985, 0.99, 1.0, pane * 0.99);
            cairo_pattern_add_color_stop_rgba(fg, 0.55, 0.86, 0.875, 0.89, pane * 0.94);
            cairo_pattern_add_color_stop_rgba(fg, 1.0, 0.66, 0.675, 0.69, pane * 0.98);
            cairo_set_source(cr, fg);
            cairo_paint(cr);
            cairo_pattern_destroy(fg);
        }
        cairo_save(cr);
        cairo_translate(cr, card_x, card_y);
        cairo_scale(cr, w / nw as f64, h / nh as f64);
        cairo_set_source_surface(cr, surf, 0.0, 0.0);
        cairo_paint_with_alpha(cr, alpha);
        cairo_restore(cr);
        if zc.abs() < 1.2 {
            let sheen = cairo_pattern_create_linear(card_x, card_y, card_x + w, card_y + h * 0.75);
            if !sheen.is_null() {
                cairo_pattern_add_color_stop_rgba(sheen, 0.0, 1.0, 1.0, 1.0, 0.10 * alpha * glass);
                cairo_pattern_add_color_stop_rgba(
                    sheen,
                    0.35,
                    1.0,
                    1.0,
                    1.0,
                    0.012 * alpha * glass,
                );
                cairo_pattern_add_color_stop_rgba(sheen, 1.0, 1.0, 1.0, 1.0, 0.0);
                cairo_set_source(cr, sheen);
                cairo_paint(cr);
                cairo_pattern_destroy(sheen);
            }
        }
        cairo_restore(cr);
        // 棱线
        cairo_save(cr);
        cairo_set_line_width(cr, (t * 0.08).max(1.0));
        let edges = [
            (
                1.0,
                1.0,
                1.0,
                0.42,
                (card_x + t, card_y - t),
                (card_x + w + t, card_y - t),
            ),
            (
                0.03,
                0.03,
                0.035,
                0.48,
                (card_x + w + t, card_y - t),
                (card_x + w + t, card_y + h - t),
            ),
            (1.0, 1.0, 1.0, 0.50, (card_x, card_y), (card_x + w, card_y)),
            (
                0.04,
                0.04,
                0.045,
                0.38,
                (card_x + w, card_y),
                (card_x + w, card_y + h),
            ),
            (
                0.9,
                0.92,
                0.94,
                0.18,
                (card_x, card_y),
                (card_x, card_y + h),
            ),
            (
                0.05,
                0.05,
                0.055,
                0.20,
                (card_x, card_y + h),
                (card_x + w, card_y + h),
            ),
        ];
        for (r, g, b, a, from, to) in edges {
            cairo_set_source_rgba(cr, r, g, b, a * alpha * glass);
            cairo_move_to(cr, from.0, from.1);
            cairo_line_to(cr, to.0, to.1);
            cairo_stroke(cr);
        }
        cairo_restore(cr);
    }
}

// ── R80: 玻璃视图 alpha/hidden 应用 ──

unsafe extern "C" {}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_apply_glass_view(alpha: f64, hidden: i32) {
    use objc2_app_kit::NSVisualEffectView;
    let p = unsafe { sw_glass_view() };
    if p.is_null() {
        return;
    }
    let view = unsafe { &*(p as *const NSVisualEffectView) };
    view.setAlphaValue(alpha);
    view.setHidden(hidden != 0);
}

// ── R83: 时光隧道逐帧编排(排序/相机/落定帧/模糊缓冲/近层) ──

unsafe extern "C" {
    fn glaspen2_tunnel_place(
        zc: f64,
        k: f64,
        travel: f64,
        surf_w: f64,
        surf_h: f64,
        out: *mut f64,
    ) -> i32;
    fn cairo_create(surface: *mut c_void) -> *mut c_void;
    fn cairo_get_source(cr: *mut c_void) -> *mut c_void;
    fn cairo_pattern_set_filter(pattern: *mut c_void, filter: i32);
}

fn tunnel_place(zc: f64, k: f64, travel: f64, surf_w: i32, surf_h: i32) -> Option<[f64; 5]> {
    let mut out = [0.0f64; 5];
    let ok = unsafe {
        glaspen2_tunnel_place(
            zc,
            k,
            travel,
            surf_w as f64,
            surf_h as f64,
            out.as_mut_ptr(),
        )
    };
    if ok != 0 { Some(out) } else { None }
}

fn tunnel_draw_card(
    cr: *mut c_void,
    placed: [f64; 5],
    main: *mut c_void,
    mip: *mut c_void,
    thick: f64,
    pane_a: f64,
    surf_w: i32,
    surf_h: i32,
    zc: f64,
) {
    if main.is_null() {
        return;
    }
    let main_w = unsafe { cairo_image_surface_get_width(main) } as f64;
    let surf = if !mip.is_null() && placed[2] < main_w * 0.70 {
        mip
    } else {
        main
    };
    let nw = unsafe { cairo_image_surface_get_width(surf) };
    let nh = unsafe { cairo_image_surface_get_height(surf) };
    glaspen2_rust_draw_tunnel_card(
        cr, surf, nw, nh, placed[0], placed[1], placed[2], placed[3], placed[4], zc, surf_w,
        surf_h, thick, pane_a,
    );
}

/// R83: 逐帧渲染; 返回 1 有落定帧(提前结束), 0 正常帧/失败。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_tunnel_render(
    p: f64,
    surface: *mut c_void,
    blur: *mut c_void,
    cards: i32,
    depths: *const i32,
    card_surf: *const *mut c_void,
    mip_surf: *const *mut c_void,
    back: i32,
    travel: f64,
    k: f64,
    thick: f64,
    pane_a: f64,
    surf_w: i32,
    surf_h: i32,
) -> i32 {
    if surface.is_null()
        || cards <= 0
        || depths.is_null()
        || card_surf.is_null()
        || mip_surf.is_null()
    {
        return 0;
    }
    let cr = unsafe { cairo_create(surface) };
    if cr.is_null() {
        return 0;
    }
    unsafe {
        cairo_set_operator(cr, CAIRO_OPERATOR_SOURCE);
        cairo_set_source_rgba(cr, 0.0, 0.0, 0.0, 0.0);
        cairo_paint(cr);
        cairo_set_operator(cr, CAIRO_OPERATOR_OVER);
    }
    let p = p.clamp(0.0, 1.0);
    let e = if p < 0.5 {
        4.0 * p * p * p
    } else {
        1.0 - (-2.0 * p + 2.0).powi(3) / 2.0
    };
    let cam = e * travel * if back != 0 { 1.0 } else { -1.0 };
    let mut slots: Vec<i32> = (0..cards).collect();
    slots.sort_by(|a, b| {
        let da = unsafe { *depths.add(*a as usize) };
        let db = unsafe { *depths.add(*b as usize) };
        db.cmp(&da)
    });
    for slot in &slots {
        let zc = unsafe { *depths.add(*slot as usize) } as f64 - cam;
        if zc.abs() < 0.005 {
            if let Some(placed) = tunnel_place(zc, k, travel, surf_w, surf_h) {
                let main = unsafe { *card_surf.add(*slot as usize) };
                let mip = unsafe { *mip_surf.add(*slot as usize) };
                tunnel_draw_card(cr, placed, main, mip, thick, pane_a, surf_w, surf_h, zc);
                unsafe { cairo_destroy(cr) };
                unsafe { glaspen2_swift_tunnel_flush() };
                return 1;
            }
        }
    }
    let mut focus_zc = 1e9f64;
    for slot in &slots {
        let zc = unsafe { *depths.add(*slot as usize) } as f64 - cam;
        if let Some(placed) = tunnel_place(zc, k, travel, surf_w, surf_h) {
            if placed[4] >= 0.30 && zc < focus_zc {
                focus_zc = zc;
            }
        }
    }
    if focus_zc > 1e8 {
        focus_zc = 0.0;
    }
    if !blur.is_null() {
        let bw = unsafe { cairo_image_surface_get_width(blur) };
        let bh = unsafe { cairo_image_surface_get_height(blur) };
        let bcr = unsafe { cairo_create(blur) };
        if !bcr.is_null() {
            unsafe {
                cairo_set_operator(bcr, CAIRO_OPERATOR_SOURCE);
                cairo_set_source_rgba(bcr, 0.0, 0.0, 0.0, 0.0);
                cairo_paint(bcr);
                cairo_set_operator(bcr, CAIRO_OPERATOR_OVER);
                cairo_scale(bcr, bw as f64 / surf_w as f64, bh as f64 / surf_h as f64);
            }
            for slot in &slots {
                let zc = unsafe { *depths.add(*slot as usize) } as f64 - cam;
                if zc <= focus_zc + 1e-6 {
                    continue;
                }
                if let Some(placed) = tunnel_place(zc, k, travel, surf_w, surf_h) {
                    let main = unsafe { *card_surf.add(*slot as usize) };
                    let mip = unsafe { *mip_surf.add(*slot as usize) };
                    tunnel_draw_card(bcr, placed, main, mip, thick, pane_a, surf_w, surf_h, zc);
                }
            }
            unsafe { cairo_destroy(bcr) };
            unsafe {
                cairo_save(cr);
                cairo_scale(cr, surf_w as f64 / bw as f64, surf_h as f64 / bh as f64);
                cairo_set_source_surface(cr, blur, 0.0, 0.0);
                let src = cairo_get_source(cr);
                if !src.is_null() {
                    cairo_pattern_set_filter(src, 2);
                }
                cairo_paint(cr);
                cairo_restore(cr);
            }
        }
    }
    for slot in &slots {
        let zc = unsafe { *depths.add(*slot as usize) } as f64 - cam;
        if !blur.is_null() && zc > focus_zc + 1e-6 {
            continue;
        }
        if let Some(placed) = tunnel_place(zc, k, travel, surf_w, surf_h) {
            let main = unsafe { *card_surf.add(*slot as usize) };
            let mip = unsafe { *mip_surf.add(*slot as usize) };
            tunnel_draw_card(cr, placed, main, mip, thick, pane_a, surf_w, surf_h, zc);
        }
    }
    unsafe { cairo_destroy(cr) };
    unsafe { glaspen2_swift_tunnel_flush() };
    0
}

// ── R86: 隧道卡片/缓冲/表面释放 ──

unsafe extern "C" {
    fn cairo_surface_destroy(surface: *mut c_void);
}

unsafe extern "C" {
    fn glaspen2_swift_tunnel_clear_state();
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_tunnel_free_cards(
    count: i32,
    card_surf: *const *mut c_void,
    mip_surf: *const *mut c_void,
    blur: *mut c_void,
    surface: *mut c_void,
) {
    if !card_surf.is_null() && !mip_surf.is_null() {
        for i in 0..count.max(0) as usize {
            let main = unsafe { *card_surf.add(i) };
            if !main.is_null() {
                unsafe { cairo_surface_destroy(main) };
            }
            let mip = unsafe { *mip_surf.add(i) };
            if !mip.is_null() {
                unsafe { cairo_surface_destroy(mip) };
            }
        }
    }
    if !blur.is_null() {
        unsafe { cairo_surface_destroy(blur) };
    }
    if !surface.is_null() {
        unsafe { cairo_surface_destroy(surface) };
    }
    tracing::info!("rust-shell: tunnel cards freed count={count}");
    unsafe { glaspen2_swift_tunnel_clear_state() };
}

// ── R88: 隧道收尾(cancel timer/final frame/FPS/free/glass) ──

unsafe extern "C" {
    fn glaspen2_swift_tunnel_cancel_timer();
    fn glaspen2_swift_tunnel_mark_inactive();
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_tunnel_finish(
    t0: f64,
    now: f64,
    frames: u64,
    surface: *mut c_void,
    blur: *mut c_void,
    cards: i32,
    depths: *const i32,
    card_surf: *const *mut c_void,
    mip_surf: *const *mut c_void,
    back: i32,
    travel: f64,
    k: f64,
    thick: f64,
    pane_a: f64,
    surf_w: i32,
    surf_h: i32,
) {
    unsafe { glaspen2_swift_tunnel_cancel_timer() };
    if !surface.is_null() && cards > 0 {
        let _ = glaspen2_rust_tunnel_render(
            1.0, surface, blur, cards, depths, card_surf, mip_surf, back, travel, k, thick, pane_a,
            surf_w, surf_h,
        );
    }
    if std::env::var_os("GLASPEN2_FLIP_FPS").is_some() {
        let secs = now - t0;
        let fps = if secs > 0.001 {
            frames as f64 / secs
        } else {
            0.0
        };
        tracing::info!("[flip] 实测 {fps:.1} fps ({frames} 帧 / {secs:.3}s)");
    }
    glaspen2_rust_tunnel_free_cards(cards, card_surf, mip_surf, blur, surface);
    unsafe { glaspen2_swift_tunnel_mark_inactive() };
    glaspen2_rust_apply_glass();
}

// ── R89: 设置 effect 分派(常用键; 未覆盖回退 Swift) ──

unsafe extern "C" {
    fn glaspen2_settings_apply(
        key: *const std::os::raw::c_char,
        value: *const std::os::raw::c_char,
    ) -> *mut std::os::raw::c_char;
    fn glaspen2_router_refresh_menu_texts();
    fn glaspen2_router_set_grid_size(v: f64);
    fn glaspen2_router_set_grid_divider(v: i32);
    fn glaspen2_router_set_minimap(on: i32);
    fn glaspen2_router_set_flip(v: i32);
    fn glaspen2_router_gif_fps() -> i32;
    fn glaspen2_router_gif_resolution() -> f64;
    fn glaspen2_router_gif_speed() -> f64;
    fn glaspen2_router_gif_end_mode() -> i32;
    fn glaspen2_router_set_gif(fps: i32, resolution: f64, speed: f64, end_mode: i32);
    fn glaspen2_router_set_soft_shadow_flag(on: i32);
    fn glaspen2_set_soft_shadow(on: i32);
    fn glaspen2_router_invert_set_ink(on: i32);
    fn glaspen2_router_invert_apply(on: i32);
    fn glaspen2_router_invert_set_fps(v: i32);
    fn glaspen2_router_invert_enabled() -> i32;
    fn glaspen2_router_invert_restart();
    fn glaspen2_router_set_grid_follow(on: i32);
    fn glaspen2_router_set_glass_follow(on: i32);
    fn glaspen2_bubble_set_capture(on: i32);
    fn glaspen2_router_startup_set_lang(v: i32);
    fn glaspen2_save_string_setting(
        key: *const std::os::raw::c_char,
        value: *const std::os::raw::c_char,
    );
    fn glaspen2_chat_auth_reload();
}

fn cstr_setting(text: &str) -> std::ffi::CString {
    std::ffi::CString::new(text).unwrap_or_default()
}

/// R89: 应用一个设置; 返回 1 已处理, 0 回退 Swift。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_apply_setting(
    key: *const std::os::raw::c_char,
    value: *const std::os::raw::c_char,
) -> i32 {
    if key.is_null() || value.is_null() {
        return 0;
    }
    let effect_raw = unsafe { glaspen2_settings_apply(key, value) };
    if effect_raw.is_null() {
        return 0;
    }
    let text = unsafe { std::ffi::CStr::from_ptr(effect_raw) }
        .to_string_lossy()
        .into_owned();
    unsafe { glaspen2_free_c_string(effect_raw) };
    let Ok(effect) = serde_json::from_str::<serde_json::Value>(&text) else {
        return 0;
    };
    let Some(kind) = effect.get("kind").and_then(|k| k.as_str()) else {
        return 0;
    };
    let b = effect
        .get("bool")
        .and_then(|x| x.as_bool())
        .unwrap_or(false);
    let iv = effect.get("int").and_then(|x| x.as_i64()).unwrap_or(0) as i32;
    let fv = effect.get("float").and_then(|x| x.as_f64()).unwrap_or(0.0);
    let sv = effect.get("str").and_then(|x| x.as_str()).unwrap_or("");
    let ignored = [
        "BubbleAutoNewPage",
        "BubbleReplayAuto",
        "BubbleReplayHoldLast",
        "BubbleReplaySpeed",
        "NasUrl",
        "NotebookStyle",
        "ShowFreeCanvas",
    ];
    if ignored.contains(&kind) {
        return 1;
    }
    match kind {
        "Color" => {
            let _ = glaspen2_rust_select_color(iv);
            1
        }
        "Width" => {
            let _ = glaspen2_rust_select_width(iv);
            1
        }
        "LaunchAtLogin" => {
            glaspen2_rust_set_launch_at_login(i32::from(b));
            1
        }
        "FrostedGlass" => {
            glaspen2_rust_set_glass_enabled(i32::from(b));
            1
        }
        "Grid" => {
            glaspen2_rust_set_grid_enabled(i32::from(b));
            1
        }
        "ChatIntegration" => {
            glaspen2_rust_set_chat_integration(i32::from(b));
            1
        }
        "PressureMonitor" => {
            glaspen2_rust_set_pressure_monitor(i32::from(b));
            1
        }
        "GridFollowStrokes" => {
            unsafe {
                glaspen2_router_set_grid_follow(i32::from(b));
                glaspen2_router_redraw_draw_view();
            }
            1
        }
        "GlassFollowStrokes" => {
            unsafe {
                glaspen2_router_set_glass_follow(i32::from(b));
            }
            glaspen2_rust_apply_glass();
            1
        }
        "SoftShadow" => {
            unsafe {
                glaspen2_router_set_soft_shadow_flag(i32::from(b));
                glaspen2_set_soft_shadow(i32::from(b));
                glaspen2_router_redraw_draw_view();
            }
            1
        }
        "InvertInk" => {
            unsafe {
                glaspen2_router_invert_set_ink(i32::from(b));
                glaspen2_router_invert_apply(i32::from(b));
            }
            1
        }
        "InvertFps" => {
            unsafe {
                glaspen2_router_invert_set_fps(iv);
                if glaspen2_router_invert_enabled() != 0 {
                    glaspen2_router_invert_restart();
                }
            }
            1
        }
        "PanelTransparent" => {
            unsafe {
                glaspen2_router_panel_transparent_full(i32::from(b));
            }
            1
        }
        "Outline" => {
            unsafe {
                glaspen2_menu_apply_outline(i32::from(b));
            }
            1
        }
        "InfiniteCanvas" => {
            let _ = glaspen2_rust_apply_infinite(i32::from(b), std::ptr::null());
            1
        }
        "Minimap" => {
            unsafe {
                glaspen2_router_set_minimap(i32::from(b));
                glaspen2_router_redraw_draw_view();
            }
            1
        }
        "GridSize" => {
            unsafe {
                glaspen2_router_set_grid_size(fv);
                glaspen2_router_redraw_draw_view();
            }
            1
        }
        "GridDivider" => {
            unsafe {
                glaspen2_router_set_grid_divider(iv);
                glaspen2_router_redraw_draw_view();
            }
            1
        }
        "FlipEffect" => {
            unsafe {
                glaspen2_router_set_flip(iv);
            }
            1
        }
        "GifFps" | "GifResolution" | "GifSpeed" | "GifEndMode" => {
            let (mut fps, mut res, mut speed, mut end_mode) = unsafe {
                (
                    glaspen2_router_gif_fps(),
                    glaspen2_router_gif_resolution(),
                    glaspen2_router_gif_speed(),
                    glaspen2_router_gif_end_mode(),
                )
            };
            match kind {
                "GifFps" => fps = iv,
                "GifResolution" => res = fv,
                "GifSpeed" => speed = fv,
                _ => end_mode = iv,
            }
            unsafe { glaspen2_router_set_gif(fps, res, speed, end_mode) };
            1
        }
        "ChatApiBase" | "ChatUser" | "ChatPassword" => {
            if kind == "ChatPassword" && !sv.is_empty() {
                let k = cstr_setting("chat_password");
                let v = cstr_setting(sv);
                unsafe {
                    glaspen2_save_string_setting(k.as_ptr(), v.as_ptr());
                }
            }
            unsafe {
                glaspen2_chat_auth_reload();
            }
            1
        }
        "ShowBubbleTab" => {
            unsafe {
                glaspen2_bubble_set_capture(i32::from(b));
            }
            1
        }
        "ShareCanvas" => {
            let chat = unsafe {
                let k = cstr_setting("chat_integration");
                glaspen2_load_bool_setting(k.as_ptr()) != 0
            };
            if chat || !b {
                unsafe {
                    glaspen2_share_ink_set_active(i32::from(chat && b));
                }
            }
            1
        }
        "Language" => {
            unsafe {
                glaspen2_router_startup_set_lang(i32::from(sv == "en"));
                glaspen2_router_refresh_menu_texts();
                glaspen2_notify_settings_changed();
            }
            1
        }
        "HotkeyRecording" => {
            unsafe {
                glaspen2_router_hotkey_recording(i32::from(b));
            }
            1
        }
        "Shortcuts" => {
            let saved = effect
                .get("json")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            let text = saved.to_string();
            let Ok(cs) = std::ffi::CString::new(text) else {
                return 1;
            };
            unsafe {
                glaspen2_shortcut_apply_json(cs.as_ptr());
            }
            let values = unsafe { glaspen2_shortcut_store_values_json() };
            if !values.is_null() {
                let payload = unsafe { std::ffi::CStr::from_ptr(values) }
                    .to_string_lossy()
                    .into_owned();
                unsafe { glaspen2_free_c_string(values) };
                let k = cstr_setting("shortcuts");
                let v = cstr_setting(&payload);
                unsafe {
                    glaspen2_save_string_setting(k.as_ptr(), v.as_ptr());
                }
            }
            unsafe {
                glaspen2_notify_settings_changed();
            }
            1
        }
        _ => {
            tracing::warn!("settings: 未处理的 effect kind={kind}");
            1
        }
    }
}

// ── R90: 快照观看页框(页外压暗/页边界线) ──

unsafe extern "C" {
    fn cairo_set_fill_rule(cr: *mut c_void, rule: i32);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_draw_page_frame(
    surface: *mut c_void,
    scale: f64,
    ox: f64,
    oy: f64,
    pscale: f64,
    page_w: i32,
    page_h: i32,
    is_fit: i32,
) -> i32 {
    if surface.is_null() || is_fit == 0 || page_w <= 0 || page_h <= 0 {
        return 0;
    }
    let cr = unsafe { cairo_create(surface) };
    if cr.is_null() {
        return 0;
    }
    let x = ox * scale;
    let y = oy * scale;
    let w = page_w as f64 * pscale * scale;
    let h = page_h as f64 * pscale * scale;
    unsafe {
        cairo_set_source_rgba(cr, 0.0, 0.0, 0.0, 0.42);
        cairo_set_fill_rule(cr, 1);
        cairo_rectangle(
            cr,
            0.0,
            0.0,
            cairo_image_surface_get_width(surface) as f64,
            cairo_image_surface_get_height(surface) as f64,
        );
        cairo_rectangle(cr, x, y, w, h);
        cairo_fill(cr);
        cairo_set_source_rgba(cr, 1.0, 1.0, 1.0, 0.55);
        cairo_set_line_width(cr, 1.0);
        cairo_rectangle(cr, x + 0.5, y + 0.5, w - 1.0, h - 1.0);
        cairo_stroke(cr);
        cairo_destroy(cr);
    }
    1
}

// ── R91: 整页重建/重放编排(rebuild view + 页框 + flush + dirty + overlay flush) ──

unsafe extern "C" {
    fn glaspen2_draw_rebuild_view(surface: *mut c_void, scale: f64, ox: f64, oy: f64, pscale: f64);
    fn cairo_surface_flush(surface: *mut c_void);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_rebuild_surface(
    surface: *mut c_void,
    scale: f64,
    ox: f64,
    oy: f64,
    pscale: f64,
    draw_page_frame: i32,
) -> i32 {
    if surface.is_null() {
        return 0;
    }
    unsafe { glaspen2_draw_rebuild_view(surface, scale, ox, oy, pscale) };
    if draw_page_frame != 0 {
        let _ = glaspen2_rust_draw_page_frame(
            surface,
            scale,
            ox,
            oy,
            pscale,
            glaspen2_rust_pageview_page_w(),
            glaspen2_rust_pageview_page_h(),
            glaspen2_rust_pageview_is_fit(),
        );
    }
    unsafe {
        cairo_surface_flush(surface);
        glaspen2_swift_overlay_flush();
    }
    glaspen2_rust_dirty_reset();
    1
}

// ── R92: 普通 chord 门控(动作查找/状态/重复键/执行) ──

unsafe extern "C" {
    fn glaspen2_shortcut_store_action_for_chord(
        chord: *const std::os::raw::c_char,
    ) -> *mut std::os::raw::c_char;
}

/// R92: 返回 1 放行 / 2 消费。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_handle_chord(
    key_code: u16,
    has_cmd: i32,
    has_ctrl: i32,
    has_alt: i32,
    has_shift: i32,
    is_repeat: i32,
) -> i32 {
    let Some(key) = keycode_name(key_code) else {
        return 1;
    };
    let ctrl = has_ctrl != 0;
    let alt = has_alt != 0;
    if !ctrl && !alt {
        return 1;
    }
    let mut parts: Vec<&str> = Vec::new();
    if has_cmd != 0 {
        parts.push("cmd");
    }
    if ctrl {
        parts.push("ctrl");
    }
    if alt {
        parts.push("alt");
    }
    if has_shift != 0 {
        parts.push("shift");
    }
    parts.push(key);
    let chord = parts.join("+");
    let Ok(cchord) = std::ffi::CString::new(chord) else {
        return 1;
    };
    let action_raw = unsafe { glaspen2_shortcut_store_action_for_chord(cchord.as_ptr()) };
    if action_raw.is_null() {
        return 1;
    }
    let action = unsafe { std::ffi::CStr::from_ptr(action_raw) }
        .to_string_lossy()
        .into_owned();
    unsafe { glaspen2_free_c_string(action_raw) };
    let st = unsafe { glaspen2_router_state() };
    if (st & 1) == 0
        && action != "toggle_draw"
        && action != "toggle_free_canvas"
        && action != "quit"
    {
        return 1;
    }
    if action.starts_with("canvas_") && ((st & 4) == 0 || (st & 2) != 0) {
        return 1;
    }
    let repeatable = action.starts_with("canvas_pan_")
        || action == "canvas_zoom_in"
        || action == "canvas_zoom_out";
    if is_repeat == 0 || repeatable {
        let Ok(caction) = std::ffi::CString::new(action) else {
            return 2;
        };
        let _ = glaspen2_rust_perform_action(caction.as_ptr());
    }
    2
}

// ── R93: 相机鼠标抓手/滚轮缩放分发 ──

unsafe extern "C" {}

/// R93: kind 0=down,1=drag,2=up; 返回 1 已消费, 0 未处理。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_camera_mouse(kind: i32, x: f64, y: f64, mods: u64) -> i32 {
    let st = unsafe { glaspen2_router_state() };
    let dragging = glaspen2_router_camera_dragging() != 0;
    if dragging && (kind == 1 || kind == 2) {
        if kind == 1 {
            if (st & 4) != 0 && (st & 1) != 0 && (st & 2) == 0 {
                let z = unsafe { glaspen2_router_camera_step_zoom() };
                // AppKit locationInWindow 是左下原点; 先换成屏幕"右/下为正"
                // 再交 core 的统一方向函数(内容跟随鼠标)。
                let (dx, dy) = Camera::drag_pan_from_screen_delta(
                    x - unsafe { glaspen2_router_camera_last_x() },
                    unsafe { glaspen2_router_camera_last_y() } - y,
                    z,
                );
                unsafe {
                    glaspen2_router_camera_queue_pan(dx, dy);
                    glaspen2_router_camera_set_last(x, y);
                }
            }
        } else {
            unsafe {
                glaspen2_router_camera_flush();
                glaspen2_router_clear_camera_dragging();
            }
        }
        return 1;
    }
    if (st & 4) != 0 && (st & 1) != 0 && (st & 2) == 0 && kind == 0 {
        let camera_mods = unsafe { glaspen2_router_camera_pan_mods() } as u64;
        if camera_mods != 0 && mods == camera_mods {
            unsafe { glaspen2_router_camera_start(x, y) };
            return 1;
        }
    }
    0
}

/// R93: 滚轮缩放; 返回 2 消费, 1 放行。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_scroll_zoom(
    dy: f64,
    precise: i32,
    x: f64,
    y: f64,
    mods: u64,
) -> i32 {
    let st = unsafe { glaspen2_router_state() };
    let camera_mods = unsafe { glaspen2_router_camera_mods() } as u64;
    if camera_mods != 0 && mods == camera_mods && (st & 1) != 0 && (st & 2) == 0 {
        let factor = if precise != 0 {
            (-dy * 0.0015).exp()
        } else if dy > 0.0 {
            1.1
        } else if dy < 0.0 {
            1.0 / 1.1
        } else {
            1.0
        };
        let h = unsafe { glaspen2_router_draw_height() };
        glaspen2_rust_camera_zoom_at(factor, x, h - y);
        return 2;
    }
    1
}

// ── R94: 清屏/保存涂鸦截图 ──

unsafe extern "C" {
    fn glaspen2_save_drawing(data: *mut u8, w: i32, h: i32, stride: i32);
    fn cairo_image_surface_get_stride(surface: *mut c_void) -> i32;
    fn cairo_image_surface_get_data(surface: *mut c_void) -> *mut u8;
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_clear_screen(
    surface: *mut c_void,
    scale: f64,
    ox: f64,
    oy: f64,
    pscale: f64,
    is_fit: i32,
) -> i32 {
    if surface.is_null() {
        return -1;
    }
    let cr = unsafe { cairo_create(surface) };
    if !cr.is_null() {
        unsafe {
            cairo_scale(cr, scale, scale);
            if is_fit != 0 {
                cairo_translate(cr, ox * scale, oy * scale);
                cairo_scale(cr, pscale, pscale);
            }
            cairo_set_operator(cr, CAIRO_OPERATOR_CLEAR);
            cairo_paint(cr);
            cairo_destroy(cr);
        }
    }
    let created = unsafe {
        glaspen2_clear_strokes(
            glaspen_core::shell_state::screen_w(),
            glaspen_core::shell_state::screen_h(),
        )
    };
    glaspen2_rust_pageview_update();
    unsafe { glaspen2_swift_overlay_flush() };
    created
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_save_drawing(surface: *mut c_void) -> i32 {
    if surface.is_null() {
        return 0;
    }
    unsafe {
        cairo_surface_flush(surface);
        let w = cairo_image_surface_get_width(surface);
        let h = cairo_image_surface_get_height(surface);
        let stride = cairo_image_surface_get_stride(surface);
        let data = cairo_image_surface_get_data(surface);
        if data.is_null() || w <= 0 || h <= 0 {
            return 0;
        }
        glaspen2_save_drawing(data, w, h, stride);
    }
    1
}

// ── R95: 笔迹表面指针/scale 状态 ──

static SURFACE_PTR: std::sync::OnceLock<std::sync::Mutex<Option<usize>>> =
    std::sync::OnceLock::new();
static SURFACE_SCALE: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1.0f64.to_bits());

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_surface_current() -> *mut c_void {
    let slot = SURFACE_PTR.get_or_init(|| std::sync::Mutex::new(None));
    slot.lock()
        .unwrap_or_else(|e| e.into_inner())
        .map_or(std::ptr::null_mut(), |p| p as *mut c_void)
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_surface_set(surface: *mut c_void) {
    let slot = SURFACE_PTR.get_or_init(|| std::sync::Mutex::new(None));
    *slot.lock().unwrap_or_else(|e| e.into_inner()) = if surface.is_null() {
        None
    } else {
        Some(surface as usize)
    };
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_surface_scale() -> f64 {
    f64::from_bits(SURFACE_SCALE.load(std::sync::atomic::Ordering::Relaxed))
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_surface_set_scale(scale: f64) {
    SURFACE_SCALE.store(scale.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

// ── R96: 笔迹表面 ensure(创建/尺寸缓存/透明填充) ──

unsafe extern "C" {
    fn cairo_image_surface_create(format: i32, w: i32, h: i32) -> *mut c_void;
    fn cairo_surface_status(surface: *mut c_void) -> i32;
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_surface_ensure(view: *mut c_void) -> i32 {
    use objc2_app_kit::NSView;
    use std::sync::atomic::Ordering;
    if view.is_null() {
        return 0;
    }
    let view = unsafe { &*(view as *const NSView) };
    let bounds = view.bounds();
    let mut scale = view.window().map(|w| w.backingScaleFactor()).unwrap_or(1.0);
    if scale < 1.0 {
        scale = 1.0;
    }
    let w = (bounds.size.width * scale) as i32;
    let h = (bounds.size.height * scale) as i32;
    if w <= 0 || h <= 0 {
        return 0;
    }
    let current = glaspen2_rust_surface_current();
    let current_scale = glaspen2_rust_surface_scale();
    if !current.is_null()
        && unsafe { cairo_image_surface_get_width(current) } == w
        && unsafe { cairo_image_surface_get_height(current) } == h
        && (current_scale - scale).abs() < f64::EPSILON
    {
        return 1;
    }
    if !current.is_null() {
        unsafe {
            glaspen2_router_surface_cache_invalidate();
            cairo_surface_destroy(current);
        }
    }
    let surface = unsafe { cairo_image_surface_create(0, w, h) };
    if surface.is_null() || unsafe { cairo_surface_status(surface) } != 0 {
        if !surface.is_null() {
            unsafe { cairo_surface_destroy(surface) }
        }
        return 0;
    }
    glaspen2_rust_surface_set(surface);
    SURFACE_SCALE.store(scale.to_bits(), Ordering::Relaxed);
    unsafe {
        let cr = cairo_create(surface);
        if !cr.is_null() {
            cairo_set_operator(cr, CAIRO_OPERATOR_CLEAR);
            cairo_paint(cr);
            cairo_set_operator(cr, CAIRO_OPERATOR_OVER);
            cairo_destroy(cr);
        }
    }
    1
}

// ── R97: 整笔共享 cairo context 的 begin/end ──

unsafe extern "C" {}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_stroke_begin() {
    if glaspen2_router_active_ctx_present() != 0 {
        return;
    }
    let surface = glaspen2_rust_surface_current();
    if surface.is_null() {
        return;
    }
    let cr = unsafe { cairo_create(surface) };
    if cr.is_null() {
        return;
    }
    let scale = glaspen2_rust_surface_scale();
    unsafe {
        cairo_scale(cr, scale, scale);
        glaspen2_router_reset_raw_stroke();
    }
    glaspen2_router_install_active_ctx(cr);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_stroke_end() {
    let ctx = glaspen2_router_take_active_ctx();
    if !ctx.is_null() {
        unsafe { cairo_destroy(ctx) };
    }
}

// ── R98: 相机鼠标平移 16ms 合帧状态/flush ──

static PAN_PENDING_DX: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0.0f64.to_bits());
static PAN_PENDING_DY: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0.0f64.to_bits());
static PAN_SCHEDULED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static PAN_GENERATION: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_camera_pan_queue(dx: f64, dy: f64) -> i32 {
    use std::sync::atomic::Ordering;
    let old_dx = f64::from_bits(PAN_PENDING_DX.load(Ordering::Relaxed));
    let old_dy = f64::from_bits(PAN_PENDING_DY.load(Ordering::Relaxed));
    PAN_PENDING_DX.store((old_dx + dx).to_bits(), Ordering::Relaxed);
    PAN_PENDING_DY.store((old_dy + dy).to_bits(), Ordering::Relaxed);
    if PAN_SCHEDULED.swap(true, Ordering::Relaxed) {
        return 0;
    }
    PAN_GENERATION.fetch_add(1, Ordering::Relaxed);
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_camera_pan_flush() {
    use std::sync::atomic::Ordering;
    PAN_SCHEDULED.store(false, Ordering::Relaxed);
    PAN_GENERATION.fetch_add(1, Ordering::Relaxed);
    let dx = f64::from_bits(PAN_PENDING_DX.swap(0.0f64.to_bits(), Ordering::Relaxed));
    let dy = f64::from_bits(PAN_PENDING_DY.swap(0.0f64.to_bits(), Ordering::Relaxed));
    let st = unsafe { glaspen2_router_state() };
    if (dx != 0.0 || dy != 0.0) && (st & 4) != 0 && (st & 1) != 0 && (st & 2) == 0 {
        glaspen2_rust_camera_pan(dx, dy);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_camera_pan_cancel() {
    use std::sync::atomic::Ordering;
    PAN_PENDING_DX.store(0.0f64.to_bits(), Ordering::Relaxed);
    PAN_PENDING_DY.store(0.0f64.to_bits(), Ordering::Relaxed);
    PAN_SCHEDULED.store(false, Ordering::Relaxed);
    PAN_GENERATION.fetch_add(1, Ordering::Relaxed);
}

// ── R99: 快捷键录制状态/60s 超时 generation ──

static RECORDING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static RECORDING_GEN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_recording_set(on: i32) -> u32 {
    use std::sync::atomic::Ordering;
    RECORDING.store(on != 0, Ordering::Relaxed);
    let generation = RECORDING_GEN.fetch_add(1, Ordering::Relaxed) + 1;
    if on != 0 { generation } else { 0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_recording_is_active() -> i32 {
    i32::from(RECORDING.load(std::sync::atomic::Ordering::Relaxed))
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_recording_timeout(generation: u32) -> i32 {
    use std::sync::atomic::Ordering;
    if RECORDING_GEN.load(Ordering::Relaxed) != generation || !RECORDING.load(Ordering::Relaxed) {
        return 0;
    }
    RECORDING.store(false, Ordering::Relaxed);
    1
}

// ── R100: 显示器变更 2.5s 防抖状态/到期重读 ──

static DISPLAY_GEN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

unsafe extern "C" {}

/// R100: 变更入口; 返回 1 需要调度防抖计时, 0 忽略。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_display_begin(new_w: i32, new_h: i32) -> i32 {
    if new_w <= 0 || new_h <= 0 {
        return 0;
    }
    let (mut cur_w, mut cur_h) = (0i32, 0i32);
    unsafe { glaspen2_display_size(&mut cur_w, &mut cur_h) };
    if new_w == cur_w && new_h == cur_h {
        return 0;
    }
    DISPLAY_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    1
}

/// R100: 防抖到期; 重读真实几何, 变化才 apply; 返回 1 已应用。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_display_finish() -> i32 {
    let (real_w, real_h) = unsafe { (sw_app_screen_w(), sw_app_screen_h()) };
    if real_w <= 0 || real_h <= 0 {
        return 0;
    }
    let (mut cur_w, mut cur_h) = (0i32, 0i32);
    unsafe { glaspen2_display_size(&mut cur_w, &mut cur_h) };
    if real_w == cur_w && real_h == cur_h {
        return 0;
    }
    unsafe { glaspen2_display_apply(real_w, real_h) };
    1
}

// ── R102: 飘渺模式隐藏/恢复/预览显示 ──

unsafe extern "C" {
    fn glaspen2_router_set_strokes_visible(on: i32);
    fn glaspen2_swift_tunnel_active() -> i32;
}

fn ethereal_glass_hidden(hidden: bool) {
    use objc2_app_kit::NSVisualEffectView;
    let p = unsafe { sw_glass_view() };
    if p.is_null() {
        return;
    }
    let view = unsafe { &*(p as *const NSVisualEffectView) };
    view.setHidden(hidden);
}

fn ethereal_hide_strokes() {
    unsafe {
        glaspen2_router_finish_active_stroke();
        glaspen2_router_set_strokes_visible(0);
        if glaspen2_router_glass_follow() != 0 {
            ethereal_glass_hidden(true);
        }
        if glaspen2_router_pressure_monitor() != 0 {
            glaspen2_router_pm_hide();
        }
        force_redraw_draw_view_now();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_ethereal_hide_now() -> i32 {
    if unsafe { glaspen2_swift_tunnel_active() != 0 } {
        return 0;
    }
    if unsafe { glaspen2_router_ethereal() } == 0 {
        return 0;
    }
    if unsafe { glaspen2_router_strokes_visible() } == 0 {
        return 0;
    }
    ethereal_hide_strokes();
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_ethereal_restore_now() {
    if unsafe { glaspen2_router_ethereal() } == 0 {
        return;
    }
    unsafe { glaspen2_router_set_strokes_visible(1) };
    ethereal_hide_strokes();
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_ethereal_auto_show() {
    if unsafe { glaspen2_router_ethereal() } == 0 {
        return;
    }
    if unsafe { glaspen2_router_strokes_visible() } == 0 {
        unsafe {
            glaspen2_router_set_strokes_visible(1);
            glaspen2_router_apply_glass_view(
                if glaspen2_router_glass_enabled() != 0 {
                    glaspen2_router_glass_opacity() * 2.0
                } else {
                    0.0
                },
                0,
            );
            if glaspen2_router_pressure_monitor() != 0 {
                glaspen2_router_pm_show();
            }
            force_redraw_draw_view_now();
        }
        tracing::info!("[ethereal] 笔迹重现");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_ethereal_preview_show() -> i32 {
    if unsafe { glaspen2_router_ethereal() } == 0 {
        return 0;
    }
    if unsafe { glaspen2_router_strokes_visible() } == 0 {
        unsafe {
            glaspen2_router_set_strokes_visible(1);
            glaspen2_router_apply_glass_view(
                if glaspen2_router_glass_enabled() != 0 {
                    glaspen2_router_glass_opacity() * 2.0
                } else {
                    0.0
                },
                0,
            );
            if glaspen2_router_pressure_monitor() != 0 {
                glaspen2_router_pm_show();
            }
            force_redraw_draw_view_now();
        }
    }
    1
}

// ── R106: cairo 缩放上下文 / 退出收尾 ──

unsafe extern "C" {
    fn glaspen2_shutdown();
}

#[unsafe(no_mangle)]
pub extern "C" fn cairo_create_scaled() -> *mut c_void {
    let surface = glaspen2_rust_surface_current();
    if surface.is_null() {
        return std::ptr::null_mut();
    }
    let cr = unsafe { cairo_create(surface) };
    if cr.is_null() {
        return cr;
    }
    let scale = glaspen2_rust_surface_scale();
    unsafe {
        cairo_scale(cr, scale, scale);
        if glaspen2_rust_pageview_is_fit() != 0 {
            cairo_translate(
                cr,
                glaspen2_rust_pageview_paper_ox() * scale,
                glaspen2_rust_pageview_paper_oy() * scale,
            );
            let ps = glaspen2_rust_pageview_paper_scale();
            cairo_scale(cr, ps, ps);
        }
    }
    cr
}

#[unsafe(no_mangle)]
pub extern "C" fn save_and_exit(_sig: i32) {
    unsafe {
        glaspen2_shutdown();
        glaspen2_swift_pressure_destroy();
        glaspen2_swift_show_cursor();
    }
    std::process::exit(0);
}

// ── R107: 快捷键默认/动作 JSON 缓存 ──

unsafe extern "C" {
    fn glaspen2_chord_defaults_json() -> *mut std::os::raw::c_char;
    fn glaspen2_chord_actions_json() -> *mut std::os::raw::c_char;
}

static CHORD_DEFAULTS_CACHE: std::sync::OnceLock<std::sync::Mutex<Option<std::ffi::CString>>> =
    std::sync::OnceLock::new();
static CHORD_ACTIONS_CACHE: std::sync::OnceLock<std::sync::Mutex<Option<std::ffi::CString>>> =
    std::sync::OnceLock::new();

unsafe fn cache_chord_json(
    slot: &'static std::sync::OnceLock<std::sync::Mutex<Option<std::ffi::CString>>>,
    producer: unsafe extern "C" fn() -> *mut std::os::raw::c_char,
) -> *const std::os::raw::c_char {
    let cache = slot.get_or_init(|| std::sync::Mutex::new(None));
    let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        let raw = unsafe { producer() };
        if !raw.is_null() {
            let text = unsafe { std::ffi::CStr::from_ptr(raw) }.to_owned();
            unsafe { glaspen2_free_c_string(raw) };
            *guard = Some(text);
        }
    }
    guard.as_ref().map_or(std::ptr::null(), |s| s.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_shortcut_defaults_json() -> *const std::os::raw::c_char {
    unsafe { cache_chord_json(&CHORD_DEFAULTS_CACHE, glaspen2_chord_defaults_json) }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_shortcut_actions_json() -> *const std::os::raw::c_char {
    unsafe { cache_chord_json(&CHORD_ACTIONS_CACHE, glaspen2_chord_actions_json) }
}

// ── R111: 图层面刷新(全量/脏矩形) ──

unsafe extern "C" {}

static FULL_REDRAW_PENDING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_overlay_flush() {
    use objc2_app_kit::NSView;
    if glaspen2_rust_surface_current().is_null() {
        return;
    }
    let p = unsafe { glaspen2_swift_draw_view() };
    if p.is_null() {
        return;
    }
    let view = unsafe { &*(p as *const NSView) };
    FULL_REDRAW_PENDING.store(false, std::sync::atomic::Ordering::SeqCst);
    glaspen2_rust_dirty_reset();
    view.setNeedsDisplay(true);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_overlay_flush_dirty() {
    use objc2_app_kit::NSView;
    use objc2_foundation::NSRect;
    if glaspen2_rust_surface_current().is_null() {
        return;
    }
    let p = unsafe { glaspen2_swift_draw_view() };
    if p.is_null() {
        return;
    }
    let view = unsafe { &*(p as *const NSView) };
    // 可见性切换(ethereal auto_show / mode toggle)只标了 full redraw; 如果紧接着
    // 的落笔 flush 只设 dirty rect, AppKit 会把旧笔迹所在区域留在已清空的
    // backing store 里, 表现为已有笔迹逐渐从无到有。全量刷新优先。
    if FULL_REDRAW_PENDING.swap(false, std::sync::atomic::Ordering::SeqCst) {
        glaspen2_rust_dirty_reset();
        view.setNeedsDisplay(true);
        return;
    }
    if glaspen2_rust_dirty_has() == 0 {
        view.setNeedsDisplay(true);
        return;
    }
    let mut r = [0.0f64; 4];
    let ok = unsafe { glaspen2_rust_dirty_rect(r.as_mut_ptr()) };
    let bounds = view.bounds();
    let x = r[0].max(bounds.origin.x);
    let y = r[1].max(bounds.origin.y);
    let max_x = (r[0] + r[2]).min(bounds.origin.x + bounds.size.width);
    let max_y = (r[1] + r[3]).min(bounds.origin.y + bounds.size.height);
    let w = max_x - x;
    let h = max_y - y;
    glaspen2_rust_dirty_reset();
    if ok == 0 || w <= 0.0 || h <= 0.0 {
        view.setNeedsDisplay(true);
        return;
    }
    view.setNeedsDisplayInRect(NSRect::new(
        objc2_foundation::NSPoint::new(x, y),
        objc2_foundation::NSSize::new(w, h),
    ));
}

// ── R119: 录制事件回喂与 effect 执行(ShowHud/GIF/剪贴板/聊天) ──

static PENDING_GIF: std::sync::Mutex<Option<Vec<u8>>> = std::sync::Mutex::new(None);

unsafe extern "C" {
    fn glaspen2_recording_event_json(
        slot: i32,
        json: *const std::os::raw::c_char,
    ) -> *mut std::os::raw::c_char;
    fn glaspen2_swift_dispatch_main(ctx: *mut c_void, cb: extern "C" fn(*mut c_void));
    fn glaspen2_gif_record_end(
        start: i32,
        end: i32,
        fps: i32,
        resolution: f64,
        speed: f64,
        end_mode: i32,
        out_len: *mut i32,
    ) -> *mut u8;
    fn glaspen2_chat_send_strokes(start: i32, end: i32) -> i32;
    fn glaspen2_ink_draft_start(w: i32, h: i32) -> i32;
    fn glaspen2_ink_draft_stop() -> i32;
    fn glaspen2_ink_draft_last_error() -> *const std::os::raw::c_char;
}

fn recording_simple_event(kind: &str) -> String {
    serde_json::json!({ "type": kind }).to_string()
}

fn recording_feed_internal(slot: i32, json: &str) -> i32 {
    let Ok(cjson) = std::ffi::CString::new(json) else {
        return 0;
    };
    let out = unsafe { glaspen2_recording_event_json(slot, cjson.as_ptr()) };
    if out.is_null() {
        tracing::warn!("[recording] 槽 {slot}: FFI 返回 NULL");
        return 0;
    }
    let text = unsafe { std::ffi::CStr::from_ptr(out) }
        .to_string_lossy()
        .into_owned();
    unsafe { glaspen2_free_c_string(out) };
    let Ok(effects) = serde_json::from_str::<serde_json::Value>(&text) else {
        tracing::warn!("[recording] 槽 {slot} 事件执行失败");
        return 0;
    };
    let Some(array) = effects.as_array() else {
        return 0;
    };
    if array.is_empty() {
        return 0;
    }
    for effect in array {
        recording_exec_effect(slot, effect);
    }
    1
}

fn recording_exec_effect(slot: i32, effect: &serde_json::Value) {
    let Some(kind) = effect.get("kind").and_then(|k| k.as_str()) else {
        return;
    };
    match kind {
        "ShowHud" => {
            let Some(hud) = effect.get("hud").and_then(|v| v.as_str()) else {
                return;
            };
            let strokes = effect.get("strokes").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let detail = effect.get("detail").and_then(|v| v.as_str());
            let Ok(chud) = std::ffi::CString::new(hud) else {
                return;
            };
            let cdetail = detail.and_then(|d| std::ffi::CString::new(d).ok());
            let text = unsafe {
                glaspen2_rust_rec_hud_text(
                    chud.as_ptr(),
                    strokes,
                    cdetail.as_ref().map_or(std::ptr::null(), |d| d.as_ptr()),
                )
            };
            if text.is_null() {
                tracing::warn!("[recording] 未实现的 HUD 文案: {hud}");
                return;
            }
            unsafe {
                glaspen2_swift_notify(text);
                libc::free(text as *mut c_void);
            }
        }
        "EncodeGif" => {
            let start = effect
                .get("start_stroke")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;
            let end = effect
                .get("end_stroke")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;
            let fps = unsafe { glaspen2_router_gif_fps() };
            let resolution = unsafe { glaspen2_router_gif_resolution() };
            let speed = unsafe { glaspen2_router_gif_speed() };
            let end_mode = unsafe { glaspen2_router_gif_end_mode() };
            std::thread::spawn(move || {
                let mut out_len = 0i32;
                let gif = unsafe {
                    glaspen2_gif_record_end(
                        start,
                        end,
                        fps,
                        resolution,
                        speed,
                        end_mode,
                        &mut out_len,
                    )
                };
                if !gif.is_null() && out_len > 0 {
                    let bytes =
                        unsafe { std::slice::from_raw_parts(gif, out_len as usize) }.to_vec();
                    unsafe { glaspen2_free_rust_bytes(gif, out_len) };
                    *PENDING_GIF.lock().unwrap_or_else(|e| e.into_inner()) = Some(bytes);
                    unsafe { glaspen2_swift_dispatch_main(std::ptr::null_mut(), recording_gif_ok) };
                } else {
                    if !gif.is_null() {
                        unsafe { glaspen2_free_rust_bytes(gif, out_len) }
                    }
                    unsafe {
                        glaspen2_swift_dispatch_main(std::ptr::null_mut(), recording_gif_fail)
                    };
                }
            });
        }
        "Clipboard" => {
            let gif = PENDING_GIF.lock().unwrap_or_else(|e| e.into_inner()).take();
            if let Some(bytes) = gif.filter(|b| !b.is_empty()) {
                unsafe { glaspen2_swift_copy_gif(bytes.as_ptr(), bytes.len() as i32) };
            } else {
                tracing::warn!("[recording] Clipboard effect 没有待写字节");
            }
            let _ = recording_feed_internal(0, &recording_simple_event("GifClipboardSucceeded"));
        }
        "StartChatDraft" => {
            let ok = unsafe {
                glaspen2_ink_draft_start(
                    glaspen_core::shell_state::screen_w(),
                    glaspen_core::shell_state::screen_h(),
                )
            } != 0;
            let event = recording_simple_event(if ok {
                "ChatDraftStarted"
            } else {
                "ChatDraftStartFailed"
            });
            let _ = recording_feed_internal(1, &event);
        }
        "StopChatDraft" => {
            unsafe { glaspen2_router_finish_active_stroke() };
            std::thread::spawn(move || {
                let r = unsafe { glaspen2_ink_draft_stop() };
                let detail = unsafe {
                    let err = glaspen2_ink_draft_last_error();
                    if !err.is_null() && *err != 0 {
                        std::ffi::CStr::from_ptr(err).to_string_lossy().into_owned()
                    } else {
                        String::new()
                    }
                };
                let payload = Box::new((r, detail));
                unsafe {
                    glaspen2_swift_dispatch_main(
                        Box::into_raw(payload) as *mut c_void,
                        recording_chat_stop_done,
                    )
                };
            });
        }
        "SendChatStrokes" => {
            let start = effect
                .get("start_stroke")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;
            let end = effect
                .get("end_stroke")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;
            std::thread::spawn(move || {
                let sent = unsafe { glaspen2_chat_send_strokes(start, end) };
                let payload = Box::new(sent);
                unsafe {
                    glaspen2_swift_dispatch_main(
                        Box::into_raw(payload) as *mut c_void,
                        recording_chat_send_done,
                    )
                };
            });
        }
        "Discard" => tracing::info!("[recording] 丢弃进行中的录制窗口(取消)"),
        _ => tracing::warn!("[recording] 未实现的录制 effect: {kind}"),
    }
    let _ = slot;
}

extern "C" fn recording_gif_ok(_ctx: *mut c_void) {
    let _ = recording_feed_internal(0, &recording_simple_event("GifEncodeSucceeded"));
}

extern "C" fn recording_gif_fail(_ctx: *mut c_void) {
    let _ = recording_feed_internal(0, &recording_simple_event("GifEncodeFailed"));
}

extern "C" fn recording_chat_stop_done(ctx: *mut c_void) {
    if ctx.is_null() {
        return;
    }
    let payload = unsafe { Box::from_raw(ctx as *mut (i32, String)) };
    let event =
        serde_json::json!({"type": "ChatDraftFinished", "code": payload.0, "detail": payload.1})
            .to_string();
    let _ = recording_feed_internal(1, &event);
}

extern "C" fn recording_chat_send_done(ctx: *mut c_void) {
    if ctx.is_null() {
        return;
    }
    let payload = unsafe { Box::from_raw(ctx as *mut i32) };
    let event = if *payload >= 0 {
        serde_json::json!({"type": "ChatSendSucceeded", "strokes": *payload}).to_string()
    } else {
        recording_simple_event("ChatSendFailed")
    };
    let _ = recording_feed_internal(1, &event);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_recording_feed(
    slot: i32,
    json: *const std::os::raw::c_char,
) -> i32 {
    if json.is_null() {
        return 0;
    }
    let text = unsafe { std::ffi::CStr::from_ptr(json) }
        .to_string_lossy()
        .into_owned();
    recording_feed_internal(slot, &text)
}

// ── R120: 笔事件分派/压感监视器分类 ──

unsafe extern "C" {
    fn glaspen2_router_pm_enabled() -> i32;
}

/// kind: CGEventType raw; 返回 2 消费, 0 未处理。
// CGEventType 原始值(不是 NSEvent 值):
// 1 leftDown, 2 leftUp, 3 rightDown, 4 rightUp, 5 mouseMoved, 6 leftDrag,
// 7 rightDrag, 10 keyDown, 11 keyUp, 22 scroll, 23 tabletPointer,
// 24 tabletProximity, 25 otherDown, 26 otherUp, 27 otherDrag.
#[inline]
fn pen_kind_down(kind: i32) -> bool {
    matches!(kind, 1 | 3 | 25)
}
#[inline]
fn pen_kind_drag(kind: i32) -> bool {
    matches!(kind, 6 | 7 | 27)
}
#[inline]
fn pen_kind_up(kind: i32) -> bool {
    matches!(kind, 2 | 4 | 26)
}
#[inline]
fn pen_kind_down_or_drag(kind: i32) -> bool {
    pen_kind_down(kind) || pen_kind_drag(kind)
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_handle_pen_event(
    kind: i32,
    x: f64,
    y: f64,
    pressure: f64,
    timestamp: f64,
    raw_width: f64,
    is_eraser: i32,
) -> i32 {
    let scale = glaspen2_rust_surface_scale();
    match kind {
        k if pen_kind_down(k) => {
            glaspen2_rust_finish_active_stroke();
            glaspen2_rust_update_eraser(is_eraser);
            tracing::info!("[glaspen2] pen DOWN p={pressure:.2} ts={timestamp:.3}");
            glaspen2_rust_pen_down(x, y, pressure, timestamp, raw_width, scale);
            2
        }
        k if pen_kind_drag(k) => {
            if glaspen2_rust_pen_stroke_active() == 0 {
                glaspen2_rust_update_eraser(is_eraser);
                glaspen2_rust_pen_down(x, y, pressure, timestamp, raw_width, scale);
            } else {
                glaspen2_rust_pen_move(x, y, pressure, timestamp, raw_width, scale);
            }
            2
        }
        k if pen_kind_up(k) => {
            tracing::info!("[pen] 抬笔 (pen up)");
            glaspen2_rust_pen_up(x, y, pressure, timestamp);
            2
        }
        _ => 0,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_update_pressure_monitor(
    kind: i32,
    raw_field: i64,
    x: f64,
    y: f64,
    pressure: f64,
) -> i32 {
    if unsafe { glaspen2_router_pm_enabled() } == 0 {
        return 0;
    }
    let pm_pressure = if raw_field > 0 {
        (raw_field * 65535 / 65536) as i32
    } else {
        (pressure * 65535.0) as i32
    };
    let tip_down = pen_kind_down_or_drag(kind);
    let name = match kind {
        1 => "DOWN",
        2 => "UP",
        6 => "DRAG",
        3 => "R_DOWN",
        4 => "R_UP",
        7 => "R_DRAG",
        25 => "O_DOWN",
        26 => "O_UP",
        27 => "O_DRAG",
        _ => {
            if tip_down {
                "DOWN"
            } else {
                "MOVE"
            }
        }
    };
    let Ok(cname) = std::ffi::CString::new(name) else {
        return 0;
    };
    unsafe { sw_pm_update(1, pm_pressure, x, y, i32::from(tip_down), cname.as_ptr()) };
    1
}

// ── R121: 笔 proximity 进入/离开 ──

unsafe extern "C" {}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_route_proximity(device: i32, enter: i32) -> i32 {
    if device != 1 && device != 3 {
        return 0;
    }
    glaspen2_rust_pen_set_tablet_proximity(enter);
    glaspen2_rust_pen_set_has_seen_proximity(1);
    if enter != 0 {
        tracing::info!(
            "[pen] {} (proximity enter)",
            if device == 3 {
                "橡皮进入"
            } else {
                "悬浮进入"
            }
        );
        unsafe {
            glaspen2_rust_pen_set_prox_device(device);
            glaspen2_router_peek_cancel();
            glaspen2_router_bubble_pen_back();
        }
        if unsafe { glaspen2_router_ethereal() } != 0 {
            unsafe { glaspen2_router_pen_auto_show() };
        }
    } else {
        tracing::info!("[pen] 悬浮离开 (proximity exit)");
        glaspen2_rust_pen_set_prox_device(0);
        unsafe {
            glaspen2_router_bubble_pen_gone();
            if (glaspen2_router_cursor_flags() & 1) != 0 {
                glaspen2_router_cursor_invalidate();
                glaspen2_router_cursor_hide();
                glaspen2_router_flush();
            }
        }
        if unsafe { glaspen2_router_ethereal() } != 0 {
            unsafe { glaspen2_router_auto_hide() };
        }
    }
    1
}

// ── R122: 笔光标更新/系统光标隐藏恢复 ──

unsafe extern "C" {}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_system_cursor(kind: i32) {
    let down_or_drag = pen_kind_down_or_drag(kind);
    if down_or_drag && unsafe { glaspen2_router_pen_drawing() } == 0 {
        unsafe {
            glaspen2_router_hide_system_cursor();
            glaspen2_router_set_pen_drawing(1);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pen_cursor_update(kind: i32, is_up: i32, x: f64, y: f64) {
    unsafe { glaspen2_router_pen_bubble(if is_up != 0 { 0 } else { 1 }) };
    let state = unsafe { glaspen2_router_pen_state() };
    let strokes_visible = (state & 1) != 0;
    let stroke_active = (state & 2) != 0;
    let cursor_visible = (state & 4) != 0;
    let proximity = glaspen2_rust_pen_tablet_proximity() != 0;
    let seen = glaspen2_rust_pen_has_seen_proximity() != 0;
    if !strokes_visible && (proximity || !seen) {
        unsafe { glaspen2_router_pen_auto_show() };
    }
    unsafe { glaspen2_router_peek_cancel() };
    let old_x = unsafe { glaspen2_router_cursor_x() };
    let old_y = unsafe { glaspen2_router_cursor_y() };
    let moved = old_x != x || old_y != y;
    if moved && cursor_visible {
        unsafe { glaspen2_router_cursor_invalidate() };
    }
    unsafe { glaspen2_router_cursor_set(x, y) };
    if !stroke_active {
        unsafe { glaspen2_router_cursor_show(1) };
    }
    if moved && unsafe { glaspen2_router_pen_cursor_visible() } != 0 {
        unsafe { glaspen2_router_cursor_invalidate() };
        if !stroke_active && unsafe { glaspen2_router_cursor_flush_due() } == 1 {
            unsafe { glaspen2_router_flush() };
        }
    }
    let _ = kind;
}

// ── R123: 主路由的鼠标移动/抬起/菜单栏/停用清理分支 ──

unsafe extern "C" {}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_route_disable_cleanup() {
    unsafe { glaspen2_router_disable_cleanup() }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_route_menu_bar_click(y: f64) -> i32 {
    if y < 30.0 {
        unsafe { glaspen2_router_tap_menu_bar_interaction() };
        1
    } else {
        0
    }
}

static LAST_PEN_ROUTE_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn pen_route_recent() -> bool {
    let last = LAST_PEN_ROUTE_MS.load(std::sync::atomic::Ordering::Relaxed);
    last != 0 && now_ms().saturating_sub(last) < 800
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_route_mouse_moved() {
    // 笔悬浮时驱动会把部分移动报成 mouseMoved; 刚有笔事件的 800ms 内
    // 不因鼠标移动隐藏笔迹, 避免 ethereal 下 auto_hide/auto_show 抖动。
    if pen_route_recent() || unsafe { glaspen2_rust_pen_tablet_proximity() != 0 } {
        return;
    }
    let flags = unsafe { glaspen2_router_cursor_flags() };
    if (flags & 1) != 0 && (flags & 2) == 0 {
        unsafe {
            glaspen2_router_cursor_invalidate();
            glaspen2_router_cursor_hide();
            if (flags & 4) != 0 {
                glaspen2_router_auto_hide();
            }
            if (flags & 8) == 0 {
                glaspen2_router_bubble_pen_gone();
            }
            glaspen2_router_flush();
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_route_mouse_up() {
    glaspen2_rust_restore_system_cursor();
    unsafe {
        glaspen2_router_cursor_hide();
        glaspen2_rust_dirty_include_point(
            glaspen2_router_cursor_x(),
            glaspen2_router_cursor_y(),
            14.0,
        );
    }
    glaspen2_rust_overlay_flush_dirty();
}

// ── R124: 键盘事件顶层路由(录制热键 + chord) ──

/// 真笔的 NSEvent.pressure 常为 0, 压力在 CGEvent 的 16-bit
/// tabletEventPointPressure(raw_field) 里; 监视器与笔迹宽度必须同源。
#[inline]
fn normalized_tablet_pressure(pressure: f64, raw_field: i64) -> f64 {
    if raw_field > 0 {
        (raw_field as f64 / 65535.0).clamp(0.0, 1.0)
    } else {
        pressure
    }
}

/// 普通 chord 只在 keyDown 分派; keyUp 不能再执行一次。
#[inline]
fn chord_dispatch_is_keydown(kind: i32) -> bool {
    kind == 10
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_route_keyboard(
    kind: i32,
    key_code: u16,
    has_cmd: i32,
    has_ctrl: i32,
    has_alt: i32,
    has_shift: i32,
    is_repeat: i32,
) -> i32 {
    if glaspen2_rust_recording_is_active() != 0 {
        return 1;
    }
    // 按住 ⌘⌃R 录制时, 系统会持续发 keyDown autorepeat。若重复进入录制
    // hotkey, 每次 StartPressed 都会 `recording_feed` → finish_active_stroke,
    // 把正在写的一笔按键盘重复率(~80ms)切成碎段, 录出的 GIF 就会断触。
    // 自动重复不产生新的按键语义, 录制只在首次 keyDown 启动、keyUp 停止。
    // 但必须**消费**这些 repeat: 直接 return 1 会把 ⌘⌃R 放给 NSApplication,
    // 系统按无匹配菜单快捷键处理, 持续发出“嘟嘟”无效按键声。
    if is_repeat != 0 {
        let chat = unsafe { glaspen2_router_chat_integration() } != 0;
        if recording_hotkey_key(key_code, has_cmd != 0, has_ctrl != 0, chat) {
            return 2;
        }
        return 1;
    }
    let down = i32::from(kind == 10);
    if glaspen2_rust_recording_hotkey(down, key_code, i32::from(has_cmd != 0 && has_ctrl != 0)) != 0
    {
        return 2;
    }
    // 普通 chord 只在 keyDown 触发一次; keyUp 若也分派会把每个热键执行两遍
    // (撤销两笔/飘渺切两次)。录制热键已在上方按 down/up 处理。
    if !chord_dispatch_is_keydown(kind) {
        return 1;
    }
    glaspen2_rust_handle_chord(key_code, has_cmd, has_ctrl, has_alt, has_shift, is_repeat)
}

// ── R127: 主路由笔事件块(压感/raw width/自动显示/分派) ──

unsafe extern "C" {
    fn glaspen2_router_strokes_hidden() -> i32;
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_route_pen(
    kind: i32,
    x: f64,
    y: f64,
    pressure: f64,
    timestamp: f64,
    raw_field: i64,
    is_eraser: i32,
) -> i32 {
    LAST_PEN_ROUTE_MS.store(now_ms(), std::sync::atomic::Ordering::Relaxed);
    if unsafe { glaspen2_router_strokes_hidden() } != 0 && pen_kind_down_or_drag(kind) {
        unsafe { glaspen2_router_pen_auto_show() };
    }
    // 真实数位笔的 NSEvent.pressure 常为 0, 压力在 CGEvent 的 16-bit
    // tabletEventPointPressure(raw_field) 里; 监视器一直用 raw_field, 但
    // 笔迹宽度之前只用 pressure, 导致真笔没有粗细变化。
    let effective_pressure = normalized_tablet_pressure(pressure, raw_field);
    // 少数笔的 pen-up NSEvent.timestamp 可能是 0; 直接传入模型器会以
    // “负时间差/非法输入”拒绝收尾, 或在 DB 尾部留下 t<0 伪点。
    let timestamp = if timestamp.is_finite() && timestamp > 0.0 {
        timestamp
    } else {
        monotonic_secs()
    };
    let _ = glaspen2_rust_update_pressure_monitor(kind, raw_field, x, y, effective_pressure);
    let raw_width = unsafe { glaspen2_router_raw_width(effective_pressure) };
    glaspen2_rust_handle_pen_event(
        kind,
        x,
        y,
        effective_pressure,
        timestamp,
        raw_width,
        is_eraser,
    )
}

// ── R143: 显示器几何变更应用(反色失效/建页/重载/窗口尺寸/重建) ──

unsafe extern "C" {
    fn glaspen2_router_invert_invalidate();
    fn glaspen2_router_set_screen_size(w: i32, h: i32);
    fn glaspen2_on_display_change(w: i32, h: i32);
    fn glaspen2_smooth_loaded_strokes();
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_apply_display_change(width: i32, height: i32) {
    tracing::info!("[glaspen2] display changed to {width}x{height}");
    unsafe {
        glaspen2_router_invert_invalidate();
        glaspen2_router_set_screen_size(width, height);
        glaspen2_on_display_change(width, height);
        glaspen2_load_strokes_for_screen(glaspen2_get_current_screen_id());
        glaspen2_smooth_loaded_strokes();
    }
    glaspen2_rust_pageview_update();
    unsafe {
        glaspen2_swift_apply_display_frames(width, height);
        glaspen2_router_ensure_surface();
        glaspen2_router_rebuild_surface();
    }
}

// ── R144: ensure surface C 符号(绘制视图指针 → Rust 表面 ensure) ──

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_ensure_surface() {
    let view = unsafe { glaspen2_swift_draw_view() };
    if !view.is_null() {
        let _ = glaspen2_rust_surface_ensure(view);
    }
}

// ── R145: 事件 tap 健康看门狗(辅助功能权限/重装/enable) ──

static AX_WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

unsafe extern "C" {
    fn AXIsProcessTrusted() -> u8;
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_check_tap() {
    use std::sync::atomic::Ordering;
    if unsafe { AXIsProcessTrusted() } == 0 {
        if !AX_WARNED.swap(true, Ordering::Relaxed) {
            unsafe { glaspen2_swift_notify_ax_warning() };
        }
        return;
    }
    AX_WARNED.store(false, Ordering::Relaxed);
    unsafe {
        if glaspen2_router_event_tap_ok() == 0 {
            glaspen2_router_event_tap_reinstall();
        } else if glaspen2_router_event_tap_enabled() == 0 {
            glaspen2_router_event_tap_enable();
        }
    }
}

// ── R151: 启动读取快捷键覆盖并应用 ──

unsafe extern "C" {
    fn glaspen2_load_string_setting(key: *const std::os::raw::c_char) -> *mut std::os::raw::c_char;
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_load_shortcuts() {
    let key = b"shortcuts\0";
    let raw = unsafe { glaspen2_load_string_setting(key.as_ptr() as *const std::os::raw::c_char) };
    if raw.is_null() {
        let _ = glaspen2_shortcut_apply_json(b"{}\0".as_ptr() as *const std::os::raw::c_char);
        return;
    }
    let text = unsafe { std::ffi::CStr::from_ptr(raw) }
        .to_string_lossy()
        .into_owned();
    unsafe { glaspen2_free_c_string(raw) };
    let json = if text.trim().is_empty() {
        "{}".to_string()
    } else {
        text
    };
    if let Ok(c) = std::ffi::CString::new(json) {
        let _ = glaspen2_shortcut_apply_json(c.as_ptr());
    }
}

// ── R153: 时光隧道标量状态(active/尺寸/卡片数/参数/时间) ──

static TS_ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static TS_SURF_W: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
static TS_SURF_H: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
static TS_CARDS: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
static TS_BACK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static TS_TRAVEL: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1.0f64.to_bits());
static TS_K: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0.45f64.to_bits());
static TS_THICK: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0.018f64.to_bits());
static TS_PANE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0.84f64.to_bits());
static TS_DUR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0.38f64.to_bits());
static TS_T0: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0.0f64.to_bits());
static TS_FRAMES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

macro_rules! tunnel_bool_accesors {
    ($get:ident, $set:ident, $cell:ident) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $get() -> i32 {
            i32::from($cell.load(std::sync::atomic::Ordering::Relaxed))
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn $set(v: i32) {
            $cell.store(v != 0, std::sync::atomic::Ordering::Relaxed);
        }
    };
}

macro_rules! tunnel_i32_accesors {
    ($get:ident, $set:ident, $cell:ident) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $get() -> i32 {
            $cell.load(std::sync::atomic::Ordering::Relaxed)
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn $set(v: i32) {
            $cell.store(v, std::sync::atomic::Ordering::Relaxed);
        }
    };
}

macro_rules! tunnel_f64_accesors {
    ($get:ident, $set:ident, $cell:ident) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $get() -> f64 {
            f64::from_bits($cell.load(std::sync::atomic::Ordering::Relaxed))
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn $set(v: f64) {
            $cell.store(v.to_bits(), std::sync::atomic::Ordering::Relaxed);
        }
    };
}

tunnel_bool_accesors!(
    glaspen2_rust_tunnel_get_active,
    glaspen2_rust_tunnel_set_active,
    TS_ACTIVE
);
tunnel_bool_accesors!(
    glaspen2_rust_tunnel_get_back,
    glaspen2_rust_tunnel_set_back,
    TS_BACK
);
tunnel_i32_accesors!(
    glaspen2_rust_tunnel_get_surf_w,
    glaspen2_rust_tunnel_set_surf_w,
    TS_SURF_W
);
tunnel_i32_accesors!(
    glaspen2_rust_tunnel_get_surf_h,
    glaspen2_rust_tunnel_set_surf_h,
    TS_SURF_H
);
tunnel_i32_accesors!(
    glaspen2_rust_tunnel_get_cards,
    glaspen2_rust_tunnel_set_cards,
    TS_CARDS
);
tunnel_f64_accesors!(
    glaspen2_rust_tunnel_get_travel,
    glaspen2_rust_tunnel_set_travel,
    TS_TRAVEL
);
tunnel_f64_accesors!(glaspen2_rust_tunnel_get_k, glaspen2_rust_tunnel_set_k, TS_K);
tunnel_f64_accesors!(
    glaspen2_rust_tunnel_get_thick,
    glaspen2_rust_tunnel_set_thick,
    TS_THICK
);
tunnel_f64_accesors!(
    glaspen2_rust_tunnel_get_pane,
    glaspen2_rust_tunnel_set_pane,
    TS_PANE
);
tunnel_f64_accesors!(
    glaspen2_rust_tunnel_get_dur,
    glaspen2_rust_tunnel_set_dur,
    TS_DUR
);
tunnel_f64_accesors!(
    glaspen2_rust_tunnel_get_t0,
    glaspen2_rust_tunnel_set_t0,
    TS_T0
);

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_tunnel_get_frames() -> u64 {
    TS_FRAMES.load(std::sync::atomic::Ordering::Relaxed)
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_tunnel_set_frames(v: u64) {
    TS_FRAMES.store(v, std::sync::atomic::Ordering::Relaxed);
}

// ── R154: 隧道参数环境变量(FLIP_DUR/K/THICK/GLASS) ──

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_tunnel_apply_env() {
    let read = |name: &str| std::env::var(name).ok().and_then(|s| s.parse::<f64>().ok());
    if let Some(v) = read("GLASPEN2_FLIP_DUR") {
        if v > 0.05 {
            TS_DUR.store(v.to_bits(), std::sync::atomic::Ordering::Relaxed);
        }
    }
    if let Some(v) = read("GLASPEN2_FLIP_K") {
        if v > 0.05 && v < 3.0 {
            TS_K.store(v.to_bits(), std::sync::atomic::Ordering::Relaxed);
        }
    }
    if let Some(v) = read("GLASPEN2_FLIP_THICK") {
        if (0.002..=0.10).contains(&v) {
            TS_THICK.store(v.to_bits(), std::sync::atomic::Ordering::Relaxed);
        }
    }
    if let Some(v) = read("GLASPEN2_FLIP_GLASS") {
        if (0.0..=0.98).contains(&v) {
            TS_PANE.store(v.to_bits(), std::sync::atomic::Ordering::Relaxed);
        }
    }
}

// ── R155: 隧道每帧时间/进度与启动计时复位 ──

fn reference_now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64() - 978_307_200.0)
        .unwrap_or(0.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_tunnel_reset_start() {
    TS_T0.store(
        reference_now().to_bits(),
        std::sync::atomic::Ordering::Relaxed,
    );
    TS_FRAMES.store(0, std::sync::atomic::Ordering::Relaxed);
}

/// 返回 2 结束, 1 继续(dur 内, out_p 写进度)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_tunnel_tick(out_p: *mut f64) -> i32 {
    let t0 = f64::from_bits(TS_T0.load(std::sync::atomic::Ordering::Relaxed));
    let dur = f64::from_bits(TS_DUR.load(std::sync::atomic::Ordering::Relaxed)).max(0.001);
    let p = (reference_now() - t0) / dur;
    if !out_p.is_null() {
        unsafe { *out_p = p.clamp(0.0, 1.0) };
    }
    i32::from(p < 1.0)
}

// ── R156: 隧道建卡循环(半分辨率快照由 Swift factory 提供) ──

/// 返回建卡数; -1 表示 factory 失败。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_tunnel_build_cards(
    cur: i64,
    count: i32,
    cur_idx: i32,
    front: f64,
    depths: *mut i32,
    card_surf: *mut *mut c_void,
    mip_surf: *mut *mut c_void,
) -> i32 {
    if depths.is_null() || card_surf.is_null() || mip_surf.is_null() || count <= 0 {
        return 0;
    }
    let start = std::time::Instant::now();
    let mut built = 0i32;
    for i in 0..count {
        let depth = cur_idx - i;
        if depth as f64 > front {
            continue;
        }
        if start.elapsed().as_secs_f64() > 0.30 {
            break;
        }
        let mut main: *mut c_void = std::ptr::null_mut();
        let mut mip: *mut c_void = std::ptr::null_mut();
        let ok = unsafe {
            glaspen2_rust_make_card(
                i32::from(depth == 0),
                depth,
                i,
                if depth == 0 { cur } else { 0 },
                &mut main,
                &mut mip,
            )
        };
        if ok == 0 || main.is_null() {
            return -1;
        }
        unsafe {
            *depths.add(i as usize) = depth;
            *card_surf.add(i as usize) = main;
            *mip_surf.add(i as usize) = mip;
        }
        built += 1;
    }
    built
}

// ── R157: 隧道启动准备(邻页/表面/blur/预载) ──

unsafe extern "C" {
    fn glaspen2_page_neighbor(id: i64, dir: i32) -> i64;
    fn glaspen2_preload_flip_pages(center: i64, going_next: i32, before: i32, after: i32) -> i32;
    fn glaspen2_swift_tunnel_set_surfaces(surface: *mut c_void, blur: *mut c_void);
}

/// 返回卡片数; -1 失败。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_tunnel_prepare(
    cur: i64,
    going_next: i32,
    scale: f64,
    preload_ms_out: *mut f64,
) -> i32 {
    let screen_w = unsafe { glaspen2_router_screen_w() };
    let screen_h = unsafe { glaspen2_router_screen_h() };
    if glaspen2_rust_surface_current().is_null() || screen_w <= 0.0 || screen_h <= 0.0 {
        return -1;
    }
    let target = unsafe { glaspen2_page_neighbor(cur, if going_next != 0 { 1 } else { -1 }) };
    if target <= 0 {
        return -1;
    }
    let sw = (screen_w * scale * 0.5) as i32;
    let sh = (screen_h * scale * 0.5) as i32;
    if sw <= 0 || sh <= 0 {
        return -1;
    }
    let surface = unsafe { cairo_image_surface_create(0, sw, sh) };
    if surface.is_null() || unsafe { cairo_surface_status(surface) } != 0 {
        if !surface.is_null() {
            unsafe { cairo_surface_destroy(surface) }
        }
        return -1;
    }
    let preload_start = std::time::Instant::now();
    let mut n = unsafe { glaspen2_preload_flip_pages(cur, going_next, 4, 1) };
    if !preload_ms_out.is_null() {
        unsafe { *preload_ms_out = preload_start.elapsed().as_secs_f64() * 1000.0 };
    }
    if n <= 0 {
        unsafe { cairo_surface_destroy(surface) };
        return -1;
    }
    if n > 512 {
        n = 512;
    }
    let blur = unsafe { cairo_image_surface_create(0, sw / 3, sh / 3) };
    let blur = if !blur.is_null() && unsafe { cairo_surface_status(blur) } == 0 {
        blur
    } else {
        if !blur.is_null() {
            unsafe { cairo_surface_destroy(blur) }
        }
        std::ptr::null_mut()
    };
    unsafe {
        glaspen2_rust_tunnel_set_surf_w(sw);
        glaspen2_rust_tunnel_set_surf_h(sh);
        glaspen2_swift_tunnel_set_surfaces(surface, blur);
    }
    n
}

// ── R159: minimap 布局 JSON 缓存(页/视图/几何变化才重算) ──

unsafe extern "C" {
    fn glaspen2_minimap_layout_json(
        cur: i64,
        view_w: f64,
        view_h: f64,
        geom_w: i32,
    ) -> *mut std::os::raw::c_char;
}

static MINIMAP_LAYOUT: std::sync::Mutex<Option<(i64, u64, u64, i32, std::ffi::CString)>> =
    std::sync::Mutex::new(None);

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_minimap_layout(
    cur: i64,
    view_w: f64,
    view_h: f64,
    geom_w: i32,
) -> *const std::os::raw::c_char {
    let mut guard = MINIMAP_LAYOUT.lock().unwrap_or_else(|e| e.into_inner());
    let key = (cur, view_w.to_bits(), view_h.to_bits(), geom_w);
    if guard
        .as_ref()
        .is_none_or(|(c, w, h, g, _)| (*c, *w, *h, *g) != key)
    {
        let raw = unsafe { glaspen2_minimap_layout_json(cur, view_w, view_h, geom_w) };
        if raw.is_null() {
            return std::ptr::null();
        }
        let text = unsafe { std::ffi::CStr::from_ptr(raw) }.to_owned();
        unsafe { glaspen2_free_c_string(raw) };
        if text.to_bytes().first().is_none() {
            return std::ptr::null();
        }
        *guard = Some((cur, view_w.to_bits(), view_h.to_bits(), geom_w, text));
    }
    guard
        .as_ref()
        .map_or(std::ptr::null(), |(_, _, _, _, s)| s.as_ptr())
}

// ── R159: 通知文字/代际状态(定时器仍 Swift) ──

static NOTIF_TEXT: std::sync::Mutex<Option<std::ffi::CString>> = std::sync::Mutex::new(None);
static NOTIF_GEN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

unsafe extern "C" {}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_notification_set(text: *const std::os::raw::c_char) -> u32 {
    let payload = if text.is_null() {
        None
    } else {
        Some(unsafe { std::ffi::CStr::from_ptr(text) }.to_owned())
    };
    *NOTIF_TEXT.lock().unwrap_or_else(|e| e.into_inner()) = payload;
    unsafe { glaspen2_notification_redraw() };
    NOTIF_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_notification_clear_if(generation: u32) -> i32 {
    if NOTIF_GEN.load(std::sync::atomic::Ordering::Relaxed) != generation {
        return 0;
    }
    *NOTIF_TEXT.lock().unwrap_or_else(|e| e.into_inner()) = None;
    unsafe { glaspen2_notification_redraw() };
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_notification_text() -> *const std::os::raw::c_char {
    let guard = NOTIF_TEXT.lock().unwrap_or_else(|e| e.into_inner());
    guard.as_ref().map_or(std::ptr::null(), |s| s.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_notification_draw(width: f64, height: f64) -> i32 {
    let guard = NOTIF_TEXT.lock().unwrap_or_else(|e| e.into_inner());
    let Some(text) = guard.as_ref() else { return 0 };
    glaspen2_rust_draw_notification(text.as_ptr(), width, height)
}

// ── R164: 状态栏图标应用(item.button.image/title) ──

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_status_icon_apply(
    item: *mut c_void,
    eraser: i32,
    enabled: i32,
    ethereal: i32,
    infinite: i32,
    width_scale: f64,
    r: f64,
    g: f64,
    b: f64,
) -> i32 {
    use objc2_app_kit::NSImage;
    use objc2_foundation::NSString;
    if item.is_null() {
        return 0;
    }
    let image =
        glaspen2_rust_status_icon_render(eraser, enabled, ethereal, infinite, width_scale, r, g, b);
    if image.is_null() {
        return 0;
    }
    let button: *mut objc2::runtime::AnyObject =
        unsafe { objc2::msg_send![item as *mut objc2::runtime::AnyObject, button] };
    if !button.is_null() {
        let image = unsafe { &*(image as *const NSImage) };
        let title = NSString::from_str("");
        let button = unsafe { &*button };
        unsafe {
            let _: () = objc2::msg_send![button, setImage: image];
            let _: () = objc2::msg_send![button, setTitle: &*title];
        }
    }
    1
}

// ── R165: 覆盖窗所在屏 displayID(多屏勿用 displays.first) ──

unsafe extern "C" {
    fn CGMainDisplayID() -> u32;
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_window_display_id() -> u32 {
    use objc2_app_kit::NSWindow;
    use objc2_foundation::{NSNumber, NSString};
    let window = unsafe { glaspen2_router_main_window() };
    if window.is_null() {
        return unsafe { CGMainDisplayID() };
    }
    let win = unsafe { &*(window as *const NSWindow) };
    let screen: *mut objc2::runtime::AnyObject = unsafe { objc2::msg_send![win, screen] };
    if !screen.is_null() {
        let desc: *mut objc2::runtime::AnyObject =
            unsafe { objc2::msg_send![screen, deviceDescription] };
        if !desc.is_null() {
            let key = NSString::from_str("NSScreenNumber");
            let value: *mut objc2::runtime::AnyObject =
                unsafe { objc2::msg_send![desc, objectForKey: &*key] };
            if !value.is_null() {
                let num = unsafe { &*(value as *const NSNumber) };
                return num.intValue() as u32;
            }
        }
    }
    unsafe { CGMainDisplayID() }
}

// ── R166: pen_state.swift C 符号迁 Rust(笔状态/模型器/raw) ──

unsafe extern "C" {
    fn glaspen2_router_width_scale() -> f64;
    fn glaspen2_modeler_begin(
        r: f64,
        g: f64,
        b: f64,
        x: f64,
        y: f64,
        pressure: f64,
        timestamp: f64,
        width_scale: f64,
    );
    fn glaspen2_modeler_move(x: f64, y: f64, pressure: f64, timestamp: f64, width_scale: f64);
    fn glaspen2_modeler_end(x: f64, y: f64, pressure: f64, timestamp: f64, width_scale: f64);
    fn glaspen2_modeler_erase_finish();
    fn glaspen2_modeler_commit_to_strokes(r: f64, g: f64, b: f64);
    fn glaspen2_canvas_input(
        view_x: f64,
        view_y: f64,
        infinite: i32,
        zoom: f64,
        pan_x: f64,
        pan_y: f64,
        page_off_x: f64,
        page_off_y: f64,
        is_fit: i32,
        paper_ox: f64,
        paper_oy: f64,
        paper_scale: f64,
        out_x: *mut f64,
        out_y: *mut f64,
    ) -> i32;
}

#[unsafe(no_mangle)]
pub extern "C" fn sw_stroke_active() -> i32 {
    glaspen2_rust_pen_stroke_active()
}
#[unsafe(no_mangle)]
pub extern "C" fn sw_tablet_proximity() -> i32 {
    glaspen2_rust_pen_tablet_proximity()
}
#[unsafe(no_mangle)]
pub extern "C" fn sw_eraser_mode() -> i32 {
    glaspen2_rust_pen_eraser_mode()
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_reset_raw_stroke() {
    glaspen2_rust_pen_clear_raw();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_clear_raw_state() {
    glaspen2_rust_pen_clear_raw();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_stroke_active() -> i32 {
    glaspen2_rust_pen_stroke_active()
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_stroke_active(on: i32) {
    glaspen2_rust_pen_set_stroke_active(on);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_eraser_mode() -> i32 {
    glaspen2_rust_pen_eraser_mode()
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_eraser_mode(on: i32) {
    glaspen2_rust_pen_set_eraser_mode(on);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_raw_draw_dot(x: f64, y: f64, w: f64) {
    if glaspen2_rust_surface_current().is_null() {
        return;
    }
    glaspen2_rust_raw_draw_dot2(x, y, w, glaspen2_rust_surface_scale());
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_raw_draw_segment(x: f64, y: f64, w: f64) {
    if glaspen2_rust_surface_current().is_null() {
        return;
    }
    glaspen2_rust_raw_draw_segment2(x, y, w, glaspen2_rust_surface_scale());
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_pen_down_state(x: f64, y: f64, raw_width: f64) {
    glaspen2_rust_pen_set_stroke_active(1);
    glaspen2_rust_pen_set_raw_last_x(x);
    glaspen2_rust_pen_set_raw_last_y(y);
    glaspen2_rust_pen_set_raw_last_w(raw_width);
    glaspen2_rust_pen_set_raw_has_last(1);
    glaspen2_rust_pen_set_raw_path_len(0.0);
    sw_cursor_set(x, y, 1);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_pen_up_state() {
    glaspen2_rust_pen_clear_raw();
    sw_cursor_show();
    glaspen2_rust_dirty_include_point(glaspen2_router_cursor_x(), glaspen2_router_cursor_y(), 14.0);
    glaspen2_rust_overlay_flush_dirty();
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_mark_tablet_proximity() {
    glaspen2_rust_pen_set_tablet_proximity(1);
    glaspen2_rust_pen_set_has_seen_proximity(1);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_prox_device(device: i32) {
    glaspen2_rust_pen_set_prox_device(device);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_prox_is_eraser() -> i32 {
    i32::from(glaspen2_rust_pen_prox_device() == 3)
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_last_raw_x() -> f64 {
    glaspen2_rust_pen_raw_last_x()
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_last_raw_y() -> f64 {
    glaspen2_rust_pen_raw_last_y()
}

fn canvas_input(x: f64, y: f64) -> (f64, f64) {
    let mut cx = 0.0f64;
    let mut cy = 0.0f64;
    unsafe {
        glaspen2_canvas_input(
            x,
            y,
            glaspen2_router_infinite(),
            glaspen2_router_zoom(),
            glaspen2_router_pan_x(),
            glaspen2_router_pan_y(),
            glaspen2_router_page_off_x(),
            glaspen2_router_page_off_y(),
            glaspen2_rust_pageview_is_fit(),
            glaspen2_rust_pageview_paper_ox(),
            glaspen2_rust_pageview_paper_oy(),
            glaspen2_rust_pageview_paper_scale(),
            &mut cx,
            &mut cy,
        );
    }
    (cx, cy)
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_modeler_begin(x: f64, y: f64, pressure: f64, timestamp: f64) {
    let (cx, cy) = canvas_input(x, y);
    unsafe {
        glaspen2_modeler_begin(
            glaspen2_router_pen_r(),
            glaspen2_router_pen_g(),
            glaspen2_router_pen_b(),
            cx,
            cy,
            pressure,
            timestamp,
            glaspen2_router_width_scale(),
        );
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_modeler_move(x: f64, y: f64, pressure: f64, timestamp: f64) {
    let (cx, cy) = canvas_input(x, y);
    unsafe {
        glaspen2_modeler_move(cx, cy, pressure, timestamp, glaspen2_router_width_scale());
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_modeler_end(x: f64, y: f64, pressure: f64, timestamp: f64) {
    let (cx, cy) = canvas_input(x, y);
    unsafe {
        glaspen2_modeler_end(cx, cy, pressure, timestamp, glaspen2_router_width_scale());
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_modeler_erase_finish() {
    unsafe { glaspen2_modeler_erase_finish() }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_commit_to_strokes() {
    unsafe {
        glaspen2_modeler_commit_to_strokes(
            glaspen2_router_pen_r(),
            glaspen2_router_pen_g(),
            glaspen2_router_pen_b(),
        );
    }
}

// ── R167: 主路由事件描述符/编排迁 Rust ──

#[repr(C)]
pub struct GlaspenEventC {
    pub kind: i32,
    pub is_pen: i32,
    pub key_code: i32,
    pub flags: i32,
    pub is_repeat: i32,
    pub raw_field: i64,
    pub x: f64,
    pub y: f64,
    pub pen_x: f64,
    pub pen_y: f64,
    pub screen_y: f64,
    pub pressure: f64,
    pub timestamp: f64,
    pub delta_y: f64,
    pub precise: i32,
    pub is_eraser: i32,
}

unsafe extern "C" {}

/// 镜头抓手只认左键 down/drag/up: CGEventType 1/6/2。
#[inline]
fn camera_mouse_dir(kind: i32) -> Option<i32> {
    match kind {
        1 => Some(0),
        6 => Some(1),
        2 => Some(2),
        _ => None,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_route_event(ev: *const GlaspenEventC) -> i32 {
    if ev.is_null() {
        return 1;
    }
    let ev = unsafe { &*ev };
    let kind = ev.kind;
    if kind == 15 || kind == 16 {
        unsafe { glaspen2_router_reenable_tap() };
        tracing::info!("[glaspen2] TAP DISABLED (timeout/userinput) — re-enabled");
        return 1;
    }
    let state = unsafe { glaspen2_router_state() };
    if (state & 1) == 0 && !matches!(kind, 10 | 11 | 22) {
        glaspen2_rust_route_disable_cleanup();
        return 1;
    }
    if kind == 10 || kind == 11 {
        let flags = ev.flags;
        return glaspen2_rust_route_keyboard(
            kind,
            ev.key_code as u16,
            i32::from(flags & (1 << 20) != 0),
            i32::from(flags & (1 << 18) != 0),
            i32::from(flags & (1 << 19) != 0),
            i32::from(flags & (1 << 17) != 0),
            ev.is_repeat,
        );
    }
    // 相机临时抓手(非笔左键 down/drag/up)。
    if ev.is_pen == 0
        && let Some(dir) = camera_mouse_dir(kind)
        && glaspen2_rust_camera_mouse(dir, ev.x, ev.y, ev.flags as u64) != 0
    {
        return 2;
    }
    if kind == 1 && ev.is_pen == 0 && glaspen2_rust_route_menu_bar_click(ev.screen_y) != 0 {
        return 1;
    }
    if kind == 22 && (state & 4) != 0 {
        return glaspen2_rust_scroll_zoom(ev.delta_y, ev.precise, ev.x, ev.y, ev.flags as u64);
    }
    if kind == 5 && ev.is_pen == 0 {
        glaspen2_rust_route_mouse_moved();
    }
    if ev.is_pen != 0 {
        let is_up = pen_kind_up(kind);
        glaspen2_rust_pen_cursor_update(kind, i32::from(is_up), ev.x, ev.y);
        glaspen2_rust_pen_system_cursor(kind);
    }
    if pen_kind_up(kind) {
        glaspen2_rust_route_mouse_up();
    }
    if ev.is_pen != 0 {
        return glaspen2_rust_route_pen(
            kind,
            ev.pen_x,
            ev.pen_y,
            ev.pressure,
            ev.timestamp,
            ev.raw_field,
            ev.is_eraser,
        );
    }
    1
}

// ── R168: minimap 渲染(布局/缩略图缓存/异步生成/绘制) ──

unsafe extern "C" {
    fn glaspen2_render_thumbnail(
        screen_id: i64,
        w: i32,
        h: i32,
        max_size: i32,
        out_len: *mut i32,
    ) -> *mut u8;
}

static MINIMAP_THUMBS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<i64, usize>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));
static MINIMAP_INFLIGHT: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<i64>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashSet::new()));

extern "C" fn minimap_thumb_ready(ctx: *mut c_void) {
    if ctx.is_null() {
        return;
    }
    let payload = unsafe { Box::from_raw(ctx as *mut (i64, Vec<u8>)) };
    use objc2_app_kit::NSImage;
    use objc2_foundation::NSData;
    let data = NSData::with_bytes(&payload.1);
    if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
        let raw = objc2::rc::Retained::into_raw(image) as usize;
        MINIMAP_THUMBS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(payload.0, raw);
    }
    MINIMAP_INFLIGHT
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&payload.0);
    unsafe { glaspen2_router_redraw_draw_view() };
}

fn minimap_request_thumbnail(screen_id: i64) {
    let mut inflight = MINIMAP_INFLIGHT.lock().unwrap_or_else(|e| e.into_inner());
    if inflight.contains(&screen_id)
        || MINIMAP_THUMBS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains_key(&screen_id)
    {
        return;
    }
    inflight.insert(screen_id);
    drop(inflight);
    let w = unsafe { glaspen2_router_screen_w() } as i32;
    let h = unsafe { glaspen2_router_screen_h() } as i32;
    std::thread::spawn(move || {
        let mut len = 0i32;
        let png = unsafe { glaspen2_render_thumbnail(screen_id, w, h, 128, &mut len) };
        if png.is_null() || len <= 0 {
            MINIMAP_INFLIGHT
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&screen_id);
            return;
        }
        let bytes = unsafe { std::slice::from_raw_parts(png, len as usize) }.to_vec();
        unsafe { glaspen2_free_rust_bytes(png, len) };
        let payload = Box::new((screen_id, bytes));
        unsafe {
            glaspen2_swift_dispatch_main(Box::into_raw(payload) as *mut c_void, minimap_thumb_ready)
        };
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_minimap_draw(
    ctx: *mut c_void,
    bx: f64,
    by: f64,
    bw: f64,
    bh: f64,
) -> i32 {
    if ctx.is_null() || bw <= 0.0 || bh <= 0.0 {
        return 0;
    }
    if unsafe { glaspen2_router_infinite() } != 0 {
        return 0;
    }
    let cur = unsafe { glaspen2_get_current_screen_id() };
    if cur <= 0 {
        return 0;
    }
    let p = unsafe {
        glaspen2_rust_minimap_layout(cur, bw, bh, glaspen2_rust_pageview_geometry_width())
    };
    if p.is_null() {
        return 0;
    }
    let text = unsafe { std::ffi::CStr::from_ptr(p) }
        .to_string_lossy()
        .into_owned();
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return 0;
    };
    if v.get("error").is_some() {
        return 0;
    }
    let Some(items_json) = v.get("items").and_then(|i| i.as_array()) else {
        return 0;
    };
    if items_json.is_empty() {
        return 0;
    }
    let strip_w = v.get("strip_w").and_then(|x| x.as_f64()).unwrap_or(0.0);
    let strip_h = v.get("strip_h").and_then(|x| x.as_f64()).unwrap_or(0.0);
    let strip_x = v.get("strip_x").and_then(|x| x.as_f64()).unwrap_or(0.0);
    let strip_y = v.get("strip_y").and_then(|x| x.as_f64()).unwrap_or(0.0);
    if strip_w <= 0.0 || strip_h <= 0.0 {
        return 0;
    }
    let mut items = Vec::with_capacity(items_json.len());
    let mut labels: Vec<std::ffi::CString> = Vec::new();
    for item in items_json {
        let Some(pid) = item.get("screen_id").and_then(|x| x.as_i64()) else {
            continue;
        };
        minimap_request_thumbnail(pid);
        let is_cur = item
            .get("current")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        let y = item.get("y").and_then(|x| x.as_f64()).unwrap_or(0.0);
        let h = item.get("h").and_then(|x| x.as_f64()).unwrap_or(0.0);
        let label = item.get("label").and_then(|x| x.as_str()).unwrap_or("");
        let label_ptr = if label.is_empty() {
            std::ptr::null()
        } else {
            let c = std::ffi::CString::new(label).unwrap_or_default();
            labels.push(c);
            labels.last().map_or(std::ptr::null(), |c| c.as_ptr())
        };
        let thumb = MINIMAP_THUMBS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&pid)
            .copied()
            .unwrap_or(0) as *mut c_void;
        items.push(MinimapItemC {
            screen_id: pid,
            current: i32::from(is_cur),
            y,
            h,
            label: label_ptr,
            thumb,
        });
    }
    if items.is_empty() {
        return 0;
    }
    unsafe {
        glaspen2_rust_draw_minimap(
            ctx,
            items.as_ptr(),
            items.len() as i32,
            strip_w,
            strip_h,
            strip_x,
            strip_y,
            bh,
        )
    }
}

// ── R169: 反色捕获状态/像素处理/consume/ink source ──

struct InvertState {
    bufs: [*mut u8; 2],
    surfs: [*mut c_void; 2],
    w: i32,
    h: i32,
    write: usize,
    present: usize,
    sums: [u64; 2],
    diffs: [u64; 2],
    dboxes: [[i32; 4]; 2],
    present_sum: u64,
    pending: i32,
}

unsafe impl Send for InvertState {}

static INVERT: std::sync::Mutex<InvertState> = std::sync::Mutex::new(InvertState {
    bufs: [std::ptr::null_mut(); 2],
    surfs: [std::ptr::null_mut(); 2],
    w: 0,
    h: 0,
    write: 0,
    present: 0,
    sums: [0; 2],
    diffs: [0; 2],
    dboxes: [[0; 4]; 2],
    present_sum: 0,
    pending: -1,
});

unsafe extern "C" {
    fn cairo_image_surface_create_for_data(
        data: *mut u8,
        format: i32,
        w: i32,
        h: i32,
        stride: i32,
    ) -> *mut c_void;
    fn cairo_surface_mark_dirty(surface: *mut c_void);
    fn cairo_pattern_create_for_surface(surface: *mut c_void) -> *mut c_void;
    fn cairo_pattern_set_matrix(pattern: *mut c_void, matrix: *const [f64; 6]);
    fn glaspen2_set_invert_background(surface: *mut c_void);
    fn glaspen2_set_invert_dirty(x: i32, y: i32, w: i32, h: i32);
    fn glaspen2_set_stroke_invert(on: i32);
}

fn invert_teardown_locked(st: &mut InvertState) {
    unsafe { glaspen2_set_invert_background(std::ptr::null_mut()) };
    for i in 0..2 {
        if !st.surfs[i].is_null() {
            unsafe { cairo_surface_destroy(st.surfs[i]) };
            st.surfs[i] = std::ptr::null_mut();
        }
        if !st.bufs[i].is_null() {
            unsafe { libc::free(st.bufs[i] as *mut c_void) };
            st.bufs[i] = std::ptr::null_mut();
        }
    }
    st.w = 0;
    st.h = 0;
    st.pending = -1;
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_invert_teardown() {
    let mut st = INVERT.lock().unwrap_or_else(|e| e.into_inner());
    invert_teardown_locked(&mut st);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_invert_ensure_cache(w: i32, h: i32) -> i32 {
    if w <= 0 || h <= 0 {
        return 0;
    }
    let mut st = INVERT.lock().unwrap_or_else(|e| e.into_inner());
    if !st.bufs[0].is_null() && st.w == w && st.h == h {
        return 1;
    }
    invert_teardown_locked(&mut st);
    let bytes = (w as usize) * (h as usize) * 4;
    for i in 0..2 {
        let buf = unsafe { libc::malloc(bytes) } as *mut u8;
        if buf.is_null() {
            invert_teardown_locked(&mut st);
            return 0;
        }
        unsafe { std::ptr::write_bytes(buf, 0, bytes) };
        st.bufs[i] = buf;
        let surf = unsafe { cairo_image_surface_create_for_data(buf, 0, w, h, w * 4) };
        if surf.is_null() {
            invert_teardown_locked(&mut st);
            return 0;
        }
        st.surfs[i] = surf;
    }
    st.w = w;
    st.h = h;
    st.write = 0;
    st.present = 0;
    st.pending = -1;
    unsafe {
        glaspen2_set_invert_background(st.surfs[0]);
        glaspen2_set_stroke_invert(1);
    }
    1
}

/// 返回 1 已处理; 0 尺寸/格式不匹配。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_invert_process(
    src: *const u8,
    format: i32,
    w: i32,
    h: i32,
    stride: i32,
) -> i32 {
    if src.is_null() || format != 0x4247_5241 || w <= 0 || h <= 0 || stride <= 0 {
        return 0;
    }
    let mut st = INVERT.lock().unwrap_or_else(|e| e.into_inner());
    if st.w != w || st.h != h || st.bufs[0].is_null() {
        return 0;
    }
    let slot = st.write;
    let row = (w * 4) as usize;
    let dst = st.bufs[slot];
    if dst.is_null() {
        return 0;
    }
    let present_buf = st.bufs[st.present];
    let mut sum = 0u64;
    let mut diff = 0u64;
    let (mut bx0, mut by0, mut bx1, mut by1) = (i32::MAX, i32::MAX, -1i32, -1i32);
    for y in 0..h as usize {
        let srow = unsafe { std::slice::from_raw_parts(src.add(y * stride as usize), row) };
        let drow = unsafe { std::slice::from_raw_parts_mut(dst.add(y * row), row) };
        let prow = if present_buf.is_null() {
            None
        } else {
            Some(unsafe { std::slice::from_raw_parts(present_buf.add(y * row), row) })
        };
        for x in 0..w as usize {
            let si = x * 4;
            let v = u32::from_le_bytes([srow[si], srow[si + 1], srow[si + 2], 0xFF]) ^ 0x00FF_FFFF
                | 0xFF00_0000;
            let b = v.to_le_bytes();
            drow[si..si + 4].copy_from_slice(&b);
            sum = sum.wrapping_add(v as u64);
            if let Some(p) = prow {
                let pv = u32::from_le_bytes([p[si], p[si + 1], p[si + 2], p[si + 3]]);
                if (v ^ pv) & 0x00FF_FFFF != 0 {
                    diff = diff.wrapping_add(1);
                    bx0 = bx0.min(x as i32);
                    bx1 = bx1.max(x as i32);
                    by0 = by0.min(y as i32);
                    by1 = by1.max(y as i32);
                }
            }
        }
    }
    st.sums[slot] = sum;
    st.diffs[slot] = diff;
    st.dboxes[slot] = if bx1 < 0 {
        [0, 0, 0, 0]
    } else {
        let x0 = (bx0 - 2).max(0);
        let y0 = (by0 - 2).max(0);
        let x1 = (bx1 + 2).min(w - 1);
        let y1 = (by1 + 2).min(h - 1);
        [x0, y0, x1 - x0 + 1, y1 - y0 + 1]
    };
    st.pending = slot as i32;
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_invert_consume() {
    let mut st = INVERT.lock().unwrap_or_else(|e| e.into_inner());
    let slot = st.pending;
    if slot < 0 || slot >= 2 {
        return;
    }
    let slot = slot as usize;
    if st.surfs[slot].is_null() {
        return;
    }
    st.write = slot ^ 1;
    let changed = st.sums[slot] != st.present_sum;
    let diff_min = (st.w as u64) * (st.h as u64) / 500 + 1;
    let significant = changed && st.diffs[slot] > diff_min;
    if changed {
        st.present = slot;
        st.present_sum = st.sums[slot];
        st.pending = -1;
        unsafe {
            glaspen2_set_invert_background(st.surfs[slot]);
            cairo_surface_mark_dirty(st.surfs[slot]);
        }
    } else {
        st.pending = -1;
    }
    let stroke_active = unsafe { glaspen2_rust_pen_stroke_active() } != 0;
    if significant && !stroke_active {
        let b = st.dboxes[slot];
        unsafe {
            glaspen2_set_invert_dirty(b[0], b[1], b[2], b[3]);
        }
        unsafe { glaspen2_swift_pageview_rebuild() };
        unsafe {
            glaspen2_router_dirty_include_surface_rect(
                b[0] as f64,
                b[1] as f64,
                b[2] as f64,
                b[3] as f64,
            )
        };
        glaspen2_rust_overlay_flush_dirty();
    } else {
        unsafe { glaspen2_set_invert_dirty(-1, -1, -1, -1) };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_invert_ink_source(cr: *mut c_void) -> *mut c_void {
    let st = INVERT.lock().unwrap_or_else(|e| e.into_inner());
    if st.w <= 0 || st.surfs[st.present].is_null() {
        return std::ptr::null_mut();
    }
    let pat = unsafe { cairo_pattern_create_for_surface(st.surfs[st.present]) };
    if pat.is_null() {
        return pat;
    }
    let scale = glaspen2_rust_surface_scale().max(0.0001);
    let m = [1.0 / scale, 0.0, 0.0, 1.0 / scale, 0.0, 0.0];
    unsafe {
        cairo_pattern_set_matrix(pat, &m);
        cairo_set_source(cr, pat);
    }
    pat
}

// ── R171: 启动设置恢复 PartA/PartB ──

unsafe extern "C" {
    fn glaspen2_swift_update_menu_texts();
    fn glaspen2_router_startup_set_glass_follow(on: i32);
    fn glaspen2_load_settings_parts(sr: *mut f64, sg: *mut f64, sb: *mut f64, sw: *mut f64) -> i32;
    fn glaspen2_nearest_color_index(r: f64, g: f64, b: f64) -> i32;
    fn glaspen2_nearest_width_index(w: f64) -> i32;
    fn glaspen2_color_preset_rgb(index: i32, r: *mut f64, g: *mut f64, b: *mut f64);
    fn glaspen2_router_startup_glass(opacity: f64, on: i32);
    fn glaspen2_router_startup_grid(on: i32, size: f64, follow: i32, divider: i32);
    fn glaspen2_router_startup_shadow(on: i32);
    fn glaspen2_router_startup_outline(on: i32);
    fn glaspen2_router_startup_invert(fps: i32, ink: i32);
    fn glaspen2_router_startup_flip(v: i32);
    fn glaspen2_clamp_setting_int(key: *const std::os::raw::c_char, v: i32) -> i32;
    fn glaspen2_clamp_setting_double(key: *const std::os::raw::c_char, v: f64) -> f64;
    fn glaspen2_router_restore_gif_settings(fps: i32, resolution: f64, speed: f64, end_mode: i32);
    fn glaspen2_router_restore_chat_integration(on: i32);
    fn glaspen2_i18n(
        key: *const std::os::raw::c_char,
        lang: *const std::os::raw::c_char,
    ) -> *mut std::os::raw::c_char;
}

fn load_str_setting(key: &str) -> Option<String> {
    let c = std::ffi::CString::new(key).ok()?;
    let p = unsafe { glaspen2_load_string_setting(c.as_ptr()) };
    if p.is_null() {
        return None;
    }
    let s = unsafe { std::ffi::CStr::from_ptr(p) }
        .to_string_lossy()
        .into_owned();
    unsafe { glaspen2_free_c_string(p) };
    Some(s)
}

fn load_bool_setting(key: &str) -> bool {
    let Ok(c) = std::ffi::CString::new(key) else {
        return false;
    };
    unsafe { glaspen2_load_bool_setting(c.as_ptr()) != 0 }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_restore_settings_a() {
    if let Some(v) = load_str_setting("language") {
        unsafe { glaspen2_router_startup_set_lang(i32::from(v == "en")) };
        unsafe { glaspen2_swift_update_menu_texts() };
    }
    glaspen2_rust_pageview_update();

    let (mut sr, mut sg, mut sb, mut sw) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    if unsafe { glaspen2_load_settings_parts(&mut sr, &mut sg, &mut sb, &mut sw) } != 0 {
        let color_idx = unsafe { glaspen2_nearest_color_index(sr, sg, sb) };
        let width_idx = unsafe { glaspen2_nearest_width_index(sw) };
        let (mut r, mut g, mut b) = (sr, sg, sb);
        unsafe { glaspen2_color_preset_rgb(color_idx, &mut r, &mut g, &mut b) };
        unsafe { glaspen2_router_startup_apply_pen(r, g, b, sw, color_idx, width_idx) };
    }
    unsafe {
        glaspen2_router_update_status_icon_state();
        glaspen2_router_update_checkmarks();
    }
    glaspen2_rust_load_shortcuts();

    let glass_milli = unsafe {
        let c = std::ffi::CString::new("glass_alpha").unwrap();
        glaspen2_load_bool_setting(c.as_ptr())
    };
    let opacity = if glass_milli > 0 {
        glass_milli as f64 / 1000.0
    } else {
        0.0
    };
    let glass_on = load_bool_setting("glass_enabled");
    unsafe { glaspen2_router_startup_glass(opacity, i32::from(glass_on)) };
    glaspen2_rust_apply_glass();

    let grid_on = load_bool_setting("grid");
    let grid_size = load_str_setting("grid_size")
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|d| *d >= 10.0 && *d <= 200.0)
        .unwrap_or(0.0);
    let grid_follow = load_bool_setting("grid_follow_strokes");
    if let Some(v) = load_str_setting("glass_follow_strokes") {
        unsafe { glaspen2_router_startup_set_glass_follow(i32::from(v != "0")) };
    }
    let divider = load_str_setting("grid_divider")
        .and_then(|v| v.parse::<i32>().ok())
        .filter(|d| *d >= 0 && *d <= 4)
        .unwrap_or(-1);
    unsafe {
        glaspen2_router_startup_grid(
            i32::from(grid_on),
            grid_size,
            i32::from(grid_follow),
            divider,
        )
    };

    let shadow = load_bool_setting("soft_shadow");
    unsafe {
        glaspen2_router_startup_shadow(i32::from(shadow));
        glaspen2_set_soft_shadow(i32::from(shadow));
    }
    let outline = load_bool_setting("outline");
    unsafe {
        glaspen2_router_startup_outline(i32::from(outline));
        glaspen2_set_stroke_outline(i32::from(outline));
        glaspen2_router_update_menu_state(667, i32::from(outline));
    }

    let fps = load_str_setting("invert_fps")
        .and_then(|v| v.parse::<i32>().ok())
        .filter(|f| matches!(f, 10 | 30 | 60 | 100))
        .unwrap_or(-1);
    let invert_ink = load_bool_setting("invert_ink");
    unsafe {
        glaspen2_router_startup_invert(fps, i32::from(invert_ink));
        if invert_ink {
            glaspen2_router_invert_apply(1);
        }
    }

    let mut flip = load_str_setting("flip_effect")
        .and_then(|v| v.parse::<i32>().ok())
        .filter(|f| *f >= 0 && *f <= 1)
        .unwrap_or(-1);
    if let Ok(env) = std::env::var("GLASPEN2_FLIP_EFFECT") {
        flip = i32::from(env == "1");
    }
    if flip >= 0 {
        unsafe { glaspen2_router_startup_flip(flip) }
    }
    unsafe { glaspen2_bubble_set_capture(i32::from(load_bool_setting("show_bubble_tab"))) };
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_restore_settings_b() {
    let infinite = load_bool_setting("infinite_canvas");
    let minimap = load_bool_setting("minimap");
    let ethereal = load_bool_setting("ethereal");
    unsafe {
        glaspen2_router_restore_canvas_flags(
            i32::from(infinite),
            i32::from(minimap),
            i32::from(ethereal),
        )
    };
    unsafe {
        glaspen2_router_update_menu_state(778, i32::from(ethereal));
        let key = std::ffi::CString::new(if ethereal {
            "canvas.fixed_mode"
        } else {
            "canvas.ethereal_mode"
        })
        .unwrap();
        let lang = std::ffi::CString::new(if unsafe { glaspen2_router_lang() } == 1 {
            "en"
        } else {
            "zh"
        })
        .unwrap();
        let p = glaspen2_i18n(key.as_ptr(), lang.as_ptr());
        if !p.is_null() {
            glaspen2_menu_set_title(778, p);
            glaspen2_free_c_string(p);
        }
        glaspen2_router_update_menu_state(668, i32::from(infinite));
        glaspen2_set_canvas_kind(i32::from(infinite));
        if infinite {
            glaspen2_load_infinite_strokes();
        }
    }
    if infinite {
        glaspen2_rust_camera_load_infinite();
    } else {
        glaspen2_rust_camera_reset_lens();
    }

    let pressure = load_bool_setting("pressure_monitor");
    unsafe { glaspen2_router_restore_pressure(i32::from(pressure)) };
    if pressure {
        unsafe { glaspen2_swift_restore_pressure_window() }
    }

    let mut gif_fps = unsafe { glaspen2_router_gif_fps() };
    let mut gif_res = unsafe { glaspen2_router_gif_resolution() };
    let mut gif_speed = unsafe { glaspen2_router_gif_speed() };
    let mut gif_end = unsafe { glaspen2_router_gif_end_mode() };
    if let Some(v) = load_str_setting("gif_fps").and_then(|v| v.parse::<i32>().ok()) {
        gif_fps = v;
    }
    if let Some(v) = load_str_setting("gif_resolution").and_then(|v| v.parse::<f64>().ok()) {
        gif_res = v;
    }
    if let Some(v) = load_str_setting("gif_speed").and_then(|v| v.parse::<f64>().ok()) {
        gif_speed = v;
    }
    if let Some(v) = load_str_setting("gif_end_mode").and_then(|v| v.parse::<i32>().ok()) {
        gif_end = v;
    }
    let (k1, k2, k3, k4) = (
        std::ffi::CString::new("gifFps").unwrap(),
        std::ffi::CString::new("gifResolution").unwrap(),
        std::ffi::CString::new("gifSpeed").unwrap(),
        std::ffi::CString::new("gifEndMode").unwrap(),
    );
    gif_fps = unsafe { glaspen2_clamp_setting_int(k1.as_ptr(), gif_fps) };
    gif_res = unsafe { glaspen2_clamp_setting_double(k2.as_ptr(), gif_res) };
    gif_speed = unsafe { glaspen2_clamp_setting_double(k3.as_ptr(), gif_speed) };
    gif_end = unsafe { glaspen2_clamp_setting_int(k4.as_ptr(), gif_end) };
    unsafe { glaspen2_router_restore_gif_settings(gif_fps, gif_res, gif_speed, gif_end) };

    unsafe {
        glaspen2_router_update_status_icon_state();
        glaspen2_chat_auth_reload();
    }
    let chat = load_bool_setting("chat_integration");
    unsafe {
        glaspen2_router_restore_chat_integration(i32::from(chat));
        glaspen2_router_rec_init();
        if chat && load_bool_setting("share_ink") {
            glaspen2_share_ink_set_active(1);
        }
        glaspen2_swift_delayed_rebuild();
    }
}

// ── R173: 单实例 flock ──

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_instance_lock_acquire() -> i32 {
    if std::env::var_os("GLASPEN2_ALLOW_MULTI").is_some() {
        return -2;
    }
    let path = std::env::temp_dir().join("glaspen2.instance.lock");
    let Ok(cpath) = std::ffi::CString::new(path.to_string_lossy().as_bytes()) else {
        return -1;
    };
    let fd = unsafe { libc::open(cpath.as_ptr(), libc::O_CREAT | libc::O_RDWR, 0o666) };
    if fd < 0 {
        return -1;
    }
    if unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) } == 0 {
        return fd;
    }
    unsafe { libc::close(fd) };
    -1
}

// ── R174: 重复实例检测 ──

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_another_instance_running() -> i32 {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::NSString;
    let me = std::process::id() as i32;
    let workspace = unsafe { NSWorkspace::sharedWorkspace() };
    let apps = unsafe { workspace.runningApplications() };
    for idx in 0..apps.count() {
        let app = apps.objectAtIndex(idx);
        let pid: i32 = unsafe { objc2::msg_send![&*app, processIdentifier] };
        if pid == me {
            continue;
        }
        let name: *mut NSString = unsafe { objc2::msg_send![&*app, localizedName] };
        if !name.is_null() && unsafe { &*name }.to_string() == "glaspen2" {
            return 1;
        }
    }
    0
}

// ── R175: 隧道卡片工厂(先独立函数, 暂不接线) ──

unsafe extern "C" {
    fn glaspen2_paint_preview_into_surface(
        surface: *mut c_void,
        slot: i32,
        scale: f64,
        ox: f64,
        oy: f64,
        pscale: f64,
        alpha: f64,
        white_bg: i32,
        depth: f64,
    ) -> i32;
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_make_card(
    is_current: i32,
    depth: i32,
    cache_slot: i32,
    screen_id: i64,
    main_out: *mut *mut c_void,
    mip_out: *mut *mut c_void,
) -> i32 {
    if main_out.is_null() || mip_out.is_null() {
        return 0;
    }
    unsafe {
        *main_out = std::ptr::null_mut();
        *mip_out = std::ptr::null_mut();
        let screen_w = glaspen2_router_screen_w();
        let screen_h = glaspen2_router_screen_h();
        let g_scale = glaspen2_rust_surface_scale();
        let mut pscale = 1.0f64;
        let mut ox = 0.0f64;
        let mut oy = 0.0f64;
        let mut pw = screen_w as i32;
        let mut ph = screen_h as i32;
        if is_current == 0 {
            glaspen2_page_dims(screen_id, &mut pw, &mut ph);
            if pw <= 0 || ph <= 0 {
                pw = screen_w as i32;
                ph = screen_h as i32;
            }
            if pw != screen_w as i32 || ph != screen_h as i32 {
                pscale = (screen_w / pw as f64).min(screen_h / ph as f64);
                ox = (screen_w - pw as f64 * pscale) / 2.0;
                oy = (screen_h - ph as f64 * pscale) / 2.0;
            }
        }
        let card_scale = g_scale * 0.5;
        let sw = (screen_w * card_scale) as i32;
        let sh = (screen_h * card_scale) as i32;
        if sw <= 0 || sh <= 0 {
            return 0;
        }
        let surf = cairo_image_surface_create(0, sw, sh);
        if surf.is_null() || cairo_surface_status(surf) != 0 {
            if !surf.is_null() {
                cairo_surface_destroy(surf)
            }
            return 0;
        }
        if is_current != 0 {
            let current = glaspen2_rust_surface_current();
            if current.is_null() {
                cairo_surface_destroy(surf);
                return 0;
            }
            let cr = cairo_create(surf);
            if !cr.is_null() {
                let f = card_scale / g_scale.max(0.0001);
                cairo_scale(cr, f, f);
                cairo_set_operator(cr, CAIRO_OPERATOR_SOURCE);
                cairo_set_source_surface(cr, current, 0.0, 0.0);
                cairo_paint(cr);
                cairo_destroy(cr);
            }
        } else {
            let _ = glaspen2_paint_preview_into_surface(
                surf,
                cache_slot,
                card_scale,
                ox * 0.5,
                oy * 0.5,
                pscale,
                1.0,
                0,
                if depth == 0 {
                    -1.0
                } else {
                    (depth as f64).abs()
                },
            );
        }
        let d = depth as f64;
        let k = f64::from_bits(TS_K.load(std::sync::atomic::Ordering::Relaxed));
        let mut smax = 1.0 / (1.0 + k * (d.abs() - 1.0));
        if smax > 1.05 {
            smax = 1.05;
        }
        let tw = (glaspen2_rust_tunnel_get_surf_w() as f64 * smax) as i32;
        let th = (glaspen2_rust_tunnel_get_surf_h() as f64 * smax) as i32;
        let nw = cairo_image_surface_get_width(surf);
        let nh = cairo_image_surface_get_height(surf);
        let mut main_surf = surf;
        if tw > 16 && th > 16 && (tw < nw || th < nh) {
            let small = cairo_image_surface_create(0, tw, th);
            if !small.is_null() && cairo_surface_status(small) == 0 {
                let c2 = cairo_create(small);
                if !c2.is_null() {
                    cairo_scale(c2, tw as f64 / nw as f64, th as f64 / nh as f64);
                    cairo_set_source_surface(c2, surf, 0.0, 0.0);
                    cairo_paint(c2);
                    cairo_destroy(c2);
                }
                cairo_surface_destroy(surf);
                main_surf = small;
            } else if !small.is_null() {
                cairo_surface_destroy(small);
            }
        }
        let mw = cairo_image_surface_get_width(main_surf) / 2;
        let mh = cairo_image_surface_get_height(main_surf) / 2;
        let mut mip: *mut c_void = std::ptr::null_mut();
        if mw > 16 && mh > 16 {
            let m = cairo_image_surface_create(0, mw, mh);
            if !m.is_null() && cairo_surface_status(m) == 0 {
                let c3 = cairo_create(m);
                if !c3.is_null() {
                    cairo_scale(c3, 0.5, 0.5);
                    cairo_set_source_surface(c3, main_surf, 0.0, 0.0);
                    cairo_paint(c3);
                    cairo_destroy(c3);
                }
                mip = m;
            } else if !m.is_null() {
                cairo_surface_destroy(m);
            }
        }
        *main_out = main_surf;
        *mip_out = mip;
        1
    }
}

// ── R177: 主覆盖窗启动组装 ──

unsafe extern "C" {
    fn sw_app_screen_w() -> i32;
    fn sw_app_screen_h() -> i32;
    fn glaspen2_swift_set_main_anchors(window: *mut c_void, draw: *mut c_void, glass: *mut c_void);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_create_main_window() -> i32 {
    let w = unsafe { sw_app_screen_w() } as f64;
    let h = unsafe { sw_app_screen_h() } as f64;
    if w <= 0.0 || h <= 0.0 {
        return 0;
    }
    let window = glaspen2_rust_overlay_window_create(0.0, 0.0, w, h);
    if window.is_null() {
        return 0;
    }
    let content = glaspen2_rust_overlay_content_view_create(0.0, 0.0, w, h);
    if content.is_null() {
        return 0;
    }
    let enabled = unsafe { glaspen2_router_glass_enabled() } != 0;
    let opacity = unsafe { glaspen2_router_glass_opacity() };
    let glass =
        glaspen2_rust_overlay_glass_view_create(0.0, 0.0, w, h, i32::from(enabled), opacity);
    if glass.is_null() {
        return 0;
    }
    let draw = glaspen2_rust_draw_view_create(0.0, 0.0, w, h);
    if draw.is_null() {
        return 0;
    }
    glaspen2_rust_overlay_assemble_content(content, glass, draw, window);
    unsafe { glaspen2_swift_set_main_anchors(window, draw, glass) };
    glaspen2_router_ensure_surface();
    if unsafe { glaspen2_router_ethereal() } != 0 {
        glaspen2_rust_ethereal_restore_now();
    }
    tracing::info!(
        "[glaspen2] window ready {}x{}, ignoresMouseEvents=1",
        w as i32,
        h as i32
    );
    1
}

// ── R178: 菜单 C 薄桥迁 Rust ──

unsafe extern "C" {
    fn glaspen2_export_infinite_pdf_paged(w: i32, h: i32) -> i32;
    fn glaspen2_export_infinite_svg() -> i32;
    fn glaspen2_chord_reserved(chord: *const std::os::raw::c_char) -> i32;
    fn glaspen2_chord_modifier_mask(chord: *const std::os::raw::c_char) -> u64;
    fn glaspen2_shortcut_store_chord(
        action: *const std::os::raw::c_char,
    ) -> *mut std::os::raw::c_char;
    fn glaspen2_shortcut_store_is_empty() -> i32;
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_menu_screen_size(w: *mut i32, h: *mut i32) {
    unsafe {
        if !w.is_null() {
            *w = sw_app_screen_w();
        }
        if !h.is_null() {
            *h = sw_app_screen_h();
        }
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_menu_export_infinite_pdf_paged(w: i32, h: i32) -> i32 {
    unsafe { glaspen2_export_infinite_pdf_paged(w, h) }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_menu_export_infinite_svg() -> i32 {
    unsafe { glaspen2_export_infinite_svg() }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_menu_shutdown() {
    unsafe { glaspen2_shutdown() }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_sync_settings_panel() {
    unsafe { glaspen2_notify_settings_changed() }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_shortcut_reserved(chord: *const std::os::raw::c_char) -> i32 {
    if chord.is_null() {
        return 0;
    }
    unsafe { glaspen2_chord_reserved(chord) }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_shortcuts_effective_json() -> *mut std::os::raw::c_char {
    if unsafe { glaspen2_shortcut_store_is_empty() } != 0 {
        let _ = glaspen2_shortcut_apply_json(b"{}\0".as_ptr() as *const std::os::raw::c_char);
    }
    unsafe { glaspen2_shortcut_store_values_json() }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_action_for_chord(
    chord: *const std::os::raw::c_char,
) -> *mut std::os::raw::c_char {
    if chord.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { glaspen2_shortcut_store_action_for_chord(chord) }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_shortcut_chord(
    action: *const std::os::raw::c_char,
) -> *mut std::os::raw::c_char {
    if action.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { glaspen2_shortcut_store_chord(action) }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_menu_will_open_bridge() {
    unsafe {
        glaspen2_router_update_status_icon_state();
        glaspen2_router_update_checkmarks();
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_menu_did_close_bridge() {
    unsafe {
        glaspen2_router_update_status_icon_state();
        glaspen2_router_event_tap_enable();
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_camera_pan_mods() -> usize {
    let action = b"canvas_pan_up\0";
    let chord =
        unsafe { glaspen2_shortcut_store_chord(action.as_ptr() as *const std::os::raw::c_char) };
    if chord.is_null() {
        return 0;
    }
    let mask = unsafe { glaspen2_chord_modifier_mask(chord) };
    unsafe { glaspen2_free_c_string(chord) };
    mask as usize
}

// ── R179: 状态栏自检(status_bar.swift 删除后) ──

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_statusbar_selftest() -> i32 {
    use objc2::runtime::AnyObject;
    let item = glaspen2_rust_status_item_create();
    if item.is_null() {
        println!("SWIFT_STATUSBAR_FAIL");
        return 1;
    }
    let button: *mut AnyObject = unsafe { objc2::msg_send![item as *mut AnyObject, button] };
    if button.is_null() {
        println!("SWIFT_STATUSBAR_FAIL");
        return 1;
    }
    println!("SWIFT_STATUSBAR_OK items=1 button=1");
    0
}

// ── R180: 持久化诊断探针 ──

unsafe extern "C" {
    fn glaspen2_init_db(w: i32, h: i32);
    fn glaspen2_modeler_clear_buffer();
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_persist_probe() -> i32 {
    unsafe { glaspen2_init_db(1920, 1080) };
    unsafe { glaspen2_modeler_clear_buffer() };
    let t0 = glaspen2_rust_reference_now();
    glaspen2_rust_pen_down(200.0, 300.0, 0.5, t0, 1.0, 1.0);
    for i in 1..=40 {
        glaspen2_rust_pen_move(
            200.0 + i as f64 * 4.0,
            300.0,
            0.5,
            t0 + i as f64 * 0.01,
            1.0,
            1.0,
        );
    }
    glaspen2_rust_pen_up(360.0, 300.0, 0.5, t0 + 0.45);
    glaspen2_rust_pen_set_prox_device(1);
    let pen_as_eraser = glaspen2_router_prox_is_eraser();
    glaspen2_rust_pen_set_prox_device(3);
    let eraser_as_eraser = glaspen2_router_prox_is_eraser();
    glaspen2_rust_pen_set_prox_device(0);
    println!("PROX_PROBE pen1_eraser={pen_as_eraser} eraser3_eraser={eraser_as_eraser}");
    std::thread::sleep(std::time::Duration::from_secs(1));
    let count = unsafe { glaspen2_stroke_count() };
    println!("PERSIST_PROBE_DONE strokes={count}");
    i32::from(count == 1) - 1
}

fn glaspen2_rust_reference_now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64() - 978_307_200.0)
        .unwrap_or(0.0)
}

// ── R181: 页面导航诊断探针 ──

unsafe extern "C" {
    fn glaspen2_debug_insert_pages(n: i32, w: i32, h: i32) -> i32;
    fn glaspen2_create_next_page(id: i64) -> i64;
    fn glaspen2_delete_screen(id: i64) -> i32;
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pages_probe() -> i32 {
    let mut failures = 0i32;
    let mut check = |name: &str, ok: bool| {
        if ok {
            println!("PAGES_PROBE {name} ok");
        } else {
            tracing::warn!("[pages-probe] FAIL {name}");
            failures += 1;
        }
    };
    unsafe { glaspen2_init_db(1920, 1080) };
    let current = unsafe { glaspen2_get_current_screen_id() };
    check("current_page", current > 0);
    let inserted = unsafe { glaspen2_debug_insert_pages(2, 1920, 1080) };
    check("debug_insert_pages", inserted == 2);
    let neighbor = unsafe { glaspen2_page_neighbor(current, 1) };
    check("page_neighbor", neighbor > 0);
    let created = unsafe { glaspen2_create_next_page(current) };
    check("create_next_page", created > 0);
    let (mut pw, mut ph) = (0i32, 0i32);
    unsafe { glaspen2_page_dims(created, &mut pw, &mut ph) };
    check("page_dims", pw == 1920 && ph == 1080);
    check(
        "stroke_count_empty",
        unsafe { glaspen2_stroke_count() } == 0,
    );
    let (mut rect, mut bounds, mut len) = ([0.0f64; 4], [0.0f64; 4], 0i32);
    let payload = unsafe {
        glaspen2_rust_canvas_overview(
            800.0,
            600.0,
            rect.as_mut_ptr(),
            bounds.as_mut_ptr(),
            &mut len,
        )
    };
    check("canvas_payload_empty", payload.is_null() && len == 0);
    if !payload.is_null() {
        unsafe { glaspen2_free_rust_bytes(payload, len) }
    }
    check(
        "delete_page",
        unsafe { glaspen2_delete_screen(created) } == 1,
    );
    println!("SWIFT_PAGES_PROBE_OK failures={failures}");
    i32::from(failures == 0) - 1
}

// ── R182: 设置诊断探针 ──

unsafe extern "C" {
    fn glaspen2_settings_snapshot_json(
        overrides: *const std::os::raw::c_char,
    ) -> *mut std::os::raw::c_char;
    fn glaspen2_router_invert_fps() -> i32;
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_settings_probe() -> i32 {
    let mut failures = 0i32;
    let mut check = |name: &str, ok: bool| {
        if ok {
            println!("SETTINGS_PROBE {name} ok");
        } else {
            tracing::warn!("[settings-probe] FAIL {name}");
            failures += 1;
        }
    };
    let apply = |key: &str, value: &str| {
        let k = std::ffi::CString::new(key).unwrap();
        let v = std::ffi::CString::new(value).unwrap();
        let _ = glaspen2_rust_apply_setting(k.as_ptr(), v.as_ptr());
    };
    apply("gridSize", "55");
    check("gridSize", unsafe { glaspen2_router_grid_size() } == 55.0);
    apply("gridDivider", "2");
    check(
        "gridDivider",
        unsafe { glaspen2_router_grid_divider() } == 2,
    );
    apply("language", "\"en\"");
    check("language", unsafe { glaspen2_router_lang() } == 1);
    apply("minimap", "true");
    check("minimap", unsafe { glaspen2_router_minimap_enabled() } != 0);
    apply("flipEffect", "1");
    check("flipEffect", unsafe { glaspen2_router_flip_effect() } == 1);
    apply("gifFps", "30");
    check("gifFps", unsafe { glaspen2_router_gif_fps() } == 30);
    apply("softShadow", "true");
    check("softShadow", unsafe { glaspen2_router_soft_shadow() } != 0);
    apply("outline", "true");
    check("outline", unsafe { glaspen2_router_outline_enabled() } != 0);
    apply("frostedGlass", "true");
    check(
        "frostedGlass",
        unsafe { glaspen2_router_glass_enabled() } != 0,
    );
    apply("invertFps", "60");
    check("invertFps", unsafe { glaspen2_router_invert_fps() } == 60);
    apply("color", "2");
    check("color", unsafe { glaspen2_router_selected_color() } == 2);
    apply("width", "5");
    check("width", unsafe { glaspen2_router_selected_width() } == 5);
    apply("gridFollowStrokes", "true");
    check(
        "gridFollowStrokes",
        unsafe { glaspen2_router_grid_follow() } != 0,
    );
    unsafe {
        glaspen2_router_set_enabled_state(0);
        check("drawDisabled", glaspen2_router_enabled() == 0);
        glaspen2_router_set_enabled_state(1);
        check("drawEnabled", glaspen2_router_enabled() != 0);
        glaspen2_router_set_outline_state(0);
        check("outlineDisabled", glaspen2_router_outline_enabled() == 0);
    }
    let overrides = unsafe { glaspen2_menu_snapshot_overrides_json() };
    if overrides.is_null() {
        tracing::warn!("[settings-probe] FAIL settings_json nil");
        return 1;
    }
    let snapshot = unsafe { glaspen2_settings_snapshot_json(overrides) };
    unsafe { libc::free(overrides as *mut c_void) };
    if snapshot.is_null() {
        tracing::warn!("[settings-probe] FAIL settings_json nil");
        return 1;
    }
    let text = unsafe { std::ffi::CStr::from_ptr(snapshot) }
        .to_string_lossy()
        .into_owned();
    unsafe { glaspen2_free_c_string(snapshot) };
    check("settings_json.gridSize", text.contains("\"gridSize\""));
    check("settings_json.language", text.contains("\"language\""));
    println!("SWIFT_SETTINGS_PROBE_OK failures={failures}");
    i32::from(failures == 0) - 1
}

// ── R183: 快捷键诊断探针 ──

unsafe extern "C" {
    fn glaspen2_router_set_ethereal(on: i32);
}

fn hotkey_action(chord: &str) -> Option<String> {
    let c = std::ffi::CString::new(chord).ok()?;
    let p = unsafe { glaspen2_shortcut_store_action_for_chord(c.as_ptr()) };
    if p.is_null() {
        return None;
    }
    let s = unsafe { std::ffi::CStr::from_ptr(p) }
        .to_string_lossy()
        .into_owned();
    unsafe { glaspen2_free_c_string(p) };
    Some(s)
}

fn hotkey_chord(action: &str) -> Option<String> {
    let c = std::ffi::CString::new(action).ok()?;
    let p = unsafe { glaspen2_shortcut_store_chord(c.as_ptr()) };
    if p.is_null() {
        return None;
    }
    let s = unsafe { std::ffi::CStr::from_ptr(p) }
        .to_string_lossy()
        .into_owned();
    unsafe { glaspen2_free_c_string(p) };
    Some(s)
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_hotkeys_probe() -> i32 {
    glaspen2_rust_load_shortcuts();
    let mut failures = 0i32;
    let defaults_text = {
        let p = glaspen2_rust_shortcut_defaults_json();
        if p.is_null() {
            String::new()
        } else {
            unsafe { std::ffi::CStr::from_ptr(p) }
                .to_string_lossy()
                .into_owned()
        }
    };
    let actions_text = {
        let p = glaspen2_rust_shortcut_actions_json();
        if p.is_null() {
            String::new()
        } else {
            unsafe { std::ffi::CStr::from_ptr(p) }
                .to_string_lossy()
                .into_owned()
        }
    };
    let defaults: serde_json::Value =
        serde_json::from_str(&defaults_text).unwrap_or(serde_json::json!({}));
    let actions: Vec<String> =
        serde_json::from_str::<Vec<String>>(&actions_text).unwrap_or_default();
    let mut checked = 0i32;
    for action in actions {
        let Some(chord) = defaults.get(&action).and_then(|v| v.as_str()) else {
            continue;
        };
        if chord.is_empty() {
            continue;
        }
        checked += 1;
        if hotkey_action(chord).as_deref() != Some(action.as_str()) {
            tracing::warn!(
                "[hotkeys-probe] FAIL {action}={chord} -> {:?}",
                hotkey_action(chord)
            );
            failures += 1;
        }
    }
    if hotkey_action("cmd+ctrl+D").as_deref() != Some("settings") {
        tracing::warn!("[hotkeys-probe] FAIL cmd+ctrl+D -> settings");
        failures += 1;
    }
    let _ = unsafe {
        glaspen2_shortcut_apply_json(b"{\"undo\":\"\"}\0".as_ptr() as *const std::os::raw::c_char)
    };
    if hotkey_action("cmd+ctrl+Z").is_some() {
        tracing::warn!("[hotkeys-probe] FAIL explicit unbind fell back");
        failures += 1;
    }
    if hotkey_chord("undo").as_deref() != Some("") {
        tracing::warn!("[hotkeys-probe] FAIL explicit unbind not empty");
        failures += 1;
    }
    if unsafe { glaspen2_router_prev_color_index() } != 9
        || unsafe { glaspen2_router_prev_width_index() } != 2
    {
        tracing::warn!("[hotkeys-probe] FAIL swap defaults");
        failures += 1;
    }
    unsafe {
        glaspen2_router_set_ethereal(1);
        glaspen2_router_set_strokes_visible(1);
    }
    let hidden = glaspen2_rust_ethereal_hide_now() != 0;
    if !hidden || unsafe { glaspen2_router_strokes_visible() } != 0 {
        tracing::warn!("[hotkeys-probe] FAIL ethereal hide");
        failures += 1;
    }
    unsafe {
        glaspen2_router_set_ethereal(0);
        glaspen2_router_set_strokes_visible(1);
        glaspen2_router_startup_glass(0.0, 1);
    }
    if unsafe { glaspen2_router_glass_opacity() } <= 0.0 {
        tracing::warn!("[hotkeys-probe] FAIL glass default opacity overwritten");
        failures += 1;
    }
    println!("HOTKEYS_PROBE checked={checked} failures={failures}");
    i32::from(failures == 0) - 1
}

// ── R184: 渲染诊断探针 ──

unsafe extern "C" {
    fn glaspen2_get_stroke_point_count(index: i32) -> i32;
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_render_probe() -> i32 {
    let mut failures = 0i32;
    let mut check = |name: &str, ok: bool| {
        if ok {
            println!("RENDER_PROBE {name} ok");
        } else {
            tracing::warn!("[render-probe] FAIL {name}");
            failures += 1;
        }
    };
    unsafe { glaspen2_init_db(1920, 1080) };
    unsafe { glaspen2_modeler_clear_buffer() };
    let t0 = glaspen2_rust_reference_now();
    unsafe { glaspen2_modeler_begin(1.0, 0.0, 0.0, 200.0, 300.0, 0.5, t0, 1.0) };
    for i in 1..=60 {
        let x = 200.0 + i as f64 * 4.0;
        let y = 300.0 + (i as f64 / 9.0).sin() * 80.0;
        unsafe { glaspen2_modeler_move(x, y, 0.5, t0 + i as f64 * 0.008, 1.0) };
    }
    unsafe { glaspen2_modeler_end(440.0, 300.0, 0.5, t0 + 0.5, 1.0) };
    unsafe { glaspen2_modeler_commit_to_strokes(1.0, 0.0, 0.0) };
    check("stroke_count", unsafe { glaspen2_stroke_count() } == 1);
    check(
        "stroke_points",
        unsafe { glaspen2_get_stroke_point_count(0) } > 10,
    );
    let (mut rect, mut bounds, mut len) = ([0.0f64; 4], [0.0f64; 4], 0i32);
    let png = unsafe {
        glaspen2_rust_canvas_overview(
            800.0,
            600.0,
            rect.as_mut_ptr(),
            bounds.as_mut_ptr(),
            &mut len,
        )
    };
    check("overview_non_nil", !png.is_null());
    check("overview_png", !png.is_null() && len > 0);
    check("overview_rect", rect.iter().any(|v| *v != 0.0));
    check("overview_bounds", bounds.iter().any(|v| *v != 0.0));
    if !png.is_null() {
        unsafe { glaspen2_free_rust_bytes(png, len) }
    }
    let (mut rect2, mut bounds2, mut len2) = ([0.0f64; 4], [0.0f64; 4], 0i32);
    let payload = unsafe {
        glaspen2_rust_canvas_overview(
            800.0,
            600.0,
            rect2.as_mut_ptr(),
            bounds2.as_mut_ptr(),
            &mut len2,
        )
    };
    check("canvas_payload_non_nil", !payload.is_null());
    check("canvas_payload_len", len2 > 0);
    if !payload.is_null() {
        unsafe { glaspen2_free_rust_bytes(payload, len2) }
    }
    check("canvas_payload_rect", rect2[2] > 0.0 && rect2[3] > 0.0);
    glaspen2_rust_finish_active_stroke();
    unsafe {
        glaspen2_clear_strokes(
            glaspen2_router_screen_w() as i32,
            glaspen2_router_screen_h() as i32,
        )
    };
    glaspen2_rust_camera_home();
    let (mut r3, mut b3, mut l3) = ([0.0f64; 4], [0.0f64; 4], 0i32);
    let p3 = unsafe {
        glaspen2_rust_canvas_overview(800.0, 600.0, r3.as_mut_ptr(), b3.as_mut_ptr(), &mut l3)
    };
    if !p3.is_null() {
        unsafe { glaspen2_free_rust_bytes(p3, l3) }
    }
    check("clear_action", unsafe { glaspen2_stroke_count() } == 0);
    println!("SWIFT_RENDER_PROBE_OK failures={failures}");
    i32::from(failures == 0) - 1
}

// ── R185: 虚拟笔诊断 ──

#[repr(C)]
struct CGPoint2 {
    x: f64,
    y: f64,
}

unsafe extern "C" {
    fn CGEventCreateMouseEvent(
        source: *mut c_void,
        mouse_type: u32,
        pos: CGPoint2,
        button: u32,
    ) -> *mut c_void;
    fn CGEventSetIntegerValueField(event: *mut c_void, field: u32, value: i64);
    fn CGEventPost(tap: u32, event: *mut c_void);
}

fn virtual_pen_post(kind: u32, x: f64, y: f64, pressure: f64) {
    let ev = unsafe { CGEventCreateMouseEvent(std::ptr::null_mut(), kind, CGPoint2 { x, y }, 0) };
    if ev.is_null() {
        return;
    }
    unsafe {
        CGEventSetIntegerValueField(ev, 7, 1); // NSEventSubtype.tabletPoint
        CGEventSetIntegerValueField(ev, 42, 0x5650_4E31); // kVirtualPenUserData
        CGEventSetIntegerValueField(ev, 0x2F, (pressure * 65535.0) as i64);
        CGEventPost(0, ev); // kCGHIDEventTap
        CFRelease(ev);
    }
}

fn virtual_pen_run() {
    let mut batch = 0i32;
    loop {
        let mut s = 0i32;
        while s < 3 && unsafe { glaspen2_router_enabled() } != 0 {
            let w = unsafe { glaspen2_router_screen_w() };
            let h = unsafe { glaspen2_router_screen_h() };
            let x0 = w * 0.22 + s as f64 * w * 0.18;
            let y0 = h * 0.35;
            tracing::info!("[glaspen2] virtual pen: batch {batch} stroke {s}");
            let mut i = 0i32;
            while i < 100 && unsafe { glaspen2_router_enabled() } != 0 {
                let t = i as f64 / 100.0;
                virtual_pen_post(5, x0 + t * w * 0.22, y0, 0.0);
                std::thread::sleep(std::time::Duration::from_millis(5));
                i += 1;
            }
            let n = 400i32;
            i = 0;
            while i <= n && unsafe { glaspen2_router_enabled() } != 0 {
                let t = i as f64 / n as f64;
                let x = x0 + t * w * 0.22;
                let y = y0 + (t * 9.42478).sin() * h * 0.12;
                let p = 0.4 + 0.5 * (t * 12.566).sin().abs();
                virtual_pen_post(if i == 0 { 1 } else { 6 }, x, y, p);
                std::thread::sleep(std::time::Duration::from_millis(5));
                i += 1;
            }
            virtual_pen_post(2, x0 + w * 0.22, y0, 0.0);
            i = 0;
            while i < 100 && unsafe { glaspen2_router_enabled() } != 0 {
                virtual_pen_post(5, x0 + w * 0.22 - i as f64 * 2.0, y0 + i as f64, 0.0);
                std::thread::sleep(std::time::Duration::from_millis(5));
                i += 1;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
            s += 1;
        }
        batch += 1;
        std::thread::sleep(std::time::Duration::from_secs(9));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_virtual_pen_maybe_start() {
    let Ok(v) = std::env::var("GLASPEN2_VIRTUAL_PEN") else {
        return;
    };
    if v.is_empty() || v == "0" {
        return;
    }
    tracing::info!("[glaspen2] virtual pen enabled (GLASPEN2_VIRTUAL_PEN): 每 12 秒画 3 笔");
    std::thread::spawn(virtual_pen_run);
}

// ── R187: 辅助功能权限告警 ──

fn accessibility_alert_text(
    zh: bool,
) -> (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
) {
    if zh {
        (
            "需要辅助功能权限",
            "请在系统设置 → 隐私与安全性 → 辅助功能中添加并勾选 glaspen2。\n\n添加后请点击下方按钮重启应用。",
            "打开系统设置并重启",
            "取消",
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
        )
    } else {
        (
            "Accessibility Permission Required",
            "Please add and enable glaspen2 in System Settings → Privacy & Security → Accessibility.\n\nAfter enabling, click below to restart.",
            "Open Settings & Restart",
            "Cancel",
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
        )
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_accessibility_alert_present() {
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2_foundation::NSString;
    unsafe {
        if glaspen2_router_event_tap_ok() != 0 {
            tracing::info!("[glaspen2] CGEventTap created OK");
            return;
        }
        tracing::warn!("[glaspen2] CGEventTap FAILED - need Accessibility permission");
        let (title, info, ok, cancel, _url) = accessibility_alert_text(glaspen2_router_lang() != 1);
        let Some(cls) = AnyClass::get(c"NSAlert") else {
            return;
        };
        let alert: *mut AnyObject = objc2::msg_send![cls, new];
        if alert.is_null() {
            return;
        }
        let t = NSString::from_str(title);
        let i = NSString::from_str(info);
        let b1 = NSString::from_str(ok);
        let b2 = NSString::from_str(cancel);
        let _: () = objc2::msg_send![alert, setMessageText: &*t];
        let _: () = objc2::msg_send![alert, setInformativeText: &*i];
        let _: *mut AnyObject = objc2::msg_send![alert, addButtonWithTitle: &*b1];
        let _: *mut AnyObject = objc2::msg_send![alert, addButtonWithTitle: &*b2];
        let resp: i64 = objc2::msg_send![alert, runModal];
        if resp != 1000 {
            return;
        }
        let Some(url_cls) = AnyClass::get(c"NSURL") else {
            return;
        };
        let url_str = NSString::from_str(
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
        );
        let url: *mut AnyObject = objc2::msg_send![url_cls, URLWithString: &*url_str];
        let Some(ws_cls) = AnyClass::get(c"NSWorkspace") else {
            return;
        };
        let ws: *mut AnyObject = objc2::msg_send![ws_cls, sharedWorkspace];
        if !ws.is_null() && !url.is_null() {
            let _: bool = objc2::msg_send![ws, openURL: url];
        }
        if let Some(bundle_cls) = AnyClass::get(c"NSBundle") {
            let bundle: *mut AnyObject = objc2::msg_send![bundle_cls, mainBundle];
            let path: *mut NSString = if bundle.is_null() {
                std::ptr::null_mut()
            } else {
                objc2::msg_send![bundle, bundlePath]
            };
            if !ws.is_null() && !path.is_null() {
                let _: bool = objc2::msg_send![ws, launchApplication: path];
            }
        }
        if let Some(app_cls) = AnyClass::get(c"NSApplication") {
            let app: *mut AnyObject = objc2::msg_send![app_cls, sharedApplication];
            if !app.is_null() {
                let _: () = objc2::msg_send![app, terminate: std::ptr::null_mut::<AnyObject>()];
            }
        }
    }
}

unsafe extern "C" {
    fn glaspen2_router_event_tap_reinstall();
}

// ── R188: i18n 文本/描边 pad/光标失效 ──

fn i18n_text_owned(key: &str) -> *mut std::os::raw::c_char {
    let Ok(k) = std::ffi::CString::new(key) else {
        return std::ptr::null_mut();
    };
    let lang = if unsafe { glaspen2_router_lang() } == 1 {
        "en"
    } else {
        "zh"
    };
    let Ok(l) = std::ffi::CString::new(lang) else {
        return std::ptr::null_mut();
    };
    let p = unsafe { glaspen2_i18n(k.as_ptr(), l.as_ptr()) };
    let text = if p.is_null() {
        key.to_string()
    } else {
        let s = unsafe { std::ffi::CStr::from_ptr(p) }
            .to_string_lossy()
            .into_owned();
        unsafe { glaspen2_free_c_string(p) };
        if s.is_empty() { key.to_string() } else { s }
    };
    let Ok(c) = std::ffi::CString::new(text) else {
        return std::ptr::null_mut();
    };
    unsafe { libc::strdup(c.as_ptr()) }
}

#[unsafe(no_mangle)]
pub extern "C" fn sw_text(key: *const std::os::raw::c_char) -> *mut std::os::raw::c_char {
    if key.is_null() {
        return std::ptr::null_mut();
    }
    let s = unsafe { std::ffi::CStr::from_ptr(key) }
        .to_string_lossy()
        .into_owned();
    i18n_text_owned(&s)
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_i18n_textf(
    key: *const std::os::raw::c_char,
    placeholder: *const std::os::raw::c_char,
    value: *const std::os::raw::c_char,
) -> *mut std::os::raw::c_char {
    if key.is_null() || placeholder.is_null() || value.is_null() {
        return std::ptr::null_mut();
    }
    let key_s = unsafe { std::ffi::CStr::from_ptr(key) }
        .to_string_lossy()
        .into_owned();
    let ph = unsafe { std::ffi::CStr::from_ptr(placeholder) }
        .to_string_lossy()
        .into_owned();
    let val = unsafe { std::ffi::CStr::from_ptr(value) }
        .to_string_lossy()
        .into_owned();
    let p = i18n_text_owned(&key_s);
    if p.is_null() {
        return p;
    }
    let text = unsafe { std::ffi::CStr::from_ptr(p) }
        .to_string_lossy()
        .into_owned();
    unsafe { libc::free(p as *mut c_void) };
    let replaced = text.replace(&format!("{{{ph}}}"), &val);
    let Ok(c) = std::ffi::CString::new(replaced) else {
        return std::ptr::null_mut();
    };
    unsafe { libc::strdup(c.as_ptr()) }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_outline_pad() -> f64 {
    1.0
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_cursor_invalidate() {
    unsafe {
        glaspen2_rust_dirty_include_point(
            glaspen2_router_cursor_x(),
            glaspen2_router_cursor_y(),
            14.0,
        );
    }
}

// ── R189: 快捷键录制定时器 ──

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_hotkey_recording_set(on: i32) {
    let generation = glaspen2_rust_recording_set(on);
    if on == 0 {
        return;
    }
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(60));
        if glaspen2_rust_recording_timeout(generation) != 0 {
            tracing::warn!("[hotkey] 录制超时, 恢复全局快捷键");
        }
    });
}

// ── R191: 设置面板打开 URL ──

unsafe extern "C" {
    fn glaspen2_swift_run_on_main_sync(ctx: *mut c_void, cb: extern "C" fn(*mut c_void));
}

extern "C" fn open_url_on_main(ctx: *mut c_void) {
    use objc2::runtime::AnyObject;
    if ctx.is_null() {
        return;
    }
    let url = unsafe { Box::from_raw(ctx as *mut usize) };
    unsafe {
        let Some(ws_cls) = objc2::runtime::AnyClass::get(c"NSWorkspace") else {
            return;
        };
        let ws: *mut AnyObject = objc2::msg_send![ws_cls, sharedWorkspace];
        if !ws.is_null() {
            let _: bool = objc2::msg_send![ws, openURL: *url as *mut AnyObject];
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_macos_open_url(url_c: *const std::os::raw::c_char) {
    use objc2::runtime::AnyObject;
    use objc2_foundation::NSString;
    if url_c.is_null() {
        return;
    }
    let raw = unsafe { std::ffi::CStr::from_ptr(url_c) }
        .to_string_lossy()
        .into_owned();
    unsafe {
        let Some(url_cls) = objc2::runtime::AnyClass::get(c"NSURL") else {
            return;
        };
        let s = NSString::from_str(&raw);
        let url: *mut AnyObject = objc2::msg_send![url_cls, URLWithString: &*s];
        if url.is_null() {
            tracing::warn!("[glaspen2] open_url: URL 解析失败: {raw}");
            return;
        }
        let ctx = Box::into_raw(Box::new(url as usize)) as *mut c_void;
        glaspen2_swift_run_on_main_sync(ctx, open_url_on_main);
    }
}

// ── R194: 显示器变更防抖线程 ──

static DISPLAY_DEBOUNCE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_display_changed() {
    let new_w = unsafe { sw_app_screen_w() };
    let new_h = unsafe { sw_app_screen_h() };
    if new_w <= 0 || new_h <= 0 {
        return;
    }
    if glaspen2_rust_display_begin(new_w, new_h) == 0 {
        return;
    }
    tracing::info!("[glaspen2] display reconfig: Rust 防抖 2.5s -> {new_w}x{new_h}");
    let generation = DISPLAY_DEBOUNCE.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(2500));
        if DISPLAY_DEBOUNCE.load(std::sync::atomic::Ordering::SeqCst) != generation {
            return;
        }
        if glaspen2_rust_display_finish() != 0 {
            tracing::info!("[glaspen2] display apply by Rust");
        }
    });
}

// ── R197: 应用启动编排 ──

unsafe extern "C" {
    fn glaspen2_swift_bootstrap_prepare();
    fn glaspen2_swift_bootstrap_delegate_menu(item: *mut c_void) -> i32;
    fn glaspen2_swift_bootstrap_display_observer();
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_app_bootstrap_start() -> i32 {
    let lock_fd = glaspen2_rust_instance_lock_acquire();
    if lock_fd == -1 || glaspen2_rust_another_instance_running() != 0 {
        unsafe { glaspen2_swift_bootstrap_running_alert() };
        return 0;
    }
    unsafe { glaspen2_swift_bootstrap_prepare() };
    let status = glaspen2_rust_status_item_create();
    let status = if status.is_null() {
        unsafe { glaspen2_swift_bootstrap_status_item() }
    } else {
        status
    };
    if status.is_null() {
        return 0;
    }
    if unsafe { glaspen2_swift_bootstrap_delegate_menu(status) } == 0 {
        return 0;
    }
    let (mut w, mut h) = (0i32, 0i32);
    unsafe { glaspen2_swift_bootstrap_screen_size(&mut w, &mut h) };
    if w <= 0 || h <= 0 {
        w = unsafe { sw_app_screen_w() };
        h = unsafe { sw_app_screen_h() };
    }
    unsafe {
        glaspen2_router_set_screen_size(w, h);
        glaspen2_init_db(w, h);
    }
    glaspen2_rust_restore_settings_a();
    glaspen2_rust_restore_settings_b();
    if glaspen2_rust_create_main_window() == 0 {
        return 0;
    }
    unsafe {
        glaspen2_router_install_signals();
        glaspen2_swift_bootstrap_display_observer();
        glaspen2_router_event_tap_reinstall();
        glaspen2_rust_accessibility_alert_present();
        glaspen2_rust_startup_run_loop();
    }
    if std::env::var("GLASPEN2_OPEN_SETTINGS").as_deref() == Ok("1") {
        glaspen2_rust_show_settings_panel();
    }
    if std::env::var("GLASPEN2_PM_PROBE").as_deref() == Ok("1") {
        glaspen2_rust_pm_probe();
    }
    if std::env::var("GLASPEN2_DRAW_PROBE").as_deref() == Ok("1") {
        glaspen2_rust_toggle_draw_enabled();
        glaspen2_rust_toggle_draw_enabled();
        glaspen2_rust_toggle_canvas_mode();
        glaspen2_rust_ethereal_peek(0.1);
        glaspen2_rust_toggle_canvas_mode();
        tracing::info!("[draw-probe] toggles done");
    }
    1
}

// ── R198: 设置面板宿主/Flutter VC 缓存 ──

static SETTINGS_VC: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

unsafe extern "C" {
    fn glaspen2_swift_make_flutter_vc() -> *mut c_void;
    fn glaspen2_swift_activate_settings_existing();
    fn glaspen2_swift_show_settings_window(window: *mut c_void);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_flutter_vc() -> *mut c_void {
    let ptr = SETTINGS_VC.load(std::sync::atomic::Ordering::SeqCst);
    if ptr != 0 {
        return ptr as *mut c_void;
    }
    let p = unsafe { glaspen2_swift_make_flutter_vc() };
    if !p.is_null() {
        SETTINGS_VC.store(p as usize, std::sync::atomic::Ordering::SeqCst);
    }
    p
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_show_settings_panel() {
    unsafe {
        if !sw_settings_window().is_null() {
            glaspen2_swift_activate_settings_existing();
            return;
        }
    }
    let vc = glaspen2_rust_flutter_vc();
    if vc.is_null() {
        return;
    }
    let title = if unsafe { glaspen2_router_lang() } == 0 {
        "玻璃涂鸦"
    } else {
        "glaspen2"
    };
    let Ok(title_c) = std::ffi::CString::new(title) else {
        return;
    };
    let window = glaspen2_rust_settings_window_create(title_c.as_ptr(), vc, 900.0, 1200.0);
    if window.is_null() {
        return;
    }
    unsafe {
        tracing::info!(
            "[settings] panel window created vc={:?} window={:?}",
            vc,
            window
        );
        sw_set_settings_window(window);
        glaspen2_swift_show_settings_window(window);
        let key = std::ffi::CString::new("panel_transparent").unwrap();
        if glaspen2_load_bool_setting(key.as_ptr()) != 0 {
            glaspen2_router_settings_apply_transparent();
        }
    }
}

// ── R199: 通知 presenter ──

unsafe extern "C" {
    fn glaspen2_page_info_text(
        screen_id: i64,
        lang: *const std::os::raw::c_char,
    ) -> *mut std::os::raw::c_char;
}

fn notification_show_owned(text: &str) {
    let Ok(c) = std::ffi::CString::new(text) else {
        return;
    };
    let generation = unsafe { glaspen2_rust_notification_set(c.as_ptr()) };
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(1));
        unsafe { glaspen2_rust_notification_clear_if(generation) };
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_notification_show(text: *const std::os::raw::c_char) {
    if text.is_null() {
        return;
    }
    let s = unsafe { std::ffi::CStr::from_ptr(text) }
        .to_string_lossy()
        .into_owned();
    notification_show_owned(&s);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_notification_show_page_info(screen_id: i64) {
    let lang = unsafe { glaspen2_router_lang_code() };
    let p = unsafe { glaspen2_page_info_text(screen_id, lang) };
    if p.is_null() {
        return;
    }
    let text = unsafe { std::ffi::CStr::from_ptr(p) }
        .to_string_lossy()
        .into_owned();
    unsafe { glaspen2_free_c_string(p) };
    if !text.is_empty() {
        notification_show_owned(&text);
    }
}

// ── R200: 涂鸦启停/画布模式切换 ──

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_toggle_draw_enabled() {
    let on = unsafe { glaspen2_router_enabled() } == 0;
    let (title_zh, title_en) = if on {
        ("关闭涂鸦", "Disable Drawing")
    } else {
        ("开启涂鸦", "Enable Drawing")
    };
    let title = if unsafe { glaspen2_router_lang() } == 1 {
        title_en
    } else {
        title_zh
    };
    let hud_key = if on { "draw.enabled" } else { "draw.disabled" };
    let hud = i18n_text_owned(hud_key);
    let Ok(t) = std::ffi::CString::new(title) else {
        return;
    };
    if hud.is_null() {
        return;
    }
    unsafe {
        let _ = glaspen2_rust_toggle_draw(t.as_ptr(), hud);
        libc::free(hud as *mut c_void);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_toggle_canvas_mode() {
    unsafe { glaspen2_router_peek_cancel() };
    tracing::debug!(
        "[canvas-mode] before ethereal={} visible={} glass_follow={} strokes={}",
        unsafe { glaspen2_router_ethereal() },
        unsafe { glaspen2_router_strokes_visible() },
        unsafe { glaspen2_router_glass_follow() },
        unsafe { glaspen2_stroke_count() }
    );
    if unsafe { glaspen2_router_ethereal() } == 0 {
        unsafe {
            glaspen2_router_set_ethereal(1);
            glaspen2_rust_finish_active_stroke();
            glaspen2_router_set_strokes_visible(0);
        }
        if unsafe { glaspen2_router_glass_follow() } != 0 {
            let glass = unsafe { sw_glass_view() };
            if !glass.is_null() {
                unsafe {
                    let _: () =
                        objc2::msg_send![glass as *mut objc2::runtime::AnyObject, setHidden: true];
                }
            }
        }
        if unsafe { glaspen2_router_pressure_monitor() } != 0 {
            unsafe { glaspen2_router_pm_hide() }
        }
        force_redraw_draw_view_now();
        let text = i18n_text_owned("canvas.ethereal_mode");
        if !text.is_null() {
            unsafe {
                glaspen2_rust_notification_show(text);
                libc::free(text as *mut c_void);
            }
        }
    } else {
        unsafe {
            glaspen2_router_set_ethereal(0);
            glaspen2_router_set_strokes_visible(1);
        }
        glaspen2_rust_apply_glass();
        if unsafe { glaspen2_router_pressure_monitor() } != 0 {
            unsafe { glaspen2_router_pm_show() }
        }
        // 固定模式必须从 STROKES 全量重建 surface: 飘渺期间 draw view 可能跳过
        // surface 合成/或启动时尚未绘制, 只 setNeedsDisplay 会继续显示空 surface。
        glaspen2_rust_pageview_rebuild();
        redraw_draw_view_now();
        unsafe {
            glaspen2_swift_dispatch_main_after(0.0, std::ptr::null_mut(), deferred_draw_display)
        };
        let text = i18n_text_owned("canvas.fixed_mode");
        if !text.is_null() {
            unsafe {
                glaspen2_rust_notification_show(text);
                libc::free(text as *mut c_void);
            }
        }
    }
    let ethereal = unsafe { glaspen2_router_ethereal() } != 0;
    tracing::debug!(
        "[canvas-mode] after ethereal={} visible={} stroke_active={}",
        ethereal,
        unsafe { glaspen2_router_strokes_visible() },
        unsafe { glaspen2_router_stroke_active() }
    );
    unsafe {
        glaspen2_router_update_menu_state(778, i32::from(ethereal));
        let key = if ethereal {
            "canvas.fixed_mode"
        } else {
            "canvas.ethereal_mode"
        };
        let title = i18n_text_owned(key);
        if !title.is_null() {
            glaspen2_menu_set_title(778, title);
            libc::free(title as *mut c_void);
        }
        let k = std::ffi::CString::new("ethereal").unwrap();
        glaspen2_save_bool_setting(k.as_ptr(), i32::from(ethereal));
        glaspen2_router_update_status_icon_state();
    }
}

// ── R201: 飘渺 peek 计时 ──

static ETHEREAL_PEEK_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static ETHEREAL_PEEK_ACTIVE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

unsafe extern "C" {
    fn glaspen2_swift_dispatch_main_after(
        delay: f64,
        ctx: *mut c_void,
        cb: extern "C" fn(*mut c_void),
    );
}

extern "C" fn ethereal_peek_timeout(ctx: *mut c_void) {
    if ctx.is_null() {
        return;
    }
    let generation = unsafe { *Box::from_raw(ctx as *mut u64) };
    if ETHEREAL_PEEK_GEN.load(std::sync::atomic::Ordering::SeqCst) != generation {
        return;
    }
    ETHEREAL_PEEK_ACTIVE.store(false, std::sync::atomic::Ordering::SeqCst);
    let ethereal = unsafe { glaspen2_router_ethereal() } != 0;
    let stroke = unsafe { glaspen2_router_stroke_active() } != 0;
    if ethereal && !stroke {
        let _ = glaspen2_rust_ethereal_hide_now();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_ethereal_peek(seconds: f64) {
    if unsafe { glaspen2_router_ethereal() } == 0 {
        return;
    }
    glaspen2_rust_ethereal_cancel_peek();
    let _ = glaspen2_rust_ethereal_preview_show();
    ETHEREAL_PEEK_ACTIVE.store(true, std::sync::atomic::Ordering::SeqCst);
    let generation = ETHEREAL_PEEK_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    let ctx = Box::into_raw(Box::new(generation)) as *mut c_void;
    unsafe { glaspen2_swift_dispatch_main_after(seconds, ctx, ethereal_peek_timeout) };
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_ethereal_cancel_peek() {
    ETHEREAL_PEEK_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    ETHEREAL_PEEK_ACTIVE.store(false, std::sync::atomic::Ordering::SeqCst);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_ethereal_peek_lens_change() {
    if unsafe { glaspen2_router_ethereal() } == 0 || unsafe { glaspen2_router_infinite() } == 0 {
        return;
    }
    if unsafe { glaspen2_router_strokes_visible() } != 0
        && !ETHEREAL_PEEK_ACTIVE.load(std::sync::atomic::Ordering::SeqCst)
    {
        return;
    }
    glaspen2_rust_ethereal_peek(1.0);
}

// ── R202: 相机鼠标平移合帧调度 ──

static CAMERA_PAN_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

extern "C" fn camera_pan_timeout(ctx: *mut c_void) {
    if ctx.is_null() {
        return;
    }
    let generation = unsafe { *Box::from_raw(ctx as *mut u64) };
    if CAMERA_PAN_GEN.load(std::sync::atomic::Ordering::SeqCst) == generation {
        glaspen2_rust_camera_pan_flush();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_camera_queue_pan(dx: f64, dy: f64) {
    if glaspen2_rust_camera_pan_queue(dx, dy) == 0 {
        return;
    }
    let generation = CAMERA_PAN_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    let ctx = Box::into_raw(Box::new(generation)) as *mut c_void;
    unsafe { glaspen2_swift_dispatch_main_after(0.016, ctx, camera_pan_timeout) };
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_camera_flush() {
    glaspen2_rust_camera_pan_flush();
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_camera_cancel() {
    glaspen2_rust_camera_pan_cancel();
    CAMERA_PAN_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

// ── R203: 运行期初始化/看门狗 ──

static STARTUP_RUN_LOOP_STARTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

unsafe extern "C" {
    fn glaspen2_perf_init_from_env();
}

extern "C" fn startup_tap_check(_ctx: *mut c_void) {
    glaspen2_rust_check_tap();
    unsafe { glaspen2_swift_dispatch_main_after(2.0, std::ptr::null_mut(), startup_tap_check) };
}

pub extern "C" fn glaspen2_rust_startup_run_loop() {
    if STARTUP_RUN_LOOP_STARTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    unsafe {
        glaspen2_perf_init_from_env();
        glaspen2_rust_virtual_pen_maybe_start();
        glaspen2_rust_flip_probe_maybe_start();
        glaspen2_swift_dispatch_main_after(2.0, std::ptr::null_mut(), startup_tap_check);
    }
}

// ── R207: 页视图重建 ──

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pageview_rebuild() {
    let surface = glaspen2_rust_surface_current();
    if surface.is_null() {
        return;
    }
    let _ = glaspen2_rust_rebuild_surface(
        surface,
        glaspen2_rust_surface_scale(),
        glaspen2_rust_pageview_paper_ox(),
        glaspen2_rust_pageview_paper_oy(),
        glaspen2_rust_pageview_paper_scale(),
        1,
    );
}

// ── R208: 清屏/仅保存涂鸦 ──

unsafe extern "C" {
    fn glaspen2_force_new_page(w: i32, h: i32) -> i64;
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_surface_clear_screen() {
    if unsafe { glaspen2_router_infinite() } != 0 {
        let text = i18n_text_owned("canvas.infinite_kept");
        if !text.is_null() {
            unsafe {
                glaspen2_rust_notification_show(text);
                libc::free(text as *mut c_void);
            }
        }
        return;
    }
    let surface = glaspen2_rust_surface_current();
    if surface.is_null() {
        return;
    }
    let mut created = unsafe {
        glaspen2_rust_clear_screen(
            surface,
            glaspen2_rust_surface_scale(),
            glaspen2_rust_pageview_paper_ox(),
            glaspen2_rust_pageview_paper_oy(),
            glaspen2_rust_pageview_paper_scale(),
            i32::from(glaspen2_rust_pageview_is_fit() != 0),
        )
    };
    if created == 0 {
        // 用户显式按"新建画布"时, 即使当前页是空白也要真的建一页; 旧
        // blank-guard 只提示不建页, 会让人以为快捷键没生效。
        let id = unsafe {
            glaspen2_force_new_page(
                glaspen_core::shell_state::screen_w(),
                glaspen_core::shell_state::screen_h(),
            )
        };
        if id > 0 {
            unsafe {
                glaspen2_load_strokes_for_screen(id);
                glaspen2_rust_pageview_update();
                glaspen2_rust_overlay_flush();
            }
            created = 1;
        }
    }
    if created != 0 {
        unsafe { glaspen2_rust_notification_show_page_info(glaspen2_get_current_screen_id()) };
    } else {
        let text = i18n_text_owned("canvas.blank_guard");
        if !text.is_null() {
            unsafe {
                glaspen2_rust_notification_show(text);
                libc::free(text as *mut c_void);
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_surface_save_drawing_only() {
    let surface = glaspen2_rust_surface_current();
    if surface.is_null() {
        return;
    }
    unsafe { glaspen2_rust_save_drawing(surface) };
    let text = i18n_text_owned("export.screenshot_saved");
    if !text.is_null() {
        unsafe {
            glaspen2_rust_notification_show(text);
            libc::free(text as *mut c_void);
        }
    }
}

// ── R211: 菜单 handler 逻辑迁 Rust ──

unsafe extern "C" {
    fn glaspen2_menu_refresh_texts();
    fn glaspen2_save_xoj();
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_menu_toggle_language() {
    let next = if unsafe { glaspen2_router_lang() } == 1 {
        0
    } else {
        1
    };
    unsafe { glaspen2_menu_set_lang(next) };
    let code = if next == 1 { "en" } else { "zh" };
    let k = std::ffi::CString::new("language").unwrap();
    let v = std::ffi::CString::new(code).unwrap();
    unsafe {
        glaspen2_save_string_setting(k.as_ptr(), v.as_ptr());
        glaspen2_menu_refresh_texts();
        glaspen2_notify_settings_changed();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_menu_save_xoj() {
    unsafe { glaspen2_save_xoj() };
    let text = i18n_text_owned("export.notes_saved");
    if !text.is_null() {
        unsafe {
            glaspen2_rust_notification_show(text);
            libc::free(text as *mut c_void);
        }
    }
}

fn menu_export_async(pdf: bool) {
    let mut w = 0i32;
    let mut h = 0i32;
    unsafe { glaspen2_menu_screen_size(&mut w, &mut h) };
    std::thread::spawn(move || {
        let ok = if pdf {
            unsafe { glaspen2_menu_export_infinite_pdf_paged(w, h) != 0 }
        } else {
            unsafe { glaspen2_menu_export_infinite_svg() != 0 }
        };
        let (ok_key, fail_key) = if pdf {
            ("export.infinite_pdf_saved", "export.infinite_pdf_failed")
        } else {
            ("export.infinite_svg_saved", "export.infinite_svg_failed")
        };
        let text = i18n_text_owned(if ok { ok_key } else { fail_key });
        if !text.is_null() {
            unsafe {
                glaspen2_rust_notification_show(text);
                libc::free(text as *mut c_void);
            }
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_menu_export_pdf_async() {
    menu_export_async(true);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_menu_export_svg_async() {
    menu_export_async(false);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_menu_set_outline_enabled(on: i32) {
    let text = i18n_text_owned(if on != 0 {
        "hud.outline_on"
    } else {
        "hud.outline_off"
    });
    if text.is_null() {
        return;
    }
    unsafe {
        glaspen2_rust_set_outline_enabled(on, text);
        libc::free(text as *mut c_void);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_menu_quit_app() {
    glaspen2_rust_quit_app();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_menu_application_should_terminate() {
    unsafe { glaspen2_menu_shutdown() };
}

// ── R229: macOS 设置 API C ABI ──

unsafe extern "C" {
    fn glaspen2_camera_centered_values(
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        w: f64,
        h: f64,
        zoom: f64,
        px: *mut f64,
        py: *mut f64,
        pz: *mut f64,
    ) -> i32;
    fn glaspen2_next_screen_id() -> i64;
    fn glaspen2_prev_screen_id() -> i64;
    fn glaspen2_export_pdf() -> i32;
    fn glaspen2_save_animated_gif(fps: i32, resolution: f64, speed: f64, end_mode: i32) -> i32;
}

extern "C" fn macos_api_refresh_page(ctx: *mut c_void) {
    if ctx.is_null() {
        return;
    }
    let id = unsafe { *Box::from_raw(ctx as *mut i64) };
    if unsafe { glaspen2_router_infinite() } != 0 {
        return;
    }
    if id != unsafe { glaspen2_get_current_screen_id() } {
        return;
    }
    unsafe {
        glaspen2_load_strokes_for_screen(id);
        glaspen2_rust_pageview_rebuild();
        glaspen2_rust_overlay_flush();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_macos_settings_json() -> *mut std::os::raw::c_char {
    unsafe {
        let overrides = glaspen2_menu_snapshot_overrides_json();
        if overrides.is_null() {
            return std::ptr::null_mut();
        }
        let json = glaspen2_settings_snapshot_json(overrides);
        libc::free(overrides as *mut c_void);
        if json.is_null() {
            return std::ptr::null_mut();
        }
        let text = std::ffi::CStr::from_ptr(json)
            .to_string_lossy()
            .into_owned();
        glaspen2_free_c_string(json);
        let c = std::ffi::CString::new(text).unwrap_or_default();
        libc::strdup(c.as_ptr())
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_macos_free_c_string(p: *mut std::os::raw::c_char) {
    unsafe { libc::free(p as *mut c_void) }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_macos_free_bytes(p: *mut u8) {
    unsafe { libc::free(p as *mut c_void) }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_macos_set_setting(
    key: *const std::os::raw::c_char,
    value: *const std::os::raw::c_char,
) {
    if key.is_null() || value.is_null() {
        return;
    }
    let _ = unsafe { glaspen2_rust_apply_setting(key, value) };
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_macos_refresh_page(id: i64) {
    if id <= 0 {
        return;
    }
    let ctx = Box::into_raw(Box::new(id)) as *mut c_void;
    unsafe { glaspen2_swift_run_on_main_sync(ctx, macos_api_refresh_page) };
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_macos_refresh_infinite() {
    if unsafe { glaspen2_router_infinite() } == 0 {
        return;
    }
    unsafe {
        glaspen2_load_infinite_strokes();
        glaspen2_rust_pageview_rebuild();
        glaspen2_rust_overlay_flush();
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_macos_navigate_to_page(id: i64) {
    if id <= 0 {
        return;
    }
    if unsafe { glaspen2_router_infinite() } != 0 {
        let empty = std::ffi::CString::new("").unwrap();
        let _ = glaspen2_rust_apply_infinite(0, empty.as_ptr());
    }
    unsafe {
        glaspen2_load_strokes_for_screen(id);
        glaspen2_rust_pageview_update();
        glaspen2_rust_pageview_rebuild();
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_macos_delete_page(id: i64) -> i32 {
    let was_current = id == unsafe { glaspen2_get_current_screen_id() };
    let next = if was_current {
        unsafe { glaspen2_next_screen_id() }
    } else {
        0
    };
    let prev = if was_current {
        unsafe { glaspen2_prev_screen_id() }
    } else {
        0
    };
    let ok = unsafe { glaspen2_delete_screen(id) };
    if unsafe { glaspen2_router_infinite() } != 0 {
        glaspen2_rust_pageview_rebuild();
        return ok;
    }
    if ok != 0 && was_current && unsafe { glaspen2_router_stroke_active() } == 0 {
        let target = if next != 0 { next } else { prev };
        if target != 0 {
            unsafe {
                glaspen2_load_strokes_for_screen(target);
                glaspen2_rust_pageview_update();
                glaspen2_rust_pageview_rebuild();
            }
        } else {
            glaspen2_rust_surface_clear_screen();
        }
    }
    ok
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_macos_hotkey(key: *const std::os::raw::c_char) {
    if key.is_null() {
        return;
    }
    let k = unsafe { std::ffi::CStr::from_ptr(key) }.to_string_lossy();
    if k == "Q" {
        glaspen2_rust_menu_quit_app();
        return;
    }
    let code: u16 = match k.as_ref() {
        "G" => 0x05,
        "`" => 0x32,
        "1" => 0x12,
        "Z" => 0x06,
        "X" => 0x07,
        "C" => 0x08,
        "V" => 0x09,
        "B" => 0x0B,
        _ => return,
    };
    unsafe { glaspen2_router_perform_hotkey(code) };
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_macos_export_pdf() -> i32 {
    unsafe { glaspen2_export_pdf() }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_macos_export_gif() -> i32 {
    let ok = unsafe {
        glaspen2_save_animated_gif(
            glaspen2_router_gif_fps(),
            glaspen2_router_gif_resolution(),
            glaspen2_router_gif_speed(),
            glaspen2_router_gif_end_mode(),
        )
    };
    if ok != 0 {
        unsafe { glaspen2_swift_copy_newest_gif() };
        notification_show_owned(&format!(
            "{} 已保存并复制到剪贴板",
            i18n_text_lossy("export.gif_saved")
        ));
    } else {
        notification_show_owned(&i18n_text_lossy(
            "recording.gif_no_strokes_or_export_failed",
        ));
    }
    ok
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_macos_canvas_payload(
    w: i32,
    h: i32,
    action: i32,
    rect_out: *mut f64,
    len_out: *mut i32,
) -> *mut u8 {
    let mut ow = w;
    let mut oh = h;
    if ow < 400 {
        ow = 1024;
    }
    if oh < 400 {
        oh = 768;
    }
    unsafe {
        if action == 1 {
            glaspen2_rust_camera_home();
        } else if action == 2 {
            let (mut bx, mut by, mut bx2, mut by2) = (0.0, 0.0, 0.0, 0.0);
            if glaspen2_stroke_bbox(&mut bx, &mut by, &mut bx2, &mut by2) != 0 {
                let (mut px, mut py, mut pz) = (0.0, 0.0, 0.0);
                if glaspen2_camera_centered_values(
                    bx,
                    by,
                    bx2,
                    by2,
                    glaspen2_router_screen_w(),
                    glaspen2_router_screen_h(),
                    glaspen2_router_zoom(),
                    &mut px,
                    &mut py,
                    &mut pz,
                ) != 0
                {
                    glaspen2_router_set_camera(px, py, pz, 0);
                    glaspen2_rust_camera_apply_transform();
                }
            }
        } else if action == 3 {
            glaspen2_rust_finish_active_stroke();
            glaspen2_clear_strokes(
                glaspen2_router_screen_w() as i32,
                glaspen2_router_screen_h() as i32,
            );
            glaspen2_rust_camera_home();
            ow = 1024;
            oh = 768;
        }
        let (mut rect, mut bounds, mut len) = ([0.0f64; 4], [0.0f64; 4], 0i32);
        let png = glaspen2_rust_canvas_overview(
            ow as f64,
            oh as f64,
            rect.as_mut_ptr(),
            bounds.as_mut_ptr(),
            &mut len,
        );
        if png.is_null() || len <= 0 {
            return std::ptr::null_mut();
        }
        let out = libc::malloc(len as usize) as *mut u8;
        if out.is_null() {
            glaspen2_free_rust_bytes(png, len);
            return std::ptr::null_mut();
        }
        std::ptr::copy_nonoverlapping(png, out, len as usize);
        glaspen2_free_rust_bytes(png, len);
        if !len_out.is_null() {
            *len_out = len;
        }
        if !rect_out.is_null() {
            for i in 0..4 {
                *rect_out.add(i) = rect[i];
            }
            for i in 0..4 {
                *rect_out.add(4 + i) = bounds[i];
            }
        }
        out
    }
}

fn i18n_text_lossy(key: &str) -> String {
    let p = i18n_text_owned(key);
    if p.is_null() {
        return key.to_string();
    }
    let s = unsafe { std::ffi::CStr::from_ptr(p) }
        .to_string_lossy()
        .into_owned();
    unsafe { libc::free(p as *mut c_void) };
    s
}

// ── R230: 热键分派迁 Rust ──

static LAST_NO_NEXT_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

unsafe extern "C" {
    fn glaspen2_save_svg();
    fn glaspen2_get_cropped_svg() -> *mut std::os::raw::c_char;
    fn glaspen2_save_gif_cropped(data: *mut u8, w: i32, h: i32, stride: i32, scale: f64) -> i32;
    fn glaspen2_undo_last_stroke() -> i32;
    fn glaspen2_swift_flip_page(target: i64, going_next: i32, from_page: i64);
}

fn notify_key(key: &str) {
    let text = i18n_text_owned(key);
    if !text.is_null() {
        unsafe {
            glaspen2_rust_notification_show(text);
            libc::free(text as *mut c_void);
        }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn flip_page(target: i64, going_next: bool, from_page: i64) {
    let _ = target;
    glaspen2_rust_flip_page(i32::from(going_next), from_page);
    glaspen2_rust_ethereal_peek(1.0);
    unsafe { glaspen2_rust_notification_show_page_info(target) };
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_hotkey_perform(key: u16) -> i32 {
    match key {
        0x08 => {
            glaspen2_rust_finish_active_stroke();
            if unsafe { glaspen2_router_infinite() } != 0 {
                notify_key("canvas.infinite_kept");
            } else {
                glaspen2_rust_surface_clear_screen();
            }
            1
        }
        0x09 => {
            glaspen2_rust_toggle_draw_enabled();
            1
        }
        0x32 => {
            if unsafe { glaspen2_router_infinite() } != 0 {
                notify_key("canvas.single_canvas_warning");
                return 1;
            }
            glaspen2_rust_finish_active_stroke();
            let target = unsafe { glaspen2_prev_screen_id() };
            if target > 0 {
                flip_page(target, false, 0);
            } else {
                notify_key("page.no_previous");
            }
            1
        }
        0x12 => {
            if unsafe { glaspen2_router_infinite() } != 0 {
                notify_key("canvas.single_canvas_warning");
                return 1;
            }
            glaspen2_rust_finish_active_stroke();
            let target = unsafe { glaspen2_next_screen_id() };
            if target > 0 {
                flip_page(target, true, 0);
                return 1;
            }
            let now = now_ms();
            let second = now
                .saturating_sub(LAST_NO_NEXT_MS.load(std::sync::atomic::Ordering::SeqCst))
                < 5000;
            LAST_NO_NEXT_MS.store(now, std::sync::atomic::Ordering::SeqCst);
            if !second {
                notify_key("page.no_next");
                return 1;
            }
            let before = unsafe { glaspen2_get_current_screen_id() };
            let page = unsafe { glaspen2_create_next_page(before) };
            if page > 0 && page != before {
                flip_page(page, true, before);
                notify_key("page.created");
            } else if page == before {
                notify_key("page.already_latest");
            } else {
                notify_key("page.create_failed");
            }
            1
        }
        0x05 => {
            unsafe { glaspen2_save_svg() };
            let surface = glaspen2_rust_surface_current();
            if !surface.is_null() {
                unsafe {
                    cairo_surface_flush(surface);
                    let w = cairo_image_surface_get_width(surface);
                    let h = cairo_image_surface_get_height(surface);
                    let stride = cairo_image_surface_get_stride(surface);
                    let data = cairo_image_surface_get_data(surface);
                    let ok = glaspen2_save_gif_cropped(
                        data,
                        w,
                        h,
                        stride,
                        glaspen2_rust_surface_scale(),
                    );
                    if ok != 0 {
                        glaspen2_swift_copy_newest_gif();
                        notify_key("export.svg_gif_saved");
                    } else {
                        notify_key("export.failed");
                    }
                }
            }
            1
        }
        0x06 => {
            if unsafe { glaspen2_router_stroke_active() } != 0 {
                notify_key("draw.stroke_in_progress");
            } else {
                let remaining = unsafe { glaspen2_undo_last_stroke() };
                if remaining < 0 {
                    notify_key("draw.nothing_to_undo");
                } else {
                    glaspen2_rust_pageview_rebuild();
                    notify_key("draw.undone");
                }
            }
            1
        }
        0x01 => {
            let svg = unsafe { glaspen2_get_cropped_svg() };
            if svg.is_null() {
                notify_key("export.no_strokes_to_copy");
            } else {
                unsafe {
                    glaspen2_swift_copy_svg_html(svg);
                    glaspen2_free_c_string(svg);
                }
                notify_key("export.svg_copied");
            }
            1
        }
        0x0B => {
            unsafe {
                glaspen2_rust_set_glass_enabled(if glaspen2_router_glass_enabled() == 0 {
                    1
                } else {
                    0
                })
            };
            1
        }
        0x07 => {
            glaspen2_rust_toggle_canvas_mode();
            1
        }
        0x02 => {
            glaspen2_rust_show_settings_panel();
            1
        }
        _ => 0,
    }
}

unsafe extern "C" {
    fn glaspen2_router_set_lang(value: i32);
}

// ── R231: 菜单 C 薄桥迁 Rust ──

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_menu_lang() -> i32 {
    unsafe { glaspen2_router_lang() }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_menu_set_lang(value: i32) {
    unsafe { glaspen2_router_set_lang(value) }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_menu_launch_at_login() -> i32 {
    unsafe { glaspen2_is_launch_at_login() }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_menu_set_launch(on: i32) {
    glaspen2_rust_set_launch_at_login(on);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_menu_glass_enabled() -> i32 {
    unsafe { glaspen2_router_glass_enabled() }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_menu_set_glass(on: i32) {
    glaspen2_rust_set_glass_enabled(on);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_menu_outline_enabled() -> i32 {
    unsafe { glaspen2_router_outline_enabled() }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_menu_apply_outline(on: i32) {
    glaspen2_rust_menu_set_outline_enabled(on);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_menu_infinite_enabled() -> i32 {
    unsafe { glaspen2_router_infinite() }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_menu_apply_infinite(on: i32) {
    let text = i18n_text_owned(if on != 0 {
        "canvas.infinite_on"
    } else {
        "canvas.infinite_off"
    });
    if text.is_null() {
        return;
    }
    unsafe {
        glaspen2_rust_apply_infinite(on, text);
        libc::free(text as *mut c_void);
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_menu_save_only() {
    glaspen2_rust_surface_save_drawing_only();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_menu_clear_screen() {
    glaspen2_rust_surface_clear_screen();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_menu_toggle_enabled() {
    glaspen2_rust_toggle_draw_enabled();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_menu_show_settings() {
    glaspen2_rust_show_settings_panel();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_menu_toggle_canvas_mode() {
    glaspen2_rust_toggle_canvas_mode();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_menu_select_color(idx: i32) {
    let _ = glaspen2_rust_select_color(idx);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_menu_select_width(idx: i32) {
    let _ = glaspen2_rust_select_width(idx);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_menu_save_xoj_raw() {
    unsafe { glaspen2_save_xoj() }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_menu_quit_bridge() {
    glaspen2_rust_menu_quit_app();
}

// ── R232: 菜单通知/飘渺 C 桥迁 Rust ──

#[unsafe(no_mangle)]
pub extern "C" fn gl_ethereal_restore_now() {
    glaspen2_rust_ethereal_restore_now();
}
#[unsafe(no_mangle)]
pub extern "C" fn auto_hide_now() -> i32 {
    glaspen2_rust_ethereal_hide_now()
}
#[unsafe(no_mangle)]
pub extern "C" fn auto_show_canvas() {
    glaspen2_rust_ethereal_auto_show();
}
#[unsafe(no_mangle)]
pub extern "C" fn peek_cancel_timer() {
    glaspen2_rust_ethereal_cancel_peek();
}
#[unsafe(no_mangle)]
pub extern "C" fn peek_lens_change() {
    glaspen2_rust_ethereal_peek_lens_change();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_menu_notify(text: *const c_void) {
    if text.is_null() {
        return;
    }
    use objc2_foundation::NSString;
    let s = unsafe { &*(text as *const NSString) }.to_string();
    notification_show_owned(&s);
}

// ── R233: 光标/缩放 C 桥迁 Rust ──

unsafe extern "C" {
    fn CGDisplayHideCursor(display: u32) -> i32;
    fn CGDisplayShowCursor(display: u32) -> i32;
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_zoom_at(factor: f64, x: f64, y: f64) {
    glaspen2_rust_camera_zoom_at(factor, x, y);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_cursor_set(x: f64, y: f64) {
    sw_cursor_set(x, y, 0);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_cursor_show(on: i32) {
    if on != 0 {
        sw_cursor_show();
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_hide_system_cursor() {
    unsafe { CGDisplayHideCursor(CGMainDisplayID()) };
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_show_system_cursor() {
    unsafe { CGDisplayShowCursor(CGMainDisplayID()) };
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_virtual_pen_user_data() -> i64 {
    0x5650_4E31
}

// ── R234: run_glue 纯状态 C 桥迁 Rust ──

unsafe extern "C" {
    fn glaspen2_bubble_pen_activity();
    fn glaspen2_bubble_pen_back();
    fn glaspen2_bubble_pen_gone();
}

fn glaspen2_width_scale_for_raw() -> f64 {
    unsafe { glaspen2_router_width_scale() }
}

unsafe extern "C" {
    fn glaspen2_pressure_raw_width(pressure: f64, width_scale: f64) -> f64;
    fn glaspen2_stroke_count() -> i64;
    fn glaspen2_recording_mode(slot: i32) -> i32;
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_state() -> i32 {
    (unsafe { glaspen2_router_enabled() })
        | (i32::from(glaspen2_rust_pen_stroke_active() != 0) << 1)
        | (unsafe { glaspen2_router_infinite() } << 2)
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_cursor_flags() -> i32 {
    i32::from(unsafe { glaspen2_router_pen_cursor_visible() } != 0)
        | (i32::from(glaspen2_rust_pen_stroke_active() != 0) << 1)
        | (unsafe { glaspen2_router_ethereal() } << 2)
        | (i32::from(glaspen2_rust_pen_has_seen_proximity() != 0) << 3)
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_pen_state() -> i32 {
    (unsafe { glaspen2_router_strokes_visible() })
        | (i32::from(glaspen2_rust_pen_stroke_active() != 0) << 1)
        | (i32::from(unsafe { glaspen2_router_pen_cursor_visible() } != 0) << 2)
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_auto_hide() {
    let _ = glaspen2_rust_ethereal_hide_now();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_flush() {
    glaspen2_rust_overlay_flush_dirty();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_pen_auto_show() {
    glaspen2_rust_ethereal_auto_show();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_peek_cancel() {
    glaspen2_rust_ethereal_cancel_peek();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_reenable_tap() {
    glaspen2_router_event_tap_enable();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_pen_bubble(back: i32) {
    if back != 0 {
        unsafe { glaspen2_bubble_pen_back() }
    } else {
        unsafe { glaspen2_bubble_pen_activity() }
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_bubble_pen_back() {
    unsafe { glaspen2_bubble_pen_back() }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_bubble_pen_gone() {
    unsafe { glaspen2_bubble_pen_gone() }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_raw_width(pressure: f64) -> f64 {
    unsafe {
        glaspen2_pressure_raw_width(pressure, glaspen2_router_width_scale())
            * glaspen2_router_zoom()
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_rec_slot_gif() -> i32 {
    0
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_rec_slot_chat() -> i32 {
    1
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_rec_mode(slot: i32) -> i32 {
    unsafe { glaspen2_recording_mode(slot) }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_rec_finish_and_count() -> i32 {
    glaspen2_rust_finish_active_stroke();
    unsafe { glaspen2_stroke_count() as i32 }
}

// ── R235: run_glue 更多纯 C 桥迁 Rust ──

static LANG_CODE_EN: std::sync::OnceLock<std::ffi::CString> = std::sync::OnceLock::new();
static LANG_CODE_ZH: std::sync::OnceLock<std::ffi::CString> = std::sync::OnceLock::new();

unsafe extern "C" {}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_camera_mods() -> usize {
    let action = b"canvas_zoom_in\0";
    let chord =
        unsafe { glaspen2_shortcut_store_chord(action.as_ptr() as *const std::os::raw::c_char) };
    if chord.is_null() {
        return 0;
    }
    let mask = unsafe { glaspen2_chord_modifier_mask(chord) };
    unsafe { glaspen2_free_c_string(chord) };
    mask as usize
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_lang_code() -> *const std::os::raw::c_char {
    let en = LANG_CODE_EN.get_or_init(|| std::ffi::CString::new("en").unwrap());
    let zh = LANG_CODE_ZH.get_or_init(|| std::ffi::CString::new("zh").unwrap());
    if unsafe { glaspen2_router_lang() } == 1 {
        en.as_ptr()
    } else {
        zh.as_ptr()
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_disable_cleanup() {
    unsafe {
        glaspen2_router_camera_clear_dragging();
        glaspen2_router_camera_cancel();
        glaspen2_rust_restore_system_cursor();
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_tap_menu_bar_interaction() {
    unsafe {
        sw_event_tap_disable();
        glaspen2_swift_dispatch_main_after(5.0, std::ptr::null_mut(), tap_menu_bar_reenable);
    }
}
extern "C" fn tap_menu_bar_reenable(_ctx: *mut c_void) {
    glaspen2_router_event_tap_enable();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_update_status_icon_state() {
    let item = unsafe { sw_status_item() };
    if item.is_null() {
        return;
    }
    let _ = glaspen2_rust_status_icon_apply(
        item,
        glaspen2_rust_pen_eraser_mode(),
        unsafe { glaspen2_router_enabled() },
        unsafe { glaspen2_router_ethereal() },
        unsafe { glaspen2_router_infinite() },
        unsafe { glaspen2_router_width_scale() },
        unsafe { glaspen2_router_pen_r() },
        unsafe { glaspen2_router_pen_g() },
        unsafe { glaspen2_router_pen_b() },
    );
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_draw_height() -> f64 {
    let view = unsafe { sw_draw_view() };
    if view.is_null() {
        return 0.0;
    }
    unsafe {
        let rect: objc2_foundation::NSRect =
            objc2::msg_send![view as *mut objc2::runtime::AnyObject, bounds];
        rect.size.height
    }
}

// ── R237: run_glue 更多纯桥迁 Rust ──

unsafe extern "C" {
    fn glaspen2_perf_summary();
    fn glaspen2_perf_is_enabled() -> i32;
    fn glaspen2_perf_record(
        name: *const std::os::raw::c_char,
        us: u64,
        notes: *const std::os::raw::c_char,
    );
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGPointR {
    x: f64,
    y: f64,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGSizeR {
    width: f64,
    height: f64,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGRectR {
    origin: CGPointR,
    size: CGSizeR,
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_perf_summary() {
    unsafe { glaspen2_perf_summary() }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_perf_enabled() -> i32 {
    unsafe { glaspen2_perf_is_enabled() }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_perf_notes(
    name: *const std::os::raw::c_char,
    notes: *const std::os::raw::c_char,
    t0: u64,
) {
    unsafe { glaspen2_perf_record(name, glaspen2_perf_elapsed_us(t0), notes) }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_rec_chat_integration_changed(on: i32) {
    let json = format!(
        "{{\"type\":\"ChatIntegrationChanged\",\"enabled\":{}}}",
        on != 0
    );
    let Ok(c) = std::ffi::CString::new(json) else {
        return;
    };
    let _ = glaspen2_rust_recording_feed(1, c.as_ptr());
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_rebuild_surface() {
    glaspen2_rust_pageview_rebuild();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_canvas_apply_transform() {
    glaspen2_rust_camera_apply_transform();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_peek_lens_change() {
    glaspen2_rust_ethereal_peek_lens_change();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_dirty_reset() {
    glaspen2_rust_dirty_reset();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_dirty_has() -> i32 {
    glaspen2_rust_dirty_has()
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_dirty_rect() -> CGRectR {
    let mut out = [0.0f64; 4];
    if glaspen2_rust_dirty_rect(out.as_mut_ptr()) == 0 {
        return CGRectR {
            origin: CGPointR { x: 0.0, y: 0.0 },
            size: CGSizeR {
                width: 0.0,
                height: 0.0,
            },
        };
    }
    CGRectR {
        origin: CGPointR {
            x: out[0],
            y: out[1],
        },
        size: CGSizeR {
            width: out[2],
            height: out[3],
        },
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_camera_pan_cancel() {
    glaspen2_router_camera_cancel();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_camera_persist_infinite() {
    glaspen2_rust_camera_persist_infinite();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_camera_load_infinite() {
    glaspen2_rust_camera_load_infinite();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_camera_reset_lens() {
    glaspen2_rust_camera_reset_lens();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_restore_system_cursor() {
    glaspen2_rust_restore_system_cursor();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_hotkey_recording(on: i32) {
    glaspen2_rust_hotkey_recording_set(on);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_flush_dirty() {
    glaspen2_rust_overlay_flush_dirty();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_finish_active_stroke() {
    glaspen2_rust_finish_active_stroke();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_surface_cache_invalidate() {}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_dirty_include_surface_point(x: f64, y: f64, pad: f64) {
    glaspen2_rust_dirty_include_surface_point(x, y, pad);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_dirty_include_surface_rect(x: f64, y: f64, w: f64, h: f64) {
    glaspen2_rust_dirty_include_surface_rect(x, y, w, h);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_perform_hotkey(key: u16) -> i32 {
    glaspen2_router_hotkey_perform(key)
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_apply_infinite_canvas(on: i32) {
    let text = i18n_text_owned(if on != 0 {
        "canvas.infinite_on"
    } else {
        "canvas.infinite_off"
    });
    if text.is_null() {
        return;
    }
    unsafe {
        glaspen2_rust_apply_infinite(on, text);
        libc::free(text as *mut c_void);
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_clear_screen() {
    glaspen2_rust_surface_clear_screen();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_replay_strokes() {
    glaspen2_rust_pageview_rebuild();
}

// ── R238: run_glue 视图/标志/延迟重建桥 ──

unsafe extern "C" {
    fn sw_set_infinite(on: i32);
    fn sw_set_minimap(on: i32);
}

fn redraw_draw_view_now() {
    let view = unsafe { sw_draw_view() };
    if view.is_null() {
        return;
    }
    FULL_REDRAW_PENDING.store(true, std::sync::atomic::Ordering::SeqCst);
    unsafe {
        let _: () = objc2::msg_send![view as *mut objc2::runtime::AnyObject, setNeedsDisplay: true];
    }
}

/// 可见性/模式切换是低频操作: 设置 needsDisplay 后立即 displayIfNeeded。
/// AppKit 在后台/非激活窗口上可能拖到下个 runloop 周期才画, 用户按快捷键
/// 后看到“固定画布模式”通知却没有笔迹, 就是这里没同步 flush。
extern "C" fn deferred_draw_display(_ctx: *mut c_void) {
    unsafe {
        let draw = sw_draw_view();
        if !draw.is_null() {
            let _: () = objc2::msg_send![draw as *mut objc2::runtime::AnyObject, displayIfNeeded];
        }
    }
}

fn force_redraw_draw_view_now() {
    redraw_draw_view_now();
    unsafe {
        let draw = sw_draw_view();
        if !draw.is_null() {
            let _: () = objc2::msg_send![draw as *mut objc2::runtime::AnyObject, displayIfNeeded];
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_swift_show_cursor() {
    unsafe { CGDisplayShowCursor(CGMainDisplayID()) };
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_swift_pageview_rebuild() {
    glaspen2_rust_pageview_rebuild();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_swift_tunnel_flush() {
    redraw_draw_view_now();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_notification_redraw() {
    redraw_draw_view_now();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_main_window() -> *mut c_void {
    unsafe { sw_main_window() }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_draw_view() -> *mut c_void {
    unsafe { sw_draw_view() }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_restore_canvas_flags(infinite: i32, minimap: i32, ethereal: i32) {
    unsafe {
        sw_set_infinite(infinite);
        sw_set_minimap(minimap);
        glaspen2_router_set_ethereal(ethereal);
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_restore_pressure(on: i32) {
    unsafe { glaspen2_router_set_pressure_monitor(on) }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_swift_notify_ax_warning() {
    let text = if unsafe { glaspen2_router_lang() } == 0 {
        "辅助功能权限已失效, 涂鸦/热键暂停 — 请到系统设置重新勾选"
    } else {
        "Accessibility permission lost — re-enable it in System Settings"
    };
    notification_show_owned(text);
}
extern "C" fn delayed_rebuild_cb(_ctx: *mut c_void) {
    if unsafe { glaspen2_router_stroke_active() } != 0 {
        return;
    }
    glaspen2_router_ensure_surface();
    glaspen2_rust_pageview_rebuild();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_swift_delayed_rebuild() {
    unsafe { glaspen2_swift_dispatch_main_after(0.3, std::ptr::null_mut(), delayed_rebuild_cb) }
}

// ── R239: 录制/异步/绘制桥迁 Rust ──

unsafe extern "C" {
    fn glaspen2_recording_init(slot: i32, chat: i32);
    fn sw_set_pen_settings(r: f64, g: f64, b: f64, width: f64, color_idx: i32, width_idx: i32);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_recording_feed(
    slot: i32,
    json: *const std::os::raw::c_char,
) -> i32 {
    glaspen2_rust_recording_feed(slot, json)
}
extern "C" fn apply_infinite_async_cb(ctx: *mut c_void) {
    if ctx.is_null() {
        return;
    }
    let on = unsafe { *Box::from_raw(ctx as *mut i32) };
    glaspen2_router_apply_infinite_canvas(on);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_apply_infinite_async(on: i32) {
    let ctx = Box::into_raw(Box::new(on)) as *mut c_void;
    unsafe { glaspen2_swift_dispatch_main_after(0.0, ctx, apply_infinite_async_cb) }
}
extern "C" fn quit_async_cb(_ctx: *mut c_void) {
    glaspen2_rust_menu_quit_app();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_quit_async() {
    unsafe { glaspen2_swift_dispatch_main_after(0.0, std::ptr::null_mut(), quit_async_cb) }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_rec_init() {
    unsafe {
        glaspen2_recording_init(0, 0);
        glaspen2_recording_init(1, glaspen2_router_chat_integration());
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_startup_apply_pen(
    r: f64,
    g: f64,
    b: f64,
    width: f64,
    color_idx: i32,
    width_idx: i32,
) {
    unsafe { sw_set_pen_settings(r, g, b, width, color_idx, width_idx) }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_swift_overlay_flush() {
    glaspen2_rust_overlay_flush();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_swift_notify(text: *const std::os::raw::c_char) {
    if text.is_null() {
        return;
    }
    let s = unsafe { std::ffi::CStr::from_ptr(text) }
        .to_string_lossy()
        .into_owned();
    notification_show_owned(&s);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_swift_draw_view() -> *mut c_void {
    unsafe { sw_draw_view() }
}

// ── R240: 面板/视图 C 桥迁 Rust ──

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_apply_pending_color_width() {
    glaspen2_rust_apply_pending_color_width();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_redraw_draw_view() {
    redraw_draw_view_now();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_window_sharing_type() -> i32 {
    let w = unsafe { sw_main_window() };
    if w.is_null() {
        return 0;
    }
    unsafe { objc2::msg_send![w as *mut objc2::runtime::AnyObject, sharingType] }
}
#[unsafe(no_mangle)]
pub extern "C" fn show_settings_panel() {
    glaspen2_rust_show_settings_panel();
}
#[unsafe(no_mangle)]
pub extern "C" fn surface_image_cache_invalidate() {}
#[unsafe(no_mangle)]
pub extern "C" fn ensure_surface(view: *mut c_void) {
    if view.is_null() {
        return;
    }
    let _ = glaspen2_rust_surface_ensure(view);
}
#[unsafe(no_mangle)]
pub extern "C" fn flush_to_layer() {
    glaspen2_rust_overlay_flush();
}
#[unsafe(no_mangle)]
pub extern "C" fn flush_dirty_to_layer() {
    glaspen2_rust_overlay_flush_dirty();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_display_size(w: *mut i32, h: *mut i32) {
    unsafe {
        if !w.is_null() {
            *w = glaspen2_router_screen_w() as i32;
        }
        if !h.is_null() {
            *h = glaspen2_router_screen_h() as i32;
        }
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_outline_dash() -> f64 {
    1.0
}

// ── R241: 压感监视窗(暂用独立探测入口) ──

static PM_WINDOW: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static PM_LABEL: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

unsafe extern "C" {
    fn CGWindowLevelForKey(key: i32) -> i32;
}

fn pm_ensure_window() {
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
    if PM_WINDOW.load(std::sync::atomic::Ordering::SeqCst) != 0 {
        return;
    }
    unsafe {
        let (Some(window_cls), Some(field_cls)) =
            (AnyClass::get(c"NSWindow"), AnyClass::get(c"NSTextField"))
        else {
            return;
        };
        let screen_h = glaspen2_router_screen_h();
        let frame = NSRect {
            origin: NSPoint {
                x: 10.0,
                y: screen_h - 40.0,
            },
            size: NSSize {
                width: 220.0,
                height: 30.0,
            },
        };
        let window: *mut AnyObject = objc2::msg_send![window_cls, alloc];
        let window: *mut AnyObject = objc2::msg_send![window, initWithContentRect: frame, styleMask: 0u64, backing: 2u64, defer: false];
        if window.is_null() {
            return;
        }
        let level = CGWindowLevelForKey(14);
        let _: () = objc2::msg_send![window, setLevel: level as i64];
        let _: () = objc2::msg_send![window, setOpaque: true];
        let bg = objc2_app_kit::NSColor::colorWithCalibratedWhite_alpha(0.12, 1.0);
        let _: () = objc2::msg_send![window, setBackgroundColor: &*bg];
        let _: () = objc2::msg_send![window, setIgnoresMouseEvents: true];
        let title = NSString::from_str("Pressure");
        let _: () = objc2::msg_send![window, setTitle: &*title];
        let _: () = objc2::msg_send![window, setReleasedWhenClosed: false];
        let label_frame = NSRect {
            origin: NSPoint { x: 4.0, y: 2.0 },
            size: NSSize {
                width: 212.0,
                height: 26.0,
            },
        };
        let label: *mut AnyObject = objc2::msg_send![field_cls, alloc];
        let label: *mut AnyObject = objc2::msg_send![label, initWithFrame: label_frame];
        if label.is_null() {
            return;
        }
        let text = NSString::from_str("P=-----  ---  (---,---)");
        let _: () = objc2::msg_send![label, setStringValue: &*text];
        let white = objc2_app_kit::NSColor::whiteColor();
        let _: () = objc2::msg_send![label, setTextColor: &*white];
        let font_cls = AnyClass::get(c"NSFont");
        if let Some(font_cls) = font_cls {
            let name = NSString::from_str("Menlo");
            let font: *mut AnyObject =
                objc2::msg_send![font_cls, fontWithName: &*name, size: 12.0f64];
            if !font.is_null() {
                let _: () = objc2::msg_send![label, setFont: font];
            }
        }
        let _: () = objc2::msg_send![label, setBezeled: false];
        let _: () = objc2::msg_send![label, setDrawsBackground: false];
        let _: () = objc2::msg_send![label, setEditable: false];
        let _: () = objc2::msg_send![label, setSelectable: false];
        let content: *mut AnyObject = objc2::msg_send![window, contentView];
        if !content.is_null() {
            let _: () = objc2::msg_send![content, addSubview: label];
        }
        PM_WINDOW.store(window as usize, std::sync::atomic::Ordering::SeqCst);
        PM_LABEL.store(label as usize, std::sync::atomic::Ordering::SeqCst);
    }
}

fn pm_show() {
    use objc2::runtime::AnyObject;
    pm_ensure_window();
    let w = PM_WINDOW.load(std::sync::atomic::Ordering::SeqCst) as *mut AnyObject;
    if w.is_null() {
        return;
    }
    unsafe {
        let _: () = objc2::msg_send![w, orderFront: std::ptr::null_mut::<AnyObject>()];
    }
}
fn pm_hide() {
    use objc2::runtime::AnyObject;
    let w = PM_WINDOW.load(std::sync::atomic::Ordering::SeqCst) as *mut AnyObject;
    if w.is_null() {
        return;
    }
    unsafe {
        let _: () = objc2::msg_send![w, orderOut: std::ptr::null_mut::<AnyObject>()];
    }
}
fn pm_destroy() {
    use objc2::runtime::AnyObject;
    let w = PM_WINDOW.swap(0, std::sync::atomic::Ordering::SeqCst) as *mut AnyObject;
    PM_LABEL.store(0, std::sync::atomic::Ordering::SeqCst);
    if w.is_null() {
        return;
    }
    unsafe {
        let _: () = objc2::msg_send![w, close];
    }
}
fn pm_update_label(text: &str) {
    use objc2::runtime::AnyObject;
    use objc2_foundation::NSString;
    let label = PM_LABEL.load(std::sync::atomic::Ordering::SeqCst) as *mut AnyObject;
    if label.is_null() {
        return;
    }
    let s = NSString::from_str(text);
    unsafe {
        let _: () = objc2::msg_send![label, setStringValue: &*s];
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_pm_probe() {
    pm_ensure_window();
    pm_update_label("P=1234  tablet  (10,20)");
    pm_show();
    std::thread::sleep(std::time::Duration::from_millis(200));
    pm_hide();
    pm_destroy();
    tracing::info!("[pm-probe] ensure/show/update/hide/destroy ok");
}

// ── R242: 压感窗 C 符号接线 ──

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_pm_show() {
    pm_show();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_pm_hide() {
    pm_hide();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_swift_pressure_destroy() {
    pm_destroy();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_pressure_label(text: *const std::os::raw::c_char) {
    if text.is_null() {
        return;
    }
    let s = unsafe { std::ffi::CStr::from_ptr(text) }.to_string_lossy();
    pm_update_label(&s);
}
fn pm_refresh_label() {
    let pressure = glaspen2_rust_pm_pressure();
    let ev_type = glaspen2_rust_pm_ev_type();
    let x = glaspen2_rust_pm_x();
    let y = glaspen2_rust_pm_y();
    glaspen2_rust_update_pressure_label(pressure, ev_type, x, y);
}
#[unsafe(no_mangle)]
pub extern "C" fn sw_pm_update(
    in_range: i32,
    pressure: i32,
    x: f64,
    y: f64,
    tip_down: i32,
    ev_type: *const std::os::raw::c_char,
) {
    glaspen2_rust_pm_update(in_range, pressure, x, y, tip_down, ev_type);
    pm_refresh_label();
}
#[unsafe(no_mangle)]
pub extern "C" fn sw_pm_refresh() {
    pm_refresh_label();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_pm_refresh() {
    pm_refresh_label();
}

// ── R243: UI 锚点状态迁 Rust ──

static ANCHOR_MAIN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static ANCHOR_DRAW: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static ANCHOR_GLASS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static ANCHOR_STATUS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static ANCHOR_MENU: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static ANCHOR_SETTINGS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

macro_rules! anchor_accessors {
    ($get:ident, $set:ident, $slot:ident) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $get() -> *mut c_void {
            $slot.load(std::sync::atomic::Ordering::SeqCst) as *mut c_void
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn $set(p: *mut c_void) {
            $slot.store(p as usize, std::sync::atomic::Ordering::SeqCst);
        }
    };
}
anchor_accessors!(sw_main_window, sw_set_main_window, ANCHOR_MAIN);
anchor_accessors!(sw_draw_view, sw_set_draw_view, ANCHOR_DRAW);
anchor_accessors!(sw_glass_view, sw_set_glass_view, ANCHOR_GLASS);
anchor_accessors!(sw_status_item, sw_set_status_item, ANCHOR_STATUS);
anchor_accessors!(sw_menu, sw_set_menu, ANCHOR_MENU);
anchor_accessors!(sw_settings_window, sw_set_settings_window, ANCHOR_SETTINGS);

// ── R244: 隧道动画定时器迁 Rust ──

static TUNNEL_TIMER_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

unsafe extern "C" {
    fn glaspen2_swift_tunnel_render(p: f64);
    fn glaspen2_swift_tunnel_finish();
}

extern "C" fn tunnel_timer_tick(_ctx: *mut c_void) {
    let mut p = 0.0f64;
    if unsafe { glaspen2_rust_tunnel_tick(&mut p) } == 0 {
        unsafe { glaspen2_swift_tunnel_finish() };
        return;
    }
    unsafe { glaspen2_swift_tunnel_render(p) };
    if unsafe { glaspen2_swift_tunnel_active() } != 0 {
        unsafe { glaspen2_swift_tunnel_flush() };
    }
    let generation = TUNNEL_TIMER_GEN.load(std::sync::atomic::Ordering::SeqCst);
    let ctx = Box::into_raw(Box::new(generation)) as *mut c_void;
    unsafe { glaspen2_swift_dispatch_main_after(0.008333, ctx, tunnel_timer_tick) }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_tunnel_timer_start() {
    let generation = TUNNEL_TIMER_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    let ctx = Box::into_raw(Box::new(generation)) as *mut c_void;
    unsafe { glaspen2_swift_dispatch_main_after(0.0, ctx, tunnel_timer_tick) }
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_tunnel_timer_cancel() {
    TUNNEL_TIMER_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

// ── R245: 显示变更帧应用迁 Rust ──

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_display_apply(w: i32, h: i32) {
    use objc2::runtime::AnyObject;
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    let frame = NSRect {
        origin: NSPoint { x: 0.0, y: 0.0 },
        size: NSSize {
            width: w as f64,
            height: h as f64,
        },
    };
    unsafe {
        let main = sw_main_window();
        if !main.is_null() {
            let win = main as *mut AnyObject;
            let _: () = objc2::msg_send![win, setFrame: frame, display: true];
            let content: *mut AnyObject = objc2::msg_send![win, contentView];
            if !content.is_null() {
                let _: () = objc2::msg_send![content, setFrame: frame];
            }
        }
        let glass = sw_glass_view();
        if !glass.is_null() {
            let _: () = objc2::msg_send![glass as *mut AnyObject, setFrame: frame];
        }
        let draw = sw_draw_view();
        if !draw.is_null() {
            let view = draw as *mut AnyObject;
            let _: () = objc2::msg_send![view, setFrame: frame];
            let _: () = objc2::msg_send![view, setNeedsDisplay: true];
        }
    }
}

// ── R246: 翻页编排迁 Rust ──

unsafe extern "C" {
    fn glaspen2_bubble_auto_page_if_due() -> i64;
    fn glaspen2_swift_tunnel_start(going_next: i32, cur_page: i64) -> i32;
}

fn flip_prepare(target: i64) {
    if target <= 0 {
        return;
    }
    unsafe {
        glaspen2_load_strokes_for_screen(target);
        glaspen2_smooth_loaded_strokes();
        glaspen2_rust_pageview_update();
    }
}
extern "C" fn flip_commit_show(ctx: *mut c_void) {
    glaspen2_rust_pageview_rebuild();
    let win = unsafe { sw_main_window() };
    if !win.is_null() {
        unsafe {
            let _: () = objc2::msg_send![win as *mut objc2::runtime::AnyObject, setIsVisible: true];
        }
    }
    if !ctx.is_null() {
        let t0 = unsafe { *Box::from_raw(ctx as *mut f64) };
        let gap = (glaspen2_rust_reference_now() - t0) * 1000.0;
        tracing::info!("[flip] 隐藏间隙 {gap:.0}ms");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_flip_page(going_next: i32, from_page: i64) -> i32 {
    if unsafe { glaspen2_swift_tunnel_active() } != 0 {
        unsafe { glaspen2_swift_tunnel_finish() };
    }
    let win = unsafe { sw_main_window() };
    let cur_before = if from_page > 0 {
        from_page
    } else {
        unsafe { glaspen2_get_current_screen_id() }
    };
    let target =
        unsafe { glaspen2_page_neighbor(cur_before, if going_next != 0 { 1 } else { -1 }) };
    if unsafe { glaspen2_router_flip_effect() } == 1
        && unsafe { glaspen2_router_infinite() } == 0
        && !glaspen2_rust_surface_current().is_null()
        && unsafe { glaspen2_router_stroke_active() } == 0
    {
        let t0 = glaspen2_rust_reference_now();
        flip_prepare(target);
        let tp1 = glaspen2_rust_reference_now();
        if unsafe { glaspen2_swift_tunnel_start(going_next, cur_before) } != 0 {
            tracing::info!("[flip] 卡顿账: prepare {:.0}ms", (tp1 - t0) * 1000.0);
            return 1;
        }
    }
    let t0 = glaspen2_rust_reference_now();
    if !win.is_null() {
        unsafe {
            let _: () =
                objc2::msg_send![win as *mut objc2::runtime::AnyObject, setIsVisible: false];
        }
    }
    flip_prepare(target);
    if win.is_null() {
        glaspen2_rust_pageview_rebuild();
        return 1;
    }
    let ctx = Box::into_raw(Box::new(t0)) as *mut c_void;
    unsafe { glaspen2_swift_dispatch_main_after(0.0, ctx, flip_commit_show) }
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_bubble_auto_page_flip() -> i32 {
    let page = unsafe { glaspen2_bubble_auto_page_if_due() };
    if page <= 0 {
        return 0;
    }
    glaspen2_rust_flip_page(1, 0);
    unsafe { glaspen2_rust_notification_show_page_info(page) };
    let text = i18n_text_owned("page.auto_created");
    if !text.is_null() {
        unsafe {
            glaspen2_rust_notification_show(text);
            libc::free(text as *mut c_void);
        }
    }
    1
}

// ── R247: 设置面板透明应用迁 Rust ──

fn apply_panel_transparent_rust(on: i32) {
    use objc2::runtime::AnyObject;
    let win = unsafe { sw_settings_window() };
    if !win.is_null() {
        unsafe {
            let _: () = objc2::msg_send![win as *mut AnyObject, setOpaque: on == 0];
            let color = if on != 0 {
                objc2_app_kit::NSColor::clearColor()
            } else {
                objc2_app_kit::NSColor::windowBackgroundColor()
            };
            let _: () = objc2::msg_send![win as *mut AnyObject, setBackgroundColor: &*color];
        }
    }
    let vc = glaspen2_rust_flutter_vc();
    if vc.is_null() {
        return;
    }
    unsafe {
        let view: *mut AnyObject = objc2::msg_send![vc as *mut AnyObject, view];
        if view.is_null() {
            return;
        }
        let layer: *mut AnyObject = objc2::msg_send![view, layer];
        if !layer.is_null() {
            let _: () = objc2::msg_send![layer, setOpaque: on == 0];
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn gl_panel_transparent_apply(on: i32) {
    apply_panel_transparent_rust(on);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_settings_apply_transparent() {
    apply_panel_transparent_rust(1);
}

// ── R248: 窗口共享类型 C 桥迁 Rust ──

fn set_window_sharing_rust(kind: i64) {
    use objc2::runtime::AnyObject;
    unsafe {
        let main = sw_main_window();
        if !main.is_null() {
            let _: () = objc2::msg_send![main as *mut AnyObject, setSharingType: kind];
        }
        let settings = sw_settings_window();
        if !settings.is_null() {
            let _: () = objc2::msg_send![settings as *mut AnyObject, setSharingType: kind];
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_window_sharing_none() {
    set_window_sharing_rust(0);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_window_sharing_readonly() {
    set_window_sharing_rust(1);
}

// ── R249: 信号安装迁 Rust ──

// 信号处理器只做 async-signal-safe 的自管道写入; 真正的保存/退出由专职线程
// 投递回主线程执行。原因: Ctrl-C 可能落在主线程上, 若在 handler 里直接碰
// AppKit/DB/日志锁, 会与主线程已持有的锁死锁, 表现就是 Ctrl-C 无法结束。
static SIG_WRITE_FD: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);

#[inline]
fn write_signal_byte(sig: i32) {
    let fd = SIG_WRITE_FD.load(std::sync::atomic::Ordering::Relaxed);
    if fd >= 0 {
        let byte = [sig as u8];
        unsafe { libc::write(fd, byte.as_ptr() as *const c_void, 1) };
    }
}

extern "C" fn signal_exit_handler(sig: i32) {
    // 仅 write(2): POSIX 列为 async-signal-safe。
    write_signal_byte(sig);
}

extern "C" fn exit_on_main(ctx: *mut c_void) {
    if ctx.is_null() {
        save_and_exit(0);
        return;
    }
    let sig = unsafe { *Box::from_raw(ctx as *mut i32) };
    save_and_exit(sig);
}

fn signal_dispatch_thread(read_fd: i32) {
    loop {
        let mut byte = [0u8; 1];
        let n = unsafe { libc::read(read_fd, byte.as_mut_ptr() as *mut c_void, 1) };
        if n == 1 {
            let ctx = Box::into_raw(Box::new(byte[0] as i32)) as *mut c_void;
            unsafe { glaspen2_swift_dispatch_main_after(0.0, ctx, exit_on_main) };
        } else if n < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() != std::io::ErrorKind::Interrupted {
                return;
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_install_signals() {
    use std::sync::Once;
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        let mut fds = [0i32; 2];
        if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
            tracing::warn!("[signal] pipe() 失败, 信号处理保持默认");
            return;
        }
        SIG_WRITE_FD.store(fds[1], std::sync::atomic::Ordering::SeqCst);
        std::thread::spawn(move || signal_dispatch_thread(fds[0]));
        unsafe {
            libc::signal(
                libc::SIGINT,
                signal_exit_handler as *const () as libc::sighandler_t,
            );
            libc::signal(
                libc::SIGTERM,
                signal_exit_handler as *const () as libc::sighandler_t,
            );
        }
    });
}

// ── R250: GIF/SVG 剪贴板桥迁 Rust ──

fn pasteboard_write_url(path: &str) {
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2_foundation::NSString;
    unsafe {
        let (Some(pb_cls), Some(url_cls), Some(array_cls)) = (
            AnyClass::get(c"NSPasteboard"),
            AnyClass::get(c"NSURL"),
            AnyClass::get(c"NSArray"),
        ) else {
            return;
        };
        let pb: *mut AnyObject = objc2::msg_send![pb_cls, generalPasteboard];
        if pb.is_null() {
            return;
        }
        let _: i64 = objc2::msg_send![pb, clearContents];
        let ns_path = NSString::from_str(path);
        let url: *mut AnyObject = objc2::msg_send![url_cls, fileURLWithPath: &*ns_path];
        if url.is_null() {
            return;
        }
        let arr: *mut AnyObject = objc2::msg_send![array_cls, arrayWithObject: url];
        if !arr.is_null() {
            let _: bool = objc2::msg_send![pb, writeObjects: arr];
        }
    }
}

fn newest_gif_path() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let dir = std::path::Path::new(&home).join("Desktop");
    let mut newest: Option<(std::time::SystemTime, String)> = None;
    for entry in std::fs::read_dir(dir).ok()? {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("glaspen2_") || path.extension().map(|e| e != "gif").unwrap_or(true) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let Ok(modified) = meta.modified() else {
            continue;
        };
        let p = path.to_string_lossy().into_owned();
        if newest.as_ref().map(|(t, _)| modified > *t).unwrap_or(true) {
            newest = Some((modified, p));
        }
    }
    newest.map(|(_, p)| p)
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_swift_copy_gif(data: *const u8, len: i32) {
    if data.is_null() || len <= 0 {
        return;
    }
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let name = format!("glaspen2_record_{}ms.gif", ms);
    let path = std::env::temp_dir().join(name);
    if std::fs::write(&path, bytes).is_err() {
        return;
    }
    pasteboard_write_url(&path.to_string_lossy());
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_swift_copy_newest_gif() {
    if let Some(p) = newest_gif_path() {
        pasteboard_write_url(&p);
    }
}

fn base64_encode(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i] as u32;
        let b1 = if i + 1 < data.len() {
            data[i + 1] as u32
        } else {
            0
        };
        let b2 = if i + 2 < data.len() {
            data[i + 2] as u32
        } else {
            0
        };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if i + 1 < data.len() {
            T[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if i + 2 < data.len() {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
        i += 3;
    }
    out
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_swift_copy_svg_html(svg: *const std::os::raw::c_char) {
    if svg.is_null() {
        return;
    }
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2_foundation::NSString;
    let svg = unsafe { std::ffi::CStr::from_ptr(svg) }.to_bytes();
    let html = format!(
        "<img src=\"data:image/svg+xml;base64,{}\" />",
        base64_encode(svg)
    );
    unsafe {
        let Some(pb_cls) = AnyClass::get(c"NSPasteboard") else {
            return;
        };
        let Some(str_cls) = AnyClass::get(c"NSString") else {
            return;
        };
        let pb: *mut AnyObject = objc2::msg_send![pb_cls, generalPasteboard];
        if pb.is_null() {
            return;
        }
        let _: i64 = objc2::msg_send![pb, clearContents];
        let s = NSString::from_str(&html);
        let t = NSString::from_str("public.utf8-plain-text");
        let _: bool = objc2::msg_send![pb, setString: &*s, forType: &*t];
    }
}

// ── R251: 显示帧/面板透明/压感窗恢复桥迁 Rust ──

extern "C" fn restore_pressure_cb(_ctx: *mut c_void) {
    pm_show();
    pm_refresh_label();
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_swift_apply_display_frames(w: i32, h: i32) {
    glaspen2_display_apply(w, h);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_panel_transparent_full(on: i32) {
    gl_panel_transparent_apply(on);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_swift_restore_pressure_window() {
    unsafe { glaspen2_swift_dispatch_main_after(0.0, std::ptr::null_mut(), restore_pressure_cb) }
}

// ── R252: 启动状态项/屏幕尺寸桥迁 Rust ──

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_swift_bootstrap_status_item() -> *mut c_void {
    glaspen2_rust_status_item_create()
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_swift_bootstrap_screen_size(w: *mut i32, h: *mut i32) {
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2_foundation::NSRect;
    unsafe {
        let Some(cls) = AnyClass::get(c"NSScreen") else {
            return;
        };
        let screen: *mut AnyObject = objc2::msg_send![cls, mainScreen];
        if screen.is_null() {
            return;
        }
        let rect: NSRect = objc2::msg_send![screen, frame];
        if !w.is_null() {
            *w = rect.size.width as i32;
        }
        if !h.is_null() {
            *h = rect.size.height as i32;
        }
    }
}

// ── R254: 已在运行告警迁 Rust ──

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_swift_bootstrap_running_alert() {
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2_foundation::NSString;
    let zh = unsafe { glaspen2_router_lang() } == 0;
    let title = if zh {
        "glaspen2 已在运行"
    } else {
        "glaspen2 is already running"
    };
    let info = if zh {
        "检测到另一个 glaspen2 实例(菜单栏图标)。两个实例的笔迹窗口会叠加显示, 全局热键会同时翻两套页 —— 看起来像“笔迹重复/只翻了一部分”。本次启动退出; 开发时如需并行调试, 设 GLASPEN2_ALLOW_MULTI=1。"
    } else {
        "Another glaspen2 instance is running (menu bar icon). Two overlays stack and global hotkeys flip both — it looks like duplicated strokes or a partial page flip. This launch exits; set GLASPEN2_ALLOW_MULTI=1 to debug in parallel."
    };
    let ok = if zh { "好" } else { "OK" };
    unsafe {
        let Some(app_cls) = AnyClass::get(c"NSApplication") else {
            return;
        };
        let app: *mut AnyObject = objc2::msg_send![app_cls, sharedApplication];
        if !app.is_null() {
            let _: () = objc2::msg_send![app, setActivationPolicy: 0i64];
        }
        let Some(alert_cls) = AnyClass::get(c"NSAlert") else {
            return;
        };
        let alert: *mut AnyObject = objc2::msg_send![alert_cls, new];
        if alert.is_null() {
            return;
        }
        let t = NSString::from_str(title);
        let i = NSString::from_str(info);
        let b = NSString::from_str(ok);
        let _: () = objc2::msg_send![alert, setMessageText: &*t];
        let _: () = objc2::msg_send![alert, setInformativeText: &*i];
        let _: *mut AnyObject = objc2::msg_send![alert, addButtonWithTitle: &*b];
        let _: i64 = objc2::msg_send![alert, runModal];
    }
}

// ── R260: 翻页诊断探针(dispatch 链) ──

static FLIP_FIRST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
static FLIP_BACK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static FLIP_EVERY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

extern "C" fn flip_probe_rust_tick(_ctx: *mut c_void) {
    use std::sync::atomic::Ordering;
    tracing::info!("[flip-probe] tick");
    if FLIP_FIRST.swap(false, Ordering::SeqCst) {
        let want = std::env::var("GLASPEN2_PROBE_PAGES")
            .ok()
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0);
        unsafe {
            glaspen2_debug_insert_pages(
                want,
                glaspen2_router_screen_w() as i32,
                glaspen2_router_screen_h() as i32,
            );
        }
        tracing::info!("[glaspen2] probe 建页 {want} 页(空白, 直插)");
    } else {
        let back = FLIP_BACK.load(Ordering::SeqCst);
        tracing::info!("[glaspen2] probe 翻页 back={}", i32::from(back));
        unsafe { glaspen2_router_perform_hotkey(if back { 0x32u16 } else { 0x12u16 }) };
        FLIP_BACK.store(!back, Ordering::SeqCst);
    }
    let every = FLIP_EVERY.load(Ordering::SeqCst);
    unsafe {
        glaspen2_swift_dispatch_main_after(every as f64, std::ptr::null_mut(), flip_probe_rust_tick)
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_flip_probe_maybe_start() {
    let Ok(raw) = std::env::var("GLASPEN2_FLIP_EVERY") else {
        return;
    };
    let Ok(every) = raw.parse::<u64>() else {
        return;
    };
    if every == 0 {
        return;
    }
    FLIP_EVERY.store(every, std::sync::atomic::Ordering::SeqCst);
    tracing::info!("[glaspen2] flip probe: 每 {every} 秒一跳(第1跳建页, 之后往返翻)");
    unsafe { glaspen2_swift_dispatch_main_after(0.5, std::ptr::null_mut(), flip_probe_rust_tick) }
}

#[cfg(test)]
mod pressure_regression_tests {
    #[test]
    fn raw_width_grows_with_pressure() {
        let zero = super::glaspen2_router_raw_width(0.0);
        let mid = super::glaspen2_router_raw_width(0.5);
        let full = super::glaspen2_router_raw_width(1.0);
        assert!(zero > 0.0, "raw width 必须为正: {zero}");
        assert!(mid > zero, "raw width 应随压力增大: {x}", x = mid);
        assert!(full > mid, "raw width 应随压力增大: {x}", x = full);
    }
}

#[cfg(test)]
mod hotkey_regression_tests {
    #[test]
    fn chord_only_dispatches_on_keydown() {
        assert!(super::chord_dispatch_is_keydown(10));
        assert!(!super::chord_dispatch_is_keydown(11));
    }
}

#[cfg(test)]
mod pen_kind_regression_tests {
    #[test]
    fn pen_event_kinds_follow_cgeventtype() {
        // CGEventType: mouseMoved=5 绝不能当成抬笔; 右/中键事件也不能错位。
        for k in [1, 3, 25] {
            assert!(super::pen_kind_down(k), "down kind {k}");
        }
        for k in [6, 7, 27] {
            assert!(super::pen_kind_drag(k), "drag kind {k}");
        }
        for k in [2, 4, 26] {
            assert!(super::pen_kind_up(k), "up kind {k}");
        }
        assert!(!super::pen_kind_up(5), "mouseMoved 不是抬笔");
        assert!(!super::pen_kind_down(4), "rightMouseUp 不是落笔");
    }
}

#[cfg(test)]
mod camera_route_regression_tests {
    #[test]
    fn camera_mouse_routes_left_down_drag_up() {
        assert_eq!(super::camera_mouse_dir(1), Some(0));
        assert_eq!(super::camera_mouse_dir(6), Some(1));
        assert_eq!(super::camera_mouse_dir(2), Some(2));
        assert_eq!(super::camera_mouse_dir(3), None); // 右键不是抓手
    }
}

#[cfg(test)]
mod pressure_source_tests {
    #[test]
    fn tablet_raw_field_wins_over_zero_nsevent_pressure() {
        assert!((super::normalized_tablet_pressure(0.0, 65535) - 1.0).abs() < 1e-9);
        assert!((super::normalized_tablet_pressure(0.0, 32768) - 0.50001).abs() < 1e-3);
        assert!((super::normalized_tablet_pressure(0.25, 0) - 0.25).abs() < 1e-9);
    }
}

#[cfg(test)]
mod color_quantize_tests {
    use super::quantize_raw_rgb;

    #[test]
    fn raw_matches_committed_rgb8_truncation() {
        let c = glaspen_core::draw_spec::Color::new(0.0, 0.639, 1.0);
        let (r, g, b) = quantize_raw_rgb(0.0, 0.639, 1.0);
        assert_eq!((r, g, b), {
            let (r, g, b) = c.to_rgb8();
            (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0)
        });
    }
}

#[cfg(test)]
mod recording_repeat_tests {
    use super::recording_hotkey_key;

    #[test]
    fn recording_keys_consume_autorepeat() {
        assert!(recording_hotkey_key(15, true, true, false));
        assert!(recording_hotkey_key(19, true, true, true));
        assert!(recording_hotkey_key(20, true, true, true));
        assert!(!recording_hotkey_key(19, true, true, false));
        assert!(!recording_hotkey_key(15, true, false, true));
        assert!(!recording_hotkey_key(0, true, true, true));
    }
}
