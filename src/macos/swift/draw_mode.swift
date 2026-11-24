// S7: 涂鸦启停(覆盖层显隐/在写笔画收尾/压感窗/状态图标/菜单/通知)。
import AppKit

@objc public final class DrawMode: NSObject {
    @objc public static func toggleEnabled() {
        let enabled = glaspen2_router_enabled() == 0
        glaspen2_router_set_enabled_state(enabled ? 1 : 0)
        let window = glaspen2_router_main_window() as? NSWindow
        if !enabled {
            EventRouter.restoreSystemCursor()
            EventRouter.finishActiveStroke()
            window?.orderOut(nil)
            if glaspen2_router_pressure_monitor() != 0 { PressureMonitorWindow.hide() }
        } else {
            if let window {
                glaspen2_router_ensure_surface()
                if glaspen2_router_ethereal() != 0
                    && glaspen2_router_strokes_visible() == 0 {
                    glaspen2_router_rebuild_surface()
                    window.setIsVisible(false)
                } else {
                    window.orderFrontRegardless()
                    glaspen2_router_rebuild_surface()
                }
            }
            if glaspen2_router_pressure_monitor() != 0 { PressureMonitorWindow.show() }
        }
        glaspen2_router_update_status_icon_state()
        let key = enabled ? "draw.enabled" : "draw.disabled"
        glaspen2_menu_notify(glaspen2_menu_l10n(key))
        glaspen2_router_update_menu_state(888, enabled ? 1 : 0)
        let zh = enabled ? "关闭涂鸦" : "开启涂鸦"
        let en = enabled ? "Disable Drawing" : "Enable Drawing"
        glaspen2_router_set_menu_title(888, glaspen2_menu_l10n_pair(zh, en))
    }
}
