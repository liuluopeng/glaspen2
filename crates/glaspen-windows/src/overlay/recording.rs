// ── D8b: 录制会话状态机接线(GIF / Ctrl+Alt+2 草稿 / Ctrl+Alt+3 直发)──
//
// 状态/起止窗口/决策/用户可见动作的唯一来源 = core `recording::RecordingMachine`
// (两个槽:GIF 机器 + 手写消息机器;草稿/直发在消息槽内互斥)。本文件只做:
//   1) 热键按下/抬起时先 finish_active_stroke 再取笔画数, 喂 Start/Release 事件;
//   2) 按返回的 effects 顺序执行 Windows 侧动作(HUD 文案/编码/剪贴板/gRPC);
//   3) 后台结果 PostMessage 回窗口线程, 再作为结果事件喂回并执行新 effects。
//
// HUD 文案与键名留 adapter(D10 再统一);空窗口/重复按下/模式不匹配/总开关
// 门控等判定全部在 core。旧的 gif_recording / msg_record_start /
// ink_draft_active 会话标志位与逐模式 start/stop 分派已删除。
//
// 与 macOS 的 P0 差异(本文件修复, 见 duplication-backlog D8b):
// ① GIF 编码与写剪贴板拆成两步:编码在后台线程(WM_RECORD_ENCODE_OK 把字节
//    投回窗口线程), 剪贴板在窗口线程, 严格按 GifEncodeSucceeded → Clipboard
//    的 effect 顺序执行(旧实现后台线程一把做完再 WM_RECORD_DONE);
// ② 总开关关闭时草稿收尾走完整 StopChatDraft(提交在写笔画 + half-close +
//    按 axum 裁决提示), 不再静默 stop(对齐 macOS)。

use glaspen_core::recording::{
    DraftOutcome, RecordingEffect, RecordingEvent, RecordingHud, RecordingMachine, RecordingMode,
};

/// 两个状态机槽位(GIF 与手写消息互相独立, 可同时录制 —— 两端旧标志位同语义)。
#[derive(Clone, Copy, PartialEq, Eq)]
enum RecSlot {
    Gif,
    Chat,
}

fn recording_slot_of(mode: RecordingMode) -> RecSlot {
    if mode.is_chat() {
        RecSlot::Chat
    } else {
        RecSlot::Gif
    }
}

fn recording_machine(state: &mut OverlayState, slot: RecSlot) -> &mut RecordingMachine {
    match slot {
        RecSlot::Gif => &mut state.gif_machine,
        RecSlot::Chat => &mut state.chat_machine,
    }
}

/// 喂一个事件, 按序执行返回的 effects(只在消息循环线程调用)。
fn recording_feed(state: &mut OverlayState, slot: RecSlot, event: RecordingEvent) {
    let effects = recording_machine(state, slot).handle(event);
    recording_exec_effects(state, effects);
}

/// 热键按下:先提交在写笔画(事件里的笔画数必须是提交后的计数, 否则窗口会
/// 漏掉正在写的那一笔), 再喂 StartPressed。重复按下由状态机忽略。
fn recording_start(state: &mut OverlayState, mode: RecordingMode) {
    // 总开关是 adapter 门(RegisterHotKey 层集成关时不注册 2/3, 这里再守一道,
    // 与旧 dispatch_hotkey 一致);状态机自己也会防御性忽略。
    if mode.is_chat() && !state.draw.chat_integration {
        return;
    }
    finish_active_stroke(state);
    let stroke_count = glaspen_core::export::glaspen2_stroke_count().max(0) as u32;
    recording_feed(
        state,
        recording_slot_of(mode),
        RecordingEvent::StartPressed { mode, stroke_count },
    );
}

/// 热键抬起:调用方已确认本槽位正处于该模式(Raw Input 看得到所有按键,
/// 不能把普通 r/2/3 抬键也吃掉);抬起不验修饰键, 先松 Ctrl/Alt 也不会卡死。
fn recording_release(state: &mut OverlayState, mode: RecordingMode) {
    finish_active_stroke(state);
    let stroke_count = glaspen_core::export::glaspen2_stroke_count().max(0) as u32;
    recording_feed(
        state,
        recording_slot_of(mode),
        RecordingEvent::ReleasePressed { mode, stroke_count },
    );
}

/// 手写消息总开关变化(actions.rs 的 CMD_TOGGLE_CHAT_INTEGRATION 调)。
/// 关闭时 effects 会静默取消直发(Discard)或优雅收尾草稿(StopChatDraft:
/// 提交在写笔画 + 完整裁决提示);GIF 槽不受影响。
fn recording_chat_integration_changed(state: &mut OverlayState, enabled: bool) {
    recording_feed(
        state,
        RecSlot::Chat,
        RecordingEvent::ChatIntegrationChanged { enabled },
    );
}

