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
}
