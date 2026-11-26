// S7: 反色突出捕获(ScreenCaptureKit; 需 macOS 12.3+, 低版本自动禁用)。
import AppKit
import CoreMedia
import CoreVideo
import ScreenCaptureKit

@available(macOS 12.3, *)
@objc public final class InvertCapture: NSObject, SCStreamOutput {
    private static let slots = 2
    private static let idle = -1
    private static let grab = -2

    private static var bufs: [UnsafeMutableRawPointer?] = [nil, nil]
    private static var surfs: [OpaquePointer?] = [nil, nil]
    private static var invW = 0
    private static var invH = 0
    private static var writeSlot = 0
    private static var presentSlot = 0
    private static var sums: [UInt64] = [0, 0]
    private static var diffs: [UInt64] = [0, 0]
    private static var dboxes: [[Int32]] = [[0, 0, 0, 0], [0, 0, 0, 0]]
    private static var presentSum: UInt64 = 0
    private static var pending = idle
    private static let lock = NSLock()
    private static var filter: SCContentFilter?
    private static var stream: SCStream?
    private static var output: InvertCapture?
    private static var queue: DispatchQueue?

    private static func loadPending() -> Int {
        lock.lock(); defer { lock.unlock() }
        return pending
    }
    private static func storePending(_ v: Int) {
        lock.lock(); pending = v; lock.unlock()
    }
    private static func tryGrab() -> Bool {
        lock.lock(); defer { lock.unlock() }
        if pending == idle { pending = grab; return true }
        return false
    }

    @objc public static func apply(on: Bool) {
        if on {
            glaspen2_set_stroke_invert(1)
            schedule(0.05)
        } else {
            teardown()
            glaspen2_set_stroke_invert(0)
            glaspen2_router_set_window_sharing_readonly()
            PageViewOps.rebuildSurface()
            glaspen2_rust_overlay_flush()
        }
    }

    @objc public static func restart() {
        stop()
        schedule(0.05)
    }

    @objc public static func invalidate() {
        stop()
        filter = nil
        glaspen2_router_set_window_sharing_readonly()
        if invW > 0 { teardown() }
    }

    @objc(inkSourceWithCR:)
    public static func inkSource(crRaw: UnsafeMutableRawPointer) -> UnsafeMutableRawPointer? {
        guard glaspen2_router_invert_enabled() != 0,
              invW > 0, let surf = surfs[presentSlot] else { return nil }
        guard let pat = cairo_pattern_create_for_surface(surf) else { return nil }
        let scale = max(SurfaceStore.scaleFactor, 0.0001)
        var m = cairo_matrix_t()
        cairo_matrix_init_scale(&m, 1.0 / scale, 1.0 / scale)
        cairo_pattern_set_matrix(pat, &m)
        cairo_set_source(OpaquePointer(crRaw), pat)
        return UnsafeMutableRawPointer(pat)
    }

    // MARK: SCStreamOutput

