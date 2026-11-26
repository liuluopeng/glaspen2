// S7: 菜单/快捷键/飘渺/压感/设置面板剩余薄桥与运行时查询(Swift 化, 取代 parts/menu.m)。
import AppKit

@objc public final class MenuRuntime: NSObject {
    static func l10n(_ key: String) -> String {
        guard let p = key.withCString({ sw_text($0) }) else { return key }
        defer { glaspen2_free_c_string(p) }
        return String(cString: p)
    }
    static func pair(_ zh: String, _ en: String) -> String { glaspen2_router_lang() == 0 ? zh : en }
    static func notify(_ text: String) { NotificationPresenter.show(text: text) }
    static func colorItemTitle(_ idx: Int32) -> NSAttributedString {
        colorItemTitle(index: idx)
    }
    static func widthItemTitle(_ idx: Int32) -> NSAttributedString {
        widthItemTitle(index: idx)
    }
    static func shortcutInstall(values: NSDictionary, lookup: NSDictionary) {
        guard let vd = try? JSONSerialization.data(withJSONObject: values),
              let v = String(data: vd, encoding: .utf8),
              let ld = try? JSONSerialization.data(withJSONObject: lookup),
              let l = String(data: ld, encoding: .utf8) else { return }
        v.withCString { vp in l.withCString { lp in
            glaspen2_shortcut_store_install(vp, lp)
        } }
    }

    static func shortcutChord(forAction action: String) -> String? {
        guard let p = action.withCString({ glaspen2_shortcut_store_chord($0) }) else { return nil }
        defer { glaspen2_free_c_string(p) }
        return String(cString: p)
    }

    static func shortcutAction(forChord chord: String) -> String? {
        guard let p = chord.withCString({ glaspen2_shortcut_store_action_for_chord($0) }) else { return nil }
        defer { glaspen2_free_c_string(p) }
        return String(cString: p)
    }

    static var shortcutStoreEmpty: Bool { glaspen2_shortcut_store_is_empty() != 0 }

    static func actionForChord(_ chord: String) -> String? {
        MenuRuntime.shortcutAction(forChord: chord)
    }
    static func shortcutChord(_ action: String) -> String? {
        MenuRuntime.shortcutChord(forAction: action)
    }
    /// R7: 动作 → NSEvent.ModifierFlags.rawValue(解析在 core::chord)。
    static func applyPanelTransparent(on: Bool) {
        guard let window = UIAnchors.settingsWindow else { return }
        SettingsGlue.runOnMainSync {
            window.isOpaque = !on
            window.backgroundColor = on ? .clear : .windowBackgroundColor
        }
    }

    static func menuItem(tag: Int, inMenu menu: NSMenu?) -> NSMenuItem? {
        guard let menu else { return nil }
        let p = glaspen2_rust_menu_item_with_tag(Unmanaged.passUnretained(menu).toOpaque(), tag)
        guard let p else { return nil }
        return Unmanaged<NSMenuItem>.fromOpaque(p).takeUnretainedValue()
    }

    static func colorItemTitle(index: Int32) -> NSAttributedString {
        var r = 0.0, g = 0.0, b = 0.0
        glaspen2_color_preset_rgb(index, &r, &g, &b)
        let color = NSColor(calibratedRed: r, green: g, blue: b, alpha: 1.0)
        let icon = MenuIconFactory.colorDot(color)
        return MenuIconFactory.menuText(icon: icon, name: colorDisplayName(index: Int(index)))
    }

    static func widthItemTitle(index: Int32) -> NSAttributedString {
        let scale = glaspen2_width_preset_value(index)
        let icon = MenuIconFactory.widthLine(scale)
        return MenuIconFactory.menuText(icon: icon, name: widthDisplayName(index: Int(index)))
    }

    static func colorDisplayName(index: Int) -> String {
        guard let p = glaspen2_color_name(Int32(index), glaspen2_router_lang()) else { return "" }
        defer { glaspen2_free_c_string(p) }
        return String(cString: p)
    }

    static func widthDisplayName(index: Int) -> String {
        guard let p = glaspen2_width_name(Int32(index), glaspen2_router_lang()) else { return "" }
        defer { glaspen2_free_c_string(p) }
        return String(cString: p)
    }

    static func maskFor(action: String) -> UInt {
        guard let chord = MenuRuntime.shortcutChord(forAction: action), !chord.isEmpty else { return 0 }
        return UInt(chord.withCString { glaspen2_chord_modifier_mask($0) })
    }

    static func shortcutReserved(_ chord: String) -> Bool {
        chord.withCString { glaspen2_chord_reserved($0) != 0 }
    }
    static func shortcutsEffectiveJSON() -> String {
        // 旧实现: 尚未 apply 过时先补默认表, 避免快照返回空对象。
        if MenuRuntime.shortcutStoreEmpty { _ = "{}".withCString { glaspen2_shortcut_apply_json($0) } }
        guard let p = glaspen2_shortcut_store_values_json() else { return "{}" }
        defer { glaspen2_free_c_string(p) }
        return String(cString: p)
    }
}

