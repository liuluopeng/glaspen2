// S7: 菜单文案刷新(颜色/粗细富文本 + 顶层/导出/更多/模式/总开关文案)。
import AppKit

@objc public final class MenuTextUpdater: NSObject {
    @objc public static func update() {
        guard let menu = UIAnchors.menu as? NSMenu else { return }
        func pair(_ zh: String, _ en: String) -> String {
            MenuRuntime.pair(zh, en)
        }
        var colorTitles: [NSAttributedString] = []
        for i in 0..<Int(glaspen2_color_preset_count()) {
            colorTitles.append(MenuRuntime.colorItemTitle(Int32(i)))
        }
        var widthTitles: [NSAttributedString] = []
        for i in 0..<Int(glaspen2_width_preset_count()) {
            widthTitles.append(MenuRuntime.widthItemTitle(Int32(i)))
        }
        let topTitles = [
            pair("保存(含背景)", "Save (with bg)"),
            pair("保存(涂鸦)", "Save (drawing)"),
            pair("新建画布", "New canvas"),
            pair("导出", "Export"),
            "",
            "",
            pair("设置...", "Settings..."),
            pair("更多", "More"),
            "",
            pair("English", "中文"),
            pair("退出", "Quit"),
        ]
        let exportTitles = [
            pair("保存笔记 (Xournal)", "Save Notes (Xournal)"),
            pair("导出无限画布 PDF (分页)", "Export infinite canvas PDF (paged)"),
            pair("导出无限画布 SVG (整幅)", "Export infinite canvas SVG (whole)"),
        ]
        let moreTitles = [
            pair("开机自启", "Launch at login"),
            pair("磨砂玻璃", "Frosted Glass"),
            pair("笔迹描边", "Stroke outline"),
            MenuRuntime.l10n("canvas.mode_infinite") ?? "",
        ]
        let modeTitle = glaspen2_router_menu_state(778) != 0
            ? (MenuRuntime.l10n("canvas.fixed_mode") ?? "")
            : (MenuRuntime.l10n("canvas.ethereal_mode") ?? "")
        let toggleTitle = glaspen2_router_enabled() != 0
            ? pair("关闭涂鸦", "Disable Drawing")
            : pair("开启涂鸦", "Enable Drawing")
        MenuUpdater.updateTexts(menu: menu, colorTitles: colorTitles,
                                widthTitles: widthTitles, topTitles: topTitles,
                                exportTitles: exportTitles, moreTitles: moreTitles,
                                modeTitle: modeTitle, toggleTitle: toggleTitle)
    }
}


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


@objc public final class MenuIconFactory: NSObject {
    @objc(colorDotWithColor:)
    public static func colorDot(_ color: NSColor) -> NSImage {
        let s: CGFloat = 13.0
        let image = NSImage(size: NSSize(width: s, height: s))
        image.lockFocus()
        let dot = NSBezierPath(ovalIn: NSRect(x: 0.5, y: 0.5, width: s - 1, height: s - 1))
        color.setFill()
        dot.fill()
        NSColor(white: 0, alpha: 0.3).setStroke()
        dot.lineWidth = 1.0
        dot.stroke()
        image.unlockFocus()
        return image
    }

    @objc(widthLineWithScale:)
    public static func widthLine(_ scale: Double) -> NSImage {
        let w: CGFloat = 16.0
        let h: CGFloat = 13.0
        let image = NSImage(size: NSSize(width: w, height: h))
        image.lockFocus()
        let lineW = min(5.5, max(1.5, 1.3 + sqrt(scale) * 2.2))
        let path = NSBezierPath()
        path.lineWidth = lineW
        path.lineCapStyle = .round
        path.move(to: NSPoint(x: 2, y: h / 2))
        path.line(to: NSPoint(x: w - 2, y: h / 2))
        NSColor.labelColor.withAlphaComponent(0.9).setStroke()
        path.stroke()
        image.unlockFocus()
        return image
    }

    @objc(menuTextWithIcon:name:)
    public static func menuText(icon: NSImage, name: String) -> NSAttributedString {
        let attachment = NSTextAttachment()
        attachment.image = icon
        attachment.bounds = NSRect(x: 0, y: -2.5, width: icon.size.width, height: icon.size.height)
        let result = NSMutableAttributedString(attributedString: NSAttributedString(attachment: attachment))
        let font = NSFont.menuFont(ofSize: 14)
        result.append(NSAttributedString(string: "   " + name, attributes: [.font: font]))
        return result
    }
}
