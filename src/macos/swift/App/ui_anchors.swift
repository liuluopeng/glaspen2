// S7: UI 锚点(主窗口/绘制视图/玻璃/状态项/菜单/设置窗口) — 状态在 Swift。
import AppKit

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