/// HUD 语义 → Windows 文案(键名/语言留 adapter, 与 macOS 差异保留到 D10)。
fn recording_hud_text(hud: &RecordingHud) -> String {
    match hud {
        RecordingHud::GifStartHint => "按住绘制, 松开生成 GIF".to_string(),
        RecordingHud::SendStartHint => "书写手写消息… 松开 Ctrl+Alt+3 发送".to_string(),
        RecordingHud::DraftStartHint => {
            "书写手写消息(草稿)… 松开 Ctrl+Alt+2 交给对方".to_string()
        }
        RecordingHud::DraftStartFailed => "手写通道开启失败".to_string(),
        RecordingHud::GifCopied => "GIF 已复制到剪贴板".to_string(),
        RecordingHud::GifNoStrokesOrExportFailed => "没有笔迹或导出失败".to_string(),
        RecordingHud::GifClipboardFailed => "GIF 复制失败".to_string(),
        RecordingHud::SendNoStrokes => "没有新手写内容".to_string(),
        RecordingHud::SendSucceeded { strokes } => format!("手写消息已发送 ({strokes} 笔)"),
        RecordingHud::SendFailed => "手写消息发送失败".to_string(),
        RecordingHud::DraftAccepted { strokes } => format!("对方已接收手写草稿 ({strokes} 笔)"),
        RecordingHud::DraftDeclined => "对方未采用这份手写草稿".to_string(),
        RecordingHud::DraftFailed { detail } => detail
            .clone()
            .filter(|d| !d.is_empty())
            .unwrap_or_else(|| "手写通道失败(未连接或中断)".to_string()),
    }
}

/// 按状态机下发的顺序执行 effects。
fn recording_exec_effects(state: &mut OverlayState, effects: Vec<RecordingEffect>) {
    for effect in effects {
        recording_exec_effect(state, effect);
    }
}

fn recording_exec_effect(state: &mut OverlayState, effect: RecordingEffect) {
    match effect {
        RecordingEffect::ShowHud(hud) => hud_notify(&recording_hud_text(&hud)),
        RecordingEffect::EncodeGif {
            start_stroke,
            end_stroke,
        } => recording_encode_gif(state, start_stroke, end_stroke),
        RecordingEffect::Clipboard => recording_copy_gif(state),
        RecordingEffect::StartChatDraft => {
            let (w, h) = (state.canvas.w, state.canvas.h);
            // 通道打开是同步 FFI(旧实现同款): 成功才提示(DraftStartHint),
            // 失败让状态机回空闲并提示 DraftStartFailed。
            if glaspen_core::export::glaspen2_ink_draft_start(w, h) != 0 {
                recording_feed(state, RecSlot::Chat, RecordingEvent::ChatDraftStarted);
            } else {
                recording_feed(state, RecSlot::Chat, RecordingEvent::ChatDraftStartFailed);
            }
        }
        RecordingEffect::StopChatDraft => recording_stop_draft(state),
        RecordingEffect::SendChatStrokes {
            start_stroke,
            end_stroke,
        } => recording_send_chat(state, start_stroke, end_stroke),
        RecordingEffect::Discard => {
            // 取消路径(总开关关闭取消直发/强制收尾):不编码/不发送。
            // 两端当前没有需要释放的 OS 资源, 只记日志。
            tracing::info!("录制会话取消: 丢弃窗口(不编码/不发送)");
        }
    }
}

/// GIF:后台回放渲染 + 编码;完成后把字节(或失败)PostMessage 回窗口线程,
/// 由窗口线程执行 Clipboard effect —— 严格两步, 顺序由状态机保证。
fn recording_encode_gif(state: &mut OverlayState, start_stroke: u32, end_stroke: u32) {
    // GIF 参数在松手时读(两平台差异 10: 录制中途改参数影响本次输出)。
    // EncodeGif 由 ReleasePressed 同步产生, 所以这里读到的就是松手瞬间的设置。
    let (fps, resolution, speed, end_mode) = gif_settings();
    // HWND 裸指针不能跨线程, 转 isize 传递
    let hwnd = state.canvas.hwnd.0 as isize;
    // 回放渲染 + GIF 压缩可能上百毫秒, 不阻塞消息循环;区间已由状态机钉死,
    // 新录制不会覆盖尚在编码的这一次。
    std::thread::spawn(move || {
        let mut len: i32 = 0;
        let ptr = glaspen_core::export::glaspen2_gif_record_end(
            start_stroke as i32,
            end_stroke as i32,
            fps,
            resolution,
            speed,
            end_mode,
            &mut len,
        );
        let hwnd = HWND(hwnd as *mut core::ffi::c_void);
        if ptr.is_null() || len <= 0 {
            if !ptr.is_null() {
                glaspen_core::export::glaspen2_free_rust_bytes(ptr, len);
            }
            unsafe {
                let _ = PostMessageW(Some(hwnd), WM_RECORD_ENCODE_FAIL, WPARAM(0), LPARAM(0));
            }
            return;
        }
        let gif = unsafe { std::slice::from_raw_parts(ptr, len as usize) }.to_vec();
        glaspen_core::export::glaspen2_free_rust_bytes(ptr, len);
        let payload = Box::into_raw(Box::new(gif));
        unsafe {
            if PostMessageW(
                Some(hwnd),
                WM_RECORD_ENCODE_OK,
                WPARAM(0),
                LPARAM(payload as isize),
            )
            .is_err()
            {
                // 窗口已销毁: 释放投递失败的盒子, 不泄漏
                drop(Box::from_raw(payload));
            }
        }
    });
}

