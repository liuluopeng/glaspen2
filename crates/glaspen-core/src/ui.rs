//! 面板的平台无关 UI schema。
//!
//! 设置面板的 Flutter 代码只有一份, 平台差异(默认键位/保留组合/强制绑定/
//! 修饰键名称/键盘示意图布局/字号/功能可用性)全部由这里的数据描述:
//! macOS 经 FRB(`ui_config_json`), Windows 经命名管道(`uiConfig`)取同一份 JSON。
//! Flutter 侧不再出现 `Platform.is*` 分支。
//!
//! 规则: 新增动作 / 改默认键位 / 改保留组合 → **只改本文件**;
//! Windows 壳 (`overlay/hotkeys.rs`) 与面板都从这里取, 不再各留一份表。

use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Macos,
    Windows,
}

impl Platform {
    fn is_windows(self) -> bool {
        matches!(self, Platform::Windows)
    }
}

/// 一个可配置动作。默认绑定必须与历史发布版本逐字一致(老用户存档按字符串
/// 对齐), 改默认值要同步升 `shortcuts_defaults_v`(Windows 壳里有迁移逻辑)。
pub struct ActionDef {
    pub id: &'static str,
    /// macOS 默认 chord; 空串 = 默认不绑定。
    pub mac_default: &'static str,
    /// Windows 默认 chord; 空串 = 默认不绑定(当前约定: 全部动作都有默认键)。
    pub win_default: &'static str,
    /// Windows 没有菜单栏/托盘, 这两个动作必须始终有键, 面板禁止解绑。
    pub win_force_bound: bool,
}

pub const ACTIONS: &[ActionDef] = &[
    ActionDef {
        id: "undo",
        mac_default: "cmd+ctrl+Z",
        win_default: "ctrl+alt+Z",
        win_force_bound: false,
    },
    ActionDef {
        id: "page_prev",
        mac_default: "cmd+ctrl+`",
        win_default: "ctrl+alt+`",
        win_force_bound: false,
    },
    ActionDef {
        id: "page_next",
        mac_default: "cmd+ctrl+1",
        win_default: "ctrl+alt+1",
        win_force_bound: false,
    },
    ActionDef {
        id: "color_swap",
        mac_default: "",
        win_default: "ctrl+alt+shift+C",
        win_force_bound: false,
    },
    ActionDef {
        id: "width_swap",
        mac_default: "",
        win_default: "ctrl+alt+T",
        win_force_bound: false,
    },
    ActionDef {
        id: "toggle_draw",
        mac_default: "cmd+ctrl+V",
        win_default: "ctrl+alt+V",
        win_force_bound: false,
    },
    ActionDef {
        id: "toggle_free_canvas",
        mac_default: "",
        win_default: "ctrl+alt+I",
        win_force_bound: false,
    },
    ActionDef {
        id: "canvas_pan_left",
        mac_default: "ctrl+alt+A",
        win_default: "cmd+alt+A",
        win_force_bound: false,
    },
    ActionDef {
        id: "canvas_pan_right",
        mac_default: "ctrl+alt+D",
        win_default: "cmd+alt+D",
        win_force_bound: false,
    },
    ActionDef {
        id: "canvas_pan_up",
        mac_default: "ctrl+alt+W",
        win_default: "cmd+alt+W",
        win_force_bound: false,
    },
    ActionDef {
        id: "canvas_pan_down",
        mac_default: "ctrl+alt+S",
        win_default: "cmd+alt+S",
        win_force_bound: false,
    },
    ActionDef {
        id: "canvas_zoom_in",
        mac_default: "ctrl+alt+X",
        win_default: "cmd+alt+X",
        win_force_bound: false,
    },
    ActionDef {
        id: "canvas_zoom_out",
        mac_default: "ctrl+alt+Z",
        win_default: "cmd+alt+Z",
        win_force_bound: false,
    },
    ActionDef {
        id: "clear",
        mac_default: "",
        win_default: "ctrl+alt+C",
        win_force_bound: false,
    },
    ActionDef {
        id: "export",
        mac_default: "",
        win_default: "ctrl+alt+G",
        win_force_bound: false,
    },
    ActionDef {
        id: "copy_svg",
        mac_default: "",
        win_default: "ctrl+alt+S",
        win_force_bound: false,
    },
    ActionDef {
        id: "toggle_glass",
        mac_default: "",
        win_default: "ctrl+alt+B",
        win_force_bound: false,
    },
    ActionDef {
        id: "toggle_ethereal",
        mac_default: "",
        win_default: "ctrl+alt+X",
        win_force_bound: false,
    },
    ActionDef {
        id: "settings",
        mac_default: "cmd+ctrl+D",
        win_default: "ctrl+alt+D",
        win_force_bound: true,
    },
    ActionDef {
        id: "quit",
        mac_default: "cmd+ctrl+shift+Q",
        win_default: "ctrl+alt+shift+Q",
        win_force_bound: true,
    },
];

