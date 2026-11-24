#include <dlfcn.h>
#import <Cocoa/Cocoa.h>
#import <QuartzCore/QuartzCore.h>
#import <Carbon/Carbon.h>
#import <ScreenCaptureKit/ScreenCaptureKit.h>
#import <CoreMedia/CoreMedia.h>
#import <CoreVideo/CoreVideo.h>
#import <IOSurface/IOSurface.h>
#import <FlutterMacOS/FlutterMacOS.h>
#import "glaspen2_macos_swift-Swift.h" // S1/S2: swiftc 生成头(构建时 -I OUT_DIR)
#include <signal.h>
#include <string.h>
#include <stdlib.h>
#include <mach/mach_time.h>
#include <fcntl.h>     // open/O_CREAT —— 单实例 flock 锁
#include <sys/file.h>  // flock

// App enabled state

// Screen dimensions in logical points (set once at startup)

// Backing scale factor for Retina rendering (1.0 = non-Retina, 2.0 = Retina)

// Forward declarations
void flush_to_layer(void);
void flush_dirty_to_layer(void);
void surface_image_cache_invalidate(void);
// 翻页动效(时光隧道)进行中(drawRect 抑制网格; 飘渺模式禁止隐藏笔迹)
static void clear_screen(void);
static void rebuild_surface_from_strokes(void);
void show_settings_panel(void);
static void perf_log_summary(void);
static void pm_ensure_window(void);
static void pm_show(void);
static void pm_hide(void);
static void pm_destroy(void);
static void pm_update(void);
// D8b 录制状态机接线: 总开关变化时把事件喂给手写消息槽并执行 effects
// (定义在 parts/run.m 录制区), 菜单/设置面板切换总开关时调用。
static void gl_recording_chat_integration_changed(BOOL on);
static void ethereal_hide_now(void); // 定义在飘渺模式区

// --- Cairo (linked via cargo) ---
#include <cairo/cairo.h>

// --- Rust FFI ---
extern void glaspen2_save_drawing(const unsigned char *data, int width, int height, int stride);
extern void glaspen2_save_with_background(
    const unsigned char *drawing_data, int drawing_width, int drawing_height, int drawing_stride,
    const unsigned char *bg_data, int bg_width, int bg_height, int bg_stride);
