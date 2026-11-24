


static void show_settings_panel(void) { [SettingsPanel show]; }

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

// Handle display configuration changes (resolution, arrangement, etc.)
// 分辨率变化的实际处理(经 2.5s 防抖后调用, 分辨率已稳定)。
static void display_change_apply(int new_w, int new_h) { [OverlayWindowFactory applyDisplayChangeWidth:new_w height:new_h]; }

// S5-3: 防抖/重读逻辑已迁 Swift(DisplayChangeMonitor); 这里只留 apply/size 桥。
void glaspen2_display_size(int *w, int *h) {
    if (w) *w = (int)glaspen2_router_screen_w();
    if (h) *h = (int)glaspen2_router_screen_h();
}

void glaspen2_display_apply(int w, int h) {
    display_change_apply(w, h);
}

// ── 反色突出(实验) ──
// SCStream 连续捕获"自身窗口以下的屏幕"(自家窗口 sharingType=None 拒绝
// 共享像素, 从源头排除反馈), 帧到后逐像素反相写入双缓冲之一, 主线程
// 消费时把该缓冲作为墨迹 pattern —— 每个墨迹像素显示其正下方背景的反色。
// SCStream 只在画面变化时送帧: 静态背景 = 零开销, 完全符合"只在背景动
// 的时候调整颜色"的预期。帧快于主线程消费时丢帧保最新。
// 帧率上限由设置 invertFps(10/30/60/100)经 minimumFrameInterval 控制。
// 注意: 捕获不含磨砂玻璃效果, 反色开启时建议关闭/调低玻璃。
// S7dj: 反色捕获已迁 Swift(InvertCapture, macOS 12.3+); 12.0-12.2 自动禁用。
cairo_pattern_t *invert_ink_source(cairo_t *cr) {
  if (@available(macOS 12.3, *)) {
    return (cairo_pattern_t *)[InvertCapture inkSourceWithCR:(void *)cr];
  }
  return NULL;
}
void invert_ink_apply(int on) {
  if (@available(macOS 12.3, *)) [InvertCapture applyOn:on != 0];
}
void invert_stream_restart(void) {
  if (@available(macOS 12.3, *)) [InvertCapture restart];
}
static void invert_stream_invalidate(void) {
  if (@available(macOS 12.3, *)) [InvertCapture invalidate];
}
void gl_panel_transparent_apply_full(BOOL on) {
  gl_panel_transparent_apply(on);
  FlutterViewController *fvc = (FlutterViewController *)glaspen2_router_settings_vc();
  if (fvc.view) fvc.view.layer.opaque = !on;
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
// S7dj: 反色捕获桥(实现已在 Swift InvertCapture, 12.3+ 才启用)
void glaspen2_router_set_window_sharing_none(void) {
  [(NSWindow *)glaspen2_router_main_window() setSharingType:NSWindowSharingNone];
  NSWindow *sw = (NSWindow *)glaspen2_router_settings_window();
  if (sw) [sw setSharingType:NSWindowSharingNone];
}
void glaspen2_router_set_window_sharing_readonly(void) {
  [(NSWindow *)glaspen2_router_main_window() setSharingType:NSWindowSharingReadOnly];
  NSWindow *sw = (NSWindow *)glaspen2_router_settings_window();
  if (sw) [sw setSharingType:NSWindowSharingReadOnly];
}
