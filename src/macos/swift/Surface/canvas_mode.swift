// S7: 画布模式切换(翻页 ↔ 无限): 状态/存储/镜头/菜单图标/tap 重装/通知。
import AppKit

@objc public final class CanvasMode: NSObject {
    @objc(applyInfiniteCanvasOn:notify:)
    public static func applyInfinite(on: Bool, notify: Bool) {
        if (glaspen2_router_infinite() != 0) == on { return }
        glaspen2_router_clear_camera_dragging()
        CameraMousePan.cancel()
        EventRouter.finishActiveStroke()
        if glaspen2_router_infinite() != 0 { CanvasCamera.persistInfinite() }
        glaspen2_router_set_infinite_state(on ? 1 : 0)
        "infinite_canvas".withCString { glaspen2_save_bool_setting($0, on ? 1 : 0) }
        glaspen2_router_update_menu_state(668, on ? 1 : 0)
        glaspen2_router_update_status_icon_state()
        glaspen2_set_canvas_kind(on ? 1 : 0)
        if on {
            glaspen2_load_infinite_strokes()
            CanvasCamera.loadInfinite()
        } else {
            let cur = glaspen2_get_current_screen_id()
            if cur > 0 {
                glaspen2_load_strokes_for_screen(cur)
            } else {
                glaspen2_clear_strokes(Int32(glaspen2_router_screen_w()),
                                       Int32(glaspen2_router_screen_h()))
            }
            PageViewOps.update()
            CanvasCamera.resetLens()
        }
        glaspen2_router_rebuild_surface()
        glaspen2_router_event_tap_reinstall()
        glaspen2_notify_settings_changed()
        if notify {
            let key = on ? "canvas.infinite_on" : "canvas.infinite_off"
            MenuRuntime.notify(MenuRuntime.l10n(key))
        }
    }
}
