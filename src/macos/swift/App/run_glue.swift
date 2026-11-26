// S7: run.m 剩余 C ABI 薄桥(Swift 化)。
import AppKit
import CoreGraphics
import Darwin

@_cdecl("glaspen2_run")
public func appRun() {
    if glaspen2_rust_app_bootstrap_start() == 0 { return }
    NSApp.run()
}

@_cdecl("glaspen2_router_camera_mods")
public func runCameraMods() -> UInt { MenuRuntime.maskFor(action: "canvas_zoom_in") }

@_cdecl("glaspen2_router_state")
public func runState() -> Int32 {
    glaspen2_router_enabled()
        | (((glaspen2_rust_pen_stroke_active() != 0) ? 1 : 0) << 1)
        | ((glaspen2_router_infinite()) << 2)
}

@_cdecl("glaspen2_router_draw_height")
public func runDrawHeight() -> Double { Double(UIAnchors.drawView?.bounds.size.height ?? 0) }

@_cdecl("glaspen2_router_zoom_at")
public func runZoomAt(_ factor: Double, _ x: Double, _ y: Double) {
    glaspen2_rust_camera_zoom_at(factor, x, y)
}

@_cdecl("glaspen2_router_cursor_flags")
public func runCursorFlags() -> Int32 {
    (glaspen2_router_pen_cursor_visible() != 0 ? 1 : 0)
        | (((glaspen2_rust_pen_stroke_active() != 0) ? 1 : 0) << 1)
        | ((glaspen2_router_ethereal()) << 2)
        | (((glaspen2_rust_pen_has_seen_proximity() != 0) ? 1 : 0) << 3)
}

@_cdecl("glaspen2_router_auto_hide")
public func runAutoHide() { _ = glaspen2_rust_ethereal_hide_now() }

@_cdecl("glaspen2_router_bubble_pen_gone")
public func runBubblePenGone() { glaspen2_bubble_pen_gone() }

@_cdecl("glaspen2_router_flush")
public func runFlush() { glaspen2_rust_overlay_flush_dirty() }

@_cdecl("glaspen2_virtual_pen_user_data")
public func runVirtualPenUserData() -> Int64 { 0x5650_4E31 }

@_cdecl("glaspen2_router_pen_state")
public func runPenState() -> Int32 {
    (glaspen2_router_strokes_visible())
        | (((glaspen2_rust_pen_stroke_active() != 0) ? 1 : 0) << 1)
        | ((glaspen2_router_pen_cursor_visible() != 0 ? 1 : 0) << 2)
}

@_cdecl("glaspen2_router_pen_bubble")
public func runPenBubble(_ back: Int32) {
    if back != 0 { glaspen2_bubble_pen_back() } else { glaspen2_bubble_pen_activity() }
}

@_cdecl("glaspen2_router_pen_auto_show")
public func runPenAutoShow() { glaspen2_rust_ethereal_auto_show() }

@_cdecl("glaspen2_router_peek_cancel")
public func runPeekCancel() { glaspen2_rust_ethereal_cancel_peek() }

@_cdecl("glaspen2_router_cursor_set")
public func runCursorSet(_ x: Double, _ y: Double) { sw_cursor_set(x, y, 0) }

@_cdecl("glaspen2_router_cursor_show")
public func runCursorShow(_ on: Int32) { if on != 0 { sw_cursor_show() } }

@_cdecl("glaspen2_router_hide_system_cursor")
public func runHideSystemCursor() { CGDisplayHideCursor(CGMainDisplayID()) }

@_cdecl("glaspen2_router_show_system_cursor")
public func runShowSystemCursor() { CGDisplayShowCursor(CGMainDisplayID()) }

@_cdecl("glaspen2_router_disable_cleanup")
public func runDisableCleanup() {
    glaspen2_router_camera_clear_dragging()
    glaspen2_router_camera_cancel()
    EventRouter.restoreSystemCursor()
}

@_cdecl("glaspen2_router_rec_slot_gif")
public func runRecSlotGif() -> Int32 { 0 }
@_cdecl("glaspen2_router_rec_slot_chat")
public func runRecSlotChat() -> Int32 { 1 }
@_cdecl("glaspen2_router_rec_mode")
public func runRecMode(_ slot: Int32) -> Int32 { glaspen2_recording_mode(slot) }
@_cdecl("glaspen2_router_rec_finish_and_count")
public func runRecFinishAndCount() -> Int32 {
    EventRouter.finishActiveStroke()
    return glaspen2_stroke_count()
}

