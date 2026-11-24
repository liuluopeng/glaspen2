@interface GlaspenMenuHandler : NSObject <NSMenuDelegate, NSApplicationDelegate>
@end

static GlaspenMenuHandler *g_menuHandler = nil;
static NSStatusItem *g_statusItem = nil;
static NSMenu *g_menu = nil;


static int g_selectedColorIndex = 0; // 0=red (default)
// 换回上一色/上一粗细的交换键用; 重新选色时更新, 初值给一个可用的另一端。
static int g_prev_color_index = 9;   // black
static int g_prev_width_index = 2;   // fine (current default is 3/medium)
// 书写中按了换色/换粗细: 等抬笔后再应用, 避免污染正在写的那一笔。
static int g_pending_color_index = -1;
static int g_pending_width_index = -1;

static NSAttributedString* gl_color_item_text(NSInteger idx);
static NSAttributedString* gl_width_item_text(NSInteger idx);
static void update_menu_texts(void) {
    // S3b-2b-5: 索引/子菜单/状态刷新在 Swift(MenuUpdater); 文案仍由这里的
    // L()/gl_text/gl_color_item_text 预先算好传入(菜单专属文案不进 core i18n)。
    NSMutableArray<NSAttributedString *> *colorTitles = [NSMutableArray array];
    for (int i = 0; i < g_color_preset_count; i++) {
        [colorTitles addObject:gl_color_item_text(i)];
    }
    NSMutableArray<NSAttributedString *> *widthTitles = [NSMutableArray array];
    for (int i = 0; i < g_width_preset_count; i++) {
        [widthTitles addObject:gl_width_item_text(i)];
    }
    NSArray<NSString *> *topTitles = @[
        L(@"保存(含背景)", @"Save (with bg)"),
        L(@"保存(涂鸦)", @"Save (drawing)"),
        L(@"新建画布", @"New canvas"),
        L(@"导出", @"Export"),
        @"",
        @"",
        L(@"设置...", @"Settings..."),
        L(@"更多", @"More"),
        @"",
        L(@"English", @"中文"),
        L(@"退出", @"Quit"),
    ];
    NSArray<NSString *> *exportTitles = @[
        L(@"保存笔记 (Xournal)", @"Save Notes (Xournal)"),
        L(@"导出无限画布 PDF (分页)", @"Export infinite canvas PDF (paged)"),
        L(@"导出无限画布 SVG (整幅)", @"Export infinite canvas SVG (whole)"),
    ];
    NSArray<NSString *> *moreTitles = @[
        L(@"开机自启", @"Launch at login"),
        L(@"磨砂玻璃", @"Frosted Glass"),
        L(@"笔迹描边", @"Stroke outline"),
    ];
    NSMenuItem *modeItem = [MenuLookup itemWithTag:778 inMenu:g_menu];
    NSString *modeTitle = (modeItem.state == NSControlStateValueOn)
        ? gl_text(@"canvas.fixed_mode")
        : gl_text(@"canvas.ethereal_mode");
    NSString *toggleTitle = g_enabled ? L(@"关闭涂鸦", @"Disable Drawing")
                                      : L(@"开启涂鸦", @"Enable Drawing");
    [MenuUpdater updateTextsInMenu:g_menu
                       colorTitles:colorTitles
                       widthTitles:widthTitles
                        topTitles:topTitles
                     exportTitles:exportTitles
                       moreTitles:moreTitles
                        modeTitle:modeTitle
                      toggleTitle:toggleTitle];
}

// ── 颜色/粗细菜单条目的富文本:内嵌小图标 + 名称 ──
// NSMenuItem.image 在新版系统菜单里不渲染(实测菜单全是字), 而正文里
// 内嵌附件图(attributedTitle + NSTextAttachment)任何系统版本都画。
// 颜色 = 圆点色块(一眼可见);粗细 = 横线粗细(粗细预设的直观映射)。

// 颜色/粗细显示名已迁 Swift(MenuNames); 文案仍由 ObjC gl_text/core i18n 决定。
// S3b-2b-4b: Swift 经 Bridging.h 的 C 桥按稳定 key 取名。
NSString *glaspen2_menu_l10n(NSString *key) {
    return gl_text(key);
}

static NSAttributedString* gl_color_item_text(NSInteger idx) {
    NSColor *c = [NSColor colorWithRed:g_color_presets[idx].r
                                 green:g_color_presets[idx].g
                                  blue:g_color_presets[idx].b
                                 alpha:1.0];
    return [MenuIconFactory menuTextWithIcon:[MenuIconFactory colorDotWithColor:c]
                                      name:[MenuNames colorDisplayNameWithIndex:idx]];
}

static NSAttributedString* gl_width_item_text(NSInteger idx) {
    return [MenuIconFactory menuTextWithIcon:[MenuIconFactory widthLineWithScale:g_width_presets[idx]]
                                      name:[MenuNames widthDisplayNameWithIndex:idx]];
}

// ── 菜单栏图标: 一支斜放的马克笔, 把当前关键状态画进 18pt 里 ──
//   笔色      → 笔杆填充色
//   笔宽      → 笔杆粗细(越粗的笔杆 = 越粗的笔画)
//   橡皮擦    → 正在用笔尾擦除时, 字形整体换成橡皮块
//   涂鸦停用  → 字形淡化 + 一条斜杠
//   飘渺画布  → 左上角小圆环徽标(ghost dot)
//   无限画布  → 右下角 ∞ 徽标
// 位图按 4x 渲染保证 Retina 清晰; 白晕 + 深描边双勾线让深/浅色菜单栏都可读
// (纯白笔杆在浅色栏、纯黑笔杆在深色栏都不会隐形)。
static BOOL g_ethereal_canvas; // 定义在下方(飘渺画布涂鸦模式), 图标要用


static void update_status_icon_color(void) {
    // S7: 图标刷新(setImage/清标题)已迁 Swift(StatusIconUpdater)。
    [StatusIconUpdater applyToStatusItem:g_statusItem
                                  eraser:g_eraser_mode
                                 enabled:g_enabled
                                ethereal:g_ethereal_canvas
                                infinite:g_infinite_canvas
                              widthScale:g_width_scale
                                  penRed:g_pen_r
                                penGreen:g_pen_g
                                 penBlue:g_pen_b];
}



static void update_menu_checkmarks(void) {
    // S3b-2b-4a: 勾选刷新逻辑在 Swift(MenuUpdater);这里只传当前状态。
    [MenuUpdater updateCheckmarksInMenu:g_menu
                             colorCount:g_color_preset_count
                             widthCount:g_width_preset_count
                             colorIndex:g_selectedColorIndex
                             widthIndex:g_selected_width_index];
}

static void toggle_enabled(void) {
    g_enabled = !g_enabled;
    if (!g_enabled) {
        // 关闭涂鸦: 临时隐藏整个 glaspen 覆盖层(笔迹/方格/磨砂玻璃),
        // 以便干净地使用其他绘图软件。
        restore_system_cursor();
        finish_active_stroke();
        if (g_window) [g_window orderOut:nil];
        if (g_pressure_monitor) pm_hide();
    } else {
        if (g_window) {
            if (!g_surface && g_draw_view) ensure_surface(g_draw_view);
            // 飘渺模式的隐藏态:V 重新启用后维持隐藏(窗口不出场)
            if (g_ethereal_canvas && !g_strokes_visible) {
                rebuild_surface_from_strokes();
                [g_window setIsVisible:NO];
            } else {
                [g_window orderFrontRegardless];
                rebuild_surface_from_strokes();
            }
        }
        if (g_pressure_monitor) pm_show();
    }
    update_status_icon_color();
    show_notification(g_enabled
        ? gl_text(@"draw.enabled")
        : gl_text(@"draw.disabled"));
    // Update menu item
    NSMenuItem *item = [MenuLookup itemWithTag:888 inMenu:g_menu];
    if (item) {
        [item setState:g_enabled ? NSControlStateValueOn : NSControlStateValueOff];
        [item setTitle:g_enabled ? L(@"关闭涂鸦", @"Disable Drawing") : L(@"开启涂鸦", @"Enable Drawing")];
    }
}

