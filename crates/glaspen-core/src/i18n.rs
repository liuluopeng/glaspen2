//! 用户可见文案表(中/英)的唯一来源: **D10a 建表 + FFI, D10b 两端接线完成**。
//!
//! 背景: 两端曾各自硬编码用户可见文案 —— macOS `src/macos/**/*.m` 里
//! `L(@"中文", @"English")` 共 105 对(去重 77 组); Windows
//! `crates/glaspen-windows/src/overlay/*.rs` 里 161 条唯一中文字面量
//! (多数是 tracing 日志, 用户可见约 60–70 条)且只有中文。本模块把
//! **两壳真正共享的语义文案**收成一张表; 两端现在只查这张表。
//!
//! ## 切片记录
//! - D10a(2026-11-22): 表 + 查表 API + 审计 FFI(`glaspen2_i18n` /
//!   `glaspen2_i18n_catalog_json`, 按既有分工放在 FFI 大本营 `export/mod.rs`,
//!   本模块只管纯数据/纯函数); 当时两端仍用硬编码, 反向源清单闸门校验表里
//!   每条文案都真实存在于两端源码(防"表里造词/孤儿 key")。
//! - D10b(2026-11-22): 两端接线 —— macOS 加 `gl_text`/`gl_textf`(经 FFI
//!   `glaspen2_i18n` 按 key 现查, 语言现读 `g_lang`, **不缓存文案**),
//!   Windows overlay 直接 `i18n::text`/`i18n::format`(覆盖层只有中文 HUD,
//!   固定 `Lang::Zh`; 面板语言设置只影响独立的 Flutter 面板进程); `{keys}`
//!   由 adapter 传平台键名(⌘⌃3 ↔ Ctrl+Alt+3)。反向闸门同时换成正向闸门
//!   `adapters_take_shared_messages_only_from_i18n`: 60 条共享文案的
//!   zh(两端)/en(两端)不得再以硬编码字面量出现在 adapter, 且两端必须引用
//!   core i18n 入口; 豁免只限登记在闸门 ALLOWLIST 的非用户文案(内部 wire
//!   名 / Windows 日志串)。
//!
//! ## house style: 枚举 + 稳定 key
//! 与 `action_effect::ActionEffect` / `recording::RecordingHud` 一致:
//! `Msg` 是穷举枚举(编译器保证每个语义都有 zh/en —— 漏写文案不可能通过
//! 编译), `Msg::key()` 是给 FFI/日志/闸门用的**稳定 wire key**(点分小写,
//! 按区域分组)。`key()` 一旦发布**不得改名**(适配器按它查表), 枚举变体名
//! 只是内部实现细节, 不承诺稳定。没有采用 `struct Key(&'static str)` +
//! 常量的写法: 常量表靠人肉查重, 枚举 + [`Msg::ALL`] 能穷举测试
//! (非空/唯一/key 形状/语言映射全覆盖)。
//!
//! ## 占位符
//! 文案里统一用 `{name}`(不是 printf 的 `%@`/`%d`, 也不是 `format!` 的 `{}`):
//! - `{keys}`: 平台键名(macOS `⌘⌃3` / Windows `Ctrl+Alt+3`),
//! - `{n}`: 笔数/条数(`SendSucceeded` / `DraftAccepted`),
//! - `{color}` / `{width}`: 颜色/粗细显示名(见 `Msg::ColorRed` 等)。
//!
//! 用 [`format`] 按名字替换; 未提供的占位符**原样保留**(便于发现漏传), 不是错误。
//! 每条带占位符的 `Msg` 在变体注释里写明参数, 也可用 [`Msg::placeholders`] 查。
//!
//! ## 语言映射
//! [`Lang::from_code`] 只有 `"en"` 是英文, 其余(空串/`"zh"`/未知/`"EN"`)一律
//! 中文 —— 与 `crate::settings_apply` 的语言归一(`language_from`)及两端现状
//! 严格一致(macOS `g_lang` 默认中文; 未知设置值按默认中文)。
//!
//! ## 收录边界
//! 只收**两壳都有、语义相同**的文案, 共 60 条: 录制 HUD(13 个
//! `RecordingHud` 变体)、页/画布 HUD、涂鸦开关/撤销、导出/保存通知、
//! 颜色/粗细显示名。
//!
//! 未收录(**故意留在 adapter**, D10b 接线时逐条保持原样; 都是单端独有或
//! 两端文案有意不同):
//! - macOS 菜单栏独有(Windows 没有菜单栏/托盘, 已读码确认): 保存(含背景)/
//!   保存(涂鸦)/新建画布/导出/保存笔记 (Xournal)/导出无限画布 PDF/SVG/
//!   设置.../更多/开机自启/磨砂玻璃/笔迹描边/开启|关闭涂鸦/English|中文/退出
//!   等 ~17 条菜单项。
//! - macOS 独有的截图失败/导出到桌面/辅助功能权限/单实例提示/清空无限画布
//!   提示等。
//! - Windows 独有的压力监控/模糊背景开关提示、"自由涂鸦只有一张画布"、
//!   "PDF 已保存到桌面"/"动画 GIF 已保存到桌面"/页切换调试串等。
//! - 两端语义相同但文案**有意不同**的("动画 GIF 已保存并复制到剪贴板" mac
//!   vs "动画 GIF 已保存到桌面" Windows; 带路径等 adapter 拼接串)——
//!   保留各自文案, 不拆 key。
//! - 键名整段嵌在文案里的三条录制提示已用 `{keys}` 占位符解决平台差异。
//! - `pageinfo.rs`(页信息/日期)已经是 core 单源, 不重复收。

/// 界面语言。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lang {
    /// 中文(默认)。
    Zh,
    /// 英文。
    En,
}

impl Lang {
    /// 设置/FFI 里的语言代码。
    pub const fn code(self) -> &'static str {
        match self {
            Self::Zh => "zh",
            Self::En => "en",
        }
    }

    /// 语言代码 → 语言。只有 "en" 是英文, 其余(含 "zh"/空串/未知)一律中文。
    pub fn from_code(code: &str) -> Self {
        if code == "en" { Self::En } else { Self::Zh }
    }
}

