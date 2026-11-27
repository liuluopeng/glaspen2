// S7: run.m 剩余 C ABI 薄桥(Swift 化)。
import AppKit
import CoreGraphics
import Darwin

@_cdecl("glaspen2_run")
public func appRun() {
    if glaspen2_rust_app_bootstrap_start() == 0 { return }
    NSApp.run()
}



























private let runLangEnPtr: UnsafeMutablePointer<CChar> = strdup("en")!
private let runLangZhPtr: UnsafeMutablePointer<CChar> = strdup("zh")!








@_cdecl("glaspen2_router_event_tap_reinstall")
public func runEventTapReinstall() {
    EventTapInstaller.install(includeScroll: glaspen2_router_infinite() != 0)
}









@_cdecl("glaspen2_swift_tunnel_active")
public func runTunnelActive() -> Int32 { TunnelState.active ? 1 : 0 }

@_cdecl("glaspen2_swift_tunnel_cancel_timer")
public func runTunnelCancelTimer() {
    TunnelState.timer?.cancel()
    TunnelState.timer = nil
}
@_cdecl("glaspen2_swift_tunnel_mark_inactive")
public func runTunnelMarkInactive() { TunnelState.active = false }

@_cdecl("glaspen2_swift_tunnel_clear_state")
public func runTunnelClearState() {
    for i in 0..<TunnelState.maxCards {
        TunnelState.cardSurf[i] = nil
        TunnelState.mipSurf[i] = nil
        TunnelState.depths[i] = 0
    }
    TunnelState.blur = nil
    TunnelState.surface = nil
    TunnelState.cards = 0
    glaspen2_rust_clear_tunnel_cache()
}


@_cdecl("glaspen2_router_refresh_menu_texts")
public func runRefreshMenuTexts() { MenuTextUpdater.update() }




