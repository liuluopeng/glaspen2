// S7/R38: drawRect 编排(裁剪/隧道/网格/笔迹/缩略图/通知/光标/性能打点), 状态均在 Swift。
import AppKit
import Darwin

@objc public final class OverlayDrawing: NSObject {
    @objc(drawInView:rect:)
    public static func draw(in view: NSView, rect: NSRect) {
        // R39: 默认 Rust 编排; GLASPEN2_SWIFT_OVERLAY_DRAW=1 回退 Swift。
        if ProcessInfo.processInfo.environment["GLASPEN2_SWIFT_OVERLAY_DRAW"] != "1",
           glaspen2_rust_overlay_draw(
               Unmanaged.passUnretained(view).toOpaque(),
               rect.origin.x, rect.origin.y, rect.size.width, rect.size.height) != 0 {
            return
        }
        let t0 = mach_absolute_time()
        guard let ctx = prepareContext() else { return }
        ctx.saveGState()
        ctx.clip(to: rect)
        let bounds = view.bounds
        if drawTunnel(ctx, bounds: bounds) {
            ctx.restoreGState()
            return
        }
        drawGrid(ctx, bounds: bounds, clipRect: rect)
        drawSurface(ctx, rect: rect, bounds: bounds)
        drawMinimap(ctx, bounds: bounds)
        drawNotification(bounds: bounds)
        drawCursor(ctx)
        ctx.restoreGState()
        writePerf(rect: rect, bounds: bounds, t0: t0)
    }

    static func prepareContext() -> CGContext? {
        if let surf = SurfaceStore.current { cairo_surface_flush(surf) }
        if let tun = TunnelState.surface { cairo_surface_flush(tun) }
        guard SurfaceStore.current != nil, let gctx = NSGraphicsContext.current else { return nil }
        return gctx.cgContext
    }

    @discardableResult
    static func drawTunnel(_ ctx: CGContext, bounds: NSRect) -> Bool {
        guard TunnelState.active, let tun = TunnelState.surface else { return false }
        if let data = cairo_image_surface_get_data(tun) {
            OverlayTunnelRenderer.drawTunnel(ctx, data: data,
                width: cairo_image_surface_get_width(tun),
                height: cairo_image_surface_get_height(tun),
                stride: cairo_image_surface_get_stride(tun),
                bounds: bounds)
        }
        return true
    }

    static func drawGrid(_ ctx: CGContext, bounds: NSRect, clipRect: NSRect) {
        OverlayGridRenderer.drawGrid(ctx, bounds: bounds, clipRect: clipRect)
    }

    static func drawSurface(_ ctx: CGContext, rect: NSRect, bounds: NSRect) {
        guard let surf = SurfaceStore.current,
              let sp = cairo_image_surface_get_data(surf) else { return }
        let w = cairo_image_surface_get_width(surf)
        let h = cairo_image_surface_get_height(surf)
        let stride = cairo_image_surface_get_stride(surf)
        if w > 0 && h > 0 && glaspen2_router_strokes_visible() != 0 {
            if ProcessInfo.processInfo.environment["GLASPEN2_SWIFT_SURFACE"] != "1",
               glaspen2_rust_draw_surface_data(
                   Unmanaged.passUnretained(ctx).toOpaque(), sp, w, h, stride,
                   rect.origin.x, rect.origin.y, rect.size.width, rect.size.height,
                   bounds.origin.x, bounds.origin.y, bounds.size.width, bounds.size.height,
                   SurfaceStore.scaleFactor) != 0 {
                return
            }
            let image = OverlaySurfaceImageCache.image(data: sp, width: w, height: h, stride: stride)
            if let image {
                OverlaySurfaceRenderer.drawSurface(image, in: ctx, rect: rect,
                                                   bounds: bounds, scale: SurfaceStore.scaleFactor)
            }
        }
    }

    static func drawMinimap(_ ctx: CGContext, bounds: NSRect) {
        let infinite = glaspen2_router_infinite() != 0
        let minimap = glaspen2_router_minimap_enabled() != 0
        if minimap && !infinite && bounds.maxX >= bounds.size.width - 96.0 {
            MinimapRenderer.draw(ctx, bounds: bounds)
        }
    }

    static func drawNotification(bounds: NSRect) {
        NotificationPresenter.draw(in: bounds)
    }

    static func drawCursor(_ ctx: CGContext) {
        OverlayCursorRenderer.drawCursor(ctx)
    }

