// S7: 渲染设置(玻璃/描边/软阴影/反色/翻转) — C 符号导出。
import AppKit

@objc public final class RenderSettings: NSObject {
    static var glassEnabled = false
    static var glassOpacity = 0.45
    static var glassFollow = true
    static var outlineEnabled = false
    static var softShadow = false
    static var invertInk = false
    static var invertFps: Int32 = 30
    static var flipEffect: Int32 = 0
}

@_cdecl("glaspen2_router_glass_enabled")
public func renderGlassEnabled() -> Int32 { RenderSettings.glassEnabled ? 1 : 0 }

@_cdecl("glaspen2_router_set_glass_state")
public func renderSetGlassState(_ on: Int32) { RenderSettings.glassEnabled = on != 0 }

@_cdecl("glaspen2_router_glass_opacity")
public func renderGlassOpacity() -> Double { RenderSettings.glassOpacity }

@_cdecl("glaspen2_router_glass_follow")
public func renderGlassFollow() -> Int32 { RenderSettings.glassFollow ? 1 : 0 }

@_cdecl("glaspen2_router_set_glass_follow")
public func renderSetGlassFollow(_ on: Int32) { RenderSettings.glassFollow = on != 0 }

@_cdecl("glaspen2_router_startup_glass")
public func renderStartupGlass(_ opacity: Double, _ enabled: Int32) {
    // 与迁移前 ObjC 一致: 库里没存 glass_alpha(0) 时保留默认 0.45。
    // 之前无条件覆盖成 0, 玻璃虽 enabled 但 alpha=0, 表现为快捷键完全没反应。
    if opacity > 0 { RenderSettings.glassOpacity = opacity }
    RenderSettings.glassEnabled = enabled != 0
}

@_cdecl("glaspen2_router_startup_set_glass_follow")
public func renderStartupGlassFollow(_ on: Int32) { RenderSettings.glassFollow = on != 0 }

@_cdecl("glaspen2_router_soft_shadow")
public func renderSoftShadow() -> Int32 { RenderSettings.softShadow ? 1 : 0 }

@_cdecl("glaspen2_router_startup_shadow")
public func renderStartupShadow(_ on: Int32) { RenderSettings.softShadow = on != 0 }

@_cdecl("glaspen2_router_set_soft_shadow_flag")
public func renderSetSoftShadowFlag(_ on: Int32) { RenderSettings.softShadow = on != 0 }

@_cdecl("glaspen2_router_outline_enabled")
public func renderOutlineEnabled() -> Int32 { RenderSettings.outlineEnabled ? 1 : 0 }

@_cdecl("glaspen2_router_set_outline_state")
public func renderSetOutlineState(_ on: Int32) { RenderSettings.outlineEnabled = on != 0 }

@_cdecl("glaspen2_router_startup_outline")
public func renderStartupOutline(_ on: Int32) { RenderSettings.outlineEnabled = on != 0 }

@_cdecl("glaspen2_router_flip_effect")
public func renderFlipEffect() -> Int32 { RenderSettings.flipEffect }

@_cdecl("glaspen2_router_startup_flip")
public func renderStartupFlip(_ effect: Int32) { RenderSettings.flipEffect = effect }

@_cdecl("glaspen2_router_set_flip")
public func renderSetFlip(_ effect: Int32) { RenderSettings.flipEffect = effect }

@_cdecl("glaspen2_router_invert_enabled")
public func renderInvertEnabled() -> Int32 { RenderSettings.invertInk ? 1 : 0 }

@_cdecl("glaspen2_router_invert_set_ink")
public func renderSetInvertInk(_ on: Int32) { RenderSettings.invertInk = on != 0 }

@_cdecl("glaspen2_router_startup_invert")
public func renderStartupInvert(_ fps: Int32, _ ink: Int32) {
    if fps == 10 || fps == 30 || fps == 60 || fps == 100 { RenderSettings.invertFps = fps }
    RenderSettings.invertInk = ink != 0
}

@_cdecl("glaspen2_router_invert_fps")
public func renderInvertFps() -> Int32 { RenderSettings.invertFps }

@_cdecl("glaspen2_router_invert_set_fps")
public func renderSetInvertFps(_ fps: Int32) { RenderSettings.invertFps = fps }
