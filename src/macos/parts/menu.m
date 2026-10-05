@interface GlaspenMenuHandler : NSObject <NSMenuDelegate, NSApplicationDelegate>
@end

static GlaspenMenuHandler *g_menuHandler = nil;
static NSStatusItem *g_statusItem = nil;
static NSMenu *g_menu = nil;
static int g_selectedColorIndex = 0; // 0=red (default)

static NSAttributedString* gl_color_item_text(NSInteger idx);
static NSAttributedString* gl_width_item_text(NSInteger idx);
static void update_menu_texts(void) {
    // 颜色/粗细条目:重建富文本(内嵌图标 + 名称;setTitle 会清掉它)
    for (int i = 0; i < g_color_preset_count; i++) {
        [[g_menu itemAtIndex:i] setAttributedTitle:gl_color_item_text(i)];
    }
    int wBase = g_color_preset_count + 1;
    for (int i = 0; i < g_width_preset_count; i++) {
        [[g_menu itemAtIndex:wBase + i] setAttributedTitle:gl_width_item_text(i)];
    }
    int base = g_color_preset_count + 1 + g_width_preset_count + 1;
    [[g_menu itemAtIndex:base+0] setTitle:L(@"保存(含背景)", @"Save (with bg)")];
    [[g_menu itemAtIndex:base+1] setTitle:L(@"保存(涂鸦)", @"Save (drawing)")];
    [[g_menu itemAtIndex:base+2] setTitle:L(@"保存笔记 (Xournal)", @"Save Notes (Xournal)")];
    [[g_menu itemAtIndex:base+3] setTitle:L(@"导出无限画布 PDF (分页)", @"Export infinite canvas PDF (paged)")];
    [[g_menu itemAtIndex:base+4] setTitle:L(@"导出无限画布 SVG (整幅)", @"Export infinite canvas SVG (whole)")];
    [[g_menu itemAtIndex:base+5] setTitle:L(@"新建画布", @"New canvas")];
    [[g_menu itemAtIndex:base+6] setTitle:L(@"彩虹指示器", @"Rainbow indicator")];
    [[g_menu itemAtIndex:base+7] setTitle:L(@"开机自启", @"Launch at login")];
    [[g_menu itemAtIndex:base+8] setTitle:L(@"磨砂玻璃", @"Frosted Glass")];
    [[g_menu itemAtIndex:base+9] setTitle:L(@"笔迹描边", @"Stroke outline")];
    [[g_menu itemAtIndex:base+10] setTitle:L(@"无限画布", @"Infinite canvas")];
    // Update toggle item title based on state
    NSMenuItem *toggleItem = [g_menu itemWithTag:888];
    if (toggleItem) [toggleItem setTitle:g_enabled ? L(@"关闭涂鸦", @"Disable Drawing") : L(@"开启涂鸦", @"Enable Drawing")];
    [[g_menu itemAtIndex:base+14] setTitle:L(@"English", @"中文")];
    [[g_menu itemAtIndex:base+15] setTitle:L(@"退出", @"Quit")];
}

// ── 颜色/粗细菜单条目的富文本:内嵌小图标 + 名称 ──
// NSMenuItem.image 在新版系统菜单里不渲染(实测菜单全是字), 而正文里
// 内嵌附件图(attributedTitle + NSTextAttachment)任何系统版本都画。
// 颜色 = 圆点色块(一眼可见);粗细 = 横线粗细(粗细预设的直观映射)。

static NSImage* gl_color_dot_image(NSColor *color) {
    const CGFloat s = 13.0;
    NSImage *image = [[NSImage alloc] initWithSize:NSMakeSize(s, s)];
    [image lockFocus];
    NSBezierPath *dot = [NSBezierPath bezierPathWithOvalInRect:NSMakeRect(0.5, 0.5, s - 1, s - 1)];
    [color setFill];
    [dot fill];
    // 细描边:白/浅色圆点在白色菜单上也有一圈轮廓
    [[NSColor colorWithWhite:0 alpha:0.3] setStroke];
    [dot setLineWidth:1.0];
    [dot stroke];
    [image unlockFocus];
    return image;
}

static NSImage* gl_width_line_image(double scale) {
    const CGFloat w = 16.0, h = 13.0;
    NSImage *image = [[NSImage alloc] initWithSize:NSMakeSize(w, h)];
    [image lockFocus];
    // 预设 0.15x..3.5x → 线宽 1.5..5.5pt(开方缓增,细档之间也能看出差别)
    CGFloat lineW = (CGFloat)MIN(5.5, MAX(1.5, 1.3 + sqrt(scale) * 2.2));
    NSBezierPath *path = [NSBezierPath bezierPath];
    [path setLineWidth:lineW];
    [path setLineCapStyle:NSLineCapStyleRound];
    [path moveToPoint:NSMakePoint(2, h / 2)];
    [path lineToPoint:NSMakePoint(w - 2, h / 2)];
    [[[NSColor labelColor] colorWithAlphaComponent:0.9] setStroke];
    [path stroke];
    [image unlockFocus];
    return image;
}