extern void glaspen2_begin_stroke(double r, double g, double b, double width_scale);
extern void glaspen2_add_point(double x, double y, double width);
extern void glaspen2_add_point_t(double x, double y, double width, double t);
extern void glaspen2_end_stroke(void);
extern void glaspen2_save_xoj(void);
extern int glaspen2_clear_strokes(int screen_w, int screen_h);
extern void glaspen2_init_db(int screen_w, int screen_h);
extern void glaspen2_save_settings(double r, double g, double b, double width_scale);
extern void glaspen2_shutdown(void);
extern int  glaspen2_load_settings_parts(double *r, double *g, double *b, double *w);
extern void glaspen2_save_bool_setting(const char *key, int val);
extern int  glaspen2_load_bool_setting(const char *key);
extern void glaspen2_save_string_setting(const char *key, const char *value);
extern char* glaspen2_load_string_setting(const char *key);
// D1 设置 effect 桥(core/settings_apply.rs): 解析 + 钳制 + 落库一次完成,
// 返回 effect JSON({"kind":..., "bool"/"int"/"float"/"str"/"json":...} 或 {"error":...});
// 调用方用 glaspen2_free_c_string 释放。ObjC 只按 kind 做平台运行时副作用。
extern char* glaspen2_settings_apply(const char *key, const char *value_json);
// D12 面板设置快照桥(core/settings_snapshot.rs): 入参 = 运行时专属值 JSON 对象
// (launchAtLogin/shortcuts/flipEffect/language), 返回完整快照单行 JSON
// (字段集合/顺序/类型/缺省值/DB 回退全在 core);失败回 {"error":...}, 永不 NULL;
// 调用方用 glaspen2_free_c_string 释放。
extern char* glaspen2_settings_snapshot_json(const char *overrides_json);
// D3b 动作 effect 桥(core/action_effect.rs): action id → 平台无关 effect JSON
// ({"kind":"<ActionEffect 变体名>"} 或 {"error":"..."}); 调用方用
// glaspen2_free_c_string 释放。ObjC 只按 kind 做本平台运行时副作用。
extern char* glaspen2_action_effect_json(const char *action);
// D5b 镜头数学桥(core/camera.rs): 输入 ObjC 持有的镜头三值 + 本次操作参数,
// 返回 core 算好的新三值 JSON({"pan_x","pan_y","zoom"}[,"hit_max","hit_min",
// "changed"] 或 {"error":"..."}), 永不 NULL;调用方用 glaspen2_free_c_string 释放。
// ObjC 只负责写回 g_pan_x/g_pan_y/g_zoom 与本平台门控/提示/持久化/重绘。
extern char* glaspen2_camera_pan(double pan_x, double pan_y, double zoom, double dx, double dy);
extern char* glaspen2_camera_zoom_at(double pan_x, double pan_y, double zoom, double factor, double vx, double vy);
extern char* glaspen2_camera_zoom_at_center(double pan_x, double pan_y, double zoom, double factor, double viewport_w, double viewport_h);
extern char* glaspen2_camera_centered(double bx, double by, double bx2, double by2, double viewport_w, double viewport_h, double zoom);
// 标量(高频拖动/键盘步长, 避免走 JSON/堆分配)。
extern double glaspen2_camera_step_zoom(double zoom);
extern double glaspen2_camera_keyboard_pan_step(double zoom);
// 回原点 + 100%(out 参数可传 NULL)。
extern void glaspen2_camera_home(double *pan_x, double *pan_y, double *zoom);
// D6 网格/分栏几何桥(core/grid.rs): 输入视图尺寸/镜头/网格设置, 返回 core 算好的
// 线段表 JSON({"minor":[...],"screen":[...],"divider":[...],"follow":bool} 或
// {"error":"..."}), 线段是视图坐标(原点左上, y 向下), ObjC 只按既有 CGContext
// 样式描边。glaspen2_grid_json = 全视口规范入口(屏幕单位=视口尺寸, macOS 两种画布
// 模式都画屏幕分界线); glaspen2_grid_json_clipped = 笔迹脏矩形热路径(只算与 clip
// 相交的线, 额外带 show_grid 开关)。永不 NULL;调用方用 glaspen2_free_c_string 释放。
extern char* glaspen2_grid_json(double view_w, double view_h, double pan_x, double pan_y, double zoom, double grid_size, int divider, int follow);
extern char* glaspen2_grid_json_clipped(double view_w, double view_h, double pan_x, double pan_y, double zoom, double grid_size, int divider, int show_grid, double clip_x0, double clip_y0, double clip_x1, double clip_y1);
extern void glaspen2_chat_auth_reload(void); // 设置/启动时把 DB 账号配置推给 Rust auth
extern void glaspen2_share_ink_set_active(int active); // 共享画布上行开关(export.rs)
// S2 双向互调探针(env 门控, 默认 no-op; 定义在 src/macos/swift/InteropProbe.m)
extern void glaspen2_macos_swift_probe_maybe(void);
// (共享画布上行已无 ObjC 入口:生命周期完全由面板 tab 驱动,FRB 直达 Rust)

// Modeler FFI
extern void glaspen2_modeler_begin(double r, double g, double b, double x, double y, double pressure, double timestamp, double width_scale);
extern void glaspen2_modeler_move(double x, double y, double pressure, double timestamp, double width_scale);
extern void glaspen2_modeler_end(double x, double y, double pressure, double timestamp, double width_scale);
extern int glaspen2_modeler_point_count(void);
extern void glaspen2_modeler_get_point(int idx, double *x, double *y, double *w);
extern void glaspen2_modeler_clear_buffer(void);
extern void glaspen2_modeler_commit_to_strokes(double r, double g, double b);

