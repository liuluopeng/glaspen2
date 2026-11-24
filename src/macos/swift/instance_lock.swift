// S7: 单实例文件锁(临时目录 flock; GLASPEN2_ALLOW_MULTI 可显式旁路)。
import AppKit
import Darwin

@objc public final class InstanceLock: NSObject {
    @objc public static func acquire() -> Int32 {
        if ProcessInfo.processInfo.environment["GLASPEN2_ALLOW_MULTI"] != nil {
            return -2
        }
        let path = (NSTemporaryDirectory() as NSString)
            .appendingPathComponent("glaspen2.instance.lock")
        let fd = path.withCString { Darwin.open($0, O_CREAT | O_RDWR, 0o666) }
        if fd < 0 { return -1 }
        if flock(fd, LOCK_EX | LOCK_NB) == 0 { return fd }
        close(fd)
        return -1
    }
}
