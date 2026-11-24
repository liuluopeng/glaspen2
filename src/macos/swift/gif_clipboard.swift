// S7: GIF 编码结果写临时文件并放入剪贴板。
import AppKit

@objc public final class GifClipboard: NSObject {
    @objc(copyData:length:)
    public static func copy(data: UnsafePointer<UInt8>?, length: Int32) {
        guard let data, length > 0 else { return }
        let bytes = Data(bytes: data, count: Int(length))
        let name = String(format: "glaspen2_record_%.0f.gif",
                          Date.timeIntervalSinceReferenceDate * 1000)
        let path = (NSTemporaryDirectory() as NSString)
            .appendingPathComponent(name)
        do {
            try bytes.write(to: URL(fileURLWithPath: path), options: .atomic)
        } catch {
            NSLog("[glaspen2] failed to write GIF temp file: \(path)")
            return
        }
        let pb = NSPasteboard.general
        pb.clearContents()
        pb.writeObjects([URL(fileURLWithPath: path) as NSURL])
    }
}
