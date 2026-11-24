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
