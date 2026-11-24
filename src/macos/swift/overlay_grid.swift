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
        let st = glaspen2_router_grid_state() as NSDictionary
        func num(_ key: String) -> NSNumber? { st[key] as? NSNumber }
        let showGrid = num("showGrid")?.boolValue ?? false
        let divider = num("divider")?.intValue ?? 0
        let tunActive = num("tunActive")?.boolValue ?? false
        let strokesVisible = num("strokesVisible")?.boolValue ?? true
        let follow = num("gridFollow")?.boolValue ?? false
        guard (showGrid || divider > 0) && !tunActive
            && (strokesVisible || !follow) else { return }
        let viewW = Double(bounds.size.width)
        let viewH = Double(bounds.size.height)
        guard viewW > 0, viewH > 0 else { return }
        var panX = 0.0, panY = 0.0, zoom = 1.0
        if num("infinite")?.boolValue ?? false {
            panX = num("panX")?.doubleValue ?? 0
            panY = num("panY")?.doubleValue ?? 0
            let z = num("zoom")?.doubleValue ?? 1
            zoom = z > 0.05 ? z : 0.05
        } else {
            panX = -(num("pageOffX")?.doubleValue ?? 0)
            panY = -(num("pageOffY")?.doubleValue ?? 0)
        }
        let gridSize = num("gridSize")?.doubleValue ?? 1
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
        let st = glaspen2_router_cursor_draw_state() as NSDictionary
        func num(_ key: String) -> NSNumber? { st[key] as? NSNumber }
        guard num("visible")?.boolValue ?? false,
              let x = num("x")?.doubleValue, x >= 0,
              let y = num("y")?.doubleValue else { return }
        let eraser = num("eraser")?.boolValue ?? false
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
