//! 设置面板快照构建单源(D12)。
//!
//! 背景: 面板启动/热重载要拿一份"当前设置"JSON。快照曾是两份手工拼装:
//! macOS crates/glaspen-macos/swift/Menu/menu_runtime.swift 的 glaspen2_macos_settings_json(37 个
//! @"key": @(值) 的 NSDictionary)与 Windows
//! crates/glaspen-windows/src/overlay/pipe.rs 的 getSettings 巨型 format!
//! (同 37 项, 但字段顺序/类型/缺省值各写一遍)。任何一端漏改就会让 Flutter
//! 面板静默回显错值(历史 bug: nasUrl 漏映射导致 NAS 地址永远回显空)。
//!
//! 本模块把字段集合 / 输出顺序 / JSON 类型 / 缺省值 / DB 回退全部收口到
//! 一处; 两端 adapter 只提供运行时专属值(见 RUNTIME_ONLY_KEYS)与各自的帧
//! 格式, 不再拼设置 JSON:
//!
//! - macOS: ObjC 收集 overrides → FFI glaspen2_settings_snapshot_json →
//!   原样返回/转 C 字符串(帧仍由 api.rs 的 FRB 结构体承接);
//! - Windows: Rust 收集 overrides → 同进程调 snapshot_json → 套
//!   {"type":"getSettings_response","data":...}\n 帧。
//!
//! ## 字段集合与顺序
//!
//! - 字段集合 = crate::settings::keys_for(platform) = 登记表里该平台可用的
//!   全部键(macOS 37 = 36 个双平台键 + flipEffect;Windows 37 = 36 +
//!   ethereal;并集 38 = settings::SETTINGS)。
//! - 输出顺序 = 登记表顺序(按 key 字典序)。扁平对象, 无嵌套; Flutter
//!   _settingsToMap 按 key 取值, 顺序不敏感, 这里固定顺序只为快照可 diff。
//! - 例如 flipEffect 是 macOS 独有的"翻页动效", 不出现在 Windows 快照;
//!   ethereal 是 Windows 独有运行态, 不出现在 macOS 快照(见登记表 platforms)。
//!
//! ## 值的来源(三类)
//!
//! 1. DB 持久化设置(绝大多数): 读 user_settings 的规范落库键
//!    (settings::db_key; 旧 camelCase 别名行按 db::load_setting_with 的
//!    顺序回退), 缺失 → 本模块的缺省值; 读到的值再走一遍
//!    crate::settings_apply::effect_for 的宽容解析 + 区间钳制 —— 与面板
//!    setSetting 写入的规则同源, 所以"面板回显 == 再保存一次的结果"。
//! 2. 派生值:
//!    - color / width: 由 DB 的画笔设置(pen_r/g/b、width_scale)按预设表
//!      取最近档位(与两端启动恢复、presets::nearest_* 同源);
//!    - chatHasPassword: DB chat_password 非空(密码本体永不进快照);
//!    - language: 见下。
//! 3. 运行时专属值(adapter 必须提供, 不在 user_settings 或不能直接读):
//!    - launchAtLogin: 平台存储(LaunchAgent plist / HKCU Run);
//!    - shortcuts: 生效表需要 adapter 的默认补齐/保留组合/冲突消解
//!      (macOS gl_shortcuts_effective_json / Windows hotkeys::effective_json);
//!    - 可选覆盖 flipEffect(macOS GLASPEN2_FLIP_EFFECT 调试环境变量会改
//!      运行时值, DB 不反映)与 language(macOS 以 g_lang 运行时值为准)。
//!
//! ## language 字段
//!
//! snapshot_json 的 lang:
//! - None = 用 DB language(缺省 zh; 只有 "en" 是英文, 与
//!   i18n::Lang::from_code / settings_apply 的语言归一一致);
//! - Some(lang) = 显式语言(测试/需要运行时真值的调用方)。
//!
//! overrides 里的 language 优先级最高(运行时 > 显式参数 > DB), 且同样归一为
//! zh/en 两个值。
//!
//! ## 平台差异与行为说明
//!
//! - Windows 旧 getSettings 里 outline/minimap/infiniteCanvas/gif*/gridSize 读
//!   的是运行时原子量, 但它们在每次变更时都会落库、启动时又从同一个库键恢复,
//!   所以改读 DB 与旧值等价(少一处镜像态);
//! - 非法/越界的旧 DB 值(macOS 运行时启动时会钳制/跳过, Windows 旧实现直接
//!   回显)现在统一按缺省值或 effect_for 的钳制处理, 例如 invertFps=25 → 30、
//!   gridDivider=9 → 4;
//! - adapter 的失败语义不变: 本模块返回 Err → FFI 输出 {"error": "..."}
//!   (单行, 永不 NULL); Windows 管道仍回自己的错误帧。
//!
//! ## FFI
//!
//! crates/glaspen-core/src/export/mod.rs 的
//! glaspen2_settings_snapshot_json(overrides_json) 固定以 macOS 平台构建
//! (只服务 ObjC), overrides 解析失败/缺运行时键/未知键都回 error JSON。

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use serde_json::{Value, json};

use crate::i18n::Lang;
use crate::settings::{Kind, Platforms, SettingDef};
use crate::ui::Platform;

