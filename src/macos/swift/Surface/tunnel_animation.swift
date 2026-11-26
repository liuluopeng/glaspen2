// S7: 时光隧道启动(状态/表面/预载建卡/参数/首帧/120Hz 定时器)。
import AppKit
import Darwin

@objc public final class TunnelAnimation: NSObject {
    @objc(startTunnelGoingNext:curPage:prepare:commit:)
    public static func start(goingNext: Bool, curPage: Int,
                             prepare: @escaping () -> Void,
                             commit: @escaping () -> Void) -> Bool {
        if TunnelState.active { return false }
        let screenW = glaspen2_router_screen_w()
        let screenH = glaspen2_router_screen_h()
        guard SurfaceStore.current != nil, screenW > 0, screenH > 0 else { return false }
        let cur = Int(curPage)
        let target = goingNext ? glaspen2_page_neighbor(Int64(cur), 1)
                               : glaspen2_page_neighbor(Int64(cur), -1)
        if target <= 0 { return false }
        TunnelLifecycle.freeCards()
        let scale = SurfaceStore.scaleFactor
        let sW = Int(Double(screenW) * scale * 0.5)
        let sH = Int(Double(screenH) * scale * 0.5)
        if sW <= 0 || sH <= 0 { return false }
        TunnelState.surfW = sW
        TunnelState.surfH = sH
        guard let surface = cairo_image_surface_create(cairo_format_t(0),
                                                       Int32(sW), Int32(sH)),
              cairo_surface_status(surface).rawValue == 0 else {
            return false
        }
        TunnelState.surface = surface
        let tb0 = TunnelState.now()
        var n = glaspen2_preload_flip_pages(Int64(cur), goingNext ? 1 : 0, 4, 1)
        let tc1 = TunnelState.now()
        if n <= 0 { return false }
        if Int(n) > TunnelState.maxCards { n = Int32(TunnelState.maxCards) }
        if let blur = cairo_image_surface_create(cairo_format_t(0),
                                                 Int32(sW / 3), Int32(sH / 3)),
           cairo_surface_status(blur).rawValue == 0 {
            TunnelState.blur = blur
        } else {
            TunnelState.blur = nil
        }
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
              (tc1 - tb0) * 1000.0, (tc2 - tc1) * 1000.0)
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
