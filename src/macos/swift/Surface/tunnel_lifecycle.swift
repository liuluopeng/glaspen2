// S7: 隧道动画结束与卡片/模糊/表面释放(状态在 Swift, 释放/收尾在 Rust)。
import AppKit
import Darwin

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
