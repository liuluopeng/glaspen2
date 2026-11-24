// S7: 实时笔画共享 cairo 上下文(整笔期间复用, 抬笔销毁)。
import AppKit

@objc public final class StrokeSurface: NSObject {
    private static var active: OpaquePointer?

    @objc public static func begin() {
        if active != nil || glaspen2_router_active_ctx_present() != 0 { return }
        guard let surface = SurfaceStore.current else { return }
        guard let ctx = cairo_create(surface) else { return }
        cairo_scale(ctx, SurfaceStore.scaleFactor, SurfaceStore.scaleFactor)
        active = ctx
        glaspen2_router_install_active_ctx(ctx)
        glaspen2_router_reset_raw_stroke()
    }

    @objc public static func end() {
        active = nil
        if let ctx = glaspen2_router_take_active_ctx() {
            cairo_destroy(ctx)
        }
    }
}
