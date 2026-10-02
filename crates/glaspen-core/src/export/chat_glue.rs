//! 手写消息与共享上行的会话胶水:⌘⌃2 草稿、⌘⌃3 直发、ShareInk、涂鸦身份。

use super::*;

/// 手写消息归属的流。当前形态是"本机涂鸦工具",单一流即可;
/// 服务端按 (notebook_id, seq) 去重。
pub(crate) const CHAT_NOTEBOOK: &str = "glaspen2-doodle";

/// 流内 seq 的高水位,持久化在 user_settings 里:应用重启后接着涨,
/// 避免服务端把重放的新消息当成重复吞掉。
static CHAT_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// 预留 n 个连续 seq,返回首个 seq。首次调用时从 DB 加载高水位。
fn reserve_chat_seqs(n: u64) -> u64 {
    use std::sync::atomic::Ordering;
    static LOADED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    LOADED.get_or_init(|| {
        let v = runtime()
            .block_on(db::load_setting("chat_seq"))
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        CHAT_SEQ.store(v, Ordering::SeqCst);
    });
    let first = CHAT_SEQ.fetch_add(n, Ordering::SeqCst) + 1;
    runtime().block_on(db::save_setting(
        "chat_seq",
        &CHAT_SEQ.load(Ordering::SeqCst).to_string(),
    ));
    first
}

/// 一条笔迹 → 一条 STROKE 消息(无作者/设备字段,涂鸦工具不携带身份)。
pub(crate) fn stroke_to_chat_message(seq: u64, s: &Stroke) -> glaspen_chat::pb::ChatMessage {
    let to8 = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    let color_rgb = (to8(s.r) << 16) | (to8(s.g) << 8) | to8(s.b);
    glaspen_chat::stroke_message(
        CHAT_NOTEBOOK,
        seq,
        "",
        "",
        color_rgb,
        1.0, // 线宽逐点携带在 points 里,全局倍率固定 1.0
        &s.points,
        None, // flow 布局:聊天流里按块下排
    )
}

/// 把录制窗口 [start_index, end_index) 内的笔迹打包成手写消息,发送到聊天服务。
/// 由 macOS 端 ⌘⌃3 key-up 在后台线程调用(start/end 均在主线程钉好)。
/// 返回发送的消息条数;无新笔迹返回 0;发送失败返回 -1。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_chat_send_strokes(start_index: c_int, end_index: c_int) -> c_int {
    let strokes: Vec<Stroke> = {
        let all = STROKES.lock().unwrap();
        let start = start_index.clamp(0, all.len() as i32) as usize;
        let end = (end_index.clamp(0, all.len() as i32) as usize).max(start);
        all[start..end].to_vec()
    };
    if strokes.is_empty() {
        return 0;
    }

    let first = reserve_chat_seqs(strokes.len() as u64);
    let msgs: Vec<_> = strokes
        .iter()
        .enumerate()
        .map(|(i, s)| stroke_to_chat_message(first + i as u64, s))
        .collect();

    let endpoint = glaspen_chat::endpoint_from_env();
    let send = async {
        let mut sink = glaspen_chat::connect(&endpoint)
            .await
            .map_err(|e| e.to_string())?;
        sink.append(&msgs).await
    };
    match runtime().block_on(send) {
        Ok(summary) => {
            eprintln!("[chat] sent {} strokes (seq {}..)", summary.accepted, first);
            summary.accepted as c_int
        }
        Err(e) => {
            eprintln!("[chat] send failed: {e}");
            -1
        }
    }
}

// ---------------------------------------------------------------------------
// 手写消息草稿通道(⌘⌃2 按住 → ChatStore/DraftInk gRPC 流 → axum 决定是否发送)
// 与 ⌘⌃3 直发的区别:⌘⌃3 是松开后一次性 AppendMessages;⌘⌃2 按住期间
// 笔迹实时流给 axum(可预览),松开 half-close 后由 axum 决定发或不发。
// 语义契约见 docs/ink-draft-grpc.md。
// ---------------------------------------------------------------------------

struct InkDraftSession {
    channel: glaspen_chat::draft::DraftChannel,
    /// 已推送的 STROKES 下标游标(会话开启时的 STROKES.len(),只增不减;
    /// 会话期间发生撤销导致游标越界时直接跳过,宁漏不重)。
    pushed_upto: usize,
    stroke_count: u32,
    started_at: std::time::SystemTime,
}

static INK_DRAFT: std::sync::Mutex<Option<InkDraftSession>> = std::sync::Mutex::new(None);

/// 最近一次草稿通道失败的用户可读原因(ObjC 通知用);CString 常驻,
/// 指针在下次 set 之前一直有效。
static INK_DRAFT_LAST_ERROR: std::sync::Mutex<Option<CString>> = std::sync::Mutex::new(None);

fn set_ink_draft_error(err: Option<&str>) {
    *INK_DRAFT_LAST_ERROR.lock().unwrap() = err.map(|s| CString::new(s).unwrap_or_default());
}