/// 系统/常用 App 占用的组合: 面板不允许选择, 壳也不允许注册。
const RESERVED_MAC: &[&str] = &[
    "cmd+ctrl+Q", // macOS 锁屏
    "cmd+ctrl+R", // 按住录 GIF
    "cmd+ctrl+2", // 按住手写草稿
    "cmd+ctrl+3", // 按住手写消息
    "cmd+ctrl+W", // 微信 显示/隐藏主面板
    "cmd+ctrl+A", // 微信 截图
    "cmd+ctrl+F", // macOS 全屏
];
const RESERVED_WINDOWS: &[&str] = &[
    "ctrl+alt+R", // 按住录 GIF
    "ctrl+alt+2", // 按住手写草稿
    "ctrl+alt+3", // 按住手写消息
];

/// 按住类的固定热键(不参与改键表, 但要给面板显示提示文案)。
pub struct HoldHotkeys {
    pub gif: &'static str,
    pub draft: &'static str,
    pub send: &'static str,
}

pub fn hold_hotkeys(platform: Platform) -> HoldHotkeys {
    match platform {
        Platform::Macos => HoldHotkeys {
            gif: "cmd+ctrl+R",
            draft: "cmd+ctrl+2",
            send: "cmd+ctrl+3",
        },
        Platform::Windows => HoldHotkeys {
            gif: "ctrl+alt+R",
            draft: "ctrl+alt+2",
            send: "ctrl+alt+3",
        },
    }
}

/// 动作 id 列表(JSON 数组, 面板/壳按此顺序展示; macOS 壳解析用)。
pub fn action_ids_json() -> String {
    serde_json::to_string(&ACTIONS.iter().map(|a| a.id).collect::<Vec<_>>())
        .unwrap_or_else(|_| "[]".to_string())
}

pub fn actions() -> &'static [ActionDef] {
    ACTIONS
}

pub fn defaults(platform: Platform) -> Vec<(&'static str, &'static str)> {
    ACTIONS
        .iter()
        .map(|a| {
            (
                a.id,
                if platform.is_windows() {
                    a.win_default
                } else {
                    a.mac_default
                },
            )
        })
        .collect()
}

pub fn default_chord(platform: Platform, action: &str) -> Option<&'static str> {
    ACTIONS.iter().find(|a| a.id == action).map(|a| {
        if platform.is_windows() {
            a.win_default
        } else {
            a.mac_default
        }
    })
}

pub fn reserved(platform: Platform) -> &'static [&'static str] {
    if platform.is_windows() {
        RESERVED_WINDOWS
    } else {
        RESERVED_MAC
    }
}

pub fn force_bound(platform: Platform) -> Vec<&'static str> {
    if !platform.is_windows() {
        return Vec::new();
    }
    ACTIONS
        .iter()
        .filter(|a| a.win_force_bound)
        .map(|a| a.id)
        .collect()
}

