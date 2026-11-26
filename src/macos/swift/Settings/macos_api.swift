// S7: Rust → macOS 的 C ABI 主体(设置快照/页刷新与导航/删除页/热键/导出)。
import AppKit
import Darwin

@objc public final class MacosApi: NSObject {
    @objc public static func settingsJSON() -> UnsafeMutablePointer<CChar>? {
        var out: UnsafeMutablePointer<CChar>?
        settingsRunOnMain {
            guard let overrides = glaspen2_menu_snapshot_overrides_json() else { return }
            defer { free(overrides) }
            guard let json = glaspen2_settings_snapshot_json(overrides) else { return }
            let text = String(cString: json)
            glaspen2_free_c_string(json)
            out = strdup(text)
        }
        return out
    }

    @objc(refreshPageWithScreenId:)
    public static func refreshPage(screenId: Int64) {
        if screenId <= 0 { return }
        settingsRunOnMain {
            if glaspen2_router_infinite() != 0 { return }
            if screenId != Int64(glaspen2_get_current_screen_id()) { return }
            glaspen2_load_strokes_for_screen(screenId)
            glaspen2_rust_pageview_rebuild()
            glaspen2_rust_overlay_flush()
        }
    }

    @objc public static func refreshInfinite() {
        settingsRunOnMain {
            if glaspen2_router_infinite() == 0 { return }
            glaspen2_load_infinite_strokes()
            glaspen2_rust_pageview_rebuild()
            glaspen2_rust_overlay_flush()
        }
    }

    @objc(navigateToPageWithScreenId:)
    public static func navigateToPage(screenId: Int64) {
        if screenId <= 0 { return }
        if glaspen2_router_infinite() != 0 {
            settingsRunOnMain {
                _ = "".withCString { glaspen2_rust_apply_infinite(0, $0) }
            }
        }
        DispatchQueue.global(qos: .userInitiated).sync {
            glaspen2_load_strokes_for_screen(screenId)
            glaspen2_rust_pageview_update()
            settingsRunOnMain {
                glaspen2_rust_pageview_rebuild()
            }
        }
    }

    @objc(deletePageWithScreenId:)
    public static func deletePage(screenId: Int64) -> Int32 {
        var ok: Int32 = 0
        let wasCurrent = screenId == Int64(glaspen2_get_current_screen_id())
        let nextId: Int64 = wasCurrent ? Int64(glaspen2_next_screen_id()) : 0
        let prevId: Int64 = wasCurrent ? Int64(glaspen2_prev_screen_id()) : 0
        DispatchQueue.global(qos: .userInitiated).sync {
            ok = glaspen2_delete_screen(screenId)
            settingsRunOnMain {
                if glaspen2_router_infinite() != 0 {
                    glaspen2_rust_pageview_rebuild()
                    return
                }
                if ok != 0 && wasCurrent {
                    let target = nextId != 0 ? nextId : prevId
                    if target != 0 {
                        glaspen2_load_strokes_for_screen(target)
                        glaspen2_rust_pageview_update()
                        glaspen2_rust_pageview_rebuild()
                    } else {
                        glaspen2_rust_surface_clear_screen()
                    }
                }
            }
        }
        return ok
    }

    @objc(hotkeyWithKey:)
    public static func hotkey(key: String) {
        settingsRunOnMain {
            if key == "Q" {
                glaspen2_rust_menu_quit_app()
                return
            }
            let keyCode: UInt16
            switch key {
            case "G": keyCode = 0x05
            case "\u{0060}": keyCode = 0x32
            case "1": keyCode = 0x12
            case "Z": keyCode = 0x06
            case "X": keyCode = 0x07
            case "C": keyCode = 0x08
            case "V": keyCode = 0x09
            case "B": keyCode = 0x0B
            default: return
            }
            _ = HotkeyDispatch.perform(keyCode: keyCode)
        }
    }

    @objc public static func exportPDF() -> Int32 {
        var ok: Int32 = 0
        DispatchQueue.global(qos: .userInitiated).sync {
            ok = glaspen2_export_pdf()
        }
        return ok
    }

    @objc public static func exportGIF() -> Int32 {
        var ok: Int32 = 0
        DispatchQueue.global(qos: .userInitiated).sync {
            ok = glaspen2_save_animated_gif(glaspen2_router_gif_fps(),
                                            glaspen2_router_gif_resolution(),
                                            glaspen2_router_gif_speed(),
                                            glaspen2_router_gif_end_mode())
            settingsRunOnMain {
                if ok != 0 {
                    copyNewestGifToPasteboard()
                    notificationShowText(text:
                        MenuRuntime.pair("动画 GIF 已保存并复制到剪贴板",
                                                "Animated GIF saved & copied"))
                } else if let text = Optional(MenuRuntime.l10n("recording.gif_no_strokes_or_export_failed")) {
                    notificationShowText(text:text)
                }
            }
        }
        return ok
    }

    private static func copyNewestGifToPasteboard() {
        guard let desktop = FileManager.default.urls(for: .desktopDirectory,
                                                     in: .userDomainMask).first else { return }
        let files = (try? FileManager.default.contentsOfDirectory(
            at: desktop, includingPropertiesForKeys: [.contentModificationDateKey])) ?? []
        var newest: URL?
        var newestDate: Date?
        for f in files where f.lastPathComponent.hasPrefix("glaspen2_")
            && f.pathExtension == "gif" {
            let d = (try? f.resourceValues(forKeys: [.contentModificationDateKey]))?
                .contentModificationDate
            if newestDate == nil || (d != nil && d! > newestDate!) {
                newestDate = d
                newest = f
            }
        }
        guard let newest else { return }
        let pb = NSPasteboard.general
        pb.clearContents()
        pb.writeObjects([newest as NSURL])
    }
}

