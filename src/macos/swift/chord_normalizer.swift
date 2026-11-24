// S7: chord 规范化包装(core/chord.rs 为唯一规则, 经 FFI)。
import AppKit

@objc public final class ChordNormalizer: NSObject {
    @objc(normalizeRaw:)
    public static func normalize(raw: String) -> String? {
        guard let out = raw.withCString({ glaspen2_chord_normalize($0) }) else {
            return nil
        }
        defer { glaspen2_free_c_string(out) }
        return String(cString: out)
    }
}
