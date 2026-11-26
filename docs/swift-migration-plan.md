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
> - 当前 Swift 文件: 103 个, 已按子系统分目录(src/macos/swift/{App,Menu,Input,Surface,Settings,Features,Diagnostics}/);目录结构/规则见 src/macos/swift/README.md,build.rs 递归收集且文件名全局唯一,tests/swift_layout.rs 守护。

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
| 2026-11-24 | S7n drawRect 第3片(续跑第29轮) | 光标十字层迁 Swift: OverlayCursorRenderer.drawCursor(白/红圈、中心点、四向线, 颜色/线宽/几何逐值一致), ObjC drawRect 内 ~45 行块删除, 状态经快照桥一次性传入 | cargo build/tests/parity 全绿;光标像素需真机/截图验证 | 本提交 |
| 2026-11-24 | S7o(续跑第30轮) | 清理网格迁移后的 ObjC 死代码: 删除无调用的 grid_json_dict(JSON 解析已在 Swift)与 grid_stroke_layer(线段绘制已在 Swift); D6 闸门移除已不存在的 grid_stroke_layer 记号, 保留 draw_grid_lines/core::grid 入口校验 | cargo build/tests/parity 全绿 | 本提交 |

## 迁移审计补充(2026-11-24,第31轮)

- 对 `src/macos/*.m` 重新做 static 函数引用扫描:除 9 个 `glaspen2_macos_*` C ABI 入口
  (由 Rust/FRB 侧调用,本地仅 1 处定义属正常)外,无未引用的 static 函数;S7 死代码清理已到位。
- 当前剩余迁移均为“大块/需像素或真机验证”的代码:
  - `run.m -drawRect:` 的笔迹 surface 合成(CGImage 缓存 + 脏矩形子图 + 反色路径)、
    minimap 缩略图、时光隧道动效;
  - `panel.m` 设置窗口/Flutter engine 宿主/压感监视器窗口;
  - `menu.m` 设置 setter 余项与 selector 转发类;
  - `glaspen2.m` 设置分派/DB 初始化/C ABI shim(保留符号名,可换 Swift 实现)。
- 这些块单函数 100–188 行,迁移后必须做截图/像素对比与真机验收,建议在上下文充足时
  按 40–60 行一片推进;无 GUI 环境不应盲改。
