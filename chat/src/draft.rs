//! 手写消息草稿通道(⌘⌃2 按住录制)— `ChatStore/DraftInk` 的客户端。
//!
//! 生命周期与快捷键严格对齐:按住 ⌘⌃2 期间打开一条客户端流,每次抬笔
//! 实时推一帧笔迹(axum 可边收边预览),松开补 `end` 帧 + half-close;
//! axum 在流结束后决定是否把这份草稿作为消息发送/落库,并以
//! [`pb::DraftReply`] 回告决定。
//!
//! ```text
//! key-down ⌘⌃2 ──► launch(begin)
//!   抬笔 ×N   ──► push_stroke(msg)          // 非阻塞,连接建立前先缓冲
//! key-up   ⌘⌃2 ──► finish(count, ms)        // end 帧 + half-close + 等回执
//! ```
//!
//! 对接语义见 docs/ink-draft-grpc.md(写给 axum 侧实现者)。

#![allow(clippy::result_large_err)] // 同 lib.rs:tonic 0.13 生成代码的固有形态

use std::sync::Mutex;
use std::time::Duration;

use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::pb::chat_store_client::ChatStoreClient;
use crate::pb::{DraftBegin, DraftFrame, DraftReply};
use crate::{endpoint_from_env, mock_enabled};
/// 连接超时:本地回环服务,起不来 2 秒足够下结论。
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
/// 会话整体超时(从 launch 到收到回执)。按住几分钟的极端录制也不该卡死。
const SESSION_TIMEOUT: Duration = Duration::from_secs(600);

/// 会话结束的结局,与 `DraftReply.sent` 对齐并折算出可读结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DraftOutcome {
    /// axum 已把草稿作为消息发送/落库。
    Sent { accepted: u64, first_msg_id: u64 },
    /// axum 收到了完整草稿但决定不发送。
    Dropped,
    /// 通道失败(连接不上/中途断开/超时/服务端报错)。
    Failed(String),
}

/// 一条活跃的草稿通道。`launch` 立即返回,连接在后台进行;连接完成前
/// push 的帧在无界通道里缓冲,连上后按序补发。
pub struct DraftChannel {
    tx: tokio::sync::mpsc::UnboundedSender<DraftFrame>,
    done: Option<tokio::task::JoinHandle<Result<DraftReply, String>>>,
}

impl DraftChannel {
    /// 读取环境变量(GLASPEN_CHAT_ENDPOINT / GLASPEN_CHAT_MOCK)并启动会话。
    /// 生产入口:gRPC 模式下自动解析登录身份(docs/grpc-auth.md)并附
    /// `authorization: Bearer <JWT>` metadata;未配置账号则不带(向后兼容)。
    pub fn launch(begin: DraftBegin) -> Self {
        Self::launch_inner(&endpoint_from_env(), mock_enabled(), begin, true)
    }

    /// 显式指定地址与模式启动会话(测试用):不做身份解析。
    pub fn launch_with(endpoint: &str, mock: bool, begin: DraftBegin) -> Self {
        Self::launch_inner(endpoint, mock, begin, false)
    }

    /// 显式指定地址与模式启动会话,并**携带登录身份**(mock=false 时)。
    /// demo/工具(mock-send --draft)与生产入口语义一致应使用此入口:
    /// 未配置账号时 token()=None,行为与 launch_with 相同(向后兼容)。
    pub fn launch_with_auth(endpoint: &str, mock: bool, begin: DraftBegin) -> Self {
        Self::launch_inner(endpoint, mock, begin, true)
    }

    fn launch_inner(endpoint: &str, mock: bool, begin: DraftBegin, use_auth: bool) -> Self {
        let (tx, rx) = unbounded_channel::<DraftFrame>();
        // 首帧固定是 begin;通道未连接时它就先在缓冲里排着。
        send_frame(&tx, begin_frame(begin));
        let task = crate::runtime().spawn(run_session(endpoint.to_owned(), mock, rx, use_auth));
        DraftChannel {
            tx,
            done: Some(task),
        }
    }

