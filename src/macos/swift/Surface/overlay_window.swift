// S5-1: 覆盖层 NSWindow 创建与基础属性(层级/透明/穿透/集合行为)+ 光标。
// 内容视图/layer/玻璃/绘制视图仍由 ObjC 组装;这里只搬可参数化的窗口原语。
import AppKit

@objc public final class OverlayWindowFactory: NSObject {
    @objc(makeWindowWithFrame:)
    public static func makeWindow(frame: NSRect) -> NSWindow {
        // R15 预研: Rust/objc2 创建主覆盖窗; 默认仍 Swift, 失败回退。
        if ProcessInfo.processInfo.environment["GLASPEN2_RUST_OVERLAY"] == "1",
           let p = glaspen2_rust_overlay_window_create(frame.origin.x, frame.origin.y,
                                                       frame.size.width, frame.size.height) {
            return Unmanaged<NSWindow>.fromOpaque(p).takeUnretainedValue()
        }
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
        let window = UIAnchors.mainWindow
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

    /// 组装覆盖层内容: 玻璃在下、绘制视图在上(透明 layer), 最后挂到窗口并 orderFront。
    @objc(assembleContent:glass:drawView:inWindow:)
    public static func assembleContent(_ content: NSView, glass: NSView,
                                       drawView: NSView, in window: NSWindow) {
        content.addSubview(glass)
        drawView.wantsLayer = true
        if let layer = drawView.layer { configureClearLayer(layer) }
        content.addSubview(drawView)
        attachContent(content, to: window)
    }

    /// S5-4: 挂载内容视图并 orderFront(与原 setContentView/orderFront 顺序一致)。
    @objc(attachContent:toWindow:)
    public static func attachContent(_ view: NSView, to window: NSWindow) {
        window.contentView = view
        window.orderFront(nil)
    }

    /// 显示器几何变更: 反色流失效 → 屏幕尺寸 → core 建页/重载 → 窗口/视图改尺寸 → 重建。
    @objc(applyDisplayChangeWidth:height:)
    public static func applyDisplayChange(width: Int32, height: Int32) {
        NSLog("[glaspen2] display changed to %dx%d", width, height)
        glaspen2_router_invert_invalidate()
        glaspen2_router_set_screen_size(width, height)
        glaspen2_on_display_change(width, height)
        glaspen2_load_strokes_for_screen(glaspen2_get_current_screen_id())
        glaspen2_smooth_loaded_strokes()
        PageViewOps.update()
        let frame = NSRect(x: 0, y: 0, width: CGFloat(width), height: CGFloat(height))
        if let window = UIAnchors.mainWindow {
            window.setFrame(frame, display: true)
            window.contentView?.frame = frame
        }
        (UIAnchors.glassView as? NSView)?.frame = frame
        if let drawView = UIAnchors.drawView {
            drawView.frame = frame
            MenuRuntime.ensureSurface()
            glaspen2_router_rebuild_surface()
            drawView.needsDisplay = true
        }
    }
}
