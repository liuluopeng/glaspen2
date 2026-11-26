// S7: 清屏(翻页模式)与仅保存涂鸦截图。
import AppKit

@objc public final class SurfaceOps: NSObject {
    /// 清空当前页(无限画布不清, 只提示)。
    @objc public static func clearScreen() {
        if glaspen2_router_infinite() != 0 {
            if let text = Optional(MenuRuntime.l10n("canvas.infinite_kept")) {
                NotificationPresenter.show(text:text)
            }
            return
        }
        guard let surface = SurfaceStore.current else { return }
        if ProcessInfo.processInfo.environment["GLASPEN2_SWIFT_SURFACE_OPS"] != "1" {
            let created = glaspen2_rust_clear_screen(
                UnsafeMutableRawPointer(surface), SurfaceStore.scaleFactor,
                PageViewOps.paperOx, PageViewOps.paperOy, PageViewOps.paperScale,
                PageViewOps.isFit ? 1 : 0)
            if created != 0 {
                NotificationPresenter.showPageInfo(screenId: Int64(glaspen2_get_current_screen_id()))
            } else if let text = Optional(MenuRuntime.l10n("canvas.blank_guard")) {
                NotificationPresenter.show(text:text)
            }
            return
        }
        if let cr = makeScaledContext(surface) {
            cairo_set_operator(cr, cairo_operator_t(3)) // CLEAR
            cairo_paint(cr)
            cairo_destroy(cr)
        }
                let created = glaspen2_clear_strokes(Int32(glaspen2_router_screen_w()),
                                             Int32(glaspen2_router_screen_h()))
        PageViewOps.update()
        glaspen2_rust_overlay_flush()
        if created != 0 {
            NotificationPresenter.showPageInfo(screenId: Int64(glaspen2_get_current_screen_id()))
        } else if let text = Optional(MenuRuntime.l10n("canvas.blank_guard")) {
            NotificationPresenter.show(text:text)
        }
    }

    /// 保存纯涂鸦 PNG(不含背景)。
    @objc public static func saveDrawingOnly() {
        guard let surface = SurfaceStore.current else { return }
        if ProcessInfo.processInfo.environment["GLASPEN2_SWIFT_SURFACE_OPS"] != "1",
           glaspen2_rust_save_drawing(UnsafeMutableRawPointer(surface)) != 0 {
            if let text = Optional(MenuRuntime.l10n("export.screenshot_saved")) {
                NotificationPresenter.show(text:text)
            }
            return
        }
        cairo_surface_flush(surface)
        let w = Int32(cairo_image_surface_get_width(surface))
        let h = Int32(cairo_image_surface_get_height(surface))
        let stride = Int32(cairo_image_surface_get_stride(surface))
        let data = cairo_image_surface_get_data(surface)
        glaspen2_save_drawing(data, w, h, stride)
        if let text = Optional(MenuRuntime.l10n("export.screenshot_saved")) {
            NotificationPresenter.show(text:text)
        }
    }

    /// 与 ObjC cairo_create_scaled 等价: 创建 context 并应用 scale + 页视图变换。
    private static func makeScaledContext(_ surface: OpaquePointer) -> OpaquePointer? {
        guard let cr = cairo_create(surface) else { return nil }
        let scale = SurfaceStore.scaleFactor
        cairo_scale(cr, scale, scale)
        if PageViewOps.isFit {
            cairo_translate(cr, PageViewOps.paperOx * scale,
                            PageViewOps.paperOy * scale)
            let ps = PageViewOps.paperScale
            cairo_scale(cr, ps, ps)
        }
        return cr
    }
}
