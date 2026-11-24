// S1-S3b: 状态栏项/图标与状态栏图标绘制的 Swift 实现。
import AppKit

@objc public final class StatusBarFactory: NSObject {
    @objc public static func createItem(icon: NSImage?) -> NSStatusItem? {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        guard let button = item.button else { return nil }
        button.title = ""
        if let icon = icon { button.image = icon }
        return item
    }

    @objc(createSquareItem)
    public static func createSquareItem() -> NSStatusItem? {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        item.button?.title = ""
        return item
    }
}

@objc public final class StatusIconRenderer: NSObject {
    /// 18pt(72px @4x)状态栏图标: 马克笔/橡皮 + 停用斜杠 + 画布模式徽标。
    /// 参数全部显式传入(原 ObjC 读全局),便于像素 parity 与测试。
    @objc(renderIconWithEraser:enabled:ethereal:infinite:widthScale:penRed:penGreen:penBlue:)
    public static func renderIcon(eraser: Bool,
                                  enabled: Bool,
                                  ethereal: Bool,
                                  infinite: Bool,
                                  widthScale: Double,
                                  penRed: Double,
                                  penGreen: Double,
                                  penBlue: Double) -> NSImage? {
        let size: CGFloat = 18.0
        let px = 72
        guard let rep = NSBitmapImageRep(bitmapDataPlanes: nil,
                                         pixelsWide: px, pixelsHigh: px,
                                         bitsPerSample: 8, samplesPerPixel: 4,
                                         hasAlpha: true, isPlanar: false,
                                         colorSpaceName: .calibratedRGB,
                                         bytesPerRow: 0, bitsPerPixel: 0) else { return nil }
        rep.size = NSSize(width: size, height: size)
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)

        let ink = NSColor(white: 0.0, alpha: 0.78)
        let halo = NSColor(white: 1.0, alpha: 0.88)
        let pen = NSColor(red: penRed, green: penGreen, blue: penBlue, alpha: 1.0)
        let dim: CGFloat = enabled ? 1.0 : 0.35

        func strokeInked(_ path: NSBezierPath, _ width: CGFloat) {
            path.lineWidth = width + 1.2
            halo.withAlphaComponent(dim).setStroke()
            path.stroke()
            path.lineWidth = width
            ink.withAlphaComponent(dim).setStroke()
            path.stroke()
        }
        var markTransform = AffineTransform.identity
        markTransform.translate(x: 9, y: 9)
        markTransform.rotate(byDegrees: 45)
        markTransform.translate(x: -9, y: -9)

        if eraser && enabled {
            let body = NSBezierPath(roundedRect: NSRect(x: 3.5, y: 6.1, width: 11.2, height: 5.8),
                                    xRadius: 1.5, yRadius: 1.5)
            body.transform(using: markTransform)
            let band = NSBezierPath(rect: NSRect(x: 10.2, y: 6.1, width: 4.5, height: 5.8))
            band.transform(using: markTransform)
            body.lineWidth = 0.8 + 1.2
            halo.withAlphaComponent(dim).setStroke()
            body.stroke()
            NSColor(white: 0.92, alpha: dim).setFill()
            body.fill()
            NSColor(white: 0.25, alpha: dim).setFill()
            band.fill()
            body.lineWidth = 0.8
            ink.withAlphaComponent(dim).setStroke()
            body.stroke()
        } else {
            let t = min(6.0, max(2.6, 2.4 + sqrt(widthScale) * 1.9))
            let body = NSBezierPath(roundedRect: NSRect(x: 5.5, y: 9 - t / 2, width: 9.0, height: t),
                                    xRadius: min(1.6, t / 2), yRadius: min(1.6, t / 2))
            let tip = NSBezierPath()
            tip.move(to: NSPoint(x: 5.5, y: 9 - t / 2))
            tip.line(to: NSPoint(x: 3.0, y: 9))
            tip.line(to: NSPoint(x: 5.5, y: 9 + t / 2))
            tip.close()
            let sil = NSBezierPath()
            sil.append(body)
            sil.append(tip)
            sil.transform(using: markTransform)
            tip.transform(using: markTransform)
            sil.lineWidth = 0.9 + 1.2
            halo.withAlphaComponent(dim).setStroke()
            sil.stroke()
            pen.withAlphaComponent(dim).setFill()
            sil.fill()
            ink.withAlphaComponent(dim).setFill()
            tip.fill()
            sil.lineWidth = 0.9
            ink.withAlphaComponent(dim).setStroke()
            sil.stroke()
        }

        if !enabled {
            let slash = NSBezierPath()
            slash.move(to: NSPoint(x: 2.6, y: 15.4))
            slash.line(to: NSPoint(x: 15.4, y: 2.6))
            slash.lineCapStyle = .round
            slash.lineWidth = 2.8
            halo.setStroke()
            slash.stroke()
            slash.lineWidth = 1.6
            ink.setStroke()
            slash.stroke()
        }

        func strokeMark(_ path: NSBezierPath) {
            path.lineWidth = 1.0 + 1.2
            halo.setStroke()
            path.stroke()
            path.lineWidth = 1.0
            ink.setStroke()
            path.stroke()
        }
        if ethereal {
            let dot = NSBezierPath(ovalIn: NSRect(x: 1.9, y: 13.1, width: 3.0, height: 3.0))
            strokeMark(dot)
        }
        if infinite {
            let inf = NSBezierPath()
            inf.append(NSBezierPath(ovalIn: NSRect(x: 12.9, y: 1.6, width: 2.4, height: 2.4)))
            inf.append(NSBezierPath(ovalIn: NSRect(x: 15.1, y: 1.6, width: 2.4, height: 2.4)))
            strokeMark(inf)
        }

        NSGraphicsContext.restoreGraphicsState()
        let image = NSImage(size: NSSize(width: size, height: size))
        image.addRepresentation(rep)
        return image
    }
}

@_cdecl("glaspen2_macos_swift_statusbar_selftest")
public func glaspen2_macos_swift_statusbar_selftest() -> Int32 {
    let app = NSApplication.shared
    app.setActivationPolicy(.accessory)
    let image = NSImage(size: NSSize(width: 1, height: 1))
    image.lockFocus()
    NSColor.labelColor.setFill()
    NSBezierPath(rect: NSRect(x: 0, y: 0, width: 1, height: 1)).fill()
    image.unlockFocus()
    guard let item = StatusBarFactory.createItem(icon: image), item.button != nil else {
        print("SWIFT_STATUSBAR_FAIL")
        return 1
    }
    print("SWIFT_STATUSBAR_OK items=1 button=1 image=1")
    return 0
}

// S7: 状态栏图标刷新(setImage/清标题), 绘制仍走 StatusIconRenderer。
@objc public final class StatusIconUpdater: NSObject {
    @objc(applyToStatusItem:eraser:enabled:ethereal:infinite:widthScale:penRed:penGreen:penBlue:)
    public static func apply(item: NSStatusItem, eraser: Bool, enabled: Bool,
                             ethereal: Bool, infinite: Bool, widthScale: Double,
                             penRed: Double, penGreen: Double, penBlue: Double) {
        let icon = StatusIconRenderer.renderIcon(eraser: eraser, enabled: enabled,
                                                 ethereal: ethereal, infinite: infinite,
                                                 widthScale: widthScale,
                                                 penRed: penRed, penGreen: penGreen,
                                                 penBlue: penBlue)
        item.button?.image = icon
        item.button?.title = ""
    }
}