// Two canvas drawing modes, switched by ⌘⌃X:
//   - 固定画布涂鸦模式 (fixed canvas mode): the canvas is always visible;
//     the pen never auto-shows or hides it.
//   - 飘渺画布涂鸦模式 (ethereal canvas mode): the strokes start hidden;
//     hovering or touching down shows them, and pen-leave hides them again
//     immediately (silent).
// Hiding only affects the strokes (and the frosted-glass backdrop) — the
// overlay window itself stays visible, so notifications and the crosshair
// keep working. Pen passthrough is managed separately by ⌘ + ⌃ + V
// (g_enabled) — X never touches it.
// 显隐为**即时**笔迹级切换(无动效): 悬浮边界笔反复进出时, 窗口级
// 系统动效会反复重启造成闪烁/哆嗦, 故此处绝不做窗口级显隐。
static BOOL g_ethereal_canvas = NO; // YES = 飘渺画布涂鸦模式
static BOOL g_strokes_visible = YES; // strokes drawn on the overlay?

// Hide the strokes (飘渺 mode). Returns YES if it actually hid them.
static void ethereal_hide_now(void) {
    if (!g_ethereal_canvas || !g_strokes_visible) return;
    finish_active_stroke(); // don't strand an in-flight stroke
    g_strokes_visible = NO;
    if (g_glass_follow_strokes && g_glass_view) g_glass_view.hidden = YES;
    if (g_pressure_monitor) pm_hide();
    [g_draw_view setNeedsDisplay:YES]; // 笔离开数位板: 立即隐藏(无动效)
}

/// 飘渺隐藏: 笔离开数位板立即隐藏(无动效, 无延迟)。
static BOOL auto_hide_now(void) {
    // 翻页动效(时光隧道)期间绝不能藏笔迹: hideNow 会把玻璃一起隐藏,
    // 整个隧道画面就"消失"了(表现为动画期间屏幕一片空白)。
    if (s_tun_active) return NO;
    if (!g_ethereal_canvas) return NO;
    if (!g_strokes_visible) return NO;
    ethereal_hide_now();
    return YES;
}

/// 启动恢复飘渺模式(run.m 在视图就绪后调用)。hideNow 以"笔迹可见"为前置,
/// 先归位再落位, 落位路径与 toggle 进入飘渺完全一致(玻璃跟随/压强计一并处理)。
void gl_ethereal_restore_now(void) {
    if (!g_ethereal_canvas) return;
    g_strokes_visible = YES;
    ethereal_hide_now();
}

// Show the strokes because the pen came back (飘渺 mode).
static void auto_show_canvas(void) {
    if (!g_ethereal_canvas) return;
    if (!g_strokes_visible) {
        g_strokes_visible = YES;
        gl_glass_apply(); // restore the glass per its own toggle
        if (g_pressure_monitor) pm_show();
        [g_draw_view setNeedsDisplay:YES];
        NSLog(@"[ethereal] 笔迹重现");
    }
}

// ── Page-navigation / lens peek (ethereal mode) ──
// 翻页载入笔迹、无限画布平移/缩放镜头时, 笔迹是隐藏的; 短暂显示让用户
// 看见新页面/新镜头, 然后按 `seconds` 隐回去(笔互动会取消隐藏计时器)。
static NSTimer *g_peek_timer = nil;

static void peek_cancel_timer(void) {
    [g_peek_timer invalidate];
    g_peek_timer = nil;
}

static void peek_strokes(double seconds) {
    if (!g_ethereal_canvas) return; // fixed mode: strokes are always visible
    peek_cancel_timer();
    if (!g_strokes_visible) {
        g_strokes_visible = YES;
        gl_glass_apply();
        if (g_pressure_monitor) pm_show();
        [g_draw_view setNeedsDisplay:YES];
    }
    g_peek_timer = [NSTimer scheduledTimerWithTimeInterval:seconds repeats:NO block:^(NSTimer *timer) {
        g_peek_timer = nil;
        if (g_ethereal_canvas && !g_stroke_active) {
            auto_hide_now();
        }
    }];
}

/// 飘渺模式下镜头平移/缩放时, 笔迹本身是隐藏的, 用户看不到画面变化。
/// 短暂 peek 一下, 让新镜头下的笔迹/网格露出来再隐回去。
///
/// 笔正悬停(笔迹已经可见、且没有 peek 计时器)时不要启动隐藏计时器,
/// 否则悬浮看画会被 1 秒后自动隐藏。已有 peek 计时器时重新计时, 让连续
/// 平移/缩放期间笔迹保持可见, 松手后再隐。
static void peek_lens_change(void) {
    if (!g_ethereal_canvas || !g_infinite_canvas) return;
    if (g_strokes_visible && g_peek_timer == nil) return; // 悬停可见, 保持
    peek_strokes(1.0);
}

// Switch between 固定画布涂鸦模式 and 飘渺画布涂鸦模式. Shortcut: ⌘ + ⌃ + X
static void toggle_canvas_mode(void) {
    peek_cancel_timer(); // a pending page-peek hide must not fire after a mode switch
    if (!g_ethereal_canvas) {
        // → 飘渺画布涂鸦模式: hide the strokes, peek rules take over
        g_ethereal_canvas = YES;
        finish_active_stroke(); // commit any in-flight stroke before hiding
        g_strokes_visible = NO;
        if (g_glass_follow_strokes && g_glass_view) g_glass_view.hidden = YES;
        if (g_pressure_monitor) pm_hide();
        [g_draw_view setNeedsDisplay:YES];
        show_notification(gl_text(@"canvas.ethereal_mode"));
    } else {
        // → 固定画布涂鸦模式: show the strokes and keep them visible
        g_ethereal_canvas = NO;
        g_strokes_visible = YES;
        gl_glass_apply();
        if (g_pressure_monitor) pm_show();
        [g_draw_view setNeedsDisplay:YES];
        show_notification(gl_text(@"canvas.fixed_mode"));
    }
    // Sync the menu item (title shows the mode you switch TO, like toggleDraw)
    NSMenuItem *item = [MenuLookup itemWithTag:778 inMenu:g_menu];
    if (item) {
        [item setState:g_ethereal_canvas ? NSControlStateValueOn : NSControlStateValueOff];
        [item setTitle:g_ethereal_canvas
            ? gl_text(@"canvas.fixed_mode")
            : gl_text(@"canvas.ethereal_mode")];
    }
    // 持久化(与 Windows 同键 "ethereal"): 启动恢复在 run.m 设置装载段
    glaspen2_save_bool_setting("ethereal", g_ethereal_canvas ? 1 : 0);
    update_status_icon_color(); // 飘渺画布 = 图标左上角的 ghost 圆环徽标
}

/// 彻底退出:菜单「退出」、可配置快捷键 quit、设置面板/自动更新流程共用**同一条**
/// 路径 —— 先记性能汇总、把光标还给系统, 再 terminate。terminate 会同步走
/// applicationShouldTerminate 把排队点写完并 checkpoint WAL(见下), 进程里
/// 没有其它需要单独收尾的子进程(设置面板是内嵌 Flutter 视图, 不是独立进程)。
static void gl_quit_app(void) {
    // S7: 退出逻辑已迁 Swift(MenuHandler.quitAppNow)。
    [MenuHandler quitAppNow];
}

@implementation GlaspenMenuHandler

- (void)saveWithBg { [MenuHandler saveWithBg]; }
- (void)saveOnly { [MenuHandler saveOnly]; }
- (void)clearScreen { [MenuHandler clearScreen]; }
- (void)toggleDraw { [MenuHandler toggleDraw]; }
- (void)saveXoj { [MenuHandler saveXoj]; }

- (void)toggleLanguage { [MenuHandler toggleLanguage]; }
- (void)showSettingsPanel { [MenuHandler showSettingsPanel]; }
- (void)toggleCanvasMode { [MenuHandler toggleCanvasMode]; }

- (void)toggleLaunch { [MenuHandler toggleLaunch]; }
- (void)toggleGlass { [MenuHandler toggleGlass]; }
- (void)toggleOutline { [MenuHandler toggleOutline]; }
- (void)toggleInfiniteCanvas { [MenuHandler toggleInfiniteCanvas]; }

