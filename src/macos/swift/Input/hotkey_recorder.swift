// S7: 快捷键录制状态(状态/超时 generation 在 Rust, Swift 只保留 60s 定时器壳)。
import AppKit

@objc public final class HotkeyRecorder: NSObject {
    @objc public static func setRecording(_ on: Bool) {
        let gen = glaspen2_rust_recording_set(on ? 1 : 0)
        guard on else { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 60.0) {
            if glaspen2_rust_recording_timeout(gen) != 0 {
                NSLog("[hotkey] 录制超时, 恢复全局快捷键")
            }
        }
    }

    public static var isRecording: Bool {
        glaspen2_rust_recording_is_active() != 0
    }
}
