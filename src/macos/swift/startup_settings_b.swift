// S7: 启动设置恢复(后半): 画布模式存储/压感窗/GIF 参数/聊天集成/延迟重建。
import AppKit
import Darwin

@objc public final class StartupSettingsB: NSObject {
    @objc public static func restorePartB() {
        let infinite = glaspen2_load_bool_setting("infinite_canvas") != 0
        let minimap = glaspen2_load_bool_setting("minimap") != 0
        let ethereal = glaspen2_load_bool_setting("ethereal") != 0
        glaspen2_router_restore_canvas_flags(infinite ? 1 : 0,
                                             minimap ? 1 : 0,
                                             ethereal ? 1 : 0)
        glaspen2_router_update_menu_state(778, ethereal ? 1 : 0)
        let modeKey = ethereal ? "canvas.fixed_mode" : "canvas.ethereal_mode"
        if let title = Optional(MenuRuntime.l10n(modeKey)) {
            MenuRuntime.setMenuTitle(tag: 778, title: title)
        }
        glaspen2_router_update_menu_state(668, infinite ? 1 : 0)
        glaspen2_set_canvas_kind(infinite ? 1 : 0)
        if infinite {
            glaspen2_load_infinite_strokes()
            CanvasCamera.loadInfinite()
        } else {
            CanvasCamera.resetLens()
        }
        // 注意: 别在这里无条件覆盖 778 的标题 —— 上一段已按 ethereal 状态
        // 设成“固定画布/飘渺画布”的动作名, 再写固定文案会让菜单显示反。

        let pressure = glaspen2_load_bool_setting("pressure_monitor") != 0
        glaspen2_router_restore_pressure(pressure ? 1 : 0)
        if pressure {
            DispatchQueue.main.async {
                PressureMonitorWindow.ensureWindow()
                PressureMonitorWindow.show()
                glaspen2_router_pm_refresh()
            }
        }

        var gifFps = glaspen2_router_gif_fps()
        var gifRes = glaspen2_router_gif_resolution()
        var gifSpeed = glaspen2_router_gif_speed()
        var gifEnd = glaspen2_router_gif_end_mode()
        if let v = loadString("gif_fps"), let n = Int32(v) { gifFps = n }
        if let v = loadString("gif_resolution"), let d = Double(v) { gifRes = d }
        if let v = loadString("gif_speed"), let d = Double(v) { gifSpeed = d }
        if let v = loadString("gif_end_mode"), let n = Int32(v) { gifEnd = n }
        gifFps = glaspen2_clamp_setting_int("gifFps", gifFps)
        gifRes = glaspen2_clamp_setting_double("gifResolution", gifRes)
        gifSpeed = glaspen2_clamp_setting_double("gifSpeed", gifSpeed)
        gifEnd = glaspen2_clamp_setting_int("gifEndMode", gifEnd)
        glaspen2_router_restore_gif_settings(gifFps, gifRes, gifSpeed, gifEnd)

        glaspen2_router_update_status_icon_state()
        glaspen2_chat_auth_reload()

        let chat = glaspen2_load_bool_setting("chat_integration") != 0
        glaspen2_router_restore_chat_integration(chat ? 1 : 0)
        glaspen2_router_rec_init()
        if chat && glaspen2_load_bool_setting("share_ink") != 0 {
            glaspen2_share_ink_set_active(1)
        }

        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
            if glaspen2_router_stroke_active() != 0 { return }
            MenuRuntime.ensureSurface()
            PageViewOps.rebuildSurface()
        }
    }

    private static func loadString(_ key: String) -> String? {
        guard let v = key.withCString({ glaspen2_load_string_setting($0) }) else {
            return nil
        }
        let s = String(cString: v)
        glaspen2_free_c_string(v)
        return s
    }
}
