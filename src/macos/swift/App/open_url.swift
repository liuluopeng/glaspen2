// glaspen2 macOS 原生层 —— Swift 实现(S1 的第一个 C shim)
//
// 本文件由根 crate 的 build.rs 调 `xcrun swiftc -c` 编成 .o,与 glaspen2.m
// 的 ObjC 产物打进同一个 libglaspen2_objc.a,再链接进主程序。
// 对 Rust/C 的边界用 @_cdecl 导出与原 ObjC 实现完全相同的 C 符号,
// 所以 Rust 侧(src/api.rs 的 extern 声明)零改动。

import AppKit
import Foundation

/// 设置面板「检查更新 → 打开下载页」:用系统默认浏览器打开 URL。
/// http/https 白名单已在 Rust 侧(src/api.rs open_url_checked)挡过。
/// NSWorkspace 必须在主线程调用;非主线程时同步切到主线程
/// (与 ObjC 版 gl_run_on_main_sync 语义一致:主线程直接执行,
///  否则 dispatch_sync 到 main queue)。
@_cdecl("glaspen2_macos_open_url")
public func glaspen2_macos_open_url(_ urlC: UnsafePointer<CChar>?) {
    guard let urlC else { return }
    let s = String(cString: urlC)
    // 用 NSURL(string:) 保持与 ObjC 版 [NSURL URLWithString:] 的解析语义一致
    // (URL(string:) 在新 Foundation 上对个别非法字符更严格,不是等价替换)。
    guard let url = NSURL(string: s) as URL? else {
        NSLog("[glaspen2] open_url: URL 解析失败: %@", s)
        return
    }
    if Thread.isMainThread {
        openInWorkspace(url, raw: s)
    } else {
        DispatchQueue.main.sync { openInWorkspace(url, raw: s) }
    }
}

/// 必须主线程调用。打开失败只记日志:与旧实现同样是"尽力而为",
/// 不打断设置面板流程(旧实现忽略返回值,这里只多一条诊断日志)。
private func openInWorkspace(_ url: URL, raw: String) {
    if !NSWorkspace.shared.open(url) {
        NSLog("[glaspen2] open_url: NSWorkspace 打开失败: %@", raw)
    }
}