static NSAttributedString* gl_menu_text(NSImage *icon, NSString *name) {
    NSTextAttachment *att = [[NSTextAttachment alloc] init];
    att.image = icon;
    att.bounds = NSMakeRect(0, -2.5, icon.size.width, icon.size.height);
    NSMutableAttributedString *s = [[NSMutableAttributedString alloc]
        initWithAttributedString:[NSAttributedString attributedStringWithAttachment:att]];
    [s appendAttributedString:[[NSAttributedString alloc]
        initWithString:[@"   " stringByAppendingString:name]
            attributes:@{NSFontAttributeName: [NSFont menuFontOfSize:14]}]];
    return s;
}

static NSAttributedString* gl_color_item_text(NSInteger idx) {
    static NSString *zh[] = {@"红", @"橙", @"黄", @"绿", @"青", @"蓝", @"紫", @"粉", @"白", @"黑"};
    NSString *name = (g_lang == 0)
        ? zh[idx]
        : [NSString stringWithUTF8String:g_color_presets[idx].name];
    NSColor *c = [NSColor colorWithRed:g_color_presets[idx].r
                                 green:g_color_presets[idx].g
                                  blue:g_color_presets[idx].b
                                 alpha:1.0];
    return gl_menu_text(gl_color_dot_image(c), name);
}

