// 晾衣架(macOS v1): 学 Tendedero 的模式 —— 屏幕顶部一根悬垂绳,
// 完结的气泡消息作为卡片用木夹挂上去。指针停在屏幕顶缘 → 绳子滑下;
// 移开 → 收起;卡片带木夹微幅摇摆;点击卡片 → 大图预览(再点关闭)。
//
// 与主覆盖层的关系:独立 borderless 透明窗口(kCGMaximumWindowLevel,
// 全屏大小), 内容图层整体平移实现滑落/收起。
// ⚠️ 窗口永远 setIgnoresMouseEvents:YES(纯像素, 不参与 WindowServer
// 的点击路由)—— v0 曾用 hitTest:nil 做局部穿透, 实测在最高层全屏窗上
// 会劫持整只鼠标(点击全面失灵)。交互全部改走 event tap:
// 悬停 = clothesline_hover_update(mouseMoved 喂入), 卡片点击/预览关闭 =
// clothesline_click(LeftMouseDown 喂入, 命中时返回 NULL 消费事件)。
//
// 数据: 卡片位图来自 core(line.rs, glaspen2_bubble_card_png → PNG →
// NSImage, minimap 同款后台串行队列);消息完结经 LINE_HOOK
// (glaspen2_bubble_line_changed)异步回主线程刷新;卡片列表
// glaspen2_bubble_cards_json(最近 kLineMaxCards 条, 旧→新)。
// 开关: 设置 bubbleClothesline(默认开), g_clothesline_enabled。
// 注意: hook 触发时 core 侧正持 bubbles 锁 —— 本文件所有取数都发生在
// dispatch_async 之后的独立时机, 严禁同步回调进 core。

#import <QuartzCore/QuartzCore.h>

// ── 常量(几何均以点计, 顶部原点) ──
static const CGFloat kLineStripH = 150.0;  // 滑下后内容条高度(绳+卡+摆幅)
static const CGFloat kLinePeekH = 3.0;     // 收起时露出的边缘厚度
static const CGFloat kLineCardH = 104.0;   // 卡片内容最大高(Tendedero 同款)
static const CGFloat kLineCardMaxW = 150.0;
static const CGFloat kLineSag = 14.0;      // 绳中点垂坠
static const CGFloat kLineRopeY = 116.0;   // 绳两端基线(内容条内, 距条顶)
static const int kLineMaxCards = 8;        // 线上最多挂几张(旧的移除)

static NSWindow *g_line_window = nil;      // 全屏透明窗(顶部条 + 预览场)
static CALayer *g_line_root = nil;         // 窗口层根(挂 container + preview)
static CALayer *g_line_container = nil;    // 滑落/收起动画载体
static CAShapeLayer *g_line_rope = nil;    // 悬垂绳
static NSMutableArray<NSDictionary *> *g_line_cards = nil; // {id, layer, pin, size}
static dispatch_queue_t g_line_queue = nil; // PNG 取图串行队列
static BOOL g_line_shown = NO;             // 当前是否滑下
static long long g_line_preview_id = 0;    // 预览中的消息 id(0 = 无)
static BOOL g_clothesline_enabled = YES;   // 设置 bubbleClothesline
static NSInteger g_line_refresh_gen = 0;   // 刷新代际(丢弃过期取图)

// ── 前向声明(C99 先声明后用) ──
static void line_ensure_window(void);
static void line_apply_slide(BOOL shown);
static void line_refresh_cards(void);
static void line_layout_cards(void);
static void line_show_preview(long long msg_id);
static void line_dismiss_preview(void);

BOOL clothesline_enabled(void) { return g_clothesline_enabled; }

void clothesline_set_enabled(BOOL on) {
  g_clothesline_enabled = on;
  dispatch_async(dispatch_get_main_queue(), ^{
    if (!on) {
      g_line_preview_id = 0;
      [g_line_window orderOut:nil];
      g_line_shown = NO;
    } else {
      line_ensure_window();
    }
  });
}

/// 飘渺模式/涂鸦总开关隐藏主覆盖层时, 晾衣架同隐(pm 窗口同款语义)。
void clothesline_set_hidden(BOOL hidden) {
  if (!g_line_window)
    return;
  dispatch_async(dispatch_get_main_queue(), ^{
    if (hidden || !g_clothesline_enabled) {
      g_line_preview_id = 0;
      [g_line_window orderOut:nil];
      g_line_shown = NO;
    } else {
      [g_line_window orderFront:nil];
    }
  });
}

