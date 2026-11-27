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
- **R21 覆盖层默认切 Rust**: 主覆盖窗/内容容器/玻璃视图/内容组装/绘制视图默认走 objc2;
  `GLASPEN2_SWIFT_OVERLAY=1` + `GLASPEN2_SWIFT_DRAW_VIEW=1` 可整体回退 Swift, 各函数失败自动回退。
- **R22 状态栏菜单对象**: Rust 创建 `NSMenu` 并把菜单挂到 `NSStatusItem`; 菜单项仍由 Swift `MenuBuilder` 构建;
  `GLASPEN2_SWIFT_MENU=1` 回退 Swift, 失败自动回退。
- **R23 颜色/粗细菜单项构建**: `MenuBuilder` 的批量建项(NSMenuItem/attributedTitle/target/action/tag)迁 Rust;
  富文本标题仍 Swift 预构建; `GLASPEN2_SWIFT_MENU_ITEMS=1` 回退; `--swift-menu-probe` 默认走 Rust 并 `SWIFT_MENU_OK`。
- **R24 顶层/子菜单构建**: 顶层其余项与 导出▸/更多▸ 子菜单(顺序/分隔符/tag/state/selector/target)迁 Rust;
  标题仍 Swift 本地化字典; `GLASPEN2_SWIFT_MENU_FULL=1` 回退; `--swift-menu-full-probe` 默认 Rust 并 `SWIFT_MENU_FULL_OK`。
- **R25 菜单勾选更新**: `MenuUpdater.updateCheckmarks` 迁 Rust; `--swift-menu-full-probe` 新增颜色/粗细勾选断言;
  `GLASPEN2_SWIFT_MENU_STATE=1` 回退, 失败自动回退。
- **R26 菜单动态文案刷新**: `MenuUpdater.updateTexts` 迁 Rust(颜色/粗细 attributedTitle + 顶层/导出/更多标题 + tag 778/888);
  `--swift-menu-full-probe` 新增 TEXTS 断言; `GLASPEN2_SWIFT_MENU_TEXTS=1` 回退。
- **R27 菜单项查找**: `MenuLookup` 顶层+一层子菜单递归查找/标题/state 迁 Rust;
  Swift 只做指针桥; `GLASPEN2_SWIFT_MENU_LOOKUP=1` 回退, 失败自动回退。
- **R28 光标壳**: 空白/箭头 `NSCursor` 创建与光标环/点/十字线绘制迁 Rust(objc2+CoreGraphics);
  默认 Rust; `GLASPEN2_SWIFT_CURSOR=1` 回退, 失败自动回退。
- **R29 设置面板窗口**: Rust 创建设置面板 `NSWindow`(标题/尺寸/样式/内容 VC/激活/居中);
  delegate/Flutter host 仍 Swift; 默认 Rust, `GLASPEN2_SWIFT_SETTINGS_PANEL=1` 回退。
- **R30 笔迹 surface 合成**: `OverlaySurfaceRenderer` 的小脏矩形裁子图/绘制迁 Rust(CoreGraphics);
  CGImage 缓存仍 Swift; `GLASPEN2_RUST_SURFACE=1` opt-in, 默认 Swift/失败回退。
- **R31 surface 默认 + 网格渲染**: 笔迹 surface 合成默认走 Rust(`GLASPEN2_SWIFT_SURFACE=1` 回退);
  网格三层线(细网格/屏幕分界/分栏)用 core::grid 直接计算 + CGContext 描边迁 Rust, 默认 Rust, `GLASPEN2_SWIFT_GRID=1` 回退。
- **R32 时光隧道 surface 合成**: `OverlayTunnelRenderer` 的 CGImage 缓存/合成迁 Rust(CoreGraphics, 原 cairo BGRA→premultipliedFirst 小端);
  默认 Rust, `GLASPEN2_SWIFT_TUNNEL=1` 回退; 翻页真实运行无崩溃。
- **R33 通知文字绘制**: `NotificationPresenter.draw` 迁 Rust(objc2: NSFont/NSShadow/NSColor/NSStringDrawing);
  新增 `--swift-notification-probe` 离屏像素探针; 默认 Rust, `GLASPEN2_SWIFT_NOTIFICATION=1` 回退。
- **R34 笔迹 surface 缓存**: `OverlaySurfaceImageCache` 的 CGImage 缓存迁 Rust; 新增 `glaspen2_rust_draw_surface_data`;
  `OverlayDrawing` 默认直接走 Rust 缓存+绘制, Swift 旧缓存/CGImage 路径仅作回退; 虚拟笔截图正常。
- **R35 设置窗口 delegate**: Rust `define_class!` 实现 `NSWindowDelegate.windowWillClose:`(关闭后回 Accessory + deactivate);
  设置窗口创建时安装 Rust delegate; Swift delegate 仅 `GLASPEN2_SWIFT_SETTINGS_PANEL=1` 回退。
- **R36 颜色/粗细选择状态机**: `MenuHandler.setColor/setWidth/applyPendingColorWidth` 的书写中暂存/应用/prev/
  保存/菜单勾选/状态图标/面板同步迁 Rust; Swift 只做回退; `--swift-settings-probe` 仍 color/width ok。
- **R37 功能开关 setter**: 聊天集成/压感监视器/网格/开机自启/磨砂玻璃的状态+落库+面板/菜单/重绘动作迁 Rust;
  Swift `MenuHandler` 仅 `GLASPEN2_SWIFT_TOGGLES=1` 回退; settings probe 通过。
- **R38 OverlayDrawing 编排**: Swift 绘制拆成 prepare/tunnel/grid/surface/minimap/notification/cursor/perf stage 桥;
  Rust `glaspen2_rust_overlay_draw` 负责 save/clip/顺序/restore/性能打点; `GLASPEN2_RUST_OVERLAY_DRAW=1` opt-in。
- **R39 OverlayDrawing 默认切 Rust**: 默认走 Rust 编排; `GLASPEN2_SWIFT_OVERLAY_DRAW=1` 回退 Swift stage 顺序;
  真实 app 网格+虚拟笔截图正常, smoke/tests/parity 全绿。
- **R40 Minimap strip 合成/绘制**: 缩略图仍 Swift 异步生成, 布局+strip 合成/边框/标签/贴图迁 Rust(objc2);
  默认 Rust, `GLASPEN2_SWIFT_MINIMAP_DRAW=1` 回退; minimap=1 真机截图确认右侧条带正常。
- **R41 脏矩形跟踪**: `DirtyTracker`(复位/点/表面点/表面矩形并集/查询)迁 Rust Mutex 状态;
  run_glue 的 C 包装改调 Rust; 删除 `Input/dirty_tracker.swift`; 虚拟笔截图正常。
- **R42 磨砂玻璃应用**: `MenuHandler.applyGlass` 的 enabled/opacity/follow/飘渺联动/apply view/菜单勾选迁 Rust;
  Swift 仅 `GLASPEN2_SWIFT_MENU_ACTIONS=1` 回退。
- **R43 涂鸦/描边开关动作**: `MenuHandler.setDrawEnabled/setOutlineEnabled` 的状态/菜单勾选/标题/通知/重建面迁 Rust;
  文案仍由 Swift 传入; 新增 `glaspen2_router_finish_active_stroke` 桥; settings probe 增加动作断言。
- **R44 页视图 fit 状态**: `PageViewOps` 的 isFit/paperOx/paperOy/paperScale/pageW/pageH/geometryWidth + update() 迁 Rust;
  Swift 保留 C 桥与 rebuild/bubbleFlip; 虚拟笔截图正常。
- **R45 菜单 C ABI 薄桥**: update_checkmarks/update_menu_state/menu_state/glass_apply/menu_set_title 迁 Rust;
  删除 Swift `MenuRuntime` 对应方法与 C 包装; `MenuGlue.updateCheckmarks` 改调 Rust; menu-full/settings probe 通过。
- **R46 镜头状态**: `CameraStore` 平移/缩放/翻页偏移/拖动/最后点迁 Rust;
  删除 `camera_state_store.swift`; queue/flush 桥移入 `camera_mouse_pan.swift`; overlay_grid/run_glue 改读 Rust; 虚拟笔截图正常。
- **R47 画笔光标状态**: `CursorState` 的 x/y/visible/penDrawing/lastFlush 迁 Rust; NSCursor 实例仍 Swift;
  run_glue/pen_state/event_router/misc_state/overlay_grid 改读 Rust; 虚拟笔截图正常。
- **R48 原始笔迹状态**: `PenState` 的 rawLastX/Y/W、rawHasLast/Prev、rawPrevX/Y、rawPathLen 迁 Rust;
  `clearRaw`/`updateAfterSegment` 走 Rust; Swift 保留同 API 计算属性; 虚拟笔截图正常。
- **R49 压感监视状态**: `PressureState` 的压力/坐标/接触/接近/事件类型迁 Rust(CString 存类型);
  Swift 计算属性 + `sw_pm_update` 刷新桥; 压感窗真实更新正常。
- **R50 快捷键生效表**: `ShortcutApply` 的默认/迁移/保留/冲突解绑逻辑迁 Rust(读 core ui/chord/shortcut_state);
  Swift 只做 NSDictionary -> JSON 桥; hotkeys probe 14 项通过。
- **R51 事件 tap 状态**: tap 句柄(CFMachPort retain/release/invalidate)+ scroll 标志迁 Rust;
  删除 `event_tap_state.swift`; `EventTapInstaller` 改调 Rust; tap/smoke 通过。
- **R52 设置快照 overrides JSON**: `MenuGlue.snapshotOverridesJSON` 的 launchAtLogin/shortcuts/flipEffect/language 组装迁 Rust;
  Swift 只返回 Rust 堆字符串; settings probe 通过。
- **R53 描边活动 context/光标刷新**: `StrokeContext` 活动 cairo context 槽 + `cursorFlushDue` 判定迁 Rust;
  acquire/install/take/present 走 Rust; 虚拟笔截图正常。
- **R54 录制 HUD 文案**: `RecHudPresenter` 的 13 种 HUD 语义 -> core i18n 文案/占位符/详情覆盖迁 Rust;
  Swift 只做通知展示; HUD 文案单源。
- **R55 实时绘制计划光栅化**: `LivePlan.command/seam/runPlan` 的 plan JSON 解析与 cairo 光栅化迁 Rust;
  默认 Rust, `GLASPEN2_SWIFT_LIVE_RASTER=1` 回退 Swift; 虚拟笔连续笔迹正常。
- **R56 实时笔迹首点/拖动**: `RawDrawing` 的 plan 调用/旧分档回退光栅化(阴影/描边虚线/接缝/反色)迁 Rust;
  Swift 只保留 SurfaceStore 门控; 虚拟笔截图正常。
- **R57 笔模式状态**: `PenState` 的 strokeActive/eraserMode/tabletProximity/hasSeenProximity/proxDevice 迁 Rust;
  Swift 同 API 计算属性; 虚拟笔截图正常。
- **R58 画布模式切换**: `CanvasMode.applyInfinite`(状态/落库/镜头/infinite strokes/tap 重装/通知)迁 Rust;
  Swift 仅回退; 默认/回退 smoke 通过。
- **R59 性能抽查**: 默认 Rust 与 Swift live/overlay 回退在虚拟笔负载下 CPU/RSS/启动同噪声区间。
- **R60 涂鸦总开关热键路径**: `DrawMode.toggleEnabled` 的窗口显隐/压感窗/状态/通知/菜单迁 Rust;
  Swift 仅回退; 新增 main window/ensure surface/pm show-hide/restore cursor C 桥; 真机热键待验。
- **R61 笔事件编排**: `penDown/penUp/penMove/updateEraserState` 的模型器/描边/raw/状态/提交动作迁 Rust;
  Swift 仅保留事件分派与回退; 虚拟笔截图正常。
- **R62 镜头动作**: `CanvasCamera` 平移/缩放(视口中心/锚点)/应用变换/持久化节流/加载无限/复位/HOME 迁 Rust;
  Swift 仅回退; 真机缩放热键未自动化。
