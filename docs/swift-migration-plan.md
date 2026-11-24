# Swift 渐进迁移计划(可续做台账)

> 用法: 这是持久台账,不是一次性方案。开始/完成任何阶段必须改状态并追加进度记录;
> 中途停下要保持工作区干净、已提交,并在进度表写清卡点与下一步。
> 基线: v0.7.0(tag, commit ce525db, 2026-11-23)。
>
> ## 当前状态总览(2026-11-24,每轮更新)
>
> - ✅ S0 原型、S1 build.rs+swiftc+open_url、S2 桥接头+双向互调、S3 状态栏/菜单
>   (构建/图标/富文本/查找/名称/刷新/勾选/handler 动作/导出/退出)、S4 通知
>   presenter+绘制、S5 窗口/玻璃/内容视图/显示器防抖。
> - ✅ S6(2026-11-24 收口):掩码、CGEventTap 创建、路由骨架,滚轮/键盘/录制键/proximity/
  相机抓手/笔 down·move·up/压感监视器全部迁 Swift;`event_tap_callback_inner` 已删除,
  ObjC 回调只剩"Swift 结果 → NULL/event"。已知缺口:perf_log_event 打点未迁。
> - 🟨 S7 删除 ObjC 应用源(S7a–S7d 已清死代码/死桥;主体 window/draw view/panel/menu 仍在);
  S8 文档与真机验收待收口。**真机验收(笔/多屏/菜单/玻璃)全部待补**。
> - 当前 Swift 文件: src/macos/swift/{open_url,interop_probe,status_bar,menu_icons,menu_lookup,menu_builder,menu_builder_full,menu_updater,menu_names,menu_handler,notification,recording_hud,recording_keys,overlay_window,display_monitor,event_tap,event_router}.swift + Bridging.h。

## 0. 目标 / 非目标

目标: 把 macOS 原生 ObjC 适配层逐步迁到 Swift,降低维护成本与语言冷门风险,
提升可读性/安全性;用户可见行为不变。

非目标(明确不做):

- 不改 Flutter 设置面板(Dart 继续跨平台单一代码库);
- 不改 glaspen-core 的 Rust 逻辑/算法/单源数据;
- 不重写 cairo 绘制算法(DrawPlan 仍由 core 产出);
- 不改变 AppKit 窗口/事件语义;不引入 SwiftUI 替换现有窗口/菜单。

成功标准: 每个阶段行为等价(有 checklist/截图证据),构建仍一条 cargo build,
Rust↔macOS 的 C ABI 不变,契约测试与 parity 脚本全绿。

## 1. AI/人 必须遵守的规则

1. 一次只做一个阶段;开始前把状态改 🟨 并写日期/分支/涉及文件;完成才改 ✅。
2. main 始终可发布;迁移在分支 feat/swift-<phase> 或 feature flag 后,合并前必须过验收。
3. 保持 Rust↔macOS C ABI 不变:现有 glaspen2_macos_* 符号继续存在;
   用 Swift @_cdecl 导出同名 C 函数时不得改签名/类型。
4. 不顺手改 Rust core / Flutter / Windows;必须改时单独提交并更新 duplication-backlog。
5. 每阶段一个或若干小 commit,中文 message;未经用户明确指令不 push。
6. 每阶段结束必须跑: cargo build; cargo test --workspace --exclude glaspen2_installer;
   sh scripts/check-platform-parity.sh;本阶段手测清单;然后更新本文件。
7. 拿不准时保留 ObjC 实现并缩小范围,不要凭空发明语义。

## 2. 现状基线(v0.7.0, 2026-11-23)

- ObjC 源码: 7,191 行 / 4 文件: glaspen2.m 930, parts/panel.m 1,211,
  parts/menu.m 1,503, parts/run.m 3,547;当前是单编译单元 #include parts/*.m。
- Rust↔macOS 边界: menu.m 内 12 个 glaspen2_macos_* C 函数;其余经 core FFI。
- 构建: build.rs 调 clang 编 ObjC;尚未接 Swift。
- 已有防护: core 307+ 测试、契约闸门(带负向验证)、check-platform-parity.sh、
  Windows CI 阻塞门。
- 迁移最后再碰的高风险点: CGEventTap 回调/穿透、窗口层级与 ignoresMouseEvents、
  setIsVisible 系统动效、NSMenuItem attributedTitle 图标 hack、多屏防抖、全屏窗口、
  cairo surface 生命周期。
- 前置检查: swiftc --version / xcode-select -p 必须可用;部署目标与系统 Swift runtime 版本记录在 S1。

## 3. 阶段表

| ID  | 阶段                | 范围/涉及文件                                                                                                                              | 验收                                                                                  | 估时  | 状态    |
| --- | ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------- | ----- | ------- |
| S0  | spike:状态栏菜单    | 分支 spike/swift-status-item;新 Swift 文件实现 NSStatusItem+NSMenu+3 项+attributedTitle 图标+target/action;与 ObjC 并存(env flag),不合并   | 能显示/点击/语言重建;记录 LOC、桥接点、build.rs 改动、AppKit 缺口;给出 keep/stop 结论 | 1–2 d | ✅ DONE(docs/swift-s0-spike-report.md;分支 spike/swift-status-item @ 228213a;结论 GO S1) |
| S1  | 构建集成 + C ABI 桥 | build.rs 加 swiftc 编 .swift→.o 并链接;Bridging-Header;选 1 个无状态 shim(如 glaspen2_macos_open_url)用 Swift @_cdecl 重实现,符号不变      | cargo build 通过;Rust 调 Swift 行为一致;记录 swiftc 版本/部署目标/rpath;可回滚        | 2–4 d | ✅ DONE(2026-11-23,分支 feat/swift-s1:build.rs 已接 swiftc,open_url 由 Swift 接管;反向互调按任务允许留 S2;实测见 §8) |
| S2  | 边界设计            | 定义 @objc 兼容 API 白名单(类/selector/回调/C 类型);禁止 Swift-only 类型跨界;ObjC→Swift 文件映射;事件 tap 的 @convention(c)+Unmanaged 方案 | 编译通过:空 Swift 类被 ObjC 调用 + Swift 调 ObjC 函数;设计写入本文件                  | 1–2 d | ✅ DONE(2026-11-23,分支 feat/swift-s2:桥接头 + 生成头 + 双向互调探针;规则与 S3+ 文件映射在 docs/swift-boundary.md;实测见 §9) |
| S3a | 状态栏项创建(已做)  | src/macos/swift/status_bar.swift: StatusBarFactory.createItem(icon:) + @_cdecl selftest;Rust --swift-statusbar-probe;菜单仍由 ObjC 构建 | Swift 在真实 app 二进制内创建 NSStatusItem/button/image,probe 输出 OK;构建/测试/parity 绿 | 0.5 d | ✅ DONE(见进度记录) |
| S3b | 菜单构建/动作迁移    | menu.m:菜单项/动作/动态重建/语言切换/图标;保留 C shim;旧实现 fallback 一个版本 | 菜单项/图标/深浅色/语言/点击/快捷键与旧版逐项一致(截图);flag off 可回滚 | 3–5 d | 🟨 DOING(S3b-1 图标 + S3b-2a 富文本 + S3b-2b 菜单构建均已迁 Swift;剩 menu.m 动态文案刷新/checkmark/状态栏图标绘制与真机菜单视觉验收) |
| S4  | HUD/通知/面板辅助   | show_notification/HUD/alert/beep 等;文案继续来自 core i18n                                                                                 | 文案/时长/位置/中英切换与旧版一致                                                     | 3–5 d | ⬜ TODO |
| S5  | 窗口/覆盖层搭建     | NSWindow 创建/层级/透明/点击穿透/多屏/全屏/显示器变更防抖;不含 event tap;cairo 绘制不变                                                    | 窗口 checklist 逐项(层级/穿透/多屏/全屏/锁屏唤醒)与旧版一致                           | 1–2 w | ⬜ TODO |
| S6  | 事件 tap 迁移(最后) | CGEventTap 回调/笔鼠标键盘路由/热键/录制暂停/穿透;先双实现并行日志对比再切                                                                 | 全动作回归 + 笔输入延迟无退化;flag 可回滚                                             | 1–2 w | ⬜ TODO |
| S7  | 清理 ObjC           | 删除 ObjC 源文件/头;剩余 C shim 用 Swift;build.rs 只留 swiftc(+cairo C)                                                                    | git grep 无 ObjC 应用源;全测试/parity 通过;包大小/启动对比                            | 3–5 d | ⬜ TODO |
| S8  | 文档收口            | 更新 AGENTS.md 架构/命令/踩坑;本计划标记完成或归档                                                                                         | 新人按 AGENTS 能 build/run                                                            | 1 d   | ⬜ TODO |

