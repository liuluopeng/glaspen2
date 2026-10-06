}

static void pageview_frame_draw(void);
static void rebuild_surface_from_strokes(void) {
  if (!g_surface)
    return;
  // Delegate the actual Cairo rendering to Rust (avoids per-point FFI overhead)
  glaspen2_draw_rebuild_view((void *)g_surface, g_scale, g_pageview_ox,
                             g_pageview_oy, g_pageview_scale);
  pageview_frame_draw();

  // Rainbow is drawn by ObjC (g_show_rainbow is a host-side boolean)
  cairo_surface_flush(g_surface);
  if (g_show_rainbow)
    draw_rainbow_indicator();

  // Full-screen refresh — undo/page-nav/resize need it
  dirty_reset();
  flush_to_layer();
}

// --- Drawing view ---

@interface GlaspenDrawView : NSView
@end

@implementation GlaspenDrawView

- (BOOL)acceptsFirstResponder {
  return YES;
}

- (void)drawRect:(NSRect)rect {
  if (!g_surface)
    return;
  uint64_t t0 = mach_absolute_time();
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
  if (g_show_grid && !s_tun_active &&
      (g_strokes_visible || !g_grid_follow_strokes)) {
    NSRect bounds = [self bounds];
    // 两种模式统一:视图 = (画布 − 镜头偏移) × 缩放。
    // 无限画布:偏移=pan、缩放=zoom;活页本:偏移=−滚动偏移、缩放=1。
    double pan_x = 0.0, pan_y = 0.0, z = 1.0;
    if (g_infinite_canvas) {
      pan_x = g_pan_x;
      pan_y = g_pan_y;
      z = (g_zoom > 0.05) ? g_zoom : 0.05;
    } else {
      pan_x = -g_page_off_x;
      pan_y = -g_page_off_y;
    }
    double gs = g_grid_size;

    // 只算与本次重绘区(clipRect)相交的线: 笔迹一段的脏区只有十几像素,
    // 省掉整屏遍历。clipRect 是全屏时退化为原来的算法。
    double vx0 = NSMinX(clipRect), vx1 = NSMaxX(clipRect);
    double vy0 = NSMinY(clipRect), vy1 = NSMaxY(clipRect);
    long kx0 = (long)floor((pan_x + vx0 / z) / gs) - 1;
    long kx1 = (long)floor((pan_x + vx1 / z) / gs) + 1;
    long ky0 = (long)floor((pan_y + (bounds.size.height - vy1) / z) / gs) - 1;
    long ky1 = (long)floor((pan_y + (bounds.size.height - vy0) / z) / gs) + 1;

    // 细网格:每 gs 一格,统一淡细线(不再每 4 格加粗)。
    CGContextSetStrokeColorWithColor(
        ctx, [[NSColor colorWithWhite:0.5 alpha:0.15] CGColor]);
    CGContextSetLineWidth(ctx, 0.5);
    for (long k = kx0; k <= kx1; k++) {
      CGFloat gx = (k * gs - pan_x) * z;
      CGContextMoveToPoint(ctx, gx, 0);
      CGContextAddLineToPoint(ctx, gx, bounds.size.height);
    }
    for (long k = ky0; k <= ky1; k++) {
      CGFloat gy = bounds.size.height - ((k * gs - pan_y) * z);
      CGContextMoveToPoint(ctx, 0, gy);
      CGContextAddLineToPoint(ctx, bounds.size.width, gy);
    }
    CGContextStrokePath(ctx);

    // 分界线:只在"屏幕尺寸"为单位处加深加粗
    // (活页本 = 页边界;无限画布 = 每屏一条参考线)。
    double bw = (g_screen_w > 0) ? (double)g_screen_w : gs * 4.0;
    double bh = (g_screen_h > 0) ? (double)g_screen_h : gs * 4.0;
    CGContextSetStrokeColorWithColor(
        ctx, [[NSColor colorWithWhite:0.5 alpha:0.55] CGColor]);
    CGContextSetLineWidth(ctx, 1.0);
    long jx0 = (long)floor((pan_x + vx0 / z) / bw) - 1;
    long jx1 = (long)floor((pan_x + vx1 / z) / bw) + 1;
    for (long k = jx0; k <= jx1; k++) {
      CGFloat gx = (k * bw - pan_x) * z;
      CGContextMoveToPoint(ctx, gx, 0);
      CGContextAddLineToPoint(ctx, gx, bounds.size.height);
    }
    long jy0 = (long)floor((pan_y + (bounds.size.height - vy1) / z) / bh) - 1;
    long jy1 = (long)floor((pan_y + (bounds.size.height - vy0) / z) / bh) + 1;
    for (long k = jy0; k <= jy1; k++) {
      CGFloat gy = bounds.size.height - ((k * bh - pan_y) * z);
      CGContextMoveToPoint(ctx, 0, gy);
      CGContextAddLineToPoint(ctx, bounds.size.width, gy);
    }
    CGContextStrokePath(ctx);

    // 分栏参考线(纯视觉):左右两栏/上下两栏 = 每屏单位 1/2 处,
    // 九宫格 = 1/3、2/3 处。切分点吸附到最近的网格线(保证加粗的
    // 永远是真实网格线),每屏单位各自吸附,线宽比分界线粗半档。
    if (g_grid_divider > 0) {
      static const double kHalf[1] = {0.5};
      static const double kThirds[2] = {1.0 / 3.0, 2.0 / 3.0};
      const double *fx = NULL;
      int nx = 0;
      const double *fy = NULL;
      int ny = 0;
      if (g_grid_divider == 1) {
        fx = kHalf;
        nx = 1;
      } else if (g_grid_divider == 2) {
        fy = kHalf;
        ny = 1;
      } else {
        fx = kThirds;
        nx = 2;
        fy = kThirds;
        ny = 2;
      }

      CGContextSetStrokeColorWithColor(
          ctx, [[NSColor colorWithWhite:0.5 alpha:0.65] CGColor]);
      CGContextSetLineWidth(ctx, 1.5);
      CGContextBeginPath(ctx);
      for (int f = 0; f < nx; f++) {
        for (long i = jx0; i <= jx1; i++) {
          long k = (long)lround((i * bw + bw * fx[f]) / gs);
          CGFloat gx = (k * gs - pan_x) * z;
          CGContextMoveToPoint(ctx, gx, 0);
          CGContextAddLineToPoint(ctx, gx, bounds.size.height);
        }
      }
      for (int f = 0; f < ny; f++) {
        for (long i = jy0; i <= jy1; i++) {
          long k = (long)lround((i * bh + bh * fy[f]) / gs);
          CGFloat gy = bounds.size.height - ((k * gs - pan_y) * z);
          CGContextMoveToPoint(ctx, 0, gy);
          CGContextAddLineToPoint(ctx, bounds.size.width, gy);
        }
      }
      CGContextStrokePath(ctx);
    }
  }

  // Reuse the cached CGImage; it wraps the live cairo buffer, so it is
  // only rebuilt when the surface itself changes.
  if (!g_surface_cgimage || g_surface_cgimage_data != data ||
      g_surface_cgimage_w != w || g_surface_cgimage_h != h ||
      g_surface_cgimage_stride != stride) {
    CGImageRelease(g_surface_cgimage);
    g_surface_cgimage = NULL;
    CGColorSpaceRef cs = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
    CGDataProviderRef provider =
        CGDataProviderCreateWithData(NULL, data, stride * h, NULL);
    g_surface_cgimage = CGImageCreate(
        w, h, 8, 32, stride, cs,
        kCGBitmapByteOrder32Little | kCGImageAlphaPremultipliedFirst, provider,
        NULL, false, kCGRenderingIntentDefault);
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
      CGFloat sy =
          ([self bounds].size.height - (rect.origin.y + rect.size.height)) *
          g_scale;
      CGFloat sw = rect.size.width * g_scale;
      CGFloat sh = rect.size.height * g_scale;
      CGRect phys = CGRectMake(sx, sy, sw, sh);
      CGImageRef sub = CGImageCreateWithImageInRect(image, phys);
      if (sub) {
        CGContextDrawImage(ctx, NSRectToCGRect(rect), sub);
        CGImageRelease(sub);
      } else {
        NSRect bounds = [self bounds];
        CGContextDrawImage(
            ctx, CGRectMake(0, 0, bounds.size.width, bounds.size.height),
            image);
      }
    } else {
      NSRect bounds = [self bounds];
      CGContextDrawImage(
          ctx, CGRectMake(0, 0, bounds.size.width, bounds.size.height), image);
    }
    // image is the cached surface image — NOT released here.
  }

  // 页面缩略图条(minimap,翻页模式)。条子贴右缘: 重绘区离得远就整段跳过
  // (阈值 96pt 远宽于条子, 宁可多画不可漏画)。
  if (g_minimap_enabled && !g_infinite_canvas &&
      NSMaxX(rect) >= [self bounds].size.width - 96.0) {
    draw_minimap(ctx, [self bounds]);
  }

  // Draw notification text
  if (g_notification) {
    NSShadow *shadow = [[NSShadow alloc] init];
    shadow.shadowColor = [NSColor colorWithWhite:0 alpha:0.8];
    shadow.shadowOffset = NSMakeSize(2, -2);
    shadow.shadowBlurRadius = 4;

    NSDictionary *attrs = @{
      NSFontAttributeName :
          [NSFont monospacedSystemFontOfSize:36 weight:NSFontWeightMedium],
      NSForegroundColorAttributeName : [NSColor whiteColor],
      NSShadowAttributeName : shadow
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

    // Outer circle: 橡皮态红圈(与状态栏橡皮块呼应), 书写白圈
    NSColor *ringColor =
        g_eraser_mode
            ? [NSColor colorWithRed:1.0 green:0.25 blue:0.2 alpha:0.95]
            : [NSColor colorWithWhite:1.0 alpha:0.8];
    CGContextSetStrokeColorWithColor(ctx, ringColor.CGColor);
    CGContextSetLineWidth(ctx, 1.5);
    CGContextStrokeEllipseInRect(
        ctx, CGRectMake(cx - radius, cy - radius, radius * 2, radius * 2));

    // Center dot
    NSColor *dotColor =
        g_eraser_mode
            ? [NSColor colorWithRed:1.0 green:0.25 blue:0.2 alpha:0.95]
            : [NSColor colorWithWhite:1.0 alpha:0.9];
    CGContextSetFillColorWithColor(ctx, dotColor.CGColor);
    CGContextFillEllipseInRect(ctx, CGRectMake(cx - 1.5, cy - 1.5, 3, 3));

    // Crosshair lines
    CGFloat gap = 3.0;
    CGContextSetStrokeColorWithColor(
        ctx, [[NSColor colorWithWhite:0 alpha:0.5] CGColor]);
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

  // 性能日志:记录每帧被请求重绘的区域。layer-backed 视图可能拿到全屏
  // rect(FULL)—— 那样笔迹拷贝就是整屏 blit, 是涂鸦 CPU 的头号嫌疑,
  // 这里把它直接量出来。
  if (g_perf_log) {
    char notes[128];
    snprintf(notes, sizeof notes, "rect=%.0fx%.0f%s%s%s", rect.size.width,
             rect.size.height, NSEqualRects(rect, [self bounds]) ? " FULL" : "",
             g_show_grid ? " grid" : "",
             (g_minimap_enabled && !g_infinite_canvas) ? " minimap" : "");
    perf_log_event_notes("drawrect", elapsed_us(t0), notes);
  }
}

@end

// 应用无限画布开关(菜单与 Flutter 设置面板共用的唯一入口)。
// 两种模式各自独立存储:翻页模式用 screens/strokes,无限画布用
// infinite_strokes(全局仅一个画布)。切换时冲刷当前笔画、切存储、载入对应笔迹。
// 模式本身持久化在 user_settings(结构性的模式,与描边这类渲染设置不同)。
static void apply_infinite_canvas(BOOL on, BOOL notify) {
  if (g_infinite_canvas == on)
    return;
  finish_active_stroke(); // 先把在写的笔画落库到"旧"存储
  if (g_infinite_canvas)
    canvas_infinite_persist(); // 离开无限画布前存镜头
  g_infinite_canvas = on;
  glaspen2_save_bool_setting("infinite_canvas", on ? 1 : 0);
  NSMenuItem *item = [g_menu itemWithTag:668];
  if (item)
    [item setState:on ? NSControlStateValueOn : NSControlStateValueOff];
  update_status_icon_state(); // 无限画布 = 图标右下角的 ∞ 徽标
  glaspen2_set_canvas_kind(on ? 1 : 0);
  if (on) {
    glaspen2_load_infinite_strokes();
    canvas_infinite_load();
  } else {
    // 回到翻页模式:载入当前页;没有页就建一页
    long cur = glaspen2_get_current_screen_id();
    if (cur > 0) {
      glaspen2_load_strokes_for_screen(cur);
    } else {
      glaspen2_clear_strokes(g_screen_w, g_screen_h);
    }
    pageview_update();
    canvas_reset_lens();
  }
  rebuild_surface_from_strokes();
  // 滚轮劫持的根治:翻页模式重装不含滚轮的 tap,系统滚轮零拦截
  event_tap_reinstall();
  sync_settings_panel(); // Flutter 面板的开关/圆点同步
  if (notify) {
    show_notification(
        on ? L(@"无限画布已开启 (⌥⇧滚轮缩放 · ⌥⌘方向键平移)",
               @"Infinite canvas on (⌥⇧scroll zoom · ⌥⌘arrows pan)")
           : L(@"无限画布已关闭", @"Infinite canvas off"));
  }
}

// 应用描边开关(菜单与 Flutter 设置面板共用的唯一入口)。
// 纯渲染设置:只改内存状态与菜单勾选,不落库。
static void apply_outline(BOOL on) {
  if (g_outline_enabled == on)
    return;
  g_outline_enabled = on;
  glaspen2_set_stroke_outline(on ? 1 : 0);
  glaspen2_save_bool_setting("outline", on ? 1 : 0);
  NSMenuItem *item = [g_menu itemWithTag:667];
  if (item)
    [item setState:on ? NSControlStateValueOn : NSControlStateValueOff];
  // 立即对已有笔迹生效(重绘 = 从 STROKES 按当前描边开关重建)
  finish_active_stroke();
  rebuild_surface_from_strokes();
  show_notification(on ? L(@"笔迹描边已开启", @"Stroke outline on")
                       : L(@"笔迹描边已关闭", @"Stroke outline off"));
}

// ── 无限画布:镜头平移(全局唯一画布) ──

// 把当前镜头变换应用到渲染 + 节流持久化(0.5s 一次)
static void canvas_apply_transform(void) {
  glaspen2_set_view_transform(g_pan_x, g_pan_y, g_zoom);
  rebuild_surface_from_strokes();
  static CFAbsoluteTime last_save = 0.0;
  CFAbsoluteTime now = CFAbsoluteTimeGetCurrent();
  if (now - last_save > 0.5) {
    last_save = now;
    canvas_infinite_persist();
  }
}

// 滚轮平移镜头(⌘⌃滚轮,书写中忽略)
static void canvas_pan_by(double dx, double dy) {
  if (g_stroke_active)
    return;
  g_pan_x -= dx;
  g_pan_y -= dy;
  canvas_apply_transform();
}

// 缩放镜头(⌘⌃滚轮):以鼠标位置为锚,zoom ∈ (0.05, 1.0]。
// 锚点的画布坐标在缩放前后保持同一屏幕位置(view = (canvas−pan)×zoom)。
static void canvas_zoom_at(double factor, double vx, double vy) {
  if (g_stroke_active)
    return;
  double ccx = vx / g_zoom + g_pan_x;
  double ccy = vy / g_zoom + g_pan_y;
  double nz = g_zoom * factor;
  if (nz > 1.0) {
    nz = 1.0;
    if (g_zoom < 1.0) {
      static CFAbsoluteTime last_hint = 0.0;
      CFAbsoluteTime now = CFAbsoluteTimeGetCurrent();
      if (now - last_hint > 1.5) {
        last_hint = now;
        show_notification(L(@"已达最大缩放 100%", @"Max zoom 100%"));
      }
    }
  }
  if (nz < 0.05)
    nz = 0.05; // 防退化下限
  g_zoom = nz;
  g_pan_x = ccx - vx / nz;
  g_pan_y = ccy - vy / nz;
  canvas_apply_transform();
}

// 保存唯一的无限画布镜头(仅无限模式;翻页模式无镜头)
// 页面缩略图条(minimap,仅翻页模式):屏幕右缘竖向展示附近 10 页(VS Code 风)。
// 性能要点:缩略图在专属串行低优先级队列逐张渲染(避免并发全屏渲染打爆 CPU),
// 每帧只把合成好的整条 strip 图贴一次(而不是逐张画 10 次)。
static dispatch_queue_t g_minimap_queue = nil;
static NSImage *g_minimap_strip = nil;        // 合成好的整条(含边框/页号)
static NSInteger g_minimap_strip_key = -1;    // 条带指纹:(当前页, 取图版本)
static NSInteger g_minimap_fetch_version = 0; // 每完成一张取图 +1

static void draw_minimap(CGContextRef ctx, NSRect bounds) {
  if (g_infinite_canvas)
    return; // 仅翻页模式
  long cur = glaspen2_get_current_screen_id();
  if (cur <= 0)
    return;

  // 附近页 id 列表(当前页变化时重新解析)
  if (g_minimap_ids == nil || g_minimap_ids_for != cur) {
    char *json = glaspen2_list_screens_json();
    if (!json)
      return;
    NSString *str = [[NSString alloc] initWithUTF8String:json];
    glaspen2_free_c_string(json);
    NSData *d = [str dataUsingEncoding:NSUTF8StringEncoding];
    NSArray *arr = [NSJSONSerialization JSONObjectWithData:d
                                                   options:0
                                                     error:nil];
    if (![arr isKindOfClass:[NSArray class]])
      return;
    NSMutableArray *ids = [NSMutableArray array];
    for (NSDictionary *o in arr) {
      // 页按分辨率分组, minimap 只显示当前几何组的页
      if (g_pageview_pw > 0 && [o[@"w"] intValue] != g_pageview_pw)
        continue;
      NSNumber *pid = o[@"id"];
      if (pid)
        [ids addObject:pid];
    }
    g_minimap_ids = ids;
    g_minimap_ids_for = cur;
    g_minimap_strip_key = -1; // 列表变了,条带重合成
  }
  NSUInteger idx = [g_minimap_ids indexOfObject:@(cur)];
  if (idx == NSNotFound)
    return;
  NSUInteger start = (idx > 5) ? idx - 5 : 0;
  NSUInteger end = MIN(start + 10, [g_minimap_ids count]);
  NSUInteger n = end - start;
  if (n == 0)
    return;

  const CGFloat tw = 64, th = 40, gap = 4;
  CGFloat total_h = n * th + (n - 1) * gap;

  // 1) 缺失的缩略图:排入专属串行队列逐张渲染(utility 优先级,
  //    不与书写落库抢锁、不并发打爆 CPU)
  static dispatch_once_t once;
  dispatch_once(&once, ^{
    g_minimap_queue =
        dispatch_queue_create("glaspen2.minimap", DISPATCH_QUEUE_SERIAL);
    dispatch_set_target_queue(g_minimap_queue,
                              dispatch_get_global_queue(QOS_CLASS_UTILITY, 0));
  });
  for (NSUInteger i = start; i < end; i++) {
    NSNumber *pid = g_minimap_ids[i];
    if ([g_minimap_thumbs objectForKey:pid])
      continue;
    if ([g_minimap_inflight containsObject:pid])
      continue;
    [g_minimap_inflight addObject:pid];
    long long p = [pid longLongValue];
    dispatch_async(g_minimap_queue, ^{
      int len = 0;
      unsigned char *png =
          glaspen2_render_thumbnail(p, g_screen_w, g_screen_h, 128, &len);
      NSImage *im = nil;
      if (png && len > 0) {
        im = [[NSImage alloc] initWithData:[NSData dataWithBytes:png
                                                          length:len]];
        glaspen2_free_rust_bytes(png, len);
      }
      dispatch_async(dispatch_get_main_queue(), ^{
        if (im) {
          [g_minimap_thumbs setObject:im forKey:pid];
          g_minimap_fetch_version++;
        }
        [g_minimap_inflight removeObject:pid];
        [g_draw_view setNeedsDisplay:YES];
      });
    });
  }

  // 2) 合成整条 strip(小图,只在缩略图版本/当前页变化时重建)
  NSInteger key = g_minimap_fetch_version * 100000 + (NSInteger)cur;
  if (g_minimap_strip == nil || key != g_minimap_strip_key) {
    g_minimap_strip_key = key;
    NSImage *strip = [[NSImage alloc] initWithSize:NSMakeSize(tw, total_h)];
    [strip lockFocus];
    for (NSUInteger i = start; i < end; i++) {
      NSNumber *pid = g_minimap_ids[i];
      BOOL isCur = ([pid longLongValue] == cur);
      NSUInteger row = i - start;
      // lockFocus 坐标 y 向上:row 0 画在条带顶部
      CGFloat y = (n - 1 - row) * (th + gap);
      NSRect rect = NSMakeRect(0, y, tw, th);
      NSImage *im = [g_minimap_thumbs objectForKey:pid];
      if (im) {
        [im drawInRect:rect
              fromRect:NSZeroRect
             operation:NSCompositingOperationSourceOver
              fraction:1.0];
      } else {
        [[NSColor colorWithCalibratedWhite:0.94 alpha:0.9] setFill];
        [NSBezierPath fillRect:rect];
      }
      NSBezierPath *border = [NSBezierPath bezierPathWithRect:rect];
      border.lineWidth = isCur ? 2.0 : 1.0;
      (isCur ? NSColor.systemRedColor : NSColor.systemGrayColor).setStroke;
      [border stroke];
      // 页号 = 全列表中的序号(从 1 起)
      NSString *num =
          [NSString stringWithFormat:@"%lu", (unsigned long)(start + row + 1)];
      [num drawAtPoint:NSMakePoint(rect.origin.x + 3, rect.origin.y + 3)
          withAttributes:@{
            NSFontAttributeName :
                [NSFont monospacedSystemFontOfSize:9
                                            weight:NSFontWeightRegular],
            NSForegroundColorAttributeName :
                [NSColor colorWithCalibratedWhite:0.25 alpha:0.9],
          }];
    }
    [strip unlockFocus];
    g_minimap_strip = strip;
  }

  // 3) 一次贴图(右缘,自上而下)
  NSRect dst = NSMakeRect(bounds.size.width - tw - 8,
                          bounds.size.height - total_h - 8, tw, total_h);
  [g_minimap_strip drawInRect:dst
                     fromRect:NSZeroRect
                    operation:NSCompositingOperationSourceOver
                     fraction:1.0];
}

// 整页翻页显示:macOS 自带的窗口切换动效。`[g_window setIsVisible:]`
// 触发 WindowServer 内置的淡入淡出(带轻微位移),参数系统写死、不可调
// —— 与当初 X 隐藏/显示页面(c80622d)看到的是同一个效果。
// 整页翻页:渐隐与显之间的隐藏间隙压到最小 —— 重活(载入/平滑)与渐隐
// 并行执行, 隐藏间隙里只剩最终表面重绘; 之后立即渐显。
// 两个 transition 的时长由系统固定; 调用方必须在主线程。
// ── 时光隧道翻页动效 ─────────────────────────────────────────────────
// 翻页 = 沿活页本的页序列**向深处推进**:页卡片按透视缩进隧道
// (Time Machine 式),目标页从隧道尽头飞出、渐实落到满屏,落定瞬间
// 无缝换成真实页内容。主窗口全程可见, 不走系统隐藏动效。
//
// 每张卡片 = 该页的快照(预热缓存渲染到独立缓冲, 见 export/pages.rs 的
// glaspen2_preload_flip_pages);当前页直接拷贝玻璃表面 —— 第 0 帧与翻页
// 前一模一样(无跳变)。
//
// 透视约定(tunnel_place):缩放 s(z) = 1/(1+K·z), 画面位置随缩放向消失点
// 收拢 —— 越深的页越小、越靠上、越淡;卡片之间轻微错位, 露出层叠的边
// (同屏 3-5 层)。相机 z 沿真实页距推进:相邻页恒为 1 个隧道单位, 跨多页
// 翻页时一次跨过多个单位 —— 页序即隧道深度, "翻多远走多深"。
//
// 实验参数:GLASPEN2_FLIP_DUR=<秒>(时长, 默认 0.62)、
// GLASPEN2_FLIP_K=<f>(透视强度, 默认 0.45, 越大隧道越深)。

#define TUNNEL_MAX_CARDS 512
#define TUNNEL_SPAN 2.0  // 相机后方可见深度(再远就飞出画面)
#define TUNNEL_FRONT 2.0 // 相机前方可见深度(页从这里飞近)

// 玻璃表面像素尺寸(卡片的坐标空间: 全屏卡片 = 整个表面)
static int s_tun_surf_w = 0, s_tun_surf_h = 0;

// 卡片:页距(相对当前页, 整数)+ 该页的快照
static cairo_surface_t *s_tun_card[TUNNEL_MAX_CARDS];
// 每张卡的半尺寸 mip:帧内按目标尺寸选级(>0.7×主尺寸用主, 否则用 mip),
// 缩放比从最多 ~3× 降到 ≤1.41× —— 合成是每帧成本的大头(实测 24ms)。
static cairo_surface_t *s_tun_card_mip[TUNNEL_MAX_CARDS];
static int s_tun_depth[TUNNEL_MAX_CARDS]; // 页距(0 = 当前页)
static int s_tun_cards = 0;
static double s_tun_travel = 1.0; // 相机推进的隧道单位(= 本次翻页跨过的页数)
static BOOL s_tun_back = NO; // 向前翻(上一页/回溯旧页): 相机**推进**页列深处
static double s_tun_dur = 0.38; // 短促: 隧道动效在感知内"一闪而过"
// 两片玻璃之间的距离 = 相邻玻璃片的尺寸比(0.60-0.90)。所有玻璃同心
// 嵌套, 消失点恒在屏幕中心;层与层的"空隙宽度"由本值决定。
static double s_tun_gap = 0.92; // 0.92 → 同屏约 34 层(0.78≈11) // 两片玻璃的距离 = 相邻玻璃尺寸比(0.60-0.90); 小=密, 大=疏
static double s_tun_k = 0.45;
// **单片玻璃的浓度**(动效期间唯一的浓度旋钮, 面/边/侧面/投影都由它派生)。
// n 片交叠的合成浓度 = 1-∏(1-a_i)(a_i 再乘各片的页距淡出):
// p=0.08 时 单片≈0.00-0.05(几乎看不出)、2 片 0.045、3 片 0.12、
// **4 片 0.17、5 片 0.195(肉眼明显)** —— 验收标准就是这一条:
// 单片不该"看得出不透明", 要 4 片叠起来才显出不透明。
// (实测校验: p=0.10 时最内 4-5 片区 = 0.235, 与模型一致。)
// GLASPEN2_FLIP_GLASS 可覆盖(评估用)。
static double s_tun_pane_a = 0.08;
static void (^s_tun_prepare)(void);
static void (^s_tun_commit)(void);
static dispatch_source_t s_tun_timer;
static double s_tun_t0 = 0;

static inline double tunnel_now(void) {
  return [NSDate timeIntervalSinceReferenceDate];
}

static void tunnel_free_cards(void) {
  for (int i = 0; i < TUNNEL_MAX_CARDS; i++) {
    if (s_tun_card[i]) {
      cairo_surface_destroy(s_tun_card[i]);
      s_tun_card[i] = NULL;
    }
    if (s_tun_card_mip[i]) {
      cairo_surface_destroy(s_tun_card_mip[i]);
      s_tun_card_mip[i] = NULL;
    }
    s_tun_depth[i] = 0;
  }
  s_tun_cards = 0;
}

// 卡片在画面里的位置与缩放。返回 NO = 飞出画面外。
// zc = 页距 − 相机位置(负 = 已越过镜头, 正 = 在隧道里)。
//
// 关键观感:后一页不是"居中缩小", 而是**向上抬出、带页框的白纸卡**
// (Time Machine 的层叠感) —— 从前页的顶边上方露出一条, 一眼看得出
// "后面还有一页";卡片上下错开, 谁压谁一眼可辨(旧实现全部居中重叠,
// 三页透明墨迹糊成一团, 完全看不出纵深 —— 这就是观感翻车的根因)。
// 卡片摆放:页列沿屏幕中轴排向**中心消失点**(镜头恒在屏幕中心)。
// s = 1/(1+k·zc) 同时决定尺寸与位置 —— 卡片中心始终在屏轴上, 越深
// 越小并向中心收拢;翻页 = 页列沿轴整体平移一格, 前端卡片飞越镜头
// 渐隐, 深端新卡浮现 —— 方向感由"哪端进哪端出"自然产生。
static BOOL tunnel_place(double zc, double *out_cx, double *out_cy,
                         double *out_w, double *out_h, double *out_alpha) {
  if (zc < -TUNNEL_SPAN - 0.35 || zc > TUNNEL_FRONT + 0.4)
    return NO;
  // 每槽缩放率 = 玻璃间距本身(0.60-0.95): 相邻两片玻璃的尺寸比即
  // "距离"。**该值同时决定同屏容纳的页数**: 比值越接近 1 缩得越慢、
  // 隧道越深 —— 0.78≈11 层, 0.92≈34 层。zc<0(镜头前方)同率放大。
  double shrink = s_tun_gap;
  if (shrink < 0.60) shrink = 0.60;
  if (shrink > 0.95) shrink = 0.95;
  double s = pow(shrink, zc);
  if (s <= 0.06 || s > 2.6)
    return NO;
  double W = (double)s_tun_surf_w, H = (double)s_tun_surf_h;
  double w = W * s * 0.92;
  double h = H * s * 0.92;
  // 中心透视:卡片中心**恒在屏幕中心**, 只有尺寸随深度变 —— 嵌套框的
  // 公共中心即消失点。(旧公式的 (1-1/s) 项会把深处卡片往下推,
  // 消失点掉到屏幕外, 表现为"页在屏幕下侧消失"。)
  // 中心透视(恒定):所有卡片同心嵌套, 公共中心 = 屏幕中心 = 消失点。
  // "两片玻璃之间的距离"以**尺寸衰减率**表达: 每深一槽按 gap 缩小,
  // 后页边框与前页边框之间的环形空隙 = 间距的视觉化身 —— gap 大 =
  // 空隙宽(疏), gap 小 = 层层紧贴(密)。中心绝不做任何偏移。
  double cx = W * 0.5;
  double cy = H * 0.5;
  double a;
  if (zc < 0.0) {
    a = 1.0 + zc / (TUNNEL_SPAN + 0.15); // 飞越镜头时渐隐
    if (a < 0.0)
      a = 0.0;
  } else {
    a = 1.0 - zc / (TUNNEL_FRONT + 0.4) * 0.72;
    // 深处玻璃渐远渐淡: 淡出与缩小同一曲线(shrink^zc), 越远越小越淡,
    // 到可见上限(6% 屏高)恰好趋近透明 —— 一条指数曲线管尺寸与透明度。
    if (zc > 1.0) {
      double fade = pow(shrink, zc - 1.0);
      if (fade < 0.05)
        fade = 0.05;
      a *= fade;
    }
  }
  if (a > 1.0)
    a = 1.0;
  *out_cx = cx;
  *out_cy = cy;
  *out_w = w;
  *out_h = h;
  *out_alpha = a;
  return YES;
}

static void tunnel_draw_card(cairo_t *cr, int slot, double cx, double cy,
                             double w, double h, double alpha) {
  cairo_surface_t *surf = s_tun_card[slot];
  // mip 选择:目标宽不足主卡 70% 时用半尺寸 mip(缩放比降一半以上)
  if (s_tun_card_mip[slot] && w < cairo_image_surface_get_width(surf) * 0.70) {
    surf = s_tun_card_mip[slot];
  }
  if (!surf || alpha <= 0.02 || w < 1.0 || h < 1.0)
    return;
  int nw = cairo_image_surface_get_width(surf);
  int nh = cairo_image_surface_get_height(surf);
  if (nw <= 0 || nh <= 0)
    return;

  // 单片玻璃的全部浓度都由**一个旋钮** s_tun_pane_a 派生: 面/边/侧面/
  // 投影按固定比例取值, 改一处即整片玻璃一起变淡。
  // 交叠合成 = 1-∏(1-a_i) (a_i 含各片的页距淡出): 单片几乎看不出,
  // 4 片叠起来才明显 —— "几片叠起来才显出不透明"是唯一的浓度来源。
  double pa = s_tun_pane_a;
  // 快照贴满正面(无留白): 玻璃平面边缘 = 正面矩形, 拟物侧面与它齐平。
  double pad = 0.0;
  double card_x = cx - w / 2.0, card_y = cy - h / 2.0;

  cairo_save(cr);
  // 投影:向**左下**偏移(背离右/上拟物侧面的挤出方向)—— 否则暗带会
  // 从蓝色侧面外侧探出, 看起来像厚度和玻璃面错位。
  // 只画**露在正面之外的那条 L 带**(左带 + 下带), 正面内部不落影:
  // 早先是整块偏移矩形直接 fill, 而正面玻璃近乎全透(面 a≈0.01),
  // 0.30 的黑影就透满整片 → 单片看上去是一整块不透明板(用户反馈的
  // "单片玻璃不透明度太高")。挖空后每片只贡献一条极淡的边影。
  {
    double dx = w * 0.010, dy = h * 0.010;
    cairo_set_source_rgba(cr, 0, 0, 0, pa * 2.2 * alpha);
    cairo_rectangle(cr, card_x - dx, card_y + dy, dx, h); // 左带
    cairo_rectangle(cr, card_x, card_y + h, w, dy);       // 下带
    cairo_fill(cr);
  }
  // 玻璃正面(面): 极淡的中性灰 —— 单片几乎看不出, 靠多片 OVER 累积
  cairo_set_source_rgba(cr, 0.80, 0.80, 0.80, pa * alpha);
  cairo_rectangle(cr, card_x, card_y, w, h);
  cairo_fill(cr);
  // 快照(已磨砂)贴进玻璃板。磨砂玻璃本来就是糊的 → FAST 插值
  // (近邻)视觉无损, 每帧合成成本砍半以上(30ms 卡顿的主项)。
  cairo_translate(cr, card_x + w * pad, card_y + h * pad);
  cairo_scale(cr, w * (1.0 - 2.0 * pad) / (double)nw,
              h * (1.0 - 2.0 * pad) / (double)nh);
  cairo_pattern_t *pt = cairo_pattern_create_for_surface(surf);
  cairo_pattern_set_filter(pt, CAIRO_FILTER_FAST);
  cairo_set_source(cr, pt);
  cairo_paint_with_alpha(cr, 0.92 * alpha);
  cairo_pattern_destroy(pt);
  cairo_restore(cr);
  // 拟物玻璃框——**侧高视图**(能看到玻璃右侧与上侧的三维板):
  // 正面 = 透明玻璃(无色, 只留墨迹); 右/上两个侧面 = 蓝色拟物(玻璃
  // 截面), 正面矩形向右上挤出厚度 T —— 板的立体感来自这两个侧面。
  // 透视不变: 正面仍同心嵌套消失于屏幕中心, 侧面随正面同缩。
  cairo_save(cr);
  double T = w * 0.006; // 板厚(挤出量, 随卡片宽等比)—— 原厚度的 1/3
  // 侧面/棱线/正面的边: 全部由 pa 派生(结构性边缘, 给足倍数让单片
  // 仍能看清"这是一片玻璃", 但整体随 pa 一起变淡)。
  double a_side = pa * 4.2;
  double a_top = pa * 4.6;
  double a_rim = pa * 5.2;
  double a_edge = pa * 2.2;

  // 1) 右侧面(挤出): 平行四边形 card 右边 → 向右上偏移 T
  cairo_set_source_rgba(cr, 0.36, 0.55, 0.72, a_side * alpha); // 蓝(受光侧)
  cairo_move_to(cr, card_x + w, card_y);
  cairo_line_to(cr, card_x + w + T, card_y - T);
  cairo_line_to(cr, card_x + w + T, card_y + h - T);
  cairo_line_to(cr, card_x + w, card_y + h);
  cairo_close_path(cr);
  cairo_fill(cr);

  // 2) 顶侧面(挤出): 顶边 → 向右上偏移 T, 比右侧亮(顶面受光)
  cairo_set_source_rgba(cr, 0.55, 0.72, 0.86, a_top * alpha);
  cairo_move_to(cr, card_x, card_y);
  cairo_line_to(cr, card_x + T, card_y - T);
  cairo_line_to(cr, card_x + w + T, card_y - T);
  cairo_line_to(cr, card_x + w, card_y);
  cairo_close_path(cr);
  cairo_fill(cr);

  // 3) 侧面的棱线: 右/上侧面的外缘描一道更亮的线(截面高光)
  cairo_set_source_rgba(cr, 0.78, 0.88, 0.96, a_rim * alpha);
  cairo_set_line_width(cr, MAX(1.0, w * 0.0008));
  cairo_move_to(cr, card_x + T, card_y - T);
  cairo_line_to(cr, card_x + w + T, card_y - T);
  cairo_line_to(cr, card_x + w + T, card_y + h - T);
  cairo_stroke(cr);

  // 4) 正面: 无色透明 —— 只描一圈极淡的中性边(玻璃正面的存在感,
  //    不带任何蓝色; 深度感全部交给右/上侧面)
  cairo_set_source_rgba(cr, 0.95, 0.95, 0.95, a_edge * alpha);
  cairo_set_line_width(cr, MAX(1.0, w * 0.0008));
  cairo_rectangle(cr, card_x, card_y, w, h);
  cairo_stroke(cr);
  cairo_restore(cr);
}

// 一帧:相机沿页距推进(缓入缓出), 卡片按隧道坐标摆位;
// 深处的先画、近处的后画 —— 层叠顺序即页序。
static void tunnel_frame(double p) {
  if (!g_surface || s_tun_cards == 0)
    return;
  double e = p < 0.5 ? 4.0 * p * p * p : 1.0 - pow(-2.0 * p + 2.0, 3.0) / 2.0;
  // 相机沿深度轴移动:回溯旧页(向前翻, s_tun_back) = cam 0→+1 深入页列,
  // 旧页从深处迎面而来, 目标页(prev, 深度+1)落定 zc=0 满屏;
  // 去往新页(向后翻) = cam 0→−1 退出, 新页(next, 深度−1)从镜头前方
  // 罩下来落定 zc=0。深度轴语义:正=旧(深处), 负=新(镜头前方)。
  double cam = e * s_tun_travel * (s_tun_back ? 1.0 : -1.0);

  cairo_t *cr = cairo_create(g_surface);
  // 背景:深色磨砂玻璃(隧道的"洞")—— 让卡片的半透明玻璃有参照,
  // 同时遮住桌面, 层次/透视一眼可读。动效结束即恢复普通透明玻璃。
  cairo_set_operator(cr, CAIRO_OPERATOR_SOURCE);
  cairo_set_source_rgba(cr, 0, 0, 0, 0); // 全透明: 只留玻璃片与其叠加
  cairo_paint(cr);
  cairo_set_operator(cr, CAIRO_OPERATOR_OVER);

  // 按隧道坐标从深到浅排序画(卡片数 ≤ 5, 插入排序足够)
  int order[TUNNEL_MAX_CARDS];
  for (int i = 0; i < s_tun_cards; i++)
    order[i] = i;
  for (int i = 1; i < s_tun_cards; i++) {
    int key = order[i];
    int j = i - 1;
    while (j >= 0 && s_tun_depth[order[j]] < s_tun_depth[key]) {
      order[j + 1] = order[j];
      j--;
    }
    order[j + 1] = key;
  }
  for (int i = 0; i < s_tun_cards; i++) {
    int slot = order[i];
    double zc = (double)s_tun_depth[slot] - cam;
    double cx, cy, w, h, a;
    if (!tunnel_place(zc, &cx, &cy, &w, &h, &a))
      continue;
    // 每层都参与交叠(浓度靠 1-(1-a)^n 累积, 4 片才肉眼可见)。
    // 浓度不足 0.4% 的层画了也看不出 —— 跳过, 省掉几十次小面积合成。
    // 可见纵深已由 tunnel_place 的 zc 窗口(±2.4)天然封顶。
    if (a * s_tun_pane_a < 0.004)
      continue;
    // 玻璃满屏化:当前页深度为 0 时正好铺满(落定无缝), 深处的页
    // 按透视缩小 —— 同心嵌套的环形区域就是"几片玻璃叠在一起"的读法。
    tunnel_draw_card(cr, slot, cx, cy, w, h, a);
  }
  cairo_destroy(cr);
  flush_to_layer();
}

static void tunnel_finish(void) {
  if (s_tun_timer) {
    dispatch_source_cancel(s_tun_timer);
    s_tun_timer = NULL;
  }
  tunnel_frame(1.0); // 最后一帧: 目标页满屏, 与真实页一模一样
  tunnel_free_cards();
  s_tun_active = NO;
  // 动效结束:回到普通工作态 —— 表面重绘真实页 + 磨砂玻璃恢复自身开关
  // (透明玻璃或用户设定的磨砂), 由 commit 的 replay 重画表面。
  gl_glass_apply();
  if (s_tun_prepare) {
    s_tun_prepare();
    s_tun_prepare = NULL;
  } // 可为 NULL
  if (s_tun_commit) {
    s_tun_commit();
    s_tun_commit = NULL;
  }
}

// 造一张卡片:某页的快照渲染到独立缓冲(与画布一致的 scale-to-fit 变换);
// 当前页直接拷贝玻璃表面, 保证第 0 帧无跳变。cache_slot = 预热缓存里的序号。
static BOOL tunnel_make_card(int slot, int cache_slot, long screen_id,
                             int is_current) {
  // scale-to-fit 变换必须与画布一致(见 pageview_update): 页几何 ≠ 屏幕
  // 时等比缩放居中, 页外压暗 —— 快照几何来自该页的 screen_w/h。
  double pscale = 1.0, ox = 0.0, oy = 0.0;
  int pw = g_screen_w, ph = g_screen_h;
  if (!is_current) {
    glaspen2_page_dims(screen_id, &pw, &ph);
    if (pw <= 0 || ph <= 0) {
      pw = g_screen_w;
      ph = g_screen_h;
    }
    if (pw != g_screen_w || ph != g_screen_h) {
      pscale =
          MIN((double)g_screen_w / (double)pw, (double)g_screen_h / (double)ph);
      ox = ((double)g_screen_w - (double)pw * pscale) / 2.0;
      oy = ((double)g_screen_h - (double)ph * pscale) / 2.0;
    }
  }
  // 快照表面用**半分辨率**: 之后必然被磨砂+预缩, 全分辨率是白烧
  // (几百笔的页软渲染全分辨率要 ~300ms/页, 动画启动前主线程全卡住)。
  // 磨砂玻璃本来就糊, 半分辨率视觉无损, 成本降 4 倍。
  double card_scale = g_scale * 0.5;
  int sw = (int)(g_screen_w * card_scale), sh = (int)(g_screen_h * card_scale);
  if (sw <= 0 || sh <= 0)
    return NO;
  cairo_surface_t *surf =
      cairo_image_surface_create(CAIRO_FORMAT_ARGB32, sw, sh);
  if (cairo_surface_status(surf) != CAIRO_STATUS_SUCCESS) {
    cairo_surface_destroy(surf);
    return NO;
  }
  if (is_current) {
    cairo_t *cr = cairo_create(surf);
    cairo_scale(cr, card_scale / g_scale, card_scale / g_scale);
    cairo_set_operator(cr, CAIRO_OPERATOR_SOURCE);
    cairo_set_source_surface(cr, g_surface, 0, 0);
    cairo_paint(cr);
    cairo_destroy(cr);
  } else {
    glaspen2_paint_preview_into_surface(
        (void *)surf, cache_slot, card_scale, ox * 0.5, oy * 0.5, pscale, 1.0,
        0,
        s_tun_depth[slot] == 0 ? -1.0 // cur 页:不磨砂, 保持透明玻璃
                               : fabs((double)s_tun_depth[slot]));
  }
  // 预缩:动画里这张卡最大就到"全屏×0.92"(tunnel_place), 与其在
  // 每帧里做全分辨率缩放合成(实测 30ms/帧), 不如建卡时一次性缩好。
  // 用 cairo 自己缩(双线性), 尺寸 = 该卡深度档的最大可能 s。
  double d = (double)s_tun_depth[slot];
  // 相机可能朝该卡推进 1 单位(它是目标时), 该卡动画最大 s:
  // zc_min = |d| - 1 → s_max = 1/(1 + k·(|d|-1)); 落定卡留 5% 余量封顶
  double smax = 1.0 / (1.0 + s_tun_k * (fabs(d) - 1.0));
  if (smax > 1.05)
    smax = 1.05;
  int tw = (int)((double)s_tun_surf_w * smax);
  int th = (int)((double)s_tun_surf_h * smax);
  int nw2 = cairo_image_surface_get_width(surf),
      nh2 = cairo_image_surface_get_height(surf);
  if (tw > 16 && th > 16 && (tw < nw2 || th < nh2)) {
    cairo_surface_t *small =
        cairo_image_surface_create(CAIRO_FORMAT_ARGB32, tw, th);
    cairo_t *c2 = cairo_create(small);
    cairo_scale(c2, (double)tw / (double)nw2, (double)th / (double)nh2);
    cairo_set_source_surface(c2, surf, 0, 0);
    cairo_paint(c2);
    cairo_destroy(c2);
    cairo_surface_destroy(surf);
    surf = small;
  }
  s_tun_card[slot] = surf;
  // 半尺寸 mip(建卡一次, 帧内近尺寸合成用)
  int mw = cairo_image_surface_get_width(surf) / 2;
  int mh = cairo_image_surface_get_height(surf) / 2;
  if (mw > 16 && mh > 16) {
    cairo_surface_t *mip =
        cairo_image_surface_create(CAIRO_FORMAT_ARGB32, mw, mh);
    cairo_t *c3 = cairo_create(mip);
    cairo_scale(c3, 0.5, 0.5);
    cairo_set_source_surface(c3, surf, 0, 0);
    cairo_paint(c3);
    cairo_destroy(c3);
    s_tun_card_mip[slot] = mip;
  }
  return YES;
}

// 返回 YES = 隧道动画已接管; NO = 无目标页/资源失败, 走系统动效。
// going_next: 向后翻(下一页) = 相机推进 1 个隧道单位; 向前翻(上一页) =
// 相机后退 —— 前一页从镜头后方飞出、当前页缩进隧道深处。
static BOOL page_flip_tunnel(BOOL going_next, long cur_page,
                             void (^prepare)(void), void (^commit)(void)) {
  if (!g_surface || g_screen_w <= 0 || g_screen_h <= 0)
    return NO;

  // cur_page 必须是**翻页前**的当前页 —— 调用方(page_flip_swap)在
  // prepare() 之前抓取; 此时全局 current id 可能已被 prepare 切到
  // 目标页, 直接读全局会让卡片集/相机方向整体错位(表现为"向后没有
  // 动效")。
  long cur = cur_page;
  long target = going_next ? glaspen2_page_neighbor(cur, 1)
                           : glaspen2_page_neighbor(cur, -1);
  if (target <= 0)
    return NO;

  tunnel_free_cards();
  s_tun_surf_w = (int)(g_screen_w * g_scale);
  s_tun_surf_h = (int)(g_screen_h * g_scale);
  if (s_tun_surf_w <= 0 || s_tun_surf_h <= 0)
    return NO;

  // 预热缓存: [最旧…, prev1, cur, next1]("透视展示 cur 到最旧的所有玻璃";
  // after 恒 1 = 目标页)。页多时受 TUNNEL_MAX_CARDS 与建卡预算保护。
  const int before = 400, after = 1;
  double tc0 = [NSDate timeIntervalSinceReferenceDate];
  int n = glaspen2_preload_flip_pages(cur, going_next ? 1 : 0, before, after);
  double tc1 = [NSDate timeIntervalSinceReferenceDate];
  if (n <= 0)
    return NO;
  if (n > TUNNEL_MAX_CARDS)
    n = TUNNEL_MAX_CARDS;

  // 可见纵深由玻璃间距决定:淡出尽头(zc 处 alpha≈0.05)之外的页
  // 画了也不可见, 不建卡 —— "透视展示 cur 到最旧"在可见意义上成立,
  // 且库有几百页时不会为主线程塞进几百次软渲染。
  double zc_visible_max = log(0.06) / log(s_tun_gap); // 缩到 6% 屏高即不可见
  double tb0 = [NSDate timeIntervalSinceReferenceDate];
  int built = 0;
  // 缓存布局 [最旧…, prev1, cur, next1](after=1): cur 恒在倒数第二,
  // 深度 = (n-2) - i —— cur 为 0, 越靠前(越旧)越深, next1 为 -1。
  int cur_idx = n - 2;
  for (int i = 0; i < n; i++) {
    // 深度轴:旧页(prev)= 正(页列深处), 新页(next)= 负(镜头前方)。
    int depth = cur_idx - i;
    if ((double)depth > zc_visible_max)
      continue; // 缓存从最旧(最深)开始: 超深页跳过, 浅页继续建
    if ([NSDate timeIntervalSinceReferenceDate] - tb0 > 0.30)
      break; // 建卡预算 300ms: 超时停下, 宁可少一层也不卡死
    s_tun_depth[i] = depth;
    if (!tunnel_make_card(i, i, depth == 0 ? cur : 0, depth == 0)) {
      tunnel_free_cards();
      return NO;
    }
    built++;
  }
  n = built;
  s_tun_cards = n;
  double tc2 = [NSDate timeIntervalSinceReferenceDate];
  NSLog(@"[flip] 时光隧道接管: %d 张卡片(cur=%ld → %ld, 方向=%@) 预载%.0fms "
        @"建卡%.0fms",
        n, cur, target, going_next ? @"后" : @"前", (tc1 - tc0) * 1000.0,
        (tc2 - tc1) * 1000.0);
  s_tun_back = !going_next; // 向前翻(回溯) → 相机推进
  s_tun_travel = 1.0;       // 相邻页之间恒为 1 个隧道单位

  s_tun_prepare = prepare;
  s_tun_commit = commit;
  s_tun_active = YES;

  double dur = 0.62;
  const char *vd = getenv("GLASPEN2_FLIP_DUR");
  if (vd) {
    double v = atof(vd);
    if (v > 0.05)
      dur = v;
  }
  s_tun_dur = dur;
  const char *vk = getenv("GLASPEN2_FLIP_K");
  if (vk) {
    double v = atof(vk);
    if (v > 0.05 && v < 3.0)
      s_tun_k = v;
  }
  // 单片玻璃浓度(评估用): 0 = 全透(只剩墨迹与棱线), 0.3 = 单片就明显
  const char *vga = getenv("GLASPEN2_FLIP_GLASS");
  if (vga) {
    double v = atof(vga);
    if (v >= 0.0 && v <= 0.9)
      s_tun_pane_a = v;
  }

  s_tun_t0 = tunnel_now();
  // 先同步画一帧并立刻上屏:动画第一帧就是"翻页前的画面", 无跳变。
  tunnel_frame(0.0);
  if (g_draw_view)
    [g_draw_view displayIfNeeded];
  s_tun_timer = dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER, 0, 0,
                                       dispatch_get_main_queue());
  dispatch_source_set_timer(s_tun_timer, dispatch_time(DISPATCH_TIME_NOW, 0),
                            (uint64_t)(1.0 / 60.0 * NSEC_PER_SEC),
                            (uint64_t)(2.0 / 1000.0 * NSEC_PER_SEC));
  dispatch_source_set_event_handler(s_tun_timer, ^{
    if (s_tun_active && g_draw_view)
      [g_draw_view displayIfNeeded];
    double t = tunnel_now() - s_tun_t0;
    double p = t / s_tun_dur;
    if (p >= 1.0) {
      tunnel_finish(); // prepare+commit: 真实页内容无缝落地
      return;
    }
    tunnel_frame(p);
  });
  dispatch_resume(s_tun_timer);
  return YES;
}