- (void)exportInfinitePdf { [MenuHandler exportInfinitePdf]; }
- (void)exportInfiniteSvg { [MenuHandler exportInfiniteSvg]; }

- (void)selectColor:(NSMenuItem *)sender { [MenuHandler selectColor:sender]; }
- (void)selectWidth:(NSMenuItem *)sender { [MenuHandler selectWidth:sender]; }

// NSApplicationDelegate
- (NSApplicationTerminateReply)applicationShouldTerminate:(NSApplication *)sender {
    return [MenuHandler applicationShouldTerminate:sender];
}

// S3b-2b-7a: quitApp/menuWillOpen:/menuDidClose: 已迁 Swift(menu_handler.swift),
// 这里保留 delegate 需要的薄桥; g_event_tap 是本文件翻译单元内的静态全局。
void glaspen2_menu_will_open_bridge(void) {
    update_status_icon_color();
    update_menu_checkmarks();
}

void glaspen2_menu_did_close_bridge(void) {
    update_status_icon_color();
    // Re-enable CGEventTap after menu closes
    if (g_event_tap) {
        CGEventTapEnable(g_event_tap, true);
    }
}

void glaspen2_menu_quit_bridge(void) {
    gl_quit_app();
}

// S3b-2b-7b: 语言切换的薄桥(g_lang 在本翻译单元; 刷新逻辑仍用原函数)
int glaspen2_menu_lang(void) {
    return g_lang;
}

void glaspen2_menu_set_lang(int value) {
    g_lang = value == 1 ? 1 : 0;
}

void glaspen2_menu_refresh_texts(void) {
    update_menu_texts();
}

// S3b-2b-7c: 开关类动作 getter/setter 薄桥(逻辑在 Swift MenuHandler)
int glaspen2_menu_launch_at_login(void) {
    return glaspen2_is_launch_at_login();
}

void glaspen2_menu_set_launch(int on) {
    gl_settings_set_launch(on != 0);
}

int glaspen2_menu_glass_enabled(void) {
    return g_glass_enabled ? 1 : 0;
}

void glaspen2_menu_set_glass(int on) {
    gl_settings_set_glass_enabled(on != 0);
}

int glaspen2_menu_outline_enabled(void) {
    return g_outline_enabled ? 1 : 0;
}

void glaspen2_menu_apply_outline(int on) {
    apply_outline(on != 0);
}

int glaspen2_menu_infinite_enabled(void) {
    return g_infinite_canvas ? 1 : 0;
}

void glaspen2_menu_apply_infinite(int on) {
    apply_infinite_canvas(on != 0, YES);
}

// S3b-2b-7d: 保存/导出/设置/选择动作薄桥
void glaspen2_menu_save_with_bg(void) { save_with_background(); }
void glaspen2_menu_save_only(void) { save_drawing_only(); }
void glaspen2_menu_clear_screen(void) { clear_screen(); }
void glaspen2_menu_toggle_enabled(void) { toggle_enabled(); }
void glaspen2_menu_show_settings(void) { show_settings_panel(); }
void glaspen2_menu_toggle_canvas_mode(void) { toggle_canvas_mode(); }
void glaspen2_menu_select_color(int idx) { gl_settings_set_color(idx); }
void glaspen2_menu_select_width(int idx) { gl_settings_set_width(idx); }
void glaspen2_menu_save_xoj_raw(void) { glaspen2_save_xoj(); }
void glaspen2_menu_notify(NSString *text) { show_notification(text); }
void glaspen2_menu_screen_size(int *w, int *h) {
    if (w) *w = g_screen_w;
    if (h) *h = g_screen_h;
}
int glaspen2_menu_export_infinite_pdf_paged(int w, int h) {
    return glaspen2_export_infinite_pdf_paged(w, h);
}
int glaspen2_menu_export_infinite_svg(void) {
    return glaspen2_export_infinite_svg();
}
void glaspen2_menu_shutdown(void) { glaspen2_shutdown(); }
NSString *glaspen2_menu_l10n_pair(NSString *zh, NSString *en) { return L(zh, en); }

// ObjC 侧保留薄 delegate 方法(仅转发给 Swift MenuHandler); 业务逻辑在 Swift。
- (void)quitApp { [MenuHandler quitApp]; }
- (void)menuWillOpen:(NSMenu *)menu { [MenuHandler menuWillOpen]; }
- (void)menuDidClose:(NSMenu *)menu { [MenuHandler menuDidClose]; }

@end

// --- Settings Panel (Flutter-based) ---
static FlutterEngine *g_flutter_engine = nil;
static FlutterViewController *g_flutter_vc = nil;
static NSWindow *g_settings_window = nil;

static void show_settings_panel(void);
static void sync_settings_panel(void);

// S7x: 遗留设置面板按钮 stub 已彻底删除(桌面面板早已移除)。

// 无限画布总览载荷:渲染当前页包围盒适配图 + 当前视口矩形。
// 返回 nil 表示空画布(无笔迹)。
static NSDictionary *canvas_overview_payload(double w, double h, double *bounds_out) {
    double bx, by, bx2, by2;
    if (!glaspen2_stroke_bbox(&bx, &by, &bx2, &by2)) {
        return nil;
    }
    double bw = bx2 - bx, bh = by2 - by;
    if (bw < 1.0) bw = 1.0;
    if (bh < 1.0) bh = 1.0;
    // 外扩 5%,笔迹不贴边
    double mx = bw * 0.05, my = bh * 0.05;
    bx -= mx; by -= my; bw += mx * 2; bh += my * 2;
    if (bounds_out) {
        bounds_out[0] = bx;
        bounds_out[1] = by;
        bounds_out[2] = bw;
        bounds_out[3] = bh;
    }

    int ow = (int)w, oh = (int)h;
    int outLen = 0;
    unsigned char *png = glaspen2_render_canvas_overview(bx, by, bw, bh, ow, oh, &outLen);
    if (!png || outLen <= 0) return nil;

    // 总览映射(scale/offset 必须与渲染一致),再映射当前视口矩形
    double ov_scale = (ow / bw) < (oh / bh) ? (ow / bw) : (oh / bh);
    double ov_ox = (ow - bw * ov_scale) * 0.5;
    double ov_oy = (oh - bh * ov_scale) * 0.5;
    // 交互 zoom 兜底(> 0.05 否则 1.0)单源在 core(D5b)
    double z = glaspen2_camera_step_zoom(g_zoom);
    double vr_w = g_screen_w / z * ov_scale;
    double vr_h = g_screen_h / z * ov_scale;
    double vx = ov_ox + (g_pan_x - bx) * ov_scale;
    double vy = ov_oy + (g_pan_y - by) * ov_scale;

    NSData *data = [NSData dataWithBytes:png length:outLen];
    glaspen2_free_rust_bytes(png, outLen);
    return @{
        @"png": data,
        @"rect": @[[NSNumber numberWithDouble:vx], [NSNumber numberWithDouble:vy],
                   [NSNumber numberWithDouble:vr_w], [NSNumber numberWithDouble:vr_h]],
    };
}

// ---------------------------------------------------------------------------
// flutter_rust_bridge:设置面板的 ObjC 侧入口
//
// 设置面板是嵌在同一进程里的 Flutter 视图,Rust 代码就在主可执行文件里,
// 所以 Dart 用 ExternalLibrary.process() 直接解析 Rust 符号 —— 下面这些 C
// 函数由 Rust(src/api.rs)反向调用,不再需要 Flutter MethodChannel。
//
// 它们会被 FRB 的工作线程调用,因此所有触碰 AppKit 的操作都要切回主线程。
// 主线程不会反过来等这些线程,所以 dispatch_sync 到主队列不会死锁。
// ---------------------------------------------------------------------------

#include <string.h>

/// 把 block 放到主线程同步执行(已在主线程则就地调用)。
static void gl_run_on_main_sync(dispatch_block_t block) {
    if ([NSThread isMainThread]) {
        block();
    } else {
        dispatch_sync(dispatch_get_main_queue(), block);
    }
}

/// 读一条字符串设置(DB),转成 NSString(未设置为空串)。调用方持有返回值。
/// 仅用于 settings_json 等非热路径;SQLite 单行读取在主线程也足够快。
static NSString *gl_string_setting(const char *key) {
    char *v = glaspen2_load_string_setting(key);
    NSString *s = v ? [NSString stringWithUTF8String:v] : @"";
    glaspen2_free_c_string(v);
    return s;
}

