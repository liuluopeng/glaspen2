// S7: 物理/面板快捷键分发(清空/翻页/导出/撤销/复制/玻璃/飘渺/设置)。
import AppKit

@objc public final class HotkeyDispatch: NSObject {
    private static var lastNoNextAt: TimeInterval = 0

    @objc(performHotkey:)
    public static func perform(keyCode: UInt16) -> Bool {
        switch keyCode {
        case 0x08: // C — clear
            EventRouter.finishActiveStroke()
            if glaspen2_router_infinite() != 0 {
                notify("canvas.infinite_kept")
                return true
            }
            glaspen2_router_clear_screen()
            return true
        case 0x09: // V
            DrawMode.toggleEnabled()
            return true
        case 0x32: // backtick — prev page
            if glaspen2_router_infinite() != 0 {
                notify("canvas.single_canvas_warning")
                return true
            }
            EventRouter.finishActiveStroke()
            let target = glaspen2_prev_screen_id()
            if target > 0 {
                glaspen2_router_page_flip(0, {
                    glaspen2_load_strokes_for_screen(target)
                    glaspen2_smooth_loaded_strokes()
                    PageViewOps.update()
                }, { glaspen2_router_replay_strokes() })
                EtherealMode.peek(seconds: 1.0)
                NotificationPresenter.showPageInfo(screenId: Int64(target))
            } else {
                notify("page.no_previous")
            }
            return true
        case 0x12: // 1 — next page
            if glaspen2_router_infinite() != 0 {
                notify("canvas.single_canvas_warning")
                return true
            }
            EventRouter.finishActiveStroke()
            let target = glaspen2_next_screen_id()
            if target > 0 {
                glaspen2_router_page_flip(1, {
                    glaspen2_load_strokes_for_screen(target)
                    glaspen2_smooth_loaded_strokes()
                    PageViewOps.update()
                }, { glaspen2_router_replay_strokes() })
                EtherealMode.peek(seconds: 1.0)
                NotificationPresenter.showPageInfo(screenId: Int64(target))
                return true
            }
            let now = ProcessInfo.processInfo.systemUptime
            let secondPress = (now - lastNoNextAt) < 5.0
            lastNoNextAt = now
            if !secondPress {
                notify("page.no_next")
                return true
            }
            let before = glaspen2_get_current_screen_id()
            let pageId = glaspen2_create_next_page(before)
            if pageId > 0 && pageId != before {
                glaspen2_router_page_flip_from(1, before, {
                    glaspen2_load_strokes_for_screen(pageId)
                    glaspen2_smooth_loaded_strokes()
                    PageViewOps.update()
                }, { glaspen2_router_replay_strokes() })
                EtherealMode.peek(seconds: 1.0)
                NotificationPresenter.showPageInfo(screenId: Int64(pageId))
                notify("page.created")
            } else if pageId == before {
                notify("page.already_latest")
            } else {
                notify("page.create_failed")
            }
            return true
        case 0x05: // G — export SVG/GIF
            glaspen2_save_svg()
            if let surface = SurfaceStore.current {
                cairo_surface_flush(surface)
                let w = Int(cairo_image_surface_get_width(surface))
                let h = Int(cairo_image_surface_get_height(surface))
                let stride = Int(cairo_image_surface_get_stride(surface))
                let data = cairo_image_surface_get_data(surface)
                let ok = glaspen2_save_gif_cropped(data, Int32(w), Int32(h),
                                                   Int32(stride),
                                                   SurfaceStore.scaleFactor)
                if ok != 0 {
                    copyNewestGifToPasteboard()
                    notify("export.svg_gif_saved")
                } else {
                    notify("export.failed")
                }
            }
            return true
        case 0x06: // Z — undo
            if glaspen2_router_stroke_active() != 0 {
                notify("draw.stroke_in_progress")
            } else {
                let remaining = glaspen2_undo_last_stroke()
                if remaining < 0 {
                    notify("draw.nothing_to_undo")
                } else {
                    glaspen2_router_rebuild_surface()
                    notify("draw.undone")
                }
            }
            return true
        case 0x01: // S — copy SVG
            if let svg = glaspen2_get_cropped_svg() {
                let svgStr = String(cString: svg)
                glaspen2_free_c_string(svg)
                let base64 = Data(svgStr.utf8).base64EncodedString()
                let html = "<img src=\"data:image/svg+xml;base64,\(base64)\" />"
                let pb = NSPasteboard.general
                pb.clearContents()
                pb.setString(html, forType: .string)
                notify("export.svg_copied")
            } else {
                notify("export.no_strokes_to_copy")
            }
            return true
        case 0x0B: // B — glass toggle
            MenuHandler.setGlassEnabled(enabled: glaspen2_router_glass_enabled() == 0)
            return true
        case 0x07: // X — canvas mode
            DrawMode.toggleCanvasMode()
            return true
        case 0x02: // D — settings
            SettingsPanel.show()
            return true
        default:
            return false
        }
    }

    private static func notify(_ key: String) {
        if let text = glaspen2_menu_l10n(key) {
            glaspen2_router_show_notification(text)
        }
    }

    private static func copyNewestGifToPasteboard() {
        guard let desktop = FileManager.default.urls(for: .desktopDirectory,
                                                     in: .userDomainMask).first else { return }
        let files = (try? FileManager.default.contentsOfDirectory(at: desktop,
                                                                  includingPropertiesForKeys: [.contentModificationDateKey])) ?? []
        var newest: URL?
        var newestDate: Date?
        for f in files where f.lastPathComponent.hasPrefix("glaspen2_")
            && f.pathExtension == "gif" {
            let d = (try? f.resourceValues(forKeys: [.contentModificationDateKey]))?.contentModificationDate
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
