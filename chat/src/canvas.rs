//! 共享画布上行(设置面板「共享画布」tab 打开期间)— `ChatStore/ShareInk`
//! 的客户端。
//!
//! glaspen2 在这条链路里**只作为手写工具**:tab 打开 → 建流;抬笔 → 推一帧
//! 笔迹;tab 关闭/面板退出 → end 帧 + half-close。axum 把笔迹转给该用户在
//! kongde 打开的接收页;连接成败不进用户界面(失败静默,只留 stderr)。
//! 协议语义见 docs/canvas-share-grpc.md。
//!
//! ```text
//! tab 打开      ──► launch()                 // begin 帧已入队,连接后台进行
//!   抬笔 ×N     ──► push_stroke(msg)          // 非阻塞,连不上就静默丢
//! tab 关闭/退出 ──► finish(count, ms)          // end 帧 + half-close
//! ```

use std::time::Duration;

use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::pb::chat_store_client::ChatStoreClient;
use crate::pb::{InkFrame, ShareBegin, ShareEnd, ShareInkReply, ShareStroke};
use crate::{endpoint_from_env, mock_enabled};

/// 连接超时:本地回环服务,起不来 2 秒足够下结论。
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
/// 会话整体超时(tab 可能开很久,给一个极大的兜底,防服务端僵死挂住任务)。
const SESSION_TIMEOUT: Duration = Duration::from_secs(3600);

/// 一条活跃的上行会话。`launch` 立即返回,连接在后台进行;连接完成前
/// push 的帧在无界通道里缓冲,连上后按序补发。
pub struct InkShareChannel {
    tx: tokio::sync::mpsc::UnboundedSender<InkFrame>,
    done: Option<tokio::task::JoinHandle<Result<ShareInkReply, String>>>,
}

impl InkShareChannel {
    /// 读取环境变量(GLASPEN_CHAT_ENDPOINT / GLASPEN_CHAT_MOCK)并启动会话。
    /// gRPC 模式下自动携带登录身份(docs/grpc-auth.md);未配置账号不带。
    pub fn launch() -> Self {
        Self::launch_inner(&endpoint_from_env(), mock_enabled(), true)
    }

    /// 显式指定地址与模式(测试用): 不做登录解析。
    pub fn launch_with(endpoint: &str, mock: bool) -> Self {
        Self::launch_inner(endpoint, mock, false)
    }

    fn launch_inner(endpoint: &str, mock: bool, use_auth: bool) -> Self {
        let (tx, rx) = unbounded_channel::<InkFrame>();
        // 首帧固定是 begin;连接完成前它先在缓冲里排着。
        let _ = tx.send(InkFrame {
            frame: Some(crate::pb::ink_frame::Frame::Begin(ShareBegin {
                session_id: new_session_id(),
                started_at_ms: now_ms(),
                author: String::new(),
                device: String::new(),
            })),
        });
        let task = crate::runtime().spawn(run_session(endpoint.to_owned(), mock, rx, use_auth));
        InkShareChannel {
            tx,
            done: Some(task),
        }
    }

    /// 实时推送一条笔迹帧。非阻塞,可在主线程/FFI 里直接调用。
    /// 返回 false = 通道已死(连接失败/对端断开),本帧被丢弃 —— 静默即可。
    pub fn push_stroke(&self, stroke: ShareStroke) -> bool {
        self.tx
            .send(InkFrame {
                frame: Some(crate::pb::ink_frame::Frame::Stroke(stroke)),
            })
            .is_ok()
    }

