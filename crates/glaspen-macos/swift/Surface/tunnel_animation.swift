import FlutterMacOS
// S7: 时光隧道启动(状态/表面/预载建卡/参数/首帧/120Hz 定时器)。
import AppKit
import Darwin

@objc public final class TunnelAnimation: NSObject {
    @objc(startTunnelGoingNext:curPage:prepare:commit:)
    public static func start(goingNext: Bool, curPage: Int,
                             prepare: @escaping () -> Void,
                             commit: @escaping () -> Void) -> Bool {
        if TunnelState.active { return false }
        let cur = Int(curPage)
        TunnelLifecycle.freeCards()
        var preloadMs = 0.0
        let n = glaspen2_rust_tunnel_prepare(Int64(cur), goingNext ? 1 : 0,
                                              surfaceScale(), &preloadMs)
        if n <= 0 { return false }
        let target = goingNext ? glaspen2_page_neighbor(Int64(cur), 1)
                               : glaspen2_page_neighbor(Int64(cur), -1)
        let buildStart = TunnelState.now()
        let count = Int(n)
        let curIdx = count - 2
        let built = TunnelState.depths.withUnsafeMutableBufferPointer { dp in
            TunnelState.cardSurf.withUnsafeMutableBufferPointer { cp in
                TunnelState.mipSurf.withUnsafeMutableBufferPointer { mp in
                    cp.baseAddress!.withMemoryRebound(to: UnsafeMutableRawPointer?.self, capacity: TunnelState.maxCards) { cardPtr in
                        mp.baseAddress!.withMemoryRebound(to: UnsafeMutableRawPointer?.self, capacity: TunnelState.maxCards) { mipPtr in
                            dp.baseAddress!.withMemoryRebound(to: Int32.self, capacity: TunnelState.maxCards) { depthPtr in
                                glaspen2_rust_tunnel_build_cards(
                                    Int64(cur), Int32(count), Int32(curIdx),
                                    glaspen2_tunnel_front(), depthPtr, cardPtr, mipPtr)
                            }
                        }
                    }
                }
            }
        }
        if built < 0 {
            TunnelLifecycle.freeCards()
            return false
        }
        TunnelState.cards = Int(built)
        let tc2 = TunnelState.now()
        NSLog("[flip] 时光隧道接管: %d 张卡片(cur=%ld → %ld, 方向=%@) 预载%.0fms 建卡%.0fms",
              built, cur, target, goingNext ? "后" : "前",
              preloadMs, (tc2 - buildStart) * 1000.0)
        TunnelState.back = !goingNext
        TunnelState.travel = 1.0
        TunnelState.prepare = prepare
        TunnelState.commit = commit
        TunnelState.active = true

        glaspen2_rust_tunnel_apply_env()
        glaspen2_rust_tunnel_reset_start()
        TunnelFrameRenderer.render(p: 0.0)
        if let view = UIAnchors.drawView {
            view.displayIfNeeded()
        }
        glaspen2_rust_tunnel_timer_start()
        return true
    }
}


@objc public final class TunnelLifecycle: NSObject {
    @objc public static func freeCards() {
        TunnelState.cardSurf.withUnsafeBufferPointer { cp in
            TunnelState.mipSurf.withUnsafeBufferPointer { mp in
                cp.baseAddress!.withMemoryRebound(to: UnsafeMutableRawPointer?.self, capacity: TunnelState.maxCards) { cardPtr in
                    mp.baseAddress!.withMemoryRebound(to: UnsafeMutableRawPointer?.self, capacity: TunnelState.maxCards) { mipPtr in
                        glaspen2_rust_tunnel_free_cards(
                            Int32(TunnelState.maxCards), cardPtr, mipPtr,
                            TunnelState.blur.map(UnsafeMutableRawPointer.init),
                            TunnelState.surface.map(UnsafeMutableRawPointer.init))
                    }
                }
            }
        }
    }

