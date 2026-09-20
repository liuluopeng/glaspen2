//! flutter_rust_bridge 的 C 入口必须出现在**主程序的导出表**里。
//!
//! macOS 设置面板的 Rust 代码以 rlib 静态链进主可执行文件,没有引用的归档
//! 成员会被链接器丢掉;而 Dart 侧是靠 `ExternalLibrary.process()`
//! (dlopen(NULL) + dlsym)在运行时解析这些符号的 —— 一旦丢掉,面板启动时才
//! 报"找不到符号",编译期完全看不出来。build.rs 用
//! `cargo:rustc-link-arg-bins=-Wl,-u,_<symbol>` 逐个强制保留,这里守住它。
//!
//! 之所以检查产物而不是在测试里 dlsym:链接参数按 cargo 的定义只作用于
//! bin/cdylib,测试二进制拿不到这些符号(它也不是 Dart 实际加载的对象)。
#![cfg(target_os = "macos")]

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

/// Dart 在 init 阶段就会查这几个:少任何一个,面板直接连不上 Rust。
const INIT_CRITICAL: [&str; 4] = [
    "frb_pde_ffi_dispatcher_primary",
    "frb_pde_ffi_dispatcher_sync",
    "frb_get_rust_content_hash",
    "frb_init_frb_dart_api_dl",
];

fn exported_symbols(path: &str) -> BTreeSet<String> {
    let output = Command::new("dyld_info")
        .args(["-exports", path])
        .output()
        .expect("dyld_info 不可用(需要 Xcode Command Line Tools)");
    assert!(output.status.success(), "dyld_info 读取 {path} 失败");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1))
        .map(str::to_owned)
        .collect()
}

fn app_binary() -> &'static str {
    env!("CARGO_BIN_EXE_glaspen2")
}

#[test]
fn app_binary_exports_init_critical_symbols() {
    let exports = exported_symbols(app_binary());
    for name in INIT_CRITICAL {
        assert!(
            exports.contains(&format!("_{name}")),
            "flutter_rust_bridge 入口 {name} 不在 {} 的导出表里 —— \
             build.rs 里的 `-Wl,-u,_{name}` 可能失效了,Dart 侧会解析不到符号",
            app_binary()
        );
    }
}

/// 漂移守卫:主程序必须导出 cdylib 里的**每一个** `_frb_*` 符号。
///
/// 基准取同一 profile 的 cdylib —— 它是这个 crate 的完整符号集,而 Dart 能
/// lookup 的 `frb_*` 名字与它一一对应。升级 FRB 或重新生成绑定后,如果多出
/// 入口而 build.rs 的列表没跟上,这里立刻失败,而不是等用户打开面板。
#[test]
fn app_binary_exports_every_frb_symbol_the_cdylib_has() {
    let bin = app_binary();
    let dylib = Path::new(bin).with_file_name("libglaspen2.dylib");
    if !dylib.exists() {
        eprintln!("跳过漂移守卫:{} 不存在(先跑 cargo build)", dylib.display());
        return;
    }
    let bin_exports = exported_symbols(bin);
    let missing: Vec<String> = exported_symbols(&dylib.to_string_lossy())
        .into_iter()
        .filter(|s| s.starts_with("_frb_") && !bin_exports.contains(s))
        .collect();
    assert!(
        missing.is_empty(),
        "这些 flutter_rust_bridge 符号在 cdylib 里有、主程序里没有:{missing:?}\n\
         把它们加进 build.rs 的 FRB_SYMBOLS(或先 cargo build 刷新 cdylib)"
    );
}

/// ObjC 侧(设置面板 shim)会调用这个 Rust 入口,同样必须导出。
#[test]
fn app_binary_exports_settings_notify_hook() {
    let exports = exported_symbols(app_binary());
    assert!(
        exports.contains("_glaspen2_notify_settings_changed"),
        "glaspen2_notify_settings_changed 未导出:ObjC 的设置变化通知会链接失败"
    );
}
