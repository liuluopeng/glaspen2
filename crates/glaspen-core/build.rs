// glaspen-core 的构建脚本:仅 macOS 需要把 cairo 的链接指令发给 cargo ——
// 测试二进制因此**链接** libcairo,运行时 find_loaded_cairo_path() 才能在
// 已加载映像里枚举到它(dlopen 兜底只认 Homebrew 固定路径)。
// 与根 build.rs 的 pkg-config 探测保持同一来源,版本/前缀跨机器可移植。
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        let cairo = pkg_config::Config::new()
            .probe("cairo")
            .expect("cairo not found via pkg-config (brew install cairo)");
        for p in &cairo.link_paths {
            println!("cargo:rustc-link-search=native={}", p.display());
        }
        println!("cargo:rustc-link-lib=dylib=cairo");
    }
}
