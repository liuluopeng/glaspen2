// S7: 快捷键默认表/动作表(core::ui 经 FFI 下发, 进程内缓存)。
import AppKit

@objc public final class ShortcutTable: NSObject {
    private static var cachedDefaults: NSDictionary?
    private static var cachedActions: NSArray?

    @objc public static func defaults() -> NSDictionary {
        if let cachedDefaults { return cachedDefaults }
        var result: NSDictionary = [:]
        if let json = glaspen2_chord_defaults_json() {
            let data = Data(bytes: json, count: strlen(json))
            glaspen2_free_c_string(json)
            if let obj = try? JSONSerialization.jsonObject(with: data),
               let dict = obj as? NSDictionary {
                result = dict
            }
        }
        cachedDefaults = result
        return result
    }

    @objc public static func actions() -> NSArray {
        if let cachedActions { return cachedActions }
        var result: NSArray = []
        if let json = glaspen2_chord_actions_json() {
            let data = Data(bytes: json, count: strlen(json))
            glaspen2_free_c_string(json)
            if let obj = try? JSONSerialization.jsonObject(with: data),
               let arr = obj as? NSArray {
                result = arr
            }
        }
        cachedActions = result
        return result
    }
}
