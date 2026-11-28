// glaspen2 跨语言/跨进程 protobuf 契约与帧编解码。
//
// 所有跨 machine boundary 的消息都走这里生成的类型; 线上格式是
// u32 little-endian 长度 + Frame protobuf bytes, 不再传 JSON 文本。
//
// 同进程 Rust <-> Rust(实时绘制计划/录制状态机)仍直接使用 typed Rust API,
// 不经 protobuf —— 那不是机器通信, 序列化只会增加开销。

#![allow(clippy::doc_markdown)]

use prost::Message;

pub mod panel_wire;

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/glaspen.ipc.v1.rs"));
}

pub use proto::*;

/// 当前线协议版本; 客户端/服务端握手时拒绝不兼容版本。
pub const PROTOCOL_VERSION: u32 = 1;

/// 单帧上限(防止坏客户端发超长长度导致 OOM)。
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    TooShort,
    TooLarge { len: usize, max: usize },
    LengthMismatch { expected: usize, actual: usize },
    Decode(String),
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort => write!(f, "protobuf 帧短于 4 字节长度头"),
            Self::TooLarge { len, max } => write!(f, "protobuf 帧过大: {len} > {max}"),
            Self::LengthMismatch { expected, actual } => {
                write!(f, "protobuf 帧长度不匹配: 头部 {expected}, 实际 {actual}")
            }
            Self::Decode(err) => write!(f, "protobuf 帧解码失败: {err}"),
        }
    }
}

impl std::error::Error for FrameError {}

/// 编码为线格式: u32 LE 长度 + protobuf bytes。
pub fn encode_frame(frame: &Frame) -> Vec<u8> {
    let payload = frame.encode_to_vec();
    let mut out = Vec::with_capacity(4 + payload.len());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&payload);
    out
}

/// 解码一个完整帧(缓冲区必须正好是一帧; 流式读取由调用方切帧)。
pub fn decode_frame(bytes: &[u8]) -> Result<Frame, FrameError> {
    if bytes.len() < 4 {
        return Err(FrameError::TooShort);
    }
    let expected = u32::from_le_bytes(bytes[..4].try_into().expect("4 bytes")) as usize;
    if expected > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge {
            len: expected,
            max: MAX_FRAME_BYTES,
        });
    }
    let actual = bytes.len() - 4;
    if expected != actual {
        return Err(FrameError::LengthMismatch { expected, actual });
    }
    Frame::decode(&bytes[4..]).map_err(|err| FrameError::Decode(err.to_string()))
}

/// 从读缓冲区切出下一帧; 返回 (frame, consumed); 不足一帧返回 None。
pub fn take_frame(bytes: &[u8]) -> Result<Option<(Frame, usize)>, FrameError> {
    if bytes.len() < 4 {
        return Ok(None);
    }
    let expected = u32::from_le_bytes(bytes[..4].try_into().expect("4 bytes")) as usize;
    if expected > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge {
            len: expected,
            max: MAX_FRAME_BYTES,
        });
    }
    let total = 4 + expected;
    if bytes.len() < total {
        return Ok(None);
    }
    let frame = decode_frame(&bytes[..total])?;
    Ok(Some((frame, total)))
}

/// 构造一个带当前协议版本的空帧。
pub fn frame(request_id: u64) -> Frame {
    Frame {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        body: None,
    }
}

/// 检查版本是否兼容。
pub fn compatible_version(version: u32) -> bool {
    version == PROTOCOL_VERSION
}

/// 原 JSON 值 → protobuf Struct/Value(过渡 pipe 用; 线上不再出现 JSON 文本)。
pub fn json_to_prost_value(value: &serde_json::Value) -> prost_types::Value {
    use prost_types::value::Kind;
    let kind = match value {
        serde_json::Value::Null => Some(Kind::NullValue(0)),
        serde_json::Value::Bool(v) => Some(Kind::BoolValue(*v)),
        serde_json::Value::Number(n) => Some(Kind::NumberValue(n.as_f64().unwrap_or(0.0))),
        serde_json::Value::String(v) => Some(Kind::StringValue(v.clone())),
        serde_json::Value::Array(items) => Some(Kind::ListValue(prost_types::ListValue {
            values: items.iter().map(json_to_prost_value).collect(),
        })),
        serde_json::Value::Object(map) => Some(Kind::StructValue(prost_types::Struct {
            fields: map
                .iter()
                .map(|(k, v)| (k.clone(), json_to_prost_value(v)))
                .collect(),
        })),
    };
    prost_types::Value { kind }
}

