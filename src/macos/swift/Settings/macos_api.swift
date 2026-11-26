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
