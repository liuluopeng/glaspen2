// S7: 内存笔迹整页重放 + 快照观看页框(页外压暗/页边界线, 物理像素坐标)。
import AppKit

@objc public final class SurfaceReplay: NSObject {
    @objc public static func replay() {
        guard let surface = glaspen2_router_surface() else { return }
        glaspen2_draw_rebuild_view(UnsafeMutableRawPointer(surface),
                                   glaspen2_router_scale(),
                                   glaspen2_router_pageview_paper_ox(),
                                   glaspen2_router_pageview_paper_oy(),
                                   glaspen2_router_pageview_paper_scale())
        drawPageFrame()
        cairo_surface_flush(surface)
        glaspen2_router_set_has_last(0)
        OverlayLayerFlush.flush()
    }

    private static func pageviewFrameDraw(_ surface: OpaquePointer) {
        if glaspen2_router_pageview_is_fit() == 0 { return }
        guard let cr = cairo_create(surface) else { return }
        let scale = glaspen2_router_scale()
        let x = glaspen2_router_pageview_paper_ox() * scale
        let y = glaspen2_router_pageview_paper_oy() * scale
        let w = glaspen2_router_pageview_page_w() * glaspen2_router_pageview_paper_scale() * scale
        let h = glaspen2_router_pageview_page_h() * glaspen2_router_pageview_paper_scale() * scale
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
        guard let surface = glaspen2_router_surface() else { return }
        pageviewFrameDraw(surface)
    }

}
