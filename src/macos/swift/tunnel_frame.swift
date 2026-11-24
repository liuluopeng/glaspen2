// S7: 时光隧道逐帧渲染(排序/落定帧/近层清晰 + 远层 1/3 模糊缓冲)。
import AppKit
import Darwin

@objc public final class TunnelFrameRenderer: NSObject {
    @objc(renderWithP:state:)
    public static func render(p: Double, state: NSDictionary) {
        guard let surfaceValue = state["surface"] as? NSValue,
              let surface = surfaceValue.pointerValue else { return }
        let surf = OpaquePointer(surface)
        let cards = state["cards"] as? [[String: Any]] ?? []
        if cards.isEmpty { return }
        let surfW = (state["surfW"] as? NSNumber)?.int32Value ?? 0
        let surfH = (state["surfH"] as? NSNumber)?.int32Value ?? 0
        let travel = (state["travel"] as? NSNumber)?.doubleValue ?? 1.0
        let back = (state["back"] as? NSNumber)?.boolValue ?? false
        let thick = (state["thick"] as? NSNumber)?.doubleValue ?? 0.018
        let paneA = (state["paneA"] as? NSNumber)?.doubleValue ?? 0.84
        let e = p < 0.5 ? 4.0 * p * p * p : 1.0 - pow(-2.0 * p + 2.0, 3.0) / 2.0
        let cam = e * travel * (back ? 1.0 : -1.0)

        guard let cr = cairo_create(surf) else { return }
        cairo_set_operator(cr, cairo_operator_t(1)) // SOURCE
        cairo_set_source_rgba(cr, 0, 0, 0, 0)
        cairo_paint(cr)
        cairo_set_operator(cr, cairo_operator_t(2)) // OVER

        let order = (0..<cards.count).sorted {
            depth(cards[$0]) > depth(cards[$1])
        }
        // 落定帧: 只画 zc≈0 的卡
        for slot in order {
            let zc = Double(depth(cards[slot])) - cam
            if abs(zc) < 0.005, let placed = place(zc) {
                drawCard(cr, cards[slot], placed, zc, surfW: surfW, surfH: surfH,
                         thick: thick, paneA: paneA)
                cairo_destroy(cr)
                glaspen2_router_flush_tunnel()
                return
            }
        }
        // 焦点卡(最靠前的可见卡)
        var focusZc = 1e9
        for slot in order {
            let zc = Double(depth(cards[slot])) - cam
            if let placed = place(zc), placed.alpha >= 0.30, zc < focusZc {
                focusZc = zc
            }
        }
        if focusZc > 1e8 { focusZc = 0.0 }

        if let blurValue = state["blur"] as? NSValue, let blurPtr = blurValue.pointerValue {
            let blur = OpaquePointer(blurPtr)
            let bw = cairo_image_surface_get_width(blur)
            let bh = cairo_image_surface_get_height(blur)
            if let bcr = cairo_create(blur) {
                cairo_set_operator(bcr, cairo_operator_t(1))
                cairo_set_source_rgba(bcr, 0, 0, 0, 0)
                cairo_paint(bcr)
                cairo_set_operator(bcr, cairo_operator_t(2))
                cairo_scale(bcr, Double(bw) / Double(surfW),
                            Double(bh) / Double(surfH))
                for slot in order {
                    let zc = Double(depth(cards[slot])) - cam
                    if zc <= focusZc + 1e-6 { continue }
                    if let placed = place(zc) {
                        drawCard(bcr, cards[slot], placed, zc, surfW: surfW,
                                 surfH: surfH, thick: thick, paneA: paneA)
                    }
                }
                cairo_destroy(bcr)
                cairo_save(cr)
                cairo_scale(cr, Double(surfW) / Double(bw),
                            Double(surfH) / Double(bh))
                cairo_set_source_surface(cr, blur, 0, 0)
                if let src = cairo_get_source(cr) {
                    cairo_pattern_set_filter(src, cairo_filter_t(2)) // BILINEAR
                }
                cairo_paint(cr)
                cairo_restore(cr)
            }
        }
        for slot in order {
            let zc = Double(depth(cards[slot])) - cam
            if state["blur"] != nil && zc > focusZc + 1e-6 { continue }
            if let placed = place(zc) {
                drawCard(cr, cards[slot], placed, zc, surfW: surfW, surfH: surfH,
                         thick: thick, paneA: paneA)
            }
        }
        cairo_destroy(cr)
        glaspen2_router_flush_tunnel()
    }

    private struct Placed {
        let cx: Double, cy: Double, w: Double, h: Double, alpha: Double
    }

    private static func depth(_ card: [String: Any]) -> Int32 {
        (card["depth"] as? NSNumber)?.int32Value ?? 0
    }

    private static func place(_ zc: Double) -> Placed? {
        guard let r = TunnelGeometry.place(zc: zc, span: 2.5, front: 4.5,
                                           k: glaspen2_router_tunnel_k(),
                                           surfW: Double(glaspen2_router_tunnel_surf_w()),
                                           surfH: Double(glaspen2_router_tunnel_surf_h()),
                                           travel: glaspen2_router_tunnel_travel()),
              r.count == 5 else { return nil }
        func v(_ i: Int) -> Double { (r[i] as? NSNumber)?.doubleValue ?? 0 }
        return Placed(cx: v(0), cy: v(1), w: v(2), h: v(3), alpha: v(4))
    }

    private static func drawCard(_ cr: OpaquePointer, _ card: [String: Any],
                                 _ placed: Placed, _ zc: Double,
                                 surfW: Int32, surfH: Int32,
                                 thick: Double, paneA: Double) {
        guard let mainValue = card["card"] as? NSValue,
              let mainPtr = mainValue.pointerValue else { return }
        let mainSurf = OpaquePointer(mainPtr)
        var surf = mainSurf
        if let mipValue = card["mip"] as? NSValue, let mipPtr = mipValue.pointerValue {
            let mipSurf = OpaquePointer(mipPtr)
            let mainW = Double(cairo_image_surface_get_width(mainSurf))
            if placed.w < mainW * 0.70 { surf = mipSurf }
        }
        TunnelCardRenderer.draw(crRaw: UnsafeMutableRawPointer(cr),
                                surfRaw: UnsafeMutableRawPointer(surf),
                                nw: cairo_image_surface_get_width(surf),
                                nh: cairo_image_surface_get_height(surf),
                                cx: placed.cx, cy: placed.cy,
                                w: placed.w, h: placed.h,
                                alpha: placed.alpha, zc: zc,
                                surfW: surfW, surfH: surfH,
                                thick: thick, paneA: paneA)
    }
}
