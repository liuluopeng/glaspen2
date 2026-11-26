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
        guard let jsonData = try? JSONSerialization.data(withJSONObject: titles),
              let json = String(data: jsonData, encoding: .utf8) else { return }
        _ = json.withCString { glaspen2_rust_menu_add_top_level(
            Unmanaged.passUnretained(menu).toOpaque(),
            handler.map { Unmanaged.passUnretained($0).toOpaque() },
            $0, launchAtLoginOn ? 1 : 0, glassOn ? 1 : 0) }
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
