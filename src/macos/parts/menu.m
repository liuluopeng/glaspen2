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
static void update_menu_texts(void) { [MenuTextUpdater update]; }

// ── 颜色/粗细菜单条目的富文本:内嵌小图标 + 名称 ──
// NSMenuItem.image 在新版系统菜单里不渲染(实测菜单全是字), 而正文里
// 内嵌附件图(attributedTitle + NSTextAttachment)任何系统版本都画。
// 颜色 = 圆点色块(一眼可见);粗细 = 横线粗细(粗细预设的直观映射)。

// 颜色/粗细显示名已迁 Swift(MenuNames); 文案仍由 ObjC gl_text/core i18n 决定。
// S3b-2b-4b: Swift 经 Bridging.h 的 C 桥按稳定 key 取名。
NSString *glaspen2_menu_l10n(NSString *key) {
    return gl_text(key);
}

static NSAttributedString* gl_color_item_text(NSInteger idx) { return [MenuPresetTitles colorItemTextWithIndex:(int)idx]; }

static NSAttributedString* gl_width_item_text(NSInteger idx) { return [MenuPresetTitles widthItemTextWithIndex:(int)idx]; }

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


static void update_menu_checkmarks(void) { [MenuGlue updateCheckmarks]; }

static void toggle_enabled(void) { [DrawMode toggleEnabled]; }

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
static void ethereal_hide_now(void) { (void)[EtherealMode hideNow]; }

/// 飘渺隐藏: 笔离开数位板立即隐藏(无动效, 无延迟)。
static BOOL auto_hide_now(void) { return [EtherealMode hideNow] ? YES : NO; }

/// 启动恢复飘渺模式(run.m 在视图就绪后调用)。hideNow 以"笔迹可见"为前置,
/// 先归位再落位, 落位路径与 toggle 进入飘渺完全一致(玻璃跟随/压强计一并处理)。
void gl_ethereal_restore_now(void) { [EtherealMode restoreNow]; }

// Show the strokes because the pen came back (飘渺 mode).
static void auto_show_canvas(void) { [EtherealMode autoShow]; }

// ── Page-navigation / lens peek (ethereal mode) ──
// 翻页载入笔迹、无限画布平移/缩放镜头时, 笔迹是隐藏的; 短暂显示让用户
// 看见新页面/新镜头, 然后按 `seconds` 隐回去(笔互动会取消隐藏计时器)。

static void peek_cancel_timer(void) { [EtherealMode cancelTimer]; }

/// 飘渺模式下镜头平移/缩放时, 笔迹本身是隐藏的, 用户看不到画面变化。
/// 短暂 peek 一下, 让新镜头下的笔迹/网格露出来再隐回去。
///
/// 笔正悬停(笔迹已经可见、且没有 peek 计时器)时不要启动隐藏计时器,
/// 否则悬浮看画会被 1 秒后自动隐藏。已有 peek 计时器时重新计时, 让连续
/// 平移/缩放期间笔迹保持可见, 松手后再隐。
static void peek_lens_change(void) { [EtherealMode peekLensChange]; }

// Switch between 固定画布涂鸦模式 and 飘渺画布涂鸦模式. Shortcut: ⌘ + ⌃ + X
static void toggle_canvas_mode(void) { [DrawMode toggleCanvasMode]; }

/// 彻底退出:菜单「退出」、可配置快捷键 quit、设置面板/自动更新流程共用**同一条**
/// 路径 —— 先记性能汇总、把光标还给系统, 再 terminate。terminate 会同步走
/// applicationShouldTerminate 把排队点写完并 checkpoint WAL(见下), 进程里
/// 没有其它需要单独收尾的子进程(设置面板是内嵌 Flutter 视图, 不是独立进程)。
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
    glaspen2_router_update_status_icon_state();
    update_menu_checkmarks();
}

void glaspen2_menu_did_close_bridge(void) {
    glaspen2_router_update_status_icon_state();
    // Re-enable CGEventTap after menu closes
    if (g_event_tap) {
        CGEventTapEnable(g_event_tap, true);
    }
}

void glaspen2_menu_quit_bridge(void) {
    [MenuHandler quitAppNow];
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
    [MenuHandler setLaunchAtLogin:(on != 0)];
}

int glaspen2_menu_glass_enabled(void) {
    return g_glass_enabled ? 1 : 0;
}

