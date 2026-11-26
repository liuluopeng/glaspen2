// S7: 网格/分栏线段绘制(样式与旧 ObjC 逐值一致)。几何仍由 core::grid 产出。
import AppKit
import CoreGraphics

@objc public final class OverlayGridRenderer: NSObject {
    /// drawRect 网格层: 门控 + pan/zoom 换算 + core::grid 线段绘制(全部在 Rust)。
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
            panX = glaspen2_router_pan_x()
            panY = glaspen2_router_pan_y()
            let z = glaspen2_router_zoom()
            zoom = z > 0.05 ? z : 0.05
        } else {
            panX = -glaspen2_router_page_off_x()
            panY = -glaspen2_router_page_off_y()
        }
        let gridSize = glaspen2_router_grid_size()
        let clipX0 = Double(clipRect.minX), clipX1 = Double(clipRect.maxX)
        let clipY0 = viewH - Double(clipRect.maxY)
        let clipY1 = viewH - Double(clipRect.minY)
        _ = glaspen2_rust_draw_grid(
            Unmanaged.passUnretained(ctx).toOpaque(), viewW, viewH,
            panX, panY, zoom, gridSize, Int32(divider), showGrid ? 1 : 0,
            glaspen2_router_screen_w(), glaspen2_router_screen_h(),
            clipX0, clipY0, clipX1, clipY1)
    }
}
// S7: drawRect 光标十字层(白/红圈 + 中心点 + 四向线), 与原 ObjC 逐值一致。
import AppKit
import CoreGraphics

@objc public final class OverlayCursorRenderer: NSObject {
    @objc(drawCursorWithContext:)
    public static func drawCursor(_ ctx: CGContext) {
        glaspen2_rust_draw_cursor(Unmanaged.passUnretained(ctx).toOpaque(),
                                  glaspen2_router_pen_cursor_visible() != 0 ? 1 : 0,
                                  glaspen2_router_cursor_x(), glaspen2_router_cursor_y(),
                                  glaspen2_router_eraser_mode())
    }
}

// S7: 笔迹 surface 合成: 小脏矩形裁子图(视图点→物理像素, Y 翻转), 失败回退全图。
// S7: 时光隧道半分辨率 surface 缓存 + 全屏绘制(Swift 持有缓存)。
@objc public final class OverlayTunnelRenderer: NSObject {
    @objc(drawTunnelWithContext:data:width:height:stride:bounds:)
    public static func drawTunnel(_ ctx: CGContext, data: UnsafeMutableRawPointer,
                                  width: Int32, height: Int32, stride: Int32,
                                  bounds: NSRect) {
        _ = glaspen2_rust_draw_tunnel(
            Unmanaged.passUnretained(ctx).toOpaque(), data, width, height, stride,
            bounds.size.width, bounds.size.height)
    }

    @objc public static func clear() {
        glaspen2_rust_clear_tunnel_cache()
    }
}
