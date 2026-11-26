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
public func runApplyPendingColorWidth() { glaspen2_rust_apply_pending_color_width() }

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
    DispatchQueue.main.async { glaspen2_rust_menu_quit_app() }
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
    glaspen2_router_hotkey_perform(keyCode)
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


@objc public final class FlipProbe: NSObject {
    private static var timer: DispatchSourceTimer?
    private static var back = false
    private static var first = true

    @objc public static func maybeStart() {
        guard let v = getenv("GLASPEN2_FLIP_EVERY"), let raw = String(validatingUTF8: v),
              let every = Int(raw), every > 0 else { return }
        NSLog("[glaspen2] flip probe: 每 %d 秒一跳(第1跳建页, 之后往返翻)", every)
        let src = DispatchSource.makeTimerSource(queue: .main)
        src.schedule(deadline: .now() + .seconds(every),
                     repeating: .seconds(every),
                     leeway: .milliseconds(200))
        src.setEventHandler {
            if first {
                first = false
                var want = 0
                if let vpn = getenv("GLASPEN2_PROBE_PAGES"),
                   let s = String(validatingUTF8: vpn), let n = Int(s) {
                    want = n
                }
                _ = glaspen2_debug_insert_pages(Int32(want),
                                                Int32(glaspen2_router_screen_w()),
                                                Int32(glaspen2_router_screen_h()))
                NSLog("[glaspen2] probe 建页 %d 页(空白, 直插)", want)
                return
            }
            NSLog("[glaspen2] probe 翻页 back=%d", back ? 1 : 0)
            _ = glaspen2_router_perform_hotkey(back ? 0x32 : 0x12)
            back.toggle()
        }
        timer = src
        src.resume()
    }
}


@objc public final class UIAnchors: NSObject {
    static var mainWindow: NSWindow?
    static var drawView: NSView?
    static var glassView: NSVisualEffectView?
    static var statusItem: NSStatusItem?
    static var menu: NSMenu?
    static var settingsWindow: NSWindow?
}

private func opaque<T: AnyObject>(_ v: T?) -> UnsafeMutableRawPointer? {
    guard let v else { return nil }
    return Unmanaged.passUnretained(v).toOpaque()
}

@_cdecl("sw_main_window")
public func uiMainWindow() -> UnsafeMutableRawPointer? { opaque(UIAnchors.mainWindow) }
@_cdecl("sw_set_main_window")
public func uiSetMainWindow(_ p: UnsafeMutableRawPointer?) {
    UIAnchors.mainWindow = p.map { Unmanaged<NSWindow>.fromOpaque($0).takeUnretainedValue() }
}
@_cdecl("sw_draw_view")
public func uiDrawView() -> UnsafeMutableRawPointer? { opaque(UIAnchors.drawView) }
@_cdecl("sw_set_draw_view")
public func uiSetDrawView(_ p: UnsafeMutableRawPointer?) {
    UIAnchors.drawView = p.map { Unmanaged<NSView>.fromOpaque($0).takeUnretainedValue() }
}
@_cdecl("sw_glass_view")
public func uiGlassView() -> UnsafeMutableRawPointer? { opaque(UIAnchors.glassView) }
@_cdecl("sw_set_glass_view")
public func uiSetGlassView(_ p: UnsafeMutableRawPointer?) {
    UIAnchors.glassView = p.map { Unmanaged<NSVisualEffectView>.fromOpaque($0).takeUnretainedValue() }
}
@_cdecl("sw_status_item")
public func uiStatusItem() -> UnsafeMutableRawPointer? { opaque(UIAnchors.statusItem) }
@_cdecl("sw_set_status_item")
public func uiSetStatusItem(_ p: UnsafeMutableRawPointer?) {
    UIAnchors.statusItem = p.map { Unmanaged<NSStatusItem>.fromOpaque($0).takeUnretainedValue() }
}
@_cdecl("sw_menu")
public func uiMenu() -> UnsafeMutableRawPointer? { opaque(UIAnchors.menu) }
@_cdecl("sw_set_menu")
public func uiSetMenu(_ p: UnsafeMutableRawPointer?) {
    UIAnchors.menu = p.map { Unmanaged<NSMenu>.fromOpaque($0).takeUnretainedValue() }
}
@_cdecl("sw_settings_window")
public func uiSettingsWindow() -> UnsafeMutableRawPointer? { opaque(UIAnchors.settingsWindow) }
@_cdecl("sw_set_settings_window")
public func uiSetSettingsWindow(_ p: UnsafeMutableRawPointer?) {
    UIAnchors.settingsWindow = p.map { Unmanaged<NSWindow>.fromOpaque($0).takeUnretainedValue() }
}


