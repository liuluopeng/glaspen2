// S7: 菜单/快捷键/飘渺/压感/设置面板剩余薄桥与运行时查询(Swift 化, 取代 parts/menu.m)。
import AppKit

@objc public final class MenuRuntime: NSObject {
    static func l10n(_ key: String) -> String {
        guard let p = key.withCString({ sw_text($0) }) else { return key }
        defer { glaspen2_free_c_string(p) }
        return String(cString: p)
    }
    static func pair(_ zh: String, _ en: String) -> String { glaspen2_router_lang() == 0 ? zh : en }
    static func notify(_ text: String) { notificationShowText(text: text) }
    static func colorItemTitle(_ idx: Int32) -> NSAttributedString {
        colorItemTitle(index: idx)
    }
    static func widthItemTitle(_ idx: Int32) -> NSAttributedString {
        widthItemTitle(index: idx)
    }
    static func shortcutInstall(values: NSDictionary, lookup: NSDictionary) {
        let v = ProtoWire.encodeStringMap(values as? [String: String] ?? [:])
        let l = ProtoWire.encodeStringMap(lookup as? [String: String] ?? [:])
        v.withUnsafeBufferPointer { vp in l.withUnsafeBufferPointer { lp in
            glaspen2_shortcut_store_install(vp.baseAddress, vp.count, lp.baseAddress, lp.count)
        } }
    }

    static func shortcutChord(forAction action: String) -> String? {
        guard let p = action.withCString({ glaspen2_shortcut_store_chord($0) }) else { return nil }
        defer { glaspen2_free_c_string(p) }
        return String(cString: p)
    }

    static func shortcutAction(forChord chord: String) -> String? {
        guard let p = chord.withCString({ glaspen2_shortcut_store_action_for_chord($0) }) else { return nil }
        defer { glaspen2_free_c_string(p) }
        return String(cString: p)
    }

    static var shortcutStoreEmpty: Bool { glaspen2_shortcut_store_is_empty() != 0 }

    static func actionForChord(_ chord: String) -> String? {
        MenuRuntime.shortcutAction(forChord: chord)
    }
    static func shortcutChord(_ action: String) -> String? {
        MenuRuntime.shortcutChord(forAction: action)
    }
    /// R7: 动作 → NSEvent.ModifierFlags.rawValue(解析在 core::chord)。
    static func applyPanelTransparent(on: Bool) {
        guard let window = UIAnchors.settingsWindow else { return }
        settingsRunOnMain {
            window.isOpaque = !on
            window.backgroundColor = on ? .clear : .windowBackgroundColor
        }
    }

    static func menuItem(tag: Int, inMenu menu: NSMenu?) -> NSMenuItem? {
        guard let menu else { return nil }
        let p = glaspen2_rust_menu_item_with_tag(Unmanaged.passUnretained(menu).toOpaque(), tag)
        guard let p else { return nil }
        return Unmanaged<NSMenuItem>.fromOpaque(p).takeUnretainedValue()
    }

    static func colorItemTitle(index: Int32) -> NSAttributedString {
        var r = 0.0, g = 0.0, b = 0.0
        glaspen2_color_preset_rgb(index, &r, &g, &b)
        let color = NSColor(calibratedRed: r, green: g, blue: b, alpha: 1.0)
        let icon = MenuIconFactory.colorDot(color)
        return MenuIconFactory.menuText(icon: icon, name: colorDisplayName(index: Int(index)))
    }

    static func widthItemTitle(index: Int32) -> NSAttributedString {
        let scale = glaspen2_width_preset_value(index)
        let icon = MenuIconFactory.widthLine(scale)
        return MenuIconFactory.menuText(icon: icon, name: widthDisplayName(index: Int(index)))
    }

    static func colorDisplayName(index: Int) -> String {
        guard let p = glaspen2_color_name(Int32(index), glaspen2_router_lang()) else { return "" }
        defer { glaspen2_free_c_string(p) }
        return String(cString: p)
    }

    static func widthDisplayName(index: Int) -> String {
        guard let p = glaspen2_width_name(Int32(index), glaspen2_router_lang()) else { return "" }
        defer { glaspen2_free_c_string(p) }
        return String(cString: p)
    }

    static func maskFor(action: String) -> UInt {
        guard let chord = MenuRuntime.shortcutChord(forAction: action), !chord.isEmpty else { return 0 }
        return UInt(chord.withCString { glaspen2_chord_modifier_mask($0) })
    }

