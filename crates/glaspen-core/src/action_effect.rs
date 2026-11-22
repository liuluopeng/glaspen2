//! 可配置动作的语义 effect 层(D3a:动作 id → 平台无关副作用)。
//!
//! 背景: 动作 id / 默认键位 / 强制绑定已由 [`crate::ui`](E1/E2)与
//! [`crate::chord`](D2)单源, 但"按下某个动作到底做什么"仍由两端各写一份分派
//! (见 `docs/duplication-backlog.md` 的 D3/D3a):
//!
//! - macOS: `src/macos/parts/run.m:1739` 的 `perform_action`(action id →
//!   `perform_hotkey(keyCode)` / 镜头专用分支 / 退出), 事件 tap
//!   (`src/macos/parts/run.m:2184-2206`)按 chord 查到 action id 后调用;
//! - Windows: `crates/glaspen-windows/src/overlay/hotkeys.rs:716` 的
//!   `dispatch_hotkey` → `:742` 的 `perform_action`(action id →
//!   `overlay/actions.rs` 各 helper / `handle_command`)。
//!
//! 本模块把 **action id → 语义** 收敛为 [`ActionEffect`]: 两端 adapter 用
//! [`effect_for_action`] 取到同一个强类型结果, 再各自执行本平台副作用
//! (窗口显隐/剪贴板/持久化/消息循环投递), 不再各写一份 action 字符串分派。
//!
//! 边界(本切片只落 core, 不接线两端):
//! - **只表达语义, 不带实现数值**: 镜头步长(80px / 1.15 倍)、末页 5 秒内
//!   二按新建页、颜色/粗细无历史时的索引回退、提示文案等仍属 adapter 或后续
//!   切片; 本层只给"做什么"与方向/种类。
//! - **不判平台可用性与前置状态**: "仅无限画布生效""书写中忽略""停用涂鸦
//!   时的按键放行/注销"等门控保留在两端现有位置(两端门控点不同, 见各变体
//!   文档), 本切片不改行为。
//! - **不统一已存在的两端行为差异**: 镜头平移方向符号、设置面板宿主形式
//!   (macOS 进程内嵌 Flutter 视图 / Windows 独立面板进程)等差异只记录在
//!   变体文档里, 接线切片(backlog D3b)按平台保留现状。
//!
//! 当前 20 个 action 全部可共享语义, 没有"仅单端"条目(所以没有 ⛔ 保留变体);
//! [`ActionEffect::variant_name`] 给每个 action 一个稳定名字, 供未来 adapter
//! 覆盖闸门(与 `settings.rs` 的 D1a 同形)按名字匹配两端分派代码。

/// 自由画布镜头平移方向: 语义方向 = 用户按下的动作方向
/// (`canvas_pan_left` → `Left`)。
///
/// **屏幕位移符号两端当前相反**(本切片不改, 接线时各保留现有符号):
/// - macOS `perform_action`(`src/macos/parts/run.m:1763-1777`)把 `Left` 翻成
///   `dx = +step`, 而 `canvas_pan_by`(`run.m:461-468`)内部是 `pan -= d`,
///   即按左 → 镜头左移 / 内容右移("视野跟随按键", 与普通看图软件一致);
/// - Windows `perform_action`(`hotkeys.rs:774-777`)把 `Left` 翻成
///   `dx = -step`, 而 `canvas_pan_by`(`actions.rs:659-668`)内部是
///   `set_cam(px - dx)`, 即按左 → pan 增加 / 内容左移("内容跟随按键";
///   2026-11 依用户反馈统一取反)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraDir {
    Left,
    Right,
    Up,
    Down,
}

impl CameraDir {
    /// 方向名(日志/测试用; [`ActionEffect::variant_name`] 拼成 `CameraPan<Dir>`)。
    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::Right => "Right",
            Self::Up => "Up",
            Self::Down => "Down",
        }
    }
}