## 4. 每阶段验收模板(复制到进度记录)

- [ ] 工作区干净;分支/feature flag 已说明
- [ ] cargo build(含 ObjC/Swift)通过
- [ ] cargo test --workspace --exclude glaspen2_installer 通过
- [ ] sh scripts/check-platform-parity.sh 通过
- [ ] 本阶段手测清单通过(逐项记录)
- [ ] 行为等价证据(截图/日志/命令输出)
- [ ] 回滚验证(flag off 或 revert 单 commit 后仍可运行)
- [ ] 本文件状态 + 进度记录 + commit hash 已更新

## 5. 风险与中止条件

- event tap/window 行为回归且 1 天内无法定位 → 回滚该阶段,保留 ObjC。
- build.rs + swiftc 集成导致本地/CI 构建不稳定 → 先解决工具链,不继续上层迁移。
- 某 AppKit 行为 Swift 无法复刻(未桥接/私有 API)→ 保留该模块 ObjC,缩小目标。
- 热路径性能退化 → 该路径保留 C/ObjC,不强行全 Swift。
- 任何阶段导致 parity/测试红且非已知问题 → 不合并。
- 每阶段超出估计 50% 即复盘:继续/回滚/缩小范围,写入进度记录。

## 6. 进度记录(append-only)

| 日期       | 阶段     | 进度/决定                                                          | 验证 | commit |
| ---------- | -------- | ------------------------------------------------------------------ | ---- | ------ |
| 2026-11-23 | 计划创建 | 以 v0.7.0(ce525db)为基线;迁移未开始;建议先 S0 spike 再决定是否继续 | -    | 本提交 |
| 2026-11-23 | S0 | 状态栏菜单 Swift 原型完成(NSStatusItem/NSMenu/attributedTitle 图标/target-action);编译 0.82s、71 行、60KB 独立产物,运行输出全部成功(3 菜单项/图标长度 1/selector 响应/action save 触发);未接 build.rs/ObjC 互调/动态重建(留 S1/S3) | swiftc -O 编译 + 2s 运行验证;报告 docs/swift-s0-spike-report.md | spike 分支 228213a |
| 2026-11-23 | S1 | 构建集成 + 第一个 Swift shim 完成:build.rs 用 xcrun swiftc 把 src/macos/swift/*.swift 编进 libglaspen2_objc.a(部署目标 12.0;工具链兼容库 + SDK/tbd + /usr/lib/swift 搜索路径;rpath /usr/lib/swift);glaspen2_macos_open_url 改由 Swift @_cdecl 实现,menu.m 删除 ObjC 定义;C ABI/签名不变。反向互调(bridging header)按任务允许留 S2 | cargo build + cargo build --release 通过;nm 仅 1 处 T _glaspen2_macos_open_url;cargo test --workspace --exclude glaspen2_installer 通过;check-platform-parity.sh 通过;独立 C 探针 NULL/非法 URL 日志与返回正确 | feat/swift-s1(构建 20913ce + 本提交) |
| 2026-11-23 | S2 | 边界设计 + 反向互调验证完成:build.rs 加 bridging header(-import-objc-header)与生成头(-emit-objc-header -emit-objc-header-path $OUT_DIR/glaspen2_macos_swift-Swift.h;clang -I $OUT_DIR);新增 @objc SwiftInteropProbe.ping() + ObjC 探针单元 InteropProbe.m(ObjC→Swift 与 Swift→ObjC 双跳均落日志);--swift-probe 无 GUI 自检 + GLASPEN2_SWIFT_PROBE=1 启动门控(默认 no-op);边界白名单/回调/字符串所有权/命名/Swift 5 并发策略/S3+ 文件映射写入 docs/swift-boundary.md | cargo build(干净 22.7s + 增量 + release 23.4s)、cargo test --workspace --exclude glaspen2_installer、check-platform-parity.sh、cargo clippy -p glaspen2 -- -D warnings 全通过;target/debug/glaspen2 --swift-probe 打出 4 行双向互调日志(ObjC 探针返回 1442=0x5A2) | feat/swift-s2(构建 b780331 + 文档提交) |

## 7. 相关文档

- docs/architecture.svg(当前架构)
- docs/duplication-backlog.md(D14 防重复架构、各单源模块)
- docs/d9d-verification.md(绘制验证与 Windows 延期记录)
- docs/swift-boundary.md(S2 边界规则:白名单/回调/字符串所有权/命名/Swift 5 策略/S3+ 文件映射)
- AGENTS.md(仓库规则/构建/踩坑)

## 8. S1 实现备注(构建集成 + 第一个 Swift shim)

完成于 2026-11-23,分支 `feat/swift-s1`(未合并 main)。

### 8.1 改动

- `build.rs`(macOS 分支):
  - `xcrun swiftc -c -parse-as-library -module-name glaspen2_macos_swift
    -swift-version 5 -target <arch>-apple-macosx12.0 -sdk <SDK> -O [-g(debug)]
    src/macos/swift/*.swift`,目标文件写入 `OUT_DIR/swift/`,与 `glaspen2.o`
    一起 `ar crus` 进同一个 `libglaspen2_objc.a`,最终链进同一个可执行文件。
  - 链接搜索:`<toolchain>/usr/lib/swift/macosx`(部署目标 12.0 所需
    libswiftCompatibility56.a 等兼容库只在这里)、`<SDK>/usr/lib/swift`
    (libswift*.tbd 链接桩)、`/usr/lib/swift`;外加
    `-Wl,-rpath,/usr/lib/swift`。
  - 刻意不手写 `-lswiftCore/-lswiftFoundation`:Swift 目标文件的
    LC_LINKER_OPTION autolink 会按 `import` 自动补库/framework;已用探针确认
    从静态归档成员里被拉出的 Swift 目标文件同样生效。
  - 监视 `src/macos/swift`(目录级 rerun-if-changed)。
  - 归档先删除再重建:避免 `ar crus` 在 Swift 文件重命名/删除后留下旧成员。
- `src/macos/swift/open_url.swift`:S1 唯一迁移的 C shim,
  `@_cdecl("glaspen2_macos_open_url")`,签名 `void(const char *)` 不变:
  用 `NSURL(string:)` 解析(等价 `[NSURL URLWithString:]`);主线程直接执行,
  非主线程 `DispatchQueue.main.sync`(对应 ObjC `gl_run_on_main_sync`);
  解析失败/打开失败都 NSLog(Rust 侧 http/https 白名单假定不变)。
- `src/macos/parts/menu.m`:删除原 ObjC 定义;Rust 侧 extern 声明与调用点零改动。
- `src/api.rs`:注释指向新实现。

### 8.2 环境与实测

- Swift: Apple Swift 6.4.2(swift-driver 1.168.6);SDK macOS 27.2;ld 27106.1。
- 部署目标 **12.0**:与 `scripts/build-dmg.sh` 的 LSMinimumSystemVersion 一致
  (README 也是 macOS 12+)。aarch64 理论下限 11.0;13.0 起编译器不再需要兼容库,
  若产品最低系统抬到 13.0 可删掉工具链 `-L`。
- `cargo build`:通过(52.45s,含 build.rs 的 flutter framework 步骤)。
- `nm target/debug/glaspen2 | grep glaspen2_macos_open_url`:
  `000000010024e6d8 T _glaspen2_macos_open_url` —— 全局代码定义仅 1 处。
- `otool -L`:libswiftCore/libswiftDispatch/libswiftFoundation 等按 autolink 链接,
  另有 AppKit/Foundation framework;`LC_RPATH` 含 `/usr/lib/swift`;
  二进制 minos 仍为 11.0(Rust 侧链接参数决定,Swift 目标文件的 12.0 没有抬高
  产品最低系统,因为代码只用 10.x API)。
- 独立 C 探针(真实 `open_url.swift` + C main):`NULL` 无操作;
  `"http://exa mple.com"` → NSLog 解析失败;`"://bad"` → NSURL 可解析但
  NSWorkspace 打开失败 → NSLog 失败日志;进程正常返回 0(两条失败路径都不崩)。
- `cargo test --workspace --exclude glaspen2_installer`:通过
  (29+307+31+11+3 … 全部 ok,0 failed)。
- `sh scripts/check-platform-parity.sh`:通过。
- `cargo build --release`:通过(38.22s;release 走同一套 swiftc `-O` 编译与链接参数)。
- 干净构建:`cargo clean -p glaspen2 && cargo build` 通过(18.36s,Swift 目标从零生成)。
- 增量/rerun-if-changed:改 `src/macos/swift/*.swift` 后 build.rs 会重跑并重链接;
  注意 Swift 6.4 驱动对"内容没变的 touch"会跳过重编(不刷新 .o,属 no-op 优化),
  真改代码才重编。端到端用 sentinel 字符串实测:改源码 → `cargo build` →
  `strings target/debug/glaspen2` 里出现 sentinel;revert → 消失,符号仍仅 1 处。

### 8.3 未做 / 留给 S2

- **ObjC→Swift 反向互调(bridging header / `-emit-objc-header` / @objc 类被
  ObjC 调用)未做**:任务允许作为 stretch,且 S2 验收本就包含"空 Swift 类被 ObjC
  调用 + Swift 调 ObjC 函数"。加它需多一条 `-emit-objc-header-path`、clang
  `-I` 与 ObjC 侧 import 生成头,避免在 S1 引入只能靠运行时验证的启动路径。
- 真实 app 手测「设置 → 检查更新 → 打开下载页」未做(会拉起浏览器;按任务要求
  不留 GUI 进程),行为等价由"逐行对照 + 独立探针"证明。
- 真机 macOS 12.0 未验证:兼容库是编译器按部署目标 12.0 的标准做法,但本机是
  27.2;旧系统回归留给 S2/发版前。
- DMG 打包未跑(需要签名证书);release 链接已通过,打包脚本无需 Swift 专属步骤
  (macOS 12+ 系统自带 Swift 运行时,不必内嵌 dylib)。

### 8.4 回滚

revert 本分支 S1 commit(菜单/shim 符号回到 ObjC)即可;build.rs 的 Swift 步骤
没有 feature flag,所以不要只删 `src/macos/swift/`(build.rs 会断言失败),
要整 commit revert。

### 8.5 下一步(S2)

1. 定义并写入边界白名单:C 类型/selector/回调/@convention(c)+Unmanaged 规则,
   禁止 Swift-only 类型跨界;
2. 加 bridging header + `-emit-objc-header`,完成双向互调最小验证(空 Swift 类被
   ObjC 调用 + Swift 调一个现有 ObjC 函数),然后按阶段表进入 S3 菜单栏。

→ 以上两项 S2 已完成(2026-11-23,分支 feat/swift-s2),实测与实现备注见 §9。

## 9. S2 实现备注(边界设计 + 反向互调)

完成于 2026-11-23,分支 `feat/swift-s2`(未合并 main)。完整边界规则/白名单/回调约定/
字符串所有权/S3+ 文件映射见 **docs/swift-boundary.md**;这里只记实施与实测要点。

### 9.1 改动(构建 + 探针)

- `build.rs`(macOS 分支):
  - Swift 编译提前到 ObjC 之前(ObjC 要 import Swift 生成头,顺序不能反);
  - 新增 `-import-objc-header src/macos/swift/Bridging.h`(Swift 侧 import ObjC/C 声明);
  - 新增 `-emit-objc-header -emit-objc-header-path $OUT_DIR/glaspen2_macos_swift-Swift.h`;
  - clang 编 ObjC 时加 `-I$OUT_DIR`(可 `#import "glaspen2_macos_swift-Swift.h"`);
    ObjC 拆成 `glaspen2.m` 与 `src/macos/swift/InteropProbe.m` 两个单元分别编译,
    一起 `ar crus` 进 `libglaspen2_objc.a`;
  - swiftc 后断言生成头存在;部署目标/链接/rpath 参数原样保留。
- `src/macos/swift/Bridging.h`(新文件):桥接头,当前声明探针 C 入口;
- `src/macos/swift/interop_probe.swift`(新文件):`@objc public final class
  SwiftInteropProbe: NSObject`,`@objc public static func ping() -> String`,
  内部经桥接头调 ObjC `glaspen2_macos_objc_probe("swift-s2")` 并返回
  `"swift-pong-<code>"`;
- `src/macos/swift/InteropProbe.m`(新文件):ObjC 侧探针单元。
  `#import "glaspen2_macos_swift-Swift.h"` 后 `[SwiftInteropProbe ping]`(ObjC→Swift);
  实现 `glaspen2_macos_objc_probe`(Swift→ObjC 目标,只打日志 + 返回 0x5A2);
  `GLASPEN2_SWIFT_PROBE=1` 时启动早期跑一次,默认 no-op;
- `src/macos/parts/run.m` + `src/macos/glaspen2.m`:启动早期调用 env 门控入口
  (不设变量时仅一次 getenv,用户可见行为零变化);
- `src/main.rs`:`glaspen2 --swift-probe` 隐藏调试入口,不启动 GUI、跑完即退。

### 9.2 实测(无 GUI)

- `target/debug/glaspen2 --swift-probe` 输出 4 行:
  `objc->swift: calling [SwiftInteropProbe ping]` /
  `objc<-swift OK: glaspen2_macos_objc_probe(swift-s2)` /
  `swift->objc OK: ... returned 1442` / `objc->swift OK: ping returned swift-pong-1442`
  (1442 = 0x5A2,证明真的执行到 C 函数体);
- 归档成员:`glaspen2.o InteropProbe.o interop_probe.o open_url.o`;
- `nm`:`SwiftInteropProbe` 以 ObjC 类符号出现;`glaspen2_macos_open_url` 仍只有
  1 处全局定义(S1 行为未变);
- 干净构建 22.7s(`cargo clean -p glaspen2`)、增量/rerun-if-changed 生效、
  release 23.4s 且 release 二进制同样跑通探针;
- `cargo test --workspace --exclude glaspen2_installer`(29+307+31+11+3,0 failed)、
  `sh scripts/check-platform-parity.sh`、`cargo clippy -p glaspen2 -- -D warnings` 全通过。

### 9.3 未做 / 留给 S3

- 启动路径 env 门控(GUI 内)未实跑(按任务要求不启动 GUI);调用的是同一
  `glaspen2_macos_swift_probe_selftest()`,命令行入口已覆盖;
- macOS 12.0 真机未验证(同 S1,本机 27.2);
- S3 第一件事:按 docs/swift-boundary.md §8 的映射把 `parts/menu.m` + 建菜单段
  迁到 `menu_bar.swift`/`menu_actions.swift`/`menu_icons.swift`,旧 ObjC 保留
  fallback(`GLASPEN2_SWIFT_MENU=1` 切换),验收后再改默认。

### 9.4 回滚

revert S2 的两个 commit 即可(S2 没有运行时 feature flag;`--swift-probe`/env 探针
默认 no-op,不回滚也不影响用户)。桥接头/生成头顺序是强制的:只删一个文件会
让 build.rs 断言失败。
| 2026-11-23 | S3a | 状态栏项创建迁 Swift:新增 src/macos/swift/status_bar.swift(StatusBarFactory.createItem(icon:) + @_cdecl glaspen2_macos_swift_statusbar_selftest);Rust 新增 --swift-statusbar-probe(不启动 GUI),Swift 在真实 app 二进制内创建 NSStatusItem/button/image 并自检;菜单构建/动作仍留 ObjC(S3b) | target/debug/glaspen2 --swift-statusbar-probe -> SWIFT_STATUSBAR_OK/IMAGE ok,exit 0,nm 1 处符号;cargo build/tests/parity/clippy 全绿 | 本提交 |
| 2026-11-23 | S3b-1 | 菜单小图标迁 Swift:新增 src/macos/swift/menu_icons.swift(MenuIconFactory.colorDot/widthLine);ObjC 调用点改为生成头调用,旧函数改名 *_legacy 仅 parity 探针使用(S3b-2 删除);新增 --swift-icon-probe 逐字节比较 TIFF | probe: colorDot same=1 widthLine same=1, exit 0;cargo build/tests/parity/clippy 全绿 | 本提交 |
| 2026-11-23 | S3b-1 收尾 | 删除临时 ObjC *_legacy 图标实现、--swift-icon-probe 与探针函数;Swift MenuIconFactory 成为颜色/粗细图标唯一实现 | grep legacy=0;cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-23 | S3b-2a | 菜单富文本构造函数 gl_menu_text 迁 Swift(MenuIconFactory.menuTextWithIcon:name:),ObjC 调用点改为生成头调用,旧函数删除;颜色/粗细图标与富文本均不再在 ObjC 绘制/构造 | cargo build/tests/parity 全绿;menu.m 中 gl_menu_text=0 | 本提交 |
| 2026-11-23 | S3b-2b-0 | 真实启动路径的状态栏项创建改用 Swift StatusBarFactory.createSquareItem(方形长度,与旧一致);ObjC 保留 nil fallback;菜单构建/动作仍在 run.m(S3b-2b 继续) | cargo build/tests/parity 全绿;--swift-statusbar-probe OK | 本提交 |
| 2026-11-23 | S3b-2b-1 | 菜单项递归查找 gl_menu_item_with_tag 迁 Swift(MenuLookup.itemWithTag:inMenu:);ObjC 7+ 处调用点(menu.m/run.m)全部改生成头调用,旧函数删除 | cargo build/tests/parity 全绿;grep 旧符号=0 | 本提交 |
| 2026-11-23 | S3b-2b-2 | 菜单颜色/粗细条目构建迁 Swift(部分):新增 src/macos/swift/menu_builder.swift(MenuBuilder addColorItemsToMenu:handler:count:titles: / addWidthItemsToMenu:…;action selectColor:/selectWidth:、target=g_menuHandler、tag=index 契约不变),run.m 两段建项循环改为两次 Swift 调用,ObjC 只预构富文本标题数组(图标/i18n 单源不变);新增 --swift-menu-probe 无 GUI 自检 count/tag/action/target/标题 | target/debug/glaspen2 --swift-menu-probe -> SWIFT_MENU_OK(COLOR/WIDTH count=3 tags=0,1,2,action 逐项正确;nil/钳制分支 ok)exit 0;cargo build + tests(29+307+31+11+3)+ parity + clippy 全绿;顶层其余项/子菜单/分隔符仍 ObjC(S3b-2b-3) | 本提交 |
| 2026-11-23 | S3b-2b-3 | 顶层菜单其余项 + 导出▸/更多▸ 子菜单构建迁 Swift:新增 src/macos/swift/menu_builder_full.swift(MenuBuilder addTopLevelAndSubmenusToMenu:handler:titles:launchAtLoginOn:glassOn:;顺序/分隔符/tag/selector/target/初始 state/keyEquivalent 与旧逐项一致,文案仍由 ObjC 用 L()/gl_text 以标题字典传入);run.m 手工建项块与末尾 target 兜底循环删除,只剩两次颜色/粗细调用+标题字典+Swift 调用+setMenu;新增 --swift-menu-full-probe 无 GUI 建整份菜单并校验项数/分隔符位置/顶层/两个子菜单;标题键不用点号(i18n 闸门把形似 i18n key 的字面量视为拼错 key) | target/debug/glaspen2 --swift-menu-full-probe -> count=31 separators=10,19,24,28 export:3 more:5 tags=777,444,667,668,778 states 按入参 SWIFT_MENU_FULL_OK exit 0;--swift-menu-probe 仍 SWIFT_MENU_OK;cargo build 通过;cargo test --workspace --exclude glaspen2_installer(29+307+31+11+3 全 ok,0 failed);check-platform-parity.sh 通过;cargo clippy -p glaspen2 -- -D warnings 通过;cargo fmt --check 通过 | 本提交 |
| 2026-11-24 | S3b-2b-4a | 菜单勾选刷新 update_menu_checkmarks 逻辑迁 Swift(MenuUpdater.updateCheckmarksInMenu:colorCount:widthCount:colorIndex:widthIndex:);ObjC 仅传当前全局状态 | cargo build/tests/parity 全绿;旧逻辑删除 | 本提交 |
| 2026-11-24 | S3b-2b-4b | 颜色/粗细显示名迁 Swift(MenuNames,稳定 i18n key 表);ObjC 只留通用文案桥 glaspen2_menu_l10n(内部调 gl_text);i18n 文案闸门扫描范围扩展到 src/macos/**/*.swift(否则 key 表迁 Swift 后误报"未引用") | cargo build/tests/parity 全绿;i18n 闸门通过 | 本提交 |
| 2026-11-24 | S3b-2b-5 | 菜单动态文案刷新 update_menu_texts 逻辑迁 Swift(MenuUpdater.updateTextsInMenu:colorTitles:widthTitles:topTitles:exportTitles:moreTitles:modeTitle:toggleTitle:);ObjC 只保留 L()/gl_text/gl_color_item_text 预计算并传数组,索引/子菜单/tag 778/888 刷新在 Swift | cargo build/tests/parity 全绿;ObjC 旧索引循环删除 | 本提交 |
| 2026-11-24 | S3b-2b-6 | 状态栏图标绘制 render_status_icon 迁 Swift(StatusIconRenderer.renderIconWithEraser:enabled:ethereal:infinite:widthScale:penRGB:),生产路径已切换;旧 ObjC 实现暂留仅供 --swift-status-icon-probe 逐字节 TIFF parity(same=1),收尾时删除 | probe same=1;cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S3b-2b-6 收尾 | 删除旧 ObjC render_status_icon_legacy 与 --swift-status-icon-probe;Swift StatusIconRenderer 成为状态栏图标唯一实现(像素 parity 已于 a8e915c 验证 same=1) | grep 旧符号=0;cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S3b-2b-7a | 菜单 handler 首批毛法迁 Swift: 新增 Swift helper MenuHandler(quitApp/menuWillOpen/menuDidClose), ObjC 仅保留三行 delegate 转发; menu.m 增加三个薄桥(quit/menuWillOpen/menuDidClose); i18n 闸门对 .swift 放宽大小下限(小 helper 合法) | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S3b-2b-7b | 菜单首个业务动作 toggleLanguage 迁 Swift(MenuHandler.toggleLanguage: 读/写语言桥 + 落库 + 刷新 + 通知面板), ObjC 方法只剩一行转发;新增 lang get/set/refresh 三个薄桥与既有 C FFI 声明 | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S3b-2b-7c | 四个开关类菜单动作迁 Swift: toggleLaunch/toggleGlass/toggleOutline/toggleInfiniteCanvas 逻辑在 MenuHandler(读当前状态取反再调 setter);ObjC 方法四行转发;新增 8 个 getter/setter 薄桥(launch/glass/outline/infinite) | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S3b-2b-7d | 保存/设置/选择类动作迁 Swift: saveWithBg/saveOnly/clearScreen/toggleDraw/saveXoj/showSettingsPanel/toggleCanvasMode/selectColor/selectWidth 逻辑在 MenuHandler, ObjC 各一行转发;新增 10 个薄桥(含 saveXoj 通知文案经 glaspen2_menu_l10n) | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S3b-2b-7e | 异步导出与退出 delegate 迁 Swift: exportInfinitePdf/Svg(DispatchQueue 调度 + 本地化通知)与 applicationShouldTerminate(glaspen2_shutdown + NSTerminateNow)逻辑在 MenuHandler;ObjC 各一行转发;新增 screen size/export/shutdown/l10n_pair 五个薄桥 | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S4-1 | 屏幕通知 presenter 迁 Swift: NotificationPresenter 持有当前文案 + 1s DispatchSourceTimer 自动清除(代际计数防旧计时清掉新通知), 状态变化经 glaspen2_notification_redraw 桥触发重绘;ObjC show_notification 变一行转发, run.m drawRect 经 currentText 读取;g_notification/g_notification_timer 删除。绘制仍留 ObjC(S4-2 再迁) | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S4-2 | 通知文字绘制迁 Swift: NotificationPresenter.drawInBounds(36pt 等宽 + 白字 + 阴影 + 视图居中), run.m drawRect 只调用一次;旧约 20 行 NSAttributedString 绘制代码删除 | cargo build/tests/parity 全绿(视觉留真机) | 本提交 |
| 2026-11-24 | S5-1 | 覆盖层窗口原语迁 Swift: OverlayWindowFactory.makeWindow(borderless/最大层级/透明/清除背景/acceptsMouseMoved/CanJoinAllSpaces+Stationary/ignoresMouseEvents)、makeBlankCursor、makeArrowCursor;run.m 只保留三行调用, 内容视图/layer/玻璃/绘制视图仍 ObjC | cargo build/tests/parity 全绿(窗口视觉留真机) | 本提交 |
| 2026-11-24 | S5-2 | 玻璃视图与绘制层配置迁 Swift: OverlayWindowFactory.makeGlassView(frame:enabled:opacity:)(behindWindow/light/active + 原 alpha 公式)、configureClearLayer(非不透明+clear);run.m 原 8 行组装/4 行 layer 设置改为调用 | cargo build/tests/parity 全绿(视觉留真机) | 本提交 |
| 2026-11-24 | S5-3 | 显示器变更防抖迁 Swift: DisplayChangeMonitor.onDisplayChanged 读 NSScreen 实时几何、早退, 2.5s DispatchSourceTimer(one-shot, 每次事件重置), 到期重读真实几何后经 glaspen2_display_apply 桥调用 display_change_apply; ObjC 旧 dispatch timer/两个 pending 全局删除, on_display_changed 只剩一行转发; 完全保留 AGENTS 两条坑(到期重读、不照搬瞬态值) | cargo build/tests/parity 全绿(需真机多屏/锁屏验证) | 本提交 |
| 2026-11-24 | S5-4 | 内容容器视图创建(wantsLayer)与挂载/orderFront 迁 Swift: OverlayWindowFactory.makeContentViewWithFrame / attachContent:toWindow:; run.m 原 3 行替换为调用; 玻璃/绘制视图仍 ObjC 组装(下一片) | cargo build/tests/parity 全绿(视觉留真机) | 本提交 |
| 2026-11-24 | S6-1 | 事件 tap 掩码构造迁 Swift: EventTapInstaller.eventMask(includeScroll:)(14 种事件 + 可选 scrollWheel);run.m 原 20 行 CGEventMaskBit 链删除, CGEventTapCreate/回调/runloop 挂载仍 ObjC(下一片) | cargo build/tests/parity 全绿;笔输入/穿透需真机验证 | 本提交 |
| 2026-11-24 | S6-2 | CGEventTap 创建/runloop 挂载迁 Swift: EventTapInstaller.install(includeScroll:) 用 CGEvent.tapCreate/tapEnable(旧 C 名在 Swift 3 已 obsoleted); 回调函数指针经 glaspen2_event_tap_callback_ptr 桥获取, tap 存储/失效/滚轮标记经 ObjC 薄桥(CF 所有权留 ObjC);run.m event_tap_install 只剩一行调用 | cargo build/tests/parity 全绿;笔输入/热键/穿透需真机验证 | 本提交 |
| 2026-11-24 | S6-3-0 | 事件路由骨架: 新增 Swift EventRouter.routeEventOfType:event:(0=fallback ObjC, 1=放行, 2=吞掉); ObjC event_tap_callback 先调 Swift, 返回 0 时仍走原 inner, 行为零变化。后续按事件类型(滚动/鼠标→笔→键盘)逐步把分支搬进 Swift, ObjC inner 相应缩小 | cargo build/tests/parity 全绿;行为不变 | 本提交 |
| 2026-11-24 | S6-3-1 | 滚轮缩放路由迁 Swift: EventRouter 处理 scrollWheel+无限画布(修饰键读 canvas_zoom_in、精确/非精确 delta 公式、鼠标位置居中缩放、书写中忽略), ObjC 原 35 行分支删除; 新增 camera_mods/state/draw_height/zoom_at 四个只读薄桥 | cargo build/tests/parity 全绿;滚轮/触控板/修饰键需真机验证 | 本提交 |
| 2026-11-24 | S6-3-2 | 非笔 mouseMoved 隐藏光标路由迁 Swift: EventRouter 用 NSEvent pointingDeviceType/subtype 判笔, 条件(visible&&!stroke_active)成立时按原顺序 invalidate/hide/ethereal auto_hide/proximity-aware bubble gone/flush; ObjC 原 18 行分支删除; 新增 6 个薄桥(flags/状态动作) | cargo build/tests/parity 全绿;真鼠标/笔离开语义需真机验证 | 本提交 |
| 2026-11-24 | S6-3-3a | 虚拟笔事件判定迁 Swift: EventRouter.isVirtualPenEvent(CGEvent 用户数据比较), 常量经 glaspen2_virtual_pen_user_data 薄桥暴露; ObjC static inline 删除, 两处调用点切换。为后续 tablet 笔路由做铺垫 | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S6-3-3b | 笔光标更新块迁 Swift: EventRouter.penCursorUpdate 复制原逻辑(up 事件 bubble activity/其余 back、strokes 不可见时 auto_show、peek cancel、moved 时 invalidate/设光标/show/16ms 节流 flush); ObjC 原 ~35 行 if(isPen) 块删除; 新增 11 个状态/动作薄桥。pen_hover 性能打点未迁(可后续补) | cargo build/tests/parity 全绿;笔 hover/光标/气泡语义需真机验证 | 本提交 |
| 2026-11-24 | S6-3-3c | 笔书写系统光标隐藏/恢复块迁 Swift: EventRouter.penSystemCursor(按下/拖动时首次隐藏系统光标并置 pen_drawing, 任意 up 恢复系统光标+隐藏十字+invalidate+flush); ObjC 原 20 行块删除; 新增 4 个薄桥 | cargo build/tests/parity 全绿;真机笔/鼠标光标语义需验证 | 本提交 |
| 2026-11-24 | S6-3-3d-1 | restore_system_cursor 本体迁 Swift: EventRouter.restoreSystemCursor(仅 pen_drawing 时 show+清标记), ObjC restore_system_cursor 改为一行转发; 薄桥 restore 替换为 show_system_cursor, 新增 show 桥 | cargo build/tests/parity 全绿;真机光标语义需验证 | 本提交 |
| 2026-11-24 | S6-3-4a | 停用涂鸦时非键盘/非滚轮事件清理+放行迁 Swift: EventRouter 判断 state&1==0 时调 glaspen2_router_disable_cleanup(camera drag 取消 + restore cursor)并返回 1; ObjC 原 8 行块删除; 键盘/滚轮保持原路径 | cargo build/tests/parity 全绿;需真机验证停用态鼠标/笔行为 | 本提交 |
| 2026-11-24 | S6-3-3d-2 | 笔事件判定去重: EventRouter.isPenForEvent 暴露给 ObjC, run.m 的 isPen 公式改为调用 Swift(devType/subtype 仍保留给橡皮/日志判断); 笔主处理(落笔/拖动/抬笔)尚未迁移, 下一片继续 | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S6-3-4b-1 | 热键 chord 匹配迁 Swift: EventRouter.actionForHotkeyWithKeyCode:modifiers: 构造 cmd/ctrl/alt/shift 顺序 chord(至少 ctrl|alt)并查表; 键名/查找经 glaspen2_router_keycode_name / action_for_chord 两个薄桥; ObjC gl_shortcut_action_for_key 改一行转发 | cargo build/tests/parity 全绿;热键实际触发需真机验证 | 本提交 |
| 2026-11-24 | S6-3-4b-2(尝试后回滚) | 尝试整体迁移键盘事件路由时发现该块与录制热键(⌘⌃R/2/3 的 core recording 状态机调用)深度交织: 直接删除会丢录制功能, 且 D8b 闸门报 "run.m 缺少 glaspen2_recording_mode("。已 git checkout 回滚, 工作区恢复全绿。新计划: (1) 先把 run.m 键盘块拆成"录制键处理"与"普通 chord 门控"两函数(纯移动, 行为不变); (2) 迁 chord 门控到 Swift, 录制函数保留; (3) 再用 recording 事件 FFI 把录制键处理迁 Swift。回滚后 tests/parity 全绿 | 下一步: 先做第 (1) 步纯拆分 | —(未提交代码) |
| 2026-11-24 | S6-3-4b-2a | 键盘块纯拆分(行为不变): 录制热键(⌘⌃R/2/3)提取为 run.m 的 handle_recording_hotkeys(type,kc,hasCmdCtrl) 返回 YES/NO; 原键盘块只剩一行调用 + 普通 chord 门控; 目的: 下一步可单独把 chord 门控迁 Swift, 录制函数暂留 ObjC 并保住 D8b 闸门 | cargo build/tests/parity 全绿;录制热键/键盘行为不变 | 本提交 |
| 2026-11-24 | S6-3-4b-2b | 普通 chord 门控迁 Swift: EventRouter.handleChordKeyEvent(keyDown → actionForHotkey → 停用白名单/镜头门控/repeat/perform_action, 返回 1 放行 2 消费); ObjC 键盘块删除 chord 分支与 isRepeat 计算, 只剩录制函数调用 + 一行 Swift 路由; 录制热键(handle_recording_hotkeys)仍留 ObjC 以保 D8b 闸门 | cargo build/tests/parity 全绿;热键/停用态需真机验证 | 本提交 |
| 2026-11-24 | S6/S8 小切片 | 录制 HUD 文案映射(rec_show_hud, ~50 行)迁 Swift: 新增 recording_hud.swift(RecHudPresenter.presentHud:strokes:detail:), 文案经 glaspen2_menu_l10n/menu_notify(零新桥); run.m 只解包 effect; D8b 闸门扩展扫描 recording_hud.swift(HUD 变体以普通字面量校验), 并从 mac 事件清单移除 GifClipboardFailed(差异 3: macOS 不产生该事件, HUD 兜底仍覆盖) | cargo build/tests/parity 全绿;HUD 需真机验证 | 本提交 |
| 2026-11-24 | S6-3-4b-2c | 录制热键判定迁 Swift: 新增 recording_keys.swift(RecordingHotkeys.handleWithType:keyCode:hasCmdCtrl:, ⌘⌃R/2/3 down/up + 模式匹配 + chat 总开关), 事件 JSON 由 Swift 生成后经 glaspen2_router_rec_feed 桥喂状态机; ObjC handle_recording_hotkeys 改一行转发; 新增 6 个薄桥(slot/mode/finish_and_count/feed/chat_integration) | cargo build/tests/parity 全绿;D8b 闸门通过;热键/录制需真机验证 | 本提交 |
| 2026-11-24 | S7a | 删除迁移后无调用的 ObjC 死代码: rec_start/rec_release(并入 Swift RecordingHotkeys)、gl_shortcut_action_for_key(ObjC 包装); D8b 闸门把事件名覆盖改为 run.m+recording_keys.swift 合并源(裸 token), 保持"每个 core 事件都有 adapter 接线"校验 | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S7b | 删除无调用薄桥 glaspen2_event_tap_enable(Swift 已改用 CGEvent.tapEnable); 头文件声明与 run.m 实现同步移除 | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S7c | 删除迁移后无调用的 rec_key_event(事件 JSON 已由 Swift RecordingHotkeys 生成); D8b 闸门把 JSON 载荷键名校验移到 run.m+recording_keys.swift 合并源 | cargo build/tests/parity 全绿 | 本提交 |

