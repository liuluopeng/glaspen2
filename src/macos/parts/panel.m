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
    // 真穿透(实验): 拟物玻璃卡片的透明窟窿需要窗口可透明。
    // Flutter view 的 layer 默认不透明, 透窗必须一并关掉。
    if (glaspen2_load_bool_setting("panel_transparent")) {
        gl_panel_transparent_apply(YES);
        if (g_flutter_vc.view.layer) g_flutter_vc.view.layer.opaque = NO;
    }
}

// Drop the cached surface image (call before the surface is destroyed).
// S7: CGImage 缓存已迁 Swift(OverlaySurfaceImageCache, Swift 持有)。
static void surface_image_cache_invalidate(void) {
    [OverlaySurfaceImageCache clear];
}

static void ensure_surface(NSView *view) { [SurfaceStore ensureWithView:view]; }

static void flush_to_layer(void) { [OverlayLayerFlush flush]; }


/// Update the active cairo context's source and stroke/fill helpers using
/// the shared g_active_cr (must have been created via stroke_begin).
/// Flushes only the dirty region covered by the latest drawing op.

/// Flush only the currently-marked dirty rect to the screen.
/// Called by per-event raw_draw_dot/raw_draw_segment during a stroke.
static void flush_dirty_to_layer(void) { [OverlayLayerFlush flushDirty]; }

/// Set up the shared cairo context for the duration of a stroke.
/// Idempotent: safe to call if g_active_cr is already set.
static void stroke_begin(void) { [StrokeSurface begin]; }

/// Tear down the shared cairo context at end of stroke.
static void stroke_end(void) { [StrokeSurface end]; }

/// Properly finish an in-flight stroke (used by hotkeys / re-entrant pen-down
/// so an interrupted stroke is committed or erased instead of silently lost).
// ── 反色突出: 前置声明(状态与实现都在下方"反色突出"区) ──
static void invert_stream_invalidate(void); // 分辨率变化: 停流弃缓存, 下轮重建

// Handle display configuration changes (resolution, arrangement, etc.)
// 分辨率变化的实际处理(经 2.5s 防抖后调用, 分辨率已稳定)。
static void display_change_apply(int new_w, int new_h) { [OverlayWindowFactory applyDisplayChangeWidth:new_w height:new_h]; }

// S5-3: 防抖/重读逻辑已迁 Swift(DisplayChangeMonitor); 这里只留 apply/size 桥。
void glaspen2_display_size(int *w, int *h) {
    if (w) *w = g_screen_w;
    if (h) *h = g_screen_h;
}

void glaspen2_display_apply(int w, int h) {
    display_change_apply(w, h);
}

static void on_display_changed(void) {
    [DisplayChangeMonitor onDisplayChanged];
}