/// 自由画布缩放方向: `canvas_zoom_in` → `In`(放大 1.15 倍), `Out` = 缩小。
///
/// 倍率/锚点/边界属 adapter(两端当前一致: 1.15、视口中心、
/// 0.05..=1.0), 见 [`ActionEffect::CameraZoom`]; 本层不携带数值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomDir {
    In,
    Out,
}

impl ZoomDir {
    /// 方向名(日志/测试用; [`ActionEffect::variant_name`] 拼成 `CameraZoom<Dir>`)。
    pub fn name(self) -> &'static str {
        match self {
            Self::In => "In",
            Self::Out => "Out",
        }
    }
}

/// 一次可配置动作的平台无关语义。
///
/// 覆盖 [`crate::ui::ACTIONS`] 的全部 20 个 action id。变体数与 action 数不
/// 相等是有意的: 4 个镜头平移动作共用 [`ActionEffect::CameraPan`] +
/// [`CameraDir`], 2 个缩放动作共用 [`ActionEffect::CameraZoom`] + [`ZoomDir`];
/// 但 [`ActionEffect::variant_name`] 对 20 个 action 给出 20 个互不相同的名字
/// (如 `CameraPanLeft`), adapter 侧可按名字逐条覆盖。
///
/// 每个变体的文档写清: 两端现在各自做什么(对应函数)、以及两端当前**真实
/// 差异**。差异只记录不消除 —— D3 的目标是单一语义层, 不是顺手改行为。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionEffect {
    /// 动作 `undo`: 撤销最后一笔。
    ///
    /// - macOS `run.m:1741` → `perform_hotkey(kVK_ANSI_Z)`(`run.m:1686-1698`):
    ///   书写中只提示"正在书写中", 否则 `glaspen2_undo_last_stroke()` + 重建;
    /// - Windows `hotkeys.rs:750` → `actions.rs:397` `undo_last_stroke`:
    ///   同一套 core FFI / 书写中保护 / 提示。
    ///
    /// 两端一致(重建表面 vs 画布是 adapter 实现细节)。
    Undo,
    /// 动作 `page_prev`: 上一页(活页本整页翻页)。
    ///
    /// - macOS `run.m:1742` → `perform_hotkey(0x32)`(`run.m:1567-1591`): 无限画布
    ///   只提示; 否则收笔 → `glaspen2_prev_screen_id` → 翻页动效 + peek 1s +
    ///   页信息; 没有上一页则提示;
    /// - Windows `hotkeys.rs:751` → `actions.rs:477` `navigate_page(false)`: 同语义
    ///   (无限画布提示 / 切页重绘 / peek / 页信息 / "没有上一页"提示)。
    ///
    /// 两端一致(整页翻转动效是 macOS 专属视觉实现)。
    PagePrev,
    /// 动作 `page_next`: 下一页(活页本整页翻页; 末页二按新建一页)。
    ///
    /// - macOS `run.m:1743` → `perform_hotkey(0x12)`(`run.m:1592-1648`): 同上一页;
    ///   末页时 5 秒内再按一次 = `glaspen2_create_next_page` 新建并翻过去;
    /// - Windows `hotkeys.rs:752` → `actions.rs:477` `navigate_page(true)`:
    ///   同语义(末页判定窗口 `NO_NEXT_SECONDS = 5s`, `actions.rs:470-471`)。
    ///
    /// 两端一致。
    PageNext,
    /// 动作 `color_swap`: 在当前颜色与上一次颜色之间切换。
    ///
    /// - macOS `run.m:1801-1810`: 取 `g_prev_color_index`; 无历史/同色时回退到
    ///   红/黑另一端(0 ↔ 9)→ `gl_settings_set_color` + 通知;
    /// - Windows `hotkeys.rs:753` → `hotkeys.rs:810` `color_swap`: 取 `prev_color`,
    ///   同一回退规则 → `CMD_SELECT_COLOR` + HUD。
    ///
    /// 语义一致; 预设表内容与提示文案留在 adapter。
    ColorSwap,
    /// 动作 `width_swap`: 在当前粗细与上一次粗细之间切换。
    ///
    /// - macOS `run.m:1811-1819`: 取 `g_prev_width_index`; 无历史/同粗细时回退
    ///   (`<= 3` → 6, 否则 → 2)→ `gl_settings_set_width` + 通知;
    /// - Windows `hotkeys.rs:754` → `hotkeys.rs:821` `width_swap`: 同一回退规则
    ///   (`<= 3` → 6, 否则 → 2)→ `CMD_SELECT_WIDTH` + HUD。
    ///
    /// 两端一致。
    WidthSwap,
    /// 动作 `toggle_draw`: 开启/停用涂鸦(覆盖层窗口显隐 + 其它动作热键注册策略)。
    ///
    /// - macOS `run.m:1744` → `perform_hotkey(kVK_ANSI_V)` →
    ///   `toggle_enabled()`(`menu.m:281`): 关闭时隐藏覆盖层窗口并注销其它热键;
    ///   重新开启时按飘渺状态决定是否立刻显示窗口;
    /// - Windows `hotkeys.rs:755` → `actions.rs:353` `toggle_enabled`: 同语义
    ///   (`ShowWindow(SW_HIDE/SW_SHOWNOACTIVATE)` + 热键重注册 + HUD)。
    ///
    /// 语义一致; 窗口显隐与热键注册方式属 OS 适配层。
    ToggleDraw,
    /// 动作 `toggle_free_canvas`: 活页本 ↔ 自由画布(无限画布)模式切换。
    ///
    /// - macOS `run.m:1745-1760`: 若面板 `show_free_canvas` 还是关的先补开(否则
    ///   面板没有自由涂鸦 tab); 下一 tick `apply_infinite_canvas(!g_infinite_canvas, YES)`;
    /// - Windows `hotkeys.rs:756-771`: 同样先补开 `show_free_canvas`, 再
    ///   `apply_infinite_canvas(..., true)`。
    ///
    /// 两端一致(含"自动打开面板开关"这层副作用; 切模式本身是 adapter 实现)。
    ToggleFreeCanvas,
    /// 动作 `canvas_pan_left/right/up/down`: 自由画布镜头平移一步。
    ///
    /// 本层只给方向, 不给步长:
    /// - 两端步长都是 `80 / zoom`(屏幕视觉距离固定): macOS `run.m:1765-1766`、
    ///   Windows `hotkeys.rs:797-801`;
    /// - 仅无限画布模式生效、书写中忽略: macOS 在事件 tap 层把组合放行给系统
    ///   (`run.m:2194-2198`), Windows 的 WM_HOTKEY 已被消费, 由
    ///   `perform_action` 顶部 early-return(`hotkeys.rs:743-748`);
    /// - **方向符号两端当前相反**(不改行为, 接线时各保留现有符号): macOS 按
    ///   "视野移动"(`Left` → `dx = +step`, `canvas_pan_by` 内 `pan -= d` → 内容
    ///   右移, `run.m:1763-1777`); Windows 按"内容跟随按键"(`Left` →
    ///   `dx = -step`, `canvas_pan_by` 内 `set_cam(px - dx)` → 内容左移,
    ///   `hotkeys.rs:774-777`; 2026-11 依用户反馈统一取反)。
    CameraPan(CameraDir),
    /// 动作 `canvas_zoom_in/out`: 自由画布镜头缩放一步(仅无限画布; 书写中忽略)。
    ///
    /// - 两端倍率都是 1.15 / (1/1.15), 锚点 = 视口中心, 范围 0.05..=1.0:
    ///   macOS `run.m:1778-1784` + `canvas_zoom_at`(`run.m:507-535`, 锚点
    ///   `g_screen_w/h * 0.5`); Windows `hotkeys.rs:778-779` `zoom_step`
    ///   (`hotkeys.rs:804-807`)+ `actions.rs:672-713` `canvas_zoom_at`
    ///   (锚点 `canvas.w/h` 中点);
    /// - 到缩放边界只提示(1.5s 节流)不重绘, 两端同款。
    ///
    /// 两端语义一致(倍率/锚点数值属 adapter, 本层不携带)。
    CameraZoom(ZoomDir),
    /// 动作 `clear`: 清空当前画布/新建一页(活页本); 无限画布不清空。
    ///
    /// - macOS `run.m:1785` → `perform_hotkey(kVK_ANSI_C)`(`run.m:1553-1563`):
    ///   先收笔; 无限画布只提示; 否则 `clear_screen()`(`glaspen2.m:722`)→
    ///   `glaspen2_clear_strokes` + 页信息 / "不能连续创建空白画布"提示;
    /// - Windows `hotkeys.rs:780` → `CMD_CLEAR_SCREEN` → `actions.rs:324-351`
    ///   `clear_screen`: 同语义(收笔 / 无限提示 / core 清空 / HUD)。
    ///
    /// 两端一致(清空操作本体在 core `glaspen2_clear_strokes`)。
    Clear,
    /// 动作 `export`: 导出 SVG + 按内容裁切的 GIF 到桌面, 并把最新 GIF 以文件
    /// 形式复制到剪贴板。
    ///
    /// - macOS `run.m:1786` → `perform_hotkey(kVK_ANSI_G)`(`run.m:1649-1685`):
    ///   `glaspen2_save_svg` + `glaspen2_save_gif_cropped`(cairo surface)→
    ///   桌面最新 GIF 写剪贴板 + 通知;
    /// - Windows `hotkeys.rs:781` → `actions.rs:1076` `export_svg_gif_clipboard`:
    ///   同语义(快照源为 canvas snapshot、scale=1.0;SVG/GIF 生成都在 core)。
    ///
    /// 两端一致; 截图源的像素格式/缩放是 adapter 差异。
    Export,
    /// 动作 `copy_svg`: 把裁切后的 SVG 以
    /// `<img src="data:image/svg+xml;base64,…" />` HTML 片段写入剪贴板文本。
    ///
    /// - macOS `run.m:1787` → `perform_hotkey(kVK_ANSI_S)`(`run.m:1699-1716`):
    ///   `glaspen2_get_cropped_svg` → base64 → NSPasteboard 文本;
    /// - Windows `hotkeys.rs:782` → `actions.rs:1124` `copy_svg_to_clipboard`:
    ///   同一 HTML 形状 → 剪贴板 UTF-16 文本。
    ///
    /// 两端一致(剪贴板 API 属 OS 适配层)。
    CopySvg,
    /// 动作 `toggle_glass`: 磨砂玻璃背景开关(并持久化)。
    ///
    /// - macOS `run.m:1788` → `perform_hotkey(kVK_ANSI_B)`(`run.m:1717-1719`)→
    ///   `gl_settings_set_glass_enabled(!g_glass_enabled)`(`menu.m:1424`):
    ///   落库 `glass_enabled` + `gl_glass_apply` 重估玻璃视图;
    /// - Windows `hotkeys.rs:783` → `actions.rs:1207` `toggle_frosted`: 翻转 +
    ///   落库(写面板键 `frostedGlass`, 经 `db::save_setting` 规范成
    ///   `glass_enabled`)+ `SetWindowCompositionAttribute` acrylic + HUD。
    ///
    /// 语义一致(最终持久化键相同); 毛玻璃渲染是平台 API 差异。
    ToggleGlass,
    /// 动作 `toggle_ethereal`: 固定画布 ↔ 飘渺画布涂鸦模式。
    ///
    /// - macOS `run.m:1789` → `perform_hotkey(kVK_ANSI_X)`(`run.m:1720-1723`)→
    ///   `toggle_canvas_mode()`(`menu.m:412-443`): 切换笔迹可见性、按飘渺规则
    ///   重估玻璃/压感浮层、落库 `ethereal`;
    /// - Windows `hotkeys.rs:784` → `CMD_TOGGLE_ETHEREAL`(`actions.rs:222-241`):
    ///   同语义(作废 peek 计时器、hide/show strokes、落库 `ethereal`、HUD)。
    ///
    /// 两端一致。
    ToggleEthereal,
    /// 动作 `settings`: 打开设置面板。
    ///
    /// - macOS `run.m:1790` → `perform_hotkey(kVK_ANSI_D)`(`run.m:1724-1728`)→
    ///   `show_settings_panel()`(`panel.m:14`): **进程内**嵌入 Flutter 视图,
    ///   已有窗口则提到最前;
    /// - Windows `hotkeys.rs:785` → `crate::show_settings_window()`
    ///   (`crates/glaspen-windows/src/lib.rs:38`): 拉起/置顶**独立面板进程**
    ///   (单实例; 退出时由 `kill_settings_processes` 收尾)。
    ///
    /// 语义一致; **面板宿主形式不同**(macOS 内嵌视图 / Windows 独立进程)是
    /// 两端既有架构, 本切片不改, 接线时按平台保留。
    OpenSettings,
    /// 动作 `quit`: 彻底退出应用(与菜单「退出」共用同一条收尾路径)。
    ///
    /// - macOS `run.m:1791-1800`: `dispatch_async` 下一 tick `gl_quit_app()`
    ///   (`menu.m:445`: 性能汇总 / 归还系统光标 / terminate →
    ///   `applicationShouldTerminate` 写完排队点 + checkpoint WAL);
    /// - Windows `hotkeys.rs:789-791`: `DestroyWindow` → WM_DESTROY
    ///   (`wndproc.rs:232`)收尾(写完排队点 + checkpoint WAL, 并结束设置面板
    ///   子进程)。
    ///
    /// 语义一致; 投递方式(主队列下一 tick vs 窗口消息)与面板子进程处理是
    /// 平台差异。
    Quit,
}

