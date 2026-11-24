// S7: 光标/画笔光标状态(位置/可见/系统光标), 对外导出 C 符号。
import AppKit

@objc public final class CursorState: NSObject {
    static var x = -100.0
    static var y = -100.0
    static var visible = false
    static var blankCursor: NSCursor?
    static var arrowCursor: NSCursor?
    static var penDrawing = false
}

@_cdecl("glaspen2_router_cursor_x")
public func cursorX() -> Double { CursorState.x }

@_cdecl("glaspen2_router_cursor_y")
public func cursorY() -> Double { CursorState.y }

@_cdecl("glaspen2_router_cursor_hide")
public func cursorHide() { CursorState.visible = false }

@_cdecl("glaspen2_router_pen_cursor_visible")
public func cursorVisible() -> Int32 { CursorState.visible ? 1 : 0 }

@_cdecl("sw_cursor_set")
public func cursorSet(_ x: Double, _ y: Double, _ visible: Int32) {
    CursorState.x = x
    CursorState.y = y
    if visible != 0 { CursorState.visible = true }
}

@_cdecl("sw_cursor_visible")
public func cursorVisibleValue() -> Int32 { CursorState.visible ? 1 : 0 }

@_cdecl("sw_cursor_x")
public func cursorPositionX() -> Double { CursorState.x }

@_cdecl("sw_cursor_y")
public func cursorPositionY() -> Double { CursorState.y }

@_cdecl("sw_cursor_show")
public func cursorShow() { CursorState.visible = true }

@_cdecl("glaspen2_router_pen_drawing")
public func penDrawing() -> Int32 { CursorState.penDrawing ? 1 : 0 }

@_cdecl("glaspen2_router_set_pen_drawing")
public func setPenDrawing(_ on: Int32) { CursorState.penDrawing = on != 0 }
