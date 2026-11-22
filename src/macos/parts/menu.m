@interface GlaspenMenuHandler : NSObject <NSMenuDelegate, NSApplicationDelegate>
@end

static GlaspenMenuHandler *g_menuHandler = nil;
static NSStatusItem *g_statusItem = nil;
static NSMenu *g_menu = nil;

/// 按 tag 找菜单项(含一层子菜单)。菜单把低频项收进子菜单后, 原先
/// `[g_menu itemWithTag:]` 找不到子菜单里的项, 统一走这个入口。
static NSMenuItem *gl_menu_item_with_tag(NSInteger tag) {
    if (!g_menu) return nil;
    NSMenuItem *found = [g_menu itemWithTag:tag];
    if (found) return found;
    for (NSMenuItem *item in [g_menu itemArray]) {
        NSMenuItem *inner = item.submenu ? [item.submenu itemWithTag:tag] : nil;
        if (inner) return inner;
    }
    return nil;
}
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
    // 颜色/粗细条目:重建富文本(内嵌图标 + 名称;setTitle 会清掉它)
    for (int i = 0; i < g_color_preset_count; i++) {
        [[g_menu itemAtIndex:i] setAttributedTitle:gl_color_item_text(i)];
    }
    int wBase = g_color_preset_count + 1;
    for (int i = 0; i < g_width_preset_count; i++) {
        [[g_menu itemAtIndex:wBase + i] setAttributedTitle:gl_width_item_text(i)];
    }
    // 顶层布局(颜色/粗细之后): 保存×2 / 新建画布 / 导出▸ / sep /
    // 涂鸦开关 / 设置 / 更多▸ / sep / 语言 / 退出。
    int base = g_color_preset_count + 1 + g_width_preset_count + 1;
    [[g_menu itemAtIndex:base + 0] setTitle:L(@"保存(含背景)", @"Save (with bg)")];
    [[g_menu itemAtIndex:base + 1] setTitle:L(@"保存(涂鸦)", @"Save (drawing)")];
    [[g_menu itemAtIndex:base + 2] setTitle:L(@"新建画布", @"New canvas")];
    [[g_menu itemAtIndex:base + 3] setTitle:L(@"导出", @"Export")];
    [[g_menu itemAtIndex:base + 6] setTitle:L(@"设置...", @"Settings...")];
    [[g_menu itemAtIndex:base + 7] setTitle:L(@"更多", @"More")];
    [[g_menu itemAtIndex:base + 9] setTitle:L(@"English", @"中文")];
    [[g_menu itemAtIndex:base + 10] setTitle:L(@"退出", @"Quit")];
    // 子菜单条目也要跟着语言走(索引与 run.m 的构建顺序一致)
    NSMenu *exportMenu = [[g_menu itemAtIndex:base + 3] submenu];
    if (exportMenu) {
        [[exportMenu itemAtIndex:0] setTitle:L(@"保存笔记 (Xournal)", @"Save Notes (Xournal)")];
        [[exportMenu itemAtIndex:1] setTitle:L(@"导出无限画布 PDF (分页)", @"Export infinite canvas PDF (paged)")];
        [[exportMenu itemAtIndex:2] setTitle:L(@"导出无限画布 SVG (整幅)", @"Export infinite canvas SVG (whole)")];
    }
    NSMenu *moreMenu = [[g_menu itemAtIndex:base + 7] submenu];
    if (moreMenu) {
        [[moreMenu itemAtIndex:0] setTitle:L(@"开机自启", @"Launch at login")];
        [[moreMenu itemAtIndex:1] setTitle:L(@"磨砂玻璃", @"Frosted Glass")];
        [[moreMenu itemAtIndex:2] setTitle:L(@"笔迹描边", @"Stroke outline")];
        [[moreMenu itemAtIndex:3] setTitle:L(@"无限画布", @"Infinite canvas")];
        // 第 4 项(固定/飘渺模式)的标题由模式切换逻辑动态维护, 这里不覆盖
    }
    // Update toggle item title based on state
    NSMenuItem *toggleItem = gl_menu_item_with_tag(888);
    if (toggleItem) [toggleItem setTitle:g_enabled ? L(@"关闭涂鸦", @"Disable Drawing") : L(@"开启涂鸦", @"Enable Drawing")];
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

static NSString *gl_color_display_name(NSInteger idx) {
    static NSString *zh[] = {@"红", @"橙", @"黄", @"绿", @"青", @"蓝", @"紫", @"粉", @"白", @"黑"};
    if (idx < 0 || idx >= g_color_preset_count) return @"";
    return (g_lang == 0)
        ? zh[idx]
        : [NSString stringWithUTF8String:g_color_presets[idx].name];
}