void glaspen2_menu_set_glass(int on) {
    [MenuHandler setGlassEnabled:(on != 0)];
}

int glaspen2_menu_outline_enabled(void) {
    return g_outline_enabled ? 1 : 0;
}

void glaspen2_menu_apply_outline(int on) {
    [MenuHandler setOutlineEnabled:(on != 0)];
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
void glaspen2_menu_select_color(int idx) { [MenuHandler setColorIndex:idx]; }
void glaspen2_menu_select_width(int idx) { [MenuHandler setWidthIndex:idx]; }
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
static void gl_run_on_main_sync(dispatch_block_t block) { [SettingsGlue runOnMainSync:block]; }

/// 读一条字符串设置(DB),转成 NSString(未设置为空串)。调用方持有返回值。
/// 仅用于 settings_json 等非热路径;SQLite 单行读取在主线程也足够快。
// ── 可配置快捷键 ─────────────────────────────────────────────────────
// 动作 id 与默认组合; 真正的动作执行在 run.m 的 perform_action()。本区只
// 负责 "chord → action" 映射、持久化、录制期间暂停全局拦截。
//
// chord 规范: cmd+ctrl+alt+shift+<Key>，顺序固定; 至少包含 ctrl 或 alt
// (只按 ⌘/⇧ 的组合容易和 App 菜单/系统快捷键冲突, 且录制时会被菜单先吃掉)。
// 空串 = 该动作未绑定。按键名走物理位置(与布局无关), 与事件 tap 的
// kVK_* keycode 一一对应。
static NSDictionary<NSString *, NSString *> *gl_shortcut_defaults(void) { return (NSDictionary *)[ShortcutTable defaults]; }

/// keycode → 规范按键名。只收录能当全局快捷键的键; 其它键录制时拒绝。
/// 规范化用户/存档 chord; 空串 = 未绑定; 非法返回 nil。
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
/// 旧默认迁移单源在 core/chord.rs(D2): 历史默认表 + "只迁仍等于旧默认的项"。
static void gl_shortcuts_apply(NSDictionary<NSString *, id> *saved) { [ShortcutApply applySaved:saved]; }

/// 当前生效表(含默认补齐)的 JSON; settings_json / 面板回显用。
static NSString *gl_shortcuts_effective_json(void) { return [SettingsGlue effectiveShortcutJSON]; }

/// 启动时从 user_settings 恢复; 必须在 event tap 安装前调用。
/// 录制快捷键期间放行所有键盘事件给设置面板; 60s 兜底, 防止面板异常退出
/// 后全局热键永久失效。
/// 事件 tap 用: keycode + 修饰键 → 规范 chord → action id。
// S6-3-4b: 键名/查找薄桥(Swift EventRouter 调; 逻辑在 Swift)
NSString *glaspen2_router_keycode_name(unsigned short kc) {
    return [ShortcutNames nameForKeyCode:kc];
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
/// 当前设置的 JSON 快照;调用方用 glaspen2_macos_free_c_string 释放。
///
/// D12: 字段集合/顺序/类型/缺省值/DB 回退唯一实现在 core
/// settings_snapshot::snapshot_json;本函数只收运行时专属值并转发 FFI。
/// FFI 永不返回 NULL;内部错误以 {"error": ...} 形式传给 api.rs(它会拒绝该帧,
/// 等价于旧实现"拿不到快照")。
char *glaspen2_macos_settings_json(void) { return (char *)[MacosApi settingsJSON]; }

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
/// 写入一项设置(D1:解析 / 钳制 / 落库单源在 core)。
///
/// core `settings_apply::apply_setting` 完成宽容解析(面板历史上发过裸标量
/// true / 3 / 2.5 / 数字字符串)+ 区间钳制 + user_settings 落库, 返回平台无关
/// effect JSON: {"kind":"GridSize","float":40} / {"kind":"Shortcuts","json":{...}},
/// 失败返回 {"error":"..."}。本函数只按 kind 执行 macOS 运行时副作用(菜单/
/// 窗口/重绘/向主线程投任务), 不再自己解析标量、不再逐键钳制或落库。
void glaspen2_macos_set_setting(const char *key_c, const char *value_json) {
  if (!key_c || !value_json) return;
  [SettingsDispatcher applyWithKey:[NSString stringWithUTF8String:key_c]
                             value:[NSString stringWithUTF8String:value_json]];
}

/// 删除一页;返回 1 表示成功。删除当前页时自动切到相邻页。
int glaspen2_macos_delete_page(long long screen_id) { return [MacosApi deletePageWithScreenId:screen_id]; }

/// 面板窗口真穿透(实验): 拟物玻璃卡片的"透明窟窿"需要窗口本身可透明。
/// 只动 NSWindow(不透明→透明+清底); Flutter view 的 layer 部分在 panel.m。
void gl_panel_transparent_apply(BOOL on) { [SettingsPanelExtras applyPanelTransparent:on]; }

/// 面板页面详情改动了当前页的数据(圈选移动/粘贴/删除) → 重建玻璃。
/// core 的 refresh hook 在后台线程调用; surface 操作必须回主线程。
void glaspen2_macos_refresh_page(long long screen_id) { [MacosApi refreshPageWithScreenId:screen_id]; }

/// 面板自由涂鸦编辑(圈选移动/粘贴/删除)改动了无限画布 → 重载并重建玻璃。
void glaspen2_macos_refresh_infinite(void) { [MacosApi refreshInfinite]; }

/// 跳转到指定页继续绘画。
void glaspen2_macos_navigate_to_page(long long screen_id) { [MacosApi navigateToPageWithScreenId:screen_id]; }

/// 设置面板快捷键按钮 → 执行与物理快捷键相同的动作。
void glaspen2_macos_hotkey(const char *key_c) { if (!key_c) return; [MacosApi hotkeyWithKey:[NSString stringWithUTF8String:key_c]]; }

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
            [CanvasCamera home];
        } else if (action == 2) {
            double bx, by, bx2, by2;
            if (glaspen2_stroke_bbox(&bx, &by, &bx2, &by2)) {
                // 居中公式单源 core Camera::centered_on(D5b)。macOS 只改 pan:
                // assign_zoom = NO 保留当前 g_zoom;Windows 会把兜底后的 zoom
                // 一并写回(差异见 camera.rs 模块文档 5)。
                NSDictionary *result =
                    [CameraState resultFromJSON:glaspen2_camera_centered(
                        bx, by, bx2, by2, (double)g_screen_w, (double)g_screen_h,
                        g_zoom)];
                if ([CameraState assignResult:result assignZoom:NO])
                    [CanvasCamera applyTransform];
            }
        } else if (action == 3) {
            [EventRouter finishActiveStroke];
            glaspen2_clear_strokes(g_screen_w, g_screen_h);
            [CanvasCamera home];
            ow = 1024;
            oh = 768;
        }
        NSDictionary *payload = [CanvasOverview payloadWithWidth:ow height:oh];
        if (!payload) return;
        NSData *data = payload[@"png"];
        NSArray *rect = payload[@"rect"];
        NSArray *boundsArr = payload[@"bounds"];
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
            for (int i = 0; i < 4 && i < (int)boundsArr.count; i++)
                rect_out[4 + i] = [boundsArr[i] doubleValue];
        }
    });
    return result_png;
}

