// S5-1: 覆盖层 NSWindow 创建与基础属性(层级/透明/穿透/集合行为)+ 光标。
// 内容视图/layer/玻璃/绘制视图仍由 ObjC 组装;这里只搬可参数化的窗口原语。
import AppKit

@objc public final class OverlayWindowFactory: NSObject {
    @objc(makeWindowWithFrame:)
    public static func makeWindow(frame: NSRect) -> NSWindow {
        // R21/R135: 主覆盖窗创建走 Rust/objc2; 失败才回退 Swift 原语。
        if let p = glaspen2_rust_overlay_window_create(frame.origin.x, frame.origin.y,
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

    /// 磨砂玻璃视图(S5-2): behind-window 混合、light 材质、active 状态;
    /// alpha/hidden 与原 ObjC 公式一致(enabled ? opacity*2 : 0)。
    @objc(makeGlassViewWithFrame:enabled:opacity:)
    public static func makeGlassView(frame: NSRect, enabled: Bool, opacity: Double) -> NSVisualEffectView {
        if let p = glaspen2_rust_overlay_glass_view_create(frame.origin.x, frame.origin.y,
                                                           frame.size.width, frame.size.height,
                                                           enabled ? 1 : 0, opacity) {
            return Unmanaged<NSVisualEffectView>.fromOpaque(p).takeUnretainedValue()
        }
        let view = NSVisualEffectView(frame: frame)
        view.blendingMode = .behindWindow
        view.material = .light
        view.state = .active
        view.alphaValue = enabled ? opacity * 2.0 : 0.0
        view.isHidden = !enabled
        return view
    }

    /// 绘制视图的透明 layer 配置(S5-2): 非不透明 + clear 背景。
    /// S5-4: 内容容器视图(layer-backed), 由 ObjC 继续往里加玻璃/绘制视图。
    @objc(makeContentViewWithFrame:)
    public static func makeContentView(frame: NSRect) -> NSView {
        if let p = glaspen2_rust_overlay_content_view_create(frame.origin.x, frame.origin.y,
                                                            frame.size.width, frame.size.height) {
            return Unmanaged<NSView>.fromOpaque(p).takeUnretainedValue()
        }
        let view = NSView(frame: frame)
        view.wantsLayer = true
        return view
    }

    /// 组装覆盖层内容: 玻璃在下、绘制视图在上(透明 layer), 最后挂到窗口并 orderFront。
    @objc(assembleContent:glass:drawView:inWindow:)
    public static func assembleContent(_ content: NSView, glass: NSView,
                                       drawView: NSView, in window: NSWindow) {
        glaspen2_rust_overlay_assemble_content(
            Unmanaged.passUnretained(content).toOpaque(),
            Unmanaged.passUnretained(glass).toOpaque(),
            Unmanaged.passUnretained(drawView).toOpaque(),
            Unmanaged.passUnretained(window).toOpaque())
    }

    /// S5-4: 挂载内容视图并 orderFront(与原 setContentView/orderFront 顺序一致)。
    /// 显示器几何变更: 反色流失效 → 屏幕尺寸 → core 建页/重载 → 窗口/视图改尺寸 → 重建。
    @objc(applyDisplayChangeWidth:height:)
    public static func applyDisplayChange(width: Int32, height: Int32) {
        glaspen2_rust_apply_display_change(width, height)
    }

}