// ── 绳上 x 位置的垂坠量(抛物线, 两端 0, 中点 kLineSag) ──
static CGFloat line_sag_at(CGFloat x, CGFloat w) {
  CGFloat t = (w > 0) ? (x / w) : 0.0;
  return kLineSag * (4.0 * t * (1.0 - t));
}

static CGFloat line_container_hidden_y(void) {
  return -(kLineStripH - kLinePeekH); // 收起: 只露边缘
}

/// 卡片位图(后台线程调用;返回 autoreleased NSImage 或 nil)。
static NSImage *line_fetch_image(long long msg_id, int max_edge) {
  int len = 0;
  unsigned char *png = glaspen2_bubble_card_png(msg_id, max_edge, &len);
  NSImage *im = nil;
  if (png && len > 0) {
    im = [[NSImage alloc] initWithData:[NSData dataWithBytes:png length:len]];
    glaspen2_free_rust_bytes(png, len);
  }
  return im;
}

/// 位图像素尺寸 → 展示尺寸: 高 ≤ kLineCardH / 宽 ≤ kLineCardMaxW,
/// 不放大超过 1.5 倍(与面板气泡一致)。
static CGSize line_card_size(CGSize px) {
  if (px.width < 1 || px.height < 1)
    return NSMakeSize(kLineCardMaxW, kLineCardH);
  CGFloat scale = MIN(kLineCardH / px.height, kLineCardMaxW / px.width);
  scale = MIN(scale, 1.5);
  return NSMakeSize(px.width * scale, px.height * scale);
}

/// 卡片在视图(底原点)坐标里的命中矩形(含 4pt 容差)。
static NSRect line_card_rect(NSDictionary *card, CGFloat viewH) {
  CALayer *layer = card[@"layer"];
  CGPoint lp = layer.position; // 卡顶中心(容器坐标, 顶原点)
  CGSize sz = [(NSValue *)card[@"size"] sizeValue];
  CGFloat vx = lp.x - sz.width / 2.0;
  CGFloat vy = viewH - (lp.y + sz.height); // 底原点: 卡片下缘
  return NSMakeRect(vx - 4, vy - 4, sz.width + 8, sz.height + 8);
}

/// 卡片摇摆(锚点在卡顶 = 木夹处, ±2.5° 自 reverse 循环, 相位随机)。
static void line_add_swing(CALayer *card) {
  CABasicAnimation *swing =
      [CABasicAnimation animationWithKeyPath:@"transform.rotation.z"];
  swing.fromValue = @(-2.5 * M_PI / 180.0);
  swing.toValue = @(2.5 * M_PI / 180.0);
  swing.duration = 1.4 + (arc4random_uniform(60) / 100.0);
  swing.autoreverses = YES;
  swing.repeatCount = HUGE_VALF;
  swing.timeOffset = arc4random_uniform(1000) / 1000.0;
  [card addAnimation:swing forKey:@"lineSwing"];
}

/// 追加一张卡: 木夹 + 卡图层(掉落动画: 从上方落到绳位)。
static void line_append_card(long long msg_id, NSInteger ts_ms, NSImage *im) {
  CGSize px = im.size;
  CGSize sz = line_card_size(px);

  CALayer *card = [CALayer layer];
  card.contents = im;
  card.bounds = NSMakeRect(0, 0, sz.width, sz.height);
  card.cornerRadius = 6.0;
  card.shadowColor = [NSColor blackColor].CGColor;
  card.shadowOpacity = 0.25;
  card.shadowRadius = 4.0;
  card.shadowOffset = CGSizeMake(0, 2);
  card.anchorPoint = CGPointMake(0.5, 0.0); // 悬挂点 = 卡顶中心
  card.zPosition = 2;

  // 木夹: 小圆角矩形, 盖在卡顶中央
  CALayer *pin = [CALayer layer];
  pin.backgroundColor = [NSColor colorWithWhite:0.35 alpha:0.95].CGColor;
  pin.cornerRadius = 2.0;
  pin.borderColor = [NSColor colorWithWhite:0.15 alpha:0.9].CGColor;
  pin.borderWidth = 0.5;
  pin.bounds = NSMakeRect(0, 0, 10, 12);
  pin.zPosition = 3;

  [g_line_container addSublayer:card];
  [g_line_container addSublayer:pin];
  [g_line_cards addObject:@{
    @"id" : @(msg_id),
    @"ts" : @(ts_ms),
    @"layer" : card,
    @"pin" : pin,
    @"size" : [NSValue valueWithSize:sz],
  }];
  line_layout_cards();

  // 掉落动画(anchorPoint 在顶, 平移 y 即可)
  CABasicAnimation *drop =
      [CABasicAnimation animationWithKeyPath:@"transform.translation.y"];
  drop.fromValue = @(-60.0);
  drop.toValue = @(0.0);
  drop.duration = 0.45;
  drop.timingFunction =
      [CAMediaTimingFunction functionWithName:kCAMediaTimingFunctionEaseOut];
  [card addAnimation:drop forKey:@"lineDrop"];
  line_add_swing(card);
}

