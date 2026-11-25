// S7: 菜单快捷键读盘/生效表 JSON/主线程同步胶水。
import AppKit

@objc public final class SettingsGlue: NSObject {
    /// 从 DB 读快捷键覆盖并应用(无效 JSON 视为空)。
    @objc public static func loadShortcuts() {
        var saved: NSDictionary = [:]
        if let v = glaspen2_load_string_setting("shortcuts") {
            let raw = String(cString: v)
            glaspen2_free_c_string(v)
            if !raw.isEmpty, let data = raw.data(using: .utf8),
               let obj = try? JSONSerialization.jsonObject(with: data),
               let dict = obj as? NSDictionary {
                saved = dict
            }
        }
        SettingsGlue.applyShortcuts(saved: saved)
    }

    /// 当前生效表 → JSON 文本(失败回 "{}")。
    @objc public static func effectiveShortcutJSON() -> NSString {
        guard let p = glaspen2_shortcut_store_values_json() else { return "{}" as NSString }
        defer { glaspen2_free_c_string(p) }
        return String(cString: p) as NSString
    }

    /// 读一条字符串设置(DB, 未设置为 nil)。
    @objc(loadStringWithKey:)
    public static func loadString(key: String) -> String? {
        guard let v = key.withCString({ glaspen2_load_string_setting($0) }) else {
            return nil
        }
        let s = String(cString: v)
        glaspen2_free_c_string(v)
        return s
    }

    /// 快捷键生效表: NSDictionary -> JSON -> Rust。
    @objc public static func applyShortcuts(saved: NSDictionary) {
        var json = "{}"
        if let data = try? JSONSerialization.data(withJSONObject: saved),
           let text = String(data: data, encoding: .utf8) {
            json = text
        }
        _ = json.withCString { glaspen2_shortcut_apply_json($0) }
    }

    /// 非主线程同步投递到主线程执行(不改变调用方等待语义)。
    @objc public static func runOnMainSync(_ block: @escaping () -> Void) {
        if Thread.isMainThread {
            block()
        } else {
            DispatchQueue.main.sync(execute: block)
        }
    }
}