/// 通道失败原因(UTF-8),无失败时返回 NULL。供 ObjC 在 stop 返回 <0 时
/// 展示具体原因(身份过期 / 未注册路由 / 连接失败等)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_ink_draft_last_error() -> *const c_char {
    match INK_DRAFT_LAST_ERROR.lock().unwrap().as_ref() {
        Some(c) => c.as_ptr(),
        None => std::ptr::null(),
    }
}

/// ⌘⌃2 key-down:打开手写草稿通道。ObjC 侧保证先 finish_active_stroke。
/// 返回 1 = 已开启;0 = 已有会话在进行(忽略本次)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_ink_draft_start(canvas_w: c_int, canvas_h: c_int) -> c_int {
    let mut g = INK_DRAFT.lock().unwrap();
    if g.is_some() {
        return 0;
    }
    let started_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let begin = glaspen_chat::pb::DraftBegin {
        session_id: glaspen_chat::draft::new_session_id(),
        started_at_ms,
        notebook_id: CHAT_NOTEBOOK.to_owned(),
        author: String::new(),
        device: String::new(),
        canvas_w: canvas_w.max(0) as u32,
        canvas_h: canvas_h.max(0) as u32,
    };
    // 端点解析优先级:设置库(chat_grpc_endpoint,面板/Dock 启动没有
    // 环境变量时靠它落地)> 环境变量 GLASPEN_CHAT_ENDPOINT > 默认值
    let db_endpoint = runtime()
        .block_on(db::load_setting("chat_grpc_endpoint"))
        .filter(|v| !v.trim().is_empty());
    let endpoint = db_endpoint
        .clone()
        .unwrap_or_else(glaspen_chat::endpoint_from_env);
    // mock 判定:显式 GLASPEN_CHAT_MOCK=0 → 真连,=1 → mock;
    // 未设置时:配置了登录账号或涂鸦端点即默认真连(否则老用户装完
    // 什么都不配仍走 mock)。此前未设置一律 mock,配置了登录也发不出去
    let mock_env = std::env::var("GLASPEN_CHAT_MOCK").ok();
    let auth_ready = glaspen_chat::auth::config().is_configured();
    let mock = match mock_env.as_deref() {
        Some("0") => false,
        Some(_) => true,
        None => !(auth_ready || db_endpoint.is_some()),
    };
    let channel = if mock {
        glaspen_chat::draft::DraftChannel::launch_with(endpoint.as_str(), true, begin)
    } else {
        glaspen_chat::draft::DraftChannel::launch_with_auth(endpoint.as_str(), false, begin)
    };
    *g = Some(InkDraftSession {
        channel,
        pushed_upto: STROKES.lock().unwrap().len(),
        stroke_count: 0,
        started_at: std::time::SystemTime::now(),
    });
    eprintln!("[ink-draft] session opened (canvas {canvas_w}x{canvas_h}, endpoint {endpoint})");
    1
}

/// pen-up 提交笔迹后的钩子:会话进行中时,把本次提交的笔迹实时推进草稿流。
/// 在主线程调用;push 非阻塞(连接建立前帧在通道里缓冲)。
pub(crate) fn ink_draft_on_stroke_committed() {
    let mut g = INK_DRAFT.lock().unwrap();
    let Some(sess) = g.as_mut() else { return };
    let strokes = STROKES.lock().unwrap();
    while sess.pushed_upto < strokes.len() {
        let s = &strokes[sess.pushed_upto];
        sess.pushed_upto += 1;
        if s.points.is_empty() {
            continue;
        }
        sess.stroke_count += 1;
        let msg = stroke_to_chat_message(sess.stroke_count as u64, s);
        if !sess.channel.push_stroke(msg) {
            // 通道已死(连接失败/对端断开)。帧丢弃,结束时的 stop 会拿到
            // Failed 并通知用户;这里只留日志。
            eprintln!(
                "[ink-draft] channel dead at stroke {}, remaining frames dropped",
                sess.stroke_count
            );
            break;
        }
    }
}

// ---------------------------------------------------------------------------
// 涂鸦身份设置(设置面板 ↔ chat::auth):DB 里的账号配置推给 auth 模块
// ---------------------------------------------------------------------------

/// 从 DB 读取涂鸦身份设置(chat_api_base / chat_user / chat_password),
/// 字段级合并环境变量默认值后注入 chat::auth(配置变化会自动清 token 缓存)。
pub(crate) fn sync_chat_auth_from_settings() {
    let (base, user, pass) = runtime().block_on(async {
        (
            db::load_setting("chat_api_base").await.unwrap_or_default(),
            db::load_setting("chat_user").await.unwrap_or_default(),
            db::load_setting("chat_password").await.unwrap_or_default(),
        )
    });
    let nonempty = |s: String| if s.trim().is_empty() { None } else { Some(s) };
    let from_db = glaspen_chat::auth::AuthConfig {
        api_base: nonempty(base),
        user: nonempty(user),
        password: nonempty(pass),
        direct_token: None, // 直接给 token 只走环境变量,不落盘
    };
    glaspen_chat::auth::set_config(glaspen_chat::auth::AuthConfig::from_env().merged(from_db));
}

