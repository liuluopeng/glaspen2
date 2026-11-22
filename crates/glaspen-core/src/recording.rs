//! 录制会话状态机(GIF / ⌘⌃2 草稿 / ⌘⌃3 直发;D8a 核心 + D8b 两端接线)。
//!
//! 三种"按住录制、松开结束"的会话在两端各写了一份状态与通知分派
//! (`docs/duplication-backlog.md` D8)。本模块把**状态 / 起止窗口 / 事件 /
//! 用户可见动作(effect)** 收敛成平台无关的唯一实现;两端 adapter 只执行
//! OS 侧动作(编码回放、剪贴板、HUD、gRPC/FFI 调用),不再各持一套会话标志位
//! 并在松手分支里各自决定提示文案。
//!
//! **D8b 已接线**:macOS 经 `export::glaspen2_recording_{init,event_json,reset,mode}`
//! 的 FFI 槽位驱动(0 = GIF,1 = 手写消息),Windows 在 `OverlayState` 里直接持
//! 两个 `RecordingMachine`;两端旧会话标志位与逐模式 HUD/裁决分支已删除,
//! `tests::adapters_drive_core_recording_machine` 闸门按文本扫描两端 adapter
//! (必须出现 core 入口/全部变体 token,禁止旧标志位指纹)。
//!
//! # 两端现状证据(D8a 读码时行号;D8b 接线后旧代码已删除,行号仅作历史)
//!
//! - macOS `src/macos/parts/run.m`:
//!   - GIF 开始/结束 `:1924-1932` / `:1938-1963`(异步编码 `:1948-1962`,
//!     剪贴板 helper `:1905-1920`);
//!   - ⌘⌃3 直发开始/结束 `:1967-1974` / `:1979-2001`;
//!   - ⌘⌃2 草稿开始/结束 `:2007-2020` / `:2025-2055`;
//!   - 事件 tap 键位 `:2198-2254`(R 抬起 `:2214-2219`、R 按下 `:2220-2225`、
//!     `3` 抬起 `:2226-2232`、`3` 按下 `:2233-2239`、`2` 抬起 `:2245-2248`、
//!     `2` 按下 `:2249-2254`);
//!   - 会话标志位 `src/macos/glaspen2.m:311-332`;手写消息总开关关闭的收尾
//!     `src/macos/parts/menu.m:958-972`。
//! - Windows `crates/glaspen-windows/src/overlay/actions.rs`:
//!   - GIF 开始/结束 `:811-820` / `:823-859`(结果码经 `WM_RECORD_DONE`
//!     回主线程,`wndproc.rs:183-191`);剪贴板 `:944-1032`;
//!   - 直发 `:867-874` / `:877-897`;草稿 `:900-912` / `:915-942`;
//!   - 键盘 Raw Input 抬起 `:1034-1082`;总开关关闭 `:180-212`;
//!   - 固定热键分发 `overlay/hotkeys.rs:716-740`、注册 `:290-306`。
//! - 会话状态字段:macOS `glaspen2.m:311-322`;Windows `overlay/state.rs:256-262`。
//! - core 侧语义入口(本层不重复实现 I/O):
//!   `export::glaspen2_gif_record_end`(`media.rs:1224-1282`,空区间/编码失败
//!   返回 NULL)、`export::glaspen2_chat_send_strokes`(`chat_glue.rs:48-87`,
//!   ≥0 = 发送条数,<0 = 失败)、`export::glaspen2_ink_draft_start`
//!   (`chat_glue.rs:131-183`,1 = 已开启,0 = 已有会话/未开启)、
//!   `export::glaspen2_ink_draft_stop`(`chat_glue.rs:336-367`,>0 接受 / 0 放弃 /
//!   <0 失败,原因经 `glaspen2_ink_draft_last_error` 取)。
//!
//! # 模型
//!
//! - 模式 [`RecordingMode`]:`Gif`(⌘⌃R / Ctrl+Alt+R)、`Draft`(⌘⌃2 / Ctrl+Alt+2)、
//!   `Send`(⌘⌃3 / Ctrl+Alt+3)。
//! - 状态 [`RecordingState`]:`Idle` 或
//!   `Recording { mode, started_at_strokes }`。GIF/直发在 key-down 时钉住
//!   起始笔画号(松手用 `[start, end)` 窗口);草稿没有下标窗口
//!   (`started_at_strokes = None`,笔迹经提交钩子实时推进流)。
//! - 事件 [`RecordingEvent`]:按键起止、总开关变化、异步结果回报、强制取消。
//! - 动作 [`RecordingEffect`]:adapter 必须按顺序执行的语义动作(HUD/编码/剪贴板/
//!   通道开合/直发/丢弃)。
//! - [`RecordingMachine::handle`] 纯函数:不碰 I/O、不读全局、确定性;同一事件
//!   序列永远得到同一结果。adapter 在**自己的线程**(两端都是主/消息循环线程)
//!   驱动状态机,后台结果投递回该线程后再作为事件喂回。
//!
//! # 两条建模决定(读两端代码后)
//!
//! 1. **一个 [`RecordingMachine`] 只承载一个会话**。两端实际用**互相独立的标志位**
//!    记录 GIF 与手写消息:macOS `g_gif_recording`(`glaspen2.m:313`)vs
//!    `g_msg_record_start` / `g_ink_draft_active`(`:318`/`:322`);Windows
//!    `gif_recording` / `msg_record_start` / `ink_draft_active`
//!    (`state.rs:256-262`),开始/结束分支各查各的标志位,互不排斥
//!    (`actions.rs:812`/`:868`/`:901`)。所以 GIF 与一份手写消息可以**同时录制**。
//!    为保持状态机显式,本层不把两个槽位塞进一个状态枚举;D8b 据此
//!    **为 GIF 与手写消息各持一个机器**(与两端原有独立标志位一一对应),
//!    手写消息的草稿/直发互斥由同一机器保证。同一机器里跨模式的
//!    `StartPressed`/`ReleasePressed` 一律忽略(见迁移表),不要把两种模式喂给
//!    同一个机器。
//! 2. **没有 Finishing 状态**。两端松手后立刻复位标志位并钉死笔画窗口,
//!    编码/发送在后台线程跑;窗口已按 `(start, end)` 传走,新一次录制可以
//!    立即开始,不会覆盖尚在编码的这一次(macOS `run.m:1934-1947` 注释、
//!    Windows `actions.rs:833-837`)。后台任务的在途状态由 adapter 自己持有
//!    (线程/消息),FSM 不跟踪;异步结果作为事件回报,且**不依赖当前状态**
//!    (窗口已钉死,结果晚到就照常提示 —— 两端现状)。
//!
//! # 不建模的部分(及原因)
//!
//! - **Timeout / Tick**:三种会话都没有超时(按住多久都行)。全仓唯一的 60s
//!   看门狗是"设置面板录制快捷键"时暂停全局热键的兜底
//!   (macOS `gl_set_hotkey_recording` / Windows `hotkeys.rs:549-594`
//!   `set_recording`),与录制会话无关,不进入本状态机。
//! - **NoStrokes 事件**:不需要单独事件 —— 松手事件已携带提交后的
//!   `stroke_count`,与状态里的起点比较即可判定空窗口(两端同样是 `end <= start`);
//!   空窗口由 FSM 直接下发 `ShowHud(..)`,不经过 adapter 的编码/发送调用。
//! - **SessionReset / PageChange / Clear**:两端翻页、清空、新建页都**不**取消
//!   录制(会话标志位只在上面的开始/结束/总开关关闭处读写,见证据行号);
//!   这里不发明行为。D8b 接线后确认没有"退出前统一收尾"的调用点,
//!   [`RecordingEvent::ForceStop`] 仍未接线(保留语义与测试)。
//! - **落笔提交(finish_active_stroke)**:两端都是"先提交在写笔画、再取
//!   笔画数"作为前置步骤(macOS `run.m:1927`/`:1946`/`:1970`/`:1982`/`:2029`,
//!   Windows `actions.rs:816`/`:830`/`:871`/`:883`/`:904`/`:920`)。它属于笔画
//!   边界/adapter,本层不重复表达;约定 **事件里的 `stroke_count` 必须是提交
//!   之后的计数**(否则窗口会漏掉正在写的那一笔)。
//! - **GIF 质量参数**(`gifFps`/`gifResolution`/`gifSpeed`/`gifEndMode`):
//!   两端在松手时从各自设置状态读取(macOS `run.m:1940-1943`,
//!   Windows `state.rs:114-181`),属于设置/adapter;本层只下发起止窗口。
//! - **用户文案**:HUD 用语义枚举 [`RecordingHud`],不出现中文/英文文案
//!   (文案与键名 `⌘⌃2` vs `Ctrl+Alt+2` 留在 adapter,等 D10 i18n 统一;
//!   见 `docs/platform-parity.md` 的 core 边界规则)。
//!
//! # 状态迁移表
//!
//! | 当前状态 | 事件 | 下一状态 | effects(按序执行) | 两端对应行为 |
//! |---|---|---|---|---|
//! | `Idle` | `StartPressed{Gif,n}` | `Recording{Gif,Some(n)}` | `ShowHud(GifStartHint)` | `run.m:1924-1932` / `actions.rs:811-820` |
//! | `Idle` | `StartPressed{Send,n}` 且总开关开 | `Recording{Send,Some(n)}` | `ShowHud(SendStartHint)` | `run.m:1967-1974` / `actions.rs:867-874` |
//! | `Idle` | `StartPressed{Draft,n}` 且总开关开 | `Recording{Draft,None}` | `StartChatDraft` | `run.m:2007-2020` / `actions.rs:900-912` |
//! | `Idle` | `StartPressed{Draft\|Send}` 且总开关关 | `Idle` | — | 热键不注册/不匹配:macOS `:2233`/`:2249` 的 `g_chat_integration` 门控、Windows `register_fixed` 只在开时注册 `hotkeys.rs:290-306` |
//! | `Recording{..}` | `StartPressed{..}`(同模式重复/跨模式) | 不变 | — | 两端各自的"已在录制"守卫:`run.m:1925`/`:1968`/`:2008`, `actions.rs:812`/`:868`/`:901` |
//! | `Recording{Draft}` | `ChatDraftStarted` | 不变 | `ShowHud(DraftStartHint)` | 通道打开成功才提示:`run.m:2011-2015` / `actions.rs:906-908` |
//! | `Recording{Draft}` | `ChatDraftStartFailed` | `Idle` | `ShowHud(DraftStartFailed)` | FFI 返回非 1(含"已有会话"):`run.m:2016-2019` / `actions.rs:909-911` |
//! | `Recording{Gif}` | `ReleasePressed{Gif,end}` 且 `end>start` | `Idle` | `EncodeGif{start,end}` | `run.m:1938-1963` / `actions.rs:823-859` |
//! | `Recording{Gif}` | `ReleasePressed{Gif,end}` 且 `end<=start` | `Idle` | `ShowHud(GifNoStrokesOrExportFailed)` | 两端空窗口/无字节同一条提示:`run.m:1958-1959` / `wndproc.rs:187` |
//! | `Recording{Send}` | `ReleasePressed{Send,end}` 且 `end>start` | `Idle` | `SendChatStrokes{start,end}` | `run.m:1979-2001` / `actions.rs:877-897` |
//! | `Recording{Send}` | `ReleasePressed{Send,end}` 且 `end<=start` | `Idle` | `ShowHud(SendNoStrokes)` | `run.m:1984-1986` / `actions.rs:885-887` |
//! | `Recording{Draft}` | `ReleasePressed{Draft,..}` | `Idle` | `StopChatDraft` | `run.m:2025-2055` / `actions.rs:914-942`(计数无意义) |
//! | `Recording{m}` | `ReleasePressed{m'}` 且 `m'!=m` | 不变 | — | 两端 key-up 分支只认自己的标志位(`run.m:2214-2254` / `actions.rs:1068-1081`) |
//! | `Idle` | `ReleasePressed{..}` | `Idle` | — | 松手时没有会话:两端条件不成立、直接放行(`run.m:2216` 等) |
//! | 任意 | `GifEncodeSucceeded` | 不变 | `Clipboard` | 编码出字节 → 写剪贴板(`run.m:1950-1956` / `actions.rs:839-848`) |
//! | 任意 | `GifEncodeFailed` | 不变 | `ShowHud(GifNoStrokesOrExportFailed)` | NULL/0 字节:`run.m:1957-1960` / Windows 结果码 2 `wndproc.rs:187` |
//! | 任意 | `GifClipboardSucceeded` | 不变 | `ShowHud(GifCopied)` | `run.m:1956` / Windows 结果码 1 `wndproc.rs:186` |
//! | 任意 | `GifClipboardFailed` | 不变 | `ShowHud(GifClipboardFailed)` | **仅 Windows** 有独立提示(结果码 0,`wndproc.rs:188`);macOS 剪贴板失败只 NSLog 仍提示成功(`run.m:1913-1916` + `:1956`) |
//! | 任意 | `ChatDraftFinished(outcome)` | 不变 | `ShowHud(..)`(按 outcome) | `run.m:2038-2052` / `actions.rs:932-940` |
//! | 任意 | `ChatSendSucceeded{strokes}` | 不变 | `ShowHud(SendSucceeded{strokes})` | `run.m:1991-1995` / `actions.rs:891-892` |
//! | 任意 | `ChatSendFailed` | 不变 | `ShowHud(SendFailed)` | `run.m:1996-1998` / `actions.rs:893-895` |
//! | `Recording{Draft}` | `ChatIntegrationChanged{enabled:false}` | `Idle` | `StopChatDraft` | macOS `menu.m:966` 走完整 `ink_draft_stop_async`(提交笔画 + 结果提示);Windows `actions.rs:201-206` 静默 stop(不提交、不提示)——差异见下 |
//! | `Recording{Send}` | `ChatIntegrationChanged{enabled:false}` | `Idle` | `Discard` | 两端都是静默复位 `msg_record_start = -1`(不发送):`menu.m:965` / `actions.rs:200` |
//! | `Recording{Gif}` / `Idle` | `ChatIntegrationChanged{..}` | 不变 | — | 总开关只管手写消息,GIF 固定热键不受影响(`hotkeys.rs:290-306` 只门控 2/3) |
//! | `Recording{Gif\|Send}` | `ForceStop` | `Idle` | `Discard` | 两端**没有**调用点(退出即终止进程;翻页/清空不取消),见"不建模";D8b 确认未接 |
//! | `Recording{Draft}` | `ForceStop` | `Idle` | `StopChatDraft` | 同上;语义同总开关关闭时的草稿收尾 |
//! | `Idle` | `ForceStop` | `Idle` | — | — |
//!
//! # 两端真实差异(只记录,不静默统一)
//!
//! 1. **按住制语义一致**:两端都是 key-down 开始、key-up 结束;抬起不校验修饰键
//!    (先松 Ctrl/⌘ 也不会卡住会话):macOS `run.m:2214-2248`,Windows Raw Input
//!    `actions.rs:1068-1081`。没有 toggle 模式。
//! 2. **总开关门控位置不同**:macOS 在事件 tap 里判断 `g_chat_integration` 后才
//!    消费组合(`run.m:2233`/`:2249`);Windows 干脆不注册 2/3 热键
//!    (`hotkeys.rs:294-306`),再加 dispatch 里的二道门(`:721-729`)。GIF 两端
//!    恒可用,且**停用涂鸦时也允许开始录制**(Windows 注释 `actions.rs:815`;
//!    macOS 键盘分支在 `!g_enabled` 放行逻辑之前,`:2286-2298` 只作用于非键盘
//!    事件),此时画不出笔迹,松手得到空窗口提示。
//! 3. **GIF 结果的粒度不同**:Windows 用 3 个结果码区分"成功 / 剪贴板失败 /
//!    没有笔迹或导出失败"(`wndproc.rs:183-190`);macOS 只有"有字节 / 没有字节"
//!    两路(`run.m:1953-1960`),剪贴板写临时文件失败只 NSLog
//!    (`run.m:1913-1915`),调用方仍提示"GIF 已复制到剪贴板"。所以
//!    `GifClipboardFailed` 目前只有 Windows 会报;D8b 保持 macOS 现状
//!    (剪贴板写失败只 NSLog, 仍回报 `GifClipboardSucceeded`)—— 本切片只修
//!    backlog 明列的两处不一致, 不改这个提示差异。
//! 4. **总开关关闭时的草稿收尾(原不一致, D8b 已修)**:macOS `menu.m:966` 调
//!    `ink_draft_stop_async()`,会先 `finish_active_stroke` 再 half-close,
//!    并按 axum 裁决提示结果;Windows `actions.rs:201-206` 只复位标志并后台
//!    stop,不提交在写笔画、丢弃结果、不提示。本层统一下发 `StopChatDraft`;
//!    D8b 把 Windows 对齐 macOS:先提交在写笔画, 再 half-close, 结果作为
//!    `ChatDraftFinished` 喂回状态机并按裁决提示(两端现在同一条路径)。
//! 5. **草稿开始失败的判定**:`glaspen2_ink_draft_start` 返回 0 也包含"已有会话"
//!    (`chat_glue.rs:135-138`),两端都把非 1 当"手写通道开启失败";本层同样把
//!    `ChatDraftStartFailed` 当成一次性失败,状态回 `Idle`。
//! 6. **直发结果映射**:core `glaspen2_chat_send_strokes` 返回 ≥0 = 接受条数,
//!    <0 = 失败;两端都按 `sent >= 0` 提示成功(带条数)。本层用
//!    `ChatSendSucceeded{strokes}` / `ChatSendFailed` 表达,不夹带负数语义。
//! 7. **草稿结果映射**:core `glaspen2_ink_draft_stop` >0 = 已发送(条数)、
//!    0 = 对方放弃、<0 = 失败(具体原因经 `last_error`);两端提示分支一致
//!    (`run.m:2038-2052` / `actions.rs:932-940`)。[`DraftOutcome::from_ffi_code`]
//!    把这条契约单源化。
//! 8. **异步投递线程不同**:macOS 编码/发送在后台队列、结果 `dispatch_async`
//!    回主线程;Windows 后台线程经 `PostMessageW(WM_RECORD_ENCODE_OK/FAIL,
//!    WM_RECORD_DRAFT_DONE, WM_RECORD_SEND_DONE)` 回窗口线程(D8b 把 GIF 拆成
//!    "后台编码 → 字节投递 → 窗口线程写剪贴板"两步, 顺序同本层迁移表)。
//!    本层要求所有 `handle` 都在
//!    拥有状态机的那条线程上调用(不是 `Sync`),后台结果先投递再喂事件。
//! 9. **HUD 呈现与文案不同**:macOS `show_notification` 画覆盖层通知;Windows
//!    `hud_notify` 是轻量 HUD。文案里键名也不同(`⌘⌃2` vs `Ctrl+Alt+2`),
//!    Windows 只有中文。本层只给 [`RecordingHud`] 语义,文案留 D10 统一。
//! 10. **GIF 参数读取时机**:两端都在松手时读当前设置(macOS 全局量
//!     `run.m:1940-1943`、Windows 原子量 `state.rs:121-129`),录制中途改参数
//!     会影响本次输出 —— 本层不碰,adapter 保持原语义。
//!
//! # 使用契约(D8b)
//!
//! 1. 开始/结束事件前,**先提交在写笔画**,再取 `glaspen2_stroke_count()` 作为
//!    `stroke_count`(两端现有顺序,见"不建模")。
//! 2. 按返回的 effects 顺序执行;`ShowHud` 的语义到平台文案的映射留在 adapter。
//! 3. 后台结果(编码/剪贴板/发送/草稿裁决)回到状态机线程后,先构造对应事件
//!    喂给 `handle`,再执行新 effects;不要跳过 `GifEncodeSucceeded -> Clipboard`
//!    这一步(顺序 = 先编码成功、再写剪贴板、最后提示)。
//! 4. GIF 与手写消息各持一个机器;总开关变化只喂手写消息机器;`ForceStop` 两个
//!    机器都要喂(如果 D8b 真的需要强制收尾)。
//! 5. 同一机器里跨模式的按键事件请忽略(两端是独立标志位,不应把 GIF 事件
//!    喂进手写消息机器,反之亦然)。
//! 6. macOS 经 `export` 的 JSON 编解码([`event_from_json`] / [`effects_to_json`])
//!    驱动槽位;事件/effects 字段名见 `export::glaspen2_recording_event_json`
//!    的文档(单行、永不 NULL、错误回 `{"error":...}`)。Windows 同进程直接用
//!    枚举。两端 adapter 的变体覆盖率由本文件 tests 的闸门守(漏实现即红)。

