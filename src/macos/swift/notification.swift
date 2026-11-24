// S4: 屏幕通知 presenter(状态 + 1s 自动清除计时)迁 Swift;绘制仍在 ObjC drawRect。
import AppKit

@objc public final class NotificationPresenter: NSObject {
    private static var current: String?
    private static var timer: DispatchSourceTimer?
    private static var generation = 0

    @objc public static func currentText() -> String? { current }

    @objc(showText:)
    public static func show(text: String) {
        current = text
        glaspen2_notification_redraw()
        timer?.cancel()
        generation += 1
        let gen = generation
        let t = DispatchSource.makeTimerSource(queue: .main)
        t.schedule(deadline: .now() + 1.0)
        t.setEventHandler {
            if gen == generation {
                current = nil
                glaspen2_notification_redraw()
            }
            t.cancel()
        }
        timer = t
        t.resume()
    }
}
