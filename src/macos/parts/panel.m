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

    // 面板与 Rust 的通信全部走 flutter_rust_bridge(Dart 侧
    // ExternalLibrary.process() 直接解析本进程内的 Rust 符号),
    // 这里不再建立 FlutterMethodChannel。

    // Create FlutterViewController
    g_flutter_vc = [[FlutterViewController alloc] initWithEngine:g_flutter_engine
                                                         nibName:nil
                                                          bundle:nil];

    // Create window — large enough for the 1.4× scaled UI on the big screen.
    NSRect frame = NSMakeRect(0, 0, 900, 1200);
    NSWindow *window = [[NSWindow alloc] initWithContentRect:frame
        styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable | NSWindowStyleMaskResizable
        backing:NSBackingStoreBuffered defer:NO];
    [window setTitle:L(@"玻璃涂鸦", @"glaspen2")];
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
        update_status_icon_state(); // 橡皮块换回笔尖
    } else {
        glaspen2_modeler_end(canvas_input_x(g_raw_last_x), canvas_input_y(g_raw_last_y), 0.0, ts, g_width_scale);
        glaspen2_modeler_commit_to_strokes(g_pen_r, g_pen_g, g_pen_b);
    }
    stroke_end();
    g_stroke_active = NO;
    g_raw_has_last = NO;
}

// Handle display configuration changes (resolution, arrangement, etc.)
// 分辨率变化的实际处理(经 2.5s 防抖后调用, 分辨率已稳定)。
static void display_change_apply(int new_w, int new_h) {
    NSLog(@"[glaspen2] display changed: %dx%d -> %dx%d", g_screen_w, g_screen_h, new_w, new_h);
    g_screen_w = new_w;
    g_screen_h = new_h;
    // Only start a new page when the current one has strokes (no silent page switch).
    glaspen2_on_display_change(g_screen_w, g_screen_h);
    // 关键: 建页/切页后必须重载当前页笔迹 —— core 只建页并切 current id,
    // 不动 STROKES 内存; 不重载的话下面的 rebuild_surface 会把旧页内容
    // 画到新页上, 表现为"相邻两页内容重复/翻页像只翻了一部分"。
    glaspen2_load_strokes_for_screen(glaspen2_get_current_screen_id());
    glaspen2_smooth_loaded_strokes();
    pageview_update(); // 新页几何可能 ≠ 屏幕

    NSRect newFrame = NSMakeRect(0, 0, new_w, new_h);
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

// 源静态持有(局部变量会被 ARC 释放, 定时器永远不触发)
static dispatch_source_t s_display_debounce;
static int s_pending_w = 0, s_pending_h = 0;

static void on_display_changed(void) {
    NSScreen *screen = [NSScreen mainScreen];
    NSRect newFrame = [screen frame];
    int new_w = (int)newFrame.size.width;
    int new_h = (int)newFrame.size.height;
    if (new_w == g_screen_w && new_h == g_screen_h) return;

    // 显示器上电/唤醒时 macOS 会先报默认分辨率(如 1920x1080)再协商出
    // 真实分辨率, 重配置事件成串到达。防抖 2.5s: 期间的新事件只重置
    // 计时器, 稳定后才处理 —— 协商回原分辨率则什么都不发生, 不再为
    // 瞬态模式凭空建页(库里曾有 71 页 1920x1080 由此而来)。
    NSLog(@"[glaspen2] display reconfig: %dx%d -> %dx%d (防抖 2.5s)", g_screen_w, g_screen_h, new_w, new_h);
    s_pending_w = new_w;
    s_pending_h = new_h;
    if (!s_display_debounce) {
        s_display_debounce = dispatch_source_create(
            DISPATCH_SOURCE_TYPE_TIMER, 0, 0, dispatch_get_main_queue());
        dispatch_source_set_event_handler(s_display_debounce, ^{
            if (s_pending_w == g_screen_w && s_pending_h == g_screen_h) return;
            display_change_apply(s_pending_w, s_pending_h);
        });
        dispatch_resume(s_display_debounce);
    }
    dispatch_source_set_timer(
        s_display_debounce,
        dispatch_time(DISPATCH_TIME_NOW, (int64_t)2.5 * NSEC_PER_SEC),
        DISPATCH_TIME_FOREVER, (int64_t)0.2 * NSEC_PER_SEC);
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


// 按笔色亮度选描边对比色(与 Rust outline_contrast_color 同参数:BT.601,阈值 0.5)
static void outline_color_for_pen(double *r, double *g, double *b) {
    double lum = 0.299 * g_pen_r + 0.587 * g_pen_g + 0.114 * g_pen_b;
    if (lum > 0.5) { *r = 0.0; *g = 0.0; *b = 0.0; }
    else           { *r = 1.0; *g = 1.0; *b = 1.0; }
}

// Raw drawing — surface only, no STROKES/DB side effects.
// Uses g_active_cr (set up by stroke_begin) for the duration of the stroke.
// Marks the affected pixel area as dirty so only that region is repainted.
static void raw_draw_dot(double x, double y, double width) {
    if (!g_surface) return;
    cairo_t *cr = g_active_cr ? g_active_cr : cairo_create_scaled();
    if (g_eraser_mode) cairo_set_operator(cr, CAIRO_OPERATOR_CLEAR);
    else cairo_set_operator(cr, CAIRO_OPERATOR_OVER);

    // 描边层(垫底):同圆放大一圈对比色
    if (g_outline_enabled && !g_eraser_mode) {
        double ol_r, ol_g, ol_b;
        outline_color_for_pen(&ol_r, &ol_g, &ol_b);
        cairo_set_source_rgba(cr, ol_r, ol_g, ol_b, 1.0);
        cairo_arc(cr, x, y, width * 0.5 + kOutlinePad, 0, 2 * M_PI);
        cairo_fill(cr);
    }

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
    cairo_set_line_cap(cr, CAIRO_LINE_CAP_ROUND);
    cairo_set_line_join(cr, CAIRO_LINE_JOIN_ROUND);

    // 描边层(垫底):同线段加宽一圈对比色。每段"先描边后上墨",
    // 接缝处墨迹圆帽覆盖描边,与整笔轮廓重绘的视觉效果一致。
    if (g_outline_enabled && !g_eraser_mode) {
        double ol_r, ol_g, ol_b;
        outline_color_for_pen(&ol_r, &ol_g, &ol_b);
        cairo_set_source_rgba(cr, ol_r, ol_g, ol_b, 1.0);
        cairo_set_line_width(cr, width + 2.0 * kOutlinePad);
        if (g_raw_has_last) {
            cairo_move_to(cr, g_raw_last_x, g_raw_last_y);
            cairo_line_to(cr, x, y);
            cairo_stroke(cr);
        } else {
            cairo_arc(cr, x, y, width * 0.5 + kOutlinePad, 0, 2 * M_PI);
            cairo_fill(cr);
        }
    }

    cairo_set_source_rgba(cr, g_pen_r, g_pen_g, g_pen_b, 1.0);
    cairo_set_line_width(cr, width);
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
