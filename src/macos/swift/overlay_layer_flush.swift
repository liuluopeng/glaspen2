// S7: 图层面刷新(全量/脏矩形) — 只触发 AppKit 重绘, 不做同步 display。
import AppKit

@objc public final class OverlayLayerFlush: NSObject {
    @objc public static func flush() {
        guard glaspen2_router_has_surface() != 0,
              let view = glaspen2_router_draw_view() as? NSView else { return }
        glaspen2_router_dirty_reset()
        view.needsDisplay = true
    }

    @objc public static func flushDirty() {
        guard glaspen2_router_has_surface() != 0,
              let view = glaspen2_router_draw_view() as? NSView else { return }
        if glaspen2_router_dirty_has() == 0 {
            view.needsDisplay = true
            return
        }
        let dirty = glaspen2_router_dirty_rect().intersection(view.bounds)
        if dirty.isEmpty {
            glaspen2_router_dirty_reset()
            view.needsDisplay = true
            return
        }
        glaspen2_router_dirty_reset()
        view.setNeedsDisplay(dirty)
    }
}