/// chord 令牌 → 面板显示名。macOS 用符号, Windows 用文本(键名一致)。
fn chord_names(platform: Platform) -> Value {
    if platform.is_windows() {
        json!({
            "cmd": "Win", "ctrl": "Ctrl", "alt": "Alt", "shift": "Shift",
            "Up": "↑", "Down": "↓", "Left": "←", "Right": "→",
            "PageUp": "PgUp", "PageDown": "PgDn", "Return": "Enter",
            "Escape": "Esc", "Backspace": "Bksp", "Delete": "Del",
            "Tab": "Tab", "Space": "Space",
        })
    } else {
        json!({
            "cmd": "⌘", "ctrl": "⌃", "alt": "⌥", "shift": "⇧",
            "Up": "↑", "Down": "↓", "Left": "←", "Right": "→",
            "PageUp": "PgUp", "PageDown": "PgDn", "Return": "↩",
            "Escape": "Esc", "Backspace": "⌫", "Delete": "⌦",
            "Tab": "⇥", "Space": "Space",
        })
    }
}

/// 键盘示意图底部修饰键行(含中央 Space, 分 left/space/right 三段):
/// `name` 是键帽显示文字(Win/Opt/Ctrl…), `label` 是 chord 令牌(cmd/alt/ctrl…),
/// 与面板 `_KeyCap(label: 显示, name: 令牌)` 的构造一致。
/// 不再由 Flutter 按平台拼这一行。
fn modifier_row(platform: Platform) -> Value {
    let (left, right) = if platform.is_windows() {
        (
            json!([
                {"name": "ctrl", "label": "ctrl", "width": 1.25, "modifier": true},
                {"name": "win", "label": "cmd", "width": 1.25, "modifier": true},
                {"name": "alt", "label": "alt", "width": 1.25, "modifier": true},
            ]),
            // 右侧与左侧镜像(旧面板键位图: alt/win/ctrl)
            json!([
                {"name": "alt", "label": "alt", "width": 1.25, "modifier": true},
                {"name": "win", "label": "cmd", "width": 1.25, "modifier": true},
                {"name": "ctrl", "label": "ctrl", "width": 1.25, "modifier": true},
            ]),
        )
    } else {
        (
            json!([
                {"name": "ctrl", "label": "ctrl", "width": 1.25, "modifier": true},
                {"name": "opt", "label": "alt", "width": 1.25, "modifier": true},
                {"name": "cmd", "label": "cmd", "width": 1.25, "modifier": true},
            ]),
            // 右侧与左侧镜像(旧面板键位图: cmd/opt/ctrl)
            json!([
                {"name": "cmd", "label": "cmd", "width": 1.25, "modifier": true},
                {"name": "opt", "label": "alt", "width": 1.25, "modifier": true},
                {"name": "ctrl", "label": "ctrl", "width": 1.25, "modifier": true},
            ]),
        )
    };
    let space = json!([{ "name": "space", "label": "Space", "width": 5.0, "modifier": false }]);
    json!({
        "left": left,
        "space": space,
        "right": right,
    })
}

/// 设置契约 + 表单控件元数据(uiConfig `settings`)。
///
/// D11a 起在原有登记表字段(键/落库键/类型/平台可用性)之外**追加** `form`:
/// - 表单可见项 = `settings_form::meta_for(key).form_json()`(分区/顺序/l10n 键/
///   控件/单位/范围/档位/features 门控/advanced);
/// - 非表单项(implicit/readOnly/hidden)= `{"control": "<分类>"}`, 面板可据此
///   跳过渲染(见 settings_form 模块文档的盘点)。
///
/// 纯增量: 旧字段一个不改、不升 protocolVersion(Dart 旧代码忽略未知字段),
/// 面板仍是旧的手写表单; D11b 才让 Dart 按这份 metadata 渲染。
fn settings_schema_with_form() -> Value {
    let mut schema = crate::settings::schema_value();
    for entry in schema
        .as_array_mut()
        .expect("settings schema 必须是数组")
        .iter_mut()
    {
        let key = entry
            .get("key")
            .and_then(Value::as_str)
            .expect("settings schema 条目必须有 key")
            .to_string();
        if let Some(meta) = crate::settings_form::meta_for(&key) {
            entry["form"] = meta.form_json();
        } else if let Some(non_form) = crate::settings_form::non_form_meta(&key) {
            entry["form"] = json!({ "control": non_form.control.as_str() });
        }
    }
    schema
}

