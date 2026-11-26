// S7: 键盘/鼠标镜头动作(数学与状态在 Rust, Swift 只保留 ObjC 选择器入口)。
import AppKit

@objc public final class CanvasCamera: NSObject {
    @objc(panByDX:dy:)
    public static func panBy(dx: Double, dy: Double) {
        glaspen2_rust_camera_pan(dx, dy)
    }

    @objc public static func applyTransform() {
        glaspen2_rust_camera_apply_transform()
    }

    @objc(zoomAtFactor:vx:vy:)
    public static func zoomAt(factor: Double, vx: Double, vy: Double) {
        glaspen2_rust_camera_zoom_at(factor, vx, vy)
    }

    @objc(zoomAtCenterFactor:)
    public static func zoomAtCenter(factor: Double) {
        glaspen2_rust_camera_zoom_at_center(factor)
    }

    @objc public static func persistInfinite() {
        glaspen2_rust_camera_persist_infinite()
    }

    @objc public static func loadInfinite() {
        glaspen2_rust_camera_load_infinite()
    }

    @objc public static func home() {
        glaspen2_rust_camera_home()
    }

    @objc public static func resetLens() {
        glaspen2_rust_camera_reset_lens()
    }
}
