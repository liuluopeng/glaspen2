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
extern char* glaspen2_ocr_recognize(const unsigned char *pixels, int width, int height);
extern char* glaspen2_ocr_page(const unsigned char *pixels, int width, int height, long screen_id);
extern int glaspen2_export_pdf(void);
extern void glaspen2_ocr_backfill_all(void);
extern int glaspen2_ocr_ensure_models(void);
extern double glaspen2_ocr_download_progress(void);
extern void glaspen2_on_display_change(int screen_w, int screen_h);
extern char* glaspen2_list_screens_json(void);
extern char* glaspen2_search_ocr_json(const char *query);
extern unsigned char* glaspen2_render_thumbnail(long long screen_id, int w, int h, int max_size, int *out_len);
extern void glaspen2_free_rust_bytes(unsigned char *ptr, int len);
extern int glaspen2_delete_screen(long long screen_id);
extern char* glaspen2_page_info_json(long long screen_id);

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
extern void glaspen2_get_stroke_color(int idx, double *r, double *g, double *b);
extern double glaspen2_get_stroke_avg_width(int idx);
extern void glaspen2_get_stroke_point(int idx, int pidx, double *x, double *y);
extern double glaspen2_get_stroke_point_width(int idx, int pidx);
extern int glaspen2_undo_last_stroke(void);

// Forward declarations
static void rebuild_surface_from_strokes(void);
static void finish_active_stroke(void);
static void ensure_surface(NSView *view);
static void ocr_ensure_models_async(void);
static void toggle_ocr_enabled(void);
static BOOL perform_hotkey(unsigned short keyCode);
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
static const double g_width_presets[] = { 0.3, 0.6, 1.0, 1.5, 2.5 };
static const int g_width_preset_count = 5;
static int g_selected_width_index = 2; // default: 1.0x

// Rainbow indicator toggle (default off)
static BOOL g_show_rainbow = NO;

// Grid overlay toggle (default off)
static BOOL g_show_grid = NO;

// When YES the grid follows the strokes and is hidden with them by
// 飘渺画布涂鸦模式 (X). When NO the grid is always visible while its own
// Flutter switch is on. Controlled by the Flutter settings panel.
static BOOL g_grid_follow_strokes = NO;

// OCR feature switch. Off by default: the models are not bundled and are
// downloaded on demand (~135 MB), so a user who never OCRs must not be
// prompted on every 新建画布/翻页. When turned on, the models download on
// first use automatically.
static BOOL g_ocr_enabled = NO;

// Glass overlay opacity (0.0 = off, 0.0-0.3 range)
static BOOL g_glass_enabled = NO;  // frosted glass ON/OFF
static double g_glass_opacity = 0.45; // opacity level (used only when enabled)