static NSAttributedString* gl_width_item_text(NSInteger idx) {
    static NSString *zh[] = {@"极细", @"很细", @"细", @"中", @"粗", @"很粗", @"超粗", @"极粗"};
    static NSString *en[] = {@"Hair", @"Very fine", @"Fine", @"Medium",
                             @"Thick", @"Very thick", @"Extra thick", @"Boldest"};
    NSString *name = (g_lang == 0) ? zh[idx] : en[idx];
    return gl_menu_text(gl_width_line_image(g_width_presets[idx]), name);
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
static NSImage* render_status_icon(void) {
    const CGFloat size = 18.0;
    const NSInteger px = 72; // 4x
    NSBitmapImageRep *rep = [[NSBitmapImageRep alloc]
        initWithBitmapDataPlanes:NULL pixelsWide:px pixelsHigh:px
        bitsPerSample:8 samplesPerPixel:4 hasAlpha:YES isPlanar:NO
        colorSpaceName:NSCalibratedRGBColorSpace bytesPerRow:0 bitsPerPixel:0];
    rep.size = NSMakeSize(size, size);
    [NSGraphicsContext saveGraphicsState];
    // 注意: rep.size(18pt) ≠ 像素尺寸(72px)时, 该上下文的 CTM 已自带
    // 点→像素的缩放, 再手动 scale 会双重放大把内容画出界(全透明)。
    [NSGraphicsContext setCurrentContext:[NSGraphicsContext graphicsContextWithBitmapImageRep:rep]];
    // 之后一律用 18pt 坐标系画

    NSColor *ink   = [NSColor colorWithWhite:0.0 alpha:0.78]; // 主描边(浅色栏可读)
    NSColor *halo  = [NSColor colorWithWhite:1.0 alpha:0.88]; // 外圈白晕(深色栏可读)
    NSColor *pen   = [NSColor colorWithRed:g_pen_r green:g_pen_g blue:g_pen_b alpha:1.0];
    CGFloat dim    = g_enabled ? 1.0 : 0.35;                  // 停用时字形淡化(斜杠不淡化)

    // 先描 halo 再描 ink: 同一条路径画两遍, 外圈白内圈黑, 任何底色都有对比
    void (^stroke_inked)(NSBezierPath *, CGFloat) = ^(NSBezierPath *path, CGFloat width) {
        [path setLineWidth:width + 1.2];
        [[halo colorWithAlphaComponent:dim] setStroke]; [path stroke];
        [path setLineWidth:width];
        [[ink colorWithAlphaComponent:dim] setStroke];  [path stroke];
    };

    if (g_eraser_mode && g_enabled) {
        // 橡皮擦进行中: 45° 斜放的橡皮块, 亮身 + 深色擦除带
        NSBezierPath *body = [NSBezierPath bezierPathWithRoundedRect:NSMakeRect(3.5, 6.1, 11.2, 5.8)
                                                        xRadius:1.5 yRadius:1.5];
        NSAffineTransform *t = [NSAffineTransform transform];
        [t translateXBy:9 yBy:9]; [t rotateByDegrees:45]; [t translateXBy:-9 yBy:-9];
        [body transformUsingAffineTransform:t];
        NSBezierPath *band = [NSBezierPath bezierPathWithRect:NSMakeRect(10.2, 6.1, 4.5, 5.8)];
        [band transformUsingAffineTransform:t];
        // 顺序: 白晕 → 填充 → 黑描边。白晕必须画在填充之前, 否则它内侧的
        // 一半会盖住小形状的内部(笔尖/擦除带会被洗白)。
        [body setLineWidth:0.8 + 1.2]; [[halo colorWithAlphaComponent:dim] setStroke]; [body stroke];
        [[NSColor colorWithWhite:0.92 alpha:dim] setFill]; [body fill];
        [[NSColor colorWithWhite:0.25 alpha:dim] setFill]; [band fill]; // 擦除带
        [body setLineWidth:0.8]; [[ink colorWithAlphaComponent:dim] setStroke]; [body stroke];
    } else {
        // 马克笔: 笔尖朝左下, 笔杆粗细 = 当前笔宽(0.15x..3.5x → 3.1..6.0pt)
        CGFloat T = MIN(6.0, MAX(2.6, 2.4 + sqrt(g_width_scale) * 1.9));
        NSBezierPath *body = [NSBezierPath bezierPathWithRoundedRect:
            NSMakeRect(5.5, 9 - T / 2, 9.0, T)
            xRadius:MIN(1.6, T / 2) yRadius:MIN(1.6, T / 2)];
        NSBezierPath *tip = [NSBezierPath bezierPath]; // 深色笔尖三角
        [tip moveToPoint:NSMakePoint(5.5, 9 - T / 2)];
        [tip lineToPoint:NSMakePoint(3.0, 9)];
        [tip lineToPoint:NSMakePoint(5.5, 9 + T / 2)];
        [tip closePath];
        NSBezierPath *sil = [NSBezierPath bezierPath]; // 剪影 = 杆 + 尖
        [sil appendBezierPath:body];
        [sil appendBezierPath:tip];
        NSAffineTransform *t = [NSAffineTransform transform];
        [t translateXBy:9 yBy:9]; [t rotateByDegrees:45]; [t translateXBy:-9 yBy:-9];
        [sil transformUsingAffineTransform:t];
        [tip transformUsingAffineTransform:t];
        [sil setLineWidth:0.9 + 1.2]; [[halo colorWithAlphaComponent:dim] setStroke]; [sil stroke];
        [[pen colorWithAlphaComponent:dim] setFill]; [sil fill];
        [[ink colorWithAlphaComponent:dim] setFill]; [tip fill]; // 笔尖深色, 与杆无缝
        [sil setLineWidth:0.9]; [[ink colorWithAlphaComponent:dim] setStroke]; [sil stroke];
    }

    if (!g_enabled) {
        // 停用斜杠(与笔身垂直)—— 全强度, 这是"停用"的主信号
        NSBezierPath *slash = [NSBezierPath bezierPath];
        [slash moveToPoint:NSMakePoint(2.6, 15.4)]; [slash lineToPoint:NSMakePoint(15.4, 2.6)];
        [slash setLineCapStyle:NSLineCapStyleRound];
        [slash setLineWidth:2.8]; [halo setStroke]; [slash stroke];
        [slash setLineWidth:1.6]; [ink setStroke];  [slash stroke];
    }

    // 模式徽标(模式在停用时依然成立, 不随字形淡化; 放在笔身对角线腾出的
    // 两个角上, 与笔身零重叠; 全强度描边压在停用斜杠之上, 白晕自然开缝)
    void (^stroke_mark)(NSBezierPath *) = ^(NSBezierPath *path) {
        [path setLineWidth:1.0 + 1.2]; [halo setStroke]; [path stroke];
        [path setLineWidth:1.0];       [ink setStroke];  [path stroke];
    };
    if (g_ethereal_canvas) {
        // 飘渺画布: 左上角 ghost 圆环
        NSBezierPath *dot = [NSBezierPath bezierPathWithOvalInRect:NSMakeRect(1.9, 13.1, 3.0, 3.0)];
        stroke_mark(dot);
    }
    if (g_infinite_canvas) {
        // 无限画布: 右下角 ∞(两个相切的小圆环)
        NSBezierPath *inf = [NSBezierPath bezierPath];
        [inf appendBezierPath:[NSBezierPath bezierPathWithOvalInRect:NSMakeRect(12.9, 1.6, 2.4, 2.4)]];
        [inf appendBezierPath:[NSBezierPath bezierPathWithOvalInRect:NSMakeRect(15.1, 1.6, 2.4, 2.4)]];
        stroke_mark(inf);
    }

    [NSGraphicsContext restoreGraphicsState];
    NSImage *image = [[NSImage alloc] initWithSize:NSMakeSize(size, size)];
    [image addRepresentation:rep];
    return image;
}

static void update_status_icon_color(void) {
    [g_statusItem.button setImage:render_status_icon()];
    [g_statusItem.button setTitle:@""]; // 只用图, 不留 "G" 文字
}

static void update_status_icon_state(void) {
    update_status_icon_color();
}

static void update_menu_checkmarks(void) {
    // Update checkmarks on color items
    for (int i = 0; i < g_color_preset_count; i++) {
        NSMenuItem *item = [g_menu itemAtIndex:i];
        [item setState:(i == g_selectedColorIndex) ? NSControlStateValueOn : NSControlStateValueOff];
    }
    // Update checkmarks on width items
    int widthOffset = g_color_preset_count + 1; // after colors + separator
    for (int i = 0; i < g_width_preset_count; i++) {
        NSMenuItem *item = [g_menu itemAtIndex:widthOffset + i];
        [item setState:(i == g_selected_width_index) ? NSControlStateValueOn : NSControlStateValueOff];
    }
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
    update_status_icon_state();
    show_notification(g_enabled
        ? L(@"涂鸦已开启", @"Drawing enabled")
        : L(@"涂鸦已关闭", @"Drawing disabled"));
    // Update menu item
    NSMenuItem *item = [g_menu itemWithTag:888];
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

// ── Page-navigation peek (ethereal mode) ──
// 翻页 loads strokes onto the hidden canvas; show them briefly so the user
// can see the page they navigated to, then hide again after `seconds`
// unless the user interacted (pen activity) or left 飘渺画布涂鸦模式.
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
        show_notification(L(@"飘渺画布涂鸦模式 (悬空/落笔显示)", @"Ethereal canvas mode (hover/down to show)"));
    } else {
        // → 固定画布涂鸦模式: show the strokes and keep them visible
        g_ethereal_canvas = NO;
        g_strokes_visible = YES;
        gl_glass_apply();
        if (g_pressure_monitor) pm_show();
        [g_draw_view setNeedsDisplay:YES];
        show_notification(L(@"固定画布涂鸦模式", @"Fixed canvas mode"));
    }
    // Sync the menu item (title shows the mode you switch TO, like toggleDraw)
    NSMenuItem *item = [g_menu itemWithTag:778];
    if (item) {
        [item setState:g_ethereal_canvas ? NSControlStateValueOn : NSControlStateValueOff];
        [item setTitle:g_ethereal_canvas
            ? L(@"固定画布涂鸦模式", @"Fixed canvas mode")
            : L(@"飘渺画布涂鸦模式", @"Ethereal canvas mode")];
    }
    update_status_icon_state(); // 飘渺画布 = 图标左上角的 ghost 圆环徽标
}

