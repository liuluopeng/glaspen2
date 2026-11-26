// S6: 事件路由。滚轮缩放(3-1)、非笔光标隐藏(3-2)、笔光标更新(3-3b)。
import AppKit

@objc public final class EventRouter: NSObject {
    /// 0 = fallback(ObjC), 1 = pass(return event), 2 = consume(return NULL)
    @objc(routeEventOfType:event:)
    public static func route(type: CGEventType, event: CGEvent) -> Int32 {
        if type == .tabletProximity {
            routeProximity(event)
            return 1
        }
        var e = GlaspenEventC()
        let kind = type.rawValue
        e.kind = Int32(kind)
        if kind == 10 || kind == 11 {
            if let ev = NSEvent(cgEvent: event) {
                e.key_code = Int32(ev.keyCode)
                let relevant: NSEvent.ModifierFlags = [.command, .control, .option, .shift]
                let raw = ev.modifierFlags.intersection(relevant).rawValue
                e.flags = Int32(bitPattern: UInt32(truncatingIfNeeded: raw))
                e.is_repeat = event.getIntegerValueField(.keyboardEventAutorepeat) != 0 ? 1 : 0
            }
            return glaspen2_rust_route_event(&e)
        }
        let pointerKind = (1...9).contains(kind) || kind == 23
        if let ev = NSEvent(cgEvent: event) {
            let relevant: NSEvent.ModifierFlags = [.command, .control, .option, .shift]
            let raw = ev.modifierFlags.intersection(relevant).rawValue
            e.flags = Int32(bitPattern: UInt32(truncatingIfNeeded: raw))
            let loc = ev.locationInWindow
            e.x = Double(loc.x)
            e.y = Double(loc.y)
            e.pen_x = e.x
            e.pen_y = glaspen2_router_draw_height() - e.y
            e.screen_y = event.location.y
            if pointerKind {
                e.is_pen = isPen(ev) ? 1 : 0
                e.raw_field = event.getIntegerValueField(.tabletEventPointPressure)
                e.pressure = Double(ev.pressure)
                e.timestamp = ev.timestamp
                e.is_eraser = (ev.pointingDeviceType == .eraser
                    || glaspen2_router_prox_is_eraser() != 0) ? 1 : 0
            } else if kind == 22 {
                e.delta_y = ev.scrollingDeltaY
                e.precise = ev.hasPreciseScrollingDeltas ? 1 : 0
            }
        }
        return glaspen2_rust_route_event(&e)
    }

    private static func isPen(_ ev: NSEvent) -> Bool {
        let dev = ev.pointingDeviceType
        if dev == .pen || dev == .eraser { return true }
        return ev.subtype == .tabletPoint || ev.subtype == .tabletProximity
    }

    /// 笔按下/拖动期间隐藏系统光标; 任意 up 恢复光标并清十字/刷新(S6-3-3c)。


    /// 恢复系统光标(S6-3-3d): 只有确实隐藏过(pen_drawing)才 show + 清标记。
    @objc(restoreSystemCursor)
    public static func restoreSystemCursor() {
        glaspen2_rust_restore_system_cursor()
    }

    /// 收尾在写笔画(退出/切页/丢 pen-up 时共用), 与 penUp 的区别是不动光标/不刷新。
    @objc(finishActiveStroke)
    public static func finishActiveStroke() {
        glaspen2_rust_finish_active_stroke()
    }

    /// S6-3-4: 键盘事件: 录制热键 → 普通 chord; 返回 1 放行 / 2 消费。


    /// S6-3-3d: 压感监视器状态更新(是否在压感区/压力/坐标/tip down/事件名)。
    /// S6-3-3d: 笔事件分派(落笔/拖动/抬笔); 返回 2 = 消费事件。
    /// S6-3-3d: 相机临时抓手(⌃⌥+左键拖动)开始/拖动/结束; 返回 true = 消费事件。


    /// S6-3-3d: 笔 proximity 进入/离开: 悬停显示/离开隐藏, 飘渺模式联动。
    private static func routeProximity(_ event: CGEvent) {
        guard let prox = NSEvent(cgEvent: event) else { return }
        let dev = prox.pointingDeviceType
        let enter = event.getIntegerValueField(.tabletProximityEventEnterProximity) == 1
        _ = glaspen2_rust_route_proximity(Int32(dev.rawValue), enter ? 1 : 0)
    }

    /// S6-3-3d: 落笔入口: 自动翻页 + 模型器 begin + 共享 context + raw dot + 落笔状态。

    /// S6-3-3d: 抬笔: 结束模型器(橡皮 finish / 正常 end+commit) + context/状态复位。

    /// S6-3-3d: 拖动实时段: 模型器 move + raw segment(响应式反馈)。

    /// S6-3-3d: 橡皮态切换 + 状态图标刷新(落笔/拖动自动补 begin/抬笔共用)。

    /// S6-3-4b-2b: 普通 chord 门控(录制热键由 ObjC 函数先行处理)。
    /// 返回 1 = 放行, 2 = 消费(返回 NULL)。
    /// ObjC 侧复用同一笔事件判定(避免两处公式各自维护)。
    /// 虚拟笔(GLASPEN2_VIRTUAL_PEN)合成事件判定。
    @objc(isVirtualPenEvent:)
    public static func isVirtualPenEvent(_ event: CGEvent) -> Bool {
        event.getIntegerValueField(.eventSourceUserData) == glaspen2_virtual_pen_user_data()
    }




}


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
        let callback: CGEventTapCallBack = { _, type, event, _ in
            let route = EventRouter.route(type: type, event: event)
            if route == 2 { return nil }
            if EventRouter.isVirtualPenEvent(event) { return nil }
            return Unmanaged.passUnretained(event)
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