// ── 反色突出(实验) ──
// SCStream 连续捕获"自身窗口以下的屏幕"(自家窗口 sharingType=None 拒绝
// 共享像素, 从源头排除反馈), 帧到后逐像素反相写入双缓冲之一, 主线程
// 消费时把该缓冲作为墨迹 pattern —— 每个墨迹像素显示其正下方背景的反色。
// SCStream 只在画面变化时送帧: 静态背景 = 零开销, 完全符合"只在背景动
// 的时候调整颜色"的预期。帧快于主线程消费时丢帧保最新。
// 帧率上限由设置 invertFps(10/30/60/100)经 minimumFrameInterval 控制。
// 注意: 捕获不含磨砂玻璃效果, 反色开启时建议关闭/调低玻璃。
#define INV_SLOTS 2
#define INV_IDLE (-1)
#define INV_GRAB (-2)
static unsigned char *s_inv_buf[INV_SLOTS];     // BGRA(premul) = cairo ARGB32 布局
static CGContextRef s_inv_ctx[INV_SLOTS];       // 包住对应 buf(32Little+ARGB premul first)
static cairo_surface_t *s_inv_surf[INV_SLOTS];  // cairo 包装同一 buf
static int s_inv_w = 0, s_inv_h = 0;
static int s_inv_write = 0;                     // 流回调下一步写入的槽
static int s_inv_present = 0;                   // pattern/主线程正在读的槽
static unsigned long long s_inv_sum[INV_SLOTS]; // 帧校验和(内容没变就跳过消费)
static unsigned long long s_inv_present_sum;    // 当前呈现帧的校验和
static unsigned long long s_inv_diff[INV_SLOTS]; // 与呈现帧不同的像素数
static int s_inv_dbox[INV_SLOTS][4];             // 变化包围盒 x,y,w,h(表面像素)
// 重绘阈值: 变化像素占比低于此(≈0.2%)视为噪声(spinner/光标微动画),
// 照常更新背景缓冲(新笔迹用新色)但不重绘已有笔迹——微变化不再让
// 全屏笔迹陪着重绘闪烁; 大变化(视频切画面)才触发。
static unsigned long long inv_diff_min(void) {
    return (unsigned long long)s_inv_w * (unsigned long long)s_inv_h / 500ULL + 1;
}
static int s_inv_pending = INV_IDLE;            // INV_IDLE/INV_GRAB/待消费槽号(__atomic 原语访问)
static UInt32 s_samp_raw, s_samp_new, s_samp_pres; // 诊断采样(回调锁定区内取)
static SCContentFilter *s_inv_filter = nil;     // 缓存(排除自身窗口; 分辨率变化时置空重建)
static SCStream *s_inv_stream = nil;
static dispatch_queue_t s_inv_queue;

@interface InvertStreamOutput : NSObject <SCStreamOutput>
@end

static InvertStreamOutput *s_inv_output = nil;
static void invert_tick_schedule(double delay);
static void invert_stream_start(void);
static void invert_consume_pending(void);

