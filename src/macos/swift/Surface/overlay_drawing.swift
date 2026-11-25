// S7: drawRect 编排(裁剪/隧道/网格/笔迹/缩略图/通知/光标/性能打点), 状态均在 Swift。
import AppKit
import Darwin

@objc public final class OverlayDrawing: NSObject {
    @objc(drawInView:rect:)
    public static func draw(in view: NSView, rect: NSRect) {
        if let surf = SurfaceStore.current { cairo_surface_flush(surf) }
        if let tun = TunnelState.surface { cairo_surface_flush(tun) }
        guard SurfaceStore.current != nil, let gctx = NSGraphicsContext.current else { return }
        let t0 = mach_absolute_time()
        let ctx = gctx.cgContext
        ctx.saveGState()
        let clipRect = rect
        ctx.clip(to: clipRect)
        let bounds = view.bounds

        if TunnelState.active, let tun = TunnelState.surface {
            if let data = cairo_image_surface_get_data(tun) {
                OverlayTunnelRenderer.drawTunnel(ctx, data: data,
                    width: cairo_image_surface_get_width(tun),
                    height: cairo_image_surface_get_height(tun),
                    stride: cairo_image_surface_get_stride(tun),
                    bounds: bounds)
            }
            ctx.restoreGState()
            return
        }

        OverlayGridRenderer.drawGrid(ctx, bounds: bounds, clipRect: clipRect)

        if let surf = SurfaceStore.current, let sp = cairo_image_surface_get_data(surf) {
            let w = cairo_image_surface_get_width(surf)
            let h = cairo_image_surface_get_height(surf)
            let stride = cairo_image_surface_get_stride(surf)
            if w > 0 && h > 0 {
                let image = OverlaySurfaceImageCache.image(data: sp, width: w,
                                                           height: h, stride: stride)
                if let image, glaspen2_router_strokes_visible() != 0 {
                    OverlaySurfaceRenderer.drawSurface(image, in: ctx, rect: rect,
                                                       bounds: bounds,
                                                       scale: SurfaceStore.scaleFactor)
                }
            }
        }

        let infinite = glaspen2_router_infinite() != 0
        let minimap = glaspen2_router_minimap_enabled() != 0
        if minimap && !infinite && rect.maxX >= bounds.size.width - 96.0 {
            MinimapRenderer.draw(ctx, bounds: bounds)
        }
        NotificationPresenter.draw(in: bounds)
        OverlayCursorRenderer.drawCursor(ctx)
        ctx.restoreGState()

        if glaspen2_router_perf_enabled() != 0 {
            var notes = String(format: "rect=%.0fx%.0f", rect.size.width, rect.size.height)
            if rect == bounds { notes += " FULL" }
            if glaspen2_router_show_grid() != 0 { notes += " grid" }
            if minimap && !infinite { notes += " minimap" }
            notes.withCString { glaspen2_router_perf_notes("drawrect", $0, t0) }
        }
    }
}
