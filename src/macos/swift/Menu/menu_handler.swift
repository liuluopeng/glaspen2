// S3b-2b-7: 菜单 handler 的 delegate 与动作迁 Swift(ObjC 只转发)。
import AppKit

@objc public final class MenuHandler: NSObject {
    @objc public static func quitApp() { glaspen2_menu_quit_bridge() }
    @objc public static func menuWillOpen() { glaspen2_menu_will_open_bridge() }
    @objc public static func menuDidClose() { glaspen2_menu_did_close_bridge() }

    @objc(toggleLanguage)
    public static func toggleLanguage() {
        let next = glaspen2_router_lang() == 1 ? 0 : 1
        menuSetLang(Int32(next))
        let code = next == 1 ? "en" : "zh"
        "language".withCString { key in
            code.withCString { value in glaspen2_save_string_setting(key, value) }
        }
        glaspen2_menu_refresh_texts()
        glaspen2_notify_settings_changed()
    }

    @objc(toggleLaunch)
    public static func toggleLaunch() {
        glaspen2_menu_set_launch(glaspen2_menu_launch_at_login() == 0 ? 1 : 0)
    }

    @objc(toggleGlass)
    public static func toggleGlass() {
        glaspen2_menu_set_glass(glaspen2_menu_glass_enabled() == 0 ? 1 : 0)
    }

    @objc(toggleOutline)
    public static func toggleOutline() {
        glaspen2_menu_apply_outline(glaspen2_menu_outline_enabled() == 0 ? 1 : 0)
    }

    @objc(toggleInfiniteCanvas)
    public static func toggleInfiniteCanvas() {
        glaspen2_menu_apply_infinite(glaspen2_menu_infinite_enabled() == 0 ? 1 : 0)
    }

    @objc(saveWithBg) public static func saveWithBg() { glaspen2_menu_save_with_bg() }
    @objc(saveOnly) public static func saveOnly() { glaspen2_menu_save_only() }
    @objc(clearScreen) public static func clearScreen() { glaspen2_menu_clear_screen() }
    @objc(toggleDraw) public static func toggleDraw() { glaspen2_menu_toggle_enabled() }
    @objc(showSettingsPanel) public static func showSettingsPanel() { glaspen2_menu_show_settings() }
    @objc(toggleCanvasMode) public static func toggleCanvasMode() { glaspen2_menu_toggle_canvas_mode() }

    @objc(selectColor:)
    public static func selectColor(_ sender: NSMenuItem) {
        glaspen2_menu_select_color(Int32(sender.tag))
    }

    @objc(selectWidth:)
    public static func selectWidth(_ sender: NSMenuItem) {
        glaspen2_menu_select_width(Int32(sender.tag))
    }

    @objc(saveXoj)
    public static func saveXoj() {
        glaspen2_menu_save_xoj_raw()
        MenuRuntime.notify(MenuRuntime.l10n("export.notes_saved"))
    }

    /// 描边开关: 状态/FFI/落库/菜单勾选/重建笔迹/提示。
    @objc(setOutlineEnabled:)
    public static func setOutlineEnabled(enabled: Bool) {
        if (glaspen2_router_outline_enabled() != 0) == enabled { return }
        glaspen2_router_set_outline_state(enabled ? 1 : 0)
        glaspen2_set_stroke_outline(enabled ? 1 : 0)
        "outline".withCString { key in
            glaspen2_save_bool_setting(key, enabled ? 1 : 0)
        }
        glaspen2_router_update_menu_state(667, enabled ? 1 : 0)
        EventRouter.finishActiveStroke()
        glaspen2_router_rebuild_surface()
        let text = enabled
            ? MenuRuntime.pair("笔迹描边已开启", "Stroke outline on")
            : MenuRuntime.pair("笔迹描边已关闭", "Stroke outline off")
        MenuRuntime.notify(text)
    }