    static func shortcutReserved(_ chord: String) -> Bool {
        chord.withCString { glaspen2_chord_reserved($0) != 0 }
    }
    static func shortcutsEffectiveJSON() -> String {
        if MenuRuntime.shortcutStoreEmpty {
            let empty = ProtoWire.encodeStringMap([:])
            _ = empty.withUnsafeBufferPointer { glaspen2_shortcut_apply_pb($0.baseAddress, $0.count) }
        }
        var len = 0
        guard let p = glaspen2_shortcut_store_values_pb(&len) else { return "{}" }
        defer { glaspen2_macos_free_bytes(p) }
        let bytes = Array(UnsafeBufferPointer(start: p, count: len))
        let map = ProtoWire.decodeStringMap(bytes)
        guard let data = try? JSONSerialization.data(withJSONObject: map),
              let json = String(data: data, encoding: .utf8) else { return "{}" }
        return json
    }
}

// C ABI 薄桥(供剩余 ObjC 与 Swift 既有 Bridging 声明使用)
@_cdecl("glaspen2_menu_refresh_texts")
public func menuRefreshTexts() { MenuTextUpdater.update() }
@_cdecl("glaspen2_menu_save_with_bg")
public func menuSaveWithBg() { SaveBackground.save() }

// R43: Rust 侧菜单通知桥(NSString* -> NotificationPresenter)。



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
        let top = ProtoWire.encodeStringList(topTitles)
        let export = ProtoWire.encodeStringList(exportTitles)
        let more = ProtoWire.encodeStringList(moreTitles)
        var colorPtrs: [UnsafeMutableRawPointer?] = colorTitles.map { Unmanaged.passUnretained($0).toOpaque() }
        var widthPtrs: [UnsafeMutableRawPointer?] = widthTitles.map { Unmanaged.passUnretained($0).toOpaque() }
        let modeItem = MenuRuntime.menuItem(tag: 778, inMenu: menu)
        let toggleItem = MenuRuntime.menuItem(tag: 888, inMenu: menu)
        let ok = colorPtrs.withUnsafeMutableBufferPointer { cb in
            widthPtrs.withUnsafeMutableBufferPointer { wb in
                top.withUnsafeBufferPointer { tb in
                    export.withUnsafeBufferPointer { eb in
                        more.withUnsafeBufferPointer { mb in
                            glaspen2_rust_menu_update_texts(
                                Unmanaged.passUnretained(menu).toOpaque(), cb.baseAddress,
                                Int32(colorTitles.count), wb.baseAddress,
                                Int32(widthTitles.count),
                                tb.baseAddress, tb.count, eb.baseAddress, eb.count,
                                mb.baseAddress, mb.count,
                                modeTitle, toggleTitle,
                                modeItem.map { Unmanaged.passUnretained($0).toOpaque() },
                                toggleItem.map { Unmanaged.passUnretained($0).toOpaque() })
                        }
                    }
                }
            }
        }
        _ = ok
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


// S7: 物理/面板快捷键分发(清空/翻页/导出/撤销/复制/玻璃/飘渺/设置)。
import AppKit



// S3b-2b-2: 菜单颜色/粗细条目的批量构建,由 run.m 建菜单循环迁入。
//
// 边界(docs/swift-boundary.md): 富文本标题(内嵌图标 + i18n 名称)仍由 ObjC
// 侧预构(gl_color_item_text / gl_width_item_text), 这里只负责机械部分:
//   建 NSMenuItem → 覆写 attributedTitle → target/tag。
// 三条契约不变(菜单点击行为依赖它们):
//   action = selectColor: / selectWidth:(GlaspenMenuHandler 实现, 逐字一致)
//   target = g_menuHandler(全局强引用, ObjC 侧保证存活)
//   tag    = 预设下标 i
import AppKit

@objc public final class MenuBuilder: NSObject {
    /// 颜色预设条目: action=selectColor:, target=handler, tag=index。
    @objc(addColorItemsToMenu:handler:count:titles:)
    public static func addColorItems(toMenu menu: NSMenu?,
                                     handler: AnyObject?,
                                     count: Int,
                                     titles: [NSAttributedString]) {
        addItems(toMenu: menu, handler: handler, count: count, titles: titles,
                 action: Selector("selectColor:"))
    }

