// S7: 无限画布总览(包围盒外扩 5% + 渲染 + 视口矩形映射)。
import AppKit

@objc public final class CanvasOverview: NSObject {
    @objc(payloadWithWidth:height:)
    public static func payload(width: Double, height: Double) -> NSDictionary? {
        var bx = 0.0, by = 0.0, bx2 = 0.0, by2 = 0.0
        guard glaspen2_stroke_bbox(&bx, &by, &bx2, &by2) != 0 else { return nil }
        var bw = bx2 - bx, bh = by2 - by
        if bw < 1.0 { bw = 1.0 }
        if bh < 1.0 { bh = 1.0 }
        let mx = bw * 0.05, my = bh * 0.05
        bx -= mx; by -= my; bw += mx * 2; bh += my * 2

        let ow = Int32(width), oh = Int32(height)
        var outLen: Int32 = 0
        guard let png = glaspen2_render_canvas_overview(bx, by, bw, bh, ow, oh, &outLen),
              outLen > 0 else { return nil }
        let data = Data(bytes: png, count: Int(outLen))
        glaspen2_free_rust_bytes(png, outLen)

        let ovScale = min(Double(ow) / bw, Double(oh) / bh)
        let ovOx = (Double(ow) - bw * ovScale) * 0.5
        let ovOy = (Double(oh) - bh * ovScale) * 0.5
        let z = glaspen2_camera_step_zoom(glaspen2_router_zoom())
        let vrW = glaspen2_router_screen_w() / z * ovScale
        let vrH = glaspen2_router_screen_h() / z * ovScale
        let vx = ovOx + (glaspen2_router_pan_x() - bx) * ovScale
        let vy = ovOy + (glaspen2_router_pan_y() - by) * ovScale
        return [
            "png": data,
            "rect": [vx, vy, vrW, vrH],
            "bounds": [bx, by, bw, bh],
        ]
    }
}
