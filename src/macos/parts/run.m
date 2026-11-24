}

// 鼠标 ⌃⌥ + 左键拖动镜头(临时抓手)状态; 由 event tap 维护。
static BOOL g_camera_mouse_dragging = NO;
static NSPoint g_camera_mouse_last = {0, 0};
// 鼠标拖动平移是高频事件; 累积位移, 每 ~16ms 只重建一次 surface.
static double g_camera_pan_pending_dx = 0.0;
static double g_camera_pan_pending_dy = 0.0;
static BOOL g_camera_pan_scheduled = NO;
static uint64_t g_camera_pan_gen = 0;
static void camera_mouse_cancel_pan(void);

static void pageview_frame_draw(void);
static void rebuild_surface_from_strokes(void) {
  if (!g_surface)
    return;
  // Delegate the actual Cairo rendering to Rust (avoids per-point FFI overhead)
  glaspen2_draw_rebuild_view((void *)g_surface, g_scale, g_pageview_ox,
                             g_pageview_oy, g_pageview_scale);
  pageview_frame_draw();

  cairo_surface_flush(g_surface);
  // Full-screen refresh — undo/page-nav/resize need it
  dirty_reset();
  flush_to_layer();
}

// 时光隧道专用半分辨率表面: 卡片快照本来就是 half-res, 直接在 half-res
// 上合成再放大上屏, 填充量降到 1/4, 是 4K 下把 40fps 拉到 60+ 的关键。
static cairo_surface_t *s_tun_surface = NULL;
static CGImageRef s_tun_cgimage = NULL;
static unsigned char *s_tun_cgimage_data = NULL;
static int s_tun_cgimage_w = 0, s_tun_cgimage_h = 0, s_tun_cgimage_stride = 0;

/// 隧道帧上屏(半分辨率表面由 drawRect 放大绘制)。
static void flush_tunnel_to_layer(void) {
  if (s_tun_surface && g_draw_view)
    [g_draw_view setNeedsDisplay:YES];
}

// ── 网格/分栏绘制(D6: 几何唯一来源 = core::grid,经 FFI 下发线段表) ──
//
// 本文件不再算任何网格几何(旧 run.m 内联的三段循环已删): 视图尺寸/镜头/网格
// 设置经 glaspen2_grid_json(_clipped) 交给 core, 这里只把 core 算好的线段按原有
// CGContext 样式描边(颜色/线宽/alpha/绘制顺序全部保持)。
// 绘制顺序 = 细网格 → 屏幕分界线 → 分栏参考线(后画的覆盖先画的, 与旧代码同序)。

// 一层线段 → CG 路径 + 单次 StrokePath(与旧实现一样整层一条路径: 交叉点只
// 合成一次 alpha)。线段是视图坐标(y 向下), CGContext 是 y 向上, 这里翻转 y。
static void grid_stroke_layer(CGContextRef ctx, NSArray *segments, double view_h,
                              CGFloat white, CGFloat alpha, CGFloat line_width) {
  if (![segments isKindOfClass:[NSArray class]] || segments.count == 0) return;
  CGContextSetStrokeColorWithColor(
      ctx, [[NSColor colorWithWhite:white alpha:alpha] CGColor]);
  CGContextSetLineWidth(ctx, line_width);
  for (id item in segments) {
    if (![item isKindOfClass:[NSDictionary class]]) continue;
    NSDictionary *seg = item;
    NSNumber *x0 = seg[@"x0"], *y0 = seg[@"y0"], *x1 = seg[@"x1"], *y1 = seg[@"y1"];
    if (![x0 isKindOfClass:[NSNumber class]] || ![y0 isKindOfClass:[NSNumber class]] ||
        ![x1 isKindOfClass:[NSNumber class]] || ![y1 isKindOfClass:[NSNumber class]])
      continue;
    CGContextMoveToPoint(ctx, [x0 doubleValue], view_h - [y0 doubleValue]);
    CGContextAddLineToPoint(ctx, [x1 doubleValue], view_h - [y1 doubleValue]);
  }
  CGContextStrokePath(ctx);
}

// core 网格 FFI JSON → 字典;失败/error JSON 只记一次日志(避免每帧刷屏)并回 nil。
static NSDictionary *grid_json_dict(char *json) {
  static BOOL s_grid_ffi_warned = NO;
  if (!json) {
    if (!s_grid_ffi_warned) {
      s_grid_ffi_warned = YES;
      NSLog(@"[grid] core 网格 FFI 返回 NULL");
    }
    return nil;
  }
  NSData *data = [NSData dataWithBytes:json length:strlen(json)];
  glaspen2_free_c_string(json);
  id parsed = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
  if (![parsed isKindOfClass:[NSDictionary class]]) {
    if (!s_grid_ffi_warned) {
      s_grid_ffi_warned = YES;
      NSLog(@"[grid] core 网格 JSON 无法解析");
    }
    return nil;
  }
  NSDictionary *dict = parsed;
  if (dict[@"error"]) {
    if (!s_grid_ffi_warned) {
      s_grid_ffi_warned = YES;
      NSLog(@"[grid] core 网格 FFI 错误: %@", dict[@"error"]);
    }
    return nil;
  }
  return dict;
}

// 网格 + 分栏参考线:门控与视图←画布变换保持旧 drawRect 逐字一致。
// clipRect = 本次 drawRect 的脏矩形(非翻转视图坐标, y 向上), 只向 core 要相交的
// 线(旧实现同样的省遍历);视图尺寸 = bounds(macOS 窗口 = 屏幕 frame)。
static void draw_grid_lines(CGContextRef ctx, NSRect bounds, NSRect clipRect) {
  // 显示网格(细网格+屏幕分界线)与分栏参考线是两个独立设置, 各画各层;
  // 网格跟随涂鸦 on 时两者都随笔迹隐藏(飘渺画布涂鸦模式)。
  if (!((g_show_grid || g_grid_divider > 0) && !s_tun_active &&
        (g_strokes_visible || !g_grid_follow_strokes)))
    return;
  double view_w = bounds.size.width, view_h = bounds.size.height;
  if (!(view_w > 0.0) || !(view_h > 0.0)) return;

  // 两种模式统一:视图 = (画布 − 镜头偏移) × 缩放。
  // 无限画布:偏移=pan、缩放=zoom(旧代码同款 0.05 兜底, 见 camera.rs 差异 3);
  // 活页本:偏移=−滚动偏移、缩放=1(整页翻页后滚动偏移恒 0)。
  double pan_x = 0.0, pan_y = 0.0, z = 1.0;
  if (g_infinite_canvas) {
    pan_x = g_pan_x;
    pan_y = g_pan_y;
    z = (g_zoom > 0.05) ? g_zoom : 0.05;
  } else {
    pan_x = -g_page_off_x;
    pan_y = -g_page_off_y;
  }

  // clipRect 是非翻转视图坐标(y 向上)→ core 视图坐标(y 向下)。
  double clip_x0 = NSMinX(clipRect), clip_x1 = NSMaxX(clipRect);
  double clip_y0 = view_h - NSMaxY(clipRect), clip_y1 = view_h - NSMinY(clipRect);
  int follow = g_grid_follow_strokes ? 1 : 0;
  int divider = (int)g_grid_divider;

  char *json;
  if (!g_show_grid) {
    // 只开分栏: 分栏线每屏单位 1–4 条, 全视口算, 不需要脏矩形裁剪。
    json = glaspen2_grid_json_clipped(view_w, view_h, pan_x, pan_y, z,
                                      g_grid_size, divider, /*show_grid=*/0,
                                      0.0, 0.0, view_w, view_h);
  } else if (clip_x0 <= 0.0 && clip_y0 <= 0.0 && clip_x1 >= view_w &&
             clip_y1 >= view_h) {
    // 整屏重绘: 规范入口(屏幕单位 = 视口尺寸 = macOS 视图/屏幕一致)。
    json = glaspen2_grid_json(view_w, view_h, pan_x, pan_y, z, g_grid_size,
                              divider, follow);
  } else {
    // 笔迹脏矩形: 只请求与 clip 相交的线(旧实现只算脏区, 省整屏遍历)。
    json = glaspen2_grid_json_clipped(view_w, view_h, pan_x, pan_y, z,
                                      g_grid_size, divider, /*show_grid=*/1,
                                      clip_x0, clip_y0, clip_x1, clip_y1);
  }
  NSDictionary *lines = grid_json_dict(json);
  if (!lines) return;
  // 样式与旧代码逐值一致: 0.5pt/0.5/alpha0.15;1.0pt/0.5/alpha0.55;
  // 1.5pt/0.5/alpha0.65。
  grid_stroke_layer(ctx, lines[@"minor"], view_h, 0.5, 0.15, 0.5);
  grid_stroke_layer(ctx, lines[@"screen"], view_h, 0.5, 0.55, 1.0);
  grid_stroke_layer(ctx, lines[@"divider"], view_h, 0.5, 0.65, 1.5);
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

  // 时光隧道期间: 只显示半分辨率隧道表面(放大到视图), 不画网格/笔迹。
  if (s_tun_active && s_tun_surface) {
    cairo_surface_flush(s_tun_surface);
    unsigned char *td = cairo_image_surface_get_data(s_tun_surface);
    int tw = cairo_image_surface_get_width(s_tun_surface);
    int th = cairo_image_surface_get_height(s_tun_surface);
    int ts = cairo_image_surface_get_stride(s_tun_surface);
    if (td && tw > 0 && th > 0 &&
        (!s_tun_cgimage || s_tun_cgimage_data != td ||
         s_tun_cgimage_w != tw || s_tun_cgimage_h != th ||
         s_tun_cgimage_stride != ts)) {
      CGImageRelease(s_tun_cgimage);
      s_tun_cgimage = NULL;
      CGColorSpaceRef cs = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
      CGDataProviderRef provider =
          CGDataProviderCreateWithData(NULL, td, ts * th, NULL);
      s_tun_cgimage = CGImageCreate(
          tw, th, 8, 32, ts, cs,
          kCGBitmapByteOrder32Little | kCGImageAlphaPremultipliedFirst,
          provider, NULL, false, kCGRenderingIntentDefault);
      CGDataProviderRelease(provider);
      CGColorSpaceRelease(cs);
      s_tun_cgimage_data = td;
      s_tun_cgimage_w = tw;
      s_tun_cgimage_h = th;
      s_tun_cgimage_stride = ts;
    }
    if (s_tun_cgimage) {
      NSRect b = [self bounds];
      CGContextDrawImage(ctx, NSRectToCGRect(b), s_tun_cgimage);
    }
    CGContextRestoreGState(ctx);
    return;
  }

  // 网格 + 分栏参考线: 几何唯一来源 = core::grid(经 FFI 线段表),
  // 本函数只保持原有门控与 CGContext 样式/绘制顺序(D6)。
  draw_grid_lines(ctx, [self bounds], clipRect);

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

  // Draw notification text (S4-2: 绘制已迁 Swift NotificationPresenter)
  {
    NSRect noteBounds = [self bounds];
    [NotificationPresenter drawInBounds:noteBounds];
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
  g_camera_mouse_dragging = NO; // 模式切换时结束任何鼠标抓手
  camera_mouse_cancel_pan();
  finish_active_stroke(); // 先把在写的笔画落库到"旧"存储
  if (g_infinite_canvas)
    canvas_infinite_persist(); // 离开无限画布前存镜头
  g_infinite_canvas = on;
  glaspen2_save_bool_setting("infinite_canvas", on ? 1 : 0);
  NSMenuItem *item = [MenuLookup itemWithTag:668 inMenu:g_menu];
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
    show_notification(on ? gl_text(@"canvas.infinite_on")
                         : gl_text(@"canvas.infinite_off"));
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
  NSMenuItem *item = [MenuLookup itemWithTag:667 inMenu:g_menu];
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

// 键盘平移镜头(滚轮缩放走 canvas_zoom_at; 书写中忽略)。
// D5b: 数学(pan -= d)单源在 core Camera::pan_by, 经 FFI 拿回三值;
// 门控/重绘/0.5s 持久化节流/飘渺 peek 仍在本文件(adapter)。
static void canvas_pan_by(double dx, double dy) {
  if (g_stroke_active)
    return;
  NSDictionary *result = camera_result_json(
      glaspen2_camera_pan(g_pan_x, g_pan_y, g_zoom, dx, dy));
  if (!camera_assign(result, YES))
    return;
  canvas_apply_transform();
  peek_lens_change(); // 飘渺模式下短暂露出笔迹, 让镜头移动可见
}

/// 鼠标拖动平移: 高频事件只累积位移, 每帧最多真正重建一次 surface。
static void camera_mouse_flush_pan(void) {
  g_camera_pan_scheduled = NO;
  ++g_camera_pan_gen; // 作废已排队的 block
  double dx = g_camera_pan_pending_dx;
  double dy = g_camera_pan_pending_dy;
  g_camera_pan_pending_dx = 0.0;
  g_camera_pan_pending_dy = 0.0;
  if ((dx != 0.0 || dy != 0.0) && g_infinite_canvas && g_enabled &&
      !g_stroke_active) {
    canvas_pan_by(dx, dy);
  }
}

static void camera_mouse_queue_pan(double dx, double dy) {
  g_camera_pan_pending_dx += dx;
  g_camera_pan_pending_dy += dy;
  if (g_camera_pan_scheduled) return;
  g_camera_pan_scheduled = YES;
  uint64_t gen = ++g_camera_pan_gen;
  dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(16 * NSEC_PER_MSEC)),
                 dispatch_get_main_queue(), ^{
                   if (gen != g_camera_pan_gen) return;
                   camera_mouse_flush_pan();
                 });
}