    @objc public static func finish() {
        let now = TunnelState.now()
        TunnelState.depths.withUnsafeBufferPointer { depths in
            TunnelState.cardSurf.withUnsafeBufferPointer { cards in
                TunnelState.mipSurf.withUnsafeBufferPointer { mips in
                    cards.baseAddress!.withMemoryRebound(to: UnsafeMutableRawPointer?.self, capacity: TunnelState.maxCards) { cardPtr in
                        mips.baseAddress!.withMemoryRebound(to: UnsafeMutableRawPointer?.self, capacity: TunnelState.maxCards) { mipPtr in
                            glaspen2_rust_tunnel_finish(
                                TunnelState.t0, now, TunnelState.frames,
                                TunnelState.surface.map(UnsafeMutableRawPointer.init),
                                TunnelState.blur.map(UnsafeMutableRawPointer.init),
                                Int32(TunnelState.cards), depths.baseAddress, cardPtr, mipPtr,
                                TunnelState.back ? 1 : 0, TunnelState.travel, TunnelState.k,
                                TunnelState.thick, TunnelState.paneA,
                                Int32(TunnelState.surfW), Int32(TunnelState.surfH))
                        }
                    }
                }
            }
        }
        let prepare = TunnelState.prepare
        let commit = TunnelState.commit
        TunnelState.prepare = nil
        TunnelState.commit = nil
        prepare?()
        commit?()
    }
}


@objc public final class TunnelState: NSObject {
    static var active: Bool {
        get { glaspen2_rust_tunnel_get_active() != 0 }
        set { glaspen2_rust_tunnel_set_active(newValue ? 1 : 0) }
    }
    static var surface: OpaquePointer?
    static var blur: OpaquePointer?
    static var surfW: Int {
        get { Int(glaspen2_rust_tunnel_get_surf_w()) }
        set { glaspen2_rust_tunnel_set_surf_w(Int32(newValue)) }
    }
    static var surfH: Int {
        get { Int(glaspen2_rust_tunnel_get_surf_h()) }
        set { glaspen2_rust_tunnel_set_surf_h(Int32(newValue)) }
    }
    static var cards: Int {
        get { Int(glaspen2_rust_tunnel_get_cards()) }
        set { glaspen2_rust_tunnel_set_cards(Int32(newValue)) }
    }
    static var depths = [Int32](repeating: 0, count: 512)
    static var cardSurf = [OpaquePointer?](repeating: nil, count: 512)
    static var mipSurf = [OpaquePointer?](repeating: nil, count: 512)
    static var back: Bool {
        get { glaspen2_rust_tunnel_get_back() != 0 }
        set { glaspen2_rust_tunnel_set_back(newValue ? 1 : 0) }
    }
    static var travel: Double {
        get { glaspen2_rust_tunnel_get_travel() }
        set { glaspen2_rust_tunnel_set_travel(newValue) }
    }
    static var k: Double {
        get { glaspen2_rust_tunnel_get_k() }
        set { glaspen2_rust_tunnel_set_k(newValue) }
    }
    static var thick: Double {
        get { glaspen2_rust_tunnel_get_thick() }
        set { glaspen2_rust_tunnel_set_thick(newValue) }
    }
    static var paneA: Double {
        get { glaspen2_rust_tunnel_get_pane() }
        set { glaspen2_rust_tunnel_set_pane(newValue) }
    }
    static var dur: Double {
        get { glaspen2_rust_tunnel_get_dur() }
        set { glaspen2_rust_tunnel_set_dur(newValue) }
    }
    static var t0: Double {
        get { glaspen2_rust_tunnel_get_t0() }
        set { glaspen2_rust_tunnel_set_t0(newValue) }
    }
    static var frames: UInt64 {
        get { glaspen2_rust_tunnel_get_frames() }
        set { glaspen2_rust_tunnel_set_frames(newValue) }
    }
    static var timer: DispatchSourceTimer?
    static var prepare: (() -> Void)?
    static var commit: (() -> Void)?

    static let maxCards = 512

    static func now() -> Double {
        Date().timeIntervalSinceReferenceDate
    }
}




