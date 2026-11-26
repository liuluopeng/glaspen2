// R44: 页视图 fit 状态/几何已迁 Rust; 本文件只做 C 桥与重建/翻页动作。
import AppKit

@objc public final class PageViewOps: NSObject {
    static var isFit: Bool { glaspen2_rust_pageview_is_fit() != 0 }
    static var paperOx: Double { glaspen2_rust_pageview_paper_ox() }
    static var paperOy: Double { glaspen2_rust_pageview_paper_oy() }
    static var paperScale: Double { glaspen2_rust_pageview_paper_scale() }
    static var pageW: Int { Int(glaspen2_rust_pageview_page_w()) }
    static var pageH: Int { Int(glaspen2_rust_pageview_page_h()) }
    static var geometryWidth: Int32 { glaspen2_rust_pageview_geometry_width() }

    /// 由 STROKES 内存整页重建表面(core 光栅 + 页框 + 全屏刷新)。
    @objc public static func rebuildSurface() {
        guard let surface = SurfaceStore.current else { return }
        _ = glaspen2_rust_rebuild_surface(UnsafeMutableRawPointer(surface),
                                         SurfaceStore.scaleFactor,
                                         PageViewOps.paperOx, PageViewOps.paperOy, PageViewOps.paperScale,
                                         1)
    }

    /// 气泡「写完自动新建一页」: 到点则建页并翻过去, 返回是否翻页。
    @objc public static func bubbleAutoPageFlip() -> Bool {
        let page = glaspen2_bubble_auto_page_if_due()
        if page <= 0 { return false }
        Flipper.swap(goingNext: true, prepare: {
            glaspen2_load_strokes_for_screen(page)
            glaspen2_smooth_loaded_strokes()
            PageViewOps.update()
        }, commit: {
            glaspen2_router_replay_strokes()
        })
        NotificationPresenter.showPageInfo(screenId: Int64(page))
        if let text = Optional(MenuRuntime.l10n("page.auto_created")) {
            NotificationPresenter.show(text:text)
        }
        return true
    }

    /// 当前页/视图几何: 唯一来源 core(页面尺寸/屏幕尺寸 -> fit)。
    @objc public static func update() {
        glaspen2_rust_pageview_update()
    }
}