/// 模式切换/停用涂鸦时调用: 丢弃未应用的鼠标拖动位移。
static void camera_mouse_cancel_pan(void) {
  g_camera_pan_pending_dx = 0.0;
  g_camera_pan_pending_dy = 0.0;
  g_camera_pan_scheduled = NO;
  ++g_camera_pan_gen;
}

// 缩放结果统一处理(鼠标锚点/键盘中心共用): 100% 上限提示(1.5s 节流)、
// 无变化早退、写回 + 应用变换/peek。
// macOS 没有 5% 下限提示(只有 Windows 有, 见 camera.rs 模块文档差异 2);
// 命中上限的判定用 core 的 hit_max(钳制前的请求越过 100%), 因此到达 100%
// 后继续放大仍会提示, 不会静默。
static void canvas_zoom_apply(NSDictionary *result) {
  if (!result)
    return;
  BOOL hit_max = [result[@"hit_max"] boolValue];
  BOOL changed = [result[@"changed"] boolValue];
  if (hit_max) {
    static CFAbsoluteTime last_hint = 0.0;
    CFAbsoluteTime now = CFAbsoluteTimeGetCurrent();
    if (now - last_hint > 1.5) {
      last_hint = now;
      show_notification(gl_text(@"zoom.max_100"));
    }
  }
  if (!camera_assign(result, YES))
    return;
  if (!changed)
    return; // 已在缩放边界/无变化: 不白 flash
  canvas_apply_transform();
  peek_lens_change(); // 飘渺模式下短暂露出笔迹, 让缩放变化可见
}

// 缩放镜头(滚轮):以视口点 (vx, vy) 为锚,zoom ∈ (0.05, 1.0]。
// 锚点的画布坐标在缩放前后保持同一屏幕位置(view = (canvas−pan)×zoom);
// 数学(锚点不变/限幅/hit 标记)单源在 core Camera::zoom_at。
static void canvas_zoom_at(double factor, double vx, double vy) {
  if (g_stroke_active)
    return;
  canvas_zoom_apply(camera_result_json(
      glaspen2_camera_zoom_at(g_pan_x, g_pan_y, g_zoom, factor, vx, vy)));
}

// 键盘缩放 = 以视口中心为锚(数学单源 core Camera::zoom_at_center)。
static void canvas_zoom_at_center(double factor) {
  if (g_stroke_active)
    return;
  canvas_zoom_apply(camera_result_json(glaspen2_camera_zoom_at_center(
      g_pan_x, g_pan_y, g_zoom, factor, (double)g_screen_w,
      (double)g_screen_h)));
}

// 保存唯一的无限画布镜头(仅无限模式;翻页模式无镜头)
// 页面缩略图条(minimap,仅翻页模式;D7: 页窗/条带几何唯一来源 = core::minimap)。
// 本文件不再算页窗与条带几何(旧 draw_minimap 的 list_screens 过滤 / 5+10 页
// 窗口 / (n-1-row)*(th+gap) 公式已删): 当前页/视图尺寸/几何组过滤宽度经
// glaspen2_minimap_layout_json 交给 core, 这里只解析 core 算好的布局 JSON,
// 再做本平台原有的缩略图队列/条带合成/描边/页号字体(样式逐字保持)。
// 布局是视图坐标(y 向下); NSImage/CGContext 是 y 向上, 合成条带时翻转。
// 性能要点:缩略图在专属串行低优先级队列逐张渲染(避免并发全屏渲染打爆 CPU),
// 每帧只把合成好的整条 strip 图贴一次(而不是逐张画 10 次);布局 JSON 只在
// 当前页/视图尺寸/几何组过滤宽度变化时重取(同旧 id 列表缓存策略)。
static dispatch_queue_t g_minimap_queue = nil;
static NSImage *g_minimap_strip = nil;        // 合成好的整条(含边框/页号)
static NSInteger g_minimap_strip_key = -1;    // 条带指纹:(当前页, 取图版本)
static NSInteger g_minimap_fetch_version = 0; // 每完成一张取图 +1

// core minimap 布局 FFI JSON → 字典;失败/error JSON 只记一次日志(避免每帧
// 刷屏)并回 nil。
static NSDictionary *minimap_layout_json(char *json) {
  static BOOL s_minimap_ffi_warned = NO;
  if (!json) {
    if (!s_minimap_ffi_warned) {
      s_minimap_ffi_warned = YES;
      NSLog(@"[minimap] core 布局 FFI 返回 NULL");
    }
    return nil;
  }
  NSData *data = [NSData dataWithBytes:json length:strlen(json)];
  glaspen2_free_c_string(json);
  id parsed = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
  if (![parsed isKindOfClass:[NSDictionary class]]) {
    if (!s_minimap_ffi_warned) {
      s_minimap_ffi_warned = YES;
      NSLog(@"[minimap] core 布局 JSON 无法解析");
    }
    return nil;
  }
  NSDictionary *dict = parsed;
  if (dict[@"error"]) {
    if (!s_minimap_ffi_warned) {
      s_minimap_ffi_warned = YES;
      NSLog(@"[minimap] core 布局 FFI 错误: %@", dict[@"error"]);
    }
    return nil;
  }
  return dict;
}

