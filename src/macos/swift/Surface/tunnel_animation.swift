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
                                              SurfaceStore.scaleFactor, &preloadMs)
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
