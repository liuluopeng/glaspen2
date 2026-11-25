// S8 自检: 快捷键默认表必须全部装进生效表(chord → action 反查一致)。
import AppKit

@objc public final class HotkeysProbe: NSObject {
    @objc public static func run() -> Int32 {
        SettingsGlue.loadShortcuts()
        let defaults = ShortcutTable.defaults()
        let actions = ShortcutTable.actions()
        var checked = 0
        var failures = 0
        for case let action as String in actions {
            guard let chord = defaults[action] as? String, !chord.isEmpty else { continue }
            checked += 1
            let mapped = ShortcutStore.action(forChord: chord)
            if mapped != action {
                NSLog("[hotkeys-probe] FAIL %@=%@ -> %@", action, chord, mapped ?? "nil")
                failures += 1
            }
        }
        if ShortcutStore.action(forChord: "cmd+ctrl+D") != "settings" {
            NSLog("[hotkeys-probe] FAIL cmd+ctrl+D -> settings")
            failures += 1
        }
        print("HOTKEYS_PROBE checked=\(checked) failures=\(failures)")
        return failures == 0 ? 0 : 1
    }
}

@_cdecl("glaspen2_swift_hotkeys_probe_selftest")
public func hotkeysProbeSelftest() -> Int32 { HotkeysProbe.run() }
