// S7: 飘渺模式显隐与 peek 计时器(翻页/镜头变化时短暂显示笔迹)。
import AppKit

@objc public final class EtherealMode: NSObject {
    private static var peekTimer: Timer?

    /// 立即隐藏笔迹(含玻璃/压感窗), 前置: 非隧道动画 + 飘渺 + 当前可见。
    @objc public static func hideNow() -> Bool {
        glaspen2_rust_ethereal_hide_now() != 0
    }

    private static func hideStrokes() {
        glaspen2_rust_ethereal_restore_now()
    }

    /// 启动恢复飘渺: 先归位可见再走同一隐藏路径。
    @objc public static func restoreNow() {
        glaspen2_rust_ethereal_restore_now()
    }

    /// 笔回来: 笔迹重现(玻璃按自身开关恢复)。
    @objc public static func autoShow() {
        glaspen2_rust_ethereal_auto_show()
    }

    @objc public static func cancelTimer() {
        peekTimer?.invalidate()
        peekTimer = nil
    }

    @objc(peekForSeconds:)
    public static func peek(seconds: Double) {
        if glaspen2_router_ethereal() == 0 { return }
        cancelTimer()
        _ = glaspen2_rust_ethereal_preview_show()
        peekTimer = Timer.scheduledTimer(withTimeInterval: seconds, repeats: false) { _ in
            peekTimer = nil
            if glaspen2_router_ethereal() != 0
                && glaspen2_router_stroke_active() == 0 {
                _ = glaspen2_rust_ethereal_hide_now()
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