static void page_flip_swap(BOOL going_next, void (^prepare)(void),
                           void (^commit)(void)) {
  if (!g_window) {
    prepare();
    commit();
    return;
  }

  // 时光隧道(自定义动效): 玻璃窗口不隐藏, 在自己的表面上逐帧绘制隧道
  // —— 页卡片按透视缩进深处, 目标页从尽头飞出落到满屏, 落定瞬间无缝
  // 换成真实页内容。快照建卡在主线程做(几百 ms 内), 期间先把目标页
  // 载入 STROKES 并平滑好 —— 落定那一下只是重绘, 不会卡。
  if (g_flip_effect == 1 && !g_infinite_canvas && g_surface &&
      !g_stroke_active) {
    long cur_before = glaspen2_get_current_screen_id(); // prepare 会切页, 先抓
    double tp0 = [NSDate timeIntervalSinceReferenceDate];
    prepare(); // 载入 + 平滑(目标页), 与建卡共用一次 DB 访问
    double tp1 = [NSDate timeIntervalSinceReferenceDate];
    if (page_flip_tunnel(going_next, cur_before, NULL, commit)) {
      NSLog(@"[flip] 卡顿账: prepare %.0fms", (tp1 - tp0) * 1000.0);
      return;
    }
  }

  double t0 = [NSDate timeIntervalSinceReferenceDate];
  [g_window setIsVisible:NO];
  prepare();
  dispatch_async(dispatch_get_main_queue(), ^{
    commit();
    [g_window setIsVisible:YES];
    double t1 = [NSDate timeIntervalSinceReferenceDate];
    NSLog(@"[flip] 隐藏间隙 %.0fms", (t1 - t0) * 1000.0);
  });
}

