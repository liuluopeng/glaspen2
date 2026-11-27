//! 平台菜单/托盘的共享菜单模型(单源)。
//!
//! macOS 的 NSStatusItem 菜单与 Windows 托盘菜单都用这张表决定:
//! 分组顺序、颜色/粗细子菜单、动作、勾选状态与中英文文案。
//! 平台 adapter 只负责把 MenuEntry 渲染成 NSMenu / Win32 POPUP，
//! 并把 MenuAction 映射到本平台已有的命令。

use crate::i18n::{self, Lang};
use crate::presets::{COLOR_PRESETS, WIDTH_PRESETS};

/// 菜单动作: 平台无关语义。平台 adapter 负责映射到本平台命令。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuAction {
    SelectColor(usize),
    SelectWidth(usize),
    ToggleDraw,
    Undo,
    Clear,
    ToggleGrid,
    ToggleGlass,
    ToggleOutline,
    ToggleEthereal,
    ToggleInfinite,
    ToggleMinimap,
    TogglePressureMonitor,
    ToggleChatIntegration,
    SaveWithBackground,
    SaveDrawing,
    SaveXoj,
    ExportSvgGif,
    OpenSettings,
    Quit,
}

fn tr(lang: Lang, zh: &'static str, en: &'static str) -> &'static str {
    match lang {
        Lang::Zh => zh,
        Lang::En => en,
    }
}

/// 勾选状态来源; adapter 每帧/打开菜单时填充。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MenuState {
    pub draw_enabled: bool,
    pub grid: bool,
    pub glass: bool,
    pub outline: bool,
    pub ethereal: bool,
    pub infinite: bool,
    pub minimap: bool,
    pub pressure_monitor: bool,
    pub chat_integration: bool,
    pub color_index: usize,
    pub width_index: usize,
}

/// 一个可渲染菜单项。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuEntry {
    pub id: String,
    pub label: String,
    pub action: Option<MenuAction>,
    pub checked: bool,
    pub children: Vec<MenuEntry>,
}

impl MenuEntry {
    fn item(id: &str, label: impl Into<String>, action: MenuAction, checked: bool) -> Self {
        Self {
            id: id.to_string(),
            label: label.into(),
            action: Some(action),
            checked,
            children: Vec::new(),
        }
    }

    fn submenu(id: &str, label: impl Into<String>, children: Vec<MenuEntry>) -> Self {
        Self {
            id: id.to_string(),
            label: label.into(),
            action: None,
            checked: false,
            children,
        }
    }

    fn separator() -> Self {
        Self {
            id: "separator".to_string(),
            label: String::new(),
            action: None,
            checked: false,
            children: Vec::new(),
        }
    }

    pub fn is_separator(&self) -> bool {
        self.action.is_none() && self.children.is_empty()
    }
}

