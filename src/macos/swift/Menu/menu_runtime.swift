// S7: 菜单/快捷键/飘渺/压感/设置面板剩余薄桥与运行时查询(Swift 化, 取代 parts/menu.m)。
import AppKit

@objc public final class MenuRuntime: NSObject {
    static func l10n(_ key: String) -> String { I18nText.text(key) }
    static func pair(_ zh: String, _ en: String) -> String { glaspen2_router_lang() == 0 ? zh : en }
    static func notify(_ text: String) { NotificationPresenter.show(text: text) }
    static func setMenuTitle(tag: Int32, title: String) {
        guard let item = MenuLookup.item(tag: Int(tag), inMenu: UIAnchors.menu) else { return }
        item.title = title
    }
    static func colorItemTitle(_ idx: Int32) -> NSAttributedString {
        MenuPresetTitles.colorItemText(index: idx)
    }
    static func widthItemTitle(_ idx: Int32) -> NSAttributedString {
        MenuPresetTitles.widthItemText(index: idx)
    }
    static func ensureSurface() {
        if let dv = UIAnchors.drawView { SurfaceStore.ensure(view: dv) }
    }
    static func updateMenuState(tag: Int32, state: Int32) {
        guard let item = MenuLookup.item(tag: Int(tag), inMenu: UIAnchors.menu) else { return }
        item.state = state != 0 ? NSControl.StateValue.on : NSControl.StateValue.off
    }
    static func menuState(_ tag: Int32) -> Int32 {
        guard let item = MenuLookup.item(tag: Int(tag), inMenu: UIAnchors.menu) else { return 0 }
        return item.state == NSControl.StateValue.on ? 1 : 0
    }
    static func applyGlassView(alpha: Double, hidden: Bool) {
        guard let gv = UIAnchors.glassView else { return }
        gv.alphaValue = alpha
        gv.isHidden = hidden
    }
    static func keycodeName(_ kc: UInt16) -> String? { ShortcutNames.name(for: kc) }
    static func actionForChord(_ chord: String) -> String? {
        ShortcutStore.action(forChord: chord)
    }
    static func shortcutChord(_ action: String) -> String? {
        ShortcutStore.chord(forAction: action)
    }
    /// R7: 动作 → NSEvent.ModifierFlags.rawValue(解析在 core::chord)。
    static func maskFor(action: String) -> UInt {
        guard let chord = ShortcutStore.chord(forAction: action), !chord.isEmpty else { return 0 }
        return UInt(chord.withCString { glaspen2_chord_modifier_mask($0) })
    }

