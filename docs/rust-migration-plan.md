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