@_cdecl("glaspen2_router_tap_menu_bar_interaction")
public func runTapMenuBar() {
    sw_event_tap_disable()
    DispatchQueue.main.asyncAfter(deadline: .now() + 5.0) {
        glaspen2_router_event_tap_enable()
    }
}

@_cdecl("glaspen2_router_update_status_icon_state")
public func runUpdateStatusIcon() {
    guard let item = UIAnchors.statusItem else { return }
    _ = glaspen2_rust_status_icon_apply(
        Unmanaged.passUnretained(item).toOpaque(),
        glaspen2_rust_pen_eraser_mode(), glaspen2_router_enabled(),
        glaspen2_router_ethereal(), glaspen2_router_infinite(),
        glaspen2_router_width_scale(), glaspen2_router_pen_r(),
        glaspen2_router_pen_g(), glaspen2_router_pen_b())
}

@_cdecl("glaspen2_router_bubble_auto_page_flip")
public func runBubbleAutoPageFlip() { _ = bubbleAutoPageFlip() }

@_cdecl("glaspen2_router_apply_pending_color_width")
public func runApplyPendingColorWidth() { MenuHandler.applyPendingColorWidth() }

@_cdecl("glaspen2_router_bubble_pen_back")
public func runBubblePenBack() { glaspen2_bubble_pen_back() }

@_cdecl("glaspen2_router_reenable_tap")
public func runReenableTap() { glaspen2_router_event_tap_enable() }

@_cdecl("glaspen2_router_raw_width")
public func runRawWidth(_ pressure: Double) -> Double {
    glaspen2_pressure_raw_width(pressure, glaspen2_router_width_scale()) * glaspen2_router_zoom()
}

private let runLangEnPtr: UnsafeMutablePointer<CChar> = strdup("en")!
private let runLangZhPtr: UnsafeMutablePointer<CChar> = strdup("zh")!

@_cdecl("glaspen2_router_lang_code")
public func runLangCode() -> UnsafePointer<CChar>? {
    // 旧实现返回静态字符串; 这里同样只分配一次, 不再每次页信息 HUD 泄漏一小块。
    UnsafePointer(glaspen2_router_lang() == 1 ? runLangEnPtr : runLangZhPtr)
}

@_cdecl("glaspen2_router_perf_summary")
public func runPerfSummary() { glaspen2_perf_summary() }
@_cdecl("glaspen2_router_perf_enabled")
public func runPerfEnabled() -> Int32 { glaspen2_perf_is_enabled() }
@_cdecl("glaspen2_router_perf_notes")
public func runPerfNotes(_ name: UnsafePointer<CChar>?, _ notes: UnsafePointer<CChar>?,
                         _ t0: UInt64) {
    glaspen2_perf_record(name, glaspen2_perf_elapsed_us(t0), notes)
}

@_cdecl("glaspen2_router_redraw_draw_view")
public func runRedraw() { UIAnchors.drawView?.needsDisplay = true }

@_cdecl("glaspen2_router_rec_chat_integration_changed")
public func runRecChatChanged(_ on: Int32) {
    let json = "{\"type\":\"ChatIntegrationChanged\",\"enabled\":\(on != 0 ? "true" : "false")}"
    _ = json.withCString { glaspen2_rust_recording_feed(1, $0) }
}

@_cdecl("glaspen2_router_rebuild_surface")
public func runRebuildSurface() { glaspen2_rust_pageview_rebuild() }

@_cdecl("glaspen2_router_canvas_apply_transform")
public func runApplyTransform() { glaspen2_rust_camera_apply_transform() }

@_cdecl("glaspen2_router_peek_lens_change")
public func runPeekLensChange() { glaspen2_rust_ethereal_peek_lens_change() }

@_cdecl("glaspen2_router_event_tap_reinstall")
public func runEventTapReinstall() {
    EventTapInstaller.install(includeScroll: glaspen2_router_infinite() != 0)
}