func settingsRunOnMain(_ block: @escaping () -> Void) {
    if Thread.isMainThread {
        block()
    } else {
        DispatchQueue.main.sync(execute: block)
    }
}


// S7: Rust FRB 反向调用的设置面板入口(C ABI, 由 Swift 实现)。
import AppKit
import Darwin

@_cdecl("glaspen2_macos_settings_json")
public func settingsApiJSON() -> UnsafeMutablePointer<CChar>? {
    MacosApi.settingsJSON()
}

@_cdecl("glaspen2_macos_free_c_string")
public func settingsApiFreeCString(_ p: UnsafeMutablePointer<CChar>?) {
    free(p)
}

@_cdecl("glaspen2_macos_free_bytes")
public func settingsApiFreeBytes(_ p: UnsafeMutablePointer<UInt8>?) {
    free(p)
}

@_cdecl("glaspen2_macos_set_setting")
public func settingsApiSetSetting(_ key: UnsafePointer<CChar>?, _ value: UnsafePointer<CChar>?) {
    guard let key, let value else { return }
    _ = glaspen2_rust_apply_setting(key, value)
}

@_cdecl("glaspen2_macos_delete_page")
public func settingsApiDeletePage(_ screenId: Int64) -> Int32 {
    MacosApi.deletePage(screenId: screenId)
}

@_cdecl("glaspen2_macos_refresh_page")
public func settingsApiRefreshPage(_ screenId: Int64) {
    MacosApi.refreshPage(screenId: screenId)
}

@_cdecl("glaspen2_macos_refresh_infinite")
public func settingsApiRefreshInfinite() { MacosApi.refreshInfinite() }

@_cdecl("glaspen2_macos_navigate_to_page")
public func settingsApiNavigate(_ screenId: Int64) {
    MacosApi.navigateToPage(screenId: screenId)
}

@_cdecl("glaspen2_macos_hotkey")
public func settingsApiHotkey(_ key: UnsafePointer<CChar>?) {
    guard let key else { return }
    MacosApi.hotkey(key: String(cString: key))
}

@_cdecl("glaspen2_macos_export_pdf")
public func settingsApiExportPDF() -> Int32 { MacosApi.exportPDF() }

@_cdecl("glaspen2_macos_export_gif")
public func settingsApiExportGIF() -> Int32 { MacosApi.exportGIF() }

private func runOnMainSync(_ block: () -> Void) {
    if Thread.isMainThread {
        block()
    } else {
        DispatchQueue.main.sync(execute: block)
    }
}

@_cdecl("glaspen2_macos_canvas_payload")
public func settingsApiCanvasPayload(_ w: Int32, _ h: Int32, _ action: Int32,
                                     _ rectOut: UnsafeMutablePointer<Double>?,
                                     _ outLen: UnsafeMutablePointer<Int32>?) -> UnsafeMutablePointer<UInt8>? {
    var result: UnsafeMutablePointer<UInt8>?
    runOnMainSync {
        var ow = Int(w), oh = Int(h)
        if ow < 400 { ow = 1024 }
        if oh < 400 { oh = 768 }
        if action == 1 {
            glaspen2_rust_camera_home()
        } else if action == 2 {
            var bx = 0.0, by = 0.0, bx2 = 0.0, by2 = 0.0
            if glaspen2_stroke_bbox(&bx, &by, &bx2, &by2) != 0 {
                var px = 0.0, py = 0.0, pz = 0.0
                if glaspen2_camera_centered_values(bx, by, bx2, by2,
                                                   glaspen2_router_screen_w(),
                                                   glaspen2_router_screen_h(),
                                                   glaspen2_router_zoom(),
                                                   &px, &py, &pz) != 0 {
                    glaspen2_router_set_camera(px, py, pz, 0)
                    glaspen2_rust_camera_apply_transform()
                }
            }
        } else if action == 3 {
            EventRouter.finishActiveStroke()
            glaspen2_clear_strokes(Int32(glaspen2_router_screen_w()),
                                   Int32(glaspen2_router_screen_h()))
            glaspen2_rust_camera_home()
            ow = 1024
            oh = 768
        }
        var rect = [Double](repeating: 0, count: 4)
        var bounds = [Double](repeating: 0, count: 4)
        var pngLen: Int32 = 0
        let png = rect.withUnsafeMutableBufferPointer { r in
            bounds.withUnsafeMutableBufferPointer { b in
                glaspen2_rust_canvas_overview(Double(ow), Double(oh), r.baseAddress, b.baseAddress, &pngLen)
            }
        }
        guard let png, pngLen > 0 else { return }
        let data = Data(bytes: png, count: Int(pngLen))
        glaspen2_free_rust_bytes(png, pngLen)
        let payload: [String: Any] = ["png": data, "rect": rect, "bounds": bounds]
        guard !data.isEmpty else { return }
        let buf = UnsafeMutablePointer<UInt8>.allocate(capacity: data.count)
        data.copyBytes(to: buf, count: data.count)
        result = buf
        outLen?.pointee = Int32(data.count)
        if let rectOut {
            if let rect = payload["rect"] as? [NSNumber], rect.count == 4 {
                for i in 0..<4 { rectOut[i] = rect[i].doubleValue }
            }
            if let bounds = payload["bounds"] as? [NSNumber] {
                for i in 0..<min(4, bounds.count) { rectOut[4 + i] = bounds[i].doubleValue }
            }
        }
    }
    return result
}


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
