// S7: 设置面板宿主(Flutter 引擎/VC 经 ObjC 薄桥创建, 窗口/激活流程在 Swift)。
import AppKit

@objc public final class SettingsPanel: NSObject {
    @objc public static func show() {
        if glaspen2_router_settings_window() != nil {
            NSApp.setActivationPolicy(.regular)
            NSApp.activate(ignoringOtherApps: true)
            (glaspen2_router_settings_window() as? NSWindow)?.makeKeyAndOrderFront(nil)
            return
        }
        _ = glaspen2_router_settings_engine()
        guard let vc = glaspen2_router_settings_vc() as? NSViewController else { return }
        let frame = NSRect(x: 0, y: 0, width: 900, height: 1200)
        let window = NSWindow(contentRect: frame,
                              styleMask: [.titled, .closable, .resizable],
                              backing: .buffered, defer: false)
        window.title = glaspen2_menu_l10n_pair("玻璃涂鸦", "glaspen2")
        window.minSize = NSSize(width: 600, height: 800)
        window.isReleasedWhenClosed = false
        if let delegate = glaspen2_router_settings_delegate() as? NSWindowDelegate {
            window.delegate = delegate
        }
        window.contentViewController = vc
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
        window.center()
        window.makeKeyAndOrderFront(nil)
        glaspen2_router_set_settings_window(window)
        if "panel_transparent".withCString({ glaspen2_load_bool_setting($0) }) != 0 {
            glaspen2_router_settings_apply_transparent()
        }
    }
}
