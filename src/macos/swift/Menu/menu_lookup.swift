// S3b-2b: 菜单项递归查找(含一层子菜单),由 ObjC gl_menu_item_with_tag 迁入。
import AppKit

@objc public final class MenuLookup: NSObject {
    @objc(itemWithTag:inMenu:)
    public static func item(tag: Int, inMenu menu: NSMenu?) -> NSMenuItem? {
        guard let menu = menu else { return nil }
        if let found = menu.item(withTag: tag) { return found }
        for item in menu.items {
            if let inner = item.submenu?.item(withTag: tag) { return inner }
        }
        return nil
    }
}
