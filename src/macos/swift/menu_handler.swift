// S3b-2b-7: 菜单 handler 的 delegate 与动作迁 Swift(ObjC 只转发)。
import AppKit

@objc public final class MenuHandler: NSObject {
    @objc public static func quitApp() { glaspen2_menu_quit_bridge() }
    @objc public static func menuWillOpen() { glaspen2_menu_will_open_bridge() }
    @objc public static func menuDidClose() { glaspen2_menu_did_close_bridge() }

    @objc(toggleLanguage)
    public static func toggleLanguage() {
        let next = glaspen2_menu_lang() == 1 ? 0 : 1
        glaspen2_menu_set_lang(Int32(next))
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
        glaspen2_menu_notify(glaspen2_menu_l10n("export.notes_saved"))
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
                    ? glaspen2_menu_l10n_pair("无限画布 PDF 已导出到桌面", "Infinite-canvas PDF saved to Desktop")
                    : glaspen2_menu_l10n_pair("无限画布为空或页数过多", "Infinite canvas empty or too many pages")
                glaspen2_menu_notify(text)
            }
        }
    }

    @objc(exportInfiniteSvg)
    public static func exportInfiniteSvg() {
        DispatchQueue.global(qos: .userInitiated).async {
            let ok = glaspen2_menu_export_infinite_svg()
            DispatchQueue.main.async {
                let text = ok != 0
                    ? glaspen2_menu_l10n_pair("无限画布 SVG 已导出到桌面", "Infinite-canvas SVG saved to Desktop")
                    : glaspen2_menu_l10n_pair("无限画布为空", "Infinite canvas is empty")
                glaspen2_menu_notify(text)
            }
        }
    }
}
