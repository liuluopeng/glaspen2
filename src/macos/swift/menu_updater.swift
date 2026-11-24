// S3b-2b-4a: 菜单颜色/粗细勾选状态刷新,由 ObjC update_menu_checkmarks 迁入。
import AppKit

@objc public final class MenuUpdater: NSObject {
    @objc(updateCheckmarksInMenu:colorCount:widthCount:colorIndex:widthIndex:)
    public static func updateCheckmarks(menu: NSMenu,
                                        colorCount: Int,
                                        widthCount: Int,
                                        colorIndex: Int,
                                        widthIndex: Int) {
        for i in 0..<max(0, colorCount) {
            menu.item(at: i)?.state = (i == colorIndex) ? .on : .off
        }
        let widthOffset = colorCount + 1 // 颜色项 + 一个分隔符
        for i in 0..<max(0, widthCount) {
            menu.item(at: widthOffset + i)?.state = (i == widthIndex) ? .on : .off
        }
    }
}