// S5-1: 覆盖层 NSWindow 创建与基础属性(层级/透明/穿透/集合行为)+ 光标。
// 内容视图/layer/玻璃/绘制视图仍由 ObjC 组装;这里只搬可参数化的窗口原语。
import AppKit





@_cdecl("invert_ink_source")
public func panelInvertInkSource(_ cr: UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? {
    if #available(macOS 12.3, *), let cr {
        return InvertCapture.inkSource(crRaw: cr)
    }
    return nil
}

@_cdecl("invert_ink_apply")
public func panelInvertApply(_ on: Int32) {
    if #available(macOS 12.3, *) { InvertCapture.apply(on: on != 0) }
}

@_cdecl("invert_stream_restart")
public func panelInvertRestart() {
    if #available(macOS 12.3, *) { InvertCapture.restart() }
}

@_cdecl("invert_stream_invalidate")
public func panelInvertInvalidate() {
    if #available(macOS 12.3, *) { InvertCapture.invalidate() }
}



// S7: 时光隧道卡片(玻璃厚度/阴影/前脸/扫光/棱线)绘制。
import AppKit
import Darwin

@objc public final class TunnelCardRenderer: NSObject {
    @objc(drawWithCR:surf:nw:nh:cx:cy:w:h:alpha:zc:surfW:surfH:thick:paneA:)
    public static func draw(crRaw: UnsafeMutableRawPointer, surfRaw: UnsafeMutableRawPointer,
                            nw: Int32, nh: Int32, cx: Double, cy: Double,
                            w: Double, h: Double, alpha: Double, zc: Double,
                            surfW: Int32, surfH: Int32, thick: Double, paneA: Double) {
        guard alpha > 0.02, w >= 1.0, h >= 1.0, nw > 0, nh > 0 else { return }
        let cr = OpaquePointer(crRaw)
        let surf = OpaquePointer(surfRaw)
        let cardX = cx - w / 2.0, cardY = cy - h / 2.0

        if abs(zc) < 0.005 {
            cairo_save(cr)
            cairo_rectangle(cr, 0, 0, Double(surfW), Double(surfH))
            cairo_clip(cr)
            cairo_translate(cr, cardX, cardY)
            cairo_scale(cr, w / Double(nw), h / Double(nh))
            cairo_set_source_surface(cr, surf, 0, 0)
            cairo_paint_with_alpha(cr, 1.0)
            cairo_restore(cr)
            return
        }

        var glass = abs(zc) / 0.10
        if glass > 1.0 { glass = 1.0 }
        let thickness = max(4.0, w * thick)
        let soff = max(3.0, w * 0.008)
        var zpos = zc > 0 ? zc : 0
        if zpos > 4.0 { zpos = 4.0 }
        var pane = (paneA + 0.02 * zpos) * glass
        if zc < 0 { pane *= alpha }
        if pane > 0.94 { pane = 0.94 }
        let T = thickness

        if abs(zc) < 1.6 {
            for i in stride(from: 2, through: 1, by: -1) {
                let off = Double(i) * soff * 1.6
                cairo_set_source_rgba(cr, 0, 0, 0, 0.028 * alpha * glass)
                cairo_rectangle(cr, cardX + off, cardY + off, w, h)
                cairo_fill(cr)
            }
        }

        // 上侧面
        if let top = cairo_pattern_create_linear(cardX, cardY, cardX + T, cardY - T) {
            cairo_pattern_add_color_stop_rgba(top, 0, 1.00, 1.00, 1.00, 0.74 * alpha * glass)
            cairo_pattern_add_color_stop_rgba(top, 1, 0.90, 0.91, 0.92, 0.52 * alpha * glass)
            cairo_set_source(cr, top)
            cairo_move_to(cr, cardX, cardY)
            cairo_line_to(cr, cardX + T, cardY - T)
            cairo_line_to(cr, cardX + w + T, cardY - T)
            cairo_line_to(cr, cardX + w, cardY)
            cairo_close_path(cr)
            cairo_fill(cr)
            cairo_pattern_destroy(top)
        }
        // 右侧面
        if let right = cairo_pattern_create_linear(cardX + w, cardY, cardX + w + T, cardY - T) {
            cairo_pattern_add_color_stop_rgba(right, 0, 0.70, 0.72, 0.74, 0.62 * alpha * glass)
            cairo_pattern_add_color_stop_rgba(right, 1, 0.28, 0.30, 0.33, 0.74 * alpha * glass)
            cairo_set_source(cr, right)
            cairo_move_to(cr, cardX + w, cardY)
            cairo_line_to(cr, cardX + w + T, cardY - T)
            cairo_line_to(cr, cardX + w + T, cardY + h - T)
            cairo_line_to(cr, cardX + w, cardY + h)
            cairo_close_path(cr)
            cairo_fill(cr)
            cairo_pattern_destroy(right)
        }
        // 前脸
        cairo_save(cr)
        cairo_rectangle(cr, cardX, cardY, w, h)
        cairo_clip(cr)
        if let fg = cairo_pattern_create_linear(cardX, cardY, cardX, cardY + h) {
            cairo_pattern_add_color_stop_rgba(fg, 0, 0.985, 0.99, 1.0, pane * 0.99)
            cairo_pattern_add_color_stop_rgba(fg, 0.55, 0.86, 0.875, 0.89, pane * 0.94)
            cairo_pattern_add_color_stop_rgba(fg, 1, 0.66, 0.675, 0.69, pane * 0.98)
            cairo_set_source(cr, fg)
            cairo_paint(cr)
            cairo_pattern_destroy(fg)
        }
        cairo_save(cr)
        cairo_translate(cr, cardX, cardY)
        cairo_scale(cr, w / Double(nw), h / Double(nh))
        cairo_set_source_surface(cr, surf, 0, 0)
        cairo_paint_with_alpha(cr, alpha)
        cairo_restore(cr)
        // 扫光
        if abs(zc) < 1.2, let sheen = cairo_pattern_create_linear(cardX, cardY, cardX + w, cardY + h * 0.75) {
            cairo_pattern_add_color_stop_rgba(sheen, 0, 1, 1, 1, 0.10 * alpha * glass)
            cairo_pattern_add_color_stop_rgba(sheen, 0.35, 1, 1, 1, 0.012 * alpha * glass)
            cairo_pattern_add_color_stop_rgba(sheen, 1, 1, 1, 1, 0.0)
            cairo_set_source(cr, sheen)
            cairo_paint(cr)
            cairo_pattern_destroy(sheen)
        }
        cairo_restore(cr)
        // 棱线
        cairo_save(cr)
        cairo_set_line_width(cr, max(1.0, T * 0.08))
        cairo_set_source_rgba(cr, 1, 1, 1, 0.42 * alpha * glass)
        cairo_move_to(cr, cardX + T, cardY - T)
        cairo_line_to(cr, cardX + w + T, cardY - T)
        cairo_stroke(cr)
        cairo_set_source_rgba(cr, 0.03, 0.03, 0.035, 0.48 * alpha * glass)
        cairo_move_to(cr, cardX + w + T, cardY - T)
        cairo_line_to(cr, cardX + w + T, cardY + h - T)
        cairo_stroke(cr)
        cairo_set_source_rgba(cr, 1, 1, 1, 0.50 * alpha * glass)
        cairo_move_to(cr, cardX, cardY)
        cairo_line_to(cr, cardX + w, cardY)
        cairo_stroke(cr)
        cairo_set_source_rgba(cr, 0.04, 0.04, 0.045, 0.38 * alpha * glass)
        cairo_move_to(cr, cardX + w, cardY)
        cairo_line_to(cr, cardX + w, cardY + h)
        cairo_stroke(cr)
        cairo_set_source_rgba(cr, 0.9, 0.92, 0.94, 0.18 * alpha * glass)
        cairo_move_to(cr, cardX, cardY)
        cairo_line_to(cr, cardX, cardY + h)
        cairo_stroke(cr)
        cairo_set_source_rgba(cr, 0.05, 0.05, 0.055, 0.20 * alpha * glass)
        cairo_move_to(cr, cardX, cardY + h)
        cairo_line_to(cr, cardX + w, cardY + h)
        cairo_stroke(cr)
        cairo_restore(cr)
    }
}


