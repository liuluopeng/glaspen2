// R54: 录制 HUD 文案映射已迁 Rust; 本文件只做通知展示。
import AppKit

@objc public final class RecHudPresenter: NSObject {
    @objc(presentHud:strokes:detail:)
    public static func present(hud: String, strokes: Int32, detail: String?) {
        let text: String? = hud.withCString { h in
            if let detail {
                return detail.withCString { d in
                    guard let p = glaspen2_rust_rec_hud_text(h, strokes, d) else { return nil }
                    defer { free(p) }
                    return String(cString: p)
                }
            }
            guard let p = glaspen2_rust_rec_hud_text(h, strokes, nil) else { return nil }
            defer { free(p) }
            return String(cString: p)
        }
        guard let text else {
            NSLog("[recording] 未实现的 HUD 文案: %@", hud)
            return
        }
        MenuRuntime.notify(text)
    }
}