static void canvas_infinite_persist(void) {
  if (!g_infinite_canvas)
    return;
  glaspen2_set_infinite_transform(g_pan_x, g_pan_y, g_zoom);
}

// 载入唯一的无限画布镜头 + 应用到渲染
static void canvas_infinite_load(void) {
  double px = 0.0, py = 0.0, pz = 1.0;
  glaspen2_get_infinite_transform(&px, &py, &pz);
  g_pan_x = px;
  g_pan_y = py;
  g_zoom = (pz > 0.0) ? pz : 1.0;
  glaspen2_set_view_transform(g_pan_x, g_pan_y, g_zoom);
}

// 翻页模式:镜头恒为原点 + 100%
static void canvas_reset_lens(void) {
  g_page_off_x = 0.0;
  g_page_off_y = 0.0;
  g_pan_x = 0.0;
  g_pan_y = 0.0;
  g_zoom = 1.0;
  glaspen2_set_view_transform(0.0, 0.0, 1.0);
}

// --- CGEventTap callback ---

// Performance logging (set g_perf_log=YES to enable)
static FILE *g_perf_file = NULL;
static uint64_t g_perf_total_calls = 0;
static uint64_t g_perf_slow_calls = 0;

static void perf_log_begin(void) {
  if (!g_perf_file) {
    NSString *dir = [NSSearchPathForDirectoriesInDomains(
        NSLibraryDirectory, NSUserDomainMask, YES) firstObject];
    NSString *logDir = [dir stringByAppendingPathComponent:@"Logs/glaspen2"];
    NSError *err = nil;
    [[NSFileManager defaultManager] createDirectoryAtPath:logDir
                              withIntermediateDirectories:YES
                                               attributes:nil
                                                    error:&err];
    if (err)
      NSLog(@"[glaspen2] perf log dir error: %@", err);
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
  if (!g_tb_inited) {
    mach_timebase_info(&g_tb);
    g_tb_inited = YES;
  }
  return (mach_absolute_time() - start) * g_tb.numer / g_tb.denom / 1000;
}

// GLASPEN2_PERF_LOG=1 打开性能日志(写 ~/Library/Logs/glaspen2/perf.log)
static void perf_log_init_from_env(void) {
  const char *v = getenv("GLASPEN2_PERF_LOG");
  if (v && *v && strcmp(v, "0") != 0) {
    g_perf_log = YES;
    if (!g_perf_file)
      perf_log_begin();
  }
}

static void perf_log_event_notes(const char *evtype, uint64_t dur_us,
                                 const char *notes) {
  if (!g_perf_log || !g_perf_file)
    return;
  g_perf_total_calls++;
  if (dur_us > 16000)
    g_perf_slow_calls++; // >16ms = frame drop
  if (!g_tb_inited) {
    mach_timebase_info(&g_tb);
    g_tb_inited = YES;
  }
  double ts_ms = (double)mach_absolute_time() * g_tb.numer / g_tb.denom / 1e6;
  fprintf(g_perf_file, "%.3f\t%s\t%llu\t%s\n", ts_ms, evtype, dur_us,
          notes ? notes : (dur_us > 16000 ? "SLOW" : ""));
  if (g_perf_total_calls % 100 == 0)
    fflush(g_perf_file);
}

static inline void perf_log_event(const char *evtype, uint64_t dur_us) {
  perf_log_event_notes(evtype, dur_us, NULL);
}

// Execute a hotkey action by key code. Returns YES if handled.
// Used by the physical shortcut (event tap) and the settings-panel buttons.
static BOOL perform_hotkey(unsigned short kc) {
  if (kc == kVK_ANSI_C) {
    finish_active_stroke(); // don't strand an in-flight stroke
    if (g_infinite_canvas) {
      // 无限画布不清空(内容多,误清损失大):新建请走设置面板
      show_notification(
          L(@"无限画布不会被清除 · 新建请到设置面板手动新建",
            @"Infinite canvas is kept — create a new one in settings"));
      return YES;
    }
    clear_screen();
    return YES;
  } else if (kc == kVK_ANSI_V) {
    toggle_enabled();
    return YES;
  } else if (kc == 0x32) { // ` — previous page
    if (g_infinite_canvas) {
      show_notification(
          L(@"无限画布只有一个画布", @"Infinite canvas has a single canvas"));
      return YES;
    }
    finish_active_stroke();
    long target = glaspen2_prev_screen_id();
    if (target > 0) {
      page_flip_swap(
          NO,
          ^{ // 渐隐期间并行: 载入 + 平滑
            glaspen2_load_strokes_for_screen(target);
            glaspen2_smooth_loaded_strokes();
            pageview_update();
          },
          ^{
            replay_strokes_from_memory();
          });
      peek_strokes(1.0); // show the page briefly in ethereal mode
      show_page_info(target);
    } else {
      show_notification(L(@"没有上一页", @"No previous page"));
    }
    return YES;
  } else if (kc == 0x12) { // 1 — next page
    if (g_infinite_canvas) {
      show_notification(
          L(@"无限画布只有一个画布", @"Infinite canvas has a single canvas"));
      return YES;
    }
    finish_active_stroke();
    long target = glaspen2_next_screen_id();
    if (target > 0) {
      page_flip_swap(
          YES,
          ^{ // 渐隐期间并行: 载入 + 平滑
            glaspen2_load_strokes_for_screen(target);
            glaspen2_smooth_loaded_strokes();
            pageview_update();
          },
          ^{
            replay_strokes_from_memory();
          });
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
        NSString *desktop = [NSSearchPathForDirectoriesInDomains(
            NSDesktopDirectory, NSUserDomainMask, YES) firstObject];
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
              newestDate = d;
              newestGif = full;
            }
          }
        }
        if (newestGif) {
          NSPasteboard *pb = [NSPasteboard generalPasteboard];
          [pb clearContents];
          [pb writeObjects:@[ [NSURL fileURLWithPath:newestGif] ]];
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
      NSString *htmlTag = [NSString
          stringWithFormat:@"<img src=\"data:image/svg+xml;base64,%@\" />",
                           base64];
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
  if (!gif || len <= 0)
    return;
  NSData *data = [NSData dataWithBytes:gif length:len];
  NSString *name = [NSString
      stringWithFormat:@"glaspen2_record_%.0f.gif",
                       [NSDate timeIntervalSinceReferenceDate] * 1000];
  NSString *path = [NSTemporaryDirectory() stringByAppendingPathComponent:name];
  if (![data writeToFile:path atomically:YES]) {
    NSLog(@"[glaspen2] failed to write GIF temp file: %@", path);
    return;
  }
  NSPasteboard *pb = [NSPasteboard generalPasteboard];
  [pb clearContents];
  [pb writeObjects:@[ [NSURL fileURLWithPath:path] ]];
}

// Begin a recording session. Commits any in-flight stroke first so the
// snapshot index is exactly where the held-key doodle starts.
static void gif_record_start(void) {
  if (g_gif_recording)
    return;
  finish_active_stroke();
  g_gif_record_start = glaspen2_stroke_count();
  g_gif_recording = YES;
  show_notification(
      L(@"按住绘制, 松开生成 GIF", @"Hold & draw, release to make GIF"));
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
    unsigned char *gif = glaspen2_gif_record_end(start, end, fps, resolution,
                                                 speed, end_mode, &out_len);
    dispatch_async(dispatch_get_main_queue(), ^{
      if (gif && out_len > 0) {
        copy_gif_data_to_clipboard(gif, out_len);
        glaspen2_free_rust_bytes(gif, out_len);
        show_notification(L(@"GIF 已复制到剪贴板", @"GIF copied to clipboard"));
      } else {
        show_notification(
            L(@"没有笔迹或导出失败", @"No strokes or export failed"));
      }
    });
  });
}