/// 一条用户可见语义和它的中/英文案。
///
/// 变体按区域分组; 声明顺序 = [Msg::ALL] 顺序。带占位符的变体在注释里
/// 写明参数名(见模块文档"占位符")。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Msg {
    // ── 录制 HUD(13 个 RecordingHud 变体) ───────────────────────────
    /// GIF 录制开始提示。
    GifStartHint,
    /// 直发录制开始提示;占位符 {keys} = 平台直发键名。
    SendStartHint,
    /// 草稿录制开始提示;占位符 {keys} = 平台草稿键名。
    DraftStartHint,
    /// 草稿通道打开失败。
    DraftStartFailed,
    /// GIF 已复制到剪贴板。
    GifCopied,
    /// GIF 没有笔迹或导出失败。
    GifNoStrokesOrExportFailed,
    /// GIF 写剪贴板失败(目前只有 Windows 会显示)。
    GifClipboardFailed,
    /// 直发没有新手写内容。
    SendNoStrokes,
    /// 直发成功;占位符 {n} = 发送笔数。
    SendSucceeded,
    /// 直发失败。
    SendFailed,
    /// 草稿被对方接受;占位符 {n} = 接受笔数。
    DraftAccepted,
    /// 草稿被对方放弃。
    DraftDeclined,
    /// 草稿通道失败(无 detail 时的通用文案)。
    DraftFailed,

    // ── 画布模式 / 页导航 HUD ───────────────────────────────────────
    /// 无限画布已开启(附镜头键提示)。
    InfiniteCanvasOn,
    /// 无限画布已关闭。
    InfiniteCanvasOff,
    /// 无限画布不会被清除, 新建请到设置面板。
    InfiniteCanvasKept,
    /// 不能连续创建空白画布。
    BlankCanvasGuard,
    /// 无限画布只有一个画布。
    SingleInfiniteCanvas,
    /// 画布模式名: 无限画布(Windows 面板切换提示用)。
    InfiniteCanvasLabel,
    /// 飘渺画布涂鸦模式。
    EtherealCanvasMode,
    /// 固定画布涂鸦模式。
    FixedCanvasMode,
    /// 已自动新建一页。
    PageAutoCreated,
    /// 已新建了一页。
    PageCreated,
    /// 新建页失败。
    PageCreateFailed,
    /// 已是最新一页。
    PageAlreadyLatest,
    /// 没有上一页。
    NoPreviousPage,
    /// 没有下一页(再按一次新建)。
    NoNextPage,
    /// 已达最大缩放 100%。
    ZoomMax100,

    // ── 涂鸦开关 / 撤销 ────────────────────────────────────────────
    /// 涂鸦已开启。
    DrawingEnabled,
    /// 涂鸦已关闭。
    DrawingDisabled,
    /// 正在书写中。
    StrokeInProgress,
    /// 没有可撤销的笔画。
    NothingToUndo,
    /// 撤销成功。
    Undone,

    // ── 导出 / 保存通知 ────────────────────────────────────────────
    /// 截图已保存。
    ScreenshotSaved,
    /// 截图(含背景)已保存。
    ScreenshotWithBackgroundSaved,
    /// 笔记已保存。
    NotesSaved,
    /// 已导出 SVG + GIF。
    SvgGifSaved,
    /// 导出失败。
    ExportFailed,
    /// SVG 已复制到剪贴板。
    SvgCopied,
    /// 没有笔迹可复制。
    NoStrokesToCopy,

    // ── 颜色 / 粗细切换 HUD(占位符 = 下面两组显示名) ───────────────
    /// 颜色切换提示;占位符 {color} = [COLOR_NAMES] 里的显示名。
    ColorChanged,
    /// 粗细切换提示;占位符 {width} = [WIDTH_NAMES] 里的显示名。
    WidthChanged,

    // ── 颜色预设显示名(索引 = presets::COLOR_PRESETS 下标) ─────────
    /// 红。
    ColorRed,
    /// 橙。
    ColorOrange,
    /// 黄。
    ColorYellow,
    /// 绿。
    ColorGreen,
    /// 青。
    ColorCyan,
    /// 蓝。
    ColorBlue,
    /// 紫。
    ColorPurple,
    /// 粉。
    ColorPink,
    /// 白。
    ColorWhite,
    /// 黑。
    ColorBlack,

    // ── 粗细预设显示名(索引 = presets::WIDTH_PRESETS 下标) ─────────
    /// 极细。
    WidthHair,
    /// 很细。
    WidthVeryFine,
    /// 细。
    WidthFine,
    /// 中。
    WidthMedium,
    /// 粗。
    WidthThick,
    /// 很粗。
    WidthVeryThick,
    /// 超粗。
    WidthExtraThick,
    /// 极粗。
    WidthBoldest,
}

/// 颜色预设显示名, 索引 = [crate::presets::COLOR_PRESETS] 下标(两端菜单/HUD 同序)。
/// macOS 只保留稳定 key 表、经 `gl_text` 查这里; Windows 经 `i18n::text` 直取。
pub const COLOR_NAMES: [Msg; 10] = [
    Msg::ColorRed,
    Msg::ColorOrange,
    Msg::ColorYellow,
    Msg::ColorGreen,
    Msg::ColorCyan,
    Msg::ColorBlue,
    Msg::ColorPurple,
    Msg::ColorPink,
    Msg::ColorWhite,
    Msg::ColorBlack,
];

/// 粗细预设显示名, 索引 = [crate::presets::WIDTH_PRESETS] 下标(两端菜单/HUD 同序)。
/// macOS 只保留稳定 key 表、经 `gl_text` 查这里; Windows 经 `i18n::text` 直取。
pub const WIDTH_NAMES: [Msg; 8] = [
    Msg::WidthHair,
    Msg::WidthVeryFine,
    Msg::WidthFine,
    Msg::WidthMedium,
    Msg::WidthThick,
    Msg::WidthVeryThick,
    Msg::WidthExtraThick,
    Msg::WidthBoldest,
];

