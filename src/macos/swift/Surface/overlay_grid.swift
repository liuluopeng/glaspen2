// S7: 网格/分栏线段绘制(样式与旧 ObjC 逐值一致)。几何仍由 core::grid 产出。
import AppKit
import CoreGraphics

@objc public final class OverlayGridRenderer: NSObject {
    @objc(strokeLayerWithContext:segments:viewHeight:white:alpha:lineWidth:)
    public static func strokeLayer(_ ctx: CGContext, segments: NSArray,
                                   viewHeight: Double, white: Double,
                                   alpha: Double, lineWidth: Double) {
        guard segments.count > 0 else { return }
        ctx.setStrokeColor(NSColor(white: white, alpha: alpha).cgColor)
        ctx.setLineWidth(lineWidth)
        for item in segments {
            guard let seg = item as? NSDictionary,
                  let x0 = seg["x0"] as? NSNumber,
                  let y0 = seg["y0"] as? NSNumber,
                  let x1 = seg["x1"] as? NSNumber,
                  let y1 = seg["y1"] as? NSNumber else { continue }
            ctx.move(to: CGPoint(x: x0.doubleValue, y: viewHeight - y0.doubleValue))
            ctx.addLine(to: CGPoint(x: x1.doubleValue, y: viewHeight - y1.doubleValue))
        }
        ctx.strokePath()
    }

    /// drawRect 网格层: 门控 + pan/zoom 换算 + core::grid 三段线段绘制。
    @objc(drawGridWithContext:bounds:clipRect:)
    public static func drawGrid(_ ctx: CGContext, bounds: NSRect, clipRect: NSRect) {
        let showGrid = glaspen2_router_show_grid() != 0
        let divider = glaspen2_router_grid_divider()
        let tunActive = TunnelState.active
        let strokesVisible = glaspen2_router_strokes_visible() != 0
        let follow = glaspen2_router_grid_follow() != 0
        guard (showGrid || divider > 0) && !tunActive
            && (strokesVisible || !follow) else { return }
        let viewW = Double(bounds.size.width)
        let viewH = Double(bounds.size.height)
        guard viewW > 0, viewH > 0 else { return }
        var panX = 0.0, panY = 0.0, zoom = 1.0
        if glaspen2_router_infinite() != 0 {
            panX = CameraStore.panX
            panY = CameraStore.panY
            let z = CameraStore.zoom
            zoom = z > 0.05 ? z : 0.05
        } else {
            panX = -CameraStore.pageOffX
            panY = -CameraStore.pageOffY
        }
        let gridSize = glaspen2_router_grid_size()
        let clipX0 = Double(clipRect.minX), clipX1 = Double(clipRect.maxX)
        let clipY0 = viewH - Double(clipRect.maxY)
        let clipY1 = viewH - Double(clipRect.minY)
        var json: UnsafeMutablePointer<CChar>?
        if !showGrid {
            json = glaspen2_grid_json_clipped(viewW, viewH, panX, panY, zoom,
                                              gridSize, Int32(divider), 0,
                                              0, 0, viewW, viewH)
        } else if clipX0 <= 0 && clipY0 <= 0 && clipX1 >= viewW && clipY1 >= viewH {
            json = glaspen2_grid_json(viewW, viewH, panX, panY, zoom, gridSize,
                                      Int32(divider), follow ? 1 : 0)
        } else {
            json = glaspen2_grid_json_clipped(viewW, viewH, panX, panY, zoom,
                                              gridSize, Int32(divider), 1,
                                              clipX0, clipY0, clipX1, clipY1)
        }
        guard let json else { return }
        let data = Data(bytes: json, count: strlen(json))
        glaspen2_free_c_string(json)
        guard let parsed = try? JSONSerialization.jsonObject(with: data),
              let dict = parsed as? [String: Any], dict["error"] == nil else { return }
        func segs(_ key: String) -> NSArray {
            (dict[key] as? [Any] ?? []) as NSArray
        }
        strokeLayer(ctx, segments: segs("minor"), viewHeight: viewH,
                    white: 0.5, alpha: 0.15, lineWidth: 0.5)
        strokeLayer(ctx, segments: segs("screen"), viewHeight: viewH,
                    white: 0.5, alpha: 0.55, lineWidth: 1.0)
        strokeLayer(ctx, segments: segs("divider"), viewHeight: viewH,
                    white: 0.5, alpha: 0.65, lineWidth: 1.5)
    }
}
// S7: drawRect 光标十字层(白/红圈 + 中心点 + 四向线), 与原 ObjC 逐值一致。
import AppKit
import CoreGraphics