/// protobuf Value → 原 JSON 值(过渡期 adapter 内部仍可复用现有 map 逻辑)。
pub fn prost_value_to_json(value: &prost_types::Value) -> serde_json::Value {
    use prost_types::value::Kind;
    match &value.kind {
        None | Some(Kind::NullValue(_)) => serde_json::Value::Null,
        Some(Kind::BoolValue(v)) => serde_json::Value::Bool(*v),
        Some(Kind::NumberValue(v)) => serde_json::Number::from_f64(*v)
            .map_or(serde_json::Value::Null, serde_json::Value::Number),
        Some(Kind::StringValue(v)) => serde_json::Value::String(v.clone()),
        Some(Kind::ListValue(list)) => {
            serde_json::Value::Array(list.values.iter().map(prost_value_to_json).collect())
        }
        Some(Kind::StructValue(s)) => serde_json::Value::Object(
            s.fields
                .iter()
                .map(|(k, v)| (k.clone(), prost_value_to_json(v)))
                .collect(),
        ),
    }
}

/// JSON 对象 → protobuf Struct(非对象返回空 Struct)。
pub fn json_to_prost_struct(value: &serde_json::Value) -> prost_types::Struct {
    let Some(map) = value.as_object() else {
        return prost_types::Struct::default();
    };
    prost_types::Struct {
        fields: map
            .iter()
            .map(|(k, v)| (k.clone(), json_to_prost_value(v)))
            .collect(),
    }
}

/// protobuf Struct → JSON 对象。
pub fn prost_struct_to_json(value: &prost_types::Struct) -> serde_json::Value {
    serde_json::Value::Object(
        value
            .fields
            .iter()
            .map(|(k, v)| (k.clone(), prost_value_to_json(v)))
            .collect(),
    )
}

/// SettingValue ↔ JSON(FRB setSetting 的 protobuf 载荷)。
pub fn decode_setting_value(
    bytes: &[u8],
) -> Result<crate::proto::SettingValue, prost::DecodeError> {
    use prost::Message;
    crate::proto::SettingValue::decode(bytes)
}

pub fn encode_setting_value(value: &crate::proto::SettingValue) -> Vec<u8> {
    use prost::Message;
    value.encode_to_vec()
}

pub fn setting_value_to_json(value: &crate::proto::SettingValue) -> serde_json::Value {
    use crate::proto::setting_value::Value;
    match &value.value {
        None => serde_json::Value::Null,
        Some(Value::BoolValue(v)) => serde_json::Value::Bool(*v),
        Some(Value::IntValue(v)) => serde_json::Value::Number((*v).into()),
        Some(Value::DoubleValue(v)) => serde_json::Number::from_f64(*v)
            .map_or(serde_json::Value::Null, serde_json::Value::Number),
        Some(Value::StringValue(v)) => serde_json::Value::String(v.clone()),
        Some(Value::StructValue(v)) => prost_struct_to_json(v),
        Some(Value::ListValue(v)) => {
            serde_json::Value::Array(v.values.iter().map(setting_value_to_json).collect())
        }
        Some(Value::BytesValue(v)) => serde_json::Value::Array(
            v.iter()
                .map(|b| serde_json::Value::Number((*b as u64).into()))
                .collect(),
        ),
        Some(Value::NullValue(_)) => serde_json::Value::Null,
    }
}

pub fn json_to_setting_value(value: &serde_json::Value) -> crate::proto::SettingValue {
    use crate::proto::setting_value::Value;
    let value = match value {
        serde_json::Value::Null => Some(Value::NullValue(true)),
        serde_json::Value::Bool(v) => Some(Value::BoolValue(*v)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(Value::IntValue(i))
            } else {
                Some(Value::DoubleValue(n.as_f64().unwrap_or(0.0)))
            }
        }
        serde_json::Value::String(v) => Some(Value::StringValue(v.clone())),
        serde_json::Value::Array(items) => Some(Value::ListValue(crate::proto::ListValue {
            values: items.iter().map(json_to_setting_value).collect(),
        })),
        serde_json::Value::Object(map) => Some(Value::StructValue(json_to_prost_struct(
            &serde_json::Value::Object(map.clone()),
        ))),
    };
    crate::proto::SettingValue { value }
}

pub fn encode_string_list(values: &[String]) -> Vec<u8> {
    use prost::Message;
    crate::proto::StringList {
        values: values.to_vec(),
    }
    .encode_to_vec()
}

pub fn decode_string_list(bytes: &[u8]) -> Result<Vec<String>, prost::DecodeError> {
    use prost::Message;
    Ok(crate::proto::StringList::decode(bytes)?.values)
}

pub fn encode_string_map(values: &std::collections::BTreeMap<String, String>) -> Vec<u8> {
    use prost::Message;
    crate::proto::StringMap {
        values: values.clone().into_iter().collect(),
    }
    .encode_to_vec()
}

