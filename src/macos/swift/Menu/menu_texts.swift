// S7: 菜单文案刷新(颜色/粗细富文本 + 顶层/导出/更多/模式/总开关文案)。
import AppKit

@objc public final class MenuTextUpdater: NSObject {
    @objc public static func update() {
        guard let menu = UIAnchors.menu as? NSMenu else { return }
        func pair(_ zh: String, _ en: String) -> String {
            MenuRuntime.pair(zh, en)
        }
        var colorTitles: [NSAttributedString] = []
        for i in 0..<Int(glaspen2_color_preset_count()) {
            colorTitles.append(MenuRuntime.colorItemTitle(Int32(i)))
        }
        var widthTitles: [NSAttributedString] = []
        for i in 0..<Int(glaspen2_width_preset_count()) {
            widthTitles.append(MenuRuntime.widthItemTitle(Int32(i)))
        }
        let topTitles = [
            pair("保存(含背景)", "Save (with bg)"),
            pair("保存(涂鸦)", "Save (drawing)"),
            pair("新建画布", "New canvas"),
            pair("导出", "Export"),
            "",
            "",
            pair("设置...", "Settings..."),
            pair("更多", "More"),
            "",
            pair("English", "中文"),
            pair("退出", "Quit"),
        ]
        let exportTitles = [
            pair("保存笔记 (Xournal)", "Save Notes (Xournal)"),
            pair("导出无限画布 PDF (分页)", "Export infinite canvas PDF (paged)"),
            pair("导出无限画布 SVG (整幅)", "Export infinite canvas SVG (whole)"),
        ]
        let moreTitles = [
            pair("开机自启", "Launch at login"),
            pair("磨砂玻璃", "Frosted Glass"),
            pair("笔迹描边", "Stroke outline"),
            MenuRuntime.l10n("canvas.mode_infinite") ?? "",
        ]
        let modeTitle = glaspen2_router_menu_state(778) != 0
            ? (MenuRuntime.l10n("canvas.fixed_mode") ?? "")
            : (MenuRuntime.l10n("canvas.ethereal_mode") ?? "")
        let toggleTitle = glaspen2_router_enabled() != 0
            ? pair("关闭涂鸦", "Disable Drawing")
            : pair("开启涂鸦", "Enable Drawing")
        MenuUpdater.updateTexts(menu: menu, colorTitles: colorTitles,
                                widthTitles: widthTitles, topTitles: topTitles,
                                exportTitles: exportTitles, moreTitles: moreTitles,
                                modeTitle: modeTitle, toggleTitle: toggleTitle)
    }
}
