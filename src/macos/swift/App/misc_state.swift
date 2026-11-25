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

@objc public final class StrokeContext: NSObject {
    static var active: OpaquePointer?
}

@_cdecl("glaspen2_router_acquire_cr")
public func strokeAcquire(_ owned: UnsafeMutablePointer<Int32>?) -> OpaquePointer? {
    if let active = StrokeContext.active {
        owned?.pointee = 0
        return active
    }
    owned?.pointee = 1
    return cairoCreateScaled()
}

@_cdecl("glaspen2_router_outline_pad")
public func strokeOutlinePad() -> Double { 1.0 }

@_cdecl("glaspen2_router_active_ctx_present")
public func strokeActivePresent() -> Int32 { StrokeContext.active != nil ? 1 : 0 }

@_cdecl("glaspen2_router_install_active_ctx")
public func strokeInstallActive(_ ctx: OpaquePointer?) { StrokeContext.active = ctx }

@_cdecl("glaspen2_router_take_active_ctx")
public func strokeTakeActive() -> OpaquePointer? {
    let ctx = StrokeContext.active
    StrokeContext.active = nil
    return ctx
}

@_cdecl("glaspen2_router_cursor_invalidate")
public func cursorInvalidate() {
    // 光标坐标是视图坐标(原点左下), 不是表面坐标(y 向下), 不能用 surface 转换。
    dirtyIncludePoint(CursorState.x, CursorState.y, 14.0)
}

@_cdecl("glaspen2_router_cursor_flush_due")
public func cursorFlushDue() -> Int32 {
    if !PenState.strokeActive,
       glaspen2_perf_elapsed_us(CursorState.lastFlush) >= 16000 {
        CursorState.lastFlush = mach_absolute_time()
        return 1
    }
    return 0
}
