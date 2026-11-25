// S7: 应用/菜单 delegate(替代原 ObjC GlaspenMenuHandler, 只做转发)。
import AppKit

@objc public final class AppDelegate: NSObject, NSApplicationDelegate, NSMenuDelegate {
    static var shared: AppDelegate?
    @objc public func saveWithBg() { MenuHandler.saveWithBg() }
    @objc public func saveOnly() { MenuHandler.saveOnly() }
    @objc public func clearScreen() { MenuHandler.clearScreen() }
    @objc public func toggleDraw() { MenuHandler.toggleDraw() }
    @objc public func saveXoj() { MenuHandler.saveXoj() }

    @objc public func toggleLanguage() { MenuHandler.toggleLanguage() }
    @objc public func showSettingsPanel() { MenuHandler.showSettingsPanel() }
    @objc public func toggleCanvasMode() { MenuHandler.toggleCanvasMode() }

    @objc public func toggleLaunch() { MenuHandler.toggleLaunch() }
    @objc public func toggleGlass() { MenuHandler.toggleGlass() }
    @objc public func toggleOutline() { MenuHandler.toggleOutline() }
    @objc public func toggleInfiniteCanvas() { MenuHandler.toggleInfiniteCanvas() }

    @objc public func exportInfinitePdf() { MenuHandler.exportInfinitePdf() }
    @objc public func exportInfiniteSvg() { MenuHandler.exportInfiniteSvg() }

    @objc(selectColor:)
    public func selectColor(_ sender: NSMenuItem) { MenuHandler.selectColor(sender) }
    @objc(selectWidth:)
    public func selectWidth(_ sender: NSMenuItem) { MenuHandler.selectWidth(sender) }

    public func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        MenuHandler.applicationShouldTerminate(sender)
    }

    @objc public func quitApp() { MenuHandler.quitApp() }
    public func menuWillOpen(_ menu: NSMenu) { MenuHandler.menuWillOpen() }
    public func menuDidClose(_ menu: NSMenu) { MenuHandler.menuDidClose() }
}