// 气泡消息「冷启动」分段的笔状态喂入(bubbles.rs):
// 活动心跳 = 悬空移动/落笔/拖动/抬笔;pen_gone = proximity 离开;
// pen_back = proximity 进入(极近的退+进 = 笔↔橡皮切换,撤销冷标记)
extern void glaspen2_bubble_set_capture(int on);
extern void glaspen2_bubble_pen_activity(void);
extern void glaspen2_bubble_pen_back(void);
extern void glaspen2_bubble_pen_gone(void);
// 「写完自动新建一页」: 落笔瞬间调用, 若这笔将开新消息则建页并切 current id,
// 返回新页 id(0 = 无动作); shell 据此翻页载入(见 run.m bubble_auto_page_flip)
extern long long glaspen2_bubble_auto_page_if_due(void);
extern void glaspen2_modeler_erase_finish(void);
extern int glaspen2_stroke_bbox(double *x_min, double *y_min, double *x_max, double *y_max);
extern void glaspen2_save_svg(void);
extern char* glaspen2_get_cropped_svg(void);
extern void glaspen2_free_c_string(char *ptr);
// D9c 实时绘制计划桥(core export/mod.rs): 本段笔迹 JSON → 单行 DrawPlan JSON
// (commands / dirty / seam; 或 {"error":...}), 永不 NULL;
// 返回值用 glaspen2_free_c_string 释放。ObjC 侧光栅化见 parts/panel.m 的
// gl_live_plan_draw_segment(失败回退旧逐段自绘)。
extern char* glaspen2_live_plan_json(const char *input);
// D10b 文案桥(core/i18n.rs): 按稳定 key 取当前语言文案, 未知 key 回显 key
// 本身(便于发现拼错), 返回缓冲用 glaspen2_free_c_string 释放; 每次调用现查
// 语言, 因此切换语言后新提示/菜单立即生效(见 gl_text/gl_textf)。
extern char* glaspen2_i18n(const char *msg_key, const char *lang);
extern int glaspen2_save_gif_cropped(const unsigned char *surface_data, int w, int h, int stride, double surface_scale);
extern int glaspen2_save_animated_gif(int fps, double resolution, double speed, int end_mode);
extern unsigned char * glaspen2_gif_record_end(int start_index, int end_index, int fps, double resolution, double speed, int end_mode, int *out_len);
extern void glaspen2_draw_rebuild(void *surface_ptr, double scale);
extern void glaspen2_page_dims(long screen_id, int *w, int *h);
extern void glaspen2_draw_rebuild_view(void *surface_ptr, double scale, double ox, double oy, double pscale);
extern int glaspen2_paint_page_into_surface(void *surface_ptr, long screen_id, double scale, int white_bg);
extern int glaspen2_export_pdf(void);
// 无限画布导出(独立存储):分页 PDF / 整幅 SVG
extern int glaspen2_export_infinite_pdf_paged(int page_w, int page_h);
extern int glaspen2_export_infinite_svg(void);
extern void glaspen2_on_display_change(int screen_w, int screen_h);
// D7 缩略图条布局桥(core/minimap.rs): 输入当前页/视图尺寸/几何组过滤宽度,
// 返回 core 算好的页窗 + 条带布局 JSON({"strip_x","strip_y","strip_w","strip_h",
// "items":[{"screen_id","index","y","h","label","current"},...]} 或
// {"error":"..."}), 坐标是视图坐标(原点左上, y 向下; 合成条带时翻转 y)。
// geometry_width > 0 = page-view-fit 模式, 只保留当前几何组(按页宽过滤)。
// 永不 NULL; 调用方用 glaspen2_free_c_string 释放。
extern char* glaspen2_minimap_layout_json(long long current_screen_id, double view_w, double view_h, int geometry_width);
extern unsigned char* glaspen2_render_thumbnail(long long screen_id, int w, int h, int max_size, int *out_len);
/// 批量缩略图:一次调用返回多页的 PNG(自描述二进制块,见 export.rs)。
extern unsigned char* glaspen2_page_thumbnails(const long long *ids, int count, int max_size, int *out_len);
extern void glaspen2_free_rust_bytes(unsigned char *ptr, int len);
/// 设置面板改为 flutter_rust_bridge 通信:菜单/快捷键改了状态后通知 Rust,
/// 由 Rust 推给订阅了设置流的 Dart 侧(取代旧的 MethodChannel 回调)。
extern void glaspen2_notify_settings_changed(void);
extern int glaspen2_delete_screen(long long screen_id);
extern char* glaspen2_page_info_text(long long screen_id, const char *lang);
// chord 单源(core/chord.rs, D2): 动作表/默认值/规范化/保留/旧默认迁移
extern char* glaspen2_chord_actions_json(void);
extern char* glaspen2_chord_defaults_json(void);
extern char* glaspen2_chord_normalize(const char *raw);
extern char* glaspen2_chord_migrate_value(const char *action, const char *raw);
extern int glaspen2_chord_reserved(const char *chord);
extern int glaspen2_chat_send_strokes(int start_index, int end_index);
// 手写消息草稿通道(⌘⌃2):开启/关闭 DraftInk gRPC 流,实现见 export.rs
extern int glaspen2_ink_draft_start(int canvas_w, int canvas_h);
extern int glaspen2_ink_draft_stop(void);
extern const char *glaspen2_ink_draft_last_error(void);
// D8b 录制会话状态机桥(core/recording.rs): ObjC 不能持 Rust 结构体, 状态机
// 活在 core 的两个槽位里(adapter 只在主 runloop 线程调用, 后台结果回主线程再喂回):
//   槽 0 = GIF(⌘⌃R);槽 1 = 手写消息(⌘⌃2 草稿 / ⌘⌃3 直发, 同一机器内互斥)。
// glaspen2_recording_event_json 喂单行事件 JSON, 回 must-execute-in-order 的
// effects JSON 数组(单行, 永不 NULL, 非法输入回 {"error":...})。
// glaspen2_recording_mode 回 0=空闲/1=GIF/2=草稿/3=直发(-1=非法槽位),
// 供 key-up 分支判断"这次抬起是否属于本槽位"。
// 字符串一律用 glaspen2_free_c_string 释放。
extern int glaspen2_recording_init(int slot, int chat_integration);
extern char* glaspen2_recording_event_json(int slot, const char *event_json);
extern int glaspen2_recording_reset(int slot);
extern int glaspen2_recording_mode(int slot);
extern void glaspen2_set_stroke_outline(int enabled);
static void apply_infinite_canvas(BOOL on, BOOL notify);