@implementation InvertStreamOutput
// 流回调(后台队列): 拿一帧 BGRA, 反相写进当前写槽; 主线程没消化上一帧
// 就直接丢帧(保最新, 天然限速)。槽翻转由主线程消费时做, 读写永不相交。
- (void)stream:(SCStream *)stream
    didOutputSampleBuffer:(CMSampleBufferRef)sampleBuffer
                   ofType:(SCStreamOutputType)type {
    if (type != SCStreamOutputTypeScreen) return;
    if (!g_invert_ink || !CMSampleBufferIsValid(sampleBuffer)) return;
    CVPixelBufferRef pb = CMSampleBufferGetImageBuffer(sampleBuffer);
    if (!pb) return;
    int expected = INV_IDLE;
    if (!__atomic_compare_exchange_n(&s_inv_pending, &expected, INV_GRAB, false,
                                     __ATOMIC_ACQ_REL, __ATOMIC_ACQUIRE))
        return; // 丢帧: 主线程还没消费上一帧
    int slot = s_inv_write;
    BOOL ok = NO;
    CVPixelBufferLockBaseAddress(pb, kCVPixelBufferLock_ReadOnly);
    FourCharCode pf = CVPixelBufferGetPixelFormatType(pb);
    size_t cw = CVPixelBufferGetWidth(pb), ch = CVPixelBufferGetHeight(pb);
    size_t cstride = CVPixelBufferGetBytesPerRow(pb);
    if (pf == kCVPixelFormatType_32BGRA && cw == (size_t)s_inv_w &&
        ch == (size_t)s_inv_h) {
        // 逐行拷贝+反相(容忍行尾 padding), alpha FF 不动; 顺带算校验和,
        // 内容与呈现帧相同则主线程整帧跳过(不重绘不闪烁)
        const unsigned char *sp = CVPixelBufferGetBaseAddress(pb);
        unsigned char *dp = s_inv_buf[slot];
        const UInt32 *pv = (const UInt32 *)s_inv_buf[s_inv_present]; // 呈现帧(只读)
        size_t row = (size_t)s_inv_w * 4;
        unsigned long long sum = 0, diff = 0;
        long bx0 = LONG_MAX, by0 = LONG_MAX, bx1 = -1, by1 = -1;
        for (size_t y = 0; y < (size_t)s_inv_h; y++) {
            const UInt32 *sr = (const UInt32 *)(sp + y * cstride);
            UInt32 *dr = (UInt32 *)(dp + y * row);
            for (size_t x = 0; x < (size_t)s_inv_w; x++) {
                // 反相 RGB 并强制 alpha=FF: SCK 的 BGRA 帧 alpha 可能是 0
                // (premultiplied 语义下 = 全透明 → pattern 墨迹隐身),
                // 屏幕帧本就不透明, 无条件按不透明处理。
                UInt32 v = (sr[x] ^ 0x00FFFFFFU) | 0xFF000000U;
                dr[x] = v;
                sum += v;
                if (pv && (v ^ pv[x]) & 0x00FFFFFFu) { // 只比 RGB
                    diff++;
                    if ((long)x < bx0) bx0 = (long)x;
                    if ((long)x > bx1) bx1 = (long)x;
                    if ((long)y < by0) by0 = (long)y;
                    if ((long)y > by1) by1 = (long)y;
                }
            }
        }
        s_inv_sum[slot] = sum;
        s_inv_diff[slot] = diff;
        { // 诊断采样(必须趁锁定: IOSurface 解锁后 base 地址即失效)
            size_t mx = (size_t)s_inv_w / 2, my = (size_t)s_inv_h / 2;
            s_samp_raw = ((const UInt32 *)(sp + my * cstride))[mx];
            s_samp_new = ((const UInt32 *)dp)[my * (size_t)s_inv_w + mx];
            s_samp_pres = pv ? pv[my * (size_t)s_inv_w + mx] : 0;
        }
        if (bx1 < 0) {
            s_inv_dbox[slot][0] = s_inv_dbox[slot][1] = 0;
            s_inv_dbox[slot][2] = s_inv_dbox[slot][3] = 0;
        } else {
            // 外扩 2px 盖住 AA 软边, 截到表面范围
            long x0 = (bx0 - 2 < 0) ? 0 : bx0 - 2;
            long y0 = (by0 - 2 < 0) ? 0 : by0 - 2;
            long x1 = (bx1 + 2 >= (long)s_inv_w) ? (long)s_inv_w - 1 : bx1 + 2;
            long y1 = (by1 + 2 >= (long)s_inv_h) ? (long)s_inv_h - 1 : by1 + 2;
            s_inv_dbox[slot][0] = (int)x0;
            s_inv_dbox[slot][1] = (int)y0;
            s_inv_dbox[slot][2] = (int)(x1 - x0 + 1);
            s_inv_dbox[slot][3] = (int)(y1 - y0 + 1);
        }
        ok = YES;
    } else {
        static int mismatch_logged = 0;
        if ((mismatch_logged++) == 0) {
            char fcc[5] = {(char)(pf >> 24), (char)(pf >> 16), (char)(pf >> 8),
                           (char)pf, 0};
            NSLog(@"[invert] 帧不匹配: fmt=%s(%u) %zux%zu stride=%zu | 期望 "
                  @"BGRA %dx%d stride=%d",
                  fcc, pf, cw, ch, cstride, s_inv_w, s_inv_h, s_inv_w * 4);
        }
    }
    CVPixelBufferUnlockBaseAddress(pb, kCVPixelBufferLock_ReadOnly);
    if (ok) {
        static int logged;
        if ((++logged & 63) == 1) {
            NSLog(@"[invert] 帧到达 #%d (%dx%d)", logged, s_inv_w, s_inv_h);
            NSLog(@"[invert] 采样(中心): 原始=%08X 反相后=%08X 呈现=%08X",
                  s_samp_raw, s_samp_new, s_samp_pres);
        }
        __atomic_store_n(&s_inv_pending, slot, __ATOMIC_RELEASE);
        dispatch_async(dispatch_get_main_queue(), ^{ invert_consume_pending(); });
    } else {
        static int dropped;
        if ((++dropped & 1) == 1)
            NSLog(@"[invert] 帧被丢(格式/尺寸不匹配) #%d", dropped);
        __atomic_store_n(&s_inv_pending, INV_IDLE, __ATOMIC_RELEASE);
    }
}
@end

