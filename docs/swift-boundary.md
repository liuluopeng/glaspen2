# Swift 迁移边界规则(OBJC ⇄ SWIFT)与 S3+ 文件映射
> **状态更新(2026-11-25, S7ei)**:macOS 应用层 ObjC 已全部迁 Swift,仓库内不再有
> `.m/.mm` 实现。本文档作为历史约束保留(解释为何边界这样设计、回调/类型规则
> 为何如此);当前跨语言面只剩 `Bridging.h` + `rust_ffi.h` 的 **C ABI/FFI 声明**,
> 不再有 ObjC↔Swift 双向消息面。新增原生代码一律写 Swift,经 C ABI 与 Rust 交互。

> 阶段:S2(边界设计 + 反向互调验证)。基线:main @ 3f60b96(S1 已合并)。
> 分支:`feat/swift-s2`。姊妹文档:`docs/swift-migration-plan.md`(台账)、
> `docs/swift-s0-spike-report.md`(spike 结论)。
> 本文件是**约束**,不是建议:跨语言新增/修改代码前先对照这里,拿不准就保留 ObjC。

## 1. 总原则

1. **边界 = C ABI + ObjC 消息**。Rust↔macOS 的 `glaspen2_macos_*` C ABI 不变
   (`glaspen2_macos_open_url` 已由 Swift `@_cdecl` 接管,符号/签名不动)。
2. ObjC↔Swift 之间只传**两边都能在头文件里写出来**的类型;Swift-only 类型
   (struct/enum/generics/async/throws/闭包…)一律留在 Swift 内部,不跨语言。
3. 一个阶段只迁移一个模块,保留旧实现 fallback(env flag)至少一个版本;
   行为等价优先于"写起来漂亮"。
4. 热路径(event tap、实时描边、cairo 光栅化)不为了迁 Swift 而迁;
   延迟/行为有疑问就留 ObjC(S2 边界规则同样适用于"不迁")。

## 2. 允许 / 禁止跨边界的类型

### 2.1 允许(白名单)

| 类别 | 说明 |
| --- | --- |
| C 基础类型 | `char/int/long long/double/float/bool`、C 数组、指针(`char *`、`void *`、`double *` 出参) |
| C 结构体 | 仅在**两侧都**用同一 C 布局时(Swift 侧用 import 到的 C struct 或显式 `@_cdecl` 参数);新写结构体优先只作为不透明指针 |
| NSObject 子类 | ObjC 类被 Swift 继承/调用,或 Swift `@objc public class X: NSObject` 被 ObjC 调用(本 S2 的 `SwiftInteropProbe`) |
| @objc 成员 | 方法/属性/初始化器,参数/返回值为下方 ObjC 兼容类型;selector 与旧实现逐字一致 |
| Foundation 桥接类型 | `NSString ↔ String`、`NSArray/NSDictionary ↔ Array/Dictionary`(**只在 @objc 面**,C ABI 面仍用 C 指针/缓冲区) |
| C 函数指针 | `@convention(c)` 的函数值,可作参数/回调;配合 `void *refcon` 传上下文 |

### 2.2 禁止(黑名单)

- Swift **struct / enum / tuple / generics / 泛型协议 / associatedtype** 出现在
  跨语言签名里(包括 `UnsafeMutablePointer<SwiftStruct>`);
- **async/await、Task、actor、throws** 跨 C/ObjC 边界(错误用返回码/出参表达);
- Swift **闭包**跨边界(除非显式 `@convention(block)` 且 ObjC 侧确实需要 block:
  仅限一次性回调,必须明确 copy/lifetime);
- Swift `String`/`Array`/`Dictionary` 直接出现在 `@_cdecl` 的 C 签名里;
- 把 Swift 对象指针伪装成 `void *` 交给 ObjC 长期持有(要持有就 `Unmanaged.passRetained`,
  并约定 release 点);
