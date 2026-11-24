// S7: 镜头状态(平移/缩放/翻页偏移/相机抓手) — C 符号导出。
import AppKit

@objc public final class CameraStore: NSObject {
    static var panX = 0.0
    static var panY = 0.0
    static var zoom = 1.0
    static var pageOffX = 0.0
    static var pageOffY = 0.0
    static var mouseDragging = false
    static var mouseLast = NSPoint(x: 0, y: 0)
}

@_cdecl("glaspen2_router_set_camera")
public func cameraSet(_ panX: Double, _ panY: Double, _ zoom: Double, _ assignZoom: Int32) {
    CameraStore.panX = panX
    CameraStore.panY = panY
    if assignZoom != 0 { CameraStore.zoom = zoom }
}

@_cdecl("glaspen2_router_pan_x")
public func cameraPanX() -> Double { CameraStore.panX }

@_cdecl("glaspen2_router_pan_y")
public func cameraPanY() -> Double { CameraStore.panY }

@_cdecl("glaspen2_router_zoom")
public func cameraZoom() -> Double { CameraStore.zoom }

@_cdecl("glaspen2_router_reset_page_offsets")
public func cameraResetPageOffsets() {
    CameraStore.pageOffX = 0
    CameraStore.pageOffY = 0
}

@_cdecl("glaspen2_router_page_off_x")
public func cameraPageOffX() -> Double { CameraStore.pageOffX }

@_cdecl("glaspen2_router_page_off_y")
public func cameraPageOffY() -> Double { CameraStore.pageOffY }

@_cdecl("glaspen2_router_camera_dragging")
public func cameraDragging() -> Int32 { CameraStore.mouseDragging ? 1 : 0 }

@_cdecl("glaspen2_router_camera_step_zoom")
public func cameraStepZoom() -> Double { glaspen2_camera_step_zoom(CameraStore.zoom) }

@_cdecl("glaspen2_router_camera_queue_pan")
public func cameraQueuePan(_ dx: Double, _ dy: Double) {
    CameraMousePan.queue(dx: dx, dy: dy)
}

@_cdecl("glaspen2_router_camera_set_last")
public func cameraSetLast(_ x: Double, _ y: Double) {
    CameraStore.mouseLast = NSPoint(x: x, y: y)
}

@_cdecl("glaspen2_router_camera_last_x")
public func cameraLastX() -> Double { CameraStore.mouseLast.x }

@_cdecl("glaspen2_router_camera_last_y")
public func cameraLastY() -> Double { CameraStore.mouseLast.y }

@_cdecl("glaspen2_router_camera_flush")
public func cameraFlush() { CameraMousePan.flush() }

@_cdecl("glaspen2_router_camera_clear_dragging")
public func cameraClearDragging() { CameraStore.mouseDragging = false }

@_cdecl("glaspen2_router_camera_start")
public func cameraStart(_ x: Double, _ y: Double) {
    CameraStore.mouseDragging = true
    CameraStore.mouseLast = NSPoint(x: x, y: y)
}

@_cdecl("glaspen2_router_clear_camera_dragging")
public func cameraClearDraggingAlias() { CameraStore.mouseDragging = false }

@_cdecl("glaspen2_router_camera_pan_mods")
public func cameraPanMods() -> UInt { ShortcutMask.maskFor(action: "canvas_pan_up") }