| 2026-11-24 | S8 审计补充(续跑第31轮) | 重新扫描 static 死代码: 仅剩 9 个 Rust 调用的 C ABI 入口(本地单引用属正常), 无真死代码; 记录剩余大块迁移门槛(需像素/真机验证, 建议 40–60 行一片) | 文档同步, 无代码变更; 构建/测试保持全绿 | 本提交 |
| 2026-11-24 | S7p drawRect 第4片(续跑第32轮) | 笔迹 surface 合成绘制迁 Swift: OverlaySurfaceRenderer.drawSurfaceImage(小脏矩形→物理像素裁剪(含 Y 翻转)、CGImage 失败回退全图、全图绘制), ObjC ~30 行块删除; CGImage 缓存构造仍 ObjC(下一片) | cargo build/tests/parity 全绿;笔迹像素/脏矩形需真机/截图验证 | 本提交 |
| 2026-11-24 | S7q(续跑第33轮) | 折叠 1 行状态图标包装: 删除 ObjC update_status_icon_state 及前向声明, 所有调用点直接用 update_status_icon_color; 桥 glaspen2_router_update_status_icon_state 直接调 color 刷新; 删除 Bridging.h 中重复声明 | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S7r drawRect 第5片(续跑第34轮) | 笔迹 surface 的 CGImage 缓存构造迁 Swift: OverlaySurfaceImageCache.imageForData:...(地址/尺寸/stride 变化才重建; 不释放 cairo 像素, release 回调空实现), Swift 持有缓存, ObjC 只借指针; surface_image_cache_invalidate 变一行 clear; 删除 ObjC 四个缓存全局与旧创建代码 | cargo build/tests/parity 全绿;笔迹上屏/缓存失效需真机+截图验证 | 本提交 |
| 2026-11-24 | S7s drawRect 第6片(续跑第35轮) | 时光隧道 surface 缓存/绘制迁 Swift: OverlayTunnelRenderer(半分辨率表面→CGImage 缓存(地址/尺寸变化重建)、全屏缩放绘制、clear), drawRect 早期分支与 tunnel_free_cards 顺序保持(先清缓存再销毁表面); 删除 ObjC 隧道 CGImage 全局; 修复构建暴露的 teardown 引用 | cargo build/tests/parity 全绿;翻页隧道动效需真机验证 | 本提交 |
| 2026-11-24 | S7t(续跑第36轮) | 网格开关迁 Swift: MenuHandler.setGridEnabled(状态/落库/面板同步/重绘), ObjC gl_settings_set_grid 变一行转发; 新增网格状态与重绘两个薄桥 | cargo build/tests/parity 全绿;网格开关需真机验证 | 本提交 |
| 2026-11-24 | S7u(续跑第37轮) | 压感监视器开关迁 Swift: MenuHandler.setPressureMonitor(状态/落库/显示隐藏/刷新/面板同步), ObjC gl_settings_set_pressure_monitor 变一行转发; 新增 4 个薄桥; 修复桥名与既有 pm_update(6 参)冲突(改名 pm_refresh) | cargo build/tests/parity 全绿;压感监视器需真机验证 | 本提交 |
| 2026-11-24 | S7v(续跑第38轮) | 涂鸦总开关迁 Swift: MenuHandler.setDrawEnabled(状态 + 菜单 888 勾选/本地化标题 + 状态图标), ObjC gl_settings_set_enabled 变一行转发; 新增 enabled 状态与菜单标题两个薄桥 | cargo build/tests/parity 全绿;总开关/菜单标题需真机验证 | 本提交 |
| 2026-11-24 | S7w(续跑第39轮) | 手写消息总开关迁 Swift: MenuHandler.setChatIntegration(状态/落库/录制状态机通知/关闭停共享、重开按 share_ink 恢复), ObjC gl_settings_set_chat_integration 变一行转发; 新增状态与录制通知薄桥; D8b 闸门改为 menu.m+menu_handler.swift 合并源校验桥名 | cargo build/tests/parity 全绿;集成开关/共享上行需真机验证 | 本提交 |
| 2026-11-24 | 续跑(第41轮) | 目标轮次上限 40→80 并重新激活; 全量测试(307+29+31+0)与 parity 全绿。至此**所有设置 setter 均已迁 Swift**(颜色/粗细/玻璃/自启/网格/压感/总开关/手写集成), 剩余为大型 app 主体: drawRect 的 minimap/剩余分支、panel.m 设置宿主、menu.m selector 转发、glaspen2.m shim | 验证通过, 无代码变更 | 本提交 |
| 2026-11-24 | S7x(续跑第41轮) | 清理遗留设置面板按钮 stub: 删除完全无引用的 g_color_buttons/g_width_buttons/g_launch_toggle, 删除 g_glass_toggle/g_glass_buttons 及 gl_glass_apply 中对应的 nil/空数组 no-op 赋值; 桌面面板早以 Flutter 取代 | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S7y(续跑第42轮) | 压感监视器标签格式化迁 Swift: MenuHandler.updatePressureLabel(P=%-5d + 事件名 + 坐标, 缺省 ---), ObjC pm_update 只做窗口/标签存在性守卫与取值; 新增标签设置薄桥 | cargo build/tests/parity 全绿;压感监视器显示需真机验证 | 本提交 |
| 2026-11-24 | S7z(续跑第43轮) | 磨砂玻璃应用逻辑迁 Swift: MenuHandler.applyGlass(组合 enabled/opacity/飘渺跟随/笔迹可见性 → alpha/hidden + 菜单 444 勾选), ObjC gl_glass_apply 变一行转发; 新增玻璃状态快照与应用两个薄桥 | cargo build/tests/parity 全绿;玻璃视觉效果需真机验证 | 本提交 |
| 2026-11-24 | S7aa(续跑第44轮) | 删除已迁 Swift 的设置 setter 转发: gl_settings_set_enabled(无调用) 直接删; gl_settings_set_grid / gl_settings_set_pressure_monitor 调用点改为直接调 MenuHandler(Swift) 后删转发; 同步删 glaspen2.m 前向声明 | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S7ab(续跑第45轮) | 继续删除已迁 Swift 的设置 setter 转发(launch/glass/chat/color/width): 调用点(设置分派/快捷键/撤销恢复/菜单选择桥)全部直调 MenuHandler, 删除 5 个 ObjC 转发函数与 glaspen2.m 前向声明 | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S7ac(续跑第46轮) | 描边开关迁 Swift: MenuHandler.setOutlineEnabled(状态/FFI/落库/菜单勾选/finish+rebuild/本地化提示), ObjC apply_outline 变一行转发(调用点不变); 新增 3 个薄桥 + set_stroke_outline FFI 声明 | cargo build/tests/parity 全绿;描边视觉需真机验证 | 本提交 |
| 2026-11-24 | S7ad(续跑第47轮) | 删除 finish_active_stroke 的 ObjC 转发: 全部调用点直调 [EventRouter finishActiveStroke](Swift), panel.m 转发函数与 glaspen2.m 前向声明删除 | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S7ae(续跑第48轮) | 删除 gl_quit_app 的 ObjC 转发: 菜单退出/设置面板退出/快捷键退出三处调用点直调 [MenuHandler quitAppNow](Swift), menu.m 转发函数与 glaspen2.m 前向声明删除 | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S7af(续跑第49轮) | 继续删转发: gl_glass_apply 调用点/桥直调 [MenuHandler applyGlass]; apply_outline 调用点/桥直调 setOutlineEnabled; sync_settings_panel 全部调用点直调 glaspen2_notify_settings_changed; 三个 ObjC 包装与前向声明删除 | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S7ag(续跑第50轮) | 删除 restore_system_cursor 的 ObjC 包装: 调用点直调 [EventRouter restoreSystemCursor](Swift, 仅 pen_drawing 时 show+清标记); glaspen2.m 包装与前向声明删除 | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S7ah(续跑第51轮) | 删除 show_page_info 的 ObjC 包装: 5 处调用点(翻页/新建页/气泡翻页)直调 [NotificationPresenter showPageInfoWithScreenId](Swift); glaspen2.m 包装与前向声明删除 | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S7ai(续跑第52轮) | 删除 update_status_icon_color 包装: 全部调用点直调桥 glaspen2_router_update_status_icon_state(), 桥实现直接调 Swift StatusIconUpdater; 桥在 ObjC 侧补前向声明(Bridging.h 仅对 Swift 可见); 期间修正自递归与未声明错误 | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S8 检查点(续跑第53轮) | 53 轮检查点: 全量测试/parity 全绿; 设置层与状态图标/页信息/光标恢复/退出等 ObjC 包装均已删除, 调用直指 Swift; 剩余为大型功能体(无限画布切换、draw_minimap、dirty 合成余量、panel.m 设置宿主、menu.m selector 转发、glaspen2.m shim), 单函数 100–188 行且需真机/截图验证, 不宜在低上下文下盲改 | 无代码变更, 验证通过 | 本提交 |
| 2026-11-24 | S7aj(续跑第54轮) | 重复实例检测迁 Swift: 新增 app_instance.swift(AppInstanceGuard.anotherInstanceRunning: 排除自身 pid, 按 localizedName/可执行名匹配 glaspen2), run.m 调用点直调 Swift 并删除 ObjC 实现 | cargo build/tests/parity 全绿;双实例启动需真机验证 | 本提交 |
| 2026-11-24 | S7ak(续跑第55轮) | 镜头 JSON 解析/应用迁 Swift: 新增 camera_state.swift(CameraState.resultFromJSON 释放 C 串+解析, assignResult 校验字段并调镜头状态桥), ObjC camera_result_json/camera_assign 变一行转发(调用点暂不变); 修复 Swift NSLog 不可用问题(改用字符串插值) | cargo build/tests/parity 全绿;镜头平移/缩放需真机验证 | 本提交 |
| 2026-11-24 | S7al(续跑第56轮) | chord 规范化包装迁 Swift: 新增 chord_normalizer.swift(ChordNormalizer.normalizeRaw 调 core FFI + 释放 C 串), menu.m 调用点直调并删除 ObjC 实现与声明 | cargo build/tests/parity 全绿;快捷键规范化需真机验证 | 本提交 |
| 2026-11-24 | S7am(续跑第57轮) | 修 ObjC 编译警告: 页面缩略图边框色把属性 getter 当副作用用, 改为显式 NSColor 变量 + setStroke; 构建不再出现 "property access result unused" 警告 | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S7an(续跑第58轮) | 快捷键录制状态迁 Swift: 新增 hotkey_recorder.swift(HotkeyRecorder: recording/generation + 60s 兜底), EventRouter 直接读 Swift 状态; settings 分派调用点直调 Swift; 删除 ObjC 状态全局/函数/桥 | cargo build/tests/parity 全绿;录制快捷键需真机验证 | 本提交 |
| 2026-11-24 | S7ao(续跑第59轮) | 单实例文件锁迁 Swift: 新增 instance_lock.swift(InstanceLock.acquire: GLASPEN2_ALLOW_MULTI 旁路 + 临时目录 flock, 拿到锁故意不 close), 启动调用点直调 Swift; 删除 ObjC 实现/声明 | cargo build/tests/parity 全绿;双开/多开需真机验证 | 本提交 |
| 2026-11-24 | S7ap(续跑第60轮) | 键盘平移镜头迁 Swift: 新增 canvas_camera.swift(CanvasCamera.panBy: 书写中忽略 → core::camera FFI → CameraState 应用 → canvas 变换/peek), ObjC canvas_pan_by 变一行转发; 新增 5 个薄桥+FFI 声明; D5b 闸门扩展扫描 canvas_camera.swift | cargo build/tests/parity 全绿;键盘平移/peek 需真机验证 | 本提交 |
| 2026-11-24 | S7aq(续跑第61轮) | 缩放结果应用迁 Swift: CanvasCamera.applyZoomResult(100% 上限提示 1.5s 节流 + CameraState 应用 + changed 门控 + 变换/peek), ObjC canvas_zoom_apply 变一行转发; D5b 闸门令牌 camera_assign( 改为 CameraState.assign( (逻辑已迁 Swift 源) | cargo build/tests/parity 全绿;缩放/提示节流需真机验证 | 本提交 |
| 2026-11-24 | S7ar(续跑第62轮) | 相机鼠标平移调度迁 Swift: 新增 camera_mouse_pan.swift(16ms 合帧 queue、flush 按状态调用 CanvasCamera.panBy、cancel 作废)， ObjC 三个状态全局删除, queue/flush/cancel 变一行转发(调用点不变) | cargo build/tests/parity 全绿;抓手平移需真机验证 | 本提交 |
| 2026-11-24 | S7as(续跑第63轮) | 快捷键存档值迁移迁 Swift: ChordNormalizer.migrateSavedAction:raw:(非字符串原样返回, 字符串走 core 迁移表并释放 C 串), 调用点直调 Swift; 删除 ObjC 实现/声明 | cargo build/tests/parity 全绿;旧快捷键存档兼容需真机验证 | 本提交 |
| 2026-11-24 | S7at(续跑第64轮) | 无限画布镜头持久化/载入迁 Swift: CanvasCamera.persistInfinite(仅无限模式 → set_infinite_transform) 与 loadInfinite(get → CameraState 应用 → set_view_transform), ObjC 两函数变一行转发(调用点不变); 新增 3 个 FFI 声明 | cargo build/tests/parity 全绿;无限画布镜头恢复需真机验证 | 本提交 |
| 2026-11-24 | S7au(续跑第65轮) | 删除无限画布镜头的 ObjC 转发: 5 处调用点直调 [CanvasCamera persistInfinite/loadInfinite](Swift), run.m 两包装与 glaspen2.m 前向声明删除 | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S7av(续跑第66轮) | 滚轮/键盘缩放 helper 迁 Swift: CanvasCamera.zoomAt/zoomAtCenter(书写中忽略 → core Camera::zoom_at(_center) → CameraState 应用), 调用点直调 Swift; 删除 ObjC 两函数/声明; 新增屏幕尺寸薄桥与 FFI 声明 | cargo build/tests/parity 全绿;滚轮/键盘缩放需真机验证 | 本提交 |
| 2026-11-24 | 质量验证(续跑第67轮) | 严格 clippy 通过: cargo clippy -p glaspen2 --all-targets -- -D warnings 退出码 0(仅 linker 旧 -ld_classic 提示, 属 linker_messages lint 不参与 -D warnings); 自 53 轮后新增的 camera/instance/hotkey 等 Swift 迁移无静态检查回退 | 无代码变更 | 本提交 |
| 2026-11-24 | S7aw(续跑第68轮) | 删除 canvas_pan_by 的 ObjC 转发: 唯一调用点直调 [CanvasCamera panByDX:dy:], wrapper 与 glaspen2.m 前向声明删除; 过程中一次越界删除(误删相邻函数)被 cargo 构建拦下, 已 git checkout 回滚并用括号匹配重做 | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S7ax(续跑第69轮) | GIF 剪贴板写入迁 Swift: 新增 gif_clipboard.swift(GifClipboard.copyData:length: 原子写临时 GIF + NSPasteboard 文件 URL), GIF 编码回调调用点直调 Swift; 删除 ObjC 实现/声明 | cargo build/tests/parity 全绿;GIF 复制到剪贴板需真机验证 | 本提交 |
| 2026-11-24 | S7ay(续跑第70轮) | 镜头变换应用迁 Swift: CanvasCamera.applyTransform(set_view_transform + rebuild_surface + 0.5s 节流 persist), ObjC canvas_apply_transform 变一行转发; 构建暴露 menu.m 前向声明缺失, 已在 glaspen2.m 补回 | cargo build/tests/parity 全绿;镜头变换/节流持久化需真机验证 | 本提交 |
| 2026-11-24 | S7az(续跑第71轮) | 笔色/笔宽预设加载迁 Swift: 新增 pen_presets.swift(PenPresets.loadPresets: 数量不一致告警 + 逐项 FFI 读入经薄桥写表), ObjC gl_load_pen_presets 一行转发; 修复 Swift 禁止类方法 load 的命名冲突; 新增 2 个写表桥 + 4 个 FFI 声明 | cargo build/tests/parity 全绿;颜色/宽度预设需真机验证 | 本提交 |
| 2026-11-24 | S7ba(续跑第72轮) | 删除笔色/笔宽预设加载的 ObjC 转发: 唯一调用点直调 [PenPresets loadPresets](Swift), run.m wrapper 与 glaspen2.m 声明删除 | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S7bb(续跑第73轮) | 死代码扫描: canvas_zoom_apply 在缩放 helper 迁 Swift 后已无调用(唯一无引用 static), 删除 wrapper 与声明 | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S7bc(续跑第74轮) | 删除最后两个镜头转发(camera_result_json/camera_assign): 唯一调用点直调 [CameraState resultFromJSON/assignResult](Swift), ObjC 实现与声明删除; D5b 闸门令牌按语言拆分(run.m 列表用 Swift 的 CameraState.assign(, menu.m 列表用 ObjC 的 CameraState assignResult:) | cargo build/tests/parity 全绿;居中镜头需真机验证 | 本提交 |
| 2026-11-24 | S7bd(续跑第75轮) | 删除 canvas_apply_transform 的 ObjC 转发: 全部调用点(居中/编辑动作/桥)直调 [CanvasCamera applyTransform](Swift), wrapper 与前向声明删除 | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S7be(续跑第76轮) | 删除相机鼠标平移三个 ObjC 包装(queue/flush/cancel): 桥与调用点直调 [CameraMousePan ...](Swift), wrapper 与 glaspen2.m 声明删除 | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S7bf(续跑第77轮) | 镜头回原点迁 Swift: CanvasCamera.home(FFI home → 状态应用 → applyTransform) 与 resetLens(仅设置变换), 三处调用点(编辑动作/清空/翻页重置)直调 Swift; 删除重复的 home+set_view/rebuild 序列; D5b menu.m 令牌改为 [CanvasCamera home]; 补 glaspen2_camera_home FFI 声明 | cargo build/tests/parity 全绿;镜头回原点需真机验证 | 本提交 |
| 2026-11-24 | S8 检查点(续跑第78轮) | 78 轮检查点: 全量测试/parity 全绿; 死代码与死桥扫描均无输出(无未引用 static / 桥); 输入、镜头、设置、工具层已全部 Swift 化; 剩余仅大型功能体(infinite canvas 切换、draw_minimap、dirty 合成余量、panel.m 宿主、menu.m selector 转发、glaspen2.m shim), 迁移必须配合真机/截图验证, 不宜低上下文盲改 | 无代码变更 | 本提交 |
| 2026-11-24 | S7bg(续跑第79轮) | 删除 canvas_reset_lens 包装: Swift resetLens 增加 page 偏移清零薄桥, 3 处调用点直调 [CanvasCamera resetLens], wrapper 与声明删除 | cargo build/tests/parity 全绿 | 本提交 |

## 80 轮(第二次目标冲刺)收尾状态(2026-11-24)

> 结论:**目标未达成**(仓库仍有约 5,993 行应用层 ObjC);goal 保持 active,轮次 80/80 已用尽。

### 本轮冲刺新增(相比第一次 40 轮)
- 事件路由收尾: 删除 `event_tap_callback_inner`,tap-disabled/笔坐标/压力/raw_w/压感/分派全部入 Swift;
- 设置层全部 Swift: 颜色/粗细/玻璃/自启/网格/压感/总开关/手写集成(含删除 ObjC 转发);
- 输入/镜头/工具层: 菜单位置分支、橡皮态、笔光标/系统光标、proximity、相机抓手与鼠标 pan 调度、
  zoom/pan/home/persist/load、实例锁、重复实例检测、录制热键/录制状态、HUD 文案、页信息、GIF 剪贴板、
  chord 规范化/迁移、笔预设加载、退出/光标恢复等全部 Swift 化并删除 ObjC 包装;
- `drawRect` 已迁: 网格层、笔迹子图绘制、CGImage 缓存、隧道 surface、光标十字、perf 打点;
- 闸门同步: i18n/D8b/D6/D5b 闸门均已扩展扫描对应 Swift 源;
- 质量: 每步 `cargo build` + 全量测试 + parity;本轮结束 clippy `-D warnings` 亦通过。

### 当前数字
- `main` = 6d7dc21(随后仅文档提交), 领先 origin/main 222+ 提交, **未推送**; `v0.7.0` tag 未动;
- ObjC 应用层 5,993 行; Swift 2,433 行(31 文件);
- 全量测试 307+29+31+0 全绿, parity 通过, clippy 通过。

### 剩余(下一轮从高上下文继续)
1. `apply_infinite_canvas` 切换(画布种类/冲刷/持久化/重建/peek/通知);
2. `draw_minimap`(布局缓存 + 异步缩略图队列 + 条带绘制, 单函数最大);
3. `drawRect` 剩余 dirty 合成/invert 路径与 `panel.m` 设置宿主;
4. `menu.m` selector 转发、`glaspen2.m` 设置分派/DB 初始化 shim;
5. **S8 真机验收**: 笔输入/延迟、多屏/锁屏、菜单视觉、热键/录制、玻璃、隧道/网格像素;
6. 已知缺口: tap 路径 perf 打点未迁。
| 2026-11-24 | S7bh(新目标第1轮) | 画布模式切换迁 Swift: 新增 canvas_mode.swift(CanvasMode.applyInfiniteCanvasOn:notify: 状态/落库/菜单 668/图标/存储切换/镜头载入或重置/tap 重装/通知), ObjC apply_infinite_canvas 变一行转发; 新增 5 个薄桥 + 6 个 FFI 声明; 构建暴露调用点先于声明, 已在 glaspen2.m 补前向声明 | cargo build/tests/parity 全绿;翻页/无限切换需真机验证 | 本提交 |
| 2026-11-24 | S7bi(新目标第2轮) | draw_minimap 整体迁 Swift: 新增 minimap.swift(MinimapRenderer: 布局缓存 + core::minimap JSON 解析 + 专属串行队列异步缩略图 + 整条 strip 合成/页号/边框 + 右缘一次贴图), ObjC 函数/JSON helper/7 个缓存全局头文件声明全部删除; 新增 pageview_pw 薄桥与 3 个 FFI 声明; D7 闸门扩展扫描 minimap.swift | cargo build/tests/parity 全绿;缩略图条视觉需真机/截图验证 | 本提交 |
| 2026-11-24 | S7bj(新目标第3轮) | 清理 minimap 迁移残留: 删除 run.m 中三个已无引用的 strip/fetch 全局(上次删除因注释文本不匹配漏掉); 全仓 grep 确认无 g_minimap_* 缓存引用(仅 g_minimap_enabled 保留) | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S7bk(新目标第4轮) | 快捷键修饰键掩码迁 Swift: 新增 shortcut_mask.swift(ShortcutMask.maskFor: 读 chord 解析 cmd/ctrl/alt/shift), run.m 两处调用点直调 Swift; 删除 ObjC 函数/声明; 新增 chord 查询薄桥 | cargo build/tests/parity 全绿;快捷键修饰键需真机验证 | 本提交 |
| 2026-11-24 | S7bl(新目标第5轮) | drawRect 整体编排迁 Swift: 新增 overlay_drawing.swift(裁剪/隧道/网格/笔迹图/缩略图/通知/光标/perf 打点), ObjC drawRect 变一行转发; 表面/隧道状态经单次快照桥 + flush 桥传入; 删除旧 ~90 行编排; D6 闸门合并源加入 overlay_drawing.swift | cargo build/tests/parity 全绿;绘制/裁剪/隧道需真机+截图验证 | 本提交 |
| 2026-11-24 | S7bm(新目标第6轮) | 压感监视器窗口迁 Swift: 新增 pressure_monitor.swift(PressureMonitorWindow: 创建/置顶/不接收鼠标/Menlo 标签/显示隐藏关闭/更新文案), ObjC pm_ensure_window/pm_show/pm_hide/pm_destroy 变一行转发, pm_update 去掉 ObjC 全局判空; MenuHandler 直接更新 Swift 标签; 删除 g_pm_window/g_pm_label 与 set_pm_label 桥 | cargo build/tests/parity 全绿;压感监视器显示需真机验证 | 本提交 |
| 2026-11-24 | S7bn(新目标第7轮) | 无限画布总览载荷迁 Swift: 新增 canvas_overview.swift(CanvasOverview.payload: 包围盒外扩 5% + core 渲染 + scale/offset 与视口矩形映射 + 释放 Rust bytes), ObjC helper 删除; 调用点改读 payload bounds; 新增 3 FFI 声明 | cargo build/tests/parity 全绿;总览图/视口框需真机验证 | 本提交 |
| 2026-11-24 | S7bo(新目标第8轮) | 涂鸦启停迁 Swift: 新增 draw_mode.swift(DrawMode.toggleEnabled: 覆盖层显隐/收尾/压感窗/飘渺隐藏态/状态图标/菜单文案/通知), ObjC toggle_enabled 一行转发; 新增 4 个薄桥; i18n 闸门调用点统计纳入 Swift 的 menu_l10n 桥 | cargo build/tests/parity 全绿;启停/覆盖层/压感窗需真机验证 | 本提交 |
| 2026-11-24 | S7bp(新目标第9轮) | 飘渺/固定画布切换迁 Swift: DrawMode.toggleCanvasMode(peek 取消/笔迹显隐/玻璃跟随隐藏/压感窗/菜单 778 勾选与文案/落库/图标/通知), ObjC toggle_canvas_mode 一行转发; 新增 4 个薄桥; 修复插入时被转义的 $0 closure 参数 | cargo build/tests/parity 全绿;飘渺模式视觉需真机验证 | 本提交 |
| 2026-11-24 | S7bq(新目标第10轮) | 菜单文案刷新迁 Swift: 新增 menu_texts.swift(MenuTextUpdater.update: 颜色/粗细富文本经桥 + 顶层/导出/更多/模式/总开关文案组装 + 调用 Swift MenuUpdater), ObjC update_menu_texts 一行转发; 新增 menu/状态/富文本标题 4 个薄桥; 修复 l10n 可选值 | cargo build/tests/parity 全绿;菜单文案/勾选需真机验证 | 本提交 |
| 2026-11-24 | S7br(新目标第11轮) | 快捷键默认表/动作表迁 Swift: 新增 shortcut_table.swift(ShortcutTable.defaults/actions: core::ui FFI JSON + 进程内缓存), ObjC 两个 loader 变一行转发; 新增 2 个 FFI 声明 | cargo build/tests/parity 全绿;快捷键默认/动作表需真机验证 | 本提交 |
| 2026-11-24 | S7bs(新目标第12轮) | 快捷键生效表应用迁 Swift: 新增 shortcut_apply.swift(ShortcutApply.applySaved: 旧值迁移/规范化/非法回退/保留组合回退/冲突解绑 + 写回生效表), ObjC gl_shortcuts_apply 一行转发; 新增 reserved/写表两个薄桥; 修复 Swift↔NSDictionary 桥接类型 | cargo build/tests/parity 全绿;快捷键冲突/迁移需真机验证 | 本提交 |
| 2026-11-24 | S7bt(新目标第13轮) | 键码名称表迁 Swift: 脚本从旧 ObjC 表生成 73 项 kVK→名称 Swift 字典(逐项一致, 避免手抄), 删除 ObjC 表; 桥 glaspen2_router_keycode_name 改调 Swift 表; 生成脚本第一版受正则转义影响无效, 已改用无正则解析重新生成并断言条数 | cargo build/tests/parity 全绿;快捷键键名映射需真机验证 | 本提交 |
| 2026-11-24 | S7bu(新目标第14轮) | 显示器几何变更迁 Swift: OverlayWindowFactory.applyDisplayChange(反色流失效/屏幕尺寸/core 建页重载/窗口与各视图改尺寸/重建), ObjC display_change_apply 一行转发; 新增 3 薄桥 + 2 FFI 声明; 发现 panel.m 末尾由 run.m 首行 } 收尾的结构, 桥改追加到 run.m 末尾 | cargo build/tests/parity 全绿;分辨率切换需真机验证 | 本提交 |
| 2026-11-24 | S7bv(新目标第15轮) | 图层面刷新迁 Swift: 新增 overlay_layer_flush.swift(全量 flush / 脏矩形 flush: 表面与视图守卫、dirty 交集/空回退、reset 后 setNeedsDisplay), ObjC 两函数变一行转发; 新增 4 个 dirty 状态薄桥 | cargo build/tests/parity 全绿;笔迹局部刷新需真机验证 | 本提交 |
| 2026-11-24 | S7bw(新目标第16轮) | cairo 接入 Swift + ensure_surface 迁 Swift: build.rs 给 swiftc 传 cairo include(含父目录), Bridging.h 引入 cairo.h; 新增 surface_store.swift(Swift 持有 cairo 表面, 尺寸/scale 变化重建 + 清屏, ObjC g_surface/g_scale 经桥借用), ObjC ensure_surface 一行转发; 新增 4 薄桥; cairo 枚举用数值(ARGB32=0, CLEAR=3, OVER=2)避免导入差异 | cargo build/tests/parity 全绿;笔迹绘制/表面重建需真机验证 | 本提交 |
| 2026-11-24 | S7bx(新目标第17轮) | 改进死代码扫描(按词边界 name( 计数, 修复子串误判): 发现并删除 5 个迁后无调用 ObjC 函数 pen_draw/gl_shortcut_actions/draw_grid_lines/apply_outline/perf_log_event 及前向声明; 复扫仅剩 signal 处理函数(按函数指针引用, 非死) | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S7by(新目标第18轮) | 首点实时绘制迁 Swift: 新增 raw_drawing.swift(RawDrawing.drawDot: 计划优先 + 软阴影三档/描边垫底/反色 pattern/落色 + dirty + flush), ObjC raw_draw_dot 一行转发; 新增 11 个 cairo/状态薄桥(活动上下文获取含 owned 标记) | cargo build/tests/parity 全绿;首点像素/阴影/描边需真机验证 | 本提交 |
| 2026-11-24 | S7bz(新目标第19轮) | 拖动实时段绘制迁 Swift: RawDrawing.drawSegment(计划优先 + 阴影/黑白虚线描边/接缝重画/反色 + raw 状态更新 + dirty/flush), ObjC raw_draw_segment 一行转发; 新增 raw 状态快照/更新、live_plan_segment 等 4 薄桥; 同时消除 panel.m 末尾由 run.m 首行 } 收尾的拆分痕迹(panel.m 补全函数, run.m 去掉孤立 }) | cargo build/tests/parity 全绿;笔迹像素/阴影/描边需真机验证 | 本提交 |
| 2026-11-24 | S7ca(新目标第20轮) | 实时笔画上下文迁 Swift: 新增 stroke_surface.swift(StrokeSurface.begin/end: 整笔复用 cairo 上下文、scale、重置 raw 相位; 抬笔销毁), ObjC stroke_begin/stroke_end 一行转发; 新增 5 薄桥; ObjC g_active_cr 仅作借用存储 | cargo build/tests/parity 全绿;书写实时渲染需真机验证 | 本提交 |
| 2026-11-24 | S7cb(新目标第21轮) | 飘渺显隐/peek 簇迁 Swift: 新增 ethereal_mode.swift(hideNow/restoreNow/autoShow/cancelTimer/peekForSeconds/peekLensChange: 笔迹显隐/玻璃跟随/压感窗/隧道动画保护/peek 计时器), ObjC 7 个函数变一行转发, 删除 g_peek_timer; 新增 tunnel_active 桥(stroke_active 已存在, 去掉重复定义) | cargo build/tests/parity 全绿;飘渺显隐/翻页 peek 需真机验证 | 本提交 |
| 2026-11-24 | S7cc(新目标第22轮) | 实时绘制计划整块迁 Swift: 新增 live_plan.swift(LivePlan: 入参 JSON/FFI 调用/命令光栅化 shadow-outline-erase-fill/接缝重画/脏区转换与兜底), 删除 ObjC gl_live_plan_source/command/seam/draw_segment/draw_dot(约 350 行); 桥改调 Swift; 新增计划脏区 rect 桥与 FFI 声明 | cargo build/tests/parity 全绿;实时笔迹像素(描边/虚线/反色/接缝)需真机+截图对比 core 整页重建 | 本提交 |
| 2026-11-24 | S7cd(新目标第23轮) | 设置面板宿主流程迁 Swift: 新增 settings_panel.swift(SettingsPanel.show: 已存在则激活前置; 引擎/VC/代理经 ObjC 薄桥创建; Swift 建窗口/标题本地化/最小尺寸/内容控制器/激活策略/透明面板), ObjC show_settings_panel 一行转发; 新增 7 薄桥 | cargo build/tests/parity 全绿;面板打开/透明需真机验证 | 本提交 |
| 2026-11-24 | S7ce(新目标第24轮) | 菜单勾选刷新与设置快照 overrides 迁 Swift: 新增 menu_glue.swift(MenuGlue.updateCheckmarks/snapshotOverridesJSON: 预设数/选中索引/快捷键 JSON/翻转效果/语言), ObjC 两函数一行转发; 新增 8 薄桥(去重已有预设数桥); D12 闸门扩展扫描 menu_glue.swift 并对 Swift 侧用带引号键做负向检查 | cargo build/tests/parity 全绿;面板快照/菜单勾选需真机验证 | 本提交 |
| 2026-11-24 | S7cf(新目标第25轮) | 性能日志簇迁 Swift: 新增 perf_log.swift(PerfLog: 环境开关/建目录与文件/时间基准/记录与 16ms SLOW 判定/每百行 flush/退出汇总), ObjC begin/init/event/summary/elapsed 全部删除, 调用点与 perf 桥改调 Swift; run.m 相关文件/计数全局删除 | cargo build/tests/parity 全绿;GLASPEN2_PERF_LOG=1 实测待真机 | 本提交 |
| 2026-11-24 | S7cg(新目标第26轮) | 虚拟笔诊断簇迁 Swift: 新增 virtual_pen.swift(VirtualPen: CGEvent 合成鼠标/平板 subtype/压力, GLASPEN2_VIRTUAL_PEN 后台循环), ObjC post/run/maybe_start 删除; 修正 user-data 常量(0x56504E31)与 ObjC 一致; 启动点改调 Swift | cargo build/tests/parity 全绿;GLASPEN2_VIRTUAL_PEN=1 压测待真机 | 本提交 |
| 2026-11-24 | S7ci(新目标第27轮) | 翻页诊断探针迁 Swift: 新增 flip_probe.swift(FlipProbe.maybeStart: GLASPEN2_FLIP_EVERY 定时器、首跳直插页、之后往返热键), ObjC 函数删除, 启动点改调 Swift; 新增 perform_hotkey/insert_pages 声明与薄桥 | cargo build/tests/parity 全绿;GLASPEN2_FLIP_EVERY 实测待真机 | 本提交 |
| 2026-11-24 | S7cj(新目标第28轮) | 菜单快捷键/主线程胶水迁 Swift: 新增 settings_glue.swift(读 DB shortcuts 覆盖→ShortcutApply; 生效表→JSON 文本; 非主线程同步投递主线程), ObjC gl_shortcuts_load/effective_json/run_on_main_sync 变一行转发; 新增生效表查询桥与 string setting FFI 声明; 修正 apply(saved:) 标签 | cargo build/tests/parity 全绿;快捷键持久化恢复需真机验证 | 本提交 |
| 2026-11-24 | S7ck(新目标第29轮) | 录制 effect 执行簇迁 Swift: 新增 recording_effects.swift(RecordingEffects: 事件回喂→FFI→effects 解析, ShowHud/EncodeGif/Clipboard/StartChatDraft/StopChatDraft/SendChatStrokes/Discard, 后台编码+主线程回喂), ObjC rec_feed 一行转发、exec_effects/exec_effect/rec_show_hud/s_pending_gif 删除; 新增 4 设置薄桥与 6 FFI 声明; D8b 闸门并入 recording_effects.swift(Swift 裸字面量). 另注: 反色捕获依赖 ScreenCaptureKit(需 macOS 12.3), 当前 Swift 部署目标 12.0, 迁移前需产品层决定是否抬版本, 暂留 ObjC. | cargo build/tests/parity 全绿;录制/GIF/聊天 effect 需真机验证 | 本提交 |
| 2026-11-24 | S7cl(新目标第30轮) | 动作分派 perform_action 迁 Swift: 新增 action_dispatch.swift(ActionDispatch: core action-effect JSON → kind 分派, 热键/自由画布/镜头平移缩放/颜色粗细交换/退出, 通知与 textf 经桥), ObjC 一行转发; 新增 7 薄桥 + 4 FFI 声明。闸门: D3b 合并 Swift 源并放宽 body 定位、D5b 并入 action_dispatch、i18n 允许 action_dispatch 的 wire kind "Undo" | cargo build/tests/parity 全绿;动作热键需真机验证 | 本提交 |
| 2026-11-24 | S7cm(新目标第31轮) | 快捷键分发 perform_hotkey 迁 Swift: 新增 hotkey_dispatch.swift(HotkeyDispatch: 清空/翻页(含 5 秒内再按建页)/SVG+GIF 导出与剪贴板/撤销/复制 SVG/玻璃/飘渺/设置面板), ObjC 一行转发; 新增 clear_screen/replay/page_flip(+from, block 桥)与 9 FFI 声明; 修正 C 桥无参数标签与 Int64 screenId | cargo build/tests/parity 全绿;快捷键全路径需真机验证 | 本提交 |
| 2026-11-24 | S7cn(新目标第32轮) | 内存笔迹重放 + 页框绘制迁 Swift: 新增 surface_replay.swift(SurfaceReplay.replay/drawPageFrame: core 重建视图 + 页外压暗/页边界线 + flush/重置/图层刷新), ObjC replay 与 pageview_frame_draw 变一行转发(后者保留供多调用点); 新增 6 页视图 getter 桥(避免与 minimap 的 pw 冲突而改名 page_w/page_h); 修正 rebuild_view 返回 void | cargo build/tests/parity 全绿;翻页/快照页框像素需真机验证 | 本提交 |
| 2026-11-24 | S7co(新目标第33轮) | 清屏与仅保存涂鸦迁 Swift: 新增 surface_ops.swift(SurfaceOps.clearScreen/saveDrawingOnly: CLEAR 绘制/清空笔画/页信息或空白保护; 纯涂鸦 PNG 导出 + 通知; 自带 scale+页视图 context helper), ObjC 两函数一行转发; 新增 2 FFI 声明; 修正 Int64 screenId | cargo build/tests/parity 全绿;清屏/导出截图需真机验证 | 本提交 |
| 2026-11-24 | S7cp(新目标第34轮) | 页视图几何/笔迹重建/气泡自动翻页迁 Swift: 新增 pageview_ops.swift(rebuildSurface: core 光栅+页框+dirty_reset; bubbleAutoPageFlip: 建页/翻页/页信息/通知; update: 页尺寸 FFI→fit scale/offset 并用桥写回 ObjC 状态), ObjC 三函数一行转发; 新增 3 FFI/桥声明; 修正 Int64 screenId | cargo build/tests/parity 全绿;翻页/页视图像素需真机验证 | 本提交 |

### 中期检查点(新目标第35轮, 2026-11-24)

- 验证: `cargo build` / 全量测试 307+29+31 / parity / clippy -D warnings 全绿。
- 规模: 应用层 ObjC 4,156 行, Swift 59 文件; `main` 257 提交领先 origin(未推送), `v0.7.0` tag 未动。
- 已迁完: 事件 tap 路由、全部设置动作、镜头/输入/光标、画布模式切换、菜单(构建/文案/勾选/快捷键表/动作与热键分派)、
  笔迹 cairo 实时绘制与计划光栅、drawRect 各层、minimap、页视图/重建/翻页(swap 编排除外)、录制 effect 分派、
  性能日志、虚拟笔/翻页诊断、设置面板宿主、清屏/导出涂鸦、笔迹重放与页框。
- 剩余大块:
  1. `run.m`: 时光隧道动画(`tunnel_frame/finish/free_cards`、卡片绘制/放置、`page_flip_tunnel`、swap 编排)与
     `glaspen2_run` 启动编排(菜单构建/设置恢复/窗口与视图创建/事件 tap 安装/退出);
  2. `glaspen2.m`: `save_with_background`/`capture_below_own_window`(ScreenCaptureKit), `glaspen2_run` 之外的 DB/窗口辅助;
  3. `panel.m`: 反色捕获子系统(ScreenCaptureKit: 双缓冲/消费/监流/pattern) —— **需产品决策**: Swift 部署目标当前 12.0,
     SCK 需 12.3, 迁移前须确认是否抬最低 macOS;
  4. `menu.m`: `glaspen2_macos_settings_json`/`set_setting` 等 C ABI 分派与设置应用小胶水。
- S8 真机验收仍未完成(不标完成): 笔迹像素与延迟、翻页/隧道动效、多屏/锁屏唤醒、菜单视觉 zh/en、热键/录制、玻璃、缩略图条。
| 2026-11-24 | S7cq(新目标第36轮) | 主菜单构建块迁 Swift: 新增 main_menu.swift(MainMenuBuilder.build: delegate/autoenable、颜色与粗细富文本项、顶层/导出/更多/语言文案字典、调用 MenuBuilder、挂到状态栏), run.m 原 ~120 行构建块替换为一次 Swift 调用 | cargo build/tests/parity 全绿;菜单构建/文案需真机验证 | 本提交 |
| 2026-11-24 | S7cr(新目标第37轮) | 时光隧道卡片绘制迁 Swift: 新增 tunnel_card.swift(TunnelCardRenderer.draw: 落定帧/玻璃厚度与阴影/上下侧面渐变/前脸磨砂+快照/扫光/棱线), ObjC tunnel_draw_card 变参数适配转发(保留 mip 选择与全局读取); 逐项保留颜色/alpha/线宽 | cargo build/tests/parity 全绿;隧道动效像素需真机验证 | 本提交 |
| 2026-11-24 | S7cs(新目标第38轮) | 隧道卡片放置几何迁 Swift: 新增 tunnel_geometry.swift(TunnelGeometry.place: 页距→中心/尺寸/透明度的单源公式), ObjC tunnel_place 变参数适配转发(读取 k/surf/travel 经薄桥); 补 4 个前向声明修正隐式声明错误 | cargo build/tests/parity 全绿;隧道层叠观感需真机验证 | 本提交 |
| 2026-11-24 | S7ct(新目标第39轮) | 隧道建卡迁 Swift: 新增 tunnel_card_factory.swift(TunnelCardFactory.make: 页几何适配/半分辨率快照(当前页拷贝或 core 预览)/按深度预缩/半尺寸 mip), ObjC tunnel_make_card 变表面适配转发(NSValue 取指针存数组); 修正 page_dims Int 与 cairo 状态枚举比较 | cargo build/tests/parity 全绿;隧道建卡/磨砂观感需真机验证 | 本提交 |
| 2026-11-24 | S7cu(新目标第40轮) | 隧道逐帧渲染迁 Swift: 新增 tunnel_frame.swift(TunnelFrameRenderer.render: 缓动/相机位移、深→浅排序、落定帧、焦点卡、1/3 模糊缓冲合成、近层绘制, 卡片表面经状态快照), ObjC tunnel_frame 仅保留 surface/cards 守卫与帧计数, 新增状态快照与 flush 桥; 补 2 前向声明 | cargo build/tests/parity 全绿;隧道逐帧像素/磨砂需真机验证 | 本提交 |
| 2026-11-24 | S7cv(新目标第41轮) | 隧道生命周期迁 Swift: 新增 tunnel_lifecycle.swift(freeCards: 逐卡/mip/blur/表面 destroy + 渲染缓存清理; finish: 取消定时器→末帧→FPS 统计→释放→恢复玻璃→回调), ObjC tunnel_finish/tunnel_free_cards 一行转发; 新增 13 个状态操作桥 | cargo build/tests/parity 全绿;隧道结束动效/资源释放需真机验证 | 本提交 |
| 2026-11-24 | S7cw(新目标第42轮) | 翻页编排迁 Swift: 新增 flipper.swift(Flipper.swapFrom/swap: 旧隧道收尾、隧道接管判定(prepare 计时/失败回退)、系统隐藏间隙动效、commit 后显示与耗时日志), ObjC page_flip_swap_from/swap 一行转发; 新增 tunnel_finish/page_flip_tunnel/flip_effect 桥; 修正 long→Int 类型 | cargo build/tests/parity 全绿;翻页动效/隧道回退需真机验证 | 本提交 |
| 2026-11-24 | S7cx(新目标第43轮) | 隧道启动迁 Swift: 新增 tunnel_animation.swift(TunnelAnimation.start: 防重入/当前页与目标页/半分辨率表面与 1/3 模糊缓冲/预载与 4 帧建卡预算/参数(时长/k/厚度/浓度)环境变量/首帧同步上屏/120Hz 定时器与末帧收尾), ObjC page_flip_tunnel 一行转发; 新增 20 个启动相关桥(含定时器 __bridge 与 Swift 强引用); 至此隧道/翻页动画逻辑全部 Swift 化 | cargo build/tests/parity 全绿;隧道动效全流程需真机验证(GLASPen2_FLIP_EFFECT=1) | 本提交 |
| 2026-11-24 | S7cy(新目标第44轮) | 启动设置恢复(前半)迁 Swift: 新增 startup_settings.swift(StartupSettings.restorePartA: 语言/页视图/笔色宽与预设吸附/状态图标/勾选/快捷键/玻璃/网格/玻璃跟随/软阴影/描边/反色/翻转(含环境变量)/气泡捕获), run.m 原约 110 行块替换为一次调用; 新增 9 状态桥与 9 FFI 声明; 清理重复的 page_neighbor 声明 | cargo build/tests/parity 全绿;启动设置恢复需真机验证 | 本提交 |
| 2026-11-24 | S7cz(新目标第45轮) | 启动设置恢复(后半)迁 Swift: 新增 startup_settings_b.swift(StartupSettingsB.restorePartB: 画布三标志/菜单同步/存储载入与镜头/压感窗/GIF 参数读盘+core 区间钳制/状态图标/聊天身份与集成/录制状态机/共享上行/300ms 延迟重建), run.m 原约 120 行替换为一次调用; 新增 7 状态桥与 5 FFI 声明; 去重 pm_refresh/share_ink 声明 | cargo build/tests/parity 全绿;启动后模式/压感/GIF/聊天状态需真机验证 | 本提交 |
| 2026-11-24 | S7da(新目标第46轮) | 启动窗口/视图组装与辅助功能告警迁 Swift: 新增 startup_window.swift(StartupWindow.create: 窗口/光标/容器/玻璃/绘制视图(NSClassFromString 实例化 ObjC GlaspenDrawView)/组装/全局接管/表面确保/飘渺落位/日志; AccessibilityAlert.presentIfNeeded: 本地化弹窗、打开系统设置并重启), run.m 两段块替换为两次调用; 新增 7 全局桥 | cargo build/tests/parity 全绿;启动窗口/权限弹窗需真机验证 | 本提交 |
| 2026-11-24 | S7db(新目标第47轮) | 运行期初始化迁 Swift: 新增 startup_run_loop.swift(StartupRunLoop.begin: 2s tap 健康看门狗(AX 信任/重建/重新启用/失效提示)、性能日志/虚拟笔/翻页探针开关), run.m 看门狗块替换为一次调用; 新增 2 tap 状态桥 | cargo build/tests/parity 全绿;权限撤销恢复流程需真机验证 | 本提交 |
| 2026-11-24 | S7dc(新目标第49轮) | 面板设置应用分派迁 Swift: 新增 settings_dispatcher.swift(SettingsDispatcher.apply: core settings_apply effect JSON → 主线程按 kind 执行: 颜色/宽度/自启/玻璃/网格/跟随/阴影/反色/面板透明/描边/无限画布/minimap/尺寸/分隔/翻转/压感/GIF/聊天/共享/语言/快捷键/录制), ObjC glaspen2_macos_set_setting 仅保留 C ABI 两行转发; 新增 12 状态桥与 3 FFI 声明; D1a 闸门并入 Swift 分派源并改用引号锚 | cargo build/tests/parity 全绿;每类设置项运行时效果需真机验证 | 本提交 |
| 2026-11-24 | S7dd(新目标第50轮) | 剩余 Rust→macOS C ABI 主体迁 Swift: 新增 macos_api.swift(MacosApi: 设置快照 JSON/页刷新/无限刷新/页导航/删除页(邻居预取+切页或清屏)/热键键名分派/PDF 导出/GIF 导出+剪贴板+提示), menu.m 八个导出函数全部变一行转发; 新增 5 FFI 声明; D12 快照闸门并入 macos_api.swift | cargo build/tests/parity 全绿;面板快照/页删除/导出需真机验证 | 本提交 |
| 2026-11-24 | S7de(新目标第51轮) | 颜色/粗细菜单富文本标题迁 Swift: 新增 menu_preset_titles.swift(MenuPresetTitles: 预设色圆点/粗细横线图标 + MenuNames 名称组装), ObjC gl_color_item_text/gl_width_item_text 一行转发; 新增 2 预设数据桥 | cargo build/tests/parity 全绿;菜单图标/名称需真机验证 | 本提交 |
| 2026-11-24 | S7df(新目标第52轮) | 启动编排迁 Swift: 新增 app_bootstrap.swift(AppBootstrap.start: 探针/单实例守卫与本地化弹窗/NSApp 策略/预设/辅助功能请求/状态栏项/菜单处理器与主菜单/屏幕尺寸与 DB/两段设置恢复/窗口创建/信号安装/显示器变更观察/tap 重装/权限告警/运行期钩子), glaspen2_run 变为 start + [NSApp run] 两行; 新增 5 全局桥与 3 FFI 声明 | cargo build/tests/parity 全绿;完整启动路径需真机验证 | 本提交 |

### 迁移收尾盘点(第53轮, 2026-11-24)

| 类别 | 行数 | 说明 |
|---|---|---|
| ObjC 总计 | 3,073 | `glaspen2.m` 785 / `menu.m` 623 / `panel.m` 495 / `run.m` 1,212(含声明与注释) |
| `glaspen2_router_*` / `glaspen2_menu_*` 桥 | ~773 | Swift/ObjC 互调与 C ABI 薄桥(功能实现均已在 Swift) |
| ScreenCaptureKit 路径 | ~331 | `save_with_background`/`capture_below_own_window`、`invert_*` 捕获子系统 |
| 小胶水(≤5 行) | ~227 | 状态 setter/getter、转发函数 |
| 剩余逻辑(>5 行) | ~92 | `cairo_create_scaled`/`gl_text(f)`/dirty tracker/`save_and_exit`/`rec_json`/`event_tap_store`/`gl_panel_transparent_apply`/`pm_update`/`gl_string_setting`/录制集成回调等 |
| 其余 | ~1,650 | 全局状态声明、extern/前向声明、宏、注释 |

- 应用层**业务逻辑**已基本迁完(启动/事件/菜单/设置/画布/绘制/翻页/隧道/录制/导出/性能日志);
- 待决策: ScreenCaptureKit 需 macOS 12.3, 当前 Swift 部署目标 12.0(与 DMG `LSMinimumSystemVersion` 对齐)。
  若允许抬到 12.3, `invert_*` 与 `save_with_background` 两个子系统(~331 行)可整体迁 Swift;
  否则它们作为**平台例外**保留并写入 `docs/swift-boundary.md`。
- 彻底无 `.m`: 还需把剩余 ~773 行 C 桥与全局状态所有权逐步移入 Swift(@_cdecl 导出或去桥), 预计 10–15 轮。
| 2026-11-24 | S7dg(新目标第54轮) | 面板透明/压感标签/字符串设置读取迁 Swift: 新增 settings_panel_extras.swift(透明窗口属性、压力状态桥→MenuHandler 标签刷新), SettingsGlue 增 loadString; ObjC gl_panel_transparent_apply/pm_update/gl_string_setting 一行转发; 新增 1 压力状态桥 | cargo build/tests/parity 全绿;面板透明/压感标签需真机验证 | 本提交 |
| 2026-11-24 | S7dh(新目标第55轮) | 脏矩形跟踪器迁 Swift(首个 @_cdecl 状态所有权反迁): 新增 dirty_tracker.swift(点/表面点/表面矩形并入视图点矩形 + 状态 getter), 以 C 符号导出, ObjC 定义与全局删除、调用点不改; 桥改为读取 Swift 状态。该模式用于后续把桥/状态所有权逐步移入 Swift | cargo build/tests/parity 全绿;局部重绘脏区需真机验证 | 本提交 |
| 2026-11-24 | S7di(新目标第56轮) | 画布辅助函数反迁 Swift(@_cdecl): cairo_create_scaled/canvas_input_x,y/save_and_exit 改 Swift 实现并导出同名 C 符号, ObjC 定义删除、调用点保持(仅 cairo 返回值按生成头 void* 加 cast); 删除 window_display_id 转发改为直调 Swift 工厂; 新增 2 页面偏移桥与 shutdown FFI 声明 | cargo build/tests/parity 全绿;输入坐标/退出收尾需真机验证 | 本提交 |
| 2026-11-24 | S7dj(新目标第57轮) | 反色捕获子系统迁 Swift(无需抬最低系统): 新增 invert_capture.swift(@available(macOS 12.3,*) InvertCapture: SCStream 输出/双缓冲反相/校验和与包围盒/主线程消费/缓存与流失效/监督循环/ink pattern), 12.0–12.2 自动禁用; 删除 ObjC InvertStreamOutput 与 invert_* 约 280 行, 保留 12.3 门控桥(ink_source/apply/restart/invalidate) 与透明面板函数; glaspen2.m/InteropProbe.m 调整 Swift 头 import 顺序(SCK 类型先行); 新增 4 状态桥与 4 FFI 声明 | cargo build/tests/parity 全绿;反色模式需在 12.3+ 真机验证(SCK 无单测) | 本提交 |
| 2026-11-24 | S7dk(新目标第58轮) | 带背景保存迁 Swift(最后一条 SCK 路径): 新增 save_background.swift(SaveBackground: 主线程表面拷贝/共享态临时恢复、14+ SCScreenshotManager 捕获、12/13 dlsym CGWindowList 回退、Display P3 合成、FFI 落盘与通知), 删除 ObjC capture_below_own_window/composite_and_save/save_with_background ~230 行; 新增共享态 getter 桥; i18n 闸门修正为使用 key。至此两个 SCK 子系统均已 Swift 化(12.3/14 可用性门控) | cargo build/tests/parity 全绿;带背景截图/合成需真机验证 | 本提交 |
| 2026-11-24 | S7dl(新目标第59轮) | 死代码清理: 词边界复扫删除迁后无引用函数 tunnel_place/tunnel_draw_card/rec_simple_event/on_display_changed/peek_strokes/gl_string_setting/gl_shortcuts_load/gl_settings_snapshot_overrides_json 与 perf 三个死桥(perf_enabled/elapsed_us/perf_init)及相关声明; D12 overrides 闸门改为 Swift 源优先、ObjC 转发可选 | cargo build/tests/parity 全绿 | 本提交 |
| 2026-11-24 | S7dm(新目标第60轮) | 页视图状态所有权迁 Swift: g_pageview_* 状态移入 PageViewOps(值+访问器), update 直接写状态; 删除 9 个 ObjC getter/setter/update 桥与 ObjC 状态定义, 全 Swift 调用点改为直接访问; 修正 isFit Bool 比较 | cargo build/tests/parity 全绿;页视图几何需真机验证 | 本提交 |
| 2026-11-24 | S7dn(新目标第61轮) | 隧道状态所有权整体迁 Swift: 新增 tunnel_state.swift(表面/模糊/卡片数组/参数/定时器/回调全由 Swift 持有), 重写 animation/frame/lifecycle/geometry/card-factory 直接读写状态; 删除 ObjC s_tun_* 状态、tunnel 桥族(~40 函数)、page_flip_tunnel 桥与 draw_state/grid_state 隧道字段; overlay/flipper/ethereal/invert/grid 改用 TunnelState。ObjC 降至 2241 行 | cargo build/tests/parity 全绿;隧道动效需真机验证(尤其 120Hz 定时器/卡片内存) | 本提交 |
| 2026-11-24 | S7do(新目标第62轮) | 表面/缩放状态所有权迁 Swift: SurfaceStore 持有 current/scaleFactor, OverlayDrawing 直读表面与标志桥, 删除 g_surface/g_scale/g_has_last 与 install/has/set_has_last/draw_state 桥及声明(canvas/scale 调用点全改 Swift); menu.m 延迟重建改直调 ensure_surface。ObjC 继续下降 | cargo build/tests/parity 全绿;笔迹上屏/局部重绘需真机验证 | 本提交 |
| 2026-11-24 | S7dp(新目标第63轮) | 光标状态迁 Swift: 新增 cursor_state.swift(CursorState: 位置/可见/系统光标/penDrawing 以 C 符号导出), 删除 ObjC g_cursor_*/g_pen_drawing/系统光标全局与 cursor_draw_state/pen_drawing/blank/arrow 桥; overlay 光标层/startup 光标/EventRouter 调用点改 Swift; run.m 内部光标赋值改导出函数 | cargo build/tests/parity 全绿;笔光标显示需真机验证 | 本提交 |
| 2026-11-24 | S7dq(新目标第64轮) | 笔输入/原始笔迹/描边状态迁 Swift: 新增 pen_state.swift(PenState: raw*/strokeActive/eraser/proximity 与同名 C 导出), 笔桥族(modeler/raw/stroke/pen-state/erase/commit/prox)整体改 Swift 实现, ObjC 定义与全局删除; RawDrawing/LivePlan 改直接读 PenState; 状态位/图标/prox 桥改用导出 getter; 新增 modeler/宽度倍率声明与桥 | cargo build/tests/parity 全绿;书写/橡皮/悬停路径需真机验证 | 本提交 |
| 2026-11-24 | S7dr(新目标第65轮) | 镜头状态迁 Swift: 新增 camera_state_store.swift(CameraStore: 平移/缩放/翻页偏移/相机抓手状态 + 同名 C 导出), 删除 ObjC 镜头桥/状态与相机抓手全局; menu.m/网格状态/压力宽度改调用导出; D5b 闸门并入 camera_state_store.swift。ObjC 继续下降 | cargo build/tests/parity 全绿;镜头平移/缩放/抓手需真机验证 | 本提交 |
| 2026-11-24 | S7ds(新目标第66轮) | 绘制/网格状态迁 Swift: 新增 view_settings.swift(ViewSettings: 无限/笔迹可见/缩略图/网格显示/尺寸/分隔/跟随 + 同名 C 导出), 删除 ObjC 状态与 grid_state/infinite/strokes/grid 六类桥; 网格渲染器/状态位/tap 重装/菜单状态改直读; 恢复 GridpenDrawView 接口声明。ObjC 继续下降 | cargo build/tests/parity 全绿;网格/无限画布/飘渺显隐需真机验证 | 本提交 |
| 2026-11-24 | S7dt(新目标第67轮) | 渲染设置状态迁 Swift: 新增 render_settings.swift(RenderSettings: 玻璃开关/浓度/跟随、描边、软阴影、反色开关/帧率、翻转效果 + 同名 C 导出), 删除 ObjC 状态与相关桥(含 glass_state dict); MenuHandler 玻璃逻辑直读 RenderSettings, 菜单 getter 改调用导出; ObjC 继续下降 | cargo build/tests/parity 全绿;玻璃/阴影/描边/反色/翻转效果需真机验证 | 本提交 |
| 2026-11-24 | S7du(新目标第68轮) | 笔外观设置迁 Swift: 新增 pen_settings_store.swift(PenSettingsStore: 笔色/宽度/选中/历史/暂存索引 + apply/save 实现, 同名 C 导出), 删除 ObjC 状态(glaspen2.m 与 menu.m 两处)与约 20 个桥; 启动恢复/状态图标改调导出; 新增 save_settings FFI 声明 | cargo build/tests/parity 全绿;换色/换宽/暂存应用需真机验证 | 本提交 |
| 2026-11-24 | S7dv(新目标第69轮) | 功能开关/GIF 设置迁 Swift: 新增 toggle_settings.swift(ToggleSettings: 总开关/飘渺/压感/聊天集成/GIF 参数 + 同名 C 导出), 删除 ObjC 状态与相关桥; 录制初始化/状态位/状态图标/启动恢复改调导出。ObjC 继续下降 | cargo build/tests/parity 全绿;总开关/飘渺/压感/聊天集成/GIF 参数需真机验证 | 本提交 |
| 2026-11-24 | S7dw(新目标第70轮) | 颜色/宽度预设表与压感状态迁 Swift: 新增 preset_store.swift(由 core 填充, 导出 count/set/rgb/width)与 pressure_state.swift(压感数值/事件类型/坐标 + 刷新, 导出 sw_pm_update/sw_pm_refresh); 删除 ObjC 预设数组/结构/计数/桥与压感全局/状态桥, PenPresets/菜单标题/笔应用/压感面板改直读; ObjC 继续下降 | cargo build/tests/parity 全绿;菜单预设图标/压感监视需真机验证 | 本提交 |
| 2026-11-25 | S7dx(新目标第71轮) | 屏幕尺寸/语言状态迁 Swift: 新增 app_state.swift(AppState: 屏宽高/语言 + 同名 C 导出), 删除 ObjC 桥与全局(screen_w/h, lang, tap_has_scroll, last_x/y), gl_text/L/lang_code/面板/菜单/启动改调导出; ObjC 继续下降 | cargo build/tests/parity 全绿;语言切换/多屏尺寸需真机验证 | 本提交 |
| 2026-11-25 | S7dy(新目标第72轮) | event tap 状态/回调迁 Swift: 新增 event_tap_state.swift(tap 全局/含滚轮标志/健康查询/启停/回调路由), ObjC callback 壳缩为 2 行转发, 删除 ObjC tap 全局与 store/health 桥; 菜单/重装/收尾改调导出; ObjC 继续下降 | cargo build/tests/parity 全绿;全局事件路由需真机验证(笔/滚轮/热键) | 本提交 |
| 2026-11-25 | S7dz(新目标第73轮) | ObjC 应用/菜单 delegate 类迁 Swift: 新增 app_delegate.swift(AppDelegate: NSApplicationDelegate/NSMenuDelegate, 菜单动作逐字转发 MenuHandler, 强引用 shared), 删除 ObjC GlaspenMenuHandler 类与 g_menuHandler/注册桥; AppBootstrap 直接创建并设为 NSApp.delegate; 恢复误删的菜单 helper/桥块; ObjC 继续下降 | cargo build/tests/parity 全绿;菜单全部动作/退出流程需真机验证 | 本提交 |
| 2026-11-25 | S7ea(新目标第74轮) | 最后两个 ObjC 类迁 Swift: 新增 window_classes.swift(GlaspenDrawView: NSView draw→OverlayDrawing; SettingsWindowDelegate: windowWillClose + shared), 删除 ObjC 类/g_settings_delegate/设置 delegate 桥; StartupWindow 直接创建绘制视图, SettingsPanel 直用 delegate。**至此仓库已无任何 ObjC @implementation 类**, 剩余均为 C ABI 函数/桥/全局状态(1781 行) | cargo build/tests/parity 全绿;窗口绘制/设置窗口关闭/激活策略需真机验证 | 本提交 |
| 2026-11-25 | S7eb(新目标第75轮) | UI 锚点所有权迁 Swift: 新增 ui_anchors.swift(主窗口/绘制视图/玻璃/状态项/菜单/设置窗口 + opaque 指针导出), ObjC 侧保留 __bridge 薄 getter/setter; 删除 ObjC 锚点全局, panel/menu/run/gl_text 内部引用改桥; Flutter 引擎/VC 锚点暂留(需 Flutter 模块导入评估)。ObjC 继续下降 | cargo build/tests/parity 全绿;窗口/玻璃/状态栏/菜单/设置窗口需真机验证 | 本提交 |
| 2026-11-25 | S7ec(新目标第76轮) | Flutter 设置面板宿主迁 Swift: build.rs 给 swiftc 增加 FlutterMacOS 模块搜索路径(-F), 新增 flutter_host.swift(FlutterHost: 引擎/VC 创建持有 + opaque 导出), ObjC 桥改为 __bridge; 删除 ObjC g_flutter_engine/g_flutter_vc 全局, panel/run 透明逻辑改用 getter。验证了 Swift 可直接 import FlutterMacOS | cargo build/tests/parity 全绿;设置面板打开/透明/关闭需真机验证 | 本提交 |
| 2026-11-25 | S7ed(新目标第77轮) | i18n 与剩余标量状态迁 Swift: 新增 misc_state.swift(I18nText 查表/格式化 + sw_text 导出, StrokeContext 活动上下文 acquire/pad/present/install/take, 光标节流 flush_due/invalidate), 删除 ObjC gl_textf/L/acquire_cr/outline_pad/active_ctx_* /cursor_* 桥与 g_active_cr/g_last_cursor_flush/g_perf_log/kOutlinePad; gl_text 改调 sw_text; 活动上下文桥类型放宽为 void* 消除 InteropProbe 冲突; i18n 闸门接受 Swift 入口 | cargo build/tests/parity 全绿;书写/光标节流需真机验证 | 本提交 |
| 2026-11-25 | S7ee(新目标第78轮) | 删除 parts/panel.m(129 行): 显示尺寸/应用、ensure_surface、flush、透明、窗口共享、反色 12.3 门控桥全部改 Swift panel_glue.swift 同名 C 导出, 死包装直接删除; menu/run 调用点改 __bridge/显式转换; invert_ink_source 桥类型放宽为 void*; kOutlineDash 移 Swift; grid 闸门改扫 panel_glue+raw_drawing。ObjC 文件剩 glaspen2.m/menu.m/run.m + InteropProbe | cargo build/tests/parity 全绿;显示变更/面板透明/反色/窗口共享需真机验证 | 本提交 |
| 2026-11-25 | S7ef(新目标第79轮) | Rust FRB 反向入口迁 Swift: 新增 settings_api_glue.swift(settings_json/free/set_setting/delete/refresh/navigate/hotkey/canvas_payload/export pdf+gif 全部 C ABI 同名实现, canvas_payload 含 home/居中/新建动作与主线程同步), 删除 menu.m 中这 12 个 ObjC 定义; 镜头/设置/快照三处闸门改扫 Swift 源并收窄快照切片 | cargo build/tests/parity 全绿;面板读写设置/总览/导出需真机+面板联调 | 本提交 |
| 2026-11-25 | S7eg(新目标第80轮) | 删除 parts/menu.m(395 行): 新增 menu_runtime.swift(菜单/快捷键/飘渺/压感/设置面板运行时 + 同名 C 导出)、shortcut_store.swift(生效表状态), Swift 调用点改为 MenuRuntime/UIAnchors/ShortcutStore 直连; 菜单 delegate 桥/willOpen/didClose/quit 等全部 Swift 导出; i18n 闸门统计新入口(阈值 40→25 因 l10n 包装收敛); camera/grid/recording/settings/snapshot/i18n 闸门源改扫 Swift。ObjC 文件剩 glaspen2.m/run.m/Probe | cargo build/tests/parity 全绿;菜单/快捷键/飘渺/压感/面板需真机验证 | 本提交 |
| 2026-11-25 | S7eh(新目标第81轮) | 删除 parts/run.m(609 行): 新增 run_glue.swift(应用入口/事件 tap 回调改 Swift 闭包/镜头/光标/脏区/实时计划/录制/状态图标/飘渺/信号/窗口共享等 60+ C 导出), 对象参数桥改 Swift 直调(Flipper/RecordingEffects/NotificationPresenter/UIAnchors/FlutterHost); 事件 tap 回调不再经 ObjC; 五处闸门改扫 run_glue+相关 Swift。ObjC 文件仅剩 glaspen2.m 与 InteropProbe.m | cargo build/tests/parity 全绿;事件路由/录制/翻页/状态栏/光标需真机验证 | 本提交 |
| 2026-11-25 | S7ei(新目标第82轮) | **应用层 ObjC 清零**: 迁移 glaspen2.m 尾部 9 个函数(gl_text/通知重绘/保存/清屏/重放)后删除 glaspen2.m; 全部 Rust FFI 声明抽取为 swift/rust_ffi.h 并入 Bridging.h; 删除 InteropProbe.m/interop_probe.swift 与 --swift-probe/探针调用; build.rs 移除 clang 编译与 ObjC 归档步骤、只编 Swift(归档仍名 libglaspen2_objc.a 以少改链接); 闸门 include_str 改扫 rust_ffi.h+run_glue.swift。**src/macos 已无任何 .m/.mm 文件** | cargo build/tests(307+29+31+11+3)/parity 全绿; S8 真机验收待做 | 本提交 |

## 最终状态:S7ei(2026-11-25)— 应用层 ObjC 清零

- **src/macos 已无任何 .m/.mm 文件**;macOS 原生层 100% Swift(src/macos/swift/*.swift)。
- build.rs 只调用 swiftc 编译/归档(libglaspen2_objc.a 保留旧归档名以少改链接参数; Swift 运行时 autolink 不变)。
- 剩余跨语言面只有 src/macos/swift/Bridging.h + rust_ffi.h 的 C ABI/FFI 声明(供 Swift 调用 Rust;无 ObjC 实现)。
- 迁移总量:台账内 S0–S7ei 共 82 个目标轮;ObjC 应用层从 3,073 行(第 35 轮 checkpoint)降到 **0 行**;Swift 106 文件、约 8,100 行。
- 校验:cargo build、cargo test --workspace --exclude glaspen2_installer(307+29+31+11+3 等)、scripts/check-platform-parity.sh 全绿;D1a/D3b/D5b/D6/D7/D8b/D12/i18n 契约闸门全部改为扫描 Swift 源。

### S8 真机验收(唯一未完成项)

见 docs/s8-acceptance-checklist.md。S8 通过后方可视为迁移目标全部完成。

---

**台账规则**:本表只追加行,不改历史行;每轮 = 一次提交(中文 commit message),不 push。

| 2026-11-25 | S8-prep(新目标第83轮) | 收尾文档与 lint: README 架构图/说明改 Swift 原生层; swift-boundary.md 加最终状态注记; 新增 docs/s8-acceptance-checklist.md(19 项真机验收); 台账追加最终状态; cargo fmt + 去除 unused mut/vertbuild 变量, Rust 侧 lint/测试全绿(Flutter 面板测试失败为既有环境问题) | cargo test 全绿; lint 仅剩 Flutter settings_schema 既有失败 | 本提交 |
| 2026-11-25 | S8-prep(新目标第84轮) | Release 打包验证: scripts/build-dmg.sh 全流程通过(Flutter framework/Release 构建/App bundle/14 个 FRB 符号断言/Homebrew dylib 打包/签名/生成 DMG; 未公证因未配置证书), 产出 release_history/glaspen2-0.7.0-arm64.dmg(48M, sha256 9e9a4c…); build.rs 清理遗留 ObjC 头 emit(-emit-objc-header/断言)与过时注释, 仅保留 Bridging.h 导入; cargo build 全绿 | Release DMG 构建/签名/FRB 符号验证通过; S8 真机交互验收仍需用户执行 | 本提交 |
| 2026-11-25 | S8-prep(新目标第85轮) | 自动化冒烟与探针验收: 打包 App 冷启动成功(window ready 3440x1440 / CGEventTap created OK / cairo loaded / DB initialized / SIGTERM 优雅退出 0); 菜单探针 --swift-menu-probe/full(31 项/子菜单/action·tag·state·target 全对)与 --swift-statusbar-probe 全绿; 修复冒烟发现的迁移回归: glaspen2_virtual_pen_user_data 常量被猜错(0x6761…→正确 0x56504E31), Release 重建; 清理 main.rs 过期 InteropProbe/ObjC 注释; 重建 DMG(sha256 cb433c26…) | 启动/事件 tap/DB/退出/菜单/状态栏自动化验证通过; 虚拟笔事件在本机未被系统投递(需真笔/权限实测, 属环境限制) | 本提交 |
| 2026-11-25 | S8-prep(新目标第86轮) | 新增 scripts/smoke-macos.sh(菜单/完整菜单/状态栏三探针 + 临时库冷启动 + 事件 tap/DB/SIGTERM 优雅退出断言, 失败退出码); 更新 docs/debugging.md 断点/线程模型/发版冒烟为 Swift-only 流程并指向 S8 清单; clippy 全绿 | sh scripts/smoke-macos.sh 通过; S8 真机交互仍待用户执行 | 本提交 |
| 2026-11-25 | S8-prep(新目标第87轮) | 新增设置链路自检 `--swift-settings-probe`: 依次应用 gridSize/gridDivider/language/minimap/flipEffect/gifFps/softShadow/outline/frostedGlass/invertFps/color/width/gridFollowStrokes 并断言 Swift 状态与 settings_json 输出(15 项全过; 发现 core 对 gifFps 钳制上限 50, 探针改用合法值 30); 纳入 scripts/smoke-macos.sh。设置分派/状态存储/快照自动化覆盖到位 | 探针 15/15 通过, smoke 全绿 | 本提交 |
| 2026-11-25 | S8-prep(新目标第88轮) | 新增 `--swift-pages-probe`: 临时库初始化 → 插入页/邻居/新建页/页尺寸/空笔画/空载总览(nil)/空 canvas_payload(nil,len=0)/删除页, 9 项断言全过; 冒烟脚本给 settings/pages 探针传入独立临时 DB(避免探针写真实配置), 并把页面探针纳入 scripts/smoke-macos.sh; Release 全量 smoke 通过 | 页面导航/空画布载荷自动化覆盖; S8 真机交互仍待用户 | 本提交 |
| 2026-11-25 | S8-prep(新目标第89轮) | 新增 `--swift-render-probe`: 临时库初始化 → 模型器 begin/move/end/commit → 笔画数/点数断言 → CanvasOverview 载荷(PNG/rect/bounds)→ canvas_payload(非空,len>0,视口矩形)→ action=3 清空复位, 10 项全过; 纳入 scripts/smoke-macos.sh(独立临时 DB)。验证了 模型器→提交→总览光栅→载荷 全链路 | Release 全量 smoke(菜单/状态栏/设置/页/渲染/启动/tap/DB/退出)通过; S8 真机笔屏验收仍待用户 | 本提交 |
| 2026-11-25 | S8-prep(新目标第90轮) | 过期 ObjC 路径引用清理: 核心模块文档注释(gates 的 run/menu/panel/glaspen2.m 引用)改写为 Swift 对应文件; duplication-backlog/swift-boundary/flip-effect 文档同步; AGENTS.md 架构/命令/日志/目录速查/流程改为 Swift-only(历史踩坑标注“历史”); 历史台账与 d9d 报告保持原样 | tests/parity 全绿 | 本提交 |
| 2026-11-25 | S8-prep(新目标第91轮) | 最终打包与冒烟: HEAD 重建 DMG(sha256 5c64448340ba35301d179d440a048d209f586430aa36e443902eec8fb3533198, 48M), Release 全量 smoke(菜单/完整菜单/状态栏/设置 15 项/页导航 9 项/渲染 10 项/冷启动/tap/DB/SIGTERM 退出)全绿; 应用层 ObjC 仍为 0。S8 真笔/真屏交互验收需用户执行, 自动化无法替代 | 自动化证据齐全; 等待用户 S8 结果 | 本提交 |
| 2026-11-25 | S8-fix(实测第1项) | 修复 S8 实测: Cmd+Ctrl+D 打不开设置面板。根因: ChordNormalizer.normalize("") 返回空串而非 nil, ShortcutApply 因此把空串当成"已绑定", 默认 chord 永远补不上 → ShortcutStore.lookup 为空(12 个默认快捷键全部失效), settings 动作查不到。修复: 空/空白 raw 一律返回 nil(空 = 未绑定)。新增 `--swift-hotkeys-probe`: 断言默认表中 12 个非空 chord 全部能反查到 action 且 cmd+ctrl+D→settings, 纳入 smoke。实测 osascript 发送 Cmd+Ctrl+D 后出现「玻璃涂鸦」设置窗口 | 快捷键恢复; 全量 smoke/测试/parity 绿 | 本提交 |
| 2026-11-25 | S8-fix(实测第1项-打包) | 含快捷键修复重建 DMG: release_history/glaspen2-0.7.0-arm64.dmg(48M, sha256 85b51ea9ec2c92c2848ce0607c0c44f201b097989b4c8a27b01ceb362bce3496), 供真机复测 Cmd+Ctrl+D/其他默认快捷键 | 打包成功 | 本提交 |
| 2026-11-25 | S8-fix(实测第2项) | 用户要求把长期习惯的 ⌘⌃X(飘渺)/⌘⌃B(磨砂玻璃)恢复为 macOS 默认快捷键: core ui.rs 两动作 mac_default 由空改为 cmd+ctrl+X / cmd+ctrl+B(与 Windows ctrl+alt+X/B 对齐); README 快捷键表/未绑定清单与 Flutter schema fixture 同步更新; hotkeys probe 覆盖 12→14 个默认组合全过; 空库实测 X/B 可切换 ethereal/glass | tests/parity 全绿; 已装用户 DB 原有自定义值保持不变 | 本提交 |
| 2026-11-25 | S8-fix(第2项-打包) | 含 ⌘⌃X/⌘⌃B 默认快捷键的 DMG: sha256 e5c42654af054d515b9dd0af030504cd629606a657b576f03ab5e8afe26a7921(48M) | 打包成功 | 本提交 |
| 2026-11-25 | S8-fix(实测第3项) | 修复飘渺模式笔离开后笔迹不消失: routeProximity 现在进/出都同步 PenState.tabletProximity(退出清除), penCursorUpdate 的自动显示仅在 proximity 仍活动时执行, 避免离开后尾随 pen 事件把刚隐藏的笔迹重新显示; 新增 GLASPEN2_DEBUG_ETHEREAL 门控日志(退出前后 visible/ethereal)便于真机定位; hotkeys probe 增加 hideNow 隐藏断言 | 全量 smoke/测试/parity 绿; 需真笔复测笔离开隐藏 | 本提交 |
| 2026-11-25 | S8-fix(第3项-打包) | 含飘渺 proximity 修复的 DMG: sha256 577d434d307509417c6966d0aba4c2c2612138e227330c2cf1ebf74b6ec677e8(48M) | 打包成功 | 本提交 |
| 2026-11-25 | S8-fix(实测第4项) | 修复“不能涂鸦”: pen 迁移时 `glaspen2_router_prox_is_eraser` 把 NSPointingDeviceType.pen(1) 错当 eraser(应为 3), 导致笔一靠近即被判橡皮, 每笔都走 erase_finish, 笔迹行被软删且 0 点 -> 看起来完全画不出。修复比较值并离开 proximity 时清零 proxDevice; 虚拟笔鼠标子类型改用 `.mouseEventSubtype`(此前 rawValue 3 错误)后本机可完整自测; 新增 `--swift-persist-probe`(EventRouter 路径 + DB 点数)与 GLASPEN2_DEBUG_PEN 诊断 | 探针 PROX pen1=0/eraser3=1、DB points 正常; 需真笔复测 | 本提交 |
| 2026-11-25 | S8-fix(实测第5项) | 修复“涂鸦时多个光标残影”: 迁移时 cursor_invalidate / pen_up_state 把视图坐标(原点左下)误用 `dirty_include_surface_point`(按表面 y 向下翻转), 旧光标位置被算到屏幕另一半, 脏区漏掉 → 旧十字光标残留。两处改为 `dirtyIncludePoint` 视图坐标; 全量 tests/parity 绿 | 需真笔复测残影是否消失 | 本提交 |
| 2026-11-25 | S8-fix(实测第6项) | 修复磨砂玻璃快捷键无视觉效果: 启动恢复 `startup_glass` 无条件把透明度写成库里的 glass_alpha(用户库=0), 覆盖默认 0.45 → enabled=1 但 alpha=0。与迁移前 ObjC 一致改为仅在 opacity>0 时覆盖; hotkeys probe 增加该回归断言。实测按 ⌘⌃B 后叠加窗口像素变化 495 万(之前 0) | smoke/tests/parity 绿 | 本提交 |
| 2026-11-25 | S8-audit(迁移语义审计第1轮) | 按最近真机 bug 的模式全面比对 ObjC 迁移语义, 修掉确认的回归: ①笔按下未先收尾漏抬笔的上一笔(63f6a97 迁移丢调用); ②快捷键显式空串解绑被当缺键回退默认(c7e1e96 副作用); ③任意 mouseup 恢复系统光标被塞进 isPen 分支; ④tap-disabled/proximity 被放到禁用态早退之后; ⑤proximity“曾见过”latch 被改成“当前在感应区”(无 proximity 平板悬停失效); ⑥minimap 合成漏减 strip_y(缩略图整体下移 8pt/底部裁切); ⑦隧道落定重复执行 prepare(旧 ObjC 传 NULL, 落点卡顿); ⑧无限画布面板/跳页切换误弹 HUD; ⑨换色/换粗细历史初值 9/2 被改成 -1; ⑩菜单语言切换漏刷“更多▸无限画布”; ⑪启动恢复把画布模式菜单标题无条件覆盖成“飘渺画布”(状态反转); ⑫快捷键快照未加载时缺默认兜底; ⑬lang_code 每次 HUD 泄漏 strdup. 探针增加显式解绑/交换初值断言 | 全量 smoke/测试/parity 绿 | 本提交 |
| 2026-11-25 | 工程化(目录重构) | src/macos/swift 由 103 个平铺 .swift 重构为 7 个子系统目录(App/Menu/Input/Surface/Settings/Features/Diagnostics), 文件用 git mv 保留历史; build.rs 改为递归收集 + 断言 basename 全局唯一 + 逐文件 rerun-if-changed; 更新 Rust include_str/契约测试/i18n 豁免/文档路径; 新增 src/macos/swift/README.md 与 tests/swift_layout.rs(目录/文件唯一性/无 ObjC/头文件位置守护) | 全量 build/smoke/tests/parity 绿 | 本提交 |
| 2026-11-25 | Rust-R1 | Rust 迁移首个切片: 时光隧道几何公式从 Swift `TunnelGeometry.place` 迁入 `glaspen-core/src/tunnel.rs`(`SPAN/FRONT/place` + 5 个单测); 新增 FFI `glaspen2_tunnel_place`/`glaspen2_tunnel_front`, Swift `tunnel_frame`/`tunnel_animation` 改调 Rust, 删除 `Surface/tunnel_geometry.swift` 与 `TunnelState.span/front` 重复常量 | core 单测/build/smoke/parity 绿 | 本提交 |
| 2026-11-25 | Rust-R2 | 预设显示名表迁 Rust: `i18n::COLOR_NAMES/WIDTH_NAMES` 查询函数 + FFI `glaspen2_color_name/width_name`; Swift `MenuNames` 删表改 C 桥; i18n 闸门增加 18 个 core-served key 豁免(并校验豁免键存在); 单测覆盖 zh/en/越界 | 全量 build/tests/smoke/parity 绿 | 本提交 |
| 2026-11-25 | Rust-R3 | 性能日志从 Swift `PerfLog` 整体迁入 `glaspen-core::perf`(mach_absolute_time/时间基/文件/统计/100 条 flush), 新增 5 个 FFI(`init_from_env/is_enabled/elapsed_us/record/summary`); Swift startup/misc_state/run_glue 改调 C, 删除 `Diagnostics/perf_log.swift`; 2 个 Rust 单测 | debug/release build、全量 tests、smoke、parity 绿; `GLASPEN2_PERF_LOG=1` 真机生成 perf.log 头正常 | 本提交 |
| 2026-11-25 | Rust-R4 | 画布输入 view→canvas 变换迁 Rust: 新增 `canvas_input::infinite_input/page_input` + FFI `glaspen2_canvas_input`; Swift pen 桥一次调用取 x/y, 删除 `canvas_input_x/y` C 导出; 2 个单测 | 全量 build/tests/smoke/parity 绿; 虚拟笔真机路径 strokes+3/points+804 | 本提交 |
| 2026-11-25 | Rust-R5 | 镜头结果去掉 JSON 往返: core 新增 `glaspen2_camera_pan_values/zoom_at_values/zoom_at_center_values/centered_values`(out 参数+hit/changed), Swift CanvasCamera/settings_api_glue 改直出, 删除 `camera_state.swift`; D5b 闸门更新为新入口 | 全量 build/tests/smoke/parity 绿 | 本提交 |
| 2026-11-25 | Rust-R6 | 壳全局状态从 Swift `AppState` 迁 `glaspen-core::shell_state`(screen_w/h/lang 原子量); Rust 导出同名 C 符号 `glaspen2_router_{screen_w,screen_h,set_screen_size,lang,set_lang,startup_set_lang}`/`sw_app_screen_{w,h}`, 删除 `App/app_state.swift`; Swift 直接引用改 FFI; 1 个单测 | 全量 build/tests/smoke/parity 绿; Swift 100→99 | 本提交 |
| 2026-11-25 | Rust-R7 | 快捷键修饰键解析迁 Rust: `chord::appkit_modifier_mask`(cmd/ctrl/alt/shift → NSEvent raw 位) + FFI `glaspen2_chord_modifier_mask`; Swift 调用收进 `MenuRuntime.maskFor`, 删除 `Input/shortcut_mask.swift`; 1 个单测 | 全量 build/tests/smoke/parity 绿; Swift 99→98 | 本提交 |
| 2026-11-25 | Rust-R8 | 快捷键生效表状态迁 Rust: `shortcut_state`(values/lookup/ isEmpty/values_json) + 5 个 FFI; Swift `ShortcutStore` 退化为 JSON/NSDictionary 桥, `SettingsGlue.effectiveShortcutJSON` 直出 Rust JSON, `MenuRuntime` 空表判断改 FFI; 1 个单测 | 全量 tests/build/smoke/parity 绿; HOTKEYS_PROBE 14/14 | 本提交 |
| 2026-11-25 | Rust-R9 | 绘制/网格状态从 Swift `ViewSettings` 迁 `glaspen-core::view_state`; 17 个 C ABI 符号改 Rust 导出(含新 grid_size/divider/follow getter); run_glue/overlay_grid/menu_handler/menu_runtime/settings_probe 改 FFI, 删除 `Settings/view_settings.swift`; 1 个单测 | 全量 tests/build/smoke/parity 绿; Swift 98→97 | 本提交 |
| 2026-11-25 | Rust-R10 | 功能开关/GIF 参数从 Swift `ToggleSettings` 迁 `glaspen-core::toggle_state`; 16 个 C ABI 符号改 Rust 导出; run_glue/settings_probe 改 FFI, 删除 `Settings/toggle_settings.swift`; 1 个单测 | 全量 tests/build/smoke/parity 绿; Swift 97→96 | 本提交 |
| 2026-11-25 | Rust-R11 | 渲染设置从 Swift `RenderSettings` 迁 `glaspen-core::render_state`; 24 个 C ABI 符号改 Rust 导出; menu_runtime/menu_handler/settings_probe 改 FFI, 删除 `Settings/render_settings.swift`; 1 个单测 | 全量 tests/build/smoke/parity 绿; Swift 96→95 | 本提交 |
| 2026-11-25 | Rust-R12 | 笔外观设置从 Swift `PenSettingsStore` 迁 `glaspen-core::pen_settings`; 颜色/宽度 apply 直接用 core presets; 全部 C ABI 符号改 Rust 导出; run_glue/status icon/settings_probe/hotkeys_probe 改 FFI, 删除 `Settings/pen_settings_store.swift`; 1 个单测 | 全量 tests/build/smoke/parity 绿; Swift 95→94 | 本提交 |
| 2026-11-25 | Rust-R13 | 删除 Swift 预设缓存 `PresetStore`/`PenPresets`; 菜单预设标题/数量直接读 core `glaspen2_color_preset_*`/`glaspen2_width_preset_*`; app_bootstrap/settings_probe 去掉 loadPresets; 删除 2 个 Swift 文件 | 全量 tests/build/smoke/parity 绿; Swift 94→92 | 本提交 |
| 2026-11-25 | Rust-R14 | AppKit 壳预研: 根 crate 加 objc2 依赖, `macos_shell::glaspen2_rust_status_item_create` 用 Rust 创建 NSStatusItem; Swift StatusBarFactory 在 GLASPEN2_RUST_STATUS_ITEM=1 时使用, 默认仍 Swift/失败回退; 冒烟默认+开关均绿 | 未改变默认行为; 验证了真实 app 内 objc2 可创建/挂状态栏项 | 本提交 |
| 2026-11-25 | Rust-R15 | 覆盖窗预研: `macos_shell` 用 objc2 创建主覆盖 NSWindow(最大层级/透明/穿透/集合行为); Swift OverlayWindowFactory 在 GLASPEN2_RUST_OVERLAY=1 时使用, 默认 Swift/失败回退 | 真实 app 日志确认 Rust 创建窗口且 window ready; 默认+开关 smoke 全绿 | 本提交 |
| 2026-11-25 | Rust-R16 | 覆盖窗内容层预研: Rust 创建 layer-backed NSView 内容容器与 NSVisualEffectView 玻璃视图; OverlayWindowFactory 在 GLASPEN2_RUST_OVERLAY=1 时使用, 默认 Swift/失败回退 | 真实 app + 默认/开关 smoke 全绿; 未切默认 | 本提交 |
| 2026-11-25 | Rust-R17 | 状态栏图标渲染移植到 objc2: 18pt/72px 马克笔/橡皮/斜杠/飘渺/无限画布像素级 parity; Swift 默认仍走原实现, `GLASPEN2_RUST_STATUS_ICON=1` opt-in; `GLASPEN2_RUST_STATUS_ICON_TEST=1` 探针实测 diff=0/20736 | 全量 tests/build/smoke/parity 绿; 图标 parity 通过 | 本提交 |
| 2026-11-25 | Rust-R18 | 覆盖层内容组装迁 Rust: addSubview 顺序/绘制视图 layer 透明/setContentView/orderFront 由 objc2 完成; `GLASPEN2_RUST_OVERLAY=1` opt-in; 默认 Swift/失败回退 | 真实 app 日志确认 Rust 组装; 开关 smoke/全量 tests/build/parity 绿 | 本提交 |
| 2026-11-25 | Rust-R19 | 绘制视图迁 Rust: `define_class!` NSView 子类 + drawRect C 桥回调 Swift OverlayDrawing; `GLASPEN2_RUST_DRAW_VIEW=1` opt-in; 虚拟笔绘制截图确认旧笔迹/新笔迹均正常渲染 | 默认/绘制视图/覆盖窗+绘制视图组合 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-25 | Rust-R20 | 状态栏项创建与图标渲染默认切 Rust(objc2): 默认路径走 Rust, GLASPEN2_SWIFT_STATUS_ITEM=1/GLASPEN2_SWIFT_STATUS_ICON=1 可回退 Swift; 失败自动回退; parity 72x72 diff=0/20736 | 默认 smoke(含状态栏)/回退 smoke/全量 tests/build/parity 绿 | 本提交 |
| 2026-11-25 | Rust-R21 | 覆盖层默认切 Rust: 主窗口/内容容器/玻璃视图/组装/绘制视图默认 objc2; GLASPEN2_SWIFT_OVERLAY=1 + GLASPEN2_SWIFT_DRAW_VIEW=1 回退 Swift; 失败自动回退 | 默认 smoke + 回退 smoke、虚拟笔截图、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-25 | Rust-R22 | 状态栏菜单对象迁 Rust(objc2): Rust 创建 NSMenu + 挂到 NSStatusItem; 菜单项仍 Swift 构建; GLASPEN2_SWIFT_MENU=1 回退; 默认 smoke/回退 smoke/全量 tests/build/parity 绿 | 默认启动日志确认 Rust 创建并挂载菜单 | 本提交 |
| 2026-11-25 | Rust-R23 | 颜色/粗细菜单项批量构建迁 Rust(objc2): NSMenuItem/attributedTitle/target/action/tag; 富文本标题仍 Swift; GLASPEN2_SWIFT_MENU_ITEMS=1 回退; --swift-menu-probe 默认 Rust SWIFT_MENU_OK | 默认/回退 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-25 | Rust-R24 | 顶层其余项/导出▸/更多▸ 子菜单构建迁 Rust(objc2); 标题仍 Swift 本地化; GLASPEN2_SWIFT_MENU_FULL=1 回退; --swift-menu-full-probe 默认 Rust SWIFT_MENU_FULL_OK(31 项/分隔符/子菜单全对) | 默认/回退 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-25 | Rust-R25 | 菜单勾选更新迁 Rust(objc2): 按颜色/粗细索引设置 NSMenuItem.state; --swift-menu-full-probe 增加 CHECKMARKS 断言; GLASPEN2_SWIFT_MENU_STATE=1 回退 | 默认/回退 smoke、全量 tests/build/parity 绿; probe SWIFT_MENU_FULL_OK | 本提交 |
| 2026-11-25 | Rust-R26 | 菜单动态文案刷新迁 Rust(objc2): 颜色/粗细 attributedTitle、顶层/导出/更多 title、tag 778/888 文案; probe 增加 TEXTS 断言; GLASPEN2_SWIFT_MENU_TEXTS=1 回退 | 默认/回退 smoke、全量 tests/build/parity 绿; probe SWIFT_MENU_FULL_OK | 本提交 |
| 2026-11-25 | Rust-R27 | 菜单项递归查找/设置标题/读 state 迁 Rust(objc2); Swift MenuLookup 退化为指针桥; GLASPEN2_SWIFT_MENU_LOOKUP=1 回退 | 默认/回退 smoke、全量 tests/build/parity 绿; menu probe SWIFT_MENU_FULL_OK | 本提交 |
| 2026-11-25 | Rust-R28 | 光标壳迁 Rust(objc2+CoreGraphics): 空白/箭头 NSCursor、光标环/点/十字线绘制; 默认 Rust, GLASPEN2_SWIFT_CURSOR=1 回退; 真实 app+虚拟笔/默认/回退 smoke 全绿 | 未做像素 parity(光标动态绘制), 行为路径验证 | 本提交 |
| 2026-11-25 | Rust-R29 | 设置面板窗口创建迁 Rust(objc2): 标题/尺寸/样式/内容 VC/激活/居中; delegate/Flutter host 仍 Swift; 默认 Rust, GLASPEN2_SWIFT_SETTINGS_PANEL=1 回退 | 热键打开面板截图确认正常; 默认/回退 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-25 | Rust-R30 | 笔迹 surface 合成迁 Rust(CoreGraphics): 小脏矩形 crop/绘制/回退全图; CGImage 缓存仍 Swift; GLASPEN2_RUST_SURFACE=1 opt-in, 虚拟笔截图确认旧/新笔迹正常 | 默认/opt-in smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-25 | Rust-R31 | surface 合成默认切 Rust; 网格三层线从 Swift JSON 解析改为 Rust 直接 core::grid+CGContext 描边并默认启用; 回退开关 GLASPEN2_SWIFT_SURFACE/GRID=1 | 真实 app 开网格+虚拟笔截图确认网格/笔迹正常; 默认/回退 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-25 | Rust-R32 | 时光隧道 surface 合成迁 Rust(CoreGraphics): CGImage 缓存 + drawImage; 默认 Rust, GLASPEN2_SWIFT_TUNNEL=1 回退; 翻页接管日志正常、默认/回退 smoke 全绿 | 页面内容少时隧道 0 卡, 但绘制路径已接 | 本提交 |
| 2026-11-25 | Rust-R33 | 通知 HUD 文字绘制迁 Rust(objc2): 36pt 等宽/白字/阴影/居中; 新增 --swift-notification-probe(离屏渲染非空像素 15309)并纳入 smoke; 默认 Rust, GLASPEN2_SWIFT_NOTIFICATION=1 回退 | 默认/全量 smoke、tests/build/parity 绿 | 本提交 |
| 2026-11-25 | Rust-R34 | 笔迹 surface CGImage 缓存迁 Rust(替代 Swift OverlaySurfaceImageCache); OverlayDrawing 默认走 glaspen2_rust_draw_surface_data, Swift 旧缓存仅回退; 虚拟笔截图确认旧/新笔迹正常 | 默认/回退 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-25 | Rust-R35 | 设置窗口 delegate 迁 Rust: define_class! + NSWindowDelegate.windowWillClose → Accessory/deactivate; 安装日志确认; Swift delegate 仅回退路径 | 默认/回退 smoke、全量 tests/build/parity 绿; 关闭动作未在自动化中点击(需真机) | 本提交 |
| 2026-11-26 | Rust-R36 | 颜色/粗细选择状态机迁 Rust: 书写中 pending、prev 记录、apply/save/checkmarks/status icon/panel sync; Swift MenuHandler 仅回退; settings probe color/width ok | 默认/回退 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R37 | 聊天/压感/网格/自启/玻璃开关 setter 迁 Rust: 状态/落库/面板/菜单/重绘; Swift MenuHandler 仅回退; settings probe 全绿 | 默认/回退 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R38 | OverlayDrawing 编排迁 Rust: Swift stage 桥 + Rust save/clip/顺序/restore/perf; GLASPEN2_RUST_OVERLAY_DRAW=1 opt-in; 网格+虚拟笔截图正常 | 默认/opt-in smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R39 | OverlayDrawing 编排默认切 Rust(save/clip/顺序/restore/perf), GLASPEN2_SWIFT_OVERLAY_DRAW=1 回退; 默认截图网格+笔迹正常 | 默认/回退 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R40 | Minimap strip 合成/绘制迁 Rust: 布局/缩略图贴图/当前页边框/标签/整体贴图; 缩略图生成仍 Swift; GLASPEN2_SWIFT_MINIMAP_DRAW=1 回退; minimap 截图正常 | 默认/回退 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R41 | 脏矩形跟踪迁 Rust: 复位/点/表面点/表面矩形并集/has/rect; run_glue 改调 Rust; 删除 Input/dirty_tracker.swift; 虚拟笔截图正常 | 默认 smoke、全量 tests/build/parity 绿; Swift 92→91 | 本提交 |
| 2026-11-26 | Rust-R42 | 磨砂玻璃应用逻辑迁 Rust: enabled/opacity/follow/飘渺 hidden 组合 + apply glass view + menu 444; Swift 仅回退; 默认/回退 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R43 | 涂鸦/描边菜单动作迁 Rust: 状态/菜单标题/勾选/通知/finish stroke/rebuild; 文案 Swift 传入; settings probe draw/outline 断言通过 | 默认/回退 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R44 | PageViewOps fit 状态/几何/update 迁 Rust; Swift 只做 C 桥与重建/翻页动作; 虚拟笔截图正常 | 默认 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R45 | 菜单 C ABI 薄桥迁 Rust: update_checkmarks/update_menu_state/menu_state/glass_apply/set_title; 删 Swift 方法/C 包装; MenuGlue 改调 Rust; menu-full/settings probe 通过 | 默认 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R46 | CameraStore 平移/缩放/页偏移/拖动/最后点状态迁 Rust; 删 camera_state_store.swift; queue/flush 桥移入 camera_mouse_pan; 虚拟笔截图正常 | 默认 smoke、全量 tests/build/parity 绿; Swift 91→90 | 本提交 |
| 2026-11-26 | Rust-R47 | 光标状态 x/y/visible/penDrawing/lastFlush 迁 Rust; NSCursor 实例留 Swift; 调用点改读 Rust; 虚拟笔截图正常 | 默认 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R48 | PenState raw last/prev/has/pathLen 迁 Rust; clearRaw/updateAfterSegment 走 Rust; Swift 计算属性保持 API; 虚拟笔截图正常 | 默认 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R49 | PressureState 压力/坐标/接触/接近/事件类型迁 Rust; Swift 计算属性 + 刷新桥; 压感窗标签真实更新 | 默认 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R50 | ShortcutApply 默认/迁移/保留/冲突解绑迁 Rust; Swift 仅 JSON 桥; hotkeys probe 14 项通过 | 默认 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R51 | 事件 tap 句柄(CFMachPort retain/release/invalidate)+scroll 标志迁 Rust; 删 event_tap_state.swift; EventTapInstaller 改调 Rust; smoke tap 检查通过 | 默认 smoke、全量 tests/build/parity 绿; Swift 90→89 | 本提交 |
| 2026-11-26 | Rust-R52 | 设置快照 overrides JSON 组装迁 Rust(launchAtLogin/shortcuts/flipEffect/language); Swift 仅返回 Rust 字符串; settings probe 通过 | 默认 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R53 | StrokeContext 活动 cairo context 槽 + cursorFlushDue 判定迁 Rust; Swift 仅保留 I18n/cursor invalidate 等; 虚拟笔截图正常 | 默认 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R54 | 录制 HUD 13 种语义 -> core i18n 文案/占位符/detail 覆盖迁 Rust; Swift 仅通知展示; 全量 tests/build/smoke/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R55 | LivePlan plan JSON 解析 + cairo 光栅化(今 shadow/outline/erase/fill/seam/dirty)迁 Rust; 默认 Rust, GLASPEN2_SWIFT_LIVE_RASTER=1 回退; 虚拟笔截图正常 | 默认/回退 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R56 | RawDrawing 首点/拖动(plan 调用 + 旧分档回退光栅化)迁 Rust; Swift 仅表面门控; 虚拟笔截图正常 | 默认 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R57 | PenState 书写中/橡皮/接近/设备状态迁 Rust; Swift 同 API 计算属性; 虚拟笔截图正常 | 默认 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R58 | CanvasMode.applyInfinite 状态/落库/镜头/infinite strokes/tap 重装/通知迁 Rust; Swift 仅回退; 真机无限画布切换待验 | 默认/回退 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R59 | 默认 Rust 壳性能抽查(release, 3 次): CPU 10.8–13.6%, RSS 331–335MB, 启动 0.20–0.28s; 与 Swift live/overlay 回退同噪声区间 | 仅抽查, 非真机笔/多屏长测 | 本提交 |
| 2026-11-26 | Rust-R60 | DrawMode.toggleEnabled 窗口显隐/压感窗/状态/通知/菜单迁 Rust; Swift 仅回退; 热键触发未自动化, 待真机 | 默认/回退 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R61 | penDown/penUp/penMove/updateEraserState 模型器/描边/raw/提交/状态动作迁 Rust; Swift 仅事件分派与回退; 虚拟笔截图正常 | 默认/回退 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R62 | CanvasCamera 平移/缩放/applyTransform/持久化/load/reset/home 迁 Rust; Swift 仅回退; 缩放热键未自动化, 待真机 | 默认/回退 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R63 | quitAppNow(NSApp terminate/perf/cursor)与压感标签格式化迁 Rust; 压感窗标签真实更新; Swift 仅回退 | 默认/回退 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R64 | finishActiveStroke/restoreSystemCursor 迁 Rust; 虚拟笔/退出路径正常; Swift 仅回退 | 默认/回退 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R65 | 删除无调用方 chord_normalizer.swift; 追加残余 Swift 清单(88 文件)与默认 Rust 真机验收清单; Swift 89→88 | 构建/smoke/tests/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R66 | 删 cursor_state.swift(死字段)与 StartupWindow 死赋值; Swift 88→87; 构建/smoke/tests/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R67 | LivePlan 入参 JSON/core 调用/plan 光栅化整链迁 Rust; 删 live_plan.swift 与 C 包装; 虚拟笔截图正常; Swift 87→86 | 默认 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R68 | 删除 shortcut_apply.swift; JSON 桥移入 SettingsGlue.applyShortcuts; hotkeys probe 14/14; Swift 86→85 | 默认 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R69 | 删除无调用方 blank/arrow 光标工厂(Swift+Rust)与 StartupWindow 死变量; 构建/smoke/tests/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R70 | RecordingHotkeys 判定 + 事件 JSON 迁 Rust; 通过桥交 RecordingEffects; Swift 仅回退; 录制链路待真机 | 默认/回退 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R71 | 完成标准评估: tag ce525db 未动/未 push/ObjC 0/Swift 85; tests/build/smoke/parity+探针全绿; 判定纯逻辑/状态迁移基本达成, 剩余 Swift 为平台壳/绘制/效果 | 继续迁移项已列 | 本提交 |
| 2026-11-26 | Rust-R72 | 60s 虚拟笔+翻页+网格+minimap 浸泡: 进程存活, strokes +10, RSS 380MB 平台化, CPU 0–6%; 2 条 modeler Up error 待真机 | 未 push/tag 未动 | 本提交 |
| 2026-11-26 | Rust-R73 | CanvasOverview 载荷几何/渲染迁 Rust; Swift 仅 Data 包装; render probe overview 全通过 | 默认 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R74 | ActionDispatch effect 分派迁 Rust; 键盘路径默认 Rust; 【flip】日志验证翻页动作生效; Swift 仅回退 | 默认/回退 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R75 | 清理已删除的 live plan C 包装声明(Rust extern + Bridging.h); 构建通过 | 本提交 |
| 2026-11-26 | Rust-R76 | 最终验证快照: tag 未动/未 push/ObjC 0/Swift 85; tests/build/smoke/parity 全绿; 干净 DB 四探针全过; render 探针在旧 DB 上因 stroke_count 假定失败(非回归); 目标保持 active | 剩余项已列 | 本提交 |
| 2026-11-26 | Rust-R77 | 虚拟键码名表迁 Rust; 删 shortcut_names.swift 与旧包装; 真机翻页日志验证; Swift 85→84 | 默认 smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R78 | 第80轮收尾快照: tag ce525db 未动/未 push/ObjC 0/Swift 84; tests/build/smoke/parity + 干净 DB 四探针全绿; 汇总77批与剩余项; 目标保持 active | 待后续/真机验收 | 本提交 |
| 2026-11-26 | Rust-R79 | TunnelCardRenderer 卡片 cairo 绘制迁 Rust(opt-in GLASPEN2_RUST_TUNNEL_CARD=1); 新增卡片像素探针并入 smoke; 默认仍 Swift; 真机隧道场景待验 | 默认/opt-in smoke、全量 tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R80 | glass view alpha/hidden 应用迁 Rust(sw_glass_view 桥); 删 MenuRuntime 旧包装; smoke/tests/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R81 | 修复隧道建卡预算从 tb0→tc1(预载慢时 0 卡); 真机 2 页内容翻页得到 5 卡隧道; Rust card opt-in 无崩溃 | 默认 smoke/tests/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R82 | 隧道卡片绘制默认切 Rust; Rust/Swift 像素 parity diff=0/43200; GLASPEN2_SWIFT_TUNNEL_CARD=1 回退; 默认/回退 smoke 绿 | 本提交 |
| 2026-11-26 | Rust-R83 | 隧道逐帧编排(ease/排序/落定/模糊/近层)迁 Rust, 复用 Rust 卡片; opt-in GLASPEN2_RUST_TUNNEL_FRAME=1; 真机5卡隧道无崩溃; 默认仍 Swift | 默认/opt-in smoke、tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R84 | 隧道逐帧编排默认切 Rust; 整帧 parity diff=0/120000; GLASPEN2_SWIFT_TUNNEL_FRAME=1 回退; 默认/回退 smoke 绿 | 本提交 |
| 2026-11-26 | Rust-R85 | 默认 Rust 隧道 5 卡真机运行无崩溃; 截图未稳定捕获动画帧, 视觉待人工复验 | 全量验证绿 | 本提交 |
| 2026-11-26 | Rust-R86 | TunnelLifecycle.freeCards 销毁迁 Rust, Swift 清状态桥; opt-in GLASPEN2_RUST_TUNNEL_LIFECYCLE=1; 真机5卡隧道无崩溃 | 默认/opt-in smoke、tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R87 | 隧道卡片释放默认切 Rust; GLASPEN2_SWIFT_TUNNEL_LIFECYCLE=1 回退; 真机5卡隧道无崩溃; 默认/回退 smoke 绿 | 本提交 |
| 2026-11-26 | Rust-R88 | 隧道 finish(cancel/final frame/FPS/free/glass)迁 Rust; 默认 Rust, GLASPEN2_SWIFT_TUNNEL_LIFECYCLE=1 回退; 真机5卡 FPS 56.4/freed5 无崩溃 | 默认/回退 smoke、tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R89 | 常用设置 effect 分派迁 Rust; Swift 仅回退; settings probe 默认/回退 failures=0; 全量验证绿 | 本提交 |
| 2026-11-26 | Rust-R90 | 快照页框绘制迁 Rust; 像素 parity diff=0/60000; GLASPEN2_SWIFT_PAGEFRAME=1 回退; parity 并入 smoke | 默认/回退 smoke、tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R91 | 整页重建/重放编排迁 Rust; 默认 Rust, GLASPEN2_SWIFT_SURFACE_REBUILD=1 回退; smoke/tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R92 | 快捷键 chord 门控/动作执行迁 Rust; 默认 Rust, GLASPEN2_SWIFT_CHORD=1 回退; 真机翻页验证; smoke/tests/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R93 | 相机抓手/滚轮缩放分发迁 Rust(opt-in GLASPEN2_RUST_CAMERA_MOUSE=1); 默认 Swift; 默认/opt-in smoke 绿 | 本提交 |
| 2026-11-26 | Rust-R94 | 清屏/保存涂鸦截图迁 Rust; 默认 Rust, GLASPEN2_SWIFT_SURFACE_OPS=1 回退; render probe clear_action 通过 | 默认/回退 smoke、tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R95 | SurfaceStore 表面指针/scale 状态迁 Rust; Swift 计算属性; 虚拟笔截图正常; smoke/tests/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R96 | SurfaceStore.ensure 创建/尺寸缓存/销毁/透明填充迁 Rust; 默认 Rust, GLASPEN2_SWIFT_SURFACE_ENSURE=1 回退; 虚拟笔截图正常 | 默认/回退 smoke、tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R97 | StrokeSurface.begin/end 迁 Rust; Rust 直接导出 C 符号; 删 stroke_surface.swift; Swift 84→83; 虚拟笔截图正常 | 默认 smoke、tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R98 | CameraMousePan 合帧状态/flush 迁 Rust; Swift 仅定时器壳; 默认 Rust, GLASPEN2_SWIFT_CAMERA_PAN=1 回退; 默认/回退 smoke 绿 | 本提交 |
| 2026-11-26 | Rust-R99 | HotkeyRecorder recording/generation/60s 超时迁 Rust; Swift 仅定时器壳; 默认 Rust, GLASPEN2_SWIFT_HOTKEY_RECORDER=1 回退; smoke/tests/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R100 | 显示器变更 2.5s 防抖/重读/apply 迁 Rust(opt-in); Swift 仅定时器/NSScreen 桥; 默认/opt-in smoke 绿 | 真机多屏待验 | 本提交 |
| 2026-11-26 | Rust-R101 | 删除 MenuNames 薄桥, 颜色/粗细名改 MenuRuntime 直调 core; Swift 83→82; smoke/tests/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R102 | EtherealMode 隐藏/恢复/预览显示迁 Rust; peek 定时器留 Swift; 默认 Rust, GLASPEN2_SWIFT_ETHEREAL=1 回退 | 默认/回退 smoke、tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R103 | 删除 SettingsDispatcher 回退桥, 设置入口直调 Rust; Swift 82→81; settings probe failures=0 | 默认 smoke、tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R104 | 删除 RawDrawing/RecHudPresenter 薄桥(内联/移入 MenuRuntime); Swift 81→79; smoke/tests/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R105 | 删除 MenuGlue/SettingsPanelExtras/GifClipboard 薄桥; Swift 79→76; smoke/tests/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R106 | cairo_create_scaled/save_and_exit 迁 Rust; 删 canvas_helpers.swift; Swift 76→75; smoke/tests/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R107 | 快捷键默认/动作 JSON 缓存迁 Rust; 删 shortcut_table.swift; Swift 75→74; hotkeys probe 通过 | 默认 smoke、tests/build/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R108 | 合并 MenuPresetTitles/PressureState 到 MenuRuntime/PressureMonitorWindow; Swift 74→72; smoke/tests/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R109 | 删除 MenuLookup, tag 查找移入 MenuRuntime; Swift 72→71; smoke/tests/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R110 | 合并 CanvasOverview 到 SettingsGlue; 删除 canvas_overview.swift; Swift 71→70; smoke/tests/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R111 | 图层面全量/脏矩形刷新迁 Rust; 删 overlay_layer_flush.swift; Swift 70→69; smoke/render probe/tests/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R112 | 修复隧道整帧 parity 探针自比较(补 FRAME 回退 env); diff=0/120000 重新有效; smoke/tests/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R113 | 清理 SurfaceStore/PageViewOps/SurfaceReplay Swift 回退; 仅保留 Rust 调用; smoke/tests/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R114 | 删除 chord Swift 回退与 keycodeName 桥; Rust 唯一路径; 真机翻页正常; smoke/tests/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R115 | 清理 CanvasCamera Swift 回退, 仅留 ObjC 入口; 文件 150→50 行; smoke/tests/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R116 | 第38轮快照: 四探针/全量验证绿; tag ce525db; Swift 69; ObjC 0; 未 push | 本提交 |
| 2026-11-26 | Rust-R117 | 清理 TunnelLifecycle Swift 回退; 仅留 Rust+闭包; 真机5卡/freed5 正常; smoke/tests/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R118 | 第40轮最终快照: 全量+四探针+像素 parity 绿; tag ce525db; Swift69/ObjC0; 目标未整体完成, 保持 active | 本提交 |
| 2026-11-26 | Rust-R119 | 录制 feed/effect 执行迁 Rust(ShowHud/GIF/剪贴板/聊天); Swift 仅 3 桥; 删 recording_effects.swift; Swift 69→68; smoke/tests/parity 绿 | 真机录制待验 | 本提交 |
| 2026-11-26 | Rust-R120 | 笔事件分派/压感监视器分类迁 Rust; EventRouter 方法变薄; 虚拟笔存活; smoke/tests/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R121 | proximity 分派迁 Rust; 删 restore/finish 旧回退; 虚拟笔存活; smoke/tests/parity 绿 | 本提交 |
| 2026-11-26 | Rust-R122 | 笔光标更新/系统光标隐藏迁 Rust; debug build 通过(加速批次) | 本提交 |
| 2026-11-26 | Rust-R123 | 主路由鼠标移动/抬起/菜单栏/停用清理迁 Rust; debug build 通过(加速批次) | 本提交 |
| 2026-11-26 | Rust-R124 | 键盘事件顶层路由(录制热键+chord)迁 Rust; debug 批次后全量 tests/smoke/parity 绿(不跑 release) | 本提交 |
| 2026-11-26 | Rust-R125 | 相机鼠标/滚轮默认切 Rust, 删 Swift 旧实现; debug build 通过 | 本提交 |
| 2026-11-26 | Rust-R126 | 清理 penDown/up/move/eraser Swift 回退; debug build 通过 | 本提交 |
| 2026-11-26 | Rust-R127 | 主路由笔事件块迁 Rust; 批次 tests/smoke/parity 绿(不跑 release) | 本提交 |
| 2026-11-26 | Rust-R128 | 清理网格/光标/surface Swift 回退; debug build 通过 | 本提交 |
