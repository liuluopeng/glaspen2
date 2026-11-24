// S3b-2b-7a: 菜单 handler 的薄 delegate 逻辑迁 Swift(ObjC 方法仅转发)。
import AppKit

@objc public final class MenuHandler: NSObject {
    @objc public static func quitApp() {
        glaspen2_menu_quit_bridge()
    }

    @objc public static func menuWillOpen() {
        glaspen2_menu_will_open_bridge()
    }

    @objc public static func menuDidClose() {
        glaspen2_menu_did_close_bridge()
    }
}
