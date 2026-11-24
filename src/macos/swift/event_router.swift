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
        if type == .tapDisabledByTimeout || type == .tapDisabledByUserInput {
            glaspen2_router_reenable_tap()
            NSLog("[glaspen2] TAP DISABLED (timeout/userinput) — re-enabled")
            return 1
        }
        if type == .keyDown || type == .keyUp {
            return routeKeyboardEvent(type: type, event: event)
        }
        if type == .tabletProximity {
            routeProximity(event)
            return 1
        }
        // 相机临时抓手: 非笔拖/放 或 ⌃⌥+左键按下(须在菜单栏判定之前)。
        if type == .leftMouseDragged || type == .leftMouseUp || type == .leftMouseDown {
            if let ev = NSEvent(cgEvent: event), !isPen(ev), routeCameraMouse(type: type, ev: ev) {
                return 2
            }
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
        // 笔事件: 坐标/压力/raw_w 在本层计算, 再交 pen 分派(原 inner 逻辑)。
        if let ev = NSEvent(cgEvent: event), isPen(ev) {
            let loc = ev.locationInWindow
            let px = Double(loc.x)
            let py = glaspen2_router_draw_height() - Double(loc.y)
            let pressure = Double(ev.pressure)
            let ts = ev.timestamp
            updatePressureMonitor(event: event, type: type, x: px, y: py, pressure: pressure)
            let rawW = glaspen2_router_raw_width(pressure)
            if glaspen2_router_strokes_hidden() != 0
                && (type == .leftMouseDown || type == .rightMouseDown || type == .otherMouseDown
                    || type == .leftMouseDragged || type == .rightMouseDragged
                    || type == .otherMouseDragged) {
                glaspen2_router_pen_auto_show()
            }
            let isEraser = ev.pointingDeviceType == .eraser
                || glaspen2_router_prox_is_eraser() != 0
            if handlePenEvent(type: type, x: px, y: py, pressure: pressure,
                              timestamp: ts, rawWidth: rawW, isEraser: isEraser) == 2 {
                return 2
            }
        }
        return 1
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

    /// S6-3-4: 键盘事件: 录制热键 → 普通 chord; 返回 1 放行 / 2 消费。
    private static func routeKeyboardEvent(type: CGEventType, event: CGEvent) -> Int32 {
        if glaspen2_router_hotkey_recording() != 0 { return 1 }
        guard let ev = NSEvent(cgEvent: event) else { return 1 }
        let hasCmdCtrl = ev.modifierFlags.contains(.command)
            && ev.modifierFlags.contains(.control)
        // 录制键的 keyDown/keyUp 都要交给状态机(抬起用于结束会话)。
        if RecordingHotkeys.handle(type: type, keyCode: ev.keyCode, hasCmdCtrl: hasCmdCtrl) {
            return 2
        }
        if handleChordKeyEvent(event) == 2 { return 2 }
        return 1
    }

    /// S6-3-3d: 压感监视器状态更新(是否在压感区/压力/坐标/tip down/事件名)。
    @objc(updatePressureMonitorWithEvent:type:x:y:pressure:)
    public static func updatePressureMonitor(event: CGEvent, type: CGEventType,
                                             x: Double, y: Double, pressure: Double) {
        guard glaspen2_router_pm_enabled() != 0 else { return }
        let rawField = event.getIntegerValueField(.tabletEventPointPressure)
        let pmPressure = rawField > 0 ? Int(rawField * 65535 / 65536)
                                      : Int(pressure * 65535)
        let tipDown = type == .leftMouseDown || type == .rightMouseDown
            || type == .otherMouseDown || type == .leftMouseDragged
            || type == .rightMouseDragged || type == .otherMouseDragged
        let name: String
        switch type {
        case .leftMouseDown: name = "DOWN"
        case .leftMouseUp: name = "UP"
        case .leftMouseDragged: name = "DRAG"
        case .rightMouseDown: name = "R_DOWN"
        case .rightMouseUp: name = "R_UP"
        case .rightMouseDragged: name = "R_DRAG"
        case .otherMouseDown: name = "O_DOWN"
        case .otherMouseUp: name = "O_UP"
        case .otherMouseDragged: name = "O_DRAG"
        default: name = tipDown ? "DOWN" : "MOVE"
        }
        glaspen2_router_pm_update(1, Int32(pmPressure), x, y, tipDown ? 1 : 0, name)
    }

    /// S6-3-3d: 笔事件分派(落笔/拖动/抬笔); 返回 2 = 消费事件。
    @objc(handlePenEventOfType:x:y:pressure:timestamp:rawWidth:isEraser:)
    public static func handlePenEvent(type: CGEventType, x: Double, y: Double,
                                      pressure: Double, timestamp: Double,
                                      rawWidth: Double, isEraser: Bool) -> Int32 {
        let downTypes: [CGEventType] = [.leftMouseDown, .rightMouseDown, .otherMouseDown]
        let dragTypes: [CGEventType] = [.leftMouseDragged, .rightMouseDragged, .otherMouseDragged]
        let upTypes: [CGEventType] = [.leftMouseUp, .rightMouseUp, .otherMouseUp]
        if downTypes.contains(type) {
            updateEraserState(isEraser: isEraser)
            NSLog("[glaspen2] pen DOWN p=%.2f ts=%.3f", pressure, timestamp)
            penDown(x: x, y: y, pressure: pressure, timestamp: timestamp, rawWidth: rawWidth)
            return 2
        }
        if dragTypes.contains(type) {
            if glaspen2_router_stroke_active() == 0 {
                updateEraserState(isEraser: isEraser)
                penDown(x: x, y: y, pressure: pressure, timestamp: timestamp, rawWidth: rawWidth)
            } else {
                penMove(x: x, y: y, pressure: pressure, timestamp: timestamp, rawWidth: rawWidth)
            }
            return 2
        }
        if upTypes.contains(type) {
            NSLog("[pen] 抬笔 (pen up)")
            penUp(x: x, y: y, pressure: pressure, timestamp: timestamp)
            return 2
        }
        return 0
    }

    /// S6-3-3d: 相机临时抓手(⌃⌥+左键拖动)开始/拖动/结束; 返回 true = 消费事件。
    private static func routeCameraMouse(type: CGEventType, ev: NSEvent) -> Bool {
        let st = state()
        if glaspen2_router_camera_dragging() != 0
            && (type == .leftMouseDragged || type == .leftMouseUp) {
            if type == .leftMouseDragged {
                if (st & 4) != 0 && (st & 1) != 0 && (st & 2) == 0 {
                    let loc = ev.locationInWindow
                    let z = glaspen2_router_camera_step_zoom()
                    glaspen2_router_camera_queue_pan(
                        (Double(loc.x) - glaspen2_router_camera_last_x()) / z,
                        (glaspen2_router_camera_last_y() - Double(loc.y)) / z)
                    glaspen2_router_camera_set_last(Double(loc.x), Double(loc.y))
                }
            } else {
                glaspen2_router_camera_flush()
                glaspen2_router_camera_clear_dragging()
            }
            return true
        }
        if (st & 4) != 0 && (st & 1) != 0 && (st & 2) == 0 && type == .leftMouseDown {
            let relevant: NSEvent.ModifierFlags = [.command, .control, .option, .shift]
            let mods = ev.modifierFlags.intersection(relevant)
            let cameraMods = UInt(glaspen2_router_camera_pan_mods())
            if cameraMods != 0 && mods.rawValue == cameraMods {
                let loc = ev.locationInWindow
                glaspen2_router_camera_start(Double(loc.x), Double(loc.y))
                return true
            }
        }
        return false
    }

    /// S6-3-3d: 笔 proximity 进入/离开: 悬停显示/离开隐藏, 飘渺模式联动。
    private static func routeProximity(_ event: CGEvent) {
        guard let prox = NSEvent(cgEvent: event) else { return }
        let dev = prox.pointingDeviceType
        guard dev == .pen || dev == .eraser else { return }
        glaspen2_router_mark_tablet_proximity()
        let enter = event.getIntegerValueField(.tabletProximityEventEnterProximity) == 1
        let label = enter ? (dev == .eraser ? "橡皮进入" : "悬浮进入") : "悬浮离开"
        NSLog("[pen] \(label) (proximity \(enter ? "enter" : "exit"))")
        if enter {
            glaspen2_router_set_prox_device(Int32(dev.rawValue))
            glaspen2_router_peek_cancel()
            glaspen2_router_bubble_pen_back()
            if glaspen2_router_ethereal() != 0 { glaspen2_router_pen_auto_show() }
        } else {
            glaspen2_router_bubble_pen_gone()
            if (glaspen2_router_cursor_flags() & 1) != 0 {
                glaspen2_router_cursor_invalidate()
                glaspen2_router_cursor_hide()
                glaspen2_router_flush()
            }
            if glaspen2_router_ethereal() != 0 { glaspen2_router_auto_hide() }
        }
    }

    /// S6-3-3d: 落笔入口: 自动翻页 + 模型器 begin + 共享 context + raw dot + 落笔状态。
    @objc(penDownWithX:y:pressure:timestamp:rawWidth:)
    public static func penDown(x: Double, y: Double, pressure: Double,
                               timestamp: Double, rawWidth: Double) {
        glaspen2_router_bubble_auto_page_flip()
        glaspen2_router_modeler_begin(x, y, pressure, timestamp)
        glaspen2_router_stroke_begin()
        glaspen2_router_raw_draw_dot(x, y, rawWidth)
        glaspen2_router_pen_down_state(x, y, rawWidth)
    }

    /// S6-3-3d: 抬笔: 结束模型器(橡皮 finish / 正常 end+commit) + context/状态复位。
    @objc(penUpWithX:y:pressure:timestamp:)
    public static func penUp(x: Double, y: Double, pressure: Double,
                             timestamp: Double) {
        guard glaspen2_router_stroke_active() != 0 else { return }
        if glaspen2_router_eraser_mode() != 0 {
            glaspen2_router_modeler_erase_finish()
            updateEraserState(isEraser: false)
        } else {
            glaspen2_router_modeler_end(x, y, pressure, timestamp)
            glaspen2_router_commit_to_strokes()
        }
        glaspen2_router_stroke_end()
        glaspen2_router_set_stroke_active(0)
        glaspen2_router_apply_pending_color_width()
        glaspen2_router_pen_up_state()
    }

    /// S6-3-3d: 拖动实时段: 模型器 move + raw segment(响应式反馈)。
    @objc(penMoveWithX:y:pressure:timestamp:rawWidth:)
    public static func penMove(x: Double, y: Double, pressure: Double,
                               timestamp: Double, rawWidth: Double) {
        glaspen2_router_modeler_move(x, y, pressure, timestamp)
        glaspen2_router_raw_draw_segment(x, y, rawWidth)
    }

    /// S6-3-3d: 橡皮态切换 + 状态图标刷新(落笔/拖动自动补 begin/抬笔共用)。
    @objc(updateEraserState:)
    public static func updateEraserState(isEraser: Bool) {
        if isEraser != (glaspen2_router_eraser_mode() != 0) {
            glaspen2_router_set_eraser_mode(isEraser ? 1 : 0)
            glaspen2_router_update_status_icon_state()
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
