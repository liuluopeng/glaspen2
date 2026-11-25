// S7: 页面缩略图条(minimap)。布局唯一来源 core::minimap, 这里只做缓存/异步取图/绘制。
import AppKit

@objc public final class MinimapRenderer: NSObject {
    private static var layout: NSDictionary?
    private static var layoutFor: Int64 = -1
    private static var layoutW = -1.0
    private static var layoutH = -1.0
    private static var layoutGeomW: Int32 = -1
    private static var thumbs: [Int64: NSImage] = [:]
    private static var inflight: Set<Int64> = []
    private static var queue: DispatchQueue?
    private static var fetchVersion = 0
    private static var strip: NSImage?
    private static var stripKey = -1

    @objc(drawWithContext:bounds:)
    public static func draw(_ ctx: CGContext, bounds: NSRect) {
        if glaspen2_router_infinite() != 0 { return }
        let cur = Int64(glaspen2_get_current_screen_id())
        if cur <= 0 { return }
        if thumbs.count > 128 { thumbs.removeAll() }

        let viewW = Double(bounds.size.width)
        let viewH = Double(bounds.size.height)
        let geomW = PageViewOps.geometryWidth
        if layout == nil || layoutFor != cur || layoutW != viewW
            || layoutH != viewH || layoutGeomW != geomW {
            layout = layoutFromJSON(glaspen2_minimap_layout_json(cur, viewW, viewH,
                                                                   Int32(geomW)))
            layoutFor = cur
            layoutW = viewW
            layoutH = viewH
            layoutGeomW = geomW
            stripKey = -1
        }
        guard let layout, let items = layout["items"] as? [NSDictionary],
              !items.isEmpty else { return }
        let stripW = (layout["strip_w"] as? NSNumber)?.doubleValue ?? 0
        let stripH = (layout["strip_h"] as? NSNumber)?.doubleValue ?? 0
        let stripX = (layout["strip_x"] as? NSNumber)?.doubleValue ?? 0
        let stripY = (layout["strip_y"] as? NSNumber)?.doubleValue ?? 0
        if !(stripW > 0) || !(stripH > 0) { return }

        // 1) 缺失缩略图: 专属串行队列逐张渲染, 主线程回填。
        if queue == nil {
            let q = DispatchQueue(label: "glaspen2.minimap", qos: .utility)
            queue = q
        }
        for item in items {
            guard let pidNum = item["screen_id"] as? NSNumber else { continue }
            let pid = pidNum.int64Value
            if thumbs[pid] != nil || inflight.contains(pid) { continue }
            inflight.insert(pid)
            let screenW = Int32(glaspen2_router_screen_w())
            let screenH = Int32(glaspen2_router_screen_h())
            queue?.async {
                var len: Int32 = 0
                let png = glaspen2_render_thumbnail(pid, screenW, screenH, 128, &len)
                var image: NSImage?
                if let png, len > 0 {
                    let data = Data(bytes: png, count: Int(len))
                    image = NSImage(data: data)
                    glaspen2_free_rust_bytes(png, len)
                }
                DispatchQueue.main.async {
                    if let image {
                        thumbs[pid] = image
                        fetchVersion += 1
                    }
                    inflight.remove(pid)
                    glaspen2_router_redraw_draw_view()
                }
            }
        }

        // 2) 合成整条 strip(缩略图版本/当前页变化才重建)。
        let key = fetchVersion * 100000 + Int(cur)
        if strip == nil || key != stripKey {
            stripKey = key
            let s = NSImage(size: NSSize(width: stripW, height: stripH))
            s.lockFocus()
            for item in items {
                guard let pidNum = item["screen_id"] as? NSNumber else { continue }
                let pid = pidNum.int64Value
                let isCur = (item["current"] as? NSNumber)?.boolValue ?? false
                let itemY = (item["y"] as? NSNumber)?.doubleValue ?? 0
                let itemH = (item["h"] as? NSNumber)?.doubleValue ?? 0
                let ly = CGFloat(stripH - ((itemY - stripY) + itemH))
                let rect = NSRect(x: 0, y: ly, width: CGFloat(stripW),
                                  height: CGFloat(itemH))
                if let im = thumbs[pid] {
                    im.draw(in: rect, from: .zero, operation: .sourceOver, fraction: 1)
                } else {
                    NSColor(calibratedWhite: 0.94, alpha: 0.9).setFill()
                    NSBezierPath.fill(rect)
                }
                let border = NSBezierPath(rect: rect)
                border.lineWidth = isCur ? 2.0 : 1.0
                let strokeColor = isCur ? NSColor.systemRed : NSColor.systemGray
                strokeColor.setStroke()
                border.stroke()
                if let num = item["label"] as? String, !num.isEmpty {
                    let attrs: [NSAttributedString.Key: Any] = [
                        .font: NSFont.monospacedSystemFont(ofSize: 9, weight: .regular),
                        .foregroundColor: NSColor(calibratedWhite: 0.25, alpha: 0.9),
                    ]
                    (num as NSString).draw(at: NSPoint(x: rect.origin.x + 3,
                                                      y: rect.origin.y + 3),
                                           withAttributes: attrs)
                }
            }
            s.unlockFocus()
            strip = s
        }

        // 3) 一次贴图(右缘, 顶端 8pt; Cocoa y 向上翻转)。
        guard let strip else { return }
        let dst = NSRect(x: CGFloat(stripX),
                         y: CGFloat(viewH - stripY - stripH),
                         width: CGFloat(stripW), height: CGFloat(stripH))
        strip.draw(in: dst, from: .zero, operation: .sourceOver, fraction: 1)
    }

    private static func layoutFromJSON(_ json: UnsafeMutablePointer<CChar>?) -> NSDictionary? {
        guard let json else {
            NSLog("[minimap] core 布局 FFI 返回 NULL")
            return nil
        }
        let data = Data(bytes: json, count: strlen(json))
        glaspen2_free_c_string(json)
        guard let obj = try? JSONSerialization.jsonObject(with: data),
              let dict = obj as? NSDictionary else {
            NSLog("[minimap] core 布局 JSON 无法解析")
            return nil
        }
        if dict["error"] != nil {
            NSLog("[minimap] core 布局 FFI 错误: \(String(describing: dict["error"]!))")
            return nil
        }
        return dict
    }
}
