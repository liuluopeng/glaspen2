// S7: 实时绘制计划(core glaspen2_live_plan_json)入参/解析/命令光栅化/接缝/脏区。
import AppKit

@objc public final class LivePlan: NSObject {
    private static var segmentFailureLogged = false
    private static var dotFailureLogged = false

    // MARK: - cairo helpers

    private static func setDash(_ cr: OpaquePointer, _ dashes: [Double], _ off: Double) {
        if dashes.isEmpty {
            cairo_set_dash(cr, nil, 0, 0)
            return
        }
        dashes.withUnsafeBufferPointer {
            cairo_set_dash(cr, $0.baseAddress, Int32($0.count), off)
        }
    }

    private static func color(_ cr: OpaquePointer, _ color: Any?, _ alpha: Double) {
        var r = 0.0, g = 0.0, b = 0.0
        if let arr = color as? [Any], arr.count == 3 {
            r = (arr[0] as? NSNumber)?.doubleValue ?? 0
            g = (arr[1] as? NSNumber)?.doubleValue ?? 0
            b = (arr[2] as? NSNumber)?.doubleValue ?? 0
        }
        cairo_set_source_rgba(cr, r, g, b, alpha)
    }

    // MARK: - command rasterizer (对齐 core 整页重建)

    private static func command(_ cr: OpaquePointer, _ command: [String: Any],
                                _ invScale: Double) {
        guard let pass = command["pass"] as? String,
              let points = command["points"] as? [[Any]], !points.isEmpty else { return }
        let alpha = (command["alpha"] as? NSNumber)?.doubleValue ?? 0
        let widthDelta = (command["width_delta"] as? NSNumber)?.doubleValue ?? 0
        setDash(cr, [], 0)
        cairo_set_operator(cr, cairo_operator_t(2)) // OVER
        cairo_set_line_cap(cr, cairo_line_cap_t(1))   // ROUND
        cairo_set_line_join(cr, cairo_line_join_t(1)) // ROUND

        func px(_ p: [Any], _ i: Int) -> Double {
            (i < p.count ? (p[i] as? NSNumber)?.doubleValue : nil) ?? 0
        }

        if pass == "shadow" {
            color(cr, command["color"], alpha)
            if points.count == 1 {
                let p = points[0]
                guard p.count >= 3 else { return }
                cairo_arc(cr, px(p, 0) * invScale, px(p, 1) * invScale,
                          px(p, 2) * 0.5 * invScale, 0, 2 * Double.pi)
                cairo_fill(cr)
                return
            }
            for i in 1..<points.count {
                let a = points[i - 1], b = points[i]
                guard a.count >= 3, b.count >= 3 else { continue }
                cairo_set_line_width(cr, (px(b, 2) + widthDelta) * invScale)
                cairo_move_to(cr, px(a, 0) * invScale, px(a, 1) * invScale)
                cairo_line_to(cr, px(b, 0) * invScale, px(b, 1) * invScale)
                cairo_stroke(cr)
            }
            return
        }
        if pass == "outline" {
            if points.count == 1 {
                let p = points[0]
                guard p.count >= 3 else { return }
                setDash(cr, [], 0)
                cairo_set_source_rgba(cr, 0, 0, 0, alpha)
                cairo_arc(cr, px(p, 0) * invScale, px(p, 1) * invScale,
                          px(p, 2) * 0.5 * invScale, 0, 2 * Double.pi)
                cairo_fill(cr)
                return
            }
            guard points.count >= 2 else { return }
            let dashInfo = command["dash"] as? [String: Any]
            var dashLen = (dashInfo?["len"] as? NSNumber)?.doubleValue ?? 1.0
            dashLen *= invScale
            if !(dashLen > 0) { dashLen = 1.0 }
            var cum = (dashInfo?["phase"] as? NSNumber)?.doubleValue ?? 0
            cum *= invScale
            let pattern = [dashLen, dashLen]
            cairo_set_line_cap(cr, cairo_line_cap_t(2)) // BUTT
            for i in 1..<points.count {
                let a = points[i - 1], b = points[i]
                guard a.count >= 3, b.count >= 3 else { continue }
                let x0 = px(a, 0) * invScale, y0 = px(a, 1) * invScale
                let x1 = px(b, 0) * invScale, y1 = px(b, 1) * invScale
                let segWidth = (px(b, 2) + widthDelta) * invScale
                var off = cum.truncatingRemainder(dividingBy: 2.0 * dashLen)
                if off < 0 { off += 2.0 * dashLen }
                cairo_set_line_width(cr, segWidth)
                setDash(cr, pattern, off)
                cairo_set_source_rgba(cr, 0, 0, 0, alpha)
                cairo_move_to(cr, x0, y0)
                cairo_line_to(cr, x1, y1)
                cairo_stroke(cr)
                let nxt = (off + dashLen).truncatingRemainder(dividingBy: 2.0 * dashLen)
                setDash(cr, pattern, nxt)
                cairo_set_source_rgba(cr, 1, 1, 1, alpha)
                cairo_move_to(cr, x0, y0)
                cairo_line_to(cr, x1, y1)
                cairo_stroke(cr)
                let dx = x1 - x0, dy = y1 - y0
                cum += (dx * dx + dy * dy).squareRoot()
            }
            setDash(cr, [], 0)
            cairo_set_line_cap(cr, cairo_line_cap_t(1))
            return
        }
        if pass == "erase" {
            cairo_set_operator(cr, cairo_operator_t(3)) // CLEAR
            cairo_set_source_rgba(cr, 0, 0, 0, 1)
            if points.count == 1 {
                let p = points[0]
                guard p.count >= 3 else { return }
                cairo_arc(cr, px(p, 0) * invScale, px(p, 1) * invScale,
                          px(p, 2) * 0.5 * invScale, 0, 2 * Double.pi)
                cairo_fill(cr)
            } else {
                for i in 1..<points.count {
                    let a = points[i - 1], b = points[i]
                    guard a.count >= 3, b.count >= 3 else { continue }
                    cairo_set_line_width(cr, px(b, 2) * invScale)
                    cairo_move_to(cr, px(a, 0) * invScale, px(a, 1) * invScale)
                    cairo_line_to(cr, px(b, 0) * invScale, px(b, 1) * invScale)
                    cairo_stroke(cr)
                }
            }
            cairo_set_operator(cr, cairo_operator_t(2))
            return
        }
        // fill
        let invertPat = glaspen2_router_invert_ink_source(cr)
        if invertPat == nil {
            color(cr, command["color"], alpha)
        }
        if points.count == 1 {
            let p = points[0]
            if p.count >= 3 {
                cairo_arc(cr, px(p, 0) * invScale, px(p, 1) * invScale,
                          px(p, 2) * 0.5 * invScale, 0, 2 * Double.pi)
                cairo_fill(cr)
            }
            if let invertPat { cairo_pattern_destroy(invertPat) }
            return
        }
        for i in 1..<points.count {
            let a = points[i - 1], b = points[i]
            guard a.count >= 3, b.count >= 3 else { continue }
            cairo_set_line_width(cr, px(b, 2) * invScale)
            cairo_move_to(cr, px(a, 0) * invScale, px(a, 1) * invScale)
            cairo_line_to(cr, px(b, 0) * invScale, px(b, 1) * invScale)
            cairo_stroke(cr)
        }
        if let invertPat { cairo_pattern_destroy(invertPat) }
    }