// ── 可配置快捷键 ─────────────────────────────────────────────────────
// 动作 id 与默认组合; 真正的动作执行在 run.m 的 perform_action()。本区只
// 负责 "chord → action" 映射、持久化、录制期间暂停全局拦截。
//
// chord 规范: cmd+ctrl+alt+shift+<Key>，顺序固定; 至少包含 ctrl 或 alt
// (只按 ⌘/⇧ 的组合容易和 App 菜单/系统快捷键冲突, 且录制时会被菜单先吃掉)。
// 空串 = 该动作未绑定。按键名走物理位置(与布局无关), 与事件 tap 的
// kVK_* keycode 一一对应。
static NSArray<NSString *> *gl_shortcut_actions(void) {
    static NSArray<NSString *> *actions;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        char *json = glaspen2_chord_actions_json();
        NSData *data = [NSData dataWithBytes:json length:strlen(json)];
        id obj = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
        actions = [obj isKindOfClass:[NSArray class]] ? obj : @[];
        glaspen2_free_c_string(json);
    });
    return actions;
}

static NSDictionary<NSString *, NSString *> *gl_shortcut_defaults(void) {
    // 默认键位单源在 core/ui.rs(经 FFI 下发, 与 Windows 同一张表)。
    static NSDictionary<NSString *, NSString *> *defaults;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        char *json = glaspen2_chord_defaults_json();
        NSData *data = [NSData dataWithBytes:json length:strlen(json)];
        id obj = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
        defaults = [obj isKindOfClass:[NSDictionary class]] ? obj : @{};
        glaspen2_free_c_string(json);
    });
    return defaults;
}

/// keycode → 规范按键名。只收录能当全局快捷键的键; 其它键录制时拒绝。
static NSDictionary<NSNumber *, NSString *> *gl_keycode_names(void) {
    static NSDictionary *m;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        m = @{
            @(kVK_ANSI_A): @"A", @(kVK_ANSI_B): @"B", @(kVK_ANSI_C): @"C",
            @(kVK_ANSI_D): @"D", @(kVK_ANSI_E): @"E", @(kVK_ANSI_F): @"F",
            @(kVK_ANSI_G): @"G", @(kVK_ANSI_H): @"H", @(kVK_ANSI_I): @"I",
            @(kVK_ANSI_J): @"J", @(kVK_ANSI_K): @"K", @(kVK_ANSI_L): @"L",
            @(kVK_ANSI_M): @"M", @(kVK_ANSI_N): @"N", @(kVK_ANSI_O): @"O",
            @(kVK_ANSI_P): @"P", @(kVK_ANSI_Q): @"Q", @(kVK_ANSI_R): @"R",
            @(kVK_ANSI_S): @"S", @(kVK_ANSI_T): @"T", @(kVK_ANSI_U): @"U",
            @(kVK_ANSI_V): @"V", @(kVK_ANSI_W): @"W", @(kVK_ANSI_X): @"X",
            @(kVK_ANSI_Y): @"Y", @(kVK_ANSI_Z): @"Z",
            @(kVK_ANSI_0): @"0", @(kVK_ANSI_1): @"1", @(kVK_ANSI_2): @"2",
            @(kVK_ANSI_3): @"3", @(kVK_ANSI_4): @"4", @(kVK_ANSI_5): @"5",
            @(kVK_ANSI_6): @"6", @(kVK_ANSI_7): @"7", @(kVK_ANSI_8): @"8",
            @(kVK_ANSI_9): @"9",
            @(kVK_ANSI_Minus): @"-", @(kVK_ANSI_Equal): @"=",
            @(kVK_ANSI_LeftBracket): @"[", @(kVK_ANSI_RightBracket): @"]",
            @(kVK_ANSI_Backslash): @"\\", @(kVK_ANSI_Semicolon): @";",
            @(kVK_ANSI_Quote): @"'", @(kVK_ANSI_Grave): @"`",
            @(kVK_ANSI_Comma): @",", @(kVK_ANSI_Period): @".",
            @(kVK_ANSI_Slash): @"/",
            @(kVK_LeftArrow): @"Left", @(kVK_RightArrow): @"Right",
            @(kVK_DownArrow): @"Down", @(kVK_UpArrow): @"Up",
            @(kVK_PageUp): @"PageUp", @(kVK_PageDown): @"PageDown",
            @(kVK_Home): @"Home", @(kVK_End): @"End",
            @(0x75): @"Delete", // kVK_ForwardDelete
            @(kVK_Delete): @"Backspace",
            @(kVK_Return): @"Return", @(kVK_Escape): @"Escape",
            @(kVK_Tab): @"Tab", @(kVK_Space): @"Space",
            @(kVK_F1): @"F1", @(kVK_F2): @"F2", @(kVK_F3): @"F3",
            @(kVK_F4): @"F4", @(kVK_F5): @"F5", @(kVK_F6): @"F6",
            @(kVK_F7): @"F7", @(kVK_F8): @"F8", @(kVK_F9): @"F9",
            @(kVK_F10): @"F10", @(kVK_F11): @"F11", @(kVK_F12): @"F12",
        };
    });
    return m;
}

/// 规范化用户/存档 chord; 空串 = 未绑定; 非法返回 nil。
static NSString *gl_normalize_chord(NSString *raw) {
    // 规范化规则单源在 core/chord.rs(经 FFI, 与 Windows 同一实现)。
    if (![raw isKindOfClass:[NSString class]]) return nil;
    char *out = glaspen2_chord_normalize(raw.UTF8String);
    if (!out) return nil;
    NSString *s = [NSString stringWithUTF8String:out];
    glaspen2_free_c_string(out);
    return s;
}

/// 保留组合: 按住类功能、系统锁屏、常用 App 默认不能被普通动作占用。
/// 列表单源在 core/ui.rs(经 FFI), 不再在 ObjC 里维护第二份。
static BOOL gl_shortcut_reserved(NSString *chord) {
    return glaspen2_chord_reserved(chord.UTF8String) != 0;
}

static NSMutableDictionary<NSString *, NSString *> *g_shortcut_values;
static NSMutableDictionary<NSString *, NSString *> *g_shortcut_lookup;

static void gl_shortcuts_apply(NSDictionary<NSString *, id> *saved);

/// 某镜头动作当前的修饰键位掩码; 鼠标滚轮/拖动按这套修饰键工作,
/// 改键后鼠标自动跟随(默认 ⌃⌥).
/// 未绑定/非法返回 0 —— 鼠标不控制镜头, 与键盘动作保持一致。
static NSUInteger gl_shortcut_modifier_mask_for(NSString *action) {
    if (!g_shortcut_values) gl_shortcuts_apply(@{});
    NSString *chord = g_shortcut_values[action];
    if (chord.length == 0) return 0;
    NSUInteger mask = 0;
    for (NSString *part in [chord componentsSeparatedByString:@"+"]) {
        if ([part isEqualToString:@"cmd"]) mask |= NSEventModifierFlagCommand;
        else if ([part isEqualToString:@"ctrl"]) mask |= NSEventModifierFlagControl;
        else if ([part isEqualToString:@"alt"]) mask |= NSEventModifierFlagOption;
        else if ([part isEqualToString:@"shift"]) mask |= NSEventModifierFlagShift;
    }
    return mask;
}
static BOOL g_hotkey_recording = NO;
static NSUInteger g_hotkey_recording_gen = 0;

/// 旧默认迁移单源在 core/chord.rs(D2): 历史默认表 + "只迁仍等于旧默认的项"。
static id gl_shortcut_migrate_saved(NSString *action, id raw) {
    if (![raw isKindOfClass:[NSString class]]) return raw;
    char *migrated = glaspen2_chord_migrate_value(action.UTF8String,
                                                  [(NSString *)raw UTF8String]);
    if (!migrated) return raw;
    NSString *s = [NSString stringWithUTF8String:migrated];
    glaspen2_free_c_string(migrated);
    return s ?: raw;
}

