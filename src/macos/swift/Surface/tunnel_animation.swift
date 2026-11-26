// S7: 时光隧道启动(状态/表面/预载建卡/参数/首帧/120Hz 定时器)。
import AppKit
import Darwin

@objc public final class TunnelAnimation: NSObject {
    @objc(startTunnelGoingNext:curPage:prepare:commit:)
    public static func start(goingNext: Bool, curPage: Int,
                             prepare: @escaping () -> Void,
                             commit: @escaping () -> Void) -> Bool {
        if TunnelState.active { return false }
        let cur = Int(curPage)
        TunnelLifecycle.freeCards()
        var preloadMs = 0.0
        let n = glaspen2_rust_tunnel_prepare(Int64(cur), goingNext ? 1 : 0,
                                              surfaceScale(), &preloadMs)
        if n <= 0 { return false }
        let target = goingNext ? glaspen2_page_neighbor(Int64(cur), 1)
                               : glaspen2_page_neighbor(Int64(cur), -1)
        let buildStart = TunnelState.now()
        let count = Int(n)
        let curIdx = count - 2
        let built = TunnelState.depths.withUnsafeMutableBufferPointer { dp in
            TunnelState.cardSurf.withUnsafeMutableBufferPointer { cp in
                TunnelState.mipSurf.withUnsafeMutableBufferPointer { mp in
                    cp.baseAddress!.withMemoryRebound(to: UnsafeMutableRawPointer?.self, capacity: TunnelState.maxCards) { cardPtr in
                        mp.baseAddress!.withMemoryRebound(to: UnsafeMutableRawPointer?.self, capacity: TunnelState.maxCards) { mipPtr in
                            dp.baseAddress!.withMemoryRebound(to: Int32.self, capacity: TunnelState.maxCards) { depthPtr in
                                glaspen2_rust_tunnel_build_cards(
                                    Int64(cur), Int32(count), Int32(curIdx),
                                    glaspen2_tunnel_front(), depthPtr, cardPtr, mipPtr)
                            }
                        }
                    }
                }
            }
        }
        if built < 0 {
            TunnelLifecycle.freeCards()
            return false
        }
        TunnelState.cards = Int(built)
        let tc2 = TunnelState.now()
        NSLog("[flip] 时光隧道接管: %d 张卡片(cur=%ld → %ld, 方向=%@) 预载%.0fms 建卡%.0fms",
              built, cur, target, goingNext ? "后" : "前",
              preloadMs, (tc2 - buildStart) * 1000.0)
        TunnelState.back = !goingNext
        TunnelState.travel = 1.0
        TunnelState.prepare = prepare
        TunnelState.commit = commit
        TunnelState.active = true

        glaspen2_rust_tunnel_apply_env()
        glaspen2_rust_tunnel_reset_start()
        TunnelFrameRenderer.render(p: 0.0)
        if let view = UIAnchors.drawView {
            view.displayIfNeeded()
        }
        let timer = DispatchSource.makeTimerSource(queue: .main)
        timer.schedule(deadline: .now(), repeating: .microseconds(8333),
                       leeway: .microseconds(1000))
        timer.setEventHandler {
            var p = 0.0
            if glaspen2_rust_tunnel_tick(&p) == 0 {
                TunnelLifecycle.finish()
                return
            }
            TunnelFrameRenderer.render(p: p)
            if TunnelState.active, let view = UIAnchors.drawView {
                view.displayIfNeeded()
            }
        }
        TunnelState.timer = timer
        timer.resume()
        return true
    }
}


@objc public final class TunnelLifecycle: NSObject {
    @objc public static func freeCards() {
        TunnelState.cardSurf.withUnsafeBufferPointer { cp in
            TunnelState.mipSurf.withUnsafeBufferPointer { mp in
                cp.baseAddress!.withMemoryRebound(to: UnsafeMutableRawPointer?.self, capacity: TunnelState.maxCards) { cardPtr in
                    mp.baseAddress!.withMemoryRebound(to: UnsafeMutableRawPointer?.self, capacity: TunnelState.maxCards) { mipPtr in
                        glaspen2_rust_tunnel_free_cards(
                            Int32(TunnelState.maxCards), cardPtr, mipPtr,
                            TunnelState.blur.map(UnsafeMutableRawPointer.init),
                            TunnelState.surface.map(UnsafeMutableRawPointer.init))
                    }
                }
            }
        }
    }

