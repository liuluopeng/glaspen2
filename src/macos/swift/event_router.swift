// S6-3-0/1: 事件路由骨架 + 滚轮缩放分支(无限画布, 修饰键来自 canvas_zoom_in)。
import AppKit

@objc public final class EventRouter: NSObject {
    /// 0 = fallback(ObjC), 1 = pass(return event), 2 = consume(return NULL)
    @objc(routeEventOfType:event:)
    public static func route(type: CGEventType, event: CGEvent) -> Int32 {
        if type == .scrollWheel && (state() & 4) != 0 {
            return routeScrollZoom(event)
        }
        return 0
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
            let factor: Double
            if ev.hasPreciseScrollingDeltas {
                factor = exp(-dy * 0.0015)
            } else if dy > 0 {
                factor = 1.1
            } else if dy < 0 {
                factor = 1.0 / 1.1
            } else {
                factor = 1.0
            }
            let loc = ev.locationInWindow
            let h = glaspen2_router_draw_height()
            glaspen2_router_zoom_at(factor, Double(loc.x), Double(h) - Double(loc.y))
            return 2
        }
        return 1
    }
}
