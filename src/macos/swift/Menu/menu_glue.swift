// S7: 菜单勾选同步 + 设置快照 overrides JSON(返回堆上 C 字符串, 调用方 free)。
import AppKit
import Darwin

@objc public final class MenuGlue: NSObject {
    @objc public static func updateCheckmarks() {
        glaspen2_router_update_checkmarks()
    }

    @objc public static func snapshotOverridesJSON() -> UnsafeMutablePointer<CChar>? {
        let overrides: [String: Any] = [
            "launchAtLogin": glaspen2_is_launch_at_login() != 0,
            "shortcuts": MenuRuntime.shortcutsEffectiveJSON() ?? "{}",
            "flipEffect": glaspen2_router_flip_effect(),
            "language": glaspen2_router_lang() == 1 ? "en" : "zh",
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: overrides),
              let text = String(data: data, encoding: .utf8) else { return nil }
        return strdup(text)
    }
}
