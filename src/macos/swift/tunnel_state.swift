// S7: 时光隧道状态(表面/卡片/参数/定时器/回调) — 全部由 Swift 持有。
import AppKit

@objc public final class TunnelState: NSObject {
    static var active = false
    static var surface: OpaquePointer?
    static var blur: OpaquePointer?
    static var surfW = 0
    static var surfH = 0
    static var cards = 0
    static var depths = [Int32](repeating: 0, count: 512)
    static var cardSurf = [OpaquePointer?](repeating: nil, count: 512)
    static var mipSurf = [OpaquePointer?](repeating: nil, count: 512)
    static var back = false
    static var travel = 1.0
    static var k = 0.45
    static var thick = 0.018
    static var paneA = 0.84
    static var dur = 0.38
    static var t0 = 0.0
    static var frames: UInt64 = 0
    static var timer: DispatchSourceTimer?
    static var prepare: (() -> Void)?
    static var commit: (() -> Void)?

    static let span = 2.5
    static let front = 4.5
    static let maxCards = 512

    static func now() -> Double {
        Date().timeIntervalSinceReferenceDate
    }
}
