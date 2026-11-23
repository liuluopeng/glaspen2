// glaspen2 macOS 原生层 —— S2 反向互调探针(ObjC 侧)
//
// 与 glaspen2.m 一样由 build.rs 用 clang 编进 libglaspen2_objc.a。单独成一个
// 小编译单元, 是为了能脱离主程序做无 GUI 自检(target/debug/glaspen2
// --swift-probe)。这里演示 S2 定下的两种跨界写法:
//   ObjC → Swift: #import swiftc 生成的 "glaspen2_macos_swift-Swift.h",
//                 调 @objc 类方法, 返回值是 ObjC 兼容的 NSString;
//   Swift → ObjC: 本文件实现 Bridging.h 声明的 C 函数, Swift 按 C 语义调。
// 默认路径零调用: 只有 --swift-probe 或 GLASPEN2_SWIFT_PROBE=1 时执行。

#import <Foundation/Foundation.h>
#include <stdlib.h>

#import "Bridging.h"
// build.rs 传 -I $OUT_DIR;头由 swiftc -emit-objc-header-path 生成。
#import "glaspen2_macos_swift-Swift.h"

// Swift → ObjC 目标: 无副作用(C 字符串参数 + int 返回, ObjC 兼容类型)。
int glaspen2_macos_objc_probe(const char *tag) {
    NSLog(@"[swift-s2-probe] objc<-swift OK: glaspen2_macos_objc_probe(%s)",
          tag ?: "(null)");
    return 0x5A2; // 固定常量 = S2;证明调用到达 C 函数体, 不是链接占位
}

// ObjC → Swift: 调用生成的 Swift 头里的类方法, 再打印 Swift 返回的 NSString。
void glaspen2_macos_swift_probe_selftest(void) {
    NSLog(@"[swift-s2-probe] objc->swift: calling [SwiftInteropProbe ping]");
    NSString *pong = [SwiftInteropProbe ping];
    NSLog(@"[swift-s2-probe] objc->swift OK: ping returned %@", pong);
}

// 启动路径的 env 门控入口: 不设 GLASPEN2_SWIFT_PROBE 时直接返回(正常启动
// 零行为变化);设了就在启动早期跑一次双向探针(只打日志)。
void glaspen2_macos_swift_probe_maybe(void) {
    if (!getenv("GLASPEN2_SWIFT_PROBE")) return;
    glaspen2_macos_swift_probe_selftest();
}