static void gl_shortcuts_apply(NSDictionary<NSString *, id> *saved) {
    NSDictionary<NSString *, NSString *> *defaults = gl_shortcut_defaults();
    NSMutableDictionary<NSString *, NSString *> *values = [NSMutableDictionary dictionary];
    NSMutableDictionary<NSString *, NSString *> *lookup = [NSMutableDictionary dictionary];
    for (NSString *action in gl_shortcut_actions()) {
        id raw = gl_shortcut_migrate_saved(action, saved[action]);
        NSString *chord = nil;
        if ([raw isKindOfClass:[NSString class]]) {
            chord = gl_normalize_chord(raw);
            if (!chord && [(NSString *)raw length] > 0) {
                NSLog(@"[hotkey] 非法组合 %@=%@, 回退默认", action, raw);
            }
        }
        if (!chord) chord = defaults[action]; // 缺失/非法 → 默认
        if (chord.length && gl_shortcut_reserved(chord)) {
            NSLog(@"[hotkey] 保留组合 %@=%@, 回退默认", action, chord);
            chord = defaults[action];
        }
        if (chord.length && lookup[chord]) {
            NSLog(@"[hotkey] 冲突 %@ 与 %@ 都是 %@; 后者解绑",
                  lookup[chord], action, chord);
            chord = @"";
        }
        values[action] = chord ?: @"";
        if (chord.length) lookup[chord] = action;
    }
    g_shortcut_values = values;
    g_shortcut_lookup = lookup;
}

/// 当前生效表(含默认补齐)的 JSON; settings_json / 面板回显用。
static NSString *gl_shortcuts_effective_json(void) {
    if (!g_shortcut_values) gl_shortcuts_apply(@{});
    NSData *d = [NSJSONSerialization dataWithJSONObject:g_shortcut_values
                                                options:0
                                                  error:nil];
    if (!d) return @"{}";
    return [[NSString alloc] initWithData:d encoding:NSUTF8StringEncoding] ?: @"{}";
}

/// 启动时从 user_settings 恢复; 必须在 event tap 安装前调用。
static void gl_shortcuts_load(void) {
    NSString *raw = gl_string_setting("shortcuts");
    NSDictionary *saved = nil;
    if (raw.length) {
        id obj = [NSJSONSerialization JSONObjectWithData:
                                     [raw dataUsingEncoding:NSUTF8StringEncoding]
                                                 options:0
                                                   error:nil];
        if ([obj isKindOfClass:[NSDictionary class]]) saved = obj;
    }
    gl_shortcuts_apply(saved);
}

/// 录制快捷键期间放行所有键盘事件给设置面板; 60s 兜底, 防止面板异常退出
/// 后全局热键永久失效。
static void gl_set_hotkey_recording(BOOL on) {
    g_hotkey_recording = on;
    NSUInteger gen = ++g_hotkey_recording_gen;
    if (!on) return;
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(60.0 * NSEC_PER_SEC)),
                   dispatch_get_main_queue(), ^{
        if (gen == g_hotkey_recording_gen && g_hotkey_recording) {
            g_hotkey_recording = NO;
            NSLog(@"[hotkey] 录制超时, 恢复全局快捷键");
        }
    });
}

/// 事件 tap 用: keycode + 修饰键 → 规范 chord → action id。
// S6-3-4b: 键名/查找薄桥(Swift EventRouter 调; 逻辑在 Swift)
NSString *glaspen2_router_keycode_name(unsigned short kc) {
    return gl_keycode_names()[@(kc)];
}

NSString *glaspen2_router_action_for_chord(NSString *chord) {
    if (!g_shortcut_lookup) gl_shortcuts_apply(@{});
    return g_shortcut_lookup[chord];
}

// S6/S7: 热键查找已直接由 Swift EventRouter 使用, 旧 ObjC 包装删除。

