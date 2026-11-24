// S7: drawRect 编排(裁剪/隧道/网格/笔迹/缩略图/通知/光标/性能打点), 各层实现均已在 Swift。
import AppKit
import Darwin

@objc public final class OverlayDrawing: NSObject {
    @objc(drawInView:rect:)
    public static func draw(in view: NSView, rect: NSRect) {
        glaspen2_router_draw_prepare()
        let st = glaspen2_router_draw_state() as NSDictionary
        func flag(_ key: String) -> Bool { (st[key] as? NSNumber)?.boolValue ?? false }
        func num(_ key: String) -> Double { (st[key] as? NSNumber)?.doubleValue ?? 0 }
        guard flag("hasSurface"), let gctx = NSGraphicsContext.current else { return }
        let t0 = mach_absolute_time()
        let ctx = gctx.cgContext
        ctx.saveGState()
        // 视图坐标与 dirty rect 同空间(非翻转/翻转目前等价), 与旧实现一致。
        let clipRect = rect
        ctx.clip(to: clipRect)
        let bounds = view.bounds

        if flag("tunnelActive"), let tv = st["tunnelData"] as? NSValue,
           let tp = tv.pointerValue, num("tunnelW") > 0, num("tunnelH") > 0 {
            OverlayTunnelRenderer.drawTunnel(ctx, data: tp,
                                            width: Int32(num("tunnelW")),
                                           height: Int32(num("tunnelH")),
                                           stride: Int32(num("tunnelStride")),
                                           bounds: bounds)
            ctx.restoreGState()
            return
        }

        OverlayGridRenderer.drawGrid(ctx, bounds: bounds, clipRect: clipRect)

        if let sv = st["surfaceData"] as? NSValue, let sp = sv.pointerValue,
           num("surfaceW") > 0, num("surfaceH") > 0 {
            let image = OverlaySurfaceImageCache.image(data: sp,
                                                             width: Int32(num("surfaceW")),
                                                            height: Int32(num("surfaceH")),
                                                            stride: Int32(num("surfaceStride")))
            if let image, flag("strokesVisible") {
                OverlaySurfaceRenderer.drawSurface(image, in: ctx, rect: rect,
                                                   bounds: bounds, scale: num("scale"))
            }
        }

        if flag("minimapEnabled") && !flag("infinite")
            && rect.maxX >= bounds.size.width - 96.0 {
            MinimapRenderer.draw(ctx, bounds: bounds)
        }
        NotificationPresenter.draw(in: bounds)
        OverlayCursorRenderer.drawCursor(ctx)
        ctx.restoreGState()

        if flag("perfEnabled") {
            var notes = String(format: "rect=%.0fx%.0f", rect.size.width, rect.size.height)
            if rect == bounds { notes += " FULL" }
            if flag("showGrid") { notes += " grid" }
            if flag("minimapEnabled") && !flag("infinite") { notes += " minimap" }
            notes.withCString { glaspen2_router_perf_notes("drawrect", $0, t0) }
        }
    }
}