/// 动作 id → 平台无关语义(唯一映射, 与 [`crate::ui::ACTIONS`] 逐条对应)。
///
/// 精确匹配 action id(小写 snake_case, 不做大小写/空白归一): 两端分派入口
/// 本来就是拿 chord 表查出的 id 精确比较, 这里保持同一严格度, 免得把
/// `"Undo"`/`" undo "` 这类脏值悄悄放行。
///
/// 返回 `Err` 的场景(adapter 应记录并忽略, 等价于两端今天的"不识别"):
/// - 空串;
/// - 未在 [`crate::ui::ACTIONS`] 登记的动作 id(防壳/面板版本漂移、手误)。
///
/// 新增动作时必须同时改 `ui::ACTIONS` 与本函数 —— 本模块测试会枚举
/// `ui::ACTIONS`, 断言每个 action 都有映射且变体名稳定。
pub fn effect_for_action(action: &str) -> Result<ActionEffect, String> {
    let effect = match action {
        "undo" => ActionEffect::Undo,
        "page_prev" => ActionEffect::PagePrev,
        "page_next" => ActionEffect::PageNext,
        "color_swap" => ActionEffect::ColorSwap,
        "width_swap" => ActionEffect::WidthSwap,
        "toggle_draw" => ActionEffect::ToggleDraw,
        "toggle_free_canvas" => ActionEffect::ToggleFreeCanvas,
        "canvas_pan_left" => ActionEffect::CameraPan(CameraDir::Left),
        "canvas_pan_right" => ActionEffect::CameraPan(CameraDir::Right),
        "canvas_pan_up" => ActionEffect::CameraPan(CameraDir::Up),
        "canvas_pan_down" => ActionEffect::CameraPan(CameraDir::Down),
        "canvas_zoom_in" => ActionEffect::CameraZoom(ZoomDir::In),
        "canvas_zoom_out" => ActionEffect::CameraZoom(ZoomDir::Out),
        "clear" => ActionEffect::Clear,
        "export" => ActionEffect::Export,
        "copy_svg" => ActionEffect::CopySvg,
        "toggle_glass" => ActionEffect::ToggleGlass,
        "toggle_ethereal" => ActionEffect::ToggleEthereal,
        "settings" => ActionEffect::OpenSettings,
        "quit" => ActionEffect::Quit,
        other => {
            return Err(format!(
                "未知动作 id: {other:?}(应来自 ui::ACTIONS 的 {} 个动作)",
                crate::ui::ACTIONS.len()
            ));
        }
    };
    Ok(effect)
}