// Color presets
typedef struct { const char *name; double r, g, b; } ColorPreset;
static const ColorPreset g_color_presets[] = {
    {"Red",     1.0, 0.0, 0.0},
    {"Orange",  1.0, 0.5, 0.0},
    {"Yellow",  1.0, 1.0, 0.0},
    {"Green",   0.0, 0.8, 0.0},
    {"Cyan",    0.0, 0.8, 0.8},
    {"Blue",    0.0, 0.4, 1.0},
    {"Purple",  0.6, 0.0, 0.8},
    {"Pink",    1.0, 0.4, 0.7},
    {"White",   1.0, 1.0, 1.0},
    {"Black",   0.0, 0.0, 0.0},
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
    if (!g_surface) return;
    cairo_t *cr = cairo_create_scaled();
    cairo_set_operator(cr, CAIRO_OPERATOR_CLEAR);
    cairo_paint(cr);
    cairo_destroy(cr);
    g_has_last = NO;
    int created = glaspen2_clear_strokes(g_screen_w, g_screen_h);
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
@interface GlaspenMenuHandler : NSObject <NSMenuDelegate, NSApplicationDelegate>
@end

static GlaspenMenuHandler *g_menuHandler = nil;
static NSStatusItem *g_statusItem = nil;
static NSMenu *g_menu = nil;
static int g_selectedColorIndex = 0; // 0=red (default)

static void update_menu_texts(void) {
    for (int i = 0; i < g_color_preset_count; i++) {
        NSMenuItem *item = [g_menu itemAtIndex:i];
        if (g_lang == 0) {
            NSString *zhNames[] = {@"红", @"橙", @"黄", @"绿", @"青", @"蓝", @"紫", @"粉", @"白", @"黑"};
            [item setTitle:zhNames[i]];
        } else {
            [item setTitle:[NSString stringWithUTF8String:g_color_presets[i].name]];
        }
    }
    int wBase = g_color_preset_count + 1;
    NSString *zhWidthNames[] = {@"极细", @"细", @"中", @"粗", @"极粗"};
    NSString *enWidthNames[] = {@"Fine", @"Thin", @"Medium", @"Thick", @"Bold"};
    for (int i = 0; i < g_width_preset_count; i++) {
        NSMenuItem *item = [g_menu itemAtIndex:wBase + i];
        [item setTitle:(g_lang == 0) ? zhWidthNames[i] : enWidthNames[i]];
    }
    int base = g_color_preset_count + 1 + g_width_preset_count + 1;
    [[g_menu itemAtIndex:base+0] setTitle:L(@"保存(含背景)", @"Save (with bg)")];
    [[g_menu itemAtIndex:base+1] setTitle:L(@"保存(涂鸦)", @"Save (drawing)")];
    [[g_menu itemAtIndex:base+2] setTitle:L(@"保存笔记 (Xournal)", @"Save Notes (Xournal)")];
    [[g_menu itemAtIndex:base+3] setTitle:L(@"新建画布", @"New canvas")];
    [[g_menu itemAtIndex:base+4] setTitle:L(@"彩虹指示器", @"Rainbow indicator")];
    [[g_menu itemAtIndex:base+5] setTitle:L(@"开机自启", @"Launch at login")];
    [[g_menu itemAtIndex:base+6] setTitle:L(@"磨砂玻璃", @"Frosted Glass")];
    // Update toggle item title based on state
    NSMenuItem *toggleItem = [g_menu itemWithTag:888];
    if (toggleItem) [toggleItem setTitle:g_enabled ? L(@"关闭涂鸦", @"Disable Drawing") : L(@"开启涂鸦", @"Enable Drawing")];
    [[g_menu itemAtIndex:base+10] setTitle:L(@"English", @"中文")];
    [[g_menu itemAtIndex:base+11] setTitle:L(@"退出", @"Quit")];
}

static NSImage* colorSwatchImage(NSColor *color, CGFloat size) {
    NSImage *image = [[NSImage alloc] initWithSize:NSMakeSize(size, size)];
    [image lockFocus];
    [color setFill];
    NSRectFill(NSMakeRect(0, 0, size, size));
    [[NSColor colorWithWhite:0 alpha:0.3] setStroke];
    NSFrameRect(NSMakeRect(0, 0, size, size));
    [image unlockFocus];
    return image;
}

static NSImage* widthIndicatorImage(double scale, CGFloat size) {
    CGFloat lineW = MAX(1.0, scale * 3.0);
    NSImage *image = [[NSImage alloc] initWithSize:NSMakeSize(size, size)];
    [image lockFocus];
    [[NSColor clearColor] setFill];
    NSRectFill(NSMakeRect(0, 0, size, size));
    NSBezierPath *path = [NSBezierPath bezierPath];
    [path setLineWidth:lineW];
    [path setLineCapStyle:NSLineCapStyleRound];
    [path moveToPoint:NSMakePoint(3, size / 2)];
    [path lineToPoint:NSMakePoint(size - 3, size / 2)];
    [[NSColor colorWithWhite:1.0 alpha:0.85] setStroke];
    [path stroke];
    [image unlockFocus];
    return image;
}

static void update_status_icon_color(void) {
    NSColor *color = [NSColor colorWithCalibratedRed:g_pen_r green:g_pen_g blue:g_pen_b alpha:1.0];
    [g_statusItem.button setImage:colorSwatchImage(color, 18)];
}

static void update_status_icon_text(void) {
    [g_statusItem.button setImage:nil];
    [g_statusItem.button setTitle:@"G"];
}

static void update_status_icon_state(void) {
    if (g_enabled) {
        update_status_icon_color();
    } else {
        // Disabled: show gray outline circle
        CGFloat size = 18;
        NSImage *image = [[NSImage alloc] initWithSize:NSMakeSize(size, size)];
        [image lockFocus];
        [[NSColor clearColor] setFill];
        NSRectFill(NSMakeRect(0, 0, size, size));
        NSBezierPath *circle = [NSBezierPath bezierPathWithOvalInRect:NSMakeRect(2, 2, size - 4, size - 4)];
        [[NSColor colorWithWhite:0.5 alpha:0.6] setStroke];
        [circle setLineWidth:2.0];
        [circle stroke];
        [image unlockFocus];
        [g_statusItem.button setImage:image];
    }
}

static void update_menu_checkmarks(void) {
    // Update checkmarks on color items
    for (int i = 0; i < g_color_preset_count; i++) {
        NSMenuItem *item = [g_menu itemAtIndex:i];
        [item setState:(i == g_selectedColorIndex) ? NSControlStateValueOn : NSControlStateValueOff];
    }
    // Update checkmarks on width items
    int widthOffset = g_color_preset_count + 1; // after colors + separator
    for (int i = 0; i < g_width_preset_count; i++) {
        NSMenuItem *item = [g_menu itemAtIndex:widthOffset + i];
        [item setState:(i == g_selected_width_index) ? NSControlStateValueOn : NSControlStateValueOff];
    }
}

static void toggle_enabled(void) {
    g_enabled = !g_enabled;
    if (!g_enabled) {
        // 关闭涂鸦: 临时隐藏整个 glaspen 覆盖层(笔迹/方格/磨砂玻璃),
        // 以便干净地使用其他绘图软件。
        restore_system_cursor();
        finish_active_stroke();
        if (g_window) [g_window orderOut:nil];
        if (g_pressure_monitor) pm_hide();
    } else {
        if (g_window) {
            if (!g_surface && g_draw_view) ensure_surface(g_draw_view);
            [g_window orderFrontRegardless];
            rebuild_surface_from_strokes();
        }
        if (g_pressure_monitor) pm_show();
    }
    update_status_icon_state();
    show_notification(g_enabled
        ? L(@"涂鸦已开启", @"Drawing enabled")
        : L(@"涂鸦已关闭", @"Drawing disabled"));
    // Update menu item
    NSMenuItem *item = [g_menu itemWithTag:888];
    if (item) {
        [item setState:g_enabled ? NSControlStateValueOn : NSControlStateValueOff];
        [item setTitle:g_enabled ? L(@"关闭涂鸦", @"Disable Drawing") : L(@"开启涂鸦", @"Enable Drawing")];
    }
}

// Two canvas drawing modes, switched by ⌘⌃X:
//   - 固定画布涂鸦模式 (fixed canvas mode): the canvas is always visible;
//     the pen never auto-shows or hides it.
//   - 飘渺画布涂鸦模式 (ethereal canvas mode): the strokes start hidden;
//     hovering or touching down shows them, and pen-leave hides them again
//     immediately (silent).
// Hiding only affects the strokes (and the frosted-glass backdrop) — the
// overlay window itself stays visible, so notifications and the crosshair
// keep working. Pen passthrough is managed separately by ⌘ + ⌃ + V
// (g_enabled) — X never touches it.
static BOOL g_ethereal_canvas = NO; // YES = 飘渺画布涂鸦模式
static BOOL g_strokes_visible = YES; // strokes drawn on the overlay?

// Hide the strokes (飘渺 mode). Returns YES if it actually hid them.
static BOOL auto_hide_now(void) {
    if (!g_ethereal_canvas) return NO;
    if (!g_strokes_visible) return NO;
    finish_active_stroke(); // don't strand an in-flight stroke
    g_strokes_visible = NO;
    if (g_glass_view) g_glass_view.hidden = YES;
    if (g_pressure_monitor) pm_hide();
    [g_draw_view setNeedsDisplay:YES];
    return YES;
}

// Show the strokes because the pen came back (飘渺 mode).
static void auto_show_canvas(void) {
    if (!g_ethereal_canvas) return;
    if (!g_strokes_visible) {
        g_strokes_visible = YES;
        gl_glass_apply(); // restore the glass per its own toggle
        if (g_pressure_monitor) pm_show();
        [g_draw_view setNeedsDisplay:YES];
    }
}

// ── Page-navigation peek (ethereal mode) ──
// 翻页 loads strokes onto the hidden canvas; show them briefly so the user
// can see the page they navigated to, then hide again after `seconds`
// unless the user interacted (pen activity) or left 飘渺画布涂鸦模式.
static NSTimer *g_peek_timer = nil;

static void peek_cancel_timer(void) {
    [g_peek_timer invalidate];
    g_peek_timer = nil;
}

static void peek_strokes(double seconds) {
    if (!g_ethereal_canvas) return; // fixed mode: strokes are always visible
    peek_cancel_timer();
    if (!g_strokes_visible) {
        g_strokes_visible = YES;
        gl_glass_apply();
        if (g_pressure_monitor) pm_show();
        [g_draw_view setNeedsDisplay:YES];
    }
    g_peek_timer = [NSTimer scheduledTimerWithTimeInterval:seconds repeats:NO block:^(NSTimer *timer) {
        g_peek_timer = nil;
        if (g_ethereal_canvas && !g_stroke_active) {
            auto_hide_now();
        }
    }];
}

// Switch between 固定画布涂鸦模式 and 飘渺画布涂鸦模式. Shortcut: ⌘ + ⌃ + X
static void toggle_canvas_mode(void) {
    peek_cancel_timer(); // a pending page-peek hide must not fire after a mode switch
    if (!g_ethereal_canvas) {
        // → 飘渺画布涂鸦模式: hide the strokes, peek rules take over
        g_ethereal_canvas = YES;
        finish_active_stroke(); // commit any in-flight stroke before hiding
        g_strokes_visible = NO;
        if (g_glass_view) g_glass_view.hidden = YES;
        if (g_pressure_monitor) pm_hide();
        [g_draw_view setNeedsDisplay:YES];
        show_notification(L(@"飘渺画布涂鸦模式 (悬空/落笔显示)", @"Ethereal canvas mode (hover/down to show)"));
    } else {
        // → 固定画布涂鸦模式: show the strokes and keep them visible
        g_ethereal_canvas = NO;
        g_strokes_visible = YES;
        gl_glass_apply();
        if (g_pressure_monitor) pm_show();
        [g_draw_view setNeedsDisplay:YES];
        show_notification(L(@"固定画布涂鸦模式", @"Fixed canvas mode"));
    }
    // Sync the menu item (title shows the mode you switch TO, like toggleDraw)
    NSMenuItem *item = [g_menu itemWithTag:778];
    if (item) {
        [item setState:g_ethereal_canvas ? NSControlStateValueOn : NSControlStateValueOff];
        [item setTitle:g_ethereal_canvas
            ? L(@"固定画布涂鸦模式", @"Fixed canvas mode")
            : L(@"飘渺画布涂鸦模式", @"Ethereal canvas mode")];
    }
}

@implementation GlaspenMenuHandler

- (void)saveWithBg {
    save_with_background();
}

- (void)saveOnly {
    save_drawing_only();
}

- (void)clearScreen {
    clear_screen();
}

- (void)toggleDraw {
    toggle_enabled();
}

- (void)saveXoj {
    glaspen2_save_xoj();
    show_notification(L(@"笔记已保存", @"Notes saved"));
}

- (void)downloadOcrModels {
    ocr_ensure_models_async();
}

- (void)toggleLanguage {
    g_lang = 1 - g_lang;
    update_menu_texts();
}
- (void)showSettingsPanel {
    show_settings_panel();
}

- (void)toggleRainbow {
    gl_settings_set_rainbow(!g_show_rainbow);
}

- (void)toggleCanvasMode {
    toggle_canvas_mode();
}

- (void)toggleOcr {
    toggle_ocr_enabled();
}

- (void)toggleLaunch {
    gl_settings_set_launch(!glaspen2_is_launch_at_login());
}

- (void)toggleGlass {
    gl_settings_set_glass_enabled(!g_glass_enabled);
}

- (void)selectColor:(NSMenuItem *)sender {
    gl_settings_set_color((int)[sender tag]);
}

- (void)selectWidth:(NSMenuItem *)sender {
    gl_settings_set_width((int)[sender tag]);
}

// NSApplicationDelegate
- (NSApplicationTerminateReply)applicationShouldTerminate:(NSApplication *)sender {
    return NSTerminateNow;
}

- (void)quitApp {
    perf_log_summary();
    CGDisplayShowCursor(kCGDirectMainDisplay);
    [NSApp terminate:nil];
}

// NSMenuDelegate
- (void)menuWillOpen:(NSMenu *)menu {
    update_status_icon_color();
    update_menu_checkmarks();
}

- (void)menuDidClose:(NSMenu *)menu {
    update_status_icon_state();
    // Re-enable CGEventTap after menu closes
    if (g_event_tap) {
        CGEventTapEnable(g_event_tap, true);
    }
}

@end

// --- Settings Panel (Flutter-based) ---
static FlutterEngine *g_flutter_engine = nil;
static FlutterViewController *g_flutter_vc = nil;
static NSWindow *g_settings_window = nil;
static FlutterMethodChannel *g_settings_channel = nil;

static void show_settings_panel(void);
static void sync_settings_panel(void);

// --- Legacy settings panel stubs (no longer used, kept for sync_settings_panel) ---
static NSButton *g_color_buttons[10];
static NSButton *g_width_buttons[5];
static NSButton *g_rainbow_toggle = nil;
static NSButton *g_launch_toggle = nil;
static NSButton *g_glass_toggle = nil;
static NSButton *g_glass_buttons[1];

@interface SettingsMethodChannelHandler : NSObject <FlutterPlugin>
@end

@implementation SettingsMethodChannelHandler
- (void)handleMethodCall:(FlutterMethodCall *)call result:(FlutterResult)result {
    if ([call.method isEqualToString:@"getSettings"]) {
        result(@{
            @"color": @(g_selectedColorIndex),
            @"width": @(g_selected_width_index),
            @"rainbow": @(g_show_rainbow),
            @"launchAtLogin": @(glaspen2_is_launch_at_login()),
            @"frostedGlass": @(g_glass_enabled),
            @"grid": @(g_show_grid),
            @"gridFollowStrokes": @(g_grid_follow_strokes),
            @"pressureMonitor": @(g_pressure_monitor),
            @"ocrEnabled": @(g_ocr_enabled),
            @"gifFps": @(g_gif_fps),
            @"gifResolution": @(g_gif_resolution),
            @"gifSpeed": @(g_gif_speed),
            @"gifEndMode": @(g_gif_end_mode),
        });
    } else if ([call.method isEqualToString:@"setSetting"]) {
        NSDictionary *args = call.arguments;
        NSString *key = args[@"key"];
        id value = args[@"value"];
        if ([key isEqualToString:@"color"]) {
            gl_settings_set_color([value intValue]);
        } else if ([key isEqualToString:@"width"]) {
            gl_settings_set_width([value intValue]);
        } else if ([key isEqualToString:@"rainbow"]) {
            gl_settings_set_rainbow([value boolValue]);
        } else if ([key isEqualToString:@"launchAtLogin"]) {
            gl_settings_set_launch([value boolValue]);
        } else if ([key isEqualToString:@"frostedGlass"]) {
            gl_settings_set_glass_enabled([value boolValue]);
        } else if ([key isEqualToString:@"opacity"]) {
            gl_settings_set_glass_opacity([value doubleValue]);
            if (!g_glass_enabled) gl_settings_set_glass_enabled(YES);
        } else if ([key isEqualToString:@"grid"]) {
            gl_settings_set_grid([value boolValue]);
        } else if ([key isEqualToString:@"gridFollowStrokes"]) {
            g_grid_follow_strokes = [value boolValue];
            glaspen2_save_bool_setting("grid_follow_strokes", g_grid_follow_strokes ? 1 : 0);
            if (g_draw_view) [g_draw_view setNeedsDisplay:YES];
        } else if ([key isEqualToString:@"ocrEnabled"]) {
            if (g_ocr_enabled != [value boolValue]) toggle_ocr_enabled();
            result(nil);
            return;
        } else if ([key isEqualToString:@"pressureMonitor"]) {
            gl_settings_set_pressure_monitor([value boolValue]);
        } else if ([key isEqualToString:@"gifFps"]) {
            g_gif_fps = [value intValue];
            if (g_gif_fps < 1) g_gif_fps = 1;
            if (g_gif_fps > 50) g_gif_fps = 50;
            NSString *s = [NSString stringWithFormat:@"%d", g_gif_fps];
            glaspen2_save_string_setting("gif_fps", [s UTF8String]);
        } else if ([key isEqualToString:@"gifResolution"]) {
            g_gif_resolution = [value doubleValue];
            if (g_gif_resolution < 0.1) g_gif_resolution = 0.1;
            if (g_gif_resolution > 1.0) g_gif_resolution = 1.0;
            NSString *s = [NSString stringWithFormat:@"%.4f", g_gif_resolution];
            glaspen2_save_string_setting("gif_resolution", [s UTF8String]);
        } else if ([key isEqualToString:@"gifSpeed"]) {
            g_gif_speed = [value doubleValue];
            if (g_gif_speed < 0.25) g_gif_speed = 0.25;
            if (g_gif_speed > 10.0) g_gif_speed = 10.0;
            NSString *s = [NSString stringWithFormat:@"%.4f", g_gif_speed];
            glaspen2_save_string_setting("gif_speed", [s UTF8String]);
        } else if ([key isEqualToString:@"gifEndMode"]) {
            g_gif_end_mode = [value intValue];
            if (g_gif_end_mode < 0) g_gif_end_mode = 0;
            if (g_gif_end_mode > 2) g_gif_end_mode = 2;
            NSString *s = [NSString stringWithFormat:@"%d", g_gif_end_mode];
            glaspen2_save_string_setting("gif_end_mode", [s UTF8String]);
        }
        result(nil);
    } else if ([call.method isEqualToString:@"hotkey"]) {
        // 设置面板快捷键按钮 → 执行与物理快捷键相同的动作。
        // Q (退出) 不走 perform_hotkey: 那是按钮专属动作, ⌃⌘Q 必须留给锁屏。
        NSDictionary *args = call.arguments;
        NSString *key = args[@"key"];
        if ([key isEqualToString:@"Q"]) {
            [NSApp terminate:nil];
            result(nil);
            return;
        }
        unsigned short kc = 0;
        if ([key isEqualToString:@"G"]) kc = kVK_ANSI_G;
        else if ([key isEqualToString:@"J"]) kc = 0x26; // J
        else if ([key isEqualToString:@"K"]) kc = 0x28; // K
        else if ([key isEqualToString:@"Z"]) kc = kVK_ANSI_Z;
        else if ([key isEqualToString:@"X"]) kc = kVK_ANSI_X;
        else if ([key isEqualToString:@"C"]) kc = kVK_ANSI_C;
        else if ([key isEqualToString:@"V"]) kc = kVK_ANSI_V;
        else if ([key isEqualToString:@"B"]) kc = kVK_ANSI_B;
        if (kc) perform_hotkey(kc);
        result(nil);
    } else if ([call.method isEqualToString:@"exportAnimatedGif"]) {
        // Run on background queue so UI stays responsive during Cairo rendering.
        // Copy newest GIF to clipboard afterwards (same as Cmd+Ctrl+A hotkey did).
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
            int ok = glaspen2_save_animated_gif(g_gif_fps, g_gif_resolution, g_gif_speed, g_gif_end_mode);
            dispatch_async(dispatch_get_main_queue(), ^{
                if (ok) {
                    NSString *desktop = [NSSearchPathForDirectoriesInDomains(NSDesktopDirectory, NSUserDomainMask, YES) firstObject];
                    NSFileManager *fm = [NSFileManager defaultManager];
                    NSArray *files = [fm contentsOfDirectoryAtPath:desktop error:nil];
                    NSString *newestGif = nil;
                    NSDate *newestDate = nil;
                    for (NSString *f in files) {
                        if ([f hasPrefix:@"glaspen2_"] && [f hasSuffix:@".gif"]) {
                            NSString *full = [desktop stringByAppendingPathComponent:f];
                            NSDictionary *attr = [fm attributesOfItemAtPath:full error:nil];
                            NSDate *d = attr[NSFileModificationDate];
                            if (!newestDate || [d compare:newestDate] == NSOrderedDescending) {
                                newestDate = d; newestGif = full;
                            }
                        }
                    }
                    if (newestGif) {
                        NSPasteboard *pb = [NSPasteboard generalPasteboard];
                        [pb clearContents];
                        [pb writeObjects:@[[NSURL fileURLWithPath:newestGif]]];
                    }
                    show_notification(L(@"动画 GIF 已保存并复制到剪贴板", @"Animated GIF saved & copied"));
                    result(@(YES));
                } else {
                    show_notification(L(@"没有笔迹或导出失败", @"No strokes or export failed"));
                    result(@(NO));
                }
            });
        });
    } else if ([call.method isEqualToString:@"setWindowSize"]) {
        NSDictionary *args = call.arguments;
        CGFloat width = [args[@"width"] doubleValue];
        CGFloat height = [args[@"height"] doubleValue];
        if (g_settings_window && width >= 300 && height >= 300) {
            NSRect frame = [g_settings_window frame];
            frame.size.width = width;
            frame.size.height = height;
            [g_settings_window setFrame:frame display:YES animate:YES];
            [g_settings_window setMinSize:NSMakeSize(300, 300)];
        }
        result(nil);
    } else if ([call.method isEqualToString:@"recognizeText"]) {
        // Run OCR on current drawing surface.
        // Copy the pixels on the main thread first — the background queue must
        // never read live cairo surface memory (torn reads / use-after-free).
        if (!g_ocr_enabled) {
            result(@"");
            return;
        }
        if (!g_surface) {
            result(@"");
            return;
        }
        ocr_ensure_models_async(); // download models on first use if needed
        cairo_surface_flush(g_surface);
        const unsigned char *data = cairo_image_surface_get_data(g_surface);
        int w = cairo_image_surface_get_width(g_surface);
        int h = cairo_image_surface_get_height(g_surface);
        int stride = cairo_image_surface_get_stride(g_surface);
        if (!data || w <= 0 || h <= 0 || stride <= 0) {
            result(@"");
            return;
        }
        unsigned char *copy = malloc((size_t)stride * h);
        if (!copy) { result(@""); return; }
        memcpy(copy, data, (size_t)stride * h);

        dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
            char *text = glaspen2_ocr_recognize(copy, w, h);
            NSString *resultText = text ? [NSString stringWithUTF8String:text] : @"";
            if (text) glaspen2_free_c_string(text);
            free(copy);
            dispatch_async(dispatch_get_main_queue(), ^{
                result(resultText);
            });
        });
    } else if ([call.method isEqualToString:@"exportPdf"]) {
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
            int ok = glaspen2_export_pdf();
            dispatch_async(dispatch_get_main_queue(), ^{
                result(@(ok));
            });
        });
    } else if ([call.method isEqualToString:@"ocrBackfill"]) {
        if (!g_ocr_enabled) {
            result(nil);
            return;
        }
        ocr_ensure_models_async(); // download models on first use if needed
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
            glaspen2_ocr_backfill_all();
            dispatch_async(dispatch_get_main_queue(), ^{
                result(nil);
            });
        });
    } else if ([call.method isEqualToString:@"listPages"]) {
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
            char *json = glaspen2_list_screens_json();
            NSString *str = json ? [NSString stringWithUTF8String:json] : @"[]";
            if (json) glaspen2_free_c_string(json);
            dispatch_async(dispatch_get_main_queue(), ^{
                result(str);
            });
        });
    } else if ([call.method isEqualToString:@"searchText"]) {
        NSString *query = call.arguments[@"query"] ?: @"";
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
            char *json = glaspen2_search_ocr_json([query UTF8String]);
            NSString *str = json ? [NSString stringWithUTF8String:json] : @"[]";
            if (json) glaspen2_free_c_string(json);
            dispatch_async(dispatch_get_main_queue(), ^{
                result(str);
            });
        });
    } else if ([call.method isEqualToString:@"getPageThumbnail"]) {
        NSDictionary *args = call.arguments;
        int64_t screenId = [args[@"screenId"] longLongValue];
        int w = [args[@"w"] intValue];
        int h = [args[@"h"] intValue];
        int maxSize = [args[@"maxSize"] intValue];
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
            int outLen = 0;
            unsigned char *png = glaspen2_render_thumbnail(screenId, w, h, maxSize, &outLen);
            NSData *data = [NSData data];
            if (png && outLen > 0) {
                data = [NSData dataWithBytes:png length:outLen];
                glaspen2_free_rust_bytes(png, outLen);
            }
            dispatch_async(dispatch_get_main_queue(), ^{ result(data); });
        });
    } else if ([call.method isEqualToString:@"deletePage"]) {
        NSDictionary *args = call.arguments;
        int64_t screenId = [args[@"screenId"] longLongValue];
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
            int ok = glaspen2_delete_screen(screenId);
            if (ok && screenId == glaspen2_get_current_screen_id()) {
                int64_t next = glaspen2_next_screen_id();
                if (next == 0) next = glaspen2_prev_screen_id();
                if (next != 0) {
                    glaspen2_load_strokes_for_screen(next);
                    dispatch_async(dispatch_get_main_queue(), ^{
                        rebuild_surface_from_strokes();
                    });
                } else {
                    dispatch_async(dispatch_get_main_queue(), ^{
                        clear_screen();
                    });
                }
            }
            dispatch_async(dispatch_get_main_queue(), ^{
                result(@(ok));
            });
        });
    } else if ([call.method isEqualToString:@"navigateToPage"]) {
        NSDictionary *args = call.arguments;
        int64_t screenId = [args[@"screenId"] longLongValue];
        if (screenId > 0) {
            dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
                glaspen2_load_strokes_for_screen(screenId);
                dispatch_async(dispatch_get_main_queue(), ^{
                    rebuild_surface_from_strokes();
                });
            });
        }
        result(nil);
    } else {
        result(FlutterMethodNotImplemented);
    }
}
@end