    /// 实时推送一条笔迹帧。非阻塞,可在主线程/FFI 里直接调用。
    /// 返回 false 表示通道已死(连接失败或对端提前断开),本帧及后续帧
    /// 都不会被送达 —— 调用方应尽快 finish 并向用户报告失败。
    pub fn push_stroke(&self, msg: crate::pb::ChatMessage) -> bool {
        send_frame(&self.tx, DraftFrame {
            frame: Some(crate::pb::draft_frame::Frame::Stroke(msg)),
        })
    }

    /// 结束会话:补 end 帧 → half-close → 等待 axum 的决定(直至回执或超时)。
    /// async 以便测试直接 await;FFI 侧用 `runtime().block_on(...)` 包一层,
    /// 在松开快捷键后的后台线程上阻塞。
    pub async fn finish(mut self, stroke_count: u32, duration_ms: u64) -> DraftOutcome {
        let _ = send_frame(&self.tx, DraftFrame {
            frame: Some(crate::pb::draft_frame::Frame::End(crate::pb::DraftEnd {
                stroke_count,
                duration_ms,
                cancelled: false,
            })),
        });
        drop(self.tx); // 关闭请求流 = half-close,服务端随即处理并回执
        let reply: Result<DraftReply, String> = match self.done.take() {
            Some(h) => h
                .await
                .map_err(|e| format!("会话任务崩溃: {e}"))
                .and_then(|r| r),
            None => Err("会话任务丢失".into()),
        };
        match reply {
            Ok(r) if r.sent => DraftOutcome::Sent {
                accepted: r.accepted,
                first_msg_id: r.first_msg_id,
            },
            Ok(_) => DraftOutcome::Dropped,
            Err(e) => DraftOutcome::Failed(e),
        }
    }
}

fn send_frame(
    tx: &tokio::sync::mpsc::UnboundedSender<DraftFrame>,
    frame: DraftFrame,
) -> bool {
    tx.send(frame).is_ok()
}

fn begin_frame(begin: DraftBegin) -> DraftFrame {
    DraftFrame {
        frame: Some(crate::pb::draft_frame::Frame::Begin(begin)),
    }
}

/// 会话任务:真实 gRPC 或模拟。连接失败时立刻丢弃 rx(此后 push 返回
/// false,调用方能及时感知),结束帧以后调用方 drop(tx) 触发 half-close。
/// `use_auth` = 是否解析登录身份并附 metadata(mock 模式不涉及)。
async fn run_session(
    endpoint: String,
    mock: bool,
    rx: UnboundedReceiver<DraftFrame>,
    use_auth: bool,
) -> Result<DraftReply, String> {
    if mock {
        return mock_session(rx).await;
    }
    // 身份在会话开始时取一次(⌘⌃2 key-down);登录失败降级为无身份发送。
    let bearer = if use_auth {
        crate::auth::token().await
    } else {
        None
    };
    run_grpc(endpoint, rx, bearer).await
}