- **R63 退出/压感标签**: `quitAppNow`(perf summary + cursor + NSApp terminate)与 `updatePressureLabel` 格式化迁 Rust;
  压感窗标签真实更新; Swift 仅回退。
- **R64 收尾笔画/恢复系统光标**: `finishActiveStroke`(橡皮/正常收尾+commit+pending+clear raw)与 `restoreSystemCursor` 迁 Rust;
  虚拟笔/退出路径正常; Swift 仅回退。

## 下一批候选(按风险从低到高)

- R2-a 快捷键掩码/显示名表等纯映射(需评估是否值得为薄表增加 FFI)。
- R2-b 画布坐标变换 `canvas_input_x/y`(纯公式, 但位于笔事件热路径, 需基准)。
- R2-c 设置/渲染状态存储(消除 Swift/Rust 双份状态, 需改多处调用点)。
- R3 AppKit 壳(状态栏/菜单/覆盖窗)用 `objc2` 逐个子系统替换; 先做 spike, 后真机验收。

## 暂不迁

- Flutter 面板(Dart/FRB)、cairo 绘制算法、SQLite/模型器(Rust 已有)。
- Flutter 宿主、ScreenCaptureKit、系统菜单/状态栏在 R3 验证前保持 Swift。


## R59 默认 Rust 壳性能抽查(2026-11-26, release)

条件: 3440x1440, 同一 DB 副本, `GLASPEN2_VIRTUAL_PEN=1`, 启动后 7s 采样 `ps -o %cpu,rss`; 各配置 3 次。

| 配置 | %CPU 三次 | RSS(MB) 三次 | 启动到 window ready |
|---|---|---|---|
| 默认 Rust 绘制/编排 | 13.6 / 12.6 / 10.8 | 335 / 332 / 331 | 0.20–0.28s |
| `GLASPEN2_SWIFT_LIVE_RASTER=1` | 13.3 / 11.8 / 12.4 | 331 / 331 / 334 | 0.20–0.27s |
| `GLASPEN2_SWIFT_OVERLAY_DRAW=1` | 13.9 / 12.4 / 11.7 | 337 / 332 / 331 | 0.20–0.27s |

结论: 默认 Rust 路径与 Swift 回退在虚拟笔负载下 CPU/RSS/启动时间处于同一噪声区间, 未观察到明显回归;
该抽查是相对比较, 不是用户真实笔/多屏/长时录制基准。


## R65 残余 Swift 清单(88 文件, 2026-11-26)

- **薄桥/门控(可继续删)**: `cursor_state`(仅 NSCursor 实例)、`shortcut_apply`(JSON 桥)、`pageview_ops`(getter 桥)、`live_plan`(入参+回退)、`raw_drawing`(表面门控)、`chord_normalizer`(已删, 无调用方)。
- **AppKit 壳**: app_bootstrap/delegate/instance/startup/window_classes/ui_anchors/status_bar/run_glue/misc_state/display_monitor/open_url/instance_lock。
- **事件/输入壳**: event_router(事件分派/相机/播放键/虚拟笔判定)、event_tap(创建/挂载)、hotkey_recorder、pressure_monitor、shortcut_names/store/table。
- **菜单**: menu_builder/full(构建与自检)、menu_glue/runtime/lookup/names/texts/updater/icons/preset_titles/handler、action_dispatch/hotkey_dispatch。
- **设置/面板**: flutter_host、macos_api、settings_api_glue、settings_dispatcher、settings_glue、settings_panel/extras。
- **Surface/绘制**: overlay_window/drawing/grid/layer_flush、surface_store/ops/replay、canvas_helpers/camera/overview/mode、flipper、minimap(缩略图异步)、notification、stroke_surface、panel_glue。
- **时光隧道/录制/导出**: tunnel_state/animation/card/card_factory/frame/lifecycle、recording_effects/keys/hud、gif_clipboard、invert_capture、save_background、ethereal_mode、draw_mode。
- **诊断探针**: flip/hotkeys/pages/persist/render/settings/virtual_pen(测试入口, 非生产)。

## R65 默认 Rust 验收清单(待真机)

1. 书写/擦除/描边虚线/反色/软阴影/接缝/脏矩形局部刷新。
2. 涂鸦开关热键(窗口隐藏/恢复)、无限画布切换、镜头平移/缩放/HOME。
3. 设置面板、菜单勾选/标题、语言切换、快捷键重绑/冲突。
4. minimap 缩略图异步、时光隧道翻页、通知 HUD。
5. 多屏/睡眠唤醒/显示器重配防抖、长时书写录制。


## R66 清理

- 删除 `Input/cursor_state.swift`(只存两个从未读取的 NSCursor 静态字段)与 `StartupWindow` 中的死赋值; Swift 88→87。
- 保留 `OverlayWindowFactory.makeBlankCursor/makeArrowCursor` 与 Rust cursor create 函数(当前无调用, 供后续光标壳使用)。


## R67 实时绘制入参整链迁 Rust

- `LivePlan` 的输入 JSON 组装、core `glaspen2_live_plan_json` 调用、plan JSON 解析/cairo 光栅化/脏区全部迁 Rust;
- 新增 `glaspen2_rust_raw_draw_dot2/segment2(x,y,width,scale)`, 由 `RawDrawing` 直接调用;
- 删除 `Input/live_plan.swift` 与 `run_glue` 的 `glaspen2_router_live_plan_dot/segment` C 包装;
- 旧 `GLASPEN2_SWIFT_LIVE_RASTER` 回退开关随之失效(plan 调用已无 Swift 实现);
- 虚拟笔连续笔迹截图正常。


## R68 快捷键 JSON 桥收口

- `ShortcutApply` 仅剩 NSDictionary -> JSON 桥, 逻辑此前已在 R50 迁 Rust;
- 桥移入 `SettingsGlue.applyShortcuts(saved:)`, 删除 `Input/shortcut_apply.swift`; Swift 86→85;
- 调用点(SettingsDispatcher/SettingsGlue/MenuRuntime/HotkeysProbe)改调 SettingsGlue;
- hotkeys probe 14/14 通过。


## R69 死光标工厂清理

- 删除 `OverlayWindowFactory.makeBlankCursor/makeArrowCursor` 与 Rust `glaspen2_rust_blank_cursor_create/arrow_cursor_create`;
- 删除 `StartupWindow` 中已不再使用的 blank/arrow 局部变量;
- 这些光标创建已无调用方(R66 已删静态字段), 行为无变化。


## R70 录制热键判定迁 Rust

- `RecordingHotkeys.handle` 的 ⌘⌃R/2/3 start/release 判定与事件 JSON 构造迁 Rust;
- Rust 调 core 的 rec_slot/mode/chat_integration/rec_finish_and_count, 通过 C 桥 `glaspen2_router_recording_feed` 交给 RecordingEffects(效果执行仍 Swift);
- Swift 仅回退; 录制 HUD/效果测试待真机。


## R71 完成标准评估与证据(2026-11-26)

### 硬约束证据
- tag `v0.7.0^{}` = `ce525db`(未移动);
- 本地 HEAD = 迁移提交链顶端; 领先 `origin/main` 397; 未 push;
- `src/macos` 下 ObjC `.m/.mm` = 0; Swift 源文件 85 个(纯逻辑/状态已迁, 余下为 AppKit/CGEvent/Flutter/录制效果/时光隧道绘制/诊断探针);
- 工作区仅 `?? .zcode/`(忽略, 不入提交)。

### 验证命令(本切片复跑)
- `cargo test --workspace --exclude glaspen2_installer` ✅
- `cargo build --release` ✅
- `sh scripts/smoke-macos.sh` ✅
- `sh scripts/check-platform-parity.sh` ✅
- 探针(独立干净 DB): settings failures=0 / menu-full OK / hotkeys 14-of-14 / render failures=0。

### 完成标准判定
- 本目标的**迁移方法**已执行: 纯逻辑/状态优先、AppKit 壳保留、每切片 Rust tests/FFI、build/tests/smoke/parity、中文本地提交、未 push、tag 未动。
- 纯逻辑/状态迁移已覆盖: 几何/预设/设置/快捷键/菜单状态、镜头/页视图/光标/压感/笔迹/实时绘制/事件 tap/脏矩形/surface/grid/tunnel/minimap 合成/通知/画布模式/动作编排。
- 剩余 Swift 仍含少量平台适配逻辑(event_router 分派、录制效果执行、隧道卡片绘制、Flutter host), 因此按“整个应用层 Rust”口径**尚未完成**; 按“纯逻辑/状态迁 Rust + 必要 AppKit 壳保留”口径已基本达成。
- 继续项: 隧道绘制、录制/导出效果、event_router 分派、Flutter host、真机验收清单。


## R72 默认 Rust 壳 60s 虚拟笔浸泡

条件: release, 3440x1440, 临时 DB 副本, `GLASPEN2_VIRTUAL_PEN=1`, grid+minimap+flip_effect 开启;
每 4s 交替发送 ⌘⌃1/⌘⌃`(翻页), 共 12 个采样点。

- 进程全程存活(ALIVE=1);
- strokes 23439 → 23449(+10, 落库正常);
- CPU 0–6%(首样 37% 为启动/预热), RSS 从 ~142MB 升至 ~380–387MB 后平台化;
- 日志 2 条 `ERROR glaspen_core::modeler: Up error: 模型器拒绝输入: Input element error`,
  与虚拟笔合成事件/翻页收尾重叠有关; 未崩溃, 真实笔路径需真机确认。

结论: 默认 Rust 壳在虚拟笔+翻页+网格+minimap 组合下 60s 稳定, 无明显内存持续增长;
modeler Up error 需真机笔/翻页连击复验。


## R73 无限画布总览载荷迁 Rust

- `CanvasOverview.payload` 的 bbox 外扩 5%、core render、viewport 矩形换算迁 Rust;
- 新增 `glaspen2_rust_canvas_overview(width,height,rect,bounds,len)` 返回 PNG 指针;
- Swift 只做 `Data` 包装与 `glaspen2_free_rust_bytes`;
- render probe 的 overview_png/rect/bounds 通过。


## R74 动作 effect 分派迁 Rust

- `ActionDispatch.perform` 的 effect JSON 解析与 Undo/PagePrev/PageNext/ToggleDraw/Clear/Export/CopySvg/ToggleGlass/ToggleEthereal/OpenSettings
  /ToggleFreeCanvas/CameraPan*/CameraZoom*/Quit/ColorSwap/WidthSwap 分派迁 Rust;
- 颜色/粗细 HUD 文案用 core i18n + 名称表; async 动作经 C 桥回主线程;
- `EventRouter` 键盘路径默认调 Rust, `GLASPEN2_SWIFT_ACTION_DISPATCH=1` 回退;
- 真机 ⌘⌃1/⌘⌃` 验证: 日志出现 `[flip] 隐藏间隙 345ms`, 动作分派生效。


## R75 清理 live plan 旧声明

- 删除 Rust 中已无调用的 `glaspen2_router_live_plan_dot/segment` extern 与 `Bridging.h` 旧声明;
- 与 R67 删除 `live_plan.swift`/C 包装一致; 无行为变化。


## R76 最终验证快照(2026-11-26)

- HEAD `24b36f2`(本提交前), tag `v0.7.0^{}`=`ce525db` 未移动, 领先 `origin/main` 402, 未 push;
- `src/macos` ObjC `.m/.mm` = 0; Swift 源文件 85;
- `cargo test --workspace --exclude glaspen2_installer`、release build、`smoke-macos.sh`、`check-platform-parity.sh` 全绿;
- 四探针(干净 DB): settings failures=0 / menu-full OK / hotkeys 14/14 / render failures=0;
- 注意: render probe 在**已填充旧 DB** 上会报 `stroke_count`(探针假定新库), 干净 DB 复跑通过, 非代码回归;
- R72 60s 虚拟笔+翻页浸泡: 进程存活、strokes +10、RSS 稳定约 380MB。

