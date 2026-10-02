#import <Cocoa/Cocoa.h>
#import <QuartzCore/QuartzCore.h>
#import <Carbon/Carbon.h>
#import <ScreenCaptureKit/ScreenCaptureKit.h>
#import <CoreMedia/CoreMedia.h>
#import <CoreVideo/CoreVideo.h>
#import <IOSurface/IOSurface.h>
#import <FlutterMacOS/FlutterMacOS.h>
#include <signal.h>
#include <string.h>
#include <stdlib.h>
#include <mach/mach_time.h>
#include <fcntl.h>     // open/O_CREAT —— 单实例 flock 锁
#include <sys/file.h>  // flock

// App enabled state
static BOOL g_enabled = YES;

// Screen dimensions in logical points (set once at startup)
static int g_screen_w = 1920;
static int g_screen_h = 1080;

// Backing scale factor for Retina rendering (1.0 = non-Retina, 2.0 = Retina)
static CGFloat g_scale = 1.0;

// Forward declarations
static void flush_to_layer(void);
static void clear_screen(void);
static void draw_rainbow_indicator(void);
static void rebuild_surface_from_strokes(void);
static void show_settings_panel(void);
static void sync_settings_panel(void);
static void gl_settings_set_color(int idx);
static void gl_settings_set_width(int idx);
static void gl_settings_set_rainbow(BOOL on);
static void perf_log_summary(void);
static void gl_settings_set_launch(BOOL on);
static void gl_settings_set_glass_enabled(BOOL on);
static void gl_settings_set_glass_opacity(double alpha);
static void gl_settings_set_grid(BOOL on);
static void gl_settings_set_pressure_monitor(BOOL on);
static void pm_ensure_window(void);
static void pm_show(void);
static void pm_hide(void);
static void pm_destroy(void);
static void pm_update(void);
static void gl_glass_apply(void);
static void gl_settings_set_enabled(BOOL on);
static void toggle_enabled(void);
static void update_status_icon_state(void);
static void ink_draft_stop_async(void); // 定义在 ink draft 区(总开关要用)
static BOOL g_strokes_visible; // 定义在飘渺模式区(总开关恢复 V 时要用)
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
extern void glaspen2_end_stroke(void);
extern void glaspen2_save_xoj(void);
extern int glaspen2_clear_strokes(int screen_w, int screen_h);
extern void glaspen2_init_db(int screen_w, int screen_h);
extern void glaspen2_save_settings(double r, double g, double b, double width_scale);
extern int  glaspen2_load_settings_parts(double *r, double *g, double *b, double *w);
extern void glaspen2_save_bool_setting(const char *key, int val);
extern int  glaspen2_load_bool_setting(const char *key);
extern void glaspen2_save_string_setting(const char *key, const char *value);
extern char* glaspen2_load_string_setting(const char *key);
extern void glaspen2_chat_auth_reload(void); // 设置/启动时把 DB 账号配置推给 Rust auth
extern void glaspen2_share_ink_set_active(int active); // 共享画布上行开关(export.rs)
// (共享画布上行已无 ObjC 入口:生命周期完全由面板 tab 驱动,FRB 直达 Rust)

