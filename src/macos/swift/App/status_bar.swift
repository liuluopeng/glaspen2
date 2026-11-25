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
        // R14 预研: Rust/objc2 创建状态栏项; 默认仍走 Swift, 失败自动回退。
        if ProcessInfo.processInfo.environment["GLASPEN2_RUST_STATUS_ITEM"] == "1",
           let p = glaspen2_rust_status_item_create() {
            return Unmanaged<NSStatusItem>.fromOpaque(p).takeRetainedValue()
        }
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
        // R17 预研: Rust/objc2 渲染状态栏图标; 默认 Swift, 失败回退。
        if ProcessInfo.processInfo.environment["GLASPEN2_RUST_STATUS_ICON"] == "1",
           let p = glaspen2_rust_status_icon_render(
               eraser ? 1 : 0, enabled ? 1 : 0, ethereal ? 1 : 0, infinite ? 1 : 0,
               widthScale, penRed, penGreen, penBlue) {
            return Unmanaged<NSImage>.fromOpaque(p).takeRetainedValue()
        }
        return renderSwiftIcon(eraser: eraser, enabled: enabled, ethereal: ethereal,
                               infinite: infinite, widthScale: widthScale,
                               penRed: penRed, penGreen: penGreen, penBlue: penBlue)
    }

    static func renderSwiftIcon(eraser: Bool,
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

/// R17: Rust/Swift 状态栏图标像素 parity 探针(测试时 GLASPEN2_RUST_STATUS_ICON_TEST=1)。
@objc public final class StatusIconParity: NSObject {
    private static func bytes(_ image: NSImage?) -> (Int, Int, [UInt8])? {
        guard let image,
              let tiff = image.tiffRepresentation,
              let rep = NSBitmapImageRep(data: tiff),
              let data = rep.bitmapData else { return nil }
        let count = rep.bytesPerRow * rep.pixelsHigh
        return (rep.pixelsWide, rep.pixelsHigh,
                Array(UnsafeBufferPointer(start: data, count: count)))
    }

    @objc public static func check() -> Int32 {
        let args: (Bool, Bool, Bool, Bool, Double, Double, Double, Double)
            = (false, true, false, false, 1.0, 1.0, 0.0, 0.0)
        let swift = StatusIconRenderer.renderSwiftIcon(
            eraser: args.0, enabled: args.1, ethereal: args.2, infinite: args.3,
            widthScale: args.4, penRed: args.5, penGreen: args.6, penBlue: args.7)
        guard let p = glaspen2_rust_status_icon_render(
            args.0 ? 1 : 0, args.1 ? 1 : 0, args.2 ? 1 : 0, args.3 ? 1 : 0,
            args.4, args.5, args.6, args.7) else {
            NSLog("STATUS_ICON_PARITY fail: rust null")
            return 1
        }
        let rust = Unmanaged<NSImage>.fromOpaque(p).takeRetainedValue()
        guard let a = bytes(swift), let b = bytes(rust) else {
            NSLog("STATUS_ICON_PARITY fail: image bytes nil")
            return 1
        }
        guard a.0 == b.0, a.1 == b.1, a.2.count == b.2.count else {
            NSLog("STATUS_ICON_PARITY fail: size %dx%d vs %dx%d bytes %d vs %d",
                  a.0, a.1, b.0, b.1, a.2.count, b.2.count)
            return 1
        }
        var diff = 0
        for i in 0..<a.2.count where a.2[i] != b.2[i] { diff += 1 }
        NSLog("STATUS_ICON_PARITY size=%dx%d diff=%d/%d", a.0, a.1, diff, a.2.count)
        return diff == 0 ? 0 : 1
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
