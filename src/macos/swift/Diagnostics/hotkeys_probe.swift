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
        // 显式空串 = 用户解绑, 不能在下次启动被默认值覆盖。
        SettingsGlue.applyShortcuts(saved: ["undo": ""])
        if ShortcutStore.action(forChord: "cmd+ctrl+Z") != nil {
            NSLog("[hotkeys-probe] FAIL explicit unbind fell back to default (undo -> cmd+ctrl+Z)")
            failures += 1
        }
        if ShortcutStore.chord(forAction: "undo") != "" {
            NSLog("[hotkeys-probe] FAIL explicit unbind not stored as empty (got %@)", ShortcutStore.chord(forAction: "undo") ?? "nil")
            failures += 1
        }
        // 换色/换粗细的“上一项”初值必须与旧 ObjC 一致(9 黑 / 2 细), 否则重启后首次交换落到别处。
        if glaspen2_router_prev_color_index() != 9 || glaspen2_router_prev_width_index() != 2 {
            NSLog("[hotkeys-probe] FAIL swap defaults prevColor=%d prevWidth=%d",
                  glaspen2_router_prev_color_index(), glaspen2_router_prev_width_index())
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
        // 库里没存 glass_alpha(0) 时, 启动恢复不得把默认透明度覆盖成 0。
        glaspen2_router_startup_glass(0.0, 1)
        if glaspen2_router_glass_opacity() <= 0.0 {
            NSLog("[hotkeys-probe] FAIL glass default opacity was overwritten to 0")
            failures += 1
        }
        print("HOTKEYS_PROBE checked=\(checked) failures=\(failures)")
        return failures == 0 ? 0 : 1
    }
}

@_cdecl("glaspen2_swift_hotkeys_probe_selftest")
public func hotkeysProbeSelftest() -> Int32 { HotkeysProbe.run() }
