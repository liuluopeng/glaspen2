#import <Cocoa/Cocoa.h>
#import <QuartzCore/QuartzCore.h>
#import <Carbon/Carbon.h>
#import <ScreenCaptureKit/ScreenCaptureKit.h>
#import <CoreMedia/CoreMedia.h>
#import <CoreVideo/CoreVideo.h>
#import <IOSurface/IOSurface.h>
#include <signal.h>
#include <string.h>

// App enabled state
static BOOL g_enabled = YES;

// Screen dimensions (set once at startup)
static int g_screen_w = 1920;
static int g_screen_h = 1080;

// Forward declarations
static void flush_to_layer(void);
static void clear_screen(void);
static void draw_rainbow_indicator(void);
static void start_inverse_timer(void);
static void stop_inverse_timer(void);
static void rebuild_surface_from_strokes(void);
static void update_inverse_colors(void);
static void sample_bg_inverse(double px, double py, double *out_r, double *out_g, double *out_b);

// Per-stroke inverse colors (ObjC side, continuously updated by timer)
#define MAX_INVERSE_STROKES 1024
static double *g_inverse_colors[MAX_INVERSE_STROKES] = {0};
static int g_inverse_color_counts[MAX_INVERSE_STROKES] = {0};
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
extern void glaspen2_clear_strokes(int screen_w, int screen_h);
extern void glaspen2_init_db(int screen_w, int screen_h);
extern void glaspen2_save_settings(double r, double g, double b, double width_scale);
extern int  glaspen2_load_settings_parts(double *r, double *g, double *b, double *w);
extern void glaspen2_save_bool_setting(const char *key, int val);
extern int  glaspen2_load_bool_setting(const char *key);

// Modeler FFI
extern void glaspen2_modeler_begin(double r, double g, double b, double x, double y, double pressure, double timestamp, double width_scale);
extern void glaspen2_modeler_move(double x, double y, double pressure, double timestamp, double width_scale);
extern void glaspen2_modeler_end(double x, double y, double pressure, double timestamp, double width_scale);
extern int glaspen2_modeler_point_count(void);
extern void glaspen2_modeler_get_point(int idx, double *x, double *y, double *w);
extern void glaspen2_modeler_clear_buffer(void);
extern void glaspen2_modeler_commit_to_strokes(double r, double g, double b, const double *inv_colors, int inv_count);
extern int glaspen2_get_stroke_point_color(int idx, int pidx, double *r, double *g, double *b);
extern int glaspen2_stroke_bbox(double *x_min, double *y_min, double *x_max, double *y_max);
extern void glaspen2_save_svg(void);
extern int glaspen2_save_gif_cropped(const unsigned char *surface_data, int w, int h, int stride);

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

// Forward declarations
static void rebuild_surface_from_strokes(void);
static NSWindow *g_window = nil;

// --- Drawing state ---
static cairo_surface_t *g_surface = NULL;
static double g_last_x = -1, g_last_y = -1;
static BOOL g_has_last = NO;
static NSView *g_draw_view = nil;

// Raw drawing state (for responsive real-time feedback during stroke)
static double g_raw_last_x = 0, g_raw_last_y = 0;
static BOOL g_raw_has_last = NO;

// Cursor state
static double g_cursor_x = -100, g_cursor_y = -100;
static BOOL g_cursor_visible = NO;
static NSCursor *g_blank_cursor = nil;
static NSCursor *g_arrow_cursor = nil;

// Pen color state
static double g_pen_r = 1.0, g_pen_g = 0.0, g_pen_b = 0.0;

// Width scale presets
static double g_width_scale = 1.0;
static const double g_width_presets[] = { 0.3, 0.6, 1.0, 1.5, 2.5 };
static const int g_width_preset_count = 5;
static int g_selected_width_index = 2; // default: 1.0x

// Rainbow indicator toggle (default off)
static BOOL g_show_rainbow = NO;

// Outline enhancement toggle (default off)
static BOOL g_outline_enabled = NO;

// Inverse color mode toggle (experimental, default off)
static BOOL g_inverse_enabled = NO;

// Current stroke color (may differ from pen color in inverse mode)
static double g_stroke_r = 1.0, g_stroke_g = 0.0, g_stroke_b = 0.0;

