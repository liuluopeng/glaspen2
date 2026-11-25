// S7: 键盘平移镜头(数学在 core::camera, 经 FFI; 门控/重绘/peek 仍由 adapter 调)。
import AppKit

@objc public final class CanvasCamera: NSObject {
    private static func setCamera(_ px: Double, _ py: Double, _ pz: Double,
                                  assignZoom: Bool) {
        glaspen2_router_set_camera(px, py, pz, assignZoom ? 1 : 0)
    }

    @objc(panByDX:dy:)
    public static func panBy(dx: Double, dy: Double) {
        if glaspen2_router_stroke_active() != 0 { return }
        var px = 0.0, py = 0.0, pz = 0.0
        guard glaspen2_camera_pan_values(glaspen2_router_pan_x(),
                                         glaspen2_router_pan_y(),
                                         glaspen2_router_zoom(), dx, dy,
                                         &px, &py, &pz) != 0 else { return }
        setCamera(px, py, pz, assignZoom: true)
        glaspen2_router_canvas_apply_transform()
        glaspen2_router_peek_lens_change()
    }

    private static var lastTransformSave: TimeInterval = 0

    /// 应用镜头变换 + 重建 surface, 0.5s 节流持久化。
    @objc public static func applyTransform() {
        glaspen2_set_view_transform(glaspen2_router_pan_x(),
                                     glaspen2_router_pan_y(),
                                     glaspen2_router_zoom())
        glaspen2_router_rebuild_surface()
        let now = ProcessInfo.processInfo.systemUptime
        if now - lastTransformSave > 0.5 {
            lastTransformSave = now
            persistInfinite()
        }
    }

    /// 以视口点 (vx,vy) 为锚缩放(数学在 core Camera::zoom_at)。
    @objc(zoomAtFactor:vx:vy:)
    public static func zoomAt(factor: Double, vx: Double, vy: Double) {
        if glaspen2_router_stroke_active() != 0 { return }
        var px = 0.0, py = 0.0, pz = 0.0
        var hitMax: Int32 = 0, hitMin: Int32 = 0, changed: Int32 = 0
        guard glaspen2_camera_zoom_at_values(glaspen2_router_pan_x(),
                                             glaspen2_router_pan_y(),
                                             glaspen2_router_zoom(), factor, vx, vy,
                                             &px, &py, &pz,
                                             &hitMax, &hitMin, &changed) != 0 else { return }
        applyZoom(px, py, pz, hitMax: hitMax, changed: changed)
    }

    /// 以视口中心为锚缩放(数学在 core Camera::zoom_at_center)。
    @objc(zoomAtCenterFactor:)
    public static func zoomAtCenter(factor: Double) {
        if glaspen2_router_stroke_active() != 0 { return }
        var px = 0.0, py = 0.0, pz = 0.0
        var hitMax: Int32 = 0, hitMin: Int32 = 0, changed: Int32 = 0
        guard glaspen2_camera_zoom_at_center_values(glaspen2_router_pan_x(),
                                                    glaspen2_router_pan_y(),
                                                    glaspen2_router_zoom(), factor,
                                                    glaspen2_router_screen_w(),
                                                    glaspen2_router_screen_h(),
                                                    &px, &py, &pz,
                                                    &hitMax, &hitMin, &changed) != 0 else { return }
        applyZoom(px, py, pz, hitMax: hitMax, changed: changed)
    }

    /// 无限画布: 持久化当前镜头(仅无限模式)。
    @objc public static func persistInfinite() {
        if (glaspen2_router_state() & 4) == 0 { return }
        glaspen2_set_infinite_transform(glaspen2_router_pan_x(),
                                        glaspen2_router_pan_y(),
                                        glaspen2_router_zoom())
    }

    /// 无限画布: 载入唯一镜头并应用(zoom 兜底已在 FFI 边界完成)。
    @objc public static func loadInfinite() {
        var px = 0.0, py = 0.0, pz = 1.0
        glaspen2_get_infinite_transform(&px, &py, &pz)
        glaspen2_router_set_camera(px, py, pz, 1)
        glaspen2_set_view_transform(px, py, pz)
    }

    private static var lastZoomHint: TimeInterval = 0

    /// 缩放结果应用: 100% 上限提示(1.5s 节流) + 镜头状态 + 变换/peek。
    private static func applyZoom(_ px: Double, _ py: Double, _ pz: Double,
                                  hitMax: Int32, changed: Int32) {
        if hitMax != 0 {
            let now = ProcessInfo.processInfo.systemUptime
            if now - lastZoomHint > 1.5 {
                lastZoomHint = now
                MenuRuntime.notify(MenuRuntime.l10n("zoom.max_100"))
            }
        }
        setCamera(px, py, pz, assignZoom: true)
        if changed == 0 { return }
        glaspen2_router_canvas_apply_transform()
        glaspen2_router_peek_lens_change()
    }

    /// 镜头回原点 + 100%(HOME 单源 core::Camera::HOME), 并重建画面。
    @objc public static func home() {
        var px = 0.0, py = 0.0, pz = 1.0
        glaspen2_camera_home(&px, &py, &pz)
        glaspen2_router_set_camera(px, py, pz, 1)
        applyTransform()
    }

    /// 翻页模式重置镜头(仅设置变换, 不重建 surface)。
    @objc public static func resetLens() {
        glaspen2_router_reset_page_offsets()
        var px = 0.0, py = 0.0, pz = 1.0
        glaspen2_camera_home(&px, &py, &pz)
        glaspen2_router_set_camera(px, py, pz, 1)
        glaspen2_set_view_transform(px, py, pz)
    }
}
