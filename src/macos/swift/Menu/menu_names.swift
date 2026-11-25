// R2: 颜色/粗细显示名表已迁 glaspen-core::i18n, 本文件只做 C 字符串桥。
import AppKit

@objc public final class MenuNames: NSObject {
    private static func take(_ p: UnsafeMutablePointer<CChar>?) -> String {
        guard let p else { return "" }
        defer { glaspen2_free_c_string(p) }
        return String(cString: p)
    }

    @objc(colorDisplayNameWithIndex:)
    public static func colorDisplayName(index: Int) -> String {
        take(glaspen2_color_name(Int32(index), glaspen2_router_lang()))
    }

    @objc(widthDisplayNameWithIndex:)
    public static func widthDisplayName(index: Int) -> String {
        take(glaspen2_width_name(Int32(index), glaspen2_router_lang()))
    }
}
