// S7: 隧道动画结束与卡片/模糊/表面释放。
import AppKit
import Darwin

@objc public final class TunnelLifecycle: NSObject {
    private static let maxCards = 512

    /// 释放全部卡片、mip、模糊缓冲与隧道表面。
    @objc public static func freeCards() {
        for i in 0..<maxCards {
            if let p = glaspen2_router_tunnel_take_card(Int32(i)) {
                cairo_surface_destroy(OpaquePointer(p))
            }
            if let p = glaspen2_router_tunnel_take_mip(Int32(i)) {
                cairo_surface_destroy(OpaquePointer(p))
            }
        }
        if let p = glaspen2_router_tunnel_take_blur() {
            cairo_surface_destroy(OpaquePointer(p))
        }
        OverlayTunnelRenderer.clear()
        if let p = glaspen2_router_tunnel_take_surface() {
            cairo_surface_destroy(OpaquePointer(p))
        }
        glaspen2_router_tunnel_set_cards(0)
    }

    /// 动画结束: 取消定时器 → 画最后一帧 → 统计 → 释放 → 恢复玻璃 → 执行回调。
    @objc public static func finish() {
        glaspen2_router_tunnel_cancel_timer()
        glaspen2_router_tunnel_render_final()
        if getenv("GLASPEN2_FLIP_FPS") != nil {
            let secs = glaspen2_router_tunnel_now() - glaspen2_router_tunnel_t0()
            let frames = glaspen2_router_tunnel_frames()
            NSLog("[flip] 实测 %.1f fps (%llu 帧 / %.3fs)",
                  secs > 0.001 ? Double(frames) / secs : 0.0, frames, secs)
        }
        freeCards()
        glaspen2_router_tunnel_set_active(0)
        MenuHandler.applyGlass()
        glaspen2_router_tunnel_run_callbacks()
    }
}
