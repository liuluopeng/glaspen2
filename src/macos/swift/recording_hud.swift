// S6/S8 小切片: 录制 HUD 语义 → 本地化文案映射(从 run.m rec_show_hud 迁入)。
import AppKit

@objc public final class RecHudPresenter: NSObject {
    @objc(presentHud:strokes:detail:)
    public static func present(hud: String, strokes: Int32, detail: String?) {
        func text(_ key: String) -> String { glaspen2_menu_l10n(key) }
        func notify(_ s: String) { glaspen2_menu_notify(s) }
        func fmt(_ key: String, _ name: String, _ value: String) -> String {
            text(key).replacingOccurrences(of: "{" + name + "}", with: value)
        }
        switch hud {
        case "GifStartHint": notify(text("recording.gif_start_hint"))
        case "SendStartHint": notify(fmt("recording.send_start_hint", "keys", "⌘⌃3"))
        case "DraftStartHint": notify(fmt("recording.draft_start_hint", "keys", "⌘⌃2"))
        case "DraftStartFailed": notify(text("recording.draft_start_failed"))
        case "GifCopied": notify(text("recording.gif_copied"))
        case "GifNoStrokesOrExportFailed": notify(text("recording.gif_no_strokes_or_export_failed"))
        case "GifClipboardFailed": notify(text("recording.gif_clipboard_failed"))
        case "SendNoStrokes": notify(text("recording.send_no_strokes"))
        case "SendSucceeded": notify(fmt("recording.send_succeeded", "n", String(strokes)))
        case "SendFailed": notify(text("recording.send_failed"))
        case "DraftAccepted": notify(fmt("recording.draft_accepted", "n", String(strokes)))
        case "DraftDeclined": notify(text("recording.draft_declined"))
        case "DraftFailed":
            if let d = detail, !d.isEmpty { notify(d) } else { notify(text("recording.draft_failed")) }
        default: NSLog("[recording] 未实现的 HUD 文案: %@", hud)
        }
    }
}