@implementation GlaspenMenuHandler

- (void)saveWithBg {
    save_with_background();
}

- (void)saveOnly {
    save_drawing_only();
}

- (void)clearScreen {
    clear_screen();
}

- (void)toggleDraw {
    toggle_enabled();
}

- (void)saveXoj {
    glaspen2_save_xoj();
    show_notification(L(@"笔记已保存", @"Notes saved"));
}

- (void)toggleLanguage {
    g_lang = 1 - g_lang;
    update_menu_texts();
}
- (void)showSettingsPanel {
    show_settings_panel();
}

- (void)toggleRainbow {
    gl_settings_set_rainbow(!g_show_rainbow);
}

- (void)toggleCanvasMode {
    toggle_canvas_mode();
}

- (void)toggleLaunch {
    gl_settings_set_launch(!glaspen2_is_launch_at_login());
}

- (void)toggleGlass {
    gl_settings_set_glass_enabled(!g_glass_enabled);
}

- (void)toggleOutline {
    apply_outline(!g_outline_enabled);
}

- (void)toggleInfiniteCanvas {
    apply_infinite_canvas(!g_infinite_canvas, YES);
}

// 无限画布 → 分页 PDF(按当前屏幕尺寸切页)
- (void)exportInfinitePdf {
    int pw = g_screen_w, ph = g_screen_h;
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
        int ok = glaspen2_export_infinite_pdf_paged(pw, ph);
        dispatch_async(dispatch_get_main_queue(), ^{
            show_notification(ok
                ? L(@"无限画布 PDF 已导出到桌面", @"Infinite-canvas PDF saved to Desktop")
                : L(@"无限画布为空或页数过多", @"Infinite canvas empty or too many pages"));
        });
    });
}

// 无限画布 → 整幅 SVG(内容包围盒, 不受镜头影响)
- (void)exportInfiniteSvg {
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
        int ok = glaspen2_export_infinite_svg();
        dispatch_async(dispatch_get_main_queue(), ^{
            show_notification(ok
                ? L(@"无限画布 SVG 已导出到桌面", @"Infinite-canvas SVG saved to Desktop")
                : L(@"无限画布为空", @"Infinite canvas is empty"));
        });
    });
}

- (void)selectColor:(NSMenuItem *)sender {
    gl_settings_set_color((int)[sender tag]);
}

- (void)selectWidth:(NSMenuItem *)sender {
    gl_settings_set_width((int)[sender tag]);
}

// NSApplicationDelegate
- (NSApplicationTerminateReply)applicationShouldTerminate:(NSApplication *)sender {
    return NSTerminateNow;
}

- (void)quitApp {
    perf_log_summary();
    CGDisplayShowCursor(kCGDirectMainDisplay);
    [NSApp terminate:nil];
}

// NSMenuDelegate
- (void)menuWillOpen:(NSMenu *)menu {
    update_status_icon_color();
    update_menu_checkmarks();
}

- (void)menuDidClose:(NSMenu *)menu {
    update_status_icon_state();
    // Re-enable CGEventTap after menu closes
    if (g_event_tap) {
        CGEventTapEnable(g_event_tap, true);
    }
}

@end

// --- Settings Panel (Flutter-based) ---
static FlutterEngine *g_flutter_engine = nil;
static FlutterViewController *g_flutter_vc = nil;
static NSWindow *g_settings_window = nil;

static void show_settings_panel(void);
static void sync_settings_panel(void);

// --- Legacy settings panel stubs (no longer used, kept for sync_settings_panel) ---
static NSButton *g_color_buttons[10];
static NSButton *g_width_buttons[5];
static NSButton *g_rainbow_toggle = nil;
static NSButton *g_launch_toggle = nil;
static NSButton *g_glass_toggle = nil;
static NSButton *g_glass_buttons[1];