// --- Unified settings functions (single source of truth) ---

static void gl_settings_set_color(int idx) {
    if (idx < 0 || idx >= g_color_preset_count) return;
    g_pen_r = g_color_presets[idx].r;
    g_pen_g = g_color_presets[idx].g;
    g_pen_b = g_color_presets[idx].b;
    g_selectedColorIndex = idx;
    glaspen2_save_settings(g_pen_r, g_pen_g, g_pen_b, g_width_scale);
    update_menu_checkmarks();
    update_status_icon_color();
    sync_settings_panel();
}

static void gl_settings_set_width(int idx) {
    if (idx < 0 || idx >= g_width_preset_count) return;
    g_width_scale = g_width_presets[idx];
    g_selected_width_index = idx;
    glaspen2_save_settings(g_pen_r, g_pen_g, g_pen_b, g_width_scale);
    update_menu_checkmarks();
    sync_settings_panel();
}

static void gl_settings_set_rainbow(BOOL on) {
    g_show_rainbow = on;
    NSMenuItem *item = [g_menu itemWithTag:999];
    [item setState:on ? NSControlStateValueOn : NSControlStateValueOff];
    sync_settings_panel();
    // Redraw the surface without the rainbow — never clear the page.
    rebuild_surface_from_strokes();
}

static void gl_settings_set_grid(BOOL on) {
    g_show_grid = on;
    glaspen2_save_bool_setting("grid", on ? 1 : 0);
    sync_settings_panel();
    // The grid is drawn by drawRect per frame (own toggle) — just redraw.
    if (g_draw_view) [g_draw_view setNeedsDisplay:YES];
}

static void gl_settings_set_launch(BOOL on) {
    glaspen2_set_launch_at_login(on ? 1 : 0);
    NSMenuItem *item = [g_menu itemWithTag:777];
    [item setState:on ? NSControlStateValueOn : NSControlStateValueOff];
    sync_settings_panel();
}

static void gl_glass_apply(void) {
    // Combine enabled + opacity into visual effect
    double visual = g_glass_enabled ? g_glass_opacity : 0.0;
    if (g_glass_view) {
        g_glass_view.alphaValue = visual * 2.0; // map to visible range
        g_glass_view.hidden = !g_glass_enabled;
    }
    NSMenuItem *gi = [g_menu itemWithTag:444];
    [gi setState:g_glass_enabled ? NSControlStateValueOn : NSControlStateValueOff];
    if (g_glass_toggle) g_glass_toggle.state = g_glass_enabled ? NSControlStateValueOn : NSControlStateValueOff;
    g_glass_buttons[0].state = (fabs(g_glass_opacity - 0.50) < 0.001) ? NSControlStateValueOn : NSControlStateValueOff;
}

static void gl_settings_set_glass_enabled(BOOL on) {
    g_glass_enabled = on;
    glaspen2_save_bool_setting("glass_enabled", on ? 1 : 0);
    gl_glass_apply();
}

static void gl_settings_set_glass_opacity(double alpha) {
    g_glass_opacity = alpha;
    glaspen2_save_bool_setting("glass_alpha", (int)(alpha * 1000));
    gl_glass_apply();
}

static void gl_settings_set_enabled(BOOL on) {
    g_enabled = on;
    NSMenuItem *item = [g_menu itemWithTag:888];
    [item setState:on ? NSControlStateValueOn : NSControlStateValueOff];
    [item setTitle:on ? L(@"关闭涂鸦", @"Disable Drawing") : L(@"开启涂鸦", @"Enable Drawing")];
    update_status_icon_state();
}

// --- Pressure monitor (top-left info window) ---

static void pm_ensure_window(void) {
    if (g_pm_window) return;
    NSScreen *screen = [NSScreen mainScreen];
    NSRect screenFrame = [screen frame];
    NSRect frame = NSMakeRect(10, screenFrame.size.height - 40, 220, 30);
    g_pm_window = [[NSWindow alloc] initWithContentRect:frame
                                              styleMask:NSWindowStyleMaskBorderless
                                                backing:NSBackingStoreBuffered
                                                  defer:NO];
    [g_pm_window setLevel:kCGMaximumWindowLevel];
    [g_pm_window setOpaque:YES];
    [g_pm_window setBackgroundColor:[NSColor colorWithWhite:0.12 alpha:1.0]];
    [g_pm_window setIgnoresMouseEvents:YES];
    [g_pm_window setTitle:@"Pressure"];
    [g_pm_window setReleasedWhenClosed:NO];

    g_pm_label = [[NSTextField alloc] initWithFrame:NSMakeRect(4, 2, 212, 26)];
    g_pm_evtype = nil;
    [g_pm_label setStringValue:@"P=-----  ---  (---,---)"];
    [g_pm_label setTextColor:[NSColor whiteColor]];
    [g_pm_label setFont:[NSFont fontWithName:@"Menlo" size:12]];
    [g_pm_label setBezeled:NO];
    [g_pm_label setDrawsBackground:NO];
    [g_pm_label setEditable:NO];
    [g_pm_label setSelectable:NO];
    [[g_pm_window contentView] addSubview:g_pm_label];
}

static void pm_show(void) {
    pm_ensure_window();
    [g_pm_window orderFront:nil];
}

static void pm_hide(void) {
    if (g_pm_window) {
        [g_pm_window orderOut:nil];
    }
}

static void pm_update(void) {
    if (!g_pm_window || !g_pm_label) return;
    NSString *typeStr = g_pm_evtype ? g_pm_evtype : @"---";
    [g_pm_label setStringValue:[NSString stringWithFormat:@"P=%-5d  %@  (%d,%d)",
                                 g_pm_pressure, typeStr,
                                 (int)g_pm_x, (int)g_pm_y]];
}

static void pm_destroy(void) {
    if (g_pm_window) {
        [g_pm_window close];
        g_pm_window = nil;
        g_pm_label = nil;
    }
}

static void gl_settings_set_pressure_monitor(BOOL on) {
    g_pressure_monitor = on;
    glaspen2_save_bool_setting("pressure_monitor", on ? 1 : 0);
    if (on) {
        pm_show();
        pm_update();
    } else {
        pm_hide();
    }
    sync_settings_panel();
}

static void sync_settings_panel(void) {
    if (!g_settings_channel) return;
    [g_settings_channel invokeMethod:@"onSettingsChanged" arguments:@{
        @"color": @(g_selectedColorIndex),
        @"width": @(g_selected_width_index),
        @"rainbow": @(g_show_rainbow),
        @"launchAtLogin": @(glaspen2_is_launch_at_login()),
        @"frostedGlass": @(g_glass_enabled),
        @"grid": @(g_show_grid),
        @"gridFollowStrokes": @(g_grid_follow_strokes),
        @"pressureMonitor": @(g_pressure_monitor),
        @"ocrEnabled": @(g_ocr_enabled),
    }];
}

@interface SettingsWindowDelegate : NSObject <NSWindowDelegate>
@end

@implementation SettingsWindowDelegate
- (void)windowWillClose:(NSNotification *)notification {
    // Switch back to Accessory when settings window closes
    [NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];
    [NSApp deactivate];
}
@end

static SettingsWindowDelegate *g_settings_delegate = nil;

