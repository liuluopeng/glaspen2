// S7: 时光隧道逐帧渲染(状态在 Swift; 排序/落定帧/模糊缓冲/近层)。
import AppKit
import Darwin

@objc public final class TunnelFrameRenderer: NSObject {
    @objc(renderWithP:)
    public static func render(p: Double) {
        guard let surface = TunnelState.surface, TunnelState.cards > 0 else { return }
        if ProcessInfo.processInfo.environment["GLASPEN2_SWIFT_TUNNEL_FRAME"] != "1" {
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
            return
        }
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
                drawCard(cr, slot, placed, zc)
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
                    if let placed = place(zc) { drawCard(bcr, slot, placed, zc) }
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
            if let placed = place(zc) { drawCard(cr, slot, placed, zc) }
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

    private static func drawCard(_ cr: OpaquePointer, _ slot: Int,
                                 _ placed: (Double, Double, Double, Double, Double),
                                 _ zc: Double) {
        guard let mainPtr = TunnelState.cardSurf[slot] else { return }
        var surf = mainPtr
        if let mipPtr = TunnelState.mipSurf[slot] {
            let mainW = Double(cairo_image_surface_get_width(mainPtr))
            if placed.2 < mainW * 0.70 { surf = mipPtr }
        }
        if ProcessInfo.processInfo.environment["GLASPEN2_SWIFT_TUNNEL_CARD"] != "1" {
            glaspen2_rust_draw_tunnel_card(
                UnsafeMutableRawPointer(cr), UnsafeMutableRawPointer(surf),
                cairo_image_surface_get_width(surf), cairo_image_surface_get_height(surf),
                placed.0, placed.1, placed.2, placed.3, placed.4, zc,
                Int32(TunnelState.surfW), Int32(TunnelState.surfH),
                TunnelState.thick, TunnelState.paneA)
            return
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
    setenv("GLASPEN2_SWIFT_TUNNEL_CARD", "1", 1)
    TunnelState.surface = b
    TunnelFrameRenderer.render(p: 0.37)
    unsetenv("GLASPEN2_SWIFT_TUNNEL_CARD")
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
