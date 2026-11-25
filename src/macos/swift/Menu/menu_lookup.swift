// R27: 菜单项递归查找已迁 glaspen-core/macos_shell; 本文件只做指针/查询桥。
import AppKit

@objc public final class MenuLookup: NSObject {
    @objc(itemWithTag:inMenu:)
    public static func item(tag: Int, inMenu menu: NSMenu?) -> NSMenuItem? {
        guard let menu else { return nil }
        if ProcessInfo.processInfo.environment["GLASPEN2_SWIFT_MENU_LOOKUP"] != "1" {
            let p = glaspen2_rust_menu_item_with_tag(
                Unmanaged.passUnretained(menu).toOpaque(), tag)
            if let p { return Unmanaged<NSMenuItem>.fromOpaque(p).takeUnretainedValue() }
            return nil
        }
        if let found = menu.item(withTag: tag) { return found }
        for item in menu.items {
            if let inner = item.submenu?.item(withTag: tag) { return inner }
        }
        return nil
    }

    @objc(setTitle:tag:inMenu:)
    public static func setTitle(_ title: String, tag: Int, inMenu menu: NSMenu?) -> Int32 {
        guard let menu else { return 0 }
        if ProcessInfo.processInfo.environment["GLASPEN2_SWIFT_MENU_LOOKUP"] != "1" {
            return title.withCString {
                glaspen2_rust_menu_set_item_title(
                    Unmanaged.passUnretained(menu).toOpaque(), tag, $0)
            }
        }
        guard let item = item(tag: tag, inMenu: menu) else { return 0 }
        item.title = title
        return 1
    }

    @objc(stateForTag:inMenu:)
    public static func state(tag: Int, inMenu menu: NSMenu?) -> Int32 {
        guard let menu else { return 0 }
        if ProcessInfo.processInfo.environment["GLASPEN2_SWIFT_MENU_LOOKUP"] != "1" {
            return glaspen2_rust_menu_item_state(
                Unmanaged.passUnretained(menu).toOpaque(), tag)
        }
        return item(tag: tag, inMenu: menu)?.state == .on ? 1 : 0
    }
}
