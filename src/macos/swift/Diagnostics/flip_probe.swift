// S7: 翻页诊断探针(Glaspen2_FLIP_EVERY=n 秒一跳, 首跳建页, 之后往返翻)。
import AppKit
import Darwin

@objc public final class FlipProbe: NSObject {
    private static var timer: DispatchSourceTimer?
    private static var back = false
    private static var first = true

    @objc public static func maybeStart() {
        guard let v = getenv("GLASPEN2_FLIP_EVERY"), let raw = String(validatingUTF8: v),
              let every = Int(raw), every > 0 else { return }
        NSLog("[glaspen2] flip probe: 每 %d 秒一跳(第1跳建页, 之后往返翻)", every)
        let src = DispatchSource.makeTimerSource(queue: .main)
        src.schedule(deadline: .now() + .seconds(every),
                     repeating: .seconds(every),
                     leeway: .milliseconds(200))
        src.setEventHandler {
            if first {
                first = false
                var want = 0
                if let vpn = getenv("GLASPEN2_PROBE_PAGES"),
                   let s = String(validatingUTF8: vpn), let n = Int(s) {
                    want = n
                }
                _ = glaspen2_debug_insert_pages(Int32(want),
                                                Int32(glaspen2_router_screen_w()),
                                                Int32(glaspen2_router_screen_h()))
                NSLog("[glaspen2] probe 建页 %d 页(空白, 直插)", want)
                return
            }
            NSLog("[glaspen2] probe 翻页 back=%d", back ? 1 : 0)
            _ = glaspen2_router_perform_hotkey(back ? 0x32 : 0x12)
            back.toggle()
        }
        timer = src
        src.resume()
    }
}
