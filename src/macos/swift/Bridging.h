// glaspen2 macOS 原生层 —— Swift → ObjC/C 桥接头(Swift migration S2)
//
// build.rs 用 `swiftc -import-objc-header src/macos/swift/Bridging.h` 把它导入
// Swift 模块。这里只允许**C 兼容声明**:
//   - C 基础类型(char/int/double/指针)/ NSObject 子类 / @objc 协议;
//   - 不得出现 Swift 调用方无法理解的东西(Block 返回值、ObjC 泛型容器等
//     需要显式转换的声明请先确认 Swift 侧能表达, 见 docs/swift-boundary.md)。
//
// 本头文件只声明 ObjC/C 侧已有的函数;实现仍留在 ObjC(或 Rust C ABI)一侧。
// 迁移新模块时: 把该模块对外暴露的 C 入口按原签名追加到这里, Swift 侧
// 直接按 C 语义调用, 不要包一层 Swift 类型。

#ifndef GLASPEN2_SWIFT_BRIDGING_H
#define GLASPEN2_SWIFT_BRIDGING_H

#import <Foundation/Foundation.h>
#import <CoreGraphics/CoreGraphics.h>
#include <cairo/cairo.h>

#ifdef __cplusplus
extern "C" {
#endif

// S2 反向互调探针: ObjC 侧实现(src/macos/swift/InteropProbe.m), 无副作用
// (只打日志 + 返回常量 0x5A2)。SwiftInteropProbe.ping() 经本声明调用它,
// 验证 Swift→ObjC 方向。
int glaspen2_macos_objc_probe(const char *tag);

// S3b-2b-4b: 菜单文案桥(实现留在 menu.m, 内部调 gl_text; Swift 只按 key 取名)
NSString *glaspen2_menu_l10n(NSString *key);

// S3b-2b-7a: 菜单 delegate/退出薄桥(Swift MenuHandler 调用; 实现留在 menu.m)
void glaspen2_menu_will_open_bridge(void);
void glaspen2_menu_did_close_bridge(void);
void glaspen2_menu_quit_bridge(void);

// S3b-2b-7b: 语言切换所需的语言状态桥(实现留在 menu.m, g_lang 是 TU 内静态全局)
int glaspen2_menu_lang(void);
void glaspen2_menu_set_lang(int value);
void glaspen2_menu_refresh_texts(void);

// 既有 C FFI(实现在 core/Rust, 这里只补声明给 Swift 用)
void glaspen2_save_string_setting(const char *key, const char *value);
void glaspen2_notify_settings_changed(void);

// S3b-2b-7c: 开关类菜单动作的 getter/setter 薄桥(实现留在 menu.m)
int glaspen2_menu_launch_at_login(void);
void glaspen2_menu_set_launch(int on);
int glaspen2_menu_glass_enabled(void);
void glaspen2_menu_set_glass(int on);
int glaspen2_menu_outline_enabled(void);
void glaspen2_menu_apply_outline(int on);
int glaspen2_menu_infinite_enabled(void);
void glaspen2_menu_apply_infinite(int on);

// S3b-2b-7d: 保存/导出/设置/选择动作的薄桥(实现留在 menu.m)
void glaspen2_menu_save_with_bg(void);
void glaspen2_menu_save_only(void);
void glaspen2_menu_clear_screen(void);
void glaspen2_menu_toggle_enabled(void);
void glaspen2_menu_show_settings(void);
void glaspen2_menu_toggle_canvas_mode(void);
void glaspen2_menu_select_color(int idx);
void glaspen2_menu_select_width(int idx);
void glaspen2_menu_save_xoj_raw(void);
void glaspen2_menu_notify(NSString *text);

// S6-3-1: 事件路由所需的只读状态/动作薄桥(实现留在 run.m)
NSUInteger glaspen2_router_camera_mods(void);
int glaspen2_router_state(void);
double glaspen2_router_draw_height(void);
void glaspen2_router_zoom_at(double factor, double x, double y);
int64_t glaspen2_virtual_pen_user_data(void);
int glaspen2_router_cursor_flags(void);
int glaspen2_router_pen_state(void);
void glaspen2_router_pen_bubble(int back);
void glaspen2_router_pen_auto_show(void);
void glaspen2_router_peek_cancel(void);
double glaspen2_router_cursor_x(void);
double glaspen2_router_cursor_y(void);
void glaspen2_router_cursor_set(double x, double y);
void glaspen2_router_cursor_show(int on);
int glaspen2_router_pen_cursor_visible(void);
int glaspen2_router_cursor_flush_due(void);
int glaspen2_router_pen_drawing(void);
void glaspen2_router_set_pen_drawing(int on);
void glaspen2_router_hide_system_cursor(void);
void glaspen2_router_show_system_cursor(void);
void glaspen2_router_disable_cleanup(void);
void glaspen2_router_tap_menu_bar_interaction(void);
int glaspen2_router_eraser_mode(void);
void glaspen2_router_set_eraser_mode(int on);
void glaspen2_router_update_status_icon_state(void);
void glaspen2_router_mark_tablet_proximity(void);
void glaspen2_router_set_prox_device(int device);
void glaspen2_router_bubble_pen_back(void);
int glaspen2_router_ethereal(void);
int glaspen2_router_camera_dragging(void);
double glaspen2_router_camera_step_zoom(void);
void glaspen2_router_camera_queue_pan(double dx, double dy);
int glaspen2_router_pm_enabled(void);
void glaspen2_router_reenable_tap(void);
double glaspen2_router_raw_width(double pressure);
int glaspen2_router_prox_is_eraser(void);
int glaspen2_router_strokes_hidden(void);
id glaspen2_router_main_window(void);
void glaspen2_router_perf_summary(void);
int glaspen2_router_pending_color(void);
int glaspen2_router_pending_width(void);
void glaspen2_router_clear_pending_color(void);
void glaspen2_router_clear_pending_width(void);
int glaspen2_router_width_preset_count(void);
int glaspen2_router_selected_width(void);
void glaspen2_router_set_prev_width(int idx);
void glaspen2_router_set_pending_width(int idx);
void glaspen2_router_apply_width(int idx);
void glaspen2_router_save_pen_settings(void);
void glaspen2_router_update_checkmarks(void);
void glaspen2_router_sync_settings_panel(void);
int glaspen2_router_color_preset_count(void);
int glaspen2_router_selected_color(void);
void glaspen2_router_set_prev_color(int idx);
void glaspen2_router_set_pending_color(int idx);
void glaspen2_router_apply_color(int idx);
void glaspen2_router_set_glass_state(int on);
void glaspen2_router_glass_apply(void);
extern void glaspen2_save_bool_setting(const char *key, int value);
void glaspen2_router_update_menu_state(int tag, int state);
extern void glaspen2_set_launch_at_login(int on);
void glaspen2_router_set_grid_state(int on);
void glaspen2_router_redraw_draw_view(void);
void glaspen2_router_set_pressure_monitor(int on);
void glaspen2_router_set_enabled_state(int on);
void glaspen2_router_set_menu_title(int tag, NSString *title);
extern int glaspen2_load_bool_setting(const char *key);
extern void glaspen2_share_ink_set_active(int active);
void glaspen2_router_set_chat_integration(int on);
void glaspen2_router_rec_chat_integration_changed(int on);
NSDictionary *glaspen2_router_glass_state(void);
void glaspen2_router_apply_glass_view(double alpha, BOOL hidden);
int glaspen2_router_outline_enabled(void);
void glaspen2_router_set_outline_state(int on);
void glaspen2_router_rebuild_surface(void);
extern void glaspen2_set_stroke_outline(int enabled);
void glaspen2_router_set_camera(double panX, double panY, double zoom, int assignZoom);
extern char *glaspen2_chord_normalize(const char *raw);
extern char *glaspen2_camera_pan(double panX, double panY, double zoom, double dx, double dy);
extern char *glaspen2_chord_migrate_value(const char *action, const char *raw);
extern void glaspen2_set_infinite_transform(double panX, double panY, double zoom);
extern void glaspen2_get_infinite_transform(double *panX, double *panY, double *zoom);
extern void glaspen2_set_view_transform(double panX, double panY, double zoom);
extern char *glaspen2_camera_zoom_at(double panX, double panY, double zoom, double factor, double vx, double vy);
extern char *glaspen2_camera_zoom_at_center(double panX, double panY, double zoom, double factor, double vw, double vh);
extern void glaspen2_camera_home(double *panX, double *panY, double *zoom);
double glaspen2_router_screen_w(void);
double glaspen2_router_screen_h(void);
void glaspen2_router_reset_page_offsets(void);
int glaspen2_router_pageview_pw(void);
NSString *glaspen2_router_shortcut_chord(NSString *action);
extern char *glaspen2_chord_defaults_json(void);
extern char *glaspen2_chord_actions_json(void);
int glaspen2_router_shortcut_reserved(NSString *chord);
void glaspen2_router_set_shortcut_tables(NSDictionary *values, NSDictionary *lookup);
int glaspen2_router_enabled(void);
int glaspen2_router_pressure_monitor(void);
int glaspen2_router_strokes_visible(void);
void glaspen2_router_ensure_surface(void);
void glaspen2_router_set_ethereal(int on);
void glaspen2_router_set_strokes_visible(int on);
int glaspen2_router_glass_follow(void);
id glaspen2_router_glass_view(void);
id glaspen2_router_menu(void);
int glaspen2_router_menu_state(int tag);
void glaspen2_router_invert_invalidate(void);
void glaspen2_router_set_screen_size(int w, int h);
id glaspen2_router_draw_view(void);
int glaspen2_router_has_surface(void);
void glaspen2_router_dirty_reset(void);
int glaspen2_router_dirty_has(void);
NSRect glaspen2_router_dirty_rect(void);
void glaspen2_router_install_surface(cairo_surface_t *surface);
void glaspen2_router_install_scale(double scale);
void glaspen2_router_set_has_last(int has);
void glaspen2_router_surface_cache_invalidate(void);
cairo_surface_t *glaspen2_router_surface(void);
cairo_t *glaspen2_router_acquire_cr(int *owned);
int glaspen2_router_soft_shadow(void);
double glaspen2_router_outline_pad(void);
double glaspen2_router_pen_r(void);
double glaspen2_router_pen_g(void);
double glaspen2_router_pen_b(void);
int glaspen2_router_live_plan_dot(cairo_t *cr, double x, double y, double width);
cairo_pattern_t *glaspen2_router_invert_ink_source(cairo_t *cr);
void glaspen2_router_dirty_include_surface_point(double x, double y, double pad);
extern void glaspen2_on_display_change(int screen_w, int screen_h);
extern void glaspen2_smooth_loaded_strokes(void);
NSAttributedString *glaspen2_router_color_item_title(int idx);
NSAttributedString *glaspen2_router_width_item_title(int idx);
void glaspen2_router_draw_prepare(void);
NSDictionary *glaspen2_router_draw_state(void);
void glaspen2_router_perf_notes(const char *name, const char *notes, uint64_t t0);
extern char *glaspen2_minimap_layout_json(long long current_screen_id, double view_w, double view_h, int geometry_width);
extern unsigned char *glaspen2_render_thumbnail(long long screen_id, int w, int h, int max_size, int *out_len);
extern void glaspen2_free_rust_bytes(unsigned char *ptr, int len);
extern int glaspen2_stroke_bbox(double *bx, double *by, double *bx2, double *by2);
extern unsigned char *glaspen2_render_canvas_overview(double bx, double by, double bw, double bh, int ow, int oh, int *out_len);
extern double glaspen2_camera_step_zoom(double zoom);
int glaspen2_router_infinite(void);
void glaspen2_router_set_infinite_state(int on);
void glaspen2_router_clear_camera_dragging(void);
void glaspen2_router_pageview_update(void);
void glaspen2_router_event_tap_reinstall(void);
extern int glaspen2_set_canvas_kind(int infinite);
extern int glaspen2_load_infinite_strokes(void);
extern long glaspen2_get_current_screen_id(void);
extern int glaspen2_load_strokes_for_screen(long screen_id);
extern int glaspen2_clear_strokes(int screen_w, int screen_h);
extern void glaspen2_notify_settings_changed(void);
extern int glaspen2_color_preset_count(void);
extern void glaspen2_color_preset_rgb(int i, double *r, double *g, double *b);
extern int glaspen2_width_preset_count(void);
extern double glaspen2_width_preset_value(int i);
void glaspen2_router_set_color_preset(int idx, double r, double g, double b);
void glaspen2_router_set_width_preset(int idx, double value);
double glaspen2_router_pan_x(void);
double glaspen2_router_pan_y(void);
double glaspen2_router_zoom(void);
void glaspen2_router_canvas_apply_transform(void);
void glaspen2_router_peek_lens_change(void);
void glaspen2_router_pm_show(void);
void glaspen2_router_pm_hide(void);
void glaspen2_router_pm_refresh(void);
NSDictionary *glaspen2_router_grid_state(void);
NSDictionary *glaspen2_router_cursor_draw_state(void);
extern char *glaspen2_grid_json(double view_w, double view_h, double pan_x, double pan_y, double zoom, double grid_size, int divider, int follow);
extern char *glaspen2_grid_json_clipped(double view_w, double view_h, double pan_x, double pan_y, double zoom, double grid_size, int divider, int show_grid, double clip_x0, double clip_y0, double clip_x1, double clip_y1);
double glaspen2_router_last_raw_x(void);
double glaspen2_router_last_raw_y(void);
void glaspen2_router_clear_raw_state(void);
const char *glaspen2_router_lang_code(void);
extern char *glaspen2_page_info_text(long long screen_id, const char *lang);
extern void glaspen2_free_c_string(char *s);
void glaspen2_router_pm_update(int inRange, int pressure, double x, double y, int tipDown, NSString *evtype);
void glaspen2_router_camera_set_last(double x, double y);
double glaspen2_router_camera_last_x(void);
double glaspen2_router_camera_last_y(void);
void glaspen2_router_camera_flush(void);
void glaspen2_router_camera_clear_dragging(void);
void glaspen2_router_camera_start(double x, double y);
NSUInteger glaspen2_router_camera_pan_mods(void);
void glaspen2_router_bubble_auto_page_flip(void);
void glaspen2_router_modeler_begin(double x, double y, double pressure, double timestamp);
void glaspen2_router_stroke_begin(void);
void glaspen2_router_raw_draw_dot(double x, double y, double rawWidth);
void glaspen2_router_pen_down_state(double x, double y, double rawWidth);
void glaspen2_router_modeler_move(double x, double y, double pressure, double timestamp);
void glaspen2_router_raw_draw_segment(double x, double y, double rawWidth);
int glaspen2_router_stroke_active(void);
void glaspen2_router_set_stroke_active(int on);
void glaspen2_router_modeler_erase_finish(void);
void glaspen2_router_modeler_end(double x, double y, double pressure, double timestamp);
void glaspen2_router_commit_to_strokes(void);
void glaspen2_router_stroke_end(void);
void glaspen2_router_apply_pending_color_width(void);
void glaspen2_router_pen_up_state(void);
NSString *glaspen2_router_keycode_name(unsigned short keyCode);
NSString *glaspen2_router_action_for_chord(NSString *chord);
void glaspen2_router_perform_action(NSString *action);
int glaspen2_router_rec_slot_gif(void);
int glaspen2_router_rec_slot_chat(void);
int glaspen2_router_rec_mode(int slot);
int glaspen2_router_rec_finish_and_count(void);
int glaspen2_router_rec_feed(int slot, NSString *json);
int glaspen2_router_chat_integration(void);
void glaspen2_router_cursor_invalidate(void);
void glaspen2_router_cursor_hide(void);
void glaspen2_router_auto_hide(void);
void glaspen2_router_bubble_pen_gone(void);
void glaspen2_router_flush(void);

// S6-2: 事件 tap 回调函数指针 / tap 存储 / 滚轮标记桥(实现留在 run.m)
CGEventTapCallBack glaspen2_event_tap_callback_ptr(void);
void glaspen2_event_tap_store(CFMachPortRef tap);
void glaspen2_event_tap_note_scroll(int on);

// S5-3: 显示器防抖所需的当前几何读取与应用桥(实现留在 panel.m)
void glaspen2_display_size(int *w, int *h);
void glaspen2_display_apply(int w, int h);

// S4: 通知 presenter 的 redraw 桥(实现留在 glaspen2.m; Swift 通知状态变化时调用)
void glaspen2_notification_redraw(void);

// S3b-2b-7e: 异步导出/退出所需薄桥
void glaspen2_menu_screen_size(int *w, int *h);
int glaspen2_menu_export_infinite_pdf_paged(int w, int h);
int glaspen2_menu_export_infinite_svg(void);
void glaspen2_menu_shutdown(void);
NSString *glaspen2_menu_l10n_pair(NSString *zh, NSString *en);

// S2 探针入口(ObjC 实现, 供 glaspen2_run / Rust --swift-probe 调用):
// 内部经生成的 glaspen2_macos_swift-Swift.h 调 [SwiftInteropProbe ping],
// 验证 ObjC→Swift 方向。普通迁移代码不需要这类"Swift 反向入口"。
void glaspen2_macos_swift_probe_selftest(void);
void glaspen2_macos_swift_probe_maybe(void);

#ifdef __cplusplus
}
#endif

#endif // GLASPEN2_SWIFT_BRIDGING_H
