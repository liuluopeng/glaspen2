// S7: 压感监视器小窗(无边框、置顶、不接收鼠标)。
import AppKit

@objc public final class PressureMonitorWindow: NSObject {
    private static var window: NSWindow?
    private static var label: NSTextField?

    @objc public static func ensureWindow() {
        if window != nil { return }
        let screenFrame = NSScreen.main?.frame ?? NSRect(x: 0, y: 0, width: 1920, height: 1080)
        let frame = NSRect(x: 10, y: screenFrame.height - 40, width: 220, height: 30)
        let w = NSWindow(contentRect: frame, styleMask: .borderless,
                         backing: .buffered, defer: false)
        w.level = NSWindow.Level(rawValue: Int(CGWindowLevelForKey(.maximumWindow)))
        w.isOpaque = true
        w.backgroundColor = NSColor(white: 0.12, alpha: 1.0)
        w.ignoresMouseEvents = true
        w.title = "Pressure"
        w.isReleasedWhenClosed = false
        let l = NSTextField(frame: NSRect(x: 4, y: 2, width: 212, height: 26))
        l.stringValue = "P=-----  ---  (---,---)"
        l.textColor = .white
        l.font = NSFont(name: "Menlo", size: 12)
        l.isBezeled = false
        l.drawsBackground = false
        l.isEditable = false
        l.isSelectable = false
        w.contentView?.addSubview(l)
        window = w
        label = l
    }

    @objc public static func show() {
        ensureWindow()
        window?.orderFront(nil)
    }

    @objc public static func hide() {
        window?.orderOut(nil)
    }

    @objc public static func destroy() {
        window?.close()
        window = nil
        label = nil
    }

    @objc public static func updateLabel(_ text: String) {
        label?.stringValue = text
    }
}
