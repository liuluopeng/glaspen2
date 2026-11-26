// S3b-2b-4/5: 菜单勾选与动态文案刷新,已从 ObjC 迁入。
import AppKit

@objc public final class MenuUpdater: NSObject {
    @objc(updateCheckmarksInMenu:colorCount:widthCount:colorIndex:widthIndex:)
    public static func updateCheckmarks(menu: NSMenu,
                                        colorCount: Int,
                                        widthCount: Int,
                                        colorIndex: Int,
                                        widthIndex: Int) {
        _ = glaspen2_rust_menu_update_checkmarks(
            Unmanaged.passUnretained(menu).toOpaque(), Int32(colorCount),
            Int32(widthCount), Int32(colorIndex), Int32(widthIndex))
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
        if let topData = try? JSONSerialization.data(withJSONObject: topTitles),
           let topJSON = String(data: topData, encoding: .utf8),
           let exportData = try? JSONSerialization.data(withJSONObject: exportTitles),
           let exportJSON = String(data: exportData, encoding: .utf8),
           let moreData = try? JSONSerialization.data(withJSONObject: moreTitles),
           let moreJSON = String(data: moreData, encoding: .utf8) {
            var colorPtrs: [UnsafeMutableRawPointer?] = colorTitles.map { Unmanaged.passUnretained($0).toOpaque() }
            var widthPtrs: [UnsafeMutableRawPointer?] = widthTitles.map { Unmanaged.passUnretained($0).toOpaque() }
            let modeItem = MenuRuntime.menuItem(tag: 778, inMenu: menu)
            let toggleItem = MenuRuntime.menuItem(tag: 888, inMenu: menu)
            let ok = colorPtrs.withUnsafeMutableBufferPointer { cb in
                widthPtrs.withUnsafeMutableBufferPointer { wb in
                    glaspen2_rust_menu_update_texts(
                        Unmanaged.passUnretained(menu).toOpaque(), cb.baseAddress,
                        Int32(colorTitles.count), wb.baseAddress,
                        Int32(widthTitles.count), topJSON, exportJSON, moreJSON,
                        modeTitle, toggleTitle,
                        modeItem.map { Unmanaged.passUnretained($0).toOpaque() },
                        toggleItem.map { Unmanaged.passUnretained($0).toOpaque() })
                }
            }
            _ = ok
        }
    }
}
