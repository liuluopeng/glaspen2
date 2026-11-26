// S7: 应用启动编排(单实例守卫/NSApp/菜单/DB/信号/显示器观察/tap/运行期钩子)。
import AppKit
import ApplicationServices

@objc public final class AppBootstrap: NSObject {
    private static var displayObserver: Any?

    @objc public static func start() -> Bool {

        let lockFd = InstanceLock.acquire()
        if lockFd == -1 || AppInstanceGuard.anotherInstanceRunning() {
            NSApplication.shared
            NSApp.setActivationPolicy(.accessory)
            let alert = NSAlert()
            alert.messageText = MenuRuntime.pair("glaspen2 已在运行",
                                                        "glaspen2 is already running")
            alert.informativeText = MenuRuntime.pair(
                "检测到另一个 glaspen2 实例(菜单栏图标)。两个实例的笔迹窗口会叠加显示, 全局热键会同时翻两套页 —— 看起来像“笔迹重复/只翻了一部分”。本次启动退出; 开发时如需并行调试, 设 GLASPEN2_ALLOW_MULTI=1。",
                "Another glaspen2 instance is running (menu bar icon). Two overlays stack and global hotkeys flip both — it looks like duplicated strokes or a partial page flip. This launch exits; set GLASPEN2_ALLOW_MULTI=1 to debug in parallel.")
            alert.addButton(withTitle: MenuRuntime.pair("好", "OK"))
            alert.runModal()
            return false
        }

        NSApplication.shared
        NSApp.setActivationPolicy(.accessory)

        let opts = [kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true]
        if !AXIsProcessTrustedWithOptions(opts as CFDictionary) {
            NSLog("[glaspen2] Accessibility permission not granted")
        }

        var statusItem = StatusBarFactory.createSquareItem()
        if statusItem == nil {
            statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
            statusItem?.button?.title = ""
        }
        guard let statusItem else { return false }
        UIAnchors.statusItem = statusItem

        let handler = AppDelegate()
        AppDelegate.shared = handler
        NSApp.delegate = handler

        if ProcessInfo.processInfo.environment["GLASPEN2_RUST_STATUS_ICON_TEST"] == "1" {
            _ = StatusIconParity.check()
        }

        let menu = MainMenuBuilder.build(handler: handler, statusItem: statusItem,
                                         launchAtLoginOn: glaspen2_is_launch_at_login() != 0,
                                         glassOn: glaspen2_router_glass_enabled() != 0)
        UIAnchors.menu = menu

        let frame = NSScreen.main?.frame ?? .zero
        glaspen2_router_set_screen_size(Int32(frame.size.width), Int32(frame.size.height))
        glaspen2_init_db(Int32(frame.size.width), Int32(frame.size.height))

        glaspen2_rust_restore_settings_a()
        glaspen2_rust_restore_settings_b()
        _ = StartupWindow.create()

        glaspen2_router_install_signals()
        displayObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.didChangeScreenParametersNotification,
            object: nil, queue: .main) { _ in
            DisplayChangeMonitor.onDisplayChanged()
        }
        glaspen2_router_event_tap_reinstall()
        AccessibilityAlert.presentIfNeeded()
        StartupRunLoop.begin()
        return true
    }
}
