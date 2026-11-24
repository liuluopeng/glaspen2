// S6-1: 事件 tap 的掩码构造(含滚轮切换)迁 Swift; 创建/回调仍留 ObjC。
import AppKit

@objc public final class EventTapInstaller: NSObject {
    @objc(eventMaskWithScroll:)
    public static func eventMask(includeScroll: Bool) -> UInt64 {
        let types: [CGEventType] = [
            .mouseMoved, .leftMouseDown, .leftMouseDragged, .leftMouseUp,
            .rightMouseDown, .rightMouseDragged, .rightMouseUp,
            .otherMouseDown, .otherMouseDragged, .otherMouseUp,
            .tabletProximity, .tabletPointer, .keyDown, .keyUp,
        ]
        var mask: UInt64 = 0
        for t in types { mask |= CGEventMask(1) << UInt64(t.rawValue) }
        if includeScroll { mask |= CGEventMask(1) << UInt64(CGEventType.scrollWheel.rawValue) }
        return mask
    }
}