    private static func seam(_ cr: OpaquePointer, _ seam: Any?, _ invScale: Double) {
        guard let s = seam as? [String: Any],
              let from = s["from"] as? [Any], from.count >= 2,
              let to = s["to"] as? [Any], to.count >= 2 else { return }
        let width = ((s["width"] as? NSNumber)?.doubleValue ?? 0) * invScale
        guard width > 0 else { return }
        func p(_ arr: [Any], _ i: Int) -> Double {
            (arr[i] as? NSNumber)?.doubleValue ?? 0
        }
        setDash(cr, [], 0)
        cairo_set_operator(cr, cairo_operator_t(2))
        cairo_set_line_cap(cr, cairo_line_cap_t(1))
        cairo_set_line_join(cr, cairo_line_join_t(1))
        cairo_set_line_width(cr, width)
        cairo_set_source_rgba(cr, glaspen2_router_pen_r(),
                              glaspen2_router_pen_g(), glaspen2_router_pen_b(), 1)
        cairo_move_to(cr, p(from, 0) * invScale, p(from, 1) * invScale)
        cairo_line_to(cr, p(to, 0) * invScale, p(to, 1) * invScale)
        cairo_stroke(cr)
    }

    // MARK: - plan execution

    private static func runPlan(_ cr: OpaquePointer, _ input: [String: Any],
                                _ invScale: Double, dot: Bool,
                                fallback: [(Double, Double, Double)]) -> Bool {
        guard let inputData = try? JSONSerialization.data(withJSONObject: input) else {
            if dot {
                if !dotFailureLogged { dotFailureLogged = true
                    NSLog("[d9c] 首点绘制计划入参序列化失败, 回退旧分档绘制") }
            } else {
                if !segmentFailureLogged { segmentFailureLogged = true
                    NSLog("[d9c] 实时绘制计划入参序列化失败, 回退旧逐段绘制") }
            }
            return false
        }
        let inputText = String(data: inputData, encoding: .utf8) ?? ""
        guard let outputRaw = inputText.withCString({
            glaspen2_live_plan_json($0)
        }) else {
            if dot {
                if !dotFailureLogged { dotFailureLogged = true
                    NSLog("[d9c] glaspen2_live_plan_json 返回 NULL(首点), 回退旧分档绘制") }
            } else {
                if !segmentFailureLogged { segmentFailureLogged = true
                    NSLog("[d9c] glaspen2_live_plan_json 返回 NULL, 回退旧逐段绘制") }
            }
            return false
        }
        let outputText = String(cString: outputRaw)
        glaspen2_free_c_string(outputRaw)
        guard let outputData = outputText.data(using: .utf8),
              let root = try? JSONSerialization.jsonObject(with: outputData),
              let plan = root as? [String: Any] else {
            if dot {
                if !dotFailureLogged { dotFailureLogged = true
                    NSLog("[d9c] 首点绘制计划 JSON 解析失败, 回退旧分档绘制") }
            } else {
                if !segmentFailureLogged { segmentFailureLogged = true
                    NSLog("[d9c] 实时绘制计划 JSON 解析失败, 回退旧逐段绘制") }
            }
            return false
        }
        if let err = plan["error"] {
            if dot {
                if !dotFailureLogged { dotFailureLogged = true
                    NSLog("[d9c] 首点绘制计划返回错误, 回退旧分档绘制: %@", "\(err)") }
            } else {
                if !segmentFailureLogged { segmentFailureLogged = true
                    NSLog("[d9c] 实时绘制计划返回错误, 回退旧逐段绘制: %@", "\(err)") }
            }
            return false
        }
        guard let commands = plan["commands"] as? [[String: Any]],
              let dirty = plan["dirty"] as? [String: Any] else { return false }
        cairo_save(cr)
        for cmd in commands { self.command(cr, cmd, invScale) }
        if !dot { seam(cr, plan["seam"], invScale) }
        cairo_restore(cr)

        let dx = ((dirty["x"] as? NSNumber)?.doubleValue ?? 0) * invScale
        let dy = ((dirty["y"] as? NSNumber)?.doubleValue ?? 0) * invScale
        let dw = ((dirty["w"] as? NSNumber)?.doubleValue ?? 0) * invScale
        let dh = ((dirty["h"] as? NSNumber)?.doubleValue ?? 0) * invScale
        if dx.isFinite, dy.isFinite, dw.isFinite, dh.isFinite, dw > 0, dh > 0 {
            glaspen2_router_dirty_include_surface_rect(dx, dy, dw, dh)
        } else {
            for (fx, fy, pad) in fallback {
                glaspen2_router_dirty_include_surface_point(fx, fy, pad)
            }
        }
        return true
    }

