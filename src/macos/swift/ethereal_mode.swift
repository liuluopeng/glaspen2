// S7: 飘渺模式显隐与 peek 计时器(翻页/镜头变化时短暂显示笔迹)。
import AppKit

@objc public final class EtherealMode: NSObject {
    private static var peekTimer: Timer?

    /// 立即隐藏笔迹(含玻璃/压感窗), 前置: 非隧道动画 + 飘渺 + 当前可见。
    @objc public static func hideNow() -> Bool {
        if TunnelState.active { return false }
        if glaspen2_router_ethereal() == 0 { return false }
        if glaspen2_router_strokes_visible() == 0 { return false }
        hideStrokes()
        return true
    }

    private static func hideStrokes() {
        EventRouter.finishActiveStroke()
        glaspen2_router_set_strokes_visible(0)
        if glaspen2_router_glass_follow() != 0,
           let glass = UIAnchors.glassView as? NSView {
            glass.isHidden = true
        }
        if glaspen2_router_pressure_monitor() != 0 { PressureMonitorWindow.hide() }
        glaspen2_router_redraw_draw_view()
    }

    /// 启动恢复飘渺: 先归位可见再走同一隐藏路径。
    @objc public static func restoreNow() {
        if glaspen2_router_ethereal() == 0 { return }
        glaspen2_router_set_strokes_visible(1)
        hideStrokes()
    }

    /// 笔回来: 笔迹重现(玻璃按自身开关恢复)。
    @objc public static func autoShow() {
        if glaspen2_router_ethereal() == 0 { return }
        if glaspen2_router_strokes_visible() == 0 {
            glaspen2_router_set_strokes_visible(1)
            MenuHandler.applyGlass()
            if glaspen2_router_pressure_monitor() != 0 { PressureMonitorWindow.show() }
            glaspen2_router_redraw_draw_view()
            NSLog("[ethereal] 笔迹重现")
        }
    }

    @objc public static func cancelTimer() {
        peekTimer?.invalidate()
        peekTimer = nil
    }

    @objc(peekForSeconds:)
    public static func peek(seconds: Double) {
        if glaspen2_router_ethereal() == 0 { return }
        cancelTimer()
        if glaspen2_router_strokes_visible() == 0 {
            glaspen2_router_set_strokes_visible(1)
            MenuHandler.applyGlass()
            if glaspen2_router_pressure_monitor() != 0 { PressureMonitorWindow.show() }
            glaspen2_router_redraw_draw_view()
        }
        peekTimer = Timer.scheduledTimer(withTimeInterval: seconds, repeats: false) { _ in
            peekTimer = nil
            if glaspen2_router_ethereal() != 0
                && glaspen2_router_stroke_active() == 0 {
                _ = hideNow()
            }
        }
    }

    /// 镜头平移/缩放时 peek: 悬停可见则保持; 连续变化重新计时。
    @objc public static func peekLensChange() {
        if glaspen2_router_ethereal() == 0 || glaspen2_router_infinite() == 0 { return }
        if glaspen2_router_strokes_visible() != 0 && peekTimer == nil { return }
        peek(seconds: 1.0)
    }
}
