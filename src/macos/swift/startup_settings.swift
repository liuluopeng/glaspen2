// S7: 启动设置恢复(语言/笔色宽/玻璃/网格/阴影/描边/反色/翻转/气泡)。
import AppKit
import Darwin

@objc public final class StartupSettings: NSObject {
    @objc public static func restorePartA() {
        if let v = loadString("language") {
            glaspen2_router_startup_set_lang(v == "en" ? 1 : 0)
            MenuTextUpdater.update()
        }
        glaspen2_router_pageview_update()

        var sr = 0.0, sg = 0.0, sb = 0.0, sw = 0.0
        if glaspen2_load_settings_parts(&sr, &sg, &sb, &sw) != 0 {
            let colorIdx = glaspen2_nearest_color_index(sr, sg, sb)
            let widthIdx = glaspen2_nearest_width_index(sw)
            var r = sr, g = sg, b = sb
            glaspen2_color_preset_rgb(colorIdx, &r, &g, &b)
            glaspen2_router_startup_apply_pen(r, g, b, sw, colorIdx, widthIdx)
        }
        glaspen2_router_update_status_icon_state()
        MenuGlue.updateCheckmarks()
        SettingsGlue.loadShortcuts()

        let glassMilli = glaspen2_load_bool_setting("glass_alpha")
        var opacity = 0.0
        if glassMilli > 0 { opacity = Double(glassMilli) / 1000.0 }
        let glassOn = glaspen2_load_bool_setting("glass_enabled") != 0
        glaspen2_router_startup_glass(opacity, glassOn ? 1 : 0)
        MenuHandler.applyGlass()

        let gridOn = glaspen2_load_bool_setting("grid") != 0
        var gridSize = 0.0
        if let v = loadString("grid_size"), let d = Double(v), d >= 10, d <= 200 {
            gridSize = d
        }
        let gridFollow = glaspen2_load_bool_setting("grid_follow_strokes") != 0
        if let v = loadString("glass_follow_strokes") {
            glaspen2_router_startup_set_glass_follow(Int32(v) != 0 ? 1 : 0)
        }
        var divider: Int32 = -1
        if let v = loadString("grid_divider"), let d = Int32(v), d >= 0, d <= 4 {
            divider = d
        }
        glaspen2_router_startup_grid(gridOn ? 1 : 0, gridSize,
                                     gridFollow ? 1 : 0, divider)

        let shadow = glaspen2_load_bool_setting("soft_shadow") != 0
        glaspen2_router_startup_shadow(shadow ? 1 : 0)
        glaspen2_set_soft_shadow(shadow ? 1 : 0)
        let outline = glaspen2_load_bool_setting("outline") != 0
        glaspen2_router_startup_outline(outline ? 1 : 0)
        glaspen2_set_stroke_outline(outline ? 1 : 0)
        glaspen2_router_update_menu_state(667, outline ? 1 : 0)

        var fps: Int32 = -1
        if let v = loadString("invert_fps"), let f = Int32(v),
           f == 10 || f == 30 || f == 60 || f == 100 { fps = f }
        let invertInk = glaspen2_load_bool_setting("invert_ink") != 0
        glaspen2_router_startup_invert(fps, invertInk ? 1 : 0)
        if invertInk { glaspen2_router_invert_apply(1) }

        var flip: Int32 = -1
        if let v = loadString("flip_effect"), let f = Int32(v), f >= 0, f <= 1 {
            flip = f
        }
        if let env = getenv("GLASPEN2_FLIP_EFFECT") {
            flip = String(cString: env) == "1" ? 1 : 0
        }
        if flip >= 0 { glaspen2_router_startup_flip(flip) }

        glaspen2_bubble_set_capture(glaspen2_load_bool_setting("show_bubble_tab") != 0 ? 1 : 0)
    }

    private static func loadString(_ key: String) -> String? {
        guard let v = key.withCString({ glaspen2_load_string_setting($0) }) else {
            return nil
        }
        let s = String(cString: v)
        glaspen2_free_c_string(v)
        return s
    }
}