/// 面板启动时拉取的一次性平台配置(JSON)。字段名即 Dart 侧 `_UiConfig` 的契约。
pub fn ui_config_json(platform: Platform) -> String {
    let win = platform.is_windows();
    let hold = hold_hotkeys(platform);
    let defaults: serde_json::Map<String, Value> = defaults(platform)
        .into_iter()
        .map(|(k, v)| (k.to_string(), Value::String(v.to_string())))
        .collect();
    let require_modifier = if win {
        json!({"zh": "需要同时按 Ctrl 或 Alt", "en": "Must include Ctrl or Alt"})
    } else {
        json!({"zh": "需要同时按 Ctrl 或 Option", "en": "Must include Ctrl or Option"})
    };
    let reserved_msg = if win {
        json!({"zh": "已被系统或按住类功能保留", "en": "Reserved by the system or hold features"})
    } else {
        json!({"zh": "已被系统、常用 App 或按住类功能保留", "en": "Reserved by macOS, common apps, or hold features"})
    };
    json!({
        "platform": if win { "windows" } else { "macos" },
        "isWindows": win,
        // macOS 面板按 1x 渲染整屏偏小, 现有行为是整体放大 1.4; Windows 交给系统缩放。
        "fontScale": if win { 1.0 } else { 1.4 },
        "chordSeparator": if win { "+" } else { "" },
        "chordNames": chord_names(platform),
        // 文案里指代 ⌥/Alt 的那个词。
        "modifierWord": if win { "Alt" } else { "Option" },
        "messages": {
            "requireModifier": require_modifier,
            "reserved": reserved_msg,
        },
        "reserved": reserved(platform),
        "forceBound": force_bound(platform),
        "defaults": defaults,
        "actionOrder": ACTIONS.iter().map(|a| a.id).collect::<Vec<_>>(),
        "holdHotkeys": {"gif": hold.gif, "draft": hold.draft, "send": hold.send},
        // 面板 ↔ 后端协议版本(独立 exe 面板防新旧混跑; 见 core/protocol.rs)。
        "protocolVersion": crate::protocol::PROTOCOL_VERSION,
        "modifierRow": modifier_row(platform),
        // 设置项契约(键/类型/落库键/平台可用性 + D11a 表单元数据),
        // 面板启动时据此自检后端是否漏项。
        "settings": settings_schema_with_form(),
        // 表单分区表(显示顺序/l10n 键/advanced/整节条件), D11a 起随
        // settings[].form 一起下发; 纯增量字段, 旧面板忽略。D11b 让 Dart 按它
        // 生成设置页(标题 l10n 键已按平台选好, Flutter 不做平台分支)。
        "settingsSections": crate::settings_form::sections_json(platform),
        // 分区内**非设置项**的自定义区块(命令按钮/登录/关于/快捷键编辑器):
        // core 只定位(分区/顺序/条件), 控件由 Dart 按稳定 id 手写。
        "settingsBlocks": crate::settings_form::blocks_json(),
        // 面板按钮的可用性由后端声明, Flutter 不再按平台隐藏。
        // Windows 侧尚未接入的导出/OCR 一律显式置 false(UI 隐藏, 杜绝静默空壳)。
        "features": {
            "invertInk": !win,
            "pageFlipEffect": !win,
            "pageExport": !win,
            "ocrSearch": !win,
        },
    })
    .to_string()
}

