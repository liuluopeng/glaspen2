// 鼠标 ⌃⌥ + 左键拖动镜头(临时抓手)状态; 由 event tap 维护。
static BOOL g_camera_mouse_dragging = NO;
static NSPoint g_camera_mouse_last = {0, 0};
// 鼠标拖动平移是高频事件; 累积位移, 每 ~16ms 只重建一次 surface.
static void camera_mouse_cancel_pan(void);

static void pageview_frame_draw(void);
static void rebuild_surface_from_strokes(void) { [PageViewOps rebuildSurface]; }

// 时光隧道专用半分辨率表面: 卡片快照本来就是 half-res, 直接在 half-res
// 上合成再放大上屏, 填充量降到 1/4, 是 4K 下把 40fps 拉到 60+ 的关键。
static cairo_surface_t *s_tun_surface = NULL;
// S7: 隧道 surface 缓存/绘制已迁 Swift(OverlayTunnelRenderer)。

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
// S7: 网格线段绘制已迁 Swift(OverlayGridRenderer)。
// core 网格 FFI JSON → 字典;失败/error JSON 只记一次日志(避免每帧刷屏)并回 nil。
// 网格 + 分栏参考线:门控与视图←画布变换保持旧 drawRect 逐字一致。
// clipRect = 本次 drawRect 的脏矩形(非翻转视图坐标, y 向上), 只向 core 要相交的
// 线(旧实现同样的省遍历);视图尺寸 = bounds(macOS 窗口 = 屏幕 frame)。
// --- Drawing view ---

@interface GlaspenDrawView : NSView
@end

@implementation GlaspenDrawView

- (BOOL)acceptsFirstResponder {
  return YES;
}

- (void)drawRect:(NSRect)rect { [OverlayDrawing drawInView:self rect:rect]; }

@end

// 应用无限画布开关(菜单与 Flutter 设置面板共用的唯一入口)。
// 两种模式各自独立存储:翻页模式用 screens/strokes,无限画布用
// infinite_strokes(全局仅一个画布)。切换时冲刷当前笔画、切存储、载入对应笔迹。
// 模式本身持久化在 user_settings(结构性的模式,与描边这类渲染设置不同)。
static void apply_infinite_canvas(BOOL on, BOOL notify) { [CanvasMode applyInfiniteCanvasOn:on notify:notify]; }

// 应用描边开关(菜单与 Flutter 设置面板共用的唯一入口)。
// S7ac: 逻辑已迁 Swift(MenuHandler.setOutlineEnabled)。
// ── 无限画布:镜头平移(全局唯一画布) ──

// 把当前镜头变换应用到渲染 + 节流持久化(0.5s 一次)
// 键盘平移镜头(滚轮缩放走 canvas_zoom_at; 书写中忽略)。
// D5b: 数学(pan -= d)单源在 core Camera::pan_by, 经 FFI 拿回三值;
// 门控/重绘/0.5s 持久化节流/飘渺 peek 仍在本文件(adapter)。
/// 鼠标拖动平移: 高频事件只累积位移, 每帧最多真正重建一次 surface。


/// 模式切换/停用涂鸦时调用: 丢弃未应用的鼠标拖动位移。

// 缩放结果统一处理(鼠标锚点/键盘中心共用): 100% 上限提示(1.5s 节流)、
// 无变化早退、写回 + 应用变换/peek。
// macOS 没有 5% 下限提示(只有 Windows 有, 见 camera.rs 模块文档差异 2);
// 命中上限的判定用 core 的 hit_max(钳制前的请求越过 100%), 因此到达 100%
// 后继续放大仍会提示, 不会静默。
// 缩放镜头(滚轮):以视口点 (vx, vy) 为锚,zoom ∈ (0.05, 1.0]。
// 锚点的画布坐标在缩放前后保持同一屏幕位置(view = (canvas−pan)×zoom);
// 数学(锚点不变/限幅/hit 标记)单源在 core Camera::zoom_at。
// 键盘缩放 = 以视口中心为锚(数学单源 core Camera::zoom_at_center)。
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

// core minimap 布局 FFI JSON → 字典;失败/error JSON 只记一次日志(避免每帧
// 刷屏)并回 nil。
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

static void tunnel_free_cards(void) { [TunnelLifecycle freeCards]; }

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
  NSArray *r = [TunnelGeometry placeWithZC:zc
                                      span:TUNNEL_SPAN
                                     front:TUNNEL_FRONT
                                         k:glaspen2_router_tunnel_k()
                                    surfW:(double)glaspen2_router_tunnel_surf_w()
                                    surfH:(double)glaspen2_router_tunnel_surf_h()
                                    travel:glaspen2_router_tunnel_travel()];
  if (!r || r.count != 5) return NO;
  *out_cx = [r[0] doubleValue];
  *out_cy = [r[1] doubleValue];
  *out_w = [r[2] doubleValue];
  *out_h = [r[3] doubleValue];
  *out_alpha = [r[4] doubleValue];
  return YES;
}

