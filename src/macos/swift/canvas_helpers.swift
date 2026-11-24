// S7: cairo 缩放上下文 / 画布输入坐标 / 退出收尾, 以 C 符号导出。
import AppKit
import CoreGraphics
import Darwin

@_cdecl("cairo_create_scaled")
public func cairoCreateScaled() -> OpaquePointer? {
    guard let surface = glaspen2_router_surface() else { return nil }
    guard let cr = cairo_create(surface) else { return nil }
    let scale = glaspen2_router_scale()
    cairo_scale(cr, scale, scale)
    if glaspen2_router_pageview_is_fit() != 0 {
        cairo_translate(cr, glaspen2_router_pageview_paper_ox() * scale,
                        glaspen2_router_pageview_paper_oy() * scale)
        let ps = glaspen2_router_pageview_paper_scale()
        cairo_scale(cr, ps, ps)
    }
    return cr
}

@_cdecl("canvas_input_x")
public func canvasInputX(_ viewX: Double) -> Double {
    if glaspen2_router_infinite() != 0 {
        return viewX / glaspen2_router_zoom() + glaspen2_router_pan_x()
    }
    if glaspen2_router_pageview_is_fit() != 0 {
        return (viewX - glaspen2_router_page_off_x()
            - glaspen2_router_pageview_paper_ox())
            / glaspen2_router_pageview_paper_scale()
    }
    return viewX - glaspen2_router_page_off_x()
}

@_cdecl("canvas_input_y")
public func canvasInputY(_ viewY: Double) -> Double {
    if glaspen2_router_infinite() != 0 {
        return viewY / glaspen2_router_zoom() + glaspen2_router_pan_y()
    }
    if glaspen2_router_pageview_is_fit() != 0 {
        return (viewY - glaspen2_router_page_off_y()
            - glaspen2_router_pageview_paper_oy())
            / glaspen2_router_pageview_paper_scale()
    }
    return viewY - glaspen2_router_page_off_y()
}

@_cdecl("save_and_exit")
public func saveAndExit(_ sig: Int32) {
    glaspen2_shutdown()
    PressureMonitorWindow.destroy()
    CGDisplayShowCursor(CGMainDisplayID())
    exit(0)
}