// Begin a handwriting message recording session. Commits any in-flight
// stroke first so the snapshot index is exactly where the recording starts.
static void msg_record_start(void) {
  if (g_msg_record_start >= 0)
    return;
  finish_active_stroke();
  g_msg_record_start = glaspen2_stroke_count();
  show_notification(L(@"书写手写消息… 松开 ⌘⌃3 发送",
                      @"Writing handwriting… release ⌘⌃3 to send"));
}

// Finish the recording and send the captured stroke window as one batch of
// chat messages. The gRPC append runs on a background queue; the window is
// pinned on the main thread (key-down/key-up), matching the GIF recorder.
static void msg_record_stop_async(void) {
  int start = g_msg_record_start;
  g_msg_record_start = -1;
  finish_active_stroke();
  int end = glaspen2_stroke_count();
  if (end <= start) {
    show_notification(L(@"没有新手写内容", @"No new strokes"));
    return;
  }
  dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
    int sent = glaspen2_chat_send_strokes(start, end);
    dispatch_async(dispatch_get_main_queue(), ^{
      if (sent >= 0) {
        show_notification(
            [NSString stringWithFormat:L(@"手写消息已发送 (%d 笔)",
                                         @"Handwriting sent (%d strokes)"),
                                       sent]);
      } else {
        show_notification(L(@"手写消息发送失败", @"Handwriting send failed"));
      }
    });
  });
}