// Modeler FFI
extern void glaspen2_modeler_begin(double r, double g, double b, double x, double y, double pressure, double timestamp, double width_scale);
extern void glaspen2_modeler_move(double x, double y, double pressure, double timestamp, double width_scale);
extern void glaspen2_modeler_end(double x, double y, double pressure, double timestamp, double width_scale);
extern int glaspen2_modeler_point_count(void);
extern void glaspen2_modeler_get_point(int idx, double *x, double *y, double *w);
extern void glaspen2_modeler_clear_buffer(void);
extern void glaspen2_modeler_commit_to_strokes(double r, double g, double b);
extern void glaspen2_modeler_erase_finish(void);
extern int glaspen2_stroke_bbox(double *x_min, double *y_min, double *x_max, double *y_max);
extern void glaspen2_save_svg(void);
extern char* glaspen2_get_cropped_svg(void);
extern void glaspen2_free_c_string(char *ptr);
extern int glaspen2_save_gif_cropped(const unsigned char *surface_data, int w, int h, int stride, double surface_scale);
extern int glaspen2_save_animated_gif(int fps, double resolution, double speed, int end_mode);
extern unsigned char * glaspen2_gif_record_end(int start_index, int end_index, int fps, double resolution, double speed, int end_mode, int *out_len);
extern void glaspen2_draw_rebuild(void *surface_ptr, double scale);
extern int glaspen2_export_pdf(void);
// 无限画布导出(独立存储):分页 PDF / 整幅 SVG
extern int glaspen2_export_infinite_pdf_paged(int page_w, int page_h);
extern int glaspen2_export_infinite_svg(void);
extern void glaspen2_on_display_change(int screen_w, int screen_h);
extern char* glaspen2_list_screens_json(void);
extern unsigned char* glaspen2_render_thumbnail(long long screen_id, int w, int h, int max_size, int *out_len);
/// 批量缩略图:一次调用返回多页的 PNG(自描述二进制块,见 export.rs)。
extern unsigned char* glaspen2_page_thumbnails(const long long *ids, int count, int max_size, int *out_len);
extern void glaspen2_free_rust_bytes(unsigned char *ptr, int len);
/// 设置面板改为 flutter_rust_bridge 通信:菜单/快捷键改了状态后通知 Rust,
/// 由 Rust 推给订阅了设置流的 Dart 侧(取代旧的 MethodChannel 回调)。
extern void glaspen2_notify_settings_changed(void);
extern int glaspen2_delete_screen(long long screen_id);
extern char* glaspen2_page_info_json(long long screen_id);
extern int glaspen2_chat_send_strokes(int start_index, int end_index);
// 手写消息草稿通道(⌘⌃2):开启/关闭 DraftInk gRPC 流,实现见 export.rs
extern int glaspen2_ink_draft_start(int canvas_w, int canvas_h);
extern int glaspen2_ink_draft_stop(void);
extern const char *glaspen2_ink_draft_last_error(void);
extern void glaspen2_set_stroke_outline(int enabled);
extern unsigned char* glaspen2_render_canvas_overview(double bx, double by, double bw, double bh, int out_w, int out_h, int *out_len);
extern void glaspen2_set_view_transform(double pan_x, double pan_y, double zoom);
// 画布存储切换 + 无限画布(独立存储,全局仅一个画布)
extern void glaspen2_set_canvas_kind(int infinite);
extern int glaspen2_load_infinite_strokes(void);
extern void glaspen2_set_infinite_transform(double pan_x, double pan_y, double zoom);
extern void glaspen2_get_infinite_transform(double *pan_x, double *pan_y, double *zoom);

// Page navigation FFI
extern long glaspen2_prev_screen_id(void);
extern long glaspen2_next_screen_id(void);
extern long glaspen2_get_current_screen_id(void);
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
static void finish_active_stroke(void);
static void ensure_surface(NSView *view);
static BOOL perform_hotkey(unsigned short keyCode);
static void apply_outline(BOOL on);
static void apply_infinite_canvas(BOOL on, BOOL notify);
static void canvas_infinite_load(void);
static void canvas_infinite_persist(void);
static void event_tap_install(BOOL include_scroll);
static CGEventRef event_tap_callback(CGEventTapProxy proxy, CGEventType type,
                                     CGEventRef event, void *refcon);
// 虚拟笔合成事件的标记。这类事件投在 HID 事件口上, 一旦放行就会被系统当
// 鼠标用(悬停移动光标、点击落到前台应用) —— 处理完必须一律吞掉。
#define kVirtualPenUserData 0x56504E31LL
static inline BOOL virtual_pen_is_event(CGEventRef event) {
    return CGEventGetIntegerValueField(event, kCGEventSourceUserData) == kVirtualPenUserData;
}
static void draw_minimap(CGContextRef ctx, NSRect bounds);
static void canvas_reset_lens(void);
static void canvas_apply_transform(void);
// 性能日志(drawRect 用得到, 实现在文件后部)
static BOOL g_perf_log = NO;
static void perf_log_event_notes(const char *evtype, uint64_t dur_us, const char *notes);
static uint64_t elapsed_us(uint64_t start);
static void event_tap_reinstall(void);
static NSWindow *g_window = nil;
static NSVisualEffectView *g_glass_view = nil;

// --- Drawing state ---
static cairo_surface_t *g_surface = NULL;

// Create a cairo context with the backing scale factor applied.
// All drawing coordinates remain in logical points; Cairo renders
// at physical pixel resolution.
static inline cairo_t *cairo_create_scaled(void) {
    cairo_t *cr = cairo_create(g_surface);
    cairo_scale(cr, g_scale, g_scale);
    return cr;
}

