// S6: 事件路由。滚轮缩放(3-1)、非笔光标隐藏(3-2)、笔光标更新(3-3b)。
import AppKit

@objc public final class EventRouter: NSObject {
    /// 0 = fallback(ObjC), 1 = pass(return event), 2 = consume(return NULL)
    @objc(routeEventOfType:event:)
    public static func route(type: CGEventType, event: CGEvent) -> Int32 {
        // 停用涂鸦时: 非键盘/非滚轮事件先清理相机/光标再放行(与原 ObjC 同序)。
        if (state() & 1) == 0 && type != .keyDown && type != .keyUp
            && type != .scrollWheel {
            glaspen2_router_disable_cleanup()
            return 1
        }
        // 点菜单栏(非笔): 临时停 tap, 让菜单能收到点击; 5s 兜底恢复。
        if type == .leftMouseDown, let ev = NSEvent(cgEvent: event), !isPen(ev) {
            if event.location.y < 30 {
                glaspen2_router_tap_menu_bar_interaction()
                return 1
            }
        }
        if type == .scrollWheel && (state() & 4) != 0 {
            return routeScrollZoom(event)
        }
        if type == .mouseMoved, let ev = NSEvent(cgEvent: event), !isPen(ev) {
            let flags = glaspen2_router_cursor_flags()
            if (flags & 1) != 0 && (flags & 2) == 0 {
                glaspen2_router_cursor_invalidate()
                glaspen2_router_cursor_hide()
                if (flags & 4) != 0 { glaspen2_router_auto_hide() }
                if (flags & 8) == 0 { glaspen2_router_bubble_pen_gone() }
                glaspen2_router_flush()
            }
        }
        if type != .tabletProximity, isPenEvent(event) {
            penCursorUpdate(type: type, event: event)
            penSystemCursor(type: type)
        }
        return 0
    }

    private static func isPen(_ ev: NSEvent) -> Bool {
        let dev = ev.pointingDeviceType
        if dev == .pen || dev == .eraser { return true }
        return ev.subtype == .tabletPoint || ev.subtype == .tabletProximity
    }

    /// 笔按下/拖动期间隐藏系统光标; 任意 up 恢复光标并清十字/刷新(S6-3-3c)。
    private static func penSystemCursor(type: CGEventType) {
        let downOrDrag = type == .leftMouseDown || type == .rightMouseDown ||
            type == .otherMouseDown || type == .leftMouseDragged ||
            type == .rightMouseDragged || type == .otherMouseDragged
        if downOrDrag && glaspen2_router_pen_drawing() == 0 {
            glaspen2_router_hide_system_cursor()
            glaspen2_router_set_pen_drawing(1)
        }
        if type == .leftMouseUp || type == .rightMouseUp || type == .otherMouseUp {
            restoreSystemCursor()
            glaspen2_router_cursor_hide()
            glaspen2_router_cursor_invalidate()
            glaspen2_router_flush()
        }
    }

    /// 恢复系统光标(S6-3-3d): 只有确实隐藏过(pen_drawing)才 show + 清标记。
    @objc(restoreSystemCursor)
    public static func restoreSystemCursor() {
        if glaspen2_router_pen_drawing() != 0 {
            glaspen2_router_show_system_cursor()
            glaspen2_router_set_pen_drawing(0)
        }
    }

    /// S6-3-4b-2b: 普通 chord 门控(录制热键由 ObjC 函数先行处理)。
    /// 返回 1 = 放行, 2 = 消费(返回 NULL)。
    @objc(handleChordKeyEvent:)
    public static func handleChordKeyEvent(_ event: CGEvent) -> Int32 {
        guard let ev = NSEvent(cgEvent: event) else { return 1 }
        guard ev.type == .keyDown else { return 1 }
        guard let action = actionForHotkey(keyCode: ev.keyCode,
                                           modifiers: ev.modifierFlags.rawValue) else {
            return 1
        }
        let st = state()
        if (st & 1) == 0
            && action != "toggle_draw" && action != "toggle_free_canvas"
            && action != "quit" {
            return 1
        }
        if action.hasPrefix("canvas_") && ((st & 4) == 0 || (st & 2) != 0) {
            return 1
        }
        let repeatable = action.hasPrefix("canvas_pan_")
            || action == "canvas_zoom_in" || action == "canvas_zoom_out"
        let isRepeat = event.getIntegerValueField(.keyboardEventAutorepeat) != 0
        if !isRepeat || repeatable { glaspen2_router_perform_action(action) }
        return 2
    }