- 任何 `@objc` API 使用 Swift-only 的默认参数/重载(生成头里会消失或二义)。

**判据**:这段 API 能不能在 `Bridging.h` / `*-Swift.h` 里写出等价的 C/ObjC 声明?
写不出 → 不许跨边界。

## 3. 回调约定

### 3.1 C 回调(@convention(c) + Unmanaged)

```swift
// 注册方: 上下文必须是 stable 的引用; 生命周期 >= 回调注册期
let ctx = Unmanaged.passUnretained(self).toOpaque()   // 长期存活对象
CGHIDEventTapCreate(..., callback, ctx)               // callback 类型 @convention(c)

// 回调里: 还原上下文, 绝不做 retain/release 失衡的操作
let me = Unmanaged<EventTapController>.fromOpaque(refcon!).takeUnretainedValue()
```

- `passUnretained` 用于"注册方保证存活"的全局对象(现有 ObjC 全局单例语义);
  只有真正需要跨注册期持有时才 `passRetained` + 明确 `release`。
- 回调内**不分配**、不加锁、不捕获 Swift 闭包(热路径);需要 defer 就投
  dispatch queue。`CGEventTap` 回调在主 runloop 线程,见 §3.3。
- 回调签名必须和 C typedef 一致(可用
  `@convention(c) (CGEventTapProxy, CGEventType, CGEvent?, UnsafeMutableRawPointer?) -> Unmanaged<CGEvent>?`)。

### 3.2 ObjC 回调(target/action、delegate、block)

- target/action: target 必须是 NSObject 子类 + `@objc` 方法;selector 与旧实现
  完全一致(菜单项 tag/selector 是行为契约,不能改名)。target 由持有者强引用
  (现有 `g_menuHandler` 是全局强引用;Swift 侧同样要保证 target 存活,
  不要依赖 unretained target)。
- delegate: Swift 类实现 `@objc` 协议方法即可;注意 delegate 通常是 weak,
  持有方要保证生命周期。
- block: 只在 ObjC API 强制要求时使用,Swift 侧写 `@convention(block)`;
  跨调用保存前在 ObjC 侧 copy。

### 3.3 线程/主线程规则

- **AppKit / CoreGraphics 显示状态(main runloop)只能在主线程改**:
  window、layer、menu、status item、cursor。
- `CGEventTap` 回调运行在 tap 安装时的 runloop(本项目=主 runloop):回调内可直接
  读全局状态;任何耗时/后台工作 dispatch 出去,结果回主线程再写 UI。
- 非主线程进入 Swift/ObjC 桥接函数时:先 `DispatchQueue.main.sync`(语义同
  ObjC `gl_run_on_main_sync`)再碰 AppKit;Rust 侧回调(如 refresh hook)
  依赖被调方自己回主线程。
- **锁序不变**:STROKES/INK_DRAFT/CANVAS 等全局锁不嵌套获取;提交钩子在放掉
  STROKES 锁后才扇出(见 AGENTS.md)。

## 4. C 字符串 / 内存所有权

| 场景 | 规则 |
| --- | --- |
| 入参 `const char *` | 借用的,仅在调用期间有效;Swift 侧当次调用内 `String(cString:)` 拷走,不得缓存指针 |
| ObjC 返回 `char *`(malloc) | 调用方用 `glaspen2_macos_free_c_string(p)` 释放;不得与 `free`/`glaspen2_free_c_string` 混用 |
| Rust FFI 返回 `char *`(Rust 分配) | 调用方用 `glaspen2_free_c_string(p)` 释放;入口注释里已按函数逐个写明(见 glaspen2.m 顶部) |
| Swift `@_cdecl` 返回 `char *`(给 C 调用) | 必须返回 **malloc/strdup** 的缓冲区,并在头文件注释里写清由谁释放;绝不能返回 Swift `String` 内部指针 |
| 出参 `double *` / `int *` | 调用方分配、调用方释放;允许传 `NULL` 表示不取(按各函数注释) |
| ObjC 对象参数 | ARC 管理;`void *` 与对象互转用 `(__bridge …)`;跨语言持有用 `Unmanaged` 显式规则 |