pub fn decode_string_map(
    bytes: &[u8],
) -> Result<std::collections::BTreeMap<String, String>, prost::DecodeError> {
    use prost::Message;
    let msg = crate::proto::StringMap::decode(bytes)?;
    Ok(msg.values.into_iter().collect())
}

fn settings_bool(v: &serde_json::Value, key: &str) -> bool {
    v.get(key)
        .map(|x| {
            x.as_bool()
                .or_else(|| x.as_i64().map(|n| n != 0))
                .unwrap_or(false)
        })
        .unwrap_or(false)
}
fn settings_i32(v: &serde_json::Value, key: &str) -> i32 {
    v.get(key).and_then(|x| x.as_i64()).unwrap_or(0) as i32
}
fn settings_f64(v: &serde_json::Value, key: &str) -> f64 {
    v.get(key).and_then(|x| x.as_f64()).unwrap_or(0.0)
}
fn settings_string(v: &serde_json::Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string()
}

fn settings_shortcuts(
    v: &serde_json::Value,
    key: &str,
) -> std::collections::HashMap<String, String> {
    match v.get(key) {
        Some(serde_json::Value::String(text)) => {
            serde_json::from_str::<std::collections::HashMap<String, String>>(text)
                .unwrap_or_default()
        }
        Some(serde_json::Value::Object(map)) => map
            .iter()
            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
            .collect(),
        _ => Default::default(),
    }
}

pub fn setting_value_from_json(v: &serde_json::Value) -> proto::SettingValue {
    use proto::setting_value::Value;
    let value = match v {
        serde_json::Value::Bool(b) => Some(Value::BoolValue(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(Value::IntValue(i))
            } else {
                n.as_f64().map(Value::DoubleValue)
            }
        }
        serde_json::Value::String(s) => Some(Value::StringValue(s.clone())),
        _ => None,
    };
    proto::SettingValue { value }
}

fn ui_str(v: &serde_json::Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string()
}

fn ui_opt_str(v: &serde_json::Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string()
}

fn ui_bool(v: &serde_json::Value, key: &str) -> bool {
    v.get(key).and_then(|x| x.as_bool()).unwrap_or(false)
}

fn ui_i32(v: &serde_json::Value, key: &str) -> i32 {
    v.get(key).and_then(|x| x.as_i64()).unwrap_or(0) as i32
}