/// 必须由 adapter 提供的运行时专属值(核心无法从 DB 恢复)。
///
/// 缺少这些键时 snapshot_json 直接报 SnapshotError::MissingRuntimeOnly
/// (宁可红, 也不悄悄回退到一个可能过时的 DB 值)。
pub const RUNTIME_ONLY_KEYS: &[&str] = &[
    // 开机自启: LaunchAgent plist(macOS)/ HKCU Run(Windows)才是真实状态。
    "launchAtLogin",
    // 生效快捷键表(action → chord 的 JSON 字符串): 默认补齐/保留组合/
    // 冲突消解在 adapter 的热键注册表里, 直接读 DB 可能缺新动作的默认值。
    "shortcuts",
];

/// 快照构建失败(FFI 侧转成 {"error": "..."})。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotError {
    /// 缺少 adapter 必须提供的运行时专属值。
    MissingRuntimeOnly(&'static str),
    /// 覆盖字典里出现登记表之外的键(拼错/协议漂移)。
    UnknownKey(String),
    /// 覆盖字典里出现该平台不支持的登记键(如 Windows 传 flipEffect)。
    PlatformUnavailable { key: String, platform: Platform },
    /// 覆盖值的 JSON 类型与登记表 kind 不符。
    BadType {
        key: String,
        expected: &'static str,
        got: &'static str,
    },
    /// 序列化失败(目前不可能: 值都是有限数/字符串/bool)。
    Serialize(String),
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRuntimeOnly(key) => write!(
                f,
                "设置快照缺少运行时专属值 {key}(adapter 必须提供: {})",
                RUNTIME_ONLY_KEYS.join(" / ")
            ),
            Self::UnknownKey(key) => write!(f, "设置快照覆盖了未知键: {key}"),
            Self::PlatformUnavailable { key, platform } => write!(
                f,
                "设置键 {key} 在 {} 不可用(面板快照不该包含该平台未登记的字段)",
                platform_name(*platform)
            ),
            Self::BadType { key, expected, got } => {
                write!(f, "设置快照覆盖值 {key} 类型应为 {expected}, 收到 {got}")
            }
            Self::Serialize(e) => write!(f, "设置快照序列化失败: {e}"),
        }
    }
}

impl std::error::Error for SnapshotError {}

/// 两平台快照字段并集(登记表顺序, 38 项)。单平台子集见 field_names。
pub fn snapshot_field_names() -> Vec<&'static str> {
    crate::settings::SETTINGS.iter().map(|d| d.key).collect()
}

/// 某平台的快照字段(顺序 = 输出顺序 = 登记表字典序)。
pub fn field_names(platform: Platform) -> Vec<&'static str> {
    crate::settings::keys_for(platform)
}

/// 构建面板设置快照的单行 JSON。
///
/// - platform: 决定字段集合(macOS 含 flipEffect, Windows 含 ethereal);
/// - lang: None = 读 DB language(缺省 zh); Some = 显式语言;
/// - overrides: 运行时专属值(见 RUNTIME_ONLY_KEYS)。允许覆盖任意本平台登记
///   键(测试/调试用), 但类型必须与登记表 kind 一致; 未知键或其它平台专属键
///   直接报错(文档化策略: 报错, 不静默忽略)。
///
/// 返回的 JSON 是扁平对象, 字段按字典序, 值类型见登记表 kind
/// (Bool → true/false, Int → 整数, Float → 数字, Str → 字符串;
/// shortcuts 是字符串而不是对象, 与 UI 侧 _decodeShortcuts 兼容)。
pub fn snapshot_json(
    platform: Platform,
    lang: Option<Lang>,
    overrides: &serde_json::Map<String, Value>,
) -> Result<String, SnapshotError> {
    let db = DbRows::load();
    let fields = build_fields(platform, lang, overrides, &db)?;
    serde_json::to_string(&fields).map_err(|e| SnapshotError::Serialize(e.to_string()))
}

fn build_fields(
    platform: Platform,
    lang: Option<Lang>,
    overrides: &serde_json::Map<String, Value>,
    db: &DbRows,
) -> Result<BTreeMap<&'static str, Value>, SnapshotError> {
    validate_overrides(platform, overrides)?;
    let lang = effective_lang(lang, db, overrides);
    let mut fields = BTreeMap::new();
    for def in crate::settings::SETTINGS
        .iter()
        .filter(|def| available(platform, def.platforms))
    {
        fields.insert(def.key, field_value(def, lang, db, overrides)?);
    }
    Ok(fields)
}

/// user_settings 全表的内存视图(一次查询代替逐键 round-trip)。
#[derive(Default)]
struct DbRows {
    rows: HashMap<String, String>,
}

impl DbRows {
    fn load() -> Self {
        let Some(pool) = crate::db::pool() else {
            // 未初始化(全新进程/单测): 全部走缺省值。
            return Self::default();
        };
        Self::from_rows(crate::runtime().block_on(crate::db::load_user_settings_map_with(pool)))
    }

    fn from_rows(rows: HashMap<String, String>) -> Self {
        Self { rows }
    }

    /// 面板键 → 落库值。查找顺序与 db::load_setting_with 一致:
    /// 规范键 → 面板键别名 → 反向别名(旧库只存了面板键, 如 glass_enabled
    /// 只存成 frostedGlass)。
    fn resolved(&self, panel_key: &str) -> Option<&str> {
        let canonical = crate::settings::canonical_db_key(panel_key);
        if let Some(value) = self.rows.get(canonical) {
            return Some(value);
        }
        if canonical != panel_key
            && let Some(value) = self.rows.get(panel_key)
        {
            return Some(value);
        }
        for def in crate::settings::SETTINGS {
            if def.db_key == canonical
                && def.key != canonical
                && let Some(value) = self.rows.get(def.key)
            {
                return Some(value);
            }
        }
        None
    }

