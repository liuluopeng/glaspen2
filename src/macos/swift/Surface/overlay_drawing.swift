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






}

// R38: Rust 编排覆盖层绘制时调用这些 stage 桥(各 stage 仍是原 Swift 实现)。