/// 录制模式:三种"按住录制、松开结束"的会话。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecordingMode {
    /// ⌘⌃R / Ctrl+Alt+R 按住:回放这段时间的笔迹生成 GIF,松手写进剪贴板。
    Gif,
    /// ⌘⌃2 / Ctrl+Alt+2 按住:草稿通道(DraftInk 实时上行,松手 half-close,
    /// 由 axum 决定发不发)。受手写消息总开关门控。
    Draft,
    /// ⌘⌃3 / Ctrl+Alt+3 按住:松手把窗口内笔迹作为一批聊天消息发出。
    /// 受手写消息总开关门控。
    Send,
}

impl RecordingMode {
    /// 稳定名字(日志/测试用)。
    pub const fn name(self) -> &'static str {
        match self {
            Self::Gif => "Gif",
            Self::Draft => "Draft",
            Self::Send => "Send",
        }
    }

    /// 是否手写消息会话(受 `chat_integration` 总开关门控;GIF 不受)。
    pub const fn is_chat(self) -> bool {
        matches!(self, Self::Draft | Self::Send)
    }
}

/// 录制状态。
///
/// 就绪与录制中二态;没有 `Finishing`(见模块文档"两条建模决定")。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RecordingState {
    /// 没有进行中的会话。
    #[default]
    Idle,
    /// 会话进行中。
    Recording {
        /// 会话模式(一个机器同一时刻只有一种模式)。
        mode: RecordingMode,
        /// 录制起点:key-down 时钉住的笔画数(GIF/直发用于 `[start, end)` 窗口);
        /// 草稿没有下标窗口,恒为 `None`。
        started_at_strokes: Option<u32>,
    },
}