static double g_last_x = -1, g_last_y = -1;
static BOOL g_has_last = NO;
static NSView *g_draw_view = nil;

// Raw drawing state (for responsive real-time feedback during stroke)
static double g_raw_last_x = 0, g_raw_last_y = 0;
static BOOL g_raw_has_last = NO;

// Track if a stroke is active (modeler has been initialized)
static BOOL g_stroke_active = NO;

// Quick GIF recording: true between the Cmd+Ctrl+R key-down and key-up.
// g_gif_record_start is the stroke count at key-down, pinning the window.
static BOOL g_gif_recording = NO;
static int g_gif_record_start = -1;
// Handwriting message recording (Cmd+Ctrl+3 hold-to-write): the stroke
// window [start, end) pinned at key-down/key-up is sent as one batch of
// chat messages when the key is released.
static int g_msg_record_start = -1;
// Handwriting DRAFT channel (Cmd+Ctrl+2 hold-to-write): strokes stream live
// over a gRPC DraftInk session while held; on release the stream closes and
// the axum side decides whether to send. Mutually exclusive with ⌘⌃3.
static BOOL g_ink_draft_active = NO;
// 手写消息集成总开关(设置面板,默认关):关 = ⌘⌃2/⌘⌃3 直通不劫持,
// 登录与画布共享界面隐藏。聊天/共享始终是增强功能,不影响涂鸦本体。
static BOOL g_chat_integration = NO;

// GIF quality/speed settings (frame rate, resolution multiplier, playback speed)
static int g_gif_fps = 15;
static double g_gif_resolution = 0.5;
static double g_gif_speed = 2.0;
// GIF ending: 0 = stop on last frame, 1 = hold 1s then loop, 2 = loop immediately
static int g_gif_end_mode = 1;

// Eraser (back end of pen) mode — clears pixels instead of drawing ink
static BOOL g_eraser_mode = NO;

// Active cairo context (reused across pen events during a stroke).
// Created on pen-down, destroyed on pen-up. Avoids per-event malloc/free
// of cairo_t and avoids the CTM scale setup cost.
static cairo_t *g_active_cr = NULL;

// 滚轮劫持修复:滚轮事件只在无限画布模式下进入 tap;
// 翻页模式重装不含滚轮的 tap,系统滚轮零拦截。
static BOOL g_tap_has_scroll = NO;

// Dirty-rect tracking (logical points, view coordinate space — origin bottom-left).
// Pen events union their affected area into g_dirty_rect and invalidate it
// with setNeedsDisplayInRect; drawRect then only copies that sub-region
// from the cairo surface to the screen, instead of the full screen each vsync.
static BOOL g_dirty_has = NO;       // any dirty region pending?
static NSRect g_dirty_rect;          // union rect in view points
static inline void dirty_reset(void) { g_dirty_has = NO; }
static inline void dirty_include_point(double x, double y, double r) {
    // Inflate by r (stroke half-width + anti-alias padding) and union.
    NSRect add = NSMakeRect(x - r, y - r, 2 * r, 2 * r);
    if (!g_dirty_has) { g_dirty_rect = add; g_dirty_has = YES; }
    else g_dirty_rect = NSUnionRect(g_dirty_rect, add);
}
// Include a point given in SURFACE coordinates (origin top-left).
// The view is non-flipped (origin bottom-left), so flip Y first.
static inline void dirty_include_surface_point(double x, double y, double r) {
    double view_h = g_draw_view ? [g_draw_view bounds].size.height : y;
    dirty_include_point(x, view_h - y, r);
}

// Cursor state
static double g_cursor_x = -100, g_cursor_y = -100;
static BOOL g_cursor_visible = NO;
static NSCursor *g_blank_cursor = nil;
static NSCursor *g_arrow_cursor = nil;

// Crosshair hover redraw throttle: flush at most every ~16 ms; the dirty
// rect accumulates intermediate positions so no ghost trails appear.
static uint64_t g_last_cursor_flush = 0;

// System cursor hidden while pen is drawing (file scope so toggle_enabled
// can restore it when the app is disabled mid-stroke).
static BOOL g_pen_drawing = NO;

// Restore the system cursor if it is currently hidden by pen drawing.
static void restore_system_cursor(void) {
    if (g_pen_drawing) {
        CGDisplayShowCursor(kCGDirectMainDisplay);
        g_pen_drawing = NO;
    }
}