@objc public final class TunnelFrameRenderer: NSObject {
    @objc(renderWithP:)
    public static func render(p: Double) {
        guard let surface = TunnelState.surface, TunnelState.cards > 0 else { return }
        TunnelState.frames += 1
        let landed = TunnelState.depths.withUnsafeBufferPointer { depths in
            TunnelState.cardSurf.withUnsafeBufferPointer { cards in
                TunnelState.mipSurf.withUnsafeBufferPointer { mips in
                    cards.baseAddress!.withMemoryRebound(to: UnsafeMutableRawPointer?.self, capacity: TunnelState.maxCards) { cardPtr in
                        mips.baseAddress!.withMemoryRebound(to: UnsafeMutableRawPointer?.self, capacity: TunnelState.maxCards) { mipPtr in
                            glaspen2_rust_tunnel_render(
                                p, UnsafeMutableRawPointer(surface), TunnelState.blur.map(UnsafeMutableRawPointer.init),
                                Int32(TunnelState.cards), depths.baseAddress, cardPtr, mipPtr,
                                TunnelState.back ? 1 : 0, TunnelState.travel, TunnelState.k,
                                TunnelState.thick, TunnelState.paneA,
                                Int32(TunnelState.surfW), Int32(TunnelState.surfH))
                        }
                    }
                }
            }
        }
        _ = landed
    }

