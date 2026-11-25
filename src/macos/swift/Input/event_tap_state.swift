// S7: event tap 状态与回调(Objective-C 侧只留 2 行 callback 壳) — C 符号导出。
import AppKit
import CoreGraphics

@objc public final class EventTapState: NSObject {
    static var tap: CFMachPort?
    static var includeScroll = false
}

@_cdecl("glaspen2_event_tap_store")
public func eventTapStore(_ tap: CFMachPort?) {
    if let old = EventTapState.tap {
        CFMachPortInvalidate(old)
    }
    EventTapState.tap = tap
}

@_cdecl("glaspen2_event_tap_note_scroll")
public func eventTapNoteScroll(_ on: Int32) { EventTapState.includeScroll = on != 0 }

@_cdecl("glaspen2_router_event_tap_ok")
public func eventTapOK() -> Int32 { EventTapState.tap != nil ? 1 : 0 }

@_cdecl("glaspen2_router_event_tap_enabled")
public func eventTapEnabled() -> Int32 {
    guard let tap = EventTapState.tap else { return 0 }
    return CGEvent.tapIsEnabled(tap: tap) ? 1 : 0
}

@_cdecl("glaspen2_router_event_tap_enable")
public func eventTapEnable() {
    if let tap = EventTapState.tap { CGEvent.tapEnable(tap: tap, enable: true) }
}

@_cdecl("sw_event_tap_disable")
public func eventTapDisable() {
    if let tap = EventTapState.tap { CGEvent.tapEnable(tap: tap, enable: false) }
}

@_cdecl("sw_event_tap_callback")
public func eventTapCallback(_ proxyRaw: UnsafeMutableRawPointer?,
                             _ typeRaw: UInt32,
                             _ eventRaw: UnsafeMutableRawPointer?,
                             _ refcon: UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? {
    guard let eventRaw else { return nil }
    let type = CGEventType(rawValue: typeRaw) ?? .null
    let event = Unmanaged<CGEvent>.fromOpaque(eventRaw).takeUnretainedValue()
    let route = EventRouter.route(type: type, event: event)
    if route == 2 { return nil }
    if EventRouter.isVirtualPenEvent(event) { return nil }
    return Unmanaged.passUnretained(event).toOpaque()
}
