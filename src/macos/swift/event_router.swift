// S6-3: 事件路由。S6-3-1 滚轮缩放;S6-3-2 非笔 mouseMoved 隐藏光标。
import AppKit

@objc public final class EventRouter: NSObject {
    /// 0 = fallback(ObjC), 1 = pass(return event), 2 = consume(return NULL)
    @objc(routeEventOfType:event:)
    public static func route(type: CGEventType, event: CGEvent) -> Int32 {
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
        return 0
    }

    private static func isPen(_ ev: NSEvent) -> Bool {
        let dev = ev.pointingDeviceType
        if dev == .pen || dev == .eraser { return true }
        return ev.subtype == .tabletPoint || ev.subtype == .tabletProximity
    }

    /// 虚拟笔(GLASPEN2_VIRTUAL_PEN)合成事件判定(S6-3-3 最小件)。
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
