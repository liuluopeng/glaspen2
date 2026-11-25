// S7: 覆盖窗口绘制视图与设置窗口 delegate(替代 ObjC 小类)。
import AppKit

@objc public final class GlaspenDrawView: NSView {
    /// 与迁移前 ObjC 一致(覆盖层窗口不 key, 但保留语义, 防止未来键盘交互回归)。
    @objc public override var acceptsFirstResponder: Bool { true }

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
