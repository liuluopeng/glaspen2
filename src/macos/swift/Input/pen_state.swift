// S7: 笔输入状态(原始笔迹/描边/橡皮/接近) — C 符号导出, 供 Swift/ObjC 调用。
import AppKit
import Darwin

@objc public final class PenState: NSObject {
    static var rawLastX: Double {
        get { glaspen2_rust_pen_raw_last_x() }
        set { glaspen2_rust_pen_set_raw_last_x(newValue) }
    }
    static var rawLastY: Double {
        get { glaspen2_rust_pen_raw_last_y() }
        set { glaspen2_rust_pen_set_raw_last_y(newValue) }
    }
    static var rawLastW: Double {
        get { glaspen2_rust_pen_raw_last_w() }
        set { glaspen2_rust_pen_set_raw_last_w(newValue) }
    }
    static var rawHasLast: Bool {
        get { glaspen2_rust_pen_raw_has_last() != 0 }
        set { glaspen2_rust_pen_set_raw_has_last(newValue ? 1 : 0) }
    }
    static var rawHasPrev: Bool {
        get { glaspen2_rust_pen_raw_has_prev() != 0 }
        set { glaspen2_rust_pen_set_raw_has_prev(newValue ? 1 : 0) }
    }
    static var rawPrevX: Double {
        get { glaspen2_rust_pen_raw_prev_x() }
        set { glaspen2_rust_pen_set_raw_prev_x(newValue) }
    }
    static var rawPrevY: Double {
        get { glaspen2_rust_pen_raw_prev_y() }
        set { glaspen2_rust_pen_set_raw_prev_y(newValue) }
    }
    static var rawPathLen: Double {
        get { glaspen2_rust_pen_raw_path_len() }
        set { glaspen2_rust_pen_set_raw_path_len(newValue) }
    }
    static var strokeActive = false
    static var eraserMode = false
    /// 当前笔是否在感应区(proximity enter 后 true, exit 后 false)。
    static var tabletProximity = false
    /// 该平板是否曾发过 proximity 事件(旧 g_has_tablet_proximity; 永不清零)。
    static var hasSeenProximity = false
    static var proxDevice: Int = 0

    static func clearRaw() { glaspen2_rust_pen_clear_raw() }

    static func updateAfterSegment(_ x: Double, _ y: Double, _ width: Double) {
        glaspen2_rust_pen_update_after_segment(x, y, width)
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

/// R4: view→canvas 变换统一走 Rust; 一次调用同时得到 x/y。
private func canvasInput(_ x: Double, _ y: Double) -> (Double, Double) {
    var cx = 0.0, cy = 0.0
    _ = glaspen2_canvas_input(
        x, y, glaspen2_router_infinite(), glaspen2_router_zoom(),
        glaspen2_router_pan_x(), glaspen2_router_pan_y(),
        glaspen2_router_page_off_x(), glaspen2_router_page_off_y(),
        PageViewOps.isFit ? 1 : 0, PageViewOps.paperOx, PageViewOps.paperOy,
        PageViewOps.paperScale, &cx, &cy)
    return (cx, cy)
}

@_cdecl("glaspen2_router_modeler_begin")
public func bridgeModelerBegin(_ x: Double, _ y: Double, _ pressure: Double,
                               _ timestamp: Double) {
    let (cx, cy) = canvasInput(x, y)
    glaspen2_modeler_begin(glaspen2_router_pen_r(), glaspen2_router_pen_g(),
                           glaspen2_router_pen_b(), cx, cy, pressure, timestamp,
                           glaspen2_router_width_scale())
}

@_cdecl("glaspen2_router_modeler_move")
public func bridgeModelerMove(_ x: Double, _ y: Double, _ pressure: Double,
                              _ timestamp: Double) {
    let (cx, cy) = canvasInput(x, y)
    glaspen2_modeler_move(cx, cy, pressure, timestamp,
                          glaspen2_router_width_scale())
}

@_cdecl("glaspen2_router_modeler_end")
public func bridgeModelerEnd(_ x: Double, _ y: Double, _ pressure: Double,
                             _ timestamp: Double) {
    let (cx, cy) = canvasInput(x, y)
    glaspen2_modeler_end(cx, cy, pressure, timestamp,
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
    sw_cursor_set(x, y, 1)
}

@_cdecl("glaspen2_router_pen_up_state")
public func bridgePenUpState() {
    PenState.clearRaw()
    sw_cursor_show()
    glaspen2_rust_dirty_include_point(glaspen2_router_cursor_x(), glaspen2_router_cursor_y(), 14.0)
    OverlayLayerFlush.flushDirty()
}

@_cdecl("glaspen2_router_clear_raw_state")
public func bridgeClearRawState() { PenState.clearRaw() }

@_cdecl("glaspen2_router_mark_tablet_proximity")
public func bridgeMarkTabletProximity() {
    PenState.tabletProximity = true
    PenState.hasSeenProximity = true
}

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
