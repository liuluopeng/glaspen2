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
        for i in 0..<n {
            let item = menu.addItem(withTitle: "", action: action, keyEquivalent: "")
            item.attributedTitle = titles[i]
            item.target = handler
            item.tag = i
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