impl RecordingState {
    /// 录制中的模式;空闲返回 `None`。
    pub const fn mode(self) -> Option<RecordingMode> {
        match self {
            Self::Idle => None,
            Self::Recording { mode, .. } => Some(mode),
        }
    }
}

/// 草稿通道收尾结果(⌘⌃2 松开后 axum 的裁决)。
///
/// 由 [`RecordingEvent::ChatDraftFinished`] 携带,对应 core
/// `glaspen2_ink_draft_stop` 的返回值契约(`export/chat_glue.rs:336-367`):
/// `>0` = 对方接受并已发送(条数),`0` = 对方放弃,`<0` = 通道失败
/// (具体原因来自 `glaspen2_ink_draft_last_error`)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DraftOutcome {
    /// 对方接受并发送了草稿(条数 = 实际发送的笔迹数)。
    Accepted {
        /// 发送的笔迹条数(两端提示里带这个数字)。
        strokes: u32,
    },
    /// 对方明确放弃这份草稿(提示"对方未采用")。
    Declined,
    /// 通道失败/中断;`detail` 是 `last_error` 的非空原因(有则原样展示,
    /// 无则用平台通用失败文案)。
    Failed {
        /// 平台可读的失败原因(空/None 时 adapter 用通用文案)。
        detail: Option<String>,
    },
}

impl DraftOutcome {
    /// 把 core FFI 返回值映射为语义结果(两端提示分支的单源版本)。
    ///
    /// 契约见 `chat_glue.rs:336-367`:正数 = 已发送条数,0 = 对方放弃,
    /// 负数 = 失败(原因另取 `glaspen2_ink_draft_last_error`)。
    /// 两端现在都是这个三分支(`run.m:2038-2052` / `actions.rs:932-940`)。
    pub fn from_ffi_code(code: i32, detail: Option<String>) -> Self {
        if code > 0 {
            Self::Accepted {
                strokes: code as u32,
            }
        } else if code == 0 {
            Self::Declined
        } else {
            Self::Failed { detail }
        }
    }
}

/// 一次录制会话中**用户可见**的 HUD/通知语义。
///
/// 只表达"该提示什么",不带文案:macOS 用 `show_notification`、Windows 用
/// `hud_notify`,且文案里的键名不同(`⌘⌃2` vs `Ctrl+Alt+2`)。文案留 adapter,
/// 等 D10 i18n 统一 —— 与 `crate::action_effect` / `crate::camera` 的边界一致。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordingHud {
    /// GIF 录制开始提示("按住绘制, 松开生成 GIF")。
    GifStartHint,
    /// 直发录制开始提示("书写手写消息… 松开 ⌘⌃3 发送")。
    SendStartHint,
    /// 草稿通道打开成功提示("书写手写消息(草稿)… 松开 ⌘⌃2 交给对方")。
    /// 只在 [`RecordingEvent::ChatDraftStarted`] 后出现(通道失败不显示)。
    DraftStartHint,
    /// 草稿通道打开失败("手写通道开启失败")。
    DraftStartFailed,
    /// GIF 已复制到剪贴板。
    GifCopied,
    /// GIF 没有笔迹或导出失败(两端同一句 "没有笔迹或导出失败";
    /// 空窗口在 FSM 直接判定,编码失败由 [`RecordingEvent::GifEncodeFailed`] 报告)。
    GifNoStrokesOrExportFailed,
    /// GIF 剪贴板写入失败("GIF 复制失败")。**目前只有 Windows 会显示**
    /// (macOS 剪贴板失败只 NSLog 且仍提示成功,见模块文档差异 3)。
    GifClipboardFailed,
    /// 直发没有新手写内容("没有新手写内容")。
    SendNoStrokes,
    /// 直发成功(带发送条数)。
    SendSucceeded {
        /// core `glaspen2_chat_send_strokes` 返回的接受条数。
        strokes: u32,
    },
    /// 直发失败("手写消息发送失败")。
    SendFailed,
    /// 草稿被对方接受(带条数)。
    DraftAccepted {
        /// axum 接受的笔迹条数。
        strokes: u32,
    },
    /// 草稿被对方放弃("对方未采用这份手写草稿")。
    DraftDeclined,
    /// 草稿通道失败;`detail` 有值时 adapter 原样展示原因,否则用通用文案。
    DraftFailed {
        /// `glaspen2_ink_draft_last_error` 的可读原因(可能为空)。
        detail: Option<String>,
    },
}

impl RecordingHud {
    /// 稳定名字(日志/测试用;D8b 的 adapter 覆盖闸门可按名字扫描)。
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::GifStartHint => "GifStartHint",
            Self::SendStartHint => "SendStartHint",
            Self::DraftStartHint => "DraftStartHint",
            Self::DraftStartFailed => "DraftStartFailed",
            Self::GifCopied => "GifCopied",
            Self::GifNoStrokesOrExportFailed => "GifNoStrokesOrExportFailed",
            Self::GifClipboardFailed => "GifClipboardFailed",
            Self::SendNoStrokes => "SendNoStrokes",
            Self::SendSucceeded { .. } => "SendSucceeded",
            Self::SendFailed => "SendFailed",
            Self::DraftAccepted { .. } => "DraftAccepted",
            Self::DraftDeclined => "DraftDeclined",
            Self::DraftFailed { .. } => "DraftFailed",
        }
    }
}

/// adapter 必须按顺序执行的语义动作。
///
/// 只有语义,不含数值/文案/平台句柄;具体怎么编码、怎么写剪贴板、怎么画 HUD
/// 由两端 adapter 实现(与 `actions.rs` / `menu.m` 的分工一致)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordingEffect {
    /// 显示一次 HUD/通知(语义见 [`RecordingHud`])。
    ShowHud(RecordingHud),
    /// GIF:把 `[start_stroke, end_stroke)` 的笔迹交给后台回放编码
    /// (`glaspen2_gif_record_end`),编码完成后回报
    /// [`RecordingEvent::GifEncodeSucceeded`] / [`RecordingEvent::GifEncodeFailed`]。
    EncodeGif {
        /// 窗口起点(key-down 时的笔画数,含端点)。
        start_stroke: u32,
        /// 窗口终点(key-up 提交后的笔画数,不含端点)。
        end_stroke: u32,
    },
    /// GIF:编码出的字节写入系统剪贴板(adapter 持有字节),写完回报
    /// [`RecordingEvent::GifClipboardSucceeded`] / [`RecordingEvent::GifClipboardFailed`]。
    Clipboard,
    /// 草稿:打开 DraftInk 通道(`glaspen2_ink_draft_start`),成功回报
    /// [`RecordingEvent::ChatDraftStarted`],失败回报
    /// [`RecordingEvent::ChatDraftStartFailed`]。
    StartChatDraft,
    /// 草稿:提交/关闭通道并等待裁决(`glaspen2_ink_draft_stop`,后台执行),
    /// 结果回报 [`RecordingEvent::ChatDraftFinished`]。
    StopChatDraft,
    /// 直发:把 `[start_stroke, end_stroke)` 作为一批聊天消息后台发送
    /// (`glaspen2_chat_send_strokes`),结果回报
    /// [`RecordingEvent::ChatSendSucceeded`] / [`RecordingEvent::ChatSendFailed`]。
    SendChatStrokes {
        /// 窗口起点(key-down 时的笔画数,含端点)。
        start_stroke: u32,
        /// 窗口终点(key-up 提交后的笔画数,不含端点)。
        end_stroke: u32,
    },
    /// 丢弃已钉住的窗口,不产生任何输出(不编码/不发送)。
    ///
    /// 两端当前没有需要释放的 OS 资源(GIF/直发只是"笔画下标窗口",没有采集器),
    /// 适配器可为空实现/只记日志;保留这个显式动作是为了让取消路径
    /// (总开关关闭、强制收尾)在两端分派表里与"忽略事件"区分开。
    Discard,
}

impl RecordingEffect {
    /// 稳定名字(日志/测试用;D8b 的 adapter 覆盖闸门可按名字扫描)。
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::ShowHud(_) => "ShowHud",
            Self::EncodeGif { .. } => "EncodeGif",
            Self::Clipboard => "Clipboard",
            Self::StartChatDraft => "StartChatDraft",
            Self::StopChatDraft => "StopChatDraft",
            Self::SendChatStrokes { .. } => "SendChatStrokes",
            Self::Discard => "Discard",
        }
    }
}

/// 喂给状态机的输入。
///
/// 按键事件来自两端热键入口;结果类事件由 adapter 在后台工作完成后
/// 投递回状态机线程再喂入;`ChatIntegrationChanged` 对应手写消息总开关
/// (`gl_settings_set_chat_integration` / `CMD_TOGGLE_CHAT_INTEGRATION`)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordingEvent {
    /// 热键按下(按住录制的起点)。
    StartPressed {
        /// 要开始的录制模式。
        mode: RecordingMode,
        /// **提交在写笔画之后**的笔画数;GIF/直发把它钉成窗口起点,
        /// 草稿忽略(没有下标窗口)。
        stroke_count: u32,
    },
    /// 热键抬起(按住录制的终点)。
    ReleasePressed {
        /// 抬起对应的录制模式(与开始不一致时忽略,见迁移表)。
        mode: RecordingMode,
        /// **提交在写笔画之后**的笔画数;GIF/直发用它作为窗口终点,
        /// 草稿忽略。
        stroke_count: u32,
    },
    /// 手写消息总开关变化。关闭时静默取消进行中的直发、优雅收尾进行中的草稿;
    /// GIF 不受影响。打开不产生动作。
    ChatIntegrationChanged {
        /// 新的开关状态。
        enabled: bool,
    },
    /// GIF 编码成功(adapter 已拿到字节,准备写剪贴板)。
    GifEncodeSucceeded,
    /// GIF 编码失败(非空窗口但 core 返回 NULL/0 字节)。
    GifEncodeFailed,
    /// GIF 字节已写入剪贴板。
    GifClipboardSucceeded,
    /// GIF 剪贴板写入失败(目前只有 Windows 会回报,见模块文档差异 3)。
    GifClipboardFailed,
    /// 草稿通道打开成功。
    ChatDraftStarted,
    /// 草稿通道打开失败(FFI 返回非 1,含"已有会话")。
    ChatDraftStartFailed,
    /// 草稿通道收尾结果(axum 裁决)。
    ChatDraftFinished(DraftOutcome),
    /// 直发成功(带上限内的接受条数)。
    ChatSendSucceeded {
        /// core 返回的接受条数(≥0)。
        strokes: u32,
    },
    /// 直发失败。
    ChatSendFailed,
    /// 强制取消当前会话:不编码/不发送;草稿仍会优雅关闭通道。
    ///
    /// 当前两端**没有调用点**(退出即终止进程,翻页/清空不取消录制);
    /// D8b 接线时确认不需要,保留语义与测试,语义 = 总开关关闭的取消路径。
    ForceStop,
}

