//! chord(快捷键组合)的唯一实现: 按键名表、规范化、保留组合、旧默认迁移。
//!
//! 规范: cmd+ctrl+alt+shift+<Key>, 顺序固定; 空串 = 未绑定; 至少含 ctrl 或 alt;
//! 修饰键大小写/别名容忍(cmd/command/meta/win, ctrl/control, alt/option/opt, shift)。
//! 按键名走物理位置语义, 与布局无关; 两端只保留自己的 OS 映射(macOS kVK /
//! Win32 VK)与注册方式。旧默认迁移只迁移"存档仍等于历史默认"的项。

use crate::ui::{self, Platform};

/// 可当全局快捷键的按键名(与两端 keycode/VK 表一一对应, 顺序无所谓)。
pub const KEY_NAMES: &[&str] = &[
    "A",
    "B",
    "C",
    "D",
    "E",
    "F",
    "G",
    "H",
    "I",
    "J",
    "K",
    "L",
    "M",
    "N",
    "O",
    "P",
    "Q",
    "R",
    "S",
    "T",
    "U",
    "V",
    "W",
    "X",
    "Y",
    "Z",
    "0",
    "1",
    "2",
    "3",
    "4",
    "5",
    "6",
    "7",
    "8",
    "9",
    "-",
    "=",
    "[",
    "]",
    "\\",
    ";",
    "'",
    "`",
    ",",
    ".",
    "/",
    "Left",
    "Right",
    "Up",
    "Down",
    "PageUp",
    "PageDown",
    "Home",
    "End",
    "Delete",
    "Backspace",
    "Return",
    "Escape",
    "Tab",
    "Space",
    "F1",
    "F2",
    "F3",
    "F4",
    "F5",
    "F6",
    "F7",
    "F8",
    "F9",
    "F10",
    "F11",
    "F12",
];

/// 规范化按键名(大小写/别名容忍); 不支持返回 None。
pub fn normalize_key_token(token: &str) -> Option<&'static str> {
    let t = token.trim();
    if t.is_empty() {
        return None;
    }
    if let Some(k) = KEY_NAMES.iter().find(|k| **k == t) {
        return Some(k);
    }
    KEY_NAMES
        .iter()
        .find(|k| k.eq_ignore_ascii_case(t))
        .copied()
}

/// 规范化 chord。空串 = 未绑定(返回 Some(""));非法返回 None。
pub fn normalize(raw: &str) -> Option<String> {
    let s = raw.trim();
    if s.is_empty() {
        return Some(String::new());
    }
    let parts: Vec<&str> = s.split('+').map(str::trim).collect();
    if parts.len() < 2 {
        return None;
    }
    let key = normalize_key_token(parts.last()?)?;
    let mut seen = [false; 4]; // cmd/ctrl/alt/shift
    for m in &parts[..parts.len() - 1] {
        let m = m.to_ascii_lowercase();
        let idx = match m.as_str() {
            "cmd" | "command" | "meta" | "win" => 0,
            "ctrl" | "control" => 1,
            "alt" | "option" | "opt" => 2,
            "shift" => 3,
            _ => return None,
        };
        if seen[idx] {
            return None;
        }
        seen[idx] = true;
    }
    if !seen[1] && !seen[2] {
        return None;
    }
    let mut out: Vec<&str> = Vec::new();
    for (i, name) in ["cmd", "ctrl", "alt", "shift"].iter().enumerate() {
        if seen[i] {
            out.push(name);
        }
    }
    out.push(key);
    Some(out.join("+"))
}

/// 保留组合(系统/常用 App/按住类功能占用), 单源在 ui::reserved。
pub fn is_reserved(platform: Platform, chord: &str) -> bool {
    ui::reserved(platform).contains(&chord)
}

/// 该平台的默认 chord 表(JSON: action -> chord), 供 macOS 壳解析。
pub fn defaults_json(platform: Platform) -> String {
    let map: std::collections::BTreeMap<&'static str, &'static str> =
        ui::defaults(platform).into_iter().collect();
    serde_json::to_string(&map).unwrap_or_else(|_| "{}".to_string())
}