    /// 磨砂玻璃应用: 组合 enabled/opacity/飘渺联动, 刷新视图与菜单勾选。
    @objc(applyGlass)
    public static func applyGlass() {
        let enabled = RenderSettings.glassEnabled
        let opacity = RenderSettings.glassOpacity
        let follow = RenderSettings.glassFollow
        let ethereal = glaspen2_router_ethereal() != 0
        let strokesVisible = ViewSettings.strokesVisible
        let hiddenByEthereal = ethereal && follow && !strokesVisible
        glaspen2_router_apply_glass_view(enabled ? opacity * 2.0 : 0.0,
                                        (!enabled || hiddenByEthereal) ? 1 : 0)
        glaspen2_router_update_menu_state(444, enabled ? 1 : 0)
    }

    /// 压感监视器标签格式化(P=压力 事件 坐标)。
    @objc(updatePressureLabelWithPressure:type:x:y:)
    public static func updatePressureLabel(pressure: Int32, type: String?,
                                           x: Double, y: Double) {
        let typeString = type ?? "---"
        let text = String(format: "P=%-5d  ", pressure) + typeString
            + String(format: "  (%d,%d)", Int(x), Int(y))
        PressureMonitorWindow.updateLabel(text)
    }

    /// 手写消息集成总开关: 状态/落库/录制状态机通知/共享上行恢复。
    @objc(setChatIntegration:)
    public static func setChatIntegration(enabled: Bool) {
        glaspen2_router_set_chat_integration(enabled ? 1 : 0)
        "chat_integration".withCString { key in
            glaspen2_save_bool_setting(key, enabled ? 1 : 0)
        }
        glaspen2_router_rec_chat_integration_changed(enabled ? 1 : 0)
        if !enabled {
            glaspen2_share_ink_set_active(0)
        } else {
            let share = "share_ink".withCString { glaspen2_load_bool_setting($0) }
            if share != 0 { glaspen2_share_ink_set_active(1) }
        }
    }

    /// 涂鸦总开关: 状态 + 菜单勾选/标题 + 状态图标。
    @objc(setDrawEnabled:)
    public static func setDrawEnabled(enabled: Bool) {
        glaspen2_router_set_enabled_state(enabled ? 1 : 0)
        glaspen2_router_update_menu_state(888, enabled ? 1 : 0)
        let zh = enabled ? "关闭涂鸦" : "开启涂鸦"
        let en = enabled ? "Disable Drawing" : "Enable Drawing"
        MenuRuntime.setMenuTitle(tag: 888, title: MenuRuntime.pair(zh, en))
        glaspen2_router_update_status_icon_state()
    }

    /// 压感监视器开关: 状态/落库/显示隐藏/面板同步。
    @objc(setPressureMonitor:)
    public static func setPressureMonitor(enabled: Bool) {
        glaspen2_router_set_pressure_monitor(enabled ? 1 : 0)
        "pressure_monitor".withCString { key in
            glaspen2_save_bool_setting(key, enabled ? 1 : 0)
        }
        if enabled {
            glaspen2_router_pm_show()
            glaspen2_router_pm_refresh()
        } else {
            glaspen2_router_pm_hide()
        }
        glaspen2_router_sync_settings_panel()
    }

    /// 网格开关: 状态/落库/面板同步/重绘。
    @objc(setGridEnabled:)
    public static func setGridEnabled(enabled: Bool) {
        glaspen2_router_set_grid_state(enabled ? 1 : 0)
        "grid".withCString { key in
            glaspen2_save_bool_setting(key, enabled ? 1 : 0)
        }
        glaspen2_router_sync_settings_panel()
        glaspen2_router_redraw_draw_view()
    }

    /// 开机自启开关: 系统设置 + 菜单勾选 + 面板同步。
    @objc(setLaunchAtLogin:)
    public static func setLaunchAtLogin(enabled: Bool) {
        glaspen2_set_launch_at_login(enabled ? 1 : 0)
        glaspen2_router_update_menu_state(777, enabled ? 1 : 0)
        glaspen2_router_sync_settings_panel()
    }

