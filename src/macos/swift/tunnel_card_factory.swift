// S7: 时光隧道建卡(半分辨率快照 + 预缩 + 半尺寸 mip)。
import AppKit
import Darwin

@objc public final class TunnelCardFactory: NSObject {
    @objc(makeCardWithIsCurrent:depth:cacheSlot:screenId:)
    public static func make(isCurrent: Bool, depth: Int32, cacheSlot: Int32,
                            screenId: Int64) -> NSDictionary? {
        let screenW = Int(glaspen2_router_screen_w())
        let screenH = Int(glaspen2_router_screen_h())
        let gScale = glaspen2_router_scale()
        var pscale = 1.0, ox = 0.0, oy = 0.0
        var pw: Int32 = Int32(screenW), ph: Int32 = Int32(screenH)
        if !isCurrent {
            glaspen2_page_dims(Int(screenId), &pw, &ph)
            if pw <= 0 || ph <= 0 { pw = Int32(screenW); ph = Int32(screenH) }
            if Int(pw) != screenW || Int(ph) != screenH {
                pscale = min(Double(screenW) / Double(pw), Double(screenH) / Double(ph))
                ox = (Double(screenW) - Double(pw) * pscale) / 2.0
                oy = (Double(screenH) - Double(ph) * pscale) / 2.0
            }
        }
        let cardScale = gScale * 0.5
        let sw = Int32(Double(screenW) * cardScale)
        let sh = Int32(Double(screenH) * cardScale)
        if sw <= 0 || sh <= 0 { return nil }
        guard let surf = cairo_image_surface_create(cairo_format_t(0), sw, sh) else {
            return nil
        }
        if cairo_surface_status(surf).rawValue != 0 { // CAIRO_STATUS_SUCCESS
            cairo_surface_destroy(surf)
            return nil
        }
        if isCurrent {
            if let cr = cairo_create(surf) {
                let f = cardScale / max(gScale, 0.0001)
                cairo_scale(cr, f, f)
                cairo_set_operator(cr, cairo_operator_t(1)) // SOURCE
                if let current = glaspen2_router_surface() {
                    cairo_set_source_surface(cr, current, 0, 0)
                    cairo_paint(cr)
                }
                cairo_destroy(cr)
            }
        } else {
            _ = glaspen2_paint_preview_into_surface(UnsafeMutableRawPointer(surf),
                                                    cacheSlot, cardScale,
                                                    ox * 0.5, oy * 0.5, pscale,
                                                    1.0, 0,
                                                    depth == 0 ? -1.0 : Double(abs(depth)))
        }
        // 预缩到该深度档最大可能尺寸
        let d = Double(depth)
        var smax = 1.0 / (1.0 + glaspen2_router_tunnel_k() * (abs(d) - 1.0))
        if smax > 1.05 { smax = 1.05 }
        let tw = Int32(Double(glaspen2_router_tunnel_surf_w()) * smax)
        let th = Int32(Double(glaspen2_router_tunnel_surf_h()) * smax)
        let nw = cairo_image_surface_get_width(surf)
        let nh = cairo_image_surface_get_height(surf)
        var mainSurf = surf
        if tw > 16 && th > 16 && (tw < nw || th < nh) {
            if let small = cairo_image_surface_create(cairo_format_t(0), tw, th) {
                if let c2 = cairo_create(small) {
                    cairo_scale(c2, Double(tw) / Double(nw), Double(th) / Double(nh))
                    cairo_set_source_surface(c2, surf, 0, 0)
                    cairo_paint(c2)
                    cairo_destroy(c2)
                }
                cairo_surface_destroy(surf)
                mainSurf = small
            }
        }
        var mipOut: OpaquePointer?
        let mw = cairo_image_surface_get_width(mainSurf) / 2
        let mh = cairo_image_surface_get_height(mainSurf) / 2
        if mw > 16 && mh > 16 {
            if let mip = cairo_image_surface_create(cairo_format_t(0), mw, mh) {
                if let c3 = cairo_create(mip) {
                    cairo_scale(c3, 0.5, 0.5)
                    cairo_set_source_surface(c3, mainSurf, 0, 0)
                    cairo_paint(c3)
                    cairo_destroy(c3)
                }
                mipOut = mip
            }
        }
        var result: [String: Any] = ["main": NSValue(pointer: UnsafeMutableRawPointer(mainSurf))]
        if let mipOut {
            result["mip"] = NSValue(pointer: UnsafeMutableRawPointer(mipOut))
        }
        return result as NSDictionary
    }
}
