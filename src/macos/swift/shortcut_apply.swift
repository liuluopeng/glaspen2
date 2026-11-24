// S7: 快捷键生效表(迁移旧值/补默认/挡保留组合/冲突解绑)。
import AppKit

@objc public final class ShortcutApply: NSObject {
    @objc(applySaved:)
    public static func apply(saved: NSDictionary) {
        let defaults = ShortcutTable.defaults() as NSDictionary
        var values: [String: String] = [:]
        var lookup: [String: String] = [:]
        for case let action as String in ShortcutTable.actions() {
            let raw: Any = saved[action] ?? ""
            let migrated = ChordNormalizer.migrateSaved(action: action, raw: raw)
            var chord: String?
            if let s = migrated as? String {
                chord = ChordNormalizer.normalize(raw: s)
                if chord == nil && !s.isEmpty {
                    NSLog("[hotkey] 非法组合 \(action)=\(s), 回退默认")
                }
            }
            if chord == nil { chord = defaults[action] as? String }
            if let c = chord, !c.isEmpty,
               glaspen2_router_shortcut_reserved(c) != 0 {
                NSLog("[hotkey] 保留组合 \(action)=\(c), 回退默认")
                chord = defaults[action] as? String
            }
            if let c = chord, !c.isEmpty, let existing = lookup[c] {
                NSLog("[hotkey] 冲突 \(existing) 与 \(action) 都是 \(c); 后者解绑")
                chord = ""
            }
            values[action] = chord ?? ""
            if let c = chord, !c.isEmpty { lookup[c] = action }
        }
        glaspen2_router_set_shortcut_tables(values as [AnyHashable: Any],
                                            lookup as [AnyHashable: Any])
    }
}
