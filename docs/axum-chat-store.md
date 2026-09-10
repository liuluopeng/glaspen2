# glaspen 聊天流存储服务(axum + tonic)实现指南

本文写给实现消息存储服务的 axum 工程。glaspen2 主程序**不持有任何 messages
表**,所有聊天流消息都存在本服务里;glaspen2 侧只有一个 gRPC 客户端
(`chat/` crate)。

协议契约是唯一一个文件:

```
proto/glaspen/chat/v1/chat.proto
```

调用方向是单向的:glaspen2(client) → axum(server)。媒体二进制与消息
分开传输,按内容寻址。

---

## 1. 快速接入

### 1.1 依赖版本

与 glaspen2 侧(`chat/Cargo.toml`)保持一致,**大版本必须相同**:

```toml
[dependencies]
tonic = "0.13"
prost = "0.13"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
tokio-stream = "0.1"   # 服务端回放流用 tokio_stream::iter
axum = "0.8"           # 你的业务路由;tonic 0.13 内部同样基于 axum
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }  # 或 rusqlite,随意

[build-dependencies]
tonic-build = "0.13"
```

### 1.2 codegen

把 `chat.proto` 拷进 axum 工程(建议路径 `proto/glaspen/chat/v1/chat.proto`),
`build.rs` 只需生成 server 端(生成 client 也无妨,联调时有用):

```rust
fn main() {
    tonic_build::configure()
        .build_server(true)
        .compile_protos(&["proto/glaspen/chat/v1/chat.proto"], &["proto"])
        .expect("compile proto");
}
```

引用生成代码:

```rust
pub mod pb { tonic::include_proto!("glaspen.chat.v1"); }
use pb::chat_store_server::ChatStoreServer;
```

### 1.3 服务骨架

生成的 trait(tonic 0.13 内部用 `async_trait` 宏)如下,方法签名照抄即可:

```rust
use tonic::{Request, Response, Status, Streaming};
use tokio_stream::Stream;
use crate::pb::{chat_store_server::ChatStore, ...};

pub struct MyStore { /* sqlx pool + media 目录 */ }

#[tonic::async_trait]
impl ChatStore for MyStore {
    type FetchMessagesStream = Pin<Box<dyn Stream<Item = Result<ChatMessage, Status>> + Send>>;
    type DownloadMediaStream = Pin<Box<dyn Stream<Item = Result<MediaChunk, Status>> + Send>>;

    async fn append_messages(&self, request: Request<Streaming<ChatMessage>>)
        -> Result<Response<AppendReply>, Status> { /* §2.1 */ }

    async fn fetch_messages(&self, request: Request<FetchRequest>)
        -> Result<Response<Self::FetchMessagesStream>, Status> { /* §2.2 */ }

    async fn upload_media(&self, request: Request<Streaming<MediaChunk>>)
        -> Result<Response<MediaAck>, Status> { /* §2.3 */ }

    async fn download_media(&self, request: Request<MediaQuery>)
        -> Result<Response<Self::DownloadMediaStream>, Status> { /* §2.4 */ }
}
```

### 1.4 与 axum 路由共存

**起步推荐:同一进程、两个端口。** gRPC 走 tonic 自己的监听(纯 h2c),
axum 业务路由独立一个 HTTP 端口,互不干扰:

```rust
let grpc = ChatStoreServer::new(store);
tonic::transport::Server::builder()
    .add_service(grpc)
    .serve("127.0.0.1:50051".parse()?)
    .await?;
// axum 服务照常 axum::serve(...) 另一个端口
```

**单端口方案**(可选):tonic 0.13 的 `Routes` 本身就是 axum Router,
可以合并后由 axum 统一监听(hyper 自动识别 HTTP/2 prior knowledge):

```rust
use tonic::service::Routes;
let grpc = Routes::new(ChatStoreServer::new(store)).into_axum_router();
let app = axum::Router::new()
    .route("/health", axum::routing::get(|| async { "ok" }))
    .merge(grpc);   // gRPC 路径:/glaspen.chat.v1.ChatStore/<Method>
axum::serve(listener, app).await?;
```

> 注意:gRPC 必须是 HTTP/2。本地无 TLS 用的是 h2c(明文 HTTP/2),
> 代理/网关转发时需放行 h2,不要降级成 HTTP/1.1。

---

## 2. 四个 RPC 的实现语义

### 2.1 AppendMessages(客户端流)

流入:`Streaming<ChatMessage>`;流出:一条 `AppendReply`。

- **按序处理**流入消息,逐条落库。
- **去重是幂等的关键**,两个唯一键:
  - `(notebook_id, seq)` — 流内序号,重传场景;
  - `client_msg_id` — 客户端 UUID,跨会话重试场景。
  命中任一唯一键的消息**跳过、不报错**,且不计入 `accepted`。
