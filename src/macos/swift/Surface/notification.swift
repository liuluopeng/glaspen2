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

    /// 页信息 HUD: 文案与日期比较单源在 core::pageinfo, 这里只负责显示。
    @objc(showPageInfoWithScreenId:)
    public static func showPageInfo(screenId: Int64) {
        let lang = glaspen2_router_lang_code()
        guard let c = glaspen2_page_info_text(screenId, lang) else { return }
        defer { glaspen2_free_c_string(c) }
        let text = String(cString: c)
        if !text.isEmpty { show(text: text) }
    }

    /// S4-2: 通知文字绘制(36pt 等宽 + 阴影 + 视图内居中)。由 ObjC drawRect 调用,
    /// 使用当前图形上下文; 样式与原 ObjC 实现逐项一致。
    @objc(drawInBounds:)
    public static func draw(in bounds: NSRect) {
        guard let text = current else { return }
        let shadow = NSShadow()
        shadow.shadowColor = NSColor(white: 0, alpha: 0.8)
        shadow.shadowOffset = NSSize(width: 2, height: -2)
        shadow.shadowBlurRadius = 4
        let attrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.monospacedSystemFont(ofSize: 36, weight: .medium),
            .foregroundColor: NSColor.white,
            .shadow: shadow,
        ]
        let ns = text as NSString
        let size = ns.size(withAttributes: attrs)
        let x = (bounds.size.width - size.width) / 2
        let y = (bounds.size.height - size.height) / 2
        ns.draw(at: NSPoint(x: x, y: y), withAttributes: attrs)
    }
}