// S7ak: 镜头 JSON 逻辑已迁 Swift(CameraState), 以下为 ObjC 转发。
// ObjC 侧使用的 Swift 薄桥前向声明(Bridging.h 只给 Swift 编译单元)
void glaspen2_router_update_status_icon_state(void);
extern void glaspen2_set_soft_shadow(int enabled);
extern void glaspen2_set_stroke_invert(int enabled);      // 反色突出开关
extern void glaspen2_set_invert_background(void *surface); // 反相背景 cairo surface
extern int  glaspen2_has_strokes(void);                    // 捕获循环空转判断
void invert_ink_apply(int on);                             // 定义在 parts/panel.m
void invert_stream_restart(void);                          // 帧率变更后重启捕获流
void gl_panel_transparent_apply_full(int on);             // 面板真穿透开关(定义在 parts/panel.m)
extern void glaspen2_set_invert_dirty(int x, int y, int w, int h); // 局部重绘脏区
extern unsigned char* glaspen2_render_canvas_overview(double bx, double by, double bw, double bh, int out_w, int out_h, int *out_len);
extern void glaspen2_set_view_transform(double pan_x, double pan_y, double zoom);
// 画布存储切换 + 无限画布(独立存储,全局仅一个画布)
extern void glaspen2_set_canvas_kind(int infinite);
extern int glaspen2_load_infinite_strokes(void);
extern void glaspen2_set_infinite_transform(double pan_x, double pan_y, double zoom);
extern void glaspen2_get_infinite_transform(double *pan_x, double *pan_y, double *zoom);