// 主线程消费: 翻转呈现槽, pattern 指向新背景, 整笔重绘(书写中跳过)。
// 顺序关键: 必须先翻转写槽、最后才放行流回调(pending=IDLE)——否则回调
// 在放行后、翻转前 CAS 成功, 会拿到尚未翻转的旧写槽(= pattern 正在
// 采样的呈现槽)并发写入, cairo 撕裂读 = 闪烁。release 放行保证回调
// 一定能看到翻转后的写槽, 双槽读写从此永不相交。
static void invert_consume_pending(void) {
    int slot = __atomic_load_n(&s_inv_pending, __ATOMIC_ACQUIRE);
    if (slot < 0 || slot >= INV_SLOTS || !s_inv_surf[slot]) return;
    s_inv_write = slot ^ 1;                                // 先翻写槽
    BOOL content_changed = s_inv_sum[slot] != s_inv_present_sum;
    BOOL significant = content_changed && s_inv_diff[slot] > inv_diff_min();
    if (content_changed) {
        s_inv_present = slot;
        s_inv_present_sum = s_inv_sum[slot];
        __atomic_store_n(&s_inv_pending, INV_IDLE, __ATOMIC_RELEASE); // 放行
        glaspen2_set_invert_background(s_inv_surf[slot]);
        cairo_surface_mark_dirty(s_inv_surf[slot]);
    } else {
        __atomic_store_n(&s_inv_pending, INV_IDLE, __ATOMIC_RELEASE);
    }
    int rx = 0, ry = 0, rw = 0, rh = 0;
    if (significant && !g_stroke_active) {
        // 局部重绘: 只清+只画背景真正变化的包围盒, 其余笔迹像素一字不动
        // (spinner/光标微动画只刷它自己头顶那几笔, 全屏涂鸦不再陪闪)。
        // blit 同步裁剪到同一矩形(setNeedsDisplayInRect): 全屏 blit 是
        // 79MB 内存搬运, 14fps 就能把主线程压到事件 tap 超时。
        rx = s_inv_dbox[slot][0];
        ry = s_inv_dbox[slot][1];
        rw = s_inv_dbox[slot][2];
        rh = s_inv_dbox[slot][3];
        glaspen2_set_invert_dirty(rx, ry, rw, rh);
        rebuild_surface_from_strokes();
        dirty_include_surface_rect(rx, ry, rw, rh);
        flush_dirty_to_layer();
    } else {
        glaspen2_set_invert_dirty(-1, -1, -1, -1);
    }
    static int consumed;
    if ((++consumed & 63) == 1)
        NSLog(@"[invert] 消费 #%d: 变化=%d 显著=%d 差异像素=%llu 脏区=(%d,%d,%d,%d) 书写中=%d",
              consumed, content_changed, significant,
              content_changed ? s_inv_diff[slot] : 0ULL,
              rx, ry, rw, rh, g_stroke_active);
}

static void invert_stream_stop(void) {
    if (!s_inv_stream) return;
    SCStream *st = s_inv_stream;
    s_inv_stream = nil;
    [st stopCaptureWithCompletionHandler:^(NSError *error) {
      if (error) NSLog(@"[invert] stream stop: %@", error);
    }];
    [st removeStreamOutput:s_inv_output type:SCStreamOutputTypeScreen error:nil];
}

