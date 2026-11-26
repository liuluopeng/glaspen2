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
        let created = glaspen2_rust_clear_screen(
            UnsafeMutableRawPointer(surface), SurfaceStore.scaleFactor,
            PageViewOps.paperOx, PageViewOps.paperOy, PageViewOps.paperScale,
            PageViewOps.isFit ? 1 : 0)
        if created != 0 {
            NotificationPresenter.showPageInfo(screenId: Int64(glaspen2_get_current_screen_id()))
        } else if let text = Optional(MenuRuntime.l10n("canvas.blank_guard")) {
            NotificationPresenter.show(text:text)
        }
    }

    /// 保存纯涂鸦 PNG(不含背景)。
    @objc public static func saveDrawingOnly() {
        guard let surface = SurfaceStore.current else { return }
        _ = glaspen2_rust_save_drawing(UnsafeMutableRawPointer(surface))
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
