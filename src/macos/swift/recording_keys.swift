// S6-3-4b-2c: 录制热键(⌘⌃R/2/3)判定迁 Swift;事件/效果执行仍经 ObjC 薄桥。
import AppKit

@objc public final class RecordingHotkeys: NSObject {
    // kVK_ANSI_R/2/3(macOS 虚拟键码)
    private static let keyR: UInt16 = 15
    private static let key2: UInt16 = 19
    private static let key3: UInt16 = 20

    @objc(handleWithType:keyCode:hasCmdCtrl:)
    public static func handle(type: CGEventType, keyCode: UInt16, hasCmdCtrl: Bool) -> Bool {
        let gif = Int(glaspen2_router_rec_slot_gif())
        let chat = Int(glaspen2_router_rec_slot_chat())
        let down = type == .keyDown
        if !down && keyCode == keyR && glaspen2_router_rec_mode(Int32(gif)) == 1 {
            feedRelease(slot: gif, mode: "Gif"); return true
        }
        if down && hasCmdCtrl && keyCode == keyR {
            feedStart(slot: gif, mode: "Gif"); return true
        }
        if !down && keyCode == key3 && glaspen2_router_rec_mode(Int32(chat)) == 3 {
            feedRelease(slot: chat, mode: "Send"); return true
        }
        if down && glaspen2_router_chat_integration() != 0 && hasCmdCtrl && keyCode == key3 {
            feedStart(slot: chat, mode: "Send"); return true
        }
        if !down && keyCode == key2 && glaspen2_router_rec_mode(Int32(chat)) == 2 {
            feedRelease(slot: chat, mode: "Draft"); return true
        }
        if down && glaspen2_router_chat_integration() != 0 && hasCmdCtrl && keyCode == key2 {
            feedStart(slot: chat, mode: "Draft"); return true
        }
        return false
    }

    private static func feedStart(slot: Int, mode: String) { feed(slot: slot, type: "StartPressed", mode: mode) }
    private static func feedRelease(slot: Int, mode: String) { feed(slot: slot, type: "ReleasePressed", mode: mode) }

    private static func feed(slot: Int, type: String, mode: String) {
        let strokes = glaspen2_router_rec_finish_and_count()
        let payload: [String: Any] = [
            "type": type, "mode": mode, "stroke_count": Int(strokes),
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: payload),
              let json = String(data: data, encoding: .utf8) else { return }
        _ = glaspen2_router_rec_feed(Int32(slot), json)
    }
}