    /// 结束会话:补 end 帧 → half-close。结果只进 stderr(用户无感)。
    pub async fn finish(mut self, stroke_count: u32, duration_ms: u64) {
        let _ = self.tx.send(InkFrame {
            frame: Some(crate::pb::ink_frame::Frame::End(ShareEnd {
                stroke_count,
                duration_ms,
            })),
        });
        drop(self.tx); // 关闭请求流 = half-close
        if let Some(h) = self.done.take() {
            match h.await {
                Ok(Ok(reply)) => {
                    eprintln!(
                        "[share-ink] session closed: {} strokes / {duration_ms}ms, reply ok={} ({})",
                        stroke_count, reply.ok, reply.message
                    );
                }
                Ok(Err(e)) => eprintln!("[share-ink] session ended with error: {e}"),
                Err(e) => eprintln!("[share-ink] session task crashed: {e}"),
            }
        }
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// 会话 id(时间戳 + 计数器,本地唯一即可)。
fn new_session_id() -> String {
    static COUNTER: std::sync::Mutex<u64> = std::sync::Mutex::new(0);
    let mut n = COUNTER.lock().unwrap();
    *n += 1;
    format!("share-{:x}-{:x}", now_ms(), *n)
}

/// 会话任务:真实 gRPC 或模拟。连接失败静默关闭(rx 丢弃 → 后续 push 为 false)。
async fn run_session(
    endpoint: String,
    mock: bool,
    rx: UnboundedReceiver<InkFrame>,
    use_auth: bool,
) -> Result<ShareInkReply, String> {
    if mock {
        return mock_session(rx).await;
    }
    // 未登录(未配置或登录失败)→ 不开启上行, 由调用方静默处理。
    let bearer = if use_auth {
        match crate::auth::token().await {
            Some(t) => Some(t),
            None => return Err("未登录: 共享画布需先在设置中登录".into()),
        }
    } else {
        None
    };
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
    match tokio::time::timeout(SESSION_TIMEOUT, client.share_ink(req)).await {
        Ok(Ok(resp)) => Ok(resp.into_inner()),
        Ok(Err(status)) => Err(format!("服务端报错: {status}")),
        Err(_) => Err("会话超时".into()),
    }
}

/// 模拟会话:逐帧打印摘要,伪造回执。axum 服务未就绪时的开发兜底。
async fn mock_session(mut rx: UnboundedReceiver<InkFrame>) -> Result<ShareInkReply, String> {
    eprintln!("[mock] -> glaspen.chat.v1.ChatStore/ShareInk (ink share open)");
    let mut strokes: u64 = 0;
    while let Some(frame) = rx.recv().await {
        match frame.frame {
            Some(crate::pb::ink_frame::Frame::Begin(b)) => {
                eprintln!("[mock]   begin session={} started={}", b.session_id, b.started_at_ms);
            }
            Some(crate::pb::ink_frame::Frame::Stroke(s)) => {
                strokes += 1;
                eprintln!(
                    "[mock]   stroke #{strokes} points={} color=#{:06X}",
                    s.points.len(),
                    s.color_rgb
                );
            }
            Some(crate::pb::ink_frame::Frame::End(e)) => {
                eprintln!("[mock]   end strokes={} duration={}ms", e.stroke_count, e.duration_ms);
            }
            None => {}
        }
    }
    eprintln!("[mock] <- ShareInkReply {{ ok: true, stroke_count: {strokes} }}");
    Ok(ShareInkReply {
        ok: true,
        message: "mock".into(),
        stroke_count: strokes,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pb::chat_store_server;
    use crate::pb::ink_frame::Frame;
    use prost::Message as _;
    use std::sync::Arc;
    use tonic::{Request, Response, Status, Streaming};

    fn stroke(points: &[(f64, f64)]) -> ShareStroke {
        ShareStroke {
            color_rgb: 0xFF0000,
            width_scale: 1.0,
            points: points
                .iter()
                .map(|(x, y)| crate::pb::StrokePoint {
                    x: *x,
                    y: *y,
                    width: 2.0,
                    t_rel: 0.0,
                })
                .collect(),
        }
    }

    /// 测试服务端:收集 stroke 帧,回执条数。
    struct CollectingStore {
        strokes: Arc<std::sync::Mutex<Vec<ShareStroke>>>,
    }

    #[tonic::async_trait]
    impl chat_store_server::ChatStore for CollectingStore {
        type FetchMessagesStream = std::pin::Pin<
            Box<dyn tokio_stream::Stream<Item = Result<crate::pb::ChatMessage, Status>> + Send>,
        >;
        type DownloadMediaStream = std::pin::Pin<
            Box<dyn tokio_stream::Stream<Item = Result<crate::pb::MediaChunk, Status>> + Send>,
        >;

        async fn draft_ink(
            &self,
            _request: Request<Streaming<crate::pb::DraftFrame>>,
        ) -> Result<Response<crate::pb::DraftReply>, Status> {
            Err(Status::unimplemented("not needed here"))
        }

        async fn share_ink(
            &self,
            request: Request<Streaming<InkFrame>>,
        ) -> Result<Response<ShareInkReply>, Status> {
            let mut stream = request.into_inner();
            while let Some(f) = stream
                .message()
                .await
                .map_err(|s| Status::internal(format!("bad frame: {s}")))?
            {
                if let Some(crate::pb::ink_frame::Frame::Stroke(s)) = f.frame {
                    self.strokes.lock().unwrap().push(s);
                }
            }
            let n = self.strokes.lock().unwrap().len() as u64;
            Ok(Response::new(ShareInkReply {
                ok: true,
                message: String::new(),
                stroke_count: n,
            }))
        }

        async fn append_messages(
            &self,
            _request: Request<Streaming<crate::pb::ChatMessage>>,
        ) -> Result<Response<crate::pb::AppendReply>, Status> {
            Err(Status::unimplemented("not needed here"))
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

    async fn spawn_store(
        strokes: Arc<std::sync::Mutex<Vec<ShareStroke>>>,
    ) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(chat_store_server::ChatStoreServer::new(CollectingStore {
                    strokes,
                }))
                .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
                .await
                .unwrap();
        });
        format!("http://{addr}")
    }

    /// 全链路:tab 打开(launch)→ 实时推 2 笔 → tab 关闭(finish)→
    /// 服务端收到 2 笔且回执条数一致。
    #[tokio::test]
    async fn ink_share_live_session() {
        let got = Arc::new(std::sync::Mutex::new(Vec::new()));
        let endpoint = spawn_store(got.clone()).await;
        let ch = InkShareChannel::launch_with(&endpoint, false);
        ch.push_stroke(stroke(&[(1.0, 2.0)]));
        // 不等连接:帧先进缓冲
        ch.push_stroke(stroke(&[(3.0, 4.0), (5.0, 6.0)]));
        ch.finish(2, 500).await;
        assert_eq!(got.lock().unwrap().len(), 2, "服务端应收满 2 笔");
        assert_eq!(
            got.lock().unwrap()[1].points.len(),
            2,
            "点列保真(第二笔 2 点)"
        );
    }

    /// 连不上的地址:push 变 false,finish 正常收尾不 panic。
    #[tokio::test]
    async fn ink_share_unreachable_is_silent() {
        let ch = InkShareChannel::launch_with("http://127.0.0.1:1", false);
        // 拒绝在 macOS 上毫秒级,在 Windows 上可能被环境拖过 300ms ——
        // 轮询兜底,上限盖过 CONNECT_TIMEOUT(draft 同款)。
        let mut refused = false;
        for _ in 0..70 {
            if !ch.push_stroke(stroke(&[(0.0, 0.0)])) {
                refused = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(refused, "连接失败后 push_stroke 应变为 false");
        ch.finish(0, 0).await; // 不抛错:共享上行失败对用户完全无感
    }

    /// InkFrame 编解码往返:begin/stroke/end 保真。
    #[test]
    fn ink_frame_roundtrip() {
        let frames = vec![
            InkFrame {
                frame: Some(Frame::Begin(ShareBegin {
                    session_id: "share-1".into(),
                    started_at_ms: 1727500000000,
                    author: String::new(),
                    device: String::new(),
                })),
            },
            InkFrame {
                frame: Some(Frame::Stroke(stroke(&[(1.0, 2.0), (3.0, 4.0)]))),
            },
            InkFrame {
                frame: Some(Frame::End(ShareEnd {
                    stroke_count: 1,
                    duration_ms: 250,
                })),
            },
        ];
        for f in frames {
            let back = InkFrame::decode(&f.encode_to_vec()[..]).unwrap();
            assert_eq!(back, f);
        }
    }

    /// 会话 id 非空且递增。
    #[test]
    fn session_ids_distinct() {
        assert_ne!(new_session_id(), new_session_id());
    }
}
