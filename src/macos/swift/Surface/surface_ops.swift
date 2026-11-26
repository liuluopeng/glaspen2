// S7: 清屏(翻页模式)与仅保存涂鸦截图。
import AppKit

@objc public final class SurfaceOps: NSObject {
    /// 清空当前页(无限画布不清, 只提示)。
    @objc public static func clearScreen() {
        if glaspen2_router_infinite() != 0 {
            if let text = Optional(MenuRuntime.l10n("canvas.infinite_kept")) {
                notificationShowText(text:text)
            }
            return
        }
        guard let surface = surfaceCurrent() else { return }
        let created = glaspen2_rust_clear_screen(
            UnsafeMutableRawPointer(surface), surfaceScale(),
            glaspen2_rust_pageview_paper_ox(), glaspen2_rust_pageview_paper_oy(), glaspen2_rust_pageview_paper_scale(),
            (glaspen2_rust_pageview_is_fit() != 0) ? 1 : 0)
        if created != 0 {
            notificationShowPageInfo(screenId: Int64(glaspen2_get_current_screen_id()))
        } else if let text = Optional(MenuRuntime.l10n("canvas.blank_guard")) {
            notificationShowText(text:text)
        }
    }

    /// 保存纯涂鸦 PNG(不含背景)。
    @objc public static func saveDrawingOnly() {
        guard let surface = surfaceCurrent() else { return }
        _ = glaspen2_rust_save_drawing(UnsafeMutableRawPointer(surface))
        if let text = Optional(MenuRuntime.l10n("export.screenshot_saved")) {
            notificationShowText(text:text)
        }
    }

    /// 与 ObjC cairo_create_scaled 等价: 创建 context 并应用 scale + 页视图变换。
    private static func makeScaledContext(_ surface: OpaquePointer) -> OpaquePointer? {
        guard let cr = cairo_create(surface) else { return nil }
        let scale = surfaceScale()
        cairo_scale(cr, scale, scale)
        if (glaspen2_rust_pageview_is_fit() != 0) {
            cairo_translate(cr, glaspen2_rust_pageview_paper_ox() * scale,
                            glaspen2_rust_pageview_paper_oy() * scale)
            let ps = glaspen2_rust_pageview_paper_scale()
            cairo_scale(cr, ps, ps)
        }
        return cr
    }
}