/// 录制会话状态机(纯逻辑,无 I/O)。
///
/// 见模块文档的状态迁移表与使用契约。`chat_integration` 只影响手写消息模式的
/// 开始门控与总开关关闭时的取消;GIF 始终可用。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RecordingMachine {
    state: RecordingState,
    /// 手写消息总开关的镜像(两端默认关;D8b 初始化时传当前设置)。
    chat_integration: bool,
}

impl RecordingMachine {
    /// 新建状态机;`chat_integration` = 当前手写消息总开关。
    pub const fn new(chat_integration: bool) -> Self {
        Self {
            state: RecordingState::Idle,
            chat_integration,
        }
    }

    /// 当前状态。
    pub const fn state(&self) -> RecordingState {
        self.state
    }

    /// 当前手写消息总开关镜像。
    pub const fn chat_integration(&self) -> bool {
        self.chat_integration
    }

    /// 是否空闲(可以开始新会话)。
    pub const fn is_idle(&self) -> bool {
        matches!(self.state, RecordingState::Idle)
    }

    /// 处理一个事件,返回 adapter 必须按序执行的 effects。
    ///
    /// 纯函数:不读全局、不碰 I/O、确定性;同一 (状态, 事件) 永远同一结果。
    pub fn handle(&mut self, event: RecordingEvent) -> Vec<RecordingEffect> {
        match event {
            RecordingEvent::StartPressed { mode, stroke_count } => {
                self.on_start_pressed(mode, stroke_count)
            }
            RecordingEvent::ReleasePressed { mode, stroke_count } => {
                self.on_release_pressed(mode, stroke_count)
            }
            RecordingEvent::ChatIntegrationChanged { enabled } => {
                self.on_chat_integration_changed(enabled)
            }
            // 结果事件与会话状态解耦:窗口在松手时已经钉死,后台结果可能晚于
            // 下一次录制(两端都会照常提示,`run.m:1952-1961` / `wndproc.rs:183-190`)
            RecordingEvent::GifEncodeSucceeded => vec![RecordingEffect::Clipboard],
            RecordingEvent::GifEncodeFailed => vec![RecordingEffect::ShowHud(
                RecordingHud::GifNoStrokesOrExportFailed,
            )],
            RecordingEvent::GifClipboardSucceeded => {
                vec![RecordingEffect::ShowHud(RecordingHud::GifCopied)]
            }
            RecordingEvent::GifClipboardFailed => {
                vec![RecordingEffect::ShowHud(RecordingHud::GifClipboardFailed)]
            }
            // 草稿开始类结果只对"正在等通道打开"的会话有效(防御陈旧事件)。
            RecordingEvent::ChatDraftStarted => {
                if self.state.mode() == Some(RecordingMode::Draft) {
                    vec![RecordingEffect::ShowHud(RecordingHud::DraftStartHint)]
                } else {
                    Vec::new()
                }
            }
            RecordingEvent::ChatDraftStartFailed => {
                if self.state.mode() == Some(RecordingMode::Draft) {
                    self.state = RecordingState::Idle;
                    vec![RecordingEffect::ShowHud(RecordingHud::DraftStartFailed)]
                } else {
                    Vec::new()
                }
            }
            RecordingEvent::ChatDraftFinished(outcome) => {
                vec![RecordingEffect::ShowHud(hud_for_draft_outcome(outcome))]
            }
            RecordingEvent::ChatSendSucceeded { strokes } => {
                vec![RecordingEffect::ShowHud(RecordingHud::SendSucceeded {
                    strokes,
                })]
            }
            RecordingEvent::ChatSendFailed => {
                vec![RecordingEffect::ShowHud(RecordingHud::SendFailed)]
            }
            RecordingEvent::ForceStop => self.on_force_stop(),
        }
    }

    /// key-down:进入录制状态并给出开始提示(草稿先开通道,成功后才提示)。
    fn on_start_pressed(&mut self, mode: RecordingMode, stroke_count: u32) -> Vec<RecordingEffect> {
        // 已在录制:同模式重复按下(键长按 repeat)与跨模式按下都忽略。
        // 两端守卫:run.m:1925/1968/2008,actions.rs:812/868/901。
        if !self.is_idle() {
            return Vec::new();
        }
        // 手写消息总开关关闭:两端热键根本不生效(事件 tap 门控 / 不注册热键),
        // 这里防御性忽略(不再弹任何提示)。
        if mode.is_chat() && !self.chat_integration {
            return Vec::new();
        }
        self.state = RecordingState::Recording {
            mode,
            started_at_strokes: if mode == RecordingMode::Draft {
                None
            } else {
                Some(stroke_count)
            },
        };
        match mode {
            RecordingMode::Gif => vec![RecordingEffect::ShowHud(RecordingHud::GifStartHint)],
            RecordingMode::Send => vec![RecordingEffect::ShowHud(RecordingHud::SendStartHint)],
            // 草稿提示要等通道打开成功(ChatDraftStarted),失败时只提示失败。
            RecordingMode::Draft => vec![RecordingEffect::StartChatDraft],
        }
    }

    /// key-up:结束会话并把窗口/通道收尾动作交给 adapter。
    fn on_release_pressed(
        &mut self,
        mode: RecordingMode,
        stroke_count: u32,
    ) -> Vec<RecordingEffect> {
        let RecordingState::Recording {
            mode: active_mode,
            started_at_strokes,
        } = self.state
        else {
            // 松手而没有会话:两端 key-up 分支条件不成立,事件放行/忽略。
            return Vec::new();
        };
        if active_mode != mode {
            // 模式不匹配:两端 key-up 各自只认自己的标志位(run.m:2214-2254 /
            // actions.rs:1068-1081),当前会话继续。
            return Vec::new();
        }
        self.state = RecordingState::Idle;
        match active_mode {
            RecordingMode::Gif => match started_at_strokes {
                Some(start) if stroke_count > start => vec![RecordingEffect::EncodeGif {
                    start_stroke: start,
                    end_stroke: stroke_count,
                }],
                // 空窗口(含翻页/清空导致笔画数变小):两端都是"没有笔迹或导出失败"。
                _ => vec![RecordingEffect::ShowHud(
                    RecordingHud::GifNoStrokesOrExportFailed,
                )],
            },
            RecordingMode::Send => match started_at_strokes {
                Some(start) if stroke_count > start => vec![RecordingEffect::SendChatStrokes {
                    start_stroke: start,
                    end_stroke: stroke_count,
                }],
                _ => vec![RecordingEffect::ShowHud(RecordingHud::SendNoStrokes)],
            },
            // 草稿:不看计数(草稿按提交钩子实时推进),只关闭通道等裁决。
            RecordingMode::Draft => vec![RecordingEffect::StopChatDraft],
        }
    }

    /// 手写消息总开关变化:关闭时取消手写消息会话,GIF 不受影响。
    fn on_chat_integration_changed(&mut self, enabled: bool) -> Vec<RecordingEffect> {
        self.chat_integration = enabled;
        if enabled {
            return Vec::new();
        }
        match self.state {
            RecordingState::Recording {
                mode: RecordingMode::Draft,
                ..
            } => {
                self.state = RecordingState::Idle;
                // macOS 走完整 stop(提交 + 结果提示);Windows 静默 stop,
                // 差异见模块文档 4。
                vec![RecordingEffect::StopChatDraft]
            }
            RecordingState::Recording {
                mode: RecordingMode::Send,
                ..
            } => {
                self.state = RecordingState::Idle;
                // 两端都静默复位,不发送;Discard 让取消在分派表里显式可见。
                vec![RecordingEffect::Discard]
            }
            RecordingState::Recording {
                mode: RecordingMode::Gif,
                ..
            }
            | RecordingState::Idle => Vec::new(),
        }
    }

    /// 强制取消:不产出 GIF/消息;草稿仍然优雅关流。
    fn on_force_stop(&mut self) -> Vec<RecordingEffect> {
        match self.state {
            RecordingState::Recording {
                mode: RecordingMode::Draft,
                ..
            } => {
                self.state = RecordingState::Idle;
                vec![RecordingEffect::StopChatDraft]
            }
            RecordingState::Recording { .. } => {
                self.state = RecordingState::Idle;
                vec![RecordingEffect::Discard]
            }
            RecordingState::Idle => Vec::new(),
        }
    }
}

/// 草稿结果 → HUD 语义(两端提示分支的单源版本)。
fn hud_for_draft_outcome(outcome: DraftOutcome) -> RecordingHud {
    match outcome {
        DraftOutcome::Accepted { strokes } => RecordingHud::DraftAccepted { strokes },
        DraftOutcome::Declined => RecordingHud::DraftDeclined,
        DraftOutcome::Failed { detail } => RecordingHud::DraftFailed { detail },
    }
}

// ── adapter JSON 编解码(D8b): macOS ObjC 不能持 Rust 状态机/枚举 ──
//
// ObjC 手工拼事件 JSON 喂进 `export::glaspen2_recording_event_json`, 拿到
// effects JSON 数组后按 `kind` 分派执行; 这里的字段名就是两端的接线契约
// (Windows 同进程直接用枚举, 不经 JSON, 但事件/动作语义与本层一致)。
//
// 事件形状(单行; `detail` 可缺省/为空):
//   {"type":"StartPressed","mode":"Gif","stroke_count":3}
//   {"type":"ReleasePressed","mode":"Draft","stroke_count":7}
//   {"type":"ChatIntegrationChanged","enabled":false}
//   {"type":"GifEncodeSucceeded"} / GifEncodeFailed /
//   GifClipboardSucceeded / GifClipboardFailed / ChatDraftStarted /
//   ChatDraftStartFailed / ChatSendFailed / ForceStop   (无载荷)
//   {"type":"ChatDraftFinished","code":2,"detail":"..."}   // 草稿裁决, 经
//       [DraftOutcome::from_ffi_code] 单源映射(正数/0/负数)
//   {"type":"ChatSendSucceeded","strokes":2}
//
// effect 形状(`kind` = [RecordingEffect::variant_name]):
//   {"kind":"ShowHud","hud":"GifStartHint"}
//   {"kind":"ShowHud","hud":"SendSucceeded","strokes":2}
//   {"kind":"ShowHud","hud":"DraftFailed","detail":"..."}   // detail 可能缺省
//   {"kind":"EncodeGif","start_stroke":3,"end_stroke":8}
//   {"kind":"Clipboard"} / {"kind":"StartChatDraft"} /
//   {"kind":"StopChatDraft"} / {"kind":"Discard"}
//   {"kind":"SendChatStrokes","start_stroke":3,"end_stroke":8}

/// 模式名 → [RecordingMode](与 [RecordingMode::name] 互逆)。
pub fn mode_from_name(name: &str) -> Option<RecordingMode> {
    match name {
        "Gif" => Some(RecordingMode::Gif),
        "Draft" => Some(RecordingMode::Draft),
        "Send" => Some(RecordingMode::Send),
        _ => None,
    }
}