impl ActionEffect {
    /// 变体名(与具体 action 的语义一一对应, 含镜头方向后缀)。
    ///
    /// 用途: ①未来 adapter 覆盖闸门按名字扫描两端分派代码(与 `settings.rs`
    /// 的 D1a 同形); ②日志/排障时要一个稳定且可读的标识。
    ///
    /// 稳定性是契约: `variant_names_are_stable` 测试固定了 20 个 action 的
    /// 期望名字, 改名 = 有意的接口变更。
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::Undo => "Undo",
            Self::PagePrev => "PagePrev",
            Self::PageNext => "PageNext",
            Self::ColorSwap => "ColorSwap",
            Self::WidthSwap => "WidthSwap",
            Self::ToggleDraw => "ToggleDraw",
            Self::ToggleFreeCanvas => "ToggleFreeCanvas",
            Self::CameraPan(dir) => match dir {
                CameraDir::Left => "CameraPanLeft",
                CameraDir::Right => "CameraPanRight",
                CameraDir::Up => "CameraPanUp",
                CameraDir::Down => "CameraPanDown",
            },
            Self::CameraZoom(dir) => match dir {
                ZoomDir::In => "CameraZoomIn",
                ZoomDir::Out => "CameraZoomOut",
            },
            Self::Clear => "Clear",
            Self::Export => "Export",
            Self::CopySvg => "CopySvg",
            Self::ToggleGlass => "ToggleGlass",
            Self::ToggleEthereal => "ToggleEthereal",
            Self::OpenSettings => "OpenSettings",
            Self::Quit => "Quit",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::ACTIONS;

