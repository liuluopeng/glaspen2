// S7: 主菜单构建(颜色/粗细富文本项 + 顶层与子菜单), 文案经 core i18n。
import AppKit

@objc public final class MainMenuBuilder: NSObject {
    @objc(buildWithHandler:statusItem:launchAtLoginOn:glassOn:)
    public static func build(handler: AnyObject, statusItem: NSStatusItem,
                             launchAtLoginOn: Bool, glassOn: Bool) -> NSMenu {
        let menu: NSMenu
        if let p = glaspen2_rust_menu_create() {
            menu = Unmanaged<NSMenu>.fromOpaque(p).takeRetainedValue()
        } else {
            menu = NSMenu()
        }
        menu.delegate = handler as? NSMenuDelegate
        menu.autoenablesItems = false

        var colorTitles: [NSAttributedString] = []
        for i in 0..<Int(glaspen2_color_preset_count()) {
            colorTitles.append(MenuRuntime.colorItemTitle(Int32(i)))
        }
        MenuBuilder.addColorItems(toMenu: menu, handler: handler,
                                  count: Int(glaspen2_color_preset_count()),
                                  titles: colorTitles)
        menu.addItem(.separator())

        var widthTitles: [NSAttributedString] = []
        for i in 0..<Int(glaspen2_width_preset_count()) {
            widthTitles.append(MenuRuntime.widthItemTitle(Int32(i)))
        }
        MenuBuilder.addWidthItems(toMenu: menu, handler: handler,
                                  count: Int(glaspen2_width_preset_count()),
                                  titles: widthTitles)

        func pair(_ zh: String, _ en: String) -> String {
            MenuRuntime.pair(zh, en)
        }
        let titles: [String: String] = [
            "saveWithBg": pair("保存(含背景)", "Save (with bg)"),
            "saveDrawing": pair("保存(涂鸦)", "Save (drawing)"),
            "newCanvas": pair("新建画布", "New canvas"),
            "export": pair("导出", "Export"),
            "settings": pair("设置...", "Settings..."),
            "more": pair("更多", "More"),
            "toggleDraw": pair("开启涂鸦", "Enable Drawing"),
            "language": pair("English", "中文"),
            "quit": pair("退出", "Quit"),
            "exportXournal": pair("保存笔记 (Xournal)", "Save Notes (Xournal)"),
            "exportInfinitePdf": pair("导出无限画布 PDF (分页)",
                                        "Export infinite canvas PDF (paged)"),
            "exportInfiniteSvg": pair("导出无限画布 SVG (整幅)",
                                        "Export infinite canvas SVG (whole)"),
            "moreLaunch": pair("开机自启", "Launch at login"),
            "moreGlass": pair("磨砂玻璃", "Frosted Glass"),
            "moreOutline": pair("笔迹描边", "Stroke outline"),
            "moreCanvasInfinite": MenuRuntime.l10n("canvas.mode_infinite") ?? "",
            "moreCanvasMode": MenuRuntime.l10n("canvas.fixed_mode") ?? "",
        ]
        MenuBuilder.addTopLevelAndSubmenus(toMenu: menu, handler: handler,
                                           titles: titles,
                                           launchAtLoginOn: launchAtLoginOn,
                                           glassOn: glassOn)
        glaspen2_rust_status_item_set_menu(
            Unmanaged.passUnretained(statusItem).toOpaque(),
            Unmanaged.passUnretained(menu).toOpaque())
        return menu
    }
}

@objc public final class AppDelegate: NSObject, NSApplicationDelegate, NSMenuDelegate {
    static var shared: AppDelegate?
    @objc public func saveWithBg() { glaspen2_menu_save_with_bg() }
    @objc public func saveOnly() { glaspen2_menu_save_only() }
    @objc public func clearScreen() { glaspen2_menu_clear_screen() }
    @objc public func toggleDraw() { glaspen2_menu_toggle_enabled() }
    @objc public func saveXoj() { glaspen2_rust_menu_save_xoj() }
    @objc public func toggleLanguage() { glaspen2_rust_menu_toggle_language() }
    @objc public func showSettingsPanel() { glaspen2_menu_show_settings() }
    @objc public func toggleCanvasMode() { glaspen2_menu_toggle_canvas_mode() }
    @objc public func toggleLaunch() { glaspen2_rust_set_launch_at_login(glaspen2_is_launch_at_login() == 0 ? 1 : 0) }
    @objc public func toggleGlass() { glaspen2_rust_set_glass_enabled(glaspen2_router_glass_enabled() == 0 ? 1 : 0) }
    @objc public func toggleOutline() { glaspen2_rust_menu_set_outline_enabled(glaspen2_router_outline_enabled() == 0 ? 1 : 0) }
    @objc public func toggleInfiniteCanvas() { glaspen2_menu_apply_infinite(glaspen2_router_infinite() == 0 ? 1 : 0) }
    @objc public func exportInfinitePdf() { glaspen2_rust_menu_export_pdf_async() }
    @objc public func exportInfiniteSvg() { glaspen2_rust_menu_export_svg_async() }
    @objc(selectColor:)
    public func selectColor(_ sender: NSMenuItem) { glaspen2_menu_select_color(Int32(sender.tag)) }
    @objc(selectWidth:)
    public func selectWidth(_ sender: NSMenuItem) { glaspen2_menu_select_width(Int32(sender.tag)) }
    public func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        glaspen2_menu_shutdown()
        return .terminateNow
    }
    @objc public func quitApp() { glaspen2_rust_menu_quit_app() }
    public func menuWillOpen(_ menu: NSMenu) { glaspen2_menu_will_open_bridge() }
    public func menuDidClose(_ menu: NSMenu) { glaspen2_menu_did_close_bridge() }
}
