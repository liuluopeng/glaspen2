// R50: 生效表计算(默认/迁移/保留/冲突)已迁 Rust; 本文件只做 NSDictionary -> JSON 桥。
import AppKit

@objc public final class ShortcutApply: NSObject {
    @objc(applySaved:)
    public static func apply(saved: NSDictionary) {
        var json = "{}"
        if let data = try? JSONSerialization.data(withJSONObject: saved),
           let text = String(data: data, encoding: .utf8) {
            json = text
        }
        _ = json.withCString { glaspen2_shortcut_apply_json($0) }
    }
}
