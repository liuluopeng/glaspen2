# macOS Swift → Rust 渐进迁移计划

> 目标: 在不改变用户可见行为、不破坏 C ABI 的前提下, 把 macOS 应用层从 Swift 逐步迁到 Rust。
> 每轮 = 一个可独立验证/回滚的小切片; 未验证前 Swift 实现保留。

## 原则

1. **先纯逻辑, 后 UI**: 数学/状态机/FFI 适配先迁; AppKit 窗口/菜单/Flutter 宿主最后迁。
2. **C ABI 不变**: Swift 通过 `Bridging.h`/`rust_ffi.h` 调 Rust; 符号名/签名保持。
3. **一处逻辑只有一个来源**: 迁完删除 Swift 实现和重复常量, Rust 单测 + 现有 Swift 探针/冒烟验证。
4. **每切片必须绿**: `cargo build`、相关 `cargo test`、`scripts/smoke-macos.sh`、`scripts/check-platform-parity.sh`。
5. **可回退**: 不删 Swift 调用点之外的行为; 出问题可只 revert 一个 commit。

## 已完成

- **R0 可行性验证(仓库外)**: `rust-appkit-overlay` 纯 Rust + objc2 实现
  顶层透明窗 + `CGEventTap` 消费笔事件 + `NSView` 绘制; 自检证明普通鼠标可穿透/笔不穿透。
- **R1 时光隧道几何**: `crates/glaspen-core/src/tunnel.rs`(SPAN/FRONT/place) + FFI;
  Swift `tunnel_frame`/`tunnel_animation` 改调 Rust; 删除 `Surface/tunnel_geometry.swift`。
- **R2 预设显示名**: 颜色/粗细显示名表与 i18n 查询迁入 `glaspen-core::i18n::color_name/width_name`;
  新增 FFI `glaspen2_color_name/width_name`; Swift `MenuNames` 只留 C 字符串桥; i18n 闸门增加 core-served 豁免。
- **R3 性能日志**: `PerfLog` 整体迁入 `glaspen-core::perf`(mach 时间/文件/统计), 新增 FFI;
  Swift 调用点改 C, 删除 `Diagnostics/perf_log.swift`。
- **R4 画布输入变换**: `canvas_input_x/y` 公式迁入 `glaspen-core::canvas_input`;
  新增 FFI `glaspen2_canvas_input`(一次输出 x/y), Swift 笔桥改调 Rust, 删除旧 C 导出; 2 个单测。

## 下一批候选(按风险从低到高)

- R2-a 快捷键掩码/显示名表等纯映射(需评估是否值得为薄表增加 FFI)。
- R2-b 画布坐标变换 `canvas_input_x/y`(纯公式, 但位于笔事件热路径, 需基准)。
- R2-c 设置/渲染状态存储(消除 Swift/Rust 双份状态, 需改多处调用点)。
- R3 AppKit 壳(状态栏/菜单/覆盖窗)用 `objc2` 逐个子系统替换; 先做 spike, 后真机验收。

## 暂不迁

- Flutter 面板(Dart/FRB)、cairo 绘制算法、SQLite/模型器(Rust 已有)。
- Flutter 宿主、ScreenCaptureKit、系统菜单/状态栏在 R3 验证前保持 Swift。