    /// (action id, 期望 variant_name) 显式清单 = 对外契约。
    ///
    /// 顺序与 `ui::ACTIONS` 一致; 改名字必须同时改这里(未来 adapter 闸门按
    /// 这些名字匹配两端分派代码, 名字漂移会让闸门静默失效)。
    const EXPECTED: &[(&str, &str)] = &[
        ("undo", "Undo"),
        ("page_prev", "PagePrev"),
        ("page_next", "PageNext"),
        ("color_swap", "ColorSwap"),
        ("width_swap", "WidthSwap"),
        ("toggle_draw", "ToggleDraw"),
        ("toggle_free_canvas", "ToggleFreeCanvas"),
        ("canvas_pan_left", "CameraPanLeft"),
        ("canvas_pan_right", "CameraPanRight"),
        ("canvas_pan_up", "CameraPanUp"),
        ("canvas_pan_down", "CameraPanDown"),
        ("canvas_zoom_in", "CameraZoomIn"),
        ("canvas_zoom_out", "CameraZoomOut"),
        ("clear", "Clear"),
        ("export", "Export"),
        ("copy_svg", "CopySvg"),
        ("toggle_glass", "ToggleGlass"),
        ("toggle_ethereal", "ToggleEthereal"),
        ("settings", "OpenSettings"),
        ("quit", "Quit"),
    ];