- `msg_id`:全局单调 `AUTOINCREMENT`,服务端是唯一分配者。客户端
  收到的 `msg_id` 一律来自回执,自己不发明。
- 回执语义:
  - `first_msg_id` = 本批**第一条新接受**消息的 msg_id(后续依次 +1);
  - `accepted` = 新接受条数(排除重复);
  - `seqs` = 接受消息的 seq,按 msg_id 分配顺序排列。
  全部重复时返回 `first_msg_id = 0, accepted = 0, seqs = []`。
- 基本校验(失败返回 `Status::invalid_argument`):
  `notebook_id` 非空、`seq > 0`、`type` 不为 `UNSPECIFIED`。
- **宽松原则**:遇到未知 `type` 数值或未知 payload 分支,**照常入库**。
  服务端不应理解 payload,只做存储与回放(见 §6 兼容性)。

### 2.2 FetchMessages(服务端流)

- 按 `seq` 升序回放;`after_seq` 只返回 `seq > after_seq` 的消息;
  `limit = 0` 表示不限。
- 用 `tokio_stream::iter(rows).map(Ok)` 构造回放流即可。
- 未知 `notebook_id` 返回空流(不报错),便于客户端区分"空笔记本"。

### 2.3 UploadMedia(客户端流) / 2.4 DownloadMedia(服务端流)

- 首帧必须是 `MediaChunk::info(MediaInfo)`,声明 `media_id`(sha256 hex)、
  `mime`、`size`;后续帧为 `chunk` 字节。
- 服务端**边收边算 sha256**,结束后:
  - 与 `media_id` 不一致 → `Status::data_loss("sha256 mismatch")`,
    丢弃临时文件;
  - 一致 → 原子落盘(写 `.tmp` 后 rename 到 `media/<sha256>`),
    DB 记录若已存在则不覆盖,回执 `deduplicated = true`。
- Download 校验 `media_id` 不存在时返回 `Status::not_found`。
- media 表按内容寻址天然去重,同一段录音/图片多消息引用同一份字节。

### 2.5 错误码约定

| 场景 | Status |
|---|---|
| notebook_id 为空 / seq=0 / type 未指定 | `invalid_argument` |
| sha256 与 media_id 不符 | `data_loss` |
| 下载不存在的 media | `not_found` |
| 其余内部错误 | `internal` |

---

## 3. 存储设计(SQLite)

推荐两张表,索引是去重的实现载体:

```sql
CREATE TABLE IF NOT EXISTS messages (
    msg_id         INTEGER PRIMARY KEY AUTOINCREMENT,
    notebook_id    TEXT    NOT NULL,
    seq            INTEGER NOT NULL,
    client_msg_id  TEXT    NOT NULL,
    type           INTEGER NOT NULL,
    subtype        INTEGER NOT NULL DEFAULT 0,
    author         TEXT    NOT NULL DEFAULT '',
    device         TEXT    NOT NULL DEFAULT '',
    created_at_ms  INTEGER NOT NULL,
    layout_kind    INTEGER NOT NULL DEFAULT 0,  -- 0=flow 1=anchor(无 layout 同 0)
    anchor_x       REAL,
    anchor_y       REAL,
    meta           TEXT    NOT NULL DEFAULT '{}', -- JSON,可丢弃的缓存类信息
    payload_branch TEXT    NOT NULL,  -- oneof 分支名: 'stroke'/'image'/...
    payload        BLOB    NOT NULL,  -- 见下方"关键取舍"
    received_at_ms INTEGER NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_notebook_seq
    ON messages(notebook_id, seq);
CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_client_msg
    ON messages(client_msg_id);

CREATE TABLE IF NOT EXISTS media (
    sha256        TEXT PRIMARY KEY,   -- 即 media_id
    mime          TEXT NOT NULL,
    size          INTEGER NOT NULL,
    path          TEXT NOT NULL,      -- media/<sha256>
    created_at_ms INTEGER NOT NULL
);
```

**关键取舍 — payload 怎么存**:推荐把**整条 `ChatMessage` 的原始
prost 字节**存进 `payload`(`Message::encode_to_vec()` 的产物),
`payload_branch` 只是提取出来建索引/列表用的辅助列。好处:

- 回放 = 直接 `decode`,**未知字段零损耗**(对 §6 的前向兼容至关重要);
- 转发、合并笔记(引用旧消息原样进新流)就是字节拷贝,无需重组;
- 服务端永远不需要理解每种 payload。

如果未来要按 payload 内容检索(如 OCR 文本),再加衍生表或列,
不要用"结构化拆列"替代原始字节存档。

`media/` 目录与 SQLite 文件同一父目录,备份 = 拷目录。