impl Msg {
    /// 全表(测试/审计遍历用; 顺序 = 变体声明顺序)。
    pub const ALL: &[Msg] = &[
        Self::GifStartHint,
        Self::SendStartHint,
        Self::DraftStartHint,
        Self::DraftStartFailed,
        Self::GifCopied,
        Self::GifNoStrokesOrExportFailed,
        Self::GifClipboardFailed,
        Self::SendNoStrokes,
        Self::SendSucceeded,
        Self::SendFailed,
        Self::DraftAccepted,
        Self::DraftDeclined,
        Self::DraftFailed,
        Self::InfiniteCanvasOn,
        Self::InfiniteCanvasOff,
        Self::InfiniteCanvasKept,
        Self::BlankCanvasGuard,
        Self::SingleInfiniteCanvas,
        Self::InfiniteCanvasLabel,
        Self::EtherealCanvasMode,
        Self::FixedCanvasMode,
        Self::PageAutoCreated,
        Self::PageCreated,
        Self::PageCreateFailed,
        Self::PageAlreadyLatest,
        Self::NoPreviousPage,
        Self::NoNextPage,
        Self::ZoomMax100,
        Self::DrawingEnabled,
        Self::DrawingDisabled,
        Self::StrokeInProgress,
        Self::NothingToUndo,
        Self::Undone,
        Self::ScreenshotSaved,
        Self::ScreenshotWithBackgroundSaved,
        Self::NotesSaved,
        Self::SvgGifSaved,
        Self::ExportFailed,
        Self::SvgCopied,
        Self::NoStrokesToCopy,
        Self::ColorChanged,
        Self::WidthChanged,
        Self::ColorRed,
        Self::ColorOrange,
        Self::ColorYellow,
        Self::ColorGreen,
        Self::ColorCyan,
        Self::ColorBlue,
        Self::ColorPurple,
        Self::ColorPink,
        Self::ColorWhite,
        Self::ColorBlack,
        Self::WidthHair,
        Self::WidthVeryFine,
        Self::WidthFine,
        Self::WidthMedium,
        Self::WidthThick,
        Self::WidthVeryThick,
        Self::WidthExtraThick,
        Self::WidthBoldest,
    ];

