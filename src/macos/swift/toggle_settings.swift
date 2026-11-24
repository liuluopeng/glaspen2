// S7: 功能开关与 GIF 设置 — C 符号导出。
import AppKit

@objc public final class ToggleSettings: NSObject {
    static var enabled = true
    static var ethereal = false
    static var pressureMonitor = false
    static var chatIntegration = false
    static var gifFps: Int32 = 15
    static var gifResolution = 0.5
    static var gifSpeed = 2.0
    static var gifEndMode: Int32 = 1
}

@_cdecl("glaspen2_router_enabled")
public func toggleEnabled() -> Int32 { ToggleSettings.enabled ? 1 : 0 }
@_cdecl("glaspen2_router_set_enabled_state")
public func toggleSetEnabled(_ on: Int32) { ToggleSettings.enabled = on != 0 }

@_cdecl("glaspen2_router_ethereal")
public func toggleEthereal() -> Int32 { ToggleSettings.ethereal ? 1 : 0 }
@_cdecl("glaspen2_router_set_ethereal")
public func toggleSetEthereal(_ on: Int32) { ToggleSettings.ethereal = on != 0 }

@_cdecl("glaspen2_router_pressure_monitor")
public func togglePressureMonitor() -> Int32 { ToggleSettings.pressureMonitor ? 1 : 0 }
@_cdecl("glaspen2_router_pm_enabled")
public func togglePmEnabled() -> Int32 { ToggleSettings.pressureMonitor ? 1 : 0 }
@_cdecl("glaspen2_router_set_pressure_monitor")
public func toggleSetPressureMonitor(_ on: Int32) { ToggleSettings.pressureMonitor = on != 0 }

@_cdecl("glaspen2_router_chat_integration")
public func toggleChatIntegration() -> Int32 { ToggleSettings.chatIntegration ? 1 : 0 }
@_cdecl("glaspen2_router_set_chat_integration")
public func toggleSetChatIntegration(_ on: Int32) { ToggleSettings.chatIntegration = on != 0 }
@_cdecl("glaspen2_router_restore_chat_integration")
public func toggleRestoreChatIntegration(_ on: Int32) { ToggleSettings.chatIntegration = on != 0 }

@_cdecl("glaspen2_router_gif_fps")
public func toggleGifFps() -> Int32 { ToggleSettings.gifFps }
@_cdecl("glaspen2_router_gif_resolution")
public func toggleGifResolution() -> Double { ToggleSettings.gifResolution }
@_cdecl("glaspen2_router_gif_speed")
public func toggleGifSpeed() -> Double { ToggleSettings.gifSpeed }
@_cdecl("glaspen2_router_gif_end_mode")
public func toggleGifEndMode() -> Int32 { ToggleSettings.gifEndMode }
@_cdecl("glaspen2_router_set_gif")
public func toggleSetGif(_ fps: Int32, _ res: Double, _ speed: Double, _ endMode: Int32) {
    ToggleSettings.gifFps = fps
    ToggleSettings.gifResolution = res
    ToggleSettings.gifSpeed = speed
    ToggleSettings.gifEndMode = endMode
}
@_cdecl("glaspen2_router_restore_gif_settings")
public func toggleRestoreGif(_ fps: Int32, _ res: Double, _ speed: Double, _ endMode: Int32) {
    toggleSetGif(fps, res, speed, endMode)
}