    /// 磨砂玻璃开关: 状态/落库/应用。
    @objc(setGlassEnabled:)
    public static func setGlassEnabled(enabled: Bool) {
        glaspen2_router_set_glass_state(enabled ? 1 : 0)
        "glass_enabled".withCString { key in
            glaspen2_save_bool_setting(key, enabled ? 1 : 0)
        }
        glaspen2_router_glass_apply()
    }

    /// 颜色设置: 书写中暂存, 否则应用并刷新菜单/状态图标/面板。
    @objc(setColorIndex:)
    public static func setColor(index: Int32) {
        let count = glaspen2_router_color_preset_count()
        guard index >= 0 && Int(index) < Int(count) else { return }
        if glaspen2_router_stroke_active() != 0 {
            glaspen2_router_set_pending_color(index)
            return
        }
        let selected = glaspen2_router_selected_color()
        if index != selected { glaspen2_router_set_prev_color(selected) }
        glaspen2_router_apply_color(index)
        glaspen2_router_save_pen_settings()
        glaspen2_router_update_checkmarks()
        glaspen2_router_update_status_icon_state()
        glaspen2_router_sync_settings_panel()
    }

    /// 粗细设置: 书写中暂存, 否则应用并刷新菜单/状态图标/面板。
    @objc(setWidthIndex:)
    public static func setWidth(index: Int32) {
        let count = glaspen2_router_width_preset_count()
        guard index >= 0 && Int(index) < Int(count) else { return }
        if glaspen2_router_stroke_active() != 0 {
            glaspen2_router_set_pending_width(index)
            return
        }
        let selected = glaspen2_router_selected_width()
        if index != selected { glaspen2_router_set_prev_width(selected) }
        glaspen2_router_apply_width(index)
        glaspen2_router_save_pen_settings()
        glaspen2_router_update_checkmarks()
        glaspen2_router_update_status_icon_state()
        glaspen2_router_sync_settings_panel()
    }

    /// 书写中暂存的颜色/粗细在抬笔后生效(原 gl_settings_apply_pending_color_width)。
    @objc(applyPendingColorWidth)
    public static func applyPendingColorWidth() {
        if glaspen2_router_stroke_active() != 0 { return }
        let color = glaspen2_router_pending_color()
        if color >= 0 {
            glaspen2_router_clear_pending_color()
            glaspen2_menu_select_color(Int32(color))
        }
        let width = glaspen2_router_pending_width()
        if width >= 0 {
            glaspen2_router_clear_pending_width()
            glaspen2_menu_select_width(Int32(width))
        }
    }

    /// 彻底退出: perf 汇总 + 恢复系统光标 + NSApp terminate(Swift 实现)。
    @objc(quitAppNow)
    public static func quitAppNow() {
        glaspen2_router_perf_summary()
        glaspen2_router_show_system_cursor()
        NSApp.terminate(nil)
    }

    @objc(applicationShouldTerminate:)
    public static func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        glaspen2_menu_shutdown()
        return .terminateNow
    }

    @objc(exportInfinitePdf)
    public static func exportInfinitePdf() {
        var w: Int32 = 0
        var h: Int32 = 0
        glaspen2_menu_screen_size(&w, &h)
        DispatchQueue.global(qos: .userInitiated).async {
            let ok = glaspen2_menu_export_infinite_pdf_paged(w, h)
            DispatchQueue.main.async {
                let text = ok != 0
                    ? MenuRuntime.pair("无限画布 PDF 已导出到桌面", "Infinite-canvas PDF saved to Desktop")
                    : MenuRuntime.pair("无限画布为空或页数过多", "Infinite canvas empty or too many pages")
                MenuRuntime.notify(text)
            }
        }
    }

    @objc(exportInfiniteSvg)
    public static func exportInfiniteSvg() {
        DispatchQueue.global(qos: .userInitiated).async {
            let ok = glaspen2_menu_export_infinite_svg()
            DispatchQueue.main.async {
                let text = ok != 0
                    ? MenuRuntime.pair("无限画布 SVG 已导出到桌面", "Infinite-canvas SVG saved to Desktop")
                    : MenuRuntime.pair("无限画布为空", "Infinite canvas is empty")
                MenuRuntime.notify(text)
            }
        }
    }
}