    public func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer,
                       of type: SCStreamOutputType) {
        if type != .screen { return }
        if glaspen2_router_invert_enabled() == 0 { return }
        if !CMSampleBufferIsValid(sampleBuffer) { return }
        guard let pb = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }
        if !InvertCapture.tryGrab() { return }
        let slot = InvertCapture.writeSlot
        var ok = false
        CVPixelBufferLockBaseAddress(pb, .readOnly)
        let pf = CVPixelBufferGetPixelFormatType(pb)
        let cw = CVPixelBufferGetWidth(pb)
        let ch = CVPixelBufferGetHeight(pb)
        let cstride = CVPixelBufferGetBytesPerRow(pb)
        if pf == kCVPixelFormatType_32BGRA && Int(cw) == InvertCapture.invW
            && Int(ch) == InvertCapture.invH {
            if let sp = CVPixelBufferGetBaseAddress(pb),
               let dp = InvertCapture.bufs[slot] {
                let pv = InvertCapture.bufs[InvertCapture.presentSlot]
                let row = InvertCapture.invW * 4
                var sum: UInt64 = 0
                var diff: UInt64 = 0
                var bx0 = Int.max, by0 = Int.max, bx1 = -1, by1 = -1
                for y in 0..<InvertCapture.invH {
                    let srow = sp.advanced(by: y * cstride).assumingMemoryBound(to: UInt32.self)
                    let drow = dp.advanced(by: y * row).assumingMemoryBound(to: UInt32.self)
                    let prow = pv?.advanced(by: y * row).assumingMemoryBound(to: UInt32.self)
                    for x in 0..<InvertCapture.invW {
                        let v = (srow[x] ^ 0x00FF_FFFF) | 0xFF00_0000
                        drow[x] = v
                        sum &+= UInt64(v)
                        if let prow, ((v ^ prow[x]) & 0x00FF_FFFF) != 0 {
                            diff &+= 1
                            if x < bx0 { bx0 = x }
                            if x > bx1 { bx1 = x }
                            if y < by0 { by0 = y }
                            if y > by1 { by1 = y }
                        }
                    }
                }
                InvertCapture.sums[slot] = sum
                InvertCapture.diffs[slot] = diff
                if bx1 < 0 {
                    InvertCapture.dboxes[slot] = [0, 0, 0, 0]
                } else {
                    let x0 = max(bx0 - 2, 0)
                    let y0 = max(by0 - 2, 0)
                    let x1 = min(bx1 + 2, InvertCapture.invW - 1)
                    let y1 = min(by1 + 2, InvertCapture.invH - 1)
                    InvertCapture.dboxes[slot] = [Int32(x0), Int32(y0),
                                                  Int32(x1 - x0 + 1), Int32(y1 - y0 + 1)]
                }
                ok = true
            }
        }
        CVPixelBufferUnlockBaseAddress(pb, .readOnly)
        if ok {
            InvertCapture.storePending(slot)
            DispatchQueue.main.async { InvertCapture.consumePending() }
        } else {
            InvertCapture.storePending(InvertCapture.idle)
        }
    }

    private static func consumePending() {
        let slot = loadPending()
        if slot < 0 || slot >= slots || surfs[slot] == nil { return }
        lock.lock(); writeSlot = slot ^ 1; lock.unlock()
        let changed = sums[slot] != presentSum
        let significant = changed && diffs[slot] > diffMin()
        if changed {
            presentSlot = slot
            presentSum = sums[slot]
            storePending(idle)
            if let surf = surfs[slot] {
                glaspen2_set_invert_background(UnsafeMutableRawPointer(surf))
                cairo_surface_mark_dirty(surf)
            }
        } else {
            storePending(idle)
        }
        if significant && glaspen2_router_stroke_active() == 0 {
            let box = dboxes[slot]
            glaspen2_set_invert_dirty(box[0], box[1], box[2], box[3])
            PageViewOps.rebuildSurface()
            glaspen2_router_dirty_include_surface_rect(Double(box[0]), Double(box[1]),
                                                       Double(box[2]), Double(box[3]))
            glaspen2_rust_overlay_flush_dirty()
        } else {
            glaspen2_set_invert_dirty(-1, -1, -1, -1)
        }
    }

    private static func diffMin() -> UInt64 {
        UInt64(invW) * UInt64(invH) / 500 + 1
    }

    private static func stop() {
        guard let st = stream else { return }
        stream = nil
        st.stopCapture { error in
            if let error { NSLog("[invert] stream stop: %@", "\(error)") }
        }
        if let output {
            try? st.removeStreamOutput(output, type: .screen)
        }
    }

    private static func teardown() {
        stop()
        glaspen2_set_invert_background(nil)
        for i in 0..<slots {
            if let s = surfs[i] { cairo_surface_destroy(s) }
            surfs[i] = nil
            if let b = bufs[i] { free(b) }
            bufs[i] = nil
        }
        invW = 0
        invH = 0
    }

    private static func ensureCache(_ dw: Int, _ dh: Int) -> Bool {
        if bufs[0] != nil && invW == dw && invH == dh { return true }
        teardown()
        let bytes = dw * dh * 4
        for i in 0..<slots {
            guard let buf = malloc(bytes) else { teardown(); return false }
            memset(buf, 0, bytes)
            bufs[i] = buf
            guard let surf = cairo_image_surface_create_for_data(
                buf.assumingMemoryBound(to: UInt8.self), cairo_format_t(0),
                Int32(dw), Int32(dh), Int32(dw * 4)) else {
                teardown(); return false
            }
            surfs[i] = surf
        }
        invW = dw
        invH = dh
        writeSlot = 0
        presentSlot = 0
        storePending(idle)
        if let surf = surfs[presentSlot] {
            glaspen2_set_invert_background(UnsafeMutableRawPointer(surf))
        }
        glaspen2_set_stroke_invert(1)
        return true
    }

    private static func start() {
        if stream != nil || filter == nil || invW <= 0 { return }
        if output == nil { output = InvertCapture() }
        if queue == nil {
            queue = DispatchQueue(label: "glaspen.invert")
        }
        let config = SCStreamConfiguration()
        config.width = invW
        config.height = invH
        config.showsCursor = false
        config.pixelFormat = kCVPixelFormatType_32BGRA
        config.queueDepth = 3
        config.minimumFrameInterval = CMTime(value: 1, timescale: Int32(glaspen2_router_invert_fps()))
        guard let filter, let output, let queue else { return }
        let st = SCStream(filter: filter, configuration: config, delegate: nil)
        do {
            try st.addStreamOutput(output, type: .screen, sampleHandlerQueue: queue)
        } catch {
            NSLog("[invert] addStreamOutput failed: %@", "\(error)")
            stream = nil
            return
        }
        stream = st
        st.startCapture { error in
            if let error { NSLog("[invert] startCapture failed: %@", "\(error)") }
        }
    }

    private static func schedule(_ delay: Double) {
        DispatchQueue.main.asyncAfter(deadline: .now() + delay) {
            tick()
        }
    }

    private static func tick() {
        if glaspen2_router_invert_enabled() == 0 { return }
        guard let surface = SurfaceStore.current else {
            schedule(0.5)
            return
        }
        if TunnelState.active {
            schedule(0.5)
            return
        }
        if glaspen2_has_strokes() == 0 && glaspen2_router_stroke_active() == 0 {
            stop()
            schedule(0.5)
            return
        }
        cairo_surface_flush(surface)
        let dw = Int(cairo_image_surface_get_width(surface))
        let dh = Int(cairo_image_surface_get_height(surface))
        if dw <= 0 || dh <= 0 { return }
        if invW != dw || invH != dh {
            stop()
            filter = nil
        }
        if !ensureCache(dw, dh) { return }
        if filter == nil {
            SCShareableContent.getExcludingDesktopWindows(false,
                                                          onScreenWindowsOnly: true) { content, error in
                DispatchQueue.main.async {
                    if error != nil || content?.displays.isEmpty != false
                        || glaspen2_router_invert_enabled() == 0 {
                        schedule(1.0)
                        return
                    }
                    guard let content, let display = content.displays.first else {
                        schedule(1.0)
                        return
                    }
                    let mine = Bundle.main.bundleIdentifier
                    let excl = content.windows.filter {
                        $0.owningApplication?.bundleIdentifier == mine
                    }
                    filter = SCContentFilter(display: display, excludingWindows: excl)
                    glaspen2_router_set_window_sharing_none()
                    NSLog("[invert] 过滤器就绪(排除 %d 个自家窗口), 共享已关闭", excl.count)
                    tick()
                }
            }
            return
        }
        start()
    }
}