    /// 稳定 wire key(点分小写, 按区域分组)。**一旦发布不得改名**。
    pub const fn key(self) -> &'static str {
        match self {
            Self::GifStartHint => "recording.gif_start_hint",
            Self::SendStartHint => "recording.send_start_hint",
            Self::DraftStartHint => "recording.draft_start_hint",
            Self::DraftStartFailed => "recording.draft_start_failed",
            Self::GifCopied => "recording.gif_copied",
            Self::GifNoStrokesOrExportFailed => "recording.gif_no_strokes_or_export_failed",
            Self::GifClipboardFailed => "recording.gif_clipboard_failed",
            Self::SendNoStrokes => "recording.send_no_strokes",
            Self::SendSucceeded => "recording.send_succeeded",
            Self::SendFailed => "recording.send_failed",
            Self::DraftAccepted => "recording.draft_accepted",
            Self::DraftDeclined => "recording.draft_declined",
            Self::DraftFailed => "recording.draft_failed",
            Self::InfiniteCanvasOn => "canvas.infinite_on",
            Self::InfiniteCanvasOff => "canvas.infinite_off",
            Self::InfiniteCanvasKept => "canvas.infinite_kept",
            Self::BlankCanvasGuard => "canvas.blank_guard",
            Self::SingleInfiniteCanvas => "canvas.single_canvas_warning",
            Self::InfiniteCanvasLabel => "canvas.mode_infinite",
            Self::EtherealCanvasMode => "canvas.ethereal_mode",
            Self::FixedCanvasMode => "canvas.fixed_mode",
            Self::PageAutoCreated => "page.auto_created",
            Self::PageCreated => "page.created",
            Self::PageCreateFailed => "page.create_failed",
            Self::PageAlreadyLatest => "page.already_latest",
            Self::NoPreviousPage => "page.no_previous",
            Self::NoNextPage => "page.no_next",
            Self::ZoomMax100 => "zoom.max_100",
            Self::DrawingEnabled => "draw.enabled",
            Self::DrawingDisabled => "draw.disabled",
            Self::StrokeInProgress => "draw.stroke_in_progress",
            Self::NothingToUndo => "draw.nothing_to_undo",
            Self::Undone => "draw.undone",
            Self::ScreenshotSaved => "export.screenshot_saved",
            Self::ScreenshotWithBackgroundSaved => "export.screenshot_with_bg_saved",
            Self::NotesSaved => "export.notes_saved",
            Self::SvgGifSaved => "export.svg_gif_saved",
            Self::ExportFailed => "export.failed",
            Self::SvgCopied => "export.svg_copied",
            Self::NoStrokesToCopy => "export.no_strokes_to_copy",
            Self::ColorChanged => "hud.color_changed",
            Self::WidthChanged => "hud.width_changed",
            Self::ColorRed => "color.red",
            Self::ColorOrange => "color.orange",
            Self::ColorYellow => "color.yellow",
            Self::ColorGreen => "color.green",
            Self::ColorCyan => "color.cyan",
            Self::ColorBlue => "color.blue",
            Self::ColorPurple => "color.purple",
            Self::ColorPink => "color.pink",
            Self::ColorWhite => "color.white",
            Self::ColorBlack => "color.black",
            Self::WidthHair => "width.hair",
            Self::WidthVeryFine => "width.very_fine",
            Self::WidthFine => "width.fine",
            Self::WidthMedium => "width.medium",
            Self::WidthThick => "width.thick",
            Self::WidthVeryThick => "width.very_thick",
            Self::WidthExtraThick => "width.extra_thick",
            Self::WidthBoldest => "width.boldest",
        }
    }

    /// 中文文案;{...} 占位符见模块文档。
    pub const fn zh(self) -> &'static str {
        match self {
            Self::GifStartHint => "按住绘制, 松开生成 GIF",
            Self::SendStartHint => "书写手写消息… 松开 {keys} 发送",
            Self::DraftStartHint => "书写手写消息(草稿)… 松开 {keys} 交给对方",
            Self::DraftStartFailed => "手写通道开启失败",
            Self::GifCopied => "GIF 已复制到剪贴板",
            Self::GifNoStrokesOrExportFailed => "没有笔迹或导出失败",
            Self::GifClipboardFailed => "GIF 复制失败",
            Self::SendNoStrokes => "没有新手写内容",
            Self::SendSucceeded => "手写消息已发送 ({n} 笔)",
            Self::SendFailed => "手写消息发送失败",
            Self::DraftAccepted => "对方已接收手写草稿 ({n} 笔)",
            Self::DraftDeclined => "对方未采用这份手写草稿",
            Self::DraftFailed => "手写通道失败(未连接或中断)",
            Self::InfiniteCanvasOn => "无限画布已开启 (镜头键可在设置 → 快捷键中查看)",
            Self::InfiniteCanvasOff => "无限画布已关闭",
            Self::InfiniteCanvasKept => "无限画布不会被清除 · 新建请到设置面板手动新建",
            Self::BlankCanvasGuard => "不能连续创建空白画布, 请先涂鸦",
            Self::SingleInfiniteCanvas => "无限画布只有一个画布",
            Self::InfiniteCanvasLabel => "无限画布",
            Self::EtherealCanvasMode => "飘渺画布涂鸦模式 (悬空/落笔显示)",
            Self::FixedCanvasMode => "固定画布涂鸦模式",
            Self::PageAutoCreated => "已自动新建一页",
            Self::PageCreated => "已新建了一页",
            Self::PageCreateFailed => "新建页失败",
            Self::PageAlreadyLatest => "已是最新一页",
            Self::NoPreviousPage => "没有上一页",
            Self::NoNextPage => "没有下一页(再按一次新建)",
            Self::ZoomMax100 => "已达最大缩放 100%",
            Self::DrawingEnabled => "涂鸦已开启",
            Self::DrawingDisabled => "涂鸦已关闭",
            Self::StrokeInProgress => "正在书写中",
            Self::NothingToUndo => "没有可撤销的笔画",
            Self::Undone => "撤销成功",
            Self::ScreenshotSaved => "截图成功",
            Self::ScreenshotWithBackgroundSaved => "截图成功(含背景)",
            Self::NotesSaved => "笔记已保存",
            Self::SvgGifSaved => "已导出 SVG + GIF",
            Self::ExportFailed => "导出失败",
            Self::SvgCopied => "SVG 已复制到剪贴板",
            Self::NoStrokesToCopy => "没有笔迹可复制",
            Self::ColorChanged => "颜色: {color}",
            Self::WidthChanged => "粗细: {width}",
            Self::ColorRed => "红",
            Self::ColorOrange => "橙",
            Self::ColorYellow => "黄",
            Self::ColorGreen => "绿",
            Self::ColorCyan => "青",
            Self::ColorBlue => "蓝",
            Self::ColorPurple => "紫",
            Self::ColorPink => "粉",
            Self::ColorWhite => "白",
            Self::ColorBlack => "黑",
            Self::WidthHair => "极细",
            Self::WidthVeryFine => "很细",
            Self::WidthFine => "细",
            Self::WidthMedium => "中",
            Self::WidthThick => "粗",
            Self::WidthVeryThick => "很粗",
            Self::WidthExtraThick => "超粗",
            Self::WidthBoldest => "极粗",
        }
    }

    /// 英文文案;占位符与 [Self::zh] 一致。
    pub const fn en(self) -> &'static str {
        match self {
            Self::GifStartHint => "Hold & draw, release to make GIF",
            Self::SendStartHint => "Writing handwriting… release {keys} to send",
            Self::DraftStartHint => "Writing handwriting draft… release {keys} to hand over",
            Self::DraftStartFailed => "Failed to open handwriting draft channel",
            Self::GifCopied => "GIF copied to clipboard",
            Self::GifNoStrokesOrExportFailed => "No strokes or export failed",
            Self::GifClipboardFailed => "Failed to copy GIF",
            Self::SendNoStrokes => "No new strokes",
            Self::SendSucceeded => "Handwriting sent ({n} strokes)",
            Self::SendFailed => "Handwriting send failed",
            Self::DraftAccepted => "Handwriting draft accepted ({n} strokes)",
            Self::DraftDeclined => "Handwriting draft declined",
            Self::DraftFailed => "Handwriting channel failed (not connected or broken)",
            Self::InfiniteCanvasOn => {
                "Infinite canvas on (camera keys are in Settings → Shortcuts)"
            }
            Self::InfiniteCanvasOff => "Infinite canvas off",
            Self::InfiniteCanvasKept => "Infinite canvas is kept — create a new one in settings",
            Self::BlankCanvasGuard => "Canvas is blank — draw something first",
            Self::SingleInfiniteCanvas => "Infinite canvas has a single canvas",
            Self::InfiniteCanvasLabel => "Infinite canvas",
            Self::EtherealCanvasMode => "Ethereal canvas mode (hover/down to show)",
            Self::FixedCanvasMode => "Fixed canvas mode",
            Self::PageAutoCreated => "Auto-created a new page",
            Self::PageCreated => "A new page was created",
            Self::PageCreateFailed => "Could not create a page",
            Self::PageAlreadyLatest => "Already on the latest page",
            Self::NoPreviousPage => "No previous page",
            Self::NoNextPage => "No next page (press again to create)",
            Self::ZoomMax100 => "Max zoom 100%",
            Self::DrawingEnabled => "Drawing enabled",
            Self::DrawingDisabled => "Drawing disabled",
            Self::StrokeInProgress => "Stroke in progress",
            Self::NothingToUndo => "Nothing to undo",
            Self::Undone => "Undo",
            Self::ScreenshotSaved => "Saved",
            Self::ScreenshotWithBackgroundSaved => "Saved (with background)",
            Self::NotesSaved => "Notes saved",
            Self::SvgGifSaved => "SVG + GIF saved",
            Self::ExportFailed => "Export failed",
            Self::SvgCopied => "SVG copied to clipboard",
            Self::NoStrokesToCopy => "No strokes to copy",
            Self::ColorChanged => "Color: {color}",
            Self::WidthChanged => "Width: {width}",
            Self::ColorRed => "Red",
            Self::ColorOrange => "Orange",
            Self::ColorYellow => "Yellow",
            Self::ColorGreen => "Green",
            Self::ColorCyan => "Cyan",
            Self::ColorBlue => "Blue",
            Self::ColorPurple => "Purple",
            Self::ColorPink => "Pink",
            Self::ColorWhite => "White",
            Self::ColorBlack => "Black",
            Self::WidthHair => "Hair",
            Self::WidthVeryFine => "Very fine",
            Self::WidthFine => "Fine",
            Self::WidthMedium => "Medium",
            Self::WidthThick => "Thick",
            Self::WidthVeryThick => "Very thick",
            Self::WidthExtraThick => "Extra thick",
            Self::WidthBoldest => "Boldest",
        }
    }

    /// 按语言取文案(等价于 [text])。
    pub const fn text(self, lang: Lang) -> &'static str {
        match lang {
            Lang::Zh => self.zh(),
            Lang::En => self.en(),
        }
    }

    /// 本条文案的占位符名(按声明顺序;无占位符返回空切片)。
    pub const fn placeholders(self) -> &'static [&'static str] {
        match self {
            Self::SendStartHint | Self::DraftStartHint => &["keys"],
            Self::SendSucceeded | Self::DraftAccepted => &["n"],
            Self::ColorChanged => &["color"],
            Self::WidthChanged => &["width"],
            _ => &[],
        }
    }

    /// 按名字替换占位符;未提供的占位符原样保留(不是错误)。
    /// 不配对的 { 也原样保留。
    pub fn format(self, lang: Lang, args: &[(&str, &str)]) -> String {
        let source = self.text(lang);
        let mut out = String::with_capacity(source.len());
        let mut rest = source;
        while let Some(start) = rest.find('{') {
            out.push_str(&rest[..start]);
            let after = &rest[start + 1..];
            let Some(end) = after.find('}') else {
                out.push_str(&rest[start..]);
                return out;
            };
            let name = &after[..end];
            if let Some((_, value)) = args.iter().find(|(key, _)| *key == name) {
                out.push_str(value);
            } else {
                out.push('{');
                out.push_str(name);
                out.push('}');
            }
            rest = &after[end + 1..];
        }
        out.push_str(rest);
        out
    }
}

