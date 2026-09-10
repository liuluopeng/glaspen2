//! glaspen 手写笔记聊天流 — 消息存储在本地 axum 服务(gRPC)。
//!
//! glaspen2 主程序不持有 messages 表:这里只负责把消息发给服务端。
//! 在 axum 服务尚未存在时,可用模拟发送器([`Sink::Mock`])假装地址存在,
//! 打印每条消息的 gRPC 帧摘要。

pub mod pb {
    //! 由 proto/glaspen/chat/v1/chat.proto 生成(build.rs)。
    tonic::include_proto!("glaspen.chat.v1");
}

use std::sync::Mutex;

use pb::ChatMessage;
use pb::chat_store_client::ChatStoreClient;

/// 本地 axum 服务的默认地址;可用环境变量 GLASPEN_CHAT_ENDPOINT 覆盖。
pub const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:50051";

/// 读取服务地址。未设置环境变量时返回默认本地地址。
pub fn endpoint_from_env() -> String {
    std::env::var("GLASPEN_CHAT_ENDPOINT").unwrap_or_else(|_| DEFAULT_ENDPOINT.to_string())
}

// ---------------------------------------------------------------------------
// 发送端:真实 gRPC / 本地模拟
// ---------------------------------------------------------------------------

/// 模拟模式下服务端"分配"的起始 msg_id(纯粹为了演示回执形状)。
const MOCK_FIRST_MSG_ID: u64 = 10001;

/// 消息汇:真实 gRPC 客户端,或假装地址存在的本地模拟器。
pub enum Sink {
    Grpc(ChatStoreClient<tonic::transport::Channel>),
    Mock,
}

/// 一次 Append 的回执(与 proto AppendReply 对齐,模拟模式返回同形状)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendSummary {
    pub first_msg_id: u64,
    pub accepted: u64,
    pub seqs: Vec<u64>,
}

/// 连接发送端。axum 服务尚未就绪期间默认走模拟;
/// 设置 `GLASPEN_CHAT_MOCK=0`(或调用 [`connect_with`])启用真实 gRPC。
pub async fn connect(endpoint: &str) -> Result<Sink, tonic::transport::Error> {
    connect_with(endpoint, mock_enabled()).await
}

/// 显式指定发送模式的连接。
pub async fn connect_with(endpoint: &str, mock: bool) -> Result<Sink, tonic::transport::Error> {
    if mock {
        return Ok(Sink::Mock);
    }
    Ok(Sink::Grpc(
        ChatStoreClient::connect(endpoint.to_owned()).await?,
    ))
}

/// 默认模拟开启;`GLASPEN_CHAT_MOCK=0` 表示走真实服务。
pub fn mock_enabled() -> bool {
    std::env::var("GLASPEN_CHAT_MOCK")
        .map(|v| v != "0")
        .unwrap_or(true)
}

impl Sink {
    /// 追加一批消息(按顺序)。模拟模式下打印逐条摘要并伪造回执。
    pub async fn append(&mut self, msgs: &[ChatMessage]) -> Result<AppendSummary, String> {
        match self {
            Sink::Grpc(client) => {
                // tonic 0.13 的客户端流直接产出消息本体。
                let req = tokio_stream::iter(msgs.to_vec());
                let resp = client
                    .append_messages(req)
                    .await
                    .map_err(|s| format!("gRPC AppendMessages failed: {s}"))?;
                let reply = resp.into_inner();
                Ok(AppendSummary {
                    first_msg_id: reply.first_msg_id,
                    accepted: reply.accepted,
                    seqs: reply.seqs,
                })
            }
            Sink::Mock => {
                eprintln!(
                    "[mock] -> glaspen.chat.v1.ChatStore/AppendMessages  notebook={}  messages={}  bytes={}",
                    msgs.first().map(|m| m.notebook_id.as_str()).unwrap_or("?"),
                    msgs.len(),
                    msgs.iter().map(prost::Message::encoded_len).sum::<usize>(),
                );
                for (i, m) in msgs.iter().enumerate() {
                    eprintln!("[mock]   #{:<2} {}", i + 1, summarize(m));
                }
                let summary = AppendSummary {
                    first_msg_id: MOCK_FIRST_MSG_ID,
                    accepted: msgs.len() as u64,
                    seqs: msgs.iter().map(|m| m.seq).collect(),
                };
                eprintln!(
                    "[mock] <- AppendReply {{ first_msg_id: {}, accepted: {} }}",
                    summary.first_msg_id, summary.accepted
                );
                Ok(summary)
            }
        }
    }
}

