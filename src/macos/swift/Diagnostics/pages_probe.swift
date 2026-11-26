// S8 自检: 页面导航 + 空画布总览载荷(隐藏 CLI 入口; 需 GLASPEN2_DB_PATH 指向临时库)。
import AppKit

@objc public final class PagesProbe: NSObject {
    @objc public static func run() -> Int32 {
        var failures = 0
        func check(_ name: String, _ ok: Bool) {
            if !ok { NSLog("[pages-probe] FAIL %@", name); failures += 1 }
            else { print("PAGES_PROBE \(name) ok") }
        }

        glaspen2_init_db(1920, 1080)
        let current = glaspen2_get_current_screen_id()
        check("current_page", current > 0)

        let inserted = glaspen2_debug_insert_pages(2, 1920, 1080)
        check("debug_insert_pages", inserted == 2)
        let neighbor = glaspen2_page_neighbor(current, 1)
        check("page_neighbor", neighbor > 0)

        let created = glaspen2_create_next_page(current)
        check("create_next_page", created > 0)

        var pw: Int32 = 0
        var ph: Int32 = 0
        glaspen2_page_dims(created, &pw, &ph)
        check("page_dims", pw == 1920 && ph == 1080)
        check("stroke_count_empty", glaspen2_stroke_count() == 0)

        // 空画布: 总览载荷应为 nil(不崩溃)。
        check("canvas_overview_empty", SettingsGlue.canvasOverviewPayload(width: 800, height: 600) == nil)
        var len: Int32 = 0
        let payload = settingsApiCanvasPayload(800, 600, 0, nil, &len)
        check("canvas_payload_empty", payload == nil && len == 0)

        let deleted = glaspen2_delete_screen(created)
        check("delete_page", deleted == 1)

        print("SWIFT_PAGES_PROBE_OK failures=\(failures)")
        return failures == 0 ? 0 : 1
    }
}

@_cdecl("glaspen2_swift_pages_probe_selftest")
public func pagesProbeSelftest() -> Int32 { PagesProbe.run() }
