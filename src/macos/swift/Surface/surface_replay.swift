// S7: 内存笔迹整页重放 + 快照观看页框(页外压暗/页边界线, 物理像素坐标)。
import AppKit

@objc public final class SurfaceReplay: NSObject {
    @objc public static func replay() {
        guard let surface = SurfaceStore.current else { return }
        if ProcessInfo.processInfo.environment["GLASPEN2_SWIFT_SURFACE_REBUILD"] != "1",
           glaspen2_rust_rebuild_surface(UnsafeMutableRawPointer(surface),
                                         SurfaceStore.scaleFactor,
                                         PageViewOps.paperOx, PageViewOps.paperOy, PageViewOps.paperScale,
                                         1) != 0 {
            return
        }
        glaspen2_draw_rebuild_view(UnsafeMutableRawPointer(surface),
                                   SurfaceStore.scaleFactor,
                                   PageViewOps.paperOx,
                                   PageViewOps.paperOy,
                                   PageViewOps.paperScale)
        drawPageFrame()
        cairo_surface_flush(surface)
        OverlayLayerFlush.flush()
    }

    private static func pageviewFrameDraw(_ surface: OpaquePointer) {
        if !PageViewOps.isFit { return }
        guard let cr = cairo_create(surface) else { return }
        let scale = SurfaceStore.scaleFactor
        let x = PageViewOps.paperOx * scale
        let y = PageViewOps.paperOy * scale
        let w = Double(PageViewOps.pageW) * PageViewOps.paperScale * scale
        let h = Double(PageViewOps.pageH) * PageViewOps.paperScale * scale
        cairo_set_source_rgba(cr, 0, 0, 0, 0.42)
        cairo_set_fill_rule(cr, cairo_fill_rule_t(1)) // EVEN_ODD
        cairo_rectangle(cr, 0, 0,
                        Double(cairo_image_surface_get_width(surface)),
                        Double(cairo_image_surface_get_height(surface)))
        cairo_rectangle(cr, x, y, w, h)
        cairo_fill(cr)
        cairo_set_source_rgba(cr, 1, 1, 1, 0.55)
        cairo_set_line_width(cr, 1.0)
        cairo_rectangle(cr, x + 0.5, y + 0.5, w - 1, h - 1)
        cairo_stroke(cr)
        cairo_destroy(cr)
    }

    /// 快照观看页框(供 ObjC 侧重建/删除页后调用)。
    @objc public static func drawPageFrame() {
        guard let surface = SurfaceStore.current else { return }
        if ProcessInfo.processInfo.environment["GLASPEN2_SWIFT_PAGEFRAME"] != "1",
           glaspen2_rust_draw_page_frame(
               UnsafeMutableRawPointer(surface), SurfaceStore.scaleFactor,
               PageViewOps.paperOx, PageViewOps.paperOy, PageViewOps.paperScale,
               Int32(PageViewOps.pageW), Int32(PageViewOps.pageH),
               PageViewOps.isFit ? 1 : 0) != 0 {
            return
        }
        pageviewFrameDraw(surface)
    }

    // R90: Rust/Swift 页框像素对比探针。
    public static func paritySelftest() -> Int32 {
        let w = 300, h = 200
        func make() -> OpaquePointer? {
            guard let s = cairo_image_surface_create(cairo_format_t(0), Int32(w), Int32(h)),
                  cairo_surface_status(s).rawValue == 0 else { return nil }
            cairo_surface_flush(s)
            return s
        }
        guard let a = make(), let b = make() else { return 1 }
        _ = glaspen2_rust_draw_page_frame(UnsafeMutableRawPointer(a), 1.0, 20, 10, 0.8, 200, 150, 1)
        if let cr = cairo_create(b) {
            let x = 20.0, y = 10.0, pw = 200.0 * 0.8, ph = 150.0 * 0.8
            cairo_set_source_rgba(cr, 0, 0, 0, 0.42)
            cairo_set_fill_rule(cr, cairo_fill_rule_t(1))
            cairo_rectangle(cr, 0, 0, Double(w), Double(h))
            cairo_rectangle(cr, x, y, pw, ph)
            cairo_fill(cr)
            cairo_set_source_rgba(cr, 1, 1, 1, 0.55)
            cairo_set_line_width(cr, 1.0)
            cairo_rectangle(cr, x + 0.5, y + 0.5, pw - 1, ph - 1)
            cairo_stroke(cr)
            cairo_destroy(cr)
        }
        cairo_surface_flush(a); cairo_surface_flush(b)
        guard let da = cairo_image_surface_get_data(a), let db = cairo_image_surface_get_data(b) else { return 1 }
        let stride = Int(cairo_image_surface_get_stride(a))
        var diff = 0
        for yy in 0..<h { for xx in 0..<w {
            let i = yy * stride + xx * 4
            if da[i] != db[i] || da[i+1] != db[i+1] || da[i+2] != db[i+2] || da[i+3] != db[i+3] { diff += 1 }
        } }
        cairo_surface_destroy(a); cairo_surface_destroy(b)
        print("SWIFT_PAGEFRAME_PARITY diff=\(diff)/\(w*h)")
        return diff == 0 ? 0 : 1
    }

}

@_cdecl("glaspen2_macos_swift_pageframe_parity_selftest")
public func glaspen2PageframeParity() -> Int32 { SurfaceReplay.paritySelftest() }