@objc public final class OverlayCursorRenderer: NSObject {
    @objc(drawCursorWithContext:)
    public static func drawCursor(_ ctx: CGContext) {
        guard CursorState.visible, CursorState.x >= 0 else { return }
        let x = CursorState.x
        let y = CursorState.y
        let eraser = glaspen2_router_eraser_mode() != 0
        let radius = 8.0
        let ring = eraser ? NSColor(calibratedRed: 1.0, green: 0.25, blue: 0.2, alpha: 0.95)
                          : NSColor(white: 1.0, alpha: 0.8)
        ctx.setStrokeColor(ring.cgColor)
        ctx.setLineWidth(1.5)
        ctx.strokeEllipse(in: CGRect(x: x - radius, y: y - radius,
                                     width: radius * 2, height: radius * 2))
        let dot = eraser ? NSColor(calibratedRed: 1.0, green: 0.25, blue: 0.2, alpha: 0.95)
                         : NSColor(white: 1.0, alpha: 0.9)
        ctx.setFillColor(dot.cgColor)
        ctx.fillEllipse(in: CGRect(x: x - 1.5, y: y - 1.5, width: 3, height: 3))
        let gap = 3.0
        ctx.setStrokeColor(NSColor(white: 0, alpha: 0.5).cgColor)
        ctx.setLineWidth(1.0)
        ctx.move(to: CGPoint(x: x, y: y - radius - 2))
        ctx.addLine(to: CGPoint(x: x, y: y - gap))
        ctx.move(to: CGPoint(x: x, y: y + gap))
        ctx.addLine(to: CGPoint(x: x, y: y + radius + 2))
        ctx.move(to: CGPoint(x: x - radius - 2, y: y))
        ctx.addLine(to: CGPoint(x: x - gap, y: y))
        ctx.move(to: CGPoint(x: x + gap, y: y))
        ctx.addLine(to: CGPoint(x: x + radius + 2, y: y))
        ctx.strokePath()
    }
}

// S7: 笔迹 surface 合成: 小脏矩形裁子图(视图点→物理像素, Y 翻转), 失败回退全图。
@objc public final class OverlaySurfaceRenderer: NSObject {
    @objc(drawSurfaceImage:inContext:rect:bounds:scale:)
    public static func drawSurface(_ image: CGImage, in ctx: CGContext,
                                   rect: NSRect, bounds: NSRect, scale: Double) {
        let smallDirty = !NSEqualRects(rect, bounds)
            && (rect.size.width < bounds.size.width
                || rect.size.height < bounds.size.height)
        if smallDirty {
            let sx = rect.origin.x * scale
            let sy = (bounds.size.height - (rect.origin.y + rect.size.height)) * scale
            let phys = CGRect(x: sx, y: sy,
                              width: rect.size.width * scale,
                              height: rect.size.height * scale)
            if let sub = image.cropping(to: phys) {
                ctx.draw(sub, in: rect)
            } else {
                ctx.draw(image, in: CGRect(origin: .zero, size: bounds.size))
            }
        } else {
            ctx.draw(image, in: CGRect(origin: .zero, size: bounds.size))
        }
    }
}

