// S7: chord 规范化包装(core/chord.rs 为唯一规则, 经 FFI)。
import AppKit

@objc public final class ChordNormalizer: NSObject {
    /// 旧存档 chord 迁移(core 迁移表); 非字符串原样返回。
    @objc(migrateSavedAction:raw:)
    public static func migrateSaved(action: String, raw: Any) -> Any {
        guard let s = raw as? String else { return raw }
        let migrated = action.withCString { a in
            s.withCString { glaspen2_chord_migrate_value(a, $0) }
        }
        guard let migrated else { return raw }
        defer { glaspen2_free_c_string(migrated) }
        return String(cString: migrated)
    }

    @objc(normalizeRaw:)
    public static func normalize(raw: String) -> String? {
        // 空串 = 未绑定(不能当成"合法空 chord", 否则默认值永远补不上)。
        if raw.trimmingCharacters(in: .whitespaces).isEmpty { return nil }
        guard let out = raw.withCString({ glaspen2_chord_normalize($0) }) else {
            return nil
        }
        defer { glaspen2_free_c_string(out) }
        return String(cString: out)
    }
}