// S6: 事件路由。滚轮缩放(3-1)、非笔光标隐藏(3-2)、笔光标更新(3-3b)。
import AppKit

@objc public final class EventRouter: NSObject {
    /// 0 = fallback(ObjC), 1 = pass(return event), 2 = consume(return NULL)
    @objc(routeEventOfType:event:)
    public static func route(type: CGEventType, event: CGEvent) -> Int32 {
        if type == .tabletProximity {
            routeProximity(event)
            return 1
        }
        var e = GlaspenEventC()
        let kind = type.rawValue
        e.kind = Int32(kind)
        if kind == 10 || kind == 11 {
            if let ev = NSEvent(cgEvent: event) {
                e.key_code = Int32(ev.keyCode)
                let relevant: NSEvent.ModifierFlags = [.command, .control, .option, .shift]
                let raw = ev.modifierFlags.intersection(relevant).rawValue
                e.flags = Int32(bitPattern: UInt32(truncatingIfNeeded: raw))
                e.is_repeat = event.getIntegerValueField(.keyboardEventAutorepeat) != 0 ? 1 : 0
            }
            return glaspen2_rust_route_event(&e)
        }
        let pointerKind = (1...9).contains(kind) || kind == 23
        if let ev = NSEvent(cgEvent: event) {
            let relevant: NSEvent.ModifierFlags = [.command, .control, .option, .shift]
            let raw = ev.modifierFlags.intersection(relevant).rawValue
            e.flags = Int32(bitPattern: UInt32(truncatingIfNeeded: raw))
            let loc = ev.locationInWindow
            e.x = Double(loc.x)
            e.y = Double(loc.y)
            e.pen_x = e.x
            e.pen_y = glaspen2_router_draw_height() - e.y
            e.screen_y = event.location.y
            if pointerKind {
                e.is_pen = isPen(ev) ? 1 : 0
                e.raw_field = event.getIntegerValueField(.tabletEventPointPressure)
                e.pressure = Double(ev.pressure)
                e.timestamp = ev.timestamp
                e.is_eraser = (ev.pointingDeviceType == .eraser
                    || glaspen2_router_prox_is_eraser() != 0) ? 1 : 0
            } else if kind == 22 {
                e.delta_y = ev.scrollingDeltaY
                e.precise = ev.hasPreciseScrollingDeltas ? 1 : 0
            }
        }
        return glaspen2_rust_route_event(&e)
    }

    private static func isPen(_ ev: NSEvent) -> Bool {
        let dev = ev.pointingDeviceType
        if dev == .pen || dev == .eraser { return true }
        return ev.subtype == .tabletPoint || ev.subtype == .tabletProximity
    }

    /// 笔按下/拖动期间隐藏系统光标; 任意 up 恢复光标并清十字/刷新(S6-3-3c)。


    /// 恢复系统光标(S6-3-3d): 只有确实隐藏过(pen_drawing)才 show + 清标记。
    @objc(restoreSystemCursor)
    public static func restoreSystemCursor() {
        glaspen2_rust_restore_system_cursor()
    }

    /// 收尾在写笔画(退出/切页/丢 pen-up 时共用), 与 penUp 的区别是不动光标/不刷新。
    @objc(finishActiveStroke)
    public static func finishActiveStroke() {
        glaspen2_rust_finish_active_stroke()
    }

    /// S6-3-4: 键盘事件: 录制热键 → 普通 chord; 返回 1 放行 / 2 消费。


    /// S6-3-3d: 压感监视器状态更新(是否在压感区/压力/坐标/tip down/事件名)。
    /// S6-3-3d: 笔事件分派(落笔/拖动/抬笔); 返回 2 = 消费事件。
    /// S6-3-3d: 相机临时抓手(⌃⌥+左键拖动)开始/拖动/结束; 返回 true = 消费事件。


