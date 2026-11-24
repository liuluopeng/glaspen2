// S7: 设置面板透明化与压感标签刷新薄逻辑。
import AppKit

@objc public final class SettingsPanelExtras: NSObject {
    /// 设置窗口透明开关(真穿透只改窗口属性; Flutter layer 由调用方处理)。
    @objc(applyPanelTransparent:)
    public static func applyPanelTransparent(on: Bool) {
        guard let window = UIAnchors.settingsWindow else {
            return
        }
        SettingsGlue.runOnMainSync {
            window.isOpaque = !on
            window.backgroundColor = on ? .clear : .windowBackgroundColor
        }
    }

    /// 用 ObjC 侧最新压力状态刷新标签。
    @objc public static func refreshPressureLabel() {
        PressureState.refresh()
    }
}