/// 解析 adapter 喂进来的事件 JSON(契约见本节顶部注释)。
///
/// 只接受已知 `type`; 缺字段/未知枚举名/类型不符一律返回可读错误 —— FFI
/// 边界把它包成 `{"error": ...}`, 不让 ObjC 侧崩溃或静默吞事件。
pub fn event_from_json(text: &str) -> Result<RecordingEvent, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("录制事件 JSON 不合法: {e}: {text}"))?;
    let obj = value
        .as_object()
        .ok_or_else(|| format!("录制事件必须是 JSON 对象: {text}"))?;
    let ty = obj
        .get("type")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("录制事件缺少 type: {text}"))?;

    // 事件里的 stroke_count 是"提交在写笔画之后"的计数(见模块文档使用契约 1)。
    let stroke_count = || -> Result<u32, String> {
        let n = obj
            .get("stroke_count")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| format!("录制事件 {ty} 缺少 stroke_count: {text}"))?;
        u32::try_from(n).map_err(|_| format!("录制事件 {ty} 的 stroke_count 越界: {n}"))
    };
    let mode = || -> Result<RecordingMode, String> {
        let name = obj
            .get("mode")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("录制事件 {ty} 缺少 mode: {text}"))?;
        mode_from_name(name).ok_or_else(|| format!("录制事件 {ty} 的未知 mode {name:?}: {text}"))
    };
    match ty {
        "StartPressed" => Ok(RecordingEvent::StartPressed {
            mode: mode()?,
            stroke_count: stroke_count()?,
        }),
        "ReleasePressed" => Ok(RecordingEvent::ReleasePressed {
            mode: mode()?,
            stroke_count: stroke_count()?,
        }),
        "ChatIntegrationChanged" => Ok(RecordingEvent::ChatIntegrationChanged {
            enabled: obj
                .get("enabled")
                .and_then(serde_json::Value::as_bool)
                .ok_or_else(|| format!("录制事件 ChatIntegrationChanged 缺少 enabled: {text}"))?,
        }),
        "GifEncodeSucceeded" => Ok(RecordingEvent::GifEncodeSucceeded),
        "GifEncodeFailed" => Ok(RecordingEvent::GifEncodeFailed),
        "GifClipboardSucceeded" => Ok(RecordingEvent::GifClipboardSucceeded),
        "GifClipboardFailed" => Ok(RecordingEvent::GifClipboardFailed),
        "ChatDraftStarted" => Ok(RecordingEvent::ChatDraftStarted),
        "ChatDraftStartFailed" => Ok(RecordingEvent::ChatDraftStartFailed),
        "ChatDraftFinished" => {
            // 裁决契约单源: code = glaspen2_ink_draft_stop 返回值,
            // detail = glaspen2_ink_draft_last_error(可为空)。
            let code = obj
                .get("code")
                .and_then(serde_json::Value::as_i64)
                .ok_or_else(|| format!("录制事件 ChatDraftFinished 缺少 code: {text}"))?;
            let code = i32::try_from(code)
                .map_err(|_| format!("录制事件 ChatDraftFinished 的 code 越界: {code}"))?;
            let detail = obj
                .get("detail")
                .and_then(serde_json::Value::as_str)
                .filter(|d| !d.is_empty())
                .map(str::to_string);
            Ok(RecordingEvent::ChatDraftFinished(
                DraftOutcome::from_ffi_code(code, detail),
            ))
        }
        "ChatSendSucceeded" => {
            let n = obj
                .get("strokes")
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| format!("录制事件 ChatSendSucceeded 缺少 strokes: {text}"))?;
            let strokes = u32::try_from(n)
                .map_err(|_| format!("录制事件 ChatSendSucceeded 的 strokes 越界: {n}"))?;
            Ok(RecordingEvent::ChatSendSucceeded { strokes })
        }
        "ChatSendFailed" => Ok(RecordingEvent::ChatSendFailed),
        "ForceStop" => Ok(RecordingEvent::ForceStop),
        other => Err(format!("录制事件未知 type {other:?}: {text}")),
    }
}

/// HUD 语义 → JSON(带 `hud` 变体名; 载荷写成平铺字段)。
fn hud_to_json(hud: &RecordingHud) -> serde_json::Value {
    let mut value = serde_json::json!({ "kind": "ShowHud", "hud": hud.variant_name() });
    match hud {
        RecordingHud::SendSucceeded { strokes } | RecordingHud::DraftAccepted { strokes } => {
            value["strokes"] = serde_json::json!(strokes);
        }
        RecordingHud::DraftFailed {
            detail: Some(detail),
        } => {
            value["detail"] = serde_json::json!(detail);
        }
        _ => {}
    }
    value
}

/// 单个 effect → JSON(载荷字段名即 ObjC 解析契约)。
pub fn effect_to_json(effect: &RecordingEffect) -> serde_json::Value {
    match effect {
        RecordingEffect::ShowHud(hud) => hud_to_json(hud),
        RecordingEffect::EncodeGif {
            start_stroke,
            end_stroke,
        } => serde_json::json!({
            "kind": effect.variant_name(),
            "start_stroke": start_stroke,
            "end_stroke": end_stroke,
        }),
        RecordingEffect::SendChatStrokes {
            start_stroke,
            end_stroke,
        } => serde_json::json!({
            "kind": effect.variant_name(),
            "start_stroke": start_stroke,
            "end_stroke": end_stroke,
        }),
        RecordingEffect::Clipboard
        | RecordingEffect::StartChatDraft
        | RecordingEffect::StopChatDraft
        | RecordingEffect::Discard => {
            serde_json::json!({ "kind": effect.variant_name() })
        }
    }
}

