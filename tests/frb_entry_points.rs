//! flutter_rust_bridge 的入口符号必须出现在**主程序的导出表**里。
//!
//! macOS 设置面板的 Rust 代码以 rlib 静态链进主可执行文件,没有引用的归档
//! 成员会被链接器丢掉;而 Dart 侧是靠 `ExternalLibrary.process()`
//! (dlopen(NULL) + dlsym)在运行时解析这些符号的 —— 一旦丢掉,面板启动时才
//! 报"找不到符号",编译期完全看不出来。build.rs 用
//! `cargo:rustc-link-arg=-Wl,-u,_<symbol>` 强制保留,这个测试守住那道防线。
//!
//! 之所以检查产物而不是在测试里 dlsym:链接参数按 cargo 的定义只作用于
//! bin/cdylib,测试二进制拿不到这些符号(而且它也不是 Dart 实际加载的对象)。
#![cfg(target_os = "macos")]

use std::process::Command;

/// 必须能从 Dart 的 DynamicLibrary.process() 解析到的三个入口。
const ENTRY_POINTS: [&str; 3] = [
    "frb_pde_ffi_dispatcher_primary",
    "frb_pde_ffi_dispatcher_sync",
    "frb_dart_fn_deliver_output",
];

#[test]
fn app_binary_exports_frb_entry_points() {
    let bin = env!("CARGO_BIN_EXE_glaspen2");
    let output = Command::new("dyld_info")
        .args(["-exports", bin])
        .output()
        .expect("dyld_info 不可用(需要 Xcode Command Line Tools)");
    assert!(output.status.success(), "dyld_info 读取 {bin} 失败");
    let exports = String::from_utf8_lossy(&output.stdout);

    for name in ENTRY_POINTS {
        assert!(
            exports.contains(name),
            "flutter_rust_bridge 入口 {name} 不在 {bin} 的导出表里 —— \
             build.rs 里的 `-Wl,-u,_{name}` 可能失效了,Dart 侧会解析不到符号"
        );
    }
}

/// ObjC 侧(设置面板 shim)会调用这个 Rust 入口,同样必须导出。
#[test]
fn app_binary_exports_settings_notify_hook() {
    let bin = env!("CARGO_BIN_EXE_glaspen2");
    let output = Command::new("dyld_info")
        .args(["-exports", bin])
        .output()
        .expect("dyld_info 不可用(需要 Xcode Command Line Tools)");
    let exports = String::from_utf8_lossy(&output.stdout);

    for name in ["glaspen2_notify_settings_changed"] {
        assert!(
            exports.contains(name),
            "{name} 不在 {bin} 的导出表里:ObjC 的设置变化通知会链接失败"
        );
    }
}