    static func writePerf(rect: NSRect, bounds: NSRect, t0: UInt64) {
        if glaspen2_router_perf_enabled() != 0 {
            let infinite = glaspen2_router_infinite() != 0
            let minimap = glaspen2_router_minimap_enabled() != 0
            var notes = String(format: "rect=%.0fx%.0f", rect.size.width, rect.size.height)
            if rect == bounds { notes += " FULL" }
            if glaspen2_router_show_grid() != 0 { notes += " grid" }
            if minimap && !infinite { notes += " minimap" }
            notes.withCString { glaspen2_router_perf_notes("drawrect", $0, t0) }
        }
    }
}

// R38: Rust 编排覆盖层绘制时调用这些 stage 桥(各 stage 仍是原 Swift 实现)。
@_cdecl("glaspen2_swift_overlay_prepare")
public func glaspen2SwiftOverlayPrepare() -> Int32 {
    OverlayDrawing.prepareContext() != nil ? 1 : 0
}

@_cdecl("glaspen2_swift_overlay_tunnel")
public func glaspen2SwiftOverlayTunnel(_ ctx: UnsafeMutableRawPointer?,
                                       _ bx: Double, _ by: Double,
                                       _ bw: Double, _ bh: Double) -> Int32 {
    guard let ctx else { return 0 }
    let c = Unmanaged<CGContext>.fromOpaque(ctx).takeUnretainedValue()
    return OverlayDrawing.drawTunnel(c, bounds: NSRect(x: bx, y: by, width: bw, height: bh)) ? 1 : 0
}

@_cdecl("glaspen2_swift_overlay_grid")
public func glaspen2SwiftOverlayGrid(_ ctx: UnsafeMutableRawPointer?,
                                     _ bx: Double, _ by: Double, _ bw: Double, _ bh: Double,
                                     _ rx: Double, _ ry: Double, _ rw: Double, _ rh: Double) {
    guard let ctx else { return }
    let c = Unmanaged<CGContext>.fromOpaque(ctx).takeUnretainedValue()
    OverlayDrawing.drawGrid(c, bounds: NSRect(x: bx, y: by, width: bw, height: bh),
                            clipRect: NSRect(x: rx, y: ry, width: rw, height: rh))
}

@_cdecl("glaspen2_swift_overlay_surface")
public func glaspen2SwiftOverlaySurface(_ ctx: UnsafeMutableRawPointer?,
                                        _ bx: Double, _ by: Double, _ bw: Double, _ bh: Double,
                                        _ rx: Double, _ ry: Double, _ rw: Double, _ rh: Double) {
    guard let ctx else { return }
    let c = Unmanaged<CGContext>.fromOpaque(ctx).takeUnretainedValue()
    OverlayDrawing.drawSurface(c, rect: NSRect(x: rx, y: ry, width: rw, height: rh),
                               bounds: NSRect(x: bx, y: by, width: bw, height: bh))
}

@_cdecl("glaspen2_swift_overlay_minimap")
public func glaspen2SwiftOverlayMinimap(_ ctx: UnsafeMutableRawPointer?,
                                        _ bx: Double, _ by: Double, _ bw: Double, _ bh: Double) {
    guard let ctx else { return }
    let c = Unmanaged<CGContext>.fromOpaque(ctx).takeUnretainedValue()
    OverlayDrawing.drawMinimap(c, bounds: NSRect(x: bx, y: by, width: bw, height: bh))
}

@_cdecl("glaspen2_swift_overlay_notification")
public func glaspen2SwiftOverlayNotification(_ bx: Double, _ by: Double, _ bw: Double, _ bh: Double) {
    OverlayDrawing.drawNotification(bounds: NSRect(x: bx, y: by, width: bw, height: bh))
}

@_cdecl("glaspen2_swift_overlay_cursor")
public func glaspen2SwiftOverlayCursor(_ ctx: UnsafeMutableRawPointer?) {
    guard let ctx else { return }
    let c = Unmanaged<CGContext>.fromOpaque(ctx).takeUnretainedValue()
    OverlayDrawing.drawCursor(c)
}

@_cdecl("glaspen2_swift_overlay_perf")
public func glaspen2SwiftOverlayPerf(_ t0: UInt64,
                                     _ bx: Double, _ by: Double, _ bw: Double, _ bh: Double,
                                     _ rx: Double, _ ry: Double, _ rw: Double, _ rh: Double) {
    OverlayDrawing.writePerf(rect: NSRect(x: rx, y: ry, width: rw, height: rh),
                             bounds: NSRect(x: bx, y: by, width: bw, height: bh), t0: t0)
}