// ── 翻页动效(时光隧道)的页快照 ──
// 预热缓存: 从 center 起前后若干页一次载入+平滑(不切当前页、不动 STROKES),
// 逐帧渲染走缓存而不是每帧查库。返回载入的页数。
extern int glaspen2_debug_insert_pages(int n, int w, int h);
extern int glaspen2_preload_flip_pages(long center, int going_next, int before, int after);
// 把缓存里第 slot 页画进 cairo 表面(透明底)。scale = 视口 backing scale;
// ox/oy/pscale = scale-to-fit 的"页像素→逻辑点"变换(不 fit 传 0,0,1);
// alpha = 整页不透明度(渐显/渐隐); white_bg = 白底(导出用)。返回 1 成功。
// depth = 该页在隧道里的页距(0=当前页); >=0 时快照被磨砂玻璃化
// (越深越暗/越模糊, 层次一眼可读), <0 = 不磨砂(原样)。
extern int glaspen2_paint_preview_into_surface(void *surface_ptr, int slot, double scale,
                                               double ox, double oy, double pscale,
                                               double alpha, int white_bg, double depth);

// Page navigation FFI
extern long glaspen2_prev_screen_id(void);
extern long glaspen2_next_screen_id(void);
extern long glaspen2_create_next_page(long current_id); // 末页后新建一页(守卫同新建画布)
extern long glaspen2_get_current_screen_id(void);
// 某页相邻的第 n 页(n>0 向后/较新, n<0 向前/较旧; 0 = 自身)。
// 页按分辨率分组, 与 prev/next 同一邻接语义。找不到返回 0。
extern long glaspen2_page_neighbor(long screen_id, int n);
extern int glaspen2_load_strokes_for_screen(long screen_id);
extern void glaspen2_smooth_loaded_strokes(void);
extern int  glaspen2_set_launch_at_login(int enable);
extern int  glaspen2_is_launch_at_login(void);
extern int glaspen2_stroke_count(void);
extern int glaspen2_get_stroke_point_count(int idx);
// 笔预设/设置钳制(glaspen-core presets 单一事实源)
extern double glaspen2_pressure_raw_width(double pressure, double width_scale);
extern int glaspen2_color_preset_count(void);
extern void glaspen2_color_preset_rgb(int i, double *r, double *g, double *b);
extern int glaspen2_width_preset_count(void);
extern double glaspen2_width_preset_value(int i);
extern int glaspen2_nearest_color_index(double r, double g, double b);
extern int glaspen2_nearest_width_index(double w);
extern double glaspen2_clamp_setting_double(const char *key, double v);
extern int glaspen2_clamp_setting_int(const char *key, int v);
extern void glaspen2_get_stroke_color(int idx, double *r, double *g, double *b);
extern double glaspen2_get_stroke_avg_width(int idx);
extern void glaspen2_get_stroke_point(int idx, int pidx, double *x, double *y);
extern double glaspen2_get_stroke_point_width(int idx, int pidx);
extern int glaspen2_undo_last_stroke(void);

// Forward declarations
static void rebuild_surface_from_strokes(void);
void ensure_surface(void *view);
static BOOL perform_hotkey(unsigned short keyCode);
static void event_tap_install(BOOL include_scroll);
static CGEventRef event_tap_callback(CGEventTapProxy proxy, CGEventType type,
                                     CGEventRef event, void *refcon);
// 虚拟笔合成事件的标记。这类事件投在 HID 事件口上, 一旦放行就会被系统当
// 鼠标用(悬停移动光标、点击落到前台应用) —— 处理完必须一律吞掉。
#define kVirtualPenUserData 0x56504E31LL
// S6-3-3: 判定逻辑已迁 Swift(EventRouter.isVirtualPenEvent); 常量经薄桥暴露。
// 性能日志(drawRect 用得到, 实现在文件后部)
extern void dirty_reset(void);
extern void dirty_include_point(double x, double y, double r);
extern void dirty_include_surface_point(double x, double y, double r);
extern void dirty_include_surface_rect(double x, double y, double w, double h);
int gl_dirty_has_state(void);
double gl_dirty_rect_x(void);
double gl_dirty_rect_y(void);
double gl_dirty_rect_w(void);
double gl_dirty_rect_h(void);
extern void *cairo_create_scaled(void); // Swift @_cdecl, 返回 cairo_t*
extern double canvas_input_x(double view_x);
extern double canvas_input_y(double view_y);
extern void save_and_exit(int sig);
double glaspen2_router_zoom(void);
double glaspen2_router_width_scale(void);
double glaspen2_router_pan_x(void);
double glaspen2_router_pan_y(void);
double glaspen2_router_screen_w(void);
double glaspen2_router_screen_h(void);
int glaspen2_router_lang(void);
void glaspen2_router_set_lang(int value);
void glaspen2_router_set_screen_size(int w, int h);
void *glaspen2_router_draw_view(void);
id glaspen2_router_menu(void);
id glaspen2_router_glass_view(void);
id glaspen2_router_main_window(void);
id glaspen2_router_settings_engine(void);
id glaspen2_router_settings_vc(void);
id glaspen2_router_settings_window(void);
char *sw_text(const char *key);
void *sw_status_item(void);
static void event_tap_reinstall(void);

