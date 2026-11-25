//! macOS AppKit 壳的 Rust 预研(R14): 先用最外层状态栏项创建验证 objc2
//! 在真实 app 内可工作; 默认仍走 Swift, 由 `GLASPEN2_RUST_STATUS_ITEM=1` 切换。
#![cfg(target_os = "macos")]

use std::ffi::c_void;

use objc2::MainThreadOnly;
use objc2::rc::Retained;
use objc2_app_kit::NSStatusBar;
use objc2_foundation::MainThreadMarker;

/// 创建方形 NSStatusItem, 返回 +1 持有的裸指针(由 Swift 侧接管引用;
/// 本项目 app 生命周期 = 进程生命周期, 不额外实现释放)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_rust_status_item_create() -> *mut c_void {
    let Some(mtm) = MainThreadMarker::new() else {
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
