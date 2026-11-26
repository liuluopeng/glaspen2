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
