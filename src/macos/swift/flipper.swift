// S7: 翻页编排(隧道接管或系统隐藏间隙; 先收尾旧动画)。
import AppKit

@objc public final class Flipper: NSObject {
    @objc(swapFromNext:fromPage:prepare:commit:)
    public static func swapFrom(goingNext: Bool, fromPage: Int64,
                                prepare: @escaping () -> Void,
                                commit: @escaping () -> Void) {
        if glaspen2_router_tunnel_active() != 0 {
            glaspen2_router_tunnel_finish()
        }
        guard let window = glaspen2_router_main_window() as? NSWindow else {
            prepare()
            commit()
            return
        }
        if glaspen2_router_flip_effect() == 1 && glaspen2_router_infinite() == 0
            && glaspen2_router_surface() != nil
            && glaspen2_router_stroke_active() == 0 {
            let curBefore: Int = fromPage > 0 ? Int(fromPage) : glaspen2_get_current_screen_id()
            let tp0 = Date().timeIntervalSinceReferenceDate
            prepare()
            let tp1 = Date().timeIntervalSinceReferenceDate
            let tunneled = glaspen2_router_page_flip_tunnel(goingNext ? 1 : 0,
                                                            curBefore, prepare, commit)
            if tunneled != 0 {
                NSLog("[flip] 卡顿账: prepare %.0fms", (tp1 - tp0) * 1000.0)
                return
            }
        }
        let t0 = Date().timeIntervalSinceReferenceDate
        window.setIsVisible(false)
        prepare()
        DispatchQueue.main.async {
            commit()
            window.setIsVisible(true)
            let t1 = Date().timeIntervalSinceReferenceDate
            NSLog("[flip] 隐藏间隙 %.0fms", (t1 - t0) * 1000.0)
        }
    }

    @objc(swapNext:prepare:commit:)
    public static func swap(goingNext: Bool, prepare: @escaping () -> Void,
                            commit: @escaping () -> Void) {
        swapFrom(goingNext: goingNext, fromPage: 0, prepare: prepare, commit: commit)
    }
}