    /// S6-3-3d: 笔 proximity 进入/离开: 悬停显示/离开隐藏, 飘渺模式联动。
    private static func routeProximity(_ event: CGEvent) {
        guard let prox = NSEvent(cgEvent: event) else { return }
        let dev = prox.pointingDeviceType
        let enter = event.getIntegerValueField(.tabletProximityEventEnterProximity) == 1
        _ = glaspen2_rust_route_proximity(Int32(dev.rawValue), enter ? 1 : 0)
    }

    /// S6-3-3d: 落笔入口: 自动翻页 + 模型器 begin + 共享 context + raw dot + 落笔状态。

    /// S6-3-3d: 抬笔: 结束模型器(橡皮 finish / 正常 end+commit) + context/状态复位。

    /// S6-3-3d: 拖动实时段: 模型器 move + raw segment(响应式反馈)。

    /// S6-3-3d: 橡皮态切换 + 状态图标刷新(落笔/拖动自动补 begin/抬笔共用)。

    /// S6-3-4b-2b: 普通 chord 门控(录制热键由 ObjC 函数先行处理)。
    /// 返回 1 = 放行, 2 = 消费(返回 NULL)。
    /// ObjC 侧复用同一笔事件判定(避免两处公式各自维护)。
    /// 虚拟笔(GLASPEN2_VIRTUAL_PEN)合成事件判定。
    @objc(isVirtualPenEvent:)
    public static func isVirtualPenEvent(_ event: CGEvent) -> Bool {
        event.getIntegerValueField(.eventSourceUserData) == glaspen2_virtual_pen_user_data()
    }




}


@objc public final class EventTapInstaller: NSObject {
    @objc(eventMaskWithScroll:)
    public static func eventMask(includeScroll: Bool) -> UInt64 {
        let types: [CGEventType] = [
            .mouseMoved, .leftMouseDown, .leftMouseDragged, .leftMouseUp,
            .rightMouseDown, .rightMouseDragged, .rightMouseUp,
            .otherMouseDown, .otherMouseDragged, .otherMouseUp,
            .tabletProximity, .tabletPointer, .keyDown, .keyUp,
        ]
        var mask: UInt64 = 0
        for t in types { mask |= CGEventMask(1) << UInt64(t.rawValue) }
        if includeScroll { mask |= CGEventMask(1) << UInt64(CGEventType.scrollWheel.rawValue) }
        return mask
    }

    @objc(installWithScroll:)
    public static func install(includeScroll: Bool) {
        let mask = eventMask(includeScroll: includeScroll)
        let callback: CGEventTapCallBack = { _, type, event, _ in
            let route = EventRouter.route(type: type, event: event)
            if route == 2 { return nil }
            if EventRouter.isVirtualPenEvent(event) { return nil }
            return Unmanaged.passUnretained(event)
        }
        guard let tap = CGEvent.tapCreate(tap: .cgSessionEventTap,
                                          place: .headInsertEventTap,
                                          options: .defaultTap,
                                          eventsOfInterest: CGEventMask(mask),
                                          callback: callback,
                                          userInfo: nil) else {
            glaspen2_event_tap_store(nil)
            return
        }
        CGEvent.tapEnable(tap: tap, enable: true)
        if let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0) {
            CFRunLoopAddSource(CFRunLoopGetMain(), source, CFRunLoopMode.commonModes)
        }
        glaspen2_event_tap_store(tap)
        glaspen2_event_tap_note_scroll(includeScroll ? 1 : 0)
    }
}


// S7: 压感监视器小窗(无边框、置顶、不接收鼠标)。
import AppKit

@objc public final class PressureMonitorWindow: NSObject {
    private static var window: NSWindow?
    private static var label: NSTextField?