// --- Drawing state ---

// 页视图变换(scale-to-fit): 当前页几何 ≠ 屏幕时, 等比缩放居中观看该页
// (页 = 某时刻玻璃的几何快照); 同几何时恒为恒等 —— 纯玻璃, 严格 1:1。
// pageview_update() 定义在 parts/run.m, 每次载入某页后调用。
// 翻页动效: 0=macOS 系统动效(默认) 1=时光隧道(自定义, 见 parts/run.m)
static void pageview_update(void);

// Create a cairo context with the backing scale factor applied.
// All drawing coordinates remain in logical points; Cairo renders
// at physical pixel resolution.

// Raw drawing state (for responsive real-time feedback during stroke)
// 描边接缝重画用: 上上点(P_{i-2})与上一段墨迹宽(w_{i-1})。第 i 段的
// 描边圆帽(r_i+pad)会压在已画的第 i-1 段墨迹上, 压力变细时在笔迹里
// 蚀出对比色环(竹节虫)—— 重画第 i-1 段墨迹盖回去。
// 整笔累计弧长(逻辑 px): 黑白虚线描边的相位接续用, 起笔清零

// Track if a stroke is active (modeler has been initialized)

// D8b: GIF / ⌘⌃2 草稿 / ⌘⌃3 直发的会话状态与起止窗口已上收 core
// recording::RecordingMachine(经 glaspen2_recording_* FFI 的两个槽位驱动),
// 本文件不再持 g_gif_recording / g_msg_record_start / g_ink_draft_active ——
// 适配器只保留 GIF 参数、文案与本平台 OS 动作。
// 手写消息集成总开关(设置面板,默认关):关 = ⌘⌃2/⌘⌃3 直通不劫持,
// 登录与画布共享界面隐藏。聊天/共享始终是增强功能,不影响涂鸦本体。

// GIF quality/speed settings (frame rate, resolution multiplier, playback speed)
// GIF ending: 0 = stop on last frame, 1 = hold 1s then loop, 2 = loop immediately

// Eraser (back end of pen) mode — clears pixels instead of drawing ink

// 最近一次 proximity 进入时的设备身份。M808 类驱动把落笔包在鼠标事件里
// (pointingDeviceType 恒为 other), 笔键 "Pen/Eraser" 切换只体现在
// proximity 事件的设备类型上 —— 落笔时用它判橡皮。
// 本会话见过 stylus proximity 事件 = 平台有显式"笔离开"信号。之后真鼠标
// 移动不能再当成笔离开的证据(否则笔还悬在板上、动一下鼠标就会给气泡
// 分段设冷); 只在完全没有 proximity 的平板上才走鼠标回退。

// Active cairo context (reused across pen events during a stroke).
// Created on pen-down, destroyed on pen-up. Avoids per-event malloc/free
// of cairo_t and avoids the CTM scale setup cost.

// 滚轮劫持修复:滚轮事件只在无限画布模式下进入 tap;
// 翻页模式重装不含滚轮的 tap,系统滚轮零拦截。

// Dirty-rect tracking (logical points, view coordinate space — origin bottom-left).
// Pen events union their affected area into g_dirty_rect and invalidate it
// with setNeedsDisplayInRect; drawRect then only copies that sub-region
// from the cairo surface to the screen, instead of the full screen each vsync.
// Include a point given in SURFACE coordinates (origin top-left).
// The view is non-flipped (origin bottom-left), so flip Y first.
// 表面像素坐标的矩形 → view 坐标脏区(反色局部重绘的 blit 用)
// Cursor state

