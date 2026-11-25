//! macOS AppKit 壳的 Rust 预研(R14): 先用最外层状态栏项创建验证 objc2
//! 在真实 app 内可工作; 默认仍走 Swift, 由 `GLASPEN2_RUST_STATUS_ITEM=1` 切换。
#![cfg(target_os = "macos")]

use std::ffi::c_void;

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