@_cdecl("glaspen2_router_dirty_reset")
public func runDirtyReset() { glaspen2_rust_dirty_reset() }
@_cdecl("glaspen2_router_dirty_has")
public func runDirtyHas() -> Int32 { glaspen2_rust_dirty_has() }
@_cdecl("glaspen2_router_dirty_rect")
public func runDirtyRect() -> CGRect {
    var out = [Double](repeating: 0, count: 4)
    let ok = out.withUnsafeMutableBufferPointer { glaspen2_rust_dirty_rect($0.baseAddress) }
    if ok == 0 { return .zero }
    return CGRect(x: out[0], y: out[1], width: out[2], height: out[3])
}

@_cdecl("glaspen2_router_camera_pan_cancel")
public func runCameraPanCancel() { glaspen2_router_camera_cancel() }
@_cdecl("glaspen2_router_camera_persist_infinite")
public func runCameraPersistInfinite() { glaspen2_rust_camera_persist_infinite() }
@_cdecl("glaspen2_router_camera_load_infinite")
public func runCameraLoadInfinite() { glaspen2_rust_camera_load_infinite() }
@_cdecl("glaspen2_router_camera_reset_lens")
public func runCameraResetLens() { glaspen2_rust_camera_reset_lens() }

@_cdecl("glaspen2_router_restore_system_cursor")
public func runRestoreSystemCursor() { EventRouter.restoreSystemCursor() }
@_cdecl("glaspen2_router_main_window")
public func runMainWindow() -> UnsafeMutableRawPointer? {
    guard let w = UIAnchors.mainWindow else { return nil }
    return Unmanaged.passUnretained(w).toOpaque()
}
@_cdecl("glaspen2_router_ensure_surface")
public func runEnsureSurface() { glaspen2_router_ensure_surface() }
@_cdecl("glaspen2_router_pm_show")
public func runPmShow() { PressureMonitorWindow.show() }
@_cdecl("glaspen2_router_pm_hide")
public func runPmHide() { PressureMonitorWindow.hide() }

@_cdecl("glaspen2_router_pressure_label")
public func runPressureLabel(_ text: UnsafePointer<CChar>?) {
    guard let text else { return }
    PressureMonitorWindow.updateLabel(String(cString: text))
}

@_cdecl("glaspen2_router_recording_feed")
public func runRecordingFeed(_ slot: Int32, _ json: UnsafePointer<CChar>?) -> Int32 {
    guard let json else { return 0 }
    return glaspen2_rust_recording_feed(slot, json)
}

@_cdecl("glaspen2_router_apply_infinite_async")
public func runApplyInfiniteAsync(_ on: Int32) {
    DispatchQueue.main.async { glaspen2_router_apply_infinite_canvas(on) }
}
@_cdecl("glaspen2_router_quit_async")
public func runQuitAsync() {
    DispatchQueue.main.async { MenuHandler.quitAppNow() }
}

@_cdecl("sw_glass_view")
public func runGlassView() -> UnsafeMutableRawPointer? {
    guard let g = UIAnchors.glassView else { return nil }
    return Unmanaged.passUnretained(g).toOpaque()
}

@_cdecl("glaspen2_swift_draw_view")
public func runDrawView() -> UnsafeMutableRawPointer? {
    guard let v = UIAnchors.drawView else { return nil }
    return Unmanaged.passUnretained(v).toOpaque()
}

@_cdecl("glaspen2_swift_pressure_destroy")
public func runPressureDestroy() { PressureMonitorWindow.destroy() }
@_cdecl("glaspen2_swift_show_cursor")
public func runShowCursor() { CGDisplayShowCursor(CGMainDisplayID()) }

@_cdecl("glaspen2_swift_tunnel_active")
public func runTunnelActive() -> Int32 { TunnelState.active ? 1 : 0 }

@_cdecl("glaspen2_swift_tunnel_cancel_timer")
public func runTunnelCancelTimer() {
    TunnelState.timer?.cancel()
    TunnelState.timer = nil
}
@_cdecl("glaspen2_swift_tunnel_mark_inactive")
public func runTunnelMarkInactive() { TunnelState.active = false }