**S2 已按此规则实现**:Swift `ping()` 返回 ObjC 兼容 `NSString`,ObjC 探针函数
`int glaspen2_macos_objc_probe(const char *)` 只借用 tag 指针、无分配。

## 5. 命名规则

| 类别 | 规则 | 例子 |
| --- | --- | --- |
| C ABI 符号 | 保持 `glaspen2_*` / `glaspen2_macos_*` 原名,Swift 用 `@_cdecl("...")` 逐字导出,禁止改签名/改名 | `glaspen2_macos_open_url` |
| Swift 导出给 ObjC 的类 | `@objc public final class`;类名与 ObjC 现有类不冲突(现有 `GlaspenMenuHandler`/`GlaspenDrawView`/…);需要时用 `@objc(Name)` 显式指定 | `SwiftInteropProbe` |
| Swift 导出给 ObjC 的方法 | `@objc` + ObjC 兼容类型;方法名保持"动词+ObjC 风格" | `@objc public static func ping() -> String` |
| ObjC selector | 与旧实现逐字一致(菜单 target/action 契约) | `selectColor:`、`toggleDraw`、`showSettingsPanel` |
| 文件 | Swift 文件 `snake_case.swift`,按模块前缀扁平放 `src/macos/swift/`(`menu_bar.swift`、`event_tap.swift`);ObjC 文件保持现有 `glaspen2.m` / `parts/*.m` | `src/macos/swift/interop_probe.swift` |
| 桥接头 | 只放 C/ObjC 声明,`extern "C"` 包裹;不 include Swift 生成头 | `src/macos/swift/Bridging.h` |
| 生成头 | `swiftc -emit-objc-header-path $OUT_DIR/glaspen2_macos_swift-Swift.h`;**不提交、不手改** | — |

## 6. Swift 版本 / 并发策略

- 语言模式:**Swift 5 模式**(`swiftc -swift-version 5`),不启用 Swift 6 严格并发检查。
  这是**刻意决策**:现有 ObjC 全局状态模型与 Swift 6 的 Sendable/actor 隔离不兼容,
  升级要独立阶段评估(见迁移计划 S7/S8 之后再谈)。
- 不跨边界用 async/await;不在启动/事件回调里创建 Task;需要异步就沿用现有
  dispatch queue 语义。
- `@MainActor` 等并发标注**不跨越 ObjC 边界使用**:Swift 5 模式下它们只影响
  Swift 内部,不改变 ObjC 调用方语义;主线程靠约定 + 代码注释保证(§3.3)。
- 全局可变状态优先留在 ObjC/Rust,直到该模块整体迁移完成;Swift 迁移部分
  尽量用 `final class` + 单例/显式传参,避免 `static var` 跨线程裸写。
- 部署目标固定 **macOS 12.0**(与 `scripts/build-dmg.sh` 的 LSMinimumSystemVersion
  一致);工具链兼容库/搜索路径见迁移计划 §8.2。

## 7. 构建集成(S2 落地)

`build.rs`(macOS 分支)顺序与职责:

1. `xcrun swiftc -c -parse-as-library -module-name glaspen2_macos_swift
   -swift-version 5 -target <arch>-apple-macosx12.0 -sdk <SDK> -O
   -import-objc-header src/macos/swift/Bridging.h
   -emit-objc-header -emit-objc-header-path $OUT_DIR/glaspen2_macos_swift-Swift.h
   src/macos/swift/*.swift`
   → `$OUT_DIR/swift/*.o` + 生成头;
2. `clang -fobjc-arc -O2 -Wall -I$OUT_DIR [-g] ... -c src/macos/glaspen2.m` 与
   `-c src/macos/swift/InteropProbe.m`(两单元各自编;ObjC 可
   `#import "glaspen2_macos_swift-Swift.h"`);
