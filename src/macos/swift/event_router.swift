// S6-3-0: 事件路由骨架。Swift 先默认返回 0(交由 ObjC 内层处理),
// 之后按事件类型逐步把分支搬进来: 1 = 放行, 2 = 吞掉(返回 NULL)。
import AppKit

@objc public final class EventRouter: NSObject {
    /// 0 = fallback(ObjC), 1 = pass(return event), 2 = consume(return NULL)
    @objc(routeEventOfType:event:)
    public static func route(type: CGEventType, event: CGEvent) -> Int32 {
        0
    }
}
