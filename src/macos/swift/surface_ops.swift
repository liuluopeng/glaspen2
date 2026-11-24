// S7: 清屏(翻页模式)与仅保存涂鸦截图。
import AppKit

@objc public final class SurfaceOps: NSObject {
    /// 清空当前页(无限画布不清, 只提示)。
    @objc public static func clearScreen() {
        if glaspen2_router_infinite() != 0 {
            if let text = glaspen2_menu_l10n("canvas.infinite_kept") {
                glaspen2_router_show_notification(text)
            }
            return
        }
        guard let surface = glaspen2_router_surface() else { return }
        if let cr = makeScaledContext(surface) {
            cairo_set_operator(cr, cairo_operator_t(3)) // CLEAR
            cairo_paint(cr)
            cairo_destroy(cr)
        }
        glaspen2_router_set_has_last(0)
        let created = glaspen2_clear_strokes(Int32(glaspen2_router_screen_w()),
                                             Int32(glaspen2_router_screen_h()))
        glaspen2_router_pageview_update()
        OverlayLayerFlush.flush()
        if created != 0 {
            NotificationPresenter.showPageInfo(screenId: Int64(glaspen2_get_current_screen_id()))
        } else if let text = glaspen2_menu_l10n("canvas.blank_guard") {
            glaspen2_router_show_notification(text)
        }
    }

    /// 保存纯涂鸦 PNG(不含背景)。
    @objc public static func saveDrawingOnly() {
        guard let surface = glaspen2_router_surface() else { return }
        cairo_surface_flush(surface)
        let w = Int32(cairo_image_surface_get_width(surface))
        let h = Int32(cairo_image_surface_get_height(surface))
        let stride = Int32(cairo_image_surface_get_stride(surface))
        let data = cairo_image_surface_get_data(surface)
        glaspen2_save_drawing(data, w, h, stride)
        if let text = glaspen2_menu_l10n("export.screenshot_saved") {
            glaspen2_router_show_notification(text)
        }
    }

    /// 与 ObjC cairo_create_scaled 等价: 创建 context 并应用 scale + 页视图变换。
    private static func makeScaledContext(_ surface: OpaquePointer) -> OpaquePointer? {
        guard let cr = cairo_create(surface) else { return nil }
        let scale = glaspen2_router_scale()
        cairo_scale(cr, scale, scale)
        if glaspen2_router_pageview_is_fit() != 0 {
            cairo_translate(cr, glaspen2_router_pageview_paper_ox() * scale,
                            glaspen2_router_pageview_paper_oy() * scale)
            let ps = glaspen2_router_pageview_paper_scale()
            cairo_scale(cr, ps, ps)
        }
        return cr
    }
}