fn ui_string_map(v: &serde_json::Value, key: &str) -> std::collections::HashMap<String, String> {
    v.get(key)
        .and_then(|x| x.as_object())
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

fn ui_string_list(v: &serde_json::Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn ui_condition(v: Option<&serde_json::Value>) -> Option<proto::UiSettingCondition> {
    let v = v?;
    let key = v.get("key")?.as_str()?.to_string();
    let equals = v.get("equals").map(setting_value_from_json);
    Some(proto::UiSettingCondition { key, equals })
}

fn ui_key_caps(v: &serde_json::Value, key: &str) -> Vec<proto::UiKeyCap> {
    v.get(key)
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .map(|x| proto::UiKeyCap {
                    name: ui_str(x, "name"),
                    label: ui_str(x, "label"),
                    width: x.get("width").and_then(|n| n.as_f64()).unwrap_or(1.0),
                    modifier: ui_bool(x, "modifier"),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn ui_form(v: &serde_json::Value) -> Option<proto::UiFormMeta> {
    let form = v.get("form")?;
    let options = form
        .get("options")
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .map(|x| proto::UiFormOption {
                    value: x.get("value").map(setting_value_from_json),
                    label_id: ui_str(x, "labelId"),
                })
                .collect()
        })
        .unwrap_or_default();
    let range = form.get("range").map(|r| proto::UiFormRange {
        min: r.get("min").and_then(|n| n.as_f64()).unwrap_or(0.0),
        max: r.get("max").and_then(|n| n.as_f64()).unwrap_or(0.0),
        step: r.get("step").and_then(|n| n.as_f64()).unwrap_or(0.0),
    });
    Some(proto::UiFormMeta {
        key: ui_str(form, "key"),
        section: ui_str(form, "section"),
        order: ui_i32(form, "order"),
        label_id: ui_str(form, "labelId"),
        description_id: ui_opt_str(form, "descriptionId"),
        hint_id: ui_opt_str(form, "hintId"),
        control: ui_str(form, "control"),
        layout: ui_str(form, "layout"),
        show_label: form
            .get("showLabel")
            .and_then(|x| x.as_bool())
            .unwrap_or(true),
        indented: ui_bool(form, "indented"),
        divider_before: ui_bool(form, "dividerBefore"),
        unit: ui_opt_str(form, "unit"),
        range,
        options,
        visible_when_feature: ui_opt_str(form, "visibleWhenFeature"),
        visible_when_setting: ui_condition(form.get("visibleWhenSetting")),
        enabled_when_setting: ui_condition(form.get("enabledWhenSetting")),
        advanced: ui_bool(form, "advanced"),
    })
}

pub fn ui_config_from_json(text: &str) -> Option<proto::UiConfig> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let messages = v
        .get("messages")
        .and_then(|x| x.as_object())
        .map(|m| {
            m.iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        proto::LocalizedText {
                            zh: ui_str(v, "zh"),
                            en: ui_str(v, "en"),
                        },
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let modifier_row = v.get("modifierRow").map(|r| proto::UiModifierRow {
        left: ui_key_caps(r, "left"),
        space: ui_key_caps(r, "space"),
        right: ui_key_caps(r, "right"),
    });
    let settings = v
        .get("settings")
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .map(|x| proto::UiSettingSpec {
                    key: ui_str(x, "key"),
                    db_key: ui_str(x, "dbKey"),
                    kind: ui_str(x, "kind"),
                    mac: ui_bool(x, "mac"),
                    win: ui_bool(x, "win"),
                    form: ui_form(x),
                })
                .collect()
        })
        .unwrap_or_default();
    let sections = v
        .get("settingsSections")
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .map(|x| proto::UiFormSection {
                    id: ui_str(x, "id"),
                    label_id: ui_str(x, "labelId"),
                    advanced: ui_bool(x, "advanced"),
                    order: ui_i32(x, "order"),
                    visible_when_setting: ui_condition(x.get("visibleWhenSetting")),
                })
                .collect()
        })
        .unwrap_or_default();
    let blocks = v
        .get("settingsBlocks")
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .map(|x| proto::UiCustomBlock {
                    id: ui_str(x, "id"),
                    section: ui_str(x, "section"),
                    order: ui_i32(x, "order"),
                    visible_when_setting: ui_condition(x.get("visibleWhenSetting")),
                })
                .collect()
        })
        .unwrap_or_default();
    let features = v
        .get("features")
        .and_then(|x| x.as_object())
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_bool().map(|b| (k.clone(), b)))
                .collect()
        })
        .unwrap_or_default();
    Some(proto::UiConfig {
        platform: ui_str(&v, "platform"),
        is_windows: ui_bool(&v, "isWindows"),
        font_scale: v.get("fontScale").and_then(|x| x.as_f64()).unwrap_or(1.0),
        chord_separator: ui_str(&v, "chordSeparator"),
        chord_names: ui_string_map(&v, "chordNames"),
        modifier_word: ui_str(&v, "modifierWord"),
        messages,
        defaults: ui_string_map(&v, "defaults"),
        hold_hotkeys: ui_string_map(&v, "holdHotkeys"),
        reserved: ui_string_list(&v, "reserved"),
        force_bound: ui_string_list(&v, "forceBound"),
        action_order: ui_string_list(&v, "actionOrder"),
        modifier_row,
        features,
        settings,
        settings_sections: sections,
        settings_blocks: blocks,
        protocol_version: v
            .get("protocolVersion")
            .and_then(|x| x.as_u64())
            .unwrap_or(0) as u32,
    })
}

pub fn encode_settings_patch_frame(
    values: &std::collections::HashMap<String, proto::SettingValue>,
) -> Vec<u8> {
    let frame = Frame {
        protocol_version: PROTOCOL_VERSION,
        request_id: 0,
        body: Some(proto::frame::Body::SettingsPatch(proto::SettingsPatch {
            values: values.clone(),
        })),
    };
    encode_frame(&frame)
}

pub fn encode_error_frame(request_id: u64, code: &str, message: &str) -> Vec<u8> {
    let frame = Frame {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        body: Some(proto::frame::Body::Error(proto::Error {
            code: code.to_string(),
            message: message.to_string(),
        })),
    };
    encode_frame(&frame)
}

pub fn encode_ui_config_frame(request_id: u64, text: &str) -> Vec<u8> {
    let config = ui_config_from_json(text).unwrap_or_default();
    let frame = Frame {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        body: Some(proto::frame::Body::UiConfigResponse(
            proto::UiConfigResponse {
                config: Some(config),
            },
        )),
    };
    encode_frame(&frame)
}

