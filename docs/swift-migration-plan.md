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
| S1  | 构建集成 + C ABI 桥 | build.rs 加 swiftc 编 .swift→.o 并链接;Bridging-Header;选 1 个无状态 shim(如 glaspen2_macos_open_url)用 Swift @_cdecl 重实现,符号不变      | cargo build 通过;Rust 调 Swift 行为一致;记录 swiftc 版本/部署目标/rpath;可回滚        | 2–4 d | ⬜ TODO |
| S2  | 边界设计            | 定义 @objc 兼容 API 白名单(类/selector/回调/C 类型);禁止 Swift-only 类型跨界;ObjC→Swift 文件映射;事件 tap 的 @convention(c)+Unmanaged 方案 | 编译通过:空 Swift 类被 ObjC 调用 + Swift 调 ObjC 函数;设计写入本文件                  | 1–2 d | ⬜ TODO |
| S3  | 菜单栏迁移          | menu.m:状态栏项/菜单构建/动作/动态重建/语言切换/图标;保留 C shim;旧实现 fallback 一个版本                                                  | 菜单项/图标/深浅色/语言/点击/快捷键与旧版逐项一致(截图);flag off 可回滚               | 3–5 d | ⬜ TODO |
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

## 7. 相关文档

- docs/architecture.svg(当前架构)
- docs/duplication-backlog.md(D14 防重复架构、各单源模块)
- docs/d9d-verification.md(绘制验证与 Windows 延期记录)
- AGENTS.md(仓库规则/构建/踩坑)
| 2026-11-23 | S0 | 状态栏菜单 Swift 原型完成(NSStatusItem/NSMenu/attributedTitle 图标/target-action);编译 0.82s、71 行、60KB 独立产物,运行输出全部成功(3 菜单项/图标长度 1/selector 响应/action save 触发);未接 build.rs/ObjC 互调/动态重建(留 S1/S3) | swiftc -O 编译 + 2s 运行验证;报告 docs/swift-s0-spike-report.md | spike 分支 228213a |
