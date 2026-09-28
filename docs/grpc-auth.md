# 涂鸦身份对接（gRPC metadata 鉴权）—— glaspen2 侧改造指南

本文写给 glaspen2 侧，说明如何给 ChatStore gRPC 调用（⌘⌃2 草稿
`DraftInk`、⌘⌃3 直发 `AppendMessages` 等）加上**登录身份**，使涂鸦
消息有明确的作者，不再依赖 axum 侧"最近注册的会话"来猜。

协议契约仍是 `proto/glaspen/chat/v1/chat.proto` 一个文件，**本次不改
proto**——身份走 gRPC metadata（[axum-chat-store.md §6](axum-chat-store.md)
预留的方案），线格式零变化。

---

## 1. 背景：为什么需要身份

DraftInk 的 `DraftBegin.author` 恒为空串，axum 只能用"kongde 最近打开
的会话"猜涂鸦归属。当同一台机器上两个账号的 kongde 都开着会话页时
（网页 + 桌面），后打开的会顶掉先打开的，涂鸦就被记到别人头上。

有身份后：**涂鸦作者 = token 用户**，接收方 = 该用户自己在 kongde
注册的目标（按用户隔离，互不干扰）。

---

## 2. axum 侧行为规格（已实现， glaspen2 按此对接）

### 2.1 metadata 约定

所有 ChatStore RPC 的请求 metadata 附带：

```
authorization: Bearer <JWT>
```

JWT 与 kongde 登录态同源（同一 secret 签发），axum 侧用与 HTTP 完全
相同的校验逻辑。

### 2.2 draft_ink 的身份语义

| 场景 | axum 行为 |
|---|---|
| 带 token 且有效 | from = token 用户；to = 该用户注册的 ink-route |
| 该用户没注册过 ink-route | `Status::failed_precondition`（提示先打开一次目标会话页） |
| 带 token 但无效/过期 | `Status::unauthenticated`（客户端应重新登录） |
| 完全不带 token | **过渡期回退**：沿用旧行为（全局"最近注册"路由），服务端打警告日志；后续版本将以配置强制拒绝 |

有身份时不再读取全局"最近注册"，双账号同时在线互不影响。

### 2.3 接收目标（ink-route）依旧由 kongde 注册

kongde 打开点对点会话页 / 在会话里发消息时，调

```
POST /api/chat/ink-route        (Authorization: Bearer <JWT>)
{"peer": "<对方用户 uuid>"}
```

axum 按用户存一行（`chat_ink_route` 表）。glaspen2 **不需要**实现这个
调用——你只负责把自己的身份带上来。

---

## 3. glaspen2 侧改造清单

### 3.1 新增配置（环境变量，均可缺省）

| 变量 | 含义 | 缺省 |
|---|---|---|
| `GLASPEN_API_BASE` | axum HTTP(S) 基址，**登录用**（注意与 gRPC 端口不同：dev 例 `https://192.168.31.58:23001`，gRPC 是 50053） | 无 |
| `GLASPEN_CHAT_USER` | 登录用户名 | 无 |
| `GLASPEN_CHAT_PASSWORD` | 登录密码 | 无 |
| `GLASPEN_CHAT_TOKEN` | 直接给 token（跳过登录，二选一） | 无 |

**未配置任何账号 = 不鉴权**：不加 metadata，走 axum 过渡行为（现状），
完全向后兼容。`GLASPEN_CHAT_ENDPOINT` 含义不变（gRPC 地址）。

### 3.2 登录换 token

```
POST {GLASPEN_API_BASE}/api/user/login
Content-Type: application/json

{"username": "...", "password": "...", "player_name": "glaspen2"}
```

成功 200：

```json
{"msg":"ok","data":{"token":"<JWT>"}}
```

要点：

- **TLS**：dev 的 23001 是自签证书（Caddy local）。HTTP 客户端需放行
  自签（reqwest：`danger_accept_invalid_certs(true)`；或把 Caddy local
  CA 装进系统信任，kongde 侧两者都做了）。
- token **内存缓存**即可，不必落盘；过期/被登出会拿到 401 或 gRPC
  `unauthenticated`，此时重新登录换新 token。
- 登录失败（网络不通/密码错）→ 降级为无 metadata 发送（过渡期仍可
  用），stderr 打一条 `[ink-draft] 登录失败，涂鸦将无身份` 即可，
  **不要**阻断涂鸦本身。

### 3.3 请求附带 metadata

每次构造 ChatStore 请求（`DraftInk` 流、`AppendMessages` 等）时：

```rust
use tonic::metadata::MetadataValue;

let mut req = tonic::Request::new(stream);
if let Ok(v) = MetadataValue::try_str(&format!("Bearer {token}")) {
    req.metadata_mut().insert("authorization", v);
}
```