// Crosshair hover redraw throttle: flush at most every ~16 ms; the dirty
// rect accumulates intermediate positions so no ghost trails appear.

// System cursor hidden while pen is drawing (file scope so toggle_enabled
// can restore it when the app is disabled mid-stroke).

// Restore the system cursor if it is currently hidden by pen drawing.
// Pressure monitor

// Pen color state

// Width scale presets
// 8 档线宽倍率:数值单源在 glaspen-core presets,启动时经 FFI 填充
// (g_width_preset_count 须与 core WIDTH_PRESETS 一致,core 测试守着)。

// 网格大小(逻辑 px),设置面板可调,默认 40

// 网格分栏参考线(纯视觉,无功能含义,独立于显示网格开关):
// 0=无 1=左右两栏 2=上下两栏 3=九宫格 4=康奈尔。
// 实现 = 把每屏单位 1/2(两栏)或 1/3、2/3(九宫格)或 30%/75%(康奈尔)
// 位置上最近的网格线加粗一档。

// ── 页面缩略图条(minimap,翻页模式) ──
// 页窗 + 条带几何的唯一来源 = core::minimap(经 FFI glaspen2_minimap_layout_json);
// 这里只缓存最近一次布局 JSON(当前页/视图尺寸/几何组过滤宽度变化时重取)。
// 缩略图队列/条带合成/描边/页号字体仍在 run.m 的 draw_minimap。

// ── 无限画布模式 ──
// 画布坐标 = 视口坐标 + pan(pan = 视口左上角在画布坐标系中的位置)。
// 模式开启时 ⌘⌃滚轮移动镜头,笔迹以画布坐标存储(可为负/超界)。
// 翻页模式的滚动偏移:整页翻页后恒为 0(变量保留,canvas_input/drawRect
// 的变换式以此为恒等项;不再有任何滑动写入路径)。
// ── D5b: 镜头数学唯一来源是 core::camera(经 FFI);本文件只持有状态 ──
//
// 下面的 helper 只做两件事: 释放 Rust 缓冲区 + 把 core 算好的三值写回全局。
// 模式门控(仅无限画布/书写中忽略)、提示文案与 1.5s 节流、持久化节流、
// 飘渺 peek、重绘都留在各调用点(adapter);两端真实差异(键盘平移方向符号、
// 最小缩放提示仅 Windows、居中 zoom 回写、载入不钳上限)见 camera.rs 模块文档。

// 解析 Rust 返回的镜头 JSON 并释放缓冲区;形状不对/error JSON 返回 nil。


// 把 core 算好的三值写回全局。assign_zoom = NO 时保留当前 g_zoom(macOS
// 居中只改 pan;Windows 会把兜底后的 zoom 写回, 差异见 camera.rs 模块文档 5)。
// 字段缺失(含 error JSON)返回 NO 且不改状态。


// 页视图变换(scale-to-fit): 当前页几何 ≠ 屏幕时, 等比缩放居中观看该页
// (页 = 某时刻玻璃的几何快照); 同几何时恒为恒等 —— 纯玻璃, 严格 1:1。


// 输入坐标(视图/屏幕逻辑点)→ 画布坐标。
// 渲染是 view = (canvas − pan) × zoom,所以 canvas = view / zoom + pan。
// 翻页模式渲染时镜头恒为原点(page_off 恒 0,整页翻页不滚动),无限画布则是 g_pan。
// 注意:此前输入写成 `+ g_page_off`,符号反了 —— 在两张之间涂鸦后一移动就跳位。
// Grid overlay toggle (default off)

// When YES the grid follows the strokes and is hidden with them by
// 飘渺画布涂鸦模式 (X). When NO the grid is always visible while its own
// Flutter switch is on. Controlled by the Flutter settings panel.

// When YES the frosted-glass backdrop is hidden with the strokes by
// 飘渺画布涂鸦模式 (X) — the historical behavior. When NO the glass stays
// visible regardless of stroke visibility. Controlled by the Flutter panel.

