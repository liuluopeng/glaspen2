# Swift 渐进迁移计划(可续做台账)

> 用法: 这是持久台账,不是一次性方案。开始/完成任何阶段必须改状态并追加进度记录;
> 中途停下要保持工作区干净、已提交,并在进度表写清卡点与下一步。
> 基线: v0.7.0(tag, commit ce525db, 2026-11-23)。迁移从未开始。

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
| S3b | 菜单构建/动作迁移    | menu.m:菜单项/动作/动态重建/语言切换/图标;保留 C shim;旧实现 fallback 一个版本 | 菜单项/图标/深浅色/语言/点击/快捷键与旧版逐项一致(截图);flag off 可回滚 | 3–5 d | 🟨 DOING(S3b-1 已完成: 颜色圆点/粗细横线图标迁 Swift 且 TIFF 逐字节 parity;剩菜单结构/动作/动态重建) |
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
