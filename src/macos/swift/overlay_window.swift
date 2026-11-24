// S5-1: 覆盖层 NSWindow 创建与基础属性(层级/透明/穿透/集合行为)+ 光标。
// 内容视图/layer/玻璃/绘制视图仍由 ObjC 组装;这里只搬可参数化的窗口原语。
import AppKit

@objc public final class OverlayWindowFactory: NSObject {
    @objc(makeWindowWithFrame:)
    public static func makeWindow(frame: NSRect) -> NSWindow {
        let window = NSWindow(contentRect: frame,
                              styleMask: .borderless,
                              backing: .buffered,
                              defer: false)
        window.level = NSWindow.Level(rawValue: Int(CGWindowLevelForKey(.maximumWindow)))
        window.isOpaque = false
        window.backgroundColor = .clear
        window.title = "glaspen2"
        window.acceptsMouseMovedEvents = true
        window.collectionBehavior = [.canJoinAllSpaces, .stationary]
        window.ignoresMouseEvents = true
        return window
    }

    @objc(makeBlankCursor)
    public static func makeBlankCursor() -> NSCursor {
        let image = NSImage(size: NSSize(width: 1, height: 1))
        image.lockFocus()
        NSColor.clear.setFill()
        NSBezierPath(rect: NSRect(x: 0, y: 0, width: 1, height: 1)).fill()
        image.unlockFocus()
        return NSCursor(image: image, hotSpot: .zero)
    }

    @objc(makeArrowCursor)
    public static func makeArrowCursor() -> NSCursor { .arrow }
}