    @objc public static func finish() {
        let now = TunnelState.now()
        TunnelState.depths.withUnsafeBufferPointer { depths in
            TunnelState.cardSurf.withUnsafeBufferPointer { cards in
                TunnelState.mipSurf.withUnsafeBufferPointer { mips in
                    cards.baseAddress!.withMemoryRebound(to: UnsafeMutableRawPointer?.self, capacity: TunnelState.maxCards) { cardPtr in
                        mips.baseAddress!.withMemoryRebound(to: UnsafeMutableRawPointer?.self, capacity: TunnelState.maxCards) { mipPtr in
                            glaspen2_rust_tunnel_finish(
                                TunnelState.t0, now, TunnelState.frames,
                                TunnelState.surface.map(UnsafeMutableRawPointer.init),
                                TunnelState.blur.map(UnsafeMutableRawPointer.init),
                                Int32(TunnelState.cards), depths.baseAddress, cardPtr, mipPtr,
                                TunnelState.back ? 1 : 0, TunnelState.travel, TunnelState.k,
                                TunnelState.thick, TunnelState.paneA,
                                Int32(TunnelState.surfW), Int32(TunnelState.surfH))
                        }
                    }
                }
            }
        }
        let prepare = TunnelState.prepare
        let commit = TunnelState.commit
        TunnelState.prepare = nil
        TunnelState.commit = nil
        prepare?()
        commit?()
    }
}


@objc public final class TunnelState: NSObject {
    static var active: Bool {
        get { glaspen2_rust_tunnel_get_active() != 0 }
        set { glaspen2_rust_tunnel_set_active(newValue ? 1 : 0) }
    }
    static var surface: OpaquePointer?
    static var blur: OpaquePointer?
    static var surfW: Int {
        get { Int(glaspen2_rust_tunnel_get_surf_w()) }
        set { glaspen2_rust_tunnel_set_surf_w(Int32(newValue)) }
    }
    static var surfH: Int {
        get { Int(glaspen2_rust_tunnel_get_surf_h()) }
        set { glaspen2_rust_tunnel_set_surf_h(Int32(newValue)) }
    }
    static var cards: Int {
        get { Int(glaspen2_rust_tunnel_get_cards()) }
        set { glaspen2_rust_tunnel_set_cards(Int32(newValue)) }
    }
    static var depths = [Int32](repeating: 0, count: 512)
    static var cardSurf = [OpaquePointer?](repeating: nil, count: 512)
    static var mipSurf = [OpaquePointer?](repeating: nil, count: 512)
    static var back: Bool {
        get { glaspen2_rust_tunnel_get_back() != 0 }
        set { glaspen2_rust_tunnel_set_back(newValue ? 1 : 0) }
    }
    static var travel: Double {
        get { glaspen2_rust_tunnel_get_travel() }
        set { glaspen2_rust_tunnel_set_travel(newValue) }
    }
    static var k: Double {
        get { glaspen2_rust_tunnel_get_k() }
        set { glaspen2_rust_tunnel_set_k(newValue) }
    }
    static var thick: Double {
        get { glaspen2_rust_tunnel_get_thick() }
        set { glaspen2_rust_tunnel_set_thick(newValue) }
    }
    static var paneA: Double {
        get { glaspen2_rust_tunnel_get_pane() }
        set { glaspen2_rust_tunnel_set_pane(newValue) }
    }
    static var dur: Double {
        get { glaspen2_rust_tunnel_get_dur() }
        set { glaspen2_rust_tunnel_set_dur(newValue) }
    }
    static var t0: Double {
        get { glaspen2_rust_tunnel_get_t0() }
        set { glaspen2_rust_tunnel_set_t0(newValue) }
    }
    static var frames: UInt64 {
        get { glaspen2_rust_tunnel_get_frames() }
        set { glaspen2_rust_tunnel_set_frames(newValue) }
    }
    static var timer: DispatchSourceTimer?
    static var prepare: (() -> Void)?
    static var commit: (() -> Void)?

    static let maxCards = 512

    static func now() -> Double {
        Date().timeIntervalSinceReferenceDate
    }
}


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