// 无限画布总览载荷:渲染当前页包围盒适配图 + 当前视口矩形。
// 返回 nil 表示空画布(无笔迹)。
static NSDictionary *canvas_overview_payload(double w, double h) {
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

    int ow = (int)w, oh = (int)h;
    int outLen = 0;
    unsigned char *png = glaspen2_render_canvas_overview(bx, by, bw, bh, ow, oh, &outLen);
    if (!png || outLen <= 0) return nil;

    // 总览映射(scale/offset 必须与渲染一致),再映射当前视口矩形
    double ov_scale = (ow / bw) < (oh / bh) ? (ow / bw) : (oh / bh);
    double ov_ox = (ow - bw * ov_scale) * 0.5;
    double ov_oy = (oh - bh * ov_scale) * 0.5;
    double z = g_zoom > 0.05 ? g_zoom : 1.0;
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

/// 当前设置的 JSON 快照;调用方用 glaspen2_macos_free_c_string 释放。
char *glaspen2_macos_settings_json(void) {
    __block char *out = NULL;
    gl_run_on_main_sync(^{
        NSDictionary *d = @{
            @"color": @(g_selectedColorIndex),
            @"width": @(g_selected_width_index),
            @"rainbow": @(g_show_rainbow),
            @"launchAtLogin": @(glaspen2_is_launch_at_login()),
            @"frostedGlass": @(g_glass_enabled),
            @"grid": @(g_show_grid),
            @"gridFollowStrokes": @(g_grid_follow_strokes),
            @"glassFollowStrokes": @(g_glass_follow_strokes),
            @"notebookStyle": @(glaspen2_load_bool_setting("notebook_style") != 0),
            @"softShadow": @(g_soft_shadow),
            @"invertInk": @(g_invert_ink),
            @"invertFps": @(g_invert_fps),
            @"pressureMonitor": @(g_pressure_monitor),
            @"outline": @(g_outline_enabled),
            @"infiniteCanvas": @(g_infinite_canvas),
            @"minimap": @(g_minimap_enabled),
            @"gridSize": @(g_grid_size),
            @"gridDivider": @(g_grid_divider),
            @"flipEffect": @(g_flip_effect),
            @"gifFps": @(g_gif_fps),
            @"gifResolution": @(g_gif_resolution),
            @"gifSpeed": @(g_gif_speed),
            @"gifEndMode": @(g_gif_end_mode),
            // 涂鸦身份:密码本体永不进快照,只回是否已保存
            @"chatApiBase": gl_string_setting("chat_api_base"),
            @"chatUser":    gl_string_setting("chat_user"),
            @"chatHasPassword": @([gl_string_setting("chat_password") length] > 0),
            @"chatIntegration": @(g_chat_integration),
            // 注意:load_bool_setting 返回 c_int,@() 会序列化成 JSON 数字 1/0,
            // Rust 侧 as_bool() 解析不出 → 永远 false。必须转成 BOOL(YES/NO)
            // 让 NSJSONSerialization 输出 true/false。
            @"showFreeCanvas": @(glaspen2_load_bool_setting("show_free_canvas") != 0),
            @"shareCanvas": @(glaspen2_load_bool_setting("share_ink") != 0),
        };
        NSData *json = [NSJSONSerialization dataWithJSONObject:d options:0 error:nil];
        if (!json) return;
        NSString *s = [[NSString alloc] initWithData:json encoding:NSUTF8StringEncoding];
        const char *cs = s.UTF8String;
        if (!cs) return;
        size_t n = strlen(cs);
        char *buf = (char *)malloc(n + 1);
        if (!buf) return;
        memcpy(buf, cs, n + 1);
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
    g_chat_integration = on;
    glaspen2_save_bool_setting("chat_integration", on ? 1 : 0);
    if (!on) {
        // 静默终止进行中的手写录制/草稿;共享上行一并停止
        g_msg_record_start = -1;
        if (g_ink_draft_active) ink_draft_stop_async();
        glaspen2_share_ink_set_active(NO);
    } else if (glaspen2_load_bool_setting("share_ink")) {
        // 重新开启集成:共享上行开关若此前是开的,一并恢复
        glaspen2_share_ink_set_active(YES);
    }
}

/// 解析 Dart 传来的 JSON 标量(true / 3 / 2.5)。
///
/// 必须带 NSJSONReadingFragmentsAllowed:JSONObjectWithData 默认只接受顶层
/// 容器,裸标量会直接报错返回 nil —— 那样每个设置都会退化成 [nil boolValue]
/// = NO / [nil intValue] = 0,Dart 侧的表现就是"按了按钮没反应"。
static id gl_parse_setting_value(const char *json) {
    if (!json) return nil;
    NSData *data = [NSData dataWithBytes:json length:strlen(json)];
    NSError *err = nil;
    id value = [NSJSONSerialization JSONObjectWithData:data
                                               options:NSJSONReadingFragmentsAllowed
                                                 error:&err];
    if (!value) {
        NSLog(@"[settings] 无法解析 value_json=\"%s\": %@", json, err.localizedDescription);
    }
    return value;
}

/// 写入一项设置。`value_json` 是 JSON 标量(true / 3 / 2.5),按 key 解析后
/// 交给与菜单、快捷键共用的 gl_settings_set_* 和全局状态。
void glaspen2_macos_set_setting(const char *key_c, const char *value_json) {
    if (!key_c) return;
    NSString *key = @(key_c);
    id value = gl_parse_setting_value(value_json);
    // 解析不出来就什么都不做:宁可这一项不生效,也不要静默写成 false/0
    if (!value) return;
    gl_run_on_main_sync(^{
        if ([key isEqualToString:@"color"]) {
            gl_settings_set_color([value intValue]);
        } else if ([key isEqualToString:@"width"]) {
            gl_settings_set_width([value intValue]);
        } else if ([key isEqualToString:@"rainbow"]) {
            gl_settings_set_rainbow([value boolValue]);
        } else if ([key isEqualToString:@"launchAtLogin"]) {
            gl_settings_set_launch([value boolValue]);
        } else if ([key isEqualToString:@"frostedGlass"]) {
            gl_settings_set_glass_enabled([value boolValue]);
        } else if ([key isEqualToString:@"opacity"]) {
            gl_settings_set_glass_opacity([value doubleValue]);
            if (!g_glass_enabled) gl_settings_set_glass_enabled(YES);
        } else if ([key isEqualToString:@"grid"]) {
            gl_settings_set_grid([value boolValue]);
        } else if ([key isEqualToString:@"gridFollowStrokes"]) {
            g_grid_follow_strokes = [value boolValue];
            glaspen2_save_bool_setting("grid_follow_strokes", g_grid_follow_strokes ? 1 : 0);
            if (g_draw_view) [g_draw_view setNeedsDisplay:YES];
        } else if ([key isEqualToString:@"glassFollowStrokes"]) {
            g_glass_follow_strokes = [value boolValue];
            glaspen2_save_bool_setting("glass_follow_strokes", g_glass_follow_strokes ? 1 : 0);
            gl_glass_apply(); // 立即按新规则重估玻璃可见性(含飘渺隐藏态)
        } else if ([key isEqualToString:@"softShadow"]) {
            g_soft_shadow = [value boolValue];
            glaspen2_save_bool_setting("soft_shadow", g_soft_shadow ? 1 : 0);
            glaspen2_set_soft_shadow(g_soft_shadow ? 1 : 0);
            if (g_draw_view) [g_draw_view setNeedsDisplay:YES];
        } else if ([key isEqualToString:@"invertInk"]) {
            g_invert_ink = [value boolValue];
            glaspen2_save_bool_setting("invert_ink", g_invert_ink ? 1 : 0);
            invert_ink_apply(g_invert_ink ? 1 : 0);
        } else if ([key isEqualToString:@"invertFps"]) {
            int fps = [value intValue];
            if (fps == 10 || fps == 30 || fps == 60 || fps == 100) {
                g_invert_fps = fps;
                char fpsStr[16];
                snprintf(fpsStr, sizeof(fpsStr), "%d", g_invert_fps);
                glaspen2_save_string_setting("invert_fps", fpsStr);
                if (g_invert_ink) invert_stream_restart(); // 用新帧率重启捕获流
            }
        } else if ([key isEqualToString:@"notebookStyle"]) {
            glaspen2_save_bool_setting("notebook_style", [value boolValue] ? 1 : 0);
        } else if ([key isEqualToString:@"outline"]) {
            apply_outline([value boolValue]);
        } else if ([key isEqualToString:@"infiniteCanvas"]) {
            apply_infinite_canvas([value boolValue], NO);
        } else if ([key isEqualToString:@"minimap"]) {
            g_minimap_enabled = [value boolValue];
            glaspen2_save_bool_setting("minimap", g_minimap_enabled ? 1 : 0);
            if (g_draw_view) [g_draw_view setNeedsDisplay:YES];
        } else if ([key isEqualToString:@"gridSize"]) {
            double gs = glaspen2_clamp_setting_double("gridSize", [value doubleValue]);
            g_grid_size = gs;
            NSString *gsStr = [NSString stringWithFormat:@"%.0f", gs];
            glaspen2_save_string_setting("grid_size", [gsStr UTF8String]);
            if (g_draw_view) [g_draw_view setNeedsDisplay:YES];
        } else if ([key isEqualToString:@"gridDivider"]) {
            NSInteger dv = [value integerValue];
            if (dv < 0) dv = 0;
            if (dv > 3) dv = 3;
            g_grid_divider = dv;
            NSString *dvStr = [NSString stringWithFormat:@"%ld", (long)dv];
            glaspen2_save_string_setting("grid_divider", [dvStr UTF8String]);
            if (g_draw_view) [g_draw_view setNeedsDisplay:YES];
        } else if ([key isEqualToString:@"flipEffect"]) {
            NSInteger fe = [value integerValue];
            if (fe < 0) fe = 0;
            if (fe > 1) fe = 1;
            g_flip_effect = fe;
            NSString *feStr = [NSString stringWithFormat:@"%ld", (long)fe];
            glaspen2_save_string_setting("flip_effect", [feStr UTF8String]);
        } else if ([key isEqualToString:@"pressureMonitor"]) {
            gl_settings_set_pressure_monitor([value boolValue]);
        } else if ([key isEqualToString:@"gifFps"]) {
            g_gif_fps = glaspen2_clamp_setting_int("gifFps", [value intValue]);
            NSString *s = [NSString stringWithFormat:@"%d", g_gif_fps];
            glaspen2_save_string_setting("gif_fps", [s UTF8String]);
        } else if ([key isEqualToString:@"gifResolution"]) {
            g_gif_resolution = glaspen2_clamp_setting_double("gifResolution", [value doubleValue]);
            NSString *s = [NSString stringWithFormat:@"%.4f", g_gif_resolution];
            glaspen2_save_string_setting("gif_resolution", [s UTF8String]);
        } else if ([key isEqualToString:@"gifSpeed"]) {
            g_gif_speed = glaspen2_clamp_setting_double("gifSpeed", [value doubleValue]);
            NSString *s = [NSString stringWithFormat:@"%.4f", g_gif_speed];
            glaspen2_save_string_setting("gif_speed", [s UTF8String]);
        } else if ([key isEqualToString:@"gifEndMode"]) {
            g_gif_end_mode = glaspen2_clamp_setting_int("gifEndMode", [value intValue]);
            NSString *s = [NSString stringWithFormat:@"%d", g_gif_end_mode];
            glaspen2_save_string_setting("gif_end_mode", [s UTF8String]);
        } else if ([key isEqualToString:@"chatApiBase"] || [key isEqualToString:@"chatUser"]) {
            // 涂鸦身份(手写消息登录):落盘 + 立即推给 Rust auth 模块
            NSString *s = [NSString stringWithFormat:@"%@", value];
            const char *dbKey = [key isEqualToString:@"chatApiBase"] ? "chat_api_base" : "chat_user";
            glaspen2_save_string_setting(dbKey, [s UTF8String]);
            glaspen2_chat_auth_reload();
        } else if ([key isEqualToString:@"chatPassword"]) {
            // 空串 = 保持已存密码不变(面板回显时不发密码)
            NSString *s = [NSString stringWithFormat:@"%@", value];
            if (s.length > 0) {
                glaspen2_save_string_setting("chat_password", [s UTF8String]);
                glaspen2_chat_auth_reload();
            }
        } else if ([key isEqualToString:@"chatIntegration"]) {
            gl_settings_set_chat_integration([value boolValue]);
        } else if ([key isEqualToString:@"showFreeCanvas"]) {
            // 「自由涂鸦」tab 显隐(默认关:最小面板只有 设置+活页本)
            glaspen2_save_bool_setting("show_free_canvas", [value boolValue] ? 1 : 0);
        } else if ([key isEqualToString:@"shareCanvas"]) {
            // 共享画布上行开关(活页本 tab):仅集成开启时真正生效
            BOOL on = [value boolValue];
            glaspen2_save_bool_setting("share_ink", on ? 1 : 0);
            if (g_chat_integration || !on) {
                glaspen2_share_ink_set_active((g_chat_integration && on) ? 1 : 0);
            }
        }
    });
}

/// 删除一页;返回 1 表示成功。删除当前页时自动切到相邻页。
int glaspen2_macos_delete_page(long long screen_id) {
    __block int ok = 0;
    dispatch_sync(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
        ok = glaspen2_delete_screen(screen_id);
        gl_run_on_main_sync(^{
            if (g_infinite_canvas) {
                // 无限画布独立存储,不受页存储的删除影响
                rebuild_surface_from_strokes();
                return;
            }
            if (ok && screen_id == glaspen2_get_current_screen_id()) {
                int64_t next = glaspen2_next_screen_id();
                if (next == 0) next = glaspen2_prev_screen_id();
                if (next != 0) {
                    glaspen2_load_strokes_for_screen(next);
                    rebuild_surface_from_strokes();
                } else {
                    clear_screen();
                }
            }
        });
    });
    return ok;
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
        // Q (退出) 不走 perform_hotkey: 那是按钮专属动作, ⌃⌘Q 必须留给锁屏。
        if ([key isEqualToString:@"Q"]) {
            [NSApp terminate:nil];
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

/// 设置面板「检查更新 → 打开下载页」:用系统默认浏览器打开 URL。
/// http/https 白名单已在 Rust 侧(src/api.rs open_url_checked)挡过;
/// NSWorkspace 必须在主线程调用, 由 gl_run_on_main_sync 负责切换。
void glaspen2_macos_open_url(const char *url_c) {
    if (!url_c) return;
    NSString *s = @(url_c);
    gl_run_on_main_sync(^{
        NSURL *url = [NSURL URLWithString:s];
        if (url) {
            [[NSWorkspace sharedWorkspace] openURL:url];
        } else {
            NSLog(@"[glaspen2] open_url: URL 解析失败: %@", s);
        }
    });
}

/// 无限画布总览:action 0=当前 1=镜头回原点 2=居中内容 3=新建(清空)。
/// 返回 PNG 缓冲区(用 glaspen2_macos_free_bytes 释放),rect_out 收视口矩形;
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
            g_pan_x = 0.0;
            g_pan_y = 0.0;
            g_zoom = 1.0;
            canvas_apply_transform();
        } else if (action == 2) {
            double bx, by, bx2, by2;
            if (glaspen2_stroke_bbox(&bx, &by, &bx2, &by2)) {
                double z = g_zoom > 0.05 ? g_zoom : 1.0;
                g_pan_x = (bx + bx2) * 0.5 - g_screen_w * 0.5 / z;
                g_pan_y = (by + by2) * 0.5 - g_screen_h * 0.5 / z;
                canvas_apply_transform();
            }
        } else if (action == 3) {
            finish_active_stroke();
            glaspen2_clear_strokes(g_screen_w, g_screen_h);
            g_pan_x = 0.0;
            g_pan_y = 0.0;
            g_zoom = 1.0;
            glaspen2_set_view_transform(g_pan_x, g_pan_y, g_zoom);
            rebuild_surface_from_strokes();
            ow = 1024;
            oh = 768;
        }
        NSDictionary *payload = canvas_overview_payload(ow, oh);
        if (!payload) return;
        NSData *data = payload[@"png"];
        NSArray *rect = payload[@"rect"];
        if (!data || data.length == 0) return;
        unsigned char *buf = (unsigned char *)malloc(data.length);
        if (!buf) return;
        memcpy(buf, data.bytes, data.length);
        result_png = buf;
        if (out_len) *out_len = (int)data.length;
        if (rect_out && rect.count == 4) {
            for (int i = 0; i < 4; i++) rect_out[i] = [rect[i] doubleValue];
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
                show_notification(L(@"没有笔迹或导出失败", @"No strokes or export failed"));
            }
        });
    });
    return ok;
}

