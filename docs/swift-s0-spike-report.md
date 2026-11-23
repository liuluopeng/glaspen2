# S0 spike 报告:Swift 状态栏菜单原型(2026-11-23)

> 性质:限时抛弃型实验,只回答「AppKit 菜单栏能否用 Swift 写、成本如何」。
> 代码在分支 spike/swift-status-item,commit 228213a,不合并。

## 环境

- macOS 27.2 arm64;Xcode at /Applications/Xcode.app
- Apple Swift 6.4.2(swift-driver 1.168.6)
- 基线:v0.7.0 后的 main(c301fab)

## 原型内容(71 行 Swift)

文件:spikes/swift-status-item/status_item.swift(分支上)

- NSStatusBar.system.statusItem + NSStatusItem.button
- attributedTitle + NSTextAttachment 内嵌 NSImage(复刻现有 ObjC 菜单图标 hack)
- NSMenu + 3 个 NSMenuItem(保存/设置/退出),target/action 指回 Swift @objc 方法
- 用 NSApp.sendAction 程序化触发菜单项,验证 target/action 路由
- 2 秒后自动退出(不常驻,避免影响桌面)

## 运行命令与实测输出

编译:swiftc -O -o /tmp/glaspen2-s0-spike status_item.swift -framework AppKit

```
SPIKE_STATUS_ITEM ok
SPIKE_MENU_ITEMS
3
SPIKE_ICON_TITLE_LEN
1
SPIKE_SELECTORS
1
1
SPIKE_ACTION save
SPIKE_EXIT
```

## 指标

| 项 | 值 |
|---|---|
| 源码行数 | 71 |
| @objc 方法 | 3 |
| guard | 1 |
| 编译耗时 | 0.82 s(单文件,swiftc -O) |
| 产物大小 | 60 KB(独立可执行) |
| 运行时 | 状态项创建、3 菜单项、图标 attributedTitle 长度 1、selector 响应、action 触发全部成功 |

## 证明了什么

- Swift 直接使用 AppKit 的 NSStatusItem/NSMenu/target-action 没有障碍;
- 现有 attributedTitle + NSTextAttachment 图标 hack 可以 1:1 复刻;
- 菜单动作路由可以程序化验证;
- 单文件编译快、无 unsafe、无需桥接头(Swift 内部)。

## 没有证明什么(S1 才验证)

- build.rs 里 swiftc 编 .swift→.o 并链接进 Rust 主程序(含 rpath/Swift runtime/部署目标);
- Swift 代码与现有 ObjC 代码互调(bridging header / -Swift.h / @objc 边界);
- 菜单动态重建、语言切换、多级子菜单、图标在深浅色/不同系统版本下的表现;
- 事件 tap、窗口、cairo 绘制(本 spike 完全没碰);
- 打包(DMG)与最小系统版本;当前 swiftc 默认 target 是宿主 27.2,必须在 S1 显式设部署目标。

## 结论:GO S1(有限范围)

S1 只做三件事,任一失败即停:

1. build.rs 增加 swiftc 编译步骤与链接(保留 clang 编 ObjC);
2. 建 bridging header,让 Swift 调一个现有 ObjC 函数、ObjC 调一个 Swift 类;
3. 选一个无状态 C shim(建议 glaspen2_macos_open_url)用 Swift @_cdecl 重实现,符号/签名不变,
   由 Rust 侧现有调用验证行为一致;跑 cargo build / 全量测试 / parity。

S1 通过 → S2 边界设计 → S3 菜单栏正式迁移(旧实现 fallback 一个版本)。
S1 失败(build/CI 不稳定或互调别扭)→ 回滚,结论改为「菜单继续 ObjC」,迁移计划暂停。

## 复现

```
git switch spike/swift-status-item
swiftc -O -o /tmp/glaspen2-s0-spike spikes/swift-status-item/status_item.swift -framework AppKit
/tmp/glaspen2-s0-spike
git switch main
```
