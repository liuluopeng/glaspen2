// S8 自检: 模型器写入 → 笔画提交 → 画布总览/PNG 载荷(隐藏 CLI 入口; 需临时 DB)。
import AppKit

@objc public final class RenderProbe: NSObject {
    @objc public static func run() -> Int32 {
        var failures = 0
        func check(_ name: String, _ ok: Bool) {
            if !ok { NSLog("[render-probe] FAIL %@", name); failures += 1 }
            else { print("RENDER_PROBE \(name) ok") }
        }

        glaspen2_init_db(1920, 1080)
        glaspen2_modeler_clear_buffer()
        let t0 = Date().timeIntervalSinceReferenceDate
        glaspen2_modeler_begin(1.0, 0.0, 0.0, 200.0, 300.0, 0.5, t0, 1.0)
        for i in 1...60 {
            let x = 200.0 + Double(i) * 4.0
            let y = 300.0 + sin(Double(i) / 9.0) * 80.0
            glaspen2_modeler_move(x, y, 0.5, t0 + Double(i) * 0.008, 1.0)
        }
        glaspen2_modeler_end(440.0, 300.0, 0.5, t0 + 0.5, 1.0)
        glaspen2_modeler_commit_to_strokes(1.0, 0.0, 0.0)

        check("stroke_count", glaspen2_stroke_count() == 1)
        check("stroke_points", glaspen2_get_stroke_point_count(0) > 10)

        let overview = SettingsGlue.canvasOverviewPayload(width: 800, height: 600)
        check("overview_non_nil", overview != nil)
        if let overview {
            let png = overview["png"] as? Data
            check("overview_png", (png?.count ?? 0) > 0)
            check("overview_rect", (overview["rect"] as? [NSNumber])?.count == 4)
            check("overview_bounds", (overview["bounds"] as? [NSNumber])?.count == 4)
        }

        var rect = [Double](repeating: 0, count: 8)
        var len: Int32 = 0
        let payload = rect.withUnsafeMutableBufferPointer { buf in
            settingsApiCanvasPayload(800, 600, 0, buf.baseAddress, &len)
        }
        check("canvas_payload_non_nil", payload != nil)
        check("canvas_payload_len", len > 0)
        if let payload { free(payload) }
        check("canvas_payload_rect", rect[2] > 0 && rect[3] > 0)

        // action 3 = 新建(清空当前画布 + 镜头回原点), 应清掉笔画。
        var len2: Int32 = 0
        let payload2 = settingsApiCanvasPayload(800, 600, 3, nil, &len2)
        if let payload2 { free(payload2) }
        check("clear_action", glaspen2_stroke_count() == 0)

        print("SWIFT_RENDER_PROBE_OK failures=\(failures)")
        return failures == 0 ? 0 : 1
    }
}

@_cdecl("glaspen2_swift_render_probe_selftest")
public func renderProbeSelftest() -> Int32 { RenderProbe.run() }
