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
        _ = text.withCString {
            glaspen2_rust_draw_notification($0, bounds.size.width, bounds.size.height)
        }
    }
}

/// R33: Rust 通知绘制探针(离屏渲染并检查非空像素)。
@_cdecl("glaspen2_macos_swift_notification_selftest")
public func glaspen2_macos_swift_notification_selftest() -> Int32 {
    _ = NSApplication.shared
    let w = 400.0, h = 200.0
    let image = NSImage(size: NSSize(width: w, height: h))
    image.lockFocus()
    let ok = "TEST NOTIFICATION".withCString {
        glaspen2_rust_draw_notification($0, w, h) != 0
    }
    image.unlockFocus()
    guard ok, let tiff = image.tiffRepresentation,
          let rep = NSBitmapImageRep(data: tiff),
          let data = rep.bitmapData else {
        print("SWIFT_NOTIFICATION_FAIL rust_draw_or_bitmap")
        return 1
    }
    var nonEmpty = 0
    let count = rep.bytesPerRow * rep.pixelsHigh
    for i in stride(from: 0, to: count, by: 4) where data[i] != 0 || data[i + 1] != 0 || data[i + 2] != 0 { nonEmpty += 1 }
    print("SWIFT_NOTIFICATION_RUST_OK pixels=\(nonEmpty)")
    return nonEmpty > 0 ? 0 : 1
}