// Pressure monitor
static BOOL g_pressure_monitor = NO;
static NSWindow *g_pm_window = nil;
static NSTextField *g_pm_label = nil;
static int g_pm_pressure = 0;
static NSString *g_pm_evtype = nil;
static BOOL g_pm_tip_down = NO;
static BOOL g_pm_in_range = NO;
static CGFloat g_pm_x = 0, g_pm_y = 0;

// Pen color state
static double g_pen_r = 1.0, g_pen_g = 0.0, g_pen_b = 0.0;

// Width scale presets
static double g_width_scale = 1.0;
// 8 档线宽倍率:数值单源在 glaspen-core presets,启动时经 FFI 填充
// (g_width_preset_count 须与 core WIDTH_PRESETS 一致,core 测试守着)。
static double g_width_presets[8];
static const int g_width_preset_count = 8;
static int g_selected_width_index = 3; // default: 1.0x

// 网格大小(逻辑 px),设置面板可调,默认 40
static double g_grid_size = 40.0;

// 网格分栏参考线(纯视觉,无功能含义):0=无 1=左右两栏 2=上下两栏 3=九宫格。
// 实现 = 把每屏单位 1/2(两栏)或 1/3、2/3(九宫格)位置上最近的网格线加粗一档。
static NSInteger g_grid_divider = 0;

// ── 页面缩略图条(minimap,翻页模式) ──
static BOOL g_minimap_enabled = NO;
static NSMutableDictionary *g_minimap_thumbs = nil; // screenId(NSNumber) → NSImage
static NSMutableArray *g_minimap_inflight = nil;     // 正在后台取图的 screenId
static NSArray *g_minimap_ids = nil;                 // 附近页 id 列表(缓存)
static long g_minimap_ids_for = -1;                  // 该列表对应的当前页 id

// ── 无限画布模式 ──
// 画布坐标 = 视口坐标 + pan(pan = 视口左上角在画布坐标系中的位置)。
// 模式开启时 ⌘⌃滚轮移动镜头,笔迹以画布坐标存储(可为负/超界)。
static BOOL g_infinite_canvas = NO;
static double g_pan_x = 0.0, g_pan_y = 0.0;
static double g_zoom = 1.0; // 视图缩放,(0,1],上限 100%
// 翻页模式的滚动偏移:整页翻页后恒为 0(变量保留,canvas_input/drawRect
// 的变换式以此为恒等项;不再有任何滑动写入路径)。
static double g_page_off_x = 0.0, g_page_off_y = 0.0;

// 输入坐标(视图/屏幕逻辑点)→ 画布坐标。
// 渲染是 view = (canvas − pan) × zoom,所以 canvas = view / zoom + pan。
// 翻页模式渲染时镜头恒为原点(page_off 恒 0,整页翻页不滚动),无限画布则是 g_pan。
// 注意:此前输入写成 `+ g_page_off`,符号反了 —— 在两张之间涂鸦后一移动就跳位。
static inline double canvas_input_x(double view_x) {
    if (g_infinite_canvas) return view_x / g_zoom + g_pan_x;
    return view_x - g_page_off_x; // 翻页模式 zoom 恒为 1
}
static inline double canvas_input_y(double view_y) {
    if (g_infinite_canvas) return view_y / g_zoom + g_pan_y;
    return view_y - g_page_off_y; // 翻页模式 zoom 恒为 1
}

// Rainbow indicator toggle (default off)
static BOOL g_show_rainbow = NO;

// Grid overlay toggle (default off)
static BOOL g_show_grid = NO;

// When YES the grid follows the strokes and is hidden with them by
// 飘渺画布涂鸦模式 (X). When NO the grid is always visible while its own
// Flutter switch is on. Controlled by the Flutter settings panel.
static BOOL g_grid_follow_strokes = NO;

// 笔迹描边(渲染设置):仅在内存,不落库,重启恢复关闭。
static BOOL g_outline_enabled = NO;
// 描边比笔迹宽出的半径(逻辑 px),与 Rust OUTLINE_PAD 保持一致。
static const double kOutlinePad = 1.0;

// Glass overlay opacity (0.0 = off, 0.0-0.3 range)
static BOOL g_glass_enabled = NO;  // frosted glass ON/OFF
static double g_glass_opacity = 0.45; // opacity level (used only when enabled)