    /// 粗细预设条目: action=selectWidth:, target=handler, tag=index。
    @objc(addWidthItemsToMenu:handler:count:titles:)
    public static func addWidthItems(toMenu menu: NSMenu?,
                                     handler: AnyObject?,
                                     count: Int,
                                     titles: [NSAttributedString]) {
        addItems(toMenu: menu, handler: handler, count: count, titles: titles,
                 action: Selector("selectWidth:"))
    }

    /// 共用建项: 与 ObjC 原循环逐项等价(先用空 title 建项, 再覆写 attributedTitle)。
    /// count 与 titles 数量理论上一一对应(调用方按同一预设表构造); 不等时取
    /// 较小值, 防御式钳制, 避免越界。
    private static func addItems(toMenu menu: NSMenu?,
                                 handler: AnyObject?,
                                 count: Int,
                                 titles: [NSAttributedString],
                                 action: Selector) {
        guard let menu = menu else { return }
        let n = min(max(count, 0), titles.count)
        guard n > 0 else { return }
        var ptrs: [UnsafeMutableRawPointer?] = titles.prefix(n).map { Unmanaged.passUnretained($0).toOpaque() }
        let handlerPtr = handler.map { Unmanaged.passUnretained($0).toOpaque() }
        let actionName = NSStringFromSelector(action)
        _ = ptrs.withUnsafeMutableBufferPointer { buf in
            glaspen2_rust_menu_add_items(
                Unmanaged.passUnretained(menu).toOpaque(), handlerPtr,
                Int32(n), buf.baseAddress, Int32(n), actionName)
        }
    }
}

// ── 隐藏自检入口(不启动 GUI): Rust `glaspen2 --swift-menu-probe` 调用 ──
// 用 3 条假富文本标题跑两个 builder, 校验 count / tag 顺序 / action selector /
// target 身份 / attributedTitle 文案, 以及 nil menu、count 钳制等防御分支。
// 不触发任何 action(只读 selector), 因此不需要真实的 GlaspenMenuHandler。
@_cdecl("glaspen2_macos_swift_menu_selftest")
public func glaspen2_macos_swift_menu_selftest() -> Int32 {
    let app = NSApplication.shared
    app.setActivationPolicy(.accessory)

    func fakeTitles(_ prefix: String) -> [NSAttributedString] {
        (0..<3).map { i in
            NSAttributedString(string: "\(prefix)-\(i)",
                               attributes: [.font: NSFont.menuFont(ofSize: 14)])
        }
    }

    /// 校验一段菜单项: 数量 / tag / action / target / attributedTitle 文案。
    func verify(_ items: [NSMenuItem],
                titles: [NSAttributedString],
                action: Selector,
                handler: NSObject,
                label: String) -> Bool {
        var ok = items.count == titles.count
        for (i, item) in items.enumerated() {
            if item.tag != i { ok = false }
            if item.action != action { ok = false }
            if !(item.target === handler) { ok = false }
            if item.attributedTitle?.string != titles[i].string { ok = false }
        }
        let tags = items.map { String($0.tag) }.joined(separator: ",")
        let actions = items.map { NSStringFromSelector($0.action ?? Selector("")) }
            .joined(separator: ",")
        let titleStrings = items.map { $0.attributedTitle?.string ?? "nil" }
            .joined(separator: ",")
        print("SWIFT_MENU_\(label) count=\(items.count) tags=\(tags) "
              + "actions=\(actions) target_identity=\(items.allSatisfy { $0.target === handler }) "
              + "titles=\(titleStrings) ok=\(ok)")
        return ok
    }

    var failures = 0
    let handler = NSObject()

    // 1) 颜色条目: 空菜单, 3 条假标题 → 追加 3 项
    let colorTitles = fakeTitles("color")
    let colorMenu = NSMenu()
    MenuBuilder.addColorItems(toMenu: colorMenu, handler: handler,
                              count: colorTitles.count, titles: colorTitles)
    if !verify(colorMenu.items, titles: colorTitles,
               action: Selector("selectColor:"), handler: handler, label: "COLOR") {
        failures += 1
    }

    // 2) 粗细条目: 先放一个分隔符(复刻真实菜单里"颜色/粗细之间"的既有项),
    //    验证 builder 是追加而非重建, tag/selector 仍按 0..<count。
    let widthTitles = fakeTitles("width")
    let widthMenu = NSMenu()
    widthMenu.addItem(NSMenuItem.separator())
    MenuBuilder.addWidthItems(toMenu: widthMenu, handler: handler,
                              count: widthTitles.count, titles: widthTitles)
    if widthMenu.items.count != widthTitles.count + 1 {
        print("SWIFT_MENU_WIDTH_FAIL count=\(widthMenu.items.count)")
        failures += 1
    } else if !verify(Array(widthMenu.items.dropFirst()), titles: widthTitles,
                      action: Selector("selectWidth:"), handler: handler,
                      label: "WIDTH") {
        failures += 1
    }

    // 3) 防御分支: nil menu 不崩; count > titles 时钳到 titles 数量;
    //    count < 0 时不建项。
    MenuBuilder.addColorItems(toMenu: nil, handler: nil, count: 3, titles: colorTitles)
    let clampMenu = NSMenu()
    MenuBuilder.addColorItems(toMenu: clampMenu, handler: handler, count: 5,
                              titles: colorTitles)
    let negMenu = NSMenu()
    MenuBuilder.addWidthItems(toMenu: negMenu, handler: handler, count: -1,
                              titles: widthTitles)
    let edgeOK = clampMenu.items.count == 3 && negMenu.items.count == 0
    print("SWIFT_MENU_EDGE nil_menu=ok clamped_count=\(clampMenu.items.count) "
          + "negative_count=\(negMenu.items.count) ok=\(edgeOK)")
    if !edgeOK { failures += 1 }

    print(failures == 0 ? "SWIFT_MENU_OK" : "SWIFT_MENU_FAIL failures=\(failures)")
    return failures == 0 ? 0 : 1
}