/// D12: 设置快照的运行时专属值(其余字段/缺省值/DB 回退单源在
/// core/settings_snapshot.rs)。这里只放不在 user_settings 或不能直接读 DB 的值:
/// - launchAtLogin: LaunchAgent plist 的真实状态;
/// - shortcuts: 生效表(含默认补齐/保留组合/冲突消解, 端点在 gl_shortcuts_*);
/// - flipEffect: 可被 GLASPEN2_FLIP_EFFECT 调试环境变量覆盖, 以运行时为准;
/// - language: 以运行时 g_lang 为准(DB 落库值由 core 兜底)。
/// 返回 malloc 的 UTF-8 JSON(调用方 free), 失败 NULL。
static char *gl_settings_snapshot_overrides_json(void) {
    NSDictionary *overrides = @{
        @"launchAtLogin": @((BOOL)(glaspen2_is_launch_at_login() != 0)),
        @"shortcuts": gl_shortcuts_effective_json(),
        @"flipEffect": @(g_flip_effect),
        @"language": (g_lang == 1 ? @"en" : @"zh"),
    };
    NSData *data = [NSJSONSerialization dataWithJSONObject:overrides options:0 error:nil];
    if (!data) return NULL;
    NSString *s = [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
    if (!s) return NULL;
    const char *cs = s.UTF8String;
    if (!cs) return NULL;
    size_t n = strlen(cs);
    char *buf = (char *)malloc(n + 1);
    if (!buf) return NULL;
    memcpy(buf, cs, n + 1);
    return buf;
}

/// 当前设置的 JSON 快照;调用方用 glaspen2_macos_free_c_string 释放。
///
/// D12: 字段集合/顺序/类型/缺省值/DB 回退唯一实现在 core
/// settings_snapshot::snapshot_json;本函数只收运行时专属值并转发 FFI。
/// FFI 永不返回 NULL;内部错误以 {"error": ...} 形式传给 api.rs(它会拒绝该帧,
/// 等价于旧实现"拿不到快照")。
char *glaspen2_macos_settings_json(void) {
    __block char *out = NULL;
    gl_run_on_main_sync(^{
        char *overrides = gl_settings_snapshot_overrides_json();
        if (!overrides) return;
        char *json = glaspen2_settings_snapshot_json(overrides);
        free(overrides);
        if (!json) return;
        size_t n = strlen(json);
        char *buf = (char *)malloc(n + 1);
        if (buf) memcpy(buf, json, n + 1);
        glaspen2_free_c_string(json);
        out = buf;
    });
    return out;
}

void glaspen2_macos_free_c_string(char *p) {
    if (p) free(p);
}

/// 释放 glaspen2_macos_canvas_payload 返回的缓冲区(ObjC 侧 malloc,
/// 不能交给 Rust 的 Vec::from_raw_parts)。
void glaspen2_macos_free_bytes(unsigned char *p) {
    if (p) free(p);
}

/// 手写消息集成总开关:关 = 热键直通、面板收起;顺带终止进行中的手写会话。
/// (共享画布上行的关闭由 Dart 侧随 tab 收起调用 FRB shareInkSetActive(false)。)
static void gl_settings_set_chat_integration(BOOL on) {
    // S7w: 逻辑已迁 Swift(MenuHandler.setChatIntegration)。
    [MenuHandler setChatIntegration:on];
}

/// 写入一项设置(D1:解析 / 钳制 / 落库单源在 core)。
///
/// core `settings_apply::apply_setting` 完成宽容解析(面板历史上发过裸标量
/// true / 3 / 2.5 / 数字字符串)+ 区间钳制 + user_settings 落库, 返回平台无关
/// effect JSON: {"kind":"GridSize","float":40} / {"kind":"Shortcuts","json":{...}},
/// 失败返回 {"error":"..."}。本函数只按 kind 执行 macOS 运行时副作用(菜单/
/// 窗口/重绘/向主线程投任务), 不再自己解析标量、不再逐键钳制或落库。
void glaspen2_macos_set_setting(const char *key_c, const char *value_json) {
    if (!key_c || !value_json) return;
    char *effect_c = glaspen2_settings_apply(key_c, value_json);
    if (!effect_c) return;
    NSData *effect_data = [NSData dataWithBytes:effect_c length:strlen(effect_c)];
    glaspen2_free_c_string(effect_c);
    id parsed = [NSJSONSerialization JSONObjectWithData:effect_data options:0 error:nil];
    if (![parsed isKindOfClass:[NSDictionary class]]) return;
    NSDictionary *effect = parsed;
    NSString *kind = effect[@"kind"];
    if (![kind isKindOfClass:[NSString class]]) {
        // 未知/只读键、值类型不合法、invertFps 不在白名单等: 记录并跳过,
        // 等价于旧实现的“这一项不生效”, 但不再静默。
        NSLog(@"[settings] %s 应用失败: %@", key_c, effect[@"error"]);
        return;
    }
    // 每个 kind 只有一个载荷字段(core SettingEffect::to_json 保证); 取错字段
    // 只会得到 0/空, 不会崩。全部在进入主线程前取好, block 只捕获值。
    BOOL boolValue = [effect[@"bool"] boolValue];
    NSInteger intValue = [effect[@"int"] integerValue];
    double floatValue = [effect[@"float"] doubleValue];
    NSString *strValue = [effect[@"str"] isKindOfClass:[NSString class]] ? effect[@"str"] : @"";
    id jsonValue = effect[@"json"];

    gl_run_on_main_sync(^{
        // ── 纯面板设置:core 已落库, macOS 运行时无副作用 ──
        if ([kind isEqualToString:@"BubbleAutoNewPage"] ||
            [kind isEqualToString:@"BubbleReplayAuto"] ||
            [kind isEqualToString:@"BubbleReplayHoldLast"] ||
            [kind isEqualToString:@"BubbleReplaySpeed"] ||
            [kind isEqualToString:@"NasUrl"] ||
            [kind isEqualToString:@"NotebookStyle"] ||
            [kind isEqualToString:@"ShowFreeCanvas"]) {
            return;
        }
        if ([kind isEqualToString:@"Color"]) {
            gl_settings_set_color((int)intValue);
        } else if ([kind isEqualToString:@"Width"]) {
            gl_settings_set_width((int)intValue);
        } else if ([kind isEqualToString:@"LaunchAtLogin"]) {
            gl_settings_set_launch(boolValue);
        } else if ([kind isEqualToString:@"FrostedGlass"]) {
            gl_settings_set_glass_enabled(boolValue);
        } else if ([kind isEqualToString:@"Grid"]) {
            gl_settings_set_grid(boolValue);
        } else if ([kind isEqualToString:@"GridFollowStrokes"]) {
            g_grid_follow_strokes = boolValue;
            if (g_draw_view) [g_draw_view setNeedsDisplay:YES];
        } else if ([kind isEqualToString:@"GlassFollowStrokes"]) {
            g_glass_follow_strokes = boolValue;
            gl_glass_apply(); // 立即按新规则重估玻璃可见性(含飘渺隐藏态)
        } else if ([kind isEqualToString:@"SoftShadow"]) {
            g_soft_shadow = boolValue;
            glaspen2_set_soft_shadow(boolValue ? 1 : 0);
            if (g_draw_view) [g_draw_view setNeedsDisplay:YES];
        } else if ([kind isEqualToString:@"InvertInk"]) {
            g_invert_ink = boolValue;
            invert_ink_apply(boolValue ? 1 : 0);
        } else if ([kind isEqualToString:@"InvertFps"]) {
            g_invert_fps = (int)intValue;
            if (g_invert_ink) invert_stream_restart(); // 用新帧率重启捕获流
        } else if ([kind isEqualToString:@"PanelTransparent"]) {
            gl_panel_transparent_apply_full(boolValue);
        } else if ([kind isEqualToString:@"Outline"]) {
            apply_outline(boolValue);
        } else if ([kind isEqualToString:@"InfiniteCanvas"]) {
            apply_infinite_canvas(boolValue, NO);
        } else if ([kind isEqualToString:@"Minimap"]) {
            g_minimap_enabled = boolValue;
            if (g_draw_view) [g_draw_view setNeedsDisplay:YES];
        } else if ([kind isEqualToString:@"GridSize"]) {
            g_grid_size = floatValue;
            if (g_draw_view) [g_draw_view setNeedsDisplay:YES];
        } else if ([kind isEqualToString:@"GridDivider"]) {
            g_grid_divider = (int)intValue;
            if (g_draw_view) [g_draw_view setNeedsDisplay:YES];
        } else if ([kind isEqualToString:@"FlipEffect"]) {
            g_flip_effect = (int)intValue;
        } else if ([kind isEqualToString:@"PressureMonitor"]) {
            gl_settings_set_pressure_monitor(boolValue);
        } else if ([kind isEqualToString:@"GifFps"]) {
            g_gif_fps = (int)intValue;
        } else if ([kind isEqualToString:@"GifResolution"]) {
            g_gif_resolution = floatValue;
        } else if ([kind isEqualToString:@"GifSpeed"]) {
            g_gif_speed = floatValue;
        } else if ([kind isEqualToString:@"GifEndMode"]) {
            g_gif_end_mode = (int)intValue;
        } else if ([kind isEqualToString:@"ChatApiBase"] ||
                   [kind isEqualToString:@"ChatUser"]) {
            // DB 已由 core 写入(chat_api_base / chat_user), 这里只重载 auth 缓存
            glaspen2_chat_auth_reload();
        } else if ([kind isEqualToString:@"ChatPassword"]) {
            // 密码本体不进 settings 快照, core 不落库: 空串 = 保持已存密码
            if (strValue.length > 0) {
                glaspen2_save_string_setting("chat_password", strValue.UTF8String);
                glaspen2_chat_auth_reload();
            }
        } else if ([kind isEqualToString:@"ChatIntegration"]) {
            gl_settings_set_chat_integration(boolValue);
        } else if ([kind isEqualToString:@"ShowBubbleTab"]) {
            // tab 关闭时同时关掉气泡捕获: 不占笔事件 CPU、不落库(捕获状态是运行态)
            glaspen2_bubble_set_capture(boolValue ? 1 : 0);
        } else if ([kind isEqualToString:@"ShareCanvas"]) {
            // 共享上行开关: 仅集成开启时真正生效(macOS 旧语义: 关闭时即便集成
            // 已关也要确保流停掉)
            if (g_chat_integration || !boolValue) {
                glaspen2_share_ink_set_active((g_chat_integration && boolValue) ? 1 : 0);
            }
        } else if ([kind isEqualToString:@"Language"]) {
            // core 已把落库值归一为 en/zh; 这里同步原生菜单并通知面板
            g_lang = [strValue isEqualToString:@"en"] ? 1 : 0;
            update_menu_texts();
            glaspen2_notify_settings_changed();
        } else if ([kind isEqualToString:@"Shortcuts"]) {
            // 规范化/补默认/挡冲突仍在 gl_shortcuts_apply; 落库值 = 规范化生效表
            // (core 先按原表写了一次, 这里用生效表覆盖, 与旧实现最终 DB 值一致)
            NSDictionary *map =
                [jsonValue isKindOfClass:[NSDictionary class]] ? jsonValue : @{};
            gl_shortcuts_apply(map);
            NSString *json = gl_shortcuts_effective_json();
            if (json.length) glaspen2_save_string_setting("shortcuts", json.UTF8String);
            sync_settings_panel();
        } else if ([kind isEqualToString:@"HotkeyRecording"]) {
            // 录制期间放行键盘给面板; 不落库, 超时自动恢复
            gl_set_hotkey_recording(boolValue);
        } else {
            // core 新增了 effect 但这里忘了执行: 记录, 方便 D1a 闸门之外的人工排障
            NSLog(@"[settings] %s: 未处理的 effect kind=%@", key_c, kind);
        }
    });
}

/// 删除一页;返回 1 表示成功。删除当前页时自动切到相邻页。
int glaspen2_macos_delete_page(long long screen_id) {
    __block int ok = 0;
    // delete_screen 会把 current 置 0; 邻居必须在**删除前**按当前页取好,
    // 否则删完 current=0, next/prev 都查不到, 外壳永远进不了切页分支。
    BOOL was_current = (screen_id == glaspen2_get_current_screen_id());
    int64_t next_id = was_current ? glaspen2_next_screen_id() : 0;
    int64_t prev_id = was_current ? glaspen2_prev_screen_id() : 0;
    dispatch_sync(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
        ok = glaspen2_delete_screen(screen_id);
        gl_run_on_main_sync(^{
            if (g_infinite_canvas) {
                // 无限画布独立存储,不受页存储的删除影响
                rebuild_surface_from_strokes();
                return;
            }
            if (ok && was_current) {
                int64_t target = next_id != 0 ? next_id : prev_id;
                if (target != 0) {
                    glaspen2_load_strokes_for_screen(target);
                    pageview_update(); // 相邻页几何可能 ≠ 屏幕
                    rebuild_surface_from_strokes();
                } else {
                    clear_screen();
                }
            }
        });
    });
    return ok;
}