// Color presets
typedef struct { const char *name; double r, g, b; } ColorPreset;
// RGB 单源在 glaspen-core presets;name 是菜单英文显示(平台 UI 字符串),
// RGB 启动时经 FFI 填充。core 测试守着色值与数量一致。
static ColorPreset g_color_presets[10] = {
    {"Red"}, {"Orange"}, {"Yellow"}, {"Green"}, {"Cyan"},
    {"Blue"}, {"Purple"}, {"Pink"}, {"White"}, {"Black"},
};
static const int g_color_preset_count = 10;

// Notification state
static NSString *g_notification = nil;
static dispatch_source_t g_notification_timer = nil;

// CGEventTap
static CFMachPortRef g_event_tap = NULL;

// Language: 0=Chinese, 1=English
static int g_lang = 0;

static NSString *L(NSString *zh, NSString *en) {
    return g_lang == 0 ? zh : en;
}

static void show_notification(NSString *text) {
    g_notification = text;
    [g_draw_view setNeedsDisplay:YES];

    // Cancel existing timer
    if (g_notification_timer) {
        dispatch_source_cancel(g_notification_timer);
        g_notification_timer = nil;
    }

    // Clear after 1 second
    g_notification_timer = dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER, 0, 0, dispatch_get_main_queue());
    dispatch_source_set_timer(g_notification_timer, dispatch_time(DISPATCH_TIME_NOW, 1 * NSEC_PER_SEC), DISPATCH_TIME_FOREVER, 0);
    dispatch_source_set_event_handler(g_notification_timer, ^{
        g_notification = nil;
        [g_draw_view setNeedsDisplay:YES];
        dispatch_source_cancel(g_notification_timer);
        g_notification_timer = nil;
    });
    dispatch_resume(g_notification_timer);
}

// Date label for the page notification: 今天 / 昨天 / YYYY-MM-DD (local time).
static NSString *page_date_label(double created) {
    NSDate *d = [NSDate dateWithTimeIntervalSince1970:created];
    NSCalendar *cal = [NSCalendar currentCalendar];
    NSUInteger units = NSCalendarUnitYear | NSCalendarUnitMonth | NSCalendarUnitDay;
    NSDateComponents *dc = [cal components:units fromDate:d];
    NSDateComponents *nc = [cal components:units fromDate:[NSDate date]];
    if (dc.year == nc.year && dc.month == nc.month && dc.day == nc.day) {
        return L(@"今天", @"Today");
    }
    NSDate *yesterday = [cal dateByAddingUnit:NSCalendarUnitDay value:-1
                                       toDate:[NSDate date] options:0];
    NSDateComponents *yc = [cal components:units fromDate:yesterday];
    if (dc.year == yc.year && dc.month == yc.month && dc.day == yc.day) {
        return L(@"昨天", @"Yesterday");
    }
    return [NSString stringWithFormat:@"%04ld-%02ld-%02ld",
            (long)dc.year, (long)dc.month, (long)dc.day];
}

// Show "今天 第2页  第3/5页" for the given screen.
static void show_page_info(long long screen_id) {
    char *json = glaspen2_page_info_json(screen_id);
    if (!json) return;
    NSString *s = [NSString stringWithUTF8String:json];
    glaspen2_free_c_string(json);
    NSData *data = [s dataUsingEncoding:NSUTF8StringEncoding];
    NSDictionary *d = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
    if (!d) return;
    long nth = [d[@"nth"] longValue];
    long pos = [d[@"pos"] longValue];
    long total = [d[@"total"] longValue];
    double created = [d[@"created"] doubleValue];
    NSString *label = page_date_label(created);
    show_notification([NSString stringWithFormat:L(@"%@ 第%ld页  第%ld/%ld页",
                                                   @"%@ page %ld (%ld/%ld)"),
                       label, nth, pos, total]);
}

static void save_drawing_only(void) {
    if (!g_surface) return;
    cairo_surface_flush(g_surface);
    const unsigned char *data = cairo_image_surface_get_data(g_surface);
    int w = cairo_image_surface_get_width(g_surface);
    int h = cairo_image_surface_get_height(g_surface);
    int stride = cairo_image_surface_get_stride(g_surface);
    glaspen2_save_drawing(data, w, h, stride);
    show_notification(L(@"截图成功", @"Saved"));
}