extension MenuBuilder {
    /// 在已有颜色/粗细条目的菜单后追加顶层其余项与两个子菜单(全部在 Rust 构建)。
    @objc(addTopLevelAndSubmenusToMenu:handler:titles:launchAtLoginOn:glassOn:)
    public static func addTopLevelAndSubmenus(toMenu menu: NSMenu?,
                                              handler: AnyObject?,
                                              titles: [String: String],
                                              launchAtLoginOn: Bool,
                                              glassOn: Bool) {
        guard let menu = menu else { return }
        let encoded = ProtoWire.encodeStringMap(titles)
        _ = encoded.withUnsafeBufferPointer { glaspen2_rust_menu_add_top_level(
            Unmanaged.passUnretained(menu).toOpaque(),
            handler.map { Unmanaged.passUnretained($0).toOpaque() },
            $0.baseAddress, $0.count, launchAtLoginOn ? 1 : 0, glassOn ? 1 : 0) }
    }
}

// ── 隐藏自检入口(不启动 GUI): Rust `glaspen2 --swift-menu-full-probe` 调用 ──
// 复刻 glaspen2_run 的建菜单顺序(颜色→分隔符→粗细→本构建器), 用假标题/假
// handler 校验: 总项数、分隔符位置(update_menu_texts 的索引契约)、顶层 action/
// tag/state/target/快捷键、两个子菜单的存在/项数/顺序/tag/初始 state/selector。
// 不触发任何 action(只读 selector), 因此不需要真实的 GlaspenMenuHandler。
@_cdecl("glaspen2_macos_swift_menu_full_selftest")
public func glaspen2_macos_swift_menu_full_selftest() -> Int32 {
    let app = NSApplication.shared
    app.setActivationPolicy(.accessory)

    // 与 core 预设表同数同序(真实: 10 色 / 8 粗细; 颜色/粗细条目的逐项校验
    // 已由 --swift-menu-probe 覆盖, 这里只用于凑出与生产一致的顶层下标)。
    let colorCount = 10
    let widthCount = 8
    let titleKeys = [
        "saveWithBg", "saveDrawing", "newCanvas", "export", "settings", "more",
        "toggleDraw", "language", "quit",
        "exportXournal", "exportInfinitePdf", "exportInfiniteSvg",
        "moreLaunch", "moreGlass", "moreOutline", "moreCanvasInfinite",
        "moreCanvasMode",
    ]
    let titles = Dictionary(uniqueKeysWithValues: titleKeys.map { ($0, "T:" + $0) })

    func fakeTitles(_ prefix: String, _ n: Int) -> [NSAttributedString] {
        (0..<n).map { i in
            NSAttributedString(string: "\(prefix)-\(i)",
                               attributes: [.font: NSFont.menuFont(ofSize: 14)])
        }
    }

    func buildMenu(launchAtLoginOn: Bool, glassOn: Bool) -> (NSMenu, NSObject) {
        let handler = NSObject()
        let menu = NSMenu()
        menu.autoenablesItems = false
        MenuBuilder.addColorItems(toMenu: menu, handler: handler,
                                  count: colorCount,
                                  titles: fakeTitles("color", colorCount))
        menu.addItem(NSMenuItem.separator())
        MenuBuilder.addWidthItems(toMenu: menu, handler: handler,
                                  count: widthCount,
                                  titles: fakeTitles("width", widthCount))
        MenuBuilder.addTopLevelAndSubmenus(toMenu: menu, handler: handler,
                                           titles: titles,
                                           launchAtLoginOn: launchAtLoginOn,
                                           glassOn: glassOn)
        return (menu, handler)
    }

    func selectorName(_ item: NSMenuItem) -> String {
        guard let action = item.action else { return "" }
        return NSStringFromSelector(action)
    }

    func stateName(_ item: NSMenuItem) -> String {
        item.state == .on ? "on" : "off"
    }

    var failures = 0
    func expect(_ condition: Bool, _ label: String) {
        if !condition {
            failures += 1
            print("SWIFT_MENU_FULL_FAIL_CHECK \(label)")
        }
    }

    let (menu, handler) = buildMenu(launchAtLoginOn: true, glassOn: false)
    let items = menu.items
    let topBase = colorCount + 1 + widthCount + 1 // 保存(含背景) 的下标
    let expectedCount = topBase + 11              // 顶层 12 项中最后一项 quit 的下标 +1

    let separators = items.indices.filter { items[$0].isSeparatorItem }
    expect(items.count == expectedCount, "count=\(items.count)")
    expect(separators == [colorCount, colorCount + 1 + widthCount,
                          topBase + 4, topBase + 8],
           "separators=\(separators)")

    // 顶层 9 个内容项(跳过 topBase+4 / topBase+8 两个分隔符)。
    // 子菜单父项(导出/更多)的 action 由 AppKit 在设置 submenu 时自动变成
    // submenuAction:(旧 ObjC 建项时 action:nil, 赋值 submenu 后同样是它)。
    let topContent: [(offset: Int, key: String, action: String, tag: Int,
                      state: NSControl.StateValue)] = [
        (0, "saveWithBg", "saveWithBg", 0, .off),
        (1, "saveDrawing", "saveOnly", 0, .off),
        (2, "newCanvas", "clearScreen", 0, .off),
        (3, "export", "submenuAction:", 0, .off),
        (5, "toggleDraw", "toggleDraw", 888, .on),
        (6, "settings", "showSettingsPanel", 0, .off),
        (7, "more", "submenuAction:", 0, .off),
        (9, "language", "toggleLanguage", 0, .off),
        (10, "quit", "quitApp", 0, .off),
    ]
    var topOK = true
    for expected in topContent {
        let item = items[topBase + expected.offset]
        if item.title != "T:" + expected.key { topOK = false }
        if selectorName(item) != expected.action { topOK = false }
        if item.tag != expected.tag { topOK = false }
        if item.state != expected.state { topOK = false }
        if !(item.target === handler) { topOK = false }
        if item.keyEquivalent != "" { topOK = false }
    }
    expect(topOK, "top_content")

    // 导出 ▸ 子菜单
    let exportMenu = items[topBase + 3].submenu
    let exportExpected: [(titleKey: String, action: String)] = [
        ("exportXournal", "saveXoj"),
        ("exportInfinitePdf", "exportInfinitePdf"),
        ("exportInfiniteSvg", "exportInfiniteSvg"),
    ]
    var exportOK = exportMenu != nil
    if let exportMenu = exportMenu {
        if exportMenu.items.count != exportExpected.count { exportOK = false }
        if exportMenu.autoenablesItems { exportOK = false }
        for (i, expected) in exportExpected.enumerated()
        where i < exportMenu.items.count {
            let item = exportMenu.items[i]
            if item.title != "T:" + expected.titleKey { exportOK = false }
            if selectorName(item) != expected.action { exportOK = false }
            if !(item.target === handler) { exportOK = false }
            if item.keyEquivalent != "" { exportOK = false }
        }
    } else {
        exportOK = false
    }
    expect(exportOK, "export_submenu")

    // 更多 ▸ 子菜单(初始 launch=true / glass=false)
    let moreMenu = items[topBase + 7].submenu
    let moreExpected: [(tag: Int, titleKey: String, action: String,
                        state: NSControl.StateValue)] = [
        (777, "moreLaunch", "toggleLaunch", .on),
        (444, "moreGlass", "toggleGlass", .off),
        (667, "moreOutline", "toggleOutline", .off),
        (668, "moreCanvasInfinite", "toggleInfiniteCanvas", .off),
        (778, "moreCanvasMode", "toggleCanvasMode", .off),
    ]
    var moreOK = moreMenu != nil
    var moreTags: [Int] = []
    var moreActions: [String] = []
    var moreStates: [String] = []
    if let moreMenu = moreMenu {
        if moreMenu.items.count != moreExpected.count { moreOK = false }
        if moreMenu.autoenablesItems { moreOK = false }
        for (i, expected) in moreExpected.enumerated()
        where i < moreMenu.items.count {
            let item = moreMenu.items[i]
            moreTags.append(item.tag)
            moreActions.append(selectorName(item))
            moreStates.append(stateName(item))
            if item.tag != expected.tag { moreOK = false }
            if item.title != "T:" + expected.titleKey { moreOK = false }
            if selectorName(item) != expected.action { moreOK = false }
            if item.state != expected.state { moreOK = false }
            if !(item.target === handler) { moreOK = false }
        }
    } else {
        moreOK = false
    }
    expect(moreOK, "more_submenu")

    // 第二份菜单: launch=false / glass=true → 初始 state 按入参, 不写死。
    let (menu2, _) = buildMenu(launchAtLoginOn: false, glassOn: true)
    var statesOK = menu2.items.count == expectedCount
    if let more2 = menu2.items[topBase + 7].submenu {
        statesOK = statesOK && more2.items.count == 5
            && more2.items[0].state == .off && more2.items[1].state == .on
            && more2.items[2].state == .off && more2.items[3].state == .off
            && more2.items[4].state == .off
    } else {
        statesOK = false
    }
    expect(statesOK, "initial_states")

    // R25: 颜色/粗细勾选更新(默认走 Rust, GLASPEN2_SWIFT_MENU_STATE=1 时走 Swift)。
    MenuUpdater.updateCheckmarks(menu: menu2, colorCount: colorCount,
                                 widthCount: widthCount, colorIndex: 3, widthIndex: 5)
    let checkmarksOK = menu2.items[3].state == .on
        && menu2.items[0].state == .off
        && menu2.items[colorCount + 1 + 5].state == .on
        && menu2.items[colorCount + 1].state == .off
    print("SWIFT_MENU_FULL_CHECKMARKS color3=\(menu2.items[3].state == .on) "
          + "width5=\(menu2.items[colorCount + 1 + 5].state == .on) ok=\(checkmarksOK)")
    expect(checkmarksOK, "checkmarks")

    // R26: 动态文案刷新(默认走 Rust, GLASPEN2_SWIFT_MENU_TEXTS=1 时走 Swift)。
    let (menu3, _) = buildMenu(launchAtLoginOn: false, glassOn: false)
    let colorNew = fakeTitles("C", colorCount)
    let widthNew = fakeTitles("W", widthCount)
    let topNew = (0..<11).map { "TOP\($0)" }
    let exportNew = ["E0", "E1", "E2"]
    let moreNew = ["M0", "M1", "M2", "M3"]
    MenuUpdater.updateTexts(menu: menu3, colorTitles: colorNew, widthTitles: widthNew,
                            topTitles: topNew, exportTitles: exportNew, moreTitles: moreNew,
                            modeTitle: "MODE", toggleTitle: "TOGGLE")
    let topBase3 = colorCount + 1 + widthCount + 1
    var textsOK = menu3.items[0].attributedTitle?.string == "C-0"
        && menu3.items[colorCount + 1].attributedTitle?.string == "W-0"
        && menu3.items[topBase3 + 0].title == "TOP0"
        && menu3.items[topBase3 + 7].title == "TOP7"
        && menu3.items[topBase3 + 3].submenu?.items[0].title == "E0"
        && menu3.items[topBase3 + 7].submenu?.items[0].title == "M0"
    if let mode = MenuRuntime.menuItem(tag: 778, inMenu: menu3) { textsOK = textsOK && mode.title == "MODE" } else { textsOK = false }
    if let toggle = MenuRuntime.menuItem(tag: 888, inMenu: menu3) { textsOK = textsOK && toggle.title == "TOGGLE" } else { textsOK = false }
    print("SWIFT_MENU_FULL_TEXTS color=\(menu3.items[0].attributedTitle?.string ?? "nil") "
          + "width=\(menu3.items[colorCount + 1].attributedTitle?.string ?? "nil") "
          + "mode=\(MenuRuntime.menuItem(tag: 778, inMenu: menu3)?.title ?? "nil") "
          + "toggle=\(MenuRuntime.menuItem(tag: 888, inMenu: menu3)?.title ?? "nil") ok=\(textsOK)")
    expect(textsOK, "texts")

    // 防御分支: nil menu 不崩(guard 在任何文案读取之前返回, 不产生日志)。
    MenuBuilder.addTopLevelAndSubmenus(toMenu: nil, handler: nil, titles: [:],
                                       launchAtLoginOn: false, glassOn: false)
    print("SWIFT_MENU_FULL_EDGE nil_menu=ok")

    let topItems = topContent.map { items[topBase + $0.offset] }
    let topTitles = topItems.map { $0.title }.joined(separator: ",")
    let topActions = topItems.map { selectorName($0) }.joined(separator: ",")
    let topTags = topItems.map { String($0.tag) }.joined(separator: ",")
    let topStates = topItems.map { stateName($0) }.joined(separator: ",")
    let topTargetsOK = topItems.allSatisfy { $0.target === handler }
    let topKeyEquivOK = topItems.allSatisfy { $0.keyEquivalent == "" }
    print("SWIFT_MENU_FULL count=\(items.count) expected=\(expectedCount) "
          + "separators=\(separators.map(String.init).joined(separator: ",")) "
          + "submenus=export:\(exportMenu?.items.count ?? -1),"
          + "more:\(moreMenu?.items.count ?? -1)")
    print("SWIFT_MENU_FULL_TOP titles=\(topTitles) actions=\(topActions) "
          + "tags=\(topTags) states=\(topStates) "
          + "targets=\(topTargetsOK) key_equiv_ok=\(topKeyEquivOK) ok=\(topOK)")
    print("SWIFT_MENU_FULL_EXPORT "
          + "actions=\(exportExpected.map { $0.action }.joined(separator: ",")) "
          + "autoenables=\(exportMenu?.autoenablesItems ?? true) ok=\(exportOK)")
    print("SWIFT_MENU_FULL_MORE "
          + "tags=\(moreTags.map(String.init).joined(separator: ",")) "
          + "actions=\(moreActions.joined(separator: ",")) "
          + "states=\(moreStates.joined(separator: ",")) ok=\(moreOK)")
    print("SWIFT_MENU_FULL_STATES initial_states_ok=\(statesOK)")

    print(failures == 0 ? "SWIFT_MENU_FULL_OK"
                        : "SWIFT_MENU_FULL_FAIL failures=\(failures)")
    return failures == 0 ? 0 : 1
}


