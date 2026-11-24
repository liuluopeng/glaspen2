// S7: 屏幕尺寸与语言状态 — C 符号导出。
import AppKit

@objc public final class AppState: NSObject {
    static var screenW: Int32 = 1920
    static var screenH: Int32 = 1080
    static var lang: Int32 = 0
}

@_cdecl("glaspen2_router_screen_w")
public func appScreenW() -> Double { Double(AppState.screenW) }
@_cdecl("glaspen2_router_screen_h")
public func appScreenH() -> Double { Double(AppState.screenH) }
@_cdecl("glaspen2_router_set_screen_size")
public func appSetScreenSize(_ w: Int32, _ h: Int32) {
    AppState.screenW = w
    AppState.screenH = h
}
@_cdecl("glaspen2_router_lang")
public func appLang() -> Int32 { AppState.lang }
@_cdecl("glaspen2_router_set_lang")
public func appSetLang(_ value: Int32) { AppState.lang = value == 1 ? 1 : 0 }
@_cdecl("glaspen2_router_startup_set_lang")
public func appStartupSetLang(_ lang: Int32) { AppState.lang = lang }
@_cdecl("sw_app_screen_w")
public func appScreenWInt() -> Int32 { AppState.screenW }
@_cdecl("sw_app_screen_h")
public func appScreenHInt() -> Int32 { AppState.screenH }