static void invert_cache_teardown(void) {
    invert_stream_stop();
    glaspen2_set_invert_background(NULL); // 先摘 Rust 侧引用
    for (int i = 0; i < INV_SLOTS; i++) {
        if (s_inv_surf[i]) {
            cairo_surface_destroy(s_inv_surf[i]);
            s_inv_surf[i] = NULL;
        }
        if (s_inv_ctx[i]) {
            CGContextRelease(s_inv_ctx[i]);
            s_inv_ctx[i] = NULL;
        }
        if (s_inv_buf[i]) {
            free(s_inv_buf[i]);
            s_inv_buf[i] = NULL;
        }
    }
    s_inv_w = s_inv_h = 0;
}

// 分辨率变化: 停流 + 弃 filter/缓存, 监督循环按新几何重建。
// 共享必须先恢复: sharingType=None 时自家窗口不出现在枚举里,
// 重建的过滤器排除列表会变空(自激闪烁的根因)。
static void invert_stream_invalidate(void) {
    invert_stream_stop();
    s_inv_filter = nil;
    [g_window setSharingType:NSWindowSharingReadOnly];
    if (g_settings_window)
        [g_settings_window setSharingType:NSWindowSharingReadOnly];
    if (s_inv_w > 0) invert_cache_teardown();
}

// 确保双缓冲与表面同尺寸; 返回 NO = 建不起来
static BOOL invert_ensure_cache(int dw, int dh) {
    if (s_inv_buf[0] && s_inv_w == dw && s_inv_h == dh) return YES;
    invert_cache_teardown();
    size_t bytes = (size_t)dw * dh * 4;
    CGColorSpaceRef srgb = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
    for (int i = 0; i < INV_SLOTS; i++) {
        s_inv_buf[i] = malloc(bytes);
        if (!s_inv_buf[i]) break;
        memset(s_inv_buf[i], 0, bytes);
        s_inv_ctx[i] = CGBitmapContextCreate(
            s_inv_buf[i], dw, dh, 8, dw * 4, srgb,
            kCGBitmapByteOrder32Little | kCGImageAlphaPremultipliedFirst);
        if (!s_inv_ctx[i]) break;
        s_inv_surf[i] = cairo_image_surface_create_for_data(
            s_inv_buf[i], CAIRO_FORMAT_ARGB32, dw, dh, dw * 4);
        if (!s_inv_surf[i]) break;
    }
    CGColorSpaceRelease(srgb); // context 已 retain
    for (int i = 0; i < INV_SLOTS; i++)
        if (!s_inv_buf[i] || !s_inv_ctx[i] || !s_inv_surf[i]) {
            invert_cache_teardown();
            return NO;
        }
    s_inv_w = dw;
    s_inv_h = dh;
    s_inv_write = 0;
    s_inv_present = 0;
    __atomic_store_n(&s_inv_pending, INV_IDLE, __ATOMIC_RELEASE);
    glaspen2_set_invert_background(s_inv_surf[s_inv_present]);
    glaspen2_set_stroke_invert(1);
    return YES;
}

static void invert_stream_start(void) {
    if (s_inv_stream || !s_inv_filter || s_inv_w <= 0) return;
    if (!s_inv_output) s_inv_output = [InvertStreamOutput new];
    if (!s_inv_queue) s_inv_queue = dispatch_queue_create("glaspen.invert", NULL);
    SCStreamConfiguration *config = [SCStreamConfiguration new];
    config.width = (size_t)s_inv_w;
    config.height = (size_t)s_inv_h;
    config.showsCursor = NO;
    config.pixelFormat = kCVPixelFormatType_32BGRA; // 与 cairo ARGB32 内存布局一致
    config.queueDepth = 3;
    config.minimumFrameInterval = CMTimeMake(1, (int32_t)g_invert_fps);
    s_inv_stream = [[SCStream alloc] initWithFilter:s_inv_filter
                                       configuration:config
                                            delegate:nil];
    NSError *err = nil;
    if (!s_inv_stream ||
        ![s_inv_stream addStreamOutput:s_inv_output
                                  type:SCStreamOutputTypeScreen
                    sampleHandlerQueue:s_inv_queue
                                 error:&err]) {
        NSLog(@"[invert] addStreamOutput failed: %@", err);
        s_inv_stream = nil;
        return;
    }
    [s_inv_stream startCaptureWithCompletionHandler:^(NSError *error) {
      if (error) NSLog(@"[invert] startCapture failed: %@", error);
    }];
}

