// S7: 相机鼠标平移的 16ms 合帧调度(状态整体在 Swift)。
import AppKit

@objc public final class CameraMousePan: NSObject {
    private static var pendingDX = 0.0
    private static var pendingDY = 0.0
    private static var scheduled = false
    private static var generation = 0

    @objc(queueDX:dy:)
    public static func queue(dx: Double, dy: Double) {
        if ProcessInfo.processInfo.environment["GLASPEN2_SWIFT_CAMERA_PAN"] != "1" {
            if glaspen2_rust_camera_pan_queue(dx, dy) == 0 { return }
            generation += 1
            let gen = generation
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.016) {
                if gen == generation { glaspen2_rust_camera_pan_flush() }
            }
            return
        }
        pendingDX += dx
        pendingDY += dy
        if scheduled { return }
        scheduled = true
        generation += 1
        let gen = generation
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.016) {
            if gen == generation { flush() }
        }
    }

    @objc public static func flush() {
        if ProcessInfo.processInfo.environment["GLASPEN2_SWIFT_CAMERA_PAN"] != "1" {
            glaspen2_rust_camera_pan_flush()
            return
        }
        scheduled = false
        generation += 1
        let dx = pendingDX, dy = pendingDY
        pendingDX = 0
        pendingDY = 0
        let st = glaspen2_router_state()
        if (dx != 0 || dy != 0) && (st & 4) != 0 && (st & 1) != 0 && (st & 2) == 0 {
            CanvasCamera.panBy(dx: dx, dy: dy)
        }
    }

    @objc public static func cancel() {
        if ProcessInfo.processInfo.environment["GLASPEN2_SWIFT_CAMERA_PAN"] != "1" {
            glaspen2_rust_camera_pan_cancel()
            generation += 1
            scheduled = false
            return
        }
        pendingDX = 0
        pendingDY = 0
        scheduled = false
        generation += 1
    }
}

@_cdecl("glaspen2_router_camera_queue_pan")
public func cameraQueuePan(_ dx: Double, _ dy: Double) {
    CameraMousePan.queue(dx: dx, dy: dy)
}
@_cdecl("glaspen2_router_camera_flush")
public func cameraFlush() { CameraMousePan.flush() }