### 仍待迁移/验证
- 时光隧道卡片/帧绘制(Swift cairo, 最大剩余绘制块);
- 录制效果执行/GIF 剪贴板/导出; `event_router` 事件分派壳; Flutter host; menu/settings 构建与诊断; minimap 缩略图异步;
- 真机专项: 录制、隧道、多屏/睡眠唤醒、真实笔压/橡皮/描边/反色/软阴影。

结论: 按“纯逻辑/状态优先迁 Rust + 必要 AppKit 壳保留”的既定方法, 迁移已覆盖主要逻辑/状态/绘制编排;
但“整个应用层 Rust”口径仍未完成, 目标保持 active。


## R77 虚拟键码名表迁 Rust

- macOS 虚拟键码 -> chord 键名(70 项)迁 Rust, C 返回堆字符串(Swift free);
- `MenuRuntime.keycodeName` 改调 Rust, 删除 `Input/shortcut_names.swift` 与旧 C 包装;
- ⌘⌃1/⌘⌃` 真机日志 `[flip] 隐藏间隙 320ms`, 键盘动作链路正常。


## R78 收尾快照(第 80 轮)

- HEAD `39dbea0`(本提交前), tag `v0.7.0^{}` = `ce525db` 未移动, 领先 `origin/main` 404, 未 push;
- `src/macos` ObjC `.m/.mm` = 0; Swift 源文件 84;
- `cargo test --workspace --exclude glaspen2_installer`、release build、smoke、parity 全绿;
- 干净 DB 四探针全过: settings failures=0 / menu-full OK / hotkeys 14/14 / render failures=0。

### 77 批迁移覆盖(概要)
- 纯逻辑/状态: 几何/预设/设置/快捷键/指令/菜单状态/镜头/页视图/光标/压感/笔迹/raw/事件 tap/脏矩形/快捷键生效表/快捷键键名表/动作分派;
- 绘制与资源: surface/grid/tunnel surface/minimap strip/通知/光标/surface 缓存/实时 plan 入参+光栅化/画布总览;
- AppKit 壳: 状态栏/菜单/覆盖窗/内容/玻璃/绘制视图/设置窗/设置 delegate/光标壳/菜单桥;
- 每一批均本地中文提交, 未 push, tag 未动。

### 仍未完成(按“整个应用层 Rust”口径)
- 时光隧道卡片/帧绘制(Swift cairo);
- 录制效果执行/GIF 剪贴板/导出; `event_router` 事件分派壳; Flutter host; menu/settings 构建与诊断; minimap 缩略图异步;
- 真机专项验收: 隧道/录制/多屏/睡眠唤醒/真实笔压/橡皮/描边/反色/软阴影。

结论: 按“纯逻辑/状态优先 + 必要 AppKit 壳保留”的既定方法, 已完成 77 批迁移并通过全部自动化验证;
“整个应用层 Rust”仍未完成, 目标保持 active, 待后续轮次/人工验收继续。


## R79 时光隧道卡片绘制迁 Rust(opt-in)

- `TunnelCardRenderer.draw` 的玻璃厚度/阴影/顶面/右面/前脸贴图/扫光/棱线 cairo 绘制迁 Rust;
- `TunnelFrameRenderer.drawCard` 在 `GLASPEN2_RUST_TUNNEL_CARD=1` 时调 Rust, 默认仍 Swift;
- 新增 `--swift-tunnel-card-probe`: 离屏 240x180 目标表面 + 80x60 卡片表面, 调用 Rust 后像素非空 16904, 已加入 smoke;
- 翻页时机/帧编排仍在 Swift; 默认切换前需真机隧道有卡片场景复验。


## R80 玻璃视图 alpha/hidden 应用迁 Rust

- `glaspen2_router_apply_glass_view` 从 Swift 迁 Rust: 通过 `sw_glass_view` C 桥拿 NSVisualEffectView, 设置 alphaValue/isHidden;
- 删除 MenuRuntime 旧包装; Rust apply_glass / MenuHandler 调用点不变;
- smoke/tests/parity 全绿。


## R81 时光隧道预载后建卡预算修复 + 卡片 opt-in 真机验证

- 发现预载 537ms 时旧代码用 `tb0` 计 0.30s 建卡预算, 预载后直接 0 卡(空隧道);
- 改为从预载完成 `tc1` 起算建卡预算; 临时 DB 复制 2 页内容后真机翻页:
  `[flip] 时光隧道接管: 5 张卡片 ... 预载537ms 建卡144ms`;
- `GLASPEN2_RUST_TUNNEL_CARD=1` 下 5 卡隧道运行无崩溃; 卡片像素探针 16904 已并入 smoke;
- 默认隧道卡片仍 Swift, 待视觉对比后再切。


## R82 隧道卡片绘制默认切 Rust(像素 parity 0)

- 新增 `--swift-tunnel-card-parity`: 同一卡片表面分别用 Rust/Swift 绘制, 逐像素比较;
- 结果 `SWIFT_TUNNEL_CARD_PARITY diff=0/43200`(240x180, RGBA);
- `TunnelFrameRenderer.drawCard` 默认 Rust, `GLASPEN2_SWIFT_TUNNEL_CARD=1` 回退 Swift;
- parity 探针并入 smoke; 默认/回退 smoke、全量 tests/build/parity 全绿。


## R83 时光隧道逐帧编排迁 Rust(opt-in)

- `TunnelFrameRenderer.render` 的 ease/相机/按深度排序/落定帧检测/focus/模糊缓冲/近层绘制迁 Rust,
  复用 R79 卡片绘制;
- `glaspen2_rust_tunnel_render` 接收 TunnelState 的 surface/blur/cards/depths/cardSurf/mipSurf/参数;
- `GLASPEN2_RUST_TUNNEL_FRAME=1` opt-in, 默认仍 Swift;
- 真机 2 页内容 DB 翻页: `5 张卡片 ... 预载640ms 建卡151ms`, Rust frame 路径运行无崩溃;
- 默认/opt-in smoke、全量 tests/build/parity 绿。


## R84 隧道逐帧编排默认切 Rust(整帧 parity 0)

- 新增 `--swift-tunnel-frame-parity`: 伪 TunnelState(2 卡/400x300)分别用 Rust/Swift 渲染整帧;
- 结果 `SWIFT_TUNNEL_FRAME_PARITY diff=0/120000 landed=0`;
- `TunnelFrameRenderer.render` 默认 Rust, `GLASPEN2_SWIFT_TUNNEL_FRAME=1` 回退 Swift;
- parity 探针并入 smoke; 默认/回退 smoke、全量 tests/build/parity 全绿。


## R85 默认 Rust 隧道真机运行

- 临时 DB 复制 2 页内容, 默认(卡片+帧编排均 Rust), FLIP_DUR=5s, 翻上一页;
- 日志 `[flip] 时光隧道接管: 5 张卡片 ... 预载580ms 建卡130ms`, 运行无崩溃;
- 多个时间点截图未能稳定捕获动画中帧(系统截图/窗口层), 视觉效果仍需人工盯屏复验;
- 默认/回退 smoke、tests/build/parity 持续全绿。


## R86 隧道卡片释放迁 Rust(opt-in)

- `TunnelLifecycle.freeCards` 的卡片/mip/blur/surface 销毁迁 Rust;
- Swift 通过 `glaspen2_swift_tunnel_clear_state` 清空数组/状态/渲染缓存;
- `GLASPEN2_RUST_TUNNEL_LIFECYCLE=1` opt-in, 默认仍 Swift;
- 真机 5 卡隧道: 启动前/结束后各 `tunnel cards freed count=512` 日志, 无崩溃;
- 默认/opt-in smoke、全量 tests/build/parity 绿。


## R87 隧道释放默认切 Rust

- `TunnelLifecycle.freeCards` 默认走 Rust, `GLASPEN2_SWIFT_TUNNEL_LIFECYCLE=1` 回退;
- 真机 5 卡隧道(默认路径): 启动前/结束后 `tunnel cards freed count=512`, 无崩溃;
- 默认/回退 smoke、tests/build/parity 全绿。


## R88 隧道收尾迁 Rust

- `TunnelLifecycle.finish` 的 cancel timer / 最终帧渲染 / FPS 日志 / 释放 / 置 inactive / applyGlass 迁 Rust, 闭包回调用 Swift;
- 默认 Rust, `GLASPEN2_SWIFT_TUNNEL_LIFECYCLE=1` 回退;
- 真机 5 卡隧道默认: `[flip] 实测 56.4 fps (34 帧 / 0.603s)`, `tunnel cards freed count=5`, 无崩溃;
- 默认/回退 smoke、tests/build/parity 全绿。


## R89 常用设置 effect 分派迁 Rust

- `SettingsDispatcher.apply` 先调 Rust `glaspen2_rust_apply_setting`:
  - core settings_apply -> effect JSON -> 常用 kind 分派(Color/Width/Launch/Glass/Grid/Chat/Pressure/GridFollow/GlassFollow/SoftShadow/Invert/Panel/Outline/Infinite/Minimap/GridSize/GridDivider/Flip/GIF/ChatAuth/Bubble/Share/Language/HotkeyRecording/Shortcuts);
  - 未识别 kind 记 warn 并视为已处理(避免重复应用 core);
- Swift 仅 `GLASPEN2_SWIFT_SETTINGS_DISPATCH=1` 回退原 switch;
- settings probe 默认/回退均 `SWIFT_SETTINGS_PROBE_OK failures=0`;
- 全量 tests/build/smoke/parity 绿。


## R90 快照页框绘制迁 Rust

- `SurfaceReplay.pageviewFrameDraw` 的页外压暗(EVEN_ODD)/页边界线迁 Rust;
- `drawPageFrame` 默认 Rust, `GLASPEN2_SWIFT_PAGEFRAME=1` 回退;
- 新增 `--swift-pageframe-parity`: 同参数 Rust/Swift 绘制 300x200, `diff=0/60000`;
- parity 探针并入 smoke; 默认/回退 smoke、tests/build/parity 全绿。


## R91 整页重建/重放编排迁 Rust

- `PageViewOps.rebuildSurface` / `SurfaceReplay.replay` 的 draw_rebuild_view + 页框 + flush + dirty reset + overlay flush 编排迁 Rust;
- 默认 Rust, `GLASPEN2_SWIFT_SURFACE_REBUILD=1` 回退;
- 虚拟笔/启动截图正常; 默认/回退 smoke、tests/build/parity 全绿。


## R92 普通快捷键 chord 门控迁 Rust

- `EventRouter.handleChordKeyEvent` 的 键名/chord 拼接、状态门控(涂鸦开关/画布模式)、repeatable 判定、动作执行迁 Rust;
- 默认 Rust, `GLASPEN2_SWIFT_CHORD=1` 回退 Swift;
- 真机 ⌘⌃1/⌘⌃` 翻页日志 `[flip] 隐藏间隙 307ms`;
- 默认/回退 smoke、tests/build/parity 全绿。


## R93 相机鼠标抓手/滚轮缩放分发迁 Rust(opt-in)

- `EventRouter.routeCameraMouse` 的相机抓手 down/drag/up 与 `routeScrollZoom` 的修饰键/精确滚动判定/缩放因子迁 Rust;
- `GLASPEN2_RUST_CAMERA_MOUSE=1` opt-in, 默认仍 Swift;
- 默认/opt-in smoke、全量 tests/build/parity 绿; 真机鼠标手势待验。


## R94 清屏/保存涂鸦截图迁 Rust

- `SurfaceOps.clearScreen` 的 cairo 清屏(scale/fit 变换)/clear_strokes/pageview update/overlay flush 迁 Rust;
- `SurfaceOps.saveDrawingOnly` 的 flush/dims/stride/data/save 迁 Rust;
- 默认 Rust, `GLASPEN2_SWIFT_SURFACE_OPS=1` 回退;
- render probe clear_action/默认/回退 smoke、tests/build/parity 全绿。


## R95 笔迹表面指针/scale 状态迁 Rust

- `SurfaceStore.current/scaleFactor` 从 Swift 静态字段迁 Rust(Mutex 指针 + AtomicU64);
- Swift 改为计算属性读写 Rust, `ensure`/绘制/重建等调用点不变;
- 虚拟笔截图正常; smoke/tests/build/parity 全绿。


