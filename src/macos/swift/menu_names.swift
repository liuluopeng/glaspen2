// S3b-2b-4b: 颜色/粗细显示名(仅稳定 i18n key 表),文案仍由 ObjC gl_text/核心 i18n 决定。
import AppKit

@objc public final class MenuNames: NSObject {
    private static let colorKeys = [
        "color.red", "color.orange", "color.yellow", "color.green", "color.cyan",
        "color.blue", "color.purple", "color.pink", "color.white", "color.black",
    ]
    private static let widthKeys = [
        "width.hair", "width.very_fine", "width.fine", "width.medium",
        "width.thick", "width.very_thick", "width.extra_thick", "width.boldest",
    ]

    @objc(colorDisplayNameWithIndex:)
    public static func colorDisplayName(index: Int) -> String {
        guard index >= 0 && index < colorKeys.count else { return "" }
        return glaspen2_menu_l10n(colorKeys[index])
    }

    @objc(widthDisplayNameWithIndex:)
    public static func widthDisplayName(index: Int) -> String {
        guard index >= 0 && index < widthKeys.count else { return "" }
        return glaspen2_menu_l10n(widthKeys[index])
    }
}