/// 重排卡片: 从左往右按旧→新挂绳, 锚点 = 各自 x 处的绳垂坠点。
static void line_layout_cards(void) {
  if (!g_line_container || !g_line_window)
    return;
  CGFloat w = g_line_window.frame.size.width;
  CGFloat x = 18.0;
  for (NSDictionary *card in g_line_cards) {
    CALayer *layer = card[@"layer"];
    CGSize sz = [(NSValue *)card[@"size"] sizeValue];
    CGFloat sag = line_sag_at(x + sz.width / 2.0, w);
    CGPoint pos = CGPointMake(x + sz.width / 2.0, kLineRopeY + sag);
    [CATransaction begin];
    [CATransaction setDisableActions:YES];
    layer.position = pos;
    CALayer *pin = card[@"pin"];
    pin.position = CGPointMake(pos.x, pos.y + 3.0);
    [CATransaction commit];
    x += sz.width + 10.0;
  }
}

/// 移除卡片图层(带淡出)。
static void line_remove_card(NSDictionary *card) {
  CALayer *layer = card[@"layer"];
  CALayer *pin = card[@"pin"];
  [CATransaction begin];
  [CATransaction setCompletionBlock:^{
    [layer removeFromSuperlayer];
    [pin removeFromSuperlayer];
  }];
  layer.opacity = 0.0;
  pin.opacity = 0.0;
  [CATransaction commit];
}

/// 依据 core 的挂卡列表重建(diff 增删;新增卡逐张后台取图再落卡)。
static void line_refresh_cards(void) {
  if (!g_line_window || !g_clothesline_enabled)
    return;
  NSInteger gen = ++g_line_refresh_gen;
  char *json = glaspen2_bubble_cards_json(kLineMaxCards);
  if (!json)
    return;
  NSData *data = [NSData dataWithBytes:json length:strlen(json)];
  glaspen2_macos_free_c_string(json);
  NSDictionary *v = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
  NSArray *cards = v[@"cards"];
  if (![cards isKindOfClass:[NSArray class]])
    return;

  NSMutableArray<NSNumber *> *want = [NSMutableArray array];
  for (NSDictionary *c in cards) {
    long long idv = [c[@"id"] longLongValue];
    if (idv > 0)
      [want addObject:@(idv)];
  }
  // 移除消失的
  NSMutableArray<NSDictionary *> *gone = [NSMutableArray array];
  for (NSDictionary *card in g_line_cards) {
    if (![want containsObject:card[@"id"]])
      [gone addObject:card];
  }
  for (NSDictionary *card in gone) {
    line_remove_card(card);
    [g_line_cards removeObject:card];
  }
  line_layout_cards();

  // 第一个缺失的新卡 → 后台取图, 回主线程落卡后继续补下一张
  long long missing = 0;
  NSInteger ts = 0;
  for (NSNumber *idv in want) {
    BOOL exists = NO;
    for (NSDictionary *card in g_line_cards) {
      if ([card[@"id"] isEqualToNumber:idv]) {
        exists = YES;
        break;
      }
    }
    if (!exists) {
      missing = [idv longLongValue];
      for (NSDictionary *c in cards) {
        if ([c[@"id"] longLongValue] == missing) {
          ts = [c[@"ts_ms"] integerValue];
          break;
        }
      }
      break;
    }
  }
  if (missing == 0)
    return;
  dispatch_async(g_line_queue, ^{
    NSImage *im = line_fetch_image(missing, 320);
    dispatch_async(dispatch_get_main_queue(), ^{
      if (gen != g_line_refresh_gen || !g_clothesline_enabled)
        return; // 过期取图(列表又变了), 下一轮再取
      if (im)
        line_append_card(missing, ts, im);
      line_refresh_cards(); // 补下一张(没有缺失时直接返回)
    });
  });
}

