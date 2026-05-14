#import <Cocoa/Cocoa.h>
#import <QuartzCore/QuartzCore.h>
#import <ApplicationServices/ApplicationServices.h>
#import <signal.h>

// --- Screen size ---

void macos_get_screen_size(uint32_t *out_w, uint32_t *out_h) {
    NSScreen *screen = [NSScreen mainScreen];
    if (screen) {
        NSRect frame = [screen frame];
        *out_w = (uint32_t)frame.size.width;
        *out_h = (uint32_t)frame.size.height;
    } else {
        *out_w = 1920;
        *out_h = 1080;
    }
}

// --- Persistent drawing canvas ---

static CGContextRef g_ctx = NULL;
static uint32_t g_width = 0;
static uint32_t g_height = 0;
static NSView *g_view = nil;
static CGPoint g_last_pos;
static BOOL g_has_last_pos = NO;

static void ensure_canvas(NSView *view, uint32_t w, uint32_t h) {
    if (g_ctx && g_width == w && g_height == h && g_view == view) return;
    if (g_ctx) CGContextRelease(g_ctx);

    CGColorSpaceRef cs = CGColorSpaceCreateDeviceRGB();
    CGBitmapInfo info = kCGImageAlphaPremultipliedLast | kCGBitmapByteOrder32Big;
    g_ctx = CGBitmapContextCreate(NULL, w, h, 8, w * 4, cs, info);
    CGColorSpaceRelease(cs);

    CGContextClearRect(g_ctx, CGRectMake(0, 0, w, h));

    CGContextSetRGBStrokeColor(g_ctx, 1.0, 0.0, 0.0, 1.0);
    CGContextSetLineCap(g_ctx, kCGLineCapRound);
    CGContextSetLineJoin(g_ctx, kCGLineJoinRound);

    g_width = w;
    g_height = h;
    g_view = view;
    g_has_last_pos = NO;
}

static void flush_canvas(void) {
    if (!g_ctx || !g_view) return;
    CGImageRef image = CGBitmapContextCreateImage(g_ctx);
    if (!image) return;
    CALayer *layer = [g_view layer];
    if (layer) {
        [CATransaction begin];
        [CATransaction setDisableActions:YES];
        layer.contents = (__bridge id)image;
        [CATransaction commit];
    }
    CGImageRelease(image);
}

static const float MIN_WIDTH = 1.0f;
static const float MAX_WIDTH = 8.0f;

static void pen_move(NSView *view, float x, float y, float pressure) {
    ensure_canvas(view, g_width, g_height);
    float width = MIN_WIDTH + pressure * (MAX_WIDTH - MIN_WIDTH);
    CGContextSetLineWidth(g_ctx, width);
    if (g_has_last_pos) {
        CGContextMoveToPoint(g_ctx, g_last_pos.x, g_last_pos.y);
        CGContextAddLineToPoint(g_ctx, x, y);
        CGContextStrokePath(g_ctx);
    } else {
        float r = width * 0.5f;
        CGRect dot = CGRectMake(x - r, y - r, width, width);
        CGContextFillEllipseInRect(g_ctx, dot);
    }
    g_last_pos = CGPointMake(x, y);
    g_has_last_pos = YES;
    flush_canvas();
}

static void pen_up(void) {
    g_has_last_pos = NO;
}

// --- Custom NSView subclass ---

static BOOL g_pen_pressing = NO;

@interface GlaspenView : NSView
@end

@implementation GlaspenView

- (BOOL)acceptsFirstResponder {
    return YES;
}

- (BOOL)wantsUpdateLayer {
    return YES;
}

- (BOOL)isStylusEvent:(NSEvent *)event {
    NSPointingDeviceType devType = [event pointingDeviceType];
    if (devType == NSPenPointingDevice || devType == NSEraserPointingDevice) {
        return YES;
    }
    if ([event subtype] == 1 || [event subtype] == 2) {
        return YES;
    }
    return NO;
}

