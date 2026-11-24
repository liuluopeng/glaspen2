// S7: 覆盖窗口绘制视图与设置窗口 delegate(替代 ObjC 小类)。
import AppKit

@objc public final class GlaspenDrawView: NSView {
    @objc public override func draw(_ rect: NSRect) {
        OverlayDrawing.draw(in: self, rect: rect)
    }
}

@objc public final class SettingsWindowDelegate: NSObject, NSWindowDelegate {
    static var shared: SettingsWindowDelegate?

    @objc public func windowWillClose(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
        NSApp.deactivate()
    }
}