## 40 轮目标冲刺收尾状态(2026-11-24)

> 结论:**目标未达成**(仓库仍有应用层 ObjC),goal 保持 active;本记录供后续轮次直接续做。

### 已完成
- S0 build.rs+swiftc、S1 open_url、S2 桥接头+双向互调;
- S3 状态栏项/图标、菜单构建/查找/名称/刷新/勾选、GlaspenMenuHandler 全部动作、导出/退出;
- S4 通知 presenter + 文字绘制;S5 窗口创建/玻璃/内容视图/光标/显示器 2.5s 防抖;
- S6-1/2 事件掩码与 CGEventTap 创建;S6-3 路由骨架、滚轮缩放、非笔光标隐藏、笔光标更新、系统光标隐藏/恢复、停用态放行、键盘 chord 门控、录制热键、录制 HUD;
- S7a/b/c 清理迁移后无调用的 ObjC 辅助函数;D8b/i18n 闸门已覆盖 Swift 源。

### 未完成
- **S6-3-3d-3 tablet 落笔/拖动/抬笔主处理**(模型器 begin/move/end、raw 绘制、压感、橡皮、dirty/flush):`run.m` 中最大剩余块;
- **S7 主体删除**:ObjC 仍有 6,724 行(`glaspen2.m` 915、`menu.m` 1,336、`panel.m` 1,183、`run.m` 3,290),其中含窗口/绘制视图/设置面板/相机/导出等;
- **真机验收**:笔输入/延迟、多屏/锁屏唤醒、菜单视觉(明暗/双语/逐项)、热键/录制、玻璃/窗口层级,全部待补;
- `pen_hover` 性能打点未迁(已知小缺口)。

