// R56: 首点/拖动实时绘制已迁 Rust(计划优先 + 回退光栅化); 本文件只保留表面门控。
import AppKit

@objc public final class RawDrawing: NSObject {
    @objc(drawDotWithX:y:width:)
    public static func drawDot(x: Double, y: Double, width: Double) {
        guard SurfaceStore.current != nil else { return }
        glaspen2_rust_raw_draw_dot(x, y, width)
    }

    @objc(drawSegmentWithX:y:width:)
    public static func drawSegment(x: Double, y: Double, width: Double) {
        guard SurfaceStore.current != nil else { return }
        glaspen2_rust_raw_draw_segment(x, y, width)
    }
}
