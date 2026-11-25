// S7: 绘制/网格可见性状态 — C 符号导出(与既有桥同名)。
import AppKit

@objc public final class ViewSettings: NSObject {
    static var infiniteCanvas = false
    static var strokesVisible = true
    static var minimapEnabled = false
    static var showGrid = false
    static var gridSize = 40.0
    static var gridDivider: Int = 0
    static var gridFollowStrokes = false
}

@_cdecl("sw_set_infinite")
public func viewSetInfinite(_ on: Int32) { ViewSettings.infiniteCanvas = on != 0 }

@_cdecl("sw_set_minimap")
public func viewSetMinimap(_ on: Int32) { ViewSettings.minimapEnabled = on != 0 }

@_cdecl("glaspen2_router_infinite")
public func viewInfinite() -> Int32 { ViewSettings.infiniteCanvas ? 1 : 0 }

@_cdecl("glaspen2_router_set_infinite_state")
public func viewSetInfiniteState(_ on: Int32) { ViewSettings.infiniteCanvas = on != 0 }

@_cdecl("glaspen2_router_strokes_visible")
public func viewStrokesVisible() -> Int32 { ViewSettings.strokesVisible ? 1 : 0 }

@_cdecl("glaspen2_router_set_strokes_visible")
public func viewSetStrokesVisible(_ on: Int32) { ViewSettings.strokesVisible = on != 0 }

@_cdecl("glaspen2_router_strokes_hidden")
public func viewStrokesHidden() -> Int32 { ViewSettings.strokesVisible ? 0 : 1 }

@_cdecl("glaspen2_router_minimap_enabled")
public func viewMinimapEnabled() -> Int32 { ViewSettings.minimapEnabled ? 1 : 0 }

@_cdecl("glaspen2_router_set_minimap")
public func viewSetMinimapBridge(_ on: Int32) { ViewSettings.minimapEnabled = on != 0 }

@_cdecl("glaspen2_router_show_grid")
public func viewShowGrid() -> Int32 { ViewSettings.showGrid ? 1 : 0 }

@_cdecl("glaspen2_router_set_grid_state")
public func viewSetGridState(_ on: Int32) { ViewSettings.showGrid = on != 0 }

@_cdecl("glaspen2_router_set_grid_follow")
public func viewSetGridFollow(_ on: Int32) { ViewSettings.gridFollowStrokes = on != 0 }

@_cdecl("glaspen2_router_set_grid_size")
public func viewSetGridSize(_ size: Double) { ViewSettings.gridSize = size }

@_cdecl("glaspen2_router_set_grid_divider")
public func viewSetGridDivider(_ divider: Int32) { ViewSettings.gridDivider = Int(divider) }

@_cdecl("glaspen2_router_startup_grid")
public func viewStartupGrid(_ show: Int32, _ size: Double, _ follow: Int32, _ divider: Int32) {
    ViewSettings.showGrid = show != 0
    if size >= 10 && size <= 200 { ViewSettings.gridSize = size }
    ViewSettings.gridFollowStrokes = follow != 0
    if divider >= 0 && divider <= 4 { ViewSettings.gridDivider = Int(divider) }
}