/// 真实 gRPC 会话:连接 → 附加身份 → 建立流,直到 half-close 拿到回执。
/// `bearer = None` 表示不鉴权(axum 过渡行为:回退全局最近路由)。
pub(crate) async fn run_grpc(
    endpoint: String,
    rx: UnboundedReceiver<DraftFrame>,
    bearer: Option<String>,
) -> Result<DraftReply, String> {
    let connect = tokio::time::timeout(CONNECT_TIMEOUT, ChatStoreClient::connect(endpoint));
    let mut client = match connect.await {
        Ok(Ok(c)) => c,
        Ok(Err(e)) => {
            drop(rx);
            return Err(format!("连接失败: {e}"));
        }
        Err(_) => {
            drop(rx);
            return Err("连接超时".into());
        }
    };
    let mut req = tonic::Request::new(UnboundedReceiverStream::new(rx));
    if let Some(v) = bearer.as_deref().and_then(crate::auth::bearer_metadata) {
        req.metadata_mut().insert("authorization", v);
    }
    match tokio::time::timeout(SESSION_TIMEOUT, client.draft_ink(req)).await {
        Ok(Ok(resp)) => Ok(resp.into_inner()),
        Ok(Err(status)) => Err(match status.code() {
            // token 失效/被登出:清缓存,下一次会话重新登录换新 token。
            tonic::Code::Unauthenticated => {
                crate::auth::invalidate();
                "涂鸦身份已过期,请重新书写一次".into()
            }
            // 身份有效但 axum 里没有该用户的 ink-route。
            tonic::Code::FailedPrecondition => {
                "先在 kongde 打开一次要发送目标者的会话页".into()
            }
            _ => format!("服务端报错: {status}"),
        }),
        Err(_) => {
            // 超时后 rx 还被请求流持有,会随 future 一起丢弃,这里拿不到;
            // 直接以失败收场。
            Err("会话超时".into())
        }
    }
}

/// 模拟会话:逐帧打印摘要,伪造"已发送"回执。axum 服务未就绪时的开发兜底。
async fn mock_session(mut rx: UnboundedReceiver<DraftFrame>) -> Result<DraftReply, String> {
    eprintln!("[mock] -> glaspen.chat.v1.ChatStore/DraftInk (draft session open)");
    let mut strokes: u64 = 0;
    while let Some(frame) = rx.recv().await {
        match frame.frame {
            Some(crate::pb::draft_frame::Frame::Begin(b)) => {
                eprintln!(
                    "[mock]   begin session={} notebook={} canvas={}x{}",
                    b.session_id, b.notebook_id, b.canvas_w, b.canvas_h
                );
            }
            Some(crate::pb::draft_frame::Frame::Stroke(m)) => {
                strokes += 1;
                eprintln!("[mock]   stroke #{strokes} {}", crate::summarize(&m));
            }
            Some(crate::pb::draft_frame::Frame::End(e)) => {
                eprintln!(
                    "[mock]   end strokes={} duration={}ms cancelled={}",
                    e.stroke_count, e.duration_ms, e.cancelled
                );
            }
            None => eprintln!("[mock]   (empty frame)"),
        }
    }
    eprintln!("[mock] <- DraftReply {{ sent: true, accepted: {strokes} }}");
    Ok(DraftReply {
        sent: true,
        accepted: strokes,
        first_msg_id: 0,
    })
}