/// 单行摘要,模拟发送与日志用。
pub fn summarize(m: &ChatMessage) -> String {
    let body = match &m.payload {
        Some(pb::chat_message::Payload::Stroke(s)) => format!(
            "points={} color=#{:06X} layout={}",
            s.points.len(),
            s.color_rgb,
            layout_desc(m.layout.as_ref()),
        ),
        Some(pb::chat_message::Payload::Text(t)) => format!("{:?}", t.text),
        Some(pb::chat_message::Payload::Image(i)) => {
            format!("media={} {}x{}", i.media_id, i.width, i.height)
        }
        Some(pb::chat_message::Payload::Voice(v)) => {
            format!("media={} {}ms", v.media_id, v.duration_ms)
        }
        Some(pb::chat_message::Payload::Video(v)) => {
            format!("media={} {}ms", v.media_id, v.duration_ms)
        }
        Some(pb::chat_message::Payload::Sticker(s)) => {
            format!("pack={}/{}", s.pack_id, s.sticker_id)
        }
        Some(pb::chat_message::Payload::File(f)) => format!("{} ({}B)", f.file_name, f.size),
        Some(pb::chat_message::Payload::Merged(mn)) => {
            format!("\"{}\" ({} items)", mn.title, mn.items.len())
        }
        Some(pb::chat_message::Payload::Quote(q)) => match &q.target {
            Some(pb::quote_content::Target::QuoteMsgId(id)) => format!("quote msg_id={id}"),
            Some(pb::quote_content::Target::QuoteRegion(r)) => {
                format!("quote page={} strokes={:?}", r.page_seq, r.stroke_seqs)
            }
            None => "quote (empty)".into(),
        },
        Some(pb::chat_message::Payload::System(s)) => format!("{:?}", s.text),
        Some(pb::chat_message::Payload::Tombstone(t)) => {
            format!("undo msg_id={} ({})", t.target_msg_id, t.reason)
        }
        None => "(no payload)".into(),
    };
    format!("seq={:<3} {}/{}  {}", m.seq, m.r#type, m.subtype, body)
}

fn layout_desc(l: Option<&pb::Layout>) -> &'static str {
    match l.and_then(|l| l.kind.as_ref()) {
        Some(pb::layout::Kind::Flow(_)) => "flow",
        Some(pb::layout::Kind::Anchor(_)) => "anchor",
        None => "default",
    }
}

// ---------------------------------------------------------------------------
// 便捷构造(对齐 glaspen2 现有笔迹模型)
// ---------------------------------------------------------------------------

/// 从 glaspen2 风格的笔迹点列 (x, y, width, relative_time) 构造一条
/// STROKE 消息。`anchor = None` 时使用 flow 布局。
#[allow(clippy::too_many_arguments)]
pub fn stroke_message(
    notebook_id: &str,
    seq: u64,
    author: &str,
    device: &str,
    color_rgb: u32,
    width_scale: f64,
    points: &[(f64, f64, f64, f64)],
    anchor: Option<(f64, f64)>,
) -> ChatMessage {
    ChatMessage {
        notebook_id: notebook_id.to_owned(),
        seq,
        client_msg_id: new_client_msg_id(),
        r#type: pb::MsgType::Stroke as i32,
        subtype: 0,
        author: author.to_owned(),
        device: device.to_owned(),
        created_at_ms: now_ms(),
        layout: Some(pb::Layout {
            kind: match anchor {
                Some((x, y)) => Some(pb::layout::Kind::Anchor(pb::layout::Anchor { x, y })),
                None => Some(pb::layout::Kind::Flow(pb::layout::Flow {})),
            },
        }),
        meta: Default::default(),
        payload: Some(pb::chat_message::Payload::Stroke(pb::StrokeContent {
            points: points
                .iter()
                .map(|(x, y, w, t)| pb::StrokePoint {
                    x: *x,
                    y: *y,
                    width: *w,
                    t_rel: *t,
                })
                .collect(),
            color_rgb,
            width_scale,
        })),
    }
}