    /// 仅 parity 探针使用: 旧 Swift 帧渲染实现。
    static func renderSwiftForProbe(p: Double) {
        guard let surface = TunnelState.surface, TunnelState.cards > 0 else { return }
        TunnelState.frames += 1
        let e = p < 0.5 ? 4.0 * p * p * p : 1.0 - pow(-2.0 * p + 2.0, 3.0) / 2.0
        let cam = e * TunnelState.travel * (TunnelState.back ? 1.0 : -1.0)
        let surf = surface
        guard let cr = cairo_create(surf) else { return }
        cairo_set_operator(cr, cairo_operator_t(1))
        cairo_set_source_rgba(cr, 0, 0, 0, 0)
        cairo_paint(cr)
        cairo_set_operator(cr, cairo_operator_t(2))

        let slots = Array(0..<TunnelState.cards)
        let order = slots.sorted { TunnelState.depths[$0] > TunnelState.depths[$1] }
        for slot in order {
            let zc = Double(TunnelState.depths[slot]) - cam
            if abs(zc) < 0.005, let placed = place(zc) {
                drawCardSwift(cr, slot, placed, zc)
                cairo_destroy(cr)
                flush()
                return
            }
        }
        var focusZc = 1e9
        for slot in order {
            let zc = Double(TunnelState.depths[slot]) - cam
            if let placed = place(zc), placed.4 >= 0.30, zc < focusZc {
                focusZc = zc
            }
        }
        if focusZc > 1e8 { focusZc = 0.0 }

        if let blurPtr = TunnelState.blur {
            let blur = blurPtr
            let bw = cairo_image_surface_get_width(blur)
            let bh = cairo_image_surface_get_height(blur)
            if let bcr = cairo_create(blur) {
                cairo_set_operator(bcr, cairo_operator_t(1))
                cairo_set_source_rgba(bcr, 0, 0, 0, 0)
                cairo_paint(bcr)
                cairo_set_operator(bcr, cairo_operator_t(2))
                cairo_scale(bcr, Double(bw) / Double(TunnelState.surfW),
                            Double(bh) / Double(TunnelState.surfH))
                for slot in order {
                    let zc = Double(TunnelState.depths[slot]) - cam
                    if zc <= focusZc + 1e-6 { continue }
                    if let placed = place(zc) { drawCardSwift(bcr, slot, placed, zc) }
                }
                cairo_destroy(bcr)
                cairo_save(cr)
                cairo_scale(cr, Double(TunnelState.surfW) / Double(bw),
                            Double(TunnelState.surfH) / Double(bh))
                cairo_set_source_surface(cr, blur, 0, 0)
                if let src = cairo_get_source(cr) {
                    cairo_pattern_set_filter(src, cairo_filter_t(2))
                }
                cairo_paint(cr)
                cairo_restore(cr)
            }
        }
        for slot in order {
            let zc = Double(TunnelState.depths[slot]) - cam
            if TunnelState.blur != nil && zc > focusZc + 1e-6 { continue }
            if let placed = place(zc) { drawCardSwift(cr, slot, placed, zc) }
        }
        cairo_destroy(cr)
        flush()
    }