    @objc public static func ensureWindow() {
        if window != nil { return }
        let screenFrame = NSScreen.main?.frame ?? NSRect(x: 0, y: 0, width: 1920, height: 1080)
        let frame = NSRect(x: 10, y: screenFrame.height - 40, width: 220, height: 30)
        let w = NSWindow(contentRect: frame, styleMask: .borderless,
                         backing: .buffered, defer: false)
        w.level = NSWindow.Level(rawValue: Int(CGWindowLevelForKey(.maximumWindow)))
        w.isOpaque = true
        w.backgroundColor = NSColor(white: 0.12, alpha: 1.0)
        w.ignoresMouseEvents = true
        w.title = "Pressure"
        w.isReleasedWhenClosed = false
        let l = NSTextField(frame: NSRect(x: 4, y: 2, width: 212, height: 26))
        l.stringValue = "P=-----  ---  (---,---)"
        l.textColor = .white
        l.font = NSFont(name: "Menlo", size: 12)
        l.isBezeled = false
        l.drawsBackground = false
        l.isEditable = false
        l.isSelectable = false
        w.contentView?.addSubview(l)
        window = w
        label = l
    }

    @objc public static func show() {
        ensureWindow()
        window?.orderFront(nil)
    }

    @objc public static func hide() {
        window?.orderOut(nil)
    }

    @objc public static func destroy() {
        window?.close()
        window = nil
        label = nil
    }

    @objc public static func updateLabel(_ text: String) {
        label?.stringValue = text
    }

    static var pressure: Int32 { glaspen2_rust_pm_pressure() }
    static var evType: String? {
        guard let p = glaspen2_rust_pm_ev_type() else { return nil }
        return String(cString: p)
    }
    static var tipDown: Bool { glaspen2_rust_pm_tip_down() != 0 }
    static var inRange: Bool { glaspen2_rust_pm_in_range() != 0 }
    static var x: Double { glaspen2_rust_pm_x() }
    static var y: Double { glaspen2_rust_pm_y() }

    static func refreshLabel() {
        if let t = evType {
            t.withCString { glaspen2_rust_update_pressure_label(pressure, $0, x, y) }
        } else {
            glaspen2_rust_update_pressure_label(pressure, nil, x, y)
        }
    }
}


@_cdecl("sw_pm_update")
public func pressureUpdate(_ inRange: Int32, _ pressure: Int32, _ x: Double, _ y: Double,
                           _ tipDown: Int32, _ evType: UnsafePointer<CChar>?) {
    glaspen2_rust_pm_update(inRange, pressure, x, y, tipDown, evType)
    PressureMonitorWindow.refreshLabel()
}

@_cdecl("sw_pm_refresh")
public func pressureRefresh() { PressureMonitorWindow.refreshLabel() }

@_cdecl("glaspen2_swift_copy_newest_gif")
public func runCopyNewestGif() {
    guard let desktop = FileManager.default.urls(for: .desktopDirectory, in: .userDomainMask).first else { return }
    let files = (try? FileManager.default.contentsOfDirectory(
        at: desktop, includingPropertiesForKeys: [.contentModificationDateKey])) ?? []
    var newest: URL?
    var newestDate: Date?
    for f in files where f.lastPathComponent.hasPrefix("glaspen2_") && f.pathExtension == "gif" {
        let d = (try? f.resourceValues(forKeys: [.contentModificationDateKey]))?.contentModificationDate
        if newestDate == nil || (d != nil && d! > newestDate!) { newestDate = d; newest = f }
    }
    guard let newest else { return }
    let pb = NSPasteboard.general
    pb.clearContents()
    pb.writeObjects([newest as NSURL])
}
// S7: 带背景保存(桌面截图 + 笔迹合成 Display P3; 14+ 用 SCScreenshotManager, 12/13 走 CGWindowList)。
import AppKit
import CoreGraphics
import ScreenCaptureKit
import Darwin