/// ObjC 入口:设置变化处与启动恢复路径调用。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_chat_auth_reload() {
    sync_chat_auth_from_settings();
}

/// 设置面板「测试登录」:强制用当前配置登录一次(成功则缓存 token)。
/// Ok = 成功;Err = 可读失败原因(给 Flutter 显示)。
pub fn chat_auth_test_login_blocking() -> Result<(), String> {
    runtime()
        .block_on(glaspen_chat::auth::force_login())
        .map(|_| ())
}

// ---------------------------------------------------------------------------
// 共享画布上行(面板「共享画布」tab 打开期间 → ChatStore/ShareInk):
// glaspen2 只作为手写工具 —— tab 开 = 建流,抬笔即推,tab 关 = half-close。
// 接收页在 kongde(经 axum 转给该用户的 ink-route);连接成败静默,
// 不进用户界面。语义与 axum 侧实现见 docs/canvas-share-grpc.md。
// ---------------------------------------------------------------------------

struct InkShareState {
    channel: glaspen_chat::canvas::InkShareChannel,
    stroke_count: u32,
    started_at: std::time::SystemTime,
}

static INK_SHARE: std::sync::Mutex<Option<InkShareState>> = std::sync::Mutex::new(None);

/// 共享上行开关(活页本 tab「共享画布」开关;启动恢复 / 开关切换)。
/// 幂等:重复开是 no-op,重复关也是 no-op。
pub(crate) fn share_ink_set_active_impl(active: bool) {
    if active {
        let mut g = INK_SHARE.lock().unwrap();
        if g.is_some() {
            return;
        }
        *g = Some(InkShareState {
            channel: glaspen_chat::canvas::InkShareChannel::launch(),
            stroke_count: 0,
            started_at: std::time::SystemTime::now(),
        });
        eprintln!("[share-ink] session opened");
        return;
    }
    let Some(sess) = INK_SHARE.lock().unwrap().take() else {
        return;
    };
    let stroke_count = sess.stroke_count;
    let duration_ms = std::time::SystemTime::now()
        .duration_since(sess.started_at)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    // end 帧 + half-close 放后台;结果只进 stderr,用户无感
    runtime().spawn(async move {
        sess.channel.finish(stroke_count, duration_ms).await;
    });
}

/// ObjC 入口:启动恢复(集成开 + share_ink 设置开)与「共享画布」开关切换。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_share_ink_set_active(active: c_int) {
    share_ink_set_active_impl(active != 0);
}

/// pen-up 提交笔迹后的共享钩子:tab 打开期间把本笔实时推给 axum。
/// 活页本/无限画布模式都发(坐标各自成系,去向由 kongde 决定);
/// 通道未连上时帧被丢弃(静默,不打扰)。
pub(crate) fn ink_share_on_stroke_committed() {
    let mut g = INK_SHARE.lock().unwrap();
    let Some(st) = g.as_mut() else { return };
    let Some(s) = STROKES.lock().unwrap().last().cloned() else {
        return;
    };
    let to8 = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    let msg = glaspen_chat::pb::ShareStroke {
        color_rgb: (to8(s.r) << 16) | (to8(s.g) << 8) | to8(s.b),
        width_scale: 1.0,
        points: s
            .points
            .iter()
            .map(|(x, y, w, t)| glaspen_chat::pb::StrokePoint {
                x: *x,
                y: *y,
                width: *w,
                t_rel: *t,
            })
            .collect(),
    };
    if st.channel.push_stroke(msg) {
        st.stroke_count += 1;
    } else {
        eprintln!("[share-ink] 通道未连接,本笔未发送");
    }
}

/// ⌘⌃2 key-up:补 end 帧 + half-close,阻塞等待 axum 的决定。
/// 由 ObjC 侧在后台线程调用(主线程先 finish_active_stroke 保证最后一笔
/// 已经过钩子推进流)。返回:>0 = axum 已发送(接受的笔迹条数);
/// 0 = axum 丢弃了草稿(含 sent 但接受 0 条);-1 = 通道失败/无会话。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_ink_draft_stop() -> c_int {
    let Some(sess) = INK_DRAFT.lock().unwrap().take() else {
        return -1;
    };
    let stroke_count = sess.stroke_count;
    let duration_ms = std::time::SystemTime::now()
        .duration_since(sess.started_at)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let outcome = runtime().block_on(sess.channel.finish(stroke_count, duration_ms));
    eprintln!(
        "[ink-draft] session closed after {stroke_count} strokes / {duration_ms}ms: {outcome:?}"
    );
    match outcome {
        glaspen_chat::draft::DraftOutcome::Sent { accepted, .. } => {
            set_ink_draft_error(None);
            accepted as c_int
        }
        glaspen_chat::draft::DraftOutcome::Dropped => {
            set_ink_draft_error(None);
            0
        }
        glaspen_chat::draft::DraftOutcome::Failed(e) => {
            set_ink_draft_error(Some(&e));
            eprintln!("[ink-draft] failed: {e}");
            -1
        }
    }
}