/// 面板窗口真穿透(实验): 拟物玻璃卡片的"透明窟窿"需要窗口本身可透明。
/// 只动 NSWindow(不透明→透明+清底); Flutter view 的 layer 部分在 panel.m。
void gl_panel_transparent_apply(BOOL on) {
    if (!g_settings_window) return;
    gl_run_on_main_sync(^{
        [g_settings_window setOpaque: !on];
        [g_settings_window setBackgroundColor: on ? [NSColor clearColor]
                                                  : [NSColor windowBackgroundColor]];
    });
}

/// 面板页面详情改动了当前页的数据(圈选移动/粘贴/删除) → 重建玻璃。
/// core 的 refresh hook 在后台线程调用; surface 操作必须回主线程。
void glaspen2_macos_refresh_page(long long screen_id) {
    if (screen_id <= 0) return;
    gl_run_on_main_sync(^{
        if (g_infinite_canvas) return; // 无限画布不受页数据操作影响
        if (screen_id != glaspen2_get_current_screen_id()) return;
        glaspen2_load_strokes_for_screen(screen_id);
        rebuild_surface_from_strokes();
        flush_to_layer();
    });
}

/// 面板自由涂鸦编辑(圈选移动/粘贴/删除)改动了无限画布 → 重载并重建玻璃。
void glaspen2_macos_refresh_infinite(void) {
    gl_run_on_main_sync(^{
        if (!g_infinite_canvas) return;
        glaspen2_load_infinite_strokes();
        rebuild_surface_from_strokes();
        flush_to_layer();
    });
}

/// 跳转到指定页继续绘画。
void glaspen2_macos_navigate_to_page(long long screen_id) {
    if (screen_id <= 0) return;
    // 从无限画布跳转到某一页:先切回翻页模式(否则会把页笔迹塞进无限画布内存)
    if (g_infinite_canvas) {
        gl_run_on_main_sync(^{
            apply_infinite_canvas(NO, NO);
        });
    }
    dispatch_sync(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
        glaspen2_load_strokes_for_screen(screen_id);
        pageview_update();
        gl_run_on_main_sync(^{
            rebuild_surface_from_strokes();
        });
    });
}

/// 设置面板快捷键按钮 → 执行与物理快捷键相同的动作。
void glaspen2_macos_hotkey(const char *key_c) {
    if (!key_c) return;
    NSString *key = @(key_c);
    gl_run_on_main_sync(^{
        // Q(彻底退出)不走 perform_hotkey: 退出是独立动作, 且必须和菜单「退出」
        // 共用同一条收尾路径(⌘⌃Q 本体留给 macOS 锁屏, 见 gl_shortcut_defaults)。
        if ([key isEqualToString:@"Q"]) {
            gl_quit_app();
            return;
        }
        unsigned short kc = 0;
        if ([key isEqualToString:@"G"]) kc = kVK_ANSI_G;
        else if ([key isEqualToString:@"`"]) kc = 0x32; // ` — 上一页
        else if ([key isEqualToString:@"1"]) kc = 0x12; // 1 — 下一页
        else if ([key isEqualToString:@"Z"]) kc = kVK_ANSI_Z;
        else if ([key isEqualToString:@"X"]) kc = kVK_ANSI_X;
        else if ([key isEqualToString:@"C"]) kc = kVK_ANSI_C;
        else if ([key isEqualToString:@"V"]) kc = kVK_ANSI_V;
        else if ([key isEqualToString:@"B"]) kc = kVK_ANSI_B;
        if (kc) perform_hotkey(kc);
    });
}

/// 无限画布总览:action 0=当前 1=镜头回原点 2=居中内容 3=新建(清空)。
/// 返回 PNG 缓冲区(用 glaspen2_macos_free_bytes 释放)。rect_out 必须指向
/// 8 个 double: [0..3] 视口矩形, [4..7] 渲染内容包围盒 [bx,by,bw,bh]
/// (面板编辑用:图片像素 → 画布坐标的映射)。
/// 空画布返回 NULL 且 *out_len = 0。
unsigned char *glaspen2_macos_canvas_payload(int w, int h, int action, double *rect_out,
                                             int *out_len) {
    __block unsigned char *result_png = NULL;
    gl_run_on_main_sync(^{
        // block 捕获的参数是只读副本,尺寸用局部变量调整
        int ow = w, oh = h;
        if (ow < 400) ow = 1024;
        if (oh < 400) oh = 768;
        if (action == 1) {
            // 镜头回原点 + 100%(HOME 常量单源 core Camera::HOME, D5b)
            glaspen2_camera_home(&g_pan_x, &g_pan_y, &g_zoom);
            canvas_apply_transform();
        } else if (action == 2) {
            double bx, by, bx2, by2;
            if (glaspen2_stroke_bbox(&bx, &by, &bx2, &by2)) {
                // 居中公式单源 core Camera::centered_on(D5b)。macOS 只改 pan:
                // assign_zoom = NO 保留当前 g_zoom;Windows 会把兜底后的 zoom
                // 一并写回(差异见 camera.rs 模块文档 5)。
                NSDictionary *result = camera_result_json(glaspen2_camera_centered(
                    bx, by, bx2, by2, (double)g_screen_w, (double)g_screen_h,
                    g_zoom));
                if (camera_assign(result, NO))
                    canvas_apply_transform();
            }
        } else if (action == 3) {
            finish_active_stroke();
            glaspen2_clear_strokes(g_screen_w, g_screen_h);
            glaspen2_camera_home(&g_pan_x, &g_pan_y, &g_zoom);
            glaspen2_set_view_transform(g_pan_x, g_pan_y, g_zoom);
            rebuild_surface_from_strokes();
            ow = 1024;
            oh = 768;
        }
        double bounds[4] = {0};
        NSDictionary *payload = canvas_overview_payload(ow, oh, bounds);
        if (!payload) return;
        NSData *data = payload[@"png"];
        NSArray *rect = payload[@"rect"];
        if (!data || data.length == 0) return;
        unsigned char *buf = (unsigned char *)malloc(data.length);
        if (!buf) return;
        memcpy(buf, data.bytes, data.length);
        result_png = buf;
        if (out_len) *out_len = (int)data.length;
        if (rect_out) {
            if (rect.count == 4) {
                for (int i = 0; i < 4; i++) rect_out[i] = [rect[i] doubleValue];
            }
            for (int i = 0; i < 4; i++) rect_out[4 + i] = bounds[i];
        }
    });
    return result_png;
}

int glaspen2_macos_export_pdf(void) {
    __block int ok = 0;
    dispatch_sync(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
        ok = glaspen2_export_pdf();
    });
    return ok;
}

/// 导出动画 GIF:后台渲染,完成后回主线程复制到剪贴板并提示。
int glaspen2_macos_export_gif(void) {
    __block int ok = 0;
    dispatch_sync(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
        ok = glaspen2_save_animated_gif(g_gif_fps, g_gif_resolution, g_gif_speed,
                                        g_gif_end_mode);
        gl_run_on_main_sync(^{
            if (ok) {
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
                show_notification(
                    L(@"动画 GIF 已保存并复制到剪贴板", @"Animated GIF saved & copied"));
            } else {
                show_notification(gl_text(@"recording.gif_no_strokes_or_export_failed"));
            }
        });
    });
    return ok;
}

// --- Unified settings functions (single source of truth) ---

static void gl_settings_set_color(int idx) {
    // S7i: 逻辑已迁 Swift(MenuHandler.setColorIndex)。
    [MenuHandler setColorIndex:idx];
}

static void gl_settings_set_width(int idx) {
    // S7h: 逻辑已迁 Swift(MenuHandler.setWidthIndex)。
    [MenuHandler setWidthIndex:idx];
}

