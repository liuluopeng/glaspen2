# 调试 glaspen2 — macOS (docs/debugging.md)

> 设置面板是**嵌在同一进程里的 Flutter 视图**, Rust 代码就在主可执行文件内,
> 两侧通过 flutter_rust_bridge (FRB) 直接调用 —— 所有调试手段都在**一个进程**
> 里, 不需要 attach 两个东西。本文只写已验证可行的手法。

---

## 1. 日常循环: `cargo run`

```bash
cargo run
```

和 FRB 之前一样: 改 Rust 直接重编; 改 `flutter_settings/lib/main.dart` 或
`flutter_settings/assets/` 时, `build.rs` 会自动重跑
`fvm flutter build macos-framework`(默认 Release, 增量约 10-20s)。

**注意 `build.rs` 的这一步失败时不会让 `cargo build` 失败**(只打印一行), 所以
Dart 改动"没生效"时先看构建输出里有没有:

```
[build.rs] fvm flutter build macos-framework [...] failed: ...
```

---

## 2. 看日志

Dart 的 `debugPrint` 和 Rust 的 `println!/eprintln!` 走同一个 stdout,
带 `flutter:` 前缀的是 Dart 侧。例如面板连不上 Rust 时:

```
flutter: [Settings] load failed: Invalid argument(s): Failed to lookup symbol ...
```

---

## 3. 断点 (lldb)

Rust 与 ObjC **都带行号**。ObjC 的调试信息来自 `build.rs` 在 debug 构建里给
`clang` 加的 `-g`; DWARF 本身由 cargo 的 unpacked 模式留在 `.o` 里, lldb 通过
二进制里的 OSO 记录自动加载(所以 `dwarfdump` 直接看二进制会以为没有)。

```bash
lldb target/debug/glaspen2
(lldb) b glaspen2_macos_settings_json            # ObjC: glaspen2.m:1050
(lldb) b src/macos/glaspen2.m:1050               # 文件:行 —— 要带目录, 裸文件名不解析
(lldb) b glaspen2::export::page_thumbnails_blob  # Rust: export.rs:2371
(lldb) b glaspen2_render_thumbnail               # no_mangle 的 C 入口也行
(lldb) run
```

### 线程模型(断点命中时看 `thread list`)

面板发起的调用**不在主线程**:

- FRB 的 async 任务跑在它自己的 tokio 工作线程上 → 断在 `api::*` 里是
  `tokio-rt-worker`;
- 因为本项目所有 FFI 入口内部都用 `runtime().block_on` 桥接 async SQLite,
  在 tokio 上下文里再 `block_on` 会 panic, 所以 `api::run_blocking` 会把阻塞
  工作换到一个**新线程**上执行 → 断在 `export::*` 里常常是这个无名线程;
- 真正碰 AppKit 的部分由 ObjC shim 切到**主线程**执行
  (`gl_run_on_main_sync`) → 断在 `canvas_overview_payload` /
  `rebuild_surface_from_strokes` 这类代码里应该在主线程。

---

## 4. 不启动 GUI 调 Dart ↔ Rust

协议层的问题(编解码、参数、返回值、二进制块)不用开面板:

```bash
cd flutter_settings
fvm flutter test test/frb_wire_test.dart
```

它用生成的 Dart 绑定直接加载 `target/debug/libglaspen2.dylib`, 跑真实的 FRB
wire: 结构体、`Int64List`、缩略图二进制块、以及会走 ObjC shim 的写操作。
**改完 Rust 先 `cargo build` 再跑**(cdylib 才会更新)。

---

## 5. 面板连不上 Rust(云朵断联图标)

症状: 面板右上角出现云朵图标, 日志是 `Failed to lookup symbol 'frb_...'`。

Dart 用 `DynamicLibrary.process()` 在运行时 dlsym 这些符号, 而 Rust 是以 rlib
静态链进主程序的 —— 没有被引用的归档成员会被链接器丢掉, **编译期完全看不出来**。
体检:

```bash
cargo test --test frb_entry_points
```

其中 `app_binary_exports_every_frb_symbol_the_cdylib_has` 是漂移守卫: 以同
profile 的 cdylib 导出集合为基准, 主程序必须覆盖其中每个 `_frb_*`。升级 FRB 或
重新生成绑定后漏项会立刻失败。

修法: 把缺的符号加进 `build.rs` 的 `FRB_SYMBOLS`。`scripts/build-dmg.sh` 打包前
会用同一基准再断言一次, 失败即中止(不会产出装上去才发现坏掉的 DMG)。

---

## 6. Rust panic 怎么表现

FRB 会把 Rust panic 捕获成 Dart 的 `PanicException`(进程不崩), 原始信息在 Dart
日志里。最典型的一条:

```
Cannot start a runtime from within a runtime
```

含义: 某个 FFI 入口在 tokio 运行时上下文里调了 `runtime().block_on`。
**新加 FRB 接口时, 只要它会转到同步 FFI/ObjC, 就要经 `api::run_blocking`。**

---

## 7. 调 Dart UI(热重载)

嵌入的 framework 默认是 **Release(AOT)**: 没有 VM service, 不能热重载, 只能用
日志。需要热重载时切到 Debug 配置(JIT, `App.framework` 里含 `kernel_blob.bin`):

```bash
GLASPEN2_FLUTTER_CONFIG=Debug cargo run
```

`build.rs` 会改为构建并链接 `framework/Debug`(只构建这一个配置, 省时间);
引擎以 JIT 启动, 一般会打印 VM service 地址, 然后另开一个终端:

```bash
cd flutter_settings && fvm flutter attach --debug-url=<上面打印的地址>
```

> **状态**: 已验证"切到 Debug 后二进制正确链接 Debug 框架"(rpath 指向
> `framework/Debug/...`, 默认仍是 `Release/...`); 引擎 JIT 启动 + `flutter
> attach` 热重载这一段**未在无 GUI 环境验证过**, 需要实际跑一次确认。

---

## 8. 相关的其它入口

| 场景 | 命令 |
| --- | --- |
| Dart 静态检查 | `cd flutter_settings && fvm flutter analyze` |
| Rust 单测 | `cargo test`(含 `api`/`db`/`export` 的回归测试) |
| Flutter 测试(含 wire 冒烟) | `cd flutter_settings && fvm flutter test` |
| 完整打包(含符号断言) | `bash scripts/build-dmg.sh` → `release_history/*.dmg` |
