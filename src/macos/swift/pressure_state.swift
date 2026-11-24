// S7: 压感监视线程状态(避开 ObjC 全局) — 状态与刷新均在 Swift。
import AppKit

@objc public final class PressureState: NSObject {
    static var pressure: Int32 = 0
    static var evType: String?
    static var tipDown = false
    static var inRange = false
    static var x = 0.0
    static var y = 0.0

    static func refresh() {
        MenuHandler.updatePressureLabel(pressure: pressure, type: evType, x: x, y: y)
    }
}

@_cdecl("sw_pm_update")
public func pressureUpdate(_ inRange: Int32, _ pressure: Int32, _ x: Double, _ y: Double,
                           _ tipDown: Int32, _ evType: UnsafePointer<CChar>?) {
    PressureState.inRange = inRange != 0
    PressureState.pressure = pressure
    PressureState.x = x
    PressureState.y = y
    PressureState.tipDown = tipDown != 0
    PressureState.evType = evType.map { String(cString: $0) }
    PressureState.refresh()
}

@_cdecl("sw_pm_refresh")
public func pressureRefresh() { PressureState.refresh() }
