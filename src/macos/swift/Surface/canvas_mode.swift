// S7: 画布模式切换(翻页 ↔ 无限): 状态/存储/镜头/菜单图标/tap 重装/通知。
import AppKit

@objc public final class CanvasMode: NSObject {
    @objc(applyInfiniteCanvasOn:notify:)
    public static func applyInfinite(on: Bool, notify: Bool) {
        let text = notify ? MenuRuntime.l10n(on ? "canvas.infinite_on" : "canvas.infinite_off") : ""
        _ = text.withCString { glaspen2_rust_apply_infinite(on ? 1 : 0, $0) }
    }
}
