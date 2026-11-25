// S7: 笔输入状态(原始笔迹/描边/橡皮/接近) — C 符号导出, 供 Swift/ObjC 调用。
import AppKit
import Darwin

@objc public final class PenState: NSObject {
    static var rawLastX = 0.0
    static var rawLastY = 0.0
    static var rawLastW = 0.0
    static var rawHasLast = false
    static var rawHasPrev = false
    static var rawPrevX = 0.0
    static var rawPrevY = 0.0
    static var rawPathLen = 0.0
    static var strokeActive = false
    static var eraserMode = false
    static var tabletProximity = false
    static var proxDevice: Int = 0

    static func clearRaw() {
        rawHasLast = false
        rawHasPrev = false
        rawPathLen = 0
    }

    static func updateAfterSegment(_ x: Double, _ y: Double, _ width: Double) {
        if rawHasLast {
            let dx = x - rawLastX
            let dy = y - rawLastY
            rawPathLen += (dx * dx + dy * dy).squareRoot()
        }
        rawPrevX = rawLastX
        rawPrevY = rawLastY
        rawHasPrev = true
        rawLastX = x
        rawLastY = y
        rawLastW = width
        rawHasLast = true
    }
}

@_cdecl("sw_stroke_active")
public func swStrokeActive() -> Int32 { PenState.strokeActive ? 1 : 0 }

@_cdecl("sw_tablet_proximity")
public func swTabletProximity() -> Int32 { PenState.tabletProximity ? 1 : 0 }

@_cdecl("glaspen2_router_reset_raw_stroke")
public func bridgeResetRawStroke() { PenState.clearRaw() }

@_cdecl("sw_eraser_mode")
public func swEraserMode() -> Int32 { PenState.eraserMode ? 1 : 0 }

@_cdecl("glaspen2_router_stroke_active")
public func bridgeStrokeActive() -> Int32 { PenState.strokeActive ? 1 : 0 }

@_cdecl("glaspen2_router_set_stroke_active")
public func bridgeSetStrokeActive(_ on: Int32) { PenState.strokeActive = on != 0 }

@_cdecl("glaspen2_router_eraser_mode")
public func bridgeEraserMode() -> Int32 { PenState.eraserMode ? 1 : 0 }

@_cdecl("glaspen2_router_set_eraser_mode")
public func bridgeSetEraserMode(_ on: Int32) { PenState.eraserMode = on != 0 }

@_cdecl("glaspen2_router_stroke_begin")
public func bridgeStrokeBegin() { StrokeSurface.begin() }

@_cdecl("glaspen2_router_stroke_end")
public func bridgeStrokeEnd() { StrokeSurface.end() }

@_cdecl("glaspen2_router_raw_draw_dot")
public func bridgeRawDrawDot(_ x: Double, _ y: Double, _ w: Double) {
    RawDrawing.drawDot(x: x, y: y, width: w)
}

@_cdecl("glaspen2_router_raw_draw_segment")
public func bridgeRawDrawSegment(_ x: Double, _ y: Double, _ w: Double) {
    RawDrawing.drawSegment(x: x, y: y, width: w)
}

@_cdecl("glaspen2_router_modeler_begin")
public func bridgeModelerBegin(_ x: Double, _ y: Double, _ pressure: Double,
                               _ timestamp: Double) {
    glaspen2_modeler_begin(glaspen2_router_pen_r(), glaspen2_router_pen_g(),
                           glaspen2_router_pen_b(), canvasInputX(x),
                           canvasInputY(y), pressure, timestamp,
                           glaspen2_router_width_scale())
}

@_cdecl("glaspen2_router_modeler_move")
public func bridgeModelerMove(_ x: Double, _ y: Double, _ pressure: Double,
                              _ timestamp: Double) {
    glaspen2_modeler_move(canvasInputX(x), canvasInputY(y), pressure, timestamp,
                          glaspen2_router_width_scale())
}

@_cdecl("glaspen2_router_modeler_end")
public func bridgeModelerEnd(_ x: Double, _ y: Double, _ pressure: Double,
                             _ timestamp: Double) {
    glaspen2_modeler_end(canvasInputX(x), canvasInputY(y), pressure, timestamp,
                         glaspen2_router_width_scale())
}

@_cdecl("glaspen2_router_modeler_erase_finish")
public func bridgeEraseFinish() { glaspen2_modeler_erase_finish() }

@_cdecl("glaspen2_router_commit_to_strokes")
public func bridgeCommitToStrokes() {
    glaspen2_modeler_commit_to_strokes(glaspen2_router_pen_r(),
                                       glaspen2_router_pen_g(),
                                       glaspen2_router_pen_b())
}

@_cdecl("glaspen2_router_pen_down_state")
public func bridgePenDownState(_ x: Double, _ y: Double, _ rawWidth: Double) {
    PenState.strokeActive = true
    PenState.rawLastX = x
    PenState.rawLastY = y
    PenState.rawLastW = rawWidth
    PenState.rawHasLast = true
    PenState.rawPathLen = 0
    CursorState.visible = true
    CursorState.x = x
    CursorState.y = y
}

@_cdecl("glaspen2_router_pen_up_state")
public func bridgePenUpState() {
    PenState.clearRaw()
    CursorState.visible = true
    glaspen2_router_dirty_include_surface_point(CursorState.x, CursorState.y, 14.0)
    OverlayLayerFlush.flushDirty()
}

@_cdecl("glaspen2_router_clear_raw_state")
public func bridgeClearRawState() { PenState.clearRaw() }

@_cdecl("glaspen2_router_mark_tablet_proximity")
public func bridgeMarkTabletProximity() { PenState.tabletProximity = true }

@_cdecl("glaspen2_router_set_prox_device")
public func bridgeSetProxDevice(_ device: Int32) { PenState.proxDevice = Int(device) }

@_cdecl("glaspen2_router_prox_is_eraser")
public func bridgeProxIsEraser() -> Int32 {
    // NSPointingDeviceType: 0=unknown, 1=pen, 2=cursor, 3=eraser(旧 NSEraserPointingDevice)。
    PenState.proxDevice == 3 ? 1 : 0
}

@_cdecl("glaspen2_router_last_raw_x")
public func bridgeLastRawX() -> Double { PenState.rawLastX }

@_cdecl("glaspen2_router_last_raw_y")
public func bridgeLastRawY() -> Double { PenState.rawLastY }