/// 构建菜单(纯函数; 两端共用同一顺序/文案/勾选语义)。
pub fn build(lang: Lang, state: &MenuState) -> Vec<MenuEntry> {
    debug_assert_eq!(COLOR_PRESETS.len(), i18n::COLOR_NAMES.len());
    debug_assert_eq!(WIDTH_PRESETS.len(), i18n::WIDTH_NAMES.len());

    let mut colors = Vec::with_capacity(COLOR_PRESETS.len());
    for i in 0..COLOR_PRESETS.len() {
        colors.push(MenuEntry::item(
            &format!("color.{i}"),
            i18n::color_name(i, lang),
            MenuAction::SelectColor(i),
            i == state.color_index,
        ));
    }

    let mut widths = Vec::with_capacity(WIDTH_PRESETS.len());
    for i in 0..WIDTH_PRESETS.len() {
        widths.push(MenuEntry::item(
            &format!("width.{i}"),
            i18n::width_name(i, lang),
            MenuAction::SelectWidth(i),
            i == state.width_index,
        ));
    }

    let toggle_draw = if state.draw_enabled {
        match lang {
            Lang::Zh => "关闭涂鸦",
            Lang::En => "Disable Drawing",
        }
    } else {
        match lang {
            Lang::Zh => "开启涂鸦",
            Lang::En => "Enable Drawing",
        }
    };

    let mode = if state.ethereal {
        match lang {
            Lang::Zh => "固定画布模式",
            Lang::En => "Fixed Canvas Mode",
        }
    } else {
        match lang {
            Lang::Zh => "飘渺模式",
            Lang::En => "Ethereal Mode",
        }
    };

    let infinite = if state.infinite {
        match lang {
            Lang::Zh => "切回活页本",
            Lang::En => "Back to Pages",
        }
    } else {
        match lang {
            Lang::Zh => "自由涂鸦(无限画布)",
            Lang::En => "Free Canvas (Infinite)",
        }
    };

    vec![
        MenuEntry::submenu("colors", tr(lang, "颜色", "Color"), colors),
        MenuEntry::separator(),
        MenuEntry::submenu("widths", tr(lang, "粗细", "Width"), widths),
        MenuEntry::separator(),
        MenuEntry::item(
            "saveWithBackground",
            tr(lang, "保存(含背景)", "Save (with bg)"),
            MenuAction::SaveWithBackground,
            false,
        ),
        MenuEntry::item(
            "saveDrawing",
            tr(lang, "保存(涂鸦)", "Save (drawing)"),
            MenuAction::SaveDrawing,
            false,
        ),
        MenuEntry::item(
            "newCanvas",
            tr(lang, "新建画布", "New canvas"),
            MenuAction::Clear,
            false,
        ),
        MenuEntry::submenu(
            "export",
            tr(lang, "导出", "Export"),
            vec![
                MenuEntry::item(
                    "saveXoj",
                    tr(lang, "保存笔记 (Xournal)", "Save Notes (Xournal)"),
                    MenuAction::SaveXoj,
                    false,
                ),
                MenuEntry::item(
                    "exportSvgGif",
                    tr(lang, "导出 SVG + 动画 GIF", "Export SVG + animated GIF"),
                    MenuAction::ExportSvgGif,
                    false,
                ),
            ],
        ),
        MenuEntry::separator(),
        MenuEntry::item("undo", tr(lang, "撤销", "Undo"), MenuAction::Undo, false),
        MenuEntry::item(
            "toggleDraw",
            toggle_draw,
            MenuAction::ToggleDraw,
            state.draw_enabled,
        ),
        MenuEntry::item(
            "settings",
            tr(lang, "设置...", "Settings..."),
            MenuAction::OpenSettings,
            false,
        ),
        MenuEntry::submenu(
            "more",
            tr(lang, "更多", "More"),
            vec![
                MenuEntry::item(
                    "toggleGrid",
                    tr(lang, "网格", "Grid"),
                    MenuAction::ToggleGrid,
                    state.grid,
                ),
                MenuEntry::item(
                    "toggleGlass",
                    tr(lang, "磨砂玻璃", "Frosted Glass"),
                    MenuAction::ToggleGlass,
                    state.glass,
                ),
                MenuEntry::item(
                    "toggleOutline",
                    tr(lang, "笔迹描边", "Stroke outline"),
                    MenuAction::ToggleOutline,
                    state.outline,
                ),
                MenuEntry::item(
                    "toggleEthereal",
                    mode,
                    MenuAction::ToggleEthereal,
                    state.ethereal,
                ),
                MenuEntry::item(
                    "toggleInfinite",
                    infinite,
                    MenuAction::ToggleInfinite,
                    state.infinite,
                ),
                MenuEntry::item(
                    "toggleMinimap",
                    tr(lang, "显示最近 10 页", "Show Recent 10 Pages"),
                    MenuAction::ToggleMinimap,
                    state.minimap,
                ),
                MenuEntry::item(
                    "togglePressureMonitor",
                    tr(lang, "压感监视", "Pressure Monitor"),
                    MenuAction::TogglePressureMonitor,
                    state.pressure_monitor,
                ),
                MenuEntry::item(
                    "toggleChatIntegration",
                    tr(lang, "手写消息与共享", "Handwriting Messages & Sharing"),
                    MenuAction::ToggleChatIntegration,
                    state.chat_integration,
                ),
            ],
        ),
        MenuEntry::separator(),
        MenuEntry::item("quit", tr(lang, "退出", "Quit"), MenuAction::Quit, false),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> MenuState {
        MenuState {
            draw_enabled: true,
            grid: true,
            glass: true,
            outline: true,
            ethereal: true,
            infinite: true,
            minimap: true,
            pressure_monitor: true,
            chat_integration: true,
            color_index: 3,
            width_index: 5,
        }
    }

    #[test]
    fn menu_has_shared_groups_and_checks() {
        let zh = build(Lang::Zh, &state());
        assert_eq!(zh[0].id, "colors");
        assert_eq!(zh[0].children.len(), COLOR_PRESETS.len());
        assert!(zh[0].children[3].checked);
        assert_eq!(zh[2].id, "widths");
        assert_eq!(zh[2].children.len(), WIDTH_PRESETS.len());
        assert!(zh[2].children[5].checked);

        let more = zh.iter().find(|e| e.id == "more").unwrap();
        assert!(more.children.iter().all(|e| e.checked));
        assert_eq!(more.children.len(), 8);
        assert!(zh.iter().any(MenuEntry::is_separator));

        let en = build(Lang::En, &state());
        assert!(en.iter().all(|e| !e.label.is_empty() || e.is_separator()));
        assert!(en[0].children.iter().all(|e| !e.label.is_empty()));
    }

    #[test]
    fn toggle_labels_follow_state() {
        let mut s = state();
        s.draw_enabled = false;
        s.ethereal = false;
        s.infinite = false;
        let menu = build(Lang::Zh, &s);
        assert_eq!(
            menu.iter().find(|e| e.id == "toggleDraw").unwrap().label,
            "开启涂鸦"
        );
        let more = menu.iter().find(|e| e.id == "more").unwrap();
        assert_eq!(more.children[3].label, "飘渺模式");
        assert_eq!(more.children[4].label, "自由涂鸦(无限画布)");
    }

    #[test]
    fn action_ids_are_unique() {
        fn walk(entries: &[MenuEntry], ids: &mut Vec<String>) {
            for e in entries {
                if !e.is_separator() {
                    assert!(!ids.contains(&e.id), "重复菜单 id: {}", e.id);
                    ids.push(e.id.clone());
                }
                walk(&e.children, ids);
            }
        }
        let mut ids = Vec::new();
        walk(&build(Lang::Zh, &state()), &mut ids);
        assert!(ids.len() > 20);
    }
}