// Counter for periodic screen re-capture during stroke

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

    // Use ScreenCaptureKit to capture screen
    [SCShareableContent getShareableContentWithCompletionHandler:^(SCShareableContent *content, NSError *error) {
        if (error || !content.displays.count) {
            NSLog(@"[glaspen2] Screen capture failed: %@", error);
            save_drawing_only();
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
                dispatch_async(dispatch_get_main_queue(), ^{
                    show_notification(L(@"截图失败，已保存涂鸦", @"Screenshot failed, drawing saved"));
                });
                save_drawing_only();
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

            // Convert cairo surface (sRGB) to Display P3
            cairo_surface_flush(g_surface);
            int dw = cairo_image_surface_get_width(g_surface);
            int dh = cairo_image_surface_get_height(g_surface);
            int dstride = cairo_image_surface_get_stride(g_surface);

            CGColorSpaceRef srgb = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
            CGDataProviderRef drawProvider = CGDataProviderCreateWithData(NULL,
                cairo_image_surface_get_data(g_surface), dstride * dh, NULL);
            CGImageRef drawImage = CGImageCreate(dw, dh, 8, 32, dstride, srgb,
                kCGBitmapByteOrder32Little | kCGImageAlphaPremultipliedLast,
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
    cairo_t *cr = cairo_create(g_surface);
    cairo_set_operator(cr, CAIRO_OPERATOR_CLEAR);
    cairo_paint(cr);
    cairo_destroy(cr);
    g_has_last = NO;
    // Clear inverse color data
    for (int i = 0; i < MAX_INVERSE_STROKES; i++) {
        if (g_inverse_colors[i]) { free(g_inverse_colors[i]); g_inverse_colors[i] = NULL; }
        g_inverse_color_counts[i] = 0;
    }
    glaspen2_clear_strokes(g_screen_w, g_screen_h);
    if (g_show_rainbow) draw_rainbow_indicator();
    flush_to_layer();
    show_notification(L(@"清屏成功", @"Screen cleared"));
}

/// Draw smoothed points from the modeler buffer onto the canvas, then commit to STROKES.
static void draw_modeler_buffer(void) {
    if (!g_surface) return;
    int count = glaspen2_modeler_point_count();
    NSLog(@"[glaspen2] draw_modeler_buffer: %d points", count);
    if (count < 1) return;

    double px, py, pw;
    double prev_x, prev_y, prev_w;

    // First point: draw as a dot
    glaspen2_modeler_get_point(0, &prev_x, &prev_y, &prev_w);
    cairo_t *cr = cairo_create(g_surface);
    cairo_set_source_rgba(cr, g_pen_r, g_pen_g, g_pen_b, 1.0);
    cairo_arc(cr, prev_x, prev_y, prev_w * 0.5, 0, 2 * M_PI);
    cairo_fill(cr);
    cairo_destroy(cr);

    // Subsequent points: draw line segments
    for (int i = 1; i < count; i++) {
        glaspen2_modeler_get_point(i, &px, &py, &pw);
        cairo_t *cr = cairo_create(g_surface);
        cairo_set_source_rgba(cr, g_pen_r, g_pen_g, g_pen_b, 1.0);
        cairo_set_line_width(cr, pw);
        cairo_set_line_cap(cr, CAIRO_LINE_CAP_ROUND);
        cairo_set_line_join(cr, CAIRO_LINE_JOIN_ROUND);
        cairo_move_to(cr, prev_x, prev_y);
        cairo_line_to(cr, px, py);
        cairo_stroke(cr);
        cairo_destroy(cr);
        prev_x = px; prev_y = py; prev_w = pw;
    }

    // Commit buffer to STROKES (takes and clears buffer)
    glaspen2_modeler_commit_to_strokes(g_pen_r, g_pen_g, g_pen_b, NULL, 0);
    flush_to_layer();
}

static void replay_strokes_from_memory(void) {
    if (!g_surface) return;

    // Clear canvas
    cairo_t *cr = cairo_create(g_surface);
    cairo_set_operator(cr, CAIRO_OPERATOR_CLEAR);
    cairo_paint(cr);
    cairo_destroy(cr);

    int count = glaspen2_stroke_count();
    for (int si = 0; si < count; si++) {
        double r, g, b;
        glaspen2_get_stroke_color(si, &r, &g, &b);
        int pc = glaspen2_get_stroke_point_count(si);
        if (pc < 2) continue;

        // Draw each segment with its own per-point width
        double x0, y0, x1, y1, w;
        for (int pi = 1; pi < pc; pi++) {
            glaspen2_get_stroke_point(si, pi - 1, &x0, &y0);
            glaspen2_get_stroke_point(si, pi, &x1, &y1);
            w = glaspen2_get_stroke_point_width(si, pi);

            cairo_t *cr = cairo_create(g_surface);
            cairo_set_source_rgba(cr, r, g, b, 1.0);
            cairo_set_line_width(cr, w);
            cairo_set_line_cap(cr, CAIRO_LINE_CAP_ROUND);
            cairo_set_line_join(cr, CAIRO_LINE_JOIN_ROUND);
            cairo_move_to(cr, x0, y0);
            cairo_line_to(cr, x1, y1);
            cairo_stroke(cr);
            cairo_destroy(cr);
        }
    }

    g_has_last = NO;
    if (g_show_rainbow) draw_rainbow_indicator();
    flush_to_layer();
}

static void save_and_exit(int sig) {
    (void)sig;
    CGDisplayShowCursor(kCGDirectMainDisplay);
    exit(0);
}

static void draw_rainbow_indicator(void) {
    if (!g_surface) return;
    cairo_t *cr = cairo_create(g_surface);
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
    [[g_menu itemAtIndex:base+3] setTitle:L(@"清屏", @"Clear screen")];
    [[g_menu itemAtIndex:base+4] setTitle:L(@"彩虹指示器", @"Rainbow indicator")];
    [[g_menu itemAtIndex:base+5] setTitle:L(@"开机自启", @"Launch at login")];
    [[g_menu itemAtIndex:base+6] setTitle:L(@"描边增强", @"Outline")];
    [[g_menu itemAtIndex:base+7] setTitle:L(@"反色模式", @"Inverse color")];
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

- (void)toggleLanguage {
    g_lang = 1 - g_lang;
    update_menu_texts();
}

- (void)toggleRainbow {
    g_show_rainbow = !g_show_rainbow;
    NSMenuItem *item = [g_menu itemWithTag:999];
    [item setState:g_show_rainbow ? NSControlStateValueOn : NSControlStateValueOff];
    if (g_show_rainbow) {
        draw_rainbow_indicator();
    } else {
        clear_screen();
    }
}

- (void)toggleLaunch {
    int cur = glaspen2_is_launch_at_login();
    int ok = glaspen2_set_launch_at_login(!cur);
    if (ok) {
        NSMenuItem *item = [g_menu itemWithTag:777];
        [item setState:(!cur) ? NSControlStateValueOn : NSControlStateValueOff];
    }
}

- (void)toggleOutline {
    g_outline_enabled = !g_outline_enabled;
    NSMenuItem *item = [g_menu itemWithTag:666];
    [item setState:g_outline_enabled ? NSControlStateValueOn : NSControlStateValueOff];
    glaspen2_save_bool_setting("outline_enabled", g_outline_enabled ? 1 : 0);
    rebuild_surface_from_strokes();
}

- (void)toggleInverse {
    g_inverse_enabled = !g_inverse_enabled;
    NSMenuItem *item = [g_menu itemWithTag:555];
    [item setState:g_inverse_enabled ? NSControlStateValueOn : NSControlStateValueOff];
    glaspen2_save_bool_setting("inverse_enabled", g_inverse_enabled ? 1 : 0);
    if (g_inverse_enabled) {
        start_inverse_timer();
    } else {
        stop_inverse_timer();
    }
}

- (void)selectColor:(NSMenuItem *)sender {
    int idx = (int)[sender tag];
    if (idx >= 0 && idx < g_color_preset_count) {
        g_pen_r = g_color_presets[idx].r;
        g_pen_g = g_color_presets[idx].g;
        g_pen_b = g_color_presets[idx].b;
        g_selectedColorIndex = idx;
        glaspen2_save_settings(g_pen_r, g_pen_g, g_pen_b, g_width_scale);
        update_menu_checkmarks();
    }
}

- (void)selectWidth:(NSMenuItem *)sender {
    int idx = (int)[sender tag];
    if (idx >= 0 && idx < g_width_preset_count) {
        g_width_scale = g_width_presets[idx];
        g_selected_width_index = idx;
        glaspen2_save_settings(g_pen_r, g_pen_g, g_pen_b, g_width_scale);
        update_menu_checkmarks();
    }
}

// NSApplicationDelegate
- (NSApplicationTerminateReply)applicationShouldTerminate:(NSApplication *)sender {
    return NSTerminateNow;
}

- (void)quitApp {
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

static void ensure_surface(NSView *view) {
    NSRect bounds = [view bounds];
    int w = (int)bounds.size.width;
    int h = (int)bounds.size.height;
    if (g_surface && cairo_image_surface_get_width(g_surface) == w &&
        cairo_image_surface_get_height(g_surface) == h) return;

    if (g_surface) cairo_surface_destroy(g_surface);
    g_surface = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, w, h);
    cairo_t *cr = cairo_create(g_surface);
    cairo_set_operator(cr, CAIRO_OPERATOR_CLEAR);
    cairo_paint(cr);
    cairo_set_operator(cr, CAIRO_OPERATOR_OVER);
    cairo_destroy(cr);
    g_has_last = NO;
}

static void flush_to_layer(void) {
    if (!g_surface || !g_draw_view) return;
    [g_draw_view setNeedsDisplay:YES];
}

static void pen_draw(double x, double y, double width) {
    if (!g_surface) return;
    cairo_t *cr = cairo_create(g_surface);
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

// --- Outline & inverse color helpers ---

static void contrast_color(double r, double g, double b,
                           double *out_r, double *out_g, double *out_b) {
    double lum = 0.299 * r + 0.587 * g + 0.114 * b;
    if (lum > 0.5) { *out_r = 0; *out_g = 0; *out_b = 0; }
    else           { *out_r = 1; *out_g = 1; *out_b = 1; }
}

// --- Screen capture via SCScreenshotManager ---
static CGImageRef g_captured_image = nil;
static NSObject *g_capture_lock = nil;
static dispatch_source_t g_inverse_timer = nil;
static BOOL g_capture_pending = NO;

static void capture_screen_async(void) {
    if (g_capture_pending) return;
    g_capture_pending = YES;
    [SCShareableContent getShareableContentWithCompletionHandler:^(SCShareableContent *content, NSError *error) {
        if (!content || error) { g_capture_pending = NO; return; }
        SCDisplay *display = [[content displays] firstObject];
        if (!display) { g_capture_pending = NO; return; }
        SCContentFilter *filter = [[SCContentFilter alloc] initWithDisplay:display excludingWindows:@[]];
        SCStreamConfiguration *config = [[SCStreamConfiguration alloc] init];
        [config setWidth:display.width];
        [config setHeight:display.height];
        [config setCapturesAudio:NO];
        [SCScreenshotManager captureImageWithFilter:filter configuration:config completionHandler:^(CGImageRef img, NSError *err) {
            if (img) {
                CGImageRef copy = CGImageCreateCopy(img);
                @synchronized(g_capture_lock) {
                    if (g_captured_image) CGImageRelease(g_captured_image);
                    g_captured_image = copy;
                }
            }
            g_capture_pending = NO;
        }];
    }];
}

static void init_display_stream(void) {
    NSLog(@"[glaspen2] init_display_stream: starting...");
    g_capture_lock = [[NSObject alloc] init];
    BOOL preflight = CGPreflightScreenCaptureAccess();
    NSLog(@"[glaspen2] Screen capture preflight: %d", preflight);
    if (!preflight) CGRequestScreenCaptureAccess();
    capture_screen_async();
}

/// Sample inverse color from captured image — single pixel, no crop overhead.
static void sample_bg_inverse(double px, double py,
                              double *out_r, double *out_g, double *out_b) {
    CGImageRef img = nil;
    @synchronized(g_capture_lock) {
        img = g_captured_image;
        if (img) CFRetain(img);
    }
    if (!img) { *out_r = 1; *out_g = 1; *out_b = 1; return; }

    int fullW = (int)CGImageGetWidth(img);
    int fullH = (int)CGImageGetHeight(img);
    if (fullW < 1 || fullH < 1) { CFRelease(img); *out_r = 1; *out_g = 1; *out_b = 1; return; }

    // Single pixel crop — fast
    int sx = (int)px;
    int sy = fullH - 1 - (int)py; // flip Y (CGImage top-left origin)
    if (sx < 0) sx = 0; if (sy < 0) sy = 0;
    if (sx >= fullW) sx = fullW - 1;
    if (sy >= fullH) sy = fullH - 1;

    CGImageRef cropped = CGImageCreateWithImageInRect(img, CGRectMake(sx, sy, 1, 1));
    CFRelease(img);
    if (!cropped) { *out_r = 1; *out_g = 1; *out_b = 1; return; }

    uint8_t buf[4] = {0};
    CGColorSpaceRef cs = CGColorSpaceCreateDeviceRGB();
    CGContextRef ctx = CGBitmapContextCreate(buf, 1, 1, 8, 4, cs,
        kCGImageAlphaPremultipliedLast | kCGBitmapByteOrder32Big);
    CGContextDrawImage(ctx, CGRectMake(0, 0, 1, 1), cropped);
    CGContextRelease(ctx);
    CGImageRelease(cropped);
    CGColorSpaceRelease(cs);

    *out_r = 1.0 - (double)buf[0] / 255.0;
    *out_g = 1.0 - (double)buf[1] / 255.0;
    *out_b = 1.0 - (double)buf[2] / 255.0;
}

/// Re-sample inverse colors for all strokes and rebuild surface.
static void update_inverse_colors(void) {
    @synchronized(g_capture_lock) {
        if (!g_captured_image) return;
    }
    capture_screen_async(); // refresh for next tick

    int n_strokes = glaspen2_stroke_count();
    for (int s = 0; s < n_strokes && s < MAX_INVERSE_STROKES; s++) {
        int n_pts = glaspen2_get_stroke_point_count(s);
        if (n_pts < 1 || !g_inverse_colors[s]) continue;
        int pts = n_pts < g_inverse_color_counts[s] ? n_pts : g_inverse_color_counts[s];
        for (int i = 0; i < pts; i++) {
            double x, y;
            glaspen2_get_stroke_point(s, i, &x, &y);
            sample_bg_inverse(x, y, &g_inverse_colors[s][i*3], &g_inverse_colors[s][i*3+1], &g_inverse_colors[s][i*3+2]);
        }
    }
    rebuild_surface_from_strokes();
}

/// Start periodic inverse color update timer (100ms interval).
static void start_inverse_timer(void) {
    if (g_inverse_timer) return;
    g_inverse_timer = dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER, 0, 0, dispatch_get_main_queue());
    dispatch_source_set_timer(g_inverse_timer, DISPATCH_TIME_NOW, 33 * NSEC_PER_MSEC, 5 * NSEC_PER_MSEC);
    dispatch_source_set_event_handler(g_inverse_timer, ^{
        update_inverse_colors();
    });
    dispatch_resume(g_inverse_timer);
}

static void stop_inverse_timer(void) {
    if (g_inverse_timer) {
        dispatch_source_cancel(g_inverse_timer);
        g_inverse_timer = nil;
    }
}

// Raw drawing — surface only, no STROKES/DB side effects
static void raw_draw_dot(double x, double y, double width) {
    if (!g_surface) return;
    cairo_t *cr = cairo_create(g_surface);
    // Outline pass
    if (g_outline_enabled) {
        double or, og, ob;
        contrast_color(g_stroke_r, g_stroke_g, g_stroke_b, &or, &og, &ob);
        double extra = fmax(width * 0.4, 2.0);
        cairo_set_source_rgba(cr, or, og, ob, 1.0);
        cairo_arc(cr, x, y, (width + extra) * 0.5, 0, 2 * M_PI);
        cairo_fill(cr);
    }
    // Main dot
    cairo_set_source_rgba(cr, g_stroke_r, g_stroke_g, g_stroke_b, 1.0);
    cairo_arc(cr, x, y, width * 0.5, 0, 2 * M_PI);
    cairo_fill(cr);
    cairo_destroy(cr);
    flush_to_layer();
}

static void raw_draw_segment(double x, double y, double width) {
    if (!g_surface) return;
    cairo_t *cr = cairo_create(g_surface);
    double extra = g_outline_enabled ? fmax(width * 0.4, 2.0) : 0;
    // Outline pass
    if (g_outline_enabled) {
        double or, og, ob;
        contrast_color(g_stroke_r, g_stroke_g, g_stroke_b, &or, &og, &ob);
        cairo_set_source_rgba(cr, or, og, ob, 1.0);
        cairo_set_line_width(cr, width + extra);
        cairo_set_line_cap(cr, CAIRO_LINE_CAP_ROUND);
        cairo_set_line_join(cr, CAIRO_LINE_JOIN_ROUND);
        if (g_raw_has_last) {
            cairo_move_to(cr, g_raw_last_x, g_raw_last_y);
            cairo_line_to(cr, x, y);
            cairo_stroke(cr);
        } else {
            cairo_arc(cr, x, y, (width + extra) * 0.5, 0, 2 * M_PI);
            cairo_fill(cr);
        }
    }
    // Main stroke
    cairo_set_source_rgba(cr, g_stroke_r, g_stroke_g, g_stroke_b, 1.0);
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
    cairo_destroy(cr);
    g_raw_last_x = x;
    g_raw_last_y = y;
    g_raw_has_last = YES;
    flush_to_layer();
}

static void rebuild_surface_from_strokes(void) {
    if (!g_surface) return;
    // Clear surface
    cairo_t *cr = cairo_create(g_surface);
    cairo_set_operator(cr, CAIRO_OPERATOR_CLEAR);
    cairo_paint(cr);
    cairo_destroy(cr);

    // Redraw rainbow if enabled
    if (g_show_rainbow) draw_rainbow_indicator();

    int n_strokes = glaspen2_stroke_count();
    for (int s = 0; s < n_strokes; s++) {
        int n_pts = glaspen2_get_stroke_point_count(s);
        if (n_pts < 1) continue;
        double r, gg, b;
        glaspen2_get_stroke_color(s, &r, &gg, &b);

        // Collect points and per-point colors (ObjC-side for continuous updates)
        double px[2048], py[2048], pw[2048];
        double pcr[2048], pcg[2048], pcb[2048];
        BOOL has_point_colors = NO;
        int pts = n_pts < 2048 ? n_pts : 2048;
        for (int i = 0; i < pts; i++) {
            glaspen2_get_stroke_point(s, i, &px[i], &py[i]);
            pw[i] = glaspen2_get_stroke_point_width(s, i);
            // Use ObjC-side inverse colors (continuously updated by timer)
            if (s < MAX_INVERSE_STROKES && g_inverse_colors[s] && i < g_inverse_color_counts[s]) {
                pcr[i] = g_inverse_colors[s][i*3];
                pcg[i] = g_inverse_colors[s][i*3+1];
                pcb[i] = g_inverse_colors[s][i*3+2];
                has_point_colors = YES;
            } else {
                pcr[i] = r; pcg[i] = gg; pcb[i] = b;
            }
        }

        // Outline pass (if enabled)
        if (g_outline_enabled) {
            cr = cairo_create(g_surface);
            for (int i = 0; i < pts; i++) {
                double or, og, ob;
                contrast_color(pcr[i], pcg[i], pcb[i], &or, &og, &ob);
                double extra = fmax(pw[i] * 0.4, 2.0);
                cairo_set_source_rgba(cr, or, og, ob, 1.0);
                if (i == 0) {
                    cairo_arc(cr, px[i], py[i], (pw[i] + extra) * 0.5, 0, 2 * M_PI);
                    cairo_fill(cr);
                } else {
                    cairo_set_line_width(cr, pw[i] + extra);
                    cairo_set_line_cap(cr, CAIRO_LINE_CAP_ROUND);
                    cairo_set_line_join(cr, CAIRO_LINE_JOIN_ROUND);
                    cairo_move_to(cr, px[i-1], py[i-1]);
                    cairo_line_to(cr, px[i], py[i]);
                    cairo_stroke(cr);
                }
            }
            cairo_destroy(cr);
        }

        // Main stroke pass
        cr = cairo_create(g_surface);
        for (int i = 0; i < pts; i++) {
            cairo_set_source_rgba(cr, pcr[i], pcg[i], pcb[i], 1.0);
            if (i == 0) {
                cairo_arc(cr, px[i], py[i], pw[i] * 0.5, 0, 2 * M_PI);
                cairo_fill(cr);
            } else {
                cairo_set_line_width(cr, pw[i]);
                cairo_set_line_cap(cr, CAIRO_LINE_CAP_ROUND);
                cairo_set_line_join(cr, CAIRO_LINE_JOIN_ROUND);
                cairo_move_to(cr, px[i-1], py[i-1]);
                cairo_line_to(cr, px[i], py[i]);
                cairo_stroke(cr);
            }
        }
        cairo_destroy(cr);
    }
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

    CGColorSpaceRef cs = CGColorSpaceCreateDeviceRGB();
    CGDataProviderRef provider = CGDataProviderCreateWithData(NULL, data, stride * h, NULL);
    CGImageRef image = CGImageCreate(w, h, 8, 32, stride, cs,
                                      kCGBitmapByteOrder32Little | kCGImageAlphaPremultipliedFirst,
                                      provider, NULL, false, kCGRenderingIntentDefault);
    CGDataProviderRelease(provider);
    CGColorSpaceRelease(cs);

    if (image) {
        CGContextRef ctx = [[NSGraphicsContext currentContext] CGContext];
        CGContextDrawImage(ctx, CGRectMake(0, 0, w, h), image);
        CGImageRelease(image);

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
            CGFloat x = (w - textSize.width) / 2;
            CGFloat y = (h - textSize.height) / 2;
            [g_notification drawAtPoint:NSMakePoint(x, y) withAttributes:attrs];
        }

        // Draw pen crosshair cursor
        if (g_cursor_visible && g_cursor_x >= 0) {
            CGFloat cx = g_cursor_x;
            CGFloat cy = g_cursor_y;
            CGFloat radius = 8.0;

            CGContextSaveGState(ctx);

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

            CGContextRestoreGState(ctx);
        }
    }
}

@end

// --- CGEventTap callback ---
static CGEventRef event_tap_callback(CGEventTapProxy proxy, CGEventType type,
                                      CGEventRef event, void *refcon) {
    // Re-enable tap if it gets disabled by timeout/user
    if (type == kCGEventTapDisabledByTimeout || type == kCGEventTapDisabledByUserInput) {
        CGEventTapEnable(g_event_tap, true);
        return event;
    }

    // Handle keyboard events separately — always intercept hotkeys
    if (type == kCGEventKeyDown) {
        NSEvent *keyEvent = [NSEvent eventWithCGEvent:event];
        if (keyEvent) {
            NSUInteger mods = [keyEvent modifierFlags];
            BOOL hasCmdCtrl = (mods & NSEventModifierFlagCommand) && (mods & NSEventModifierFlagControl);
            if (hasCmdCtrl) {
                unsigned short kc = [keyEvent keyCode];
                if (kc == kVK_ANSI_C) { clear_screen(); return NULL; }
                else if (kc == kVK_ANSI_V) { toggle_enabled(); return NULL; }
                else if (kc == 0x26) { // J
                    long target = glaspen2_prev_screen_id();
                    if (target > 0) {
                        glaspen2_load_strokes_for_screen(target);
                        glaspen2_smooth_loaded_strokes();
                        replay_strokes_from_memory();
                    } else {
                        show_notification(L(@"没有上一页", @"No previous page"));
                    }
                    return NULL;
                } else if (kc == 0x28) { // K
                    long target = glaspen2_next_screen_id();
                    if (target > 0) {
                        glaspen2_load_strokes_for_screen(target);
                        glaspen2_smooth_loaded_strokes();
                        replay_strokes_from_memory();
                    } else {
                        show_notification(L(@"没有下一页", @"No next page"));
                    }
                    return NULL;
                } else if (kc == kVK_ANSI_G) {
                    glaspen2_save_svg();
                    if (g_surface) {
                        cairo_surface_flush(g_surface);
                        const unsigned char *data = cairo_image_surface_get_data(g_surface);
                        int w = cairo_image_surface_get_width(g_surface);
                        int h = cairo_image_surface_get_height(g_surface);
                        int stride = cairo_image_surface_get_stride(g_surface);
                        if (glaspen2_save_gif_cropped(data, w, h, stride)) {
                            // Find newest glaspen2 GIF on desktop, copy file URL to clipboard
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
                    return NULL;
                }
            }
        }
        // Not a hotkey — pass through to system
        return event;
    }

    // All non-keyboard events: if app is disabled, pass through
    if (!g_enabled) return event;

    // Convert to NSEvent to check pen properties
    NSEvent *nsevent = [NSEvent eventWithCGEvent:event];
    if (!nsevent) return event;

    NSPointingDeviceType devType = [nsevent pointingDeviceType];
    NSInteger subtype = [nsevent subtype];
    NSEventType etype = [nsevent type];
    CGFloat pressure = [nsevent pressure];
    BOOL isPen = (devType == NSPenPointingDevice || devType == NSEraserPointingDevice ||
                  subtype == 1 || subtype == 2);

    // Update cursor position for pen events only
    if (isPen) {
        NSPoint loc = [nsevent locationInWindow];
        g_cursor_x = loc.x;
        g_cursor_y = loc.y;
        g_cursor_visible = YES;
        [g_draw_view setNeedsDisplay:YES];
    }

    // Hide system cursor while pen is drawing, restore on any mouse up
    static BOOL g_pen_drawing = NO;
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
        if (g_pen_drawing) {
            CGDisplayShowCursor(kCGDirectMainDisplay);
            g_pen_drawing = NO;
        }
        g_cursor_visible = NO;
        [g_draw_view setNeedsDisplay:YES];
    }

    // Check if click is on the menu bar
    if (etype == NSEventTypeLeftMouseDown) {
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
    static BOOL g_stroke_active = NO;

    // Width from pressure (same formula as Rust modeler::pressure_to_width)
    double raw_w = (pressure > 0.01) ? (0.3 + pressure * pressure * 7.7) * g_width_scale
                                     : 1.0 * g_width_scale;

    if (isPen && (etype == NSEventTypeLeftMouseDown || etype == NSEventTypeRightMouseDown ||
                  etype == NSEventTypeOtherMouseDown)) {
        // Pen down: determine stroke color, start modeler, draw raw dot immediately (no lag)
        NSLog(@"[glaspen2] pen DOWN at (%.1f, %.1f) p=%.2f ts=%.3f", px, py, pressure, ts);
        // modeler_begin always uses original pen color (for DB/STROKES)
        glaspen2_modeler_begin(g_pen_r, g_pen_g, g_pen_b, px, py, pressure, ts, g_width_scale);
        g_stroke_active = YES;
        // For raw drawing, use inverse color if enabled
        if (g_inverse_enabled) {
            sample_bg_inverse(px, py, &g_stroke_r, &g_stroke_g, &g_stroke_b);
        } else {
            g_stroke_r = g_pen_r; g_stroke_g = g_pen_g; g_stroke_b = g_pen_b;
        }
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
            glaspen2_modeler_begin(g_pen_r, g_pen_g, g_pen_b, px, py, pressure, ts, g_width_scale);
            g_stroke_active = YES;
            if (g_inverse_enabled) {
                sample_bg_inverse(px, py, &g_stroke_r, &g_stroke_g, &g_stroke_b);
            } else {
                g_stroke_r = g_pen_r; g_stroke_g = g_pen_g; g_stroke_b = g_pen_b;
            }
            raw_draw_dot(px, py, raw_w);
            g_raw_last_x = px;
            g_raw_last_y = py;
            g_raw_has_last = YES;
            return NULL; // begin already recorded this point, don't feed duplicate to modeler
        }
        // Per-point inverse color for real-time raw drawing feedback
        if (g_inverse_enabled) {
            static int move_count = 0;
            sample_bg_inverse(px, py, &g_stroke_r, &g_stroke_g, &g_stroke_b);
        }
        // Feed modeler, draw raw segment for responsive real-time feedback
        glaspen2_modeler_move(px, py, pressure, ts, g_width_scale);
        raw_draw_segment(px, py, raw_w);
        return NULL;
    }
    if (isPen && (etype == NSEventTypeLeftMouseUp || etype == NSEventTypeRightMouseUp ||
                  etype == NSEventTypeOtherMouseUp)) {
        // Pen up: finalize modeler, commit smoothed points, rebuild surface
        if (g_stroke_active) {
            glaspen2_modeler_end(px, py, pressure, ts, g_width_scale);

            if (g_inverse_enabled) {
                // Sample inverse color at each modeler output position
                int mcnt = glaspen2_modeler_point_count();
                double inv_buf[2048 * 3];
                int inv_n = mcnt < 2048 ? mcnt : 2048;
                for (int i = 0; i < inv_n; i++) {
                    double mx, my, mw;
                    glaspen2_modeler_get_point(i, &mx, &my, &mw);
                    sample_bg_inverse(mx, my, &inv_buf[i*3], &inv_buf[i*3+1], &inv_buf[i*3+2]);
                }
                glaspen2_modeler_commit_to_strokes(g_pen_r, g_pen_g, g_pen_b, inv_buf, inv_n);
                // Store in ObjC-side array for continuous timer updates
                int stroke_idx = glaspen2_stroke_count() - 1;
                if (stroke_idx >= 0 && stroke_idx < MAX_INVERSE_STROKES) {
                    if (g_inverse_colors[stroke_idx]) free(g_inverse_colors[stroke_idx]);
                    g_inverse_colors[stroke_idx] = (double *)malloc(inv_n * 3 * sizeof(double));
                    memcpy(g_inverse_colors[stroke_idx], inv_buf, inv_n * 3 * sizeof(double));
                    g_inverse_color_counts[stroke_idx] = inv_n;
                }
            } else {
                glaspen2_modeler_commit_to_strokes(g_pen_r, g_pen_g, g_pen_b, NULL, 0);
            }

            // Rebuild surface from smoothed STROKES data
            rebuild_surface_from_strokes();
            g_stroke_active = NO;
            g_raw_has_last = NO;
        }
        return NULL;
    }

    return event;
}

// --- App ---

void glaspen2_run(void) {
    @autoreleasepool {
        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];

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
        [g_menu addItemWithTitle:L(@"清屏", @"Clear screen") action:@selector(clearScreen) keyEquivalent:@""];
        NSMenuItem *rainbowItem = [g_menu addItemWithTitle:L(@"彩虹指示器", @"Rainbow indicator") action:@selector(toggleRainbow) keyEquivalent:@""];
        rainbowItem.target = g_menuHandler;
        rainbowItem.tag = 999;
        rainbowItem.state = NSControlStateValueOff;
        NSMenuItem *launchItem = [g_menu addItemWithTitle:L(@"开机自启", @"Launch at login") action:@selector(toggleLaunch) keyEquivalent:@""];
        launchItem.target = g_menuHandler;
        launchItem.tag = 777;
        launchItem.state = glaspen2_is_launch_at_login() ? NSControlStateValueOn : NSControlStateValueOff;
        NSMenuItem *outlineItem = [g_menu addItemWithTitle:L(@"描边增强", @"Outline") action:@selector(toggleOutline) keyEquivalent:@""];
        outlineItem.target = g_menuHandler;
        outlineItem.tag = 666;
        outlineItem.state = NSControlStateValueOff;
        NSMenuItem *inverseItem = [g_menu addItemWithTitle:L(@"反色模式", @"Inverse color") action:@selector(toggleInverse) keyEquivalent:@""];
        inverseItem.target = g_menuHandler;
        inverseItem.tag = 555;
        inverseItem.state = NSControlStateValueOff;
        [g_menu addItem:[NSMenuItem separatorItem]];
        NSMenuItem *toggleItem = [g_menu addItemWithTitle:L(@"开启涂鸦", @"Enable Drawing") action:@selector(toggleDraw) keyEquivalent:@""];
        toggleItem.target = g_menuHandler;
        toggleItem.tag = 888;
        toggleItem.state = NSControlStateValueOn;
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
        init_display_stream();

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

        // Restore outline and inverse settings
        g_outline_enabled = glaspen2_load_bool_setting("outline_enabled") != 0;
        g_inverse_enabled = glaspen2_load_bool_setting("inverse_enabled") != 0;
        [[g_menu itemWithTag:666] setState:g_outline_enabled ? NSControlStateValueOn : NSControlStateValueOff];
        [[g_menu itemWithTag:555] setState:g_inverse_enabled ? NSControlStateValueOn : NSControlStateValueOff];
        if (g_inverse_enabled) {
            start_inverse_timer();
        }

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

        GlaspenDrawView *drawView = [[GlaspenDrawView alloc] initWithFrame:screenFrame];
        [drawView setWantsLayer:YES];
        CALayer *layer = [drawView layer];
        if (layer) {
            [layer setOpaque:NO];
            [layer setBackgroundColor:[[NSColor clearColor] CGColor]];
        }
        [g_window setContentView:drawView];
        [g_window orderFront:nil];

        g_draw_view = drawView;
        ensure_surface(drawView);

        NSLog(@"[glaspen2] window ready %dx%d, ignoresMouseEvents=%d",
              (int)screenFrame.size.width, (int)screenFrame.size.height,
              [g_window ignoresMouseEvents]);

        // Register signal handlers for graceful exit
        signal(SIGINT, save_and_exit);   // Ctrl+C
        signal(SIGTERM, save_and_exit);  // kill command

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
                              CGEventMaskBit(kCGEventKeyDown);

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
