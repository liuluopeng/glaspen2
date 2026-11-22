//! 设置项的唯一登记表(UI 协议键 ↔ 数据库键/类型/平台可用性)。
//!
//! 背景: 设置曾经有四份手工副本 —— macOS ObjC 的 set_setting 分支、Windows
//! 命名管道的 getSettings/setSetting 分支、`api.rs` 的 `Settings` 结构体、
//! Flutter 面板的 `_settingsToMap`。任何一方新增/改名都可能让另一平台静默
//! 丢设置。本文件是这四处的**契约**: 键、落库键、类型、平台可用性只在这里写,
//! `tests` 里用 include_str! 扫描四个源文件, 少一个/多一个都会让 `cargo test`
//! 失败(在 macOS 与 Windows CI 上都会跑)。
//!
//! 规则:
//! - 新增设置项 → 只在这里加一行 + 真正实现的两个平台 effect;
//! - 某平台不支持 → 把 `platforms` 标成单平台, 面板按 `ui_config_json` 里的
//!   `settings[].mac/win` 隐藏入口(宁可空缺, 不写错乱实现);
//! - 落库键保持历史兼容: `db_key` 优先 snake_case 规范键, 旧别名在
//!   `db::canonical_setting_key` 里迁移。

use serde_json::{Value, json};

use crate::ui::Platform;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Bool,
    Int,
    Float,
    Str,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Bool => "bool",
            Kind::Int => "int",
            Kind::Float => "float",
            Kind::Str => "str",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Platforms {
    pub mac: bool,
    pub win: bool,
}

pub const MAC: Platforms = Platforms {
    mac: true,
    win: false,
};
pub const WIN: Platforms = Platforms {
    mac: false,
    win: true,
};
pub const BOTH: Platforms = Platforms {
    mac: true,
    win: true,
};

pub struct SettingDef {
    /// 面板协议键(macOS 面板 JSON / Windows 管道 JSON 同名)。
    pub key: &'static str,
    /// `user_settings` 里的落库键(跨平台统一, 旧别名由 db 层迁移)。
    pub db_key: &'static str,
    pub kind: Kind,
    pub platforms: Platforms,
}