@_cdecl("glaspen2_swift_tunnel_clear_state")
public func runTunnelClearState() {
    for i in 0..<TunnelState.maxCards {
        TunnelState.cardSurf[i] = nil
        TunnelState.mipSurf[i] = nil
        TunnelState.depths[i] = 0
    }
    TunnelState.blur = nil
    TunnelState.surface = nil
    TunnelState.cards = 0
    glaspen2_rust_clear_tunnel_cache()
}

@_cdecl("glaspen2_swift_tunnel_flush")
public func runTunnelFlush() { UIAnchors.drawView?.needsDisplay = true }

@_cdecl("glaspen2_router_refresh_menu_texts")
public func runRefreshMenuTexts() { MenuTextUpdater.update() }
@_cdecl("glaspen2_router_hotkey_recording")
public func runHotkeyRecording(_ on: Int32) { glaspen2_rust_hotkey_recording_set(on) }

@_cdecl("glaspen2_router_flush_dirty")
public func runFlushDirty() { glaspen2_rust_overlay_flush_dirty() }

@_cdecl("glaspen2_router_finish_active_stroke")
public func runFinishActiveStroke() { EventRouter.finishActiveStroke() }

@_cdecl("glaspen2_router_surface_cache_invalidate")
public func runSurfaceCacheInvalidate() { panelSurfaceImageCacheInvalidate() }