pub fn app_settings_from_json(v: &serde_json::Value) -> crate::proto::AppSettings {
    crate::proto::AppSettings {
        color: settings_i32(v, "color"),
        width: settings_i32(v, "width"),
        launch_at_login: settings_bool(v, "launchAtLogin"),
        frosted_glass: settings_bool(v, "frostedGlass"),
        grid: settings_bool(v, "grid"),
        grid_follow_strokes: settings_bool(v, "gridFollowStrokes"),
        glass_follow_strokes: settings_bool(v, "glassFollowStrokes"),
        soft_shadow: settings_bool(v, "softShadow"),
        invert_ink: settings_bool(v, "invertInk"),
        notebook_style: settings_bool(v, "notebookStyle"),
        panel_transparent: settings_bool(v, "panelTransparent"),
        invert_fps: settings_i32(v, "invertFps"),
        pressure_monitor: settings_bool(v, "pressureMonitor"),
        outline: settings_bool(v, "outline"),
        infinite_canvas: settings_bool(v, "infiniteCanvas"),
        minimap: settings_bool(v, "minimap"),
        grid_size: settings_f64(v, "gridSize"),
        grid_divider: settings_i32(v, "gridDivider"),
        flip_effect: settings_i32(v, "flipEffect"),
        gif_fps: settings_i32(v, "gifFps"),
        gif_resolution: settings_f64(v, "gifResolution"),
        gif_speed: settings_f64(v, "gifSpeed"),
        gif_end_mode: settings_i32(v, "gifEndMode"),
        chat_api_base: settings_string(v, "chatApiBase"),
        chat_user: settings_string(v, "chatUser"),
        chat_has_password: settings_bool(v, "chatHasPassword"),
        chat_integration: settings_bool(v, "chatIntegration"),
        show_free_canvas: settings_bool(v, "showFreeCanvas"),
        show_bubble_tab: settings_bool(v, "showBubbleTab"),
        bubble_replay_speed: settings_i32(v, "bubbleReplaySpeed"),
        bubble_replay_hold_last: settings_bool(v, "bubbleReplayHoldLast"),
        bubble_replay_auto: settings_bool(v, "bubbleReplayAuto"),
        bubble_auto_new_page: settings_bool(v, "bubbleAutoNewPage"),
        nas_url: settings_string(v, "nasUrl"),
        share_canvas: settings_bool(v, "shareCanvas"),
        language: settings_string(v, "language"),
        shortcuts: settings_shortcuts(v, "shortcuts"),
    }
}

pub fn app_settings_to_json(s: &crate::proto::AppSettings) -> serde_json::Value {
    serde_json::json!({
        "color": s.color,
        "width": s.width,
        "launchAtLogin": s.launch_at_login,
        "frostedGlass": s.frosted_glass,
        "grid": s.grid,
        "gridFollowStrokes": s.grid_follow_strokes,
        "glassFollowStrokes": s.glass_follow_strokes,
        "softShadow": s.soft_shadow,
        "invertInk": s.invert_ink,
        "notebookStyle": s.notebook_style,
        "panelTransparent": s.panel_transparent,
        "invertFps": s.invert_fps,
        "pressureMonitor": s.pressure_monitor,
        "outline": s.outline,
        "infiniteCanvas": s.infinite_canvas,
        "minimap": s.minimap,
        "gridSize": s.grid_size,
        "gridDivider": s.grid_divider,
        "flipEffect": s.flip_effect,
        "gifFps": s.gif_fps,
        "gifResolution": s.gif_resolution,
        "gifSpeed": s.gif_speed,
        "gifEndMode": s.gif_end_mode,
        "chatApiBase": s.chat_api_base,
        "chatUser": s.chat_user,
        "chatHasPassword": s.chat_has_password,
        "chatIntegration": s.chat_integration,
        "showFreeCanvas": s.show_free_canvas,
        "showBubbleTab": s.show_bubble_tab,
        "bubbleReplaySpeed": s.bubble_replay_speed,
        "bubbleReplayHoldLast": s.bubble_replay_hold_last,
        "bubbleReplayAuto": s.bubble_replay_auto,
        "bubbleAutoNewPage": s.bubble_auto_new_page,
        "nasUrl": s.nas_url,
        "shareCanvas": s.share_canvas,
        "language": s.language,
        "shortcuts": serde_json::to_string(&s.shortcuts).unwrap_or_else(|_| "{}".to_string()),
    })
}

pub fn encode_settings_frame(request_id: u64, json: &str) -> Vec<u8> {
    let value: serde_json::Value = serde_json::from_str(json).unwrap_or(serde_json::Value::Null);
    let settings = app_settings_from_json(&value);
    let frame = Frame {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        body: Some(proto::frame::Body::SettingsSnapshotResponse(
            proto::SettingsSnapshotResponse {
                snapshot: Some(proto::SettingsSnapshot {
                    settings: Some(settings),
                    metadata: Default::default(),
                }),
            },
        )),
    };
    encode_frame(&frame)
}

