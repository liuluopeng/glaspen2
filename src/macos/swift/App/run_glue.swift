// S7: run.m 剩余 C ABI 薄桥(Swift 化)。
import AppKit
import CoreGraphics
import Darwin

@_cdecl("glaspen2_run")
public func appRun() {
    if !AppBootstrap.start() { return }
    NSApp.run()
}

@_cdecl("glaspen2_router_camera_mods")
public func runCameraMods() -> UInt { MenuRuntime.maskFor(action: "canvas_zoom_in") }

@_cdecl("glaspen2_router_state")
public func runState() -> Int32 {
    glaspen2_router_enabled()
        | ((PenState.strokeActive ? 1 : 0) << 1)
        | ((glaspen2_router_infinite()) << 2)
}

@_cdecl("glaspen2_router_draw_height")
public func runDrawHeight() -> Double { Double(UIAnchors.drawView?.bounds.size.height ?? 0) }

@_cdecl("glaspen2_router_zoom_at")
public func runZoomAt(_ factor: Double, _ x: Double, _ y: Double) {
    CanvasCamera.zoomAt(factor: factor, vx: x, vy: y)
}

@_cdecl("glaspen2_router_cursor_flags")
public func runCursorFlags() -> Int32 {
    (glaspen2_router_pen_cursor_visible() != 0 ? 1 : 0)
        | ((PenState.strokeActive ? 1 : 0) << 1)
        | ((glaspen2_router_ethereal()) << 2)
        | ((PenState.hasSeenProximity ? 1 : 0) << 3)
}

@_cdecl("glaspen2_router_auto_hide")
public func runAutoHide() { _ = EtherealMode.hideNow() }

@_cdecl("glaspen2_router_bubble_pen_gone")
public func runBubblePenGone() { glaspen2_bubble_pen_gone() }

@_cdecl("glaspen2_router_flush")
public func runFlush() { OverlayLayerFlush.flushDirty() }

@_cdecl("glaspen2_virtual_pen_user_data")
public func runVirtualPenUserData() -> Int64 { 0x5650_4E31 }

@_cdecl("glaspen2_router_pen_state")
public func runPenState() -> Int32 {
    (glaspen2_router_strokes_visible())
        | ((PenState.strokeActive ? 1 : 0) << 1)
        | ((glaspen2_router_pen_cursor_visible() != 0 ? 1 : 0) << 2)
}

@_cdecl("glaspen2_router_pen_bubble")
public func runPenBubble(_ back: Int32) {
    if back != 0 { glaspen2_bubble_pen_back() } else { glaspen2_bubble_pen_activity() }
}

@_cdecl("glaspen2_router_pen_auto_show")
public func runPenAutoShow() { EtherealMode.autoShow() }

@_cdecl("glaspen2_router_peek_cancel")
public func runPeekCancel() { EtherealMode.cancelTimer() }

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
    CameraMousePan.cancel()
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
    StatusIconUpdater.apply(item: item, eraser: PenState.eraserMode,
                            enabled: glaspen2_router_enabled() != 0,
                            ethereal: glaspen2_router_ethereal() != 0,
                            infinite: glaspen2_router_infinite() != 0,
                            widthScale: glaspen2_router_width_scale(),
                            penRed: glaspen2_router_pen_r(),
                            penGreen: glaspen2_router_pen_g(),
                            penBlue: glaspen2_router_pen_b())
}

@_cdecl("glaspen2_router_bubble_auto_page_flip")
public func runBubbleAutoPageFlip() { _ = PageViewOps.bubbleAutoPageFlip() }

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
    _ = RecordingEffects.feed(slot: 1, json: json)
}

@_cdecl("glaspen2_router_rebuild_surface")
public func runRebuildSurface() { PageViewOps.rebuildSurface() }

@_cdecl("glaspen2_router_canvas_apply_transform")
public func runApplyTransform() { CanvasCamera.applyTransform() }

@_cdecl("glaspen2_router_peek_lens_change")
public func runPeekLensChange() { EtherealMode.peekLensChange() }

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
public func runCameraPanCancel() { CameraMousePan.cancel() }
@_cdecl("glaspen2_router_camera_persist_infinite")
public func runCameraPersistInfinite() { CanvasCamera.persistInfinite() }
@_cdecl("glaspen2_router_camera_load_infinite")
public func runCameraLoadInfinite() { CanvasCamera.loadInfinite() }
@_cdecl("glaspen2_router_camera_reset_lens")
public func runCameraResetLens() { CanvasCamera.resetLens() }

@_cdecl("glaspen2_router_restore_system_cursor")
public func runRestoreSystemCursor() { EventRouter.restoreSystemCursor() }
@_cdecl("glaspen2_router_main_window")
public func runMainWindow() -> UnsafeMutableRawPointer? {
    guard let w = UIAnchors.mainWindow else { return nil }
    return Unmanaged.passUnretained(w).toOpaque()
}
@_cdecl("glaspen2_router_ensure_surface")
public func runEnsureSurface() { MenuRuntime.ensureSurface() }
@_cdecl("glaspen2_router_pm_show")
public func runPmShow() { PressureMonitorWindow.show() }
@_cdecl("glaspen2_router_pm_hide")
public func runPmHide() { PressureMonitorWindow.hide() }

@_cdecl("glaspen2_router_flush_dirty")
public func runFlushDirty() { OverlayLayerFlush.flushDirty() }

@_cdecl("glaspen2_router_finish_active_stroke")
public func runFinishActiveStroke() { EventRouter.finishActiveStroke() }

@_cdecl("glaspen2_router_surface_cache_invalidate")
public func runSurfaceCacheInvalidate() { panelSurfaceImageCacheInvalidate() }

@_cdecl("glaspen2_router_live_plan_dot")
public func runLivePlanDot(_ cr: UnsafeMutableRawPointer?, _ x: Double, _ y: Double,
                           _ width: Double) -> Int32 {
    guard let cr else { return 0 }
    return LivePlan.drawDot(context: cr, x: x, y: y, width: width) ? 1 : 0
}

@_cdecl("glaspen2_router_live_plan_segment")
public func runLivePlanSegment(_ cr: UnsafeMutableRawPointer?, _ x: Double, _ y: Double,
                               _ width: Double) -> Int32 {
    guard let cr else { return 0 }
    return LivePlan.drawSegment(context: cr, x: x, y: y, width: width) ? 1 : 0
}

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
    CanvasMode.applyInfinite(on: on != 0, notify: true)
}

@_cdecl("glaspen2_router_clear_screen")
public func runClearScreen() { SurfaceOps.clearScreen() }

@_cdecl("glaspen2_router_replay_strokes")
public func runReplayStrokes() { SurfaceReplay.replay() }

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
    signal(SIGINT) { _ in saveAndExit(0) }
    signal(SIGTERM) { _ in saveAndExit(0) }
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