@_cdecl("glaspen2_router_invert_ink_source")
public func runInvertInkSource(_ cr: UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? {
    panelInvertInkSource(cr)
}

@_cdecl("glaspen2_router_dirty_include_surface_point")
public func runDirtySurfacePoint(_ x: Double, _ y: Double, _ pad: Double) {
    glaspen2_rust_dirty_include_surface_point(x, y, pad)
}

@_cdecl("glaspen2_router_dirty_include_surface_rect")
public func runDirtySurfaceRect(_ x: Double, _ y: Double, _ w: Double, _ h: Double) {
    glaspen2_rust_dirty_include_surface_rect(x, y, w, h)
}

@_cdecl("glaspen2_router_settings_apply_transparent")
public func runSettingsTransparent() {
    panelTransparentFull(1)
}

@_cdecl("glaspen2_router_perform_hotkey")
public func runPerformHotkey(_ keyCode: UInt16) -> Int32 {
    HotkeyDispatch.perform(keyCode: keyCode) ? 1 : 0
}

@_cdecl("glaspen2_router_apply_infinite_canvas")
public func runApplyInfinite(_ on: Int32) {
    let text = MenuRuntime.l10n(on != 0 ? "canvas.infinite_on" : "canvas.infinite_off")
    _ = text.withCString { glaspen2_rust_apply_infinite(on, $0) }
}

@_cdecl("glaspen2_router_clear_screen")
public func runClearScreen() { glaspen2_rust_surface_clear_screen() }

@_cdecl("glaspen2_router_replay_strokes")
public func runReplayStrokes() { glaspen2_rust_pageview_rebuild() }

@_cdecl("glaspen2_router_invert_apply")
public func runInvertApply(_ on: Int32) { panelInvertApply(on) }

@_cdecl("glaspen2_router_restore_canvas_flags")
public func runRestoreCanvasFlags(_ infinite: Int32, _ minimap: Int32, _ ethereal: Int32) {
    sw_set_infinite(infinite)
    sw_set_minimap(minimap)
    glaspen2_router_set_ethereal(ethereal)
}

@_cdecl("glaspen2_router_restore_pressure")
public func runRestorePressure(_ on: Int32) {
    glaspen2_router_set_pressure_monitor(on)
}

@_cdecl("glaspen2_router_rec_init")
public func runRecInit() {
    glaspen2_recording_init(0, 0)
    glaspen2_recording_init(1, glaspen2_router_chat_integration())
}

@_cdecl("glaspen2_router_startup_apply_pen")
public func runStartupApplyPen(_ r: Double, _ g: Double, _ b: Double, _ width: Double,
                               _ colorIdx: Int32, _ widthIdx: Int32) {
    sw_set_pen_settings(r, g, b, width, colorIdx, widthIdx)
}

@_cdecl("glaspen2_router_invert_restart")
public func runInvertRestart() { panelInvertRestart() }

@_cdecl("glaspen2_router_panel_transparent_full")
public func runPanelTransparentFull(_ on: Int32) { panelTransparentFull(on) }

@_cdecl("glaspen2_router_install_signals")
public func runInstallSignals() {
    signal(SIGINT) { _ in save_and_exit(0) }
    signal(SIGTERM) { _ in save_and_exit(0) }
}

@_cdecl("glaspen2_router_window_sharing_type")
public func runWindowSharingType() -> Int32 {
    Int32(UIAnchors.mainWindow?.sharingType.rawValue ?? 0)
}

@_cdecl("glaspen2_router_draw_view")
public func runDrawViewPtr() -> UnsafeMutableRawPointer? {
    UIAnchors.drawView.map { Unmanaged.passUnretained($0).toOpaque() }
}

@_cdecl("glaspen2_router_invert_invalidate")
public func runInvertInvalidate() { panelInvertInvalidate() }

@_cdecl("glaspen2_notification_redraw")
public func runNotificationRedraw() {
    UIAnchors.drawView?.needsDisplay = true
}

@_cdecl("glaspen2_swift_overlay_flush")
public func runOverlayFlush() { glaspen2_rust_overlay_flush() }

// R119: 录制效果 Rust 侧 UI 桥。
@_cdecl("glaspen2_swift_notify")
public func runRecordingNotify(_ text: UnsafePointer<CChar>?) {
    guard let text else { return }
    MenuRuntime.notify(String(cString: text))
}

@_cdecl("glaspen2_swift_copy_gif")
public func runRecordingCopyGif(_ data: UnsafePointer<UInt8>?, _ len: Int32) {
    guard let data, len > 0 else { return }
    let bytes = Data(bytes: data, count: Int(len))
    let name = String(format: "glaspen2_record_%.0f.gif",
                      Date.timeIntervalSinceReferenceDate * 1000)
    let path = (NSTemporaryDirectory() as NSString).appendingPathComponent(name)
    do {
        try bytes.write(to: URL(fileURLWithPath: path), options: .atomic)
    } catch {
        NSLog("[glaspen2] failed to write GIF temp file: \(path)")
        return
    }
    let pb = NSPasteboard.general
    pb.clearContents()
    pb.writeObjects([URL(fileURLWithPath: path) as NSURL])
}

public typealias RecordingMainCallback = @convention(c) (UnsafeMutableRawPointer?) -> Void

@_cdecl("glaspen2_swift_dispatch_main")
public func runRecordingDispatchMain(_ ctx: UnsafeMutableRawPointer?, _ cb: RecordingMainCallback?) {
    DispatchQueue.main.async { cb?(ctx) }
}

@_cdecl("glaspen2_swift_apply_display_frames")
public func runApplyDisplayFrames(_ width: Int32, _ height: Int32) {
    let frame = NSRect(x: 0, y: 0, width: CGFloat(width), height: CGFloat(height))
    if let window = UIAnchors.mainWindow {
        window.setFrame(frame, display: true)
        window.contentView?.frame = frame
    }
    (UIAnchors.glassView as? NSView)?.frame = frame
    if let drawView = UIAnchors.drawView {
        drawView.frame = frame
        drawView.needsDisplay = true
    }
}

@_cdecl("glaspen2_swift_notify_ax_warning")
public func runNotifyAxWarning() {
    notificationShowText(text: MenuRuntime.pair(
        "辅助功能权限已失效, 涂鸦/热键暂停 — 请到系统设置重新勾选",
        "Accessibility permission lost — re-enable it in System Settings"))
}


@_cdecl("glaspen2_swift_tunnel_set_surfaces")
public func runTunnelSetSurfaces(_ surface: UnsafeMutableRawPointer?, _ blur: UnsafeMutableRawPointer?) {
    TunnelState.surface = surface.map { OpaquePointer($0) }
    TunnelState.blur = blur.map { OpaquePointer($0) }
}

@_cdecl("glaspen2_swift_tunnel_surface")
public func runTunnelSurface() -> UnsafeMutableRawPointer? {
    guard let s = TunnelState.surface else { return nil }
    return UnsafeMutableRawPointer(s)
}

@_cdecl("glaspen2_swift_pageview_rebuild")
public func runPageviewRebuild() { glaspen2_rust_pageview_rebuild() }

@_cdecl("glaspen2_swift_update_menu_texts")
public func runUpdateMenuTexts() { MenuTextUpdater.update() }
@_cdecl("glaspen2_swift_restore_pressure_window")
public func runRestorePressureWindow() {
    DispatchQueue.main.async {
        PressureMonitorWindow.ensureWindow()
        PressureMonitorWindow.show()
        glaspen2_router_pm_refresh()
    }
}
@_cdecl("glaspen2_swift_delayed_rebuild")
public func runDelayedRebuild() {
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
        if glaspen2_router_stroke_active() != 0 { return }
        glaspen2_router_ensure_surface()
        glaspen2_rust_pageview_rebuild()
    }
}