// ── 滑落 / 收起 ──

static void line_apply_slide(BOOL shown) {
  if (!g_line_container)
    return;
  CGFloat toY = shown ? 0.0 : line_container_hidden_y();
  CABasicAnimation *slide =
      [CABasicAnimation animationWithKeyPath:@"transform.translation.y"];
  slide.fromValue = @(g_line_container.transform.m32);
  slide.toValue = @(toY);
  slide.duration = 0.35;
  slide.timingFunction =
      [CAMediaTimingFunction functionWithName:kCAMediaTimingFunctionEaseOut];
  [g_line_container addAnimation:slide forKey:@"lineSlide"];
  [CATransaction begin];
  [CATransaction setDisableActions:YES];
  g_line_container.transform = CATransform3DMakeTranslation(0, toY, 0);
  [CATransaction commit];
}

/// event tap 悬停检测(tap 回调在主 runloop): 指针停在屏幕顶缘 → 滑下;
/// 离开内容条(含缓冲) → 收起。只在状态变化时动画。
void clothesline_hover_update(CGFloat top_y) {
  if (!g_line_window || !g_clothesline_enabled)
    return;
  BOOL wantShown;
  if (g_line_preview_id != 0)
    wantShown = YES; // 预览中保持展开
  else if (g_line_shown)
    wantShown = top_y < kLineStripH + 30.0; // 展开时留缓冲带防抖
  else
    wantShown = top_y < 24.0;
  if (wantShown == g_line_shown)
    return;
  g_line_shown = wantShown;
  line_apply_slide(wantShown);
}

/// event tap 点击路由(tap 回调在主 runloop;坐标 = CG 全局, 顶部原点):
/// 预览打开 → 点哪都关;线展开 → 命中卡片 = 开预览。命中返回 YES(tap 消费
/// 该事件), 未命中 NO = 事件原样放行(涂鸦/下层应用不受任何影响)。
BOOL clothesline_click(CGFloat gx, CGFloat gy) {
  if (!g_line_window || !g_clothesline_enabled)
    return NO;
  if (g_line_preview_id != 0) {
    line_dismiss_preview();
    return YES; // 预览中点哪都关, 消费掉防止误触下层
  }
  if (!g_line_shown)
    return NO;
  CGFloat viewH = g_line_window.frame.size.height;
  NSPoint p = CGPointMake(gx, viewH - gy); // CG 顶原点 → 视图底原点
  for (NSDictionary *card in g_line_cards) {
    if (NSPointInRect(p, line_card_rect(card, viewH))) {
      line_show_preview([card[@"id"] longLongValue]);
      return YES;
    }
  }
  return NO;
}

// ── 预览 ──

static void line_show_preview(long long msg_id) {
  if (!g_line_window || g_line_preview_id != 0)
    return;
  g_line_preview_id = msg_id;
  dispatch_async(g_line_queue, ^{
    NSImage *im = line_fetch_image(msg_id, 1100);
    dispatch_async(dispatch_get_main_queue(), ^{
      if (!im || g_line_preview_id != msg_id)
        return;
      CALayer *backdrop = [CALayer layer];
      backdrop.name = @"linePreview";
      backdrop.frame = g_line_root.bounds;
      backdrop.backgroundColor =
          [NSColor colorWithWhite:0.0 alpha:0.45].CGColor;
      backdrop.zPosition = 100;

      CALayer *big = [CALayer layer];
      big.name = @"linePreviewInk";
      big.contents = im;
      big.contentsGravity = kCAGravityResizeAspect;
      CGSize px = im.size;
      CGFloat scale = MIN(MIN(g_line_root.bounds.size.width * 0.72 / px.width,
                              g_line_root.bounds.size.height * 0.72 / px.height),
                          1.5);
      big.bounds = NSMakeRect(0, 0, px.width * scale, px.height * scale);
      big.position = CGPointMake(g_line_root.bounds.size.width / 2.0,
                                 g_line_root.bounds.size.height / 2.0);
      big.zPosition = 101;
      big.cornerRadius = 10.0;
      big.shadowColor = [NSColor blackColor].CGColor;
      big.shadowOpacity = 0.5;
      big.shadowRadius = 18.0;
      [backdrop addSublayer:big];
      [g_line_root addSublayer:backdrop];

      CABasicAnimation *fade =
          [CABasicAnimation animationWithKeyPath:@"opacity"];
      fade.fromValue = @0.0;
      fade.toValue = @1.0;
      fade.duration = 0.18;
      [backdrop addAnimation:fade forKey:@"lineFade"];
    });
  });
}

