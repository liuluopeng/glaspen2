// S7: cairo 缩放上下文 / 画布输入坐标 / 退出收尾, 以 C 符号导出。
import AppKit
import CoreGraphics
import Darwin

@_cdecl("cairo_create_scaled")
public func cairoCreateScaled() -> OpaquePointer? {
    guard let surface = SurfaceStore.current else { return nil }
    guard let cr = cairo_create(surface) else { return nil }
    let scale = SurfaceStore.scaleFactor
    cairo_scale(cr, scale, scale)
    if PageViewOps.isFit {
        cairo_translate(cr, PageViewOps.paperOx * scale,
                        PageViewOps.paperOy * scale)
        let ps = PageViewOps.paperScale
        cairo_scale(cr, ps, ps)
    }
    return cr
}

@_cdecl("save_and_exit")
public func saveAndExit(_ sig: Int32) {
    glaspen2_shutdown()
    PressureMonitorWindow.destroy()
    CGDisplayShowCursor(CGMainDisplayID())
    exit(0)
}
