// S7: 笔外观设置(颜色/粗细/选中/历史/暂存) — C 符号导出。
import AppKit

@objc public final class PenSettingsStore: NSObject {
    static var penR = 1.0
    static var penG = 0.0
    static var penB = 0.0
    static var widthScale = 1.0
    static var selectedColor = 0
    static var selectedWidth = 3
    static var prevColor = -1
    static var prevWidth = -1
    static var pendingColor = -1
    static var pendingWidth = -1
}

@_cdecl("glaspen2_router_pen_r")
public func penRed() -> Double { PenSettingsStore.penR }
@_cdecl("glaspen2_router_pen_g")
public func penGreen() -> Double { PenSettingsStore.penG }
@_cdecl("glaspen2_router_pen_b")
public func penBlue() -> Double { PenSettingsStore.penB }
@_cdecl("glaspen2_router_width_scale")
public func penWidthScale() -> Double { PenSettingsStore.widthScale }

@_cdecl("glaspen2_router_selected_color")
public func penSelectedColor() -> Int32 { Int32(PenSettingsStore.selectedColor) }
@_cdecl("glaspen2_router_selected_color_index")
public func penSelectedColorIndex() -> Int32 { Int32(PenSettingsStore.selectedColor) }
@_cdecl("glaspen2_router_selected_width")
public func penSelectedWidth() -> Int32 { Int32(PenSettingsStore.selectedWidth) }
@_cdecl("glaspen2_router_selected_width_index")
public func penSelectedWidthIndex() -> Int32 { Int32(PenSettingsStore.selectedWidth) }

@_cdecl("glaspen2_router_prev_color_index")
public func penPrevColor() -> Int32 { Int32(PenSettingsStore.prevColor) }
@_cdecl("glaspen2_router_prev_width_index")
public func penPrevWidth() -> Int32 { Int32(PenSettingsStore.prevWidth) }
@_cdecl("glaspen2_router_set_prev_color")
public func penSetPrevColor(_ idx: Int32) { PenSettingsStore.prevColor = Int(idx) }
@_cdecl("glaspen2_router_set_prev_width")
public func penSetPrevWidth(_ idx: Int32) { PenSettingsStore.prevWidth = Int(idx) }

@_cdecl("glaspen2_router_pending_color")
public func penPendingColor() -> Int32 { Int32(PenSettingsStore.pendingColor) }
@_cdecl("glaspen2_router_pending_width")
public func penPendingWidth() -> Int32 { Int32(PenSettingsStore.pendingWidth) }
@_cdecl("glaspen2_router_set_pending_color")
public func penSetPendingColor(_ idx: Int32) { PenSettingsStore.pendingColor = Int(idx) }
@_cdecl("glaspen2_router_set_pending_width")
public func penSetPendingWidth(_ idx: Int32) { PenSettingsStore.pendingWidth = Int(idx) }
@_cdecl("glaspen2_router_clear_pending_color")
public func penClearPendingColor() { PenSettingsStore.pendingColor = -1 }
@_cdecl("glaspen2_router_clear_pending_width")
public func penClearPendingWidth() { PenSettingsStore.pendingWidth = -1 }

@_cdecl("glaspen2_router_apply_color")
public func penApplyColor(_ idx: Int32) {
    var r = 0.0, g = 0.0, b = 0.0
    glaspen2_router_color_preset_rgb_at(idx, &r, &g, &b)
    PenSettingsStore.penR = r
    PenSettingsStore.penG = g
    PenSettingsStore.penB = b
    PenSettingsStore.selectedColor = Int(idx)
}

@_cdecl("glaspen2_router_apply_width")
public func penApplyWidth(_ idx: Int32) {
    PenSettingsStore.widthScale = glaspen2_router_width_preset_scale_at(idx)
    PenSettingsStore.selectedWidth = Int(idx)
}

@_cdecl("sw_set_pen_settings")
public func penSetSettings(_ r: Double, _ g: Double, _ b: Double, _ width: Double,
                           _ colorIdx: Int32, _ widthIdx: Int32) {
    PenSettingsStore.penR = r
    PenSettingsStore.penG = g
    PenSettingsStore.penB = b
    PenSettingsStore.widthScale = width
    PenSettingsStore.selectedColor = Int(colorIdx)
    PenSettingsStore.selectedWidth = Int(widthIdx)
}

@_cdecl("glaspen2_router_save_pen_settings")
public func penSaveSettings() {
    glaspen2_save_settings(PenSettingsStore.penR, PenSettingsStore.penG,
                           PenSettingsStore.penB, PenSettingsStore.widthScale)
}