// --- Unified settings functions (single source of truth) ---

static void gl_settings_set_color(int idx) {
    if (idx < 0 || idx >= g_color_preset_count) return;
    g_pen_r = g_color_presets[idx].r;
    g_pen_g = g_color_presets[idx].g;
    g_pen_b = g_color_presets[idx].b;
    g_selectedColorIndex = idx;
    glaspen2_save_settings(g_pen_r, g_pen_g, g_pen_b, g_width_scale);
    update_menu_checkmarks();
    update_status_icon_color();
    sync_settings_panel();
}

static void gl_settings_set_width(int idx) {
    if (idx < 0 || idx >= g_width_preset_count) return;
    g_width_scale = g_width_presets[idx];
    g_selected_width_index = idx;
    glaspen2_save_settings(g_pen_r, g_pen_g, g_pen_b, g_width_scale);
    update_menu_checkmarks();
    update_status_icon_state(); // 笔宽画在图标笔杆粗细里
    sync_settings_panel();
}

static void gl_settings_set_rainbow(BOOL on) {
    g_show_rainbow = on;
    NSMenuItem *item = [g_menu itemWithTag:999];
    [item setState:on ? NSControlStateValueOn : NSControlStateValueOff];
    sync_settings_panel();
    // Redraw the surface without the rainbow — never clear the page.
    rebuild_surface_from_strokes();
}

