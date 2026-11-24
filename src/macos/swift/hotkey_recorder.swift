// S7: 快捷键录制状态(录制期间键盘事件全部放行给 Flutter;60s 兜底恢复)。
import AppKit

@objc public final class HotkeyRecorder: NSObject {
    private static var recording = false
    private static var generation = 0

    @objc public static func setRecording(_ on: Bool) {
        recording = on
        generation += 1
        guard on else { return }
        let gen = generation
        DispatchQueue.main.asyncAfter(deadline: .now() + 60.0) {
            if gen == generation && recording {
                recording = false
                NSLog("[hotkey] 录制超时, 恢复全局快捷键")
            }
        }
    }

    public static var isRecording: Bool { recording }
}