@objc public final class MainMenuBuilder: NSObject {
    @objc(buildWithHandler:statusItem:launchAtLoginOn:glassOn:)
    public static func build(handler: AnyObject, statusItem: NSStatusItem,
                             launchAtLoginOn: Bool, glassOn: Bool) -> NSMenu {
        let menu: NSMenu
        if let p = glaspen2_rust_menu_create() {
            menu = Unmanaged<NSMenu>.fromOpaque(p).takeRetainedValue()
        } else {
            menu = NSMenu()
        }
        menu.delegate = handler as? NSMenuDelegate
        menu.autoenablesItems = false

        var colorTitles: [NSAttributedString] = []
        for i in 0..<Int(glaspen2_color_preset_count()) {
            colorTitles.append(MenuRuntime.colorItemTitle(Int32(i)))
        }
        MenuBuilder.addColorItems(toMenu: menu, handler: handler,
                                  count: Int(glaspen2_color_preset_count()),
                                  titles: colorTitles)
        menu.addItem(.separator())

        var widthTitles: [NSAttributedString] = []
        for i in 0..<Int(glaspen2_width_preset_count()) {
            widthTitles.append(MenuRuntime.widthItemTitle(Int32(i)))
        }
        MenuBuilder.addWidthItems(toMenu: menu, handler: handler,
                                  count: Int(glaspen2_width_preset_count()),
                                  titles: widthTitles)

        func pair(_ zh: String, _ en: String) -> String {
            MenuRuntime.pair(zh, en)
        }
        let titles: [String: String] = [
            "saveWithBg": pair("保存(含背景)", "Save (with bg)"),
            "saveDrawing": pair("保存(涂鸦)", "Save (drawing)"),
            "newCanvas": pair("新建画布", "New canvas"),
            "export": pair("导出", "Export"),
            "settings": pair("设置...", "Settings..."),
            "more": pair("更多", "More"),
            "toggleDraw": pair("开启涂鸦", "Enable Drawing"),
            "language": pair("English", "中文"),
            "quit": pair("退出", "Quit"),
            "exportXournal": pair("保存笔记 (Xournal)", "Save Notes (Xournal)"),
            "exportInfinitePdf": pair("导出无限画布 PDF (分页)",
                                        "Export infinite canvas PDF (paged)"),
            "exportInfiniteSvg": pair("导出无限画布 SVG (整幅)",
                                        "Export infinite canvas SVG (whole)"),
            "moreLaunch": pair("开机自启", "Launch at login"),
            "moreGlass": pair("磨砂玻璃", "Frosted Glass"),
            "moreOutline": pair("笔迹描边", "Stroke outline"),
            "moreCanvasInfinite": MenuRuntime.l10n("canvas.mode_infinite") ?? "",
            "moreCanvasMode": MenuRuntime.l10n("canvas.fixed_mode") ?? "",
        ]
        MenuBuilder.addTopLevelAndSubmenus(toMenu: menu, handler: handler,
                                           titles: titles,
                                           launchAtLoginOn: launchAtLoginOn,
                                           glassOn: glassOn)
        glaspen2_rust_status_item_set_menu(
            Unmanaged.passUnretained(statusItem).toOpaque(),
            Unmanaged.passUnretained(menu).toOpaque())
        return menu
    }
}

