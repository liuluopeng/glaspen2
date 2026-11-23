// glaspen2 macOS 原生层 —— S2 反向互调探针(Swift 侧)
//
// 由 build.rs 的 swiftc 步骤编进 glaspen2_macos_swift 模块;同时 swiftc 会
// emit 出 glaspen2_macos_swift-Swift.h, 让 ObjC(InteropProbe.m)能 import 并
// 调用这里 @objc 暴露的类/方法。验证路径(默认零行为):
//   glaspen2 --swift-probe            (Rust 隐藏调试入口, 不启动 GUI)
//   GLASPEN2_SWIFT_PROBE=1 glaspen2   (启动早期 env 门控)
//
// 边界约束(见 docs/swift-boundary.md): 跨语言只暴露 @objc + ObjC 兼容类型
// (这里是 NSString↔String), 不暴露 Swift struct/enum/generics/async/throws。

import Foundation

/// S2 双向互调探针。
/// ObjC → Swift: InteropProbe.m 调 `[SwiftInteropProbe ping]`;
/// Swift → ObjC: 本方法经 Bridging.h 调 `glaspen2_macos_objc_probe("swift-s2")`,
/// 两跳都写 NSLog, 用日志证明调用真的执行(而非仅链接通过)。
@objc public final class SwiftInteropProbe: NSObject {
    /// 生成的头里对应 `+ (NSString *)ping`;返回 ObjC 兼容类型 NSString。
    @objc public static func ping() -> String {
        let code = glaspen2_macos_objc_probe("swift-s2")
        NSLog("[swift-s2-probe] swift->objc OK: glaspen2_macos_objc_probe returned %d", code)
        return "swift-pong-\(code)"
    }
}