static void save_with_background(void) {
    if (!g_surface) return;

    // Copy the cairo surface on the main thread so the background capture
    // never reads live surface memory (data race / use-after-free on rebuild).
    cairo_surface_flush(g_surface);
    int dw = cairo_image_surface_get_width(g_surface);
    int dh = cairo_image_surface_get_height(g_surface);
    int dstride = cairo_image_surface_get_stride(g_surface);
    const unsigned char *dptr = cairo_image_surface_get_data(g_surface);
    if (!dptr || dw <= 0 || dh <= 0 || dstride <= 0) {
        save_drawing_only();
        return;
    }
    unsigned char *drawingCopy = malloc((size_t)dstride * dh);
    if (!drawingCopy) {
        save_drawing_only();
        return;
    }
    memcpy(drawingCopy, dptr, (size_t)dstride * dh);

    // Use ScreenCaptureKit to capture screen
    [SCShareableContent getShareableContentWithCompletionHandler:^(SCShareableContent *content, NSError *error) {
        if (error || !content.displays.count) {
            NSLog(@"[glaspen2] Screen capture failed: %@", error);
            free(drawingCopy);
            dispatch_async(dispatch_get_main_queue(), ^{ save_drawing_only(); });
            return;
        }

        SCDisplay *display = content.displays.firstObject;
        SCContentFilter *filter = [[SCContentFilter alloc] initWithDisplay:display excludingWindows:@[]];
        SCStreamConfiguration *config = [SCStreamConfiguration new];
        config.width = display.width;
        config.height = display.height;

        [SCScreenshotManager captureImageWithFilter:filter configuration:config completionHandler:^(CGImageRef image, NSError *error) {
            if (error || !image) {
                NSLog(@"[glaspen2] Screenshot failed: %@", error);
                free(drawingCopy);
                dispatch_async(dispatch_get_main_queue(), ^{
                    show_notification(L(@"截图失败，已保存涂鸦", @"Screenshot failed, drawing saved"));
                    save_drawing_only();
                });
                return;
            }

            // Use Display P3 color space (matches what user sees on screen)
            CGColorSpaceRef displayP3 = CGColorSpaceCreateWithName(kCGColorSpaceDisplayP3);
            size_t bw = CGImageGetWidth(image);
            size_t bh = CGImageGetHeight(image);

            // Convert screenshot to Display P3
            CGContextRef bgCtx = CGBitmapContextCreate(NULL, bw, bh, 8, bw * 4, displayP3,
                kCGImageAlphaPremultipliedLast);
            CGContextDrawImage(bgCtx, CGRectMake(0, 0, bw, bh), image);
            CGImageRef p3Image = CGBitmapContextCreateImage(bgCtx);
            CGContextRelease(bgCtx);

            // Get screenshot pixel data
            CGDataProviderRef bgProvider = CGImageGetDataProvider(p3Image);
            CFDataRef bgDataRef = CGDataProviderCopyData(bgProvider);
            const unsigned char *bgData = CFDataGetBytePtr(bgDataRef);
            size_t bgStride = CGImageGetBytesPerRow(p3Image);

            // Convert the copied cairo surface (sRGB, BGRA in memory) to Display P3.
            // Cairo ARGB32 on little-endian is B,G,R,A in memory — matches
            // kCGBitmapByteOrder32Little | kCGImageAlphaPremultipliedFirst.
            CGColorSpaceRef srgb = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
            CGDataProviderRef drawProvider = CGDataProviderCreateWithData(NULL,
                drawingCopy, dstride * dh, NULL);
            CGImageRef drawImage = CGImageCreate(dw, dh, 8, 32, dstride, srgb,
                kCGBitmapByteOrder32Little | kCGImageAlphaPremultipliedFirst,
                drawProvider, NULL, false, kCGRenderingIntentDefault);
            CGDataProviderRelease(drawProvider);
            CGColorSpaceRelease(srgb);

            // Draw cairo image into Display P3 context
            CGContextRef drawCtx = CGBitmapContextCreate(NULL, dw, dh, 8, dw * 4, displayP3,
                kCGImageAlphaPremultipliedLast);
            CGContextDrawImage(drawCtx, CGRectMake(0, 0, dw, dh), drawImage);
            CGImageRelease(drawImage);
            CGImageRef p3DrawImage = CGBitmapContextCreateImage(drawCtx);
            CGContextRelease(drawCtx);

            // Get drawing pixel data in Display P3
            CGDataProviderRef p3DrawProvider = CGImageGetDataProvider(p3DrawImage);
            CFDataRef drawDataRef = CGDataProviderCopyData(p3DrawProvider);
            const unsigned char *drawData = CFDataGetBytePtr(drawDataRef);
            size_t drawStride = CGImageGetBytesPerRow(p3DrawImage);

            CGColorSpaceRelease(displayP3);
            free(drawingCopy);

            // Call Rust to composite and save (both in Display P3)
            glaspen2_save_with_background(
                drawData, dw, dh, (int)drawStride,
                bgData, (int)bw, (int)bh, (int)bgStride);

            CFRelease(bgDataRef);
            CFRelease(drawDataRef);
            CGImageRelease(p3Image);
            CGImageRelease(p3DrawImage);
            dispatch_async(dispatch_get_main_queue(), ^{
                show_notification(L(@"截图成功(含背景)", @"Saved (with background)"));
            });
        }];
    }];
}

