//! NAS 备份: 把整库一致性快照(VACUUM INTO, 含全部笔迹/设置/缩略图)
//! 推送到用户 NAS 上的 axum 服务。传输形态 = **文件**, 不是结构化行 ——
//! Postgres 只做登记(device_uuid/时间/大小/sha256), NAS 磁盘存快照;
//! 契约与 NAS 侧参考实现见 docs/nas-backup.md。
//!
//! 触发 = 面板「立即备份」按钮(FRB nas_backup_now → 本模块)。铁律:
//! 失败只进返回 JSON 与日志, 永不阻断涂鸦; 快照在临时目录生成后即删。
//! v1 不带鉴权(局域网 NAS), 需要时再挂 chat 同款 token。

use serde_json::json;

/// 上传超时: 局域网百 MB 级快照也要留足余量。
const UPLOAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);

/// 执行一次备份, 返回给面板的结果 JSON:
/// `{"ok":true,"size_bytes":..,"sha256":"..","stroke_count":..,
///   "elapsed_ms":..,"server":{...}}` 或 `{"ok":false,"error":"..","hint":".."}`。
pub fn backup_now() -> String {
    let t0 = std::time::Instant::now();
    match backup_inner() {
        Ok(mut v) => {
            v["ok"] = json!(true);
            v["elapsed_ms"] = json!(t0.elapsed().as_millis() as u64);
            tracing::info!(elapsed_ms = v["elapsed_ms"].as_u64().unwrap_or(0),
                           "NAS 备份完成: {}", v);
            v.to_string()
        }
        Err((error, hint)) => {
            tracing::warn!("NAS 备份失败: {error}");
            json!({ "ok": false, "error": error, "hint": hint }).to_string()
        }
    }
}

fn backup_inner() -> Result<serde_json::Value, (String, String)> {
    let nas_url = crate::runtime()
        .block_on(crate::db::load_setting("nas_url"))
        .unwrap_or_default();
    let nas_url = nas_url.trim().trim_end_matches('/').to_string();
    if nas_url.is_empty() {
        return Err((
            "未配置 NAS 地址".into(),
            "设置 → 数据 → NAS 备份, 填入 NAS 上 axum 服务的地址".into(),
        ));
    }

    // 1) 一致性快照到临时目录(VACUUM INTO; 目标必须不存在)
    let ts_ms = chrono::Utc::now().timestamp_millis().unsigned_abs();
    let tmp = std::env::temp_dir().join(format!(
        "glaspen2-nas-{}-{}.db",
        std::process::id(),
        ts_ms
    ));
    let _ = std::fs::remove_file(&tmp);
    crate::runtime()
        .block_on(crate::db::snapshot_to(&tmp))
        .map_err(|e| (format!("生成本地快照失败: {e}"), "磁盘空间不足?".into()))?;

    let result = upload(&nas_url, &tmp, ts_ms);
    let _ = std::fs::remove_file(&tmp); // 无论成败都清理临时快照
    result
}

/// sha256(流式读文件)+ 上传 + 解析 NAS 应答。
fn upload(
    nas_url: &str,
    snapshot: &std::path::Path,
    ts_ms: u64,
) -> Result<serde_json::Value, (String, String)> {
    use sha2::Digest;
    use std::io::Read;
    let mut file = std::fs::File::open(snapshot)
        .map_err(|e| (format!("打开快照失败: {e}"), String::new()))?;
    let size_bytes = file
        .metadata()
        .map_err(|e| (format!("读取快照大小失败: {e}"), String::new()))?
        .len();
    let mut hasher = sha2::Sha256::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| (format!("读取快照失败: {e}"), String::new()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let sha256: String =
        hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();

    let stroke_count =
        crate::runtime().block_on(crate::db::stroke_count());
    let device = crate::device::device_uuid();

    let url = format!("{nas_url}/api/glaspen/backups");
    let tls = ureq::tls::TlsConfig::builder()
        .disable_verification(true) // 局域网自签放行(chat 同款)
        .build();
    let config = ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_global(Some(UPLOAD_TIMEOUT))
        .build();
    let agent = ureq::Agent::new_with_config(config);

    // 重新打开文件从头流式上传(body = 原始字节流, NAS 侧直接落盘)
    let mut body = std::fs::File::open(snapshot)
        .map_err(|e| (format!("打开快照失败: {e}"), String::new()))?;
    let mut resp = agent
        .post(&url)
        .header("Content-Type", "application/octet-stream")
        .header("X-Device-Uuid", &device)
        .header("X-Backup-Ts-Ms", ts_ms.to_string())
        .header("X-SHA256", &sha256)
        .header("X-Stroke-Count", stroke_count.to_string())
        .send(ureq::SendBody::from_reader(&mut body))
        .map_err(|e| {
            (
                format!("上传到 {url} 失败: {e}"),
                "检查 NAS 是否可达、地址是否正确".into(),
            )
        })?;
    let text = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| (format!("读取 NAS 应答失败: {e}"), String::new()))?;
    let v: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| (format!("NAS 应答不是合法 JSON: {e}"), text.into()))?;
    if v.get("ok").and_then(|b| b.as_bool()) != Some(true) {
        let msg = v
            .get("error")
            .and_then(|m| m.as_str())
            .unwrap_or("(NAS 未返回 error)");
        return Err((format!("NAS 拒绝备份: {msg}"), String::new()));
    }
    Ok(json!({
        "size_bytes": size_bytes,
        "sha256": sha256,
        "stroke_count": stroke_count,
        "server": v,
    }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn unconfigured_returns_hint() {
        // 未配置地址(测试 DB 无 nas_url)→ 结构化失败 + 提示
        let v: serde_json::Value = serde_json::from_str(&super::backup_now()).unwrap();
        assert_eq!(v["ok"], false);
        assert!(v["error"].as_str().unwrap().contains("未配置"));
        assert!(!v["hint"].as_str().unwrap().is_empty());
    }

    #[test]
    fn url_trailing_slash_is_trimmed() {
        // 纯字符串处理: 保证拼接端点不会出现双斜杠
        let u = "http://nas:8080/".trim_end_matches('/').to_string();
        assert_eq!(format!("{u}/api/glaspen/backups"), "http://nas:8080/api/glaspen/backups");
    }
}
