// S7: 隧道动画结束与卡片/模糊/表面释放(状态在 Swift)。
import AppKit
import Darwin

@objc public final class TunnelLifecycle: NSObject {
    @objc public static func freeCards() {
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
