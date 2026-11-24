// S7: 时光隧道启动(表面/磨砂缓冲/预载建卡/参数/首帧/120Hz 定时器)。
import AppKit
import Darwin

@objc public final class TunnelAnimation: NSObject {
    private static var activeTimer: DispatchSourceTimer?
    @objc(startTunnelGoingNext:curPage:prepare:commit:)
    public static func start(goingNext: Bool, curPage: Int,
                             prepare: @escaping () -> Void,
                             commit: @escaping () -> Void) -> Bool {
        if glaspen2_router_tunnel_active() != 0 { return false }
        let screenW = glaspen2_router_screen_w()
        let screenH = glaspen2_router_screen_h()
        guard glaspen2_router_surface() != nil, screenW > 0, screenH > 0 else { return false }
        let cur = Int(curPage)
        let target = goingNext ? glaspen2_page_neighbor(cur, 1)
                               : glaspen2_page_neighbor(cur, -1)
        if target <= 0 { return false }
        glaspen2_router_tunnel_free_all()
        let scale = glaspen2_router_scale()
        let sW = Int32(Double(screenW) * scale * 0.5)
        let sH = Int32(Double(screenH) * scale * 0.5)
        if sW <= 0 || sH <= 0 { return false }
        if glaspen2_router_tunnel_create_surface(sW, sH) == 0 { return false }
        let tb0 = Date().timeIntervalSinceReferenceDate
        var n = glaspen2_preload_flip_pages(cur, goingNext ? 1 : 0, 4, 1)
        let tc1 = Date().timeIntervalSinceReferenceDate
        if n <= 0 { return false }
        if n > 512 { n = 512 }
        _ = glaspen2_router_tunnel_create_blur(sW / 3, sH / 3)
        let count = Int(n)
        let curIdx = count - 2
        var built = 0
        for i in 0..<count {
            let depth = curIdx - i
            if Double(depth) > 4.5 { continue }
            if Date().timeIntervalSinceReferenceDate - tb0 > 0.30 { break }
            glaspen2_router_tunnel_set_depth(Int32(i), Int32(depth))
            if glaspen2_router_tunnel_make_card(Int32(i), Int32(i),
                                                depth == 0 ? Int(cur) : 0,
                                                depth == 0 ? 1 : 0) == 0 {
                glaspen2_router_tunnel_free_all()
                return false
            }
            built += 1
        }
        glaspen2_router_tunnel_set_cards(Int32(built))
        let tc2 = Date().timeIntervalSinceReferenceDate
        NSLog("[flip] 时光隧道接管: %d 张卡片(cur=%ld → %ld, 方向=%@) 预载%.0fms 建卡%.0fms",
              built, cur, target, goingNext ? "后" : "前",
              (tc1 - tb0) * 1000.0, (tc2 - tc1) * 1000.0)
        glaspen2_router_tunnel_set_motion(goingNext ? 0 : 1, 1.0)
        glaspen2_router_tunnel_set_callbacks(prepare, commit)
        glaspen2_router_tunnel_set_active(1)

        var dur = 0.62
        if let vd = getenv("GLASPEN2_FLIP_DUR"), let s = String(validatingUTF8: vd),
           let v = Double(s), v > 0.05 { dur = v }
        glaspen2_router_tunnel_set_dur(dur)
        if let vk = getenv("GLASPEN2_FLIP_K"), let s = String(validatingUTF8: vk),
           let v = Double(s), v > 0.05, v < 3.0 {
            glaspen2_router_tunnel_set_k(v)
        }
        if let vt = getenv("GLASPEN2_FLIP_THICK"), let s = String(validatingUTF8: vt),
           let v = Double(s), v >= 0.002, v <= 0.10 {
            glaspen2_router_tunnel_set_thick(v)
        }
        if let vga = getenv("GLASPEN2_FLIP_GLASS"), let s = String(validatingUTF8: vga),
           let v = Double(s), v >= 0.0, v <= 0.98 {
            glaspen2_router_tunnel_set_pane_a(v)
        }
        glaspen2_router_tunnel_set_t0_frames(glaspen2_router_tunnel_now())
        glaspen2_router_tunnel_frame_at(0.0)
        glaspen2_router_tunnel_display_now()

        let timer = DispatchSource.makeTimerSource(queue: .main)
        timer.schedule(deadline: .now(),
                       repeating: .microseconds(8333),
                       leeway: .microseconds(1000))
        timer.setEventHandler {
            let t = glaspen2_router_tunnel_now() - glaspen2_router_tunnel_t0()
            let p = t / max(glaspen2_router_tunnel_dur(), 0.001)
            if p >= 1.0 {
                glaspen2_router_tunnel_finish()
                return
            }
            glaspen2_router_tunnel_frame_at(p)
            if glaspen2_router_tunnel_active() != 0 {
                glaspen2_router_tunnel_display_now()
            }
        }
        activeTimer = timer
        glaspen2_router_tunnel_set_timer(Unmanaged.passUnretained(timer).toOpaque())
        timer.resume()
        return true
    }
}
