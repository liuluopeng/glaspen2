// S7: 笔迹首点实时绘制(计划优先, FFI 失败回落旧分档自绘)。
import AppKit

@objc public final class RawDrawing: NSObject {
    @objc(drawDotWithX:y:width:)
    public static func drawDot(x: Double, y: Double, width: Double) {
        guard SurfaceStore.current != nil else { return }
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

    /// 拖动实时段: 计划优先, 失败回落旧逐段 pass(阴影/虚线描边/接缝重画/反色)。
    @objc(drawSegmentWithX:y:width:)
    public static func drawSegment(x: Double, y: Double, width: Double) {
        guard SurfaceStore.current != nil else { return }
        var owned: Int32 = 0
        guard let cr = glaspen2_router_acquire_cr(&owned) else { return }
        let st = glaspen2_router_raw_state() as NSDictionary
        func num(_ key: String) -> Double { (st[key] as? NSNumber)?.doubleValue ?? 0 }
        func flag(_ key: String) -> Bool { (st[key] as? NSNumber)?.boolValue ?? false }
        let hasLast = flag("has_last")
        let hasPrev = flag("has_prev")
        let lastX = num("last_x"), lastY = num("last_y")
        let lastW = num("last_w"), pathLen = num("path_len")
        let planned = glaspen2_router_live_plan_segment(cr, x, y, width) != 0
        if !planned {
            let eraser = glaspen2_router_eraser_mode() != 0
            cairo_set_operator(cr, cairo_operator_t(eraser ? 3 : 2))
            cairo_set_line_cap(cr, cairo_line_cap_t(1))
            cairo_set_line_join(cr, cairo_line_join_t(1))
            let shadow = glaspen2_router_soft_shadow() != 0
            if shadow && !eraser {
                let steps: [(Double, Double)] = [(1.5, 0.20), (3.0, 0.13), (5.0, 0.07)]
                for (sp, alpha) in steps {
                    cairo_set_source_rgba(cr, 0, 0, 0, alpha)
                    cairo_set_line_width(cr, width + 2.0 * sp)
                    if hasLast {
                        cairo_move_to(cr, lastX, lastY)
                        cairo_line_to(cr, x, y)
                        cairo_stroke(cr)
                    } else {
                        cairo_arc(cr, x, y, width * 0.5 + sp, 0, 2 * Double.pi)
                        cairo_fill(cr)
                    }
                }
            }
            let outline = glaspen2_router_outline_enabled() != 0
            let padConst = glaspen2_router_outline_pad()
            let dash = glaspen2_router_outline_dash()
            if outline && !eraser {
                var off = pathLen.truncatingRemainder(dividingBy: 2.0 * dash)
                cairo_set_line_cap(cr, cairo_line_cap_t(2))
                cairo_set_line_width(cr, width + 2.0 * padConst)
                if hasLast {
                    let dashes = [dash, dash]
                    dashes.withUnsafeBufferPointer {
                        cairo_set_dash(cr, $0.baseAddress, Int32($0.count), off)
                    }
                    cairo_set_source_rgba(cr, 0, 0, 0, 1)
                    cairo_move_to(cr, lastX, lastY)
                    cairo_line_to(cr, x, y)
                    cairo_stroke(cr)
                    off = (off + dash).truncatingRemainder(dividingBy: 2.0 * dash)
                    cairo_set_dash(cr, nil, 0, 0)
                    cairo_set_source_rgba(cr, 1, 1, 1, 1)
                    cairo_move_to(cr, lastX, lastY)
                    cairo_line_to(cr, x, y)
                    cairo_stroke(cr)
                    cairo_set_dash(cr, nil, 0, 0)
                } else {
                    cairo_set_dash(cr, nil, 0, 0)
                    cairo_set_source_rgba(cr, 0, 0, 0, 1)
                    cairo_arc(cr, x, y, width * 0.5 + padConst, 0, 2 * Double.pi)
                    cairo_fill(cr)
                }
                cairo_set_line_cap(cr, cairo_line_cap_t(1))
            }
            let ipat = glaspen2_router_invert_ink_source(cr)
            if ipat == nil {
                cairo_set_source_rgba(cr, glaspen2_router_pen_r(),
                                      glaspen2_router_pen_g(),
                                      glaspen2_router_pen_b(), 1.0)
            }
            cairo_set_line_width(cr, width)
            if hasLast {
                cairo_move_to(cr, lastX, lastY)
                cairo_line_to(cr, x, y)
                cairo_stroke(cr)
            } else {
                cairo_arc(cr, x, y, width * 0.5, 0, 2 * Double.pi)
                cairo_fill(cr)
            }
            if let ipat { cairo_pattern_destroy(ipat) }
            if outline && !eraser && hasPrev {
                cairo_set_source_rgba(cr, glaspen2_router_pen_r(),
                                      glaspen2_router_pen_g(),
                                      glaspen2_router_pen_b(), 1.0)
                cairo_set_line_width(cr, lastW)
                cairo_move_to(cr, num("prev_x"), num("prev_y"))
                cairo_line_to(cr, lastX, lastY)
                cairo_stroke(cr)
            }
            let pad = width * 0.5 + 1.5 + (shadow ? 5.0 : 0.0)
            if hasLast {
                glaspen2_router_dirty_include_surface_point(lastX, lastY, pad)
            }
            glaspen2_router_dirty_include_surface_point(x, y, pad)
        }
        if owned != 0 { cairo_destroy(cr) }
        glaspen2_router_update_raw_after_segment(x, y, width)
        OverlayLayerFlush.flushDirty()
    }
}
