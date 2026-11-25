// S7: 面板设置应用分派(core settings_apply 下发 effect → 原生运行时动作)。
import AppKit

@objc public final class SettingsDispatcher: NSObject {
    @objc(applyWithKey:value:)
    public static func apply(key: String, value: String) {
        guard let effectRaw = key.withCString({ k in
            value.withCString { v in glaspen2_settings_apply(k, v) }
        }) else { return }
        let text = String(cString: effectRaw)
        glaspen2_free_c_string(effectRaw)
        guard let data = text.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data),
              let effect = obj as? [String: Any] else { return }
        guard let kind = effect["kind"] as? String else {
            NSLog("[settings] %@ 应用失败: %@", key, "\(effect["error"] ?? "")")
            return
        }
        func flag(_ k: String) -> Bool { (effect[k] as? NSNumber)?.boolValue ?? false }
        func int(_ k: String) -> Int32 { (effect[k] as? NSNumber)?.int32Value ?? 0 }
        func dbl(_ k: String) -> Double { (effect[k] as? NSNumber)?.doubleValue ?? 0 }
        let b = flag("bool")
        let iv = int("int")
        let fv = dbl("float")
        let sv = effect["str"] as? String ?? ""
        let json = effect["json"]
        SettingsGlue.runOnMainSync {
            apply(kind: kind, key: key, boolValue: b, intValue: iv,
                  floatValue: fv, strValue: sv, jsonValue: json)
        }
    }

    private static func apply(kind: String, key: String, boolValue: Bool,
                              intValue: Int32, floatValue: Double,
                              strValue: String, jsonValue: Any?) {
        switch kind {
        case "BubbleAutoNewPage", "BubbleReplayAuto", "BubbleReplayHoldLast",
             "BubbleReplaySpeed", "NasUrl", "NotebookStyle", "ShowFreeCanvas":
            return
        case "Color": MenuHandler.setColor(index: intValue)
        case "Width": MenuHandler.setWidth(index: intValue)
        case "LaunchAtLogin": MenuHandler.setLaunchAtLogin(enabled: boolValue)
        case "FrostedGlass": MenuHandler.setGlassEnabled(enabled: boolValue)
        case "Grid": MenuHandler.setGridEnabled(enabled: boolValue)
        case "GridFollowStrokes":
            glaspen2_router_set_grid_follow(boolValue ? 1 : 0)
            glaspen2_router_redraw_draw_view()
        case "GlassFollowStrokes":
            glaspen2_router_set_glass_follow(boolValue ? 1 : 0)
            MenuHandler.applyGlass()
        case "SoftShadow":
            glaspen2_router_set_soft_shadow_flag(boolValue ? 1 : 0)
            glaspen2_set_soft_shadow(boolValue ? 1 : 0)
            glaspen2_router_redraw_draw_view()
        case "InvertInk":
            glaspen2_router_invert_set_ink(boolValue ? 1 : 0)
            glaspen2_router_invert_apply(boolValue ? 1 : 0)
        case "InvertFps":
            glaspen2_router_invert_set_fps(intValue)
            if glaspen2_router_invert_enabled() != 0 {
                glaspen2_router_invert_restart()
            }
        case "PanelTransparent":
            glaspen2_router_panel_transparent_full(boolValue ? 1 : 0)
        case "Outline": MenuHandler.setOutlineEnabled(enabled: boolValue)
        case "InfiniteCanvas":
            // 面板/菜单设置路径旧实现 no-notify; HUD 只在热键/菜单动作里弹。
            CanvasMode.applyInfinite(on: boolValue, notify: false)
        case "Minimap":
            glaspen2_router_set_minimap(boolValue ? 1 : 0)
            glaspen2_router_redraw_draw_view()
        case "GridSize":
            glaspen2_router_set_grid_size(floatValue)
            glaspen2_router_redraw_draw_view()
        case "GridDivider":
            glaspen2_router_set_grid_divider(intValue)
            glaspen2_router_redraw_draw_view()
        case "FlipEffect": glaspen2_router_set_flip(intValue)
        case "PressureMonitor": MenuHandler.setPressureMonitor(enabled: boolValue)
        case "GifFps":
            glaspen2_router_set_gif(intValue, glaspen2_router_gif_resolution(),
                                    glaspen2_router_gif_speed(),
                                    glaspen2_router_gif_end_mode())
        case "GifResolution":
            glaspen2_router_set_gif(glaspen2_router_gif_fps(), floatValue,
                                    glaspen2_router_gif_speed(),
                                    glaspen2_router_gif_end_mode())
        case "GifSpeed":
            glaspen2_router_set_gif(glaspen2_router_gif_fps(),
                                    glaspen2_router_gif_resolution(), floatValue,
                                    glaspen2_router_gif_end_mode())
        case "GifEndMode":
            glaspen2_router_set_gif(glaspen2_router_gif_fps(),
                                    glaspen2_router_gif_resolution(),
                                    glaspen2_router_gif_speed(), intValue)
        case "ChatApiBase", "ChatUser": glaspen2_chat_auth_reload()
        case "ChatPassword":
            if !strValue.isEmpty {
                "chat_password".withCString { k in
                    strValue.withCString { v in glaspen2_save_string_setting(k, v) }
                }
                glaspen2_chat_auth_reload()
            }
        case "ChatIntegration": MenuHandler.setChatIntegration(enabled: boolValue)
        case "ShowBubbleTab":
            glaspen2_bubble_set_capture(boolValue ? 1 : 0)
        case "ShareCanvas":
            let chat = glaspen2_load_bool_setting("chat_integration") != 0
            if chat || !boolValue {
                glaspen2_share_ink_set_active((chat && boolValue) ? 1 : 0)
            }
        case "Language":
            glaspen2_router_startup_set_lang(strValue == "en" ? 1 : 0)
            MenuTextUpdater.update()
            glaspen2_notify_settings_changed()
        case "Shortcuts":
            let map = jsonValue as? NSDictionary ?? [:]
            SettingsGlue.applyShortcuts(saved: map)
            let json = SettingsGlue.effectiveShortcutJSON() as String
            if !json.isEmpty {
                "shortcuts".withCString { k in
                    json.utf8CString.withUnsafeBufferPointer { buf in
                        glaspen2_save_string_setting(k, buf.baseAddress)
                    }
                }
            }
            glaspen2_notify_settings_changed()
        case "HotkeyRecording": HotkeyRecorder.setRecording(boolValue)
        default: NSLog("[settings] %@: 未处理的 effect kind=%@", key, kind)
        }
    }
}