static void show_settings_panel(void) {
    // If window already exists, just bring it forward
    if (g_settings_window) {
        [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
        [NSApp activateIgnoringOtherApps:YES];
        [g_settings_window makeKeyAndOrderFront:nil];
        return;
    }

    // Create Flutter engine (singleton)
    if (!g_flutter_engine) {
        g_flutter_engine = [[FlutterEngine alloc] initWithName:@"glaspen_settings"
                                                      project:nil];
        [g_flutter_engine runWithEntrypoint:nil];
    }

    // Set up MethodChannel for settings communication
    g_settings_channel = [FlutterMethodChannel
        methodChannelWithName:@"com.glaspen/settings"
              binaryMessenger:g_flutter_engine.binaryMessenger];

    SettingsMethodChannelHandler *handler = [[SettingsMethodChannelHandler alloc] init];
    [g_settings_channel setMethodCallHandler:^(FlutterMethodCall *call, FlutterResult result) {
        [handler handleMethodCall:call result:result];
    }];

    // Create FlutterViewController
    g_flutter_vc = [[FlutterViewController alloc] initWithEngine:g_flutter_engine
                                                         nibName:nil
                                                          bundle:nil];

    // Create window — large enough for the 1.4× scaled UI on the big screen.
    NSRect frame = NSMakeRect(0, 0, 900, 1200);
    NSWindow *window = [[NSWindow alloc] initWithContentRect:frame
        styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable | NSWindowStyleMaskResizable
        backing:NSBackingStoreBuffered defer:NO];
    [window setTitle:L(@"Glaspen2 设置", @"Glaspen2 Settings")];
    [window setMinSize:NSMakeSize(600, 800)];
    [window setReleasedWhenClosed:NO];

    // Set delegate to switch back to Accessory when window closes
    g_settings_delegate = [[SettingsWindowDelegate alloc] init];
    [window setDelegate:g_settings_delegate];

    // Standard embedding: FlutterViewController owns the content view so the
    // device pixel ratio / backing scale is applied correctly.
    window.contentViewController = g_flutter_vc;

    // Switch to Regular mode so the window can get focus
    [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
    [NSApp activateIgnoringOtherApps:YES];
    [window center];
    [window makeKeyAndOrderFront:nil];
    g_settings_window = window;
}

// Cached full-surface CGImage for drawRect (rebuilt only when the surface
// changes). CGImage wraps the shared cairo buffer, so pixels stay live.
static CGImageRef g_surface_cgimage = NULL;
static const unsigned char *g_surface_cgimage_data = NULL;
static int g_surface_cgimage_w = 0, g_surface_cgimage_h = 0, g_surface_cgimage_stride = 0;

// Drop the cached surface image (call before the surface is destroyed).
static void surface_image_cache_invalidate(void) {
    if (g_surface_cgimage) {
        CGImageRelease(g_surface_cgimage);
        g_surface_cgimage = NULL;
    }
    g_surface_cgimage_data = NULL;
    g_surface_cgimage_w = 0;
    g_surface_cgimage_h = 0;
    g_surface_cgimage_stride = 0;
}

static void ensure_surface(NSView *view) {
    NSRect bounds = [view bounds];
    CGFloat scale = [[view window] backingScaleFactor];
    if (scale < 1.0) scale = 1.0;
    int w = (int)(bounds.size.width * scale);
    int h = (int)(bounds.size.height * scale);
    if (g_surface && cairo_image_surface_get_width(g_surface) == w &&
        cairo_image_surface_get_height(g_surface) == h && g_scale == scale) return;
    if (g_surface) {
        surface_image_cache_invalidate();
        cairo_surface_destroy(g_surface);
    }
    g_surface = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, w, h);
    g_scale = scale;
    cairo_t *cr = cairo_create_scaled();
    cairo_set_operator(cr, CAIRO_OPERATOR_CLEAR);
    cairo_paint(cr);
    cairo_set_operator(cr, CAIRO_OPERATOR_OVER);
    cairo_destroy(cr);
    g_has_last = NO;
}

static void flush_to_layer(void) {
    if (!g_surface || !g_draw_view) return;
    dirty_reset();
    [g_draw_view setNeedsDisplay:YES];
    // Note: deliberately NOT calling displayIfNeeded here.  The display
    // will happen on the next vsync via the runloop, batching all pending
    // pen events into a single frame.  This cuts CPU from ~20 % to ~8-10 %
    // without perceptible latency because the vsync cadence (60/120 Hz) is
    // far slower than raw pen events (200+ Hz).
}


/// Update the active cairo context's source and stroke/fill helpers using
/// the shared g_active_cr (must have been created via stroke_begin).
/// Flushes only the dirty region covered by the latest drawing op.

/// Flush only the currently-marked dirty rect to the screen.
/// Called by per-event raw_draw_dot/raw_draw_segment during a stroke.
static void flush_dirty_to_layer(void) {
    if (!g_surface || !g_draw_view) return;
    if (!g_dirty_has) { [g_draw_view setNeedsDisplay:YES]; return; }
    // Clip dirty rect to view bounds; if empty, fall back to full refresh.
    NSRect bounds = [g_draw_view bounds];
    NSRect dr = NSIntersectionRect(g_dirty_rect, bounds);
    if (NSIsEmptyRect(dr)) {
        dirty_reset();
        [g_draw_view setNeedsDisplay:YES];
        return;
    }
    // Reset dirty tracker; AppKit will deliver drawRect with this rect.
    dirty_reset();
    [g_draw_view setNeedsDisplayInRect:dr];
}

/// Set up the shared cairo context for the duration of a stroke.
/// Idempotent: safe to call if g_active_cr is already set.
static void stroke_begin(void) {
    if (g_active_cr) return;
    if (!g_surface) return;
    g_active_cr = cairo_create(g_surface);
    cairo_scale(g_active_cr, g_scale, g_scale);
}

/// Tear down the shared cairo context at end of stroke.
static void stroke_end(void) {
    if (g_active_cr) {
        cairo_destroy(g_active_cr);
        g_active_cr = NULL;
    }
}

/// Properly finish an in-flight stroke (used by hotkeys / re-entrant pen-down
/// so an interrupted stroke is committed or erased instead of silently lost).
static void finish_active_stroke(void) {
    if (!g_stroke_active) return;
    double ts = [[NSProcessInfo processInfo] systemUptime];
    if (g_eraser_mode) {
        glaspen2_modeler_erase_finish();
        g_eraser_mode = NO;
    } else {
        glaspen2_modeler_end(g_raw_last_x, g_raw_last_y, 0.0, ts, g_width_scale);
        glaspen2_modeler_commit_to_strokes(g_pen_r, g_pen_g, g_pen_b);
    }
    stroke_end();
    g_stroke_active = NO;
    g_raw_has_last = NO;
}

// Handle display configuration changes (resolution, arrangement, etc.)
static void on_display_changed(void) {
    NSScreen *screen = [NSScreen mainScreen];
    NSRect newFrame = [screen frame];
    int new_w = (int)newFrame.size.width;
    int new_h = (int)newFrame.size.height;
    if (new_w == g_screen_w && new_h == g_screen_h) return;

    NSLog(@"[glaspen2] display changed: %dx%d -> %dx%d", g_screen_w, g_screen_h, new_w, new_h);
    g_screen_w = new_w;
    g_screen_h = new_h;
    // Only start a new page when the current one has strokes (no silent page switch).
    glaspen2_on_display_change(g_screen_w, g_screen_h);

    if (g_window) {
        [g_window setFrame:newFrame display:YES];
        NSView *cv = [g_window contentView];
        if (cv) [cv setFrame:NSMakeRect(0, 0, new_w, new_h)];
    }
    if (g_glass_view) [g_glass_view setFrame:newFrame];
    if (g_draw_view) {
        [g_draw_view setFrame:newFrame];
        ensure_surface(g_draw_view);
        rebuild_surface_from_strokes();
        [g_draw_view setNeedsDisplay:YES];
    }
}

static void pen_draw(double x, double y, double width) {
    if (!g_surface) return;
    cairo_t *cr = cairo_create_scaled();
    cairo_set_source_rgba(cr, g_pen_r, g_pen_g, g_pen_b, 1.0);
    cairo_set_line_width(cr, width);
    cairo_set_line_cap(cr, CAIRO_LINE_CAP_ROUND);
    cairo_set_line_join(cr, CAIRO_LINE_JOIN_ROUND);

    if (g_has_last) {
        cairo_move_to(cr, g_last_x, g_last_y);
        cairo_line_to(cr, x, y);
        cairo_stroke(cr);
    } else {
        cairo_arc(cr, x, y, width * 0.5, 0, 2 * M_PI);
        cairo_fill(cr);
    }
    cairo_destroy(cr);

    g_last_x = x;
    g_last_y = y;
    g_has_last = YES;
    glaspen2_add_point(x, y, width);
    flush_to_layer();
}


// Raw drawing — surface only, no STROKES/DB side effects.
// Uses g_active_cr (set up by stroke_begin) for the duration of the stroke.
// Marks the affected pixel area as dirty so only that region is repainted.
static void raw_draw_dot(double x, double y, double width) {
    if (!g_surface) return;
    cairo_t *cr = g_active_cr ? g_active_cr : cairo_create_scaled();
    if (g_eraser_mode) cairo_set_operator(cr, CAIRO_OPERATOR_CLEAR);
    else cairo_set_operator(cr, CAIRO_OPERATOR_OVER);
    cairo_set_source_rgba(cr, g_pen_r, g_pen_g, g_pen_b, 1.0);
    cairo_arc(cr, x, y, width * 0.5, 0, 2 * M_PI);
    cairo_fill(cr);
    if (!g_active_cr) cairo_destroy(cr);
    double pad = width * 0.5 + 1.5; // AA padding
    dirty_include_surface_point(x, y, pad);
    flush_dirty_to_layer();
}

static void raw_draw_segment(double x, double y, double width) {
    if (!g_surface) return;
    cairo_t *cr = g_active_cr ? g_active_cr : cairo_create_scaled();
    if (g_eraser_mode) cairo_set_operator(cr, CAIRO_OPERATOR_CLEAR);
    else cairo_set_operator(cr, CAIRO_OPERATOR_OVER);
    cairo_set_source_rgba(cr, g_pen_r, g_pen_g, g_pen_b, 1.0);
    cairo_set_line_width(cr, width);
    cairo_set_line_cap(cr, CAIRO_LINE_CAP_ROUND);
    cairo_set_line_join(cr, CAIRO_LINE_JOIN_ROUND);
    if (g_raw_has_last) {
        cairo_move_to(cr, g_raw_last_x, g_raw_last_y);
        cairo_line_to(cr, x, y);
        cairo_stroke(cr);
    } else {
        cairo_arc(cr, x, y, width * 0.5, 0, 2 * M_PI);
        cairo_fill(cr);
    }
    if (!g_active_cr) cairo_destroy(cr);

    double pad = width * 0.5 + 1.5; // AA padding
    if (g_raw_has_last) {
        dirty_include_surface_point(g_raw_last_x, g_raw_last_y, pad);
    }
    dirty_include_surface_point(x, y, pad);

    g_raw_last_x = x;
    g_raw_last_y = y;
    g_raw_has_last = YES;
    flush_dirty_to_layer();
}

static void rebuild_surface_from_strokes(void) {
    if (!g_surface) return;
    // Delegate the actual Cairo rendering to Rust (avoids per-point FFI overhead)
    glaspen2_draw_rebuild((void *)g_surface, g_scale);

    // Rainbow is drawn by ObjC (g_show_rainbow is a host-side boolean)
    cairo_surface_flush(g_surface);
    if (g_show_rainbow) draw_rainbow_indicator();

    // Full-screen refresh — undo/page-nav/resize need it
    dirty_reset();
    flush_to_layer();
}

// --- Drawing view ---

@interface GlaspenDrawView : NSView
@end

@implementation GlaspenDrawView

- (BOOL)acceptsFirstResponder { return YES; }

- (void)drawRect:(NSRect)rect {
    if (!g_surface) return;
    cairo_surface_flush(g_surface);
    unsigned char *data = cairo_image_surface_get_data(g_surface);
    int w = cairo_image_surface_get_width(g_surface);
    int h = cairo_image_surface_get_height(g_surface);
    int stride = cairo_image_surface_get_stride(g_surface);

    // Clip to the dirty rect we were asked to repaint (rect parameter).
    // For layer-backed views AppKit may still pass the full bounds; that's fine —
    // the code below handles either case correctly.
    CGContextRef ctx = [[NSGraphicsContext currentContext] CGContext];
    CGContextSaveGState(ctx);
    NSRect clipRect = [self isFlipped] ? rect : rect;
    CGContextClipToRect(ctx, NSRectToCGRect(clipRect));

    // Grid — drawn directly in the view, gated by its own toggle (显示网格).
    // When 网格跟随涂鸦 is on, it hides with the strokes in 飘渺画布涂鸦模式;
    // otherwise it stays visible regardless. Always sits behind the strokes.
    if (g_show_grid && (g_strokes_visible || !g_grid_follow_strokes)) {
        CGContextSetStrokeColorWithColor(ctx, [[NSColor colorWithWhite:0.5 alpha:0.15] CGColor]);
        CGContextSetLineWidth(ctx, 0.5);
        NSRect bounds = [self bounds];
        for (CGFloat gx = 0; gx < bounds.size.width; gx += 40.0) {
            CGContextMoveToPoint(ctx, gx, 0);
            CGContextAddLineToPoint(ctx, gx, bounds.size.height);
        }
        for (CGFloat gy = 0; gy < bounds.size.height; gy += 40.0) {
            CGContextMoveToPoint(ctx, 0, gy);
            CGContextAddLineToPoint(ctx, bounds.size.width, gy);
        }
        CGContextStrokePath(ctx);
    }

    // Reuse the cached CGImage; it wraps the live cairo buffer, so it is
    // only rebuilt when the surface itself changes.
    if (!g_surface_cgimage ||
        g_surface_cgimage_data != data ||
        g_surface_cgimage_w != w ||
        g_surface_cgimage_h != h ||
        g_surface_cgimage_stride != stride) {
        CGImageRelease(g_surface_cgimage);
        g_surface_cgimage = NULL;
        CGColorSpaceRef cs = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
        CGDataProviderRef provider = CGDataProviderCreateWithData(NULL, data, stride * h, NULL);
        g_surface_cgimage = CGImageCreate(w, h, 8, 32, stride, cs,
                                          kCGBitmapByteOrder32Little | kCGImageAlphaPremultipliedFirst,
                                          provider, NULL, false, kCGRenderingIntentDefault);
        CGDataProviderRelease(provider);
        CGColorSpaceRelease(cs);
        g_surface_cgimage_data = data;
        g_surface_cgimage_w = w;
        g_surface_cgimage_h = h;
        g_surface_cgimage_stride = stride;
    }
    CGImageRef image = g_surface_cgimage;

    // The strokes can be hidden independently (飘渺画布涂鸦模式) — skip the
    // surface image, but keep drawing the notification and crosshair below.
    if (image && g_strokes_visible) {
        // If only a small dirty rect was requested, extract just that sub-image
        // from the surface (in physical pixel coords) to avoid scaling the
        // whole 3840×2160 surface each frame.
        BOOL small_dirty = !NSEqualRects(rect, [self bounds]) &&
                           (rect.size.width < [self bounds].size.width ||
                            rect.size.height < [self bounds].size.height);

        if (small_dirty) {
            // Map view points → physical pixels. Surface is top-left origin;
            // view rect is bottom-left origin (non-flipped), so flip Y too.
            CGFloat sx = rect.origin.x * g_scale;
            CGFloat sy = ([self bounds].size.height - (rect.origin.y + rect.size.height)) * g_scale;
            CGFloat sw = rect.size.width * g_scale;
            CGFloat sh = rect.size.height * g_scale;
            CGRect phys = CGRectMake(sx, sy, sw, sh);
            CGImageRef sub = CGImageCreateWithImageInRect(image, phys);
            if (sub) {
                CGContextDrawImage(ctx, NSRectToCGRect(rect), sub);
                CGImageRelease(sub);
            } else {
                NSRect bounds = [self bounds];
                CGContextDrawImage(ctx, CGRectMake(0, 0, bounds.size.width, bounds.size.height), image);
            }
        } else {
            NSRect bounds = [self bounds];
            CGContextDrawImage(ctx, CGRectMake(0, 0, bounds.size.width, bounds.size.height), image);
        }
        // image is the cached surface image — NOT released here.
    }

    // Draw notification text
    if (g_notification) {
        NSShadow *shadow = [[NSShadow alloc] init];
        shadow.shadowColor = [NSColor colorWithWhite:0 alpha:0.8];
        shadow.shadowOffset = NSMakeSize(2, -2);
        shadow.shadowBlurRadius = 4;

        NSDictionary *attrs = @{
            NSFontAttributeName: [NSFont monospacedSystemFontOfSize:36 weight:NSFontWeightMedium],
            NSForegroundColorAttributeName: [NSColor whiteColor],
            NSShadowAttributeName: shadow
        };
        NSSize textSize = [g_notification sizeWithAttributes:attrs];
        // Use view bounds for centering, not surface dimensions — surface may
        // be stale after display resolution changes.
        NSRect bounds = [self bounds];
        CGFloat x = (bounds.size.width - textSize.width) / 2;
        CGFloat y = (bounds.size.height - textSize.height) / 2;
        [g_notification drawAtPoint:NSMakePoint(x, y) withAttributes:attrs];
    }

    // Draw pen crosshair cursor
    if (g_cursor_visible && g_cursor_x >= 0) {
        CGFloat cx = g_cursor_x;
        CGFloat cy = g_cursor_y;
        CGFloat radius = 8.0;

        // Outer circle
        CGContextSetStrokeColorWithColor(ctx, [[NSColor colorWithWhite:1.0 alpha:0.8] CGColor]);
        CGContextSetLineWidth(ctx, 1.5);
        CGContextStrokeEllipseInRect(ctx, CGRectMake(cx - radius, cy - radius, radius * 2, radius * 2));

        // Center dot
        CGContextSetFillColorWithColor(ctx, [[NSColor colorWithWhite:1.0 alpha:0.9] CGColor]);
        CGContextFillEllipseInRect(ctx, CGRectMake(cx - 1.5, cy - 1.5, 3, 3));

        // Crosshair lines
        CGFloat gap = 3.0;
        CGContextSetStrokeColorWithColor(ctx, [[NSColor colorWithWhite:0 alpha:0.5] CGColor]);
        CGContextSetLineWidth(ctx, 1.0);

        // Top
        CGContextMoveToPoint(ctx, cx, cy - radius - 2);
        CGContextAddLineToPoint(ctx, cx, cy - gap);
        // Bottom
        CGContextMoveToPoint(ctx, cx, cy + gap);
        CGContextAddLineToPoint(ctx, cx, cy + radius + 2);
        // Left
        CGContextMoveToPoint(ctx, cx - radius - 2, cy);
        CGContextAddLineToPoint(ctx, cx - gap, cy);
        // Right
        CGContextMoveToPoint(ctx, cx + gap, cy);
        CGContextAddLineToPoint(ctx, cx + radius + 2, cy);
        CGContextStrokePath(ctx);
    }
    CGContextRestoreGState(ctx);
}

@end

// OCR feature on/off (models are downloaded on demand, so OCR must be
// opted in). Menu item tag 666; also exposed in the Flutter settings.
static void toggle_ocr_enabled(void) {
    g_ocr_enabled = !g_ocr_enabled;
    glaspen2_save_bool_setting("ocr_enabled", g_ocr_enabled ? 1 : 0);
    NSMenuItem *item = [g_menu itemWithTag:666];
    if (item) {
        [item setState:g_ocr_enabled ? NSControlStateValueOn : NSControlStateValueOff];
        [item setTitle:g_ocr_enabled
            ? L(@"关闭 OCR 识别", @"Disable OCR")
            : L(@"开启 OCR 识别 (需下载模型)", @"Enable OCR (downloads models)")];
    }
    if (g_ocr_enabled) {
        ocr_ensure_models_async(); // start the download right away
    }
    show_notification(g_ocr_enabled
        ? L(@"OCR 识别已开启", @"OCR enabled")
        : L(@"OCR 识别已关闭", @"OCR disabled"));
}

// ── OCR model on-demand download (models are not bundled) ──

static NSTimer *g_ocr_dl_timer = nil;

// Start the OCR model download if the models are missing, and show a
// progress notification while it runs. Safe to call repeatedly.
static void ocr_ensure_models_async(void) {
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
        int r = glaspen2_ocr_ensure_models();
        if (r == 0) return; // models already ready
        dispatch_async(dispatch_get_main_queue(), ^{
            if (r < 0) {
                show_notification(L(@"OCR 模型下载启动失败", @"OCR model download failed to start"));
                return;
            }
            [g_ocr_dl_timer invalidate];
            g_ocr_dl_timer = [NSTimer scheduledTimerWithTimeInterval:0.5 repeats:YES block:^(NSTimer *timer) {
                double p = glaspen2_ocr_download_progress();
                if (p >= 100.0) {
                    [timer invalidate];
                    g_ocr_dl_timer = nil;
                    show_notification(L(@"OCR 模型下载完成", @"OCR models ready"));
                } else if (p <= -2.0) {
                    [timer invalidate];
                    g_ocr_dl_timer = nil;
                    show_notification(L(@"OCR 模型下载失败", @"OCR model download failed"));
                } else if (p >= 0.0) {
                    show_notification([NSString stringWithFormat:L(@"下载 OCR 模型 %d%%", @"Downloading OCR models %d%%"), (int)p]);
                }
            }];
        });
    });
}