    @objc(drawDotContext:x:y:width:)
    public static func drawDot(context: UnsafeMutableRawPointer, x: Double, y: Double,
                               width: Double) -> Bool {
        guard glaspen2_router_surface() != nil else { return false }
        let cr = OpaquePointer(context)
        guard x.isFinite, y.isFinite, width.isFinite,
              glaspen2_router_pen_r().isFinite, glaspen2_router_pen_g().isFinite,
              glaspen2_router_pen_b().isFinite else { return false }
        let scale = max(glaspen2_router_scale(), 0.0001)
        guard let surface = glaspen2_router_surface() else { return false }
        let surfaceW = Int(cairo_image_surface_get_width(surface))
        let surfaceH = Int(cairo_image_surface_get_height(surface))
        if surfaceW <= 0 || surfaceH <= 0 { return false }
        let pad = width * 0.5 + 1.5 + (glaspen2_router_soft_shadow() != 0 ? 5.0 : 0.0)
        let input: [String: Any] = [
            "points": [[x, y, width]],
            "params": [
                "viewport": [surfaceW, surfaceH],
                "scale": scale,
                "zoom": 1.0,
                "pan": [0.0, 0.0],
                "shadow": glaspen2_router_soft_shadow() != 0,
                "outline": glaspen2_router_outline_enabled() != 0,
                "eraser": glaspen2_router_eraser_mode() != 0,
                "alpha": 1.0,
                "arc_len_start": 0.0,
                "color": [glaspen2_router_pen_r(), glaspen2_router_pen_g(),
                          glaspen2_router_pen_b()],
            ],
        ]
        return runPlan(cr, input, 1.0 / scale, dot: true, fallback: [(x, y, pad)])
    }