@_cdecl("glaspen2_swift_set_main_anchors")
public func runSetMainAnchors(_ window: UnsafeMutableRawPointer?,
                              _ draw: UnsafeMutableRawPointer?,
                              _ glass: UnsafeMutableRawPointer?) {
    if let window { UIAnchors.mainWindow = Unmanaged<NSWindow>.fromOpaque(window).takeUnretainedValue() }
    if let draw { UIAnchors.drawView = Unmanaged<NSView>.fromOpaque(draw).takeUnretainedValue() }
    if let glass { UIAnchors.glassView = Unmanaged<NSVisualEffectView>.fromOpaque(glass).takeUnretainedValue() }
}

func i18nTextf(_ key: String, _ placeholder: String, _ value: String) -> String {
    guard let p = key.withCString({ k in placeholder.withCString { ph in value.withCString { v in
        glaspen2_rust_i18n_textf(k, ph, v)
    } } }) else { return key }
    defer { glaspen2_free_c_string(p) }
    return String(cString: p)
}


@_cdecl("glaspen2_swift_run_on_main_sync")
public func runOnMainSyncBridge(_ ctx: UnsafeMutableRawPointer?,
                                _ cb: (@convention(c) (UnsafeMutableRawPointer?) -> Void)?) {
    guard let cb else { return }
    if Thread.isMainThread {
        cb(ctx)
    } else {
        DispatchQueue.main.sync { cb(ctx) }
    }
}

@_cdecl("glaspen2_swift_draw_overlay_bridge")
public func glaspen2SwiftDrawOverlayBridge(_ view: UnsafeMutableRawPointer?,
                                           _ x: Double, _ y: Double,
                                           _ w: Double, _ h: Double) {
    guard let view else { return }
    let nsView = Unmanaged<NSView>.fromOpaque(view).takeUnretainedValue()
    _ = glaspen2_rust_overlay_draw(Unmanaged.passUnretained(nsView).toOpaque(), x, y, w, h)
}

@_cdecl("glaspen2_swift_bootstrap_prepare")
public func bootstrapPrepare() {
    NSApplication.shared
    NSApp.setActivationPolicy(.accessory)
    let opts = [kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true]
    if !AXIsProcessTrustedWithOptions(opts as CFDictionary) {
        NSLog("[glaspen2] Accessibility permission not granted")
    }
}

@_cdecl("glaspen2_swift_bootstrap_running_alert")
public func bootstrapRunningAlert() {
    NSApplication.shared
    NSApp.setActivationPolicy(.accessory)
    let alert = NSAlert()
    alert.messageText = MenuRuntime.pair("glaspen2 已在运行", "glaspen2 is already running")
    alert.informativeText = MenuRuntime.pair(
        "检测到另一个 glaspen2 实例(菜单栏图标)。两个实例的笔迹窗口会叠加显示, 全局热键会同时翻两套页 —— 看起来像“笔迹重复/只翻了一部分”。本次启动退出; 开发时如需并行调试, 设 GLASPEN2_ALLOW_MULTI=1。",
        "Another glaspen2 instance is running (menu bar icon). Two overlays stack and global hotkeys flip both — it looks like duplicated strokes or a partial page flip. This launch exits; set GLASPEN2_ALLOW_MULTI=1 to debug in parallel.")
    alert.addButton(withTitle: MenuRuntime.pair("好", "OK"))
    alert.runModal()
}

