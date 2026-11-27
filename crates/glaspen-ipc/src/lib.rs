// glaspen2 跨语言/跨进程 protobuf 契约与帧编解码。
//
// 所有跨 machine boundary 的消息都走这里生成的类型; 线上格式是
// u32 little-endian 长度 + Frame protobuf bytes, 不再传 JSON 文本。
//
// 同进程 Rust <-> Rust(实时绘制计划/录制状态机)仍直接使用 typed Rust API,
// 不经 protobuf —— 那不是机器通信, 序列化只会增加开销。

#![allow(clippy::doc_markdown)]

use prost::Message;

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

/// 过渡 panel 消息: kind + 原 data 对象 + request_id。
pub fn panel_frame(request_id: u64, kind: &str, data: &serde_json::Value) -> Frame {
    Frame {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        body: Some(proto::frame::Body::Panel(Panel {
            kind: kind.to_string(),
            data: Some(json_to_prost_struct(data)),
            ok: true,
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{Ack, SettingValue, setting_value};
    use prost_types::{Struct, Value};

    fn sample() -> Frame {
        let mut values = std::collections::HashMap::new();
        values.insert(
            "language".to_string(),
            SettingValue {
                value: Some(setting_value::Value::StringValue("zh".to_string())),
            },
        );
        values.insert(
            "ui".to_string(),
            SettingValue {
                value: Some(setting_value::Value::StructValue(Struct {
                    fields: [(
                        "theme".to_string(),
                        Value {
                            kind: Some(prost_types::value::Kind::StringValue("light".to_string())),
                        },
                    )]
                    .into_iter()
                    .collect(),
                })),
            },
        );
        Frame {
            protocol_version: PROTOCOL_VERSION,
            request_id: 42,
            body: Some(proto::frame::Body::SettingsSnapshot(SettingsSnapshot {
                values,
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
    fn panel_struct_roundtrip() {
        let value = serde_json::json!({
            "screenId": 7.0,
            "ok": true,
            "items": [1.0, 2.0, 3.0],
            "nested": {"label": "页"}
        });
        let frame = panel_frame(9, "getPageThumbnails", &value);
        let bytes = encode_frame(&frame);
        let decoded = decode_frame(&bytes).unwrap();
        let Some(proto::frame::Body::Panel(panel)) = decoded.body else {
            panic!("expected panel frame");
        };
        assert_eq!(panel.kind, "getPageThumbnails");
        let back = prost_struct_to_json(panel.data.as_ref().unwrap());
        assert_eq!(back, value);
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
