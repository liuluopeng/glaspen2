// S7: 设置面板透明化与压感标签刷新薄逻辑。
import AppKit

@objc public final class SettingsPanelExtras: NSObject {
    /// 设置窗口透明开关(真穿透只改窗口属性; Flutter layer 由调用方处理)。
    @objc(applyPanelTransparent:)
    public static func applyPanelTransparent(on: Bool) {
        guard let window = glaspen2_router_settings_window() as? NSWindow else {
            return
        }
        SettingsGlue.runOnMainSync {
            window.isOpaque = !on
            window.backgroundColor = on ? .clear : .windowBackgroundColor
        }
    }

    /// 用 ObjC 侧最新压力状态刷新标签。
    @objc public static func refreshPressureLabel() {
        let state = glaspen2_router_pressure_state() as NSDictionary
        let pressure = (state["pressure"] as? NSNumber)?.int32Value ?? 0
        let type = state["type"] as? String
        let x = (state["x"] as? NSNumber)?.doubleValue ?? 0
        let y = (state["y"] as? NSNumber)?.doubleValue ?? 0
        MenuHandler.updatePressureLabel(pressure: pressure, type: type, x: x, y: y)
    }
}
