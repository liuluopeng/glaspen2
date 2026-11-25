// S7: 重复实例检测(启动时避免两个 glaspen2 争用 event tap / 数据库)。
import AppKit

@objc public final class AppInstanceGuard: NSObject {
    @objc public static func anotherInstanceRunning() -> Bool {
        let me = ProcessInfo.processInfo.processIdentifier
        for app in NSWorkspace.shared.runningApplications {
            if app.processIdentifier == me { continue }
            let name = app.localizedName ?? app.executableURL?.lastPathComponent
            if name == "glaspen2" { return true }
        }
        return false
    }
}