/// 抬笔后应用书写中按下的换色/换粗细(只保留最后一次)。
static void gl_settings_apply_pending_color_width(void) {
    // S7: 逻辑已迁 Swift(MenuHandler.applyPendingColorWidth)。
    [MenuHandler applyPendingColorWidth];
}

static void gl_settings_set_grid(BOOL on) {
    // S7t: 逻辑已迁 Swift(MenuHandler.setGridEnabled)。
    [MenuHandler setGridEnabled:on];
}

static void gl_settings_set_launch(BOOL on) {
    // S7k: 逻辑已迁 Swift(MenuHandler.setLaunchAtLogin)。
    [MenuHandler setLaunchAtLogin:on];
}

static void gl_glass_apply(void) {
    // Combine enabled + opacity into visual effect
    // 飘渺模式且「玻璃跟随涂鸦」开: 笔迹隐藏时玻璃一起藏(历史行为);
    // 关则玻璃只听自己的开关, 常驻背景。
    BOOL hidden_by_ethereal =
        g_ethereal_canvas && g_glass_follow_strokes && !g_strokes_visible;
    double visual = g_glass_enabled ? g_glass_opacity : 0.0;
    if (g_glass_view) {
        g_glass_view.alphaValue = visual * 2.0; // map to visible range
        g_glass_view.hidden = !g_glass_enabled || hidden_by_ethereal;
    }
    NSMenuItem *gi = [MenuLookup itemWithTag:444 inMenu:g_menu];
    [gi setState:g_glass_enabled ? NSControlStateValueOn : NSControlStateValueOff];
}

static void gl_settings_set_glass_enabled(BOOL on) {
    // S7j: 逻辑已迁 Swift(MenuHandler.setGlassEnabled)。
    [MenuHandler setGlassEnabled:on];
}

static void gl_settings_set_enabled(BOOL on) {
    // S7v: 逻辑已迁 Swift(MenuHandler.setDrawEnabled)。
    [MenuHandler setDrawEnabled:on];
}

// --- Pressure monitor (top-left info window) ---

static void pm_ensure_window(void) {
    if (g_pm_window) return;
    NSScreen *screen = [NSScreen mainScreen];
    NSRect screenFrame = [screen frame];
    NSRect frame = NSMakeRect(10, screenFrame.size.height - 40, 220, 30);
    g_pm_window = [[NSWindow alloc] initWithContentRect:frame
                                              styleMask:NSWindowStyleMaskBorderless
                                                backing:NSBackingStoreBuffered
                                                  defer:NO];
    [g_pm_window setLevel:kCGMaximumWindowLevel];
    [g_pm_window setOpaque:YES];
    [g_pm_window setBackgroundColor:[NSColor colorWithWhite:0.12 alpha:1.0]];
    [g_pm_window setIgnoresMouseEvents:YES];
    [g_pm_window setTitle:@"Pressure"];
    [g_pm_window setReleasedWhenClosed:NO];

    g_pm_label = [[NSTextField alloc] initWithFrame:NSMakeRect(4, 2, 212, 26)];
    g_pm_evtype = nil;
    [g_pm_label setStringValue:@"P=-----  ---  (---,---)"];
    [g_pm_label setTextColor:[NSColor whiteColor]];
    [g_pm_label setFont:[NSFont fontWithName:@"Menlo" size:12]];
    [g_pm_label setBezeled:NO];
    [g_pm_label setDrawsBackground:NO];
    [g_pm_label setEditable:NO];
    [g_pm_label setSelectable:NO];
    [[g_pm_window contentView] addSubview:g_pm_label];
}

static void pm_show(void) {
    pm_ensure_window();
    [g_pm_window orderFront:nil];
}

static void pm_hide(void) {
    if (g_pm_window) {
        [g_pm_window orderOut:nil];
    }
}

static void pm_update(void) {
    if (!g_pm_window || !g_pm_label) return;
    // S7y: 标签格式化已迁 Swift(MenuHandler.updatePressureLabel)。
    [MenuHandler updatePressureLabelWithPressure:g_pm_pressure
                                            type:g_pm_evtype
                                               x:g_pm_x
                                               y:g_pm_y];
}

static void pm_destroy(void) {
    if (g_pm_window) {
        [g_pm_window close];
        g_pm_window = nil;
        g_pm_label = nil;
    }
}

static void gl_settings_set_pressure_monitor(BOOL on) {
    // S7u: 逻辑已迁 Swift(MenuHandler.setPressureMonitor)。
    [MenuHandler setPressureMonitor:on];
}

static void sync_settings_panel(void) {
    // 菜单/快捷键改了状态 → 推给订阅了 FRB 设置流的设置面板
    glaspen2_notify_settings_changed();
}


// S7: 暂存颜色/粗细状态薄桥(变量在 menu.m)
int glaspen2_router_pending_color(void) { return g_pending_color_index; }
int glaspen2_router_pending_width(void) { return g_pending_width_index; }
void glaspen2_router_clear_pending_color(void) { g_pending_color_index = -1; }
void glaspen2_router_clear_pending_width(void) { g_pending_width_index = -1; }

// S7h: 粗细设置的 Swift 薄桥(状态/刷新函数都在 menu.m)
int glaspen2_router_width_preset_count(void) { return g_width_preset_count; }
int glaspen2_router_selected_width(void) { return g_selected_width_index; }
void glaspen2_router_set_prev_width(int idx) { g_prev_width_index = idx; }
void glaspen2_router_set_pending_width(int idx) { g_pending_width_index = idx; }
void glaspen2_router_apply_width(int idx) {
  g_width_scale = g_width_presets[idx];
  g_selected_width_index = idx;
}
void glaspen2_router_save_pen_settings(void) {
  glaspen2_save_settings(g_pen_r, g_pen_g, g_pen_b, g_width_scale);
}
void glaspen2_router_update_checkmarks(void) { update_menu_checkmarks(); }
void glaspen2_router_sync_settings_panel(void) { sync_settings_panel(); }

// S7i: 颜色设置的 Swift 薄桥
int glaspen2_router_color_preset_count(void) { return g_color_preset_count; }
int glaspen2_router_selected_color(void) { return g_selectedColorIndex; }
void glaspen2_router_set_prev_color(int idx) { g_prev_color_index = idx; }
void glaspen2_router_set_pending_color(int idx) { g_pending_color_index = idx; }
void glaspen2_router_apply_color(int idx) {
  g_pen_r = g_color_presets[idx].r;
  g_pen_g = g_color_presets[idx].g;
  g_pen_b = g_color_presets[idx].b;
  g_selectedColorIndex = idx;
}

// S7j: 磨砂玻璃开关的 Swift 薄桥
void glaspen2_router_set_glass_state(int on) { g_glass_enabled = on != 0; }
void glaspen2_router_glass_apply(void) { gl_glass_apply(); }

// S7k: 菜单项勾选状态薄桥(按 tag 查菜单)
void glaspen2_router_update_menu_state(int tag, int state) {
  NSMenuItem *item = [MenuLookup itemWithTag:tag inMenu:g_menu];
  [item setState:state ? NSControlStateValueOn : NSControlStateValueOff];
}

// S7t: 网格开关状态薄桥
void glaspen2_router_set_grid_state(int on) { g_show_grid = on != 0; }

// S7u: 压感监视器开关的 Swift 薄桥
void glaspen2_router_set_pressure_monitor(int on) { g_pressure_monitor = on != 0; }
void glaspen2_router_pm_show(void) { pm_show(); }
void glaspen2_router_pm_hide(void) { pm_hide(); }
void glaspen2_router_pm_refresh(void) { pm_update(); }

// S7v: 涂鸦总开关的 Swift 薄桥
void glaspen2_router_set_enabled_state(int on) { g_enabled = on != 0; }
void glaspen2_router_set_menu_title(int tag, NSString *title) {
  NSMenuItem *item = [MenuLookup itemWithTag:tag inMenu:g_menu];
  if (item) [item setTitle:title];
}

// S7w: 手写消息总开关状态薄桥
void glaspen2_router_set_chat_integration(int on) { g_chat_integration = on != 0; }

// S7y: 压感监视器标签设置薄桥
void glaspen2_router_set_pm_label(NSString *text) {
  if (g_pm_label) [g_pm_label setStringValue:text];
}
