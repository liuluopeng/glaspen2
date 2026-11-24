// S7: 时光隧道卡片位置/缩放/透明度(页距 → 画面矩形)。
import AppKit

@objc public final class TunnelGeometry: NSObject {
    @objc(placeWithZC:span:front:k:surfW:surfH:travel:)
    public static func place(zc: Double, span: Double, front: Double, k: Double,
                             surfW: Double, surfH: Double, travel: Double) -> NSArray? {
        if zc < -span || zc > front { return nil }
        let s = 1.0 / (1.0 + k * zc)
        if s <= 0.06 || s > 2.4 { return nil }
        let W = surfW, H = surfH
        let w = W * s
        let h = H * s
        let cx = W * 0.5 + W * 0.0045 * zc
        let cy = H * 0.5 - H * 0.027 * zc
        var a: Double
        if zc < 0.0 {
            a = 1.0 + zc / travel
            if a < 0.0 { a = 0.0 }
        } else {
            a = 1.0 / (1.0 + 0.90 * zc)
        }
        if a > 1.0 { a = 1.0 }
        return [cx, cy, w, h, a]
    }
}