static void clear_screen(void) {
    if (g_infinite_canvas) {
        // 无限画布只能手动新建,菜单/快捷键不清空
        show_notification(L(@"无限画布不会被清除 · 新建请到设置面板手动新建",
                            @"Infinite canvas is kept — create a new one in settings"));
        return;
    }
    if (!g_surface) return;
    cairo_t *cr = cairo_create_scaled();
    cairo_set_operator(cr, CAIRO_OPERATOR_CLEAR);
    cairo_paint(cr);
    cairo_destroy(cr);
    g_has_last = NO;
    int created = glaspen2_clear_strokes(g_screen_w, g_screen_h);
    if (g_infinite_canvas) {
        // 无限画布只有一个画布:清空内容 + 复位镜头,不新建页。
        if (created) {
            canvas_reset_lens();
            canvas_infinite_persist();
        }
        if (g_show_rainbow) draw_rainbow_indicator();
        flush_to_layer();
        show_notification(created
            ? L(@"已清空无限画布", @"Infinite canvas cleared")
            : L(@"无限画布本来就是空的", @"Infinite canvas is already empty"));
        return;
    }
    if (g_show_rainbow) draw_rainbow_indicator();
    flush_to_layer();
    if (created) {
        show_page_info(glaspen2_get_current_screen_id());
    } else {
        // The current canvas was never edited — don't allow blank-on-blank.
        show_notification(L(@"不能连续创建空白画布, 请先涂鸦", @"Canvas is blank — draw something first"));
    }
}

static void replay_strokes_from_memory(void) {
    if (!g_surface) return;
    // Same as rebuild: clear + redraw all strokes via Rust
    glaspen2_draw_rebuild((void *)g_surface, g_scale);

    cairo_surface_flush(g_surface);
    if (g_show_rainbow) draw_rainbow_indicator();
    g_has_last = NO;
    flush_to_layer();
}

static void save_and_exit(int sig) {
    (void)sig;
    pm_destroy();
    CGDisplayShowCursor(kCGDirectMainDisplay);
    exit(0);
}

static void draw_rainbow_indicator(void) {
    if (!g_surface) return;
    cairo_t *cr = cairo_create_scaled();
    cairo_set_operator(cr, CAIRO_OPERATOR_OVER);

    // HSV rainbow with full saturation
    for (int col = 0; col < 14; col++) {
        // Convert HSV to RGB (H varies, S=1, V=1)
        double h = col / 14.0;
        double r, g, b;
        int i = (int)(h * 6);
        double f = h * 6 - i;
        double q = 1 - f;
        switch (i % 6) {
            case 0: r = 1; g = f; b = 0; break;
            case 1: r = q; g = 1; b = 0; break;
            case 2: r = 0; g = 1; b = f; break;
            case 3: r = 0; g = q; b = 1; break;
            case 4: r = f; g = 0; b = 1; break;
            case 5: r = 1; g = 0; b = q; break;
        }
        cairo_set_source_rgba(cr, r, g, b, 1.0);
        cairo_rectangle(cr, col * 2, 0, 2, 4);
        cairo_fill(cr);
    }

    cairo_destroy(cr);
    flush_to_layer();
}

// Forward declaration

// ── 文件拆分: #include 文本包含(同一编译单元, clang 报错行号直指子文件);
//    拆分为纯移动, 行为零变化。构建仍只编译 glaspen2.m 这一个入口。──
#include "parts/menu.m"
#include "parts/panel.m"
#include "parts/run.m"
