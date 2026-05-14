#import <Cocoa/Cocoa.h>
#import <QuartzCore/QuartzCore.h>
#import <Carbon/Carbon.h>
#import <ScreenCaptureKit/ScreenCaptureKit.h>
#include <signal.h>

// Hot key ID
enum {
    kHotKeyClearScreen = 1
};

// Forward declarations
static void flush_to_layer(void);
static void clear_screen(void);
static void draw_rainbow_indicator(void);

// Hot key handler function
static OSStatus hotKeyHandler(EventHandlerCallRef next, EventRef event, void *userData) {
    EventHotKeyID hotKey;
    GetEventParameter(event, kEventParamDirectObject, typeEventHotKeyID, NULL, sizeof(hotKey), NULL, &hotKey);
    if (hotKey.id == kHotKeyClearScreen) {
        clear_screen();
    }
    return noErr;
}

// --- Cairo (linked via cargo) ---
#include <cairo/cairo.h>

// --- Rust FFI ---
extern void glaspen2_save_drawing(const unsigned char *data, int width, int height, int stride);
extern void glaspen2_save_with_background(
    const unsigned char *drawing_data, int drawing_width, int drawing_height, int drawing_stride,
    const unsigned char *bg_data, int bg_width, int bg_height, int bg_stride);
extern void glaspen2_begin_stroke(double r, double g, double b);
extern void glaspen2_add_point(double x, double y, double width);
extern void glaspen2_end_stroke(void);
extern void glaspen2_save_xoj(void);
extern void glaspen2_clear_strokes(void);

// --- Drawing state ---
static cairo_surface_t *g_surface = NULL;
static double g_last_x = -1, g_last_y = -1;
static BOOL g_has_last = NO;
static NSView *g_draw_view = nil;

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
    glaspen2_clear_strokes();
    if (g_show_rainbow) draw_rainbow_indicator();
    flush_to_layer();
    show_notification(L(@"清屏成功", @"Screen cleared"));
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
    [[g_menu itemAtIndex:base+6] setTitle:L(@"English", @"中文")];
    [[g_menu itemAtIndex:base+7] setTitle:L(@"退出", @"Quit")];
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

- (void)selectColor:(NSMenuItem *)sender {
    int idx = (int)[sender tag];
    if (idx >= 0 && idx < g_color_preset_count) {
        g_pen_r = g_color_presets[idx].r;
        g_pen_g = g_color_presets[idx].g;
        g_pen_b = g_color_presets[idx].b;
        g_selectedColorIndex = idx;
        update_menu_checkmarks();
    }
}

- (void)selectWidth:(NSMenuItem *)sender {
    int idx = (int)[sender tag];
    if (idx >= 0 && idx < g_width_preset_count) {
        g_width_scale = g_width_presets[idx];
        g_selected_width_index = idx;
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
    update_status_icon_text();
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

    // Draw on pen contact/drag events
    if (isPen && (etype == NSEventTypeLeftMouseDown || etype == NSEventTypeLeftMouseDragged ||
                  etype == NSEventTypeRightMouseDown || etype == NSEventTypeRightMouseDragged ||
                  etype == NSEventTypeOtherMouseDown || etype == NSEventTypeOtherMouseDragged)) {
        if (!g_has_last) {
            glaspen2_begin_stroke(g_pen_r, g_pen_g, g_pen_b);
        }
        double w = (pressure > 0.01) ? (0.3 + pressure * pressure * 7.7) : 1.0;
        w *= g_width_scale;
        NSPoint loc = [nsevent locationInWindow];
        CGFloat h = [g_draw_view bounds].size.height;
        pen_draw(loc.x, h - loc.y, w);
        return NULL;
    }
    if (isPen && (etype == NSEventTypeLeftMouseUp || etype == NSEventTypeRightMouseUp ||
                  etype == NSEventTypeOtherMouseUp)) {
        g_has_last = NO;
        glaspen2_end_stroke();
        return NULL;
    }

    return event;
}

// --- App ---

static NSWindow *g_window = nil;

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

        // Register global hot key: Cmd+Ctrl+C to clear screen
        EventHotKeyID hotKeyID = { .signature = 'glsp', .id = kHotKeyClearScreen };
        EventTypeSpec eventType = { .eventClass = kEventClassKeyboard, .eventKind = kEventHotKeyPressed };
        InstallApplicationEventHandler(NewEventHandlerUPP(hotKeyHandler), 1, &eventType, NULL, NULL);
        RegisterEventHotKey(kVK_ANSI_C, cmdKey | controlKey, hotKeyID, GetApplicationEventTarget(), 0, &hotKeyID);

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
                              CGEventMaskBit(kCGEventTabletPointer);

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