/// 真穿透完整应用(窗口 + Flutter view layer)。菜单分发经 prelude 声明调用。
void gl_panel_transparent_apply_full(BOOL on) {
    gl_panel_transparent_apply(on);
    if (g_flutter_vc.view) g_flutter_vc.view.layer.opaque = !on;
}

void invert_ink_apply(int on) {
    if (on) {
        glaspen2_set_stroke_invert(1);
        // 注意: sharingType=None 必须等过滤器建好之后再设——它会让自家
        // 窗口从 SCShareableContent 枚举里消失, 排除列表变空, 捕获反而
        // 连墨迹一起收进去(自激闪烁)。见下方 filter 构建完成处。
        invert_tick_schedule(0.05); // 监督循环快速首轮
    } else {
        invert_cache_teardown();
        glaspen2_set_stroke_invert(0);
        [g_window setSharingType:NSWindowSharingReadOnly];
        if (g_settings_window)
            [g_settings_window setSharingType:NSWindowSharingReadOnly];
        rebuild_surface_from_strokes(); // 墨迹回笔色
        flush_to_layer();
    }
}

// 帧率设置变更: 停流即可, 监督循环用新 minimumFrameInterval 重启
void invert_stream_restart(void) {
    invert_stream_stop();
    invert_tick_schedule(0.05);
}

// 监督循环(0.5s): 确保 filter/缓存/流就绪; 无笔迹时停流省电。
// 真正的帧流由 SCStream 驱动(只在画面变化时送帧), 本循环本身近零开销。
static void invert_capture_tick(void) {
    if (!g_invert_ink) return; // 总开关关: 循环结束(重开时 apply 会再拉起)
    // 未就绪(启动早期表面未建/翻页动效中): 等待并保持循环——绝不能
    // 直接 return 杀死循环, 否则本轮会话反色永久失效。
    if (!g_surface || s_tun_active) {
        invert_tick_schedule(0.5);
        return;
    }
    if (!glaspen2_has_strokes() && !g_stroke_active) {
        invert_stream_stop(); // 空闲: 流也停(SCK 不送帧, 但会话本身有底噪)
        invert_tick_schedule(0.5);
        return;
    }
    cairo_surface_flush(g_surface);
    int dw = cairo_image_surface_get_width(g_surface);
    int dh = cairo_image_surface_get_height(g_surface);
    if (dw <= 0 || dh <= 0) return;
    if (s_inv_w != dw || s_inv_h != dh) {
        invert_stream_stop();
        s_inv_filter = nil;
    }
    if (!invert_ensure_cache(dw, dh)) return;
    if (!s_inv_filter) {
        // 首次: 缓存 display + 排除本 app 全部窗口的 filter
        [SCShareableContent getShareableContentWithCompletionHandler:^(
            SCShareableContent *content, NSError *error) {
          dispatch_async(dispatch_get_main_queue(), ^{
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
            // 过滤器已显式排除自家窗口, 现在才关共享(双保险: 截图/其他
            // 录屏也拿不到这层)。顺序反了枚举不到自家窗口, 排除变空。
            [g_window setSharingType:NSWindowSharingNone];
            if (g_settings_window)
                [g_settings_window setSharingType:NSWindowSharingNone];
            NSLog(@"[invert] 过滤器就绪(排除 %lu 个自家窗口), 共享已关闭",
                  (unsigned long)excl.count);
            invert_capture_tick(); // 立即重试(这次有 filter)
          });
        }];
        return;
    }
    invert_stream_start();
}