3. `ar crus $OUT_DIR/libglaspen2_objc.a` 收入 ObjC + Swift 全部 .o;
4. 链接参数(Flutter frameworks、cairo、Swift 运行时搜索路径/rpath)不变。

**顺序很重要**:Swift 头必须先于 ObjC 编译生成;新增 ObjC 桥接单元时要加进
`build.rs` 的 `compile_objc` 列表(否则不参与链接)。`src/macos/swift/` 目录级
`rerun-if-changed` 已覆盖新 .swift;新增 .m 也放进该目录以免漏监视。

**新增一条跨边界调用的 checklist**:

1. ObjC/C 侧函数先写进 `Bridging.h`(签名与定义逐字一致);
2. Swift 侧 `@_cdecl`(C ABI)或 `@objc`(ObjC 面)实现,类型过 §2 白名单;
3. 需要 ObjC 调 Swift 时,Import 生成头(不要手写 extern 声明);
4. `cargo build` → 跑 `target/debug/glaspen2 --swift-probe` 之类无 GUI 自检;
5. 行为验证通过才进下一阶段;不通过就保留 ObjC。

## 8. S3+ 文件映射计划

目标:一次迁一个模块,ObjC 旧实现保留一版作 fallback(env flag),S7 统一删除。
Swift 文件暂**扁平**放 `src/macos/swift/`(build.rs 当前只扫顶层 .swift;
若要子目录,需先改 build.rs 递归收集)。

| 阶段 | 现在(ObjC) | 迁到(Swift) | 桥接/边界要点 |
| --- | --- | --- | --- |
| S3 菜单栏 | `parts/menu.m` + `glaspen2_run` 里建菜单段(run.m 3018–3169):状态栏项、菜单构建、`GlaspenMenuHandler` target/action、图标(attributedTitle+NSTextAttachment)、语言重建、动态重建 | `menu_bar.swift`(NSStatusItem+NSMenu 构建/重建/语言)、`menu_actions.swift`(@objc GlaspenMenuHandler,selector 逐字保留)、`menu_icons.swift`(图标/checkmark/状态图标绘制) | 无 Swift-only 类型;文案走 `glaspen2_i18n` + `glaspen2_free_c_string`(经 Bridging.h)或保留 ObjC `gl_text` 由 ObjC 侧提供 C 入口;菜单项 tag/selector 是契约;旧 ObjC 构建保留,env `GLASPEN2_SWIFT_MENU=1` 切换,默认待验收后再默认新实现 |
| S4 HUD/通知/面板辅助 | `glaspen2.m` 的 `show_notification`/`show_page_info`、`run.m` 的 `rec_show_hud`、`menu.m` 的压强计窗口 `pm_*`、各 NSAlert | `hud_notification.swift`、`hud_recording.swift`、`pressure_monitor.swift`、`alerts.swift` | 文案继续来自 core i18n(不搬字符串);NSTimer/dispatch source 生命周期;与飘渺/玻璃显隐的联动状态仍由 ObjC 全局驱动,通过 narrow C 入口查询 |
| S5 窗口/覆盖层 | `glaspen2_run` 窗口搭建段(run.m 3390–3449)、`panel.m` 的显示变更防抖 `display_change_apply`/`on_display_changed`、页面 scale-to-fit 视图变换 | `overlay_window.swift`(NSWindow/层级/透明/ignoresMouseEvents/内容视图/磨砂层)、`display_debounce.swift`(2.5s 防抖 + 到期重读真实几何)、`page_view.swift`(scale-to-fit 变换) | 不含 event tap、不含 cairo 绘制(DrawView 仍 ObjC);窗口语义逐项对照;多屏/全屏/锁屏唤醒手测清单 |
| S6 事件 tap(最后) | `run.m` 的 `event_tap_install/reinstall`/`event_tap_callback`/热键路由/虚拟笔/录制暂停 | `event_tap.swift`(@convention(c) 回调 + Unmanaged 上下文)、`hotkeys.swift`(`perform_hotkey`/`perform_action`) | 热路径:**先与 ObjC 双实现并行日志对比**,确认延迟无退化再切;回调内零分配;失败立刻回滚 |
| S7 清理 | 剩余 `glaspen2.m`/`parts/*.m`、C shim | 剩余 C shim 改 Swift `@_cdecl`;build.rs 只留 swiftc(+cairo C) | `git grep` 无 ObjC 应用源;全测试/parity/包大小/启动对比 |

