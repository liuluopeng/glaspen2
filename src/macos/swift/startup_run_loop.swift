// S7: 运行期初始化(tap 健康看门狗/调试开关)。
import AppKit
import ApplicationServices

@objc public final class StartupRunLoop: NSObject {
    private static var axWarned = false

    @objc public static func begin() {
        Timer.scheduledTimer(withTimeInterval: 2.0, repeats: true) { _ in
            checkTap()
        }
        PerfLog.initFromEnv()
        VirtualPen.maybeStart()
        FlipProbe.maybeStart()
    }

    private static func checkTap() {
        if !AXIsProcessTrusted() {
            if !axWarned {
                axWarned = true
                let text = glaspen2_menu_l10n_pair(
                    "辅助功能权限已失效, 涂鸦/热键暂停 — 请到系统设置重新勾选",
                    "Accessibility permission lost — re-enable it in System Settings")
                glaspen2_router_show_notification(text)
            }
            return
        }
        axWarned = false
        if glaspen2_router_event_tap_ok() == 0 {
            glaspen2_router_event_tap_reinstall()
        } else if glaspen2_router_event_tap_enabled() == 0 {
            glaspen2_router_event_tap_enable()
        }
    }
}
