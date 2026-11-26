// S7: 笔迹 cairo 表面生命周期(Swift 持有, ObjC 借用 g_surface)。
import AppKit

@objc public final class SurfaceStore: NSObject {
    static var current: OpaquePointer? {
        get { glaspen2_rust_surface_current().map { OpaquePointer($0) } }
        set { glaspen2_rust_surface_set(newValue.map(UnsafeMutableRawPointer.init)) }
    }
    static var scaleFactor: Double {
        get { glaspen2_rust_surface_scale() }
        set { glaspen2_rust_surface_set_scale(newValue) }
    }

    @objc(ensureWithView:)
    public static func ensure(view: NSView) {
        let bounds = view.bounds
        var scale = view.window?.backingScaleFactor ?? 1
        if scale < 1 { scale = 1 }
        let w = Int32(bounds.size.width * scale)
        let h = Int32(bounds.size.height * scale)
        if let s = current,
           cairo_image_surface_get_width(s) == w,
           cairo_image_surface_get_height(s) == h,
           scaleFactor == Double(scale) { return }
        if let s = current {
            glaspen2_router_surface_cache_invalidate()
            cairo_surface_destroy(s)
        }
        let s = cairo_image_surface_create(cairo_format_t(0), w, h)
        current = s
        scaleFactor = Double(scale)
        if let cr = cairo_create(s) {
            cairo_set_operator(cr, cairo_operator_t(3))
            cairo_paint(cr)
            cairo_set_operator(cr, cairo_operator_t(2))
            cairo_destroy(cr)
        }
    }
}
