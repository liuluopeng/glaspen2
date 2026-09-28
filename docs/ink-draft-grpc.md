# 手写消息草稿通道(⌘⌃2)—— axum 侧实现指南

本文写给实现本地 axum 服务的工程,描述 glaspen2 新增的**手写消息草稿通道**
如何对接。它与既有的 `AppendMessages` 直发流程(见
[axum-chat-store.md](axum-chat-store.md))平行存在:

| | ⌘⌃3 直发 | **⌘⌃2 草稿(本文)** |
|---|---|---|
| 用户动作 | 按住书写,松开 | 按住书写,松开 |
| gRPC 方法 | `AppendMessages`(松开后一次性) | **`DraftInk`(按住期间实时流)** |
| 谁决定发送 | glaspen2(发了就是发了) | **axum 侧(收完再决定)** |
| 典型用途 | 直接存入消息流 | 先预览/确认/识别,再决定发或丢弃 |

协议契约仍然只有 `proto/glaspen/chat/v1/chat.proto` 一个文件;本次是
**纯追加**改动(新 RPC + 新消息),线格式与老版本完全兼容。

---

## 1. 交互与流生命周期

用户按住 ⌘⌃2 开始书写、松开结束(glaspen2 侧已实现,axum 只需实现服务端):

```text
⌘⌃2 key-down   →  glaspen2 打开一条 DraftInk 客户端流
                   └─ 帧 1:  DraftFrame.begin
书写(每次抬笔) →  └─ 帧 2..N+1: DraftFrame.stroke(实时,一笔一帧)
⌘⌃2 key-up     →  └─ 帧 N+2: DraftFrame.end
                   └─ half-close(客户端关请求流)
                   ◄─ DraftReply(axum 的决定:sent / accepted / first_msg_id)
```

要点:

- **stroke 帧是实时到达的**(每次抬笔立刻推),axum 可以边收边画预览;
  不必等流结束。
- 一次会话 = 一条 gRPC 流。连接在 key-down 时建立,在 key-up + 回执后关闭。
- 连接失败时 glaspen2 会照常收集笔迹,结束时收到失败并通知用户 ——
  axum 未启动不影响 glaspen2 本身的涂鸦功能。
- 两套快捷键互斥:⌘⌃2(草稿)与 ⌘⌃3(直发)不会同时生效。
- 橡皮擦擦除不产生 stroke 帧(草稿只含手写笔迹)。

## 2. 方法签名

```proto
// service glaspen.chat.v1.ChatStore 内新增:
rpc DraftInk(stream DraftFrame) returns (DraftReply);
```

```proto
// DraftInk 的流内帧。帧序固定:begin → stroke×N(每抬笔一帧)→ end。
message DraftFrame {
  oneof frame {
    DraftBegin begin = 1;   // 首帧:会话上下文
    ChatMessage stroke = 2; // 中间帧:一条笔迹 = 一条 STROKE 消息
    DraftEnd end = 3;       // 末帧:松开快捷键,通道即将 half-close
  }
}

message DraftBegin {
  string session_id = 1;   // 会话标识(客户端生成,日志/幂等用)
  int64 started_at_ms = 2; // 会话开始时间(UTC 毫秒)
  string notebook_id = 3;  // 若草稿被接受,消息应归属的消息流
  string author = 4;       // 身份(涂鸦工具当前发空串)
  string device = 5;
  uint32 canvas_w = 6;     // 笔迹坐标空间的页面宽高(逻辑点 @1x),渲染提示
  uint32 canvas_h = 7;
}

message DraftEnd {
  uint32 stroke_count = 1; // 本会话推送的 stroke 帧数
  uint64 duration_ms = 2;  // begin → end 的墙钟时长
  bool cancelled = 3;      // 预留,当前恒为 false
}

message DraftReply {
  bool sent = 1;           // true=已发送/落库;false=丢弃
  uint64 accepted = 2;     // sent=true 时:接受的笔迹条数
  uint64 first_msg_id = 3; // sent=true 时:分配的首个 msg_id(后续 +1)
}
```

## 3. stroke 帧的载荷语义

stroke 帧就是标准的 `ChatMessage`,payload 固定为 `StrokeContent`。
各字段含义(与 ⌘⌃3 直发完全一致,`stroke_message()` 构造器见
`chat/src/lib.rs`):

| 字段 | 语义 |
|---|---|
| `points[i].x / .y` | 画布页坐标。原点 = 当前页左上角,**y 向下**,单位 = 逻辑点(@1x,macOS point)。接收方按包围盒自行归一化渲染 |
| `points[i].width` | 该点线宽(px @1x,含笔锋:0.3–8 常见) |
| `points[i].t_rel` | 相对该**笔画**起点的时间(秒),升序;笔顺回放用 |
| `color_rgb` | `0xRRGGBB` |
| `width_scale` | 恒为 1.0(线宽已逐点携带) |
| `layout` | flow(聊天流里独立成块) |
| `seq` | ⚠️ **草稿内局部序号 1..N**,不是最终流内序号 |
| `client_msg_id` | 客户端生成的幂等键,同一直发流程 |
| `created_at_ms` | 该笔抬笔时刻(UTC 毫秒) |
| `notebook_id` | 与 `DraftBegin.notebook_id` 一致(参考值;以 begin 为准) |