注意：token 在**会话开始时**取一次即可（⌘⌃2 key-down 打开通道时）；
`DraftChannel::launch(begin)` 之类的封装需要把 token 传进去或暴露
`set_token`。

### 3.4 错误处理

| 信号 | 处理 |
|---|---|
| `Status::unauthenticated` | token 失效 → 清缓存 → 重新登录 → 提示"涂鸦身份已过期"（本次草稿按现有 Failed 通知路径） |
| `Status::failed_precondition`（未注册路由） | 提示"先在 kongde 打开一次要发送目标者的会话页" |
| 登录请求失败 | 降级无身份发送 + stderr 提示（过渡期不阻断） |

mock 模式（`GLASPEN_CHAT_MOCK` ≠ 0）完全不受影响。

---

## 4. axum 侧计划中的切换

- 当前为**过渡模式**：有 token 用身份，没 token 走旧全局路由。
- 后续 axum 增加 `GLASPEN_REQUIRE_GRPC_AUTH=1`：无 token 一律
  `unauthenticated`。glaspen2 先上线登录即可平滑切换，无需停机配合。
- ⌘⌃3 直发（AppendMessages）同样附 metadata 后，axum 后续会把直发
  笔迹也归到作者名下进聊天流（另行实现，协议无变化）。

---

## 5. 联调

```bash
# glaspen2 侧（示意）：
GLASPEN_CHAT_MOCK=0 \
GLASPEN_CHAT_ENDPOINT=http://127.0.0.1:50053 \
GLASPEN_API_BASE=https://192.168.31.58:23001 \
GLASPEN_CHAT_USER=abc GLASPEN_CHAT_PASSWORD=xxxx \
cargo run
```

验证点：

1. kongde 以 abc 登录、打开与 def 的会话页（注册路由）。
2. glaspen2 以 **abc** 身份登录后按住 ⌘⌃2 涂鸦、松开。
3. kongde 的 def 会话页实时出现涂鸦，消息作者是 **abc**；
   即使 def 的网页同时开着会话页，归属也不会变。
4. axum 日志不再出现 "无身份 token，回退全局最近路由"。

---

## 6. glaspen2 侧实现状态（已落地）

对照本清单的实现位置：

| 项 | 位置 |
|---|---|
| 配置解析 / 登录换 token / 内存缓存 / 失效 | `chat/src/auth.rs`（`AuthConfig::from_env` / `token()` / `invalidate()`） |
| 自签证书放行 | `auth.rs::login_blocking`（ureq `TlsConfig::disable_verification(true)`，等效 `danger_accept_invalid_certs`） |
| DraftInk 附 metadata | `chat/src/draft.rs::run_grpc`（`launch()` 生产入口在会话开启时取一次 token，等价 §3.3 的"传 token / set_token"） |
| AppendMessages 附 metadata | `chat/src/lib.rs` `Sink::append`（Grpc 分支；`unauthenticated` 时清缓存） |
| unauthenticated / failed_precondition 映射 | `draft.rs::run_grpc` → 用户可读文案（"涂鸦身份已过期…" / "先在 kongde 打开一次要发送目标者的会话页"），经 `glaspen2_ink_draft_last_error()`（export.rs）回给 ObjC 通知 |
| 登录失败降级 | stderr `[ink-draft] 登录失败，涂鸦将无身份(不阻断涂鸦)`，本次请求不带 metadata |
| mock 不受影响 | mock 分支不解析身份 |
| **设置面板登录（⌘⌃2 所在 GUI）** | Flutter 面板「涂鸦身份」区（`flutter_settings/lib/main.dart`）：服务地址 / 用户名 / 密码三个字段 + 「保存并测试登录」。经 FRB `setSetting('chatApiBase'|'chatUser'|'chatPassword')` 落 DB（`user_settings` 的 `chat_api_base` / `chat_user` / `chat_password`），ObjC 侧 `glaspen2_chat_auth_reload()`（export.rs）把 DB 值字段级合并环境变量后注入 `chat::auth::set_config`（配置变化自动清 token 缓存）。「测试登录」走 FRB `testChatLogin()` → `chat::auth::force_login()`，成功缓存 token。密码不回显（快照只带 `chatHasPassword`），留空提交 = 保持已存。**DB 与环境变量都未配置 = 不鉴权**；DB 有值时优先于环境变量 |

补充约定：

- **部分配置视为未配置**：四个变量缺任何一个能登录的组合（如给了
  `GLASPEN_API_BASE` 但没给用户名/密码），一律静默走无身份路径，
  不报错、不阻断涂鸦。
- token 失效后**当前这份草稿**按 Failed 通知"涂鸦身份已过期，请重新
  书写一次"；重新登录发生在下一次 ⌘⌃2/⌘⌃3 会话开始时（即 §3.4 的
  语义，不做流内自动重试，避免把半截流重放给服务端）。
