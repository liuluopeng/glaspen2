// S7: 录制状态机事件回喂与 effect 执行(GIF 编码/剪贴板/聊天草稿/直发)。
import AppKit

@objc public final class RecordingEffects: NSObject {
    private static let slotGif: Int32 = 0
    private static let slotChat: Int32 = 1
    private static var pendingGif: Data?

    @objc(feedSlot:json:)
    public static func feed(slot: Int32, json: String) -> Bool {
        guard let out = json.withCString({ glaspen2_recording_event_json(slot, $0) }) else {
            NSLog("[recording] 槽 %d: FFI 返回 NULL(不应发生)", slot)
            return false
        }
        let text = String(cString: out)
        glaspen2_free_c_string(out)
        guard let data = text.data(using: .utf8),
              let parsed = try? JSONSerialization.jsonObject(with: data),
              let effects = parsed as? [Any] else {
            NSLog("[recording] 槽 %d 事件执行失败", slot)
            return false
        }
        if effects.isEmpty { return false }
        for case let effect as NSDictionary in effects { execEffect(slot, effect) }
        return true
    }

    private static func execEffect(_ slot: Int32, _ effect: NSDictionary) {
        guard let kind = effect["kind"] as? String else { return }
        if kind == "ShowHud" {
            if let hud = effect["hud"] as? String {
                let strokes = (effect["strokes"] as? NSNumber)?.int32Value ?? 0
                RecHudPresenter.present(hud: hud, strokes: strokes,
                                        detail: effect["detail"] as? String)
            }
            return
        }
        if kind == "EncodeGif" {
            let start = (effect["start_stroke"] as? NSNumber)?.int32Value ?? 0
            let end = (effect["end_stroke"] as? NSNumber)?.int32Value ?? 0
            let fps = glaspen2_router_gif_fps()
            let resolution = glaspen2_router_gif_resolution()
            let speed = glaspen2_router_gif_speed()
            let endMode = glaspen2_router_gif_end_mode()
            DispatchQueue.global(qos: .userInitiated).async {
                var outLen: Int32 = 0
                let gif = glaspen2_gif_record_end(start, end, fps, resolution,
                                                  speed, endMode, &outLen)
                DispatchQueue.main.async {
                    if let gif, outLen > 0 {
                        pendingGif = Data(bytes: gif, count: Int(outLen))
                        glaspen2_free_rust_bytes(gif, outLen)
                        _ = feed(slot: slotGif, json: simpleEvent("GifEncodeSucceeded"))
                    } else {
                        if let gif { glaspen2_free_rust_bytes(gif, outLen) }
                        _ = feed(slot: slotGif, json: simpleEvent("GifEncodeFailed"))
                    }
                }
            }
            return
        }
        if kind == "Clipboard" {
            let gif = pendingGif
            pendingGif = nil
            if let gif, !gif.isEmpty {
                gif.withUnsafeBytes {
                    GifClipboard.copy(data: $0.bindMemory(to: UInt8.self).baseAddress,
                                      length: Int32(gif.count))
                }
            } else {
                NSLog("[recording] Clipboard effect 没有待写字节")
            }
            _ = feed(slot: slotGif, json: simpleEvent("GifClipboardSucceeded"))
            return
        }
        if kind == "StartChatDraft" {
            let ok = glaspen2_ink_draft_start(Int32(glaspen2_router_screen_w()),
                                              Int32(glaspen2_router_screen_h())) != 0
            _ = feed(slot: slotChat,
                     json: simpleEvent(ok ? "ChatDraftStarted" : "ChatDraftStartFailed"))
            return
        }
        if kind == "StopChatDraft" {
            EventRouter.finishActiveStroke()
            DispatchQueue.global(qos: .userInitiated).async {
                let r = glaspen2_ink_draft_stop()
                var detail = ""
                if let err = glaspen2_ink_draft_last_error(), err.pointee != 0 {
                    detail = String(cString: err)
                }
                DispatchQueue.main.async {
                    _ = feed(slot: slotChat, json: jsonObject([
                        "type": "ChatDraftFinished", "code": r, "detail": detail]))
                }
            }
            return
        }
        if kind == "SendChatStrokes" {
            let start = (effect["start_stroke"] as? NSNumber)?.int32Value ?? 0
            let end = (effect["end_stroke"] as? NSNumber)?.int32Value ?? 0
            DispatchQueue.global(qos: .userInitiated).async {
                let sent = glaspen2_chat_send_strokes(start, end)
                DispatchQueue.main.async {
                    if sent >= 0 {
                        _ = feed(slot: slotChat,
                                 json: jsonObject(["type": "ChatSendSucceeded",
                                                   "strokes": sent]))
                    } else {
                        _ = feed(slot: slotChat, json: simpleEvent("ChatSendFailed"))
                    }
                }
            }
            return
        }
        if kind == "Discard" {
            NSLog("[recording] 丢弃进行中的录制窗口(取消)")
            return
        }
        NSLog("[recording] 未实现的录制 effect: %@", kind)
    }

    private static func jsonObject(_ dict: [String: Any]) -> String {
        guard let data = try? JSONSerialization.data(withJSONObject: dict),
              let text = String(data: data, encoding: .utf8) else { return "{}" }
        return text
    }
    private static func simpleEvent(_ type: String) -> String {
        jsonObject(["type": type])
    }
}