int glaspen2_macos_export_pdf(void) { return [MacosApi exportPDF]; }

/// 导出动画 GIF:后台渲染,完成后回主线程复制到剪贴板并提示。
int glaspen2_macos_export_gif(void) { return [MacosApi exportGIF]; }

// --- Unified settings functions (single source of truth) ---

/// 抬笔后应用书写中按下的换色/换粗细(只保留最后一次)。
static void gl_settings_apply_pending_color_width(void) {
    // S7: 逻辑已迁 Swift(MenuHandler.applyPendingColorWidth)。
    [MenuHandler applyPendingColorWidth];
}

// --- Pressure monitor (top-left info window) ---

static void pm_ensure_window(void) { [PressureMonitorWindow ensureWindow]; }

static void pm_show(void) { [PressureMonitorWindow show]; }

static void pm_hide(void) { [PressureMonitorWindow hide]; }

static void pm_update(void) { [SettingsPanelExtras refreshPressureLabel]; }

static void pm_destroy(void) { [PressureMonitorWindow destroy]; }

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
void glaspen2_router_sync_settings_panel(void) { glaspen2_notify_settings_changed(); }

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
void glaspen2_router_glass_apply(void) { [MenuHandler applyGlass]; }

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

// S7z: 磨砂玻璃应用的 Swift 薄桥
NSDictionary *glaspen2_router_glass_state(void) {
  return @{
    @"enabled" : @(g_glass_enabled ? YES : NO),
    @"opacity" : @(g_glass_opacity),
    @"ethereal" : @(g_ethereal_canvas ? YES : NO),
    @"follow" : @(g_glass_follow_strokes ? YES : NO),
    @"strokesVisible" : @(g_strokes_visible ? YES : NO),
  };
}
void glaspen2_router_apply_glass_view(double alpha, BOOL hidden) {
  if (g_glass_view) {
    g_glass_view.alphaValue = alpha;
    g_glass_view.hidden = hidden;
  }
}