static void tunnel_draw_card(cairo_t *cr, int slot, double cx, double cy,
                             double w, double h, double alpha, double zc) {
  cairo_surface_t *surf = s_tun_card[slot];
  if (s_tun_card_mip[slot] && w < cairo_image_surface_get_width(surf) * 0.70) {
    surf = s_tun_card_mip[slot];
  }
  if (!surf) return;
  [TunnelCardRenderer drawWithCR:(void *)cr surf:(void *)surf
                              nw:cairo_image_surface_get_width(surf)
                              nh:cairo_image_surface_get_height(surf)
                              cx:cx cy:cy w:w h:h alpha:alpha zc:zc
                            surfW:s_tun_surf_w surfH:s_tun_surf_h
                            thick:s_tun_thick paneA:s_tun_pane_a];
}

// 一帧:相机沿页距推进(缓入缓出), 卡片按隧道坐标摆位;
// 深处的先画、近处的后画 —— 层叠顺序即页序。
static void tunnel_frame(double p) {
  if (!s_tun_surface || s_tun_cards == 0) return;
  s_tun_frames++;
  [TunnelFrameRenderer renderWithP:p state:glaspen2_router_tunnel_state()];
}

static void tunnel_finish(void) { [TunnelLifecycle finish]; }

// 造一张卡片:某页的快照渲染到独立缓冲(与画布一致的 scale-to-fit 变换);
// 当前页直接拷贝玻璃表面, 保证第 0 帧无跳变。cache_slot = 预热缓存里的序号。
static BOOL tunnel_make_card(int slot, int cache_slot, long screen_id,
                             int is_current) {
  NSDictionary *r = [TunnelCardFactory makeCardWithIsCurrent:(is_current ? YES : NO)
                                                       depth:s_tun_depth[slot]
                                                   cacheSlot:cache_slot
                                                    screenId:screen_id];
  if (!r) return NO;
  s_tun_card[slot] = (cairo_surface_t *)[r[@"main"] pointerValue];
  s_tun_card_mip[slot] = (cairo_surface_t *)[r[@"mip"] pointerValue];
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
static BOOL bubble_auto_page_flip(void) { return [PageViewOps bubbleAutoPageFlip] ? YES : NO; }

// 载入唯一的无限画布镜头 + 应用到渲染
// 翻页模式:镜头恒为原点 + 100%(HOME 常量单源在 core Camera::HOME)
// --- CGEventTap callback ---

// Performance logging (set g_perf_log=YES to enable)


// GLASPEN2_PERF_LOG=1 打开性能日志(写 ~/Library/Logs/glaspen2/perf.log)
// Execute a hotkey action by key code. Returns YES if handled.
// Used by the physical shortcut (event tap) and the settings-panel buttons.
static BOOL perform_hotkey(unsigned short kc) { return [HotkeyDispatch performHotkey:kc] ? YES : NO; }

// 按动作 id 执行(事件 tap 的可配置键位表命中后调用)。旧 perform_hotkey
// 仍保留给设置面板按钮/诊断钩子; 这里用固定的 keyCode 当作 action 选择器,
// 与用户实际按下的键无关 —— 键位匹配在菜单层按 chord 完成。
//
// D3b: action id → 语义的唯一来源是 core `action_effect::effect_for_action`
// (经 FFI `glaspen2_action_effect_json` 下发 {"kind":"<变体名>"})。本函数只按
// kind 字符串分派 macOS 副作用(ObjC 无字符串 switch), 不再自己比较 action id;
// 每个分支的代码/顺序/门控/文案与旧 action 字符串链逐行一致(镜头方向符号、
// 退出路径、未知动作返回 NO 都保持原样)。
static BOOL perform_action(NSString *action) { return [ActionDispatch performAction:action ?: @""] ? YES : NO; }

// ── Quick GIF recording (hold Cmd+Ctrl+R, doodle, release to copy) ──

// Write the GIF bytes to a temp .gif file and put that file URL on the
// pasteboard so it can be pasted as an animation in apps that accept files.
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
  [EventRouter finishActiveStroke];
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


// 喂一个事件并执行返回的 effects(在调用线程 = 主 runloop 上同步执行)。
// 返回事件是否产生了 effects; key-up 分支据此决定是否消费按键。
static BOOL rec_feed(int slot, NSString *event_json) {
  return [RecordingEffects feedSlot:slot json:event_json ?: @""] ? YES : NO;
}

// S6/S7: rec_start/rec_release 已并入 Swift RecordingHotkeys(事件 JSON 由 Swift 生成)。

// GIF 编码结果回主线程后待写剪贴板的字节(Clipboard effect 消费)。

// HUD 语义 → 平台文案: 文案单源在 core::i18n(D10b, 按稳定 key 查),
// 只有 {keys} 的键名差异(⌘⌃3/⌘⌃2)由本函数作为参数传入。
// 按状态机下发的顺序执行单个 effect。
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
/// 合成事件(虚拟笔)处理完一律吞掉: 落笔/拖动本来就是 return NULL, 但**悬停
/// 走的是放行路径** —— 不拦住的话虚拟笔的 hover 流会真的移动用户光标。
/// 测试工具绝不能抢用户的鼠标。
static CGEventRef event_tap_callback(CGEventTapProxy proxy, CGEventType type,
                                     CGEventRef event, void *refcon) {
  // S6: 事件路由全部在 Swift; 这里只按结果放行/吞掉。
  (void)proxy;
  (void)refcon;
  int32_t route = [EventRouter routeEventOfType:type event:event];
  CGEventRef out = (route == 2) ? NULL : event;
  return [EventRouter isVirtualPenEvent:event] ? NULL : out;
}

// Call this at app exit to dump stats
// --- App ---

// ---------------------------------------------------------------------------
// 虚拟笔(GLASPEN2_VIRTUAL_PEN):合成笔事件驱动完整热路径,用于性能剖析
// ---------------------------------------------------------------------------
// 合成 tablet 点事件(含压力)投递到 HID 事件口, 与真笔走同一条事件水龙头,
// 因此覆盖 模型器 → cairo → CA 上屏 全链路。GLASPEN2_VIRTUAL_PEN=1 启用:
// 每 12 秒画 3 笔(每笔 2 秒 @ 200Hz, 屏幕上会真的出现笔迹)。
// 配套 GLASPEN2_DB_PATH 指向临时库即可不碰真实数据。
// 绘制处于关闭状态(g_enabled=NO)时暂停: 否则合成事件会漏给前台应用当鼠标。
// 每次载入某页(翻页/启动/显示变化/新建复用)后调用: 依据该页几何
// 设置页视图变换。页 = 某时刻玻璃的几何快照; 同几何 = 恒等(纯玻璃)。
static void pageview_update(void) { [PageViewOps update]; }

// 诊断钩子: GLASPEN2_FLIP_EVERY=<秒> 周期性触发翻页(走 perform_hotkey
// 与真实热键同一条路)。第一跳先建第二页, 之后往返翻页供连拍/录屏实验。
// 颜色/粗细预设表从 glaspen-core presets 填充(RGB 单源;名字是本平台
// UI 字符串)。须在任何读表代码(建菜单/恢复设置)之前调用。
// 单实例守卫: 两个实例 = 两套全屏窗口叠加显示 + 同一全局热键同时翻
// 两套页, 表现为"笔迹重复/翻页只翻了一部分"。flock 随进程生死自动
// 释放, 崩溃也不会留下死锁。GLASPEN2_ALLOW_MULTI=1 可并行调试。
// 返回锁 fd(保持打开, 不 close); 已有实例时返回 -1。
// 旧版实例(没有 flock 逻辑的安装版)不会去抢锁 —— 用运行应用列表兜底:
// 排除自己之外还有叫 glaspen2 的进程就算已运行。自动更新的 --updater
// 帮手与主程序同名, 更新窗口期内手动启动会误报, 属可接受的罕见边角。
void glaspen2_run(void) {
  @autoreleasepool {
    // S2 双向互调探针: GLASPEN2_SWIFT_PROBE=1 时在启动早期跑一次
    // ObjC→Swift→ObjC(只打日志);不设环境变量时 getenv 一次即返回,
    // 正常启动行为零变化。定义在 src/macos/swift/InteropProbe.m。
    glaspen2_macos_swift_probe_maybe();

    // 单实例守卫先于一切初始化: 第二个实例直接弹窗退出
    int lock_fd = [InstanceLock acquire];
    if (lock_fd == -1 || [AppInstanceGuard anotherInstanceRunning]) {
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
    [PenPresets loadPresets];

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

    g_menu = [MainMenuBuilder buildWithHandler:g_menuHandler
                                             statusItem:g_statusItem
                                         launchAtLoginOn:(glaspen2_is_launch_at_login() ? YES : NO)
                                                glassOn:g_glass_enabled];

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
    glaspen2_router_update_status_icon_state();
    update_menu_checkmarks();

    // 可配置快捷键: 必须在 event_tap_reinstall 之前装载, 否则首帧仍是旧键位。
    gl_shortcuts_load();

    // Restore glass settings (opacity stored as millipercent, enabled as bool)
    int glass_milli = glaspen2_load_bool_setting("glass_alpha");
    if (glass_milli > 0)
      g_glass_opacity = glass_milli / 1000.0;
    g_glass_enabled = glaspen2_load_bool_setting("glass_enabled") != 0;
    [MenuHandler applyGlass];

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
      [CanvasCamera loadInfinite];
    } else {
      [CanvasCamera resetLens];
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
    glaspen2_router_update_status_icon_state();

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
    // Drawing view on top
    GlaspenDrawView *drawView =
        [[GlaspenDrawView alloc] initWithFrame:screenFrame];

    // 组装(玻璃在下/绘制视图在上/layer 透明/挂载+orderFront)已迁 Swift。
    [OverlayWindowFactory assembleContent:contentView
                                  glass:g_glass_view
                               drawView:drawView
                               inWindow:g_window];

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
    [PerfLog initFromEnv];
    [VirtualPen maybeStart];
    [FlipProbe maybeStart];

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
  return [ShortcutMask maskForAction:@"canvas_zoom_in"];
}

int glaspen2_router_state(void) {
  return (g_enabled ? 1 : 0) | ((g_stroke_active ? 1 : 0) << 1) |
         ((g_infinite_canvas ? 1 : 0) << 2);
}

double glaspen2_router_draw_height(void) {
  return g_draw_view ? [g_draw_view bounds].size.height : 0.0;
}

void glaspen2_router_zoom_at(double factor, double x, double y) {
  [CanvasCamera zoomAtFactor:factor vx:x vy:y];
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
  if (!g_stroke_active && [PerfLog elapsedUsSince:g_last_cursor_flush] >= 16000) {
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
  [CameraMousePan cancel];
  [EventRouter restoreSystemCursor];
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
void glaspen2_router_update_status_icon_state(void) {
  [StatusIconUpdater applyToStatusItem:g_statusItem eraser:g_eraser_mode
                               enabled:g_enabled ethereal:g_ethereal_canvas
                              infinite:g_infinite_canvas widthScale:g_width_scale
                                  penRed:g_pen_r penGreen:g_pen_g penBlue:g_pen_b];
}

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
void glaspen2_router_camera_queue_pan(double dx, double dy) { [CameraMousePan queueDX:dx dy:dy]; }
void glaspen2_router_camera_set_last(double x, double y) {
  g_camera_mouse_last = NSMakePoint(x, y);
}
double glaspen2_router_camera_last_x(void) { return g_camera_mouse_last.x; }
double glaspen2_router_camera_last_y(void) { return g_camera_mouse_last.y; }
void glaspen2_router_camera_flush(void) { [CameraMousePan flush]; }
void glaspen2_router_camera_clear_dragging(void) { g_camera_mouse_dragging = NO; }
void glaspen2_router_camera_start(double x, double y) {
  g_camera_mouse_dragging = YES;
  g_camera_mouse_last = NSMakePoint(x, y);
}
NSUInteger glaspen2_router_camera_pan_mods(void) {
  return [ShortcutMask maskForAction:@"canvas_pan_up"];
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

// S6-3-3d: callback 内层收尾薄桥
void glaspen2_router_reenable_tap(void) {
  if (g_event_tap) CGEventTapEnable(g_event_tap, true);
}
double glaspen2_router_raw_width(double pressure) {
  return glaspen2_pressure_raw_width(pressure, g_width_scale) * g_zoom;
}
int glaspen2_router_prox_is_eraser(void) {
  return g_prox_dev == NSEraserPointingDevice ? 1 : 0;
}
int glaspen2_router_strokes_hidden(void) { return g_strokes_visible ? 0 : 1; }

// 页信息 HUD: 语言代码薄桥(Swift 调 core::pageinfo)
const char *glaspen2_router_lang_code(void) { return gl_lang_code(); }

// S7: finish_active_stroke 迁 Swift 所需薄桥
double glaspen2_router_last_raw_x(void) { return g_raw_last_x; }
double glaspen2_router_last_raw_y(void) { return g_raw_last_y; }
void glaspen2_router_clear_raw_state(void) {
  g_raw_has_last = NO;
  g_raw_has_prev = NO;
  g_raw_path_len = 0;
}

// S7: 覆盖层窗口引用薄桥(供 Swift 取截图目标屏)
id glaspen2_router_main_window(void) { return g_window; }

// S7: 退出前 perf 汇总薄桥(perf_log_summary 在本文件)
void glaspen2_router_perf_summary(void) { [PerfLog summary]; }

// S7m: 网格层状态快照桥(供 Swift OverlayGridRenderer 读)
NSDictionary *glaspen2_router_grid_state(void) {
  return @{
    @"showGrid" : @(g_show_grid ? YES : NO),
    @"divider" : @((int)g_grid_divider),
    @"tunActive" : @(s_tun_active ? YES : NO),
    @"strokesVisible" : @(g_strokes_visible ? YES : NO),
    @"gridFollow" : @(g_grid_follow_strokes ? YES : NO),
    @"infinite" : @(g_infinite_canvas ? YES : NO),
    @"panX" : @(g_pan_x),
    @"panY" : @(g_pan_y),
    @"zoom" : @(g_zoom),
    @"pageOffX" : @(g_page_off_x),
    @"pageOffY" : @(g_page_off_y),
    @"gridSize" : @(g_grid_size),
  };
}

// S7n: 光标十字层状态快照桥(供 Swift OverlayCursorRenderer)
NSDictionary *glaspen2_router_cursor_draw_state(void) {
  return @{
    @"visible" : @(g_cursor_visible ? YES : NO),
    @"x" : @(g_cursor_x),
    @"y" : @(g_cursor_y),
    @"eraser" : @(g_eraser_mode ? YES : NO),
  };
}

// S7t: 触发绘制视图重绘薄桥
void glaspen2_router_redraw_draw_view(void) {
  if (g_draw_view) [g_draw_view setNeedsDisplay:YES];
}

// S7w: 录制状态机总开关变化薄桥
void glaspen2_router_rec_chat_integration_changed(int on) { gl_recording_chat_integration_changed(on != 0); }

// S7ac: 描边开关的 Swift 薄桥
int glaspen2_router_outline_enabled(void) { return g_outline_enabled ? 1 : 0; }
void glaspen2_router_set_outline_state(int on) { g_outline_enabled = on != 0; }
void glaspen2_router_rebuild_surface(void) { rebuild_surface_from_strokes(); }

// S7ak: 镜头状态应用薄桥
void glaspen2_router_set_camera(double panX, double panY, double zoom, int assignZoom) {
  g_pan_x = panX;
  g_pan_y = panY;
  if (assignZoom) g_zoom = zoom;
}

// S7ap: 键盘平移镜头的 Swift 薄桥
double glaspen2_router_pan_x(void) { return g_pan_x; }
double glaspen2_router_pan_y(void) { return g_pan_y; }
double glaspen2_router_zoom(void) { return g_zoom; }
void glaspen2_router_canvas_apply_transform(void) { [CanvasCamera applyTransform]; }
void glaspen2_router_peek_lens_change(void) { peek_lens_change(); }

// S7av: 屏幕尺寸薄桥
double glaspen2_router_screen_w(void) { return g_screen_w; }
double glaspen2_router_screen_h(void) { return g_screen_h; }

// S7bg: 翻页模式镜头重置前清 page 偏移
void glaspen2_router_reset_page_offsets(void) {
  g_page_off_x = 0.0;
  g_page_off_y = 0.0;
}

// S7: 画布模式切换的 Swift 薄桥
int glaspen2_router_infinite(void) { return g_infinite_canvas ? 1 : 0; }
void glaspen2_router_set_infinite_state(int on) { g_infinite_canvas = on != 0; }
void glaspen2_router_clear_camera_dragging(void) { g_camera_mouse_dragging = NO; }
void glaspen2_router_pageview_update(void) { pageview_update(); }
void glaspen2_router_event_tap_reinstall(void) { event_tap_reinstall(); }

// S7: minimap 状态薄桥
int glaspen2_router_pageview_pw(void) { return g_pageview_pw; }

// S7bl: drawRect 编排的薄桥(表面/隧道状态快照 + flush)
void glaspen2_router_draw_prepare(void) {
  if (g_surface) cairo_surface_flush(g_surface);
  if (s_tun_active && s_tun_surface) cairo_surface_flush(s_tun_surface);
}
NSDictionary *glaspen2_router_draw_state(void) {
  NSMutableDictionary *d = [NSMutableDictionary dictionary];
  d[@"hasSurface"] = @(g_surface ? YES : NO);
  if (g_surface) {
    d[@"surfaceData"] = [NSValue valueWithPointer:cairo_image_surface_get_data(g_surface)];
    d[@"surfaceW"] = @(cairo_image_surface_get_width(g_surface));
    d[@"surfaceH"] = @(cairo_image_surface_get_height(g_surface));
    d[@"surfaceStride"] = @(cairo_image_surface_get_stride(g_surface));
  }
  d[@"tunnelActive"] = @(s_tun_active && s_tun_surface);
  if (s_tun_active && s_tun_surface) {
    d[@"tunnelData"] = [NSValue valueWithPointer:cairo_image_surface_get_data(s_tun_surface)];
    d[@"tunnelW"] = @(cairo_image_surface_get_width(s_tun_surface));
    d[@"tunnelH"] = @(cairo_image_surface_get_height(s_tun_surface));
    d[@"tunnelStride"] = @(cairo_image_surface_get_stride(s_tun_surface));
  }
  d[@"scale"] = @(g_scale);
  d[@"strokesVisible"] = @(g_strokes_visible ? YES : NO);
  d[@"showGrid"] = @(g_show_grid ? YES : NO);
  d[@"minimapEnabled"] = @(g_minimap_enabled ? YES : NO);
  d[@"infinite"] = @(g_infinite_canvas ? YES : NO);
  d[@"perfEnabled"] = @([PerfLog isEnabled] ? YES : NO);
  return d;
}

// S7bl: drawRect perf 打点薄桥
void glaspen2_router_perf_notes(const char *name, const char *notes, uint64_t t0) {
  [PerfLog recordName:[NSString stringWithUTF8String:name ? name : ""]
               durUs:[PerfLog elapsedUsSince:t0]
               notes:notes ? [NSString stringWithUTF8String:notes] : nil];
}

// S7bu: 显示器几何变更的薄桥
void glaspen2_router_invert_invalidate(void) { invert_stream_invalidate(); }
void glaspen2_router_set_screen_size(int w, int h) {
  g_screen_w = w;
  g_screen_h = h;
}
id glaspen2_router_draw_view(void) { return g_draw_view; }

// S7bv: 图层面刷新的薄桥(dirty tracker 在 panel.m)
int glaspen2_router_has_surface(void) { return g_surface ? 1 : 0; }
void glaspen2_router_dirty_reset(void) { dirty_reset(); }
int glaspen2_router_dirty_has(void) { return g_dirty_has ? 1 : 0; }
NSRect glaspen2_router_dirty_rect(void) { return g_dirty_rect; }

// S7bw: surface 生命周期薄桥
void glaspen2_router_install_surface(cairo_surface_t *surface) { g_surface = surface; }
void glaspen2_router_install_scale(double scale) { g_scale = scale; }
void glaspen2_router_set_has_last(int has) { g_has_last = has != 0; }
void glaspen2_router_surface_cache_invalidate(void) { surface_image_cache_invalidate(); }

// S7by: 首点实时绘制薄桥
cairo_surface_t *glaspen2_router_surface(void) { return g_surface; }
cairo_t *glaspen2_router_acquire_cr(int *owned) {
  if (g_active_cr) { if (owned) *owned = 0; return g_active_cr; }
  if (owned) *owned = 1;
  return cairo_create_scaled();
}
int glaspen2_router_soft_shadow(void) { return g_soft_shadow ? 1 : 0; }
double glaspen2_router_outline_pad(void) { return kOutlinePad; }
double glaspen2_router_pen_r(void) { return g_pen_r; }
double glaspen2_router_pen_g(void) { return g_pen_g; }
double glaspen2_router_pen_b(void) { return g_pen_b; }
int glaspen2_router_live_plan_dot(cairo_t *cr, double x, double y, double width) {
  return [LivePlan drawDotContext:(void *)cr x:x y:y width:width] ? 1 : 0;
}
cairo_pattern_t *glaspen2_router_invert_ink_source(cairo_t *cr) {
  return invert_ink_source(cr);
}
void glaspen2_router_dirty_include_surface_point(double x, double y, double pad) {
  dirty_include_surface_point(x, y, pad);
}

// S7bz: 实时段绘制薄桥
NSDictionary *glaspen2_router_raw_state(void) {
  return @{
    @"last_x" : @(g_raw_last_x),
    @"last_y" : @(g_raw_last_y),
    @"last_w" : @(g_raw_last_w),
    @"has_last" : @(g_raw_has_last ? YES : NO),
    @"has_prev" : @(g_raw_has_prev ? YES : NO),
    @"prev_x" : @(g_raw_prev_x),
    @"prev_y" : @(g_raw_prev_y),
    @"path_len" : @(g_raw_path_len),
  };
}
void glaspen2_router_update_raw_after_segment(double x, double y, double width) {
  if (g_raw_has_last) {
    double dx = x - g_raw_last_x, dy = y - g_raw_last_y;
    g_raw_path_len += sqrt(dx * dx + dy * dy);
  }
  g_raw_prev_x = g_raw_last_x;
  g_raw_prev_y = g_raw_last_y;
  g_raw_has_prev = YES;
  g_raw_last_x = x;
  g_raw_last_y = y;
  g_raw_last_w = width;
  g_raw_has_last = YES;
}
int glaspen2_router_live_plan_segment(cairo_t *cr, double x, double y, double width) {
  return [LivePlan drawSegmentContext:(void *)cr x:x y:y width:width] ? 1 : 0;
}
double glaspen2_router_outline_dash(void) { return kOutlineDash; }

// S7ca: 实时笔画上下文薄桥
double glaspen2_router_scale(void) { return g_scale; }
int glaspen2_router_active_ctx_present(void) { return g_active_cr ? 1 : 0; }
void glaspen2_router_install_active_ctx(cairo_t *ctx) { g_active_cr = ctx; }
cairo_t *glaspen2_router_take_active_ctx(void) {
  cairo_t *ctx = g_active_cr;
  g_active_cr = NULL;
  return ctx;
}
void glaspen2_router_reset_raw_stroke(void) {
  g_raw_has_prev = NO;
  g_raw_path_len = 0;
}

// S7cb: 飘渺模式薄桥
int glaspen2_router_tunnel_active(void) { return s_tun_active ? 1 : 0; }

// S7cc: 计划脏区薄桥
void glaspen2_router_dirty_include_surface_rect(double x, double y, double w, double h) {
  dirty_include_surface_rect(x, y, w, h);
}

// S7cd: 设置面板宿主薄桥(Flutter 对象仍在 ObjC 创建)
id glaspen2_router_settings_engine(void) {
  if (!g_flutter_engine) {
    g_flutter_engine = [[FlutterEngine alloc] initWithName:@"glaspen_settings"
                                                    project:nil];
    [g_flutter_engine runWithEntrypoint:nil];
  }
  return g_flutter_engine;
}
id glaspen2_router_settings_vc(void) {
  if (!g_flutter_vc) {
    g_flutter_vc = [[FlutterViewController alloc] initWithEngine:g_flutter_engine
                                                         nibName:nil
                                                          bundle:nil];
  }
  return g_flutter_vc;
}
id glaspen2_router_settings_delegate(void) {
  if (!g_settings_delegate) g_settings_delegate = [[SettingsWindowDelegate alloc] init];
  return g_settings_delegate;
}
id glaspen2_router_settings_window(void) { return g_settings_window; }
void glaspen2_router_set_settings_window(id window) { g_settings_window = window; }
void glaspen2_router_settings_apply_transparent(void) {
  gl_panel_transparent_apply(YES);
  if (g_flutter_vc.view.layer) g_flutter_vc.view.layer.opaque = NO;
}

// S7cf: 性能日志薄桥
int glaspen2_router_perf_enabled(void) { return [PerfLog isEnabled] ? 1 : 0; }
uint64_t glaspen2_router_elapsed_us(uint64_t start) { return [PerfLog elapsedUsSince:start]; }
void glaspen2_router_perf_init(void) { [PerfLog initFromEnv]; }

// S7ci: 诊断探针薄桥
int glaspen2_router_perform_hotkey(unsigned short keyCode) {
  return perform_hotkey(keyCode) ? 1 : 0;
}

// S7ck: 录制效果薄桥
int glaspen2_router_gif_fps(void) { return g_gif_fps; }
double glaspen2_router_gif_resolution(void) { return g_gif_resolution; }
double glaspen2_router_gif_speed(void) { return g_gif_speed; }
int glaspen2_router_gif_end_mode(void) { return g_gif_end_mode; }

// S7cl: 动作分派薄桥
void glaspen2_router_apply_infinite_canvas(int on) { apply_infinite_canvas(on != 0, YES); }
void glaspen2_router_show_notification(NSString *text) { show_notification(text); }
NSString *glaspen2_router_textf(NSString *key, NSDictionary *args) { return gl_textf(key, args); }
int glaspen2_router_prev_color_index(void) { return g_prev_color_index; }
int glaspen2_router_prev_width_index(void) { return g_prev_width_index; }

// S7cm: 快捷键分发薄桥
int glaspen2_router_glass_enabled(void) { return g_glass_enabled ? 1 : 0; }
void glaspen2_router_clear_screen(void) { clear_screen(); }
void glaspen2_router_replay_strokes(void) { replay_strokes_from_memory(); }
void glaspen2_router_page_flip(int forward, dispatch_block_t prepare, dispatch_block_t commit) {
  page_flip_swap(forward != 0, prepare, commit);
}
void glaspen2_router_page_flip_from(int forward, long from, dispatch_block_t prepare, dispatch_block_t commit) {
  page_flip_swap_from(forward != 0, from, prepare, commit);
}

// S7cn: 页视图/重放薄桥
int glaspen2_router_pageview_is_fit(void) { return g_pageview_fit ? 1 : 0; }
double glaspen2_router_pageview_paper_ox(void) { return g_pageview_ox; }
double glaspen2_router_pageview_paper_oy(void) { return g_pageview_oy; }
double glaspen2_router_pageview_paper_scale(void) { return g_pageview_scale; }
double glaspen2_router_pageview_page_w(void) { return g_pageview_pw; }
double glaspen2_router_pageview_page_h(void) { return g_pageview_ph; }

// S7cp: 页视图几何薄桥
void glaspen2_router_pageview_set(int fit, double scale, double ox, double oy, int pw, int ph) {
  g_pageview_fit = fit != 0;
  g_pageview_scale = scale;
  g_pageview_ox = ox;
  g_pageview_oy = oy;
  g_pageview_pw = pw;
  g_pageview_ph = ph;
}

// S7cs: 隧道几何薄桥
double glaspen2_router_tunnel_k(void) { return s_tun_k; }
int glaspen2_router_tunnel_surf_w(void) { return s_tun_surf_w; }
int glaspen2_router_tunnel_surf_h(void) { return s_tun_surf_h; }
double glaspen2_router_tunnel_travel(void) { return s_tun_travel; }

// S7cu: 隧道帧渲染桥
void glaspen2_router_flush_tunnel(void) { flush_tunnel_to_layer(); }
NSDictionary *glaspen2_router_tunnel_state(void) {
  NSMutableArray *cards = [NSMutableArray arrayWithCapacity:(NSUInteger)s_tun_cards];
  for (int i = 0; i < s_tun_cards; i++) {
    if (!s_tun_card[i]) continue;
    NSMutableDictionary *c = [NSMutableDictionary dictionary];
    c[@"depth"] = @(s_tun_depth[i]);
    c[@"card"] = [NSValue valueWithPointer:s_tun_card[i]];
    if (s_tun_card_mip[i]) c[@"mip"] = [NSValue valueWithPointer:s_tun_card_mip[i]];
    [cards addObject:c];
  }
  NSMutableDictionary *d = [NSMutableDictionary dictionary];
  d[@"surface"] = [NSValue valueWithPointer:s_tun_surface];
  if (s_tun_blur) d[@"blur"] = [NSValue valueWithPointer:s_tun_blur];
  d[@"cards"] = cards;
  d[@"surfW"] = @(s_tun_surf_w);
  d[@"surfH"] = @(s_tun_surf_h);
  d[@"travel"] = @(s_tun_travel);
  d[@"back"] = @(s_tun_back ? YES : NO);
  d[@"thick"] = @(s_tun_thick);
  d[@"paneA"] = @(s_tun_pane_a);
  return d;
}

// S7cv: 隧道生命周期桥
void *glaspen2_router_tunnel_take_card(int idx) {
  if (idx < 0 || idx >= TUNNEL_MAX_CARDS) return NULL;
  void *p = s_tun_card[idx]; s_tun_card[idx] = NULL; return p;
}
void *glaspen2_router_tunnel_take_mip(int idx) {
  if (idx < 0 || idx >= TUNNEL_MAX_CARDS) return NULL;
  void *p = s_tun_card_mip[idx]; s_tun_card_mip[idx] = NULL; return p;
}
void *glaspen2_router_tunnel_take_blur(void) { void *p = s_tun_blur; s_tun_blur = NULL; return p; }
void *glaspen2_router_tunnel_take_surface(void) { void *p = s_tun_surface; s_tun_surface = NULL; return p; }
void glaspen2_router_tunnel_set_cards(int count) { s_tun_cards = count; }
void glaspen2_router_tunnel_cancel_timer(void) {
  if (s_tun_timer) { dispatch_source_cancel(s_tun_timer); s_tun_timer = NULL; }
}
void glaspen2_router_tunnel_render_final(void) { tunnel_frame(1.0); }
void glaspen2_router_tunnel_set_active(int active) { s_tun_active = active != 0; }
void glaspen2_router_tunnel_run_callbacks(void) {
  if (s_tun_prepare) { s_tun_prepare(); s_tun_prepare = NULL; }
  if (s_tun_commit) { s_tun_commit(); s_tun_commit = NULL; }
}
double glaspen2_router_tunnel_now(void) { return tunnel_now(); }
double glaspen2_router_tunnel_t0(void) { return s_tun_t0; }
uint64_t glaspen2_router_tunnel_frames(void) { return s_tun_frames; }
