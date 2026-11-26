// S7: 相机鼠标平移的 16ms 合帧调度(状态在 Rust, Swift 只保留定时器壳)。
import AppKit

@objc public final class CameraMousePan: NSObject {
    private static var generation = 0

    @objc(queueDX:dy:)
    public static func queue(dx: Double, dy: Double) {
        if glaspen2_rust_camera_pan_queue(dx, dy) == 0 { return }
        generation += 1
        let gen = generation
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.016) {
            if gen == generation { glaspen2_rust_camera_pan_flush() }
        }
    }

    @objc public static func flush() {
        glaspen2_rust_camera_pan_flush()
    }

    @objc public static func cancel() {
        glaspen2_rust_camera_pan_cancel()
        generation += 1
    }
}

@_cdecl("glaspen2_router_camera_queue_pan")
public func cameraQueuePan(_ dx: Double, _ dy: Double) {
    CameraMousePan.queue(dx: dx, dy: dy)
}
@_cdecl("glaspen2_router_camera_flush")
public func cameraFlush() { CameraMousePan.flush() }
