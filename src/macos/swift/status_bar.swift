// S3a: Swift 创建 macOS 状态栏项(最小切片;菜单仍由 ObjC 构建)。
// 边界规则见 docs/swift-boundary.md: 只暴露 @objc/NSObject 与 C ABI。
import AppKit

@objc public final class StatusBarFactory: NSObject {
    /// 创建一个空的 NSStatusItem(图标由调用方提供;这里只验证 AppKit 路径)。
    @objc public static func createItem(icon: NSImage?) -> NSStatusItem? {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        guard let button = item.button else { return nil }
        button.title = ""
        if let icon = icon {
            button.image = icon
        }
        return item
    }

    /// 与 ObjC 原实现一致: 方形长度状态栏项; 图标由调用方随后设置。
    @objc(createSquareItem)
    public static func createSquareItem() -> NSStatusItem? {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        item.button?.title = ""
        return item
    }
}

/// 隐藏自检入口:目标文件 build.rs 已自动编入;由 Rust --swift-statusbar-probe 调用。
@_cdecl("glaspen2_macos_swift_statusbar_selftest")
public func glaspen2_macos_swift_statusbar_selftest() -> Int32 {
    let app = NSApplication.shared
    app.setActivationPolicy(.accessory)
    let image = NSImage(size: NSSize(width: 1, height: 1))
    image.lockFocus()
    NSColor.labelColor.setFill()
    NSBezierPath(rect: NSRect(x: 0, y: 0, width: 1, height: 1)).fill()
    image.unlockFocus()
    guard let item = StatusBarFactory.createItem(icon: image) else {
        print("SWIFT_STATUSBAR_FAIL item_nil")
        return 1
    }
    guard let button = item.button else {
        print("SWIFT_STATUSBAR_FAIL button_nil")
        return 1
    }
    let hasImage = button.image != nil
    print("SWIFT_STATUSBAR_OK items=1 button=1 image=1")
    print(hasImage ? "SWIFT_STATUSBAR_IMAGE ok" : "SWIFT_STATUSBAR_IMAGE missing")
    return hasImage ? 0 : 1
}
