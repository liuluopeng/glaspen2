// S6: 事件路由。滚轮缩放(3-1)、非笔光标隐藏(3-2)、笔光标更新(3-3b)。
import AppKit

@objc public final class EventRouter: NSObject {
    /// 0 = fallback(ObjC), 1 = pass(return event), 2 = consume(return NULL)
    @objc(routeEventOfType:event:)
    public static func route(type: CGEventType, event: CGEvent) -> Int32 {
        // tap 被系统停用必须优先恢复(旧实现也在最前); 涂鸦停用时同样要能恢复。
        if type == .tapDisabledByTimeout || type == .tapDisabledByUserInput {
            glaspen2_router_reenable_tap()
            NSLog("[glaspen2] TAP DISABLED (timeout/userinput) — re-enabled")
            return 1
        }
        // 旧实现: proximity 在 "停用涂鸦" 提前返回之前处理, 保持状态不陈旧。
        if type == .tabletProximity {
            routeProximity(event)
            return 1
        }
        // 停用涂鸦时: 非键盘/非滚轮事件先清理相机/光标再放行(与原 ObjC 同序)。
        if (state() & 1) == 0 && type != .keyDown && type != .keyUp
            && type != .scrollWheel {
            glaspen2_router_disable_cleanup()
            return 1
        }
        if type == .keyDown || type == .keyUp {
            return routeKeyboardEvent(type: type, event: event)
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
        // 旧实现: 任意鼠标/笔抬起都恢复系统光标并清掉十字(不只笔事件)。
        if type == .leftMouseUp || type == .rightMouseUp || type == .otherMouseUp {
            EventRouter.restoreSystemCursor()
            glaspen2_router_cursor_hide()
            glaspen2_rust_dirty_include_point(glaspen2_router_cursor_x(), glaspen2_router_cursor_y(), 14.0)
            glaspen2_rust_overlay_flush_dirty()
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
        glaspen2_rust_pen_system_cursor(Int32(type.rawValue))
    }

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
    private static func routeKeyboardEvent(type: CGEventType, event: CGEvent) -> Int32 {
        if HotkeyRecorder.isRecording { return 1 }
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
        let rawField = event.getIntegerValueField(.tabletEventPointPressure)
        _ = glaspen2_rust_update_pressure_monitor(Int32(type.rawValue), rawField, x, y, pressure)
    }

    /// S6-3-3d: 笔事件分派(落笔/拖动/抬笔); 返回 2 = 消费事件。
    @objc(handlePenEventOfType:x:y:pressure:timestamp:rawWidth:isEraser:)
    public static func handlePenEvent(type: CGEventType, x: Double, y: Double,
                                      pressure: Double, timestamp: Double,
                                      rawWidth: Double, isEraser: Bool) -> Int32 {
        glaspen2_rust_handle_pen_event(Int32(type.rawValue), x, y, pressure, timestamp,
                                       rawWidth, isEraser ? 1 : 0)
    }

    /// S6-3-3d: 相机临时抓手(⌃⌥+左键拖动)开始/拖动/结束; 返回 true = 消费事件。
    private static func routeCameraMouse(type: CGEventType, ev: NSEvent) -> Bool {
        if ProcessInfo.processInfo.environment["GLASPEN2_RUST_CAMERA_MOUSE"] == "1" {
            let kind: Int32
            if type == .leftMouseDown { kind = 0 }
            else if type == .leftMouseDragged { kind = 1 }
            else if type == .leftMouseUp { kind = 2 }
            else { return false }
            let loc = ev.locationInWindow
            let relevant: NSEvent.ModifierFlags = [.command, .control, .option, .shift]
            let mods = ev.modifierFlags.intersection(relevant).rawValue
            return glaspen2_rust_camera_mouse(kind, Double(loc.x), Double(loc.y), UInt64(mods)) != 0
        }
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
        let enter = event.getIntegerValueField(.tabletProximityEventEnterProximity) == 1
        _ = glaspen2_rust_route_proximity(Int32(dev.rawValue), enter ? 1 : 0)
    }

    /// S6-3-3d: 落笔入口: 自动翻页 + 模型器 begin + 共享 context + raw dot + 落笔状态。
    @objc(penDownWithX:y:pressure:timestamp:rawWidth:)
    public static func penDown(x: Double, y: Double, pressure: Double,
                               timestamp: Double, rawWidth: Double) {
        if ProcessInfo.processInfo.environment["GLASPEN2_SWIFT_PEN_ORCH"] != "1" {
            glaspen2_rust_pen_down(x, y, pressure, timestamp, rawWidth, SurfaceStore.scaleFactor)
            return
        }
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
        if ProcessInfo.processInfo.environment["GLASPEN2_SWIFT_PEN_ORCH"] != "1" {
            glaspen2_rust_pen_up(x, y, pressure, timestamp)
            return
        }
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
        if ProcessInfo.processInfo.environment["GLASPEN2_SWIFT_PEN_ORCH"] != "1" {
            glaspen2_rust_pen_move(x, y, pressure, timestamp, rawWidth, SurfaceStore.scaleFactor)
            return
        }
        glaspen2_router_modeler_move(x, y, pressure, timestamp)
        glaspen2_router_raw_draw_segment(x, y, rawWidth)
    }

    /// S6-3-3d: 橡皮态切换 + 状态图标刷新(落笔/拖动自动补 begin/抬笔共用)。
    @objc(updateEraserState:)
    public static func updateEraserState(isEraser: Bool) {
        if ProcessInfo.processInfo.environment["GLASPEN2_SWIFT_PEN_ORCH"] != "1" {
            glaspen2_rust_update_eraser(isEraser ? 1 : 0)
            return
        }
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
        let m = ev.modifierFlags
        return glaspen2_rust_handle_chord(
            ev.keyCode,
            m.contains(.command) ? 1 : 0,
            m.contains(.control) ? 1 : 0,
            m.contains(.option) ? 1 : 0,
            m.contains(.shift) ? 1 : 0,
            event.getIntegerValueField(.keyboardEventAutorepeat) != 0 ? 1 : 0)
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
        let loc = ev.locationInWindow
        glaspen2_rust_pen_cursor_update(Int32(type.rawValue), isUp ? 1 : 0,
                                        Double(loc.x), Double(loc.y))
    }

    /// 虚拟笔(GLASPEN2_VIRTUAL_PEN)合成事件判定。
    @objc(isVirtualPenEvent:)
    public static func isVirtualPenEvent(_ event: CGEvent) -> Bool {
        event.getIntegerValueField(.eventSourceUserData) == glaspen2_virtual_pen_user_data()
    }

    private static func state() -> Int32 { glaspen2_router_state() }

    private static func routeScrollZoom(_ cgEvent: CGEvent) -> Int32 {
        guard let ev = NSEvent(cgEvent: cgEvent) else { return 1 }
        if ProcessInfo.processInfo.environment["GLASPEN2_RUST_CAMERA_MOUSE"] == "1" {
            let relevant: NSEvent.ModifierFlags = [.command, .control, .option, .shift]
            let mods = ev.modifierFlags.intersection(relevant).rawValue
            let loc = ev.locationInWindow
            return glaspen2_rust_scroll_zoom(
                ev.scrollingDeltaY, ev.hasPreciseScrollingDeltas ? 1 : 0,
                Double(loc.x), Double(loc.y), UInt64(mods))
        }
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
