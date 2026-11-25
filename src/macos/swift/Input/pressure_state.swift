// R49: 压感监视状态已迁 Rust; 本文件只做 C 桥与 UI 刷新。
import AppKit

@objc public final class PressureState: NSObject {
    static var pressure: Int32 { glaspen2_rust_pm_pressure() }
    static var evType: String? {
        guard let p = glaspen2_rust_pm_ev_type() else { return nil }
        return String(cString: p)
    }
    static var tipDown: Bool { glaspen2_rust_pm_tip_down() != 0 }
    static var inRange: Bool { glaspen2_rust_pm_in_range() != 0 }
    static var x: Double { glaspen2_rust_pm_x() }
    static var y: Double { glaspen2_rust_pm_y() }

    static func refresh() {
        MenuHandler.updatePressureLabel(pressure: pressure, type: evType, x: x, y: y)
    }
}

@_cdecl("sw_pm_update")
public func pressureUpdate(_ inRange: Int32, _ pressure: Int32, _ x: Double, _ y: Double,
                           _ tipDown: Int32, _ evType: UnsafePointer<CChar>?) {
    glaspen2_rust_pm_update(inRange, pressure, x, y, tipDown, evType)
    PressureState.refresh()
}

@_cdecl("sw_pm_refresh")
public func pressureRefresh() { PressureState.refresh() }