/// 系统通知(新建画布/翻页,渲染为居中灰字)。
pub fn system_notice(notebook_id: &str, seq: u64, text: &str) -> ChatMessage {
    ChatMessage {
        notebook_id: notebook_id.to_owned(),
        seq,
        client_msg_id: new_client_msg_id(),
        r#type: pb::MsgType::System as i32,
        subtype: 0,
        author: String::new(),
        device: String::new(),
        created_at_ms: now_ms(),
        layout: None,
        meta: Default::default(),
        payload: Some(pb::chat_message::Payload::System(pb::SystemNotice {
            text: text.to_owned(),
            extra: Default::default(),
        })),
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// 客户端幂等键。本地模拟期用计数器 + 时间戳,真实接入时可换 uuid。
fn new_client_msg_id() -> String {
    static COUNTER: Mutex<u64> = Mutex::new(0);
    let mut n = COUNTER.lock().unwrap();
    *n += 1;
    format!("local-{:x}-{:x}", now_ms(), *n)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use pb::chat_message::Payload;
    use pb::{BBox, MsgType, QuoteContent, StrokeContent, SystemNotice, TextContent, Tombstone};
    use prost::Message as _;

    fn sample_stroke() -> ChatMessage {
        stroke_message(
            "nb-1",
            2,
            "llp",
            "mac",
            0xFF0000,
            1.0,
            &[(0.0, 0.0, 2.0, 0.0), (10.0, 10.0, 3.0, 0.05)],
            Some((120.0, 340.0)),
        )
    }

    /// 所有 payload 各构造一条。
    fn all_payload_messages() -> Vec<ChatMessage> {
        // 闭包工厂:每条新消息都要全新的非 Copy 字段(String/meta)。
        let base = || system_notice("nb-1", 1, "x");
        let media = |id: &str| pb::MediaContent {
            media_id: id.to_owned(),
            width: 1920,
            height: 1080,
            duration_ms: 1500,
            thumb_media_id: String::new(),
        };
        vec![
            ChatMessage {
                payload: Some(Payload::Stroke(StrokeContent::default())),
                ..sample_stroke()
            },
            ChatMessage {
                payload: Some(Payload::Text(TextContent {
                    text: "hello".into(),
                })),
                ..base()
            },
            ChatMessage {
                payload: Some(Payload::Image(media("img"))),
                ..base()
            },
            ChatMessage {
                payload: Some(Payload::Voice(media("voice"))),
                ..base()
            },
            ChatMessage {
                payload: Some(Payload::Video(media("video"))),
                ..base()
            },
            ChatMessage {
                payload: Some(Payload::Sticker(pb::StickerContent {
                    pack_id: "p".into(),
                    sticker_id: "s".into(),
                })),
                ..base()
            },
            ChatMessage {
                payload: Some(Payload::File(pb::FileContent {
                    file_name: "a.pdf".into(),
                    size: 9,
                    media_id: "f".into(),
                })),
                ..base()
            },
            ChatMessage {
                payload: Some(Payload::Merged(pb::MergedNotes {
                    title: "merged".into(),
                    items: vec![pb::merged_notes::MergedItem {
                        notebook_id: "nb-0".into(),
                        msg_id: 7,
                        client_msg_id: String::new(),
                    }],
                })),
                ..base()
            },
            ChatMessage {
                payload: Some(Payload::Quote(Box::new(QuoteContent {
                    target: Some(pb::quote_content::Target::QuoteRegion(
                        pb::quote_content::StrokeRegion {
                            page_seq: 1,
                            stroke_seqs: vec![2],
                            bbox: Some(BBox {
                                x: 0.0,
                                y: 0.0,
                                w: 3.0,
                                h: 4.0,
                            }),
                        },
                    )),
                    // 递归消息:oneof 变体与 reply 字段在生成代码中均为 Box。
                    reply: Some(Box::new(system_notice("nb-1", 9, "reply"))),
                }))),
                ..base()
            },
            ChatMessage {
                payload: Some(Payload::Tombstone(Tombstone {
                    target_msg_id: 42,
                    reason: "undo".into(),
                })),
                ..base()
            },
            // 无 payload:只改正文的信封也应可编码(渲染占位)。
            ChatMessage {
                payload: None,
                ..base()
            },
        ]
    }

    /// 所有 payload 走一遍 prost 编码往返,字段必须无损。
    #[test]
    fn roundtrip_all_payloads() {
        for m in all_payload_messages() {
            let bytes = m.encode_to_vec();
            let back = ChatMessage::decode(&bytes[..]).unwrap();
            assert_eq!(back, m);
        }
    }

    /// glaspen2 点列 (x, y, w, t) 映射到 StrokePoint 的顺序必须一致。
    #[test]
    fn stroke_point_mapping() {
        let m = sample_stroke();
        let Some(Payload::Stroke(s)) = m.payload else {
            panic!("wrong payload");
        };
        assert_eq!(s.points.len(), 2);
        assert_eq!(
            (
                s.points[1].x,
                s.points[1].y,
                s.points[1].width,
                s.points[1].t_rel
            ),
            (10.0, 10.0, 3.0, 0.05)
        );
        assert_eq!(s.color_rgb, 0xFF0000);
        let Some(pb::layout::Kind::Anchor(a)) = m.layout.unwrap().kind else {
            panic!("expected anchor layout");
        };
        assert_eq!((a.x, a.y), (120.0, 340.0));
    }

    /// 模拟发送:回执形状与真实 gRPC 对齐,seq 按序回显。
    #[tokio::test]
    async fn mock_append_returns_summary() {
        let mut sink = Sink::Mock;
        let msgs = vec![system_notice("nb-1", 1, "新建画布"), sample_stroke()];
        let s = sink.append(&msgs).await.unwrap();
        assert_eq!(
            s,
            AppendSummary {
                first_msg_id: MOCK_FIRST_MSG_ID,
                accepted: 2,
                seqs: vec![1, 2],
            }
        );
    }

    /// 服务地址:默认值 + 环境变量覆盖(env 测试加锁串行)。
    #[test]
    fn endpoint_resolution() {
        static LOCK: Mutex<()> = Mutex::new(());
        let _g = LOCK.lock().unwrap();
        assert_eq!(endpoint_from_env(), DEFAULT_ENDPOINT);
        unsafe { std::env::set_var("GLASPEN_CHAT_ENDPOINT", "http://127.0.0.1:9999") };
        assert_eq!(endpoint_from_env(), "http://127.0.0.1:9999");
        unsafe { std::env::remove_var("GLASPEN_CHAT_ENDPOINT") };
        assert_eq!(endpoint_from_env(), DEFAULT_ENDPOINT);
    }

    /// 前向兼容:手工编码一条携带未注册 payload 字段号(99)的消息,
    /// 解码后信封字段(notebook/seq/type)完好、payload 为 None ——
    /// 渲染器据此画占位卡片,老客户端遇到新类型不会崩。
    #[test]
    fn unknown_payload_keeps_envelope() {
        fn put_varint(buf: &mut Vec<u8>, mut v: u64) {
            loop {
                let b = (v & 0x7f) as u8;
                v >>= 7;
                if v == 0 {
                    buf.push(b);
                    return;
                }
                buf.push(b | 0x80);
            }
        }
        let mut buf = Vec::new();
        // notebook_id = "nb-1"(field 1, LEN):tag = 0x0A
        buf.push(0x0A);
        put_varint(&mut buf, 4);
        buf.extend_from_slice(b"nb-1");
        // seq = 7(field 2, VARINT):tag = 0x10
        buf.push(0x10);
        put_varint(&mut buf, 7);
        // type = MSG_TYPE_STROKE(1)(field 4, VARINT):tag = 0x20
        buf.push(0x20);
        put_varint(&mut buf, 1);
        // 未知 payload 字段 99(LEN)
        put_varint(&mut buf, (99 << 3) | 2);
        put_varint(&mut buf, 2);
        buf.extend_from_slice(&[0xAB, 0xCD]);

        let m = ChatMessage::decode(&buf[..]).unwrap();
        assert_eq!(m.notebook_id, "nb-1");
        assert_eq!(m.seq, 7);
        assert_eq!(m.r#type, MsgType::Stroke as i32);
        assert!(m.payload.is_none());
    }

    /// SystemNotice 构造器:subtype 必须为 0,文本保真。
    #[test]
    fn system_notice_fields() {
        let m = system_notice("nb-9", 12, "新建画布");
        assert_eq!(m.notebook_id, "nb-9");
        assert_eq!(m.seq, 12);
        assert_eq!(m.r#type, MsgType::System as i32);
        assert_eq!(m.subtype, 0);
        let Some(Payload::System(SystemNotice { text, .. })) = m.payload else {
            panic!("wrong payload");
        };
        assert_eq!(text, "新建画布");
    }
}
