// S7/R38: drawRect 编排(裁剪/隧道/网格/笔迹/缩略图/通知/光标/性能打点), 状态均在 Swift。
import AppKit
import Darwin

@objc public final class OverlayDrawing: NSObject {
    @objc(drawInView:rect:)
    public static func draw(in view: NSView, rect: NSRect) {
        _ = glaspen2_rust_overlay_draw(
            Unmanaged.passUnretained(view).toOpaque(),
            rect.origin.x, rect.origin.y, rect.size.width, rect.size.height)
    }





    static func drawMinimap(_ ctx: CGContext, bounds: NSRect) {
        let infinite = glaspen2_router_infinite() != 0
        let minimap = glaspen2_router_minimap_enabled() != 0
        if minimap && !infinite && bounds.maxX >= bounds.size.width - 96.0 {
            MinimapRenderer.draw(ctx, bounds: bounds)
        }
    }

}

// R38: Rust 编排覆盖层绘制时调用这些 stage 桥(各 stage 仍是原 Swift 实现)。




@_cdecl("glaspen2_swift_overlay_minimap")
public func glaspen2SwiftOverlayMinimap(_ ctx: UnsafeMutableRawPointer?,
                                        _ bx: Double, _ by: Double, _ bw: Double, _ bh: Double) {
    guard let ctx else { return }
    let c = Unmanaged<CGContext>.fromOpaque(ctx).takeUnretainedValue()
    OverlayDrawing.drawMinimap(c, bounds: NSRect(x: bx, y: by, width: bw, height: bh))
}





