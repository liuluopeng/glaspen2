// S7: 虚拟笔诊断(Glaspen2_VIRTUAL_PEN=1 时后台循环合成笔事件, 供自动化/压测)。
import AppKit
import Darwin

@objc public final class VirtualPen: NSObject {
    private static let kUserData = 0x56504E31 // 与 glaspen2.m 的 kVirtualPenUserData 一致

    private static func post(_ type: CGEventType, _ x: Double, _ y: Double,
                             _ pressure: Double) {
        guard let ev = CGEvent(mouseEventSource: nil, mouseType: type,
                               mouseCursorPosition: CGPoint(x: x, y: y),
                               mouseButton: .left) else { return }
        ev.setIntegerValueField(.mouseEventSubtype, value: 1) // NSEventSubtype.tabletPoint
        ev.setIntegerValueField(CGEventField(rawValue: 42)!,
                                value: Int64(kUserData)) // source user data
        ev.setIntegerValueField(CGEventField(rawValue: 0x2F)!,
                                value: Int64(pressure * 65535.0))
        ev.post(tap: .cghidEventTap)
    }

    @objc public static func maybeStart() {
        guard let v = getenv("GLASPEN2_VIRTUAL_PEN"), v.pointee != 0,
              String(cString: v) != "0" else { return }
        NSLog("[glaspen2] virtual pen enabled (GLASPEN2_VIRTUAL_PEN): 每 12 秒画 3 笔")
        DispatchQueue.global(qos: .userInitiated).async { run() }
    }

    private static func run() {
        var batch = 0
        while true {
            autoreleasepool {
                var s = 0
                while s < 3 && glaspen2_router_enabled() != 0 {
                    let w = glaspen2_router_screen_w()
                    let h = glaspen2_router_screen_h()
                    let x0 = w * 0.22 + Double(s) * w * 0.18
                    let y0 = h * 0.35
                    let n = 400 // 2 秒 @ 200Hz
                    NSLog("[glaspen2] virtual pen: batch %d stroke %d", batch, s)
                    var i = 0
                    while i < 100 && glaspen2_router_enabled() != 0 {
                        let t = Double(i) / 100
                        post(.mouseMoved, x0 + t * w * 0.22, y0, 0)
                        usleep(5000)
                        i += 1
                    }
                    i = 0
                    while i <= n && glaspen2_router_enabled() != 0 {
                        let t = Double(i) / Double(n)
                        let x = x0 + t * w * 0.22
                        let y = y0 + sin(t * 9.42478) * h * 0.12
                        let p = 0.4 + 0.5 * abs(sin(t * 12.566))
                        post(i == 0 ? .leftMouseDown : .leftMouseDragged, x, y, p)
                        usleep(5000)
                        i += 1
                    }
                    post(.leftMouseUp, x0 + w * 0.22, y0, 0)
                    i = 0
                    while i < 100 && glaspen2_router_enabled() != 0 {
                        post(.mouseMoved, x0 + w * 0.22 - Double(i) * 2.0,
                             y0 + Double(i) * 1.0, 0)
                        usleep(5000)
                        i += 1
                    }
                    usleep(200000)
                    s += 1
                }
            }
            batch += 1
            sleep(9)
        }
    }
}
