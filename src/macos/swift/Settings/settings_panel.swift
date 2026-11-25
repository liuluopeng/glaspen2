// S7: 设置面板宿主(Flutter 引擎/VC 经 ObjC 薄桥创建, 窗口/激活流程在 Swift)。
import AppKit

@objc public final class SettingsPanel: NSObject {
    @objc public static func show() {
        if UIAnchors.settingsWindow != nil {
            NSApp.setActivationPolicy(.regular)
            NSApp.activate(ignoringOtherApps: true)
            UIAnchors.settingsWindow?.makeKeyAndOrderFront(nil)
            return
        }
        _ = FlutterHost.ensureEngine()
        let vc = FlutterHost.ensureViewController()
        let title = MenuRuntime.pair("玻璃涂鸦", "glaspen2")
        var window: NSWindow
        if ProcessInfo.processInfo.environment["GLASPEN2_SWIFT_SETTINGS_PANEL"] != "1",
           let p = title.withCString({ glaspen2_rust_settings_window_create(
               $0, Unmanaged.passUnretained(vc).toOpaque(), 900, 1200) }) {
            window = Unmanaged<NSWindow>.fromOpaque(p).takeRetainedValue()
        } else {
        let frame = NSRect(x: 0, y: 0, width: 900, height: 1200)
        window = NSWindow(contentRect: frame,
                          styleMask: [.titled, .closable, .resizable],
                          backing: .buffered, defer: false)
        window.title = title
        window.minSize = NSSize(width: 600, height: 800)
        window.isReleasedWhenClosed = false
        window.contentViewController = vc
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
        window.center()
        }
        if ProcessInfo.processInfo.environment["GLASPEN2_SWIFT_SETTINGS_PANEL"] == "1" {
            if SettingsWindowDelegate.shared == nil {
                SettingsWindowDelegate.shared = SettingsWindowDelegate()
            }
            window.delegate = SettingsWindowDelegate.shared
        }
        window.makeKeyAndOrderFront(nil)
        UIAnchors.settingsWindow = window
        if "panel_transparent".withCString({ glaspen2_load_bool_setting($0) }) != 0 {
            glaspen2_router_settings_apply_transparent()
        }
    }
}