@_cdecl("glaspen2_swift_bootstrap_status_item")
public func bootstrapStatusItem() -> UnsafeMutableRawPointer? {
    let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
    item.button?.title = ""
    UIAnchors.statusItem = item
    return Unmanaged.passUnretained(item).toOpaque()
}

@_cdecl("glaspen2_swift_bootstrap_delegate_menu")
public func bootstrapDelegateMenu(_ item: UnsafeMutableRawPointer?) -> Int32 {
    guard let item else { return 0 }
    let statusItem = Unmanaged<NSStatusItem>.fromOpaque(item).takeUnretainedValue()
    UIAnchors.statusItem = statusItem
    NSApplication.shared
    NSApp.setActivationPolicy(.accessory)
    let handler = AppDelegate()
    AppDelegate.shared = handler
    NSApp.delegate = handler
    let menu = MainMenuBuilder.build(handler: handler, statusItem: statusItem,
                                     launchAtLoginOn: glaspen2_is_launch_at_login() != 0,
                                     glassOn: glaspen2_router_glass_enabled() != 0)
    UIAnchors.menu = menu
    return 1
}

@_cdecl("glaspen2_swift_bootstrap_display_observer")
public func bootstrapDisplayObserver() {
    NotificationCenter.default.addObserver(
        forName: NSApplication.didChangeScreenParametersNotification,
        object: nil, queue: .main) { _ in glaspen2_rust_display_changed() }
}

@_cdecl("glaspen2_swift_flip_probe_maybe_start")
public func bootstrapFlipProbe() { FlipProbe.maybeStart() }

@_cdecl("glaspen2_swift_bootstrap_screen_size")
public func bootstrapScreenSize(_ w: UnsafeMutablePointer<Int32>?,
                                _ h: UnsafeMutablePointer<Int32>?) {
    let frame = NSScreen.main?.frame ?? .zero
    w?.pointee = Int32(frame.size.width)
    h?.pointee = Int32(frame.size.height)
}

import FlutterMacOS

private var bootstrapFlutterEngine: FlutterEngine?
private var bootstrapFlutterVC: FlutterViewController?

@_cdecl("glaspen2_swift_make_flutter_vc")
public func bootstrapMakeFlutterVC() -> UnsafeMutableRawPointer? {
    if let vc = bootstrapFlutterVC {
        return Unmanaged.passUnretained(vc).toOpaque()
    }
    let engine = FlutterEngine(name: "glaspen_settings", project: nil)
    engine.run(withEntrypoint: nil)
    bootstrapFlutterEngine = engine
    let vc = FlutterViewController(engine: engine, nibName: nil, bundle: nil)
    bootstrapFlutterVC = vc
    return Unmanaged.passUnretained(vc).toOpaque()
}

@_cdecl("glaspen2_swift_activate_settings_existing")
public func bootstrapActivateSettingsExisting() {
    NSApp.setActivationPolicy(.regular)
    NSApp.activate(ignoringOtherApps: true)
    UIAnchors.settingsWindow?.makeKeyAndOrderFront(nil)
}

@_cdecl("glaspen2_swift_show_settings_window")
public func bootstrapShowSettingsWindow(_ p: UnsafeMutableRawPointer?) {
    guard let p else { return }
    let window = Unmanaged<NSWindow>.fromOpaque(p).takeUnretainedValue()
    NSApp.setActivationPolicy(.regular)
    NSApp.activate(ignoringOtherApps: true)
    window.center()
    window.makeKeyAndOrderFront(nil)
}


func notificationShowText(text: String) {
    text.withCString { glaspen2_rust_notification_show($0) }
}
func notificationShowPageInfo(screenId: Int64) {
    glaspen2_rust_notification_show_page_info(screenId)
}

@_cdecl("glaspen2_macos_swift_notification_selftest")
public func notificationSelftest() -> Int32 {
    _ = NSApplication.shared
    let w = 400.0, h = 200.0
    let image = NSImage(size: NSSize(width: w, height: h))
    image.lockFocus()
    let ok = "TEST NOTIFICATION".withCString {
        glaspen2_rust_draw_notification($0, w, h) != 0
    }
    image.unlockFocus()
    guard ok, let tiff = image.tiffRepresentation,
          let rep = NSBitmapImageRep(data: tiff),
          let data = rep.bitmapData else {
        print("SWIFT_NOTIFICATION_FAIL rust_draw_or_bitmap")
        return 1
    }
    var nonEmpty = 0
    let count = rep.bytesPerRow * rep.pixelsHigh
    for i in stride(from: 0, to: count, by: 4) where data[i] != 0 || data[i + 1] != 0 || data[i + 2] != 0 { nonEmpty += 1 }
    print("SWIFT_NOTIFICATION_RUST_OK pixels=\(nonEmpty)")
    return nonEmpty > 0 ? 0 : 1
}

