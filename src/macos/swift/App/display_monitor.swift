// S5-3: 显示器变更 2.5s 防抖(AGENTS 两个坑: 到期重读真实几何; 早退也刷新 pending)。
import AppKit

@objc public final class DisplayChangeMonitor: NSObject {
    private static var timer: DispatchSourceTimer?

    @objc public static func onDisplayChanged() {
        if ProcessInfo.processInfo.environment["GLASPEN2_RUST_DISPLAY_MONITOR"] == "1" {
            guard let screen = NSScreen.main else { return }
            let f = screen.frame
            let nw = Int32(f.size.width), nh = Int32(f.size.height)
            if glaspen2_rust_display_begin(nw, nh) == 0 { return }
            NSLog("[glaspen2] display reconfig: Rust 防抖 2.5s -> %dx%d", nw, nh)
            if timer == nil {
                let t = DispatchSource.makeTimerSource(queue: .main)
                t.setEventHandler {
                    if glaspen2_rust_display_finish() != 0 {
                        NSLog("[glaspen2] display apply by Rust")
                    }
                }
                timer = t
                t.resume()
            }
            timer?.schedule(deadline: .now() + 2.5, repeating: .never, leeway: .milliseconds(200))
            return
        }
        guard let screen = NSScreen.main else { return }
        let frame = screen.frame
        let newW = Int(frame.size.width)
        let newH = Int(frame.size.height)

        var curW: Int32 = 0
        var curH: Int32 = 0
        glaspen2_display_size(&curW, &curH)
        if newW == Int(curW) && newH == Int(curH) { return }

        NSLog("[glaspen2] display reconfig: %dx%d -> %dx%d (防抖 2.5s)", curW, curH, newW, newH)

        if timer == nil {
            let t = DispatchSource.makeTimerSource(queue: .main)
            t.setEventHandler {
                // 到期重读真实几何: 最后一个事件可能携带瞬态读数。
                guard let s = NSScreen.main else { return }
                let f = s.frame
                let w = Int(f.size.width)
                let h = Int(f.size.height)
                var cw: Int32 = 0
                var ch: Int32 = 0
                glaspen2_display_size(&cw, &ch)
                if w == Int(cw) && h == Int(ch) { return }
                glaspen2_display_apply(Int32(w), Int32(h))
            }
            timer = t
            t.resume()
        }
        timer?.schedule(deadline: .now() + 2.5, repeating: .never, leeway: .milliseconds(200))
    }
}

@_cdecl("glaspen2_swift_display_size")
public func glaspen2SwiftDisplaySize(_ w: UnsafeMutablePointer<Int32>?,
                                     _ h: UnsafeMutablePointer<Int32>?) -> Int32 {
    guard let screen = NSScreen.main, let w, let h else { return 0 }
    w.pointee = Int32(screen.frame.size.width)
    h.pointee = Int32(screen.frame.size.height)
    return 1
}
