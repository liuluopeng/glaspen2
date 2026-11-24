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

    /// 磨砂玻璃视图(S5-2): behind-window 混合、light 材质、active 状态;
    /// alpha/hidden 与原 ObjC 公式一致(enabled ? opacity*2 : 0)。
    @objc(makeGlassViewWithFrame:enabled:opacity:)
    public static func makeGlassView(frame: NSRect, enabled: Bool, opacity: Double) -> NSVisualEffectView {
        let view = NSVisualEffectView(frame: frame)
        view.blendingMode = .behindWindow
        view.material = .light
        view.state = .active
        view.alphaValue = enabled ? opacity * 2.0 : 0.0
        view.isHidden = !enabled
        return view
    }

    /// 绘制视图的透明 layer 配置(S5-2): 非不透明 + clear 背景。
    @objc(configureClearLayer:)
    public static func configureClearLayer(_ layer: CALayer) {
        layer.isOpaque = false
        layer.backgroundColor = NSColor.clear.cgColor
    }

    /// 截图目标屏: 取覆盖层窗口所在屏, 退回主屏(多屏时勿用 displays.first)。
    @objc(windowDisplayId)
    public static func windowDisplayID() -> UInt32 {
        let window = glaspen2_router_main_window() as? NSWindow
        let screen = window?.screen ?? NSScreen.main
        let key = NSDeviceDescriptionKey("NSScreenNumber")
        if let n = screen?.deviceDescription[key] as? NSNumber {
            return n.uint32Value
        }
        return CGMainDisplayID()
    }

    /// S5-4: 内容容器视图(layer-backed), 由 ObjC 继续往里加玻璃/绘制视图。
    @objc(makeContentViewWithFrame:)
    public static func makeContentView(frame: NSRect) -> NSView {
        let view = NSView(frame: frame)
        view.wantsLayer = true
        return view
    }

    /// S5-4: 挂载内容视图并 orderFront(与原 setContentView/orderFront 顺序一致)。
    @objc(attachContent:toWindow:)
    public static func attachContent(_ view: NSView, to window: NSWindow) {
        window.contentView = view
        window.orderFront(nil)
    }
}