    @objc(drawSegmentContext:x:y:width:)
    public static func drawSegment(context: UnsafeMutableRawPointer, x: Double, y: Double,
                                   width: Double) -> Bool {
        guard glaspen2_router_surface() != nil else { return false }
        let st = glaspen2_router_raw_state() as NSDictionary
        func num(_ k: String) -> Double { (st[k] as? NSNumber)?.doubleValue ?? 0 }
        func flag(_ k: String) -> Bool { (st[k] as? NSNumber)?.boolValue ?? false }
        guard flag("has_last") else { return false }
        guard x.isFinite, y.isFinite, width.isFinite,
              num("last_x").isFinite, num("last_y").isFinite,
              num("last_w").isFinite, num("path_len").isFinite,
              glaspen2_router_pen_r().isFinite, glaspen2_router_pen_g().isFinite,
              glaspen2_router_pen_b().isFinite,
              num("prev_x").isFinite, num("prev_y").isFinite else { return false }
        let scale = max(glaspen2_router_scale(), 0.0001)
        guard let surface = glaspen2_router_surface() else { return false }
        let surfaceW = Int(cairo_image_surface_get_width(surface))
        let surfaceH = Int(cairo_image_surface_get_height(surface))
        if surfaceW <= 0 || surfaceH <= 0 { return false }
        let cr = OpaquePointer(context)
        var input: [String: Any] = [
            "points": [[num("last_x"), num("last_y"), num("last_w")], [x, y, width]],
            "params": [
                "viewport": [surfaceW, surfaceH],
                "scale": scale,
                "zoom": 1.0,
                "pan": [0.0, 0.0],
                "shadow": glaspen2_router_soft_shadow() != 0,
                "outline": glaspen2_router_outline_enabled() != 0,
                "eraser": glaspen2_router_eraser_mode() != 0,
                "alpha": 1.0,
                "arc_len_start": num("path_len"),
                "color": [glaspen2_router_pen_r(), glaspen2_router_pen_g(),
                          glaspen2_router_pen_b()],
            ],
        ]
        if flag("has_prev") {
            input["seam"] = [num("prev_x"), num("prev_y")]
        }
        let shadowPad = glaspen2_router_soft_shadow() != 0 ? 5.0 : 0.0
        let pad = width * 0.5 + 1.5 + shadowPad
        let lastPad = num("last_w") * 0.5 + 1.5 + shadowPad
        return runPlan(cr, input, 1.0 / scale, dot: false,
                       fallback: [(x, y, pad), (num("last_x"), num("last_y"), lastPad)])
    }
}
