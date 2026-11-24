// S7: 动作 id → effect(kind) 分派(热键/镜头/颜色/退出/自由画布)。
import AppKit

@objc public final class ActionDispatch: NSObject {
    @objc(performAction:)
    public static func perform(action: String) -> Bool {
        guard let effectRaw = action.withCString({ glaspen2_action_effect_json($0) }) else {
            return false
        }
        let text = String(cString: effectRaw)
        glaspen2_free_c_string(effectRaw)
        guard let data = text.data(using: .utf8),
              let parsed = try? JSONSerialization.jsonObject(with: data),
              let effect = parsed as? [String: Any] else {
            NSLog("[action] %@ 的 effect JSON 无法解析", action)
            return false
        }
        guard let kind = effect["kind"] as? String else {
            NSLog("[action] %@ 分发失败: %@", action, "\(effect["error"] ?? "") ")
            return false
        }
        switch kind {
        case "Undo": return hotkey(0x06) // kVK_ANSI_Z
        case "PagePrev": return hotkey(0x32)
        case "PageNext": return hotkey(0x12)
        case "ToggleDraw": return hotkey(0x09) // kVK_ANSI_V
        case "Clear": return hotkey(0x08) // kVK_ANSI_C
        case "Export": return hotkey(0x05) // kVK_ANSI_G
        case "CopySvg": return hotkey(0x01) // kVK_ANSI_S
        case "ToggleGlass": return hotkey(0x0B) // kVK_ANSI_B
        case "ToggleEthereal": return hotkey(0x07) // kVK_ANSI_X
        case "OpenSettings": return hotkey(0x02) // kVK_ANSI_D
        default: break
        }
        if kind == "ToggleFreeCanvas" {
            if glaspen2_router_infinite() == 0,
               glaspen2_load_bool_setting("show_free_canvas") == 0 {
                "show_free_canvas".withCString { glaspen2_save_bool_setting($0, 1) }
            }
            DispatchQueue.main.async {
                glaspen2_router_apply_infinite_canvas(glaspen2_router_infinite() != 0 ? 0 : 1)
            }
            return true
        }
        if kind.hasPrefix("CameraPan") {
            if glaspen2_router_infinite() == 0 { return true }
            let step = glaspen2_camera_keyboard_pan_step(glaspen2_router_zoom())
            var dx = 0.0, dy = 0.0
            if kind == "CameraPanLeft" { dx = step }
            else if kind == "CameraPanRight" { dx = -step }
            else if kind == "CameraPanUp" { dy = step }
            else if kind == "CameraPanDown" { dy = -step }
            CanvasCamera.panBy(dx: dx, dy: dy)
            return true
        }
        if kind == "CameraZoomIn" || kind == "CameraZoomOut" {
            if glaspen2_router_infinite() == 0 { return true }
            let factor = kind == "CameraZoomIn" ? 1.15 : 1.0 / 1.15
            CanvasCamera.zoomAtCenter(factor: factor)
            return true
        }
        if kind == "Quit" {
            DispatchQueue.main.async { MenuHandler.quitAppNow() }
            return true
        }
        if kind == "ColorSwap" {
            var prev = Int(glaspen2_router_prev_color_index())
            let selected = Int(glaspen2_router_selected_color_index())
            if prev < 0 || prev >= Int(glaspen2_router_color_preset_count()) || prev == selected {
                prev = selected == 0 ? 9 : 0
            }
            MenuHandler.setColor(index: Int32(prev))
            let name = MenuNames.colorDisplayName(index: prev)
            glaspen2_router_show_notification(
                I18nText.textf("hud.color_changed", ["color": name]))
            return true
        }
        if kind == "WidthSwap" {
            var prev = Int(glaspen2_router_prev_width_index())
            let selected = Int(glaspen2_router_selected_width_index())
            if prev < 0 || prev >= Int(glaspen2_router_width_preset_count()) || prev == selected {
                prev = selected <= 3 ? 6 : 2
            }
            MenuHandler.setWidth(index: Int32(prev))
            let name = MenuNames.widthDisplayName(index: prev)
            glaspen2_router_show_notification(
                I18nText.textf("hud.width_changed", ["width": name]))
            return true
        }
        NSLog("[action] %@ 的 effect kind %@ 未在 macOS 适配器实现", action, kind)
        return false
    }

    private static func hotkey(_ keyCode: UInt16) -> Bool {
        glaspen2_router_perform_hotkey(keyCode) != 0
    }
}