static void gl_settings_set_grid(BOOL on) {
    g_show_grid = on;
    glaspen2_save_bool_setting("grid", on ? 1 : 0);
    sync_settings_panel();
    // The grid is drawn by drawRect per frame (own toggle) — just redraw.
    if (g_draw_view) [g_draw_view setNeedsDisplay:YES];
}

static void gl_settings_set_launch(BOOL on) {
    glaspen2_set_launch_at_login(on ? 1 : 0);
    NSMenuItem *item = [g_menu itemWithTag:777];
    [item setState:on ? NSControlStateValueOn : NSControlStateValueOff];
    sync_settings_panel();
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
    NSMenuItem *gi = [g_menu itemWithTag:444];
    [gi setState:g_glass_enabled ? NSControlStateValueOn : NSControlStateValueOff];
    if (g_glass_toggle) g_glass_toggle.state = g_glass_enabled ? NSControlStateValueOn : NSControlStateValueOff;
    g_glass_buttons[0].state = (fabs(g_glass_opacity - 0.50) < 0.001) ? NSControlStateValueOn : NSControlStateValueOff;
}

static void gl_settings_set_glass_enabled(BOOL on) {
    g_glass_enabled = on;
    glaspen2_save_bool_setting("glass_enabled", on ? 1 : 0);
    gl_glass_apply();
}