---

## 4. proto 演进流程(axum 改 → 复制回 glaspen2)

约定:**proto 的修改在 axum 侧进行**,然后把文件复制回 glaspen2 重新
生成。proto 文件内容必须逐字节一致,复制即同步。

### 4.1 修改规则(线格式兼容红线)

- **字段号永不复用、永不改 wire type**。删除字段用 `reserved` 占住编号:
  ```proto
  reserved 12, 13;
  // reserved "old_field_name";
  ```
- **enum 新值追加在文末**,`0` 值永远是 `*_UNSPECIFIED`。enum 值也是
  "编号",同样禁止复用。
- oneof 新分支用新字段号,规则同普通字段。
- 改名随意(线上只有编号);改类型需谨慎:`int32↔int64`、
  `sint*` 互通,`string↔bytes` 不兼容。
- `subtype` 的语义编号继续对齐微信(CARD: 6 文件/19 合并/57 引用),
  新增卡片子类型优先沿用微信编号,便于记忆与工具互通。
- proto 头部注释里维护一行变更记录(type/subtype 新增值写明日期),
  两个仓库的文件同时更新。

### 4.2 同步步骤

```
1. 在 axum 工程修改 proto,提交;
2. 复制到 glaspen2:proto/glaspen/chat/v1/chat.proto(整文件覆盖);
3. cd glaspen2 && cargo build -p glaspen-chat   # build.rs 自动重新生成
4. cargo test  -p glaspen-chat                  # 往返/映射测试兜底
5. 服务器先行升级,客户端(glaspen2)随后。
```

### 4.3 两侧版本对齐

- tonic / prost / tonic-build 大版本两侧一致(当前 `0.13` / `0.13`);
- glaspen2 侧 prost 生成代码的命名约定,写代码时留意:
  - proto 字段 `type` → Rust 字段 `r#type`(关键字原名保留);
  - **递归消息自动装箱**:`QuoteContent.reply` 是 `Box<ChatMessage>`,
    oneof 里的递归变体 `Payload::Quote(Box<QuoteContent>)` 同理;
  - 枚举值 `MSG_TYPE_STROKE` → `MsgType::Stroke`(自动剥前缀)。
- tonic 0.13 的客户端流直接产出消息本体(`Stream<Item = ChatMessage>`),
  不再接受 `Result` 包装,两边对齐后写法保持一致。

### 4.4 兼容方向

- **服务端先升级**:新客户端发来的未知字段/未知 payload,服务端
  (prost ≥ 3.5)解码时保留在 unknown fields,原样存 `payload` 字节,
  回放时不丢——这正是 §3 推荐"整条消息原始字节存档"的原因;
- **老客户端遇到新 payload**:解码后 `payload = None`(未知分支),
  信封字段完好 → 渲染占位卡片"[不支持的消息]",不允许崩溃或丢弃。
- 跨版本窗口期只保证:追加类改动(新字段/新枚举值/新 oneof 分支)
  完全兼容;破坏性改动(改编号/改类型)必须停机切换或走新 notebook。

---

## 5. 联调

glaspen2 侧已内置客户端与演示发送器:

```bash
# 模拟(默认,不连服务):
cargo run -p glaspen-chat --bin mock-send

# 真连(你的 axum 服务启动后):
GLASPEN_CHAT_MOCK=0 cargo run -p glaspen-chat --bin mock-send -- --grpc
GLASPEN_CHAT_ENDPOINT=http://127.0.0.1:50051 \
GLASPEN_CHAT_MOCK=0 cargo run -p glaspen-chat --bin mock-send -- --grpc
```

预期:服务端实现完成后,demo 会发送 4 条消息(系统页头 → 笔迹 → 图片 →
引用笔迹的卡片),客户端打印 `append done: first_msg_id=..., accepted=4,
seqs=[1,2,3,4]`。若服务未实现该服务,会收到 `status: Unimplemented`。

调试利器 grpcurl(未实现反射时需要 proto 文件 + import 路径):

```bash
grpcurl -plaintext -import-path proto -proto glaspen/chat/v1/chat.proto \
  -d '{"notebook_id":"nb1","after_seq":0,"limit":10}' \
  127.0.0.1:50051 glaspen.chat.v1.ChatStore/FetchMessages
```

---

## 6. 鉴权与运维(简注)

- 本地服务默认只监听 `127.0.0.1`,无 TLS;若要加鉴权,走 gRPC metadata
  (如 `authorization: Bearer <token>`),在 tonic 拦截层校验,协议本身
  不预留字段。
- 数据目录 = SQLite 文件 + `media/`,整体可拷贝备份;
  按内容寻址的 media 可以放心做引用计数清理(删消息不删 media,
  定期 GC 无引用的 sha256 即可)。