    fn effect(action: &str) -> ActionEffect {
        effect_for_action(action).unwrap_or_else(|err| panic!("动作 {action} 应映射成功: {err}"))
    }

    /// `ui::ACTIONS` 里每个动作都必须映射成功, 且变体名非空。
    /// core 加了动作而本模块没跟上时, 这个测试必须失败。
    #[test]
    fn every_action_id_maps_to_an_effect() {
        assert_eq!(ACTIONS.len(), 20, "动作数变了, 同步本模块与测试");
        let mut checked = 0;
        for def in ACTIONS {
            let effect = effect_for_action(def.id)
                .unwrap_or_else(|err| panic!("动作 {} 未映射: {err}", def.id));
            assert!(
                !effect.variant_name().is_empty(),
                "{} 的 variant_name 不能为空",
                def.id
            );
            checked += 1;
        }
        assert_eq!(checked, ACTIONS.len(), "动作覆盖不完整");
    }

    /// 空串/大小写变体/带空白/未知 id 一律拒绝(两端分派都是精确比较)。
    #[test]
    fn unknown_and_empty_actions_are_rejected() {
        for action in [
            "",
            " ",
            "undo ",
            " Undo",
            "Undo",
            "page",
            "no_such_action",
            "canvas_pan_sideways",
        ] {
            let err =
                effect_for_action(action).expect_err(&format!("{action:?} 不应被识别为已登记动作"));
            assert!(
                err.contains("未知动作"),
                "未知动作的错误信息应说明原因: {err}"
            );
        }
    }