static NSString *gl_width_display_name(NSInteger idx) {
    static NSString *zh[] = {@"极细", @"很细", @"细", @"中", @"粗", @"很粗", @"超粗", @"极粗"};
    static NSString *en[] = {@"Hair", @"Very fine", @"Fine", @"Medium",
                             @"Thick", @"Very thick", @"Extra thick", @"Boldest"};
    if (idx < 0 || idx >= g_width_preset_count) return @"";
    return (g_lang == 0) ? zh[idx] : en[idx];
}

static NSAttributedString* gl_color_item_text(NSInteger idx) {
    NSColor *c = [NSColor colorWithRed:g_color_presets[idx].r
                                 green:g_color_presets[idx].g
                                  blue:g_color_presets[idx].b
                                 alpha:1.0];
    return gl_menu_text(gl_color_dot_image(c), gl_color_display_name(idx));
}

static NSAttributedString* gl_width_item_text(NSInteger idx) {
    return gl_menu_text(gl_width_line_image(g_width_presets[idx]),
                        gl_width_display_name(idx));
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
    NSMenuItem *item = gl_menu_item_with_tag(888);
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
    NSMenuItem *item = gl_menu_item_with_tag(778);
    if (item) {
        [item setState:g_ethereal_canvas ? NSControlStateValueOn : NSControlStateValueOff];
        [item setTitle:g_ethereal_canvas
            ? L(@"固定画布涂鸦模式", @"Fixed canvas mode")
            : L(@"飘渺画布涂鸦模式", @"Ethereal canvas mode")];
    }
    // 持久化(与 Windows 同键 "ethereal"): 启动恢复在 run.m 设置装载段
    glaspen2_save_bool_setting("ethereal", g_ethereal_canvas ? 1 : 0);
    update_status_icon_state(); // 飘渺画布 = 图标左上角的 ghost 圆环徽标
}

/// 彻底退出:菜单「退出」、可配置快捷键 quit、设置面板/自动更新流程共用**同一条**
/// 路径 —— 先记性能汇总、把光标还给系统, 再 terminate。terminate 会同步走
/// applicationShouldTerminate 把排队点写完并 checkpoint WAL(见下), 进程里
/// 没有其它需要单独收尾的子进程(设置面板是内嵌 Flutter 视图, 不是独立进程)。
static void gl_quit_app(void) {
    perf_log_summary();
    CGDisplayShowCursor(kCGDirectMainDisplay);
    [NSApp terminate:nil];
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
    glaspen2_save_string_setting("language", g_lang == 1 ? "en" : "zh");
    update_menu_texts();
    // 面板若开着, 让 Dart 侧跟着切换(不再跟随系统 locale)
    glaspen2_notify_settings_changed();
}
- (void)showSettingsPanel {
    show_settings_panel();
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
    // NSApp terminate 同样不走 Rust 析构: 退出前写完排队点并 checkpoint WAL,
    // 否则自动更新的 .db 快照可能缺最后一小段笔迹。
    glaspen2_shutdown();
    return NSTerminateNow;
}

