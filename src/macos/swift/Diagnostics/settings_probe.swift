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

        PenPresets.loadPresets()

        SettingsDispatcher.apply(key: "gridSize", value: "55")
        check("gridSize", glaspen2_router_grid_size() == 55)
        SettingsDispatcher.apply(key: "gridDivider", value: "2")
        check("gridDivider", glaspen2_router_grid_divider() == 2)
        SettingsDispatcher.apply(key: "language", value: "\"en\"")
        check("language", glaspen2_router_lang() == 1)
        SettingsDispatcher.apply(key: "minimap", value: "true")
        check("minimap", glaspen2_router_minimap_enabled() != 0)
        SettingsDispatcher.apply(key: "flipEffect", value: "1")
        check("flipEffect", glaspen2_router_flip_effect() == 1)
        SettingsDispatcher.apply(key: "gifFps", value: "30")
        check("gifFps", glaspen2_router_gif_fps() == 30)
        SettingsDispatcher.apply(key: "softShadow", value: "true")
        check("softShadow", glaspen2_router_soft_shadow() != 0)
        SettingsDispatcher.apply(key: "outline", value: "true")
        check("outline", glaspen2_router_outline_enabled() != 0)
        SettingsDispatcher.apply(key: "frostedGlass", value: "true")
        check("frostedGlass", glaspen2_router_glass_enabled() != 0)
        SettingsDispatcher.apply(key: "invertFps", value: "60")
        check("invertFps", glaspen2_router_invert_fps() == 60)
        SettingsDispatcher.apply(key: "color", value: "2")
        check("color", glaspen2_router_selected_color() == 2)
        SettingsDispatcher.apply(key: "width", value: "5")
        check("width", glaspen2_router_selected_width() == 5)
        SettingsDispatcher.apply(key: "gridFollowStrokes", value: "true")
        check("gridFollowStrokes", glaspen2_router_grid_follow() != 0)

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
