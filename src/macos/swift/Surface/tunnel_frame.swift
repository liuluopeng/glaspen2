// S7: 时光隧道逐帧渲染(状态在 Swift; 排序/落定帧/模糊缓冲/近层)。
import AppKit
import Darwin

@objc public final class TunnelFrameRenderer: NSObject {
    @objc(renderWithP:)
    public static func render(p: Double) {
        guard let surface = TunnelState.surface, TunnelState.cards > 0 else { return }
        TunnelState.frames += 1
        let e = p < 0.5 ? 4.0 * p * p * p : 1.0 - pow(-2.0 * p + 2.0, 3.0) / 2.0
        let cam = e * TunnelState.travel * (TunnelState.back ? 1.0 : -1.0)
        let surf = surface
        guard let cr = cairo_create(surf) else { return }
        cairo_set_operator(cr, cairo_operator_t(1))
        cairo_set_source_rgba(cr, 0, 0, 0, 0)
        cairo_paint(cr)
        cairo_set_operator(cr, cairo_operator_t(2))

        let slots = Array(0..<TunnelState.cards)
        let order = slots.sorted { TunnelState.depths[$0] > TunnelState.depths[$1] }
        for slot in order {
            let zc = Double(TunnelState.depths[slot]) - cam
            if abs(zc) < 0.005, let placed = place(zc) {
                drawCard(cr, slot, placed, zc)
                cairo_destroy(cr)
                flush()
                return
            }
        }
        var focusZc = 1e9
        for slot in order {
            let zc = Double(TunnelState.depths[slot]) - cam
            if let placed = place(zc), placed.4 >= 0.30, zc < focusZc {
                focusZc = zc
            }
        }
        if focusZc > 1e8 { focusZc = 0.0 }

        if let blurPtr = TunnelState.blur {
            let blur = blurPtr
            let bw = cairo_image_surface_get_width(blur)
            let bh = cairo_image_surface_get_height(blur)
            if let bcr = cairo_create(blur) {
                cairo_set_operator(bcr, cairo_operator_t(1))
                cairo_set_source_rgba(bcr, 0, 0, 0, 0)
                cairo_paint(bcr)
                cairo_set_operator(bcr, cairo_operator_t(2))
                cairo_scale(bcr, Double(bw) / Double(TunnelState.surfW),
                            Double(bh) / Double(TunnelState.surfH))
                for slot in order {
                    let zc = Double(TunnelState.depths[slot]) - cam
                    if zc <= focusZc + 1e-6 { continue }
                    if let placed = place(zc) { drawCard(bcr, slot, placed, zc) }
                }
                cairo_destroy(bcr)
                cairo_save(cr)
                cairo_scale(cr, Double(TunnelState.surfW) / Double(bw),
                            Double(TunnelState.surfH) / Double(bh))
                cairo_set_source_surface(cr, blur, 0, 0)
                if let src = cairo_get_source(cr) {
                    cairo_pattern_set_filter(src, cairo_filter_t(2))
                }
                cairo_paint(cr)
                cairo_restore(cr)
            }
        }
        for slot in order {
            let zc = Double(TunnelState.depths[slot]) - cam
            if TunnelState.blur != nil && zc > focusZc + 1e-6 { continue }
            if let placed = place(zc) { drawCard(cr, slot, placed, zc) }
        }
        cairo_destroy(cr)
        flush()
    }

    private static func flush() {
        if let view = UIAnchors.drawView {
            view.needsDisplay = true
        }
    }

    private static func place(_ zc: Double) -> (Double, Double, Double, Double, Double)? {
        var out = [Double](repeating: 0, count: 5)
        let ok = out.withUnsafeMutableBufferPointer { buf in
            glaspen2_tunnel_place(zc, TunnelState.k, TunnelState.travel,
                                  Double(TunnelState.surfW), Double(TunnelState.surfH),
                                  buf.baseAddress)
        }
        guard ok != 0 else { return nil }
        return (out[0], out[1], out[2], out[3], out[4])
    }

    private static func drawCard(_ cr: OpaquePointer, _ slot: Int,
                                 _ placed: (Double, Double, Double, Double, Double),
                                 _ zc: Double) {
        guard let mainPtr = TunnelState.cardSurf[slot] else { return }
        var surf = mainPtr
        if let mipPtr = TunnelState.mipSurf[slot] {
            let mainW = Double(cairo_image_surface_get_width(mainPtr))
            if placed.2 < mainW * 0.70 { surf = mipPtr }
        }
        TunnelCardRenderer.draw(crRaw: UnsafeMutableRawPointer(cr),
                                surfRaw: UnsafeMutableRawPointer(surf),
                                nw: cairo_image_surface_get_width(surf),
                                nh: cairo_image_surface_get_height(surf),
                                cx: placed.0, cy: placed.1,
                                w: placed.2, h: placed.3,
                                alpha: placed.4, zc: zc,
                                surfW: Int32(TunnelState.surfW),
                                surfH: Int32(TunnelState.surfH),
                                thick: TunnelState.thick,
                                paneA: TunnelState.paneA)
    }
}
