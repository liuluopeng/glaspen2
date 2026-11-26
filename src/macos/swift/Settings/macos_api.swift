// S7: Rust → macOS 的 C ABI 主体(设置快照/页刷新与导航/删除页/热键/导出)。
import AppKit
import Darwin

@objc public final class MacosApi: NSObject {
    @objc public static func settingsJSON() -> UnsafeMutablePointer<CChar>? {
        var out: UnsafeMutablePointer<CChar>?
        settingsRunOnMain {
            guard let overrides = glaspen2_menu_snapshot_overrides_json() else { return }
            defer { free(overrides) }
            guard let json = glaspen2_settings_snapshot_json(overrides) else { return }
            let text = String(cString: json)
            glaspen2_free_c_string(json)
            out = strdup(text)
        }
        return out
    }

    @objc(refreshPageWithScreenId:)
    public static func refreshPage(screenId: Int64) {
        if screenId <= 0 { return }
        settingsRunOnMain {
            if glaspen2_router_infinite() != 0 { return }
            if screenId != Int64(glaspen2_get_current_screen_id()) { return }
            glaspen2_load_strokes_for_screen(screenId)
            glaspen2_rust_pageview_rebuild()
            glaspen2_rust_overlay_flush()
        }
    }

    @objc public static func refreshInfinite() {
        settingsRunOnMain {
            if glaspen2_router_infinite() == 0 { return }
            glaspen2_load_infinite_strokes()
            glaspen2_rust_pageview_rebuild()
            glaspen2_rust_overlay_flush()
        }
    }

    @objc(navigateToPageWithScreenId:)
    public static func navigateToPage(screenId: Int64) {
        if screenId <= 0 { return }
        if glaspen2_router_infinite() != 0 {
            settingsRunOnMain {
                _ = "".withCString { glaspen2_rust_apply_infinite(0, $0) }
            }
        }
        DispatchQueue.global(qos: .userInitiated).sync {
            glaspen2_load_strokes_for_screen(screenId)
            glaspen2_rust_pageview_update()
            settingsRunOnMain {
                glaspen2_rust_pageview_rebuild()
            }
        }
    }

    @objc(deletePageWithScreenId:)
    public static func deletePage(screenId: Int64) -> Int32 {
        var ok: Int32 = 0
        let wasCurrent = screenId == Int64(glaspen2_get_current_screen_id())
        let nextId: Int64 = wasCurrent ? Int64(glaspen2_next_screen_id()) : 0
        let prevId: Int64 = wasCurrent ? Int64(glaspen2_prev_screen_id()) : 0
        DispatchQueue.global(qos: .userInitiated).sync {
            ok = glaspen2_delete_screen(screenId)
            settingsRunOnMain {
                if glaspen2_router_infinite() != 0 {
                    glaspen2_rust_pageview_rebuild()
                    return
                }
                if ok != 0 && wasCurrent {
                    let target = nextId != 0 ? nextId : prevId
                    if target != 0 {
                        glaspen2_load_strokes_for_screen(target)
                        glaspen2_rust_pageview_update()
                        glaspen2_rust_pageview_rebuild()
                    } else {
                        glaspen2_rust_surface_clear_screen()
                    }
                }
            }
        }
        return ok
    }

    @objc(hotkeyWithKey:)
    public static func hotkey(key: String) {
        settingsRunOnMain {
            if key == "Q" {
                glaspen2_rust_menu_quit_app()
                return
            }
            let keyCode: UInt16
            switch key {
            case "G": keyCode = 0x05
            case "\u{0060}": keyCode = 0x32
            case "1": keyCode = 0x12
            case "Z": keyCode = 0x06
            case "X": keyCode = 0x07
            case "C": keyCode = 0x08
            case "V": keyCode = 0x09
            case "B": keyCode = 0x0B
            default: return
            }
            _ = HotkeyDispatch.perform(keyCode: keyCode)
        }
    }

    @objc public static func exportPDF() -> Int32 {
        var ok: Int32 = 0
        DispatchQueue.global(qos: .userInitiated).sync {
            ok = glaspen2_export_pdf()
        }
        return ok
    }

    @objc public static func exportGIF() -> Int32 {
        var ok: Int32 = 0
        DispatchQueue.global(qos: .userInitiated).sync {
            ok = glaspen2_save_animated_gif(glaspen2_router_gif_fps(),
                                            glaspen2_router_gif_resolution(),
                                            glaspen2_router_gif_speed(),
                                            glaspen2_router_gif_end_mode())
            settingsRunOnMain {
                if ok != 0 {
                    copyNewestGifToPasteboard()
                    notificationShowText(text:
                        MenuRuntime.pair("动画 GIF 已保存并复制到剪贴板",
                                                "Animated GIF saved & copied"))
                } else if let text = Optional(MenuRuntime.l10n("recording.gif_no_strokes_or_export_failed")) {
                    notificationShowText(text:text)
                }
            }
        }
        return ok
    }

    private static func copyNewestGifToPasteboard() {
        guard let desktop = FileManager.default.urls(for: .desktopDirectory,
                                                     in: .userDomainMask).first else { return }
        let files = (try? FileManager.default.contentsOfDirectory(
            at: desktop, includingPropertiesForKeys: [.contentModificationDateKey])) ?? []
        var newest: URL?
        var newestDate: Date?
        for f in files where f.lastPathComponent.hasPrefix("glaspen2_")
            && f.pathExtension == "gif" {
            let d = (try? f.resourceValues(forKeys: [.contentModificationDateKey]))?
                .contentModificationDate
            if newestDate == nil || (d != nil && d! > newestDate!) {
                newestDate = d
                newest = f
            }
        }
        guard let newest else { return }
        let pb = NSPasteboard.general
        pb.clearContents()
        pb.writeObjects([newest as NSURL])
    }
}

func settingsRunOnMain(_ block: @escaping () -> Void) {
    if Thread.isMainThread {
        block()
    } else {
        DispatchQueue.main.sync(execute: block)
    }
}


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
            glaspen2_rust_camera_home()
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
                    glaspen2_rust_camera_apply_transform()
                }
            }
        } else if action == 3 {
            EventRouter.finishActiveStroke()
            glaspen2_clear_strokes(Int32(glaspen2_router_screen_w()),
                                   Int32(glaspen2_router_screen_h()))
            glaspen2_rust_camera_home()
            ow = 1024
            oh = 768
        }
        var rect = [Double](repeating: 0, count: 4)
        var bounds = [Double](repeating: 0, count: 4)
        var pngLen: Int32 = 0
        let png = rect.withUnsafeMutableBufferPointer { r in
            bounds.withUnsafeMutableBufferPointer { b in
                glaspen2_rust_canvas_overview(Double(ow), Double(oh), r.baseAddress, b.baseAddress, &pngLen)
            }
        }
        guard let png, pngLen > 0 else { return }
        let data = Data(bytes: png, count: Int(pngLen))
        glaspen2_free_rust_bytes(png, pngLen)
        let payload: [String: Any] = ["png": data, "rect": rect, "bounds": bounds]
        guard !data.isEmpty else { return }
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
