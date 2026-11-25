// S7: 快捷键生效表状态(取代 menu.m 的 g_shortcut_values/lookup)。
import AppKit

@objc public final class ShortcutStore: NSObject {
    static var values: [String: String] = [:]
    static var lookup: [String: String] = [:]

    static func install(values: NSDictionary, lookup: NSDictionary) {
        var v: [String: String] = [:]
        var l: [String: String] = [:]
        for (k, value) in values {
            if let key = k as? String, let s = value as? String { v[key] = s }
        }
        for (k, value) in lookup {
            if let key = k as? String, let s = value as? String { l[key] = s }
        }
        ShortcutStore.values = v
        ShortcutStore.lookup = l
    }

    static func valuesSnapshot() -> NSDictionary { values as NSDictionary }
    static func chord(forAction action: String) -> String? { values[action] }
    static func action(forChord chord: String) -> String? { lookup[chord] }
}