static void invert_tick_schedule(double delay) {
    dispatch_after(
        dispatch_time(DISPATCH_TIME_NOW, (int64_t)(delay * NSEC_PER_SEC)),
        dispatch_get_main_queue(), ^{ invert_capture_tick(); });
}

// 反色模式给 cr 挂背景 pattern(cr 有 g_scale 缩放, pattern 矩阵补偿);
// 返回需 cairo_pattern_destroy 的 pattern, NULL = 用笔色。
static cairo_pattern_t *invert_ink_source(cairo_t *cr) {
    if (!g_invert_ink || !s_inv_surf[s_inv_present]) return NULL;
    cairo_pattern_t *pat = cairo_pattern_create_for_surface(s_inv_surf[s_inv_present]);
    if (!pat) return NULL;
    cairo_matrix_t m;
    double sc = (g_scale > 0.0) ? g_scale : 1.0;
    cairo_matrix_init_scale(&m, 1.0 / sc, 1.0 / sc);
    cairo_pattern_set_matrix(pat, &m);
    cairo_set_source(cr, pat);
    return pat;
}

// 描边 = 黑白相间 1px 虚线(marching ants): 黑偶相位、白奇相位各描一遍,
// 任何背景上恒有一半可见。相位按整笔累计弧长连续 —— cairo 每次 stroke()
// 都会把虚线相位重置到路径起点, 分段增量绘制时用 offset 拨回接续点。
static const double kOutlineDash = 1.0; // 虚线段长(逻辑 px, 黑白各一段)

// D9c: 首点计划绘制定义在下方 gl_live_plan_* 区, raw_draw_dot 先于它使用,
// C99 需前向声明。返回 NO = FFI/JSON 失败, 调用方回退旧分档自绘。

// Raw drawing — surface only, no STROKES/DB side effects.
// Uses g_active_cr (set up by stroke_begin) for the duration of the stroke.
// Marks the affected pixel area as dirty so only that region is repainted.
static void raw_draw_dot(double x, double y, double width) { [RawDrawing drawDotWithX:x y:y width:width]; }

// ── D9c 实时分段绘制: 消费 core DrawPlan ─────────────────────────────────
// raw_draw_segment 每段把「上一采样点 → 本点」喂给 core glaspen2_live_plan_json
// (输入/输出 schema 见 crates/glaspen-core/src/export/mod.rs 函数注释),
// 执行返回的命令流 + 接缝重绘 + 脏区; 返回 NO = FFI/JSON 失败, 调用方回退旧
// 逐段自绘(失败只记一次日志, 避免每个笔事件刷屏)。坐标: 本段输入是视图逻辑点
// (y 向下, 与 raw 绘制同一坐标系); 计划输出表面像素(y 向下), 执行时 ÷ g_scale
// 画进已缩放的 cairo 上下文, 宽度/dash 同步换算。像素回归归 D9d 真机。

// 执行一条计划命令: 只按 pass 选画法, 顺序/宽度/颜色/alpha/dash 全部来自计划。
// 算法对齐 core 整页重建光栅器(export/mod.rs::rasterize_macos_command); 实时段
// 第 0 点是已画过的上一采样点, 跳过它的圆点, 与旧逐段实现一致。
// 接缝重绘(plan 不建模, 几何由 FFI 计算): 用墨迹色把上一段重画一遍, 盖回
// 描边压在已干墨迹上的痕迹(旧代码接缝区的等价物)。
// 本段实时绘制入口: 组 JSON → core 计划 → 光栅化 + 脏区。返回 NO 时调用方
// 继续跑旧逐段 pass(兜底), 日志只记一次避免高频笔事件刷屏。
// 首点实时绘制入口: 单点(无前一点/无接缝)喂 core 计划 → 光栅化命令 + 脏区。
// 返回 NO 时调用方回退旧 raw_draw_dot 分档自绘; 日志只记一次避免高频落笔刷屏。
static void raw_draw_segment(double x, double y, double width) { [RawDrawing drawSegmentWithX:x y:y width:width]; }