## 4. 服务端实现要求

```rust
async fn draft_ink(&self, request: Request<Streaming<DraftFrame>>)
    -> Result<Response<DraftReply>, Status>
{
    let mut stream = request.into_inner();
    let mut ctx: Option<DraftBegin> = None;
    let mut strokes = Vec::new();
    while let Some(frame) = stream.message().await? {
        match frame.frame {
            Some(DraftFrame1::Begin(b)) => ctx = Some(b),   // 必为首帧
            Some(DraftFrame1::Stroke(m)) => strokes.push(m), // 实时缓冲/预览
            Some(DraftFrame1::End(_)) => break,              // half-close 前的末帧
            None => {}                                       // 未知分支:忽略
        }
    }
    // 流在这里自然结束(客户端已 half-close)。
    // —— 决策点:在这里决定这份草稿发不发 ——

    // 发送:走与 AppendMessages 相同的落库路径
    //   * seq 以草稿局部序号重排/重分配,不得直接沿用(它只是 1..N)
    //   * client_msg_id 保留,天然幂等(同一草稿二次接受不会重复)
    // 丢弃:直接回 sent=false
}
```

具体约定:

1. **必须容忍空会话**:begin + end 直接到达(stroke_count = 0,用户按住后
   没画)。正常返回 `sent=false`(或按业务视为取消)。
2. **顺序处理**:stroke 帧按书写顺序到达;同会话内 `seq` 严格 1..N。
3. **`DraftEnd.stroke_count` 应与实收 stroke 帧数一致**;不一致以实收为准,
   不报错(防御性)。
4. **决策 = 回执**:决定落库时,复用 AppendMessages 的入库与去重逻辑,
   `sent=true` 并回填 `accepted` / `first_msg_id`;决定不发时
   `sent=false`(两个计数留 0)。**不要**用 gRPC error 表达"不发送"。
5. **错误码**:中途协议错误用 `internal`;当前客户端遇到任何 Status 都按
   "通道失败"通知用户。回执不设时限,但 glaspen2 侧对整个会话有 10 分钟
   超时兜底。
6. **实时预览是可选的**:只在 `Stroke` 帧到达时画即可;若你的实现选择
   完全缓冲到流结束,功能上同样正确。

## 5. 联调

glaspen2 侧开关(与直发共用同一组环境变量):

```bash
# 默认 mock(不连服务,stderr 打印逐帧摘要):
cargo run

# 真连你的 axum 服务:
GLASPEN_CHAT_MOCK=0 GLASPEN_CHAT_ENDPOINT=http://127.0.0.1:50051 cargo run
```

按住 ⌘⌃2 书写、松开,服务端应收到一条 `DraftInk` 流;glaspen2 通知栏显示
axum 的决定。glaspen2 侧日志在 stderr(前缀 `[ink-draft]`)。

不想拿笔也可以用现成的演示客户端(与主程序同一套代码路径):

```bash
cargo run -p glaspen-chat --bin mock-send -- --draft \
    GLASPEN_CHAT_ENDPOINT=http://127.0.0.1:50051
```

grpcurl 手工调用(流式入参从 stdin 读,每行一个 JSON 帧):

```bash
grpcurl -plaintext -import-path proto -proto glaspen/chat/v1/chat.proto \
  -d @ 127.0.0.1:50051 glaspen.chat.v1.ChatStore/DraftInk <<'EOF'
{"begin":{"session_id":"demo-1","started_at_ms":1727400000000,
          "notebook_id":"glaspen2-doodle","canvas_w":1920,"canvas_h":1080}}
{"stroke":{"notebook_id":"glaspen2-doodle","seq":1,"client_msg_id":"demo-s1",
           "type":1,"created_at_ms":1727400001000,
           "payload":{"stroke":{"color_rgb":16711680,"width_scale":1,
             "points":[{"x":0,"y":0,"width":2,"t_rel":0},
                       {"x":40,"y":12,"width":3.2,"t_rel":0.08}]}}}}
{"end":{"stroke_count":1,"duration_ms":1200,"cancelled":false}}
EOF
```

glaspen2 侧已有的测试可作参考实现(客户端视角):
`chat/src/draft.rs::tests::draft_channel_live_session` 起了一个内嵌 tonic
服务端收完整会话 —— 把它的断言反过来就是 axum 服务端的最小行为规格。

## 6. 版本与兼容

- 本次改动对 proto 是**纯追加**(新 RPC + 新消息 + 新 oneof 分支),
  老服务端收到 `DraftInk` 调用会返回 `Unimplemented`,glaspen2 侧按
  "通道失败"处理,不影响其它功能。
- 沿用 [axum-chat-store.md](axum-chat-store.md) §4 的演进规则:字段号
  永不复用;proto 以 axum 侧为源头改完后整文件复制回 glaspen2。
- 服务端先升级:实现 `DraftInk` 后,旧 glaspen2 不受影响;glaspen2 侧
  无需配置,检测到 `GLASPEN_CHAT_MOCK=0` 即走真通道。
