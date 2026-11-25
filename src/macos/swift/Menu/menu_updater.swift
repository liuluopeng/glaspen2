// S3b-2b-4/5: 菜单勾选与动态文案刷新,已从 ObjC 迁入。
import AppKit

@objc public final class MenuUpdater: NSObject {
    @objc(updateCheckmarksInMenu:colorCount:widthCount:colorIndex:widthIndex:)
    public static func updateCheckmarks(menu: NSMenu,
                                        colorCount: Int,
                                        widthCount: Int,
                                        colorIndex: Int,
                                        widthIndex: Int) {
        if ProcessInfo.processInfo.environment["GLASPEN2_SWIFT_MENU_STATE"] != "1",
           glaspen2_rust_menu_update_checkmarks(
               Unmanaged.passUnretained(menu).toOpaque(), Int32(colorCount),
               Int32(widthCount), Int32(colorIndex), Int32(widthIndex)) != 0 {
            return
        }
        for i in 0..<max(0, colorCount) {
            menu.item(at: i)?.state = (i == colorIndex) ? .on : .off
        }
        let widthOffset = colorCount + 1 // 颜色项 + 一个分隔符
        for i in 0..<max(0, widthCount) {
            menu.item(at: widthOffset + i)?.state = (i == widthIndex) ? .on : .off
        }
    }

    /// 动态文案刷新(语言切换/状态变化)。索引契约与旧 ObjC 实现逐项一致:
    /// 颜色 0..<n、分隔、粗细 n+1..、分隔、顶层 base 起。文案由 ObjC 侧 L()/gl_text
    /// 预先算好传入, Swift 只负责按索引/子菜单/按钮 tag 写回。
    @objc(updateTextsInMenu:colorTitles:widthTitles:topTitles:exportTitles:moreTitles:modeTitle:toggleTitle:)
    public static func updateTexts(menu: NSMenu,
                                   colorTitles: [NSAttributedString],
                                   widthTitles: [NSAttributedString],
                                   topTitles: [String],
                                   exportTitles: [String],
                                   moreTitles: [String],
                                   modeTitle: String,
                                   toggleTitle: String) {
        let colorCount = colorTitles.count
        for i in 0..<colorCount {
            menu.item(at: i)?.attributedTitle = colorTitles[i]
        }
        let widthBase = colorCount + 1
        for i in 0..<widthTitles.count {
            menu.item(at: widthBase + i)?.attributedTitle = widthTitles[i]
        }
        let base = widthBase + widthTitles.count + 1
        // 顶层固定 11 个位置(空串表示该位置不需要刷新)
        for (offset, title) in topTitles.enumerated() where !title.isEmpty {
            menu.item(at: base + offset)?.title = title
        }
        if let exportMenu = menu.item(at: base + 3)?.submenu {
            for (i, title) in exportTitles.enumerated() where i < exportMenu.items.count {
                exportMenu.item(at: i)?.title = title
            }
        }
        if let moreMenu = menu.item(at: base + 7)?.submenu {
            // 0 开机自启 / 1 磨砂玻璃 / 2 笔迹描边 / 3 无限画布;
            // 第 4 项“画布模式”文案随状态走, 由下方 tag 778 单独更新。
            for (i, title) in moreTitles.enumerated() where i < moreMenu.items.count {
                moreMenu.item(at: i)?.title = title
            }
        }
        if let modeItem = MenuLookup.item(tag: 778, inMenu: menu) {
            modeItem.title = modeTitle
        }
        if let toggleItem = MenuLookup.item(tag: 888, inMenu: menu) {
            toggleItem.title = toggleTitle
        }
    }
}
