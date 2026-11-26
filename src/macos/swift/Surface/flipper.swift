// S7: 翻页编排(隧道接管或系统隐藏间隙; 先收尾旧动画)。
import AppKit

@objc public final class Flipper: NSObject {
    @objc(swapFromNext:fromPage:prepare:commit:)
    public static func swapFrom(goingNext: Bool, fromPage: Int64,
                                prepare: @escaping () -> Void,
                                commit: @escaping () -> Void) {
        if TunnelState.active {
            TunnelLifecycle.finish()
        }
        guard let window = UIAnchors.mainWindow else {
            prepare()
            commit()
            return
        }
        if glaspen2_router_flip_effect() == 1 && glaspen2_router_infinite() == 0
            && surfaceCurrent() != nil
            && glaspen2_router_stroke_active() == 0 {
            let curBefore: Int64 = fromPage > 0 ? fromPage : glaspen2_get_current_screen_id()
            let tp0 = Date().timeIntervalSinceReferenceDate
            prepare()
            let tp1 = Date().timeIntervalSinceReferenceDate
            // prepare 已在建卡前执行过(与建卡共用一次 DB 访问); 旧 ObjC 这里传 NULL,
            // 否则动画落定瞬间会重复 load+smooth+update, 造成落点卡顿。
            let tunneled = TunnelAnimation.start(goingNext: goingNext, curPage: Int(curBefore),
                                                 prepare: {}, commit: commit)
            if tunneled {
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