static void draw_minimap(CGContextRef ctx, NSRect bounds) {
  if (g_infinite_canvas)
    return; // 仅翻页模式
  long cur = glaspen2_get_current_screen_id();
  if (cur <= 0)
    return;

  // 容器懒初始化。此前两个全局变量恒为 nil: 发消息给 nil 是 no-op,
  // 于是"已缓存/在飞"判断永远为 false → 每帧重复排队渲染、永不缓存,
  // minimap 既空白又持续烧 CPU。
  if (!g_minimap_thumbs)
    g_minimap_thumbs = [NSMutableDictionary new];
  if (!g_minimap_inflight)
    g_minimap_inflight = [NSMutableArray new];
  if (g_minimap_thumbs.count > 128)
    [g_minimap_thumbs removeAllObjects]; // 活页本很大时别无限涨

  // 页窗 + 条带几何: 唯一来源 core::minimap(FFI)。只在当前页/视图尺寸/
  // 几何组过滤宽度变化时重取(与旧 id 列表缓存同策略: 不在绘制热路径查库)。
  // g_pageview_pw > 0 = page-view-fit 模式, core 只保留当前几何组(按页宽过滤);
  // 0 = 不过滤(与旧 if (g_pageview_pw > 0 ...) 同款)。
  double view_w = bounds.size.width, view_h = bounds.size.height;
  if (g_minimap_layout == nil || g_minimap_layout_for != cur ||
      g_minimap_layout_w != view_w || g_minimap_layout_h != view_h ||
      g_minimap_layout_geom_w != g_pageview_pw) {
    g_minimap_layout = minimap_layout_json(glaspen2_minimap_layout_json(
        cur, view_w, view_h, g_pageview_pw));
    g_minimap_layout_for = cur;
    g_minimap_layout_w = view_w;
    g_minimap_layout_h = view_h;
    g_minimap_layout_geom_w = g_pageview_pw;
    g_minimap_strip_key = -1; // 布局变了, 条带重合成
  }
  NSArray *items = g_minimap_layout[@"items"];
  if (![items isKindOfClass:[NSArray class]] || items.count == 0)
    return;
  double strip_w = [g_minimap_layout[@"strip_w"] doubleValue];
  double strip_h = [g_minimap_layout[@"strip_h"] doubleValue];
  double strip_x = [g_minimap_layout[@"strip_x"] doubleValue];
  double strip_y = [g_minimap_layout[@"strip_y"] doubleValue];
  if (!(strip_w > 0.0) || !(strip_h > 0.0))
    return;

  // 1) 缺失的缩略图:排入专属串行队列逐张渲染(utility 优先级,
  //    不与书写落库抢锁、不并发打爆 CPU)
  static dispatch_once_t once;
  dispatch_once(&once, ^{
    g_minimap_queue =
        dispatch_queue_create("glaspen2.minimap", DISPATCH_QUEUE_SERIAL);
    dispatch_set_target_queue(g_minimap_queue,
                              dispatch_get_global_queue(QOS_CLASS_UTILITY, 0));
  });
  for (NSDictionary *item in items) {
    if (![item isKindOfClass:[NSDictionary class]])
      continue;
    NSNumber *pid = item[@"screen_id"];
    if (![pid isKindOfClass:[NSNumber class]])
      continue;
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

  // 2) 合成整条 strip(小图,只在缩略图版本/当前页/布局变化时重建)
  NSInteger key = g_minimap_fetch_version * 100000 + (NSInteger)cur;
  if (g_minimap_strip == nil || key != g_minimap_strip_key) {
    g_minimap_strip_key = key;
    NSImage *strip =
        [[NSImage alloc] initWithSize:NSMakeSize(strip_w, strip_h)];
    [strip lockFocus];
    for (NSDictionary *item in items) {
      if (![item isKindOfClass:[NSDictionary class]])
        continue;
      NSNumber *pid = item[@"screen_id"];
      if (![pid isKindOfClass:[NSNumber class]])
        continue;
      BOOL isCur = [item[@"current"] boolValue];
      // 视图 y 向下 → 条带本地向下 → lockFocus(y 向上) 翻转
      double item_y = [item[@"y"] doubleValue] - strip_y;
      double item_h = [item[@"h"] doubleValue];
      CGFloat ly = (CGFloat)(strip_h - (item_y + item_h));
      NSRect rect = NSMakeRect(0, ly, (CGFloat)strip_w, (CGFloat)item_h);
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
      // 页号 = core 下发的全列表 1 基序号(label);位置/字体/颜色逐字保持
      NSString *num = item[@"label"];
      if ([num isKindOfClass:[NSString class]] && num.length > 0) {
        [num drawAtPoint:NSMakePoint(rect.origin.x + 3, rect.origin.y + 3)
            withAttributes:@{
              NSFontAttributeName :
                  [NSFont monospacedSystemFontOfSize:9
                                              weight:NSFontWeightRegular],
              NSForegroundColorAttributeName :
                  [NSColor colorWithCalibratedWhite:0.25 alpha:0.9],
            }];
      }
    }
    [strip unlockFocus];
    g_minimap_strip = strip;
  }

  // 3) 一次贴图(右缘, 顶端 8pt; y 向下布局 → Cocoa y 向上翻转)
  NSRect dst = NSMakeRect((CGFloat)strip_x,
                          (CGFloat)(view_h - strip_y - strip_h),
                          (CGFloat)strip_w, (CGFloat)strip_h);
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
#define TUNNEL_SPAN 2.5  // 相机后方可见深度(再远就飞出画面)
#define TUNNEL_FRONT 4.5 // 相机前方可见深度(页从这里飞近)

// 玻璃表面像素尺寸(卡片的坐标空间: 全屏卡片 = 整个表面)
static int s_tun_surf_w = 0, s_tun_surf_h = 0;

// 卡片:页距(相对当前页, 整数)+ 该页的快照
static cairo_surface_t *s_tun_card[TUNNEL_MAX_CARDS];
// 每张卡的半尺寸 mip:帧内按目标尺寸选级(>0.7×主尺寸用主, 否则用 mip),
// 缩放比从最多 ~3× 降到 ≤1.41× —— 合成是每帧成本的大头(实测 24ms)。
static cairo_surface_t *s_tun_card_mip[TUNNEL_MAX_CARDS];
static cairo_surface_t *s_tun_blur = NULL; // 后层低分辨率缓冲(磨砂玻璃的"模糊")
static int s_tun_depth[TUNNEL_MAX_CARDS]; // 页距(0 = 当前页)
static int s_tun_cards = 0;
static double s_tun_travel = 1.0; // 相机推进的隧道单位(= 本次翻页跨过的页数)
static BOOL s_tun_back = NO; // 向前翻(上一页/回溯旧页): 相机**推进**页列深处
static double s_tun_dur = 0.38; // 短促: 隧道动效在感知内"一闪而过"
// 透视强度 K: s(z)=1/(1+K·z)。K 越大, 层间尺寸差越明显、隧道越"深"。
// GLASPEN2_FLIP_K 可覆盖(0.05-3.0)。
static double s_tun_k = 0.45;
// 玻璃厚度(相对卡片宽度的比例)。可调: GLASPEN2_FLIP_THICK。
// 1.8% 是直角侧视图下"能看出厚度但不抢戏"的默认值。
static double s_tun_thick = 0.018;
// 玻璃面的基准不透明度。真实玻璃需要"面"来遮挡后页; 旧值 0.08 只有
// 一层灰, 每页的空白都完全透明, 所有笔迹叠在一起(观感发假的主因)。
// 0.84 是磨砂玻璃: 近页基本挡住后页, 后页只从边缘/模糊里透出一点。
// GLASPEN2_FLIP_GLASS 可覆盖(0-0.98, 评估用)。
static double s_tun_pane_a = 0.84;
static void (^s_tun_prepare)(void);
static void (^s_tun_commit)(void);
static dispatch_source_t s_tun_timer;
static double s_tun_t0 = 0;
static uint64_t s_tun_frames = 0;

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
  if (s_tun_blur) {
    cairo_surface_destroy(s_tun_blur);
    s_tun_blur = NULL;
  }
  // 先放掉引用表面数据的 CGImage, 再销毁表面。
  if (s_tun_cgimage) {
    CGImageRelease(s_tun_cgimage);
    s_tun_cgimage = NULL;
    s_tun_cgimage_data = NULL;
  }
  if (s_tun_surface) {
    cairo_surface_destroy(s_tun_surface);
    s_tun_surface = NULL;
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
// 直角玻璃: 前脸是矩形, 只有**上侧和右侧**挤出厚度, 形成侧视图。
// 旧版圆角+四边厚板像一块平板; 侧视图能让每片玻璃的方向和前后关系
// 一眼可读(上/右侧面 = 你正看着玻璃的截面)。
static BOOL tunnel_place(double zc, double *out_cx, double *out_cy,
                         double *out_w, double *out_h, double *out_alpha) {
  if (zc < -TUNNEL_SPAN || zc > TUNNEL_FRONT)
    return NO;
  double s = 1.0 / (1.0 + s_tun_k * zc);
  if (s <= 0.06 || s > 2.4)
    return NO;
  double W = (double)s_tun_surf_w, H = (double)s_tun_surf_h;
  double w = W * s;
  double h = H * s;
  // 轻微透视偏移: 越深越靠上/靠右, 层间边缘可读; 落定 zc=0 时正好居中。
  double cx = W * 0.5 + W * 0.0045 * zc;
  double cy = H * 0.5 - H * 0.027 * zc;
  double a;
  if (zc < 0.0) {
    // 目标页从镜头前方飞入: 在 zc=-travel 处完全不可见, 避免首帧有一张
    // 半透明大页盖在当前页上。
    a = 1.0 + zc / s_tun_travel;
    if (a < 0.0)
      a = 0.0;
  } else {
    // 深度雾: 远页快速变淡(比 1/z 更陡), 只留 2-3 层可读。
    a = 1.0 / (1.0 + 0.90 * zc);
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
                             double w, double h, double alpha, double zc) {
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

  double card_x = cx - w / 2.0, card_y = cy - h / 2.0;

  // 落定帧(zc≈0): 只画真实页快照, 不叠侧面/投影。否则落定卡的空白
  // 区域仍会透出后面几页的笔迹, commit 换真实页时整屏"变干净"。
  if (fabs(zc) < 0.005) {
    cairo_save(cr);
    cairo_rectangle(cr, 0, 0, (double)s_tun_surf_w, (double)s_tun_surf_h);
    cairo_clip(cr);
    cairo_translate(cr, card_x, card_y);
    cairo_scale(cr, w / (double)nw, h / (double)nh);
    cairo_set_source_surface(cr, surf, 0, 0);
    cairo_paint_with_alpha(cr, 1.0);
    cairo_restore(cr);
    return;
  }

  double glass = fabs(zc) / 0.10;
  if (glass > 1.0)
    glass = 1.0;

  // 直角玻璃厚度 = 上/右侧面挤出的宽度。3.2% 卡宽 + 最小 4px:
  // 4K 屏上约 110px, 足以在透视里区分前后几片玻璃。
  double T = MAX(4.0, w * s_tun_thick);
  // 阴影偏移与厚度解耦, 避免厚度加大时阴影变成黑雾。
  double soff = MAX(3.0, w * 0.008);

  double zpos = zc > 0.0 ? zc : 0.0;
  if (zpos > 4.0)
    zpos = 4.0;
  double pane = (s_tun_pane_a + 0.02 * zpos) * glass;
  if (zc < 0.0)
    pane *= alpha;
  if (pane > 0.94)
    pane = 0.94;

  // 软阴影: 2 层足够(5 层全卡大小填充是 4K 下最大的单帧开销之一)。
  // 远层已经模糊/很淡, 不再画阴影。
  if (fabs(zc) < 1.6) {
    for (int i = 2; i >= 1; i--) {
      double off = i * soff * 1.6;
      cairo_set_source_rgba(cr, 0, 0, 0, 0.028 * alpha * glass);
      cairo_rectangle(cr, card_x + off, card_y + off, w, h);
      cairo_fill(cr);
    }
  }

  // ── 上侧面: 从前脸顶边向右上挤出 T ──
  {
    cairo_pattern_t *top = cairo_pattern_create_linear(
        card_x, card_y, card_x + T, card_y - T);
    cairo_pattern_add_color_stop_rgba(top, 0.0, 1.00, 1.00, 1.00,
                                      0.74 * alpha * glass);
    cairo_pattern_add_color_stop_rgba(top, 1.0, 0.90, 0.91, 0.92,
                                      0.52 * alpha * glass);
    cairo_set_source(cr, top);
    cairo_move_to(cr, card_x, card_y);
    cairo_line_to(cr, card_x + T, card_y - T);
    cairo_line_to(cr, card_x + w + T, card_y - T);
    cairo_line_to(cr, card_x + w, card_y);
    cairo_close_path(cr);
    cairo_fill(cr);
    cairo_pattern_destroy(top);
  }

  // ── 右侧面: 从前脸右边向右上挤出 T ──
  {
    cairo_pattern_t *right = cairo_pattern_create_linear(
        card_x + w, card_y, card_x + w + T, card_y - T);
    cairo_pattern_add_color_stop_rgba(right, 0.0, 0.70, 0.72, 0.74,
                                      0.62 * alpha * glass);
    cairo_pattern_add_color_stop_rgba(right, 1.0, 0.28, 0.30, 0.33,
                                      0.74 * alpha * glass);
    cairo_set_source(cr, right);
    cairo_move_to(cr, card_x + w, card_y);
    cairo_line_to(cr, card_x + w + T, card_y - T);
    cairo_line_to(cr, card_x + w + T, card_y + h - T);
    cairo_line_to(cr, card_x + w, card_y + h);
    cairo_close_path(cr);
    cairo_fill(cr);
    cairo_pattern_destroy(right);
  }

  // ── 前脸: 矩形磨砂面 + 内容 + 斜向高光 ──
  cairo_save(cr);
  cairo_rectangle(cr, card_x, card_y, w, h);
  cairo_clip(cr);
  cairo_pattern_t *fg = cairo_pattern_create_linear(card_x, card_y, card_x,
                                                    card_y + h);
  cairo_pattern_add_color_stop_rgba(fg, 0.0, 0.985, 0.99, 1.0, pane * 0.99);
  cairo_pattern_add_color_stop_rgba(fg, 0.55, 0.86, 0.875, 0.89, pane * 0.94);
  cairo_pattern_add_color_stop_rgba(fg, 1.0, 0.66, 0.675, 0.69, pane * 0.98);
  cairo_set_source(cr, fg);
  cairo_paint(cr);
  cairo_pattern_destroy(fg);

  cairo_save(cr);
  cairo_translate(cr, card_x, card_y);
  cairo_scale(cr, w / (double)nw, h / (double)nh);
  cairo_set_source_surface(cr, surf, 0, 0);
  cairo_paint_with_alpha(cr, alpha);
  cairo_restore(cr);

  // 扫光只给近处的焦点卡; 远层已经模糊, 不值得再铺一整张渐变。
  cairo_pattern_t *sheen = NULL;
  if (fabs(zc) < 1.2)
    sheen = cairo_pattern_create_linear(card_x, card_y, card_x + w,
                                        card_y + h * 0.75);
  if (sheen) {
    cairo_pattern_add_color_stop_rgba(sheen, 0.0, 1, 1, 1,
                                      0.10 * alpha * glass);
    cairo_pattern_add_color_stop_rgba(sheen, 0.35, 1, 1, 1,
                                      0.012 * alpha * glass);
    cairo_pattern_add_color_stop_rgba(sheen, 1.0, 1, 1, 1, 0.0);
    cairo_set_source(cr, sheen);
    cairo_paint(cr);
    cairo_pattern_destroy(sheen);
  }
  cairo_restore(cr);

  // ── 侧视图棱线: 背侧顶/右暗边, 前脸顶/右亮边 ──
  cairo_save(cr);
  cairo_set_line_width(cr, MAX(1.0, T * 0.08));
  // 背侧外缘(远离观察者): 左上亮、右下暗
  cairo_set_source_rgba(cr, 1.0, 1.0, 1.0, 0.42 * alpha * glass);
  cairo_move_to(cr, card_x + T, card_y - T);
  cairo_line_to(cr, card_x + w + T, card_y - T);
  cairo_stroke(cr);
  cairo_set_source_rgba(cr, 0.03, 0.03, 0.035, 0.48 * alpha * glass);
  cairo_move_to(cr, card_x + w + T, card_y - T);
  cairo_line_to(cr, card_x + w + T, card_y + h - T);
  cairo_stroke(cr);

  // 前脸与侧面的交界棱: 顶边亮、右边暗, 让截面立起来
  cairo_set_source_rgba(cr, 1.0, 1.0, 1.0, 0.50 * alpha * glass);
  cairo_move_to(cr, card_x, card_y);
  cairo_line_to(cr, card_x + w, card_y);
  cairo_stroke(cr);
  cairo_set_source_rgba(cr, 0.04, 0.04, 0.045, 0.38 * alpha * glass);
  cairo_move_to(cr, card_x + w, card_y);
  cairo_line_to(cr, card_x + w, card_y + h);
  cairo_stroke(cr);

  // 前脸左/下边缘: 极淡收边, 直角玻璃不靠圆角找轮廓
  cairo_set_source_rgba(cr, 0.9, 0.92, 0.94, 0.18 * alpha * glass);
  cairo_move_to(cr, card_x, card_y);
  cairo_line_to(cr, card_x, card_y + h);
  cairo_stroke(cr);
  cairo_set_source_rgba(cr, 0.05, 0.05, 0.055, 0.20 * alpha * glass);
  cairo_move_to(cr, card_x, card_y + h);
  cairo_line_to(cr, card_x + w, card_y + h);
  cairo_stroke(cr);
  cairo_restore(cr);
}

// 一帧:相机沿页距推进(缓入缓出), 卡片按隧道坐标摆位;
// 深处的先画、近处的后画 —— 层叠顺序即页序。
static void tunnel_frame(double p) {
  if (!s_tun_surface || s_tun_cards == 0)
    return;
  s_tun_frames++;
  double e = p < 0.5 ? 4.0 * p * p * p : 1.0 - pow(-2.0 * p + 2.0, 3.0) / 2.0;
  // 相机沿深度轴移动:回溯旧页(向前翻, s_tun_back) = cam 0→+1 深入页列,
  // 旧页从深处迎面而来, 目标页(prev, 深度+1)落定 zc=0 满屏;
  // 去往新页(向后翻) = cam 0→−1 退出, 新页(next, 深度−1)从镜头前方
  // 罩下来落定 zc=0。深度轴语义:正=旧(深处), 负=新(镜头前方)。
  double cam = e * s_tun_travel * (s_tun_back ? 1.0 : -1.0);

  cairo_t *cr = cairo_create(s_tun_surface);
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
  // 落定帧: 只画当前/目标页, 不叠其余玻璃片 —— 保证与真实页一致。
  // (zc≈0 的那张卡就是此刻的"真实页")
  for (int i = 0; i < s_tun_cards; i++) {
    int slot = order[i];
    double zc = (double)s_tun_depth[slot] - cam;
    if (fabs(zc) < 0.005) {
      double cx, cy, w, h, a;
      if (tunnel_place(zc, &cx, &cy, &w, &h, &a)) {
        tunnel_draw_card(cr, slot, cx, cy, w, h, a, zc);
        cairo_destroy(cr);
        flush_tunnel_to_layer();
        return;
      }
    }
  }
  // 非落定帧: **不铺全屏底色**。旧实现铺 42% 深色, 屏幕外围一圈都
  // 变灰, 看起来像给整个桌面蒙了层滤镜。现在只画玻璃本身, 玻璃组成的
  // 透视区域之外保持透明(桌面/窗口原样), 只有玻璃是灰的。
  // 焦点 = 最靠前且已经可见的卡片; 它画清晰, 后面的页先画到 1/3
  // 缓冲再放大(磨砂模糊)。这样近页的笔迹不会和远页的笔迹叠成重影。
  double focus_zc = 1e9;
  for (int i = 0; i < s_tun_cards; i++) {
    int slot = order[i];
    double zc = (double)s_tun_depth[slot] - cam;
    double cx, cy, w, h, a;
    if (!tunnel_place(zc, &cx, &cy, &w, &h, &a))
      continue;
    if (a >= 0.30 && zc < focus_zc)
      focus_zc = zc;
  }
  if (focus_zc > 1e8)
    focus_zc = 0.0;

  if (s_tun_blur) {
    int bw = cairo_image_surface_get_width(s_tun_blur);
    int bh = cairo_image_surface_get_height(s_tun_blur);
    cairo_t *bcr = cairo_create(s_tun_blur);
    cairo_set_operator(bcr, CAIRO_OPERATOR_SOURCE);
    cairo_set_source_rgba(bcr, 0, 0, 0, 0);
    cairo_paint(bcr);
    cairo_set_operator(bcr, CAIRO_OPERATOR_OVER);
    cairo_scale(bcr, (double)bw / (double)s_tun_surf_w,
                (double)bh / (double)s_tun_surf_h);
    for (int i = 0; i < s_tun_cards; i++) {
      int slot = order[i];
      double zc = (double)s_tun_depth[slot] - cam;
      if (zc <= focus_zc + 1e-6)
        continue;
      double cx, cy, w, h, a;
      if (!tunnel_place(zc, &cx, &cy, &w, &h, &a))
        continue;
      tunnel_draw_card(bcr, slot, cx, cy, w, h, a, zc);
    }
    cairo_destroy(bcr);
    // 放大回主表面(双线性 = 磨砂)。
    cairo_save(cr);
    cairo_scale(cr, (double)s_tun_surf_w / (double)bw,
                (double)s_tun_surf_h / (double)bh);
    cairo_set_source_surface(cr, s_tun_blur, 0, 0);
    cairo_pattern_set_filter(cairo_get_source(cr), CAIRO_FILTER_BILINEAR);
    cairo_paint(cr);
    cairo_restore(cr);
  }

  for (int i = 0; i < s_tun_cards; i++) {
    int slot = order[i];
    double zc = (double)s_tun_depth[slot] - cam;
    if (s_tun_blur && zc > focus_zc + 1e-6)
      continue;
    double cx, cy, w, h, a;
    if (!tunnel_place(zc, &cx, &cy, &w, &h, &a))
      continue;
    tunnel_draw_card(cr, slot, cx, cy, w, h, a, zc);
  }
  cairo_destroy(cr);
  flush_tunnel_to_layer();
}

static void tunnel_finish(void) {
  if (s_tun_timer) {
    dispatch_source_cancel(s_tun_timer);
    s_tun_timer = NULL;
  }
  tunnel_frame(1.0); // 最后一帧: 目标页满屏, 与真实页一模一样
  if (getenv("GLASPEN2_FLIP_FPS")) {
    double secs = tunnel_now() - s_tun_t0;
    NSLog(@"[flip] 实测 %.1f fps (%llu 帧 / %.3fs)",
          secs > 0.001 ? (double)s_tun_frames / secs : 0.0,
          (unsigned long long)s_tun_frames, secs);
  }
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
  // 防重入兜底: page_flip_swap_from 已经先 tunnel_finish() 收尾上一段;
  // 若仍处于活动态(异常路径), 不要再销毁/重建表面。
  if (s_tun_active)
    return NO;
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
  // 半分辨率: 卡片快照本身就是这个尺寸, 合成后再由 drawRect 放大 2x。
  s_tun_surf_w = (int)(g_screen_w * g_scale * 0.5);
  s_tun_surf_h = (int)(g_screen_h * g_scale * 0.5);
  if (s_tun_surf_w <= 0 || s_tun_surf_h <= 0)
    return NO;
  s_tun_surface = cairo_image_surface_create(CAIRO_FORMAT_ARGB32,
                                             s_tun_surf_w, s_tun_surf_h);
  if (cairo_surface_status(s_tun_surface) != CAIRO_STATUS_SUCCESS) {
    cairo_surface_destroy(s_tun_surface);
    s_tun_surface = NULL;
    return NO;
  }
  // 预热缓存: [最旧…, prev1, cur, next1]("透视展示 cur 到最旧的所有玻璃";
  // after 恒 1 = 目标页)。页多时受 TUNNEL_MAX_CARDS 与建卡预算保护。
  // 只预载可见纵深附近的一小段。此前 400 页会在事件 tap 线程里逐页
  // 载入 + 平滑 + 软渲染, 大库一次翻页卡数百 ms~数秒, 直接触发
  // kCGEventTapDisabledByTimeout(笔和热键一起哑)。
  // tunnel_place 只显示 zc<=TUNNEL_FRONT 的页; 12 页里 8 页建了永远画不到。
  // 4 页足够覆盖可见纵深(当前/前 2 页/后 1 页), 翻页启动快 3 倍。
  const int before = 4, after = 1;
  double tc0 = [NSDate timeIntervalSinceReferenceDate];
  int n = glaspen2_preload_flip_pages(cur, going_next ? 1 : 0, before, after);
  double tc1 = [NSDate timeIntervalSinceReferenceDate];
  if (n <= 0)
    return NO;
  if (n > TUNNEL_MAX_CARDS)
    n = TUNNEL_MAX_CARDS;

  // 后层以 1/3 分辨率绘制再放大 = 廉价磨砂模糊: 深处的页只留色块和轮廓,
  // 不会和当前页的笔迹叠成"双重曝光"。
  s_tun_blur = cairo_image_surface_create(CAIRO_FORMAT_ARGB32,
                                          s_tun_surf_w / 3,
                                          s_tun_surf_h / 3);
  if (cairo_surface_status(s_tun_blur) != CAIRO_STATUS_SUCCESS) {
    cairo_surface_destroy(s_tun_blur);
    s_tun_blur = NULL; // 退化为全部清晰绘制, 不影响动画
  }


  // 可见纵深由玻璃间距决定:淡出尽头(zc 处 alpha≈0.05)之外的页
  // 画了也不可见, 不建卡 —— "透视展示 cur 到最旧"在可见意义上成立,
  // 且库有几百页时不会为主线程塞进几百次软渲染。
  double zc_visible_max = TUNNEL_FRONT; // 真透视下由 tunnel_place 的窗口封顶
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
  // 玻璃厚度(评估用): 0.004 = 纸片, 0.04 = 厚砖
  const char *vt = getenv("GLASPEN2_FLIP_THICK");
  if (vt) {
    double v = atof(vt);
    if (v >= 0.002 && v <= 0.10)
      s_tun_thick = v;
  }
  const char *vga = getenv("GLASPEN2_FLIP_GLASS");
  if (vga) {
    double v = atof(vga);
    if (v >= 0.0 && v <= 0.98)
      s_tun_pane_a = v;
  }

  s_tun_t0 = tunnel_now();
  s_tun_frames = 0;
  // 先同步画一帧并立刻上屏:动画第一帧就是"翻页前的画面", 无跳变。
  tunnel_frame(0.0);
  if (g_draw_view)
    [g_draw_view displayIfNeeded];
  s_tun_timer = dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER, 0, 0,
                                       dispatch_get_main_queue());
  // 120Hz 定时器: 60Hz 屏会被 vsync 合并, 120Hz ProMotion 能跑满;
  // 先画新帧再 display(旧顺序是先 display 上一帧, 每 tick 多画一次旧画面)。
  dispatch_source_set_timer(s_tun_timer, dispatch_time(DISPATCH_TIME_NOW, 0),
                            (uint64_t)(1.0 / 120.0 * NSEC_PER_SEC),
                            (uint64_t)(1.0 / 1000.0 * NSEC_PER_SEC));
  dispatch_source_set_event_handler(s_tun_timer, ^{
    double t = tunnel_now() - s_tun_t0;
    double p = t / s_tun_dur;
    if (p >= 1.0) {
      tunnel_finish(); // prepare+commit: 真实页内容无缝落地
      return;
    }
    tunnel_frame(p);
    if (s_tun_active && g_draw_view)
      [g_draw_view displayIfNeeded];
  });
  dispatch_resume(s_tun_timer);
  return YES;
}

static void page_flip_swap_from(BOOL going_next, long from_page,
                                void (^prepare)(void), void (^commit)(void)) {
  // 上一段隧道还没落定时先让它收尾(commit 旧目标页), 再开始新的翻页。
  // 否则 tunnel_free_cards() 会在旧 timer 仍在跑时销毁表面, 旧 timer 也
  // 永远不会被 cancel(表现为翻页越来越快/卡顿/漏帧)。
  if (s_tun_active) {
    tunnel_finish();
  }
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
    // 新建页/气泡自动翻页的 current 可能已经被 core 切到目标页, 调用方
    // 可显式传入翻页**前**的页; 0 = 读当前页(普通上一页/下一页)。
    long cur_before =
        from_page > 0 ? from_page : glaspen2_get_current_screen_id();
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

/// 普通翻页: 翻页前的当前页 = 调用时的 current。
static void page_flip_swap(BOOL going_next, void (^prepare)(void),
                           void (^commit)(void)) {
  page_flip_swap_from(going_next, 0, prepare, commit);
}

/// 气泡「写完自动新建一页」: 落笔瞬间(笔画尚未开始)若这一笔将开新消息
/// (冷启动/静默超时, 设置 bubbleAutoNewPage × 气泡 tab 开启, 仅翻页模式),
/// 先建页并按 ⌥⌘↓ 新建同款动效翻过去 —— 笔画落在新页, 消息边界 = 页边界。
/// 返回是否翻了页。
static BOOL bubble_auto_page_flip(void) {
  long long page = glaspen2_bubble_auto_page_if_due();
  if (page <= 0)
    return NO;
  page_flip_swap(YES,
                 ^{ glaspen2_load_strokes_for_screen(page);
                    glaspen2_smooth_loaded_strokes();
                    pageview_update(); },
                 ^{ replay_strokes_from_memory(); });
  show_page_info(page);
  show_notification(gl_text(@"page.auto_created"));
  return YES;
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
  // zoom 兜底(>0 否则 1.0, 不钳 100% 上限)已在 FFI 边界按 from_db_values
  // 语义完成(D5b), 这里不再重复公式。
  g_zoom = pz;
  glaspen2_set_view_transform(g_pan_x, g_pan_y, g_zoom);
}

// 翻页模式:镜头恒为原点 + 100%(HOME 常量单源在 core Camera::HOME)
static void canvas_reset_lens(void) {
  g_page_off_x = 0.0;
  g_page_off_y = 0.0;
  glaspen2_camera_home(&g_pan_x, &g_pan_y, &g_zoom);
  glaspen2_set_view_transform(g_pan_x, g_pan_y, g_zoom);
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
      show_notification(gl_text(@"canvas.infinite_kept"));
      return YES;
    }
    clear_screen();
    return YES;
  } else if (kc == kVK_ANSI_V) {
    toggle_enabled();
    return YES;
  } else if (kc == 0x32) { // ` — previous page
    if (g_infinite_canvas) {
      show_notification(gl_text(@"canvas.single_canvas_warning"));
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
      show_notification(gl_text(@"page.no_previous"));
    }
    return YES;
  } else if (kc == 0x12) { // 1 — next page
    if (g_infinite_canvas) {
      show_notification(gl_text(@"canvas.single_canvas_warning"));
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
      // 没有下一页: 5 秒内再按一次 = 新建一页并翻过去(写完接着写)
      static NSTimeInterval lastNoNextAt = 0;
      NSTimeInterval nowTs = [[NSProcessInfo processInfo] systemUptime];
      BOOL secondPress = (nowTs - lastNoNextAt) < 5.0;
      lastNoNextAt = nowTs;
      if (!secondPress) {
        show_notification(gl_text(@"page.no_next"));
        return YES;
      }
      // core 建页时会顺手把 current 切到新页; 翻页前的页必须先抓下来,
      // 否则拿返回 id 再和 current 比较永远相等 → 建了页却不载入/不翻。
      long before = glaspen2_get_current_screen_id();
      long pageId = glaspen2_create_next_page(before);
      if (pageId > 0 && pageId != before) {
        page_flip_swap_from(
            YES, before,
            ^{ // 渐隐期间并行: 载入 + 平滑
              glaspen2_load_strokes_for_screen(pageId);
              glaspen2_smooth_loaded_strokes();
              pageview_update();
            },
            ^{
              replay_strokes_from_memory();
            });
        peek_strokes(1.0);
        show_page_info(pageId);
        show_notification(gl_text(@"page.created"));
      } else if (pageId == before) {
        show_notification(gl_text(@"page.already_latest"));
      } else {
        show_notification(gl_text(@"page.create_failed"));
      }
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
        show_notification(gl_text(@"export.svg_gif_saved"));
      } else {
        show_notification(gl_text(@"export.failed"));
      }
    }
    return YES;
  } else if (kc == kVK_ANSI_Z) {
    if (g_stroke_active) {
      show_notification(gl_text(@"draw.stroke_in_progress"));
    } else {
      int remaining = glaspen2_undo_last_stroke();
      if (remaining < 0) {
        show_notification(gl_text(@"draw.nothing_to_undo"));
      } else {
        rebuild_surface_from_strokes();
        show_notification(gl_text(@"draw.undone"));
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
      show_notification(gl_text(@"export.svg_copied"));
    } else {
      show_notification(gl_text(@"export.no_strokes_to_copy"));
    }
    return YES;
  } else if (kc == kVK_ANSI_B) {
    gl_settings_set_glass_enabled(!g_glass_enabled);
    return YES;
  } else if (kc == kVK_ANSI_X) {
    // Switch canvas mode: 固定 ↔ 飘渺 (⌘ + ⌃ + X)
    toggle_canvas_mode();
    return YES;
  } else if (kc == kVK_ANSI_D) {
    // 打开设置面板。⌘⌃D 在 macOS 是文本场景的"查词典"快捷键 ——
    // 涂鸦开启时被 tap 消费不冲突; 关闭涂鸦后词典功能照常。
    show_settings_panel();
    return YES;
  }
  // 注意: 不要在这里处理 Q — ⌃⌘Q 是 macOS 的系统锁屏快捷键,
  // 拦截它会顶掉锁屏。可配置的彻底退出动作走 perform_action("quit")
  // (默认 ⌘⌃⇧Q), 见 gl_shortcut_defaults()。
  return NO;
}

// 按动作 id 执行(事件 tap 的可配置键位表命中后调用)。旧 perform_hotkey
// 仍保留给设置面板按钮/诊断钩子; 这里用固定的 keyCode 当作 action 选择器,
// 与用户实际按下的键无关 —— 键位匹配在菜单层按 chord 完成。
//
// D3b: action id → 语义的唯一来源是 core `action_effect::effect_for_action`
// (经 FFI `glaspen2_action_effect_json` 下发 {"kind":"<变体名>"})。本函数只按
// kind 字符串分派 macOS 副作用(ObjC 无字符串 switch), 不再自己比较 action id;
// 每个分支的代码/顺序/门控/文案与旧 action 字符串链逐行一致(镜头方向符号、
// 退出路径、未知动作返回 NO 都保持原样)。
static BOOL perform_action(NSString *action) {
  if (![action isKindOfClass:[NSString class]]) return NO;
  const char *action_c = [action UTF8String];
  if (!action_c) return NO;
  char *effect_c = glaspen2_action_effect_json(action_c);
  if (!effect_c) return NO;
  // 先拷出再释放: Rust 侧返回的缓冲区由 glaspen2_free_c_string 回收。
  NSData *effect_data = [NSData dataWithBytes:effect_c length:strlen(effect_c)];
  glaspen2_free_c_string(effect_c);
  id parsed = [NSJSONSerialization JSONObjectWithData:effect_data options:0 error:nil];
  if (![parsed isKindOfClass:[NSDictionary class]]) {
    NSLog(@"[action] %@ 的 effect JSON 无法解析", action);
    return NO;
  }
  NSDictionary *effect = parsed;
  NSString *kind = effect[@"kind"];
  if (![kind isKindOfClass:[NSString class]]) {
    // 未登记/空的 action id: 记录并忽略, 等价于旧实现落空返回 NO。
    NSLog(@"[action] %@ 分发失败: %@", action, effect[@"error"]);
    return NO;
  }
  if ([kind isEqualToString:@"Undo"]) return perform_hotkey(kVK_ANSI_Z);
  if ([kind isEqualToString:@"PagePrev"]) return perform_hotkey(0x32);
  if ([kind isEqualToString:@"PageNext"]) return perform_hotkey(0x12);
  if ([kind isEqualToString:@"ToggleDraw"]) return perform_hotkey(kVK_ANSI_V);
  if ([kind isEqualToString:@"ToggleFreeCanvas"]) {
    // 打开自由画布时, 如果 Flutter 的「自由涂鸦模式」开关还是关的, 自动
    // 打开它: 否则面板没有自由涂鸦 tab, 模式切换既看不见落点, 也容易被
    // 面板的 tab 逻辑推回翻页模式。sync_settings_panel 会随后的
    // apply_infinite_canvas 把 showFreeCanvas 推给 Flutter。
    if (!g_infinite_canvas &&
        glaspen2_load_bool_setting("show_free_canvas") == 0) {
      glaspen2_save_bool_setting("show_free_canvas", 1);
    }
    // 事件 tap 回调里重装 tap 不安全: 下一 tick 再切模式(与菜单/面板同一条
    // apply_infinite_canvas 路径, 会同步面板 tab 与 ∞ 徽标)。
    dispatch_async(dispatch_get_main_queue(), ^{
      apply_infinite_canvas(!g_infinite_canvas, YES);
    });
    return YES;
  }
  // 自由涂鸦镜头: 动作只在无限画布模式生效; 事件 tap 层会放行非自由画布
  // 模式下的组合。步长除以 zoom, 保证屏幕上每次移动的视觉距离一致。
  if ([kind hasPrefix:@"CameraPan"]) {
    if (!g_infinite_canvas) return YES;
    // 步长 80/step_zoom 单源在 core(D5b);方向符号留在本平台:
    // 按键方向 = 视野移动方向(与 Windows 的"内容跟随"相反, 见
    // ActionEffect::CameraPan 文档)。
    double step = glaspen2_camera_keyboard_pan_step(g_zoom);
    double dx = 0.0, dy = 0.0;
    // 按键方向 = 视野移动方向(与普通软件一致): 按左 → 镜头左移(内容右移),
    // 按上 → 镜头上移(内容下移)。canvas_pan_by 内部是 pan -= d, 所以 d 取正
    // 表示镜头朝该方向移动。
    if ([kind isEqualToString:@"CameraPanLeft"])       dx = step;
    else if ([kind isEqualToString:@"CameraPanRight"]) dx = -step;
    else if ([kind isEqualToString:@"CameraPanUp"])    dy = step;
    else if ([kind isEqualToString:@"CameraPanDown"])  dy = -step;
    canvas_pan_by(dx, dy);
    return YES;
  }
  if ([kind isEqualToString:@"CameraZoomIn"] ||
      [kind isEqualToString:@"CameraZoomOut"]) {
    if (!g_infinite_canvas) return YES;
    double factor = [kind isEqualToString:@"CameraZoomIn"] ? 1.15 : 1.0 / 1.15;
    // 键盘缩放锚点 = 视口中心(core Camera::zoom_at_center 单源, D5b)
    canvas_zoom_at_center(factor);
    return YES;
  }
  if ([kind isEqualToString:@"Clear"]) return perform_hotkey(kVK_ANSI_C);
  if ([kind isEqualToString:@"Export"]) return perform_hotkey(kVK_ANSI_G);
  if ([kind isEqualToString:@"CopySvg"]) return perform_hotkey(kVK_ANSI_S);
  if ([kind isEqualToString:@"ToggleGlass"]) return perform_hotkey(kVK_ANSI_B);
  if ([kind isEqualToString:@"ToggleEthereal"]) return perform_hotkey(kVK_ANSI_X);
  if ([kind isEqualToString:@"OpenSettings"]) return perform_hotkey(kVK_ANSI_D);
  if ([kind isEqualToString:@"Quit"]) {
    // 彻底退出: 与菜单「退出」共用 gl_quit_app 这条收尾路径(terminate →
    // applicationShouldTerminate 写完排队点 + checkpoint WAL)。事件 tap 回调里
    // 同步 terminate 会把 DB 收尾卡在回调栈上, 放到下一 tick 更稳; 本事件仍吞掉,
    // 不让组合键漏给前台 App。
    dispatch_async(dispatch_get_main_queue(), ^{
      gl_quit_app();
    });
    return YES;
  }
  if ([kind isEqualToString:@"ColorSwap"]) {
    int prev = g_prev_color_index;
    if (prev < 0 || prev >= g_color_preset_count || prev == g_selectedColorIndex) {
      prev = (g_selectedColorIndex == 0) ? 9 : 0; // 无历史时给红/黑另一端
    }
    gl_settings_set_color(prev);
    show_notification(gl_textf(@"hud.color_changed",
                               @{@"color" : [MenuNames colorDisplayNameWithIndex:prev]}));
    return YES;
  }
  if ([kind isEqualToString:@"WidthSwap"]) {
    int prev = g_prev_width_index;
    if (prev < 0 || prev >= g_width_preset_count || prev == g_selected_width_index) {
      prev = (g_selected_width_index <= 3) ? 6 : 2; // 无历史时给细/粗另一端
    }
    gl_settings_set_width(prev);
    show_notification(gl_textf(@"hud.width_changed",
                               @{@"width" : [MenuNames widthDisplayNameWithIndex:prev]}));
    return YES;
  }
  NSLog(@"[action] %@ 的 effect kind %@ 未在 macOS 适配器实现", action, kind);
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

// ── 录制会话接线(D8b): GIF / ⌘⌃2 草稿 / ⌘⌃3 直发 ──
//
// 状态/起止窗口/决策/用户可见动作的唯一来源 = core recording::RecordingMachine
// (经 glaspen2_recording_* FFI;槽 0 = GIF,槽 1 = 手写消息)。本文件只做:
//   1) 键按下/抬起时先 finish_active_stroke 再取笔画数, 拼事件 JSON 喂状态机;
//   2) 按返回的 effects 顺序执行本平台 OS 动作(通知/编码/剪贴板/gRPC);
//   3) 后台结果 dispatch 回主线程后作为结果事件喂回, 再执行新 effects。
// HUD 文案单源在 core::i18n(D10b), 键名由 adapter 传参; 空窗口/模式不匹配/重复按下/总开关
// 门控等判定全部在 core(旧的逐模式 stop 分支与三个会话标志位已删除)。

#define REC_SLOT_GIF 0
#define REC_SLOT_CHAT 1

// 在写笔画先落定再取计数: 事件里的 stroke_count 必须是提交之后的计数,
// 否则窗口会漏掉正在写的那一笔(与两端旧顺序一致)。
static int rec_finish_and_count(void) {
  finish_active_stroke();
  return glaspen2_stroke_count();
}

// 事件 JSON 用 NSJSONSerialization 生成(单行 + 自动转义 detail 等字符串)。
static NSString *rec_json(NSDictionary *fields) {
  NSData *data = [NSJSONSerialization dataWithJSONObject:fields
                                                 options:0
                                                   error:nil];
  return data ? [[NSString alloc] initWithData:data
                                      encoding:NSUTF8StringEncoding]
              : nil;
}

// S7c: rec_key_event 已随录制热键迁移删除(事件 JSON 由 Swift 生成)。

static NSString *rec_simple_event(NSString *type) {
  return rec_json(@{@"type" : type});
}

static void rec_exec_effects(int slot, NSArray *effects);

// 喂一个事件并执行返回的 effects(在调用线程 = 主 runloop 上同步执行)。
// 返回事件是否产生了 effects; key-up 分支据此决定是否消费按键。
static BOOL rec_feed(int slot, NSString *event_json) {
  const char *c = event_json.UTF8String;
  char *out = glaspen2_recording_event_json(slot, c ? c : "");
  if (!out) {
    NSLog(@"[recording] 槽 %d: FFI 返回 NULL(不应发生)", slot);
    return NO;
  }
  NSData *data = [NSData dataWithBytes:out length:strlen(out)];
  glaspen2_free_c_string(out);
  id parsed = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
  if (![parsed isKindOfClass:[NSArray class]]) {
    NSLog(@"[recording] 槽 %d 事件执行失败: %@", slot, parsed);
    return NO;
  }
  if ([(NSArray *)parsed count] == 0)
    return NO;
  rec_exec_effects(slot, parsed);
  return YES;
}

// S6/S7: rec_start/rec_release 已并入 Swift RecordingHotkeys(事件 JSON 由 Swift 生成)。

// GIF 编码结果回主线程后待写剪贴板的字节(Clipboard effect 消费)。
static NSData *s_pending_gif = nil;

// HUD 语义 → 平台文案: 文案单源在 core::i18n(D10b, 按稳定 key 查),
// 只有 {keys} 的键名差异(⌘⌃3/⌘⌃2)由本函数作为参数传入。
static void rec_show_hud(NSDictionary *effect) {
  NSString *hud = effect[@"hud"];
  if (![hud isKindOfClass:[NSString class]])
    return;
  int strokes = [effect[@"strokes"] intValue];
  NSString *detail = effect[@"detail"];
  // S6/S8: 文案映射已迁 Swift(RecHudPresenter); 这里只解包 effect。
  [RecHudPresenter presentHud:hud strokes:strokes detail:detail];
}

// 按状态机下发的顺序执行单个 effect。
static void rec_exec_effect(int slot, NSDictionary *effect) {
  NSString *kind = effect[@"kind"];
  if (![kind isKindOfClass:[NSString class]])
    return;
  if ([kind isEqualToString:@"ShowHud"]) {
    rec_show_hud(effect);
    return;
  }
  if ([kind isEqualToString:@"EncodeGif"]) {
    int start = [effect[@"start_stroke"] intValue];
    int end = [effect[@"end_stroke"] intValue];
    // GIF 参数在松手时读(两平台差异 10: 录制中途改参数影响本次输出)。
    // EncodeGif 由 ReleasePressed 同步产生, 所以这里读到的就是松手瞬间的设置。
    int fps = g_gif_fps;
    double resolution = g_gif_resolution;
    double speed = g_gif_speed;
    int end_mode = g_gif_end_mode;
    // 回放渲染 + GIF 压缩可能上百毫秒, 不阻塞主 runloop;窗口已由状态机钉死,
    // 新一次录制不会覆盖尚在编码的这一次。
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
      int out_len = 0;
      unsigned char *gif = glaspen2_gif_record_end(
          start, end, fps, resolution, speed, end_mode, &out_len);
      dispatch_async(dispatch_get_main_queue(), ^{
        if (gif && out_len > 0) {
          s_pending_gif = [NSData dataWithBytes:gif length:out_len];
          glaspen2_free_rust_bytes(gif, out_len);
          rec_feed(REC_SLOT_GIF, rec_simple_event(@"GifEncodeSucceeded"));
        } else {
          if (gif)
            glaspen2_free_rust_bytes(gif, out_len);
          rec_feed(REC_SLOT_GIF, rec_simple_event(@"GifEncodeFailed"));
        }
      });
    });
    return;
  }
  if ([kind isEqualToString:@"Clipboard"]) {
    NSData *gif = s_pending_gif;
    s_pending_gif = nil;
    if (gif.length > 0) {
      copy_gif_data_to_clipboard((unsigned char *)gif.bytes, (int)gif.length);
    } else {
      NSLog(@"[recording] Clipboard effect 没有待写字节");
    }
    // 保持旧行为: 剪贴板写临时文件失败只 NSLog, 仍提示成功(差异 3, 不属本切片)。
    rec_feed(REC_SLOT_GIF, rec_simple_event(@"GifClipboardSucceeded"));
    return;
  }
  if ([kind isEqualToString:@"StartChatDraft"]) {
    if (glaspen2_ink_draft_start(g_screen_w, g_screen_h) != 0) {
      rec_feed(REC_SLOT_CHAT, rec_simple_event(@"ChatDraftStarted"));
    } else {
      rec_feed(REC_SLOT_CHAT, rec_simple_event(@"ChatDraftStartFailed"));
    }
    return;
  }
  if ([kind isEqualToString:@"StopChatDraft"]) {
    // 提交在写笔画, 再后台 half-close + 取裁决;结果回主线程作为
    // ChatDraftFinished 喂回状态机(裁决映射单源在 DraftOutcome::from_ffi_code)。
    finish_active_stroke();
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
      int r = glaspen2_ink_draft_stop();
      // 失败原因在后台线程取好再回主线程, 避免与下一次会话的写入竞争。
      const char *err = glaspen2_ink_draft_last_error();
      NSString *detail = (err && *err) ? [NSString stringWithUTF8String:err] : @"";
      dispatch_async(dispatch_get_main_queue(), ^{
        rec_feed(REC_SLOT_CHAT, rec_json(@{
                   @"type" : @"ChatDraftFinished",
                   @"code" : @(r),
                   @"detail" : detail ?: @"",
                 }));
      });
    });
    return;
  }
  if ([kind isEqualToString:@"SendChatStrokes"]) {
    int start = [effect[@"start_stroke"] intValue];
    int end = [effect[@"end_stroke"] intValue];
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
      int sent = glaspen2_chat_send_strokes(start, end);
      dispatch_async(dispatch_get_main_queue(), ^{
        if (sent >= 0) {
          rec_feed(REC_SLOT_CHAT, rec_json(@{
                     @"type" : @"ChatSendSucceeded",
                     @"strokes" : @(sent),
                   }));
        } else {
          rec_feed(REC_SLOT_CHAT, rec_simple_event(@"ChatSendFailed"));
        }
      });
    });
    return;
  }
  if ([kind isEqualToString:@"Discard"]) {
    // 取消路径(总开关关闭取消直发/强制收尾): 不编码/不发送, 无 OS 资源可释放。
    NSLog(@"[recording] 丢弃进行中的录制窗口(取消)");
    return;
  }
  NSLog(@"[recording] 未实现的录制 effect: %@", kind);
}

