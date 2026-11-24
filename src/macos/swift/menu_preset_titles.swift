// S7: 颜色/粗细菜单项的富文本(内嵌图标 + 名称)。
import AppKit

@objc public final class MenuPresetTitles: NSObject {
    @objc(colorItemTextWithIndex:)
    public static func colorItemText(index: Int32) -> NSAttributedString {
        let (r, g, b) = PresetStore.rgb(Int(index))
        let color = NSColor(calibratedRed: r, green: g, blue: b, alpha: 1.0)
        let icon = MenuIconFactory.colorDot(color)
        let name = MenuNames.colorDisplayName(index: Int(index))
        return MenuIconFactory.menuText(icon: icon, name: name)
    }

    @objc(widthItemTextWithIndex:)
    public static func widthItemText(index: Int32) -> NSAttributedString {
        let scale = PresetStore.width(Int(index))
        let icon = MenuIconFactory.widthLine(scale)
        let name = MenuNames.widthDisplayName(index: Int(index))
        return MenuIconFactory.menuText(icon: icon, name: name)
    }
}