    static func shortcutReserved(_ chord: String) -> Bool {
        chord.withCString { glaspen2_chord_reserved($0) != 0 }
    }
    static func shortcutValues() -> NSDictionary {
        ShortcutStore.valuesSnapshot()
    }
    static func shortcutsEffectiveJSON() -> String {
        // 旧实现: 尚未 apply 过时先补默认表, 避免快照返回空对象。
        if ShortcutStore.isEmpty { ShortcutApply.apply(saved: NSDictionary()) }
        return SettingsGlue.effectiveShortcutJSON() as String
    }
    static func setShortcutTables(values: NSDictionary, lookup: NSDictionary) {
        ShortcutStore.install(values: values, lookup: lookup)
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
public func menuGlassEnabled() -> Int32 { RenderSettings.glassEnabled ? 1 : 0 }
@_cdecl("glaspen2_menu_set_glass")
public func menuSetGlass(_ on: Int32) { MenuHandler.setGlassEnabled(enabled: on != 0) }
@_cdecl("glaspen2_menu_outline_enabled")
public func menuOutlineEnabled() -> Int32 { RenderSettings.outlineEnabled ? 1 : 0 }
@_cdecl("glaspen2_menu_apply_outline")
public func menuApplyOutline(_ on: Int32) { MenuHandler.setOutlineEnabled(enabled: on != 0) }
@_cdecl("glaspen2_menu_infinite_enabled")
public func menuInfiniteEnabled() -> Int32 { ViewSettings.infiniteCanvas ? 1 : 0 }
@_cdecl("glaspen2_menu_apply_infinite")
public func menuApplyInfinite(_ on: Int32) {
    CanvasMode.applyInfinite(on: on != 0, notify: true)
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
@_cdecl("glaspen2_menu_screen_size")
public func menuScreenSize(_ w: UnsafeMutablePointer<Int32>?, _ h: UnsafeMutablePointer<Int32>?) {
    w?.pointee = sw_app_screen_w()
    h?.pointee = sw_app_screen_h()
}
@_cdecl("glaspen2_menu_export_infinite_pdf_paged")
public func menuExportPDFPaged(_ w: Int32, _ h: Int32) -> Int32 {
    glaspen2_export_infinite_pdf_paged(w, h)
}
@_cdecl("glaspen2_menu_export_infinite_svg")
public func menuExportSVG() -> Int32 { glaspen2_export_infinite_svg() }
@_cdecl("glaspen2_menu_shutdown")
public func menuShutdown() { glaspen2_shutdown() }
@_cdecl("glaspen2_menu_will_open_bridge")
public func menuWillOpenBridge() {
    glaspen2_router_update_status_icon_state()
    MenuGlue.updateCheckmarks()
}
@_cdecl("glaspen2_menu_did_close_bridge")
public func menuDidCloseBridge() {
    glaspen2_router_update_status_icon_state()
    glaspen2_router_event_tap_enable()
}
@_cdecl("glaspen2_menu_quit_bridge")
public func menuQuitBridge() { MenuHandler.quitAppNow() }
@_cdecl("glaspen2_router_update_checkmarks")
public func menuUpdateCheckmarks() { MenuGlue.updateCheckmarks() }
@_cdecl("glaspen2_router_sync_settings_panel")
public func menuSyncSettingsPanel() { glaspen2_notify_settings_changed() }
@_cdecl("glaspen2_router_glass_apply")
public func menuGlassApply() { MenuHandler.applyGlass() }
@_cdecl("glaspen2_router_menu_state")
public func menuStateValue(_ tag: Int32) -> Int32 { MenuRuntime.menuState(tag) }
@_cdecl("glaspen2_router_update_menu_state")
public func menuUpdateState(_ tag: Int32, _ state: Int32) {
    MenuRuntime.updateMenuState(tag: tag, state: state)
}
@_cdecl("glaspen2_router_pm_show")
public func menuPMShow() { PressureMonitorWindow.show() }
@_cdecl("glaspen2_router_pm_hide")
public func menuPMHide() { PressureMonitorWindow.hide() }
@_cdecl("glaspen2_router_pm_refresh")
public func menuPMRefresh() { PressureState.refresh() }
@_cdecl("glaspen2_router_ensure_surface")
public func menuEnsureSurface() { MenuRuntime.ensureSurface() }
@_cdecl("glaspen2_router_apply_glass_view")
public func menuApplyGlassView(_ alpha: Double, _ hidden: Int32) {
    MenuRuntime.applyGlassView(alpha: alpha, hidden: hidden != 0)
}
@_cdecl("glaspen2_router_shortcut_reserved")
public func menuShortcutReserved(_ chord: UnsafePointer<CChar>?) -> Int32 {
    guard let chord else { return 0 }
    return MenuRuntime.shortcutReserved(String(cString: chord)) ? 1 : 0
}
@_cdecl("glaspen2_router_shortcuts_effective_json")
public func menuShortcutsEffectiveJSON() -> UnsafeMutablePointer<CChar>? {
    strdup(MenuRuntime.shortcutsEffectiveJSON())
}
@_cdecl("glaspen2_router_keycode_name")
public func menuKeycodeName(_ kc: UInt16) -> UnsafeMutablePointer<CChar>? {
    guard let name = MenuRuntime.keycodeName(kc) else { return nil }
    return strdup(name)
}
@_cdecl("glaspen2_router_action_for_chord")
public func menuActionForChord(_ chord: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>? {
    guard let chord, let action = MenuRuntime.actionForChord(String(cString: chord)) else { return nil }
    return strdup(action)
}
@_cdecl("glaspen2_router_shortcut_chord")
public func menuShortcutChord(_ action: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>? {
    guard let action, let chord = MenuRuntime.shortcutChord(String(cString: action)) else { return nil }
    return strdup(chord)
}
@_cdecl("gl_panel_transparent_apply")
public func menuPanelTransparent(_ on: Int32) {
    SettingsPanelExtras.applyPanelTransparent(on: on != 0)
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
