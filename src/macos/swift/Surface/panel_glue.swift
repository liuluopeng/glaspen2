// S7: 面板/显示/反色/窗口共享薄桥(Swift 化, 取代 parts/panel.m)。
import AppKit
import CoreGraphics

@_cdecl("show_settings_panel")
public func panelShowSettings() { SettingsPanel.show() }

@_cdecl("surface_image_cache_invalidate")
public func panelSurfaceImageCacheInvalidate() { OverlaySurfaceImageCache.clear() }

@_cdecl("ensure_surface")
public func panelEnsureSurface(_ viewRaw: UnsafeMutableRawPointer?) {
    guard let viewRaw else { return }
    let view = Unmanaged<NSView>.fromOpaque(viewRaw).takeUnretainedValue()
    SurfaceStore.ensure(view: view)
}

@_cdecl("flush_to_layer")
public func panelFlushToLayer() { OverlayLayerFlush.flush() }

@_cdecl("flush_dirty_to_layer")
public func panelFlushDirtyToLayer() { OverlayLayerFlush.flushDirty() }

@_cdecl("glaspen2_display_size")
public func panelDisplaySize(_ w: UnsafeMutablePointer<Int32>?, _ h: UnsafeMutablePointer<Int32>?) {
    w?.pointee = Int32(glaspen2_router_screen_w())
    h?.pointee = Int32(glaspen2_router_screen_h())
}

@_cdecl("glaspen2_display_apply")
public func panelDisplayApply(_ w: Int32, _ h: Int32) {
    OverlayWindowFactory.applyDisplayChange(width: w, height: h)
}

@_cdecl("gl_panel_transparent_apply_full")
public func panelTransparentFull(_ on: Int32) {
    MenuRuntime.applyPanelTransparent(on: on != 0)
    let vc = FlutterHost.ensureViewController()
    vc.view.layer?.isOpaque = on == 0
}

@_cdecl("glaspen2_router_set_window_sharing_none")
public func panelSharingNone() {
    UIAnchors.mainWindow?.sharingType = .none
    UIAnchors.settingsWindow?.sharingType = .none
}

@_cdecl("glaspen2_router_set_window_sharing_readonly")
public func panelSharingReadOnly() {
    UIAnchors.mainWindow?.sharingType = .readOnly
    UIAnchors.settingsWindow?.sharingType = .readOnly
}

@_cdecl("invert_ink_source")
public func panelInvertInkSource(_ cr: UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? {
    if #available(macOS 12.3, *), let cr {
        return InvertCapture.inkSource(crRaw: cr)
    }
    return nil
}

@_cdecl("invert_ink_apply")
public func panelInvertApply(_ on: Int32) {
    if #available(macOS 12.3, *) { InvertCapture.apply(on: on != 0) }
}

@_cdecl("invert_stream_restart")
public func panelInvertRestart() {
    if #available(macOS 12.3, *) { InvertCapture.restart() }
}

@_cdecl("invert_stream_invalidate")
public func panelInvertInvalidate() {
    if #available(macOS 12.3, *) { InvertCapture.invalidate() }
}

@_cdecl("glaspen2_router_outline_dash")
public func panelOutlineDash() -> Double { 1.0 }
