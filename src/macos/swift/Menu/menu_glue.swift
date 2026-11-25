// S7: 菜单勾选同步 + 设置快照 overrides JSON(返回堆上 C 字符串, 调用方 free)。
import AppKit
import Darwin

@objc public final class MenuGlue: NSObject {
    @objc public static func updateCheckmarks() {
        glaspen2_router_update_checkmarks()
    }

    @objc public static func snapshotOverridesJSON() -> UnsafeMutablePointer<CChar>? {
        glaspen2_menu_snapshot_overrides_json()
    }
}