static void gl_settings_set_glass_opacity(double alpha) {
    g_glass_opacity = alpha;
    glaspen2_save_bool_setting("glass_alpha", (int)(alpha * 1000));
    gl_glass_apply();
}

static void gl_settings_set_enabled(BOOL on) {
    g_enabled = on;
    NSMenuItem *item = [g_menu itemWithTag:888];
    [item setState:on ? NSControlStateValueOn : NSControlStateValueOff];
    [item setTitle:on ? L(@"关闭涂鸦", @"Disable Drawing") : L(@"开启涂鸦", @"Enable Drawing")];
    update_status_icon_state();
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
    NSString *typeStr = g_pm_evtype ? g_pm_evtype : @"---";
    [g_pm_label setStringValue:[NSString stringWithFormat:@"P=%-5d  %@  (%d,%d)",
                                 g_pm_pressure, typeStr,
                                 (int)g_pm_x, (int)g_pm_y]];
}

static void pm_destroy(void) {
    if (g_pm_window) {
        [g_pm_window close];
        g_pm_window = nil;
        g_pm_label = nil;
    }
}

static void gl_settings_set_pressure_monitor(BOOL on) {
    g_pressure_monitor = on;
    glaspen2_save_bool_setting("pressure_monitor", on ? 1 : 0);
    if (on) {
        pm_show();
        pm_update();
    } else {
        pm_hide();
    }
    sync_settings_panel();
}

static void sync_settings_panel(void) {
    // 菜单/快捷键改了状态 → 推给订阅了 FRB 设置流的设置面板
    glaspen2_notify_settings_changed();
}