@_cdecl("glaspen2_router_invert_ink_source")
public func runInvertInkSource(_ cr: UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? {
    panelInvertInkSource(cr)
}








@_cdecl("glaspen2_router_invert_apply")
public func runInvertApply(_ on: Int32) { panelInvertApply(on) }





@_cdecl("glaspen2_router_invert_restart")
public func runInvertRestart() { panelInvertRestart() }





@_cdecl("glaspen2_router_invert_invalidate")
public func runInvertInvalidate() { panelInvertInvalidate() }



// R119: 录制效果 Rust 侧 UI 桥。


public typealias RecordingMainCallback = @convention(c) (UnsafeMutableRawPointer?) -> Void

@_cdecl("glaspen2_swift_dispatch_main")
public func runRecordingDispatchMain(_ ctx: UnsafeMutableRawPointer?, _ cb: RecordingMainCallback?) {
    DispatchQueue.main.async { cb?(ctx) }
}




@_cdecl("glaspen2_swift_tunnel_set_surfaces")
public func runTunnelSetSurfaces(_ surface: UnsafeMutableRawPointer?, _ blur: UnsafeMutableRawPointer?) {
    TunnelState.surface = surface.map { OpaquePointer($0) }
    TunnelState.blur = blur.map { OpaquePointer($0) }
}

@_cdecl("glaspen2_swift_tunnel_surface")
public func runTunnelSurface() -> UnsafeMutableRawPointer? {
    guard let s = TunnelState.surface else { return nil }
    return UnsafeMutableRawPointer(s)
}


@_cdecl("glaspen2_swift_update_menu_texts")
public func runUpdateMenuTexts() { MenuTextUpdater.update() }

@_cdecl("glaspen2_swift_set_main_anchors")
public func runSetMainAnchors(_ window: UnsafeMutableRawPointer?,
                              _ draw: UnsafeMutableRawPointer?,
                              _ glass: UnsafeMutableRawPointer?) {
    if let window { UIAnchors.mainWindow = Unmanaged<NSWindow>.fromOpaque(window).takeUnretainedValue() }
    if let draw { UIAnchors.drawView = Unmanaged<NSView>.fromOpaque(draw).takeUnretainedValue() }
    if let glass { UIAnchors.glassView = Unmanaged<NSVisualEffectView>.fromOpaque(glass).takeUnretainedValue() }
}

func i18nTextf(_ key: String, _ placeholder: String, _ value: String) -> String {
    guard let p = key.withCString({ k in placeholder.withCString { ph in value.withCString { v in
        glaspen2_rust_i18n_textf(k, ph, v)
    } } }) else { return key }
    defer { glaspen2_free_c_string(p) }
    return String(cString: p)
}


@_cdecl("glaspen2_swift_run_on_main_sync")
public func runOnMainSyncBridge(_ ctx: UnsafeMutableRawPointer?,
                                _ cb: (@convention(c) (UnsafeMutableRawPointer?) -> Void)?) {
    guard let cb else { return }
    if Thread.isMainThread {
        cb(ctx)
    } else {
        DispatchQueue.main.sync { cb(ctx) }
    }
}

@_cdecl("glaspen2_swift_draw_overlay_bridge")
public func glaspen2SwiftDrawOverlayBridge(_ view: UnsafeMutableRawPointer?,
                                           _ x: Double, _ y: Double,
                                           _ w: Double, _ h: Double) {
    guard let view else { return }
    let nsView = Unmanaged<NSView>.fromOpaque(view).takeUnretainedValue()
    _ = glaspen2_rust_overlay_draw(Unmanaged.passUnretained(nsView).toOpaque(), x, y, w, h)
}

@_cdecl("glaspen2_swift_bootstrap_prepare")
public func bootstrapPrepare() {
    NSApplication.shared
    NSApp.setActivationPolicy(.accessory)
    let opts = [kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true]
    if !AXIsProcessTrustedWithOptions(opts as CFDictionary) {
        NSLog("[glaspen2] Accessibility permission not granted")
    }
}



@_cdecl("glaspen2_swift_bootstrap_delegate_menu")
public func bootstrapDelegateMenu(_ item: UnsafeMutableRawPointer?) -> Int32 {
    guard let item else { return 0 }
    let statusItem = Unmanaged<NSStatusItem>.fromOpaque(item).takeUnretainedValue()
    UIAnchors.statusItem = statusItem
    NSApplication.shared
    NSApp.setActivationPolicy(.accessory)
    let handler = AppDelegate()
    AppDelegate.shared = handler
    NSApp.delegate = handler
    let menu = MainMenuBuilder.build(handler: handler, statusItem: statusItem,
                                     launchAtLoginOn: glaspen2_is_launch_at_login() != 0,
                                     glassOn: glaspen2_router_glass_enabled() != 0)
    UIAnchors.menu = menu
    return 1
}

@_cdecl("glaspen2_swift_bootstrap_display_observer")
public func bootstrapDisplayObserver() {
    NotificationCenter.default.addObserver(
        forName: NSApplication.didChangeScreenParametersNotification,
        object: nil, queue: .main) { _ in glaspen2_rust_display_changed() }
}



import FlutterMacOS

private var bootstrapFlutterEngine: FlutterEngine?
private var bootstrapFlutterVC: FlutterViewController?

@_cdecl("glaspen2_swift_make_flutter_vc")
public func bootstrapMakeFlutterVC() -> UnsafeMutableRawPointer? {
    if let vc = bootstrapFlutterVC {
        return Unmanaged.passUnretained(vc).toOpaque()
    }
    let engine = FlutterEngine(name: "glaspen_settings", project: nil)
    engine.run(withEntrypoint: nil)
    bootstrapFlutterEngine = engine
    let vc = FlutterViewController(engine: engine, nibName: nil, bundle: nil)
    bootstrapFlutterVC = vc
    return Unmanaged.passUnretained(vc).toOpaque()
}

@_cdecl("glaspen2_swift_activate_settings_existing")
public func bootstrapActivateSettingsExisting() {
    NSApp.setActivationPolicy(.regular)
    NSApp.activate(ignoringOtherApps: true)
    UIAnchors.settingsWindow?.makeKeyAndOrderFront(nil)
}

@_cdecl("glaspen2_swift_show_settings_window")
public func bootstrapShowSettingsWindow(_ p: UnsafeMutableRawPointer?) {
    guard let p else { return }
    let window = Unmanaged<NSWindow>.fromOpaque(p).takeUnretainedValue()
    NSApp.setActivationPolicy(.regular)
    NSApp.activate(ignoringOtherApps: true)
    window.center()
    window.makeKeyAndOrderFront(nil)
}


func notificationShowText(text: String) {
    text.withCString { glaspen2_rust_notification_show($0) }
}
func notificationShowPageInfo(screenId: Int64) {
    glaspen2_rust_notification_show_page_info(screenId)
}

@_cdecl("glaspen2_macos_swift_notification_selftest")
public func notificationSelftest() -> Int32 {
    _ = NSApplication.shared
    let w = 400.0, h = 200.0
    let image = NSImage(size: NSSize(width: w, height: h))
    image.lockFocus()
    let ok = "TEST NOTIFICATION".withCString {
        glaspen2_rust_draw_notification($0, w, h) != 0
    }
    image.unlockFocus()
    guard ok, let tiff = image.tiffRepresentation,
          let rep = NSBitmapImageRep(data: tiff),
          let data = rep.bitmapData else {
        print("SWIFT_NOTIFICATION_FAIL rust_draw_or_bitmap")
        return 1
    }
    var nonEmpty = 0
    let count = rep.bytesPerRow * rep.pixelsHigh
    for i in stride(from: 0, to: count, by: 4) where data[i] != 0 || data[i + 1] != 0 || data[i + 2] != 0 { nonEmpty += 1 }
    print("SWIFT_NOTIFICATION_RUST_OK pixels=\(nonEmpty)")
    return nonEmpty > 0 ? 0 : 1
}

@_cdecl("glaspen2_swift_dispatch_main_after")
public func runDispatchMainAfter(_ delay: Double, _ ctx: UnsafeMutableRawPointer?,
                                 _ cb: (@convention(c) (UnsafeMutableRawPointer?) -> Void)?) {
    guard let cb else { return }
    DispatchQueue.main.asyncAfter(deadline: .now() + delay) { cb(ctx) }
}


func surfaceCurrent() -> OpaquePointer? {
    glaspen2_rust_surface_current().map { OpaquePointer($0) }
}
func surfaceScale() -> Double { glaspen2_rust_surface_scale() }
func surfaceSet(_ ptr: OpaquePointer?) {
    glaspen2_rust_surface_set(ptr.map(UnsafeMutableRawPointer.init))
}
func surfaceEnsure(view: NSView) {
    _ = glaspen2_rust_surface_ensure(Unmanaged.passUnretained(view).toOpaque())
}




@_cdecl("glaspen2_macos_swift_pageframe_parity_selftest")
public func pageframeParitySelftest() -> Int32 {
    let w = 300, h = 200
    func make() -> OpaquePointer? {
        guard let s = cairo_image_surface_create(cairo_format_t(0), Int32(w), Int32(h)),
              cairo_surface_status(s).rawValue == 0 else { return nil }
        cairo_surface_flush(s)
        return s
    }
    guard let a = make(), let b = make() else { return 1 }
    _ = glaspen2_rust_draw_page_frame(UnsafeMutableRawPointer(a), 1.0, 20, 10, 0.8, 200, 150, 1)
    if let cr = cairo_create(b) {
        let x = 20.0, y = 10.0, pw = 200.0 * 0.8, ph = 150.0 * 0.8
        cairo_set_source_rgba(cr, 0, 0, 0, 0.42)
        cairo_set_fill_rule(cr, cairo_fill_rule_t(1))
        cairo_rectangle(cr, 0, 0, Double(w), Double(h))
        cairo_rectangle(cr, x, y, pw, ph)
        cairo_fill(cr)
        cairo_set_source_rgba(cr, 1, 1, 1, 0.55)
        cairo_set_line_width(cr, 1.0)
        cairo_rectangle(cr, x + 0.5, y + 0.5, pw - 1, ph - 1)
        cairo_stroke(cr)
        cairo_destroy(cr)
    }
    cairo_surface_flush(a); cairo_surface_flush(b)
    guard let da = cairo_image_surface_get_data(a), let db = cairo_image_surface_get_data(b) else { return 1 }
    let stride = Int(cairo_image_surface_get_stride(a))
    var diff = 0
    for yy in 0..<h { for xx in 0..<w {
        let i = yy * stride + xx * 4
        if da[i] != db[i] || da[i+1] != db[i+1] || da[i+2] != db[i+2] || da[i+3] != db[i+3] { diff += 1 }
    } }
    cairo_surface_destroy(a); cairo_surface_destroy(b)
    print("SWIFT_PAGEFRAME_PARITY diff=\(diff)/\(w*h)")
    return diff == 0 ? 0 : 1
}




@objc public final class UIAnchors: NSObject {
    static var mainWindow: NSWindow? {
        get { sw_main_window().map { Unmanaged<NSWindow>.fromOpaque($0).takeUnretainedValue() } }
        set { sw_set_main_window(newValue.map { Unmanaged.passUnretained($0).toOpaque() }) }
    }
    static var drawView: NSView? {
        get { sw_draw_view().map { Unmanaged<NSView>.fromOpaque($0).takeUnretainedValue() } }
        set { sw_set_draw_view(newValue.map { Unmanaged.passUnretained($0).toOpaque() }) }
    }
    static var glassView: NSVisualEffectView? {
        get { sw_glass_view().map { Unmanaged<NSVisualEffectView>.fromOpaque($0).takeUnretainedValue() } }
        set { sw_set_glass_view(newValue.map { Unmanaged.passUnretained($0).toOpaque() }) }
    }
    static var statusItem: NSStatusItem? {
        get { sw_status_item().map { Unmanaged<NSStatusItem>.fromOpaque($0).takeUnretainedValue() } }
        set { sw_set_status_item(newValue.map { Unmanaged.passUnretained($0).toOpaque() }) }
    }
    static var menu: NSMenu? {
        get { sw_menu().map { Unmanaged<NSMenu>.fromOpaque($0).takeUnretainedValue() } }
        set { sw_set_menu(newValue.map { Unmanaged.passUnretained($0).toOpaque() }) }
    }
    static var settingsWindow: NSWindow? {
        get { sw_settings_window().map { Unmanaged<NSWindow>.fromOpaque($0).takeUnretainedValue() } }
        set { sw_set_settings_window(newValue.map { Unmanaged.passUnretained($0).toOpaque() }) }
    }
}

private func opaque<T: AnyObject>(_ v: T?) -> UnsafeMutableRawPointer? {
    guard let v else { return nil }
    return Unmanaged.passUnretained(v).toOpaque()
}



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
        let pointerKind = (1...9).contains(kind) || kind == 23 || (25...27).contains(kind)
        if pointerKind {
            // 即使 NSEvent(cgEvent:) 取不到, 虚拟笔/感应范围内的悬浮移动也必须
            // 按笔事件路由, 不能落到 mouseMoved 的 ethereal auto_hide 分支。
            if EventRouter.isVirtualPenEvent(event)
                || (kind == 5 && glaspen2_rust_pen_tablet_proximity() != 0) {
                e.is_pen = 1
            }
        }
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
                // 笔在数位板感应范围内时, 驱动会把悬浮移动报成 mouseMoved;
                // 这类事件必须按笔处理, 否则 ethereal 的 mouseMoved 分支会
                // auto_hide, 与 pen_auto_show 交替造成老笔迹闪烁。
                if e.is_pen == 0 {
                    let hoverWithPenInRange = kind == 5 && glaspen2_rust_pen_tablet_proximity() != 0
                    let virtualPen = EventRouter.isVirtualPenEvent(event)
                    e.is_pen = (isPen(ev) || hoverWithPenInRange || virtualPen) ? 1 : 0
                }
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


// S7: 压感监视器小窗(无边框、置顶、不接收鼠标)。
import AppKit





// S7: 带背景保存(桌面截图 + 笔迹合成 Display P3; 14+ 用 SCScreenshotManager, 12/13 走 CGWindowList)。
import AppKit
import CoreGraphics
import ScreenCaptureKit
import Darwin

@objc public final class SaveBackground: NSObject {
    @objc public static func save() {
        guard let surface = surfaceCurrent() else { return }
        cairo_surface_flush(surface)
        let dw = Int(cairo_image_surface_get_width(surface))
        let dh = Int(cairo_image_surface_get_height(surface))
        let dstride = Int(cairo_image_surface_get_stride(surface))
        guard let dptr = cairo_image_surface_get_data(surface),
              dw > 0, dh > 0, dstride > 0 else {
            glaspen2_rust_surface_save_drawing_only()
            return
        }
        let size = dstride * dh
        guard let drawingCopy = malloc(size) else {
            glaspen2_rust_surface_save_drawing_only()
            return
        }
        memcpy(drawingCopy, dptr, size)

        let wasNone = glaspen2_router_window_sharing_type() == 0
        if wasNone {
            glaspen2_router_set_window_sharing_readonly()
        }
        func restoreSharing() {
            if wasNone { glaspen2_router_set_window_sharing_none() }
        }
        func failAndSaveDrawing(_ message: String) {
            restoreSharing()
            free(drawingCopy)
            DispatchQueue.main.async {
                if !message.isEmpty {
                    notificationShowText(text:MenuRuntime.pair(message, message))
                }
                glaspen2_rust_surface_save_drawing_only()
            }
        }

        if #available(macOS 14.0, *) {
            captureSCK(drawingCopy: drawingCopy, width: dw, height: dh,
                       stride: dstride) { image in
                restoreSharing()
                if let image {
                    compositeAndSave(image, drawingCopy, dw, dh, dstride)
                } else {
                    failAndSaveDrawing("截图失败，已保存涂鸦")
                }
            }
        } else {
            let image = captureBelowWindow()
            restoreSharing()
            if let image {
                compositeAndSave(image, drawingCopy, dw, dh, dstride)
            } else {
                failAndSaveDrawing("截图失败(需要屏幕录制权限), 已保存涂鸦")
            }
        }
    }

    @available(macOS 14.0, *)
    private static func captureSCK(drawingCopy: UnsafeMutableRawPointer,
                                   width: Int, height: Int, stride: Int,
                                   completion: @escaping (CGImage?) -> Void) {
        SCShareableContent.getExcludingDesktopWindows(false,
                                                      onScreenWindowsOnly: true) { content, error in
            if error != nil || content?.displays.isEmpty != false {
                NSLog("[glaspen2] Screen capture failed: %@", "\(String(describing: error))")
                completion(nil)
                return
            }
            guard let content else { completion(nil); return }
            let want = glaspen2_rust_window_display_id()
            let display = content.displays.first { $0.displayID == want }
                ?? content.displays.first!
            let filter = SCContentFilter(display: display, excludingWindows: [])
            let config = SCStreamConfiguration()
            config.width = display.width
            config.height = display.height
            SCScreenshotManager.captureImage(contentFilter: filter,
                                             configuration: config) { image, error in
                if error != nil || image == nil {
                    NSLog("[glaspen2] Screenshot failed: %@", "\(String(describing: error))")
                    completion(nil)
                    return
                }
                completion(image)
            }
        }
    }

    private static func captureBelowWindow() -> CGImage? {
        typealias CaptureFn = @convention(c) (CGRect, CGWindowListOption,
                                              CGWindowID, CGWindowImageOption) -> Unmanaged<CGImage>?
        guard let handle = dlopen(nil, RTLD_LAZY),
              let sym = dlsym(handle, "CGWindowListCreateImage") else { return nil }
        let fn = unsafeBitCast(sym, to: CaptureFn.self)
        guard let window = UIAnchors.mainWindow else { return nil }
        return fn(.null, .optionOnScreenBelowWindow,
                  CGWindowID(window.windowNumber), CGWindowImageOption(rawValue: 0))?.takeRetainedValue()
    }

    private static func compositeAndSave(_ image: CGImage, _ drawingCopy: UnsafeMutableRawPointer,
                                         _ dw: Int, _ dh: Int, _ dstride: Int) {
        let bw = image.width, bh = image.height
        guard let p3 = CGColorSpace(name: CGColorSpace.displayP3),
              let srgb = CGColorSpace(name: CGColorSpace.sRGB) else {
            free(drawingCopy); return
        }
        // 截图 → Display P3
        guard let bgCtx = CGContext(data: nil, width: bw, height: bh,
                                    bitsPerComponent: 8, bytesPerRow: bw * 4,
                                    space: p3,
                                    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else {
            free(drawingCopy); return
        }
        bgCtx.draw(image, in: CGRect(x: 0, y: 0, width: bw, height: bh))
        guard let p3Image = bgCtx.makeImage(),
              let bgData = p3Image.dataProvider?.data,
              let bgPtr = CFDataGetBytePtr(bgData) else {
            free(drawingCopy); return
        }
        let bgStride = p3Image.bytesPerRow

        // 笔迹(cairo BGRA 内存) → CGImage → Display P3
        let drawData = Data(bytesNoCopy: drawingCopy, count: dstride * dh,
                            deallocator: .none)
        guard let drawProvider = CGDataProvider(data: drawData as CFData),
              let drawImage = CGImage(width: dw, height: dh, bitsPerComponent: 8,
                                      bitsPerPixel: 32, bytesPerRow: dstride,
                                      space: srgb,
                                      bitmapInfo: CGBitmapInfo(
                                        rawValue: CGImageAlphaInfo.premultipliedFirst.rawValue
                                            | CGBitmapInfo.byteOrder32Little.rawValue),
                                      provider: drawProvider, decode: nil,
                                      shouldInterpolate: false,
                                      intent: .defaultIntent) else {
            free(drawingCopy); return
        }
        guard let drawCtx = CGContext(data: nil, width: dw, height: dh,
                                      bitsPerComponent: 8, bytesPerRow: dw * 4,
                                      space: p3,
                                      bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else {
            free(drawingCopy); return
        }
        drawCtx.draw(drawImage, in: CGRect(x: 0, y: 0, width: dw, height: dh))
        guard let p3Draw = drawCtx.makeImage(),
              let drawP3Data = p3Draw.dataProvider?.data,
              let drawPtr = CFDataGetBytePtr(drawP3Data) else {
            free(drawingCopy); return
        }
        let drawStride = p3Draw.bytesPerRow

        glaspen2_save_with_background(drawPtr, Int32(dw), Int32(dh), Int32(drawStride),
                                      bgPtr, Int32(bw), Int32(bh), Int32(bgStride))
        free(drawingCopy)
        if let message = Optional(MenuRuntime.l10n("export.screenshot_with_bg_saved")) {
            DispatchQueue.main.async {
                notificationShowText(text:message)
            }
        }
    }
}


// S7: 反色突出捕获(ScreenCaptureKit 壳; 状态/像素处理/缓存均在 Rust)。
import AppKit
import CoreMedia
import CoreVideo
import ScreenCaptureKit

@available(macOS 12.3, *)
@objc public final class InvertCapture: NSObject, SCStreamOutput {
    private static var filter: SCContentFilter?
    private static var stream: SCStream?
    private static var output: InvertCapture?
    private static var queue: DispatchQueue?
    private static var captureW = 0
    private static var captureH = 0

    @objc public static func apply(on: Bool) {
        if on {
            glaspen2_set_stroke_invert(1)
            schedule(0.05)
        } else {
            teardown()
            glaspen2_set_stroke_invert(0)
            glaspen2_router_set_window_sharing_readonly()
            glaspen2_rust_pageview_rebuild()
            glaspen2_rust_overlay_flush()
        }
    }

    @objc public static func restart() {
        stop()
        schedule(0.05)
    }

    @objc public static func invalidate() {
        stop()
        filter = nil
        glaspen2_router_set_window_sharing_readonly()
        teardown()
    }

    @objc(inkSourceWithCR:)
    public static func inkSource(crRaw: UnsafeMutableRawPointer) -> UnsafeMutableRawPointer? {
        guard glaspen2_router_invert_enabled() != 0 else { return nil }
        return glaspen2_rust_invert_ink_source(crRaw).map { $0 }
    }

    public func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer,
                       of type: SCStreamOutputType) {
        if type != .screen { return }
        if glaspen2_router_invert_enabled() == 0 { return }
        if !CMSampleBufferIsValid(sampleBuffer) { return }
        guard let pb = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }
        CVPixelBufferLockBaseAddress(pb, .readOnly)
        defer { CVPixelBufferUnlockBaseAddress(pb, .readOnly) }
        guard let base = CVPixelBufferGetBaseAddress(pb) else { return }
        let ok = glaspen2_rust_invert_process(
            base.assumingMemoryBound(to: UInt8.self),
            Int32(CVPixelBufferGetPixelFormatType(pb)),
            Int32(CVPixelBufferGetWidth(pb)),
            Int32(CVPixelBufferGetHeight(pb)),
            Int32(CVPixelBufferGetBytesPerRow(pb)))
        if ok != 0 {
            DispatchQueue.main.async { glaspen2_rust_invert_consume() }
        }
    }

    private static func stop() {
        guard let st = stream else { return }
        stream = nil
        st.stopCapture { error in
            if let error { NSLog("[invert] stream stop: %@", "\(error)") }
        }
        if let output {
            try? st.removeStreamOutput(output, type: .screen)
        }
    }

    private static func teardown() {
        stop()
        glaspen2_rust_invert_teardown()
        captureW = 0
        captureH = 0
    }

    private static func start() {
        if stream != nil || filter == nil || captureW <= 0 { return }
        if output == nil { output = InvertCapture() }
        if queue == nil {
            queue = DispatchQueue(label: "glaspen.invert")
        }
        let config = SCStreamConfiguration()
        config.width = captureW
        config.height = captureH
        config.showsCursor = false
        config.pixelFormat = kCVPixelFormatType_32BGRA
        config.queueDepth = 3
        config.minimumFrameInterval = CMTime(value: 1, timescale: Int32(glaspen2_router_invert_fps()))
        guard let filter, let output, let queue else { return }
        let st = SCStream(filter: filter, configuration: config, delegate: nil)
        do {
            try st.addStreamOutput(output, type: .screen, sampleHandlerQueue: queue)
        } catch {
            NSLog("[invert] addStreamOutput failed: %@", "\(error)")
            stream = nil
            return
        }
        stream = st
        st.startCapture { error in
            if let error { NSLog("[invert] startCapture failed: %@", "\(error)") }
        }
    }

    private static func schedule(_ delay: Double) {
        DispatchQueue.main.asyncAfter(deadline: .now() + delay) {
            tick()
        }
    }

    private static func tick() {
        if glaspen2_router_invert_enabled() == 0 { return }
        guard let surface = surfaceCurrent() else {
            schedule(0.5)
            return
        }
        if TunnelState.active {
            schedule(0.5)
            return
        }
        if glaspen2_has_strokes() == 0 && glaspen2_router_stroke_active() == 0 {
            stop()
            schedule(0.5)
            return
        }
        cairo_surface_flush(surface)
        let dw = Int(cairo_image_surface_get_width(surface))
        let dh = Int(cairo_image_surface_get_height(surface))
        if dw <= 0 || dh <= 0 { return }
        if captureW != dw || captureH != dh {
            stop()
            filter = nil
            captureW = dw
            captureH = dh
        }
        if glaspen2_rust_invert_ensure_cache(Int32(dw), Int32(dh)) == 0 { return }
        if filter == nil {
            SCShareableContent.getExcludingDesktopWindows(false,
                                                          onScreenWindowsOnly: true) { content, error in
                DispatchQueue.main.async {
                    if error != nil || content?.displays.isEmpty != false
                        || glaspen2_router_invert_enabled() == 0 {
                        schedule(1.0)
                        return
                    }
                    guard let content, let display = content.displays.first else {
                        schedule(1.0)
                        return
                    }
                    let mine = Bundle.main.bundleIdentifier
                    let excl = content.windows.filter {
                        $0.owningApplication?.bundleIdentifier == mine
                    }
                    filter = SCContentFilter(display: display, excludingWindows: excl)
                    glaspen2_router_set_window_sharing_none()
                    NSLog("[invert] 过滤器就绪(排除 %d 个自家窗口), 共享已关闭", excl.count)
                    tick()
                }
            }
            return
        }
        start()
    }
}

func settingsRunOnMain(_ block: @escaping () -> Void) {
    if Thread.isMainThread {
        block()
    } else {
        DispatchQueue.main.sync(execute: block)
    }
}



@_cdecl("glaspen2_swift_tunnel_start")
public func runTunnelStart(_ goingNext: Int32, _ curPage: Int64) -> Int32 {
    TunnelAnimation.start(goingNext: goingNext != 0, curPage: Int(curPage), prepare: {}, commit: {
        glaspen2_rust_pageview_rebuild()
    }) ? 1 : 0
}