/// OCR the current surface on a background queue and save result to DB.
/// Copies the pixel data so the UI thread can proceed without waiting.
static void ocr_current_page_async(void) {
    if (!g_ocr_enabled) return; // OCR is off — no downloads, no inference
    ocr_ensure_models_async(); // download models on first use if needed
    if (!g_surface) return;
    cairo_surface_flush(g_surface);
    const unsigned char *data = cairo_image_surface_get_data(g_surface);
    int w = cairo_image_surface_get_width(g_surface);
    int h = cairo_image_surface_get_height(g_surface);
    int stride = cairo_image_surface_get_stride(g_surface);
    long sid = glaspen2_get_current_screen_id();

    size_t data_size = stride * h;
    unsigned char *copy = malloc(data_size);
    memcpy(copy, data, data_size);

    dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
        char *text = glaspen2_ocr_page(copy, w, h, sid);
        if (text) glaspen2_free_c_string(text);
        free(copy);
    });
}

/// OCR the current surface and return recognized text (caller must free).
static char* ocr_current_page(void) {
    if (!g_surface) return NULL;
    cairo_surface_flush(g_surface);
    const unsigned char *data = cairo_image_surface_get_data(g_surface);
    int w = cairo_image_surface_get_width(g_surface);
    int h = cairo_image_surface_get_height(g_surface);
    long sid = glaspen2_get_current_screen_id();
    return glaspen2_ocr_page(data, w, h, sid);
}

// --- CGEventTap callback ---

// Performance logging (set g_perf_log=YES to enable)
static BOOL g_perf_log = NO;
static FILE *g_perf_file = NULL;
static uint64_t g_perf_total_calls = 0;
static uint64_t g_perf_slow_calls = 0;

static void perf_log_begin(void) {
    if (!g_perf_file) {
        NSString *dir = [NSSearchPathForDirectoriesInDomains(NSLibraryDirectory, NSUserDomainMask, YES) firstObject];
        NSString *logDir = [dir stringByAppendingPathComponent:@"Logs/glaspen2"];
        NSError *err = nil;
        [[NSFileManager defaultManager] createDirectoryAtPath:logDir withIntermediateDirectories:YES attributes:nil error:&err];
        if (err) NSLog(@"[glaspen2] perf log dir error: %@", err);
        NSString *path = [logDir stringByAppendingPathComponent:@"perf.log"];
        g_perf_file = fopen([path UTF8String], "w");
        if (g_perf_file) {
            NSLog(@"[glaspen2] performance log: %@", path);
            fprintf(g_perf_file, "ts_ms\ttype\tdur_us\tnotes\n");
            fflush(g_perf_file);
        } else {
            NSLog(@"[glaspen2] perf log open failed: %@", path);
        }
    }
}

static mach_timebase_info_data_t g_tb;
static BOOL g_tb_inited = NO;

static uint64_t elapsed_us(uint64_t start) {
    if (!g_tb_inited) { mach_timebase_info(&g_tb); g_tb_inited = YES; }
    return (mach_absolute_time() - start) * g_tb.numer / g_tb.denom / 1000;
}

static void perf_log_event(const char *evtype, uint64_t dur_us) {
    if (!g_perf_log || !g_perf_file) return;
    g_perf_total_calls++;
    if (dur_us > 16000) g_perf_slow_calls++; // >16ms = frame drop
    if (!g_tb_inited) { mach_timebase_info(&g_tb); g_tb_inited = YES; }
    double ts_ms = (double)mach_absolute_time() * g_tb.numer / g_tb.denom / 1e6;
    fprintf(g_perf_file, "%.3f\t%s\t%llu\t%s\n", ts_ms, evtype, dur_us,
            dur_us > 16000 ? "SLOW" : "");
    if (g_perf_total_calls % 100 == 0) fflush(g_perf_file);
}