// ── 手写消息草稿通道(⌘⌃2 hold-to-draft)──
// 按住期间笔迹经 ChatStore/DraftInk gRPC 流实时推给 axum(可预览);
// 松开 half-close,由 axum 决定是否发送。与 ⌘⌃3(直发)互斥。

static void ink_draft_start(void) {
  if (g_ink_draft_active)
    return;
  finish_active_stroke(); // 把在写的笔画先落定,基线之外的新笔才进通道
  if (glaspen2_ink_draft_start(g_screen_w, g_screen_h)) {
    g_ink_draft_active = YES;
    show_notification(
        L(@"书写手写消息(草稿)… 松开 ⌘⌃2 交给对方",
          @"Writing handwriting draft… release ⌘⌃2 to hand over"));
  } else {
    show_notification(
        L(@"手写通道开启失败", @"Failed to open handwriting draft channel"));
  }
}

// Stop the draft session. finish_active_stroke runs on the main thread first
// so the final stroke goes through the commit hook before the stream closes;
// the blocking wait for axum's verdict runs on a background queue.
static void ink_draft_stop_async(void) {
  if (!g_ink_draft_active)
    return;
  g_ink_draft_active = NO;
  finish_active_stroke();
  dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
    int r = glaspen2_ink_draft_stop();
    // 失败原因(身份过期/未注册路由/连接失败)在后台线程取好再回主线程,
    // 避免与下一次会话的写入竞争。
    const char *err = glaspen2_ink_draft_last_error();
    NSString *detail =
        (err && *err) ? [NSString stringWithUTF8String:err] : nil;
    dispatch_async(dispatch_get_main_queue(), ^{
      if (r > 0) {
        show_notification([NSString
            stringWithFormat:L(@"对方已接收手写草稿 (%d 笔)",
                               @"Handwriting draft accepted (%d strokes)"),
                             r]);
      } else if (r == 0) {
        show_notification(
            L(@"对方未采用这份手写草稿", @"Handwriting draft declined"));
      } else if (detail) {
        show_notification(detail);
      } else {
        show_notification(
            L(@"手写通道失败(未连接或中断)",
              @"Handwriting channel failed (not connected or broken)"));
      }
    });
  });
}

// (重)装 event tap:include_scroll 决定滚轮事件是否进入 tap。
static void event_tap_install(BOOL include_scroll) {
  if (g_event_tap) {
    CFMachPortInvalidate(g_event_tap);
    CFRelease(g_event_tap);
    g_event_tap = NULL;
  }
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
  if (include_scroll) {
    tapMask |= CGEventMaskBit(kCGEventScrollWheel);
  }
  g_event_tap = CGEventTapCreate(kCGSessionEventTap, kCGHeadInsertEventTap,
                                 kCGEventTapOptionDefault, tapMask,
                                 event_tap_callback, NULL);
  if (g_event_tap) {
    CGEventTapEnable(g_event_tap, true);
    CFRunLoopSourceRef source =
        CFMachPortCreateRunLoopSource(kCFAllocatorDefault, g_event_tap, 0);
    CFRunLoopAddSource(CFRunLoopGetMain(), source, kCFRunLoopCommonModes);
    CFRelease(source);
    g_tap_has_scroll = include_scroll;
  }
}

// 按当前画布模式重装 event tap(无限=含滚轮,翻页=不含)。
static void event_tap_reinstall(void) { event_tap_install(g_infinite_canvas); }

