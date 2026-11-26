// S7: 运行期初始化(tap 健康看门狗/调试开关)。
import AppKit
import ApplicationServices

@objc public final class StartupRunLoop: NSObject {
    @objc public static func begin() {
        Timer.scheduledTimer(withTimeInterval: 2.0, repeats: true) { _ in
            checkTap()
        }
        glaspen2_perf_init_from_env()
        glaspen2_rust_virtual_pen_maybe_start()
        FlipProbe.maybeStart()
    }

    private static func checkTap() {
        glaspen2_rust_check_tap()
    }

}