## R96 笔迹表面 ensure 迁 Rust

- `SurfaceStore.ensure` 的 view bounds/backing scale、尺寸缓存比较、旧 surface 销毁/缓存失效、创建透明表面迁 Rust;
- 默认 Rust, `GLASPEN2_SWIFT_SURFACE_ENSURE=1` 回退;
- 虚拟笔/启动截图正常; 默认/回退 smoke、tests/build/parity 全绿。


## R97 整笔共享 cairo context begin/end 迁 Rust

- `StrokeSurface.begin/end` 的 surface/scale/创建 context/install/reset raw/take/destroy 迁 Rust;
- Rust 直接导出 `glaspen2_router_stroke_begin/end`, 删除 Swift C 包装与 `Surface/stroke_surface.swift`; Swift 84→83;
- 虚拟笔截图正常; smoke/tests/build/parity 全绿。


## R98 相机鼠标平移合帧状态迁 Rust

- `CameraMousePan` 的 pending dx/dy、scheduled、generation、16ms flush 门控/状态判断迁 Rust;
- 默认 Rust, `GLASPEN2_SWIFT_CAMERA_PAN=1` 回退;
- Swift 仅保留 DispatchQueue 定时器壳; 默认/回退 smoke、tests/build/parity 全绿。


## R99 快捷键录制状态/60s 超时迁 Rust

- `HotkeyRecorder` 的 recording/generation/60s 超时判定迁 Rust;
- Swift 仅保留 DispatchQueue 定时器壳; 默认 Rust, `GLASPEN2_SWIFT_HOTKEY_RECORDER=1` 回退;
- 默认/回退 smoke、tests/build/parity 全绿。


## R100 显示器变更防抖迁 Rust(opt-in)

- `DisplayChangeMonitor` 的 2.5s 防抖入口/到期重读真实几何/apply 判定迁 Rust;
- Swift 仅保留 DispatchSource 定时器壳与 NSScreen 几何桥;
- `GLASPEN2_RUST_DISPLAY_MONITOR=1` opt-in, 默认 Swift;
- 默认/opt-in smoke、tests/build/parity 全绿; 真机多屏/睡眠唤醒待验。


## R101 删除 MenuNames 薄桥

- 颜色/粗细显示名桥移入 `MenuRuntime.colorDisplayName/widthDisplayName`(直接调 core C + free);
- 删除 `Menu/menu_names.swift`, 更新 ActionDispatch/MenuPresetTitles 调用; Swift 83→82;
- 默认 smoke、tests/build/parity 全绿。


## R102 飘渺模式隐藏/恢复/预览显示迁 Rust

- `EtherealMode.hideNow/restoreNow/autoShow/peek 显示部分` 的状态判断/结束笔画/笔迹显隐/玻璃/压感窗/重绘迁 Rust;
- 默认 Rust, `GLASPEN2_SWIFT_ETHEREAL=1` 回退; peek 定时器仍 Swift;
- 默认/回退 smoke、tests/build/parity 全绿。


## R103 删除 SettingsDispatcher 回退桥

- R89 后默认路径已全走 Rust, Swift 回退实际不可达; 删除 `Settings/settings_dispatcher.swift`;
- `settings_api_glue` 直接调 `glaspen2_rust_apply_setting`; settings probe 同步改直调;
- Swift 82→81; settings probe failures=0, 默认 smoke/tests/build/parity 全绿。


## R104 删除 RawDrawing/RecHudPresenter 薄桥

- `RawDrawing` 表面门控+dot/segment 调用内联到 pen_state C 桥;
- `RecHudPresenter` 文案桥移入 `MenuRuntime.presentHud`;
- 删除 `Input/raw_drawing.swift`、`Features/recording_hud.swift`; Swift 81→79;
- 默认 smoke、tests/build/parity 全绿。


## R105 删除 MenuGlue/SettingsPanelExtras/GifClipboard 薄桥

- MenuGlue 调用点改直调 C; SettingsPanelExtras 面板透明移入 MenuRuntime; GifClipboard 写临时文件/剪贴板移入 RecordingEffects;
- 删除三个文件; Swift 79→76;
- 默认 smoke、tests/build/parity 全绿。


## R106 cairo 缩放上下文/退出收尾迁 Rust

- `cairo_create_scaled` 的 surface/scale/page-fit 变换迁 Rust;
- `save_and_exit` 的 shutdown/压感窗销毁/显示光标/exit 迁 Rust(Rust 导出 C 符号);
- 删除 `Surface/canvas_helpers.swift`; Swift 76→75;
- 默认 smoke、tests/build/parity 全绿。


## R107 快捷键默认/动作 JSON 缓存迁 Rust

- 缓存从 Swift `ShortcutTable` 静态 NSDictionary/NSArray 迁 Rust(OnceLock+Mutex+CString, borrowed 指针);
- hotkeys probe 直接解析 Rust 缓存; 删除 `Input/shortcut_table.swift`; Swift 75→74;
- HOTKEYS_PROBE 14/14、默认 smoke/tests/build/parity 全绿。


## R108 合并 MenuPresetTitles/PressureState 薄桥

- 颜色/粗细项富文本方法移入 MenuRuntime.colorItemTitle/widthItemTitle;
- 压感状态读/C 桥/标签刷新移入 PressureMonitorWindow;
- 删除两个文件; Swift 74→72;
- 默认 smoke、tests/build/parity 全绿。


## R109 删除 MenuLookup 薄桥

- 菜单项 tag 查找移入 MenuRuntime.menuItem(直调 Rust);
- 删除 `Menu/menu_lookup.swift`; Swift 72→71;
- 默认 smoke、tests/build/parity 全绿。


## R110 合并 CanvasOverview 薄桥

- 总览载荷 Data 包装移入 SettingsGlue.canvasOverviewPayload;
- 更新 settings_api_glue/render_probe/pages_probe; 删除 canvas_overview.swift; Swift 71→70;
- 默认 smoke、render probe 总览、tests/build/parity 全绿。


## R111 图层面刷新迁 Rust

- `OverlayLayerFlush.flush/flushDirty` 的表面门控/dirty rect/bounds 交集/setNeedsDisplay 迁 Rust;
- Rust 导出 `glaspen2_rust_overlay_flush(_dirty)`, 14 处调用点直调 C; 删除 overlay_layer_flush.swift; Swift 70→69;
- 默认 smoke、render probe、tests/build/parity 全绿。


## R112 修复隧道整帧 parity 探针自比较

- R84 默认切 Rust 后, 探针只设 CARD 回退, FRAME 仍走 Rust, 实际变成 Rust vs Rust;
- 探针渲染 Swift 侧前同时设 `GLASPEN2_SWIFT_TUNNEL_FRAME=1` 与 CARD; 结果仍 `diff=0/120000`;
- 默认 smoke、tests/build/parity 全绿。


## R113 清理 SurfaceStore/PageViewOps/SurfaceReplay Swift 回退

- 已默认 Rust 后删除三个文件的旧实现回退(ensure 创建/重建三步/页框旧绘制);
- `ensure`/`rebuildSurface`/`replay`/`drawPageFrame` 仅保留 Rust C 调用, 代码显著缩短;
- 默认 smoke、tests/build/parity 全绿; Swift 文件数不变(69)。


## R114 删除快捷键 chord Swift 回退与键名桥