pub fn encode_settings_changed_frame(json: &str) -> Vec<u8> {
    let value: serde_json::Value = serde_json::from_str(json).unwrap_or(serde_json::Value::Null);
    let settings = app_settings_from_json(&value);
    let frame = Frame {
        protocol_version: PROTOCOL_VERSION,
        request_id: 0,
        body: Some(proto::frame::Body::SettingsChanged(
            proto::SettingsChanged {
                snapshot: Some(proto::SettingsSnapshot {
                    settings: Some(settings),
                    metadata: Default::default(),
                }),
            },
        )),
    };
    encode_frame(&frame)
}

pub fn encode_page_list_frame(request_id: u64, pages: &[(i64, i32, i32)]) -> Vec<u8> {
    let frame = Frame {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        body: Some(proto::frame::Body::PageListResponse(
            proto::PageListResponse {
                pages: pages
                    .iter()
                    .map(|&(id, width, height)| proto::PageSummary {
                        id,
                        width,
                        height,
                        stroke_count: 0,
                    })
                    .collect(),
            },
        )),
    };
    encode_frame(&frame)
}

pub fn encode_page_thumbnails_frame(request_id: u64, entries: &[(i64, Vec<u8>)]) -> Vec<u8> {
    let frame = Frame {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        body: Some(proto::frame::Body::PageThumbnailsResponse(
            proto::PageThumbnailsResponse {
                thumbnails: entries
                    .iter()
                    .map(|(id, png)| proto::PageThumbnail {
                        id: *id,
                        png: png.clone(),
                    })
                    .collect(),
            },
        )),
    };
    encode_frame(&frame)
}

pub fn encode_bubble_messages_frame(request_id: u64, messages: proto::BubbleMessages) -> Vec<u8> {
    let frame = Frame {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        body: Some(proto::frame::Body::BubbleMessages(messages)),
    };
    encode_frame(&frame)
}

pub enum PageOpValue {
    Ok(bool),
    Ids(Vec<i64>),
    Payload(String),
    Count(i32),
}

pub fn encode_download_progress_frame(
    request_id: u64,
    received: u64,
    total: u64,
    done: bool,
    error: &str,
    path: &str,
) -> Vec<u8> {
    let frame = Frame {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        body: Some(proto::frame::Body::DownloadProgress(
            proto::DownloadProgress {
                received,
                total,
                done,
                error: error.to_string(),
                path: path.to_string(),
            },
        )),
    };
    encode_frame(&frame)
}

pub fn encode_apply_update_frame(request_id: u64, ok: bool, message: &str) -> Vec<u8> {
    let frame = Frame {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        body: Some(proto::frame::Body::ApplyUpdateOutcome(
            proto::ApplyUpdateOutcome {
                ok,
                message: message.to_string(),
            },
        )),
    };
    encode_frame(&frame)
}

pub fn encode_nas_backup_frame(
    request_id: u64,
    ok: bool,
    size_bytes: u64,
    stroke_count: u64,
    error: &str,
    hint: &str,
) -> Vec<u8> {
    let frame = Frame {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        body: Some(proto::frame::Body::NasBackupOutcome(
            proto::NasBackupOutcome {
                ok,
                size_bytes,
                stroke_count,
                error: error.to_string(),
                hint: hint.to_string(),
            },
        )),
    };
    encode_frame(&frame)
}

pub fn encode_update_info_frame(
    request_id: u64,
    ok: bool,
    current: &str,
    latest: &str,
    has_update: bool,
    url: &str,
    notes: &str,
    error: &str,
) -> Vec<u8> {
    let frame = Frame {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        body: Some(proto::frame::Body::UpdateInfo(proto::UpdateInfo {
            ok,
            current: current.to_string(),
            latest: latest.to_string(),
            has_update,
            url: url.to_string(),
            notes: notes.to_string(),
            error: error.to_string(),
        })),
    };
    encode_frame(&frame)
}

pub fn encode_backup_outcome_frame(
    request_id: u64,
    kind: &str,
    ok: bool,
    message: &str,
) -> Vec<u8> {
    let frame = Frame {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        body: Some(proto::frame::Body::BackupOutcome(proto::BackupOutcome {
            ok,
            message: message.to_string(),
            kind: kind.to_string(),
        })),
    };
    encode_frame(&frame)
}

pub fn encode_chat_login_outcome_frame(request_id: u64, ok: bool, message: &str) -> Vec<u8> {
    let frame = Frame {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        body: Some(proto::frame::Body::ChatLoginOutcome(
            proto::ChatLoginOutcome {
                ok,
                message: message.to_string(),
                kind: "testChatLogin_response".to_string(),
            },
        )),
    };
    encode_frame(&frame)
}