// 笔迹描边(渲染设置):仅在内存,不落库,重启恢复关闭。
// 软阴影(独立于描边可叠加): 笔迹下方三档加宽递减 alpha 的黑影,
// 同色背景上靠亮度分离保底可见。Controlled by the Flutter settings panel.
// 反色突出(实验): 墨迹逐像素取背景反色。背景由 SCScreenshotManager 持续
// 捕获(排除自身窗口)并反相, 经 cairo pattern 作为墨迹 source。
// 反色追踪帧率上限(10/30/60/100): SCStream minimumFrameInterval。
// 流本身只在画面变化时送帧, 此值只是上限; 静态背景零帧零开销。
// 描边比笔迹宽出的半径(逻辑 px),与 Rust OUTLINE_PAD 保持一致。

// Glass overlay opacity (0.0 = off, 0.0-0.3 range)

// Color presets
typedef struct { double r, g, b; } ColorPreset;
// RGB 单源在 glaspen-core presets, 启动时经 FFI 填充(core 测试守着色值与
// 数量一致); 显示名文案单源在 core::i18n(D10b, 经 gl_text 按 key 取)。

// Notification state
// S4: 通知状态/计时已迁 Swift(NotificationPresenter); 绘制代码经 currentText 读取。

// CGEventTap

// Language: 0=Chinese, 1=English

// ── D10b 共享文案接线: 两端语义相同的用户可见文案单源在 core i18n 表 ──
// 遗留的 L(@"zh", @"en") 只用于单平台独有/两端有意不同的字符串(见
// i18n.rs 模块文档的收录边界); 共享文案一律走 gl_text/gl_textf, 由稳定 key
// 现查表。

/// 当前语言代码(只有 "en" 是英文, 其余中文; 与 core::i18n::Lang 一致)。
static const char *gl_lang_code(void) {
    return glaspen2_router_lang() == 1 ? "en" : "zh";
}

/// 稳定 key → 本地化文案(共享文案的唯一入口)。
/// 不缓存文案: 每次调用现读 g_lang, 语言切换后菜单重建/新提示立即是新语言。
static NSString *gl_text(NSString *key) {
    const char *c_key = [key UTF8String];
    if (!c_key) return key;
    char *c_text = sw_text(c_key);
    if (!c_text) return key;
    NSString *text = [NSString stringWithUTF8String:c_text];
    free(c_text);
    return text.length ? text : key;
}

/// 带占位符的文案: {name} 按字典替换(未提供的占位符原样保留, 与
/// core::i18n::Msg::format 同语义)。平台键名等差异由调用方作为参数传入。

// S4: presenter redraw 桥(Swift 通知状态变化时调用)
void glaspen2_notification_redraw(void) {
    [(__bridge NSView *)glaspen2_router_draw_view() setNeedsDisplay:YES];
}

static void show_notification(NSString *text) {
    [NotificationPresenter showText:text];
}

// 页信息 HUD: 已迁 Swift(NotificationPresenter.showPageInfoWithScreenId:);
// 文案与日期比较单源仍在 core::pageinfo。
static void save_drawing_only(void) { [SurfaceOps saveDrawingOnly]; }

/// 当前覆盖层所在屏幕的 CGDirectDisplayID。
/// 截图曾经永远取 `content.displays.firstObject` —— 多屏时会把副屏的
/// 桌面合成进主屏的笔迹里。
/// macOS 12/13 的桌面截图回退。
///
/// SDK 从 macOS 15 起把 CGWindowListCreateImage 标成 unavailable(运行时
/// 在 12/13 仍然存在), 所以这里用 dlsym 取符号, 避免编译期被拒。
/// 取"覆盖层下方的窗口", 不会把当前笔迹再截一遍。
static void save_with_background(void) { [SaveBackground save]; }


static void clear_screen(void) { [SurfaceOps clearScreen]; }

// 快照观看的页框: 页外区域压暗 + 页边界细线(物理像素坐标, 不吃变换)。
static void pageview_frame_draw(void) { [SurfaceReplay drawPageFrame]; }
static void pageview_frame_draw(void);
static void replay_strokes_from_memory(void) { [SurfaceReplay replay]; }

// ── 文件拆分: #include 文本包含(同一编译单元, clang 报错行号直指子文件);
//    拆分为纯移动, 行为零变化。构建仍只编译 glaspen2.m 这一个入口。──