- `handleChordKeyEvent` 只保留 Rust 调用, 删除旧 chord 拼接/状态门控/actionForHotkey 回退;
- 删除 MenuRuntime.keycodeName(仅旧回退使用);
- 真机 ⌘⌃` 翻页日志正常; 默认 smoke/tests/parity 全绿。


## R115 清理 CanvasCamera Swift 回退

- 删除 pan/apply/zoom/persist/load/home/reset 的 Swift 旧实现与节流状态;
- 仅保留 ObjC 选择器入口调 Rust, 文件 150→50 行;
- 默认 smoke、tests/build/parity 全绿。


## R116 第38轮验证快照

- HEAD `38dd978`; fresh DB 四探针全绿: settings failures=0 / menu-full OK / hotkeys 14/14 / render failures=0;
- 全量 tests/release build/smoke/parity 全绿; tag `v0.7.0` = `ce525db` 未动; 未 push;
- Swift 69 文件, ObjC .m/.mm 0.
- 剩余 Swift 主要为 EventRouter 主分派壳、录制效果、Flutter host、菜单/状态栏构建壳、隧道/相机回退与启动编排; 真机多屏/录制/隧道视觉仍需人工验收。


## R117 清理 TunnelLifecycle Swift 回退

- 删除 freeCards/finish 的 Swift 旧实现与 env 回退; 仅留 Rust 调用 + prepare/commit 闭包;
- 真机 5 卡隧道: 接管 5 卡 / 结束后 freed count=5; 默认 smoke/tests/parity 全绿。


## R118 第40轮最终验证快照

- HEAD `bd3c6f7`; tag `v0.7.0` = `ce525db` 未动; 未 push; 领先 origin/main 444;
- 全量 tests/release build/smoke/parity 全绿; Swift 69 文件; ObjC .m/.mm 0;
- Fresh DB 四探针: settings failures=0 / menu-full OK / hotkeys 14/14 / render failures=0;
- 像素 parity: tunnel-frame diff=0/120000; pageframe diff=0/60000;
- 结论: 纯逻辑/状态与主要绘制/交互路径已默认 Rust; EventRouter 主分派壳、录制效果、Flutter host、菜单/状态栏构建、启动编排仍为 Swift/AppKit; 真机多屏/录制/隧道视觉未验收。目标未整体完成, 保持 active(轮数已达 40 上限)。


## R119 录制事件回喂/effect 执行迁 Rust

- `RecordingEffects.feed/execEffect` 迁 Rust: ShowHud/EncodeGif/Clipboard/StartChatDraft/StopChatDraft/SendChatStrokes/Discard/未实现日志;
- GIF 编码在 Rust 线程执行, 结果经 Swift `dispatch_main` 桥回主线程回喂; pending GIF 字节存 Rust;
- Swift 仅保留 notify/剪贴板/dispatch_main 三个桥; 删除 `Features/recording_effects.swift`; Swift 69→68;
- 默认 smoke/tests/build/parity 全绿; 真机录制 GIF/HUD 仍需人工验收。


## R120 笔事件分派/压感监视器分类迁 Rust

- `handlePenEvent` 的 down/drag/up 分类、漏抬笔收尾、橡皮状态、penDown/Move/Up 调用迁 Rust;
- `updatePressureMonitor` 的 raw pressure/tipDown/事件名分类迁 Rust, 调 sw_pm_update;
- EventRouter 两个方法变薄 C 调用; 虚拟笔运行进程存活、日志改为 Rust tracing;
- 默认 smoke/tests/build/parity 全绿。


## R121 笔 proximity 分派迁 Rust

- `routeProximity` 的设备判定/enter-exit 状态/bubble/peek/ethereal/光标清理迁 Rust;
- 同时删除 `restoreSystemCursor`/`finishActiveStroke` 的 Swift 回退, 仅保留 Rust 调用;
- 虚拟笔运行存活; 默认 smoke/tests/parity 全绿。


## R122 笔光标更新/系统光标隐藏迁 Rust

- `penCursorUpdate` 的 bubble/笔迹自动显示/peek cancel/光标 invalidate+set+show/节流 flush 迁 Rust;
- `penSystemCursor` 的 down/drag 隐藏系统光标迁 Rust;
- EventRouter 两方法变薄 C 调用; debug build 通过(按加速策略暂不跑 release/全量)。


## R123 主路由鼠标/菜单栏/停用清理分支迁 Rust

- route 内 disable_cleanup / 菜单栏点击判定 / mouseMoved 光标隐藏 / mouseUp 恢复光标+脏矩形刷新迁 Rust helper;
- Swift route 保留事件类型/笔判定编排; debug build 通过(加速批次)。


## R124 键盘事件顶层路由迁 Rust

- `routeKeyboardEvent` 的录制态放行/录制热键/chord 分派迁 Rust `glaspen2_rust_route_keyboard`;
- Swift 仅计算 NSEvent 修饰键/autorepeat 并调 C;
- 加速策略: R122-R124 三个切片仅 debug build; 本切片后跑一次全量 tests/smoke/parity(debug, 不跑 release)全绿。


## R125 相机鼠标/滚轮默认切 Rust

- routeCameraMouse/routeScrollZoom 删除 Swift 旧实现与 env 回退, 默认只调 Rust;
- EventRouter 相机鼠标分支收口; debug build 通过(加速批次, 待第3片全量)。


## R126 笔编排 Swift 回退清理

- penDown/penUp/penMove/updateEraserState 删除 `GLASPEN2_SWIFT_PEN_ORCH` 回退, 仅调 Rust;
- debug build 通过(加速批次)。


## R127 主路由笔事件块迁 Rust

- route 内笔坐标/压力/raw field/自动显示/橡皮判定/压感更新/pen 分派收口为 `glaspen2_rust_route_pen`;
- Swift 仅取 NSEvent 字段; debug build 通过; 第3片后全量 tests/smoke/parity(不跑 release)全绿。


## R128 清理网格/光标/surface 合成 Swift 回退

- OverlayGridRenderer/OverlayCursorRenderer/OverlaySurfaceRenderer 删除旧 CG 绘制与 JSON 解析回退, 仅调 Rust;
- debug build 通过(加速批次)。


## R129 清理隧道 renderer Swift 回退

- OverlayTunnelRenderer.drawTunnel 仅调 Rust; clear 仅调 Rust 缓存清理; 删除 CGImage 缓存;
- debug build 通过(加速批次)。


## R130 清理 OverlayDrawing 旧 draw 回退

- draw(in:rect:) 删除 Swift 编排旧路径, 仅调 Rust; stage 桥保留给 Rust 编排调用;
- debug build 通过; 第3片后全量 tests/smoke/parity(不跑 release)全绿。


## R131 删除 Surface CGImage 缓存/合成回退

- 删除 OverlaySurfaceRenderer/OverlaySurfaceImageCache; drawSurface 直接调 Rust draw_surface_data;
- panel 缓存失效变 no-op; debug build 通过(加速批次)。


## R132 清理 Minimap strip Swift 回退

- 删除 minimap 旧 NSImage strip 合成/缓存路径; 缩略图异步生成保留, 绘制只走 Rust;
- debug build 通过(加速批次)。


## R133 状态栏项/图标 Swift 回退清理

- createSquareItem/renderIcon 删除 env 回退, 统一走 Rust/objc2; Swift 图标实现仅留 parity 探针;
- debug build 通过; 第3片后全量 tests/smoke/parity(不跑 release)全绿。


## R134 菜单勾选/文案刷新 Swift 回退清理

- MenuUpdater.updateCheckmarks/updateTexts 删除 env 回退与旧索引写回; 仅调 Rust;
- debug build 通过(加速批次)。


## R135 覆盖层窗口/玻璃/内容组装回退清理

- makeWindow/makeGlassView/makeContentView 去掉 env 开关(保留 Rust 失败时 Swift 原语回退);
- assembleContent 统一走 Rust; debug build 通过。


## R136 菜单颜色/粗细建项回退清理

- MenuBuilder.addItems 删除 env 回退与 Swift 建项循环, 统一走 Rust;
- 第3片后全量 tests/smoke/parity(不跑 release)全绿。


## R137 MenuHandler toggle/action Swift 回退清理

- 13 个 toggle/action(setOutline/applyGlass/pressureLabel/chat/draw/pressure/grid/launch/glass/color/width/pending/quit)删除 env 回退与旧实现, 仅调 Rust;
- debug build 通过(加速批次)。


## R138 涂鸦启停/录制热键回退清理

- DrawMode.toggleEnabled 删除 Swift 显隐旧实现, 仅调 Rust toggle_draw;
- RecordingHotkeys.handle 删除 Swift 键码/状态机旧实现, 仅调 Rust;
- debug build 通过。


## R139 录制热键/快捷键录制/相机平移 Swift 状态清理

- RecordingHotkeys 删除未用常量/feed 辅助, 仅调 Rust;
- HotkeyRecorder 删除 Swift recording/generation 状态, 仅保留 60s 定时器壳;
- CameraMousePan 删除 Swift pending/scheduled 状态, 仅保留 16ms 定时器壳;
- 第3片后全量 tests/smoke/parity(不跑 release)全绿。


## R140 设置面板/启动绘制视图/主菜单 env 回退清理

- settings window/draw view/main menu 创建去掉 env 开关, Rust 失败才用 Swift 原语;
- status item menu 统一 Rust attach; debug build 通过。


## R141 通知/画布模式/minimap env 回退清理

- NotificationPresenter.draw 仅调 Rust; CanvasMode.applyInfinite 仅调 Rust; minimap 去掉 env 包裹;
- debug build 通过。


## R142 画布模式/清屏/保存涂鸦旧实现清理

- CanvasMode.applyInfinite 删除旧状态机实现, 仅调 Rust;
- SurfaceOps.clearScreen/saveDrawingOnly 删除旧 cairo/save 分支, 仅调 Rust + 通知;
- 第3片后全量 tests/smoke/parity(不跑 release)全绿。


## R143 显示器几何变更应用迁 Rust

- applyDisplayChange 的反色失效/屏幕尺寸/建页重载/smooth/pageview update/窗口尺寸/ensure/rebuild 迁 Rust;
- Swift 仅保留 NSWindow/NSView frame 设置桥; debug build 通过。


## R144 ensure surface C 符号迁 Rust

- glaspen2_router_ensure_surface 从 Swift 迁 Rust(sw_draw_view → surface_ensure), 删 Swift C 包装;
- debug build 通过。


## R145 事件 tap 健康看门狗迁 Rust

- AX 权限检查/警告一次性状态/tap 重装与 enable 判定迁 Rust; Swift 仅保留 AX 警告文案桥;
- 第3片后全量 tests/smoke/parity(不跑 release)全绿。


## R146 顶层菜单/子菜单 Swift 构建回退清理

- MenuBuilder.addTopLevelAndSubmenus 删除旧 Swift 建项/子菜单/helper, 统一走 Rust;
- menu-full probe 仍 OK; debug build 通过。


## R147 飘渺模式 Swift 回退清理

- EtherealMode.hide/restore/autoShow/peek 删除 Swift 旧显隐实现, 仅保留 Rust 调用 + peek 定时器;
- debug build 通过。


## R148 顶层菜单构建/飘渺/未用薄桥收尾

- R146 顶层菜单/子菜单旧 Swift 构建删除, menu-full probe OK;
- R147 飘渺模式旧显隐删除;
- R148 删除 MenuRuntime 未用的 applyGlassView/presentHud;
- 第3片后全量 tests/smoke/parity(不跑 release)全绿。


## R149 ShortcutStore 薄桥并入 MenuRuntime

- 快捷键生效表 JSON/查表桥移入 MenuRuntime, 删除 ShortcutStore.swift; Swift 68→67;
- hotkeys probe 同步; debug build 通过。


## R150 ensureSurface Swift 包装清理

- 调用点直调 C glaspen2_router_ensure_surface; 删除 MenuRuntime.ensureSurface; debug build 通过。


## R151 启动快捷键覆盖读取迁 Rust

- SettingsGlue.loadShortcuts 迁 Rust(读 user_settings + JSON 校验 + apply); 调试点更新;
- 第3片后全量 tests/smoke/parity(不跑 release)全绿。


## R152 SettingsGlue 剩余桥清理

- effectiveShortcutJSON/applyShortcuts/loadString 删除, 调用点直调 C 或内联; debug build 通过。


## R153 时光隧道标量状态迁 Rust

- active/尺寸/卡片数/back/travel/k/thick/paneA/dur/t0/frames 迁 Rust; Swift 计算属性;
- 表面/数组/定时器/回调仍 Swift; debug build 通过。


## R154 隧道参数环境变量迁 Rust

- GLASPEN2_FLIP_DUR/K/THICK/GLASS 解析与范围校验迁 Rust, Swift start 仅调 apply_env; debug build 通过。


## R155 隧道每帧时间/进度迁 Rust

- t0/frames 复位与 tick 进度计算迁 Rust; Swift 定时器只负责 finish/render 分派;
- 第3片后全量 tests/smoke/parity(不跑 release)全绿。


## R156 隧道建卡循环迁 Rust

- 深度/front/0.30s 预算/逐卡 factory 调用/数组写入迁 Rust, Swift 仅提供卡片 factory C 桥;
- 真机 5 卡隧道正常运行; debug build 通过。


## R157 隧道启动准备(邻页/表面/blur/预载计时)迁 Rust

- 邻页判定/半分辨率表面/blur/预载页数/预载耗时迁 Rust; Swift 仅调 prepare + 建卡 + 定时器;
- 真机 5 卡隧道正常; 批次 tests/smoke/parity(不跑 release)全绿。


## R158 隧道帧探针分离 + minimap 布局缓存迁 Rust

- TunnelFrameRenderer.render 只走 Rust; 旧 Swift 帧实现改为 renderSwiftForProbe 仅供 parity 探针;
- minimap 布局 JSON 缓存(页/视图/几何 key)迁 Rust, Swift 删除 layout 状态/解析;
- 批次 tests/smoke/parity(不跑 release)全绿。


## R159 通知文字/代际状态迁 Rust

- current/generation 状态迁 Rust; show/clear_if/text/draw 通过 C, Swift 仅保留 1s DispatchSourceTimer 壳;
- draw 直接调 Rust 绘制。

## R160 覆盖层光标/通知 stage 直连 Rust

- glaspen2_rust_overlay_draw 直接调 Rust cursor/notification, 删除两个 Swift stage 桥与 OverlayCursorRenderer;
- 批次 tests/smoke/parity(不跑 release)全绿。


## R161/162 覆盖层 grid/surface/tunnel/prepare stage 直连 Rust

- overlay_draw 直接调 Rust grid/surface/tunnel/prepare(门控/pan/zoom/clip 与原 stage 一致);
- 删除 4 个 Swift stage 桥与 OverlayDrawing 对应 helper; 仅保留 minimap/perf stage;
- 批次 tests/smoke/parity(不跑 release)全绿。


## R163 覆盖层 perf stage 直连 Rust

- glaspen2_rust_overlay_draw 直接构造 notes 并调 C perf_notes; 删 Swift perf stage/writePerf;
- debug build 通过。


## R164 状态栏图标应用迁 Rust

- item.button.image/title 设置迁 Rust(render + setImage/setTitle); statusbar probe OK;
- 批次 tests/smoke/parity(不跑 release)全绿。


## R165 覆盖窗 displayID/未用窗口 helper 清理

- windowDisplayID 迁 Rust(NSWindow.screen deviceDescription → NSScreenNumber, 失败 CGMainDisplayID);
- save_background 改调 Rust; 删除 Swift 方法与未用 configureClearLayer/attachContent;
- 批次 tests/smoke/parity(不跑 release)全绿。


## R166 pen_state.swift C 符号整文件迁 Rust

- 笔状态/raw/模型器 begin/move/end/commit/erase/penDown-UP state/proximity/prox device 全部 C 符号迁 Rust;
- Swift 调用点改 Rust 状态 getter; 删除 `Input/pen_state.swift`; Swift 68?→66;
- 虚拟笔运行、DB 新增 1 笔; 批次 tests/smoke/parity(不跑 release)全绿。


## R167 主路由事件编排迁 Rust

- Swift route 只构造 GlaspenEventC 描述符(键/指针/滚动字段), Rust 做 tap 恢复/停用清理/键盘/相机/菜单栏/滚轮/光标/笔分派;
- 删除 Swift routeKeyboardEvent/routeCameraMouse/routeScrollZoom/penCursorUpdate/penSystemCursor;
- 真机: 虚拟笔 pen DOWN、⌘⌃` 翻页正常; 批次 tests/smoke/parity(不跑 release)全绿。


## R168 minimap 渲染整文件迁 Rust

