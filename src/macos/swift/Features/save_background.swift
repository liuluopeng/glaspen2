// S7: 带背景保存(桌面截图 + 笔迹合成 Display P3; 14+ 用 SCScreenshotManager, 12/13 走 CGWindowList)。
import AppKit
import CoreGraphics
import ScreenCaptureKit
import Darwin

@objc public final class SaveBackground: NSObject {
    @objc public static func save() {
        guard let surface = SurfaceStore.current else { return }
        cairo_surface_flush(surface)
        let dw = Int(cairo_image_surface_get_width(surface))
        let dh = Int(cairo_image_surface_get_height(surface))
        let dstride = Int(cairo_image_surface_get_stride(surface))
        guard let dptr = cairo_image_surface_get_data(surface),
              dw > 0, dh > 0, dstride > 0 else {
            SurfaceOps.saveDrawingOnly()
            return
        }
        let size = dstride * dh
        guard let drawingCopy = malloc(size) else {
            SurfaceOps.saveDrawingOnly()
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
                    NotificationPresenter.show(text:MenuRuntime.pair(message, message))
                }
                SurfaceOps.saveDrawingOnly()
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
            let want = OverlayWindowFactory.windowDisplayID()
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
                NotificationPresenter.show(text:message)
            }
        }
    }
}
