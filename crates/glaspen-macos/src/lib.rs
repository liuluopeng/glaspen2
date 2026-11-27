//! macOS 原生层 crate: AppKit/SCK/Flutter 宿主 + Swift 桥 + macOS 应用入口。
//!
//! 纯逻辑/存储/导出仍在 `glaspen-core`; 本 crate 只承载 macOS 专属的
//! 原生对象生命周期、C ABI 与 Swift 原生源码。
#![allow(clippy::not_unsafe_ptr_arg_deref)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]

#[cfg(target_os = "macos")]
pub mod entry;
#[cfg(target_os = "macos")]
pub mod macos_shell;

#[cfg(target_os = "macos")]
pub use entry::macos_run;