// Execute a hotkey action by key code. Returns YES if handled.
// Used by the physical shortcut (event tap) and the settings-panel buttons.
static BOOL perform_hotkey(unsigned short kc) {
    if (kc == kVK_ANSI_C) {
        finish_active_stroke(); // don't strand an in-flight stroke
        ocr_current_page_async();
        clear_screen();
        return YES;
    } else if (kc == kVK_ANSI_V) { toggle_enabled(); return YES; }
    else if (kc == 0x26) { // J — previous page
        finish_active_stroke();
        ocr_current_page_async();
        long target = glaspen2_prev_screen_id();
        if (target > 0) {
            glaspen2_load_strokes_for_screen(target);
            glaspen2_smooth_loaded_strokes();
            replay_strokes_from_memory();
            peek_strokes(1.0); // show the page briefly in ethereal mode
            show_page_info(target);
        } else {
            show_notification(L(@"没有上一页", @"No previous page"));
        }
        return YES;
    } else if (kc == 0x28) { // K — next page
        finish_active_stroke();
        ocr_current_page_async();
        long target = glaspen2_next_screen_id();
        if (target > 0) {
            glaspen2_load_strokes_for_screen(target);
            glaspen2_smooth_loaded_strokes();
            replay_strokes_from_memory();
            peek_strokes(1.0); // show the page briefly in ethereal mode
            show_page_info(target);
        } else {
            show_notification(L(@"没有下一页", @"No next page"));
        }
        return YES;
    } else if (kc == kVK_ANSI_G) {
        glaspen2_save_svg();
        if (g_surface) {
            cairo_surface_flush(g_surface);
            const unsigned char *data = cairo_image_surface_get_data(g_surface);
            int w = cairo_image_surface_get_width(g_surface);
            int h = cairo_image_surface_get_height(g_surface);
            int stride = cairo_image_surface_get_stride(g_surface);
            if (glaspen2_save_gif_cropped(data, w, h, stride, (double)g_scale)) {
                NSString *desktop = [NSSearchPathForDirectoriesInDomains(NSDesktopDirectory, NSUserDomainMask, YES) firstObject];
                NSFileManager *fm = [NSFileManager defaultManager];
                NSArray *files = [fm contentsOfDirectoryAtPath:desktop error:nil];
                NSString *newestGif = nil;
                NSDate *newestDate = nil;
                for (NSString *f in files) {
                    if ([f hasPrefix:@"glaspen2_"] && [f hasSuffix:@".gif"]) {
                        NSString *full = [desktop stringByAppendingPathComponent:f];
                        NSDictionary *attr = [fm attributesOfItemAtPath:full error:nil];
                        NSDate *d = attr[NSFileModificationDate];
                        if (!newestDate || [d compare:newestDate] == NSOrderedDescending) {
                            newestDate = d; newestGif = full;
                        }
                    }
                }
                if (newestGif) {
                    NSPasteboard *pb = [NSPasteboard generalPasteboard];
                    [pb clearContents];
                    [pb writeObjects:@[[NSURL fileURLWithPath:newestGif]]];
                }
                show_notification(L(@"已导出 SVG + GIF", @"SVG + GIF saved"));
            } else {
                show_notification(L(@"导出失败", @"Export failed"));
            }
        }
        return YES;
    } else if (kc == kVK_ANSI_Z) {
        if (g_stroke_active) {
            show_notification(L(@"正在书写中", @"Stroke in progress"));
        } else {
            int remaining = glaspen2_undo_last_stroke();
            if (remaining < 0) {
                show_notification(L(@"没有可撤销的笔画", @"Nothing to undo"));
            } else {
                rebuild_surface_from_strokes();
                show_notification(L(@"撤销成功", @"Undo"));
            }
        }
        return YES;
    } else if (kc == kVK_ANSI_S) {
        char *svg = glaspen2_get_cropped_svg();
        if (svg) {
            NSString *svgStr = [NSString stringWithUTF8String:svg];
            NSData *svgData = [svgStr dataUsingEncoding:NSUTF8StringEncoding];
            NSString *base64 = [svgData base64EncodedStringWithOptions:0];
            NSString *htmlTag = [NSString stringWithFormat:@"<img src=\"data:image/svg+xml;base64,%@\" />", base64];
            NSPasteboard *pb = [NSPasteboard generalPasteboard];
            [pb clearContents];
            [pb setString:htmlTag forType:NSPasteboardTypeString];
            glaspen2_free_c_string(svg);
            show_notification(L(@"SVG 已复制到剪贴板", @"SVG copied to clipboard"));
        } else {
            show_notification(L(@"没有笔迹可复制", @"No strokes to copy"));
        }
        return YES;
    } else if (kc == kVK_ANSI_B) {
        gl_settings_set_glass_enabled(!g_glass_enabled);
        return YES;
    } else if (kc == kVK_ANSI_X) {
        // Switch canvas mode: 固定 ↔ 飘渺 (⌘ + ⌃ + X)
        toggle_canvas_mode();
        return YES;
    } else if (kc == kVK_ANSI_Comma) {
        show_settings_panel();
        return YES;
    }
    // 注意: 不要在这里处理 Q — ⌃⌘Q 是 macOS 的系统锁屏快捷键,
    // 拦截它会顶掉锁屏。退出只由设置面板按钮触发(见 'hotkey' 处理器)。
    return NO;
}

// ── Quick GIF recording (hold Cmd+Ctrl+R, doodle, release to copy) ──

// Write the GIF bytes to a temp .gif file and put that file URL on the
// pasteboard so it can be pasted as an animation in apps that accept files.
static void copy_gif_data_to_clipboard(unsigned char *gif, int len) {
    if (!gif || len <= 0) return;
    NSData *data = [NSData dataWithBytes:gif length:len];
    NSString *name = [NSString stringWithFormat:@"glaspen2_record_%.0f.gif",
                      [NSDate timeIntervalSinceReferenceDate] * 1000];
    NSString *path = [NSTemporaryDirectory() stringByAppendingPathComponent:name];
    if (![data writeToFile:path atomically:YES]) {
        NSLog(@"[glaspen2] failed to write GIF temp file: %@", path);
        return;
    }
    NSPasteboard *pb = [NSPasteboard generalPasteboard];
    [pb clearContents];
    [pb writeObjects:@[[NSURL fileURLWithPath:path]]];
}

// Begin a recording session. Commits any in-flight stroke first so the
// snapshot index is exactly where the held-key doodle starts.
static void gif_record_start(void) {
    if (g_gif_recording) return;
    finish_active_stroke();
    g_gif_record_start = glaspen2_stroke_count();
    g_gif_recording = YES;
    show_notification(L(@"按住绘制, 松开生成 GIF", @"Hold & draw, release to make GIF"));
}

// Finish the recording and copy the resulting GIF to the clipboard. The heavy
// Cairo rendering/encoding runs on a background queue; the stroke window is
// pinned from start (key-down) and end (key-up) captured on the main thread,
// so a new recording can never clobber a pending one.
static void gif_record_stop_async(void) {
    int start = g_gif_record_start;
    int fps = g_gif_fps;
    double resolution = g_gif_resolution;
    double speed = g_gif_speed;
    int end_mode = g_gif_end_mode;
    g_gif_record_start = -1;
    g_gif_recording = NO;
    finish_active_stroke();
    int end = glaspen2_stroke_count();
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
        int out_len = 0;
        unsigned char *gif = glaspen2_gif_record_end(start, end, fps, resolution, speed, end_mode, &out_len);
        dispatch_async(dispatch_get_main_queue(), ^{
            if (gif && out_len > 0) {
                copy_gif_data_to_clipboard(gif, out_len);
                glaspen2_free_rust_bytes(gif, out_len);
                show_notification(L(@"GIF 已复制到剪贴板", @"GIF copied to clipboard"));
            } else {
                show_notification(L(@"没有笔迹或导出失败", @"No strokes or export failed"));
            }
        });
    });
}

