// S8 诊断: 模型器一笔 → 等待异步 flush → 退出; 外部检查 DB points。
import AppKit
import Darwin

@objc public final class PersistProbe: NSObject {
    @objc public static func run() -> Int32 {
        glaspen2_init_db(1920, 1080)
        glaspen2_modeler_clear_buffer()
        let t0 = Date().timeIntervalSinceReferenceDate
        EventRouter.penDown(x: 200.0, y: 300.0, pressure: 0.5,
                            timestamp: t0, rawWidth: 1.0)
        for i in 1...40 {
            EventRouter.penMove(x: 200.0 + Double(i) * 4.0, y: 300.0, pressure: 0.5,
                                timestamp: t0 + Double(i) * 0.01, rawWidth: 1.0)
        }
        EventRouter.penUp(x: 360.0, y: 300.0, pressure: 0.5, timestamp: t0 + 0.45)
        // proximity 设备枚举: pen=1 不是橡皮, eraser=3 才是。
        glaspen2_rust_pen_set_prox_device(1)
        let penAsEraser = glaspen2_router_prox_is_eraser()
        glaspen2_rust_pen_set_prox_device(3)
        let eraserAsEraser = glaspen2_router_prox_is_eraser()
        glaspen2_rust_pen_set_prox_device(0)
        print("PROX_PROBE pen1_eraser=\(penAsEraser) eraser3_eraser=\(eraserAsEraser)")
        // 等后台 flush_pending 完成。
        Thread.sleep(forTimeInterval: 1.0)
        print("PERSIST_PROBE_DONE strokes=\(glaspen2_stroke_count())")
        return glaspen2_stroke_count() == 1 ? 0 : 1
    }
}

@_cdecl("glaspen2_swift_persist_probe_selftest")
public func persistProbeSelftest() -> Int32 { PersistProbe.run() }
