// S3b-2b-7: 菜单 handler 的薄 delegate 与简单动作迁 Swift(ObjC 只转发)。
import AppKit

@objc public final class MenuHandler: NSObject {
    @objc public static func quitApp() { glaspen2_menu_quit_bridge() }
    @objc public static func menuWillOpen() { glaspen2_menu_will_open_bridge() }
    @objc public static func menuDidClose() { glaspen2_menu_did_close_bridge() }

    @objc(toggleLanguage)
    public static func toggleLanguage() {
        let next = glaspen2_menu_lang() == 1 ? 0 : 1
        glaspen2_menu_set_lang(Int32(next))
        let code = next == 1 ? "en" : "zh"
        "language".withCString { key in
            code.withCString { value in
                glaspen2_save_string_setting(key, value)
            }
        }
        glaspen2_menu_refresh_texts()
        glaspen2_notify_settings_changed()
    }

    @objc(toggleLaunch)
    public static func toggleLaunch() {
        glaspen2_menu_set_launch(glaspen2_menu_launch_at_login() == 0 ? 1 : 0)
    }

    @objc(toggleGlass)
    public static func toggleGlass() {
        glaspen2_menu_set_glass(glaspen2_menu_glass_enabled() == 0 ? 1 : 0)
    }

    @objc(toggleOutline)
    public static func toggleOutline() {
        glaspen2_menu_apply_outline(glaspen2_menu_outline_enabled() == 0 ? 1 : 0)
    }

    @objc(toggleInfiniteCanvas)
    public static func toggleInfiniteCanvas() {
        glaspen2_menu_apply_infinite(glaspen2_menu_infinite_enabled() == 0 ? 1 : 0)
    }
}