static CGEventRef event_tap_callback(CGEventTapProxy proxy, CGEventType type,
                                      CGEventRef event, void *refcon) {
    if (g_perf_log && !g_perf_file) perf_log_begin();

    uint64_t t0 = mach_absolute_time();

    // Re-enable tap if it gets disabled by timeout/user
    if (type == kCGEventTapDisabledByTimeout || type == kCGEventTapDisabledByUserInput) {
        NSLog(@"[glaspen2] TAP DISABLED (timeout=%d, userinput=%d)", type == kCGEventTapDisabledByTimeout, type == kCGEventTapDisabledByUserInput);
        CGEventTapEnable(g_event_tap, true);
        perf_log_event("tap_disabled", elapsed_us(t0));
        return event;
    }

    // Pen proximity (hover in/out): the crosshair tracks the pen itself —
    // it appears with hover moves and is hidden on leave regardless of the
    // canvas mode. In 飘渺画布涂鸦模式 hovering also shows the strokes and
    // pen-leave hides them.
    if (type == kCGEventTabletProximity) {
        NSEvent *proxEvent = [NSEvent eventWithCGEvent:event];
        BOOL isStylus = proxEvent &&
            ([proxEvent pointingDeviceType] == NSPenPointingDevice ||
             [proxEvent pointingDeviceType] == NSEraserPointingDevice);
        if (isStylus) {
            int64_t proxState =
                CGEventGetIntegerValueField(event, kCGTabletProximityEventEnterProximity);
            if (proxState == 1) {
                peek_cancel_timer(); // pen is back — the peek stays
                if (g_ethereal_canvas) auto_show_canvas();
            } else {
                // Pen left the screen: hide the crosshair right away instead
                // of leaving a frozen ghost over the hidden strokes.
                if (g_cursor_visible) {
                    dirty_include_point(g_cursor_x, g_cursor_y, 14.0);
                    g_cursor_visible = NO;
                    flush_dirty_to_layer();
                }
                if (g_ethereal_canvas) auto_hide_now();
            }
        }
        return event;
    }

    // Handle keyboard events separately — always intercept hotkeys
    if (type == kCGEventKeyDown || type == kCGEventKeyUp) {
        NSEvent *keyEvent = [NSEvent eventWithCGEvent:event];
        if (keyEvent) {
            NSUInteger mods = [keyEvent modifierFlags];
            BOOL hasCmdCtrl = (mods & NSEventModifierFlagCommand) && (mods & NSEventModifierFlagControl);
            unsigned short kc = [keyEvent keyCode];
            // Stop the GIF recorder on R key-up regardless of modifiers, so a
            // Cmd/Ctrl released before R can't leave the recorder stuck.
            if (type == kCGEventKeyUp && kc == kVK_ANSI_R && g_gif_recording) {
                gif_record_stop_async();
                return NULL;
            }
            if (hasCmdCtrl && kc == kVK_ANSI_R && type == kCGEventKeyDown) {
                // Hold-to-record GIF: key-down starts the recording.
                if (!g_gif_recording) gif_record_start();
                return NULL;
            }
            if (hasCmdCtrl && type == kCGEventKeyDown) {
                if (perform_hotkey(kc)) return NULL;
            }
        }
        // Not a hotkey — pass through to system
        return event;
    }

    // All non-keyboard events: if app is disabled, pass through.
    // Restore the system cursor first — disabling mid-stroke must never
    // leave the cursor hidden.
    if (!g_enabled) {
        restore_system_cursor();
        return event;
    }

    // Convert to NSEvent to check pen properties
    NSEvent *nsevent = [NSEvent eventWithCGEvent:event];
    if (!nsevent) return event;

    NSPointingDeviceType devType = [nsevent pointingDeviceType];
    NSInteger subtype = [nsevent subtype];
    NSEventType etype = [nsevent type];
    CGFloat pressure = [nsevent pressure];

    BOOL isPen = (devType == NSPenPointingDevice || devType == NSEraserPointingDevice ||
                  subtype == 1 || subtype == 2);

    // Non-pen mouse move while no active stroke and cursor visible → hide crosshair.
    // Covers pen-leave-proximity on tablets that don't emit proximity events.
    // The g_stroke_active guard prevents spurious non-pen events interleaved during
    // a stroke (from trackpad / secondary input) from causing extra redraws.
    if (!isPen && etype == NSEventTypeMouseMoved && g_cursor_visible && !g_stroke_active) {
        // Invalidate only the old crosshair region (partial refresh).
        dirty_include_point(g_cursor_x, g_cursor_y, 14.0);
        g_cursor_visible = NO;
        // 飘渺画布涂鸦模式: the pen just left (no proximity events on this
        // tablet) — hide as a fallback.
        if (g_ethereal_canvas) auto_hide_now();
        flush_dirty_to_layer();
    }

    // Update cursor position for pen events only. The crosshair is hidden
    // while a stroke is being drawn (the ink itself is the feedback) and
    // re-shown on pen-up; during hover its redraws are throttled to ~16 ms
    // to cut per-event work at high hover rates.
    if (isPen) {
        // Tablets without proximity events: hover moves are the only signal —
        // treat them like a hover to show the strokes in 飘渺画布涂鸦模式.
        if (!g_strokes_visible) auto_show_canvas();
        peek_cancel_timer(); // pen interaction overrides a pending page peek
        NSPoint loc = [nsevent locationInWindow];
        BOOL moved = (g_cursor_x != loc.x || g_cursor_y != loc.y);
        if (moved && g_cursor_visible && !g_stroke_active) {
            dirty_include_point(g_cursor_x, g_cursor_y, 14.0); // old position
        }
        g_cursor_x = loc.x;
        g_cursor_y = loc.y;
        if (!g_stroke_active) {
            g_cursor_visible = YES;
            dirty_include_point(g_cursor_x, g_cursor_y, 14.0);
            if (moved && elapsed_us(g_last_cursor_flush) >= 16000) {
                g_last_cursor_flush = mach_absolute_time();
                flush_dirty_to_layer();
            }
        }
    }

    // Hide system cursor while pen is drawing, restore on any mouse up
    if (isPen && (etype == NSEventTypeLeftMouseDown || etype == NSEventTypeRightMouseDown ||
                  etype == NSEventTypeOtherMouseDown ||
                  etype == NSEventTypeLeftMouseDragged || etype == NSEventTypeRightMouseDragged ||
                  etype == NSEventTypeOtherMouseDragged)) {
        if (!g_pen_drawing) {
            CGDisplayHideCursor(kCGDirectMainDisplay);
            g_pen_drawing = YES;
        }
    }
    if (etype == NSEventTypeLeftMouseUp || etype == NSEventTypeRightMouseUp ||
        etype == NSEventTypeOtherMouseUp) {
        restore_system_cursor();
        g_cursor_visible = NO;
        dirty_include_point(g_cursor_x, g_cursor_y, 14.0);
        flush_dirty_to_layer();
    }

    // Check if click is on the menu bar (real mouse clicks only — a pen
    // stroke starting near the top of the screen must not freeze input).
    if (etype == NSEventTypeLeftMouseDown && !isPen) {
        CGPoint cgLoc = CGEventGetLocation(event);
        if (cgLoc.y < 30) {
            CGEventTapEnable(g_event_tap, false);
            dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 10 * NSEC_PER_SEC), dispatch_get_main_queue(), ^{
                if (g_event_tap) CGEventTapEnable(g_event_tap, true);
            });
            return event;
        }
    }

    // Draw on pen contact/drag events — routed through stroke modeler
    NSPoint loc = [nsevent locationInWindow];
    CGFloat view_h = [g_draw_view bounds].size.height;
    double px = loc.x;
    double py = view_h - loc.y;
    double ts = [nsevent timestamp]; // NSTimeInterval (seconds since boot)

    // Track if a stroke is active (modeler has been initialized)

    // Width from pressure (same formula as Rust modeler::pressure_to_width)
    double raw_w = (pressure > 0.01) ? (0.3 + pressure * pressure * 7.7) * g_width_scale
                                     : 1.0 * g_width_scale;

    // Pressure monitor: update on pen event before early returns
    if (g_pressure_monitor && isPen) {
        g_pm_in_range = YES;
        int64_t rawField = CGEventGetIntegerValueField(event, kCGTabletEventPointPressure);
        g_pm_pressure = (rawField > 0)
            ? (int)(rawField * 65535 / 65536)
            : (int)(pressure * 65535);
        g_pm_x = px;
        g_pm_y = py;
        g_pm_tip_down = (etype == NSEventTypeLeftMouseDown || etype == NSEventTypeRightMouseDown ||
                         etype == NSEventTypeOtherMouseDown ||
                         etype == NSEventTypeLeftMouseDragged || etype == NSEventTypeRightMouseDragged ||
                         etype == NSEventTypeOtherMouseDragged);
                switch (etype) {
            case NSEventTypeLeftMouseDown:    g_pm_evtype = @"DOWN";  break;
            case NSEventTypeLeftMouseUp:      g_pm_evtype = @"UP";    break;
            case NSEventTypeLeftMouseDragged: g_pm_evtype = @"DRAG";  break;
            case NSEventTypeRightMouseDown:   g_pm_evtype = @"R_DOWN"; break;
            case NSEventTypeRightMouseUp:     g_pm_evtype = @"R_UP";  break;
            case NSEventTypeRightMouseDragged:g_pm_evtype = @"R_DRAG"; break;
            case NSEventTypeOtherMouseDown:   g_pm_evtype = @"O_DOWN"; break;
            case NSEventTypeOtherMouseUp:     g_pm_evtype = @"O_UP";  break;
            case NSEventTypeOtherMouseDragged:g_pm_evtype = @"O_DRAG"; break;
            case NSEventTypeMouseMoved:       g_pm_evtype = g_pm_tip_down ? @"DOWN" : @"MOVE"; break;
            default:                          g_pm_evtype = g_pm_tip_down ? @"DOWN" : @"MOVE"; break;
        }
        pm_update();
    }

    // Strokes hidden (飘渺画布涂鸦模式): touching down always re-shows them
    // and the stroke proceeds below. The pen is swallowed so it never acts
    // as a mouse while drawing mode (V) is enabled.
    if (!g_strokes_visible && isPen &&
        (etype == NSEventTypeLeftMouseDown || etype == NSEventTypeRightMouseDown ||
         etype == NSEventTypeOtherMouseDown ||
         etype == NSEventTypeLeftMouseDragged || etype == NSEventTypeRightMouseDragged ||
         etype == NSEventTypeOtherMouseDragged)) {
        auto_show_canvas(); // show (and draw) — fallback for tablets without hover
    }

    if (isPen && (etype == NSEventTypeLeftMouseDown || etype == NSEventTypeRightMouseDown ||
                  etype == NSEventTypeOtherMouseDown)) {
        // Pen down: start modeler, draw raw dot immediately (no lag)
        // If a previous stroke was never ended (missed pen-up), finish it first.
        if (g_stroke_active) {
            finish_active_stroke();
        }
        g_eraser_mode = (devType == NSEraserPointingDevice);
        NSLog(@"[glaspen2] pen DOWN at (%.1f, %.1f) p=%.2f ts=%.3f", px, py, pressure, ts);
        glaspen2_modeler_begin(g_pen_r, g_pen_g, g_pen_b, px, py, pressure, ts, g_width_scale);
        g_stroke_active = YES;
        g_cursor_visible = NO; // the ink is the feedback while drawing
        stroke_begin(); // reuse one cairo context for the whole stroke
        raw_draw_dot(px, py, raw_w);
        g_raw_last_x = px;
        g_raw_last_y = py;
        g_raw_has_last = YES;
        return NULL;
    }
    if (isPen && (etype == NSEventTypeLeftMouseDragged || etype == NSEventTypeRightMouseDragged ||
                  etype == NSEventTypeOtherMouseDragged)) {
        // If no DOWN event was seen (pen detection lag), auto-initialize
        if (!g_stroke_active) {
            g_eraser_mode = (devType == NSEraserPointingDevice);
            glaspen2_modeler_begin(g_pen_r, g_pen_g, g_pen_b, px, py, pressure, ts, g_width_scale);
            g_stroke_active = YES;
            g_cursor_visible = NO;
            stroke_begin();
            raw_draw_dot(px, py, raw_w);
            g_raw_last_x = px;
            g_raw_last_y = py;
            g_raw_has_last = YES;
            return NULL; // begin already recorded this point, don't feed duplicate to modeler
        }
        // Feed modeler, draw raw segment for responsive real-time feedback
        glaspen2_modeler_move(px, py, pressure, ts, g_width_scale);
        raw_draw_segment(px, py, raw_w);
        return NULL;
    }
    if (isPen && (etype == NSEventTypeLeftMouseUp || etype == NSEventTypeRightMouseUp ||
                  etype == NSEventTypeOtherMouseUp)) {
        // Pen up: finalize modeler, commit smoothed points (or erase)
        if (g_stroke_active) {
            if (g_eraser_mode) {
                glaspen2_modeler_erase_finish();
                g_eraser_mode = NO;
            } else {
                glaspen2_modeler_end(px, py, pressure, ts, g_width_scale);
                glaspen2_modeler_commit_to_strokes(g_pen_r, g_pen_g, g_pen_b);
            }

            // P0: no rebuild — raw drawing remains on the surface.
            // Undo still calls rebuild_surface_from_strokes() to clear erased strokes.
            stroke_end(); // release shared cairo context
            g_stroke_active = NO;
            g_raw_has_last = NO;
            // Pen is back to hovering — restore the crosshair.
            g_cursor_visible = YES;
            dirty_include_point(g_cursor_x, g_cursor_y, 14.0);
            flush_dirty_to_layer();
        }
        return NULL;
    }

    perf_log_event("tick", elapsed_us(t0));
    return event;
}

// Call this at app exit to dump stats
static void perf_log_summary(void) {
    if (!g_perf_file) return;
    fprintf(g_perf_file, "\n# SUMMARY: total=%llu slow=%llu (%.1f%%)\n",
            g_perf_total_calls, g_perf_slow_calls,
            g_perf_total_calls > 0 ? 100.0 * g_perf_slow_calls / g_perf_total_calls : 0);
    fclose(g_perf_file);
    g_perf_file = NULL;
}

// --- App ---

