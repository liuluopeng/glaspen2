// S7: i18n 文本 / 描边活动上下文 / 光标节流等剩余状态与桥。
import AppKit
import Darwin

@objc public final class I18nText: NSObject {
    static func text(_ key: String) -> String {
        guard let c = glaspen2_i18n(key, glaspen2_router_lang() == 1 ? "en" : "zh") else {
            return key
        }
        let s = String(cString: c)
        glaspen2_free_c_string(c)
        return s.isEmpty ? key : s
    }

    static func textf(_ key: String, _ args: [String: String]) -> String {
        var result = text(key)
        for (name, value) in args {
            result = result.replacingOccurrences(of: "{\(name)}", with: value)
        }
        return result
    }
}

@_cdecl("sw_text")
public func i18nText(_ key: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>? {
    guard let key else { return nil }
    let text = I18nText.text(String(cString: key))
    return strdup(text)
}

@_cdecl("glaspen2_router_outline_pad")
public func strokeOutlinePad() -> Double { 1.0 }

@_cdecl("glaspen2_router_cursor_invalidate")
public func cursorInvalidate() {
    // 光标坐标是视图坐标(原点左下), 不是表面坐标(y 向下), 不能用 surface 转换。
    glaspen2_rust_dirty_include_point(glaspen2_router_cursor_x(), glaspen2_router_cursor_y(), 14.0)
}

