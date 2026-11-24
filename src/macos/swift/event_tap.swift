// S6: 事件 tap 的掩码构造与创建/挂载; 回调函数指针与 tap 存储经 ObjC 薄桥(CF 所有权留在 ObjC)。
import AppKit
import CoreGraphics

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

    @objc(installWithScroll:)
    public static func install(includeScroll: Bool) {
        let mask = eventMask(includeScroll: includeScroll)
        guard let callback = glaspen2_event_tap_callback_ptr() else {
            glaspen2_event_tap_store(nil)
            return
        }
        guard let tap = CGEvent.tapCreate(tap: .cgSessionEventTap,
                                          place: .headInsertEventTap,
                                          options: .defaultTap,
                                          eventsOfInterest: CGEventMask(mask),
                                          callback: callback,
                                          userInfo: nil) else {
            glaspen2_event_tap_store(nil)
            return
        }
        CGEvent.tapEnable(tap: tap, enable: true)
        if let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0) {
            CFRunLoopAddSource(CFRunLoopGetMain(), source, CFRunLoopMode.commonModes)
        }
        glaspen2_event_tap_store(tap)
        glaspen2_event_tap_note_scroll(includeScroll ? 1 : 0)
    }
}
