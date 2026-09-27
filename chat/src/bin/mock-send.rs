//! 模拟发送演示:构造一小段聊天流,发往本地 axum 服务。
//!
//! 默认走模拟发送器(假装地址存在,逐条打印 gRPC 帧摘要);
//! `--grpc` 或 `GLASPEN_CHAT_MOCK=0` 时连真实服务。
//! `--draft` 演示 ⌘⌃2 手写草稿通道(DraftInk 流)而非直发。
//!
//!   cargo run -p glaspen-chat --bin mock-send
//!   GLASPEN_CHAT_ENDPOINT=http://127.0.0.1:7777 cargo run -p glaspen-chat --bin mock-send -- --grpc
//!   cargo run -p glaspen-chat --bin mock-send -- --draft --grpc

use glaspen_chat::{connect_with, endpoint_from_env, mock_enabled, stroke_message, system_notice};

/// 生成一条正弦波笔迹(72 点),模拟真实手写。
fn sine_stroke(seq: u64) -> glaspen_chat::pb::ChatMessage {
    let points: Vec<(f64, f64, f64, f64)> = (0..72)
        .map(|i| {
            let t = i as f64 / 71.0;
            (
                120.0 + t * 600.0,
                340.0 + (t * std::f64::consts::TAU * 1.5).sin() * 80.0,
                1.5 + t * 2.0, // 笔锋:起笔细、收笔粗
                t * 0.6,       // 0.6 秒写完
            )
        })
        .collect();
    stroke_message(
        "notebook-2026-09-11",
        seq,
        "llp",
        "macbook",
        0xFF0000, // glaspen2 默认红
        1.0,
        &points,
        Some((120.0, 340.0)),
    )
}

fn image_message(seq: u64) -> glaspen_chat::pb::ChatMessage {
    glaspen_chat::pb::ChatMessage {
        notebook_id: "notebook-2026-09-11".into(),
        seq,
        client_msg_id: format!("demo-img-{seq}"),
        r#type: glaspen_chat::pb::MsgType::Image as i32,
        subtype: 0,
        author: "llp".into(),
        device: "macbook".into(),
        created_at_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64,
        layout: None, // 默认 flow
        meta: Default::default(),
        payload: Some(glaspen_chat::pb::chat_message::Payload::Image(
            glaspen_chat::pb::MediaContent {
                // 真实接入时先 UploadMedia 拿到 sha256 id
                media_id: "demo-sha256-e3b0c44298fc1c14".into(),
                width: 1920,
                height: 1080,
                duration_ms: 0,
                thumb_media_id: String::new(),
            },
        )),
    }
}

fn quote_message(seq: u64, page_seq: u64, stroke_seq: u64) -> glaspen_chat::pb::ChatMessage {
    glaspen_chat::pb::ChatMessage {
        notebook_id: "notebook-2026-09-11".into(),
        seq,
        client_msg_id: format!("demo-quote-{seq}"),
        r#type: glaspen_chat::pb::MsgType::Card as i32,
        subtype: 57, // 带引用的消息(对齐微信)
        author: "llp".into(),
        device: "macbook".into(),
        created_at_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64,
        layout: None,
        meta: Default::default(),
        payload: Some(glaspen_chat::pb::chat_message::Payload::Quote(Box::new(
            glaspen_chat::pb::QuoteContent {
                target: Some(glaspen_chat::pb::quote_content::Target::QuoteRegion(
                    glaspen_chat::pb::quote_content::StrokeRegion {
                        page_seq,
                        stroke_seqs: vec![stroke_seq],
                        bbox: Some(glaspen_chat::pb::BBox {
                            x: 120.0,
                            y: 260.0,
                            w: 600.0,
                            h: 160.0,
                        }),
                    },
                )),
                // 递归消息在 prost 生成代码里是 Box<ChatMessage>。
                reply: Some(Box::new(glaspen_chat::pb::ChatMessage {
                    payload: Some(glaspen_chat::pb::chat_message::Payload::Text(
                        glaspen_chat::pb::TextContent {
                            text: "这里再展开讲一下".into(),
                        },
                    )),
                    ..system_notice("notebook-2026-09-11", seq, "")
                })),
            },
        ))),
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let endpoint = endpoint_from_env();
    let force_grpc = std::env::args().any(|a| a == "--grpc");
    let draft_mode = std::env::args().any(|a| a == "--draft");
    if draft_mode {
        return draft_demo(&endpoint, force_grpc).await;
    }
    let mock = if force_grpc { false } else { mock_enabled() };
    let mut sink = connect_with(&endpoint, mock)
        .await
        .map_err(|e| format!("连接 {endpoint} 失败: {e}"))?;
    eprintln!(
        "{}",
        if mock {
            "[demo] axum 服务未就绪,使用模拟发送".to_string()
        } else {
            format!("[demo] 已连接 {endpoint}")
        }
    );

    let msgs = vec![
        system_notice("notebook-2026-09-11", 1, "新建画布 2026-09-11"),
        sine_stroke(2),
        image_message(3),
        quote_message(4, 1, 2), // 引用第 1 页(seq=1 页头)的笔迹 seq=2
    ];

    let summary = sink.append(&msgs).await?;
    println!(
        "append done: first_msg_id={}, accepted={}, seqs={:?}",
        summary.first_msg_id, summary.accepted, summary.seqs
    );
    Ok(())
}

/// 模拟一次 ⌘⌃2 草稿会话:打开 DraftInk 流,分两笔实时推送(间隔 300ms,
/// 便于服务端做实时预览),松开后收回执。与主程序 draft.rs 同一代码路径。
async fn draft_demo(endpoint: &str, force_grpc: bool) -> Result<(), String> {
    use glaspen_chat::draft::{DraftChannel, DraftOutcome, new_session_id};
    use glaspen_chat::pb::DraftBegin;

    let mock = !force_grpc && mock_enabled();
    eprintln!(
        "{}",
        if mock {
            "[demo] axum 服务未就绪,草稿通道走模拟".to_string()
        } else {
            format!("[demo] 草稿通道连接 {endpoint}")
        }
    );
    // 与生产入口一致携带登录身份(配置了账号时),否则与旧行为相同
    let channel = DraftChannel::launch_with_auth(
        endpoint,
        mock,
        DraftBegin {
            session_id: new_session_id(),
            started_at_ms: now_ms(),
            notebook_id: "notebook-2026-09-11".into(),
            author: "llp".into(),
            device: "macbook".into(),
            canvas_w: 1920,
            canvas_h: 1080,
        },
    );
    let started = std::time::Instant::now();
    for (i, seq) in (1..=2u64).enumerate() {
        // 每笔推送前停顿,模拟真实书写节奏(实时帧,非攒批)。
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        let mut msg = sine_stroke(seq);
        msg.seq = seq; // 草稿内局部序号 1..N
        msg.layout = None; // 草稿笔迹与直发一致:flow 布局
        if !channel.push_stroke(msg) {
            eprintln!("[demo] 通道已断开(第 {} 笔后)", i);
            return Ok(());
        }
        eprintln!("[demo] pushed stroke {seq}");
    }
    let outcome = channel
        .finish(2, started.elapsed().as_millis() as u64)
        .await;
    match outcome {
        DraftOutcome::Sent {
            accepted,
            first_msg_id,
        } => println!("draft sent: accepted={accepted}, first_msg_id={first_msg_id}"),
        DraftOutcome::Dropped => println!("draft declined by axum"),
        DraftOutcome::Failed(e) => return Err(e),
    }
    Ok(())
}
