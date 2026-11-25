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
- **R5 镜头结果直出**: 新增 `glaspen2_camera_{pan,zoom_at,zoom_at_center,centered}_values` 系列(out 参数)替代 JSON 解析;
  Swift `CanvasCamera`/`settings_api_glue` 改调, 删除 `camera_state.swift`; 更新 D5b 镜头接线闸门 token。
- **R6 壳全局状态**: `AppState`(屏幕尺寸/语言)迁入 `glaspen-core::shell_state`(原子量);
  C ABI 同名符号改由 Rust 导出, Swift 三处直接引用改 FFI, 删除 `App/app_state.swift`; 1 个单测。
- **R7 快捷键修饰键掩码**: chord→NSEvent 修饰键解析迁 `chord::appkit_modifier_mask` + FFI;
  Swift `MenuRuntime.maskFor` 调 Rust, 删除 `Input/shortcut_mask.swift`; 1 个单测。
- **R8 快捷键生效表状态**: values/lookup 迁 `glaspen-core::shortcut_state`(BTreeMap+Mutex);
  FFI install/查询/JSON 快照; Swift `ShortcutStore` 只做 NSDictionary/JSON 桥, `SettingsGlue` 快照直出; 1 个单测。
- **R9 绘制/网格状态**: `ViewSettings`(无限画布/笔迹可见/缩略图/网格尺寸/分隔/跟随)迁 `glaspen-core::view_state`(原子量);
  原 C ABI 符号改 Rust 导出 + 3 个网格 getter; Swift 直接引用改 FFI, 删除 `Settings/view_settings.swift`; 1 个单测。
- **R10 功能开关/GIF 参数**: `ToggleSettings` 迁 `glaspen-core::toggle_state`(原子量); 16 个 C ABI 符号改 Rust 导出;
  Swift 直接引用改 FFI, 删除 `Settings/toggle_settings.swift`; 1 个单测。
- **R11 渲染设置**: `RenderSettings` 迁 `glaspen-core::render_state`(玻璃/描边/软阴影/反色/翻转);
  24 个 C ABI 符号改 Rust 导出; Swift 直接引用改 FFI, 删除 `Settings/render_settings.swift`; 1 个单测。
- **R12 笔外观设置**: `PenSettingsStore`(颜色/粗细/选中/历史/暂存)迁 `glaspen-core::pen_settings`;
  颜色/宽度预设直接用 core presets; 全部 C ABI 符号改 Rust 导出, 删除 `Settings/pen_settings_store.swift`; 1 个单测。
- **R13 预设缓存清理**: 删除 Swift `PresetStore`/`PenPresets` 缓存层; 菜单/预设标题直接读 core `glaspen2_color_preset_*` / `glaspen2_width_preset_*`; 预设数量调用改 core; 2 个 Swift 文件删除。
- **R14 AppKit 壳预研**: 根 crate 增加 objc2/objc2-app-kit macO 依赖; 新增 `src/macos_shell.rs`,
  用 Rust 创建 `NSStatusItem`; Swift `StatusBarFactory` 仅在 `GLASPEN2_RUST_STATUS_ITEM=1` 时走 Rust, 默认保持 Swift, 失败自动回退。
  smoke(默认+Rust 开关)与全量测试均绿; 下一步再迁状态栏图标/菜单/覆盖窗。
- **R15 覆盖窗预研**: `macos_shell` 增加 `glaspen2_rust_overlay_window_create`, Rust 创建主覆盖 `NSWindow`
  (无边框/最大层级/透明/穿透/全空间); Swift `OverlayWindowFactory` 仅在 `GLASPEN2_RUST_OVERLAY=1` 时使用, 默认 Swift/失败回退。
- **R16 覆盖窗内容层**: `macos_shell` 增加 Rust 创建内容容器 `NSView`(layer-backed)与磨砂玻璃 `NSVisualEffectView`
  (behind-window/light/active); `OverlayWindowFactory` 同开关复用, 默认 Swift/失败回退。
- **R17 状态栏图标渲染**: `macos_shell` 移植 `StatusIconRenderer`(NSBitmapImageRep/NSBezierPath/NSAffineTransform),
  新增 `GLASPEN2_RUST_STATUS_ICON=1` 切换与 `GLASPEN2_RUST_STATUS_ICON_TEST=1` 像素 parity 探针; 实测 72x72 diff=0/20736。
- **R18 覆盖层内容组装**: `macos_shell` 新增 `glaspen2_rust_overlay_assemble_content`
  (玻璃/绘制视图加入顺序、layer 透明背景、setContentView/orderFront); `GLASPEN2_RUST_OVERLAY=1` 时整条覆盖层组装走 Rust。
- **R19 绘制视图**: `macos_shell` 用 `define_class!` 创建 NSView 子类 `GlaspenRustDrawView`,
  `drawRect:` 经 C 桥回调 Swift `OverlayDrawing`; `GLASPEN2_RUST_DRAW_VIEW=1` opt-in, 默认 Swift/失败回退。
- **R20 状态栏默认切 Rust**: 状态栏项创建 + 状态栏图标渲染默认走 objc2; 保留 `GLASPEN2_SWIFT_STATUS_ITEM=1` /
  `GLASPEN2_SWIFT_STATUS_ICON=1` 回退开关与失败自动回退; parity 探针默认 diff=0。

## 下一批候选(按风险从低到高)

- R2-a 快捷键掩码/显示名表等纯映射(需评估是否值得为薄表增加 FFI)。
- R2-b 画布坐标变换 `canvas_input_x/y`(纯公式, 但位于笔事件热路径, 需基准)。
- R2-c 设置/渲染状态存储(消除 Swift/Rust 双份状态, 需改多处调用点)。
- R3 AppKit 壳(状态栏/菜单/覆盖窗)用 `objc2` 逐个子系统替换; 先做 spike, 后真机验收。

## 暂不迁

- Flutter 面板(Dart/FRB)、cairo 绘制算法、SQLite/模型器(Rust 已有)。
- Flutter 宿主、ScreenCaptureKit、系统菜单/状态栏在 R3 验证前保持 Swift。