static CGEventRef event_tap_callback_inner(CGEventTapProxy proxy,
                                           CGEventType type, CGEventRef event,
                                           void *refcon) {
  if (g_perf_log && !g_perf_file)
    perf_log_begin();

  uint64_t t0 = mach_absolute_time();

  // Re-enable tap if it gets disabled by timeout/user
  if (type == kCGEventTapDisabledByTimeout ||
      type == kCGEventTapDisabledByUserInput) {
    NSLog(@"[glaspen2] TAP DISABLED (timeout=%d, userinput=%d)",
          type == kCGEventTapDisabledByTimeout,
          type == kCGEventTapDisabledByUserInput);
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
    BOOL isStylus =
        proxEvent && ([proxEvent pointingDeviceType] == NSPenPointingDevice ||
                      [proxEvent pointingDeviceType] == NSEraserPointingDevice);
    if (isStylus) {
      int64_t proxState = CGEventGetIntegerValueField(
          event, kCGTabletProximityEventEnterProximity);
      NSLog(@"[pen] %@ (proximity %@)",
            proxState == 1
                ? ([proxEvent pointingDeviceType] == NSEraserPointingDevice
                       ? @"橡皮进入"
                       : @"悬浮进入")
                : @"悬浮离开",
            proxState == 1 ? @"enter" : @"exit");
      if (proxState == 1) {
        g_prox_dev = [proxEvent pointingDeviceType]; // 笔键 Pen/Eraser 切换的证据链
        peek_cancel_timer(); // pen is back — the peek stays
        if (g_ethereal_canvas)
          auto_show_canvas();
      } else {
        // Pen left the screen: hide the crosshair right away instead
        // of leaving a frozen ghost over the hidden strokes.
        if (g_cursor_visible) {
          dirty_include_point(g_cursor_x, g_cursor_y, 14.0);
          g_cursor_visible = NO;
          flush_dirty_to_layer();
        }
        if (g_ethereal_canvas) {
          NSLog(@"[pen] 悬浮离开 → 隐藏笔迹");
          auto_hide_now();
        }
      }
    }
    return event;
  }

  // ⌘⌃滚轮:无限画布镜头平移。书写中忽略(模型器坐标系不能中途跳变);
  // 非无限画布/未按修饰键时滚轮原样放行给系统。
  if (type == kCGEventScrollWheel && g_infinite_canvas) {
    NSEvent *scrollEvent = [NSEvent eventWithCGEvent:event];
    if (scrollEvent) {
      NSUInteger smods = [scrollEvent modifierFlags];
      // ⌥⌘ 避开系统占用的 ⌘⌃(⌃滚轮=辅助功能缩放)
      BOOL sHasOptCmd = (smods & NSEventModifierFlagOption) &&
                        (smods & NSEventModifierFlagCommand);
      if (sHasOptCmd && g_enabled && !g_stroke_active) {
        // ⌘⌃滚轮:缩放,以鼠标位置为中心(上限 100%)。
        // 平移改用 ⌘⌃方向键。
        double dy = [scrollEvent scrollingDeltaY];
        double factor;
        if ([scrollEvent hasPreciseScrollingDeltas]) {
          factor = exp(-dy * 0.0015); // 触控板连续缩放
        } else {
          factor = (dy > 0) ? 1.1 : ((dy < 0) ? 1.0 / 1.1 : 1.0);
        }
        // 鼠标位置 → 绘图坐标(与笔事件同约定:窗口原点在左下)
        NSPoint mloc = [scrollEvent locationInWindow];
        double vh = [g_draw_view bounds].size.height;
        double mx = mloc.x;
        double my = vh - mloc.y;
        canvas_zoom_at(factor, mx, my);
        perf_log_event("scroll_zoom", elapsed_us(t0));
        return NULL; // 吞掉,避免下层 app 同时滚动/缩放
      }
    }
    return event;
  }

  // Handle keyboard events separately — always intercept hotkeys
  if (type == kCGEventKeyDown || type == kCGEventKeyUp) {
    NSEvent *keyEvent = [NSEvent eventWithCGEvent:event];
    if (keyEvent) {
      NSUInteger mods = [keyEvent modifierFlags];
      BOOL hasCmdCtrl = (mods & NSEventModifierFlagCommand) &&
                        (mods & NSEventModifierFlagControl);
      unsigned short kc = [keyEvent keyCode];
      // Stop the GIF recorder on R key-up regardless of modifiers, so a
      // Cmd/Ctrl released before R can't leave the recorder stuck.
      if (type == kCGEventKeyUp && kc == kVK_ANSI_R && g_gif_recording) {
        gif_record_stop_async();
        return NULL;
      }
      if (hasCmdCtrl && kc == kVK_ANSI_R && type == kCGEventKeyDown) {
        // Hold-to-record GIF: key-down starts the recording.
        if (!g_gif_recording)
          gif_record_start();
        return NULL;
      }
      // Stop the handwriting recorder on '3' key-up regardless of
      // modifiers, so a Cmd/Ctrl released before '3' can't wedge it.
      if (type == kCGEventKeyUp && kc == kVK_ANSI_3 &&
          g_msg_record_start >= 0) {
        msg_record_stop_async();
        return NULL;
      }
      if (g_chat_integration && hasCmdCtrl && kc == kVK_ANSI_3 &&
          type == kCGEventKeyDown) {
        // Hold-to-record handwriting message: key-down pins the start.
        if (g_msg_record_start < 0 && !g_ink_draft_active)
          msg_record_start();
        return NULL;
      }
      // ⌘⌃2 hold-to-draft handwriting: strokes stream live to the axum
      // side over ChatStore/DraftInk while held; release closes the
      // stream and axum decides whether to send. '2' key-up ends it
      // regardless of modifiers (same reasoning as '3' above).
      // 总开关关闭时整段不匹配 → 事件原样放行(不占用这两个键)。
      if (type == kCGEventKeyUp && kc == kVK_ANSI_2 && g_ink_draft_active) {
        ink_draft_stop_async();
        return NULL;
      }
      if (g_chat_integration && hasCmdCtrl && kc == kVK_ANSI_2 &&
          type == kCGEventKeyDown) {
        if (!g_ink_draft_active && g_msg_record_start < 0)
          ink_draft_start();
        return NULL;
      }
      BOOL kHasOptCmd = (mods & NSEventModifierFlagOption) &&
                        (mods & NSEventModifierFlagCommand);
      // ⌘⌃PageUp/PageDown:缩放(额外绑定,不依赖滚轮)
      if (g_infinite_canvas && hasCmdCtrl && g_enabled && !g_stroke_active &&
          type == kCGEventKeyDown && (kc == kVK_PageUp || kc == kVK_PageDown)) {
        // 键盘缩放以视口中心为锚
        canvas_zoom_at(kc == kVK_PageUp ? 1.15 : 1.0 / 1.15,
                       (double)g_screen_w * 0.5, (double)g_screen_h * 0.5);
        return NULL;
      }
      // 活页本模式:⌥⌘↑/↓ = 整页翻页(↑ 上一页 / ↓ 下一页),
      // 带窗口级原生淡入淡出;无限画布模式则由下方分支做镜头平移。
      if (!g_infinite_canvas && kHasOptCmd && !g_stroke_active &&
          type == kCGEventKeyDown &&
          (kc == kVK_UpArrow || kc == kVK_DownArrow)) {
        finish_active_stroke();
        BOOL up = (kc == kVK_UpArrow);
        long target =
            up ? glaspen2_prev_screen_id() : glaspen2_next_screen_id();
        if (target > 0) {
          page_flip_swap(
              up ? NO : YES,
              ^{ // 渐隐期间并行: 载入 + 平滑
                glaspen2_load_strokes_for_screen(target);
                glaspen2_smooth_loaded_strokes();
                pageview_update();
              },
              ^{
                replay_strokes_from_memory();
              });
          peek_strokes(1.0); // show the page briefly in ethereal mode
          show_page_info(target);
        } else {
          show_notification(up ? L(@"没有上一页", @"No previous page")
                               : L(@"没有下一页", @"No next page"));
        }
        return NULL;
      }
      // 自由画布(无限画布)模式:⌥⌘上下左右 = 镜头四向平移。
      // (活页本模式只做上下移动,⌥⌘←/→ 在此不处理,见上。)
      if (g_infinite_canvas && kHasOptCmd && g_enabled && !g_stroke_active &&
          type == kCGEventKeyDown && kc >= kVK_LeftArrow && kc <= kVK_UpArrow) {
        double step = 80.0 / g_zoom;
        double dx = 0.0, dy = 0.0;
        if (kc == kVK_LeftArrow)
          dx = -step;
        else if (kc == kVK_RightArrow)
          dx = step;
        else if (kc == kVK_UpArrow)
          dy = step;
        else if (kc == kVK_DownArrow)
          dy = -step;
        canvas_pan_by(dx, dy);
        return NULL;
      }
      if (hasCmdCtrl && type == kCGEventKeyDown) {
        if (perform_hotkey(kc))
          return NULL;
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
  if (!nsevent)
    return event;

  NSPointingDeviceType devType = [nsevent pointingDeviceType];
  NSInteger subtype = [nsevent subtype];
  NSEventType etype = [nsevent type];
  CGFloat pressure = [nsevent pressure];

  BOOL isPen =
      (devType == NSPenPointingDevice || devType == NSEraserPointingDevice ||
       subtype == 1 || subtype == 2);

  // Non-pen mouse move while no active stroke and cursor visible → hide
  // crosshair. Covers pen-leave-proximity on tablets that don't emit proximity
  // events. The g_stroke_active guard prevents spurious non-pen events
  // interleaved during a stroke (from trackpad / secondary input) from causing
  // extra redraws.
  if (!isPen && etype == NSEventTypeMouseMoved && g_cursor_visible &&
      !g_stroke_active) {
    // Invalidate only the old crosshair region (partial refresh).
    dirty_include_point(g_cursor_x, g_cursor_y, 14.0);
    g_cursor_visible = NO;
    // 飘渺画布涂鸦模式: the pen just left (no proximity events on this
    // tablet) — hide as a fallback.
    if (g_ethereal_canvas)
      auto_hide_now();
    flush_dirty_to_layer();
  }

  // Update cursor position for pen events only. The crosshair is hidden
  // while a stroke is being drawn (the ink itself is the feedback) and
  // re-shown on pen-up; during hover its redraws are throttled to ~16 ms
  // to cut per-event work at high hover rates.
  if (isPen) {
    // Tablets without proximity events: hover moves are the only signal —
    // treat them like a hover to show the strokes in 飘渺画布涂鸦模式.
    if (!g_strokes_visible)
      auto_show_canvas();
    peek_cancel_timer(); // pen interaction overrides a pending page peek
    NSPoint loc = [nsevent locationInWindow];
    BOOL moved = (g_cursor_x != loc.x || g_cursor_y != loc.y);
    if (etype == NSEventTypeMouseMoved) {
      perf_log_event("pen_hover",
                     elapsed_us(t0)); // 只算纯悬停, 拖动另记 pen_move
    }
    if (moved && g_cursor_visible) {
      dirty_include_point(g_cursor_x, g_cursor_y, 14.0); // old position
    }
    g_cursor_x = loc.x;
    g_cursor_y = loc.y;
    if (!g_stroke_active) {
      g_cursor_visible = YES;
    }
    if (moved && g_cursor_visible) {
      // 书写/擦除中也跟随: raw_draw 已把笔尖区域标脏, 十字恰在笔尖
      dirty_include_point(g_cursor_x, g_cursor_y, 14.0);
      if (!g_stroke_active && elapsed_us(g_last_cursor_flush) >= 16000) {
        g_last_cursor_flush = mach_absolute_time();
        flush_dirty_to_layer();
      }
    }
  }

  // Hide system cursor while pen is drawing, restore on any mouse up
  if (isPen && (etype == NSEventTypeLeftMouseDown ||
                etype == NSEventTypeRightMouseDown ||
                etype == NSEventTypeOtherMouseDown ||
                etype == NSEventTypeLeftMouseDragged ||
                etype == NSEventTypeRightMouseDragged ||
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
      dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 10 * NSEC_PER_SEC),
                     dispatch_get_main_queue(), ^{
                       if (g_event_tap)
                         CGEventTapEnable(g_event_tap, true);
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

  // Width from pressure (公式单源在 core modeler::pressure_to_width)
  double raw_w = glaspen2_pressure_raw_width(pressure, g_width_scale);
  raw_w *= g_zoom; // 屏幕呈现宽度随缩放

  // Pressure monitor: update on pen event before early returns
  if (g_pressure_monitor && isPen) {
    g_pm_in_range = YES;
    int64_t rawField =
        CGEventGetIntegerValueField(event, kCGTabletEventPointPressure);
    g_pm_pressure = (rawField > 0) ? (int)(rawField * 65535 / 65536)
                                   : (int)(pressure * 65535);
    g_pm_x = px;
    g_pm_y = py;
    g_pm_tip_down = (etype == NSEventTypeLeftMouseDown ||
                     etype == NSEventTypeRightMouseDown ||
                     etype == NSEventTypeOtherMouseDown ||
                     etype == NSEventTypeLeftMouseDragged ||
                     etype == NSEventTypeRightMouseDragged ||
                     etype == NSEventTypeOtherMouseDragged);
    switch (etype) {
    case NSEventTypeLeftMouseDown:
      g_pm_evtype = @"DOWN";
      break;
    case NSEventTypeLeftMouseUp:
      g_pm_evtype = @"UP";
      break;
    case NSEventTypeLeftMouseDragged:
      g_pm_evtype = @"DRAG";
      break;
    case NSEventTypeRightMouseDown:
      g_pm_evtype = @"R_DOWN";
      break;
    case NSEventTypeRightMouseUp:
      g_pm_evtype = @"R_UP";
      break;
    case NSEventTypeRightMouseDragged:
      g_pm_evtype = @"R_DRAG";
      break;
    case NSEventTypeOtherMouseDown:
      g_pm_evtype = @"O_DOWN";
      break;
    case NSEventTypeOtherMouseUp:
      g_pm_evtype = @"O_UP";
      break;
    case NSEventTypeOtherMouseDragged:
      g_pm_evtype = @"O_DRAG";
      break;
    case NSEventTypeMouseMoved:
      g_pm_evtype = g_pm_tip_down ? @"DOWN" : @"MOVE";
      break;
    default:
      g_pm_evtype = g_pm_tip_down ? @"DOWN" : @"MOVE";
      break;
    }
    pm_update();
  }

  // Strokes hidden (飘渺画布涂鸦模式): touching down always re-shows them
  // and the stroke proceeds below. The pen is swallowed so it never acts
  // as a mouse while drawing mode (V) is enabled.
  if (!g_strokes_visible && isPen &&
      (etype == NSEventTypeLeftMouseDown ||
       etype == NSEventTypeRightMouseDown ||
       etype == NSEventTypeOtherMouseDown ||
       etype == NSEventTypeLeftMouseDragged ||
       etype == NSEventTypeRightMouseDragged ||
       etype == NSEventTypeOtherMouseDragged)) {
    auto_show_canvas(); // show (and draw) — fallback for tablets without hover
  }

  if (isPen && (etype == NSEventTypeLeftMouseDown ||
                etype == NSEventTypeRightMouseDown ||
                etype == NSEventTypeOtherMouseDown)) {
    // Pen down: start modeler, draw raw dot immediately (no lag)
    // If a previous stroke was never ended (missed pen-up), finish it first.
    if (g_stroke_active) {
      finish_active_stroke();
    }
    BOOL eraser = (devType == NSEraserPointingDevice) ||
                  (g_prox_dev == NSEraserPointingDevice);
    if (eraser != g_eraser_mode) {
      g_eraser_mode = eraser;
      update_status_icon_state();
    } // 图标换橡皮块
    NSLog(@"[glaspen2] pen DOWN at (%.1f, %.1f) p=%.2f ts=%.3f tool=%@/%@",
          px, py, pressure, ts,
          devType == NSEraserPointingDevice   ? @"ERASER"
          : devType == NSPenPointingDevice    ? @"pen"
                                              : @"other",
          g_prox_dev == NSEraserPointingDevice ? @"eraser-prox"
          : g_prox_dev == NSPenPointingDevice  ? @"pen-prox"
                                               : @"other-prox");
    glaspen2_modeler_begin(g_pen_r, g_pen_g, g_pen_b, canvas_input_x(px),
                           canvas_input_y(py), pressure, ts, g_width_scale);
    g_stroke_active = YES;
    // 光标全程跟随(此前落笔即隐藏 "ink is the feedback" —— 擦除时无新墨
    // 可看, 用户实测盲擦)。橡皮态光标变红。
    g_cursor_visible = YES;
    g_cursor_x = px;
    g_cursor_y = py;
    stroke_begin();        // reuse one cairo context for the whole stroke
    raw_draw_dot(px, py, raw_w);
    g_raw_last_x = px;
    g_raw_last_y = py;
    g_raw_last_w = raw_w;
    g_raw_has_last = YES;
    g_raw_path_len = 0;
    perf_log_event("pen_down", elapsed_us(t0));
    return NULL;
  }
  if (isPen && (etype == NSEventTypeLeftMouseDragged ||
                etype == NSEventTypeRightMouseDragged ||
                etype == NSEventTypeOtherMouseDragged)) {
    // If no DOWN event was seen (pen detection lag), auto-initialize
    if (!g_stroke_active) {
      BOOL eraser = (devType == NSEraserPointingDevice) ||
                    (g_prox_dev == NSEraserPointingDevice);
      if (eraser != g_eraser_mode) {
        g_eraser_mode = eraser;
        update_status_icon_state();
      }
      glaspen2_modeler_begin(g_pen_r, g_pen_g, g_pen_b, canvas_input_x(px),
                             canvas_input_y(py), pressure, ts, g_width_scale);
      g_stroke_active = YES;
      g_cursor_visible = YES;
      g_cursor_x = px;
      g_cursor_y = py;
      stroke_begin();
      raw_draw_dot(px, py, raw_w);
      g_raw_last_x = px;
      g_raw_last_y = py;
      g_raw_last_w = raw_w;
      g_raw_has_last = YES;
      g_raw_path_len = 0;
      return NULL; // begin already recorded this point, don't feed duplicate to
                   // modeler
    }
    // Feed modeler, draw raw segment for responsive real-time feedback
    glaspen2_modeler_move(canvas_input_x(px), canvas_input_y(py), pressure, ts,
                          g_width_scale);
    raw_draw_segment(px, py, raw_w);
    perf_log_event("pen_move", elapsed_us(t0));
    return NULL;
  }
  if (isPen &&
      (etype == NSEventTypeLeftMouseUp || etype == NSEventTypeRightMouseUp ||
       etype == NSEventTypeOtherMouseUp)) {
    // Pen up: finalize modeler, commit smoothed points (or erase)
    NSLog(@"[pen] 抬笔 (pen up)");
    if (g_stroke_active) {
      if (g_eraser_mode) {
        glaspen2_modeler_erase_finish();
        g_eraser_mode = NO;
        update_status_icon_state(); // 橡皮块换回笔尖
      } else {
        glaspen2_modeler_end(canvas_input_x(px), canvas_input_y(py), pressure,
                             ts, g_width_scale);
        glaspen2_modeler_commit_to_strokes(g_pen_r, g_pen_g, g_pen_b);
      }

      // P0: no rebuild — raw drawing remains on the surface.
      // Undo still calls rebuild_surface_from_strokes() to clear erased
      // strokes.
      stroke_end(); // release shared cairo context
      g_stroke_active = NO;
      g_raw_has_last = NO;
      g_raw_has_prev = NO;
      g_raw_path_len = 0;
      // Pen is back to hovering — restore the crosshair.
      g_cursor_visible = YES;
      dirty_include_point(g_cursor_x, g_cursor_y, 14.0);
      flush_dirty_to_layer();
      perf_log_event("pen_up", elapsed_us(t0));
    }
    return NULL;
  }

  perf_log_event("tick", elapsed_us(t0));
  return event;
}

/// 合成事件(虚拟笔)处理完一律吞掉: 落笔/拖动本来就是 return NULL, 但**悬停
/// 走的是放行路径** —— 不拦住的话虚拟笔的 hover 流会真的移动用户光标。
/// 测试工具绝不能抢用户的鼠标。
static CGEventRef event_tap_callback(CGEventTapProxy proxy, CGEventType type,
                                     CGEventRef event, void *refcon) {
  CGEventRef out = event_tap_callback_inner(proxy, type, event, refcon);
  return virtual_pen_is_event(event) ? NULL : out;
}

// Call this at app exit to dump stats
static void perf_log_summary(void) {
  if (!g_perf_file)
    return;
  fprintf(g_perf_file, "\n# SUMMARY: total=%llu slow=%llu (%.1f%%)\n",
          g_perf_total_calls, g_perf_slow_calls,
          g_perf_total_calls > 0
              ? 100.0 * g_perf_slow_calls / g_perf_total_calls
              : 0);
  fclose(g_perf_file);
  g_perf_file = NULL;
}

// --- App ---

// ---------------------------------------------------------------------------
// 虚拟笔(GLASPEN2_VIRTUAL_PEN):合成笔事件驱动完整热路径,用于性能剖析
// ---------------------------------------------------------------------------
// 合成 tablet 点事件(含压力)投递到 HID 事件口, 与真笔走同一条事件水龙头,
// 因此覆盖 模型器 → cairo → CA 上屏 全链路。GLASPEN2_VIRTUAL_PEN=1 启用:
// 每 12 秒画 3 笔(每笔 2 秒 @ 200Hz, 屏幕上会真的出现笔迹)。
// 配套 GLASPEN2_DB_PATH 指向临时库即可不碰真实数据。
// 绘制处于关闭状态(g_enabled=NO)时暂停: 否则合成事件会漏给前台应用当鼠标。
static void virtual_pen_post(CGEventType type, double x, double y,
                             double pressure) {
  CGEventRef ev = CGEventCreateMouseEvent(NULL, type, CGPointMake(x, y),
                                          kCGMouseButtonLeft);
  if (!ev)
    return;
  // NSEvent.subtype == 1(NSTabletPointEventSubtype) → 被判定为笔事件
  CGEventSetIntegerValueField(ev, kCGMouseEventSubtype, 1);
  CGEventSetIntegerValueField(ev, kCGEventSourceUserData, kVirtualPenUserData);
  CGEventSetIntegerValueField(ev, kCGTabletEventPointPressure,
                              (int64_t)(pressure * 65535.0));
  CGEventPost(kCGHIDEventTap, ev);
  CFRelease(ev);
}

static void virtual_pen_run(void) {
  for (int batch = 0;; batch++) {
    @autoreleasepool {
      for (int s = 0; s < 3 && g_enabled; s++) {
        double w = (double)g_screen_w, h = (double)g_screen_h;
        double x0 = w * 0.22 + s * w * 0.18;
        double y0 = h * 0.35;
        const int n = 400; // 2 秒 @ 200Hz
        NSLog(@"[glaspen2] virtual pen: batch %d stroke %d", batch, s);
        // 接触前 0.5 秒悬停(真笔接近板面时的 hover 流)
        for (int i = 0; i < 100 && g_enabled; i++) {
          double t = (double)i / 100;
          virtual_pen_post(kCGEventMouseMoved, x0 + t * w * 0.22, y0, 0.0);
          usleep(5000);
        }
        for (int i = 0; i <= n && g_enabled; i++) {
          double t = (double)i / n;
          double x = x0 + t * w * 0.22;
          double y = y0 + sin(t * 9.42478) * h * 0.12;
          double p = 0.4 + 0.5 * fabs(sin(t * 12.566));
          virtual_pen_post(i == 0 ? kCGEventLeftMouseDown
                                  : kCGEventLeftMouseDragged,
                           x, y, p);
          usleep(5000); // ~200Hz
        }
        virtual_pen_post(kCGEventLeftMouseUp, x0 + w * 0.22, y0, 0.0);
        // 抬笔后再悬停 0.5 秒
        for (int i = 0; i < 100 && g_enabled; i++) {
          virtual_pen_post(kCGEventMouseMoved, x0 + w * 0.22 - i * 2.0,
                           y0 + i * 1.0, 0.0);
          usleep(5000);
        }
        usleep(200000);
      }
    }
    sleep(9);
  }
}

static void virtual_pen_maybe_start(void) {
  const char *v = getenv("GLASPEN2_VIRTUAL_PEN");
  if (!v || !*v || strcmp(v, "0") == 0)
    return;
  NSLog(@"[glaspen2] virtual pen enabled (GLASPEN2_VIRTUAL_PEN): 每 12 秒画 3 "
        @"笔");
  dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
    virtual_pen_run();
  });
}

// 每次载入某页(翻页/启动/显示变化/新建复用)后调用: 依据该页几何
// 设置页视图变换。页 = 某时刻玻璃的几何快照; 同几何 = 恒等(纯玻璃)。
static void pageview_update(void) {
  g_pageview_pw = 0;
  g_pageview_ph = 0;
  if (g_infinite_canvas) {
    g_pageview_fit = NO;
    g_pageview_scale = 1.0;
    g_pageview_ox = g_pageview_oy = 0.0;
    return;
  }
  glaspen2_page_dims(glaspen2_get_current_screen_id(), &g_pageview_pw,
                     &g_pageview_ph);
  if (g_pageview_pw <= 0 || g_pageview_ph <= 0 ||
      (g_pageview_pw == g_screen_w && g_pageview_ph == g_screen_h)) {
    g_pageview_fit = NO;
    g_pageview_scale = 1.0;
    g_pageview_ox = g_pageview_oy = 0.0;
    return;
  }
  g_pageview_scale = MIN((double)g_screen_w / g_pageview_pw,
                         (double)g_screen_h / g_pageview_ph);
  g_pageview_ox = ((double)g_screen_w - g_pageview_pw * g_pageview_scale) / 2.0;
  g_pageview_oy = ((double)g_screen_h - g_pageview_ph * g_pageview_scale) / 2.0;
  g_pageview_fit = YES;
  NSLog(@"[glaspen2] pageview fit: 页 %dx%d → 屏 %dx%d scale=%.3f "
        @"off=(%.0f,%.0f)",
        g_pageview_pw, g_pageview_ph, g_screen_w, g_screen_h, g_pageview_scale,
        g_pageview_ox, g_pageview_oy);
}

// 诊断钩子: GLASPEN2_FLIP_EVERY=<秒> 周期性触发翻页(走 perform_hotkey
// 与真实热键同一条路)。第一跳先建第二页, 之后往返翻页供连拍/录屏实验。
static void flip_probe_maybe_start(void) {
  const char *v = getenv("GLASPEN2_FLIP_EVERY");
  if (!v || atoi(v) <= 0)
    return;
  int every = atoi(v);
  NSLog(@"[glaspen2] flip probe: 每 %d 秒一跳(第1跳建页, 之后往返翻)", every);
  // 源必须静态持有: 局部变量在函数退出时被 ARC 释放, 定时器永远不触发
  static dispatch_source_t s_probe_src;
  static BOOL back = NO;
  static BOOL first = YES;
  s_probe_src = dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER, 0, 0,
                                       dispatch_get_main_queue());
  dispatch_source_set_timer(
      s_probe_src,
      dispatch_time(DISPATCH_TIME_NOW, (int64_t)every * NSEC_PER_SEC),
      (uint64_t)every * NSEC_PER_SEC, (int64_t)0.2 * NSEC_PER_SEC);
  dispatch_source_set_event_handler(s_probe_src, ^{
    if (first) {
      first = NO;
      // 诊断环境: GLASPEN2_PROBE_PAGES=n 直接铺 n 页空白页(不涂鸦,
      // 翻页测试只需要页存在; 玻璃框本身提供视觉内容)。
      int want = 0;
      const char *vpn = getenv("GLASPEN2_PROBE_PAGES");
      if (vpn)
        want = atoi(vpn);
      glaspen2_debug_insert_pages(want, g_screen_w, g_screen_h);
      NSLog(@"[glaspen2] probe 建页 %d 页(空白, 直插)", want);
      return;
    }
    NSLog(@"[glaspen2] probe 翻页 back=%d", back);
    perform_hotkey(back ? 0x32 : 0x12); // 往返: 上一页 / 下一页
    back = !back;
  });
  dispatch_resume(s_probe_src);
}

// 颜色/粗细预设表从 glaspen-core presets 填充(RGB 单源;名字是本平台
// UI 字符串)。须在任何读表代码(建菜单/恢复设置)之前调用。
static void gl_load_pen_presets(void) {
  if (glaspen2_color_preset_count() != g_color_preset_count ||
      glaspen2_width_preset_count() != g_width_preset_count) {
    NSLog(@"[glaspen2] WARN: core 预设数量与 ObjC 表不一致 (color %d/%d, width "
          @"%d/%d)",
          glaspen2_color_preset_count(), g_color_preset_count,
          glaspen2_width_preset_count(), g_width_preset_count);
  }
  for (int i = 0; i < g_color_preset_count; i++) {
    glaspen2_color_preset_rgb(i, &g_color_presets[i].r, &g_color_presets[i].g,
                              &g_color_presets[i].b);
  }
  for (int i = 0; i < g_width_preset_count; i++) {
    g_width_presets[i] = glaspen2_width_preset_value(i);
  }
}

// 单实例守卫: 两个实例 = 两套全屏窗口叠加显示 + 同一全局热键同时翻
// 两套页, 表现为"笔迹重复/翻页只翻了一部分"。flock 随进程生死自动
// 释放, 崩溃也不会留下死锁。GLASPEN2_ALLOW_MULTI=1 可并行调试。
// 返回锁 fd(保持打开, 不 close); 已有实例时返回 -1。
static int instance_lock_acquire(void) {
  if (getenv("GLASPEN2_ALLOW_MULTI"))
    return -2; // 显式旁路
  NSString *path = [NSTemporaryDirectory()
      stringByAppendingPathComponent:@"glaspen2.instance.lock"];
  int fd = open([path fileSystemRepresentation], O_CREAT | O_RDWR, 0666);
  if (fd < 0)
    return -1;
  if (flock(fd, LOCK_EX | LOCK_NB) == 0)
    return fd; // 拿到锁, 故意不 close
  close(fd);
  return -1;
}

// 旧版实例(没有 flock 逻辑的安装版)不会去抢锁 —— 用运行应用列表兜底:
// 排除自己之外还有叫 glaspen2 的进程就算已运行。自动更新的 --updater
// 帮手与主程序同名, 更新窗口期内手动启动会误报, 属可接受的罕见边角。
static BOOL another_glaspen2_running(void) {
  for (NSRunningApplication *app in
       [NSWorkspace.sharedWorkspace runningApplications]) {
    if (app.processIdentifier == getpid())
      continue;
    NSString *name = app.localizedName ?: app.executableURL.lastPathComponent;
    if ([name isEqualToString:@"glaspen2"])
      return YES;
  }
  return NO;
}

void glaspen2_run(void) {
  @autoreleasepool {
    // 单实例守卫先于一切初始化: 第二个实例直接弹窗退出
    int lock_fd = instance_lock_acquire();
    if (lock_fd == -1 || another_glaspen2_running()) {
      [NSApplication sharedApplication];
      [NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];
      NSAlert *a = [[NSAlert alloc] init];
      a.messageText = L(@"glaspen2 已在运行", @"glaspen2 is already running");
      a.informativeText =
          L(@"检测到另一个 glaspen2 "
            @"实例(菜单栏图标)。两个实例的笔迹窗口会叠加显示, "
            @"全局热键会同时翻两套页 —— "
            @"看起来像“笔迹重复/只翻了一部分”。本次启动退出; "
            @"开发时如需并行调试, 设 GLASPEN2_ALLOW_MULTI=1。",
            @"Another glaspen2 instance is running (menu bar icon). Two "
            @"overlays stack and global hotkeys flip both — it looks like "
            @"duplicated strokes or a partial page flip. This launch exits; "
            @"set GLASPEN2_ALLOW_MULTI=1 to debug in parallel.");
      [a addButtonWithTitle:L(@"好", @"OK")];
      [a runModal];
      return;
    }

    [NSApplication sharedApplication];
    [NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];

    // 预设表先于建菜单/恢复设置填充(单一事实源在 core)
    gl_load_pen_presets();

    // Request accessibility permission (needed for CGEventTap)
    NSDictionary *opts = @{(__bridge id)kAXTrustedCheckOptionPrompt : @YES};
    if (!AXIsProcessTrustedWithOptions((__bridge CFDictionaryRef)opts)) {
      NSLog(@"[glaspen2] Accessibility permission not granted");
    }

    // Create status bar menu
    g_statusItem = [[NSStatusBar systemStatusBar]
        statusItemWithLength:NSSquareStatusItemLength];
    [g_statusItem.button setTitle:@""]; // 纯图标, 不带文字(状态都画在图里)

    g_menuHandler = [[GlaspenMenuHandler alloc] init];
    [NSApp setDelegate:g_menuHandler];

    g_menu = [[NSMenu alloc] init];
    [g_menu setDelegate:g_menuHandler];
    [g_menu setAutoenablesItems:NO];

    // Color items with swatch images and names
    // Color items: inline color dot + name (attributed — see
    // gl_color_item_text)
    for (int i = 0; i < g_color_preset_count; i++) {
      NSMenuItem *item = [g_menu addItemWithTitle:@""
                                           action:@selector(selectColor:)
                                    keyEquivalent:@""];
      item.attributedTitle = gl_color_item_text(i);
      item.target = g_menuHandler;
      item.tag = i;
    }

    [g_menu addItem:[NSMenuItem separatorItem]];

    // Width items: inline line-thickness icon + name
    for (int i = 0; i < g_width_preset_count; i++) {
      NSMenuItem *item = [g_menu addItemWithTitle:@""
                                           action:@selector(selectWidth:)
                                    keyEquivalent:@""];
      item.attributedTitle = gl_width_item_text(i);
      item.target = g_menuHandler;
      item.tag = i;
    }

    [g_menu addItem:[NSMenuItem separatorItem]];
    [g_menu addItemWithTitle:L(@"保存(含背景)", @"Save (with bg)")
                      action:@selector(saveWithBg)
               keyEquivalent:@""];
    [g_menu addItemWithTitle:L(@"保存(涂鸦)", @"Save (drawing)")
                      action:@selector(saveOnly)
               keyEquivalent:@""];
    [g_menu addItemWithTitle:L(@"保存笔记 (Xournal)", @"Save Notes (Xournal)")
                      action:@selector(saveXoj)
               keyEquivalent:@""];
    [g_menu addItemWithTitle:L(@"导出无限画布 PDF (分页)",
                               @"Export infinite canvas PDF (paged)")
                      action:@selector(exportInfinitePdf)
               keyEquivalent:@""];
    [g_menu addItemWithTitle:L(@"导出无限画布 SVG (整幅)",
                               @"Export infinite canvas SVG (whole)")
                      action:@selector(exportInfiniteSvg)
               keyEquivalent:@""];
    [g_menu addItemWithTitle:L(@"新建画布", @"New canvas")
                      action:@selector(clearScreen)
               keyEquivalent:@""];
    NSMenuItem *rainbowItem =
        [g_menu addItemWithTitle:L(@"彩虹指示器", @"Rainbow indicator")
                          action:@selector(toggleRainbow)
                   keyEquivalent:@""];
    rainbowItem.target = g_menuHandler;
    rainbowItem.tag = 999;
    rainbowItem.state = NSControlStateValueOff;
    NSMenuItem *launchItem =
        [g_menu addItemWithTitle:L(@"开机自启", @"Launch at login")
                          action:@selector(toggleLaunch)
                   keyEquivalent:@""];
    launchItem.target = g_menuHandler;
    launchItem.tag = 777;
    launchItem.state = glaspen2_is_launch_at_login() ? NSControlStateValueOn
                                                     : NSControlStateValueOff;
    NSMenuItem *glassItem =
        [g_menu addItemWithTitle:L(@"磨砂玻璃", @"Frosted Glass")
                          action:@selector(toggleGlass)
                   keyEquivalent:@""];
    glassItem.target = g_menuHandler;
    glassItem.tag = 444;
    glassItem.state =
        g_glass_enabled ? NSControlStateValueOn : NSControlStateValueOff;
    NSMenuItem *outlineItem =
        [g_menu addItemWithTitle:L(@"笔迹描边", @"Stroke outline")
                          action:@selector(toggleOutline)
                   keyEquivalent:@""];
    outlineItem.target = g_menuHandler;
    outlineItem.tag = 667;
    outlineItem.state = NSControlStateValueOff;
    NSMenuItem *infiniteItem =
        [g_menu addItemWithTitle:L(@"无限画布", @"Infinite canvas")
                          action:@selector(toggleInfiniteCanvas)
                   keyEquivalent:@""];
    infiniteItem.target = g_menuHandler;
    infiniteItem.tag = 668;
    infiniteItem.state = NSControlStateValueOff;
    NSMenuItem *modeItem =
        [g_menu addItemWithTitle:L(@"固定画布涂鸦模式", @"Fixed canvas mode")
                          action:@selector(toggleCanvasMode)
                   keyEquivalent:@""];
    modeItem.target = g_menuHandler;
    modeItem.tag = 778;
    modeItem.state = NSControlStateValueOff;
    [g_menu addItem:[NSMenuItem separatorItem]];
    NSMenuItem *toggleItem =
        [g_menu addItemWithTitle:L(@"开启涂鸦", @"Enable Drawing")
                          action:@selector(toggleDraw)
                   keyEquivalent:@""];
    toggleItem.target = g_menuHandler;
    toggleItem.tag = 888;
    toggleItem.state = NSControlStateValueOn;
    NSMenuItem *settingsItem =
        [g_menu addItemWithTitle:L(@"设置...", @"Settings...")
                          action:@selector(showSettingsPanel)
                   keyEquivalent:@""];
    settingsItem.target = g_menuHandler;
    [g_menu addItem:[NSMenuItem separatorItem]];
    NSMenuItem *langItem = [g_menu addItemWithTitle:L(@"English", @"中文")
                                             action:@selector(toggleLanguage)
                                      keyEquivalent:@""];
    langItem.target = g_menuHandler;
    NSMenuItem *quitItem = [g_menu addItemWithTitle:L(@"退出", @"Quit")
                                             action:@selector(quitApp)
                                      keyEquivalent:@""];
    quitItem.target = g_menuHandler;

    // Set target for action items (save, clear)
    for (NSMenuItem *item in [g_menu itemArray]) {
      if (!item.isSeparatorItem && !item.target &&
          item.action != @selector(selectColor:) &&
          item.action != @selector(selectWidth:)) {
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
    pageview_update(); // 启动落在末页, 页几何可能 ≠ 屏幕

    // Restore saved pen color and width
    double sr, sg, sb, sw;
    if (glaspen2_load_settings_parts(&sr, &sg, &sb, &sw)) {
      g_pen_r = sr;
      g_pen_g = sg;
      g_pen_b = sb;
      g_width_scale = sw;
      // 最近预设匹配(core 单源,Windows 同一实现)
      g_selectedColorIndex = glaspen2_nearest_color_index(sr, sg, sb);
      g_selected_width_index = glaspen2_nearest_width_index(sw);
      // 存档色吸附到最近预设: 色板调亮这类更新后, 旧存档值自动迁移
      // (本来就是预设值时为恒等, 自定义场景不存在——色板是唯一入口)
      glaspen2_color_preset_rgb(g_selectedColorIndex, &g_pen_r, &g_pen_g,
                                &g_pen_b);
    }
    update_status_icon_state();
    update_menu_checkmarks();

    // Restore glass settings (opacity stored as millipercent, enabled as bool)
    int glass_milli = glaspen2_load_bool_setting("glass_alpha");
    if (glass_milli > 0)
      g_glass_opacity = glass_milli / 1000.0;
    g_glass_enabled = glaspen2_load_bool_setting("glass_enabled") != 0;
    gl_glass_apply();

    // Restore grid setting
    g_show_grid = glaspen2_load_bool_setting("grid") != 0;
    {
      char *vgs = glaspen2_load_string_setting("grid_size");
      if (vgs) {
        double gs = atof(vgs);
        if (gs >= 10 && gs <= 200)
          g_grid_size = gs;
        glaspen2_free_c_string(vgs);
      }
    }
    g_grid_follow_strokes =
        glaspen2_load_bool_setting("grid_follow_strokes") != 0;
    {
      // 玻璃跟随涂鸦: 缺省 = 跟随(历史行为), 只有存过关才改
      char *vgf = glaspen2_load_string_setting("glass_follow_strokes");
      if (vgf) {
        g_glass_follow_strokes = atoi(vgf) != 0;
        glaspen2_free_c_string(vgf);
      }
    }
    g_soft_shadow = glaspen2_load_bool_setting("soft_shadow") != 0;
    glaspen2_set_soft_shadow(g_soft_shadow ? 1 : 0);
    // 描边(蚂蚁线)持久化: 恢复开关并同步菜单项状态
    g_outline_enabled = glaspen2_load_bool_setting("outline") != 0;
    glaspen2_set_stroke_outline(g_outline_enabled ? 1 : 0);
    {
      NSMenuItem *oi = [g_menu itemWithTag:667];
      if (oi)
        [oi setState:g_outline_enabled ? NSControlStateValueOn
                                       : NSControlStateValueOff];
    }
    {
      char *vif = glaspen2_load_string_setting("invert_fps");
      if (vif) {
        int fps = atoi(vif);
        if (fps == 10 || fps == 30 || fps == 60 || fps == 100)
          g_invert_fps = fps;
        glaspen2_free_c_string(vif);
      }
    }
    g_invert_ink = glaspen2_load_bool_setting("invert_ink") != 0;
    if (g_invert_ink) invert_ink_apply(1);
    {
      char *vfe = glaspen2_load_string_setting("flip_effect");
      if (vfe) {
        int fe = atoi(vfe);
        if (fe >= 0 && fe <= 1)
          g_flip_effect = fe;
        glaspen2_free_c_string(vfe);
      }
    }
    // 调试: GLASPEN2_FLIP_EFFECT=1 强制时光隧道(不动用户存档设置,
    // 环境变量优先级最高 —— 评估动效时不用先去面板里改选项)
    {
      const char *vfe_env = getenv("GLASPEN2_FLIP_EFFECT");
      if (vfe_env)
        g_flip_effect = atoi(vfe_env) == 1 ? 1 : 0;
    }
    {
      char *vdv = glaspen2_load_string_setting("grid_divider");
      if (vdv) {
        int dv = atoi(vdv);
        if (dv >= 0 && dv <= 3)
          g_grid_divider = dv;
        glaspen2_free_c_string(vdv);
      }
    }

    // Restore canvas mode and load the matching independent store.
    // 翻页/无限两套存储互不影响:无限画布全局仅一个。
    g_infinite_canvas = glaspen2_load_bool_setting("infinite_canvas") != 0;
    g_minimap_enabled = glaspen2_load_bool_setting("minimap") != 0;
    NSMenuItem *infiniteRestoreItem = [g_menu itemWithTag:668];
    if (infiniteRestoreItem) {
      [infiniteRestoreItem setState:g_infinite_canvas ? NSControlStateValueOn
                                                      : NSControlStateValueOff];
    }
    glaspen2_set_canvas_kind(g_infinite_canvas ? 1 : 0);
    if (g_infinite_canvas) {
      glaspen2_load_infinite_strokes();
      canvas_infinite_load();
    } else {
      canvas_reset_lens();
    }

    // Canvas mode starts in 固定画布涂鸦模式; menu item shows the switch target
    [[g_menu itemWithTag:778]
        setTitle:L(@"飘渺画布涂鸦模式", @"Ethereal canvas mode")];

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
    if (vfps) {
      g_gif_fps = atoi(vfps);
      glaspen2_free_c_string(vfps);
    }
    char *vres = glaspen2_load_string_setting("gif_resolution");
    if (vres) {
      g_gif_resolution = atof(vres);
      glaspen2_free_c_string(vres);
    }
    char *vspeed = glaspen2_load_string_setting("gif_speed");
    if (vspeed) {
      g_gif_speed = atof(vspeed);
      glaspen2_free_c_string(vspeed);
    }
    char *vend = glaspen2_load_string_setting("gif_end_mode");
    if (vend) {
      g_gif_end_mode = atoi(vend);
      glaspen2_free_c_string(vend);
    }
    // 区间单源在 core(与 Windows 同表)
    g_gif_fps = glaspen2_clamp_setting_int("gifFps", g_gif_fps);
    g_gif_resolution =
        glaspen2_clamp_setting_double("gifResolution", g_gif_resolution);
    g_gif_speed = glaspen2_clamp_setting_double("gifSpeed", g_gif_speed);
    g_gif_end_mode = glaspen2_clamp_setting_int("gifEndMode", g_gif_end_mode);

    // 图标反映的状态(颜色/宽度/无限画布)到这里才全部恢复完, 补刷一次
    // (恢复流程更早处的刷新发生在 infinite_canvas 恢复之前, ∞ 徽标会丢)
    update_status_icon_state();

    // 涂鸦身份:DB 里的账号配置(若用户在面板配过)推给 Rust auth 模块
    glaspen2_chat_auth_reload();

    // 手写消息集成总开关(默认关;关 = 热键直通、面板收起)
    g_chat_integration = glaspen2_load_bool_setting("chat_integration") != 0;
    // 共享画布上行:集成开 + 开关此前为开 → 恢复上行
    if (g_chat_integration && glaspen2_load_bool_setting("share_ink")) {
      glaspen2_share_ink_set_active(YES);
    }

    // Apply glass visual on startup (skip if the user already started drawing)
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 300 * NSEC_PER_MSEC),
                   dispatch_get_main_queue(), ^{
                     if (g_stroke_active)
                       return;
                     if (!g_surface && g_draw_view)
                       ensure_surface(g_draw_view);
                     rebuild_surface_from_strokes();
                   });

    g_window = [[NSWindow alloc] initWithContentRect:screenFrame
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
    g_blank_cursor = [[NSCursor alloc] initWithImage:blankImg
                                             hotSpot:NSZeroPoint];
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
    GlaspenDrawView *drawView =
        [[GlaspenDrawView alloc] initWithFrame:screenFrame];
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
    signal(SIGINT, save_and_exit);  // Ctrl+C
    signal(SIGTERM, save_and_exit); // kill command

    // Listen for display changes (resolution, arrangement, etc.)
    [[NSNotificationCenter defaultCenter]
        addObserverForName:NSApplicationDidChangeScreenParametersNotification
                    object:nil
                     queue:[NSOperationQueue mainQueue]
                usingBlock:^(NSNotification *note) {
                  on_display_changed();
                }];

    // CGEventTap: intercept events at system level before dispatch
    event_tap_reinstall();

    if (g_event_tap) {
      NSLog(@"[glaspen2] CGEventTap created OK, enabled=%d",
            CGEventTapIsEnabled(g_event_tap));
    } else {
      NSString *bundlePath = [[NSBundle mainBundle] bundlePath];
      NSLog(@"[glaspen2] CGEventTap FAILED - need Accessibility permission "
            @"for: %@",
            bundlePath);
      // Show alert and open accessibility settings
      dispatch_async(dispatch_get_main_queue(), ^{
        NSAlert *alert = [[NSAlert alloc] init];
        alert.messageText =
            L(@"需要辅助功能权限", @"Accessibility Permission Required");
        alert.informativeText =
            L(@"请在系统设置 → 隐私与安全性 → 辅助功能中添加并勾选 "
              @"glaspen2。\n\n添加后请点击下方按钮重启应用。",
              @"Please add and enable glaspen2 in System Settings → Privacy & "
              @"Security → Accessibility.\n\nAfter enabling, click below to "
              @"restart.");
        [alert addButtonWithTitle:L(@"打开系统设置并重启",
                                    @"Open Settings & Restart")];
        [alert addButtonWithTitle:L(@"取消", @"Cancel")];
        if ([alert runModal] == NSAlertFirstButtonReturn) {
          [[NSWorkspace sharedWorkspace]
              openURL:[NSURL URLWithString:
                                 @"x-apple.systempreferences:com.apple."
                                 @"preference.security?Privacy_Accessibility"]];
          // Relaunch after a short delay
          NSString *path = bundlePath;
          dispatch_after(
              dispatch_time(DISPATCH_TIME_NOW, (int64_t)(0.5 * NSEC_PER_SEC)),
              dispatch_get_main_queue(), ^{
                [[NSWorkspace sharedWorkspace] launchApplication:path];
                [NSApp terminate:nil];
              });
        }
      });
    }

    // 调试开关(环境变量):性能日志 / 虚拟笔, 默认都关
    perf_log_init_from_env();
    virtual_pen_maybe_start();
    flip_probe_maybe_start();

    [NSApp run];
  }
}
