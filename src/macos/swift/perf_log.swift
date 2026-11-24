// S7: 性能日志(Glaspen2_PERF_LOG=1 → ~/Library/Logs/glaspen2/perf.log)。
import AppKit
import Darwin

@objc public final class PerfLog: NSObject {
    private static var file: FileHandle?
    private static var enabled = false
    private static var totalCalls: UInt64 = 0
    private static var slowCalls: UInt64 = 0
    private static var timebase: mach_timebase_info_data_t = mach_timebase_info_data_t()
    private static var timebaseInited = false

    private static func ensureTimebase() {
        if timebaseInited { return }
        mach_timebase_info(&timebase)
        timebaseInited = true
    }

    @objc public static func elapsedUs(since start: UInt64) -> UInt64 {
        ensureTimebase()
        let delta = mach_absolute_time() - start
        return delta * UInt64(timebase.numer) / UInt64(timebase.denom) / 1000
    }

    @objc public static func begin() {
        if file != nil { return }
        let dir = NSSearchPathForDirectoriesInDomains(.libraryDirectory,
                                                     .userDomainMask, true).first
            ?? NSHomeDirectory() + "/Library"
        let logDir = dir + "/Logs/glaspen2"
        do {
            try FileManager.default.createDirectory(atPath: logDir,
                                                    withIntermediateDirectories: true)
        } catch {
            NSLog("[glaspen2] perf log dir error: \(error)")
        }
        let path = logDir + "/perf.log"
        FileManager.default.createFile(atPath: path, contents: nil)
        guard let fh = FileHandle(forWritingAtPath: path) else {
            NSLog("[glaspen2] perf log open failed: \(path)")
            return
        }
        file = fh
        NSLog("[glaspen2] performance log: \(path)")
        write("ts_ms\ttype\tdur_us\tnotes\n")
    }

    @objc public static func initFromEnv() {
        guard let v = getenv("GLASPEN2_PERF_LOG"), v.pointee != 0,
              String(cString: v) != "0" else { return }
        enabled = true
        if file == nil { begin() }
    }

    @objc public static func isEnabled() -> Bool { enabled }

    @objc(recordName:durUs:notes:)
    public static func record(name: String, durUs: UInt64, notes: String?) {
        guard enabled, file != nil else { return }
        totalCalls += 1
        if durUs > 16000 { slowCalls += 1 }
        ensureTimebase()
        let tsMs = Double(mach_absolute_time()) * Double(timebase.numer)
            / Double(timebase.denom) / 1e6
        let noteText = notes ?? (durUs > 16000 ? "SLOW" : "")
        write(String(format: "%.3f\t%@\t%llu\t%@\n", tsMs, name, durUs, noteText))
        if totalCalls % 100 == 0 { file?.synchronizeFile() }
    }

    private static func write(_ text: String) {
        guard let data = text.data(using: .utf8) else { return }
        file?.write(data)
    }

    @objc public static func summary() {
        guard file != nil else { return }
        let pct = totalCalls > 0 ? 100.0 * Double(slowCalls) / Double(totalCalls) : 0
        write(String(format: "\n# SUMMARY: total=%llu slow=%llu (%.1f%%)\n",
                     totalCalls, slowCalls, pct))
        file?.closeFile()
        file = nil
    }
}