static void rec_exec_effects(int slot, NSArray *effects) {
  for (id effect in effects) {
    if ([effect isKindOfClass:[NSDictionary class]])
      rec_exec_effect(slot, effect);
  }
}

// 手写消息总开关变化(menu.m 调): 把总开关镜像喂给手写槽。关闭时的 effects
// 会静默取消直发(Discard)或优雅收尾草稿(StopChatDraft: 提交在写笔画 +
// 完整 stop + 裁决提示)。GIF 槽不受总开关影响。
static void gl_recording_chat_integration_changed(BOOL on) {
  rec_feed(REC_SLOT_CHAT, rec_json(@{
             @"type" : @"ChatIntegrationChanged",
             @"enabled" : @(on),
           }));
}

// 启动: 按当前总开关初始化两个槽位(必须在 g_chat_integration 恢复之后调用)。
static void gl_recording_init(void) {
  glaspen2_recording_init(REC_SLOT_GIF, 0);
  glaspen2_recording_init(REC_SLOT_CHAT, g_chat_integration ? 1 : 0);
}

// (重)装 event tap:include_scroll 决定滚轮事件是否进入 tap。
// S6-2: 掩码/创建/runloop 挂载已迁 Swift; 回调指针与 tap 存储经薄桥留在 ObjC。
static void event_tap_install(BOOL include_scroll) {
  [EventTapInstaller installWithScroll:include_scroll];
}