// S7: 笔迹 surface → CGImage 缓存(Swift 持有;表面地址/尺寸/stride 变化才重建)。
@objc public final class OverlaySurfaceImageCache: NSObject {
    private static var cached: CGImage?
    private static var dataPtr: UnsafeMutableRawPointer?
    private static var width = 0, height = 0, stride = 0

    @objc(imageForData:width:height:stride:)
    public static func image(data: UnsafeMutableRawPointer, width: Int32,
                             height: Int32, stride: Int32) -> CGImage? {
        if cached != nil, dataPtr == data, self.width == Int(width),
           self.height == Int(height), self.stride == Int(stride) {
            return cached
        }
        let bytes = data.assumingMemoryBound(to: UInt8.self)
        // 表面生命周期由 ObjC/cairo 负责, 这里不释放底层像素。
        let release: CGDataProviderReleaseDataCallback = { _, _, _ in }
        guard let provider = CGDataProvider(dataInfo: nil, data: bytes,
                                           size: Int(height) * Int(stride),
                                           releaseData: release),
              let space = CGColorSpace(name: CGColorSpace.sRGB) else {
            cached = nil
            return nil
        }
        let info = CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedFirst.rawValue
                                | CGBitmapInfo.byteOrder32Little.rawValue)
        let image = CGImage(width: Int(width), height: Int(height),
                            bitsPerComponent: 8, bitsPerPixel: 32,
                            bytesPerRow: Int(stride), space: space,
                            bitmapInfo: info, provider: provider,
                            decode: nil, shouldInterpolate: false,
                            intent: .defaultIntent)
        cached = image
        dataPtr = data
        self.width = Int(width)
        self.height = Int(height)
        self.stride = Int(stride)
        return image
    }

    @objc public static func clear() {
        cached = nil
        dataPtr = nil
        width = 0; height = 0; stride = 0
    }
}

// S7: 时光隧道半分辨率 surface 缓存 + 全屏绘制(Swift 持有缓存)。
@objc public final class OverlayTunnelRenderer: NSObject {
    private static var cached: CGImage?
    private static var dataPtr: UnsafeMutableRawPointer?
    private static var width = 0, height = 0, stride = 0

    @objc(drawTunnelWithContext:data:width:height:stride:bounds:)
    public static func drawTunnel(_ ctx: CGContext, data: UnsafeMutableRawPointer,
                                  width: Int32, height: Int32, stride: Int32,
                                  bounds: NSRect) {
        let image = cachedImage(data: data, width: width, height: height, stride: stride)
        guard let image else { return }
        ctx.draw(image, in: CGRect(origin: .zero, size: bounds.size))
    }

    @objc public static func clear() {
        cached = nil
        dataPtr = nil
        width = 0; height = 0; stride = 0
    }

    private static func cachedImage(data: UnsafeMutableRawPointer, width: Int32,
                                    height: Int32, stride: Int32) -> CGImage? {
        if cached != nil, dataPtr == data, self.width == Int(width),
           self.height == Int(height), self.stride == Int(stride) {
            return cached
        }
        let bytes = data.assumingMemoryBound(to: UInt8.self)
        let release: CGDataProviderReleaseDataCallback = { _, _, _ in }
        guard let provider = CGDataProvider(dataInfo: nil, data: bytes,
                                           size: Int(height) * Int(stride),
                                           releaseData: release),
              let space = CGColorSpace(name: CGColorSpace.sRGB) else {
            cached = nil
            return nil
        }
        let info = CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedFirst.rawValue
                                | CGBitmapInfo.byteOrder32Little.rawValue)
        let image = CGImage(width: Int(width), height: Int(height),
                            bitsPerComponent: 8, bitsPerPixel: 32,
                            bytesPerRow: Int(stride), space: space,
                            bitmapInfo: info, provider: provider,
                            decode: nil, shouldInterpolate: false,
                            intent: .defaultIntent)
        cached = image
        dataPtr = data
        self.width = Int(width)
        self.height = Int(height)
        self.stride = Int(stride)
        return image
    }
}
