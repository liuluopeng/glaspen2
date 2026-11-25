// S7: 颜色/粗细菜单项的富文本(内嵌图标 + 名称)。
import AppKit

@objc public final class MenuPresetTitles: NSObject {
    @objc(colorItemTextWithIndex:)
    public static func colorItemText(index: Int32) -> NSAttributedString {
        var r = 0.0, g = 0.0, b = 0.0
        glaspen2_color_preset_rgb(index, &r, &g, &b)
        let color = NSColor(calibratedRed: r, green: g, blue: b, alpha: 1.0)
        let icon = MenuIconFactory.colorDot(color)
        let name = MenuNames.colorDisplayName(index: Int(index))
        return MenuIconFactory.menuText(icon: icon, name: name)
    }

    @objc(widthItemTextWithIndex:)
    public static func widthItemText(index: Int32) -> NSAttributedString {
        let scale = glaspen2_width_preset_value(index)
        let icon = MenuIconFactory.widthLine(scale)
        let name = MenuNames.widthDisplayName(index: Int(index))
        return MenuIconFactory.menuText(icon: icon, name: name)
    }
}
