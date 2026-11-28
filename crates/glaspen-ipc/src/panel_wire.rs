// Windows 面板管道的 protobuf 分帧/转换层(纯逻辑, 可在任意平台单测)。
//
// 线格式: 连接开头必须发 PB_MAGIC, 之后每帧 = u32 LE 长度 + Frame protobuf。
// 不再支持 JSON 行协议。

use crate::{Frame, FrameError, PROTOCOL_VERSION, encode_frame, panel_frame, take_frame};

/// protobuf 客户端在连接开头发的魔数。
pub const PB_MAGIC: &[u8; 4] = b"GPB1";

pub struct PanelDecoder {
    buf: Vec<u8>,
    started: bool,
}

impl Default for PanelDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl PanelDecoder {
    pub fn new() -> Self {
        Self {
            buf: Vec::new(),
            started: false,
        }
    }

    /// 喂入原始字节, 返回已切出的 typed/JSON 请求。
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<PanelInput>, FrameError> {
        self.buf.extend_from_slice(bytes);
        if !self.started {
            if self.buf.starts_with(PB_MAGIC) {
                self.buf.drain(..PB_MAGIC.len());
                self.started = true;
            } else if self.buf.len() < PB_MAGIC.len() && PB_MAGIC.starts_with(&self.buf[..]) {
                return Ok(Vec::new());
            } else {
                return Err(FrameError::Decode(
                    "面板管道首帧不是 GPB1 protobuf 魔数".to_string(),
                ));
            }
        }

        let mut out = Vec::new();
        while let Some((frame, used)) = take_frame(&self.buf)? {
            self.buf.drain(..used);
            out.push(panel_input_from_frame(frame)?);
        }
        Ok(out)
    }
}

pub enum PanelInput {
    Json(String),
    GetSettings {
        request_id: u64,
    },
    SetSetting {
        request_id: u64,
        key: String,
        value: serde_json::Value,
    },
    ListPages {
        request_id: u64,
    },
    PageThumbnails {
        request_id: u64,
        ids: Vec<i64>,
        max_size: i32,
    },
    PageOp {
        request_id: u64,
        request: crate::proto::PageOpRequest,
    },
    GetBubbles {
        request_id: u64,
    },
    NavigateToPage {
        screen_id: i64,
    },
    Hotkey {
        key: String,
    },
    OpenUrl {
        url: String,
    },
    FlushBubbles {
        request_id: u64,
    },
    AppVersion {
        request_id: u64,
    },
    BackupCommand {
        request_id: u64,
        command: crate::proto::BackupCommand,
    },
    ChatLogin {
        request_id: u64,
    },
    ExportPdf {
        request_id: u64,
    },
    CanvasOverview {
        request_id: u64,
        w: i32,
        h: i32,
        action: crate::proto::CanvasAction,
    },
}

fn panel_input_from_frame(frame: Frame) -> Result<PanelInput, FrameError> {
    if frame.protocol_version != 0 && frame.protocol_version != PROTOCOL_VERSION {
        return Err(FrameError::Decode(format!(
            "不兼容的协议版本 {}",
            frame.protocol_version
        )));
    }
    match frame.body {
        Some(crate::proto::frame::Body::Panel(panel)) => {
            let data = panel
                .data
                .as_ref()
                .map(crate::prost_value_to_json)
                .unwrap_or_else(|| serde_json::json!({}));
            let mut object = data.as_object().cloned().unwrap_or_default();
            object.insert(
                "type".to_string(),
                serde_json::Value::String(panel.kind.clone()),
            );
            object.insert(
                "reqId".to_string(),
                serde_json::Value::Number(serde_json::Number::from(frame.request_id)),
            );
            Ok(PanelInput::Json(
                serde_json::Value::Object(object).to_string(),
            ))
        }
        Some(crate::proto::frame::Body::GetSettingsRequest(_)) => Ok(PanelInput::GetSettings {
            request_id: frame.request_id,
        }),
        Some(crate::proto::frame::Body::SetSetting(req)) => Ok(PanelInput::SetSetting {
            request_id: frame.request_id,
            key: req.key,
            value: req
                .value
                .as_ref()
                .map(crate::setting_value_to_json)
                .unwrap_or(serde_json::Value::Null),
        }),
        Some(crate::proto::frame::Body::ListPagesRequest(_)) => Ok(PanelInput::ListPages {
            request_id: frame.request_id,
        }),
        Some(crate::proto::frame::Body::PageThumbnailsRequest(req)) => {
            Ok(PanelInput::PageThumbnails {
                request_id: frame.request_id,
                ids: req.ids,
                max_size: req.max_size,
            })
        }
        _ => Err(FrameError::Decode("面板管道不支持该 Frame".to_string())),
    }
}