    private static func flush() {
        if let view = UIAnchors.drawView {
            view.needsDisplay = true
        }
    }

    private static func place(_ zc: Double) -> (Double, Double, Double, Double, Double)? {
        var out = [Double](repeating: 0, count: 5)
        let ok = out.withUnsafeMutableBufferPointer { buf in
            glaspen2_tunnel_place(zc, TunnelState.k, TunnelState.travel,
                                  Double(TunnelState.surfW), Double(TunnelState.surfH),
                                  buf.baseAddress)
        }
        guard ok != 0 else { return nil }
        return (out[0], out[1], out[2], out[3], out[4])
    }

    private static func drawCardSwift(_ cr: OpaquePointer, _ slot: Int,
                                 _ placed: (Double, Double, Double, Double, Double),
                                 _ zc: Double) {
        guard let mainPtr = TunnelState.cardSurf[slot] else { return }
        var surf = mainPtr
        if let mipPtr = TunnelState.mipSurf[slot] {
            let mainW = Double(cairo_image_surface_get_width(mainPtr))
            if placed.2 < mainW * 0.70 { surf = mipPtr }
        }
        TunnelCardRenderer.draw(crRaw: UnsafeMutableRawPointer(cr),
                                surfRaw: UnsafeMutableRawPointer(surf),
                                nw: cairo_image_surface_get_width(surf),
                                nh: cairo_image_surface_get_height(surf),
                                cx: placed.0, cy: placed.1,
                                w: placed.2, h: placed.3,
                                alpha: placed.4, zc: zc,
                                surfW: Int32(TunnelState.surfW),
                                surfH: Int32(TunnelState.surfH),
                                thick: TunnelState.thick,
                                paneA: TunnelState.paneA)
    }
}

// R79: Rust 卡片绘制探针(创建 cairo 目标/卡片表面, 调用 Rust, 检查非空像素)。
@_cdecl("glaspen2_macos_swift_tunnel_card_selftest")
public func glaspen2TunnelCardSelftest() -> Int32 {
    let sw = 240, sh = 180
    guard let target = cairo_image_surface_create(cairo_format_t(0), Int32(sw), Int32(sh)),
          cairo_surface_status(target).rawValue == 0,
          let cr = cairo_create(target) else {
        print("SWIFT_TUNNEL_CARD_FAIL surface")
        return 1
    }
    guard let card = cairo_image_surface_create(cairo_format_t(0), 80, 60),
          cairo_surface_status(card).rawValue == 0,
          let ccr = cairo_create(card) else {
        print("SWIFT_TUNNEL_CARD_FAIL card")
        cairo_destroy(cr)
        cairo_surface_destroy(target)
        return 1
    }
    cairo_set_source_rgba(ccr, 0.2, 0.5, 0.8, 1.0)
    cairo_paint(ccr)
    cairo_destroy(ccr)
    cairo_surface_flush(card)
    glaspen2_rust_draw_tunnel_card(
        UnsafeMutableRawPointer(cr), UnsafeMutableRawPointer(card),
        80, 60, 120.0, 90.0, 140.0, 110.0, 1.0, 0.05,
        Int32(sw), Int32(sh), 0.018, 0.84)
    cairo_destroy(cr)
    cairo_surface_flush(target)
    guard let data = cairo_image_surface_get_data(target) else {
        print("SWIFT_TUNNEL_CARD_FAIL data")
        cairo_surface_destroy(card)
        cairo_surface_destroy(target)
        return 1
    }
    var nonEmpty = 0
    let stride = Int(cairo_image_surface_get_stride(target))
    for y in 0..<sh {
        for x in 0..<sw {
            let i = y * stride + x * 4
            if data[i] != 0 || data[i + 1] != 0 || data[i + 2] != 0 { nonEmpty += 1 }
        }
    }
    cairo_surface_destroy(card)
    cairo_surface_destroy(target)
    print("SWIFT_TUNNEL_CARD_RUST_OK pixels=\(nonEmpty)")
    return nonEmpty > 0 ? 0 : 1
}

