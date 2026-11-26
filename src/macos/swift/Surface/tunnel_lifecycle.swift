// S7: 隧道动画结束与卡片/模糊/表面释放(状态在 Swift)。
import AppKit
import Darwin

@objc public final class TunnelLifecycle: NSObject {
    @objc public static func freeCards() {
        if ProcessInfo.processInfo.environment["GLASPEN2_SWIFT_TUNNEL_LIFECYCLE"] != "1" {
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
            return
        }
        for i in 0..<TunnelState.maxCards {
            if let s = TunnelState.cardSurf[i] { cairo_surface_destroy(s) }
            TunnelState.cardSurf[i] = nil
            if let m = TunnelState.mipSurf[i] { cairo_surface_destroy(m) }
            TunnelState.mipSurf[i] = nil
            TunnelState.depths[i] = 0
        }
        if let b = TunnelState.blur { cairo_surface_destroy(b); TunnelState.blur = nil }
        OverlayTunnelRenderer.clear()
        if let s = TunnelState.surface { cairo_surface_destroy(s); TunnelState.surface = nil }
        TunnelState.cards = 0
    }

    @objc public static func finish() {
        if ProcessInfo.processInfo.environment["GLASPEN2_SWIFT_TUNNEL_LIFECYCLE"] != "1" {
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
            return
        }
        TunnelState.timer?.cancel()
        TunnelState.timer = nil
        TunnelFrameRenderer.render(p: 1.0)
        if getenv("GLASPEN2_FLIP_FPS") != nil {
            let secs = TunnelState.now() - TunnelState.t0
            let frames = TunnelState.frames
            NSLog("[flip] 实测 %.1f fps (%llu 帧 / %.3fs)",
                  secs > 0.001 ? Double(frames) / secs : 0.0, frames, secs)
        }
        freeCards()
        TunnelState.active = false
        MenuHandler.applyGlass()
        let prepare = TunnelState.prepare
        let commit = TunnelState.commit
        TunnelState.prepare = nil
        TunnelState.commit = nil
        prepare?()
        commit?()
    }
}