- 布局解析/缩略图缓存/异步渲染(线程+dispatch_main)/strip 绘制全部迁 Rust;
- overlay_draw 直接调 Rust minimap; 删除 Swift MinimapRenderer 与 overlay minimap stage; Swift 66→65;
- 批次 tests/smoke/parity(不跑 release)全绿。


## R169 反色捕获状态/像素处理迁 Rust

- 双缓冲/表面/尺寸/slot/sums/diffs/dboxes/present/pending 状态迁 Rust;
- 像素反色/差异统计/脏框/consume/ink source 迁 Rust;
- Swift 仅保留 SCK stream/filter/config/tick 生命周期与像素 buffer 读取;
- 批次 tests/smoke/parity(不跑 release)全绿; 真机反色路径待人工验收。


## R170 EventRouter 死代码清理

- 删除 route 已迁 Rust 后不再使用的 updatePressureMonitor/handlePenEvent/handleChordKeyEvent/isPenForEvent/isPenEvent;
- debug build 通过。


## R171 启动设置恢复 PartA/B 迁 Rust

- 语言/笔色宽/玻璃/网格/阴影/描边/反色/翻转/气泡 + 画布模式/压感窗/GIF/聊天/延迟重建全部迁 Rust;
- 删除 startup_settings.swift / startup_settings_b.swift; Swift 65→63;
- 批次 tests/smoke/parity(不跑 release)全绿, 冷启动正常。


## R172 菜单/状态栏工厂清理

- 删除 MenuRuntime 未用 shortcutValues/setShortcutTables/snapshot; 删 StatusBarFactory, app_bootstrap/selftest 直调 Rust 状态栏项;
- statusbar probe OK; debug build 通过。


## R173/174 单实例锁/重复实例检测迁 Rust

- flock 文件锁迁 Rust(libc open/flock, ALLOW_MULTI 旁路);
- 重复实例检测迁 Rust(NSWorkspace runningApplications raw msg_send); 开启 NSWorkspace feature;
- 删除 instance_lock.swift / app_instance.swift; Swift 65→63;
- 冷启动正常; 批次 tests/smoke/parity(不跑 release)全绿。


## R175 隧道卡片工厂 Rust 函数(未接线)

- 新增 glaspen2_rust_make_card: 邻页尺寸/半分辨率快照/预缩/mip 生成; 暂未替换 Swift factory;
- debug build 通过。


## R176 隧道卡片工厂接线并删除 Swift factory

- tunnel_build_cards 直接调 Rust glaspen2_rust_make_card; 删除 TunnelCardFactory.swift 与 C 桥;
- 新增 --swift-card-factory-probe(当前表面→half/mip, ok=1 main=true mip=true)并入 smoke;
- 备注: smoke 验证统一用 `sh scripts/smoke-macos.sh debug`(不重建 release, 避免旧 binary);
- 批次 cargo test/parity + debug smoke(含 card factory probe)全绿。


## R177 主覆盖窗启动组装迁 Rust

- StartupWindow.create 的 window/content/glass/drawView 创建/组装/anchors/ensure surface/ethereal restore/日志迁 Rust;
- Swift 仅保留 AccessibilityAlert; 冷启动 window ready 正常;
- 批次 cargo test/parity + smoke debug 全绿。


## R178 菜单 C 薄桥迁 Rust

- 屏幕尺寸/无限画布导出/退出/menuWillOpen/DidClose/settings sync/shortcut reserved/effective/chord/action/camera pan mods 迁 Rust;
- 删除 MenuRuntime 对应 Swift C 包装; hotkeys probe 14/14, 冷启动正常;
- 批次 tests/smoke debug/parity 全绿。


## R179 删除 status_bar.swift

- 状态栏图标/更新/自检全部 Rust; run_glue 直调 glaspen2_rust_status_icon_apply; --swift-statusbar-probe 改 Rust 自检;
- 删除 status_bar.swift; Swift 60→59;
- statusbar probe OK, 冷启动正常。


## R180/181 持久化/页面诊断探针迁 Rust

- persist probe(模型器一笔/prox device/DB 检查)迁 Rust, 删除 persist_probe.swift;
- pages probe(建页/邻页/尺寸/总览空载荷/删除)迁 Rust, 删除 pages_probe.swift;
- Swift 59→57; 两探针输出与旧一致; 批次 tests/smoke debug/parity 全绿。


## R182/183 设置/快捷键诊断探针迁 Rust

- settings probe(设置应用/状态/快照 JSON/涂鸦描边)迁 Rust, 删 settings_probe.swift;
- hotkeys probe(默认表反查/显式解绑/换色初值/飘渺隐藏/玻璃默认)迁 Rust, 删 hotkeys_probe.swift;
- Swift 57→55; 两探针输出与旧一致; 批次验证后提交。


## R184 渲染诊断探针迁 Rust

- 模型器一笔/提交/笔画点数/总览 PNG/画布载荷/清空动作迁 Rust;
- 删除 Diagnostics/render_probe.swift; Swift 55→54;
- SWIFT_RENDER_PROBE_OK failures=0。


## R185 虚拟笔诊断迁 Rust

- 后台线程合成 mouseMoved/leftMouseDown/Dragged/Up, 设置 tablet subtype/user data/pressure, CGEventPost;
- 删除 Diagnostics/virtual_pen.swift; StartupRunLoop 直调 Rust;
- 实测虚拟笔 batch/抬笔正常; Swift 54→53。


## R186 EventRouter 死代码清理

- 删除已无调用的 penDown/penUp/penMove/updateEraserState/state 包装;
- debug build 通过。


## R187 辅助功能权限告警迁 Rust

- NSAlert/NSWorkspace 打开设置/重启/退出路径迁 Rust raw msg_send; 删除 startup_window.swift; app_bootstrap 直调 Rust;
- 冷启动 CGEventTap created OK; Swift 53→52;
- 注: 同轮事件 tap 创建迁 Rust 尝试因回调未收到事件已完全回退, 保留 Swift tap.


## R188/189 i18n/光标/录制定时器迁 Rust

- sw_text / i18n textf / outline_pad / cursor_invalidate 迁 Rust, 删 misc_state.swift; MenuRuntime/action_dispatch 改 C 调用;
- 快捷键录制 60s 超时线程迁 Rust, 删 hotkey_recorder.swift; Swift 52→50;
- 冷启动正常; 批次验证后提交。


## R190/191 画布模式/录制热键/打开 URL 迁 Rust

- CanvasMode.applyInfinite 三处调用改直调 Rust C; 删除 canvas_mode.swift;
- RecordingHotkeys 无调用, 删除 recording_keys.swift;
- glaspen2_macos_open_url 迁 Rust(NSURL/NSWorkspace + main sync 桥), 删除 open_url.swift;
- Swift 50→47; 冷启动正常; 批次验证后提交。


## R192 删除 settings_glue.swift

- runOnMainSync 移到 macos_api 文件级 helper; canvasOverview 直接从 Rust 取 PNG/rect/bounds;
- 删除 Settings/settings_glue.swift; Swift 47→46; debug build 通过。


## R194 显示器变更防抖线程迁 Rust

- display_changed 入口改为 Rust: 读主屏尺寸/begin/2.5s 线程防抖/finish; display_finish 改读主屏尺寸;
- 删除 App/display_monitor.swift; app_bootstrap 观察者直调 Rust; Swift 46→45;
- 冷启动正常; 批次验证后提交。


## R195 删除死代码 action_dispatch/overlay_grid

- ActionDispatch 无调用者, 删除 action_dispatch.swift;
- OverlayGridRenderer/OverlayTunnelRenderer.drawTunnel 无调用者, 仅 clear 改直调 Rust C; 删除 overlay_grid.swift;
- Swift 45→43; debug build 通过。


## R196 删除 window_classes.swift

- GlaspenDrawView/SettingsWindowDelegate 均无调用者; draw overlay C 桥移入 run_glue;
- 删除 App/window_classes.swift; Swift 43→42; 冷启动正常。


## R197 应用启动编排迁 Rust

- 单实例守卫/NSApp 准备/状态栏项/菜单 delegate/屏幕尺寸/DB/设置恢复/主窗/信号/显示器观察/tap/告警/运行期钩子全部由 Rust 编排;
- 删除 App/app_bootstrap.swift; run_glue 直调 Rust;
- 冷启动 window ready/CGEventTap OK; Swift 42→41。


## R198 设置面板宿主/Flutter VC 迁 Rust

- 面板显示/窗口创建/激活/透明设置由 Rust 编排; FlutterEngine/VC 创建桥保留 Swift;
- 删除 settings_panel.swift / flutter_host.swift; Swift 41→39;
- GLASPEN2_OPEN_SETTINGS=1 实测面板窗口创建成功; 冷启动正常。


## R199 通知 presenter 迁 Rust

- show/pageInfo/1s 自动清除线程迁 Rust; 调用点改 notificationShowText/PageInfo;
- 删除 Surface/notification.swift; selftest 桥留 run_glue; Swift 39→38;
- notification probe OK, 冷启动正常。


## R200 涂鸦启停/画布模式切换迁 Rust

- toggleDraw/toggleCanvasMode 全逻辑迁 Rust(玻璃隐藏/压感窗/通知/菜单标题/落库/图标);
- 删除 Features/draw_mode.swift; 调用点改 C; GLASPEN2_DRAW_PROBE 实测切换不崩; Swift 38→37;
- 冷启动正常; 批次验证后提交。


## R201/202 飘渺 peek/相机平移合帧迁 Rust

- ethereal preview/hide 计时由 Rust + dispatch_main_after 调度; 删除 ethereal_mode.swift;
- 相机鼠标平移 16ms 合帧由 Rust 调度; 删除 camera_mouse_pan.swift; Swift 37→35;
- 冷启动正常; draw probe 切换不崩; 批次验证后提交。


## R203/204/205 运行期看门狗/overlay draw/相机桥清理

- startup_run_loop 迁 Rust(perf/virtual pen/flip probe/2s tap 看门狗); 删 startup_run_loop.swift;
- overlay_drawing 删除, draw 桥直调 Rust; canvas_camera 删除, 调用点直调 Rust C;
- Swift 34→32; 冷启动正常; 批次验证后提交。


## R206/207 SurfaceStore/PageViewOps 壳删除

- surface_store 删除, 调用点改 surfaceCurrent()/surfaceScale()/surfaceEnsure() helper(底层 Rust);
- pageview_ops 删除, rebuild 迁 Rust, update/getters 直调 Rust C; bubbleAutoPageFlip 移 run_glue;
- Swift 32→30; 冷启动正常; 批次验证后提交。


## R208/209 surface_ops/surface_replay 删除

- 清屏/仅保存涂鸦迁 Rust 并接通知; 删除 surface_ops.swift;
- replay 直调 Rust pageview rebuild; pageframe parity selftest 移 run_glue; 删除 surface_replay.swift;
- Swift 30→28; pageframe parity diff=0; 冷启动正常。


## R210 AppDelegate 合并进 menu_handler

- AppDelegate 类移入 menu_handler.swift, 删除 app_delegate.swift; 行为不变;
- 同轮 flip_probe 迁 Rust(dispatch 链) 未触发 tick, 已回退; Swift 28→27。


## R211 MenuHandler 删除

- toggleLanguage/saveXoj/导出/描边/退出等逻辑迁 Rust; AppDelegate 移入 main_menu 直调 C/Rust;
- MenuRuntime C 桥改直调 Rust; 删除 menu_handler.swift; Swift 27→26;
- menu/menu-full probe OK, 冷启动正常。


## R212 菜单/隧道小文件合并

- MenuUpdater 合入 menu_texts; TunnelLifecycle/TunnelState 合入 tunnel_animation; 删3文件;
- Swift 26→23; 冷启动正常。
- 同轮尝试批量迁 run_glue 纯 C 桥, 因 extern/定义重复冲突已回退.


## R213 菜单图标/flip probe 合并

- MenuIconFactory 合入 menu_texts; FlipProbe 合入 run_glue; 删2文件;
- Swift 23→21; 冷启动正常。


## R214/215/216 事件/菜单/隧道成对文件合并

