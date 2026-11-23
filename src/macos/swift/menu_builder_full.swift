// S3b-2b-3: 顶层菜单其余项 + 导出▸/更多▸ 子菜单的构建, 由 run.m 建菜单段迁入。
//
// 边界(docs/swift-boundary.md): i18n 文案单源仍在 ObjC 侧(L()/gl_text), 这里只
// 接收已本地化的标题字典; 颜色/粗细条目仍走 MenuBuilder.addColorItems/
// addWidthItems(富文本标题由 ObjC 预构)。Swift 只负责菜单结构:
// 顺序 / 分隔符 / tag / selector / target / 初始 state / keyEquivalent。
//
// 菜单契约(点击行为与 menu.m update_menu_texts 的索引假设都依赖它, 不得改):
//   顶层(颜色/分隔符/粗细之后, 共 12 项, 顺序固定):
//     分隔符, 保存(含背景), 保存(涂鸦), 新建画布, 导出▸,
//     分隔符, 涂鸦开关(tag 888, state On), 设置..., 更多▸,
//     分隔符, 语言切换, 退出
//   action = saveWithBg / saveOnly / clearScreen / toggleDraw / showSettingsPanel /
//            toggleLanguage / quitApp; 子菜单 saveXoj / exportInfinitePdf /
//            exportInfiniteSvg 与 toggleLaunch / toggleGlass / toggleOutline /
//            toggleInfiniteCanvas / toggleCanvasMode
//   target = handler: 所有非分隔符项都显式设置, 因此 ObjC 侧末尾的 target
//            兜底循环已删除(S3b-2b-3)。
//   tag    = 888 / 777 / 444 / 667 / 668 / 778
//   state  = 涂鸦开关 On; 开机自启/磨砂玻璃按入参(建菜单时的 ObjC 全局值);
//            其余 Off
//   标题键(titles 字典; ObjC 侧用 L()/gl_text 填):
//     saveWithBg, saveDrawing, newCanvas, export, settings, more, toggleDraw,
//     language, quit, exportXournal, exportInfinitePdf, exportInfiniteSvg,
//     moreLaunch, moreGlass, moreOutline, moreCanvasInfinite, moreCanvasMode
import AppKit

extension MenuBuilder {
    /// 在已有颜色/粗细条目的菜单后追加顶层其余项与两个子菜单。
    /// 与 run.m 原手工建菜单段逐项等价(顺序/tag/action/target/state/keyEquivalent)。
    @objc(addTopLevelAndSubmenusToMenu:handler:titles:launchAtLoginOn:glassOn:)
    public static func addTopLevelAndSubmenus(toMenu menu: NSMenu?,
                                              handler: AnyObject?,
                                              titles: [String: String],
                                              launchAtLoginOn: Bool,
                                              glassOn: Bool) {
        guard let menu = menu else { return }

        // 标题字典由 ObjC 侧按契约键填好(L()/gl_text 单源); 缺键时留空并打日志,
        // 不猜文案(菜单仍完整, 只是该项无字, 属明显可诊断的接线错误)。
        func localized(_ key: String) -> String {
            if let text = titles[key] { return text }
            NSLog("[menu] 缺少本地化标题键: %@", key)
            return ""
        }

        // 颜色/粗细段之后的分隔符(原 run.m 在此处单独 addItem 分隔符)。
        menu.addItem(NSMenuItem.separator())

        // 高频动作留在顶层; 低频导出/实验开关收进子菜单, 首屏不再铺满 30+ 行。
        addActionItem(to: menu, title: localized("saveWithBg"),
                      action: "saveWithBg", handler: handler)
        addActionItem(to: menu, title: localized("saveDrawing"),
                      action: "saveOnly", handler: handler)
        addActionItem(to: menu, title: localized("newCanvas"),
                      action: "clearScreen", handler: handler)

        // ── 导出 ▸ ──
        let exportItem = addActionItem(to: menu, title: localized("export"),
                                       action: nil, handler: handler)
        exportItem.submenu = makeExportMenu(localized: localized,
                                            handler: handler)

        menu.addItem(NSMenuItem.separator())

        let toggleItem = addActionItem(to: menu, title: localized("toggleDraw"),
                                       action: "toggleDraw", handler: handler)
        toggleItem.tag = 888
        toggleItem.state = .on

        addActionItem(to: menu, title: localized("settings"),
                      action: "showSettingsPanel", handler: handler)

        // ── 更多 ▸(低频实验/系统开关) ──
        let moreItem = addActionItem(to: menu, title: localized("more"),
                                     action: nil, handler: handler)
        moreItem.submenu = makeMoreMenu(localized: localized,
                                        handler: handler,
                                        launchAtLoginOn: launchAtLoginOn,
                                        glassOn: glassOn)

        menu.addItem(NSMenuItem.separator())

        addActionItem(to: menu, title: localized("language"),
                      action: "toggleLanguage", handler: handler)
        addActionItem(to: menu, title: localized("quit"),
                      action: "quitApp", handler: handler)
    }

    /// 导出▸ 子菜单: 3 项, 顺序/selector/target 与旧实现一致。
    private static func makeExportMenu(localized: (String) -> String,
                                       handler: AnyObject?) -> NSMenu {
        let menu = NSMenu()
        menu.autoenablesItems = false
        addActionItem(to: menu, title: localized("exportXournal"),
                      action: "saveXoj", handler: handler)
        addActionItem(to: menu, title: localized("exportInfinitePdf"),
                      action: "exportInfinitePdf", handler: handler)
        addActionItem(to: menu, title: localized("exportInfiniteSvg"),
                      action: "exportInfiniteSvg", handler: handler)
        return menu
    }

    /// 更多▸ 子菜单: 5 项, tag 777/444/667/668/778, 前两项的初始 state 按入参。
    private static func makeMoreMenu(localized: (String) -> String,
                                     handler: AnyObject?,
                                     launchAtLoginOn: Bool,
                                     glassOn: Bool) -> NSMenu {
        let menu = NSMenu()
        menu.autoenablesItems = false

        let launchItem = addActionItem(to: menu, title: localized("moreLaunch"),
                                       action: "toggleLaunch", handler: handler)
        launchItem.tag = 777
        launchItem.state = launchAtLoginOn ? .on : .off

        let glassItem = addActionItem(to: menu, title: localized("moreGlass"),
                                      action: "toggleGlass", handler: handler)
        glassItem.tag = 444
        glassItem.state = glassOn ? .on : .off

        let outlineItem = addActionItem(to: menu, title: localized("moreOutline"),
                                        action: "toggleOutline", handler: handler)
        outlineItem.tag = 667
        outlineItem.state = .off

        let infiniteItem =
            addActionItem(to: menu, title: localized("moreCanvasInfinite"),
                          action: "toggleInfiniteCanvas", handler: handler)
        infiniteItem.tag = 668
        infiniteItem.state = .off

        let modeItem = addActionItem(to: menu, title: localized("moreCanvasMode"),
                                     action: "toggleCanvasMode", handler: handler)
        modeItem.tag = 778
        modeItem.state = .off

        return menu
    }

    /// 建项 + 显式 target(action 为 nil 时只用于子菜单父项, 与旧 target 兜底循环的
    /// 最终状态一致)。keyEquivalent 旧实现全部为空串, 这里保持一致。
    @discardableResult
    private static func addActionItem(to menu: NSMenu,
                                      title: String,
                                      action: String?,
                                      handler: AnyObject?) -> NSMenuItem {
        let item = menu.addItem(withTitle: title,
                                action: action.map { Selector($0) },
                                keyEquivalent: "")
        item.target = handler
        return item
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