@objc public final class SaveBackground: NSObject {
    @objc public static func save() {
        guard let surface = surfaceCurrent() else { return }
        cairo_surface_flush(surface)
        let dw = Int(cairo_image_surface_get_width(surface))
        let dh = Int(cairo_image_surface_get_height(surface))
        let dstride = Int(cairo_image_surface_get_stride(surface))
        guard let dptr = cairo_image_surface_get_data(surface),
              dw > 0, dh > 0, dstride > 0 else {
            glaspen2_rust_surface_save_drawing_only()
            return
        }
        let size = dstride * dh
        guard let drawingCopy = malloc(size) else {
            glaspen2_rust_surface_save_drawing_only()
            return
        }
        memcpy(drawingCopy, dptr, size)

        let wasNone = glaspen2_router_window_sharing_type() == 0
        if wasNone {
            glaspen2_router_set_window_sharing_readonly()
        }
        func restoreSharing() {
            if wasNone { glaspen2_router_set_window_sharing_none() }
        }
        func failAndSaveDrawing(_ message: String) {
            restoreSharing()
            free(drawingCopy)
            DispatchQueue.main.async {
                if !message.isEmpty {
                    notificationShowText(text:MenuRuntime.pair(message, message))
                }
                glaspen2_rust_surface_save_drawing_only()
            }
        }

        if #available(macOS 14.0, *) {
            captureSCK(drawingCopy: drawingCopy, width: dw, height: dh,
                       stride: dstride) { image in
                restoreSharing()
                if let image {
                    compositeAndSave(image, drawingCopy, dw, dh, dstride)
                } else {
                    failAndSaveDrawing("截图失败，已保存涂鸦")
                }
            }
        } else {
            let image = captureBelowWindow()
            restoreSharing()
            if let image {
                compositeAndSave(image, drawingCopy, dw, dh, dstride)
            } else {
                failAndSaveDrawing("截图失败(需要屏幕录制权限), 已保存涂鸦")
            }
        }
    }

    @available(macOS 14.0, *)
    private static func captureSCK(drawingCopy: UnsafeMutableRawPointer,
                                   width: Int, height: Int, stride: Int,
                                   completion: @escaping (CGImage?) -> Void) {
        SCShareableContent.getExcludingDesktopWindows(false,
                                                      onScreenWindowsOnly: true) { content, error in
            if error != nil || content?.displays.isEmpty != false {
                NSLog("[glaspen2] Screen capture failed: %@", "\(String(describing: error))")
                completion(nil)
                return
            }
            guard let content else { completion(nil); return }
            let want = glaspen2_rust_window_display_id()
            let display = content.displays.first { $0.displayID == want }
                ?? content.displays.first!
            let filter = SCContentFilter(display: display, excludingWindows: [])
            let config = SCStreamConfiguration()
            config.width = display.width
            config.height = display.height
            SCScreenshotManager.captureImage(contentFilter: filter,
                                             configuration: config) { image, error in
                if error != nil || image == nil {
                    NSLog("[glaspen2] Screenshot failed: %@", "\(String(describing: error))")
                    completion(nil)
                    return
                }
                completion(image)
            }
        }
    }

    private static func captureBelowWindow() -> CGImage? {
        typealias CaptureFn = @convention(c) (CGRect, CGWindowListOption,
                                              CGWindowID, CGWindowImageOption) -> Unmanaged<CGImage>?
        guard let handle = dlopen(nil, RTLD_LAZY),
              let sym = dlsym(handle, "CGWindowListCreateImage") else { return nil }
        let fn = unsafeBitCast(sym, to: CaptureFn.self)
        guard let window = UIAnchors.mainWindow else { return nil }
        return fn(.null, .optionOnScreenBelowWindow,
                  CGWindowID(window.windowNumber), CGWindowImageOption(rawValue: 0))?.takeRetainedValue()
    }

    private static func compositeAndSave(_ image: CGImage, _ drawingCopy: UnsafeMutableRawPointer,
                                         _ dw: Int, _ dh: Int, _ dstride: Int) {
        let bw = image.width, bh = image.height
        guard let p3 = CGColorSpace(name: CGColorSpace.displayP3),
              let srgb = CGColorSpace(name: CGColorSpace.sRGB) else {
            free(drawingCopy); return
        }
        // 截图 → Display P3
        guard let bgCtx = CGContext(data: nil, width: bw, height: bh,
                                    bitsPerComponent: 8, bytesPerRow: bw * 4,
                                    space: p3,
                                    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else {
            free(drawingCopy); return
        }
        bgCtx.draw(image, in: CGRect(x: 0, y: 0, width: bw, height: bh))
        guard let p3Image = bgCtx.makeImage(),
              let bgData = p3Image.dataProvider?.data,
              let bgPtr = CFDataGetBytePtr(bgData) else {
            free(drawingCopy); return
        }
        let bgStride = p3Image.bytesPerRow

        // 笔迹(cairo BGRA 内存) → CGImage → Display P3
        let drawData = Data(bytesNoCopy: drawingCopy, count: dstride * dh,
                            deallocator: .none)
        guard let drawProvider = CGDataProvider(data: drawData as CFData),
              let drawImage = CGImage(width: dw, height: dh, bitsPerComponent: 8,
                                      bitsPerPixel: 32, bytesPerRow: dstride,
                                      space: srgb,
                                      bitmapInfo: CGBitmapInfo(
                                        rawValue: CGImageAlphaInfo.premultipliedFirst.rawValue
                                            | CGBitmapInfo.byteOrder32Little.rawValue),
                                      provider: drawProvider, decode: nil,
                                      shouldInterpolate: false,
                                      intent: .defaultIntent) else {
            free(drawingCopy); return
        }
        guard let drawCtx = CGContext(data: nil, width: dw, height: dh,
                                      bitsPerComponent: 8, bytesPerRow: dw * 4,
                                      space: p3,
                                      bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else {
            free(drawingCopy); return
        }
        drawCtx.draw(drawImage, in: CGRect(x: 0, y: 0, width: dw, height: dh))
        guard let p3Draw = drawCtx.makeImage(),
              let drawP3Data = p3Draw.dataProvider?.data,
              let drawPtr = CFDataGetBytePtr(drawP3Data) else {
            free(drawingCopy); return
        }
        let drawStride = p3Draw.bytesPerRow

        glaspen2_save_with_background(drawPtr, Int32(dw), Int32(dh), Int32(drawStride),
                                      bgPtr, Int32(bw), Int32(bh), Int32(bgStride))
        free(drawingCopy)
        if let message = Optional(MenuRuntime.l10n("export.screenshot_with_bg_saved")) {
            DispatchQueue.main.async {
                notificationShowText(text:message)
            }
        }
    }
}


// S7: 反色突出捕获(ScreenCaptureKit 壳; 状态/像素处理/缓存均在 Rust)。
import AppKit
import CoreMedia
import CoreVideo
import ScreenCaptureKit

@available(macOS 12.3, *)
@objc public final class InvertCapture: NSObject, SCStreamOutput {
    private static var filter: SCContentFilter?
    private static var stream: SCStream?
    private static var output: InvertCapture?
    private static var queue: DispatchQueue?
    private static var captureW = 0
    private static var captureH = 0

    @objc public static func apply(on: Bool) {
        if on {
            glaspen2_set_stroke_invert(1)
            schedule(0.05)
        } else {
            teardown()
            glaspen2_set_stroke_invert(0)
            glaspen2_router_set_window_sharing_readonly()
            glaspen2_rust_pageview_rebuild()
            glaspen2_rust_overlay_flush()
        }
    }

    @objc public static func restart() {
        stop()
        schedule(0.05)
    }

    @objc public static func invalidate() {
        stop()
        filter = nil
        glaspen2_router_set_window_sharing_readonly()
        teardown()
    }

    @objc(inkSourceWithCR:)
    public static func inkSource(crRaw: UnsafeMutableRawPointer) -> UnsafeMutableRawPointer? {
        guard glaspen2_router_invert_enabled() != 0 else { return nil }
        return glaspen2_rust_invert_ink_source(crRaw).map { $0 }
    }

    public func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer,
                       of type: SCStreamOutputType) {
        if type != .screen { return }
        if glaspen2_router_invert_enabled() == 0 { return }
        if !CMSampleBufferIsValid(sampleBuffer) { return }
        guard let pb = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }
        CVPixelBufferLockBaseAddress(pb, .readOnly)
        defer { CVPixelBufferUnlockBaseAddress(pb, .readOnly) }
        guard let base = CVPixelBufferGetBaseAddress(pb) else { return }
        let ok = glaspen2_rust_invert_process(
            base.assumingMemoryBound(to: UInt8.self),
            Int32(CVPixelBufferGetPixelFormatType(pb)),
            Int32(CVPixelBufferGetWidth(pb)),
            Int32(CVPixelBufferGetHeight(pb)),
            Int32(CVPixelBufferGetBytesPerRow(pb)))
        if ok != 0 {
            DispatchQueue.main.async { glaspen2_rust_invert_consume() }
        }
    }

    private static func stop() {
        guard let st = stream else { return }
        stream = nil
        st.stopCapture { error in
            if let error { NSLog("[invert] stream stop: %@", "\(error)") }
        }
        if let output {
            try? st.removeStreamOutput(output, type: .screen)
        }
    }

    private static func teardown() {
        stop()
        glaspen2_rust_invert_teardown()
        captureW = 0
        captureH = 0
    }

    private static func start() {
        if stream != nil || filter == nil || captureW <= 0 { return }
        if output == nil { output = InvertCapture() }
        if queue == nil {
            queue = DispatchQueue(label: "glaspen.invert")
        }
        let config = SCStreamConfiguration()
        config.width = captureW
        config.height = captureH
        config.showsCursor = false
        config.pixelFormat = kCVPixelFormatType_32BGRA
        config.queueDepth = 3
        config.minimumFrameInterval = CMTime(value: 1, timescale: Int32(glaspen2_router_invert_fps()))
        guard let filter, let output, let queue else { return }
        let st = SCStream(filter: filter, configuration: config, delegate: nil)
        do {
            try st.addStreamOutput(output, type: .screen, sampleHandlerQueue: queue)
        } catch {
            NSLog("[invert] addStreamOutput failed: %@", "\(error)")
            stream = nil
            return
        }
        stream = st
        st.startCapture { error in
            if let error { NSLog("[invert] startCapture failed: %@", "\(error)") }
        }
    }

    private static func schedule(_ delay: Double) {
        DispatchQueue.main.asyncAfter(deadline: .now() + delay) {
            tick()
        }
    }

    private static func tick() {
        if glaspen2_router_invert_enabled() == 0 { return }
        guard let surface = surfaceCurrent() else {
            schedule(0.5)
            return
        }
        if TunnelState.active {
            schedule(0.5)
            return
        }
        if glaspen2_has_strokes() == 0 && glaspen2_router_stroke_active() == 0 {
            stop()
            schedule(0.5)
            return
        }
        cairo_surface_flush(surface)
        let dw = Int(cairo_image_surface_get_width(surface))
        let dh = Int(cairo_image_surface_get_height(surface))
        if dw <= 0 || dh <= 0 { return }
        if captureW != dw || captureH != dh {
            stop()
            filter = nil
            captureW = dw
            captureH = dh
        }
        if glaspen2_rust_invert_ensure_cache(Int32(dw), Int32(dh)) == 0 { return }
        if filter == nil {
            SCShareableContent.getExcludingDesktopWindows(false,
                                                          onScreenWindowsOnly: true) { content, error in
                DispatchQueue.main.async {
                    if error != nil || content?.displays.isEmpty != false
                        || glaspen2_router_invert_enabled() == 0 {
                        schedule(1.0)
                        return
                    }
                    guard let content, let display = content.displays.first else {
                        schedule(1.0)
                        return
                    }
                    let mine = Bundle.main.bundleIdentifier
                    let excl = content.windows.filter {
                        $0.owningApplication?.bundleIdentifier == mine
                    }
                    filter = SCContentFilter(display: display, excludingWindows: excl)
                    glaspen2_router_set_window_sharing_none()
                    NSLog("[invert] 过滤器就绪(排除 %d 个自家窗口), 共享已关闭", excl.count)
                    tick()
                }
            }
            return
        }
        start()
    }
}

func settingsRunOnMain(_ block: @escaping () -> Void) {
    if Thread.isMainThread {
        block()
    } else {
        DispatchQueue.main.sync(execute: block)
    }
}

@_cdecl("glaspen2_swift_flip_page")
public func runFlipPage(_ target: Int64, _ goingNext: Int32, _ fromPage: Int64) {
    Flipper.swapFrom(goingNext: goingNext != 0, fromPage: fromPage, prepare: {
        glaspen2_load_strokes_for_screen(target)
        glaspen2_smooth_loaded_strokes()
        glaspen2_rust_pageview_update()
    }, commit: {
        glaspen2_rust_pageview_rebuild()
    })
}

@_cdecl("glaspen2_swift_copy_svg_html")
public func runCopySvgHtml(_ svg: UnsafePointer<CChar>?) {
    guard let svg else { return }
    let svgStr = String(cString: svg)
    let base64 = Data(svgStr.utf8).base64EncodedString()
    let html = "<img src=\"data:image/svg+xml;base64,\(base64)\" />"
    let pb = NSPasteboard.general
    pb.clearContents()
    pb.setString(html, forType: .string)
}
