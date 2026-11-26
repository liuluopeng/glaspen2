// S7: Rust FRB 反向调用的设置面板入口(C ABI, 由 Swift 实现)。
import AppKit
import Darwin

@_cdecl("glaspen2_macos_settings_json")
public func settingsApiJSON() -> UnsafeMutablePointer<CChar>? {
    MacosApi.settingsJSON()
}

@_cdecl("glaspen2_macos_free_c_string")
public func settingsApiFreeCString(_ p: UnsafeMutablePointer<CChar>?) {
    free(p)
}

@_cdecl("glaspen2_macos_free_bytes")
public func settingsApiFreeBytes(_ p: UnsafeMutablePointer<UInt8>?) {
    free(p)
}

@_cdecl("glaspen2_macos_set_setting")
public func settingsApiSetSetting(_ key: UnsafePointer<CChar>?, _ value: UnsafePointer<CChar>?) {
    guard let key, let value else { return }
    _ = glaspen2_rust_apply_setting(key, value)
}

@_cdecl("glaspen2_macos_delete_page")
public func settingsApiDeletePage(_ screenId: Int64) -> Int32 {
    MacosApi.deletePage(screenId: screenId)
}

@_cdecl("glaspen2_macos_refresh_page")
public func settingsApiRefreshPage(_ screenId: Int64) {
    MacosApi.refreshPage(screenId: screenId)
}

@_cdecl("glaspen2_macos_refresh_infinite")
public func settingsApiRefreshInfinite() { MacosApi.refreshInfinite() }

@_cdecl("glaspen2_macos_navigate_to_page")
public func settingsApiNavigate(_ screenId: Int64) {
    MacosApi.navigateToPage(screenId: screenId)
}

@_cdecl("glaspen2_macos_hotkey")
public func settingsApiHotkey(_ key: UnsafePointer<CChar>?) {
    guard let key else { return }
    MacosApi.hotkey(key: String(cString: key))
}

@_cdecl("glaspen2_macos_export_pdf")
public func settingsApiExportPDF() -> Int32 { MacosApi.exportPDF() }

@_cdecl("glaspen2_macos_export_gif")
public func settingsApiExportGIF() -> Int32 { MacosApi.exportGIF() }

private func runOnMainSync(_ block: () -> Void) {
    if Thread.isMainThread {
        block()
    } else {
        DispatchQueue.main.sync(execute: block)
    }
}

@_cdecl("glaspen2_macos_canvas_payload")
public func settingsApiCanvasPayload(_ w: Int32, _ h: Int32, _ action: Int32,
                                     _ rectOut: UnsafeMutablePointer<Double>?,
                                     _ outLen: UnsafeMutablePointer<Int32>?) -> UnsafeMutablePointer<UInt8>? {
    var result: UnsafeMutablePointer<UInt8>?
    runOnMainSync {
        var ow = Int(w), oh = Int(h)
        if ow < 400 { ow = 1024 }
        if oh < 400 { oh = 768 }
        if action == 1 {
            CanvasCamera.home()
        } else if action == 2 {
            var bx = 0.0, by = 0.0, bx2 = 0.0, by2 = 0.0
            if glaspen2_stroke_bbox(&bx, &by, &bx2, &by2) != 0 {
                var px = 0.0, py = 0.0, pz = 0.0
                if glaspen2_camera_centered_values(bx, by, bx2, by2,
                                                   glaspen2_router_screen_w(),
                                                   glaspen2_router_screen_h(),
                                                   glaspen2_router_zoom(),
                                                   &px, &py, &pz) != 0 {
                    glaspen2_router_set_camera(px, py, pz, 0)
                    CanvasCamera.applyTransform()
                }
            }
        } else if action == 3 {
            EventRouter.finishActiveStroke()
            glaspen2_clear_strokes(Int32(glaspen2_router_screen_w()),
                                   Int32(glaspen2_router_screen_h()))
            CanvasCamera.home()
            ow = 1024
            oh = 768
        }
        guard let payload = SettingsGlue.canvasOverviewPayload(width: Double(ow), height: Double(oh)),
              let data = payload["png"] as? Data, !data.isEmpty else { return }
        let buf = UnsafeMutablePointer<UInt8>.allocate(capacity: data.count)
        data.copyBytes(to: buf, count: data.count)
        result = buf
        outLen?.pointee = Int32(data.count)
        if let rectOut {
            if let rect = payload["rect"] as? [NSNumber], rect.count == 4 {
                for i in 0..<4 { rectOut[i] = rect[i].doubleValue }
            }
            if let bounds = payload["bounds"] as? [NSNumber] {
                for i in 0..<min(4, bounds.count) { rectOut[4 + i] = bounds[i].doubleValue }
            }
        }
    }
    return result
}