    /// S6-3-4b: 键盘事件 → 已绑定 action(键名/查找分别走薄桥)。
    @objc(actionForHotkeyWithKeyCode:modifiers:)
    public static func actionForHotkey(keyCode: UInt16, modifiers: UInt) -> String? {
        guard let key = glaspen2_router_keycode_name(keyCode) else { return nil }
        let mods = NSEvent.ModifierFlags(rawValue: modifiers)
        var parts: [String] = []
        if mods.contains(.command) { parts.append("cmd") }
        if mods.contains(.control) { parts.append("ctrl") }
        if mods.contains(.option) { parts.append("alt") }
        if mods.contains(.shift) { parts.append("shift") }
        if !parts.contains("ctrl") && !parts.contains("alt") { return nil }
        parts.append(key)
        return glaspen2_router_action_for_chord(parts.joined(separator: "+"))
    }

    /// ObjC 侧复用同一笔事件判定(避免两处公式各自维护)。
    @objc(isPenForEvent:)
    public static func isPenForEvent(_ event: CGEvent) -> Bool {
        isPenEvent(event)
    }

    private static func isPenEvent(_ event: CGEvent) -> Bool {
        guard let ev = NSEvent(cgEvent: event) else { return false }
        return isPen(ev)
    }

    private static func penCursorUpdate(type: CGEventType, event: CGEvent) {
        guard let ev = NSEvent(cgEvent: event) else { return }
        let isUp = type == .leftMouseUp || type == .rightMouseUp || type == .otherMouseUp
        glaspen2_router_pen_bubble(isUp ? 0 : 1)
        let state = glaspen2_router_pen_state()
        let strokesVisible = (state & 1) != 0
        let strokeActive = (state & 2) != 0
        let cursorVisible = (state & 4) != 0
        if !strokesVisible { glaspen2_router_pen_auto_show() }
        glaspen2_router_peek_cancel()
        let loc = ev.locationInWindow
        let oldX = glaspen2_router_cursor_x()
        let oldY = glaspen2_router_cursor_y()
        let moved = oldX != loc.x || oldY != loc.y
        if moved && cursorVisible { glaspen2_router_cursor_invalidate() }
        glaspen2_router_cursor_set(Double(loc.x), Double(loc.y))
        if !strokeActive { glaspen2_router_cursor_show(1) }
        if moved && glaspen2_router_pen_cursor_visible() != 0 {
            glaspen2_router_cursor_invalidate()
            if !strokeActive && glaspen2_router_cursor_flush_due() == 1 {
                glaspen2_router_flush()
            }
        }
    }

    /// 虚拟笔(GLASPEN2_VIRTUAL_PEN)合成事件判定。
    @objc(isVirtualPenEvent:)
    public static func isVirtualPenEvent(_ event: CGEvent) -> Bool {
        event.getIntegerValueField(.eventSourceUserData) == glaspen2_virtual_pen_user_data()
    }

    private static func state() -> Int32 { glaspen2_router_state() }

    private static func routeScrollZoom(_ cgEvent: CGEvent) -> Int32 {
        guard let ev = NSEvent(cgEvent: cgEvent) else { return 1 }
        let relevant: NSEvent.ModifierFlags = [.command, .control, .option, .shift]
        let mods = ev.modifierFlags.intersection(relevant)
        let cameraMods = UInt(glaspen2_router_camera_mods())
        let hasCameraMods = cameraMods != 0 && mods.rawValue == cameraMods
        let state = state()
        if hasCameraMods && (state & 1) != 0 && (state & 2) == 0 {
            let dy = ev.scrollingDeltaY
            let factor: Double = ev.hasPreciseScrollingDeltas ? exp(-dy * 0.0015)
                : (dy > 0 ? 1.1 : (dy < 0 ? 1.0 / 1.1 : 1.0))
            let loc = ev.locationInWindow
            let h = glaspen2_router_draw_height()
            glaspen2_router_zoom_at(factor, Double(loc.x), Double(h) - Double(loc.y))
            return 2
        }
        return 1
    }
}