pub fn encode_canvas_overview_frame(request_id: u64, png: &[u8], rect: &[f64]) -> Vec<u8> {
    let frame = Frame {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        body: Some(proto::frame::Body::CanvasOverviewResponse(
            proto::CanvasOverviewResponse {
                png: png.to_vec(),
                rect: rect.to_vec(),
            },
        )),
    };
    encode_frame(&frame)
}

pub fn encode_app_version_frame(request_id: u64, version: &str) -> Vec<u8> {
    let frame = Frame {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        body: Some(proto::frame::Body::AppVersionResponse(
            proto::AppVersionResponse {
                version: version.to_string(),
            },
        )),
    };
    encode_frame(&frame)
}

pub fn decode_page_op_frame(bytes: &[u8]) -> Result<(String, PageOpValue), FrameError> {
    let frame = decode_frame(bytes)?;
    let Some(proto::frame::Body::PageOpResponse(r)) = frame.body else {
        return Err(FrameError::Decode("期望 PageOpResponse".to_string()));
    };
    let value = match r.result {
        Some(proto::page_op_response::Result::Ok(ok)) => PageOpValue::Ok(ok),
        Some(proto::page_op_response::Result::Ids(ids)) => PageOpValue::Ids(ids.values),
        Some(proto::page_op_response::Result::Payload(p)) => PageOpValue::Payload(p),
        Some(proto::page_op_response::Result::Count(c)) => PageOpValue::Count(c),
        None => PageOpValue::Ok(false),
    };
    Ok((r.kind, value))
}

pub fn encode_page_op_frame(request_id: u64, kind: &str, value: PageOpValue) -> Vec<u8> {
    let result = match value {
        PageOpValue::Ok(ok) => proto::page_op_response::Result::Ok(ok),
        PageOpValue::Ids(ids) => {
            proto::page_op_response::Result::Ids(proto::Int64List { values: ids })
        }
        PageOpValue::Payload(payload) => proto::page_op_response::Result::Payload(payload),
        PageOpValue::Count(count) => proto::page_op_response::Result::Count(count),
    };
    let frame = Frame {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        body: Some(proto::frame::Body::PageOpResponse(proto::PageOpResponse {
            kind: kind.to_string(),
            result: Some(result),
        })),
    };
    encode_frame(&frame)
}

pub fn decode_bubble_messages_frame(bytes: &[u8]) -> Result<proto::BubbleMessages, FrameError> {
    let frame = decode_frame(bytes)?;
    let Some(proto::frame::Body::BubbleMessages(messages)) = frame.body else {
        return Err(FrameError::Decode("期望 BubbleMessages".to_string()));
    };
    Ok(messages)
}

pub fn decode_page_thumbnails_frame(bytes: &[u8]) -> Result<Vec<(i64, Vec<u8>)>, FrameError> {
    let frame = decode_frame(bytes)?;
    let Some(proto::frame::Body::PageThumbnailsResponse(resp)) = frame.body else {
        return Err(FrameError::Decode(
            "期望 PageThumbnailsResponse".to_string(),
        ));
    };
    Ok(resp.thumbnails.into_iter().map(|t| (t.id, t.png)).collect())
}

pub fn decode_page_list_frame(bytes: &[u8]) -> Result<Vec<(i64, i32, i32)>, FrameError> {
    let frame = decode_frame(bytes)?;
    let Some(proto::frame::Body::PageListResponse(resp)) = frame.body else {
        return Err(FrameError::Decode("期望 PageListResponse".to_string()));
    };
    Ok(resp
        .pages
        .into_iter()
        .map(|p| (p.id, p.width, p.height))
        .collect())
}