/// 旧默认迁移: 返回迁移后的规范 chord; 无需迁移返回 None。
pub fn migrate_value(platform: Platform, action: &str, raw: &str) -> Option<String> {
    let norm = normalize(raw)?;
    let target = match platform {
        Platform::Macos => mac_legacy_target(action, &norm),
        Platform::Windows => windows_legacy_target(action, &norm),
    }?;
    Some(target.to_string())
}

fn mac_legacy_target(action: &str, norm: &str) -> Option<&'static str> {
    match action {
        "width_swap" if norm == "cmd+ctrl+W" => Some("cmd+ctrl+T"),
        "canvas_pan_left" if matches!(norm, "cmd+alt+Left" | "cmd+ctrl+Left" | "alt+A") => {
            ui::default_chord(Platform::Macos, action)
        }
        "canvas_pan_right" if matches!(norm, "cmd+alt+Right" | "cmd+ctrl+Right" | "alt+D") => {
            ui::default_chord(Platform::Macos, action)
        }
        "canvas_pan_up" if matches!(norm, "cmd+alt+Up" | "cmd+ctrl+Up" | "alt+W") => {
            ui::default_chord(Platform::Macos, action)
        }
        "canvas_pan_down" if matches!(norm, "cmd+alt+Down" | "cmd+ctrl+Down" | "alt+S") => {
            ui::default_chord(Platform::Macos, action)
        }
        "canvas_zoom_in" if matches!(norm, "cmd+alt+PageUp" | "cmd+ctrl+PageUp" | "alt+X") => {
            ui::default_chord(Platform::Macos, action)
        }
        "canvas_zoom_out" if matches!(norm, "cmd+alt+PageDown" | "cmd+ctrl+PageDown" | "alt+Z") => {
            ui::default_chord(Platform::Macos, action)
        }
        _ => None,
    }
}

