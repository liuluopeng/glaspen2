# macOS Swift 原生层目录

本目录是 glaspen2 的 macOS 应用层(Swift 5, 单一 Swift 模块)。`build.rs`
递归收集这里的 `*.swift` 一次性编译, 因此**目录只用于工程组织, 不改变编译单元**:
所有文件之间可以直接互相引用, 不需要 import。

## 目录结构

| 目录 | 职责 | 典型文件 |
| --- | --- | --- |
| `App/` | 应用生命周期/启动编排/窗口与状态栏/显示器变化/UI 锚点/C ABI 入口 | `app_bootstrap`、`startup_*`、`status_bar`、`run_glue` |
| `Menu/` | 菜单构建/文案与勾选刷新/菜单 handler/动作分派 | `menu_builder*`、`menu_handler`、`action_dispatch` |
| `Input/` | CGEventTap、笔/光标/压感状态、实时绘制、快捷键与 chord | `event_router`、`pen_state`、`raw_drawing`、`shortcut_*` |
| `Surface/` | cairo 表面/覆盖层/页视图/缩略图/翻页与隧道/镜头/面板显示胶水 | `surface_*`、`overlay_*`、`tunnel_*`、`minimap` |
| `Settings/` | 渲染/视图/笔状态, 设置分派与快照 API, Flutter 面板宿主 | `settings_*`、`render_settings`、`settings_panel` |
| `Features/` | 涂鸦/飘渺模式、录制与 GIF、背景保存、反色捕获 | `draw_mode`、`recording_*`、`save_background` |
| `Diagnostics/` | 性能日志、隐藏自检探针、虚拟笔 | `perf_log`、`*_probe`、`virtual_pen` |

根目录只保留 C ABI 边界头文件:

- `Bridging.h`: Swift → C/ObjC 声明入口(`-import-objc-header`); `#include "rust_ffi.h"`
- `rust_ffi.h`: Rust FFI 声明

## 规则

1. **文件名全局唯一**(snake_case)。`swiftc -c` 的目标文件名只取 basename,
   不同目录同名会互相覆盖; `build.rs` 会在编译前直接断言报错。
2. **新文件放对应子系统目录**; 只有一层子目录, 不再按更细粒度嵌套。
   新子系统目录需要同时更新本 README 和 `tests/swift_layout.rs` 的允许列表。
3. **C ABI 边界不变**: `@_cdecl("glaspen2_...")` 符号名/签名保持; 头文件只在根目录,
   子目录不新增头文件。
4. Rust 契约测试用 `include_str!(".../crates/glaspen-macos/swift/<Dir>/<file>.swift")` 直接读源码;
   移动文件时同步更新这些路径(`grep -rn 'crates/glaspen-macos/swift/' crates src tests`)。
5. 应用层不得新增 `.m`/`.mm`; 新增原生代码一律 Swift。
6. 目录布局由 `tests/swift_layout.rs` 守护(无平铺文件、无未知目录、basename 唯一)。
