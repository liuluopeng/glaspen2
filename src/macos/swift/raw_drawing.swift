// S7: 笔迹首点实时绘制(计划优先, FFI 失败回落旧分档自绘)。
import AppKit

@objc public final class RawDrawing: NSObject {
    @objc(drawDotWithX:y:width:)
    public static func drawDot(x: Double, y: Double, width: Double) {
        guard glaspen2_router_surface() != nil else { return }
        var owned: Int32 = 0
        guard let cr = glaspen2_router_acquire_cr(&owned) else { return }
        let planned = glaspen2_router_live_plan_dot(cr, x, y, width) != 0
        if !planned {
            let eraser = glaspen2_router_eraser_mode() != 0
            cairo_set_operator(cr, cairo_operator_t(eraser ? 3 : 2))
            let shadow = glaspen2_router_soft_shadow() != 0
            if shadow && !eraser {
                let steps: [(Double, Double)] = [(1.5, 0.20), (3.0, 0.13), (5.0, 0.07)]
                for (pad, alpha) in steps {
                    cairo_set_source_rgba(cr, 0, 0, 0, alpha)
                    cairo_arc(cr, x, y, width * 0.5 + pad, 0, 2 * Double.pi)
                    cairo_fill(cr)
                }
            }
            if glaspen2_router_outline_enabled() != 0 && !eraser {
                cairo_set_source_rgba(cr, 0, 0, 0, 1)
                cairo_arc(cr, x, y, width * 0.5 + glaspen2_router_outline_pad(),
                          0, 2 * Double.pi)
                cairo_fill(cr)
            }
            let ipat = glaspen2_router_invert_ink_source(cr)
            if ipat == nil {
                cairo_set_source_rgba(cr, glaspen2_router_pen_r(),
                                      glaspen2_router_pen_g(),
                                      glaspen2_router_pen_b(), 1.0)
            }
            cairo_arc(cr, x, y, width * 0.5, 0, 2 * Double.pi)
            cairo_fill(cr)
            if let ipat { cairo_pattern_destroy(ipat) }
            let pad = width * 0.5 + 1.5 + (shadow ? 5.0 : 0.0)
            glaspen2_router_dirty_include_surface_point(x, y, pad)
        }
        if owned != 0 { cairo_destroy(cr) }
        OverlayLayerFlush.flushDirty()
    }
}
