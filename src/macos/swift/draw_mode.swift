// S7: 涂鸦启停(覆盖层显隐/在写笔画收尾/压感窗/状态图标/菜单/通知)。
import AppKit

@objc public final class DrawMode: NSObject {
    @objc public static func toggleEnabled() {
        let enabled = glaspen2_router_enabled() == 0
        glaspen2_router_set_enabled_state(enabled ? 1 : 0)
        let window = UIAnchors.mainWindow
        if !enabled {
            EventRouter.restoreSystemCursor()
            EventRouter.finishActiveStroke()
            window?.orderOut(nil)
            if glaspen2_router_pressure_monitor() != 0 { PressureMonitorWindow.hide() }
        } else {
            if let window {
                MenuRuntime.ensureSurface()
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
        MenuRuntime.notify(MenuRuntime.l10n(key))
        glaspen2_router_update_menu_state(888, enabled ? 1 : 0)
        let zh = enabled ? "关闭涂鸦" : "开启涂鸦"
        let en = enabled ? "Disable Drawing" : "Enable Drawing"
        MenuRuntime.setMenuTitle(tag: 888, title: MenuRuntime.pair(zh, en))
    }

    /// 飘渺画布 ↔ 固定画布: 笔迹显隐/玻璃跟随/压感窗/菜单文案/落库/图标。
    @objc public static func toggleCanvasMode() {
        glaspen2_router_peek_cancel()
        if glaspen2_router_ethereal() == 0 {
            glaspen2_router_set_ethereal(1)
            EventRouter.finishActiveStroke()
            glaspen2_router_set_strokes_visible(0)
            if glaspen2_router_glass_follow() != 0,
               let glass = UIAnchors.glassView as? NSView {
                glass.isHidden = true
            }
            if glaspen2_router_pressure_monitor() != 0 { PressureMonitorWindow.hide() }
            glaspen2_router_redraw_draw_view()
            MenuRuntime.notify(MenuRuntime.l10n("canvas.ethereal_mode"))
        } else {
            glaspen2_router_set_ethereal(0)
            glaspen2_router_set_strokes_visible(1)
            MenuHandler.applyGlass()
            if glaspen2_router_pressure_monitor() != 0 { PressureMonitorWindow.show() }
            glaspen2_router_redraw_draw_view()
            MenuRuntime.notify(MenuRuntime.l10n("canvas.fixed_mode"))
        }
        let ethereal = glaspen2_router_ethereal() != 0
        glaspen2_router_update_menu_state(778, ethereal ? 1 : 0)
        MenuRuntime.setMenuTitle(tag: 778,
            title: MenuRuntime.l10n(ethereal ? "canvas.fixed_mode" : "canvas.ethereal_mode"))
        "ethereal".withCString { glaspen2_save_bool_setting($0, ethereal ? 1 : 0) }
        glaspen2_router_update_status_icon_state()
    }
}
