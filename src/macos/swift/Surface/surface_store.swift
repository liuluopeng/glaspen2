// S7: 笔迹 cairo 表面生命周期(状态/创建在 Rust, Swift 只读指针)。
import AppKit

@objc public final class SurfaceStore: NSObject {
    static var current: OpaquePointer? {
        get { glaspen2_rust_surface_current().map { OpaquePointer($0) } }
        set { glaspen2_rust_surface_set(newValue.map(UnsafeMutableRawPointer.init)) }
    }
    static var scaleFactor: Double {
        get { glaspen2_rust_surface_scale() }
        set { glaspen2_rust_surface_set_scale(newValue) }
    }

    @objc(ensureWithView:)
    public static func ensure(view: NSView) {
        _ = glaspen2_rust_surface_ensure(Unmanaged.passUnretained(view).toOpaque())
    }
}
