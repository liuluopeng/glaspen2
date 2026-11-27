# glaspen-macos

macOS 原生层 crate：

- `src/macos_shell.rs` — AppKit/SCK/Flutter 宿主、C ABI、菜单/事件/窗口/隧道渲染调用；
- `src/entry.rs` — macOS 应用入口 `macos_run()`；
- `swift/` — Swift 原生层（Bridging.h / rust_ffi.h + App/Menu/Surface），仍是 C ABI 的 Swift 侧；

纯逻辑/存储/导出在 `crates/glaspen-core`。

Swift/Flutter 的编译、Framework 链接与 `-u` FRB 符号保留由根 `build.rs` 统一完成（最终二进制
由根 crate 链接，Windows 分支和 FRB link args 也在根 build.rs）；本 crate 只提供 Rust 库目标。
