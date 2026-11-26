// S7: 时光隧道状态(标量状态在 Rust; 表面/卡片/定时器/回调由 Swift 持有)。
import AppKit

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