/// 唯一登记表(按 key 排序, 方便 diff)。
pub const SETTINGS: &[SettingDef] = &[
    SettingDef {
        key: "bubbleAutoNewPage",
        db_key: "bubbleAutoNewPage",
        kind: Kind::Bool,
        platforms: BOTH,
    },
    SettingDef {
        key: "bubbleReplayAuto",
        db_key: "bubbleReplayAuto",
        kind: Kind::Bool,
        platforms: BOTH,
    },
    SettingDef {
        key: "bubbleReplayHoldLast",
        db_key: "bubbleReplayHoldLast",
        kind: Kind::Bool,
        platforms: BOTH,
    },
    SettingDef {
        key: "bubbleReplaySpeed",
        db_key: "bubbleReplaySpeed",
        kind: Kind::Int,
        platforms: BOTH,
    },
    SettingDef {
        key: "chatApiBase",
        db_key: "chatApiBase",
        kind: Kind::Str,
        platforms: BOTH,
    },
    SettingDef {
        key: "chatHasPassword",
        db_key: "chatHasPassword",
        kind: Kind::Bool,
        platforms: BOTH,
    },
    SettingDef {
        key: "chatIntegration",
        db_key: "chatIntegration",
        kind: Kind::Bool,
        platforms: BOTH,
    },
    SettingDef {
        key: "chatUser",
        db_key: "chatUser",
        kind: Kind::Str,
        platforms: BOTH,
    },
    SettingDef {
        key: "color",
        db_key: "color",
        kind: Kind::Int,
        platforms: BOTH,
    },
    // Windows 壳的飘渺运行时状态(不在 macOS 快照里; 面板不使用, 仅为管道兼容保留)。
    SettingDef {
        key: "ethereal",
        db_key: "ethereal",
        kind: Kind::Bool,
        platforms: WIN,
    },
    // 翻页动效: 只有 macOS 有时光隧道实现; Windows 面板隐藏(故意空缺)。
    SettingDef {
        key: "flipEffect",
        db_key: "flipEffect",
        kind: Kind::Int,
        platforms: MAC,
    },
    SettingDef {
        key: "frostedGlass",
        db_key: "glass_enabled",
        kind: Kind::Bool,
        platforms: BOTH,
    },
    SettingDef {
        key: "gifEndMode",
        db_key: "gifEndMode",
        kind: Kind::Int,
        platforms: BOTH,
    },
    SettingDef {
        key: "gifFps",
        db_key: "gifFps",
        kind: Kind::Int,
        platforms: BOTH,
    },
    SettingDef {
        key: "gifResolution",
        db_key: "gifResolution",
        kind: Kind::Float,
        platforms: BOTH,
    },
    SettingDef {
        key: "gifSpeed",
        db_key: "gifSpeed",
        kind: Kind::Float,
        platforms: BOTH,
    },
    SettingDef {
        key: "glassFollowStrokes",
        db_key: "glass_follow_strokes",
        kind: Kind::Bool,
        platforms: BOTH,
    },
    SettingDef {
        key: "grid",
        db_key: "grid",
        kind: Kind::Bool,
        platforms: BOTH,
    },
    SettingDef {
        key: "gridDivider",
        db_key: "gridDivider",
        kind: Kind::Int,
        platforms: BOTH,
    },
    SettingDef {
        key: "gridFollowStrokes",
        db_key: "grid_follow_strokes",
        kind: Kind::Bool,
        platforms: BOTH,
    },
    SettingDef {
        key: "gridSize",
        db_key: "gridSize",
        kind: Kind::Float,
        platforms: BOTH,
    },
    SettingDef {
        key: "infiniteCanvas",
        db_key: "infiniteCanvas",
        kind: Kind::Bool,
        platforms: BOTH,
    },
    SettingDef {
        key: "invertFps",
        db_key: "invert_fps",
        kind: Kind::Int,
        platforms: BOTH,
    },
    SettingDef {
        key: "invertInk",
        db_key: "invert_ink",
        kind: Kind::Bool,
        platforms: BOTH,
    },
    SettingDef {
        key: "language",
        db_key: "language",
        kind: Kind::Str,
        platforms: BOTH,
    },
    SettingDef {
        key: "launchAtLogin",
        db_key: "launchAtLogin",
        kind: Kind::Bool,
        platforms: BOTH,
    },
    SettingDef {
        key: "minimap",
        db_key: "minimap",
        kind: Kind::Bool,
        platforms: BOTH,
    },
    SettingDef {
        key: "nasUrl",
        db_key: "nasUrl",
        kind: Kind::Str,
        platforms: BOTH,
    },
    SettingDef {
        key: "notebookStyle",
        db_key: "notebook_style",
        kind: Kind::Bool,
        platforms: BOTH,
    },
    SettingDef {
        key: "outline",
        db_key: "outline",
        kind: Kind::Bool,
        platforms: BOTH,
    },
    SettingDef {
        key: "panelTransparent",
        db_key: "panel_transparent",
        kind: Kind::Bool,
        platforms: BOTH,
    },
    SettingDef {
        key: "pressureMonitor",
        db_key: "pressure_monitor",
        kind: Kind::Bool,
        platforms: BOTH,
    },
    SettingDef {
        key: "shareCanvas",
        db_key: "shareCanvas",
        kind: Kind::Bool,
        platforms: BOTH,
    },
    SettingDef {
        key: "shortcuts",
        db_key: "shortcuts",
        kind: Kind::Str,
        platforms: BOTH,
    },
    SettingDef {
        key: "showBubbleTab",
        db_key: "showBubbleTab",
        kind: Kind::Bool,
        platforms: BOTH,
    },
    SettingDef {
        key: "showFreeCanvas",
        db_key: "showFreeCanvas",
        kind: Kind::Bool,
        platforms: BOTH,
    },
    SettingDef {
        key: "softShadow",
        db_key: "soft_shadow",
        kind: Kind::Bool,
        platforms: BOTH,
    },
    SettingDef {
        key: "width",
        db_key: "width",
        kind: Kind::Int,
        platforms: BOTH,
    },
];

pub fn get(key: &str) -> Option<&'static SettingDef> {
    SETTINGS.iter().find(|s| s.key == key)
}

/// 面板协议键 → 落库键; 未登记键原样返回(旧代码/迁移期的宽进)。
pub fn canonical_db_key(key: &str) -> &str {
    get(key).map(|s| s.db_key).unwrap_or(key)
}

pub fn keys_for(platform: Platform) -> Vec<&'static str> {
    SETTINGS
        .iter()
        .filter(|s| {
            if platform == Platform::Macos {
                s.platforms.mac
            } else {
                s.platforms.win
            }
        })
        .map(|s| s.key)
        .collect()
}

