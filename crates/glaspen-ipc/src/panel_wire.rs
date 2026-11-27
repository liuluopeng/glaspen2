// Windows 面板管道的双协议分帧/转换层(纯逻辑, 可在任意平台单测)。
//
// 线格式:
//   - 旧客户端: 换行分隔 JSON(保持兼容);
//   - 新客户端: 先发 PB_MAGIC, 之后每帧 = u32 LE 长度 + Frame protobuf。
//
// 服务端把两种输入统一转成原 JSON request 字符串交给现有 process_pipe_message,
// 响应再按连接协议编码: JSON 客户端继续收 JSON 行, protobuf 客户端收 binary 帧。
// 这样业务 handler 不需要一次性重写。

use crate::{Frame, FrameError, PROTOCOL_VERSION, encode_frame, panel_frame, take_frame};

/// 新 protobuf 客户端在连接开头发的魔数; 旧 JSON 客户端不会发, 天然区分。
pub const PB_MAGIC: &[u8; 4] = b"GPB1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireProtocol {
    Json,
    Protobuf,
}

pub struct PanelDecoder {
    buf: Vec<u8>,
    protocol: Option<WireProtocol>,
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
            protocol: None,
        }
    }

    pub fn protocol(&self) -> Option<WireProtocol> {
        self.protocol
    }

    /// 喂入原始字节, 返回已经切出的 JSON request 行(仍由旧 handler 解析)。
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<String>, FrameError> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        loop {
            if self.protocol.is_none() {
                if self.buf.starts_with(PB_MAGIC) {
                    self.buf.drain(..PB_MAGIC.len());
                    self.protocol = Some(WireProtocol::Protobuf);
                } else if self.buf.first() == Some(&b'{') {
                    self.protocol = Some(WireProtocol::Json);
                } else if self.buf.len() < PB_MAGIC.len() && PB_MAGIC.starts_with(&self.buf[..]) {
                    break; // 魔数还没收全
                } else if !self.buf.is_empty() {
                    // 不是魔数也不像 JSON: 按旧协议容错, 让 handler 报错。
                    self.protocol = Some(WireProtocol::Json);
                } else {
                    break;
                }
            }

            match self.protocol {
                Some(WireProtocol::Json) => {
                    let Some(pos) = self.buf.iter().position(|b| *b == b'\n') else {
                        break;
                    };
                    let line = self.buf[..pos].to_vec();
                    self.buf.drain(..=pos);
                    let text = String::from_utf8_lossy(&line).trim().to_string();
                    if !text.is_empty() {
                        out.push(text);
                    }
                }
                Some(WireProtocol::Protobuf) => {
                    let Some((frame, used)) = take_frame(&self.buf)? else {
                        break;
                    };
                    self.buf.drain(..used);
                    out.push(panel_request_from_frame(frame)?);
                }
                None => break,
            }
        }
        Ok(out)
    }
}

fn panel_request_from_frame(frame: Frame) -> Result<String, FrameError> {
    if frame.protocol_version != 0 && frame.protocol_version != PROTOCOL_VERSION {
        return Err(FrameError::Decode(format!(
            "不兼容的协议版本 {}",
            frame.protocol_version
        )));
    }
    let Some(crate::proto::frame::Body::Panel(panel)) = frame.body else {
        return Err(FrameError::Decode("面板管道只接受 Frame.panel".to_string()));
    };
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
    Ok(serde_json::Value::Object(object).to_string())
}

/// 把业务层的 JSON 响应行编码成连接协议对应的字节。
pub fn encode_response(protocol: WireProtocol, json_line: &str) -> Vec<u8> {
    match protocol {
        WireProtocol::Json => {
            let mut out = json_line.as_bytes().to_vec();
            if !out.ends_with(b"\n") {
                out.push(b'\n');
            }
            out
        }
        WireProtocol::Protobuf => {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pb(text: &str) -> Vec<u8> {
        let frame = panel_frame(
            7,
            "getSettings",
            &serde_json::json!({"screenId": 3.0, "ok": true}),
        );
        let _ = text;
        let mut out = PB_MAGIC.to_vec();
        out.extend_from_slice(&encode_frame(&frame));
        out
    }

    #[test]
    fn json_mode_stays_compatible() {
        let mut decoder = PanelDecoder::new();
        let lines = decoder
            .feed(b"{\"type\":\"getSettings\",\"reqId\":1}\n")
            .unwrap();
        assert_eq!(decoder.protocol(), Some(WireProtocol::Json));
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("getSettings"));
        let out = encode_response(WireProtocol::Json, &lines[0]);
        assert!(out.ends_with(b"\n"));
    }

    #[test]
    fn protobuf_magic_and_partial_frame() {
        let bytes = pb("x");
        let split = PB_MAGIC.len() + 2;
        let mut decoder = PanelDecoder::new();
        assert!(decoder.feed(&bytes[..split]).unwrap().is_empty());
        let lines = decoder.feed(&bytes[split..]).unwrap();
        assert_eq!(decoder.protocol(), Some(WireProtocol::Protobuf));
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("\"type\":\"getSettings\""));
        assert!(lines[0].contains("\"screenId\":3.0"));
    }

    #[test]
    fn protobuf_response_roundtrip() {
        let bytes = encode_response(
            WireProtocol::Protobuf,
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
