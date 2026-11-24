// S7: 从 core 加载颜色/宽度预设到 ObjC 表(数量不一致时告警)。
import AppKit

@objc public final class PenPresets: NSObject {
    @objc public static func loadPresets() {
        let coreColors = Int(glaspen2_color_preset_count())
        let coreWidths = Int(glaspen2_width_preset_count())
        let colors = Int(glaspen2_router_color_preset_count())
        let widths = Int(glaspen2_router_width_preset_count())
        if coreColors != colors || coreWidths != widths {
            NSLog("[glaspen2] WARN: core 预设数量与 ObjC 表不一致 (color \(coreColors)/\(colors), width \(coreWidths)/\(widths))")
        }
        for i in 0..<colors {
            var r = 0.0, g = 0.0, b = 0.0
            glaspen2_color_preset_rgb(Int32(i), &r, &g, &b)
            glaspen2_router_set_color_preset(Int32(i), r, g, b)
        }
        for i in 0..<widths {
            glaspen2_router_set_width_preset(Int32(i), glaspen2_width_preset_value(Int32(i)))
        }
    }
}
