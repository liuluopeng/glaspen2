// S7: 笔迹整页重建 / 气泡自动翻页 / 页视图几何更新。
import AppKit

@objc public final class PageViewOps: NSObject {
    /// 由 STROKES 内存整页重建表面(core 光栅 + 页框 + 全屏刷新)。
    @objc public static func rebuildSurface() {
        guard let surface = glaspen2_router_surface() else { return }
        glaspen2_draw_rebuild_view(UnsafeMutableRawPointer(surface),
                                   glaspen2_router_scale(),
                                   glaspen2_router_pageview_paper_ox(),
                                   glaspen2_router_pageview_paper_oy(),
                                   glaspen2_router_pageview_paper_scale())
        SurfaceReplay.drawPageFrame()
        cairo_surface_flush(surface)
        glaspen2_router_dirty_reset()
        OverlayLayerFlush.flush()
    }

    /// 气泡「写完自动新建一页」: 到点则建页并翻过去, 返回是否翻页。
    @objc public static func bubbleAutoPageFlip() -> Bool {
        let page = glaspen2_bubble_auto_page_if_due()
        if page <= 0 { return false }
        glaspen2_router_page_flip(1, {
            glaspen2_load_strokes_for_screen(page)
            glaspen2_smooth_loaded_strokes()
            glaspen2_router_pageview_update()
        }, {
            glaspen2_router_replay_strokes()
        })
        NotificationPresenter.showPageInfo(screenId: Int64(page))
        if let text = glaspen2_menu_l10n("page.auto_created") {
            glaspen2_router_show_notification(text)
        }
        return true
    }

    /// 当前页/视图几何: 唯一来源 core::minimap 的页窗参数(FFI)。
    @objc public static func update() {
        glaspen2_router_pageview_set(0, 1.0, 0, 0, 0, 0)
        if glaspen2_router_infinite() != 0 {
            glaspen2_router_pageview_set(0, 1.0, 0, 0, 0, 0)
            return
        }
        var pw: Int32 = 0
        var ph: Int32 = 0
        glaspen2_page_dims(glaspen2_get_current_screen_id(), &pw, &ph)
        let screenW = glaspen2_router_screen_w()
        let screenH = glaspen2_router_screen_h()
        if pw <= 0 || ph <= 0 || (pw == Int32(screenW) && ph == Int32(screenH)) {
            glaspen2_router_pageview_set(0, 1.0, 0, 0, 0, 0)
            return
        }
        let scale = min(screenW / Double(pw), screenH / Double(ph))
        let ox = (screenW - Double(pw) * scale) / 2.0
        let oy = (screenH - Double(ph) * scale) / 2.0
        glaspen2_router_pageview_set(1, scale, ox, oy, Int32(pw), Int32(ph))
        NSLog("[glaspen2] pageview fit: 页 %dx%d → 屏 %dx%d scale=%.3f off=(%.0f,%.0f)",
              pw, ph, Int32(screenW), Int32(screenH), scale, ox, oy)
    }
}