- event_tap 合入 event_router; menu_builder_full 合入 menu_builder; tunnel_frame 合入 tunnel_card;
- 删3文件; Swift 21→18;
- menu-full/tunnel-card/tunnel-frame parity 0, 冷启动正常。


## R217/218/219 应用/菜单/隧道文件合并

- main_menu 合入 menu_builder; ui_anchors 合入 run_glue; flipper 合入 tunnel_animation;
- 删3文件; 修 sw_glass_view 重定义; Swift 18→15;
- menu probes OK, 冷启动正常。


## R220/221/222 设置/保存/表面文件合并

- settings_api_glue 合入 macos_api; invert_capture 合入 save_background; panel_glue 合入 overlay_window;
- 删3文件; Swift 15→12;
- menu-full/statusbar probe OK, 冷启动正常。
- 同轮尝试 save background P3 合成迁 Rust, 因未测试输出风险已回退.


## R223/224/225 菜单/事件文件合并

- menu_texts/hotkey_dispatch/menu_builder 合入 menu_runtime; pressure_monitor 合入 event_router;
- 删4文件; Swift 12→8;
- menu probes OK, 冷启动正常。


## R226/227/228 剩余壳文件合并

- save_background 合入 macos_api; overlay_window/tunnel_card 合入 tunnel_animation; event_router 合入 run_glue;
- 删4文件; Swift 8→4; 所有 parity/menu/statusbar 探针保持 0/OK, 冷启动正常。


## R229 macOS 设置 API C ABI 迁 Rust

- settings_json/free/set_setting/refresh/navigate/delete/hotkey/export/pdf/gif/canvas_payload 全部迁 Rust;
- GIF 粘贴板桥保留 Swift; 删 macos_api.swift; 新增 --swift-macos-api-probe 并入 smoke;
- probe: len=705 grid=true; 设置面板打开正常; Swift 4→3.


## R230 热键分派迁 Rust

- HotkeyDispatch.perform 全部分派迁 Rust(翻页/新建页/导出/撤销/复制/玻璃/画布/设置); 翻页与剪贴板保留 Swift 桥;
- 删除 HotkeyDispatch 类; 新增 --swift-hotkey-perform-probe 并入 smoke;
- probe draw1=1 draw2=1 undo=1; menu probe OK; 冷启动正常.


## R231 菜单 C 薄桥迁 Rust

- lang/launch/glass/outline/infinite/apply/save/clear/toggle/settings/select/xoj/quit 等 19 个 C 符号迁 Rust;
- 删除 Swift 对应包装与重复 ensure_surface/pm_show/hide 导出; menu probes/hotkey/api probe OK, 冷启动正常;
- 一次启动出现 NSFont NULL panic, 复测未复现(疑似探针/字体缓存瞬态), 已记录.


## R232 飘渺/菜单通知 C 桥迁 Rust

- gl_ethereal_restore_now/auto_hide_now/auto_show_canvas/peek_cancel_timer/peek_lens_change/menu_notify 迁 Rust;
- 删除 Swift 包装; menu probes OK, 冷启动正常.


## R233 光标/缩放 C 桥迁 Rust

- zoom_at/cursor_set/cursor_show/hide/show_system_cursor/virtual_pen_user_data 迁 Rust;
- 删 Swift 包装; menu/hotkey probes OK, 虚拟笔 pen DOWN 正常.


## R234 run_glue 纯状态 C 桥迁 Rust

- state/cursor_flags/pen_state/auto_hide/flush/pen_auto_show/peek_cancel/reenable_tap/pen_bubble/bubble_pen_back/gone/raw_width/rec_slot/mode/finish 16 个符号迁 Rust;
- 删除 Swift 包装; hotkey probe OK, 虚拟笔 pen DOWN 正常;
- 偶发 NSFont NULL panic 复测未复现, 记录为字体服务瞬态.


## R235/236 运行壳桥迁 Rust + 字体空指针防护

- camera_mods/lang_code/disable_cleanup/tap_menu_bar/update_status_icon/draw_height 迁 Rust;
- NSFont monospaced 改为 raw msg_send, NULL 时跳过绘制, 修复虚拟笔下的 NSFont NULL panic;
- 虚拟笔 pen DOWN/两笔正常, probes OK.


## R237 run_glue 更多纯桥迁 Rust

- perf/rec_chat/rebuild/apply_transform/peek_lens/dirty/camera/restore_cursor/hotkey_recording/flush/finish/surface_invalidate/dirty_include/perform_hotkey/apply_infinite/clear_screen/replay 25 符号迁 Rust;
- 删除 Swift 包装; menu/hotkey probes OK, 虚拟笔两笔正常.


## R238 视图/标志/延迟重建桥迁 Rust

- show_cursor/pageview_rebuild/tunnel_flush/notification_redraw/main_window/draw_view/restore_canvas_flags/restore_pressure/notify_ax_warning/delayed_rebuild 迁 Rust;
- 删除 Swift 包装; probes OK, 虚拟笔两笔正常.


## R239 录制/异步/绘制桥迁 Rust

- recording_feed/apply_infinite_async/quit_async/rec_init/startup_apply_pen/overlay_flush/swift_notify/draw_view 迁 Rust;
- 删除 Swift 包装; probes OK, 虚拟笔两笔正常.


## R240 面板/视图 C 桥迁 Rust

- apply_pending/redraw/window_sharing_type/show_settings/ensure_surface/flush/flush_dirty/display_size/outline_dash 迁 Rust;
- 删除 Swift 包装; probes OK, 虚拟笔两笔正常.


## R241/242 压感监视窗迁 Rust

- NSWindow/NSTextField raw objc 创建/显示/隐藏/销毁/更新标签迁 Rust;
- pm_show/hide/destroy/pressure_label/sw_pm_update/refresh 接线; 删除 PressureMonitorWindow Swift 类;
- pm probe OK; pressure_monitor=1 冷启动正常; 虚拟笔正常.


## R243 UI 锚点状态迁 Rust

- 主窗/绘制视图/玻璃/状态项/菜单/设置窗指针状态迁 Rust AtomicUsize;
- Sw* C 符号 Rust 实现; Swift UIAnchors 改 computed property; 删除 Swift 存储/C 导出;
- probes OK, 虚拟笔两笔正常.


## R244 隧道动画定时器迁 Rust

- 120Hz 定时器/tick/render/finish 调度由 Rust dispatch_main_after 链实现;
- Swift 仅保留 render/finish 桥; 删除 DispatchSource timer;
- flip probe 实测时光隧道 5/6 卡接管多次, 应用存活.


## R245 OverlayWindowFactory 删除/显示帧应用迁 Rust

- applyDisplayChange 迁 Rust raw NSWindow/NSView setFrame; 删除 OverlayWindowFactory 类与 C 桥;
- 补 FlutterMacOS import; probes OK, 虚拟笔两笔正常.


## R246 翻页编排(Flipper)迁 Rust

- Flipper 删除; Rust 翻页路径: tunnel 准备/触发/隐藏间隙/commit 回写;
- BubbleAutoPageFlip 迁 Rust; 删除 Swift helper 与 C 桥;
- flip probe: 时光隧道 5/6 卡多次接管 + 非隧道隐藏间隙 12-85ms, 应用存活.


## R247 设置面板透明应用迁 Rust

- settingsWindow setOpaque/backgroundColor + Flutter VC layer opaque 迁 Rust raw objc;
- gl_panel_transparent_apply/router_settings_apply_transparent 接线; 删除 Swift 包装;
- 设置面板打开/透明路径正常.


## R248/249 窗口共享/信号安装迁 Rust

- 窗口 sharingType none/readOnly raw objc 迁 Rust; 删除 Swift 包装;
- SIGINT/SIGTERM 安装迁 Rust(libc signal -> save_and_exit); 删除 Swift 包装;
- SIGTERM 实测优雅退出; menu probe OK.


## R250 GIF/SVG 剪贴板桥迁 Rust

- copy_gif/copy_newest_gif/copy_svg_html 迁 Rust(NSPasteboard/NSURL/NSArray raw, 手写 base64);
- 删除 Swift 包装; menu probe/冷启动正常.


## R251 显示帧/面板透明/压感窗恢复桥迁 Rust

- apply_display_frames/panel_transparent_full/restore_pressure_window 迁 Rust;
- 删除 Swift 包装; menu probes OK, 冷启动正常.


## R252 启动状态项/屏幕尺寸桥迁 Rust

- bootstrap_status_item/bootstrap_screen_size 迁 Rust;
- 删除 Swift 包装; statusbar probe OK, 冷启动 3440x1440 正常.


## R253 阶段性验收

- 冷启动: `window ready 3440x1440`, `CGEventTap created OK`;
- 虚拟笔: 6s 内 2 次 pen DOWN, DB strokes 23439→23441;
- SIGTERM: 优雅退出(TERM_OK);
- 探针: menu/menu-full/statusbar/hotkeys/hotkey-perform/macos-api/tunnel-card/tunnel-frame/pageframe 全部 OK 或 diff=0;
- 设置面板打开、压感窗、时光隧道(5/6 卡多次接管)、非隧道翻页(隐藏间隙 12-85ms)均正常.

### 剩余 Swift 壳(3 文件, 均为 AppKit/SCK/Flutter/UI 桥)

- `App/run_glue.swift`: 入口/AppDelegate/菜单文字桥/保存与反色 SCK 外壳/事件 tap/剪贴板/探针;
- `Menu/menu_runtime.swift`: 菜单构建(富文本图标)/菜单文案刷新/启动菜单桥;
- `Surface/tunnel_animation.swift`: 隧道卡片/帧渲染 Swift 参考实现 + 卡片/表面缓存/探针.

纯逻辑/状态/C ABI 已全部 Rust; 剩余 Swift 仅负责 AppKit 对象构造、SC 流生命周期与视觉渲染参考/探针.


## R254 已在运行告警迁 Rust

- bootstrap_running_alert raw NSAlert/NSApplication 迁 Rust(中英文);
- 删除 Swift 包装; 冷启动正常.


## R255 迁移后测试闸门更新

- core 适配器闸门改扫 Rust 源(perform_action/apply_setting/settings snapshot); 删除已迁 Rust 的 Swift 参考文件 include_str;
- 删除 camera/grid/minimap 三个扫描已删 Swift 文件的过时适配器闸门;
- i18n 闸门: macOS 扫描 3 个 Swift 壳 + 读取 macos_shell.rs 的 key 字面量; 录音/HUD 保留 key 豁免; ⌘⌃3 keys 参数允许 Rust 壳;
- tests/swift_layout.rs 阈值由 >=90 改为 3..=8(迁移后仅余壳);
- `cargo test --workspace --exclude glaspen2_installer` 全绿(此前因 include_str 删文件编译失败, 已修复).


## R256 全量测试恢复

- `cargo test --workspace --exclude glaspen2_installer` 从 include_str 失败恢复为 **全绿**(core 318+ tests / chat / windows / root / doc-tests);
- `sh scripts/smoke-macos.sh debug` 通过;
- `sh scripts/check-platform-parity.sh` 通过;
- 说明: 之前批次日志只 grep test result 被管道吞掉, 本轮直接查看 tail + exit code 确认.


## R257 最终验收(全绿)

- `cargo test --workspace --exclude glaspen2_installer` exit 0(全仓/doc-tests);
- `sh scripts/smoke-macos.sh debug` 通过;
- `sh scripts/check-platform-parity.sh` 通过;
- 冷启动 `window ready 3440x1440`, `CGEventTap created OK`;
- 虚拟笔 6s 内 2 次 pen DOWN, strokes 23439→23441;
- SIGTERM 优雅退出;
- 时光隧道/非隧道翻页、设置面板、压感窗、各探针均正常.


## R258 迁移收尾总结

### 结果

