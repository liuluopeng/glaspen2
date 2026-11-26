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
