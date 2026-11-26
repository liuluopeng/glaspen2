// S6-3-4b-2c: 录制热键(⌘⌃R/2/3)判定迁 Swift;事件/效果执行仍经 ObjC 薄桥。
import AppKit

@objc public final class RecordingHotkeys: NSObject {
    @objc(handleWithType:keyCode:hasCmdCtrl:)
    public static func handle(type: CGEventType, keyCode: UInt16, hasCmdCtrl: Bool) -> Bool {
        glaspen2_rust_recording_hotkey(type == .keyDown ? 1 : 0, keyCode,
                                        hasCmdCtrl ? 1 : 0) != 0
    }


}