/// 把业务层的 JSON 响应行编码成 Panel protobuf 帧。
pub fn encode_response(json_line: &str) -> Vec<u8> {
    let value: serde_json::Value = match serde_json::from_str(json_line) {
        Ok(value) => value,
        Err(err) => {
            let frame = panel_frame(
                0,
                "error",
                &serde_json::json!({"message": format!("服务端响应 JSON 非法: {err}")}),
            );
            return encode_frame(&frame);
        }
    };
    let kind = value
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("response");
    let req_id = value
        .get("reqId")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let data = value
        .get("data")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    encode_frame(&panel_frame(req_id, kind, &data))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pb_bytes(request_id: u64) -> Vec<u8> {
        let frame = panel_frame(
            request_id,
            "getSettings",
            &serde_json::json!({"screenId": 3.0, "ok": true}),
        );
        let mut out = PB_MAGIC.to_vec();
        out.extend_from_slice(&encode_frame(&frame));
        out
    }

    #[test]
    fn requires_magic() {
        let mut decoder = PanelDecoder::new();
        assert!(decoder.feed(b"{\"type\":\"getSettings\"}\n").is_err());
    }

    #[test]
    fn magic_and_partial_frame() {
        let bytes = pb_bytes(7);
        let split = PB_MAGIC.len() + 2;
        let mut decoder = PanelDecoder::new();
        assert!(decoder.feed(&bytes[..split]).unwrap().is_empty());
        let lines = decoder.feed(&bytes[split..]).unwrap();
        assert_eq!(lines.len(), 1);
        let PanelInput::Json(line) = &lines[0] else {
            panic!("expected json input");
        };
        assert!(line.contains("\"type\":\"getSettings\""));
        assert!(line.contains("\"screenId\":3.0"));
    }

    #[test]
    fn typed_set_setting_decodes() {
        let mut frame = crate::frame(9);
        frame.body = Some(crate::proto::frame::Body::SetSetting(
            crate::proto::SetSetting {
                key: "grid".to_string(),
                value: Some(crate::json_to_setting_value(&serde_json::json!(true))),
            },
        ));
        let mut bytes = PB_MAGIC.to_vec();
        bytes.extend_from_slice(&crate::encode_frame(&frame));
        let inputs = PanelDecoder::new().feed(&bytes).unwrap();
        let PanelInput::SetSetting {
            request_id,
            key,
            value,
        } = &inputs[0]
        else {
            panic!("expected SetSetting");
        };
        assert_eq!(*request_id, 9);
        assert_eq!(key, "grid");
        assert_eq!(value, &serde_json::json!(true));
    }

    #[test]
    fn protobuf_response_roundtrip() {
        let bytes = encode_response(
            r#"{"type":"getSettings_response","reqId":9,"data":{"language":"zh"}}"#,
        );
        let frame = crate::decode_frame(&bytes).unwrap();
        let Some(crate::proto::frame::Body::Panel(panel)) = frame.body else {
            panic!("expected panel");
        };
        assert_eq!(panel.kind, "getSettings_response");
        let data = crate::prost_value_to_json(panel.data.as_ref().unwrap());
        assert_eq!(data["language"], "zh");
    }

    #[test]
    fn bad_version_rejected() {
        let mut frame = panel_frame(1, "x", &serde_json::json!({}));
        frame.protocol_version = 99;
        let mut bytes = PB_MAGIC.to_vec();
        bytes.extend_from_slice(&encode_frame(&frame));
        let mut decoder = PanelDecoder::new();
        assert!(decoder.feed(&bytes).is_err());
    }
}