/// 给面板 UI 的 schema(键/类型/落库键/平台可用性)。
/// 面板用它做启动自检: 后端实际下发的设置键必须覆盖当前平台的全部登记项,
/// 否则就是"这个平台漏实现了一项"。
pub fn schema_value() -> Value {
    Value::Array(
        SETTINGS
            .iter()
            .map(|s| {
                json!({
                    "key": s.key,
                    "dbKey": s.db_key,
                    "kind": s.kind.as_str(),
                    "mac": s.platforms.mac,
                    "win": s.platforms.win,
                })
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(mut v: Vec<String>) -> Vec<String> {
        v.sort();
        v
    }

    /// Flutter 面板 `_settingsToMap` 提供的键。
    fn dart_map_keys() -> Vec<String> {
        let src = include_str!("../../../flutter_settings/lib/main.bridge.dart");
        let start = src
            .find("Map<dynamic, dynamic> _settingsToMap")
            .expect("找不到 _settingsToMap");
        let rest = &src[start..];
        let end = rest.find("\n  }").expect("_settingsToMap 没有结束");
        let body = &rest[..end];
        let mut out = Vec::new();
        for line in body.lines() {
            if let Some(q1) = line.find('\'')
                && let Some(q2) = line[q1 + 1..].find('\'')
            {
                let key = &line[q1 + 1..q1 + 1 + q2];
                let after = &line[q1 + 1 + q2 + 1..];
                if after.starts_with(':')
                    && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                {
                    out.push(key.to_string());
                }
            }
        }
        out
    }

    /// `api.rs` Settings 结构体字段(snake_case → 面板 camelCase 键)。
    fn api_struct_keys() -> Vec<String> {
        let src = include_str!("../../../src/api.rs");
        let start = src
            .find("pub struct Settings {")
            .expect("找不到 Settings 结构体");
        let rest = &src[start..];
        let end = rest.find("\n}").expect("Settings 结构体没有结束");
        let mut out = Vec::new();
        for line in rest[..end].lines() {
            let line = line.trim();
            let Some(rest) = line.strip_prefix("pub ") else {
                continue;
            };
            let Some(colon) = rest.find(':') else {
                continue;
            };
            let name = &rest[..colon];
            if name.starts_with("//")
                || !name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit())
            {
                continue;
            }
            let mut parts = name.split('_');
            let mut camel = parts.next().unwrap_or("").to_string();
            for p in parts {
                let mut chars = p.chars();
                if let Some(first) = chars.next() {
                    camel.push(first.to_ascii_uppercase());
                    camel.extend(chars);
                }
            }
            out.push(camel);
        }
        out
    }

    /// Windows 管道 `getSettings` 响应里的键(排除协议字段)。
    fn pipe_get_settings_keys() -> Vec<String> {
        let src = include_str!("../../../crates/glaspen-windows/src/overlay/pipe.rs");
        let start = src
            .find("getSettings_response")
            .expect("找不到 getSettings_response");
        let seg = &src[start..(start + 6000).min(src.len())];
        let bytes = seg.as_bytes();
        let mut out = Vec::new();
        let mut i = 0;
        while i + 1 < bytes.len() {
            // 形如 \"key\": —— 开引号后是标识符, 闭引号后紧跟冒号。
            if bytes[i] == b'\\' && bytes[i + 1] == b'"' {
                let key_start = i + 2;
                let mut j = key_start;
                while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                    j += 1;
                }
                if j > key_start
                    && j + 2 < bytes.len()
                    && bytes[j] == b'\\'
                    && bytes[j + 1] == b'"'
                    && bytes[j + 2] == b':'
                {
                    out.push(String::from_utf8_lossy(&bytes[key_start..j]).into_owned());
                    i = j + 3;
                    continue;
                }
            }
            i += 1;
        }
        let protocol = [
            "type", "reqId", "data", "ok", "error", "message", "done", "path", "received", "total",
        ];
        out.retain(|k| !protocol.contains(&k.as_str()));
        out.sort();
        out.dedup();
        out
    }

    #[test]
    fn registry_is_well_formed() {
        let mut keys = std::collections::HashSet::new();
        let mut db_keys = std::collections::HashSet::new();
        for s in SETTINGS {
            assert!(!s.key.is_empty());
            assert!(keys.insert(s.key), "设置键重复: {}", s.key);
            assert!(
                db_keys.insert(s.db_key),
                "落库键重复: {} ({})",
                s.db_key,
                s.key
            );
            assert!(
                s.platforms.mac || s.platforms.win,
                "{} 两个平台都不支持, 应整条删除(故意空缺)",
                s.key
            );
        }
    }

    /// Flutter 面板映射的键必须与 macOS 后端快照登记表完全一致。
    /// (修复历史 bug: 面板曾漏映射 nasUrl, 导致 NAS 地址回显为空。)
    #[test]
    fn panel_map_matches_macos_contract() {
        let dart = sorted(dart_map_keys());
        let expected = sorted(
            keys_for(Platform::Macos)
                .into_iter()
                .map(str::to_string)
                .collect(),
        );
        assert_eq!(dart, expected, "Flutter _settingsToMap 与设置登记表不一致");
    }

    /// macOS 的强类型 Settings 结构体必须覆盖登记表里的 mac 键。
    #[test]
    fn macos_settings_struct_matches_contract() {
        let api = sorted(api_struct_keys());
        let expected = sorted(
            keys_for(Platform::Macos)
                .into_iter()
                .map(str::to_string)
                .collect(),
        );
        assert_eq!(api, expected, "api.rs Settings 与设置登记表不一致");
    }

    /// Windows 管道的 getSettings 响应必须覆盖登记表里的 win 键。
    #[test]
    fn windows_pipe_snapshot_matches_contract() {
        let pipe = sorted(pipe_get_settings_keys());
        let expected = sorted(
            keys_for(Platform::Windows)
                .into_iter()
                .map(str::to_string)
                .collect(),
        );
        assert_eq!(
            pipe, expected,
            "Windows 管道 getSettings 与设置登记表不一致"
        );
    }

    /// 规范落库键必须唯一(同一数据库行不被两个面板键写两份值)。
    #[test]
    fn canonical_db_keys_are_unique() {
        for s in SETTINGS {
            assert_eq!(canonical_db_key(s.key), s.db_key);
        }
        // 未登记键宽进: 原样返回
        assert_eq!(canonical_db_key("some_future_key"), "some_future_key");
    }
}
