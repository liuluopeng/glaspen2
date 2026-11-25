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