void glaspen2_run(void) {
    @autoreleasepool {
        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];

        // Request accessibility permission (needed for CGEventTap)
        NSDictionary *opts = @{(__bridge id)kAXTrustedCheckOptionPrompt: @YES};
        if (!AXIsProcessTrustedWithOptions((__bridge CFDictionaryRef)opts)) {
            NSLog(@"[glaspen2] Accessibility permission not granted");
        }

        // Create status bar menu
        g_statusItem = [[NSStatusBar systemStatusBar] statusItemWithLength:NSSquareStatusItemLength];
        [g_statusItem.button setTitle:@"G"];

        g_menuHandler = [[GlaspenMenuHandler alloc] init];
        [NSApp setDelegate:g_menuHandler];

        g_menu = [[NSMenu alloc] init];
        [g_menu setDelegate:g_menuHandler];
        [g_menu setAutoenablesItems:NO];

        // Color items with swatch images and names
        NSString *zhColorNames[] = {@"红", @"橙", @"黄", @"绿", @"青", @"蓝", @"紫", @"粉", @"白", @"黑"};
        for (int i = 0; i < g_color_preset_count; i++) {
            NSString *title = (g_lang == 0) ? zhColorNames[i] : [NSString stringWithUTF8String:g_color_presets[i].name];
            NSColor *c = [NSColor colorWithRed:g_color_presets[i].r green:g_color_presets[i].g blue:g_color_presets[i].b alpha:1.0];
            NSMenuItem *item = [g_menu addItemWithTitle:title action:@selector(selectColor:) keyEquivalent:@""];
            item.image = colorSwatchImage(c, 18);
            item.target = g_menuHandler;
            item.tag = i;
        }

        [g_menu addItem:[NSMenuItem separatorItem]];

        // Width items with line indicator images
        NSString *zhWidthNames[] = {@"极细", @"细", @"中", @"粗", @"极粗"};
        NSString *enWidthNames[] = {@"Fine", @"Thin", @"Medium", @"Thick", @"Bold"};
        for (int i = 0; i < g_width_preset_count; i++) {
            NSString *title = (g_lang == 0) ? zhWidthNames[i] : enWidthNames[i];
            NSMenuItem *item = [g_menu addItemWithTitle:title action:@selector(selectWidth:) keyEquivalent:@""];
            item.image = widthIndicatorImage(g_width_presets[i], 18);
            item.target = g_menuHandler;
            item.tag = i;
        }

        [g_menu addItem:[NSMenuItem separatorItem]];
        [g_menu addItemWithTitle:L(@"保存(含背景)", @"Save (with bg)") action:@selector(saveWithBg) keyEquivalent:@""];
        [g_menu addItemWithTitle:L(@"保存(涂鸦)", @"Save (drawing)") action:@selector(saveOnly) keyEquivalent:@""];
        [g_menu addItemWithTitle:L(@"保存笔记 (Xournal)", @"Save Notes (Xournal)") action:@selector(saveXoj) keyEquivalent:@""];
        NSMenuItem *ocrDlItem = [g_menu addItemWithTitle:L(@"下载 OCR 模型 (135M)", @"Download OCR models (135M)") action:@selector(downloadOcrModels) keyEquivalent:@""];
        ocrDlItem.target = g_menuHandler;
        [g_menu addItemWithTitle:L(@"新建画布", @"New canvas") action:@selector(clearScreen) keyEquivalent:@""];
        NSMenuItem *rainbowItem = [g_menu addItemWithTitle:L(@"彩虹指示器", @"Rainbow indicator") action:@selector(toggleRainbow) keyEquivalent:@""];
        rainbowItem.target = g_menuHandler;
        rainbowItem.tag = 999;
        rainbowItem.state = NSControlStateValueOff;
        NSMenuItem *launchItem = [g_menu addItemWithTitle:L(@"开机自启", @"Launch at login") action:@selector(toggleLaunch) keyEquivalent:@""];
        launchItem.target = g_menuHandler;
        launchItem.tag = 777;
        launchItem.state = glaspen2_is_launch_at_login() ? NSControlStateValueOn : NSControlStateValueOff;
        NSMenuItem *glassItem = [g_menu addItemWithTitle:L(@"磨砂玻璃", @"Frosted Glass") action:@selector(toggleGlass) keyEquivalent:@""];
        glassItem.target = g_menuHandler;
        glassItem.tag = 444;
        glassItem.state = g_glass_enabled ? NSControlStateValueOn : NSControlStateValueOff;
        NSMenuItem *modeItem = [g_menu addItemWithTitle:L(@"固定画布涂鸦模式", @"Fixed canvas mode") action:@selector(toggleCanvasMode) keyEquivalent:@""];
        modeItem.target = g_menuHandler;
        modeItem.tag = 778;
        modeItem.state = NSControlStateValueOff;
        NSMenuItem *ocrItem = [g_menu addItemWithTitle:L(@"开启 OCR 识别 (需下载模型)", @"Enable OCR (downloads models)") action:@selector(toggleOcr) keyEquivalent:@""];
        ocrItem.target = g_menuHandler;
        ocrItem.tag = 666;
        ocrItem.state = NSControlStateValueOff;
        [g_menu addItem:[NSMenuItem separatorItem]];
        NSMenuItem *toggleItem = [g_menu addItemWithTitle:L(@"开启涂鸦", @"Enable Drawing") action:@selector(toggleDraw) keyEquivalent:@""];
        toggleItem.target = g_menuHandler;
        toggleItem.tag = 888;
        toggleItem.state = NSControlStateValueOn;
        NSMenuItem *settingsItem = [g_menu addItemWithTitle:L(@"设置...", @"Settings...") action:@selector(showSettingsPanel) keyEquivalent:@""];
        settingsItem.target = g_menuHandler;
        [g_menu addItem:[NSMenuItem separatorItem]];
        NSMenuItem *langItem = [g_menu addItemWithTitle:L(@"English", @"中文") action:@selector(toggleLanguage) keyEquivalent:@""];
        langItem.target = g_menuHandler;
        NSMenuItem *quitItem = [g_menu addItemWithTitle:L(@"退出", @"Quit") action:@selector(quitApp) keyEquivalent:@""];
        quitItem.target = g_menuHandler;

        // Set target for action items (save, clear)
        for (NSMenuItem *item in [g_menu itemArray]) {
            if (!item.isSeparatorItem && !item.target
                && item.action != @selector(selectColor:)
                && item.action != @selector(selectWidth:)) {
                item.target = g_menuHandler;
            }
        }

        [g_statusItem setMenu:g_menu];

        NSScreen *screen = [NSScreen mainScreen];
        NSRect screenFrame = [screen frame];

        // Store screen dimensions for DB
        g_screen_w = (int)screenFrame.size.width;
        g_screen_h = (int)screenFrame.size.height;
        glaspen2_init_db(g_screen_w, g_screen_h);

        // Restore saved pen color and width
        double sr, sg, sb, sw;
        if (glaspen2_load_settings_parts(&sr, &sg, &sb, &sw)) {
            g_pen_r = sr; g_pen_g = sg; g_pen_b = sb; g_width_scale = sw;
            // Find closest matching color preset
            int bestColor = 0;
            double bestDist = 1e9;
            for (int i = 0; i < g_color_preset_count; i++) {
                double dr = g_color_presets[i].r - sr;
                double dg = g_color_presets[i].g - sg;
                double db = g_color_presets[i].b - sb;
                double dist = dr*dr + dg*dg + db*db;
                if (dist < bestDist) { bestDist = dist; bestColor = i; }
            }
            g_selectedColorIndex = bestColor;
            // Find closest matching width preset
            int bestWidth = 2;
            bestDist = 1e9;
            for (int i = 0; i < g_width_preset_count; i++) {
                double d = g_width_presets[i] - sw;
                if (d*d < bestDist) { bestDist = d*d; bestWidth = i; }
            }
            g_selected_width_index = bestWidth;
        }
        update_status_icon_state();
        update_menu_checkmarks();

        // Restore glass settings (opacity stored as millipercent, enabled as bool)
        int glass_milli = glaspen2_load_bool_setting("glass_alpha");
        if (glass_milli > 0) g_glass_opacity = glass_milli / 1000.0;
        g_glass_enabled = glaspen2_load_bool_setting("glass_enabled") != 0;
        gl_glass_apply();

        // Restore grid setting
        g_show_grid = glaspen2_load_bool_setting("grid") != 0;
        g_grid_follow_strokes = glaspen2_load_bool_setting("grid_follow_strokes") != 0;

        // Canvas mode starts in 固定画布涂鸦模式; menu item shows the switch target
        [[g_menu itemWithTag:778] setTitle:L(@"飘渺画布涂鸦模式", @"Ethereal canvas mode")];

        // Restore OCR feature setting (off by default — models download on demand)
        g_ocr_enabled = glaspen2_load_bool_setting("ocr_enabled") != 0;
        NSMenuItem *restoredOcrItem = [g_menu itemWithTag:666];
        if (restoredOcrItem) {
            [restoredOcrItem setState:g_ocr_enabled ? NSControlStateValueOn : NSControlStateValueOff];
            [restoredOcrItem setTitle:g_ocr_enabled
                ? L(@"关闭 OCR 识别", @"Disable OCR")
                : L(@"开启 OCR 识别 (需下载模型)", @"Enable OCR (downloads models)")];
        }

        // Restore pressure monitor setting
        g_pressure_monitor = glaspen2_load_bool_setting("pressure_monitor") != 0;
        if (g_pressure_monitor) {
            dispatch_async(dispatch_get_main_queue(), ^{
                pm_ensure_window();
                pm_show();
                pm_update();
            });
        }

        // Restore GIF quality/speed settings
        char *vfps = glaspen2_load_string_setting("gif_fps");
        if (vfps) { g_gif_fps = atoi(vfps); glaspen2_free_c_string(vfps); }
        char *vres = glaspen2_load_string_setting("gif_resolution");
        if (vres) { g_gif_resolution = atof(vres); glaspen2_free_c_string(vres); }
        char *vspeed = glaspen2_load_string_setting("gif_speed");
        if (vspeed) { g_gif_speed = atof(vspeed); glaspen2_free_c_string(vspeed); }
        char *vend = glaspen2_load_string_setting("gif_end_mode");
        if (vend) { g_gif_end_mode = atoi(vend); glaspen2_free_c_string(vend); }
        if (g_gif_fps < 1) g_gif_fps = 1;
        if (g_gif_fps > 50) g_gif_fps = 50;
        if (g_gif_resolution < 0.1) g_gif_resolution = 0.1;
        if (g_gif_resolution > 1.0) g_gif_resolution = 1.0;
        if (g_gif_speed < 0.25) g_gif_speed = 0.25;
        if (g_gif_speed > 10.0) g_gif_speed = 10.0;
        if (g_gif_end_mode < 0) g_gif_end_mode = 0;
        if (g_gif_end_mode > 2) g_gif_end_mode = 2;

        // Apply glass visual on startup (skip if the user already started drawing)
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 300 * NSEC_PER_MSEC), dispatch_get_main_queue(), ^{
            if (g_stroke_active) return;
            if (!g_surface && g_draw_view) ensure_surface(g_draw_view);
            rebuild_surface_from_strokes();
        });

        g_window = [[NSWindow alloc]
            initWithContentRect:screenFrame
            styleMask:NSWindowStyleMaskBorderless
            backing:NSBackingStoreBuffered
            defer:NO];

        [g_window setLevel:kCGMaximumWindowLevel];
        [g_window setOpaque:NO];
        [g_window setBackgroundColor:[NSColor clearColor]];
        [g_window setTitle:@"glaspen2"];
        [g_window setAcceptsMouseMovedEvents:YES];
        [g_window setCollectionBehavior:NSWindowCollectionBehaviorCanJoinAllSpaces |
                                       NSWindowCollectionBehaviorStationary];

        // Make click-through

        // Create blank cursor for hiding system cursor on our window only
        NSImage *blankImg = [[NSImage alloc] initWithSize:NSMakeSize(1, 1)];
        [blankImg lockFocus];
        [[NSColor clearColor] setFill];
        NSRectFill(NSMakeRect(0, 0, 1, 1));
        [blankImg unlockFocus];
        g_blank_cursor = [[NSCursor alloc] initWithImage:blankImg hotSpot:NSZeroPoint];
        g_arrow_cursor = [NSCursor arrowCursor];
        [g_window setIgnoresMouseEvents:YES];

        // Container view for glass + drawing layers
        NSView *contentView = [[NSView alloc] initWithFrame:screenFrame];
        [contentView setWantsLayer:YES];

        // Frosted glass layer (behind drawing)
        g_glass_view = [[NSVisualEffectView alloc] initWithFrame:screenFrame];
        [g_glass_view setBlendingMode:NSVisualEffectBlendingModeBehindWindow];
        [g_glass_view setMaterial:NSVisualEffectMaterialLight];
        [g_glass_view setState:NSVisualEffectStateActive];
        double vis = g_glass_enabled ? g_glass_opacity * 2.0 : 0.0;
        g_glass_view.alphaValue = vis;
        g_glass_view.hidden = !g_glass_enabled;
        [contentView addSubview:g_glass_view];

        // Drawing view on top
        GlaspenDrawView *drawView = [[GlaspenDrawView alloc] initWithFrame:screenFrame];
        [drawView setWantsLayer:YES];
        CALayer *layer = [drawView layer];
        if (layer) {
            [layer setOpaque:NO];
            [layer setBackgroundColor:[[NSColor clearColor] CGColor]];
        }
        [contentView addSubview:drawView];

        [g_window setContentView:contentView];
        [g_window orderFront:nil];

        g_draw_view = drawView;
        ensure_surface(drawView);

        NSLog(@"[glaspen2] window ready %dx%d, ignoresMouseEvents=%d",
              (int)screenFrame.size.width, (int)screenFrame.size.height,
              [g_window ignoresMouseEvents]);

        // Register signal handlers for graceful exit
        signal(SIGINT, save_and_exit);   // Ctrl+C
        signal(SIGTERM, save_and_exit);  // kill command

        // Listen for display changes (resolution, arrangement, etc.)
        [[NSNotificationCenter defaultCenter]
            addObserverForName:NSApplicationDidChangeScreenParametersNotification
            object:nil
            queue:[NSOperationQueue mainQueue]
            usingBlock:^(NSNotification *note) { on_display_changed(); }];

        // CGEventTap: intercept events at system level before dispatch
        CGEventMask tapMask = CGEventMaskBit(kCGEventMouseMoved) |
                              CGEventMaskBit(kCGEventLeftMouseDown) |
                              CGEventMaskBit(kCGEventLeftMouseDragged) |
                              CGEventMaskBit(kCGEventLeftMouseUp) |
                              CGEventMaskBit(kCGEventRightMouseDown) |
                              CGEventMaskBit(kCGEventRightMouseDragged) |
                              CGEventMaskBit(kCGEventRightMouseUp) |
                              CGEventMaskBit(kCGEventOtherMouseDown) |
                              CGEventMaskBit(kCGEventOtherMouseDragged) |
                              CGEventMaskBit(kCGEventOtherMouseUp) |
                              CGEventMaskBit(kCGEventTabletProximity) |
                              CGEventMaskBit(kCGEventTabletPointer) |
                              CGEventMaskBit(kCGEventKeyDown) |
                              CGEventMaskBit(kCGEventKeyUp);

        g_event_tap = CGEventTapCreate(kCGSessionEventTap, kCGHeadInsertEventTap,
                                       kCGEventTapOptionDefault, tapMask,
                                       event_tap_callback, NULL);

        if (g_event_tap) {
            CFRunLoopSourceRef source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, g_event_tap, 0);
            CFRunLoopAddSource(CFRunLoopGetMain(), source, kCFRunLoopCommonModes);
            CGEventTapEnable(g_event_tap, true);
            CFRelease(source);
            NSLog(@"[glaspen2] CGEventTap created OK, enabled=%d", CGEventTapIsEnabled(g_event_tap));
        } else {
            NSString *bundlePath = [[NSBundle mainBundle] bundlePath];
            NSLog(@"[glaspen2] CGEventTap FAILED - need Accessibility permission for: %@", bundlePath);
            // Show alert and open accessibility settings
            dispatch_async(dispatch_get_main_queue(), ^{
                NSAlert *alert = [[NSAlert alloc] init];
                alert.messageText = L(@"需要辅助功能权限", @"Accessibility Permission Required");
                alert.informativeText = L(
                    @"请在系统设置 → 隐私与安全性 → 辅助功能中添加并勾选 glaspen2。\n\n添加后请点击下方按钮重启应用。",
                    @"Please add and enable glaspen2 in System Settings → Privacy & Security → Accessibility.\n\nAfter enabling, click below to restart.");
                [alert addButtonWithTitle:L(@"打开系统设置并重启", @"Open Settings & Restart")];
                [alert addButtonWithTitle:L(@"取消", @"Cancel")];
                if ([alert runModal] == NSAlertFirstButtonReturn) {
                    [[NSWorkspace sharedWorkspace] openURL:
                        [NSURL URLWithString:@"x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"]];
                    // Relaunch after a short delay
                    NSString *path = bundlePath;
                    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(0.5 * NSEC_PER_SEC)),
                        dispatch_get_main_queue(), ^{
                            [[NSWorkspace sharedWorkspace] launchApplication:path];
                            [NSApp terminate:nil];
                        });
                }
            });
        }

        [NSApp run];
    }
}