/// 生成会话 id(时间戳 + 计数器,本地唯一即可)。
pub fn new_session_id() -> String {
    static COUNTER: Mutex<u64> = Mutex::new(0);
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let mut n = COUNTER.lock().unwrap();
    *n += 1;
    format!("draft-{ms:x}-{:x}", *n)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pb::draft_frame::Frame;
    use crate::pb::{ChatMessage, chat_store_server};
    use crate::stroke_message;
    use prost::Message as _;
    use std::sync::Arc;
    use tonic::{Request, Response, Status, Streaming};

    /// DraftFrame 编解码往返,帧序 begin → stroke → end 保真。
    #[test]
    fn draft_frame_roundtrip() {
        let frames = vec![
            begin_frame(DraftBegin {
                session_id: "draft-1".into(),
                started_at_ms: 1727400000000,
                notebook_id: "glaspen2-doodle".into(),
                author: String::new(),
                device: String::new(),
                canvas_w: 1920,
                canvas_h: 1080,
            }),
            DraftFrame {
                frame: Some(Frame::Stroke(stroke_message(
                    "glaspen2-doodle",
                    1,
                    "",
                    "",
                    0xFF0000,
                    1.0,
                    &[(0.0, 0.0, 2.0, 0.0), (10.0, 4.0, 3.0, 0.05)],
                    None,
                ))),
            },
            DraftFrame {
                frame: Some(Frame::End(crate::pb::DraftEnd {
                    stroke_count: 1,
                    duration_ms: 1234,
                    cancelled: false,
                })),
            },
        ];
        for f in frames {
            let bytes = f.encode_to_vec();
            let back = DraftFrame::decode(&bytes[..]).unwrap();
            assert_eq!(back, f);
        }
    }

    /// 测试用服务端:记录帧序与 authorization 头,按预设行为回执或报错。
    enum Behavior {
        /// 消费全部帧后回执"已发送"。
        Collect,
        /// 记录 auth 头并消费帧后,返回指定 Status(测错误映射)。
        Reject(Status),
    }
    struct ProbeStore {
        frames: Arc<Mutex<Vec<Frame>>>,
        captured_auth: Arc<Mutex<Option<String>>>,
        behavior: Behavior,
    }

    #[tonic::async_trait]
    impl chat_store_server::ChatStore for ProbeStore {
        type FetchMessagesStream =
            std::pin::Pin<Box<dyn tokio_stream::Stream<Item = Result<ChatMessage, Status>> + Send>>;
        type DownloadMediaStream =
            std::pin::Pin<Box<dyn tokio_stream::Stream<Item = Result<crate::pb::MediaChunk, Status>> + Send>>;

        async fn append_messages(
            &self,
            _request: Request<Streaming<ChatMessage>>,
        ) -> Result<Response<crate::pb::AppendReply>, Status> {
            Err(Status::unimplemented("not needed here"))
        }

        async fn draft_ink(
            &self,
            request: Request<Streaming<DraftFrame>>,
        ) -> Result<Response<DraftReply>, Status> {
            *self.captured_auth.lock().unwrap() = request
                .metadata()
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            let mut stream = request.into_inner();
            while let Some(f) = stream.message().await.map_err(|s| {
                Status::internal(format!("bad frame: {s}"))
            })? {
                if let Some(frame) = f.frame {
                    self.frames.lock().unwrap().push(frame);
                }
            }
            match &self.behavior {
                Behavior::Collect => {
                    let strokes = self
                        .frames
                        .lock()
                        .unwrap()
                        .iter()
                        .filter(|f| matches!(f, Frame::Stroke(_)))
                        .count() as u64;
                    Ok(Response::new(DraftReply {
                        sent: true,
                        accepted: strokes,
                        first_msg_id: 100 + strokes,
                    }))
                }
                Behavior::Reject(status) => Err(status.clone()),
            }
        }

        async fn fetch_messages(
            &self,
            _request: Request<crate::pb::FetchRequest>,
        ) -> Result<Response<Self::FetchMessagesStream>, Status> {
            Err(Status::unimplemented("not needed here"))
        }

        async fn upload_media(
            &self,
            _request: Request<Streaming<crate::pb::MediaChunk>>,
        ) -> Result<Response<crate::pb::MediaAck>, Status> {
            Err(Status::unimplemented("not needed here"))
        }

        async fn download_media(
            &self,
            _request: Request<crate::pb::MediaQuery>,
        ) -> Result<Response<Self::DownloadMediaStream>, Status> {
            Err(Status::unimplemented("not needed here"))
        }
    }

    /// 起一个内嵌 tonic 服务端,返回其地址。端口由系统分配。
    async fn spawn_store(store: ProbeStore) -> Result<String, Box<dyn std::error::Error>> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(chat_store_server::ChatStoreServer::new(store))
                .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
                .await
                .unwrap();
        });
        Ok(format!("http://{addr}"))
    }

    fn probe_store(
        frames: Arc<Mutex<Vec<Frame>>>,
        captured_auth: Arc<Mutex<Option<String>>>,
        behavior: Behavior,
    ) -> ProbeStore {
        ProbeStore {
            frames,
            captured_auth,
            behavior,
        }
    }

    /// 全链路:launch → 逐笔实时推送 → finish → 服务端按序收到
    /// begin/2×stroke/end,回执"已发送 2 笔"。
    #[tokio::test]
    async fn draft_channel_live_session() {
        let frames = Arc::new(Mutex::new(Vec::new()));
        let endpoint = spawn_store(probe_store(
            frames.clone(),
            Arc::new(Mutex::new(None)),
            Behavior::Collect,
        ))
        .await
        .unwrap();

        let channel = DraftChannel::launch_with(
            &endpoint,
            false,
            DraftBegin {
                session_id: new_session_id(),
                started_at_ms: 1727400000000,
                notebook_id: "glaspen2-doodle".into(),
                author: String::new(),
                device: String::new(),
                canvas_w: 1920,
                canvas_h: 1080,
            },
        );
        // 模拟两笔:push 立即返回(不等连接),帧在通道里缓冲。
        for seq in 1..=2u64 {
            assert!(channel.push_stroke(stroke_message(
                "glaspen2-doodle",
                seq,
                "",
                "",
                0x00FF00,
                1.0,
                &[(seq as f64, 1.0, 2.0, 0.0)],
                None,
            )));
        }
        let outcome = channel.finish(2, 800).await;
        assert_eq!(
            outcome,
            DraftOutcome::Sent {
                accepted: 2,
                first_msg_id: 102
            }
        );
        let got = frames.lock().unwrap();
        assert_eq!(got.len(), 4, "begin + 2 strokes + end");
        assert!(matches!(got[0], Frame::Begin(_)));
        assert!(matches!(got[1], Frame::Stroke(_)));
        assert!(matches!(got[2], Frame::Stroke(_)));
        assert!(
            matches!(got[3], Frame::End(ref e) if e.stroke_count == 2 && e.duration_ms == 800)
        );
    }

    /// 连不上的地址:push 变 false(通道已死),finish 返回 Failed。
    #[tokio::test]
    async fn draft_channel_unreachable_fails() {
        // 127.0.0.1:1 几乎必然拒绝连接,2 秒超时兜底。
        let channel = DraftChannel::launch_with(
            "http://127.0.0.1:1",
            false,
            DraftBegin {
                session_id: "x".into(),
                started_at_ms: 0,
                notebook_id: "glaspen2-doodle".into(),
                author: String::new(),
                device: String::new(),
                canvas_w: 0,
                canvas_h: 0,
            },
        );
        // 等任务跑到连接失败并丢弃 rx。
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(!channel.push_stroke(stroke_message(
            "glaspen2-doodle", 1, "", "", 0xFF0000, 1.0, &[(0.0, 0.0, 1.0, 0.0)], None,
        )));
        assert!(matches!(channel.finish(0, 0).await, DraftOutcome::Failed(_)));
    }

    /// 模拟会话:逐帧打印并伪造回执,outcome 与真实形状一致。
    #[tokio::test]
    async fn draft_channel_mock_session() {
        let channel = DraftChannel::launch_with(
            "http://ignored:1",
            true,
            DraftBegin {
                session_id: "mock".into(),
                started_at_ms: 0,
                notebook_id: "glaspen2-doodle".into(),
                author: String::new(),
                device: String::new(),
                canvas_w: 100,
                canvas_h: 100,
            },
        );
        assert!(channel.push_stroke(stroke_message(
            "glaspen2-doodle", 1, "", "", 0xFF0000, 1.0, &[(0.0, 0.0, 1.0, 0.0)], None,
        )));
        assert_eq!(
            channel.finish(1, 500).await,
            DraftOutcome::Sent {
                accepted: 1,
                first_msg_id: 0
            }
        );
    }

    /// finish 之后通道不复存在(消费 self),避免误用。
    #[tokio::test]
    async fn finish_consumes_channel() {
        let channel = DraftChannel::launch_with(
            "http://ignored:1",
            true,
            DraftBegin {
                session_id: "mock".into(),
                started_at_ms: 0,
                notebook_id: "n".into(),
                author: String::new(),
                device: String::new(),
                canvas_w: 0,
                canvas_h: 0,
            },
        );
        let _ = channel.finish(0, 0).await; // move 语义:再调用直接编译错
    }

    /// 会话 id:非空且带 draft- 前缀,同进程内递增不重复。
    #[test]
    fn session_ids_distinct() {
        let a = new_session_id();
        let b = new_session_id();
        assert!(a.starts_with("draft-") && b.starts_with("draft-"));
        assert_ne!(a, b);
    }

    /// run_grpc 带 bearer 时,请求 metadata 必须携带 authorization 头。
    #[tokio::test]
    async fn grpc_request_carries_bearer_metadata() {
        let frames = Arc::new(Mutex::new(Vec::new()));
        let auth = Arc::new(Mutex::new(None));
        let endpoint = spawn_store(probe_store(frames, auth.clone(), Behavior::Collect))
            .await
            .unwrap();
        let (tx, rx) = unbounded_channel();
        send_frame(&tx, begin_frame(DraftBegin {
            session_id: "auth-1".into(),
            started_at_ms: 0,
            notebook_id: "glaspen2-doodle".into(),
            author: String::new(),
            device: String::new(),
            canvas_w: 0,
            canvas_h: 0,
        }));
        assert!(send_frame(&tx, DraftFrame {
            frame: Some(Frame::Stroke(stroke_message(
                "glaspen2-doodle", 1, "", "", 0xFF0000, 1.0, &[(0.0, 0.0, 1.0, 0.0)], None,
            ))),
        }));
        drop(tx); // half-close
        let reply = super::run_grpc(endpoint, rx, Some("jwt-live".into())).await
            .expect("session should succeed");
        assert!(reply.sent && reply.accepted == 1);
        assert_eq!(auth.lock().unwrap().as_deref(), Some("Bearer jwt-live"));
    }

    /// 不带身份(bearer=None)时不发 authorization 头 —— axum 过渡回退。
    #[tokio::test]
    async fn grpc_request_without_token_omits_metadata() {
        let frames = Arc::new(Mutex::new(Vec::new()));
        let auth = Arc::new(Mutex::new(None));
        let endpoint = spawn_store(probe_store(frames, auth.clone(), Behavior::Collect))
            .await
            .unwrap();
        let (tx, rx) = unbounded_channel();
        drop(tx); // 空会话:begin 都不发也允许,直接 half-close
        let reply = super::run_grpc(endpoint, rx, None).await.expect("should succeed");
        assert!(reply.sent);
        assert_eq!(auth.lock().unwrap().as_deref(), None);
    }

    /// unauthenticated → 清缓存 + "身份已过期"的用户可读错误。
    #[tokio::test]
    async fn unauthenticated_maps_to_expired_identity() {
        let endpoint = spawn_store(probe_store(
            Arc::new(Mutex::new(Vec::new())),
            Arc::new(Mutex::new(None)),
            Behavior::Reject(Status::unauthenticated("token expired")),
        ))
        .await
        .unwrap();
        let (tx, rx) = unbounded_channel();
        drop(tx);
        let err = super::run_grpc(endpoint, rx, Some("jwt-stale".into()))
            .await
            .unwrap_err();
        assert!(err.contains("身份已过期"), "{err}");
    }

    /// failed_precondition → 提示先去 kongde 打开目标会话页。
    #[tokio::test]
    async fn failed_precondition_maps_to_route_hint() {
        let endpoint = spawn_store(probe_store(
            Arc::new(Mutex::new(Vec::new())),
            Arc::new(Mutex::new(None)),
            Behavior::Reject(Status::failed_precondition("no ink-route")),
        ))
        .await
        .unwrap();
        let (tx, rx) = unbounded_channel();
        drop(tx);
        let err = super::run_grpc(endpoint, rx, Some("jwt-ok".into()))
            .await
            .unwrap_err();
        assert!(err.contains("kongde"), "{err}");
        assert!(err.contains("会话页"), "{err}");
    }
}