// 按当前画布模式重装 event tap(无限=含滚轮,翻页=不含)。
static void event_tap_reinstall(void) { event_tap_install(g_infinite_canvas); }

// S6-3-4b-2 拆分: 录制热键(⌘⌃R/2/3)处理, 返回 YES = 事件已消费。
// 本次只是把原键盘块里的录制分支纯移动到独立函数(行为不变);
// 下一步用 recording 事件 FFI 把这函数整体迁 Swift。
static BOOL handle_recording_hotkeys(CGEventType type, unsigned short kc,
                                     BOOL hasCmdCtrl) {
  // S6-3-4b-2c: 判定已迁 Swift(RecordingHotkeys); 本函数只转发。
  return [RecordingHotkeys handleWithType:type keyCode:kc hasCmdCtrl:hasCmdCtrl];
}

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

  // S6-3-3d: tablet proximity 进入/离开分支已迁 Swift EventRouter。


  // 滚轮缩放(S6-3-1: 已迁 Swift EventRouter; 这里不再处理)

  // S6-3-4: 键盘事件(录制键 + chord)已迁 Swift EventRouter。

  // S6-3-4: 停用涂鸦时的清理/放行已迁 Swift EventRouter。


  // Convert to NSEvent to check pen properties
  NSEvent *nsevent = [NSEvent eventWithCGEvent:event];
  if (!nsevent)
    return event;

  NSPointingDeviceType devType = [nsevent pointingDeviceType];
  NSInteger subtype = [nsevent subtype];
  NSEventType etype = [nsevent type];
  CGFloat pressure = [nsevent pressure];

  // S6-3-3d-2: 笔事件判定单源在 Swift(EventRouter.isPenForEvent:),
  // 这里保留 devType/subtype 供后续橡皮/日志判断使用。
  BOOL isPen = [EventRouter isPenForEvent:event];

  // ⌃⌥ + 左键拖动 = 临时抓手; 与镜头键位用同一套修饰键。
  // 正常鼠标左键(无修饰键)仍然原样放行给下层 App。
  // S6-3-3d: 相机临时抓手(开始/拖动/结束)已迁 Swift EventRouter。

  // S6-3-3d: 菜单栏点击的 tap 暂停/恢复已迁 Swift EventRouter。


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

  // S6-3-3d: 压感监视器更新已迁 Swift。
  if (isPen) {
    [EventRouter updatePressureMonitorWithEvent:event
                                         type:type
                                            x:px
                                            y:py
                                     pressure:pressure];
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

  // S6-3-3d: 笔事件分派(落笔/拖动/抬笔)已迁 Swift EventRouter。
  int32_t penRoute = [EventRouter handlePenEventOfType:type x:px y:py pressure:pressure
                                         timestamp:ts rawWidth:raw_w
                                           isEraser:(devType == NSEraserPointingDevice) ||
                                                    (g_prox_dev == NSEraserPointingDevice)];
  if (penRoute == 2) return NULL;

  perf_log_event("tick", elapsed_us(t0));
  return event;
}