// R82: Rust / Swift 隧道卡片绘制像素对比探针。
@_cdecl("glaspen2_macos_swift_tunnel_card_parity_selftest")
public func glaspen2TunnelCardParitySelftest() -> Int32 {
    let sw = 240, sh = 180
    func makeCard() -> OpaquePointer? {
        guard let card = cairo_image_surface_create(cairo_format_t(0), 80, 60),
              cairo_surface_status(card).rawValue == 0,
              let ccr = cairo_create(card) else { return nil }
        cairo_set_source_rgba(ccr, 0.2, 0.5, 0.8, 1.0)
        cairo_paint(ccr)
        cairo_destroy(ccr)
        cairo_surface_flush(card)
        return card
    }
    func render(useRust: Bool, card: OpaquePointer) -> OpaquePointer? {
        guard let target = cairo_image_surface_create(cairo_format_t(0), Int32(sw), Int32(sh)),
              cairo_surface_status(target).rawValue == 0,
              let cr = cairo_create(target) else { return nil }
        if useRust {
            glaspen2_rust_draw_tunnel_card(
                UnsafeMutableRawPointer(cr), UnsafeMutableRawPointer(card),
                80, 60, 120.0, 90.0, 140.0, 110.0, 1.0, 0.05,
                Int32(sw), Int32(sh), 0.018, 0.84)
        } else {
            TunnelCardRenderer.draw(
                crRaw: UnsafeMutableRawPointer(cr), surfRaw: UnsafeMutableRawPointer(card),
                nw: 80, nh: 60, cx: 120.0, cy: 90.0, w: 140.0, h: 110.0,
                alpha: 1.0, zc: 0.05, surfW: Int32(sw), surfH: Int32(sh),
                thick: 0.018, paneA: 0.84)
        }
        cairo_destroy(cr)
        cairo_surface_flush(target)
        return target
    }
    guard let card = makeCard(),
          let a = render(useRust: true, card: card),
          let b = render(useRust: false, card: card),
          let da = cairo_image_surface_get_data(a),
          let db = cairo_image_surface_get_data(b) else {
        print("SWIFT_TUNNEL_CARD_PARITY_FAIL surface")
        return 1
    }
    let stride = Int(cairo_image_surface_get_stride(a))
    var diff = 0
    let total = sw * sh
    for y in 0..<sh {
        for x in 0..<sw {
            let i = y * stride + x * 4
            if da[i] != db[i] || da[i + 1] != db[i + 1] || da[i + 2] != db[i + 2] || da[i + 3] != db[i + 3] {
                diff += 1
            }
        }
    }
    cairo_surface_destroy(card)
    cairo_surface_destroy(a)
    cairo_surface_destroy(b)
    print("SWIFT_TUNNEL_CARD_PARITY diff=\(diff)/\(total)")
    return diff == 0 ? 0 : 1
}

