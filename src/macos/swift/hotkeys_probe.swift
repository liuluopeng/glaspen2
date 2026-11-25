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
        // 飘渺隐藏路径: visible=1 + ethereal=1 时 hideNow 必须置 0。
        glaspen2_router_set_ethereal(1)
        glaspen2_router_set_strokes_visible(1)
        let hidden = EtherealMode.hideNow()
        if !hidden || glaspen2_router_strokes_visible() != 0 {
            NSLog("[hotkeys-probe] FAIL ethereal hide (hidden=%d visible=%d)",
                  hidden ? 1 : 0, glaspen2_router_strokes_visible())
            failures += 1
        }
        glaspen2_router_set_ethereal(0)
        glaspen2_router_set_strokes_visible(1)
        print("HOTKEYS_PROBE checked=\(checked) failures=\(failures)")
        return failures == 0 ? 0 : 1
    }
}

@_cdecl("glaspen2_swift_hotkeys_probe_selftest")
public func hotkeysProbeSelftest() -> Int32 { HotkeysProbe.run() }