fn windows_legacy_target(action: &str, norm: &str) -> Option<&'static str> {
    let default = |action: &str| ui::default_chord(Platform::Windows, action);
    match action {
        "canvas_pan_left"
            if matches!(
                norm,
                "ctrl+alt+Left" | "ctrl+alt+A" | "ctrl+alt+shift+A" | "alt+shift+A"
            ) =>
        {
            default(action)
        }
        "canvas_pan_right"
            if matches!(
                norm,
                "ctrl+alt+Right" | "ctrl+alt+D" | "ctrl+alt+shift+D" | "alt+shift+D"
            ) =>
        {
            default(action)
        }
        "canvas_pan_up"
            if matches!(
                norm,
                "ctrl+alt+Up" | "ctrl+alt+W" | "ctrl+alt+shift+W" | "alt+shift+W"
            ) =>
        {
            default(action)
        }
        "canvas_pan_down"
            if matches!(
                norm,
                "ctrl+alt+Down" | "ctrl+alt+S" | "ctrl+alt+shift+S" | "alt+shift+S"
            ) =>
        {
            default(action)
        }
        "canvas_zoom_in"
            if matches!(
                norm,
                "ctrl+alt+PageUp" | "ctrl+alt+X" | "ctrl+alt+shift+X" | "alt+shift+X"
            ) =>
        {
            default(action)
        }
        "canvas_zoom_out"
            if matches!(
                norm,
                "ctrl+alt+PageDown" | "ctrl+alt+shift+Z" | "alt+shift+Z"
            ) =>
        {
            default(action)
        }
        "settings" if norm == "ctrl+alt+shift+D" => default(action),
        "quit" if norm == "ctrl+alt+Q" => default(action),
        "color_swap" | "toggle_free_canvas" | "clear" | "export" | "copy_svg" | "toggle_glass"
        | "toggle_ethereal"
            if norm.is_empty() =>
        {
            default(action)
        }
        "width_swap" if matches!(norm, "" | "ctrl+alt+shift+W" | "ctrl+alt+shift+T") => {
            default(action)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_rules() {
        assert_eq!(normalize("").unwrap(), "");
        assert_eq!(normalize("  ").unwrap(), "");
        assert_eq!(
            normalize("shift+alt+ctrl+cmd+z").unwrap(),
            "cmd+ctrl+alt+shift+Z"
        );
        assert_eq!(normalize("option+ctrl+a").unwrap(), "ctrl+alt+A");
        assert_eq!(normalize("win+alt+a").unwrap(), "cmd+alt+A");
        assert_eq!(normalize("command+control+1").unwrap(), "cmd+ctrl+1");
        assert!(normalize("Z").is_none());
        assert!(normalize("cmd+shift+Z").is_none());
        assert!(normalize("ctrl+ctrl+Z").is_none());
        assert!(normalize("ctrl+alt+NoSuchKey").is_none());
        assert!(normalize("hyper+alt+Z").is_none());
    }

    #[test]
    fn key_tokens() {
        assert_eq!(normalize_key_token("a"), Some("A"));
        assert_eq!(normalize_key_token("pageup"), Some("PageUp"));
        assert_eq!(normalize_key_token("  Return "), Some("Return"));
        assert_eq!(normalize_key_token("NoSuchKey"), None);
    }

    #[test]
    fn mac_legacy_migration() {
        assert_eq!(
            migrate_value(Platform::Macos, "width_swap", "cmd+ctrl+W").as_deref(),
            Some("cmd+ctrl+T")
        );
        assert_eq!(
            migrate_value(Platform::Macos, "canvas_pan_left", "cmd+alt+Left").as_deref(),
            Some("ctrl+alt+A")
        );
        assert_eq!(
            migrate_value(Platform::Macos, "canvas_zoom_out", "alt+Z").as_deref(),
            Some("ctrl+alt+Z")
        );
        assert!(migrate_value(Platform::Macos, "canvas_pan_left", "ctrl+alt+A").is_none());
        assert!(migrate_value(Platform::Macos, "undo", "cmd+ctrl+Z").is_none());
        assert!(migrate_value(Platform::Macos, "width_swap", "cmd+ctrl+T").is_none());
    }

    #[test]
    fn windows_legacy_migration() {
        assert_eq!(
            migrate_value(Platform::Windows, "canvas_pan_up", "ctrl+alt+Up").as_deref(),
            Some("cmd+alt+W")
        );
        assert_eq!(
            migrate_value(Platform::Windows, "canvas_pan_right", "alt+shift+D").as_deref(),
            Some("cmd+alt+D")
        );
        assert_eq!(
            migrate_value(Platform::Windows, "settings", "ctrl+alt+shift+D").as_deref(),
            Some("ctrl+alt+D")
        );
        assert_eq!(
            migrate_value(Platform::Windows, "quit", "ctrl+alt+Q").as_deref(),
            Some("ctrl+alt+shift+Q")
        );
        assert_eq!(
            migrate_value(Platform::Windows, "color_swap", "").as_deref(),
            Some("ctrl+alt+shift+C")
        );
        assert_eq!(
            migrate_value(Platform::Windows, "width_swap", "ctrl+alt+shift+T").as_deref(),
            Some("ctrl+alt+T")
        );
        assert!(migrate_value(Platform::Windows, "undo", "ctrl+alt+U").is_none());
        assert!(migrate_value(Platform::Windows, "canvas_pan_left", "ctrl+alt+Q").is_none());
    }

    #[test]
    fn reserved_and_defaults_are_single_source() {
        assert!(is_reserved(Platform::Macos, "cmd+ctrl+Q"));
        assert!(is_reserved(Platform::Windows, "ctrl+alt+R"));
        assert!(!is_reserved(Platform::Macos, "ctrl+alt+A"));

        let mac: std::collections::BTreeMap<String, String> =
            serde_json::from_str(&defaults_json(Platform::Macos)).unwrap();
        let win: std::collections::BTreeMap<String, String> =
            serde_json::from_str(&defaults_json(Platform::Windows)).unwrap();
        let expect_mac: std::collections::BTreeMap<String, String> = ui::defaults(Platform::Macos)
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let expect_win: std::collections::BTreeMap<String, String> =
            ui::defaults(Platform::Windows)
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
        assert_eq!(mac, expect_mac);
        assert_eq!(win, expect_win);
        assert_eq!(
            mac.get("quit").map(String::as_str),
            Some("cmd+ctrl+shift+Q")
        );
        assert_eq!(
            win.get("quit").map(String::as_str),
            Some("ctrl+alt+shift+Q")
        );
    }
}
