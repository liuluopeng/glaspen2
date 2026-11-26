// S7: 启动窗口/视图组装与失焦事件 tap 的辅助功能告警。
import AppKit

@objc public final class StartupWindow: NSObject {
    @objc public static func create() -> Bool {
        glaspen2_rust_create_main_window() != 0
    }
}

@objc public final class AccessibilityAlert: NSObject {
    private static var presented = false

    @objc public static func presentIfNeeded() {
        if glaspen2_router_event_tap_ok() != 0 {
            NSLog("[glaspen2] CGEventTap created OK")
            return
        }
        let bundlePath = Bundle.main.bundlePath
        NSLog("[glaspen2] CGEventTap FAILED - need Accessibility permission for: %@",
              bundlePath)
        let alert = NSAlert()
        alert.messageText = MenuRuntime.pair("需要辅助功能权限",
                                                    "Accessibility Permission Required")
        alert.informativeText = MenuRuntime.pair(
            "请在系统设置 → 隐私与安全性 → 辅助功能中添加并勾选 glaspen2。\n\n添加后请点击下方按钮重启应用。",
            "Please add and enable glaspen2 in System Settings → Privacy & Security → Accessibility.\n\nAfter enabling, click below to restart.")
        alert.addButton(withTitle: MenuRuntime.pair("打开系统设置并重启",
                                                           "Open Settings & Restart"))
        alert.addButton(withTitle: MenuRuntime.pair("取消", "Cancel"))
        DispatchQueue.main.async {
            guard alert.runModal() == .alertFirstButtonReturn else { return }
            if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility") {
                NSWorkspace.shared.open(url)
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
                NSWorkspace.shared.launchApplication(bundlePath)
                NSApp.terminate(nil)
            }
        }
    }
}
