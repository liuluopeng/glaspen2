// S7: 快捷键录制状态(录制期间键盘事件全部放行给 Flutter;60s 兜底恢复)。
import AppKit

@objc public final class HotkeyRecorder: NSObject {
    private static var recording = false
    private static var generation = 0

    @objc public static func setRecording(_ on: Bool) {
        if ProcessInfo.processInfo.environment["GLASPEN2_SWIFT_HOTKEY_RECORDER"] != "1" {
            let gen = glaspen2_rust_recording_set(on ? 1 : 0)
            guard on else { return }
            DispatchQueue.main.asyncAfter(deadline: .now() + 60.0) {
                if glaspen2_rust_recording_timeout(gen) != 0 {
                    NSLog("[hotkey] 录制超时, 恢复全局快捷键")
                }
            }
            return
        }
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

    public static var isRecording: Bool {
        if ProcessInfo.processInfo.environment["GLASPEN2_SWIFT_HOTKEY_RECORDER"] != "1" {
            return glaspen2_rust_recording_is_active() != 0
        }
        return recording
    }
}