static void line_dismiss_preview(void) {
  if (g_line_preview_id == 0)
    return;
  g_line_preview_id = 0;
  CALayer *backdrop = nil;
  for (CALayer *sub in g_line_root.sublayers) {
    if ([sub.name isEqualToString:@"linePreview"]) {
      backdrop = sub;
      break;
    }
  }
  if (!backdrop)
    return;
  [CATransaction begin];
  [CATransaction setCompletionBlock:^{
    [backdrop removeFromSuperlayer];
  }];
  backdrop.opacity = 0.0;
  [CATransaction commit];
}

// ── 窗口与视图 ──

static void line_ensure_window(void) {
  if (g_line_window)
    return;
  NSRect screenFrame = [NSScreen mainScreen].frame;
  g_line_cards = [NSMutableArray array];
  static dispatch_once_t once;
  dispatch_once(&once, ^{
    g_line_queue = dispatch_queue_create("glaspen2.line", DISPATCH_QUEUE_SERIAL);
    dispatch_set_target_queue(g_line_queue,
                              dispatch_get_global_queue(QOS_CLASS_UTILITY, 0));
  });

  g_line_window = [[NSWindow alloc]
      initWithContentRect:screenFrame
                styleMask:NSWindowStyleMaskBorderless
                  backing:NSBackingStoreBuffered
                    defer:NO];
  [g_line_window setLevel:kCGMaximumWindowLevel];
  [g_line_window setOpaque:NO];
  [g_line_window setBackgroundColor:[NSColor clearColor]];
  // 纯像素窗口: 永不参与点击路由(v0 的 hitTest 局部穿透实测会劫持整只
  // 鼠标)。卡片点击/预览关闭由 event tap 经 clothesline_click 驱动。
  [g_line_window setIgnoresMouseEvents:YES];
  [g_line_window setCollectionBehavior:NSWindowCollectionBehaviorCanJoinAllSpaces |
                                       NSWindowCollectionBehaviorStationary];
  [g_line_window setReleasedWhenClosed:NO];

  NSView *view = [[NSView alloc] initWithFrame:screenFrame];
  [view setWantsLayer:YES];
  g_line_root = [view layer];
  g_line_root.backgroundColor = [NSColor clearColor].CGColor;

  g_line_container = [CALayer layer];
  [g_line_root addSublayer:g_line_container];

  // 悬垂绳: 两端锚在内容条上部, 中点垂坠
  g_line_rope = [CAShapeLayer layer];
  CGFloat w = screenFrame.size.width;
  CGMutablePathRef path = CGPathCreateMutable();
  CGPathMoveToPoint(path, NULL, 0, kLineRopeY);
  CGPathAddQuadCurveToPoint(path, NULL, w / 2.0, kLineRopeY + kLineSag * 2.0,
                            w, kLineRopeY);
  g_line_rope.path = path;
  CGPathRelease(path);
  g_line_rope.strokeColor = [NSColor colorWithWhite:0.30 alpha:0.85].CGColor;
  g_line_rope.fillColor = NULL;
  g_line_rope.lineWidth = 2.0;
  g_line_rope.lineCap = kCALineCapRound;
  [g_line_container addSublayer:g_line_rope];

  // 初始收起(只露边缘)
  g_line_container.transform =
      CATransform3DMakeTranslation(0, line_container_hidden_y(), 0);

  [g_line_window setContentView:view];
  [g_line_window orderFront:nil];
}

// ── 对外入口 ──

/// 消息完结钩子(core LINE_HOOK): 必须异步 —— core 侧此刻持锁。
void glaspen2_bubble_line_changed(void) {
  dispatch_async(dispatch_get_main_queue(), ^{
    line_refresh_cards();
  });
}

/// 启动: 开关开启时建窗(收起状态挂最前)。
void clothesline_startup(void) {
  if (!g_clothesline_enabled)
    return;
  dispatch_async(dispatch_get_main_queue(), ^{
    line_ensure_window();
  });
}