    /// 非登记键原样读取(pen_* / chat_password 等)。
    fn raw(&self, key: &str) -> Option<&str> {
        self.rows.get(key).map(String::as_str)
    }

    /// 画笔设置(与 db::load_settings 同语义: 四行任一缺失/解析失败 → 全缺省)。
    fn pen_settings(&self) -> (f64, f64, f64, f64) {
        let parse = |key: &str| self.raw(key)?.trim().parse::<f64>().ok();
        match (
            parse("pen_r"),
            parse("pen_g"),
            parse("pen_b"),
            parse("width_scale"),
        ) {
            (Some(r), Some(g), Some(b), Some(w)) if [r, g, b, w].iter().all(|v| v.is_finite()) => {
                (r, g, b, w)
            }
            // 与两端启动恢复的缺省画笔一致: 红 / 1.0 倍宽。
            _ => (1.0, 0.0, 0.0, 1.0),
        }
    }
}

fn validate_overrides(
    platform: Platform,
    overrides: &serde_json::Map<String, Value>,
) -> Result<(), SnapshotError> {
    for (key, value) in overrides {
        let Some(def) = crate::settings::get(key) else {
            return Err(SnapshotError::UnknownKey(key.clone()));
        };
        if !available(platform, def.platforms) {
            return Err(SnapshotError::PlatformUnavailable {
                key: key.clone(),
                platform,
            });
        }
        if !matches_kind(value, def.kind) {
            return Err(SnapshotError::BadType {
                key: key.clone(),
                expected: def.kind.as_str(),
                got: json_type_name(value),
            });
        }
    }
    for key in RUNTIME_ONLY_KEYS {
        let def = crate::settings::get(key).expect("运行时专属键必须登记在 settings::SETTINGS");
        debug_assert!(
            available(platform, def.platforms),
            "运行时专属键 {key} 必须两个平台都可用"
        );
        if !overrides.contains_key(*key) {
            return Err(SnapshotError::MissingRuntimeOnly(key));
        }
    }
    Ok(())
}

fn matches_kind(value: &Value, kind: Kind) -> bool {
    match kind {
        Kind::Bool => value.is_boolean(),
        Kind::Int => value.as_i64().is_some(),
        Kind::Float => value.as_f64().is_some_and(f64::is_finite),
        Kind::Str => value.is_string(),
    }
}

fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn available(platform: Platform, platforms: Platforms) -> bool {
    match platform {
        Platform::Macos => platforms.mac,
        Platform::Windows => platforms.win,
    }
}

fn platform_name(platform: Platform) -> &'static str {
    match platform {
        Platform::Macos => "macOS",
        Platform::Windows => "Windows",
    }
}

/// 语言字段的最终值: overrides(运行时)> 显式参数 > DB > 中文缺省。
fn effective_lang(
    lang: Option<Lang>,
    db: &DbRows,
    overrides: &serde_json::Map<String, Value>,
) -> Lang {
    if let Some(code) = overrides.get("language").and_then(Value::as_str) {
        return Lang::from_code(code);
    }
    if let Some(lang) = lang {
        return lang;
    }
    Lang::from_code(db.resolved("language").unwrap_or(""))
}

/// 单个字段的 JSON 值: override > 派生 > DB/缺省。
fn field_value(
    def: &SettingDef,
    lang: Lang,
    db: &DbRows,
    overrides: &serde_json::Map<String, Value>,
) -> Result<Value, SnapshotError> {
    // 运行时语言: 不读 DB(见 effective_lang), 归一为 en/zh; 也吃 overrides。
    if def.key == "language" {
        return Ok(Value::String(lang.code().to_string()));
    }
    if let Some(value) = overrides.get(def.key) {
        return Ok(value.clone());
    }
    match def.key {
        // 派生: 画笔 RGB/宽度 → 最近预设档位(与两端启动恢复同源)。
        "color" => {
            let (r, g, b, _) = db.pen_settings();
            Ok(json!(crate::presets::nearest_color_index(r, g, b) as i64))
        }
        "width" => {
            let (_, _, _, w) = db.pen_settings();
            Ok(json!(crate::presets::nearest_width_index(w) as i64))
        }
        // 派生: 密码本体永不进快照, 只回是否已保存。
        "chatHasPassword" => Ok(Value::Bool(
            db.raw("chat_password").is_some_and(|v| !v.is_empty()),
        )),
        // ethereal 是 Windows 的只读运行态(READONLY_KEYS, 面板不消费),
        // 但仍按旧 Windows 快照回显 DB 值。
        "ethereal" => Ok(Value::Bool(db.resolved(def.key).is_some_and(parse_db_bool))),
        key if RUNTIME_ONLY_KEYS.contains(&key) => {
            // validate_overrides 已挡住; 这里防御性报错而不是悄悄回退。
            Err(SnapshotError::MissingRuntimeOnly(key))
        }
        _ => Ok(db_value_or_default(def, db)),
    }
}

