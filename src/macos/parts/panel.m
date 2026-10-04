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
    g_raw_has_prev = NO; // 新笔没有"上一段"可重画
    g_raw_path_len = 0;  // 虚线相位从黑段起
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
    g_raw_has_prev = NO;
    g_raw_path_len = 0;
}

// ── 反色突出: 状态与前置声明(定义在下方"反色突出"区) ──
static unsigned char *s_inv_buf = NULL;     // BGRA(premul) = cairo ARGB32 布局
static CGContextRef s_inv_ctx = NULL;       // 包住 s_inv_buf(32Little+ARGB premul first)
static cairo_surface_t *s_inv_surf = NULL;  // cairo 包装同一 buf
static int s_inv_w = 0, s_inv_h = 0;
static BOOL s_inv_busy = NO;                // 仅主线程访问
static SCContentFilter *s_inv_filter = nil; // 缓存(排除自身窗口; 分辨率变化时置空重建)
static void invert_capture_tick(void);

// Handle display configuration changes (resolution, arrangement, etc.)
// 分辨率变化的实际处理(经 2.5s 防抖后调用, 分辨率已稳定)。
static void display_change_apply(int new_w, int new_h) {
    NSLog(@"[glaspen2] display changed: %dx%d -> %dx%d", g_screen_w, g_screen_h, new_w, new_h);
    s_inv_filter = nil; // 反色捕获的 display/filter 缓存失效, 下一轮重建
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

// ── 反色突出(实验) ──
// SCScreenshotManager 持续捕获"自身窗口以下的屏幕"(排除本 app 全部窗口,
// 防反馈), 反相 RGB 后经 cairo pattern 作为墨迹 source —— 每个墨迹像素
// 显示的是其正下方背景的反色, 视频等动态背景以 ~15fps 追踪(整笔重绘)。
// 注意: 捕获不含磨砂玻璃效果, 反色开启时建议关闭/调低玻璃。
static void invert_tick_schedule(double delay) {
    dispatch_after(
        dispatch_time(DISPATCH_TIME_NOW, (int64_t)(delay * NSEC_PER_SEC)),
        dispatch_get_main_queue(), ^{ invert_capture_tick(); });
}

static void invert_cache_teardown(void) {
    glaspen2_set_invert_background(NULL); // 先摘 Rust 侧引用
    if (s_inv_surf) {
        cairo_surface_destroy(s_inv_surf);
        s_inv_surf = NULL;
    }
    if (s_inv_ctx) {
        CGContextRelease(s_inv_ctx);
        s_inv_ctx = NULL;
    }
    if (s_inv_buf) {
        free(s_inv_buf);
        s_inv_buf = NULL;
    }
    s_inv_w = s_inv_h = 0;
}

void invert_ink_apply(int on) {
    if (on) {
        glaspen2_set_stroke_invert(1);
        invert_tick_schedule(0.05); // 快速首轮捕获
    } else {
        invert_cache_teardown();
        glaspen2_set_stroke_invert(0);
        rebuild_surface_from_strokes(); // 墨迹回笔色
        flush_to_layer();
    }
}

static void invert_capture_tick(void) {
    if (!g_invert_ink || !g_surface || s_tun_active || s_inv_busy) return;
    if (!glaspen2_has_strokes() && !g_stroke_active) {
        invert_tick_schedule(0.5); // 无笔迹: 慢轮询
        return;
    }
    cairo_surface_flush(g_surface);
    int dw = cairo_image_surface_get_width(g_surface);
    int dh = cairo_image_surface_get_height(g_surface);
    if (dw <= 0 || dh <= 0) return;
    if (!s_inv_buf || s_inv_w != dw || s_inv_h != dh) {
        invert_cache_teardown();
        s_inv_buf = malloc((size_t)dw * dh * 4);
        if (!s_inv_buf) return;
        memset(s_inv_buf, 0, (size_t)dw * dh * 4);
        s_inv_ctx = CGBitmapContextCreate(
            s_inv_buf, dw, dh, 8, dw * 4,
            CGColorSpaceCreateWithName(kCGColorSpaceSRGB),
            kCGBitmapByteOrder32Little | kCGImageAlphaPremultipliedFirst);
        if (!s_inv_ctx) {
            invert_cache_teardown();
            return;
        }
        s_inv_surf = cairo_image_surface_create_for_data(
            s_inv_buf, CAIRO_FORMAT_ARGB32, dw, dh, dw * 4);
        if (!s_inv_surf) {
            invert_cache_teardown();
            return;
        }
        s_inv_w = dw;
        s_inv_h = dh;
        glaspen2_set_invert_background(s_inv_surf);
        glaspen2_set_stroke_invert(1);
    }
    s_inv_busy = YES;
    if (!s_inv_filter) {
        // 首次: 缓存 display + 排除本 app 全部窗口的 filter
        [SCShareableContent getShareableContentWithCompletionHandler:^(
            SCShareableContent *content, NSError *error) {
          dispatch_async(dispatch_get_main_queue(), ^{
            s_inv_busy = NO;
            if (error || !content.displays.count || !g_invert_ink) {
              invert_tick_schedule(1.0);
              return;
            }
            NSMutableArray<SCWindow *> *excl = [NSMutableArray array];
            NSString *mine = [[NSBundle mainBundle] bundleIdentifier];
            for (SCWindow *w in content.windows) {
              if (w.owningApplication.bundleIdentifier &&
                  [w.owningApplication.bundleIdentifier isEqualToString:mine])
                [excl addObject:w];
            }
            s_inv_filter = [[SCContentFilter alloc]
                initWithDisplay:content.displays.firstObject
               excludingWindows:excl];
            invert_capture_tick(); // 立即重试(这次有 filter)
          });
        }];
        return;
    }
    SCStreamConfiguration *config = [SCStreamConfiguration new];
    config.width = (size_t)s_inv_w;
    config.height = (size_t)s_inv_h;
    config.showsCursor = NO;
    [SCScreenshotManager captureImageWithFilter:s_inv_filter
                                   configuration:config
                               completionHandler:^(CGImageRef image, NSError *error) {
          dispatch_async(dispatch_get_main_queue(), ^{
            s_inv_busy = NO;
            if (error || !image || !g_invert_ink || !s_inv_ctx) {
              invert_tick_schedule(1.0);
              return;
            }
            CGRect rect = CGRectMake(0, 0, (CGFloat)s_inv_w, (CGFloat)s_inv_h);
            CGContextClearRect(s_inv_ctx, rect);
            CGContextDrawImage(s_inv_ctx, rect, image);
            // 反相 RGB(每像素低三字节 = B,G,R; alpha FF 不动)
            UInt32 *p = (UInt32 *)s_inv_buf;
            size_t n = (size_t)s_inv_w * (size_t)s_inv_h;
            for (size_t i = 0; i < n; i++) p[i] ^= 0x00FFFFFFU;
            cairo_surface_mark_dirty(s_inv_surf);
            rebuild_surface_from_strokes(); // 整笔以新背景重绘(含 pattern)
            flush_to_layer();
            invert_tick_schedule(1.0 / 15.0); // ~15fps 追踪
          });
        }];
}

// 反色模式给 cr 挂背景 pattern(cr 有 g_scale 缩放, pattern 矩阵补偿);
// 返回需 cairo_pattern_destroy 的 pattern, NULL = 用笔色。
static cairo_pattern_t *invert_ink_source(cairo_t *cr) {
    if (!g_invert_ink || !s_inv_surf) return NULL;
    cairo_pattern_t *pat = cairo_pattern_create_for_surface(s_inv_surf);
    if (!pat) return NULL;
    cairo_matrix_t m;
    double sc = (g_scale > 0.0) ? g_scale : 1.0;
    cairo_matrix_init_scale(&m, 1.0 / sc, 1.0 / sc);
    cairo_pattern_set_matrix(pat, &m);
    cairo_set_source(cr, pat);
    return pat;
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


// 描边 = 黑白相间 1px 虚线(marching ants): 黑偶相位、白奇相位各描一遍,
// 任何背景上恒有一半可见。相位按整笔累计弧长连续 —— cairo 每次 stroke()
// 都会把虚线相位重置到路径起点, 分段增量绘制时用 offset 拨回接续点。
static const double kOutlineDash = 1.0; // 虚线段长(逻辑 px, 黑白各一段)

// Raw drawing — surface only, no STROKES/DB side effects.
// Uses g_active_cr (set up by stroke_begin) for the duration of the stroke.
// Marks the affected pixel area as dirty so only that region is repainted.
static void raw_draw_dot(double x, double y, double width) {
    if (!g_surface) return;
    cairo_t *cr = g_active_cr ? g_active_cr : cairo_create_scaled();
    if (g_eraser_mode) cairo_set_operator(cr, CAIRO_OPERATOR_CLEAR);
    else cairo_set_operator(cr, CAIRO_OPERATOR_OVER);

    // 软阴影层(最底): 三档加宽递减 alpha 的圆头黑影
    if (g_soft_shadow && !g_eraser_mode) {
        static const double kSteps[3][2] = {{1.5, 0.20}, {3.0, 0.13}, {5.0, 0.07}};
        cairo_set_line_cap(cr, CAIRO_LINE_CAP_ROUND);
        for (int si = 0; si < 3; si++) {
            cairo_set_source_rgba(cr, 0.0, 0.0, 0.0, kSteps[si][1]);
            cairo_arc(cr, x, y, width * 0.5 + kSteps[si][0], 0, 2 * M_PI);
            cairo_fill(cr);
        }
    }

    // 描边层(垫底): 起笔圆头垫一圈黑(相位 0 = 黑虚段起点)
    if (g_outline_enabled && !g_eraser_mode) {
        cairo_set_source_rgba(cr, 0.0, 0.0, 0.0, 1.0);
        cairo_arc(cr, x, y, width * 0.5 + kOutlinePad, 0, 2 * M_PI);
        cairo_fill(cr);
    }

    cairo_pattern_t *ipat = invert_ink_source(cr);
    if (!ipat) cairo_set_source_rgba(cr, g_pen_r, g_pen_g, g_pen_b, 1.0);
    cairo_arc(cr, x, y, width * 0.5, 0, 2 * M_PI);
    cairo_fill(cr);
    if (ipat) cairo_pattern_destroy(ipat);
    if (!g_active_cr) cairo_destroy(cr);
    double pad = width * 0.5 + 1.5 + (g_soft_shadow ? 5.0 : 0.0); // AA+shadow
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

    // 软阴影层(最底): 三档加宽递减 alpha 的圆头黑影
    if (g_soft_shadow && !g_eraser_mode) {
        static const double kSteps[3][2] = {{1.5, 0.20}, {3.0, 0.13}, {5.0, 0.07}};
        cairo_set_line_cap(cr, CAIRO_LINE_CAP_ROUND);
        for (int si = 0; si < 3; si++) {
            cairo_set_source_rgba(cr, 0.0, 0.0, 0.0, kSteps[si][1]);
            cairo_set_line_width(cr, width + 2.0 * kSteps[si][0]);
            if (g_raw_has_last) {
                cairo_move_to(cr, g_raw_last_x, g_raw_last_y);
                cairo_line_to(cr, x, y);
                cairo_stroke(cr);
            } else {
                cairo_arc(cr, x, y, width * 0.5 + kSteps[si][0], 0, 2 * M_PI);
                cairo_fill(cr);
            }
        }
    }

    // 描边层(垫底): 黑白相间虚线, 平头(圆帽半径超过 1px 段长会把相邻
    // 黑白段互相吞掉)。黑偶相位、白奇相位; 相位 = 累计弧长 mod 周期。
    if (g_outline_enabled && !g_eraser_mode) {
        double dashes[2] = {kOutlineDash, kOutlineDash};
        double off = fmod(g_raw_path_len, 2.0 * kOutlineDash);
        cairo_set_line_cap(cr, CAIRO_LINE_CAP_BUTT);
        cairo_set_line_width(cr, width + 2.0 * kOutlinePad);
        if (g_raw_has_last) {
            cairo_set_dash(cr, dashes, 2, off);
            cairo_set_source_rgba(cr, 0.0, 0.0, 0.0, 1.0);
            cairo_move_to(cr, g_raw_last_x, g_raw_last_y);
            cairo_line_to(cr, x, y);
            cairo_stroke(cr);
            cairo_set_dash(cr, dashes, 2, fmod(off + kOutlineDash, 2.0 * kOutlineDash));
            cairo_set_source_rgba(cr, 1.0, 1.0, 1.0, 1.0);
            cairo_move_to(cr, g_raw_last_x, g_raw_last_y);
            cairo_line_to(cr, x, y);
            cairo_stroke(cr);
            cairo_set_dash(cr, NULL, 0, 0); // 墨迹绝不能被虚线化
        } else {
            // 起笔圆头垫底(相位 0 = 黑)
            cairo_set_dash(cr, NULL, 0, 0);
            cairo_set_source_rgba(cr, 0.0, 0.0, 0.0, 1.0);
            cairo_arc(cr, x, y, width * 0.5 + kOutlinePad, 0, 2 * M_PI);
            cairo_fill(cr);
        }
        cairo_set_line_cap(cr, CAIRO_LINE_CAP_ROUND);
    }

    cairo_pattern_t *ipat = invert_ink_source(cr);
    if (!ipat) cairo_set_source_rgba(cr, g_pen_r, g_pen_g, g_pen_b, 1.0);
    cairo_set_line_width(cr, width);
    if (g_raw_has_last) {
        cairo_move_to(cr, g_raw_last_x, g_raw_last_y);
        cairo_line_to(cr, x, y);
        cairo_stroke(cr);
    } else {
        cairo_arc(cr, x, y, width * 0.5, 0, 2 * M_PI);
        cairo_fill(cr);
    }
    if (ipat) cairo_pattern_destroy(ipat);

    // 接缝重画: 描边带画在了已干的第 i-1 段墨迹边缘上(弯道内侧尤甚),
    // 墨迹不透明、重画幂等 —— 整段重画第 i-1 段墨迹, 接缝恢复
    // "描边垫底、墨迹在上"的层级, 与整页重绘视觉效果一致。
    if (g_outline_enabled && !g_eraser_mode && g_raw_has_prev) {
        cairo_set_source_rgba(cr, g_pen_r, g_pen_g, g_pen_b, 1.0);
        cairo_set_line_width(cr, g_raw_last_w);
        cairo_move_to(cr, g_raw_prev_x, g_raw_prev_y);
        cairo_line_to(cr, g_raw_last_x, g_raw_last_y);
        cairo_stroke(cr);
    }
    if (!g_active_cr) cairo_destroy(cr);

    double pad = width * 0.5 + 1.5 + (g_soft_shadow ? 5.0 : 0.0); // AA+shadow
    if (g_raw_has_last) {
        dirty_include_surface_point(g_raw_last_x, g_raw_last_y, pad);
    }
    dirty_include_surface_point(x, y, pad);

    if (g_raw_has_last) {
        double ddx = x - g_raw_last_x, ddy = y - g_raw_last_y;
        g_raw_path_len += sqrt(ddx * ddx + ddy * ddy); // 虚线相位接续用
    }
    g_raw_prev_x = g_raw_last_x;
    g_raw_prev_y = g_raw_last_y;
    g_raw_has_prev = YES;
    g_raw_last_x = x;
    g_raw_last_y = y;
    g_raw_last_w = width;
    g_raw_has_last = YES;
    flush_dirty_to_layer();