- (void)handlePenEvent:(NSEvent *)event {
    NSPoint locInWin = [event locationInWindow];
    NSPoint loc = [self convertPoint:locInWin fromView:nil];
    CGFloat h = [self bounds].size.height;
    float x = (float)loc.x;
    float y = (float)(h - loc.y);

    // Also get global cursor position for comparison
    NSPoint cursor = [NSEvent mouseLocation];

    float pressure = [event pressure];
    static int log_count = 0;
    if (log_count < 30) {
        log_count++;
        NSLog(@"[glaspen2] PEN#%d type=%lu sub=%ld p=%.3f view=(%.0f,%.0f) win=(%.0f,%.0f) cursor=(%.0f,%.0f) d1=%ld d2=%ld pressing=%d",
              log_count, (unsigned long)[event type], (long)[event subtype],
              pressure, x, y, locInWin.x, locInWin.y, cursor.x, cursor.y,
              (long)[event data1], (long)[event data2], g_pen_pressing);
    }

    if (pressure > 0.0f) {
        g_pen_pressing = YES;
        pen_move(self, x, y, pressure);
    } else {
        if (g_pen_pressing) {
            g_pen_pressing = NO;
            pen_up();
        }
    }
}

- (void)mouseMoved:(NSEvent *)event {
    if ([self isStylusEvent:event]) {
        // Proximity hover — don't draw
        return;
    }
    [super mouseMoved:event];
}

- (void)mouseDragged:(NSEvent *)event {
    if ([self isStylusEvent:event]) {
        [self handlePenEvent:event];
        return;
    }
    [super mouseDragged:event];
}

- (void)mouseDown:(NSEvent *)event {
    if ([self isStylusEvent:event]) {
        g_pen_pressing = YES;
        [self handlePenEvent:event];
        return;
    }
    [super mouseDown:event];
}

- (void)mouseUp:(NSEvent *)event {
    if ([self isStylusEvent:event] || g_pen_pressing) {
        g_pen_pressing = NO;
        pen_up();
        return;
    }
    [super mouseUp:event];
}

- (void)tabletPoint:(NSEvent *)event {
    [self handlePenEvent:event];
}

- (void)tabletProximity:(NSEvent *)event {
    if (![event isEnteringProximity]) {
        g_pen_pressing = NO;
        pen_up();
    }
}

- (void)rightMouseDown:(NSEvent *)event {}
- (void)rightMouseUp:(NSEvent *)event {}
- (void)rightMouseDragged:(NSEvent *)event {}
- (void)otherMouseDown:(NSEvent *)event {}
- (void)otherMouseUp:(NSEvent *)event {}
- (void)otherMouseDragged:(NSEvent *)event {}

@end

// --- App lifecycle ---

static NSWindow *g_window = nil;
static BOOL g_running = NO;

static void handle_sigint(int sig) {
    (void)sig;
    g_running = NO;
}

void macos_run_app(uint32_t screen_w, uint32_t screen_h) {
    @autoreleasepool {
        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];

        NSScreen *mainScreen = [NSScreen mainScreen];
        NSRect frame = [mainScreen frame];

        NSRect winRect = NSMakeRect(100, 100, 800, 600);
        g_window = [[NSWindow alloc]
            initWithContentRect:winRect
            styleMask:NSWindowStyleMaskBorderless
            backing:NSBackingStoreBuffered
            defer:NO];

        [g_window setLevel:NSFloatingWindowLevel];
        [g_window setOpaque:YES];
        [g_window setBackgroundColor:[NSColor colorWithWhite:0.0 alpha:0.3]];
        [g_window setTitle:@"glaspen2"];
        [g_window setAcceptsMouseMovedEvents:YES];

        GlaspenView *glaspenView = [[GlaspenView alloc] initWithFrame:winRect];
        [glaspenView setWantsLayer:YES];
        CALayer *layer = [glaspenView layer];
        if (layer) {
            [layer setOpaque:NO];
            [layer setBackgroundColor:[[NSColor clearColor] CGColor]];
        }
        [g_window setContentView:glaspenView];
        [g_window makeKeyAndOrderFront:nil];
        [glaspenView.window makeFirstResponder:glaspenView];

        g_view = glaspenView;
        g_width = screen_w;
        g_height = screen_h;

        NSLog(@"[glaspen2] window %ux%u", screen_w, screen_h);
        NSLog(@"[glaspen2] starting NSApp run");

        g_running = YES;
        signal(SIGINT, handle_sigint);

        [NSApp run];
    }
}

void macos_stop_app(void) {
    g_running = NO;
}

void macos_clear_canvas(void) {
    if (!g_ctx) return;
    CGContextClearRect(g_ctx, CGRectMake(0, 0, g_width, g_height));
    g_has_last_pos = NO;
    flush_canvas();
}