/// 合成事件(虚拟笔)处理完一律吞掉: 落笔/拖动本来就是 return NULL, 但**悬停
/// 走的是放行路径** —— 不拦住的话虚拟笔的 hover 流会真的移动用户光标。
/// 测试工具绝不能抢用户的鼠标。
static CGEventRef event_tap_callback(CGEventTapProxy proxy, CGEventType type,
                                     CGEventRef event, void *refcon) {
  // S6-3-0: Swift 路由骨架先行; 返回 0 表示仍走 ObjC 内层(行为不变)。
  int32_t route = [EventRouter routeEventOfType:type event:event];
  if (route == 2) return NULL;
  if (route == 1) return [EventRouter isVirtualPenEvent:event] ? NULL : event;
  CGEventRef out = event_tap_callback_inner(proxy, type, event, refcon);
  return [EventRouter isVirtualPenEvent:event] ? NULL : out;
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
    // S2 双向互调探针: GLASPEN2_SWIFT_PROBE=1 时在启动早期跑一次
    // ObjC→Swift→ObjC(只打日志);不设环境变量时 getenv 一次即返回,
    // 正常启动行为零变化。定义在 src/macos/swift/InteropProbe.m。
    glaspen2_macos_swift_probe_maybe();

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
    // S3b-2b: 状态栏项创建改由 Swift StatusBarFactory 负责; ObjC 保留 fallback。
    g_statusItem = [StatusBarFactory createSquareItem];
    if (!g_statusItem) {
        g_statusItem = [[NSStatusBar systemStatusBar]
            statusItemWithLength:NSSquareStatusItemLength];
        [g_statusItem.button setTitle:@""];
    }

    g_menuHandler = [[GlaspenMenuHandler alloc] init];
    [NSApp setDelegate:g_menuHandler];

    g_menu = [[NSMenu alloc] init];
    [g_menu setDelegate:g_menuHandler];
    [g_menu setAutoenablesItems:NO];

    // Color items: inline color dot + name (attributed — see gl_color_item_text)
    // S3b-2b-2: 建项循环(建项/attributedTitle/target/tag)已迁 Swift
    // MenuBuilder; ObjC 只预构富文本标题(图标/文案单源仍在
    // MenuIconFactory + core i18n, selectColor:/selectWidth: selector 不变)。
    NSMutableArray<NSAttributedString *> *colorTitles =
        [NSMutableArray arrayWithCapacity:g_color_preset_count];
    for (int i = 0; i < g_color_preset_count; i++) {
      [colorTitles addObject:gl_color_item_text(i)];
    }
    [MenuBuilder addColorItemsToMenu:g_menu
                             handler:g_menuHandler
                               count:g_color_preset_count
                              titles:colorTitles];

    [g_menu addItem:[NSMenuItem separatorItem]];

    // Width items: inline line-thickness icon + name
    NSMutableArray<NSAttributedString *> *widthTitles =
        [NSMutableArray arrayWithCapacity:g_width_preset_count];
    for (int i = 0; i < g_width_preset_count; i++) {
      [widthTitles addObject:gl_width_item_text(i)];
    }
    [MenuBuilder addWidthItemsToMenu:g_menu
                             handler:g_menuHandler
                               count:g_width_preset_count
                              titles:widthTitles];

    // S3b-2b-3: 顶层其余项(保存×2/新建画布/导出▸/涂鸦开关/设置/更多▸/语言/
    // 退出)与两个子菜单的结构(顺序/分隔符/tag/selector/target/初始 state/
    // keyEquivalent)已迁 Swift MenuBuilder(契约见 menu_builder_full.swift 顶部);
    // ObjC 只提供本地化文案(L()/gl_text 单源)与两个动态初始状态。
    // 高频动作留在顶层; 低频导出/实验开关收进子菜单, 首屏不再铺满 30+ 行。
    NSDictionary<NSString *, NSString *> *menuTitles = @{
      @"saveWithBg" : L(@"保存(含背景)", @"Save (with bg)"),
      @"saveDrawing" : L(@"保存(涂鸦)", @"Save (drawing)"),
      @"newCanvas" : L(@"新建画布", @"New canvas"),
      @"export" : L(@"导出", @"Export"),
      @"settings" : L(@"设置...", @"Settings..."),
      @"more" : L(@"更多", @"More"),
      @"toggleDraw" : L(@"开启涂鸦", @"Enable Drawing"),
      @"language" : L(@"English", @"中文"),
      @"quit" : L(@"退出", @"Quit"),
      @"exportXournal" : L(@"保存笔记 (Xournal)", @"Save Notes (Xournal)"),
      @"exportInfinitePdf" : L(@"导出无限画布 PDF (分页)",
                               @"Export infinite canvas PDF (paged)"),
      @"exportInfiniteSvg" : L(@"导出无限画布 SVG (整幅)",
                               @"Export infinite canvas SVG (whole)"),
      @"moreLaunch" : L(@"开机自启", @"Launch at login"),
      @"moreGlass" : L(@"磨砂玻璃", @"Frosted Glass"),
      @"moreOutline" : L(@"笔迹描边", @"Stroke outline"),
      @"moreCanvasInfinite" : gl_text(@"canvas.mode_infinite"),
      @"moreCanvasMode" : gl_text(@"canvas.fixed_mode"),
    };
    BOOL launchAtLoginOn = glaspen2_is_launch_at_login() ? YES : NO;
    [MenuBuilder addTopLevelAndSubmenusToMenu:g_menu
                                     handler:g_menuHandler
                                      titles:menuTitles
                              launchAtLoginOn:launchAtLoginOn
                                     glassOn:g_glass_enabled];

    [g_statusItem setMenu:g_menu];

    NSScreen *screen = [NSScreen mainScreen];
    NSRect screenFrame = [screen frame];

    // Store screen dimensions for DB
    g_screen_w = (int)screenFrame.size.width;
    g_screen_h = (int)screenFrame.size.height;
    glaspen2_init_db(g_screen_w, g_screen_h);
    // 语言不再跟随系统: 读面板上次选择(缺省中文), 菜单在建好后再刷新文案
    {
      char *vlang = glaspen2_load_string_setting("language");
      if (vlang) {
        g_lang = (strcmp(vlang, "en") == 0) ? 1 : 0;
        glaspen2_free_c_string(vlang);
        update_menu_texts();
      }
    }
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

    // 可配置快捷键: 必须在 event_tap_reinstall 之前装载, 否则首帧仍是旧键位。
    gl_shortcuts_load();

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
      NSMenuItem *oi = [MenuLookup itemWithTag:667 inMenu:g_menu];
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
        if (dv >= 0 && dv <= 4) // 4 = 康奈尔
          g_grid_divider = dv;
        glaspen2_free_c_string(vdv);
      }
    }

    // 气泡捕获只在「气泡消息」tab 打开时运行(默认关): 关闭时笔事件钩子
    // 立即返回, 不占 CPU、不落库、不读历史。必须在 event tap 安装前装载。
    glaspen2_bubble_set_capture(
        glaspen2_load_bool_setting("show_bubble_tab") != 0 ? 1 : 0);

    // Restore canvas mode and load the matching independent store.
    // 翻页/无限两套存储互不影响:无限画布全局仅一个。
    g_infinite_canvas = glaspen2_load_bool_setting("infinite_canvas") != 0;
    g_minimap_enabled = glaspen2_load_bool_setting("minimap") != 0;
    // 飘渺画布模式(⌘⌃X): 与 Windows 共用 "ethereal" 键, toggle_canvas_mode 落盘。
    // 此处只恢复标志 + 同步菜单; 笔迹/玻璃的显隐在视图就绪后由
    // gl_ethereal_restore_now() 统一落位(下方 window ready 之后)。
    g_ethereal_canvas = glaspen2_load_bool_setting("ethereal") != 0;
    {
      NSMenuItem *ethRestoreItem = [MenuLookup itemWithTag:778 inMenu:g_menu];
      if (ethRestoreItem) {
        [ethRestoreItem setState:g_ethereal_canvas ? NSControlStateValueOn
                                                   : NSControlStateValueOff];
        [ethRestoreItem setTitle:g_ethereal_canvas
            ? gl_text(@"canvas.fixed_mode")
            : gl_text(@"canvas.ethereal_mode")];
      }
    }
    NSMenuItem *infiniteRestoreItem = [MenuLookup itemWithTag:668 inMenu:g_menu];
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
    [[MenuLookup itemWithTag:778 inMenu:g_menu]
        setTitle:gl_text(@"canvas.ethereal_mode")];

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
    // D8b: 两个录制状态机按当前总开关初始化(必须在 g_chat_integration 恢复后)
    gl_recording_init();
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

    // S5-1: 窗口创建/层级/透明/穿透/集合行为与光标已迁 Swift。
    g_window = [OverlayWindowFactory makeWindowWithFrame:screenFrame];
    g_blank_cursor = [OverlayWindowFactory makeBlankCursor];
    g_arrow_cursor = [OverlayWindowFactory makeArrowCursor];

    // Container view for glass + drawing layers
    // S5-4: 容器视图创建/wantsLayer 已迁 Swift。
    NSView *contentView = [OverlayWindowFactory makeContentViewWithFrame:screenFrame];

    // Frosted glass layer (behind drawing)
    // S5-2: 玻璃视图创建/材质/透明度已迁 Swift。
    g_glass_view = [OverlayWindowFactory makeGlassViewWithFrame:screenFrame
                                                        enabled:g_glass_enabled
                                                        opacity:g_glass_opacity];
    [contentView addSubview:g_glass_view];

    // Drawing view on top
    GlaspenDrawView *drawView =
        [[GlaspenDrawView alloc] initWithFrame:screenFrame];
    [drawView setWantsLayer:YES];
    CALayer *layer = [drawView layer];
    if (layer) {
      [OverlayWindowFactory configureClearLayer:layer]; // S5-2
    }
    [contentView addSubview:drawView];

    // S5-4: 挂载 + orderFront 已迁 Swift。
    [OverlayWindowFactory attachContent:contentView toWindow:g_window];

    g_draw_view = drawView;
    ensure_surface(drawView);

    // 视图就绪: 若恢复到飘渺画布模式, 此处统一落位(笔迹隐藏/玻璃跟随隐藏/
    // 压强计隐藏), 落位路径与 toggle 进入飘渺完全一致。
    if (g_ethereal_canvas) gl_ethereal_restore_now();


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

    // tap 健康看门狗: TCC 被撤销 / tap 被系统禁用 / 意外失效时, 用户看到
    // 的只是"软件突然不响应笔了"。每 2s 自检, 能恢复就恢复, 不能恢复就
    // 明确提示一次, 而不是静默装死。
    {
      static BOOL s_ax_warned = NO;
      dispatch_source_t wd = dispatch_source_create(
          DISPATCH_SOURCE_TYPE_TIMER, 0, 0, dispatch_get_main_queue());
      dispatch_source_set_timer(
          wd, dispatch_time(DISPATCH_TIME_NOW, 2 * NSEC_PER_SEC),
          2 * NSEC_PER_SEC, (uint64_t)0.2 * NSEC_PER_SEC);
      dispatch_source_set_event_handler(wd, ^{
        if (!AXIsProcessTrusted()) {
          if (!s_ax_warned) {
            s_ax_warned = YES;
            show_notification(L(@"辅助功能权限已失效, 涂鸦/热键暂停 — 请到系统设置重新勾选",
                                @"Accessibility permission lost — re-enable it in System Settings"));
          }
          return;
        }
        s_ax_warned = NO;
        if (!g_event_tap) {
          event_tap_reinstall();
        } else if (!CGEventTapIsEnabled(g_event_tap)) {
          CGEventTapEnable(g_event_tap, true);
        }
      });
      dispatch_resume(wd);
    }

    // 调试开关(环境变量):性能日志 / 虚拟笔, 默认都关
    perf_log_init_from_env();
    virtual_pen_maybe_start();
    flip_probe_maybe_start();

    [NSApp run];
  }
}