/// Windows 命名管道的一次 `uiConfig` 响应行(以 \n 结尾 —— 面板按行分帧)。
///
/// 放在 core 里(而不是 Windows-only 模块里)是为了能在 macOS 上用单测锁住
/// 协议形状: type/reqId/data 字段、末尾换行、data 是 JSON 对象而非转义字符串。
pub fn pipe_ui_config_response_json(req_id: i64) -> String {
    format!(
        "{{\"type\":\"uiConfig_response\",\"reqId\":{req_id},\"data\":{}}}\n",
        ui_config_json(Platform::Windows)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(platform: Platform) -> Value {
        serde_json::from_str(&ui_config_json(platform)).expect("ui config 必须是合法 JSON")
    }

    #[test]
    fn action_table_is_consistent() {
        let mut ids = std::collections::HashSet::new();
        for a in ACTIONS {
            assert!(!a.id.is_empty());
            assert!(ids.insert(a.id), "动作 id 重复: {}", a.id);
            assert!(
                !a.win_default.is_empty(),
                "Windows 约定所有动作都有默认键: {}",
                a.id
            );
            for chord in [a.mac_default, a.win_default] {
                if chord.is_empty() {
                    continue;
                }
                let parts: Vec<&str> = chord.split('+').collect();
                assert!(parts.len() >= 2, "chord 至少 2 段: {chord}");
                let mods = &parts[..parts.len() - 1];
                assert!(
                    mods.iter()
                        .any(|m| *m == "ctrl" || *m == "alt" || *m == "cmd"),
                    "chord 必须有修饰键: {chord}"
                );
            }
        }
    }

    #[test]
    fn defaults_cover_every_action() {
        for platform in [Platform::Macos, Platform::Windows] {
            let map = defaults(platform);
            assert_eq!(map.len(), ACTIONS.len());
            for a in ACTIONS {
                assert!(
                    map.iter().any(|(id, _)| *id == a.id),
                    "缺少默认值: {} ({platform:?})",
                    a.id
                );
            }
        }
    }

    #[test]
    fn force_bound_only_windows_settings_and_quit() {
        assert!(force_bound(Platform::Macos).is_empty());
        let win = force_bound(Platform::Windows);
        assert_eq!(win, vec!["settings", "quit"]);
    }

    #[test]
    fn json_shape_is_platform_symmetric() {
        let mac = parsed(Platform::Macos);
        let win = parsed(Platform::Windows);
        for (label, v) in [("macos", &mac), ("windows", &win)] {
            for key in [
                "platform",
                "isWindows",
                "fontScale",
                "chordSeparator",
                "chordNames",
                "modifierWord",
                "messages",
                "reserved",
                "forceBound",
                "defaults",
                "actionOrder",
                "holdHotkeys",
                "modifierRow",
                "features",
                "settings",
                "settingsSections",
                "settingsBlocks",
                "protocolVersion",
            ] {
                assert!(v.get(key).is_some(), "{label} 缺字段 {key}");
            }
            assert_eq!(v["actionOrder"].as_array().unwrap().len(), ACTIONS.len());
            assert_eq!(v["defaults"].as_object().unwrap().len(), ACTIONS.len());
            assert!(!v["reserved"].as_array().unwrap().is_empty());
            let row = &v["modifierRow"];
            assert_eq!(row["left"].as_array().unwrap().len(), 3);
            assert_eq!(row["right"].as_array().unwrap().len(), 3);
            assert_eq!(row["space"].as_array().unwrap().len(), 1);
        }
        assert_eq!(mac["platform"], "macos");
        assert_eq!(win["platform"], "windows");
        assert_eq!(mac["forceBound"].as_array().unwrap().len(), 0);
        assert_eq!(win["forceBound"].as_array().unwrap().len(), 2);
        assert_eq!(mac["features"]["invertInk"], true);
        assert_eq!(win["features"]["invertInk"], false);
        // 面板/壳必须看到同一套动作 id。
        assert_eq!(mac["actionOrder"], win["actionOrder"]);
        // 设置契约: 两个平台拿到同一份登记表, 各自平台标记可用项。
        assert_eq!(
            mac["settings"].as_array().unwrap().len(),
            crate::settings::SETTINGS.len()
        );
        assert_eq!(
            win["settings"].as_array().unwrap().len(),
            crate::settings::SETTINGS.len()
        );
        for (label, v, flag) in [("macos", &mac, "mac"), ("windows", &win, "win")] {
            let n = v["settings"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|s| s[flag] == true)
                .count();
            assert!(n > 0, "{label} 没有任何可用设置?");
        }
        assert_eq!(
            mac["settings"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|s| s["mac"] == true)
                .count(),
            crate::settings::keys_for(Platform::Macos).len()
        );
        assert_eq!(
            win["settings"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|s| s["win"] == true)
                .count(),
            crate::settings::keys_for(Platform::Windows).len()
        );

        // D11a: 表单 metadata 随设置契约下发(纯增量, 旧字段仍在上面逐项断言)。
        assert_eq!(
            mac["settingsSections"].as_array().unwrap().len(),
            crate::settings_form::SECTIONS.len()
        );
        for entry in mac["settings"].as_array().unwrap() {
            assert!(entry.get("form").is_some(), "设置项缺 form 分类: {entry}");
        }
        let form_of = |v: &Value, key: &str| -> Value {
            v["settings"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["key"] == key)
                .unwrap_or_else(|| panic!("设置契约缺 {key}"))["form"]
                .clone()
        };
        assert_eq!(form_of(&mac, "color")["control"], "colorGrid");
        assert_eq!(form_of(&mac, "invertFps")["unit"], "hz");
        assert_eq!(
            form_of(&mac, "flipEffect")["visibleWhenFeature"],
            "pageFlipEffect"
        );
        assert_eq!(form_of(&mac, "shortcuts")["control"], "shortcuts");
        // D11b 第一步: 条件显示/可用与布局提示随 form 下发。
        assert_eq!(
            form_of(&mac, "bubbleReplaySpeed")["visibleWhenSetting"],
            json!({"key": "showBubbleTab", "equals": true})
        );
        assert_eq!(
            form_of(&mac, "bubbleReplaySpeed")["layout"],
            "inlineSegmented"
        );
        assert_eq!(form_of(&mac, "bubbleReplaySpeed")["indented"], true);
        assert_eq!(
            form_of(&mac, "panelTransparent")["enabledWhenSetting"],
            json!({"key": "notebookStyle", "equals": true})
        );
        assert_eq!(form_of(&mac, "gifFps")["layout"], "chips");
        assert_eq!(form_of(&mac, "language")["showLabel"], false);
        assert_eq!(
            form_of(&mac, "invertFps")["descriptionId"],
            "invertTrackingFpsCap"
        );
        assert_eq!(form_of(&mac, "chatApiBase")["hintId"], "eGHttps192");
        // 整节条件: sharedCanvas 只在 chatIntegration 打开时出现。
        let sections = mac["settingsSections"].as_array().unwrap();
        let shared = sections
            .iter()
            .find(|s| s["id"] == "sharedCanvas")
            .expect("sharedCanvas 分区");
        assert_eq!(
            shared["visibleWhenSetting"],
            json!({"key": "chatIntegration", "equals": true})
        );
        // 快捷键分区标题按平台选 ARB 键(core 决定, Flutter 不做平台分支)。
        let shortcut_label = |v: &Value| -> String {
            v["settingsSections"]
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["id"] == "shortcuts")
                .expect("shortcuts 分区")["labelId"]
                .as_str()
                .unwrap()
                .to_string()
        };
        assert_eq!(shortcut_label(&mac), "hotkeys");
        assert_eq!(shortcut_label(&win), "hotkeysCtrlAlt");
        // 自定义区块: 分区/顺序/条件, 由 Dart 按 id 渲染。
        let blocks = mac["settingsBlocks"]
            .as_array()
            .expect("settingsBlocks 数组");
        assert_eq!(
            blocks.len(),
            crate::settings_form::CUSTOM_BLOCKS.len(),
            "自定义区块数"
        );
        let login = blocks
            .iter()
            .find(|b| b["id"] == "loginActions")
            .expect("登录自定义区块");
        assert_eq!(login["section"], "doodleIdentity");
        assert_eq!(login["order"], 3);
        assert_eq!(
            login["visibleWhenSetting"],
            json!({"key": "chatIntegration", "equals": true})
        );
        assert!(blocks.iter().any(|b| b["id"] == "about"));
        assert!(
            blocks.iter().all(|b| crate::settings_form::SECTIONS
                .iter()
                .any(|s| Some(s.id) == b["section"].as_str())),
            "自定义区块的分区必须存在"
        );
        // 非表单项只有分类(与 settings_form 的盘点一致)。
        assert_eq!(form_of(&mac, "infiniteCanvas")["control"], "implicit");
        assert_eq!(form_of(&mac, "chatHasPassword")["control"], "readOnly");
        assert_eq!(form_of(&mac, "frostedGlass")["control"], "hidden");
        assert_eq!(form_of(&mac, "ethereal")["control"], "hidden");
        assert_eq!(mac["settingsSections"].as_array().unwrap()[0]["id"], "pen");
        assert_eq!(
            mac["settingsSections"].as_array().unwrap()[0]["advanced"],
            false
        );

        assert_eq!(mac["protocolVersion"], crate::protocol::PROTOCOL_VERSION);
        assert_eq!(win["protocolVersion"], crate::protocol::PROTOCOL_VERSION);
    }

    /// Flutter widget 测试用的 uiConfig 固定样例必须与本模块输出逐字段一致。
    ///
    /// 面板的 schema 渲染器不在 `flutter test` 里连后端(FRB 符号不可用), 所以
    /// Dart 测试读 `test/fixtures/ui_config_{macos,windows}.json` 当"真实
    /// schema"; 这里是它的漂移闸门 —— core 改了 schema 而忘记重新生成 fixture
    /// 时, 这个测试直接红(生成方式见 fixtures 目录旁 README)。
    #[test]
    fn flutter_schema_fixtures_match_core_output() {
        for (name, platform, fixture) in [
            (
                "macos",
                Platform::Macos,
                include_str!("../../../flutter_settings/test/fixtures/ui_config_macos.json"),
            ),
            (
                "windows",
                Platform::Windows,
                include_str!("../../../flutter_settings/test/fixtures/ui_config_windows.json"),
            ),
        ] {
            let want: Value =
                serde_json::from_str(&ui_config_json(platform)).expect("core uiConfig JSON");
            let got: Value = serde_json::from_str(fixture)
                .unwrap_or_else(|e| panic!("fixture ui_config_{name}.json 不是合法 JSON: {e}"));
            assert_eq!(
                got, want,
                "fixture ui_config_{name}.json 与 core 输出不一致;重新生成:
                 cargo run -q -p glaspen-core --example dump_ui_config 后按 @@MACOS@@/@@WINDOWS@@ 拆行"
            );
        }
    }

    /// 命名管道的响应协议形状(主机上可测, 不依赖 Windows 真机)。
    #[test]
    fn pipe_ui_config_response_is_line_framed_object() {
        let line = pipe_ui_config_response_json(42);
        assert!(line.ends_with('\n'), "响应必须以换行结尾(面板按行分帧)");
        assert_eq!(line.matches('\n').count(), 1, "一条响应只占一行");
        let v: Value = serde_json::from_str(line.trim_end()).expect("响应必须是合法 JSON");
        assert_eq!(v["type"], "uiConfig_response");
        assert_eq!(v["reqId"], 42);
        assert!(
            v["data"].is_object(),
            "data 必须是 JSON 对象, 不能是转义字符串: {}",
            v["data"]
        );
        assert_eq!(v["data"]["platform"], "windows");
        assert_eq!(
            v["data"]["actionOrder"],
            parsed(Platform::Windows)["actionOrder"]
        );
    }
}