// R84: 整帧 Rust/Swift 渲染像素对比探针(伪 TunnelState, 400x300)。
@_cdecl("glaspen2_macos_swift_tunnel_frame_parity_selftest")
public func glaspen2TunnelFrameParitySelftest() -> Int32 {
    let w = 400, h = 300
    func makeTarget() -> OpaquePointer? {
        guard let s = cairo_image_surface_create(cairo_format_t(0), Int32(w), Int32(h)),
              cairo_surface_status(s).rawValue == 0 else { return nil }
        cairo_surface_flush(s)
        return s
    }
    func makeCard() -> OpaquePointer? {
        guard let s = cairo_image_surface_create(cairo_format_t(0), 100, 80),
              cairo_surface_status(s).rawValue == 0,
              let cr = cairo_create(s) else { return nil }
        cairo_set_source_rgba(cr, 0.25, 0.55, 0.85, 1.0)
        cairo_paint(cr)
        cairo_destroy(cr)
        cairo_surface_flush(s)
        return s
    }
    guard let a = makeTarget(), let b = makeTarget(), let card = makeCard() else {
        print("SWIFT_TUNNEL_FRAME_PARITY_FAIL surface")
        return 1
    }
    var depths = [Int32](repeating: 0, count: TunnelState.maxCards)
    depths[0] = 0
    depths[1] = 1
    var cards = [OpaquePointer?](repeating: nil, count: TunnelState.maxCards)
    cards[0] = card
    cards[1] = card
    var mips = [OpaquePointer?](repeating: nil, count: TunnelState.maxCards)
    TunnelState.surface = a
    TunnelState.blur = nil
    TunnelState.cards = 2
    TunnelState.depths = depths
    TunnelState.cardSurf = cards
    TunnelState.mipSurf = mips
    TunnelState.back = true
    TunnelState.travel = 1.0
    TunnelState.k = 0.45
    TunnelState.thick = 0.018
    TunnelState.paneA = 0.84
    TunnelState.surfW = w
    TunnelState.surfH = h

    let rustLanded = depths.withUnsafeBufferPointer { dp in
        cards.withUnsafeBufferPointer { cp in
            mips.withUnsafeBufferPointer { mp in
                cp.baseAddress!.withMemoryRebound(to: UnsafeMutableRawPointer?.self, capacity: TunnelState.maxCards) { cardPtr in
                    mp.baseAddress!.withMemoryRebound(to: UnsafeMutableRawPointer?.self, capacity: TunnelState.maxCards) { mipPtr in
                        glaspen2_rust_tunnel_render(
                            0.37, UnsafeMutableRawPointer(a), nil, 2,
                            dp.baseAddress, cardPtr, mipPtr, 1, 1.0, 0.45, 0.018, 0.84,
                            Int32(w), Int32(h))
                    }
                }
            }
        }
    }
    TunnelState.surface = b
    TunnelFrameRenderer.renderSwiftForProbe(p: 0.37)
    cairo_surface_flush(a)
    cairo_surface_flush(b)
    guard let da = cairo_image_surface_get_data(a), let db = cairo_image_surface_get_data(b) else {
        print("SWIFT_TUNNEL_FRAME_PARITY_FAIL data")
        return 1
    }
    let stride = Int(cairo_image_surface_get_stride(a))
    var diff = 0
    for y in 0..<h {
        for x in 0..<w {
            let i = y * stride + x * 4
            if da[i] != db[i] || da[i + 1] != db[i + 1] || da[i + 2] != db[i + 2] || da[i + 3] != db[i + 3] {
                diff += 1
            }
        }
    }
    cairo_surface_destroy(card)
    cairo_surface_destroy(a)
    cairo_surface_destroy(b)
    print("SWIFT_TUNNEL_FRAME_PARITY diff=\(diff)/\(w*h) landed=\(rustLanded)")
    return diff == 0 ? 0 : 1
}


@_cdecl("glaspen2_swift_tunnel_render")
public func tunnelRender(_ p: Double) {
    TunnelFrameRenderer.render(p: p)
}
@_cdecl("glaspen2_swift_tunnel_finish")
public func tunnelFinish() { TunnelLifecycle.finish() }
