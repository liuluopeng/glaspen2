// R47: 光标/画笔光标数值状态已迁 Rust; 本文件只保留 NSCursor 实例。
import AppKit

@objc public final class CursorState: NSObject {
    static var blankCursor: NSCursor?
    static var arrowCursor: NSCursor?
}
