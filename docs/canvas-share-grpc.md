# 共享画布上行(ShareInk)—— axum 侧实现指南

本文写给实现本地 axum 服务的工程,描述 glaspen2 的**共享画布上行**如何
对接。设计哲学一句话:**glaspen2 只作为手写工具** —— 设置面板切到
「共享画布」tab 期间,画布上每一笔抬笔提交都实时推给 axum;axum 把
笔迹转给该用户在 kongde 打开的接收页。glaspen2 不关心接收端是谁、
连接是否成功(失败静默),也没有任何房间/配对概念。

- 协议契约:`proto/glaspen/chat/v1/chat.proto`(与
  [axum-chat-store.md](axum-chat-store.md) 同一个文件,纯追加)。
- 面向用户的开关:glaspen2 设置面板「启用手写消息与共享」,默认关闭,
  关闭时第 4 个 tab(共享画布)不存在、不建流。
- glaspen2 侧客户端参考实现:`chat/src/canvas.rs`
  (含 `ink_share_live_session` 联测,反向断言即本文件 §3 的规格)。

---

## 1. 快速接入

### 1.1 RPC 签名

```proto
// service glaspen.chat.v1.ChatStore 内新增(客户端流,应答为一条回执):
rpc ShareInk(stream InkFrame) returns (ShareInkReply);
```

tonic 生成:
`async fn share_ink(&self, request: Request<Streaming<InkFrame>>)
-> Result<Response<ShareInkReply>, Status>`(无流关联类型)。

### 1.2 帧定义

```proto
message InkFrame {
  oneof frame {
    ShareBegin begin = 1;   // 首帧:会话上下文
    ShareStroke stroke = 2; // 中间帧:一条抬笔提交的笔迹
    ShareEnd end = 3;       // 末帧:tab 关闭/面板退出,即将 half-close
  }
}
message ShareBegin {
  string session_id = 1;    // 客户端生成,日志/幂等用
  int64 started_at_ms = 2;
  string author = 3;        // 当前为空串;作者以 metadata token 为准
  string device = 4;
}
message ShareStroke {
  uint32 color_rgb = 2;     // 0xRRGGBB
  double width_scale = 3;   // 恒 1.0(线宽逐点携带)
  repeated StrokePoint points = 4;
}
message ShareEnd { uint32 stroke_count = 1; uint64 duration_ms = 2; }
message ShareInkReply { bool ok = 1; string message = 2; uint64 stroke_count = 3; }
```

与 DraftInk 同构(客户端流 + 末尾回执),但**语义不同**:

| | DraftInk(⌘⌃2) | **ShareInk(共享画布 tab)** |
|---|---|---|
| 会话生命周期 | 按住快捷键 | 面板「共享画布」tab 打开(可能数小时) |
| 笔迹去向 | axum 缓冲,流结束**由 axum 决定**是否发送 | **实时转给该用户的接收页**(kongde 已打开) |
| 回执意义 | 发送/丢弃的决定 | 仅调试统计,客户端不读 |

## 2. 服务端语义

1. **入流**:首帧必须是 `begin`(校验失败 `invalid_argument`)。
   建流即代表"该用户的共享画布会话开始"。
2. **实时转发(核心)**:每到一个 `stroke` 帧,立刻转给该用户当前
   注册的接收端(与 DraftInk 同一套 ink-route:用户在 kongde 打开
   接收页时经 `POST /api/chat/ink-route` 注册)。转发要**低延迟**
   ——逐帧转发,不要攒批。没有注册接收端时静默丢弃(或缓冲最新一帧),
   不报错:glaspen2 对失败完全无感,这是产品要求。
3. **不落库、不回放**:axum 只做路由转发;是否把整段涂鸦留档由
   kongde 接收页自行决定。
4. **结束**:收到 `end` 或流断开 = 会话结束,转发通道随之关闭;
   `ShareInkReply` 随意填(`ok:true, stroke_count:实收笔数`),
   客户端只打日志。
5. **身份**:与 DraftInk 相同 —— metadata `authorization: Bearer <JWT>`;
   无 token 走过渡期行为(全局路由),后续 `GLASPEN_REQUIRE_GRPC_AUTH=1`
   切换后强制拒绝。**作者 = token 用户**,`begin.author` 可忽略。
6. **多会话**:同一用户同时只应有一条活跃 ShareInk(glaspen2 侧保证
   单实例单会话);新流到来时可挤掉旧流。

## 3. stroke 帧载荷

| 字段 | 语义 |
|---|---|
| `points[i].x/.y` | 画布坐标:活页本模式 = 页内坐标(原点左上,y 向下);无限画布模式 = 画布绝对坐标。单位逻辑点 @1x。**v1 不区分两种模式**,接收端按需要自行归一化(建议按包围盒缩放渲染) |
| `points[i].width` | 该点线宽(px @1x,含笔锋:0.3–8 常见) |
| `points[i].t_rel` | 相对该笔画起点的时间(秒),笔顺回放用 |
| `color_rgb` | `0xRRGGBB` |
| `width_scale` | 恒 1.0 |

粒度 = **抬笔提交**(一发即整笔);`points` 非空、按书写顺序。

## 4. 接收端体验(kongde)

用户在 kongde 打开接收页(注册 ink-route)→ 回到画布切到「共享画布」
tab → 直接书写。kongde 接收页逐笔渲染(按 `t_rel` 可做笔顺动画);
glaspen2 侧涂鸦照常保存在本机活页本里,与上行互不影响。

## 5. 联调

```bash
# glaspen2(mock 默认开,stderr 打印逐帧摘要):
cargo run
# 真连:
GLASPEN_CHAT_MOCK=0 GLASPEN_CHAT_ENDPOINT=http://<nas>:50053 cargo run
```

验证:面板开启「启用手写消息与共享」→ 切到「共享画布」tab → 在画布上
写几笔 → 服务端每抬笔收到一帧;切走 tab 收到 end + 回执。
glaspen2 侧日志前缀 `[share-ink]`。

grpcurl 手工验证(客户端流):

```bash
grpcurl -plaintext -import-path proto -proto glaspen/chat/v1/chat.proto \
  -d @ 127.0.0.1:50053 glaspen.chat.v1.ChatStore/ShareInk <<'EOF'
{"begin":{"session_id":"demo-1","started_at_ms":1727500000000}}
{"stroke":{"color_rgb":16711680,"width_scale":1,
  "points":[{"x":0,"y":0,"width":2,"t_rel":0},{"x":40,"y":12,"width":3.2,"t_rel":0.08}]}}
{"end":{"stroke_count":1,"duration_ms":600}}
EOF
```

## 6. 版本兼容

纯追加改动;老服务端收到 `ShareInk` 返回 `Unimplemented`,glaspen2
静默降级(面板 tab 依旧存在,笔迹只留本地),其余功能不受影响。
proto 演进沿用 [axum-chat-store.md](axum-chat-store.md) §4 的规则。
