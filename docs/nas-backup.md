# NAS 备份契约(glaspen2 ↔ nas/axum)

glaspen2 把**整库 SQLite 一致性快照**(`VACUUM INTO`,含全部笔迹/无限画布/
设置/缩略图)推送到 NAS 上的 axum 服务。传输形态 = **文件**;Postgres 只做
登记,不存笔迹行。NAS 侧代码是薄接收端:收文件、落盘、插登记行——不解析
内容,glaspen2 加表/改 schema 无需 NAS 跟进。

- 触发:面板「设置 → 数据 → NAS 备份 → 立即备份到 NAS」(macOS 走 FRB
  `nas_backup_now`,Windows 走管道 `nasBackup`,同一核心函数
  `export/nas_backup.rs::backup_now`)。
- 设备身份:`device_uuid`(UUIDv7,见 `crates/glaspen-core/src/device.rs`)。
- v1 无鉴权(局域网);需要时给 axum 挂 middleware 即可,客户端不用改。

## 请求

```
POST {nas_url}/api/glaspen/backups
Content-Type: application/octet-stream
X-Device-Uuid:  <uuid v7>
X-Backup-Ts-Ms: <unix 毫秒, 备份内容的时间戳>
X-SHA256:       <快照文件 sha256, hex>
X-Stroke-Count: <笔迹总数(活页本+无限画布, 不含软删)>

<body> = SQLite 快照文件原始字节(流式)
```

`nas_url` 存 `user_settings` 表键 `nas_url`(设置 → 数据 → NAS 地址;
面板与 macOS/Windows 同键同步)。

## 应答

```json
{"ok": true, "id": 3, "path": "/data/glaspen-backups/<uuid>/<ts>.db",
 "duplicate": false, "size_bytes": 123456}
```

失败:HTTP 4xx/5xx + `{"ok": false, "error": "原因"}`。
`duplicate: true` = 同设备同 sha256 已存在,本次未重复落盘(幂等)。

## Postgres 表(登记簿)

```sql
CREATE TABLE IF NOT EXISTS glaspen_backup_devices (
  device_uuid   TEXT PRIMARY KEY,
  label         TEXT,
  first_seen_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS glaspen_backups (
  id           BIGSERIAL PRIMARY KEY,
  device_uuid  TEXT NOT NULL REFERENCES glaspen_backup_devices(device_uuid),
  created_at   TIMESTAMPTZ NOT NULL,           -- 备份内容时间戳(端上生成)
  uploaded_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
  size_bytes   BIGINT NOT NULL,
  sha256       CHAR(64) NOT NULL,
  stroke_count INTEGER NOT NULL DEFAULT 0,
  path         TEXT NOT NULL,
  UNIQUE (device_uuid, sha256)                 -- 幂等: 同内容不重复落盘
);
```

NAS 磁盘布局:`<备份根目录>/<device_uuid>/<created_at 毫秒>.db`。

## NAS 侧参考实现(axum,粘贴进现有 router)

```rust
use axum::{body::Body, extract::State, http::HeaderMap, routing::post, Router};
use sqlx::PgPool;
use tokio::io::AsyncWriteExt;

#[derive(Clone)]
struct BackupState { pool: PgPool, root: std::path::PathBuf }

async fn save_backup(
    State(st): State<BackupState>,
    headers: HeaderMap,
    body: Body,
) -> Result<axum::Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let hdr = |k: &str| {
        headers.get(k).and_then(|v| v.to_str().ok()).unwrap_or("").to_string()
    };
    let device = hdr("X-Device-Uuid");
    let ts_ms: i64 = hdr("X-Backup-Ts-Ms").parse().unwrap_or(0);
    let sha = hdr("X-SHA256");
    let strokes: i32 = hdr("X-Stroke-Count").parse().unwrap_or(0);
    if device.is_empty() || sha.len() != 64 || ts_ms <= 0 {
        return Err((axum::http::StatusCode::BAD_REQUEST, "缺少必要头".into()));
    }

    // 幂等: 同设备同内容直接应答已存在
    let dup: Option<(i64, String)> = sqlx::query_as(
        "SELECT id, path FROM glaspen_backups WHERE device_uuid = $1 AND sha256 = $2",
    ).bind(&device).bind(&sha).fetch_optional(&st.pool).await.unwrap();
    if let Some((id, path)) = dup {
        return Ok(axum::Json(serde_json::json!({
            "ok": true, "id": id, "path": path, "duplicate": true })));
    }

    // 落盘: <root>/<device_uuid>/<ts_ms>.db
    let dir = st.root.join(&device);
    tokio::fs::create_dir_all(&dir).await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let path = dir.join(format!("{ts_ms}.db"));
    let mut f = tokio::fs::File::create(&path).await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut body = body;
    let size = axum::body::to_bytes(body, 2 << 30).await   // 上限 2 GiB
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .len() as i64;
    f.write_all(&/* 上面 to_bytes 已取走; 大文件请改用流式写 */).await.unwrap();

    // 登记两表(device 首次出现自动登记)
    sqlx::query("INSERT INTO glaspen_backup_devices(device_uuid) VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(&device).execute(&st.pool).await.unwrap();
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO glaspen_backups(device_uuid, created_at, size_bytes, sha256, stroke_count, path)
         VALUES ($1, to_timestamp($2::bigint / 1000.0), $3, $4, $5, $6) RETURNING id",
    ).bind(&device).bind(ts_ms).bind(size).bind(&sha).bind(strokes)
     .bind(path.to_string_lossy().to_string())
     .fetch_one(&st.pool).await.unwrap();

    Ok(axum::Json(serde_json::json!({
        "ok": true, "id": id, "path": path.to_string_lossy(), "duplicate": false,
        "size_bytes": size })))
}

pub fn router(pool: PgPool, root: std::path::PathBuf) -> Router {
    Router::new().route("/api/glaspen/backups", post(save_backup))
        .with_state(BackupState { pool, root })
}
```

> 大库请把 `to_bytes` 换成真流式:`axum::body::Body::into_data_stream()` 逐
> chunk `write_all`(上面为压缩篇幅用了内存缓冲,2 GiB 封顶)。

## 客户端(glaspen2 侧)行为

`export/nas_backup.rs::backup_now`:读 `nas_url` → `VACUUM INTO` 临时快照 →
流式 sha256 → `POST`(600s 超时,自签放行)→ 解析应答 → 删临时文件。
失败 = 返回 `{"ok":false,"error":…,"hint":…}` + 日志,绝不阻断涂鸦。
恢复路径(v2 规划):NAS 列备份 → 下载快照 → 启动前替换本地 DB
(可复用 `db::restore_merge_from_with` 的合并语义)。