/// DB 值 → 快照值: 缺失/非法读值 → 缺省值; 合法值再按 D1 的 effect_for
/// 规则钳制(与 setSetting 写入同源)。
fn db_value_or_default(def: &SettingDef, db: &DbRows) -> Value {
    let fallback = default_value(def);
    let Some(raw) = db.resolved(def.key) else {
        return fallback;
    };
    let raw_json = match def.kind {
        // 历史落库是 "1"/"0"; 非法值按 atoi 语义 = 0(false), 与两端旧读取一致。
        Kind::Bool => Value::Bool(parse_db_bool(raw)),
        Kind::Int => match raw.trim().parse::<i64>() {
            Ok(n) => json!(n),
            Err(_) => return fallback,
        },
        Kind::Float => match raw.trim().parse::<f64>() {
            Ok(f) if f.is_finite() => json!(f),
            _ => return fallback,
        },
        Kind::Str => Value::String(raw.to_string()),
    };
    match crate::settings_apply::effect_for(def.key, &raw_json) {
        Ok(effect) => effect_payload(&effect),
        // 只读/派生键(此路径已排除)或非法值(如 invertFps=25): 用缺省值。
        Err(_) => fallback,
    }
}

fn parse_db_bool(raw: &str) -> bool {
    raw.trim().parse::<i64>().map(|n| n != 0).unwrap_or(false)
}

/// SettingEffect → 快照里该字段的 JSON 值(取类型载荷字段)。
fn effect_payload(effect: &crate::settings_apply::SettingEffect) -> Value {
    let json = effect.to_json();
    ["bool", "int", "float", "str"]
        .iter()
        .find_map(|field| json.get(*field).cloned())
        .unwrap_or(Value::Null)
}