// S6-2: event tap 的 Swift 薄桥(回调/全局均已在前面定义)
CGEventTapCallBack glaspen2_event_tap_callback_ptr(void) {
  return event_tap_callback;
}

void glaspen2_event_tap_store(CFMachPortRef tap) {
  if (g_event_tap) {
    CFMachPortInvalidate(g_event_tap);
    CFRelease(g_event_tap);
    g_event_tap = NULL;
  }
  if (tap) {
    g_event_tap = (CFMachPortRef)CFRetain(tap);
  }
}

void glaspen2_event_tap_note_scroll(int on) {
  g_tap_has_scroll = on != 0;
}


// S6-3-1: 事件路由薄桥
NSUInteger glaspen2_router_camera_mods(void) {
  return gl_shortcut_modifier_mask_for(@"canvas_zoom_in");
}

int glaspen2_router_state(void) {
  return (g_enabled ? 1 : 0) | ((g_stroke_active ? 1 : 0) << 1) |
         ((g_infinite_canvas ? 1 : 0) << 2);
}

double glaspen2_router_draw_height(void) {
  return g_draw_view ? [g_draw_view bounds].size.height : 0.0;
}

void glaspen2_router_zoom_at(double factor, double x, double y) {
  canvas_zoom_at(factor, x, y);
}