// S7az: 预设表写入薄桥
void glaspen2_router_set_color_preset(int idx, double r, double g, double b) {
  g_color_presets[idx].r = r;
  g_color_presets[idx].g = g;
  g_color_presets[idx].b = b;
}
void glaspen2_router_set_width_preset(int idx, double value) {
  g_width_presets[idx] = value;
}

// S7bk: 快捷键 chord 查询薄桥
NSString *glaspen2_router_shortcut_chord(NSString *action) {
  if (!g_shortcut_values) gl_shortcuts_apply(@{});
  return g_shortcut_values[action];
}

// S7bo: 涂鸦启停的 Swift 薄桥
int glaspen2_router_enabled(void) { return g_enabled ? 1 : 0; }
int glaspen2_router_pressure_monitor(void) { return g_pressure_monitor ? 1 : 0; }
int glaspen2_router_strokes_visible(void) { return g_strokes_visible ? 1 : 0; }
void glaspen2_router_ensure_surface(void) {
  if (!g_surface && g_draw_view) ensure_surface(g_draw_view);
}

// S7bp: 画布模式切换的 Swift 薄桥
void glaspen2_router_set_ethereal(int on) { g_ethereal_canvas = on != 0; }
void glaspen2_router_set_strokes_visible(int on) { g_strokes_visible = on != 0; }
int glaspen2_router_glass_follow(void) { return g_glass_follow_strokes ? 1 : 0; }
id glaspen2_router_glass_view(void) { return g_glass_view; }

// S7bq: 菜单文案/状态薄桥
id glaspen2_router_menu(void) { return g_menu; }
int glaspen2_router_menu_state(int tag) {
  NSMenuItem *item = [MenuLookup itemWithTag:tag inMenu:g_menu];
  return (item && item.state == NSControlStateValueOn) ? 1 : 0;
}
NSAttributedString *glaspen2_router_color_item_title(int idx) { return gl_color_item_text(idx); }
NSAttributedString *glaspen2_router_width_item_title(int idx) { return gl_width_item_text(idx); }

// S7bs: 快捷键生效表薄桥
int glaspen2_router_shortcut_reserved(NSString *chord) {
  return gl_shortcut_reserved(chord) ? 1 : 0;
}
void glaspen2_router_set_shortcut_tables(NSDictionary *values, NSDictionary *lookup) {
  g_shortcut_values = [values mutableCopy];
  g_shortcut_lookup = [lookup mutableCopy];
}

// S7ce: 菜单勾选/设置快照薄桥
int glaspen2_router_selected_color_index(void) { return g_selectedColorIndex; }
int glaspen2_router_selected_width_index(void) { return g_selected_width_index; }
NSString *glaspen2_router_shortcuts_effective_json(void) {
  return gl_shortcuts_effective_json();
}
int glaspen2_router_flip_effect(void) { return g_flip_effect; }
int glaspen2_router_lang(void) { return g_lang; }

// S7cj: 快捷键生效表薄桥
NSDictionary *glaspen2_router_shortcut_values(void) {
  return g_shortcut_values ?: @{};
}

// S7de: 预设标题桥
void glaspen2_router_color_preset_rgb_at(int idx, double *r, double *g, double *b) {
  if (idx < 0 || idx >= g_color_preset_count) { *r = *g = *b = 0; return; }
  *r = g_color_presets[idx].r; *g = g_color_presets[idx].g; *b = g_color_presets[idx].b;
}
double glaspen2_router_width_preset_scale_at(int idx) {
  if (idx < 0 || idx >= g_width_preset_count) return 0;
  return g_width_presets[idx];
}

// S7dg: 压感状态桥(变量在 menu.m)
NSDictionary *glaspen2_router_pressure_state(void) {
  return @{
    @"pressure" : @(g_pm_pressure),
    @"type" : g_pm_evtype ?: @"---",
    @"x" : @(g_pm_x),
    @"y" : @(g_pm_y),
  };
}
