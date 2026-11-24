// S3b-2b-7: 菜单 handler 的薄 delegate 与简单动作迁 Swift(ObjC 只转发)。
import AppKit

@objc public final class MenuHandler: NSObject {
    @objc public static func quitApp() { glaspen2_menu_quit_bridge() }
    @objc public static func menuWillOpen() { glaspen2_menu_will_open_bridge() }
    @objc public static func menuDidClose() { glaspen2_menu_did_close_bridge() }

    /// 语言切换: 状态读写经薄桥, 落库/刷新/通知面板仍走既有 C FFI。
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
}