- (void)quitApp {
    gl_quit_app();
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
static NSButton *g_launch_toggle = nil;
static NSButton *g_glass_toggle = nil;
static NSButton *g_glass_buttons[1];

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
static NSString *gl_shortcut_action_for_key(unsigned short kc, NSUInteger mods) {
    if (!g_shortcut_lookup) gl_shortcuts_apply(@{});
    NSString *key = gl_keycode_names()[@(kc)];
    if (!key) return nil;
    NSMutableArray<NSString *> *parts = [NSMutableArray array];
    if (mods & NSEventModifierFlagCommand) [parts addObject:@"cmd"];
    if (mods & NSEventModifierFlagControl) [parts addObject:@"ctrl"];
    if (mods & NSEventModifierFlagOption) [parts addObject:@"alt"];
    if (mods & NSEventModifierFlagShift) [parts addObject:@"shift"];
    if (![parts containsObject:@"ctrl"] && ![parts containsObject:@"alt"]) return nil;
    [parts addObject:key];
    return g_shortcut_lookup[[parts componentsJoinedByString:@"+"]];
}

/// 当前设置的 JSON 快照;调用方用 glaspen2_macos_free_c_string 释放。
char *glaspen2_macos_settings_json(void) {
    __block char *out = NULL;
    gl_run_on_main_sync(^{
        // 气泡重播设置(纯面板自用, 落库读取): 速度缺省 1, 停末帧缺省开。
        // @() 装箱不能包语句块, 先算成局部变量再进字典。
        char *vsp = glaspen2_load_string_setting("bubble_replay_speed");
        int replay_speed = vsp ? atoi(vsp) : 1;
        glaspen2_free_c_string(vsp);
        if (replay_speed < 1) replay_speed = 1;
        if (replay_speed > 8) replay_speed = 8;
        char *vhl = glaspen2_load_string_setting("bubble_replay_hold_last");
        BOOL replay_hold = vhl ? (atoi(vhl) != 0) : YES;
        glaspen2_free_c_string(vhl);
        NSDictionary *d = @{
            @"color": @(g_selectedColorIndex),
            @"width": @(g_selected_width_index),
            @"launchAtLogin": @(glaspen2_is_launch_at_login()),
            @"frostedGlass": @(g_glass_enabled),
            @"grid": @(g_show_grid),
            @"gridFollowStrokes": @(g_grid_follow_strokes),
            @"glassFollowStrokes": @(g_glass_follow_strokes),
            @"notebookStyle": @(glaspen2_load_bool_setting("notebook_style") != 0),
            @"panelTransparent": @(glaspen2_load_bool_setting("panel_transparent") != 0),
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
            @"showBubbleTab": @(glaspen2_load_bool_setting("show_bubble_tab") != 0),
            // 气泡消息重播: 速度倍率(缺省 1)/ 停在最后一帧(缺省开)/ 自动重播(缺省关)
            @"bubbleReplaySpeed": @(replay_speed),
            @"bubbleReplayHoldLast": @(replay_hold),
            @"bubbleReplayAuto": @(glaspen2_load_bool_setting("bubble_replay_auto") != 0),
            @"bubbleAutoNewPage": @(glaspen2_load_bool_setting("bubble_auto_new_page") != 0),
            @"nasUrl": gl_string_setting("nas_url"),
            @"shareCanvas": @(glaspen2_load_bool_setting("share_ink") != 0),
            // 语言由面板选择器决定, 不再跟随系统; 缺省中文
            @"language": (g_lang == 1 ? @"en" : @"zh"),
            // 可配置快捷键: 动作 id → chord 的 JSON 字符串(含默认补齐)
            @"shortcuts": gl_shortcuts_effective_json(),
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
    // 录制会话收尾交给 core 状态机(D8b): 关闭时直发静默丢弃, 草稿走完整
    // StopChatDraft(提交在写笔画 + half-close + 按 axum 裁决提示);GIF 不受影响。
    gl_recording_chat_integration_changed(on);
    if (!on) {
        // 共享上行一并停止
        glaspen2_share_ink_set_active(NO);
    } else if (glaspen2_load_bool_setting("share_ink")) {
        // 重新开启集成:共享上行开关若此前是开的,一并恢复
        glaspen2_share_ink_set_active(YES);
    }
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
                show_notification(L(@"没有笔迹或导出失败", @"No strokes or export failed"));
            }
        });
    });
    return ok;
}

// --- Unified settings functions (single source of truth) ---

static void gl_settings_set_color(int idx) {
    if (idx < 0 || idx >= g_color_preset_count) return;
    // 书写中换色: 存 pending, 抬笔后再应用 —— 否则当前笔的尾部会被改色。
    if (g_stroke_active) {
        g_pending_color_index = idx;
        return;
    }
    if (idx != g_selectedColorIndex) g_prev_color_index = g_selectedColorIndex;
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
    if (g_stroke_active) {
        g_pending_width_index = idx;
        return;
    }
    if (idx != g_selected_width_index) g_prev_width_index = g_selected_width_index;
    g_width_scale = g_width_presets[idx];
    g_selected_width_index = idx;
    glaspen2_save_settings(g_pen_r, g_pen_g, g_pen_b, g_width_scale);
    update_menu_checkmarks();
    update_status_icon_state(); // 笔宽画在图标笔杆粗细里
    sync_settings_panel();
}

/// 抬笔后应用书写中按下的换色/换粗细(只保留最后一次)。
static void gl_settings_apply_pending_color_width(void) {
    if (g_stroke_active) return;
    if (g_pending_color_index >= 0) {
        int idx = g_pending_color_index;
        g_pending_color_index = -1;
        gl_settings_set_color(idx);
    }
    if (g_pending_width_index >= 0) {
        int idx = g_pending_width_index;
        g_pending_width_index = -1;
        gl_settings_set_width(idx);
    }
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
    NSMenuItem *item = gl_menu_item_with_tag(777);
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
    NSMenuItem *gi = gl_menu_item_with_tag(444);
    [gi setState:g_glass_enabled ? NSControlStateValueOn : NSControlStateValueOff];
    if (g_glass_toggle) g_glass_toggle.state = g_glass_enabled ? NSControlStateValueOn : NSControlStateValueOff;
    g_glass_buttons[0].state = (fabs(g_glass_opacity - 0.50) < 0.001) ? NSControlStateValueOn : NSControlStateValueOff;
}

static void gl_settings_set_glass_enabled(BOOL on) {
    g_glass_enabled = on;
    glaspen2_save_bool_setting("glass_enabled", on ? 1 : 0);
    gl_glass_apply();
}

static void gl_settings_set_enabled(BOOL on) {
    g_enabled = on;
    NSMenuItem *item = gl_menu_item_with_tag(888);
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