// C ABI 薄桥(供剩余 ObjC 与 Swift 既有 Bridging 声明使用)
@_cdecl("glaspen2_menu_lang")
public func menuLang() -> Int32 { glaspen2_router_lang() }
@_cdecl("glaspen2_menu_set_lang")
public func menuSetLang(_ value: Int32) { glaspen2_router_set_lang(value) }
@_cdecl("glaspen2_menu_refresh_texts")
public func menuRefreshTexts() { MenuTextUpdater.update() }
@_cdecl("glaspen2_menu_launch_at_login")
public func menuLaunchAtLogin() -> Int32 { glaspen2_is_launch_at_login() }
@_cdecl("glaspen2_menu_set_launch")
public func menuSetLaunch(_ on: Int32) { MenuHandler.setLaunchAtLogin(enabled: on != 0) }
@_cdecl("glaspen2_menu_glass_enabled")
public func menuGlassEnabled() -> Int32 { glaspen2_router_glass_enabled() }
@_cdecl("glaspen2_menu_set_glass")
public func menuSetGlass(_ on: Int32) { MenuHandler.setGlassEnabled(enabled: on != 0) }
@_cdecl("glaspen2_menu_outline_enabled")
public func menuOutlineEnabled() -> Int32 { glaspen2_router_outline_enabled() }
@_cdecl("glaspen2_menu_apply_outline")
public func menuApplyOutline(_ on: Int32) { MenuHandler.setOutlineEnabled(enabled: on != 0) }
@_cdecl("glaspen2_menu_infinite_enabled")
public func menuInfiniteEnabled() -> Int32 { glaspen2_router_infinite() }
@_cdecl("glaspen2_menu_apply_infinite")
public func menuApplyInfinite(_ on: Int32) {
    let text = MenuRuntime.l10n(on != 0 ? "canvas.infinite_on" : "canvas.infinite_off")
    _ = text.withCString { glaspen2_rust_apply_infinite(on, $0) }
}
@_cdecl("glaspen2_menu_save_with_bg")
public func menuSaveWithBg() { SaveBackground.save() }
@_cdecl("glaspen2_menu_save_only")
public func menuSaveOnly() { SurfaceOps.saveDrawingOnly() }
@_cdecl("glaspen2_menu_clear_screen")
public func menuClearScreen() { SurfaceOps.clearScreen() }
@_cdecl("glaspen2_menu_toggle_enabled")
public func menuToggleEnabled() { DrawMode.toggleEnabled() }
@_cdecl("glaspen2_menu_show_settings")
public func menuShowSettings() { SettingsPanel.show() }
@_cdecl("glaspen2_menu_toggle_canvas_mode")
public func menuToggleCanvasMode() { DrawMode.toggleCanvasMode() }
@_cdecl("glaspen2_menu_select_color")
public func menuSelectColor(_ idx: Int32) { MenuHandler.setColor(index: idx) }
@_cdecl("glaspen2_menu_select_width")
public func menuSelectWidth(_ idx: Int32) { MenuHandler.setWidth(index: idx) }
@_cdecl("glaspen2_menu_save_xoj_raw")
public func menuSaveXoj() { glaspen2_save_xoj() }
@_cdecl("glaspen2_menu_quit_bridge")
public func menuQuitBridge() { MenuHandler.quitAppNow() }
@_cdecl("glaspen2_router_pm_show")
public func menuPMShow() { PressureMonitorWindow.show() }
@_cdecl("glaspen2_router_pm_hide")
public func menuPMHide() { PressureMonitorWindow.hide() }
@_cdecl("glaspen2_router_pm_refresh")
public func menuPMRefresh() { PressureMonitorWindow.refreshLabel() }
@_cdecl("gl_panel_transparent_apply")
public func menuPanelTransparent(_ on: Int32) {
    MenuRuntime.applyPanelTransparent(on: on != 0)
}
@_cdecl("gl_ethereal_restore_now")
public func menuEtherealRestoreNow() { EtherealMode.restoreNow() }
@_cdecl("auto_hide_now")
public func menuAutoHideNow() -> Int32 { EtherealMode.hideNow() ? 1 : 0 }
@_cdecl("auto_show_canvas")
public func menuAutoShowCanvas() { EtherealMode.autoShow() }
@_cdecl("peek_cancel_timer")
public func menuPeekCancelTimer() { EtherealMode.cancelTimer() }
@_cdecl("peek_lens_change")
public func menuPeekLensChange() { EtherealMode.peekLensChange() }

// R43: Rust 侧菜单通知桥(NSString* -> NotificationPresenter)。
@_cdecl("glaspen2_menu_notify")
public func menuNotify(_ text: UnsafeMutableRawPointer?) {
    guard let text else { return }
    let ns = Unmanaged<NSString>.fromOpaque(text).takeUnretainedValue()
    NotificationPresenter.show(text: ns as String)
}

