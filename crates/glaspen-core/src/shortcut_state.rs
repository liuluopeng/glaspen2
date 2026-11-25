//! 快捷键生效表状态(从 Swift `ShortcutStore` 迁移)。
//!
//! values: action → chord(空串 = 显式解绑); lookup: chord → action。
//! 迁移/补默认/冲突处理仍在 Swift `ShortcutApply`(需要 AppKit 默认表),
//! 这里只保存最终生效表并提供查询/快照。
#![cfg(target_os = "macos")]

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

#[derive(Default)]
struct Store {
    values: BTreeMap<String, String>,
    lookup: BTreeMap<String, String>,
}

fn store() -> &'static Mutex<Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(Store::default()))
}

pub fn install(values: BTreeMap<String, String>, lookup: BTreeMap<String, String>) {
    let mut st = store().lock().unwrap_or_else(|e| e.into_inner());
    st.values = values;
    st.lookup = lookup;
}

pub fn chord(action: &str) -> Option<String> {
    store()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .values
        .get(action)
        .cloned()
}

pub fn action_for_chord(chord: &str) -> Option<String> {
    store()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .lookup
        .get(chord)
        .cloned()
}

/// 生效表为空(启动尚未 apply)。
pub fn is_empty() -> bool {
    store()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .values
        .is_empty()
}

/// 生效表 JSON(与旧 Swift `valuesSnapshot` 的 JSONSerialization 语义等价)。
pub fn values_json() -> String {
    let st = store().lock().unwrap_or_else(|e| e.into_inner());
    serde_json::to_string(&st.values).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_lookup_and_snapshot() {
        install(
            BTreeMap::from([
                ("undo".to_string(), "cmd+ctrl+Z".to_string()),
                ("redo".to_string(), "".to_string()),
            ]),
            BTreeMap::from([("cmd+ctrl+Z".to_string(), "undo".to_string())]),
        );
        assert_eq!(chord("undo").as_deref(), Some("cmd+ctrl+Z"));
        assert_eq!(chord("redo").as_deref(), Some(""));
        assert_eq!(action_for_chord("cmd+ctrl+Z").as_deref(), Some("undo"));
        assert!(action_for_chord("cmd+ctrl+Y").is_none());
        assert!(!is_empty());
        let v: serde_json::Value = serde_json::from_str(&values_json()).unwrap();
        assert_eq!(v["undo"], "cmd+ctrl+Z");
        assert_eq!(v["redo"], "");
        install(BTreeMap::new(), BTreeMap::new());
        assert!(is_empty());
        assert_eq!(values_json(), "{}");
    }
}