pub fn decode_settings_frame(bytes: &[u8]) -> Result<serde_json::Value, FrameError> {
    let frame = decode_frame(bytes)?;
    let snapshot = match frame.body {
        Some(proto::frame::Body::SettingsSnapshotResponse(resp)) => resp.snapshot,
        Some(proto::frame::Body::SettingsSnapshot(snapshot)) => Some(snapshot),
        _ => None,
    };
    let Some(snapshot) = snapshot else {
        return Err(FrameError::Decode(
            "期望 SettingsSnapshotResponse".to_string(),
        ));
    };
    let Some(settings) = snapshot.settings else {
        return Err(FrameError::Decode(
            "SettingsSnapshot 缺少 settings".to_string(),
        ));
    };
    Ok(app_settings_to_json(&settings))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::Ack;

    fn sample() -> Frame {
        Frame {
            protocol_version: PROTOCOL_VERSION,
            request_id: 42,
            body: Some(proto::frame::Body::SettingsSnapshot(SettingsSnapshot {
                settings: Some(crate::proto::AppSettings::default()),
                metadata: Default::default(),
            })),
        }
    }

    #[test]
    fn roundtrip_frame() {
        let frame = sample();
        let bytes = encode_frame(&frame);
        let decoded = decode_frame(&bytes).expect("decode");
        assert_eq!(decoded, frame);
    }

    #[test]
    fn take_frame_handles_partial_and_extra() {
        let bytes = encode_frame(&sample());
        assert!(take_frame(&bytes[..2]).unwrap().is_none());
        assert!(take_frame(&bytes[..bytes.len() - 1]).unwrap().is_none());
        let (frame, used) = take_frame(&bytes).unwrap().unwrap();
        assert_eq!(used, bytes.len());
        assert_eq!(frame.request_id, 42);
    }

    #[test]
    fn rejects_bad_length_prefix() {
        let bytes = encode_frame(&sample());
        let short = &bytes[..bytes.len() - 1];
        assert!(matches!(
            decode_frame(short),
            Err(FrameError::LengthMismatch { .. })
        ));
        let mut huge = 0u32.to_le_bytes().to_vec();
        huge[..4].copy_from_slice(&(MAX_FRAME_BYTES as u32 + 1).to_le_bytes());
        assert!(matches!(
            decode_frame(&huge),
            Err(FrameError::TooLarge { .. })
        ));
    }

    #[test]
    fn typed_page_op_roundtrip() {
        let bytes = encode_page_op_frame(1, "lassoSelect_response", PageOpValue::Ids(vec![3, 4]));
        let (kind, value) = decode_page_op_frame(&bytes).unwrap();
        assert_eq!(kind, "lassoSelect_response");
        assert!(matches!(value, PageOpValue::Ids(ids) if ids == vec![3, 4]));
    }

    #[test]
    fn typed_bubble_messages_roundtrip() {
        let messages = proto::BubbleMessages {
            device: "dev".to_string(),
            messages: vec![proto::BubbleMessage {
                id: 1,
                ts_ms: 2,
                device: "dev".to_string(),
                strokes: vec![proto::BubbleStroke {
                    rgb: 0xff0000,
                    width_scale: 2.0,
                    gap_ms: 0,
                    points: vec![proto::BubblePoint {
                        x: 1.0,
                        y: 2.0,
                        width: 3.0,
                    }],
                }],
            }],
        };
        let bytes = encode_bubble_messages_frame(5, messages.clone());
        assert_eq!(decode_bubble_messages_frame(&bytes).unwrap(), messages);
    }

    #[test]
    fn typed_page_thumbnails_roundtrip() {
        let entries = vec![(1i64, vec![1u8, 2, 3]), (2i64, vec![9u8])];
        let bytes = encode_page_thumbnails_frame(8, &entries);
        assert_eq!(decode_page_thumbnails_frame(&bytes).unwrap(), entries);
    }

    #[test]
    fn typed_page_list_roundtrip() {
        let bytes = encode_page_list_frame(7, &[(1, 1920, 1080), (2, 1280, 720)]);
        let pages = decode_page_list_frame(&bytes).unwrap();
        assert_eq!(pages, vec![(1, 1920, 1080), (2, 1280, 720)]);
    }

    #[test]
    fn typed_settings_frame_roundtrip() {
        let bytes = encode_settings_frame(7, r#"{"grid":true,"gridSize":80,"language":"zh"}"#);
        let value = decode_settings_frame(&bytes).unwrap();
        assert_eq!(value["grid"], true);
        assert_eq!(value["gridSize"], 80.0);
        assert_eq!(value["language"], "zh");
    }

    #[test]
    fn setting_value_json_roundtrip() {
        for value in [
            serde_json::json!(true),
            serde_json::json!(false),
            serde_json::json!(25),
            serde_json::json!(3.5),
            serde_json::json!("zh"),
            serde_json::json!([1, 2, 3]),
            serde_json::json!({"theme": "light", "scale": 2.0}),
        ] {
            let setting = json_to_setting_value(&value);
            assert_eq!(setting_value_to_json(&setting), value);
        }
    }

    #[test]
    fn ack_roundtrip() {
        let frame = Frame {
            protocol_version: PROTOCOL_VERSION,
            request_id: 1,
            body: Some(proto::frame::Body::Ack(Ack { ok: true })),
        };
        let bytes = encode_frame(&frame);
        let decoded = decode_frame(&bytes).unwrap();
        assert!(matches!(
            decoded.body,
            Some(proto::frame::Body::Ack(Ack { ok: true }))
        ));
    }
}