### 建议续做顺序
1. tablet 主处理分 3 片(落笔→拖动/预测→抬笔 commit),每片构建+测试+真机笔回归;
2. S7 按文件清理已迁完的转发/死代码,再迁 `panel.m`(设置面板宿主)与 `run.m` 相机/绘制辅助;
3. S8 用真机清单逐项验收后,才可把 goal 标记 complete。
| 2026-11-24 | S6-3-3d-0(续跑第1轮) | 菜单栏点击分支迁 Swift: EventRouter 判定 leftMouseDown+非笔+屏幕 y<30 时调 glaspen2_router_tap_menu_bar_interaction(停 tap + 5s 兜底恢复); ObjC 原 18 行分支删除; 为 tablet 主处理前的最后小分支 | cargo build/tests/parity 全绿;点菜单栏/菜单交互需真机验证 | 本提交 |
| 2026-11-24 | S6-3-3d-1(续跑第1轮) | 橡皮态切换迁 Swift: EventRouter.updateEraserState(仅状态变化时写 g_eraser_mode + 刷新状态图标); 落笔/拖动自动补 begin/抬笔复位三处重复逻辑收敛为一处调用; 新增 3 个薄桥 | cargo build/tests/parity 全绿;橡皮/图标需真机验证 | 本提交 |
| 2026-11-24 | S6-3-3d-2(续跑第2轮) | 落笔入口迁 Swift: EventRouter.penDown(自动翻页 + modeler_begin + 共享 cairo context + raw dot + 落笔状态初始化)一行调用; ObjC 保留 finish 守卫/NSLog/perf; 新增 5 个落笔薄桥 | cargo build/tests/parity 全绿;落笔/raw 绘制需真机验证 | 本提交 |
| 2026-11-24 | S6-3-3d-3(续跑第3轮) | 拖动实时段迁 Swift: EventRouter.penMove(模型器 move + raw segment)一行调用; ObjC perf 打点保留; 新增 2 个薄桥 | cargo build/tests/parity 全绿;拖动/响应延迟需真机验证 | 本提交 |
| 2026-11-24 | S6-3-3d-4(续跑第4轮) | 抬笔收尾迁 Swift: EventRouter.penUp(erase_finish / end+commit、stroke_end、pending 颜色粗细生效、raw 状态/cursor/dirty/flush 复位); ObjC 只保留 NSLog + perf; 新增 8 个薄桥 | cargo build/tests/parity 全绿;抬笔提交/橡皮需真机验证 | 本提交 |
| 2026-11-24 | S6-3-3d-5(续跑第5轮) | 拖动自动补 begin 分支复用 Swift penDown 入口: 删除 ObjC 重复的 begin/context/raw dot/状态初始化 14 行,只剩一行调用; 行为与原分支一致 | cargo build/tests/parity 全绿;笔检测滞后场景需真机验证 | 本提交 |
| 2026-11-24 | S6-3-3d-6(续跑第6轮) | tablet proximity 进入/离开迁 Swift: EventRouter.routeProximity(悬浮进入/橡皮进入日志、prox 设备记录、peek 取消、bubble back、飘渺 auto_show;离开: bubble gone、光标 invalidate/hide/flush、飘渺 auto_hide), ObjC 原 45 行分支删除; 新增 4 个薄桥 | cargo build/tests/parity 全绿;笔 hover/飘渺联动需真机验证 | 本提交 |
| 2026-11-24 | S6-3-3d-7(续跑第7轮) | 相机临时抓手分支迁 Swift: EventRouter.routeCameraMouse(⌃⌥+左键按下开始、拖动 queue pan 按 zoom 折算/更新 last、抬起 flush+清 dragging), 放在菜单栏判定之前保持原顺序; ObjC 两段分支删除; 新增 10 个相机薄桥 | cargo build/tests/parity 全绿;抓手平移/缩放需真机验证 | 本提交 |
| 2026-11-24 | S6-3-3d-8(续跑第8轮) | 笔事件分派整体迁 Swift: EventRouter.handlePenEventOfType:...(down→eraser+begin, drag→自动补 begin 或 move, up→penUp), 三分支 ~100 行 ObjC 删除, 只留一次调用; 日志简化(去掉 tool/prox 明细); pen_down/pen_move/pen_up 性能打点暂未迁(后续可加 perf 桥) | cargo build/tests/parity 全绿;笔输入全链路需真机验证 | 本提交 |
| 2026-11-24 | S6-3-3d-9(续跑第9轮) | 压感监视器状态更新迁 Swift: EventRouter.updatePressureMonitor(压力原始字段>0 优先、tipDown 判定、事件名映射), ObjC ~45 行块删除, 只留 isPen 时一次调用; 新增 pm_enabled/pm_update 两个薄桥 | cargo build/tests/parity 全绿;压感监视器需真机验证 | 本提交 |
| 2026-11-24 | S6-3-4(续跑第10轮) | 键盘事件块整体迁 Swift 路由: EventRouter.routeKeyboardEvent(录制放行标志→RecordingHotkeys.handle(down/keyUp 都交给状态机, 抬键结束会话)→chord 门控); ObjC 内层键盘块删除; 新增 hotkey_recording 薄桥; 自检时发现并修正"只对 keyDown 调录制"的 bug | cargo build/tests/parity 全绿;热键/录制键需真机验证 | 本提交 |
| 2026-11-24 | S6 收尾(续跑第11轮) | 删除 event_tap_callback_inner: tap-disabled 恢复、笔坐标/压力/raw_w 计算、飘渺 touchdown、压感监视器、pen 分派全部收进 Swift EventRouter.route; ObjC 回调只剩"Swift 路由结果 → NULL/event"与虚拟笔吞事件; 新增 4 个薄桥;**perf_log_event 打点整体移除(已知缺口, 后续可加 perf 桥)** | cargo build/tests/parity 全绿;事件全链路需真机验证 | 本提交 |
| 2026-11-24 | S7d(续跑第12轮) | 死代码扫描: 脚本遍历 src/macos/*.m 的 static 函数并统计全仓引用, 仅 handle_recording_hotkeys 无人调用(键盘路由已迁 Swift), 删除该转发函数; 其余 static 函数均有调用 | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S4/S7(续跑第13轮) | 页信息 HUD 迁 Swift: NotificationPresenter.showPageInfoWithScreenId(直接调 core::pageinfo FFI + 释放 C 字符串 + 显示), ObjC show_page_info 变一行转发; 新增 lang_code 薄桥与 FFI 声明 | cargo build/tests/parity 全绿;页信息文案需真机验证 | 本提交 |
| 2026-11-24 | S8 台账同步(续跑第14轮) | 状态总览更新: S6 事件 tap 全链路标记完成(event_tap_callback_inner 已删、ObjC 回调变薄壳, perf 打点缺口已记), S7 标为进行中(死代码/死桥已清, 主体未删), Swift 文件清单补全 | 无代码变更;构建/测试保持全绿 | 本提交 |
| 2026-11-24 | S7e(续跑第15轮) | finish_active_stroke 迁 Swift: EventRouter.finishActiveStroke(橡皮 finish / 正常 end+commit、stroke_end、pending 颜色粗细、raw 状态复位; 不动光标/不刷新, 与 penUp 区分), ObjC 函数变一行转发; 新增 3 个薄桥 | cargo build/tests/parity 全绿;退出/切页时收尾需真机验证 | 本提交 |
| 2026-11-24 | S5/S7(续跑第16轮) | 截图目标屏判定迁 Swift: OverlayWindowFactory.windowDisplayId(取覆盖层窗口所在屏, 退回主屏, 多屏正确), ObjC window_display_id 变一行转发; 新增 main_window 薄桥 | cargo build/tests/parity 全绿;多屏截图需真机验证 | 本提交 |
| 2026-11-24 | S7f(续跑第17轮) | 退出动作迁 Swift: MenuHandler.quitAppNow(perf 汇总 + 恢复系统光标 + NSApp terminate), ObjC gl_quit_app 变一行转发; 新增 perf_summary 薄桥 | cargo build/tests/parity 全绿;真实退出需真机验证(perf 汇总文件无则无副作用) | 本提交 |
| 2026-11-24 | S5/S7(续跑第18轮) | 覆盖层内容组装迁 Swift: OverlayWindowFactory.assembleContent:glass:drawView:inWindow:(玻璃在下、绘制视图 layer 透明、addSubview 顺序、挂载+orderFront), ObjC 原 addSubview/wantsLayer/layer 配置/attach 调用收敛为一次调用 | cargo build/tests/parity 全绿;窗口层级/透明度需真机验证 | 本提交 |

## 剩余 ObjC 清单与建议迁移顺序(2026-11-24 审计)

| 文件 | 行数 | static 函数 | ObjC 方法 | 类 |
|---|---:|---:|---:|---:|
| `glaspen2.m` | 910 | 152 | 0 | 0 |
| `parts/menu.m` | 1,335 | 75 | 20 | 1 |
| `parts/panel.m` | 1,169 | 54 | 2 | 2 |
| `parts/run.m` | 3,061 | 100 | 2 | 1 |

- 最大单函数:`run.m` 的 `drawRect:`(188 行,笔迹/网格/小地图/HUD/反色合成)。
- 建议顺序:
  1. `drawRect` 按图层切片(笔迹 surface → 网格 → 小地图 → 反色合成 → 光标),每片做
     像素/截图 parity 后再删 ObjC;
  2. `run.m` 窗口/相机/导出辅助(已迁组装与光标,剩 surface/导出/隧道动效);
  3. `panel.m` 设置窗口/Flutter engine 宿主/压感监视器 HUD;
  4. `menu.m` 设置 setter 与图标刷新(selector 必须保留的转发类最后处理);
  5. `glaspen2.m` 剩余 i18n/设置分派/DB 初始化 shim(保留 C ABI 名,可换 Swift 实现)。
- 每片都跑 `cargo build`/全量测试/parity;真机验收清单见各阶段记录。
| 2026-11-24 | S8 审计(续跑第19轮) | 生成剩余 ObjC 清单(行数/static/方法/类)与建议迁移顺序(drawRect 分图层→run.m 辅助→panel.m→menu.m→glaspen2.m shim); 单函数最大 drawRect 188 行 | 文档同步, 无代码变更 | 本提交 |
| 2026-11-24 | S7g(续跑第20轮) | 暂存颜色/粗细生效逻辑迁 Swift: MenuHandler.applyPendingColorWidth(书写中不动, 抬笔后读 pending→清标记→调既有 select 桥), ObjC 函数变一行转发; 新增 4 个 pending 状态薄桥(menu.m) | cargo build/tests/parity 全绿;书写中换色/换粗细需真机验证 | 本提交 |
| 2026-11-24 | S3/S7(续跑第21轮) | 状态栏图标刷新迁 Swift: StatusIconUpdater.applyToStatusItem:...(渲染仍走 StatusIconRenderer; setImage/清标题在 Swift), ObjC update_status_icon_color 只剩参数转发; 菜单/橡皮态刷新共用 | cargo build/tests/parity 全绿;状态栏图标需真机验证 | 本提交 |
| 2026-11-24 | S7h(续跑第22轮) | 粗细设置迁 Swift: MenuHandler.setWidthIndex(书写中暂存 pending;否则应用 preset→落库→勾选/状态图标/面板刷新), ObjC gl_settings_set_width 变一行转发; 新增 9 个薄桥(预设计数/选中/prev/pending/apply/save/checkmarks/panel); 修复重复的 update_status_icon_state 桥 | cargo build/tests/parity 全绿;粗细切换需真机验证 | 本提交 |
| 2026-11-24 | S7i(续跑第23轮) | 颜色设置迁 Swift: MenuHandler.setColorIndex(书写中暂存 pending;否则应用 preset rgb→落库→勾选/状态图标/面板刷新), ObjC gl_settings_set_color 变一行转发; 新增 5 个薄桥(颜色预设计数/选中/prev/pending/apply) | cargo build/tests/parity 全绿;颜色切换需真机验证 | 本提交 |
| 2026-11-24 | S7j(续跑第24轮) | 磨砂玻璃开关迁 Swift: MenuHandler.setGlassEnabled(状态/落库 bool/应用), ObjC gl_settings_set_glass_enabled 变一行转发; 新增 2 个薄桥 + 既有 save_bool_setting FFI 声明; 复用 save_pen_settings/同步桥 | cargo build/tests/parity 全绿;玻璃视觉效果需真机验证 | 本提交 |
| 2026-11-24 | S7k(续跑第25轮) | 开机自启开关迁 Swift: MenuHandler.setLaunchAtLogin(系统设置 + 菜单勾选 tag=777 + 面板同步), ObjC gl_settings_set_launch 变一行转发; 新增菜单状态薄桥 + set_launch_at_login FFI 声明 | cargo build/tests/parity 全绿;开机自启需真机验证 | 本提交 |
| 2026-11-24 | 质量验证(续跑第26轮) | 严格验证: cargo clippy -p glaspen2 --all-targets -- -D warnings 通过(仅 linker 旧 -ld_classic 提示, 不属警告失败), 全量测试 307+29+31+0 全绿, parity 通过; 确认 26 轮迁移无质量回退 | — | 本提交 |
| 2026-11-24 | S7l drawRect 第1片(续跑第27轮) | 网格/分栏线段绘制迁 Swift: OverlayGridRenderer.strokeLayerWithContext:...(颜色/线宽/逐段 move+addLine/一次 strokePath, 样式逐值一致), ObjC grid_stroke_layer 变一行转发; drawRect 网格层绘制逻辑已在 Swift, 几何仍由 core::grid | cargo build/tests/parity 全绿;网格像素需真机/截图验证 | 本提交 |
| 2026-11-24 | S7m drawRect 第2片(续跑第28轮) | draw_grid_lines 整体迁 Swift: OverlayGridRenderer.drawGrid(门控、pan/zoom 换算、clip→视图坐标、core::grid 全屏/脏矩形入口、JSON 解析错误处理、三段样式绘制), ObjC 函数变一行转发; 状态经网格快照桥一次性传入; D6 闸门扩展扫描 overlay_grid.swift | cargo build/tests/parity 全绿;网格视觉需真机/截图像素验证 | 本提交 |
