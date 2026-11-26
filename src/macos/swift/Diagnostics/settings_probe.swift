// S8 自检:设置应用分派 + 状态存储 + 设置快照(隐藏 CLI 入口, 不启动 GUI)。
import AppKit

@objc public final class SettingsProbe: NSObject {
    @objc public static func run() -> Int32 {
        var failures = 0
        func check(_ name: String, _ ok: Bool) {
            if !ok {
                NSLog("[settings-probe] FAIL %@", name)
                failures += 1
            } else {
                print("SETTINGS_PROBE \(name) ok")
            }
        }


        func apply(_ key: String, _ value: String) {
            _ = key.withCString { k in value.withCString { v in glaspen2_rust_apply_setting(k, v) } }
        }

        apply("gridSize", "55")
        check("gridSize", glaspen2_router_grid_size() == 55)
        apply("gridDivider", "2")
        check("gridDivider", glaspen2_router_grid_divider() == 2)
        apply("language", "\"en\"")
        check("language", glaspen2_router_lang() == 1)
        apply("minimap", "true")
        check("minimap", glaspen2_router_minimap_enabled() != 0)
        apply("flipEffect", "1")
        check("flipEffect", glaspen2_router_flip_effect() == 1)
        apply("gifFps", "30")
        check("gifFps", glaspen2_router_gif_fps() == 30)
        apply("softShadow", "true")
        check("softShadow", glaspen2_router_soft_shadow() != 0)
        apply("outline", "true")
        check("outline", glaspen2_router_outline_enabled() != 0)
        apply("frostedGlass", "true")
        check("frostedGlass", glaspen2_router_glass_enabled() != 0)
        apply("invertFps", "60")
        check("invertFps", glaspen2_router_invert_fps() == 60)
        apply("color", "2")
        check("color", glaspen2_router_selected_color() == 2)
        apply("width", "5")
        check("width", glaspen2_router_selected_width() == 5)
        apply("gridFollowStrokes", "true")
        check("gridFollowStrokes", glaspen2_router_grid_follow() != 0)

        // R43: 涂鸦/描边菜单动作(默认 Rust)也做状态自检。
        MenuHandler.setDrawEnabled(enabled: false)
        check("drawDisabled", glaspen2_router_enabled() == 0)
        MenuHandler.setDrawEnabled(enabled: true)
        check("drawEnabled", glaspen2_router_enabled() != 0)
        MenuHandler.setOutlineEnabled(enabled: false)
        check("outlineDisabled", glaspen2_router_outline_enabled() == 0)

        guard let raw = MacosApi.settingsJSON() else {
            NSLog("[settings-probe] FAIL settings_json nil")
            return 1
        }
        let json = String(cString: raw)
        glaspen2_free_c_string(raw)
        check("settings_json.gridSize", json.contains("\"gridSize\""))
        check("settings_json.language", json.contains("\"language\""))

        print("SWIFT_SETTINGS_PROBE_OK failures=\(failures)")
        return failures == 0 ? 0 : 1
    }
}

@_cdecl("glaspen2_swift_settings_probe_selftest")
public func settingsProbeSelftest() -> Int32 {
    SettingsProbe.run()
}