// S6-3-2: 非笔 mouseMoved 隐藏光标所需的薄桥
int glaspen2_router_cursor_flags(void) {
  return (g_cursor_visible ? 1 : 0) | ((g_stroke_active ? 1 : 0) << 1) |
         ((g_ethereal_canvas ? 1 : 0) << 2) |
         ((g_has_tablet_proximity ? 1 : 0) << 3);
}

void glaspen2_router_cursor_invalidate(void) {
  dirty_include_point(g_cursor_x, g_cursor_y, 14.0);
}

void glaspen2_router_cursor_hide(void) { g_cursor_visible = NO; }
void glaspen2_router_auto_hide(void) { auto_hide_now(); }
void glaspen2_router_bubble_pen_gone(void) { glaspen2_bubble_pen_gone(); }
void glaspen2_router_flush(void) { flush_dirty_to_layer(); }

int64_t glaspen2_virtual_pen_user_data(void) {
  return (int64_t)kVirtualPenUserData;
}

// S6-3-3b: 笔光标更新的状态/动作薄桥
int glaspen2_router_pen_state(void) {
  return (g_strokes_visible ? 1 : 0) | ((g_stroke_active ? 1 : 0) << 1) |
         ((g_cursor_visible ? 1 : 0) << 2);
}

void glaspen2_router_pen_bubble(int back) {
  if (back) glaspen2_bubble_pen_back();
  else glaspen2_bubble_pen_activity();
}

void glaspen2_router_pen_auto_show(void) { auto_show_canvas(); }
void glaspen2_router_peek_cancel(void) { peek_cancel_timer(); }
double glaspen2_router_cursor_x(void) { return g_cursor_x; }
double glaspen2_router_cursor_y(void) { return g_cursor_y; }
void glaspen2_router_cursor_set(double x, double y) {
  g_cursor_x = x;
  g_cursor_y = y;
}
void glaspen2_router_cursor_show(int on) {
  if (on) g_cursor_visible = YES;
}
int glaspen2_router_pen_cursor_visible(void) { return g_cursor_visible ? 1 : 0; }
int glaspen2_router_cursor_flush_due(void) {
  if (!g_stroke_active && elapsed_us(g_last_cursor_flush) >= 16000) {
    g_last_cursor_flush = mach_absolute_time();
    return 1;
  }
  return 0;
}

// S6-3-3c: 笔书写系统光标隐藏/恢复薄桥
int glaspen2_router_pen_drawing(void) { return g_pen_drawing ? 1 : 0; }
void glaspen2_router_set_pen_drawing(int on) { g_pen_drawing = on != 0; }
void glaspen2_router_hide_system_cursor(void) {
  CGDisplayHideCursor(kCGDirectMainDisplay);
}
void glaspen2_router_show_system_cursor(void) {
  CGDisplayShowCursor(kCGDirectMainDisplay);
}

// S6-3-4: 停用涂鸦时非键盘事件的清理+放行薄桥
void glaspen2_router_disable_cleanup(void) {
  g_camera_mouse_dragging = NO;
  camera_mouse_cancel_pan();
  restore_system_cursor();
}

// S6-3-4b-2b: chord 门控的 perform_action 薄桥
void glaspen2_router_perform_action(NSString *action) { perform_action(action); }

// S6-3-4b-2c: 录制热键 Swift 判定所需薄桥
int glaspen2_router_rec_slot_gif(void) { return REC_SLOT_GIF; }
int glaspen2_router_rec_slot_chat(void) { return REC_SLOT_CHAT; }
int glaspen2_router_rec_mode(int slot) { return glaspen2_recording_mode(slot); }
int glaspen2_router_rec_finish_and_count(void) { return rec_finish_and_count(); }
int glaspen2_router_rec_feed(int slot, NSString *json) { return rec_feed(slot, json) ? 1 : 0; }
int glaspen2_router_chat_integration(void) { return g_chat_integration ? 1 : 0; }

// S6-3-3d: 点菜单栏时临时停 tap + 5s 兜底恢复(逻辑由 Swift EventRouter 触发)
void glaspen2_router_tap_menu_bar_interaction(void) {
  CGEventTapEnable(g_event_tap, false);
  dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 5 * NSEC_PER_SEC),
                 dispatch_get_main_queue(), ^{
                   if (g_event_tap) CGEventTapEnable(g_event_tap, true);
                 });
}

// S6-3-3d: 橡皮态切换薄桥
int glaspen2_router_eraser_mode(void) { return g_eraser_mode ? 1 : 0; }
void glaspen2_router_set_eraser_mode(int on) { g_eraser_mode = on != 0; }
void glaspen2_router_update_status_icon_state(void) { update_status_icon_state(); }

// S6-3-3d: 落笔入口薄桥
void glaspen2_router_bubble_auto_page_flip(void) { bubble_auto_page_flip(); }
void glaspen2_router_modeler_begin(double x, double y, double pressure, double timestamp) {
  glaspen2_modeler_begin(g_pen_r, g_pen_g, g_pen_b, canvas_input_x(x),
                         canvas_input_y(y), pressure, timestamp, g_width_scale);
}
void glaspen2_router_stroke_begin(void) { stroke_begin(); }
void glaspen2_router_raw_draw_dot(double x, double y, double rawWidth) {
  raw_draw_dot(x, y, rawWidth);
}
void glaspen2_router_pen_down_state(double x, double y, double rawWidth) {
  g_stroke_active = YES;
  g_cursor_visible = YES;
  g_cursor_x = x;
  g_cursor_y = y;
  g_raw_last_x = x;
  g_raw_last_y = y;
  g_raw_last_w = rawWidth;
  g_raw_has_last = YES;
  g_raw_path_len = 0;
}

// S6-3-3d: 拖动实时段薄桥
void glaspen2_router_modeler_move(double x, double y, double pressure, double timestamp) {
  glaspen2_modeler_move(canvas_input_x(x), canvas_input_y(y), pressure, timestamp,
                        g_width_scale);
}
void glaspen2_router_raw_draw_segment(double x, double y, double rawWidth) {
  raw_draw_segment(x, y, rawWidth);
}

// S6-3-3d: 抬笔薄桥
int glaspen2_router_stroke_active(void) { return g_stroke_active ? 1 : 0; }
void glaspen2_router_set_stroke_active(int on) { g_stroke_active = on != 0; }
void glaspen2_router_modeler_erase_finish(void) { glaspen2_modeler_erase_finish(); }
void glaspen2_router_modeler_end(double x, double y, double pressure, double timestamp) {
  glaspen2_modeler_end(canvas_input_x(x), canvas_input_y(y), pressure, timestamp,
                       g_width_scale);
}
void glaspen2_router_commit_to_strokes(void) {
  glaspen2_modeler_commit_to_strokes(g_pen_r, g_pen_g, g_pen_b);
}
void glaspen2_router_stroke_end(void) { stroke_end(); }
void glaspen2_router_apply_pending_color_width(void) {
  gl_settings_apply_pending_color_width();
}
void glaspen2_router_pen_up_state(void) {
  g_raw_has_last = NO;
  g_raw_has_prev = NO;
  g_raw_path_len = 0;
  g_cursor_visible = YES;
  dirty_include_point(g_cursor_x, g_cursor_y, 14.0);
  flush_dirty_to_layer();
}

// S6-3-3d: proximity 分支薄桥
void glaspen2_router_mark_tablet_proximity(void) { g_has_tablet_proximity = YES; }
void glaspen2_router_set_prox_device(int device) {
  g_prox_dev = (NSPointingDeviceType)device;
}
void glaspen2_router_bubble_pen_back(void) { glaspen2_bubble_pen_back(); }
int glaspen2_router_ethereal(void) { return g_ethereal_canvas ? 1 : 0; }

// S6-3-3d: 相机临时抓手薄桥
int glaspen2_router_camera_dragging(void) { return g_camera_mouse_dragging ? 1 : 0; }
double glaspen2_router_camera_step_zoom(void) { return glaspen2_camera_step_zoom(g_zoom); }
void glaspen2_router_camera_queue_pan(double dx, double dy) { camera_mouse_queue_pan(dx, dy); }
void glaspen2_router_camera_set_last(double x, double y) {
  g_camera_mouse_last = NSMakePoint(x, y);
}
double glaspen2_router_camera_last_x(void) { return g_camera_mouse_last.x; }
double glaspen2_router_camera_last_y(void) { return g_camera_mouse_last.y; }
void glaspen2_router_camera_flush(void) { camera_mouse_flush_pan(); }
void glaspen2_router_camera_clear_dragging(void) { g_camera_mouse_dragging = NO; }
void glaspen2_router_camera_start(double x, double y) {
  g_camera_mouse_dragging = YES;
  g_camera_mouse_last = NSMakePoint(x, y);
}
NSUInteger glaspen2_router_camera_pan_mods(void) {
  return gl_shortcut_modifier_mask_for(@"canvas_pan_up");
}

// S6-3-3d: 压感监视器薄桥
int glaspen2_router_pm_enabled(void) { return g_pressure_monitor ? 1 : 0; }
void glaspen2_router_pm_update(int inRange, int pressure, double x, double y,
                               int tipDown, NSString *evtype) {
  g_pm_in_range = inRange != 0;
  g_pm_pressure = pressure;
  g_pm_x = x;
  g_pm_y = y;
  g_pm_tip_down = tipDown != 0;
  g_pm_evtype = evtype;
  pm_update();
}

// S6-3-4: 键盘录制放行标志薄桥
int glaspen2_router_hotkey_recording(void) { return g_hotkey_recording ? 1 : 0; }
