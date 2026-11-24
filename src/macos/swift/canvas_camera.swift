// S7: 键盘平移镜头(数学在 core::camera, 经 FFI; 门控/重绘/peek 仍由 adapter 调)。
import AppKit

@objc public final class CanvasCamera: NSObject {
    @objc(panByDX:dy:)
    public static func panBy(dx: Double, dy: Double) {
        if glaspen2_router_stroke_active() != 0 { return }
        let json = glaspen2_camera_pan(glaspen2_router_pan_x(),
                                       glaspen2_router_pan_y(),
                                       glaspen2_router_zoom(), dx, dy)
        guard let result = CameraState.resultFromJSON(json) else { return }
        guard CameraState.assign(result: result, assignZoom: true) else { return }
        glaspen2_router_canvas_apply_transform()
        glaspen2_router_peek_lens_change()
    }

    private static var lastZoomHint: TimeInterval = 0

    /// 缩放结果应用: 100% 上限提示(1.5s 节流) + 镜头状态 + 变换/peek。
    @objc(applyZoomResult:)
    public static func applyZoomResult(_ result: NSDictionary) {
        func num(_ key: String) -> NSNumber? { result[key] as? NSNumber }
        if num("hit_max")?.boolValue ?? false {
            let now = ProcessInfo.processInfo.systemUptime
            if now - lastZoomHint > 1.5 {
                lastZoomHint = now
                glaspen2_menu_notify(glaspen2_menu_l10n("zoom.max_100"))
            }
        }
        if !CameraState.assign(result: result, assignZoom: true) { return }
        if !(num("changed")?.boolValue ?? false) { return }
        glaspen2_router_canvas_apply_transform()
        glaspen2_router_peek_lens_change()
    }
}
