// S3b-1: 菜单小图标(颜色圆点/粗细横线)绘制迁 Swift;
// 旧 ObjC 实现暂时保留为 * _legacy 仅供 parity 探针, S3b-2 删除。
import AppKit

@objc public final class MenuIconFactory: NSObject {
    @objc(colorDotWithColor:)
    public static func colorDot(_ color: NSColor) -> NSImage {
        let s: CGFloat = 13.0
        let image = NSImage(size: NSSize(width: s, height: s))
        image.lockFocus()
        let dot = NSBezierPath(ovalIn: NSRect(x: 0.5, y: 0.5, width: s - 1, height: s - 1))
        color.setFill()
        dot.fill()
        NSColor(white: 0, alpha: 0.3).setStroke()
        dot.lineWidth = 1.0
        dot.stroke()
        image.unlockFocus()
        return image
    }

    @objc(widthLineWithScale:)
    public static func widthLine(_ scale: Double) -> NSImage {
        let w: CGFloat = 16.0
        let h: CGFloat = 13.0
        let image = NSImage(size: NSSize(width: w, height: h))
        image.lockFocus()
        let lineW = min(5.5, max(1.5, 1.3 + sqrt(scale) * 2.2))
        let path = NSBezierPath()
        path.lineWidth = lineW
        path.lineCapStyle = .round
        path.move(to: NSPoint(x: 2, y: h / 2))
        path.line(to: NSPoint(x: w - 2, y: h / 2))
        NSColor.labelColor.withAlphaComponent(0.9).setStroke()
        path.stroke()
        image.unlockFocus()
        return image
    }
}
