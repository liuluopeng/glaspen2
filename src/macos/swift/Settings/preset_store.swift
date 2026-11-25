// S7: 颜色/宽度预设表(由 core 填充) — C 符号导出。
import AppKit

@objc public final class PresetStore: NSObject {
    static var colors: [(Double, Double, Double)] = []
    static var widths: [Double] = []

    static func rgb(_ idx: Int) -> (Double, Double, Double) {
        if idx < 0 || idx >= colors.count { return (0, 0, 0) }
        return colors[idx]
    }
    static func setColor(_ idx: Int, _ r: Double, _ g: Double, _ b: Double) {
        while colors.count <= idx { colors.append((0, 0, 0)) }
        colors[idx] = (r, g, b)
    }
    static func setWidth(_ idx: Int, _ value: Double) {
        while widths.count <= idx { widths.append(0) }
        widths[idx] = value
    }

    static func width(_ idx: Int) -> Double {
        if idx < 0 || idx >= widths.count { return 0 }
        return widths[idx]
    }
}

@_cdecl("glaspen2_router_color_preset_count")
public func presetColorCount() -> Int32 { Int32(PresetStore.colors.count) }
@_cdecl("glaspen2_router_width_preset_count")
public func presetWidthCount() -> Int32 { Int32(PresetStore.widths.count) }

@_cdecl("glaspen2_router_set_color_preset")
public func presetSetColor(_ idx: Int32, _ r: Double, _ g: Double, _ b: Double) {
    let i = Int(idx)
    while PresetStore.colors.count <= i { PresetStore.colors.append((0, 0, 0)) }
    PresetStore.colors[i] = (r, g, b)
}

@_cdecl("glaspen2_router_set_width_preset")
public func presetSetWidth(_ idx: Int32, _ value: Double) {
    let i = Int(idx)
    while PresetStore.widths.count <= i { PresetStore.widths.append(0) }
    PresetStore.widths[i] = value
}

@_cdecl("glaspen2_router_color_preset_rgb_at")
public func presetColorAt(_ idx: Int32, _ r: UnsafeMutablePointer<Double>?,
                          _ g: UnsafeMutablePointer<Double>?,
                          _ b: UnsafeMutablePointer<Double>?) {
    let (cr, cg, cb) = PresetStore.rgb(Int(idx))
    r?.pointee = cr
    g?.pointee = cg
    b?.pointee = cb
}

@_cdecl("glaspen2_router_width_preset_scale_at")
public func presetWidthAt(_ idx: Int32) -> Double { PresetStore.width(Int(idx)) }