    /// 20 个 action 的变体名逐条钉死(接口稳定性)。
    #[test]
    fn variant_names_are_stable() {
        assert_eq!(
            EXPECTED.len(),
            ACTIONS.len(),
            "期望清单与 ui::ACTIONS 数量不一致"
        );
        for (action, name) in EXPECTED {
            assert!(
                ACTIONS.iter().any(|def| def.id == *action),
                "期望清单里的 {action} 不在 ui::ACTIONS"
            );
            assert_eq!(
                effect(action).variant_name(),
                *name,
                "动作 {action} 的变体名漂移"
            );
        }
        // 反向: ui::ACTIONS 新增动作时, 必须在期望清单里补一行。
        for def in ACTIONS {
            assert!(
                EXPECTED.iter().any(|(action, _)| *action == def.id),
                "动作 {} 缺期望变体名",
                def.id
            );
        }
    }

    /// 变体名必须两两不同(未来 adapter 闸门按名字匹配, 重名会互相顶替)。
    #[test]
    fn variant_names_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for def in ACTIONS {
            let name = effect(def.id).variant_name();
            assert!(seen.insert(name), "变体名重复: {name}");
        }
        assert_eq!(
            seen.len(),
            ACTIONS.len(),
            "20 个动作应得到 20 个互不相同的变体名"
        );
    }

    /// 镜头方向/缩放种类作为语义载荷, 4 个平移 + 2 个缩放不能混。
    #[test]
    fn camera_payloads_carry_direction() {
        use CameraDir::{Down, Left, Right, Up};
        use ZoomDir::{In, Out};

        assert_eq!(effect("canvas_pan_left"), ActionEffect::CameraPan(Left));
        assert_eq!(effect("canvas_pan_right"), ActionEffect::CameraPan(Right));
        assert_eq!(effect("canvas_pan_up"), ActionEffect::CameraPan(Up));
        assert_eq!(effect("canvas_pan_down"), ActionEffect::CameraPan(Down));
        assert_eq!(effect("canvas_zoom_in"), ActionEffect::CameraZoom(In));
        assert_eq!(effect("canvas_zoom_out"), ActionEffect::CameraZoom(Out));

        // 方向名助手与 variant_name 后缀一致(adapter 日志/测试用)。
        assert_eq!(Left.name(), "Left");
        assert_eq!(Right.name(), "Right");
        assert_eq!(Up.name(), "Up");
        assert_eq!(Down.name(), "Down");
        assert_eq!(In.name(), "In");
        assert_eq!(Out.name(), "Out");
    }
}
