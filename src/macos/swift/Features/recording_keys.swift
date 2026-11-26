// S6-3-4b-2c: 录制热键(⌘⌃R/2/3)判定迁 Swift;事件/效果执行仍经 ObjC 薄桥。
import AppKit

@objc public final class RecordingHotkeys: NSObject {
    // kVK_ANSI_R/2/3(macOS 虚拟键码)
    private static let keyR: UInt16 = 15
    private static let key2: UInt16 = 19
    private static let key3: UInt16 = 20

    @objc(handleWithType:keyCode:hasCmdCtrl:)
    public static func handle(type: CGEventType, keyCode: UInt16, hasCmdCtrl: Bool) -> Bool {
        glaspen2_rust_recording_hotkey(type == .keyDown ? 1 : 0, keyCode,
                                        hasCmdCtrl ? 1 : 0) != 0
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
        _ = json.withCString { glaspen2_rust_recording_feed(Int32(slot), $0) }
    }
}
