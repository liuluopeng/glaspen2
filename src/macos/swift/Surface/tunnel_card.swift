// S7: 时光隧道卡片(玻璃厚度/阴影/前脸/扫光/棱线)绘制。
import AppKit
import Darwin

@objc public final class TunnelCardRenderer: NSObject {
    @objc(drawWithCR:surf:nw:nh:cx:cy:w:h:alpha:zc:surfW:surfH:thick:paneA:)
    public static func draw(crRaw: UnsafeMutableRawPointer, surfRaw: UnsafeMutableRawPointer,
                            nw: Int32, nh: Int32, cx: Double, cy: Double,
                            w: Double, h: Double, alpha: Double, zc: Double,
                            surfW: Int32, surfH: Int32, thick: Double, paneA: Double) {
        guard alpha > 0.02, w >= 1.0, h >= 1.0, nw > 0, nh > 0 else { return }
        let cr = OpaquePointer(crRaw)
        let surf = OpaquePointer(surfRaw)
        let cardX = cx - w / 2.0, cardY = cy - h / 2.0

        if abs(zc) < 0.005 {
            cairo_save(cr)
            cairo_rectangle(cr, 0, 0, Double(surfW), Double(surfH))
            cairo_clip(cr)
            cairo_translate(cr, cardX, cardY)
            cairo_scale(cr, w / Double(nw), h / Double(nh))
            cairo_set_source_surface(cr, surf, 0, 0)
            cairo_paint_with_alpha(cr, 1.0)
            cairo_restore(cr)
            return
        }

        var glass = abs(zc) / 0.10
        if glass > 1.0 { glass = 1.0 }
        let thickness = max(4.0, w * thick)
        let soff = max(3.0, w * 0.008)
        var zpos = zc > 0 ? zc : 0
        if zpos > 4.0 { zpos = 4.0 }
        var pane = (paneA + 0.02 * zpos) * glass
        if zc < 0 { pane *= alpha }
        if pane > 0.94 { pane = 0.94 }
        let T = thickness

        if abs(zc) < 1.6 {
            for i in stride(from: 2, through: 1, by: -1) {
                let off = Double(i) * soff * 1.6
                cairo_set_source_rgba(cr, 0, 0, 0, 0.028 * alpha * glass)
                cairo_rectangle(cr, cardX + off, cardY + off, w, h)
                cairo_fill(cr)
            }
        }

        // 上侧面
        if let top = cairo_pattern_create_linear(cardX, cardY, cardX + T, cardY - T) {
            cairo_pattern_add_color_stop_rgba(top, 0, 1.00, 1.00, 1.00, 0.74 * alpha * glass)
            cairo_pattern_add_color_stop_rgba(top, 1, 0.90, 0.91, 0.92, 0.52 * alpha * glass)
            cairo_set_source(cr, top)
            cairo_move_to(cr, cardX, cardY)
            cairo_line_to(cr, cardX + T, cardY - T)
            cairo_line_to(cr, cardX + w + T, cardY - T)
            cairo_line_to(cr, cardX + w, cardY)
            cairo_close_path(cr)
            cairo_fill(cr)
            cairo_pattern_destroy(top)
        }
        // 右侧面
        if let right = cairo_pattern_create_linear(cardX + w, cardY, cardX + w + T, cardY - T) {
            cairo_pattern_add_color_stop_rgba(right, 0, 0.70, 0.72, 0.74, 0.62 * alpha * glass)
            cairo_pattern_add_color_stop_rgba(right, 1, 0.28, 0.30, 0.33, 0.74 * alpha * glass)
            cairo_set_source(cr, right)
            cairo_move_to(cr, cardX + w, cardY)
            cairo_line_to(cr, cardX + w + T, cardY - T)
            cairo_line_to(cr, cardX + w + T, cardY + h - T)
            cairo_line_to(cr, cardX + w, cardY + h)
            cairo_close_path(cr)
            cairo_fill(cr)
            cairo_pattern_destroy(right)
        }
        // 前脸
        cairo_save(cr)
        cairo_rectangle(cr, cardX, cardY, w, h)
        cairo_clip(cr)
        if let fg = cairo_pattern_create_linear(cardX, cardY, cardX, cardY + h) {
            cairo_pattern_add_color_stop_rgba(fg, 0, 0.985, 0.99, 1.0, pane * 0.99)
            cairo_pattern_add_color_stop_rgba(fg, 0.55, 0.86, 0.875, 0.89, pane * 0.94)
            cairo_pattern_add_color_stop_rgba(fg, 1, 0.66, 0.675, 0.69, pane * 0.98)
            cairo_set_source(cr, fg)
            cairo_paint(cr)
            cairo_pattern_destroy(fg)
        }
        cairo_save(cr)
        cairo_translate(cr, cardX, cardY)
        cairo_scale(cr, w / Double(nw), h / Double(nh))
        cairo_set_source_surface(cr, surf, 0, 0)
        cairo_paint_with_alpha(cr, alpha)
        cairo_restore(cr)
        // 扫光
        if abs(zc) < 1.2, let sheen = cairo_pattern_create_linear(cardX, cardY, cardX + w, cardY + h * 0.75) {
            cairo_pattern_add_color_stop_rgba(sheen, 0, 1, 1, 1, 0.10 * alpha * glass)
            cairo_pattern_add_color_stop_rgba(sheen, 0.35, 1, 1, 1, 0.012 * alpha * glass)
            cairo_pattern_add_color_stop_rgba(sheen, 1, 1, 1, 1, 0.0)
            cairo_set_source(cr, sheen)
            cairo_paint(cr)
            cairo_pattern_destroy(sheen)
        }
        cairo_restore(cr)
        // 棱线
        cairo_save(cr)
        cairo_set_line_width(cr, max(1.0, T * 0.08))
        cairo_set_source_rgba(cr, 1, 1, 1, 0.42 * alpha * glass)
        cairo_move_to(cr, cardX + T, cardY - T)
        cairo_line_to(cr, cardX + w + T, cardY - T)
        cairo_stroke(cr)
        cairo_set_source_rgba(cr, 0.03, 0.03, 0.035, 0.48 * alpha * glass)
        cairo_move_to(cr, cardX + w + T, cardY - T)
        cairo_line_to(cr, cardX + w + T, cardY + h - T)
        cairo_stroke(cr)
        cairo_set_source_rgba(cr, 1, 1, 1, 0.50 * alpha * glass)
        cairo_move_to(cr, cardX, cardY)
        cairo_line_to(cr, cardX + w, cardY)
        cairo_stroke(cr)
        cairo_set_source_rgba(cr, 0.04, 0.04, 0.045, 0.38 * alpha * glass)
        cairo_move_to(cr, cardX + w, cardY)
        cairo_line_to(cr, cardX + w, cardY + h)
        cairo_stroke(cr)
        cairo_set_source_rgba(cr, 0.9, 0.92, 0.94, 0.18 * alpha * glass)
        cairo_move_to(cr, cardX, cardY)
        cairo_line_to(cr, cardX, cardY + h)
        cairo_stroke(cr)
        cairo_set_source_rgba(cr, 0.05, 0.05, 0.055, 0.20 * alpha * glass)
        cairo_move_to(cr, cardX, cardY + h)
        cairo_line_to(cr, cardX + w, cardY + h)
        cairo_stroke(cr)
        cairo_restore(cr)
    }
}