/// 按语言取文案(与 [Msg::text] 等价; 适配器/FFI 的统一入口)。
pub fn text(lang: Lang, msg: &Msg) -> &'static str {
    msg.text(lang)
}

/// 按名字替换占位符(见模块文档"占位符"; 与 [Msg::format] 等价)。
pub fn format(lang: Lang, msg: &Msg, args: &[(&str, &str)]) -> String {
    msg.format(lang, args)
}

/// 稳定 key → 文案语义;未知 key 返回 None。
pub fn by_key(key: &str) -> Option<Msg> {
    Msg::ALL.iter().copied().find(|msg| msg.key() == key)
}

/// 全表审计 JSON: {"<key>":{"zh":"...","en":"..."},...}, 单行, key 字典序
/// (serde_json 默认 Map 为 BTreeMap)。FFI glaspen2_i18n_catalog_json 直接转发。
pub fn catalog_json() -> String {
    let mut map = serde_json::Map::new();
    for msg in Msg::ALL {
        map.insert(
            msg.key().to_string(),
            serde_json::json!({ "zh": msg.zh(), "en": msg.en() }),
        );
    }
    serde_json::Value::Object(map).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// 文案里出现的占位符(按出现顺序)。
    fn placeholders_in(text: &str) -> Vec<&str> {
        let mut out = Vec::new();
        let mut rest = text;
        while let Some(start) = rest.find('{') {
            let after = &rest[start + 1..];
            let Some(end) = after.find('}') else { break };
            out.push(&after[..end]);
            rest = &after[end + 1..];
        }
        out
    }

    /// 抽取源码里的字符串字面量(剥注释; ObjC @"..." 与 C/Rust "..." 同形)。
    /// 返回 (起始字节偏移, 字面量); 偏移只给闸门报行号用。
    ///
    /// 只服务本闸门, 不是完整词法分析: 认识 //、/* */、普通字符串(带反斜杠
    /// 转义)与字符字面量('"' / '\n' / 生命周期 'a 不会误吞后续代码);
    /// 仓库里这两棵 adapter 树没有 raw string, 需要时再补。
    fn string_literals(src: &str) -> Vec<(usize, String)> {
        let bytes = src.as_bytes();
        let mut out: Vec<(usize, String)> = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'/' if bytes.get(i + 1) == Some(&b'/') => {
                    while i < bytes.len() && bytes[i] != b'\n' {
                        i += 1;
                    }
                }
                b'/' if bytes.get(i + 1) == Some(&b'*') => {
                    i += 2;
                    while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                        i += 1;
                    }
                    i = (i + 2).min(bytes.len());
                }
                b'\'' => {
                    // 字符字面量 / 生命周期: 跳过, 别把 '"' 的引号当成字符串起点。
                    if bytes.get(i + 1) == Some(&b'\\') || bytes.get(i + 2) == Some(&b'\'') {
                        i += 3;
                    } else {
                        i += 1;
                    }
                }
                b'"' => {
                    let start = i;
                    i += 1;
                    let mut lit = String::new();
                    while i < bytes.len() {
                        let b = bytes[i];
                        if b == b'\\' {
                            if let Some(&next) = bytes.get(i + 1) {
                                lit.push('\\');
                                lit.push(next as char);
                            }
                            i += 2;
                        } else if b == b'"' {
                            i += 1;
                            break;
                        } else if let Some(ch) = src[i..].chars().next() {
                            lit.push(ch);
                            i += ch.len_utf8();
                        } else {
                            break;
                        }
                    }
                    out.push((start, lit));
                }
                _ => i += 1,
            }
        }
        out
    }

    /// 文案按 {} 拆成静态片段(占位符可替换; 片段按序出现在旧格式串里)。
    fn template_segments(text: &str) -> Vec<&str> {
        let mut out = Vec::new();
        let mut rest = text;
        while let Some(start) = rest.find('{') {
            out.push(&rest[..start]);
            let after = &rest[start + 1..];
            let Some(end) = after.find('}') else {
                out.push(&rest[start..]);
                return out;
            };
            rest = &after[end + 1..];
        }
        out.push(rest);
        out.into_iter().filter(|s| !s.is_empty()).collect()
    }

    /// 旧 adapter 硬编码格式串是否命中模板: 首段开头 + 其余片段按序出现。
    /// (要求"首段开头"才能避开 Windows 日志里"…({count} 笔)"这类子串误报。)
    fn matches_template(literal: &str, segments: &[&str]) -> bool {
        let Some((first, rest_segments)) = segments.split_first() else {
            return false;
        };
        let Some(mut rest) = literal.strip_prefix(first) else {
            return false;
        };
        for segment in rest_segments {
            let Some(pos) = rest.find(segment) else {
                return false;
            };
            rest = &rest[pos + segment.len()..];
        }
        true
    }

    /// 字节偏移 → 1-based 行号(闸门报错定位用)。
    fn line_of(src: &str, offset: usize) -> usize {
        src.as_bytes()[..offset]
            .iter()
            .filter(|b| **b == b'\n')
            .count()
            + 1
    }

    /// 剥注释后的源码(字符串内部原样保留, 不把 URL/正则里的 // 当注释)。
    /// 只给"必须引用 core i18n 入口"的 token 断言用 —— 注释里提到不算。
    fn strip_comments(src: &str) -> String {
        let bytes = src.as_bytes();
        let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'/' if bytes.get(i + 1) == Some(&b'/') => {
                    while i < bytes.len() && bytes[i] != b'\n' {
                        i += 1;
                    }
                }
                b'/' if bytes.get(i + 1) == Some(&b'*') => {
                    i += 2;
                    while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                        i += 1;
                    }
                    i = (i + 2).min(bytes.len());
                }
                b'"' => {
                    let start = i;
                    i += 1;
                    while i < bytes.len() {
                        match bytes[i] {
                            b'\\' => i += 2,
                            b'"' => {
                                i += 1;
                                break;
                            }
                            _ => i += 1,
                        }
                    }
                    out.extend_from_slice(&bytes[start..i.min(bytes.len())]);
                }
                b'\'' => {
                    // 字符字面量 / 生命周期: 原样保留(与 string_literals 同一判定)。
                    let skip =
                        if bytes.get(i + 1) == Some(&b'\\') || bytes.get(i + 2) == Some(&b'\'') {
                            3
                        } else {
                            1
                        };
                    let end = (i + skip).min(bytes.len());
                    out.extend_from_slice(&bytes[i..end]);
                    i = end;
                }
                b => {
                    out.push(b);
                    i += 1;
                }
            }
        }
        String::from_utf8(out).unwrap_or_default()
    }

    #[test]
    fn every_message_has_zh_en_unique_key_and_known_lookup() {
        assert_eq!(
            Msg::ALL.len(),
            60,
            "D10a 收录 60 条; 增删 key 必须同步更新本断言与台账"
        );
        let mut seen = BTreeSet::new();
        for msg in Msg::ALL {
            assert!(!msg.zh().is_empty(), "{} 缺中文文案", msg.key());
            assert!(!msg.en().is_empty(), "{} 缺英文文案", msg.key());
            assert!(seen.insert(msg.key()), "稳定 key 重复: {}", msg.key());
            assert_eq!(by_key(msg.key()), Some(*msg), "{} 查表往返失败", msg.key());
            assert_eq!(msg.text(Lang::Zh), msg.zh());
            assert_eq!(msg.text(Lang::En), msg.en());
            assert_eq!(text(Lang::Zh, msg), msg.zh());
            // key 形状: 小写 ASCII + 数字 + ./_(FFI/日志/闸门都按原样用)。
            assert!(
                msg.key().bytes().all(|b| b.is_ascii_lowercase()
                    || b.is_ascii_digit()
                    || b == b'.'
                    || b == b'_'),
                "{} 不是点分小写 key",
                msg.key()
            );
            assert!(
                msg.key().contains('.'),
                "{} 应带区域前缀(如 recording./canvas.)",
                msg.key()
            );
        }
        assert_eq!(seen.len(), Msg::ALL.len());
        assert_eq!(by_key("no.such.key"), None);
    }

    #[test]
    fn language_mapping_matches_shell_behavior() {
        assert_eq!(Lang::from_code("en"), Lang::En);
        assert_eq!(Lang::from_code("zh"), Lang::Zh);
        // 两端现状: 只有精确 "en" 是英文, 其余(空/未知/大小写)按默认中文。
        for code in ["", "EN", "En", " en", "en ", "fr", "zh-CN", "english"] {
            assert_eq!(Lang::from_code(code), Lang::Zh, "{code:?} 应按默认中文");
        }
        assert_eq!(Lang::from_code(Lang::En.code()), Lang::En);
        assert_eq!(Lang::from_code(Lang::Zh.code()), Lang::Zh);
        assert_eq!(Lang::Zh.code(), "zh");
        assert_eq!(Lang::En.code(), "en");
    }

    #[test]
    fn placeholders_are_documented_and_format_replaces_them() {
        for msg in Msg::ALL {
            let declared = msg.placeholders();
            for source in [msg.zh(), msg.en()] {
                assert_eq!(
                    placeholders_in(source),
                    declared,
                    "{} 文案里的占位符与 placeholders() 不一致: {source}",
                    msg.key()
                );
                // 不传参数 → 原样返回(未提供占位符保留)。
                assert_eq!(msg.format(Lang::Zh, &[]), msg.zh());
                assert_eq!(msg.format(Lang::En, &[]), msg.en());
                if declared.is_empty() {
                    // 无占位符: 任何参数都不影响输出。
                    assert_eq!(
                        format(Lang::Zh, msg, &[("keys", "X"), ("n", "1")]),
                        msg.zh()
                    );
                } else {
                    let args: Vec<(&str, &str)> =
                        declared.iter().map(|name| (*name, "X")).collect();
                    for lang in [Lang::Zh, Lang::En] {
                        let rendered = format(lang, msg, &args);
                        assert!(
                            !rendered.contains('{') && !rendered.contains('}'),
                            "{} 占位符未替换干净: {rendered}",
                            msg.key()
                        );
                    }
                }
            }
        }
        // 具体格式(键名/条数/颜色名走占位符, 平台差异由 adapter 传参)。
        assert_eq!(
            format(Lang::Zh, &Msg::SendSucceeded, &[("n", "3")]),
            "手写消息已发送 (3 笔)"
        );
        assert_eq!(
            format(Lang::En, &Msg::DraftAccepted, &[("n", "2")]),
            "Handwriting draft accepted (2 strokes)"
        );
        assert_eq!(
            format(Lang::Zh, &Msg::SendStartHint, &[("keys", "⌘⌃3")]),
            "书写手写消息… 松开 ⌘⌃3 发送"
        );
        assert_eq!(
            format(Lang::En, &Msg::SendStartHint, &[("keys", "Ctrl+Alt+3")]),
            "Writing handwriting… release Ctrl+Alt+3 to send"
        );
        assert_eq!(
            format(Lang::Zh, &Msg::ColorChanged, &[("color", "红")]),
            "颜色: 红"
        );
        assert_eq!(
            format(Lang::En, &Msg::WidthChanged, &[("width", "Thick")]),
            "Width: Thick"
        );
    }

    #[test]
    fn preset_name_tables_match_presets() {
        assert_eq!(COLOR_NAMES.len(), crate::presets::COLOR_PRESETS.len());
        assert_eq!(WIDTH_NAMES.len(), crate::presets::WIDTH_PRESETS.len());
        for (i, msg) in COLOR_NAMES.iter().enumerate() {
            assert_eq!(by_key(msg.key()), Some(*msg), "颜色名表下标 {i} 无效");
        }
        for (i, msg) in WIDTH_NAMES.iter().enumerate() {
            assert_eq!(by_key(msg.key()), Some(*msg), "粗细名表下标 {i} 无效");
        }
    }

    #[test]
    fn catalog_json_is_single_line_complete_and_sorted() {
        let json = catalog_json();
        assert!(!json.contains('\n'), "审计 JSON 必须单行");
        let value: serde_json::Value = serde_json::from_str(&json).expect("审计 JSON 合法");
        let entries = value.as_object().expect("审计 JSON 是对象");
        assert_eq!(entries.len(), Msg::ALL.len());
        for msg in Msg::ALL {
            let entry = entries.get(msg.key()).expect("每条 key 都在审计表里");
            assert_eq!(entry["zh"], msg.zh());
            assert_eq!(entry["en"], msg.en());
        }
        let keys: Vec<&String> = entries.keys().collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted, "审计 JSON 的 key 应字典序稳定");
    }

    /// 正向闸门: 两端 adapter 只许通过 core i18n 入口显示 60 条共享文案。
    ///
    /// 与 D10a 的反向源清单闸门相反:
    /// - 共享文案的 zh/en 均不得再以硬编码字面量出现在 adapter(Windows 覆盖层
    ///   没有英文 UI, 但 en 也一起查: 将来加英文 UI 时同样必须走 i18n);
    /// - 无占位符文案按整串字面量相等判定; 带占位符的旧格式串(如
    ///   `颜色: %@` / `颜色: {}` / `手写消息已发送 (%d 笔)`)按模板判定:
    ///   首段开头 + 其余静态片段按序出现在同一字面量里(Windows 日志里
    ///   `…({count} 笔)` 这类子串不会误报);
    /// - 两端必须真的引用 core i18n 入口(macOS `glaspen2_i18n`/`gl_text`/
    ///   `gl_textf`; Windows `i18n::text`/`i18n::format`), 且调用点数量有下限;
    /// - macOS 的 key 是字符串(编译器不查), 再断言 60 个 key 全部以字面量出现
    ///   在 macOS 源码, 且"区域.名字"形状的字面量都能被 [by_key] 查到;
    /// - 豁免只在 [ALLOWLIST] 显式登记(文件后缀 + 字面量 + 原因)。
    ///
    /// 自检: 扫描器必须抽到已知的**未收录**文案(两端各一条), 证明扫描不是
    /// 空跑; 文件数/入口调用点都有下限。
    #[test]
    fn adapters_take_shared_messages_only_from_i18n() {
        use std::path::{Path, PathBuf};

        /// 闸门豁免: 只放过"非用户文案"的内部字符串, 必须写清原因。
        const ALLOWLIST: &[(&str, &str, &str)] = &[
            (
                "src/macos/parts/run.m",
                "Undo",
                "action_effect 的 wire kind 名(内部协议, 不是用户文案)",
            ),
            (
                "src/macos/swift/action_dispatch.swift",
                "Undo",
                "action_effect 的 wire kind 名(内部协议, 迁 Swift 后同义)",
            ),
            (
                "crates/glaspen-windows/src/overlay/pipe.rs",
                "无限画布",
                "tracing 日志里的画布模式名(非用户可见 HUD, 且不等于 canvas.mode_infinite 的用途)",
            ),
        ];

        /// 递归收集 (workspace 相对路径, 源码); 目录结构固定。
        fn collect_sources(root: &Path, dir: &Path, ext: &str, out: &mut Vec<(String, String)>) {
            let entries = std::fs::read_dir(dir).unwrap_or_else(|e| {
                panic!("i18n 闸门: 无法扫描 {}: {e}", dir.display());
            });
            for entry in entries {
                let path: PathBuf = entry.expect("目录项").path();
                if path.is_dir() {
                    collect_sources(root, &path, ext, out);
                } else if path.extension().and_then(|e| e.to_str()) == Some(ext) {
                    let rel = path
                        .strip_prefix(root)
                        .expect("源码在 workspace 内")
                        .to_string_lossy()
                        .replace('\\', "/");
                    let src = std::fs::read_to_string(&path)
                        .unwrap_or_else(|e| panic!("i18n 闸门: 读 {} 失败: {e}", path.display()));
                    out.push((rel, src));
                }
            }
        }

        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("core 在 crates/ 下")
            .parent()
            .expect("workspace 根")
            .to_path_buf();

        let mut mac_files: Vec<(String, String)> = Vec::new();
        collect_sources(
            &workspace,
            &workspace.join("src/macos"),
            "m",
            &mut mac_files,
        );
        // S3 起 macOS adapter 含 Swift 源; 文案闸门必须同时扫描 .swift,
        // 否则 key 表迁到 Swift 后会被误判为"macOS 未引用"。
        collect_sources(
            &workspace,
            &workspace.join("src/macos"),
            "swift",
            &mut mac_files,
        );
        let mut win_files: Vec<(String, String)> = Vec::new();
        collect_sources(
            &workspace,
            &workspace.join("crates/glaspen-windows/src/overlay"),
            "rs",
            &mut win_files,
        );
        assert!(
            mac_files.len() >= 4,
            "i18n 闸门: macOS 树只扫到 {} 个 .m 文件, 路径可疑",
            mac_files.len()
        );
        assert!(
            win_files.len() >= 14,
            "i18n 闸门: Windows 树只扫到 {} 个 .rs 文件, 路径可疑",
            win_files.len()
        );

        // 逐文件抽字面量(带文件/行号), 同时拼出剥注释源码供入口 token 断言。
        let scan = |files: &[(String, String)]| -> (Vec<(String, usize, String)>, String) {
            let mut lits = Vec::new();
            let mut code = String::new();
            for (rel, src) in files {
                // Swift 文件可能是很小的 helper(如 delegate 转发), 600 字节下限
                // 只对 ObjC 这种大文件防"路径扫错"; Swift 只要非空即可。
                let min_len = if rel.ends_with(".swift") { 1 } else { 500 };
                assert!(src.len() > min_len, "i18n 闸门: {rel} 过短, 扫描路径可疑");
                for (offset, lit) in string_literals(src) {
                    lits.push((rel.clone(), line_of(src, offset), lit));
                }
                code.push_str(&strip_comments(src));
                code.push('\n');
            }
            (lits, code)
        };
        let (mac_lits, mac_code) = scan(&mac_files);
        let (win_lits, win_code) = scan(&win_files);
        let allowlisted = |rel: &str, literal: &str| -> bool {
            ALLOWLIST
                .iter()
                .any(|(file, lit, _)| rel.ends_with(file) && *lit == literal)
        };

        // 1) 两端都不许再硬编码共享文案(zh + en, 剥注释后按字面量查)。
        let mut problems = Vec::new();
        for msg in Msg::ALL {
            let templated = !msg.placeholders().is_empty();
            for (lang, text) in [("zh", msg.zh()), ("en", msg.en())] {
                let segments = if templated {
                    template_segments(text)
                } else {
                    vec![text]
                };
                for (rel, line, literal) in &mac_lits {
                    let hit = if templated {
                        matches_template(literal, &segments)
                    } else {
                        literal == text
                    };
                    if hit && !allowlisted(rel, literal) {
                        problems.push(format!(
                            "{}: macOS 还硬编码 {lang} 文案 {literal:?} ({rel}:{line})",
                            msg.key()
                        ));
                    }
                }
                for (rel, line, literal) in &win_lits {
                    let hit = if templated {
                        matches_template(literal, &segments)
                    } else {
                        literal == text
                    };
                    if hit && !allowlisted(rel, literal) {
                        problems.push(format!(
                            "{}: Windows 还硬编码 {lang} 文案 {literal:?} ({rel}:{line})",
                            msg.key()
                        ));
                    }
                }
            }
        }
        assert!(
            problems.is_empty(),
            "D10b 正向闸门失败(adapter 只许走 core i18n):\n{}",
            problems.join("\n")
        );

        // 2) macOS key 是字符串(编译器不查): 60 个 key 必须全部出现在源码里,
        //    且 key 形状的字面量都必须真的在表里(拼错 key 运行时会原样显示)。
        let mac_literals: BTreeSet<&str> =
            mac_lits.iter().map(|(_, _, lit)| lit.as_str()).collect();
        let missing_keys: Vec<&str> = Msg::ALL
            .iter()
            .map(|msg| msg.key())
            .filter(|key| !mac_literals.contains(key))
            .collect();
        assert!(
            missing_keys.is_empty(),
            "i18n 闸门: macOS 源码未引用这些 key: {missing_keys:?}"
        );
        let regions: BTreeSet<&str> = Msg::ALL
            .iter()
            .filter_map(|msg| msg.key().split('.').next())
            .collect();
        for (rel, line, literal) in &mac_lits {
            let Some(first) = literal.split('.').next() else {
                continue;
            };
            if !regions.contains(first) {
                continue;
            }
            let key_shaped = literal.len() < 64
                && literal.split('.').count() >= 2
                && literal.split('.').all(|part| {
                    !part.is_empty()
                        && part
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
                });
            if key_shaped && by_key(literal).is_none() {
                problems.push(format!(
                    "macOS 出现了表里没有的 i18n key 形状字面量 {literal:?} ({rel}:{line})"
                ));
            }
        }
        assert!(
            problems.is_empty(),
            "i18n 闸门: macOS key 字面量有问题:\n{}",
            problems.join("\n")
        );

        // 3) 入口与调用点下限: 两端必须真的在查 core i18n(不是只剩几个字面量)。
        // Swift 侧经 glaspen2_menu_l10n(_pair) 桥查 core i18n, 同样计入调用点。
        let mac_calls = mac_code.matches("gl_text(").count()
            + mac_code.matches("gl_textf(").count()
            + mac_code.matches("glaspen2_menu_l10n(").count()
            + mac_code.matches("glaspen2_menu_l10n_pair(").count();
        let win_calls =
            win_code.matches("i18n::text(").count() + win_code.matches("i18n::format(").count();
        assert!(
            mac_code.contains("glaspen2_i18n("),
            "i18n 闸门: macOS 未引用 FFI glaspen2_i18n"
        );
        assert!(
            mac_code.contains("gl_textf("),
            "i18n 闸门: macOS 未引用占位符入口 gl_textf"
        );
        assert!(
            mac_calls >= 40,
            "i18n 闸门: macOS 只找到 {mac_calls} 个 gl_text/gl_textf 调用点, 少于预期"
        );
        assert!(
            win_code.contains("i18n::text("),
            "i18n 闸门: Windows 未引用 i18n::text"
        );
        assert!(
            win_code.contains("i18n::format("),
            "i18n 闸门: Windows 未引用 i18n::format"
        );
        assert!(
            win_calls >= 30,
            "i18n 闸门: Windows 只找到 {win_calls} 个 i18n::text/format 调用点, 少于预期"
        );

        // 4) 自检: 扫描器确实抽到未收录的单端文案(故意留下), 证明扫描不是空跑;
        //    平台键名确实由 adapter 作为 {keys} 参数传入, 没有写进 core 表。
        assert!(
            mac_lits.iter().any(|(_, _, lit)| lit.contains("截图失败")),
            "i18n 闸门: macOS 扫描器没抽到已知的未收录文案"
        );
        assert!(
            win_lits
                .iter()
                .any(|(_, _, lit)| lit.contains("自由涂鸦只有一张画布")),
            "i18n 闸门: Windows 扫描器没抽到已知的未收录文案"
        );
        assert!(
            mac_lits.iter().any(|(_, _, lit)| lit == "⌘⌃3"),
            "macOS 源码应含 ⌘⌃3 键名(作为 {{keys}} 参数)"
        );
        assert!(
            win_lits.iter().any(|(_, _, lit)| lit == "Ctrl+Alt+3"),
            "Windows 源码应含 Ctrl+Alt+3 键名(作为 {{keys}} 参数)"
        );
    }
}