@objc public final class AppDelegate: NSObject, NSApplicationDelegate, NSMenuDelegate {
    static var shared: AppDelegate?
    @objc public func saveWithBg() { glaspen2_menu_save_with_bg() }
    @objc public func saveOnly() { glaspen2_menu_save_only() }
    @objc public func clearScreen() { glaspen2_menu_clear_screen() }
    @objc public func toggleDraw() { glaspen2_menu_toggle_enabled() }
    @objc public func saveXoj() { glaspen2_rust_menu_save_xoj() }
    @objc public func toggleLanguage() { glaspen2_rust_menu_toggle_language() }
    @objc public func showSettingsPanel() { glaspen2_menu_show_settings() }
    @objc public func toggleCanvasMode() { glaspen2_menu_toggle_canvas_mode() }
    @objc public func toggleLaunch() { glaspen2_rust_set_launch_at_login(glaspen2_is_launch_at_login() == 0 ? 1 : 0) }
    @objc public func toggleGlass() { glaspen2_rust_set_glass_enabled(glaspen2_router_glass_enabled() == 0 ? 1 : 0) }
    @objc public func toggleOutline() { glaspen2_rust_menu_set_outline_enabled(glaspen2_router_outline_enabled() == 0 ? 1 : 0) }
    @objc public func toggleInfiniteCanvas() { glaspen2_menu_apply_infinite(glaspen2_router_infinite() == 0 ? 1 : 0) }
    @objc public func exportInfinitePdf() { glaspen2_rust_menu_export_pdf_async() }
    @objc public func exportInfiniteSvg() { glaspen2_rust_menu_export_svg_async() }
    @objc(selectColor:)
    public func selectColor(_ sender: NSMenuItem) { glaspen2_menu_select_color(Int32(sender.tag)) }
    @objc(selectWidth:)
    public func selectWidth(_ sender: NSMenuItem) { glaspen2_menu_select_width(Int32(sender.tag)) }
    public func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        glaspen2_menu_shutdown()
        return .terminateNow
    }
    @objc public func quitApp() { glaspen2_rust_menu_quit_app() }
    public func menuWillOpen(_ menu: NSMenu) { glaspen2_menu_will_open_bridge() }
    public func menuDidClose(_ menu: NSMenu) { glaspen2_menu_did_close_bridge() }
}
