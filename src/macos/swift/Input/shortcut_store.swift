// R8: 快捷键生效表状态已迁 glaspen-core::shortcut_state; 本文件只做 NSDictionary/JSON 桥。
import AppKit

@objc public final class ShortcutStore: NSObject {
    private static func json(_ dict: NSDictionary) -> String {
        guard let data = try? JSONSerialization.data(withJSONObject: dict),
              let text = String(data: data, encoding: .utf8) else { return "{}" }
        return text
    }

    private static func takeString(_ p: UnsafeMutablePointer<CChar>?) -> String? {
        guard let p else { return nil }
        defer { glaspen2_free_c_string(p) }
        return String(cString: p)
    }

    private static func takeJSON(_ p: UnsafeMutablePointer<CChar>?) -> NSDictionary {
        guard let p else { return [:] }
        let data = Data(bytes: p, count: strlen(p))
        glaspen2_free_c_string(p)
        guard let obj = try? JSONSerialization.jsonObject(with: data),
              let dict = obj as? NSDictionary else { return [:] }
        return dict
    }

    static func install(values: NSDictionary, lookup: NSDictionary) {
        let v = json(values), l = json(lookup)
        v.withCString { vp in l.withCString { lp in
            glaspen2_shortcut_store_install(vp, lp)
        } }
    }

    static func valuesSnapshot() -> NSDictionary {
        takeJSON(glaspen2_shortcut_store_values_json())
    }

    static func chord(forAction action: String) -> String? {
        takeString(action.withCString { glaspen2_shortcut_store_chord($0) })
    }

    static func action(forChord chord: String) -> String? {
        takeString(chord.withCString { glaspen2_shortcut_store_action_for_chord($0) })
    }

    static var isEmpty: Bool { glaspen2_shortcut_store_is_empty() != 0 }
}
