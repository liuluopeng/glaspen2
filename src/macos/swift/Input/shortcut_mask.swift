// S7: 动作 → 修饰键掩码(只解析 chord 的 cmd/ctrl/alt/shift 部分)。
import AppKit

@objc public final class ShortcutMask: NSObject {
    @objc(maskForAction:)
    public static func maskFor(action: String) -> UInt {
        guard let chord = MenuRuntime.shortcutChord(action),
              !chord.isEmpty else { return 0 }
        var mask: UInt = 0
        for part in chord.components(separatedBy: "+") {
            switch part {
            case "cmd": mask |= NSEvent.ModifierFlags.command.rawValue
            case "ctrl": mask |= NSEvent.ModifierFlags.control.rawValue
            case "alt": mask |= NSEvent.ModifierFlags.option.rawValue
            case "shift": mask |= NSEvent.ModifierFlags.shift.rawValue
            default: break
            }
        }
        return mask
    }
}