@_cdecl("glaspen2_swift_dispatch_main_after")
public func runDispatchMainAfter(_ delay: Double, _ ctx: UnsafeMutableRawPointer?,
                                 _ cb: (@convention(c) (UnsafeMutableRawPointer?) -> Void)?) {
    guard let cb else { return }
    DispatchQueue.main.asyncAfter(deadline: .now() + delay) { cb(ctx) }
}


func surfaceCurrent() -> OpaquePointer? {
    glaspen2_rust_surface_current().map { OpaquePointer($0) }
}
func surfaceScale() -> Double { glaspen2_rust_surface_scale() }
func surfaceSet(_ ptr: OpaquePointer?) {
    glaspen2_rust_surface_set(ptr.map(UnsafeMutableRawPointer.init))
}
func surfaceEnsure(view: NSView) {
    _ = glaspen2_rust_surface_ensure(Unmanaged.passUnretained(view).toOpaque())
}


func bubbleAutoPageFlip() -> Bool {
    let page = glaspen2_bubble_auto_page_if_due()
    if page <= 0 { return false }
    Flipper.swap(goingNext: true, prepare: {
        glaspen2_load_strokes_for_screen(page)
        glaspen2_smooth_loaded_strokes()
        glaspen2_rust_pageview_update()
    }, commit: {
        glaspen2_router_replay_strokes()
    })
    notificationShowPageInfo(screenId: Int64(page))
    if let text = Optional(MenuRuntime.l10n("page.auto_created")) {
        notificationShowText(text: text)
    }
    return true
}


@_cdecl("glaspen2_macos_swift_pageframe_parity_selftest")
public func pageframeParitySelftest() -> Int32 {
    let w = 300, h = 200
    func make() -> OpaquePointer? {
        guard let s = cairo_image_surface_create(cairo_format_t(0), Int32(w), Int32(h)),
              cairo_surface_status(s).rawValue == 0 else { return nil }
        cairo_surface_flush(s)
        return s
    }
    guard let a = make(), let b = make() else { return 1 }
    _ = glaspen2_rust_draw_page_frame(UnsafeMutableRawPointer(a), 1.0, 20, 10, 0.8, 200, 150, 1)
    if let cr = cairo_create(b) {
        let x = 20.0, y = 10.0, pw = 200.0 * 0.8, ph = 150.0 * 0.8
        cairo_set_source_rgba(cr, 0, 0, 0, 0.42)
        cairo_set_fill_rule(cr, cairo_fill_rule_t(1))
        cairo_rectangle(cr, 0, 0, Double(w), Double(h))
        cairo_rectangle(cr, x, y, pw, ph)
        cairo_fill(cr)
        cairo_set_source_rgba(cr, 1, 1, 1, 0.55)
        cairo_set_line_width(cr, 1.0)
        cairo_rectangle(cr, x + 0.5, y + 0.5, pw - 1, ph - 1)
        cairo_stroke(cr)
        cairo_destroy(cr)
    }
    cairo_surface_flush(a); cairo_surface_flush(b)
    guard let da = cairo_image_surface_get_data(a), let db = cairo_image_surface_get_data(b) else { return 1 }
    let stride = Int(cairo_image_surface_get_stride(a))
    var diff = 0
    for yy in 0..<h { for xx in 0..<w {
        let i = yy * stride + xx * 4
        if da[i] != db[i] || da[i+1] != db[i+1] || da[i+2] != db[i+2] || da[i+3] != db[i+3] { diff += 1 }
    } }
    cairo_surface_destroy(a); cairo_surface_destroy(b)
    print("SWIFT_PAGEFRAME_PARITY diff=\(diff)/\(w*h)")
    return diff == 0 ? 0 : 1
}
