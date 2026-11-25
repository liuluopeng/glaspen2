// S7: 覆盖窗口绘制视图与设置窗口 delegate(替代 ObjC 小类)。
import AppKit

@objc public final class GlaspenDrawView: NSView {
    /// 与迁移前 ObjC 一致(覆盖层窗口不 key, 但保留语义, 防止未来键盘交互回归)。
    @objc public override var acceptsFirstResponder: Bool { true }

    @objc public override func draw(_ rect: NSRect) {
        OverlayDrawing.draw(in: self, rect: rect)
    }
}

/// R19: Rust 绘制视图的 drawRect C 桥(重入现有 OverlayDrawing)。
@_cdecl("glaspen2_swift_draw_overlay_bridge")
public func glaspen2SwiftDrawOverlayBridge(_ view: UnsafeMutableRawPointer?,
                                           _ x: Double, _ y: Double,
                                           _ w: Double, _ h: Double) {
    guard let view else { return }
    let nsView = Unmanaged<NSView>.fromOpaque(view).takeUnretainedValue()
    OverlayDrawing.draw(in: nsView, rect: NSRect(x: x, y: y, width: w, height: h))
}

@objc public final class SettingsWindowDelegate: NSObject, NSWindowDelegate {
    static var shared: SettingsWindowDelegate?

    @objc public func windowWillClose(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
        NSApp.deactivate()
    }
}