**各阶段共同约束**:

- 迁移中 ObjC 与 Swift 同时存在:旧实现不删,用 env flag/编译期开关切换;
  行为等价验收(截图/日志/清单)通过后才改默认路径;
- 迁移引发的 Rust core/Flutter/Windows 改动必须单独提交并更新
  `docs/duplication-backlog.md`;
- 每阶段跑完整验收:build(clean+incremental)、全量测试、parity、
  `cargo clippy -p glaspen2 -- -D warnings`、本阶段手测清单。

## 9. S2 验证记录(2026-11-23)

构建/探针 commit: `b780331`(分支 `feat/swift-s2`)。实现:

- `src/macos/swift/Bridging.h`:Swift→ObjC/C 声明(桥接头),当前含探针
  `int glaspen2_macos_objc_probe(const char *tag)` 与两个 ObjC 入口声明;
- `src/macos/swift/interop_probe.swift`:`@objc public final class
  SwiftInteropProbe: NSObject`,`@objc public static func ping() -> String`,
  内部经桥接头调 ObjC 探针并返回 `"swift-pong-<code>"`;
- `src/macos/swift/InteropProbe.m`:ObjC 侧探针单元(单独编译),
  `#import "glaspen2_macos_swift-Swift.h"` 后调 `[SwiftInteropProbe ping]`;
  `GLASPEN2_SWIFT_PROBE=1` 时在 `glaspen2_run` 启动早期执行一次(默认 no-op);
- `src/main.rs`:隐藏调试入口 `glaspen2 --swift-probe`,不启动 GUI,跑完即退;
- `build.rs`:见 §7(swiftc 先产 `.o`+生成头,clang 后编 ObjC,`-I $OUT_DIR`)。

实测(无 GUI):

```
$ target/debug/glaspen2 --swift-probe
[swift-s2-probe] objc->swift: calling [SwiftInteropProbe ping]
[swift-s2-probe] objc<-swift OK: glaspen2_macos_objc_probe(swift-s2)
[swift-s2-probe] swift->objc OK: glaspen2_macos_objc_probe returned 1442
[swift-s2-probe] objc->swift OK: ping returned swift-pong-1442
```

- `1442 = 0x5A2`(S2 常量),证明调用真的进入 C 函数体,不是链接占位;
- 归档成员:`glaspen2.o InteropProbe.o interop_probe.o open_url.o`;
- `nm` 中 `SwiftInteropProbe` 以 ObjC 类符号
  (`_OBJC_CLASS_$__TtC20glaspen2_macos_swift17SwiftInteropProbe`)出现,
  `glaspen2_macos_open_url` 仍只有 1 处全局定义(S1 行为未变);
- debug 干净构建 22.7s、release 构建 23.4s 均通过;release 二进制同样跑通探针。

**未做**:启动路径 env 门控(`GLASPEN2_SWIFT_PROBE=1`)会拉起完整 GUI,
按"验证不启动 GUI"要求未实跑;其调用的是同一个已由 `--swift-probe` 验证的
`glaspen2_macos_swift_probe_selftest()`。

## 10. 相关

- `docs/swift-migration-plan.md`(阶段表/进度/每阶段验收模板)
- `docs/swift-s0-spike-report.md`(AppKit 菜单可行性 spike)
- `AGENTS.md`(仓库构建/踩坑/日志规范)