/// GIF 字节写剪贴板(窗口线程;Clipboard effect 的落点, 保持旧剪贴板实现)。
fn recording_copy_gif(state: &mut OverlayState) {
    let bytes = std::mem::take(&mut state.pending_gif_bytes);
    let ok = !bytes.is_empty() && unsafe { copy_gif_bytes_to_clipboard(&bytes) };
    recording_feed(
        state,
        RecSlot::Gif,
        if ok {
            RecordingEvent::GifClipboardSucceeded
        } else {
            RecordingEvent::GifClipboardFailed
        },
    );
}

/// 草稿:后台 half-close 等 axum 裁决, 结果盒投递回窗口线程再喂
/// ChatDraftFinished(裁决映射单源 = DraftOutcome::from_ffi_code)。
fn recording_stop_draft(state: &mut OverlayState) {
    // 提交在写笔画(总开关关闭时的收尾也走这里, 修复旧 Windows 静默 stop);
    // finish 必须在窗口线程先跑完, 最后一笔才会经提交钩子进流。
    finish_active_stroke(state);
    let hwnd = state.canvas.hwnd.0 as isize;
    std::thread::spawn(move || {
        let code = glaspen_core::export::glaspen2_ink_draft_stop();
        // 失败原因在后台线程取好再投递, 避免与下一次会话竞争(macOS 同款)
        let ptr = glaspen_core::export::glaspen2_ink_draft_last_error();
        let detail = if ptr.is_null() {
            String::new()
        } else {
            unsafe { std::ffi::CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned()
        };
        let done = Box::into_raw(Box::new(DraftDone { code, detail }));
        unsafe {
            if PostMessageW(
                Some(HWND(hwnd as *mut core::ffi::c_void)),
                WM_RECORD_DRAFT_DONE,
                WPARAM(0),
                LPARAM(done as isize),
            )
            .is_err()
            {
                drop(Box::from_raw(done));
            }
        }
    });
}

/// 直发:后台把 [start,end) 作为一批聊天消息发送, 结果盒投递回窗口线程。
fn recording_send_chat(state: &mut OverlayState, start_stroke: u32, end_stroke: u32) {
    let hwnd = state.canvas.hwnd.0 as isize;
    std::thread::spawn(move || {
        let sent = glaspen_core::export::glaspen2_chat_send_strokes(
            start_stroke as i32,
            end_stroke as i32,
        );
        let done = Box::into_raw(Box::new(SendDone {
            strokes: (sent >= 0).then_some(sent as u32),
        }));
        unsafe {
            if PostMessageW(
                Some(HWND(hwnd as *mut core::ffi::c_void)),
                WM_RECORD_SEND_DONE,
                WPARAM(0),
                LPARAM(done as isize),
            )
            .is_err()
            {
                drop(Box::from_raw(done));
            }
        }
    });
}

/// 后台草稿裁决结果盒(窗口线程接管)。
struct DraftDone {
    code: i32,
    detail: String,
}

/// 后台直发结果盒(窗口线程接管)。
struct SendDone {
    strokes: Option<u32>,
}

/// 窗口线程:后台 GIF 编码成功 → 存字节并喂 GifEncodeSucceeded(其 effects
/// 第一步就是 Clipboard, 由窗口线程写剪贴板)。
fn recording_encode_done(state: &mut OverlayState, gif: Vec<u8>) {
    state.pending_gif_bytes = gif;
    recording_feed(state, RecSlot::Gif, RecordingEvent::GifEncodeSucceeded);
}

/// 窗口线程:后台 GIF 编码失败 → 喂 GifEncodeFailed(提示"没有笔迹或导出失败")。
fn recording_encode_failed(state: &mut OverlayState) {
    recording_feed(state, RecSlot::Gif, RecordingEvent::GifEncodeFailed);
}

/// 窗口线程:后台草稿裁决 → ChatDraftFinished(HUD 文案由状态机选)。
fn recording_draft_done(state: &mut OverlayState, done: DraftDone) {
    let detail = (!done.detail.is_empty()).then_some(done.detail);
    let outcome = DraftOutcome::from_ffi_code(done.code, detail);
    recording_feed(
        state,
        RecSlot::Chat,
        RecordingEvent::ChatDraftFinished(outcome),
    );
}

/// 窗口线程:后台直发结果 → ChatSendSucceeded/Failed。
fn recording_send_done(state: &mut OverlayState, done: SendDone) {
    let event = match done.strokes {
        Some(strokes) => RecordingEvent::ChatSendSucceeded { strokes },
        None => RecordingEvent::ChatSendFailed,
    };
    recording_feed(state, RecSlot::Chat, event);
}