/// effects 列表 → 单行 JSON 数组(ObjC 用 NSJSONSerialization 解析)。
pub fn effects_to_json(effects: &[RecordingEffect]) -> String {
    serde_json::Value::Array(effects.iter().map(effect_to_json).collect()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use RecordingEffect as E;
    use RecordingHud as H;
    use RecordingMode::{Draft, Gif, Send};

    fn start(mode: RecordingMode, stroke_count: u32) -> RecordingEvent {
        RecordingEvent::StartPressed { mode, stroke_count }
    }

    fn release(mode: RecordingMode, stroke_count: u32) -> RecordingEvent {
        RecordingEvent::ReleasePressed { mode, stroke_count }
    }

    fn hud(hud: RecordingHud) -> Vec<RecordingEffect> {
        vec![E::ShowHud(hud)]
    }

    /// 一次 (初始状态, 事件) → (effects, 新状态) 的表驱动断言。
    struct Case {
        name: &'static str,
        chat_integration: bool,
        setup: Vec<RecordingEvent>,
        event: RecordingEvent,
        effects: Vec<RecordingEffect>,
        state: RecordingState,
    }

    fn run_cases(cases: Vec<Case>) {
        for case in cases {
            let mut machine = RecordingMachine::new(case.chat_integration);
            for step in case.setup {
                let _ = machine.handle(step);
            }
            let effects = machine.handle(case.event);
            assert_eq!(effects, case.effects, "用例 {}: effects 不符", case.name);
            assert_eq!(machine.state(), case.state, "用例 {}: 状态不符", case.name);
        }
    }

    // ── 主迁移表:三种模式起止/重复/模式不匹配/空窗口/门控/取消 ──

    #[test]
    fn transition_table() {
        run_cases(vec![
            Case {
                name: "gif 开始",
                chat_integration: false,
                setup: vec![],
                event: start(Gif, 3),
                effects: hud(H::GifStartHint),
                state: RecordingState::Recording {
                    mode: Gif,
                    started_at_strokes: Some(3),
                },
            },
            Case {
                name: "gif 正常结束",
                chat_integration: false,
                setup: vec![start(Gif, 3)],
                event: release(Gif, 8),
                effects: vec![E::EncodeGif {
                    start_stroke: 3,
                    end_stroke: 8,
                }],
                state: RecordingState::Idle,
            },
            Case {
                name: "gif 空窗口(等于起点)",
                chat_integration: false,
                setup: vec![start(Gif, 3)],
                event: release(Gif, 3),
                effects: hud(H::GifNoStrokesOrExportFailed),
                state: RecordingState::Idle,
            },
            Case {
                name: "gif 空窗口(清页后计数变小)",
                chat_integration: false,
                setup: vec![start(Gif, 8)],
                event: release(Gif, 3),
                effects: hud(H::GifNoStrokesOrExportFailed),
                state: RecordingState::Idle,
            },
            Case {
                name: "gif 重复按下被忽略(键 repeat)",
                chat_integration: false,
                setup: vec![start(Gif, 3)],
                event: start(Gif, 9),
                effects: vec![],
                state: RecordingState::Recording {
                    mode: Gif,
                    started_at_strokes: Some(3),
                },
            },
            Case {
                name: "gif 录制中收到其他模式抬起被忽略",
                chat_integration: true,
                setup: vec![start(Gif, 3)],
                event: release(Send, 9),
                effects: vec![],
                state: RecordingState::Recording {
                    mode: Gif,
                    started_at_strokes: Some(3),
                },
            },
            Case {
                name: "直发开始(总开关开)",
                chat_integration: true,
                setup: vec![],
                event: start(Send, 2),
                effects: hud(H::SendStartHint),
                state: RecordingState::Recording {
                    mode: Send,
                    started_at_strokes: Some(2),
                },
            },
            Case {
                name: "直发开始(总开关关,热键不生效)",
                chat_integration: false,
                setup: vec![],
                event: start(Send, 2),
                effects: vec![],
                state: RecordingState::Idle,
            },
            Case {
                name: "直发正常结束",
                chat_integration: true,
                setup: vec![start(Send, 2)],
                event: release(Send, 5),
                effects: vec![E::SendChatStrokes {
                    start_stroke: 2,
                    end_stroke: 5,
                }],
                state: RecordingState::Idle,
            },
            Case {
                name: "直发空窗口",
                chat_integration: true,
                setup: vec![start(Send, 2)],
                event: release(Send, 2),
                effects: hud(H::SendNoStrokes),
                state: RecordingState::Idle,
            },
            Case {
                name: "草稿开始(总开关开)",
                chat_integration: true,
                setup: vec![],
                event: start(Draft, 0),
                effects: vec![E::StartChatDraft],
                state: RecordingState::Recording {
                    mode: Draft,
                    started_at_strokes: None,
                },
            },
            Case {
                name: "草稿开始(总开关关,热键不生效)",
                chat_integration: false,
                setup: vec![],
                event: start(Draft, 0),
                effects: vec![],
                state: RecordingState::Idle,
            },
            Case {
                name: "草稿通道打开成功才提示",
                chat_integration: true,
                setup: vec![start(Draft, 0)],
                event: RecordingEvent::ChatDraftStarted,
                effects: hud(H::DraftStartHint),
                state: RecordingState::Recording {
                    mode: Draft,
                    started_at_strokes: None,
                },
            },
            Case {
                name: "草稿通道打开失败回空闲并提示",
                chat_integration: true,
                setup: vec![start(Draft, 0)],
                event: RecordingEvent::ChatDraftStartFailed,
                effects: hud(H::DraftStartFailed),
                state: RecordingState::Idle,
            },
            Case {
                name: "陈旧的草稿打开失败事件被忽略",
                chat_integration: true,
                setup: vec![],
                event: RecordingEvent::ChatDraftStartFailed,
                effects: vec![],
                state: RecordingState::Idle,
            },
            Case {
                name: "草稿结束(计数无意义)",
                chat_integration: true,
                setup: vec![start(Draft, 0), RecordingEvent::ChatDraftStarted],
                event: release(Draft, 12345),
                effects: vec![E::StopChatDraft],
                state: RecordingState::Idle,
            },
            Case {
                name: "草稿录制中开始直发被忽略(互斥)",
                chat_integration: true,
                setup: vec![start(Draft, 0)],
                event: start(Send, 9),
                effects: vec![],
                state: RecordingState::Recording {
                    mode: Draft,
                    started_at_strokes: None,
                },
            },
            Case {
                name: "没有会话时松手被忽略",
                chat_integration: true,
                setup: vec![],
                event: release(Gif, 3),
                effects: vec![],
                state: RecordingState::Idle,
            },
            // 总开关关闭 = 取消手写消息会话;GIF 不受影响。
            Case {
                name: "总开关关闭取消直发(静默不发送)",
                chat_integration: true,
                setup: vec![start(Send, 1)],
                event: RecordingEvent::ChatIntegrationChanged { enabled: false },
                effects: vec![E::Discard],
                state: RecordingState::Idle,
            },
            Case {
                name: "总开关关闭收尾草稿通道",
                chat_integration: true,
                setup: vec![start(Draft, 0), RecordingEvent::ChatDraftStarted],
                event: RecordingEvent::ChatIntegrationChanged { enabled: false },
                effects: vec![E::StopChatDraft],
                state: RecordingState::Idle,
            },
            Case {
                name: "总开关关闭不影响 GIF 录制",
                chat_integration: false,
                setup: vec![start(Gif, 4)],
                event: RecordingEvent::ChatIntegrationChanged { enabled: false },
                effects: vec![],
                state: RecordingState::Recording {
                    mode: Gif,
                    started_at_strokes: Some(4),
                },
            },
            Case {
                name: "总开关打开不产生动作,之后可开始直发",
                chat_integration: false,
                setup: vec![
                    RecordingEvent::ChatIntegrationChanged { enabled: true },
                    start(Send, 1),
                ],
                event: RecordingEvent::ChatIntegrationChanged { enabled: true },
                effects: vec![],
                state: RecordingState::Recording {
                    mode: Send,
                    started_at_strokes: Some(1),
                },
            },
            // 强制取消(当前两端无调用点,见模块文档)。
            Case {
                name: "强制取消 GIF:丢弃窗口不编码",
                chat_integration: false,
                setup: vec![start(Gif, 1)],
                event: RecordingEvent::ForceStop,
                effects: vec![E::Discard],
                state: RecordingState::Idle,
            },
            Case {
                name: "强制取消直发:丢弃窗口不发送",
                chat_integration: true,
                setup: vec![start(Send, 1)],
                event: RecordingEvent::ForceStop,
                effects: vec![E::Discard],
                state: RecordingState::Idle,
            },
            Case {
                name: "强制取消草稿:优雅关流",
                chat_integration: true,
                setup: vec![start(Draft, 0)],
                event: RecordingEvent::ForceStop,
                effects: vec![E::StopChatDraft],
                state: RecordingState::Idle,
            },
            Case {
                name: "空闲时强制取消无动作",
                chat_integration: true,
                setup: vec![],
                event: RecordingEvent::ForceStop,
                effects: vec![],
                state: RecordingState::Idle,
            },
            // 异步结果 → 剪贴板/HUD;不依赖当前会话状态。
            Case {
                name: "GIF 编码成功 → 写剪贴板",
                chat_integration: false,
                setup: vec![],
                event: RecordingEvent::GifEncodeSucceeded,
                effects: vec![E::Clipboard],
                state: RecordingState::Idle,
            },
            Case {
                name: "GIF 编码成功不打扰进行中的新录制",
                chat_integration: false,
                setup: vec![start(Gif, 5)],
                event: RecordingEvent::GifEncodeSucceeded,
                effects: vec![E::Clipboard],
                state: RecordingState::Recording {
                    mode: Gif,
                    started_at_strokes: Some(5),
                },
            },
            Case {
                name: "GIF 编码失败提示(与空窗口同一条)",
                chat_integration: false,
                setup: vec![],
                event: RecordingEvent::GifEncodeFailed,
                effects: hud(H::GifNoStrokesOrExportFailed),
                state: RecordingState::Idle,
            },
            Case {
                name: "GIF 剪贴板成功提示",
                chat_integration: false,
                setup: vec![],
                event: RecordingEvent::GifClipboardSucceeded,
                effects: hud(H::GifCopied),
                state: RecordingState::Idle,
            },
            Case {
                name: "GIF 剪贴板失败提示(Windows 专属)",
                chat_integration: false,
                setup: vec![],
                event: RecordingEvent::GifClipboardFailed,
                effects: hud(H::GifClipboardFailed),
                state: RecordingState::Idle,
            },
            Case {
                name: "直发成功提示带条数",
                chat_integration: true,
                setup: vec![],
                event: RecordingEvent::ChatSendSucceeded { strokes: 3 },
                effects: hud(H::SendSucceeded { strokes: 3 }),
                state: RecordingState::Idle,
            },
            Case {
                name: "直发失败提示",
                chat_integration: true,
                setup: vec![],
                event: RecordingEvent::ChatSendFailed,
                effects: hud(H::SendFailed),
                state: RecordingState::Idle,
            },
            Case {
                name: "草稿被接受提示带条数",
                chat_integration: true,
                setup: vec![],
                event: RecordingEvent::ChatDraftFinished(DraftOutcome::Accepted { strokes: 4 }),
                effects: hud(H::DraftAccepted { strokes: 4 }),
                state: RecordingState::Idle,
            },
            Case {
                name: "草稿被放弃提示",
                chat_integration: true,
                setup: vec![],
                event: RecordingEvent::ChatDraftFinished(DraftOutcome::Declined),
                effects: hud(H::DraftDeclined),
                state: RecordingState::Idle,
            },
            Case {
                name: "草稿失败带原因",
                chat_integration: true,
                setup: vec![],
                event: RecordingEvent::ChatDraftFinished(DraftOutcome::Failed {
                    detail: Some("身份过期".to_string()),
                }),
                effects: hud(H::DraftFailed {
                    detail: Some("身份过期".to_string()),
                }),
                state: RecordingState::Idle,
            },
        ]);
    }

    // ── 完整会话的 effect 顺序(逐条断言,防止中途顺序漂移) ──

    #[test]
    fn gif_session_effects_in_order() {
        let mut machine = RecordingMachine::new(false);
        assert_eq!(
            machine.handle(start(Gif, 10)),
            hud(H::GifStartHint),
            "key-down 先提示按住绘制"
        );
        // 长按 repeat:不重复提示。
        assert_eq!(machine.handle(start(Gif, 10)), Vec::new());
        assert_eq!(
            machine.handle(release(Gif, 13)),
            vec![E::EncodeGif {
                start_stroke: 10,
                end_stroke: 13,
            }]
        );
        assert!(
            machine.is_idle(),
            "松手先回空闲,后台编码在途由 adapter 持有"
        );
        // 编码成功 → 剪贴板 → 成功提示,顺序固定。
        assert_eq!(
            machine.handle(RecordingEvent::GifEncodeSucceeded),
            vec![E::Clipboard]
        );
        assert_eq!(
            machine.handle(RecordingEvent::GifClipboardSucceeded),
            hud(H::GifCopied)
        );
    }

    #[test]
    fn send_session_effects_in_order() {
        let mut machine = RecordingMachine::new(true);
        assert_eq!(machine.handle(start(Send, 0)), hud(H::SendStartHint));
        assert_eq!(
            machine.handle(release(Send, 4)),
            vec![E::SendChatStrokes {
                start_stroke: 0,
                end_stroke: 4,
            }]
        );
        assert_eq!(
            machine.handle(RecordingEvent::ChatSendSucceeded { strokes: 4 }),
            hud(H::SendSucceeded { strokes: 4 })
        );
    }

    #[test]
    fn draft_session_effects_in_order() {
        let mut machine = RecordingMachine::new(true);
        assert_eq!(machine.handle(start(Draft, 0)), vec![E::StartChatDraft]);
        assert_eq!(
            machine.handle(RecordingEvent::ChatDraftStarted),
            hud(H::DraftStartHint)
        );
        assert_eq!(machine.handle(release(Draft, 0)), vec![E::StopChatDraft]);
        // 裁决结果晚到也照常提示(会话已回空闲)。
        assert_eq!(
            machine.handle(RecordingEvent::ChatDraftFinished(DraftOutcome::Accepted {
                strokes: 2
            })),
            hud(H::DraftAccepted { strokes: 2 })
        );
    }

    // ── 契约:默认关总开关 / 计数 0 起点 / 草稿结果映射 / 稳定名字 ──

    #[test]
    fn default_machine_is_idle_with_chat_integration_off() {
        // 两端默认 `g_chat_integration = NO` / `chat_integration` 缺省 0。
        let mut machine = RecordingMachine::default();
        assert!(machine.is_idle());
        assert!(!machine.chat_integration());
        assert_eq!(machine.handle(start(Draft, 0)), Vec::new());
        assert_eq!(machine.handle(start(Gif, 0)), hud(H::GifStartHint));
    }

    #[test]
    fn zero_start_index_is_a_valid_window_start() {
        let mut machine = RecordingMachine::new(false);
        let _ = machine.handle(start(Gif, 0));
        assert_eq!(
            machine.handle(release(Gif, 1)),
            vec![E::EncodeGif {
                start_stroke: 0,
                end_stroke: 1,
            }]
        );
    }

    #[test]
    fn draft_outcome_maps_ffi_codes() {
        // chat_glue.rs:336-367:>0 接受条数,0 放弃,<0 失败(原因另取)。
        assert_eq!(
            DraftOutcome::from_ffi_code(5, None),
            DraftOutcome::Accepted { strokes: 5 }
        );
        assert_eq!(DraftOutcome::from_ffi_code(0, None), DraftOutcome::Declined);
        assert_eq!(
            DraftOutcome::from_ffi_code(-1, None),
            DraftOutcome::Failed { detail: None }
        );
        assert_eq!(
            DraftOutcome::from_ffi_code(-1, Some("身份过期".to_string())),
            DraftOutcome::Failed {
                detail: Some("身份过期".to_string())
            }
        );
    }

    #[test]
    fn mode_names_and_chat_classification_are_stable() {
        assert_eq!(
            (Gif.name(), Draft.name(), Send.name()),
            ("Gif", "Draft", "Send")
        );
        assert!(!Gif.is_chat());
        assert!(Draft.is_chat());
        assert!(Send.is_chat());
        assert_ne!(Gif, Draft);
    }

    #[test]
    fn effect_variant_names_are_stable_and_unique() {
        let effects = [
            E::ShowHud(H::GifCopied),
            E::EncodeGif {
                start_stroke: 0,
                end_stroke: 1,
            },
            E::Clipboard,
            E::StartChatDraft,
            E::StopChatDraft,
            E::SendChatStrokes {
                start_stroke: 0,
                end_stroke: 1,
            },
            E::Discard,
        ];
        let expected = [
            "ShowHud",
            "EncodeGif",
            "Clipboard",
            "StartChatDraft",
            "StopChatDraft",
            "SendChatStrokes",
            "Discard",
        ];
        let mut seen = std::collections::BTreeSet::new();
        for (effect, name) in effects.iter().zip(expected) {
            assert_eq!(effect.variant_name(), name);
            assert!(seen.insert(name), "effect 变体名重复: {name}");
        }
    }

    #[test]
    fn hud_variant_names_are_stable_and_unique() {
        let huds = [
            H::GifStartHint,
            H::SendStartHint,
            H::DraftStartHint,
            H::DraftStartFailed,
            H::GifCopied,
            H::GifNoStrokesOrExportFailed,
            H::GifClipboardFailed,
            H::SendNoStrokes,
            H::SendSucceeded { strokes: 1 },
            H::SendFailed,
            H::DraftAccepted { strokes: 1 },
            H::DraftDeclined,
            H::DraftFailed { detail: None },
        ];
        let expected = [
            "GifStartHint",
            "SendStartHint",
            "DraftStartHint",
            "DraftStartFailed",
            "GifCopied",
            "GifNoStrokesOrExportFailed",
            "GifClipboardFailed",
            "SendNoStrokes",
            "SendSucceeded",
            "SendFailed",
            "DraftAccepted",
            "DraftDeclined",
            "DraftFailed",
        ];
        let mut seen = std::collections::BTreeSet::new();
        for (hud, name) in huds.iter().zip(expected) {
            assert_eq!(hud.variant_name(), name);
            assert!(seen.insert(name), "HUD 变体名重复: {name}");
        }
    }

    /// 状态机必须完全确定:同一事件序列重放得到同一结果(没有隐藏 I/O/随机)。
    #[test]
    fn replay_is_deterministic() {
        let events = vec![
            RecordingEvent::ChatIntegrationChanged { enabled: true },
            start(Draft, 0),
            RecordingEvent::ChatDraftStarted,
            RecordingEvent::ChatDraftFinished(DraftOutcome::Declined),
            start(Gif, 7),
            release(Gif, 9),
            RecordingEvent::GifEncodeSucceeded,
            RecordingEvent::GifClipboardSucceeded,
            RecordingEvent::ForceStop,
        ];
        let run = || {
            let mut machine = RecordingMachine::new(true);
            let mut transcript = Vec::new();
            for event in events.clone() {
                transcript.push((machine.state(), machine.handle(event)));
            }
            (transcript, machine.state())
        };
        assert_eq!(run(), run());
    }

    // ── D8b adapter JSON 编解码:字段名/单行/错误分支 ──

    fn parse_event(text: &str) -> RecordingEvent {
        event_from_json(text).unwrap_or_else(|e| panic!("事件 JSON 应可解析: {e}: {text}"))
    }

    /// 每一种事件(含草稿裁决单源映射)都能从 adapter JSON 还原。
    #[test]
    fn event_json_roundtrip_covers_every_variant() {
        assert_eq!(
            parse_event(r#"{"type":"StartPressed","mode":"Gif","stroke_count":3}"#),
            start(Gif, 3)
        );
        assert_eq!(
            parse_event(r#"{"type":"ReleasePressed","mode":"Draft","stroke_count":7}"#),
            release(Draft, 7)
        );
        assert_eq!(
            parse_event(r#"{"type":"ChatIntegrationChanged","enabled":false}"#),
            RecordingEvent::ChatIntegrationChanged { enabled: false }
        );
        for (json, event) in [
            (
                r#"{"type":"GifEncodeSucceeded"}"#,
                RecordingEvent::GifEncodeSucceeded,
            ),
            (
                r#"{"type":"GifEncodeFailed"}"#,
                RecordingEvent::GifEncodeFailed,
            ),
            (
                r#"{"type":"GifClipboardSucceeded"}"#,
                RecordingEvent::GifClipboardSucceeded,
            ),
            (
                r#"{"type":"GifClipboardFailed"}"#,
                RecordingEvent::GifClipboardFailed,
            ),
            (
                r#"{"type":"ChatDraftStarted"}"#,
                RecordingEvent::ChatDraftStarted,
            ),
            (
                r#"{"type":"ChatDraftStartFailed"}"#,
                RecordingEvent::ChatDraftStartFailed,
            ),
            (
                r#"{"type":"ChatSendFailed"}"#,
                RecordingEvent::ChatSendFailed,
            ),
            (r#"{"type":"ForceStop"}"#, RecordingEvent::ForceStop),
        ] {
            assert_eq!(parse_event(json), event, "事件 JSON 漂移: {json}");
        }
        assert_eq!(
            parse_event(r#"{"type":"ChatSendSucceeded","strokes":4}"#),
            RecordingEvent::ChatSendSucceeded { strokes: 4 }
        );
        // 草稿裁决: code 经 DraftOutcome::from_ffi_code 单源映射, detail 可缺省。
        assert_eq!(
            parse_event(r#"{"type":"ChatDraftFinished","code":3}"#),
            RecordingEvent::ChatDraftFinished(DraftOutcome::Accepted { strokes: 3 })
        );
        assert_eq!(
            parse_event(r#"{"type":"ChatDraftFinished","code":0,"detail":""}"#),
            RecordingEvent::ChatDraftFinished(DraftOutcome::Declined)
        );
        assert_eq!(
            parse_event(r#"{"type":"ChatDraftFinished","code":-1,"detail":"身份过期"}"#),
            RecordingEvent::ChatDraftFinished(DraftOutcome::Failed {
                detail: Some("身份过期".to_string())
            })
        );
        // 模式名反查是 name() 的逆。
        for mode in [Gif, Draft, Send] {
            assert_eq!(mode_from_name(mode.name()), Some(mode));
        }
        assert_eq!(mode_from_name("gif"), None);
    }

    /// 坏 JSON / 缺字段 / 未知枚举名都要给出可读错误(FFI 包成 error JSON)。
    #[test]
    fn event_json_rejects_bad_input() {
        for bad in [
            "",
            "not json",
            "[]",
            r#"{"type":"NoSuchEvent"}"#,
            r#"{"type":"StartPressed","mode":"Gif"}"#,
            r#"{"type":"StartPressed","stroke_count":1}"#,
            r#"{"type":"StartPressed","mode":"gif","stroke_count":1}"#,
            r#"{"type":"StartPressed","mode":"Gif","stroke_count":-1}"#,
            r#"{"type":"ChatIntegrationChanged","enabled":1}"#,
            r#"{"type":"ChatDraftFinished"}"#,
            r#"{"type":"ChatSendSucceeded"}"#,
        ] {
            let err = event_from_json(bad).expect_err(&format!("坏事件应报错: {bad}"));
            assert!(!err.is_empty(), "错误文案不能为空: {bad}");
        }
    }

    /// effects JSON:每个变体一种形状(ObjC 按 kind 分派), 单行数组。
    #[test]
    fn effects_json_shape_covers_every_variant() {
        let cases: Vec<(RecordingEffect, serde_json::Value)> = vec![
            (
                E::ShowHud(H::GifStartHint),
                serde_json::json!({"kind":"ShowHud","hud":"GifStartHint"}),
            ),
            (
                E::ShowHud(H::SendSucceeded { strokes: 2 }),
                serde_json::json!({"kind":"ShowHud","hud":"SendSucceeded","strokes":2}),
            ),
            (
                E::ShowHud(H::DraftAccepted { strokes: 5 }),
                serde_json::json!({"kind":"ShowHud","hud":"DraftAccepted","strokes":5}),
            ),
            (
                E::ShowHud(H::DraftFailed {
                    detail: Some("身份过期".to_string()),
                }),
                serde_json::json!({"kind":"ShowHud","hud":"DraftFailed","detail":"身份过期"}),
            ),
            (
                E::ShowHud(H::DraftFailed { detail: None }),
                serde_json::json!({"kind":"ShowHud","hud":"DraftFailed"}),
            ),
            (
                E::EncodeGif {
                    start_stroke: 3,
                    end_stroke: 8,
                },
                serde_json::json!({"kind":"EncodeGif","start_stroke":3,"end_stroke":8}),
            ),
            (E::Clipboard, serde_json::json!({"kind":"Clipboard"})),
            (
                E::StartChatDraft,
                serde_json::json!({"kind":"StartChatDraft"}),
            ),
            (
                E::StopChatDraft,
                serde_json::json!({"kind":"StopChatDraft"}),
            ),
            (
                E::SendChatStrokes {
                    start_stroke: 1,
                    end_stroke: 4,
                },
                serde_json::json!({"kind":"SendChatStrokes","start_stroke":1,"end_stroke":4}),
            ),
            (E::Discard, serde_json::json!({"kind":"Discard"})),
        ];
        for (effect, expected) in cases {
            let value = effect_to_json(&effect);
            assert_eq!(value, expected, "effect JSON 漂移: {effect:?}");
        }
        // 多个 effect = JSON 数组, 单行(ObjC NSJSONSerialization)。
        let text = effects_to_json(&[
            E::ShowHud(H::GifCopied),
            E::EncodeGif {
                start_stroke: 0,
                end_stroke: 2,
            },
        ]);
        assert!(!text.contains('\n'), "effects JSON 必须单行: {text}");
        let parsed: serde_json::Value = serde_json::from_str(&text).expect("effects JSON 合法");
        assert_eq!(parsed.as_array().map(Vec::len), Some(2));
        assert_eq!(effects_to_json(&[]), "[]");
    }

    // ── D8b: 两端 adapter 录制接线闸门(唯一来源 = core RecordingMachine)──
    //
    // 按文本扫描两端 adapter(剥掉注释), 两类断言:
    // 1) **必须**出现 core 入口/变体 token —— macOS 经 glaspen2_recording_* FFI,
    //    按 effects JSON 的 kind 与 HUD 名字分派;Windows 直接 match
    //    RecordingEvent/RecordingEffect 变体。漏接线/漏实现某变体即红。
    // 2) 旧会话标志位 / 逐模式决策的指纹**不得**出现 —— 把
    //    g_gif_recording / gif_recording / msg_record_start / ink_draft_active
    //    或旧的 stop/HUD 决策分支抄回 adapter 即红。
    //
    // 行注释与块注释先剥掉(注释里允许解释旧实现);include_str! 指错文件/文件
    // 被清空时闸门不能静默变绿(过短直接失败)。

    /// 剥掉行注释(// ...)与块注释(/* ... */), 只留代码做指纹匹配。
    fn strip_comments(src: &str) -> String {
        let mut out = String::with_capacity(src.len());
        let mut chars = src.chars().peekable();
        let mut in_line = false;
        let mut in_block = false;
        while let Some(ch) = chars.next() {
            let next = chars.peek().copied();
            if in_line {
                if ch == '\n' {
                    in_line = false;
                    out.push(ch);
                }
                continue;
            }
            if in_block {
                if ch == '*' && next == Some('/') {
                    chars.next();
                    in_block = false;
                }
                continue;
            }
            if ch == '/' && next == Some('/') {
                chars.next();
                in_line = true;
                continue;
            }
            if ch == '/' && next == Some('*') {
                chars.next();
                in_block = true;
                continue;
            }
            out.push(ch);
        }
        out
    }

    fn gate_violations(name: &str, src: &str, forbidden: &[&str]) -> Vec<String> {
        let code = strip_comments(src);
        forbidden
            .iter()
            .filter_map(|pat| {
                code.find(pat).map(|at| {
                    format!(
                        "{name}:{} 仍含旧录制会话标志/决策 {:?}",
                        code[..at].matches('\n').count() + 1,
                        pat
                    )
                })
            })
            .collect()
    }

    fn gate_missing(name: &str, src: &str, required: &[&str]) -> Vec<String> {
        let code = strip_comments(src);
        required
            .iter()
            .filter(|token| !code.contains(**token))
            .map(|token| format!("{name} 缺少 core 录制状态机接线 {token:?}"))
            .collect()
    }

    #[test]
    fn adapters_drive_core_recording_machine() {
        // macOS: run.m = 热键/事件 tap/effects 执行; glaspen2.m = FFI 声明 +
        // (已删除的)会话标志位; menu.m = 总开关关闭收尾。
        let mac_main = include_str!("../../../src/macos/glaspen2.m");
        let mac_run = include_str!("../../../src/macos/parts/run.m");
        let mac_menu = include_str!("../../../src/macos/parts/menu.m");
        // Windows: recording.rs = 接线/effects 分派; state.rs = 两个状态机字段;
        // actions.rs = Raw Input/总开关入口; hotkeys.rs = WM_HOTKEY; wndproc =
        // 后台结果回窗口线程; cmds.rs = 自定义消息。
        let win_recording =
            include_str!("../../../crates/glaspen-windows/src/overlay/recording.rs");
        let win_state = include_str!("../../../crates/glaspen-windows/src/overlay/state.rs");
        let win_actions = include_str!("../../../crates/glaspen-windows/src/overlay/actions.rs");
        let win_hotkeys = include_str!("../../../crates/glaspen-windows/src/overlay/hotkeys.rs");
        let win_wndproc = include_str!("../../../crates/glaspen-windows/src/overlay/wndproc.rs");
        let win_cmds = include_str!("../../../crates/glaspen-windows/src/overlay/cmds.rs");
        for (name, src) in [
            ("glaspen2.m", mac_main),
            ("run.m", mac_run),
            ("menu.m", mac_menu),
            ("recording.rs", win_recording),
            ("state.rs", win_state),
            ("actions.rs", win_actions),
            ("hotkeys.rs", win_hotkeys),
            ("wndproc.rs", win_wndproc),
            ("cmds.rs", win_cmds),
        ] {
            assert!(
                src.len() > 500,
                "闸门扫到的 {name} 过短, include_str! 路径可疑"
            );
        }

        // 旧会话标志位 + 逐模式 start/stop 函数 + 旧结果码分派(两端)。
        let mac_forbidden = [
            "g_gif_recording",
            "g_gif_record_start",
            "g_msg_record_start",
            "g_ink_draft_active",
            "gif_record_start",
            "gif_record_stop",
            "msg_record_start",
            "msg_record_stop",
            "ink_draft_stop_async",
            "static void ink_draft_start(",
            // 旧 HUD/裁决分派指纹: 编码成功分支里直接写剪贴板 + 仍提示成功,
            // 以及草稿裁决的本地三分支(判定/文案选择应已上收 FSM)。
            "copy_gif_data_to_clipboard(gif, out_len)",
            "if (r > 0)",
        ];
        let win_forbidden = [
            "gif_recording",
            "gif_record_start",
            "gif_record_stop",
            "msg_record_start",
            "msg_record_stop",
            "ink_draft_active",
            "chat_record_start",
            "chat_record_stop",
            // 旧实现: 后台线程一把做完编码+剪贴板再按结果码分派
            "WM_RECORD_DONE",
            "copy_gif_bytes_to_clipboard(gif)",
            // 旧 HUD/裁决分派指纹
            "if (sent >= 0)",
            "if (r > 0)",
        ];

        let mut problems = Vec::new();
        for (name, src) in [
            ("glaspen2.m", mac_main),
            ("run.m", mac_run),
            ("menu.m", mac_menu),
        ] {
            problems.extend(gate_violations(name, src, &mac_forbidden));
        }
        for (name, src) in [
            ("recording.rs", win_recording),
            ("state.rs", win_state),
            ("actions.rs", win_actions),
            ("hotkeys.rs", win_hotkeys),
            ("wndproc.rs", win_wndproc),
            ("cmds.rs", win_cmds),
        ] {
            problems.extend(gate_violations(name, src, &win_forbidden));
        }

        // macOS 必须经 FFI 驱动 core 状态机, 且 effects kind / HUD 名字全覆盖。
        problems.extend(gate_missing(
            "glaspen2.m",
            mac_main,
            &[
                "extern int glaspen2_recording_init(",
                "extern char* glaspen2_recording_event_json(",
                "extern int glaspen2_recording_reset(",
                "extern int glaspen2_recording_mode(",
            ],
        ));
        problems.extend(gate_missing(
            "run.m",
            mac_run,
            &[
                "glaspen2_recording_event_json(",
                "glaspen2_recording_mode(",
                "gl_recording_init(",
                "rec_finish_and_count(",
                "rec_exec_effect(",
                "gl_recording_chat_integration_changed(",
            ],
        ));
        // macOS effects/HUD 分派必须覆盖 core 的每个 kind/hud 名字(D3b 同款:
        // ObjC 没有字符串 switch, 编译器管不到 —— 漏一个变体即红)。
        let effect_kinds = [
            "ShowHud",
            "EncodeGif",
            "Clipboard",
            "StartChatDraft",
            "StopChatDraft",
            "SendChatStrokes",
            "Discard",
        ];
        let hud_names = [
            "GifStartHint",
            "SendStartHint",
            "DraftStartHint",
            "DraftStartFailed",
            "GifCopied",
            "GifNoStrokesOrExportFailed",
            "GifClipboardFailed",
            "SendNoStrokes",
            "SendSucceeded",
            "SendFailed",
            "DraftAccepted",
            "DraftDeclined",
            "DraftFailed",
        ];
        let event_types = [
            "StartPressed",
            "ReleasePressed",
            "ChatIntegrationChanged",
            "GifEncodeSucceeded",
            "GifEncodeFailed",
            "GifClipboardSucceeded",
            "GifClipboardFailed",
            "ChatDraftStarted",
            "ChatDraftStartFailed",
            "ChatDraftFinished",
            "ChatSendSucceeded",
            "ChatSendFailed",
        ];
        let mut mac_required: Vec<String> =
            effect_kinds.iter().map(|n| format!("@\"{n}\"")).collect();
        mac_required.extend(hud_names.iter().map(|n| format!("@\"{n}\"")));
        mac_required.extend(event_types.iter().map(|n| format!("@\"{n}\"")));
        // NSJSONSerialization 生成的键名(事件/effects 载荷)也要在 adapter 出现。
        mac_required.extend(
            [
                "\"type\"",
                "\"mode\"",
                "\"kind\"",
                "\"hud\"",
                "\"stroke_count\"",
            ]
            .iter()
            .map(|s| (*s).to_string()),
        );
        let mac_required_refs: Vec<&str> = mac_required.iter().map(String::as_str).collect();
        problems.extend(gate_missing("run.m(变体覆盖)", mac_run, &mac_required_refs));
        problems.extend(gate_missing(
            "menu.m",
            mac_menu,
            &["gl_recording_chat_integration_changed("],
        ));

        // Windows 必须直接 match 两个状态机/全部事件与 effect 变体。
        problems.extend(gate_missing(
            "state.rs",
            win_state,
            &[
                "gif_machine: glaspen_core::recording::RecordingMachine",
                "chat_machine: glaspen_core::recording::RecordingMachine",
            ],
        ));
        let win_rec_required = [
            "RecordingEvent::StartPressed",
            "RecordingEvent::ReleasePressed",
            "RecordingEvent::ChatIntegrationChanged",
            "RecordingEvent::GifEncodeSucceeded",
            "RecordingEvent::GifEncodeFailed",
            "RecordingEvent::GifClipboardSucceeded",
            "RecordingEvent::GifClipboardFailed",
            "RecordingEvent::ChatDraftStarted",
            "RecordingEvent::ChatDraftStartFailed",
            "RecordingEvent::ChatDraftFinished",
            "RecordingEvent::ChatSendSucceeded",
            "RecordingEvent::ChatSendFailed",
            "RecordingEffect::ShowHud",
            "RecordingEffect::EncodeGif",
            "RecordingEffect::Clipboard",
            "RecordingEffect::StartChatDraft",
            "RecordingEffect::StopChatDraft",
            "RecordingEffect::SendChatStrokes",
            "RecordingEffect::Discard",
            "DraftOutcome::from_ffi_code",
            "recording_feed(",
            "recording_start(",
            "recording_release(",
            "recording_chat_integration_changed(",
            "WM_RECORD_ENCODE_OK",
            "WM_RECORD_ENCODE_FAIL",
            "WM_RECORD_DRAFT_DONE",
            "WM_RECORD_SEND_DONE",
        ];
        problems.extend(gate_missing(
            "recording.rs",
            win_recording,
            &win_rec_required,
        ));
        // Hotkeys 只喂 Start 事件;Raw Input 只喂 Release;窗口线程分发后台结果。
        problems.extend(gate_missing(
            "hotkeys.rs",
            win_hotkeys,
            &[
                "recording_start(state, RecordingMode::Gif)",
                "recording_start(state, RecordingMode::Draft)",
                "recording_start(state, RecordingMode::Send)",
            ],
        ));
        problems.extend(gate_missing(
            "actions.rs",
            win_actions,
            &[
                "recording_release(state, RecordingMode::Gif)",
                "recording_release(state, RecordingMode::Draft)",
                "recording_release(state, RecordingMode::Send)",
                "recording_chat_integration_changed(state, on)",
            ],
        ));
        problems.extend(gate_missing(
            "wndproc.rs",
            win_wndproc,
            &[
                "recording_encode_done(",
                "recording_encode_failed(",
                "recording_draft_done(",
                "recording_send_done(",
            ],
        ));
        // Windows HUD 文案必须覆盖 core 的每个 RecordingHud 变体。
        problems.extend(gate_missing(
            "recording.rs(HUD 覆盖)",
            win_recording,
            &hud_names,
        ));

        assert!(
            problems.is_empty(),
            "D8b 录制接线闸门失败(两端必须只经 core::recording 状态机):\n{}",
            problems.join("\n")
        );
    }
}
