// R73: 总览载荷几何/渲染迁 Rust; 本文件只做 Data 包装。
import AppKit

@objc public final class CanvasOverview: NSObject {
    @objc(payloadWithWidth:height:)
    public static func payload(width: Double, height: Double) -> NSDictionary? {
        var rect = [Double](repeating: 0, count: 4)
        var bounds = [Double](repeating: 0, count: 4)
        var len: Int32 = 0
        let png = rect.withUnsafeMutableBufferPointer { r in
            bounds.withUnsafeMutableBufferPointer { b in
                glaspen2_rust_canvas_overview(width, height, r.baseAddress, b.baseAddress, &len)
            }
        }
        guard let png, len > 0 else { return nil }
        let data = Data(bytes: png, count: Int(len))
        glaspen2_free_rust_bytes(png, len)
        return [
            "png": data,
            "rect": rect,
            "bounds": bounds,
        ]
    }
}