/// 缺省值(两端旧快照/启动恢复的缺省; 新增键先在这里登记)。
fn default_value(def: &SettingDef) -> Value {
    match def.key {
        // 两端启动恢复里的"缺省开"(历史行为)。
        "glassFollowStrokes" | "bubbleReplayHoldLast" => Value::Bool(true),
        // 数值缺省与两端一致(macOS g_* 初值 / Windows 运行时缺省)。
        "gridSize" => json!(40.0),
        "gifFps" => json!(15),
        "gifResolution" => json!(0.5),
        "gifSpeed" => json!(2.0),
        "gifEndMode" => json!(1),
        "invertFps" => json!(30),
        "bubbleReplaySpeed" => json!(1),
        // 派生字段的缺省(无画笔存档时): 红 / 1.0 倍宽。
        "color" => json!(0),
        "width" => json!(3),
        _ => match def.kind {
            Kind::Bool => Value::Bool(false),
            Kind::Int => json!(0),
            Kind::Float => json!(0.0),
            Kind::Str => Value::String(String::new()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Map;
    use sqlx::SqlitePool;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn overrides(values: &[(&str, Value)]) -> Map<String, Value> {
        values
            .iter()
            .map(|(k, v)| ((*k).to_string(), v.clone()))
            .collect()
    }

    /// 两端 adapter 的最小合法 overrides(运行时专属值)。
    fn runtime_overrides() -> Map<String, Value> {
        overrides(&[
            ("launchAtLogin", Value::Bool(false)),
            ("shortcuts", Value::String("{}".to_string())),
        ])
    }

    fn fields(
        platform: Platform,
        lang: Option<Lang>,
        overrides: &Map<String, Value>,
        db: &DbRows,
    ) -> BTreeMap<&'static str, Value> {
        build_fields(platform, lang, overrides, db).expect("快照构建应成功")
    }

    /// 旧 macOS glaspen2_macos_settings_json 的 37 个字段(D12 重构前逐字抄录,
    /// 用于证明新单源没有加/删/改字段)。
    const OLD_MACOS_FIELDS: &[&str] = &[
        "color",
        "width",
        "launchAtLogin",
        "frostedGlass",
        "grid",
        "gridFollowStrokes",
        "glassFollowStrokes",
        "notebookStyle",
        "panelTransparent",
        "softShadow",
        "invertInk",
        "invertFps",
        "pressureMonitor",
        "outline",
        "infiniteCanvas",
        "minimap",
        "gridSize",
        "gridDivider",
        "flipEffect",
        "gifFps",
        "gifResolution",
        "gifSpeed",
        "gifEndMode",
        "chatApiBase",
        "chatUser",
        "chatHasPassword",
        "chatIntegration",
        "showFreeCanvas",
        "showBubbleTab",
        "bubbleReplaySpeed",
        "bubbleReplayHoldLast",
        "bubbleReplayAuto",
        "bubbleAutoNewPage",
        "nasUrl",
        "shareCanvas",
        "language",
        "shortcuts",
    ];

    /// 旧 Windows getSettings 的 37 个字段(同上, 逐字抄录)。
    const OLD_WINDOWS_FIELDS: &[&str] = &[
        "color",
        "width",
        "outline",
        "grid",
        "gridDivider",
        "gridSize",
        "minimap",
        "gridFollowStrokes",
        "glassFollowStrokes",
        "softShadow",
        "invertInk",
        "invertFps",
        "notebookStyle",
        "panelTransparent",
        "frostedGlass",
        "pressureMonitor",
        "ethereal",
        "infiniteCanvas",
        "gifFps",
        "gifResolution",
        "gifSpeed",
        "gifEndMode",
        "launchAtLogin",
        "chatIntegration",
        "shareCanvas",
        "showFreeCanvas",
        "showBubbleTab",
        "bubbleReplaySpeed",
        "bubbleReplayHoldLast",
        "bubbleReplayAuto",
        "bubbleAutoNewPage",
        "nasUrl",
        "chatHasPassword",
        "chatApiBase",
        "chatUser",
        "language",
        "shortcuts",
    ];

    fn sorted(fields: &[&str]) -> Vec<String> {
        let mut out: Vec<String> = fields.iter().map(|s| (*s).to_string()).collect();
        out.sort();
        out
    }

    /// 旧两端字段集合 → 新快照字段集合逐平台一致, 且 = 登记表该平台的键。
    #[test]
    fn field_sets_match_old_builders_and_registry() {
        assert_eq!(
            sorted(OLD_MACOS_FIELDS),
            sorted(&field_names(Platform::Macos))
        );
        assert_eq!(
            sorted(OLD_WINDOWS_FIELDS),
            sorted(&field_names(Platform::Windows))
        );
        // 并集 38 = 登记表全集; 平台专属字段各只在一端。
        let union = snapshot_field_names();
        assert_eq!(union.len(), 38);
        assert!(
            !field_names(Platform::Windows).contains(&"flipEffect"),
            "flipEffect 是 macOS 专属"
        );
        assert!(
            !field_names(Platform::Macos).contains(&"ethereal"),
            "ethereal 是 Windows 专属"
        );
        for key in OLD_MACOS_FIELDS.iter().chain(OLD_WINDOWS_FIELDS) {
            assert!(
                crate::settings::get(key).is_some(),
                "{key} 不在设置登记表里"
            );
        }
    }

    /// 字段输出顺序 = 登记表字典序(白盒: build_fields 的 BTreeMap 已排序)。
    #[test]
    fn field_order_is_registry_order() {
        let empty = DbRows::default();
        for platform in [Platform::Macos, Platform::Windows] {
            let keys: Vec<&str> = fields(platform, None, &runtime_overrides(), &empty)
                .keys()
                .copied()
                .collect();
            assert_eq!(keys, field_names(platform));
        }
    }

    /// 值的 JSON 类型必须与登记表 kind 一致(Flutter 按 bool/num/String 解析,
    /// 历史 bug: Windows 曾把 bool 发成 0/1 数字导致整份设置加载失败)。
    #[test]
    fn value_types_match_registry_kinds() {
        let empty = DbRows::default();
        for platform in [Platform::Macos, Platform::Windows] {
            let built = fields(platform, None, &runtime_overrides(), &empty);
            for def in crate::settings::SETTINGS
                .iter()
                .filter(|def| available(platform, def.platforms))
            {
                let value = &built[def.key];
                let ok = match def.kind {
                    Kind::Bool => value.is_boolean(),
                    Kind::Int => value.as_i64().is_some(),
                    Kind::Float => value.as_f64().is_some(),
                    Kind::Str => value.is_string(),
                };
                assert!(
                    ok,
                    "{}: {} 的类型与登记表 {:?} 不符: {value}",
                    platform_name(platform),
                    def.key,
                    def.kind
                );
            }
        }
    }

    /// macOS 缺省快照(空库 + 最小 overrides)的黄金串: 字段/顺序/类型/缺省值
    /// 一次锁死。与旧 builder 的差异只有顺序(旧 macOS 是 NSDictionary, 顺序本就
    /// 不保证; Flutter 按 key 取值)。
    #[test]
    fn golden_snapshot_macos() {
        let golden = serde_json::to_string(&fields(
            Platform::Macos,
            Some(Lang::Zh),
            &runtime_overrides(),
            &DbRows::default(),
        ))
        .unwrap();
        assert_eq!(
            golden,
            concat!(
                r#"{"bubbleAutoNewPage":false,"bubbleReplayAuto":false,"#,
                r#""bubbleReplayHoldLast":true,"bubbleReplaySpeed":1,"#,
                r#""chatApiBase":"","chatHasPassword":false,"chatIntegration":false,"#,
                r#""chatUser":"","color":0,"flipEffect":0,"frostedGlass":false,"#,
                r#""gifEndMode":1,"gifFps":15,"gifResolution":0.5,"gifSpeed":2.0,"#,
                r#""glassFollowStrokes":true,"grid":false,"gridDivider":0,"#,
                r#""gridFollowStrokes":false,"gridSize":40.0,"infiniteCanvas":false,"#,
                r#""invertFps":30,"invertInk":false,"language":"zh","launchAtLogin":false,"#,
                r#""minimap":false,"nasUrl":"","notebookStyle":false,"outline":false,"#,
                r#""panelTransparent":false,"pressureMonitor":false,"shareCanvas":false,"#,
                r#""shortcuts":"{}","showBubbleTab":false,"showFreeCanvas":false,"#,
                r#""softShadow":false,"width":3}"#
            )
        );
        // 公开入口也必须是单行合法 JSON, 且字段数与类型契约不变。
        let json = snapshot_json(Platform::Macos, Some(Lang::Zh), &runtime_overrides())
            .expect("快照构建应成功");
        assert!(!json.contains('\n'), "快照必须是单行 JSON");
        let parsed: Value = serde_json::from_str(&json).expect("快照必须是合法 JSON");
        let object = parsed.as_object().expect("快照必须是 JSON 对象");
        assert_eq!(object.len(), 37);
        assert_eq!(object.get("language"), Some(&json!("zh")));
    }

    /// Windows 缺省快照黄金串(无 flipEffect, 有 ethereal)。
    #[test]
    fn golden_snapshot_windows() {
        let golden = serde_json::to_string(&fields(
            Platform::Windows,
            Some(Lang::Zh),
            &runtime_overrides(),
            &DbRows::default(),
        ))
        .unwrap();
        assert_eq!(
            golden,
            concat!(
                r#"{"bubbleAutoNewPage":false,"bubbleReplayAuto":false,"#,
                r#""bubbleReplayHoldLast":true,"bubbleReplaySpeed":1,"#,
                r#""chatApiBase":"","chatHasPassword":false,"chatIntegration":false,"#,
                r#""chatUser":"","color":0,"ethereal":false,"frostedGlass":false,"#,
                r#""gifEndMode":1,"gifFps":15,"gifResolution":0.5,"gifSpeed":2.0,"#,
                r#""glassFollowStrokes":true,"grid":false,"gridDivider":0,"#,
                r#""gridFollowStrokes":false,"gridSize":40.0,"infiniteCanvas":false,"#,
                r#""invertFps":30,"invertInk":false,"language":"zh","launchAtLogin":false,"#,
                r#""minimap":false,"nasUrl":"","notebookStyle":false,"outline":false,"#,
                r#""panelTransparent":false,"pressureMonitor":false,"shareCanvas":false,"#,
                r#""shortcuts":"{}","showBubbleTab":false,"showFreeCanvas":false,"#,
                r#""softShadow":false,"width":3}"#
            )
        );
    }

    /// 运行时专属键缺失 → 明确报错(不静默回退 DB/缺省)。
    #[test]
    fn runtime_only_keys_are_required() {
        let empty = DbRows::default();
        let no_runtime = Map::new();
        let err = build_fields(Platform::Macos, None, &no_runtime, &empty).unwrap_err();
        assert_eq!(err, SnapshotError::MissingRuntimeOnly("launchAtLogin"));
        let only_launch = overrides(&[("launchAtLogin", Value::Bool(true))]);
        let err = build_fields(Platform::Macos, None, &only_launch, &empty).unwrap_err();
        assert_eq!(err, SnapshotError::MissingRuntimeOnly("shortcuts"));
        assert!(err.to_string().contains("shortcuts"));
    }

    /// 未知键 / 其它平台专属键 / 值类型不符 → 报错(文档化策略: 不静默忽略)。
    #[test]
    fn invalid_overrides_are_rejected() {
        let empty = DbRows::default();
        let unknown = overrides(&[
            ("launchAtLogin", Value::Bool(false)),
            ("shortcuts", Value::String("{}".to_string())),
            ("noSuchSetting", Value::Bool(true)),
        ]);
        assert_eq!(
            build_fields(Platform::Macos, None, &unknown, &empty).unwrap_err(),
            SnapshotError::UnknownKey("noSuchSetting".to_string())
        );
        let flip_on_windows = overrides(&[
            ("launchAtLogin", Value::Bool(false)),
            ("shortcuts", Value::String("{}".to_string())),
            ("flipEffect", json!(1)),
        ]);
        assert_eq!(
            build_fields(Platform::Windows, None, &flip_on_windows, &empty).unwrap_err(),
            SnapshotError::PlatformUnavailable {
                key: "flipEffect".to_string(),
                platform: Platform::Windows
            }
        );
        let bad_type = overrides(&[
            ("launchAtLogin", json!(1)),
            ("shortcuts", Value::String("{}".to_string())),
        ]);
        assert_eq!(
            build_fields(Platform::Macos, None, &bad_type, &empty).unwrap_err(),
            SnapshotError::BadType {
                key: "launchAtLogin".to_string(),
                expected: "bool",
                got: "number"
            }
        );
    }

    /// 语言字段: 显式参数 > DB; override 最高; 只认 "en"(其余 = zh)。
    #[test]
    fn language_field_is_language_dependent() {
        let empty = DbRows::default();
        let zh = fields(
            Platform::Macos,
            Some(Lang::Zh),
            &runtime_overrides(),
            &empty,
        );
        assert_eq!(zh["language"], json!("zh"));
        let en = fields(
            Platform::Macos,
            Some(Lang::En),
            &runtime_overrides(),
            &empty,
        );
        assert_eq!(en["language"], json!("en"));
        // None + 空库 → 中文缺省。
        let auto = fields(Platform::Macos, None, &runtime_overrides(), &empty);
        assert_eq!(auto["language"], json!("zh"));
        // overrides.language 优先级最高(运行时真值)。
        let mut with_lang = runtime_overrides();
        with_lang.insert("language".to_string(), json!("en"));
        let runtime_en = fields(Platform::Macos, Some(Lang::Zh), &with_lang, &empty);
        assert_eq!(runtime_en["language"], json!("en"));
        // 未知语言码归一为中文(与 i18n::Lang / settings_apply 一致)。
        let mut weird = runtime_overrides();
        weird.insert("language".to_string(), json!("fr"));
        let normalized = fields(Platform::Macos, Some(Lang::En), &weird, &empty);
        assert_eq!(normalized["language"], json!("zh"));
    }

    // ── DB 回退测试(临时 sqlite 池, 不碰全局库) ──────────────────────

    fn temp_pool() -> (SqlitePool, std::path::PathBuf) {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "glaspen2_settings_snapshot_{}_{}.db",
            std::process::id(),
            n
        ));
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
        let pool = crate::runtime().block_on(async {
            let pool = SqlitePool::connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true),
            )
            .await
            .expect("临时 sqlite 池");
            sqlx::query("CREATE TABLE user_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
                .execute(&pool)
                .await
                .expect("建 user_settings 表");
            pool
        });
        (pool, path)
    }

    fn set_setting(pool: &SqlitePool, key: &str, value: &str) {
        crate::runtime().block_on(async {
            sqlx::query("INSERT OR REPLACE INTO user_settings (key, value) VALUES (?1, ?2)")
                .bind(key)
                .bind(value)
                .execute(pool)
                .await
                .expect("写测试设置");
        });
    }

    fn rows(pool: &SqlitePool) -> DbRows {
        DbRows::from_rows(crate::runtime().block_on(crate::db::load_user_settings_map_with(pool)))
    }

    fn close_pool(pool: SqlitePool, path: &std::path::Path) {
        crate::runtime().block_on(pool.close());
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }

    /// DB 值覆盖缺省值; 旧 camelCase 别名行可回退; 非法/越界值按缺省/钳制处理;
    /// 派生字段(画笔档位/密码)来自 DB。
    #[test]
    fn db_values_win_over_defaults() {
        let (pool, path) = temp_pool();
        for (key, value) in [
            ("grid", "1"),
            ("grid_size", "55"),
            ("glass_follow_strokes", "0"),
            // 旧 Windows 的 camelCase 行(规范键 glass_enabled 缺失)也要能读。
            ("frostedGlass", "1"),
            ("invert_fps", "25"),
            ("grid_divider", "9"),
            ("gif_resolution", "999"),
            ("bubble_replay_speed", "0"),
            ("chat_password", "secret"),
            ("pen_r", "0.0"),
            ("pen_g", "0.588"),
            ("pen_b", "1.0"),
            ("width_scale", "2.5"),
            ("show_free_canvas", "1"),
            ("nas_url", "http://nas.local:8080"),
            ("language", "en"),
        ] {
            set_setting(&pool, key, value);
        }
        let db = rows(&pool);
        let built = fields(Platform::Macos, None, &runtime_overrides(), &db);
        assert_eq!(built["grid"], json!(true));
        assert_eq!(built["gridSize"], json!(55.0));
        assert_eq!(built["glassFollowStrokes"], json!(false));
        assert_eq!(built["frostedGlass"], json!(true));
        assert_eq!(built["invertFps"], json!(30), "非法帧率回缺省");
        assert_eq!(built["gridDivider"], json!(4), "分栏钳制 0..=4");
        assert_eq!(built["gifResolution"], json!(1.0), "分辨率钳制 0.1..=1.0");
        assert_eq!(built["bubbleReplaySpeed"], json!(1), "重播速度钳制 1..=8");
        assert_eq!(built["chatHasPassword"], json!(true));
        assert_eq!(built["color"], json!(5), "蓝笔(COLOR_PRESETS[5])");
        assert_eq!(built["width"], json!(6), "2.5 倍宽(WIDTH_PRESETS[6])");
        assert_eq!(built["showFreeCanvas"], json!(true));
        assert_eq!(built["nasUrl"], json!("http://nas.local:8080"));
        assert_eq!(built["language"], json!("en"), "lang=None 时读 DB language");

        // override 覆盖 DB(overrides > DB > 缺省)。
        let mut with_override = runtime_overrides();
        with_override.insert("grid".to_string(), json!(false));
        with_override.insert("launchAtLogin".to_string(), json!(true));
        let built = fields(Platform::Macos, None, &with_override, &db);
        assert_eq!(built["grid"], json!(false));
        assert_eq!(built["launchAtLogin"], json!(true));

        // Windows 专属字段: ethereal 从 DB 回显(旧快照行为)。
        set_setting(&pool, "ethereal", "1");
        let db = rows(&pool);
        let built = fields(Platform::Windows, None, &runtime_overrides(), &db);
        assert_eq!(built["ethereal"], json!(true));
        close_pool(pool, &path);
    }

    // ── D12 适配器闸门: 快照只许由 core 构建 ─────────────────────────
    //
    // 快照曾是两端手拼(macOS 37 项 NSDictionary / Windows 巨型 format!)。
    // 闸门要求: 两端"取快照"的函数体里必须出现 core 入口 token, 且不得再出现
    // 任何登记表字段键(手拼指纹); adapter 的 overrides 只允许运行时专属值 +
    // 文档化可选覆盖(flipEffect/language), 不许夹带持久化设置。
    // 负向验证见 docs/duplication-backlog.md D12 进度记录。

    const MAC_MENU_SRC: &str =
        include_str!("../../../crates/glaspen-macos/swift/Menu/menu_runtime.swift");
    const WIN_PIPE_SRC: &str = include_str!("../../../crates/glaspen-windows/src/overlay/pipe.rs");

    /// 去注释(保留字符串字面量; 支持 // 与 /* */; 处理 Rust 字符字面量与
    /// 生命周期, 避免把 '"' 误当字符串起点)。
    fn strip_comments(src: &str) -> String {
        let mut out = String::with_capacity(src.len());
        let mut chars = src.chars().peekable();
        let mut in_string = false;
        let mut escaped = false;
        while let Some(c) = chars.next() {
            if in_string {
                out.push(c);
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == '"' {
                    in_string = false;
                }
                continue;
            }
            match c {
                '"' => {
                    in_string = true;
                    out.push(c);
                }
                '\'' => {
                    // 字符字面量('"', '\n', 'a')才整体跳过; 生命周期 'a 原样输出。
                    let mut look = chars.clone();
                    let next = look.next();
                    let after = look.next();
                    let is_char = matches!(next, Some('\\')) || matches!(after, Some('\''));
                    out.push(c);
                    if is_char && let Some(n) = chars.next() {
                        out.push(n);
                        if n == '\\'
                            && let Some(e) = chars.next()
                        {
                            out.push(e);
                        }
                        if chars.peek() == Some(&'\'') {
                            out.push(chars.next().unwrap());
                        }
                    }
                }
                '/' => match chars.peek() {
                    Some('/') => {
                        for c in chars.by_ref() {
                            if c == '\n' {
                                out.push('\n');
                                break;
                            }
                        }
                    }
                    Some('*') => {
                        chars.next();
                        let mut prev = '\0';
                        for c in chars.by_ref() {
                            if prev == '*' && c == '/' {
                                break;
                            }
                            prev = c;
                        }
                    }
                    _ => out.push(c),
                },
                _ => out.push(c),
            }
        }
        out
    }

    /// 从 needle 后第一个 '{' 起做花括号配对, 返回函数/块体(含大括号)。
    fn brace_body(src: &str, at: usize) -> Option<&str> {
        let open = at + src[at..].find('{')?;
        let mut depth = 0i32;
        for (k, ch) in src[open..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(&src[open..=open + k]);
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn body_after<'a>(src: &'a str, needle: &str) -> Option<&'a str> {
        brace_body(src, src.find(needle)?)
    }

    /// Windows 管道 if/else-if 链里的一个分支体(到下一个 else-if 之前)。
    fn pipe_branch<'a>(code: &'a str, needle: &str) -> Option<&'a str> {
        let start = code.find(needle)?;
        let rest = &code[start..];
        let end = rest.find("} else if msg_type ==").unwrap_or(rest.len());
        Some(&rest[..end])
    }

    #[test]
    fn adapters_build_panel_snapshot_only_in_core() {
        let mac_code = strip_comments(MAC_MENU_SRC);
        // overrides 组装已迁 Swift(menu_glue.swift), 闸门同时扫描两边。
        let mac_glue_swift =
            include_str!("../../../crates/glaspen-macos/swift/Menu/menu_runtime.swift");
        let win_code = strip_comments(WIN_PIPE_SRC);
        let mac_snapshot_glue = include_str!("../../../crates/glaspen-macos/src/macos_shell.rs");
        let fn_at = mac_snapshot_glue
            .find("fn settings_snapshot_pb")
            .expect("macos_shell.rs 缺 settings_snapshot_pb");
        let rel_end = mac_snapshot_glue[fn_at..]
            .find('}')
            .expect("settingsApiJSON 不完整");
        let mac_snapshot = &mac_snapshot_glue[fn_at..fn_at + rel_end];
        let win_snapshot = pipe_branch(&win_code, "msg_type == \"getSettings\"")
            .expect("Windows 管道缺 getSettings 分支");

        assert!(
            mac_snapshot.contains("settings_snapshot::snapshot_json"),
            "macOS 快照函数未走 core settings_snapshot::snapshot_json"
        );
        assert!(
            win_snapshot.contains("settings_snapshot::snapshot_json"),
            "Windows getSettings 分支未走 core 入口(settings_snapshot::snapshot_json)"
        );

        // 1) 快照函数体里不得再出现任何登记字段键(旧手拼指纹; 负向验证: 插回
        //    一行 @"grid": ... 或 "\"grid\": ... 即红)。
        let mut problems = Vec::new();
        for def in crate::settings::SETTINGS {
            if mac_snapshot.contains(def.key) {
                problems.push(format!("macOS 快照函数体仍手拼字段 {}", def.key));
            }
            if win_snapshot.contains(def.key) {
                problems.push(format!("Windows getSettings 分支仍手拼字段 {}", def.key));
            }
        }
        assert!(
            problems.is_empty(),
            "D12 闸门失败(adapter 只许收运行时值, 快照交给 core):\n{}",
            problems.join("\n")
        );

        // 2) overrides 只允许运行时专属值 + 文档化可选运行时覆盖; 出现持久化
        //    设置键 = 又在一处"手工回填", 应改 core settings_snapshot。
        const ALLOWED_OVERRIDES: &[&str] =
            &["launchAtLogin", "shortcuts", "flipEffect", "language"];
        // overrides 组装已迁 Swift(menu_glue.swift); ObjC 转发可选。
        let mac_overrides_owner =
            body_after(&mac_code, "gl_settings_snapshot_overrides_json(void)").unwrap_or("");
        let mac_overrides = format!("{mac_overrides_owner}\n{mac_glue_swift}");
        let win_overrides = body_after(&win_code, "fn settings_snapshot_overrides")
            .expect("Windows 缺 settings_snapshot_overrides");
        let mut problems = Vec::new();
        for def in crate::settings::SETTINGS {
            if ALLOWED_OVERRIDES.contains(&def.key) {
                continue;
            }
            if mac_overrides_owner.contains(def.key) {
                problems.push(format!("macOS overrides 夹带了持久化设置 {}", def.key));
            }
            if mac_glue_swift.contains(&format!("\"{}\":", def.key)) {
                problems.push(format!(
                    "macOS Swift overrides 夹带了持久化设置 {}",
                    def.key
                ));
            }
            if win_overrides.contains(def.key) {
                problems.push(format!("Windows overrides 夹带了持久化设置 {}", def.key));
            }
        }
        for key in RUNTIME_ONLY_KEYS {
            if !mac_overrides.contains(key) {
                problems.push(format!("macOS overrides 缺运行时专属值 {key}"));
            }
            if !win_overrides.contains(key) {
                problems.push(format!("Windows overrides 缺运行时专属值 {key}"));
            }
        }
        assert!(
            problems.is_empty(),
            "D12 闸门失败(overrides 只许放运行时专属值):\n{}",
            problems.join("\n")
        );
    }
}