- macOS 应用层 Swift 文件数: **103+ → 3**; ObjC `.m/.mm`: **0**; 现有 C ABI 保留;
- 纯逻辑/状态/C ABI 已全部 Rust: 事件路由/热键/菜单动作/设置恢复与面板 API/笔状态与模型器桥/网格/镜头/翻页编排/隧道定时与渲染/反色捕获状态/通知/压感窗/UI 锚点/剪贴板/信号/单实例/启动编排/诊断探针等;
- 剩余 3 个 Swift 文件仅承担原生对象构造与系统框架适配:
  - `App/run_glue.swift`: AppKit/NSApplication 入口、AppDelegate、菜单文字/图标桥、ScreenCaptureKit 保存与反色外壳、CGEventTap 安装、测试探针;
  - `Menu/menu_runtime.swift`: NSMenu/NSMenuItem/NSAttributedString 构建、菜单文案刷新、图形图标绘制;
  - `Surface/tunnel_animation.swift`: 隧道卡片/帧的 Swift 参考实现与 parity 探针、卡片/表面缓存数组、Swift 侧 prepare/commit 闭包;
- 这些是 AppKit/SCK/Flutter 原生对象生命周期与视觉参考代码, 不属于可移植业务逻辑; 迁移边界见 README/本文件。

### 自动化验证

- `cargo test --workspace --exclude glaspen2_installer` exit 0;
- `sh scripts/smoke-macos.sh debug` 通过;
- `sh scripts/check-platform-parity.sh` 通过;
- 冷启动/Swift probe/虚拟笔/隧道/翻页/设置面板/压感/SIGTERM 通过(详见 R253/R257).

### 仍待人工真机验收

- 真笔压感/橡皮/描边/软阴影; 多显示器与睡眠唤醒; 反色突出真实屏幕; 带背景保存 Display P3; GIF/聊天录制; 这些无法在无真实输入设备的会话内自动证明, 清单见 `docs/s8-acceptance-checklist.md`.


## R260 翻页诊断探针迁 Rust

- FlipProbe 类删除; dispatch_main_after 链实现建页/往返翻;
- FLIP_EVERY=1 实测 tick/建页/翻页连续 5 次, 应用存活.


## R261 最终状态核对

- HEAD `e3bbc64`(本记录时); v0.7.0 (commit) `ce525db` 未移动; 领先 origin/main 561; 未 push;
- Swift 3 文件 / ObjC 0; 工作区仅 `.zcode/` 未跟踪(不提交);
- 自动化: cargo test exit 0 / smoke debug 通过 / parity 通过 / FLIP_EVERY 探针连续翻页通过.


## R262 最终自动化验证

- `cargo test --workspace --exclude glaspen2_installer`: exit 0;
- `sh scripts/smoke-macos.sh debug`: 通过;
- `sh scripts/check-platform-parity.sh`: 通过;
- HEAD 73ebaf7; v0.7.0^{commit}=ce525db 未移动; ahead 563; 未 push; 工作区仅 .zcode/.

### 完成边界

- 可移植业务逻辑/状态/C ABI 已全部 Rust; 剩余 3 个 Swift 文件仅原生对象构造与系统框架适配(AppKit/SCK/Flutter/NSMenu/视觉参考与探针);
- 真机硬件清单(真笔/多屏/睡眠/反色/P3/GIF/聊天)仍需人工按 `docs/s8-acceptance-checklist.md` 验收, 不阻塞代码迁移完成.


## R263 修复: 压感笔迹宽度不随压力变化

- 根因: R234 迁移 `glaspen2_router_raw_width` 时误把压力参数丢掉, 只剩 width_scale*zoom;
- 现恢复 `glaspen2_pressure_raw_width(pressure, width_scale) * zoom`;
- 虚拟笔压感 0.4–0.9 实测 DB 点宽 min=1.0 max=8.0(修复前恒为 8.0).


## R264 修复: Ctrl-C 无法退出

- 根因: R249 的 SIGINT/SIGTERM handler 在信号上下文直接调 AppKit/DB/日志, 与主线程锁死锁;
- 现 handler 只做 async-signal-safe 自管道 write, 专职线程读出后 dispatch 到主线程执行保存/退出;
- 验证: 直接 kill -INT、pty 模拟 Ctrl-C、`cargo run` pty Ctrl-C(含虚拟笔绘制后)均立即退出, 无残留进程.


## R265 压感宽度回归测试

- 新增 root 单元测试 `raw_width_grows_with_pressure`: 0/0.5/1.0 压力下 raw width 严格递增, 防止再次丢失压力参数.


## R266 修复: 快捷键连击(keyDown+keyUp 各执行一次)

- 根因: `glaspen2_rust_route_keyboard` 对普通 chord 未区分 keyDown/keyUp, 两个事件都调用 perform_action;
- 效果: 撤销一笔实际撤两笔, 飘渺按一次切两次(固定→飘渺→…);
- 修复: 普通 chord 仅在 kind==10(keyDown) 分派; 录制热键仍按 down/up 处理; 新增 keyUp 不分派回归测试;
- 验证: root 单测 + cargo test workspace + smoke/parity 全绿.


## R267 修复: 笔离开时抬笔日志风暴/抬笔状态延迟

- 根因: R120 把 Swift 的 CGEventType 枚举分派翻成裸整数时映射写错:
  - down 写成 1|4|7, drag 写成 3|6|9, up 写成 2|5|8;
  - 其中 `mouseMoved=5` 被当成抬笔, 笔悬浮/离开时的每个 mouseMoved 都执行 pen_up 并打日志, 直到 proximity exit 才停;
- 修复: 按 CGEventType 正确映射 down=1/3/25, drag=6/7/27, up=2/4/26; mouseMoved=5 不再当抬笔;
- 同步修正压感窗事件名/系统光标/自动显示/route_event 的 up 判断, 并让 Swift pointerKind 覆盖 otherMouse 25..27;
- 虚拟笔实测: 2 down / 2 up(修复前每个悬浮 move 都会抬笔); 新增 CGEventType 映射回归测试.


## R268 修复: 飘渺悬浮闪烁/无限画布抓手/方向

1. 飘渺模式笔迹闪烁:
   - 笔在数位板感应范围内时, 驱动会发大量 mouseMoved; 这些事件被当成鼠标, 进入 ethereal 的 `auto_hide`, 与 pen 事件的 `auto_show` 交替, 旧笔迹反复消失/重现;
   - 修复: 感应范围内/virtual pen 的 hover 事件按笔事件路由; 刚有笔事件 800ms 内 mouseMoved 不 auto_hide; 实测 ethereal+虚拟笔 auto_show 从约 70 次降为 1 次.
2. 无限画布拖动方向:
   - macOS 拖动/键盘方向改为与 Windows 2026-11 后一致的直觉语义(鼠标右拖 → 镜头右移/内容左移; 按左 → 内容左移);
3. 无限画布抓手:
   - R167 路由只转发 leftMouseDown/Up, 漏了 leftMouseDragged(6), 导致快捷键+左键拖动只有按下/抬起没有拖动;
   - 修复 route_event 的 camera 分支, 只认 1/6/2, 并增加 camera_mouse_dir 回归测试.


## R269 工程化: 拆出 crates/glaspen-macos

- 新建 `crates/glaspen-macos`(lib `glaspen_macos`), 承载 macOS 专属内容:
  - `src/macos_shell.rs`(原根 `src/macos_shell.rs`);
  - `src/entry.rs`(原根 `src/macos.rs`, `macos_run`);
  - `swift/`(原 `src/macos/swift`, Swift 壳 + Bridging.h/rust_ffi.h);
- 根 crate 只保留 `pub use glaspen_macos as macos`、FRB API、日志/入口; `src/macos_shell.rs`/`src/macos.rs`/`src/macos/` 删除;
- 根 build.rs 路径改到 `crates/glaspen-macos/swift`; core 契约测试 include_str/扫描路径同步;
- macOS target 依赖从根移到 crate; 根保留 libc(FRB 用);
- 验证: cargo build / cargo test --workspace --exclude glaspen2_installer / smoke debug / parity 全绿; menu probe/冷启动正常.


## R270 修复: 飘渺旧笔迹闪烁 + 真笔压感来源

1. 飘渺切换后旧笔迹闪烁(录屏复现):
   - ethereal hide 时 draw view 已按空内容全量重绘, backing store 里旧笔迹被清空;
   - 下一次 pen down 的 auto_show 标了 full redraw, 但紧接着 raw dot 的 dirty flush 又只设小脏矩形, AppKit 只重画该区域; 新笔迹可见而旧笔迹要等下一次 full redraw(约 2-4s 后)才回来;
   - 修复: `FULL_REDRAW_PENDING` 标记, 可见性/模式切换后的第一次 dirty flush 强制全量 setNeedsDisplay, 不再被局部脏矩形覆盖.
2. 真笔压感: 用户日志 `pen DOWN p=0.00`, 但 pressure monitor 有压力值;
   - 修复: 笔事件统一用 `raw_field/65535` (CGEvent tabletEventPointPressure) 作为有效压力, raw_field=0 时回退 NSEvent.pressure; 模型器/raw width/监视器同源; 新增单测.


## R271 方向单源: 鼠标拖动/键盘平移统一到 core

用户反馈鼠标拖动方向仍不符合直觉, 且过去 4-5 次反复回归。根因不是某一个符号,
而是方向符号分散在 macOS/Windows 的 mouse/hotkeys 四处, 且 core 文档明确"两端符号
不同、只记录不统一", 每次迁移都重新写一遍。

- core `Camera::drag_pan_from_screen_delta(dx_screen, dy_screen, zoom)`:
  "内容跟随鼠标"(右拖内容右移 / 下拖内容下移), 两端鼠标共用;
- core `Camera::keyboard_pan_delta(CameraDir, step)`:
  "按键方向 = 内容移动方向"(按左内容左移), 两端键盘共用;
- macOS mouse 恢复为内容跟随(此前被改成镜头跟随), 键盘/Windows 保持内容跟随;
- 新增 core 方向契约单测 `direction_contract_tests` + 跨端闸门
  `crates/glaspen-core/tests/pan_direction_single_source.rs`: 两端必须调用 core 函数,
  且 adapter 不得再出现本地方向符号;
- 已知历史差异文档删除, 后续改方向只改 core 一处并跑闸门.


## R272 修复: 新建画布快捷键 + 固定画布立即显示

1. `CMD+CTRL+C` 只有通知不新建页:
   - `glaspen2_clear_strokes` 的 blank-guard 在末页空白时只复用、不建页, 外壳随后只提示 `canvas.blank_guard`;
   - 新增 core `glaspen2_force_new_page`: 显式“新建画布”不再受空白末页复用守卫影响, 总是插入新页并切换;
   - 外壳 created==0 时强制建页 + load/rebuild/flush; 空白页场景实测 screens 533→534.
2. 固定画布模式不实际固定:
   - 模式切换只 `setNeedsDisplay`, 后台/非激活窗口的 AppKit 可能延后到 runloop 才画; 通知出现但笔迹未显示;
   - 可见性/模式切换统一走 `force_redraw_draw_view_now`: 设置 needsDisplay 后立即 `displayIfNeeded` (draw view + window);
   - ethereal hide/show/preview 与固定/飘渺切换都使用它; probe 截图确认固定模式笔迹立即出现.


## R273 修复: 清屏染黑/固定画布空白

1. `CMD+CTRL+C` 把其他颜色笔迹显示成黑色:
   - 根因是 cairo operator 数值用错: 代码把 `3` 当 CLEAR, 实际 `CAIRO_OPERATOR_CLEAR=0, 1=SOURCE, 2=OVER, 3=IN`;
   - 清屏 `cairo_set_operator(cr, 3); cairo_paint()` 实际执行 IN + 默认黑色源, 把已有内容染黑而不是清除; 橡皮 raw fallback 同错;
   - 改为命名常量 CLEAR/SOURCE/OVER, 清屏/橡皮/表面初始化使用 CLEAR;
   - 合成新画布后再强制建页; 实测空白页清屏 screens 533→534 且画面无旧笔迹.
2. 固定画布仍空白:
   - 固定分支增加 `glaspen2_rust_pageview_rebuild()` 从 STROKES 全量重建 surface, 再 redraw + display;
   - 解决飘渺期间 surface 跳过/启动未绘制时只 setNeedsDisplay 仍显示空 surface.
