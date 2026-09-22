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

## 8. 发版前 GUI 冒烟清单

自动化覆盖不到"必须在主线程的 AppKit 行为"(以及所有真实的输入/权限行为),
所以打 DMG 之前手点一遍, 约 3 分钟。装刚打出来的包, 或直接 `cargo run`。

1. 启动后笔能画、橡皮能用(⌃⌘X 切换), 压力显示正常
2. 打开设置面板 → 右上角**没有**云朵断联图标
3. 设置项逐个改一次, 确认覆盖层立刻生效: 颜色 / 粗细 / 网格 / 显示附近 10 页 /
   描边 / 压力显示
4. 活页本 tab: 缩略图直接出现(没有占位图标)、点页面跳转、右键删除页面、导出 PDF
5. 自由涂鸦 tab: 在两页之间来回切标签(应能切到无限画布)、
   总览 / 回原点 / 居中 / 新建 四个按钮都有效
6. 数据备份: 「备份全部数据」→ 桌面出现 `glaspen2_backup_*.db`;
   「从最新备份恢复」→ 提示成功
7. 快速录制 GIF: ⌃⌘R 按住画几笔再松开 → 剪贴板里拿到动画 GIF
8. 快捷键按钮网格逐个点一遍
9. 退出重开: 设置与笔迹都还在(证明 schema 迁移与持久化正常)

任何一条不通过, 先跑这两条缩小范围(它们不启动 GUI):

```bash
cargo test
cd flutter_settings && fvm flutter test test/frb_wire_test.dart
```

---

## 9. 性能剖析(虚拟笔, 不占你的手)

性能问题分两层量, 都不用真笔:

**Rust 层**(模型器 + 缓冲 + pen-up 提交):

```bash
cargo test bench_virtual_stroke -- --ignored --nocapture              # debug
cargo test --release bench_virtual_stroke -- --ignored --nocapture    # release
```

**全链路**(合成 CGEvent 虚拟笔, 含接触前后的悬停流, 走真事件水龙头):

```bash
GLASPEN2_VIRTUAL_PEN=1 GLASPEN2_PERF_LOG=1 GLASPEN2_DB_PATH=/tmp/x.db cargo run
```

每 12 秒画 3 笔(悬停 0.5s + 画 2s + 悬停 0.5s), 屏幕上会真的出现笔迹。
`GLASPEN2_DB_PATH` 指向临时库, 不会碰真实数据。
`GLASPEN2_PERF_LOG=1` 写 `~/Library/Logs/glaspen2/perf.log`, 每行是
`时间戳 / 事件类型 / 微秒 / 备注` —— `drawrect` 行带 rect 尺寸与 `FULL` 标记
(用于识别全屏拷贝), `pen_move` / `pen_hover` / `pen_down` / `pen_up` 是各
事件入口的绝对耗时。**绝对值可以直接相加成 CPU 占比**, 不用猜采样率。

真笔的验证(压感手感、近距悬停、笔身按钮、以及真平板的事件流量):
`GLASPEN2_PERF_LOG=1` 启动后随手画 10 秒, 再看 perf.log 即可。

### 2026-09-22 基线(debug 构建, 3440×1440, 网格 80px + minimap 开)

| 项 | 成本 | 备注 |
| --- | --- | --- |
| `pen_move` | **114-128 µs/事件** | 模型器仅 1.4µs(debug)/0.15µs(release), 其余是 NSEvent 构造 + cairo 一段 + setNeedsDisplay |
| `pen_down` | ~1 ms/笔 | 落笔的 `db::begin_stroke`(INSERT+UPDATE 两段); UPDATE 已改后台写 |
| `pen_hover`(+tick) | 20-40 µs/事件 | |
| `drawrect` | 133→**78 µs/帧**(网格+minimap) / 45 µs(裸) | 网格+minimap 改为按脏区裁剪后 −41% |
| 帧率 | 0.35 帧/事件(≈86Hz) | 显示**本来就被 vsync 合并**, 不是每事件一帧 |
| 合计 | ≈ 120 µs/事件 | 200Hz 事件流 ≈ **2.4% 单核** |

已排除: 模型器(µs 级)、全屏拷贝(rect 实测只有十几像素, dirty-rect 正常)、
"每事件一帧"(帧数 0.35/事件, 显示本来就合并)。

已做: 网格线范围限定到脏区、minimap 与脏区不相交时整段跳过(帧成本 −41%)、
落笔的 edited 标记改后台写(移出落笔关键路径)。

**虚拟笔的合成事件一律在事件口吞掉**(打 `kCGEventSourceUserData` 标记):
之前悬停流走放行路径, 会真的移动用户的光标 —— 测试工具绝不抢鼠标;
且每次测量都应"同一条命令内 测完即杀进程"。

---

## 10. 相关的其它入口

| 场景 | 命令 |
| --- | --- |
| Dart 静态检查 | `cd flutter_settings && fvm flutter analyze` |
| Rust 单测 | `cargo test`(含 `api`/`db`/`export` 的回归测试) |
| Flutter 测试(含 wire 冒烟) | `cd flutter_settings && fvm flutter test` |
| 完整打包(含符号断言) | `bash scripts/build-dmg.sh` → `release_history/*.dmg` |
