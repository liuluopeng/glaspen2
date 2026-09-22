//! 「检查更新」:对比 GitHub Releases 上的最新正式版与当前编译版本。
//!
//! 结构刻意分成两层,单测只碰纯函数:
//!
//! - **纯函数**([`parse_release`] / [`parse_version`] / [`is_newer`]) —
//!   解析与版本比较,`cargo test` 直接跑,不碰网络;
//! - **网络**([`fetch_latest`]) — 走 GitHub REST API(`releases/latest`
//!   只返回正式版,draft / prerelease 不会出现),10 秒超时。
//!
//! 检查是**手动触发**的(设置面板「关于 → 检查更新」),不在启动时自动联网。
//!
//! 调用方有两处:macOS 设置面板经 flutter_rust_bridge(`crate::api`),Windows
//! 设置面板是独立进程,经命名管道(`src/windows/overlay.rs`)。

use std::time::Duration;

/// GitHub REST API:最新正式版 release。
pub const RELEASES_API: &str = "https://api.github.com/repos/liuluopeng/glaspen2/releases/latest";

/// 下载页(`releases/latest` 会重定向到最新 tag),给「打开下载页」用。
pub const RELEASES_PAGE: &str = "https://github.com/liuluopeng/glaspen2/releases/latest";

/// GitHub API 不带 User-Agent 会直接 403。
const USER_AGENT: &str = concat!("glaspen2/", env!("CARGO_PKG_VERSION"));

/// 网络请求超时:面板按钮点了必须在可预期的时间内给出结果。
const TIMEOUT: Duration = Duration::from_secs(10);

/// 最新发布(GitHub release 对象的子集)。
#[derive(Debug, Clone, PartialEq)]
pub struct LatestRelease {
    /// 原始 tag,如 `v0.5.1`。
    pub tag: String,
    /// release 页面地址(`html_url`)。
    pub url: String,
    /// release 标题(常为空,展示时退回 tag)。
    pub name: String,
    /// 发布时间(RFC3339,如 `2026-09-16T12:02:48Z`)。
    pub published_at: String,
}

/// 当前版本:编译时取自 `Cargo.toml`,与发布物一一对应。
pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

// ---------------------------------------------------------------------------
// 网络
// ---------------------------------------------------------------------------

fn agent() -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        // 只走 https:这里不解析任何明文 URL,GitHub 也不会降级。
        .https_only(true)
        .build();
    ureq::Agent::new_with_config(config)
}

/// 请求 `releases/latest` 并解析。失败时 `Err` 是**给用户看的中文原因**。
pub fn fetch_latest() -> Result<LatestRelease, String> {
    let mut resp = match agent()
        .get(RELEASES_API)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .call()
    {
        Ok(r) => r,
        Err(ureq::Error::StatusCode(404)) => return Err("GitHub 上还没有发布版本".into()),
        Err(ureq::Error::StatusCode(403)) | Err(ureq::Error::StatusCode(429)) => {
            return Err("请求被 GitHub 限流,请稍后再试".into());
        }
        Err(ureq::Error::StatusCode(c)) => return Err(format!("GitHub 接口返回 {c}")),
        Err(e) => return Err(format!("网络错误:{e}")),
    };
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("读取响应失败:{e}"))?;
    parse_release(&body)
}

// ---------------------------------------------------------------------------
// 纯函数(单测覆盖,不碰网络)
// ---------------------------------------------------------------------------

/// 从 GitHub release 对象(JSON 文本)取出需要的字段。
pub fn parse_release(json: &str) -> Result<LatestRelease, String> {
    let v: serde_json::Value =
        serde_json::from_str(json).map_err(|_| "GitHub 返回的数据无法解析".to_string())?;
    let tag = v
        .get("tag_name")
        .and_then(|x| x.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "GitHub 返回的版本号为空".to_string())?
        .to_string();
    let url = v
        .get("html_url")
        .and_then(|x| x.as_str())
        .filter(|s| s.starts_with("https://") || s.starts_with("http://"))
        .unwrap_or(RELEASES_PAGE)
        .to_string();
    let field = |k: &str| {
        v.get(k)
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string()
    };
    Ok(LatestRelease {
        tag,
        url,
        name: field("name"),
        published_at: field("published_at"),
    })
}

/// `"v1.2.3"` / `"1.2.3-beta.1"` → `[1, 2, 3]`。
///
/// 只取开头的 semver 数字核心:遇到非数字非 `.` 的字符(预发布/构建后缀)就
/// 停 —— 也就是 `1.2.3-rc1` 与 `1.2.3` 等价,不会被当成比正式版新(也不会
/// 因为 `-beta.1` 里还带一个点而多出一段)。
pub fn parse_version(v: &str) -> Vec<u64> {
    let core: String = v
        .trim()
        .trim_start_matches(['v', 'V'])
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    core.split('.')
        .map(|seg| seg.parse().unwrap_or(0))
        .collect()
}

/// 最新版本是否比当前版本新(段数不同时缺段按 0)。
pub fn is_newer(latest: &str, current: &str) -> bool {
    let a = parse_version(latest);
    let b = parse_version(current);
    for i in 0..a.len().max(b.len()) {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        if x != y {
            return x > y;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// 真实 GitHub API 返回的裁剪样例(字段形状与线上一致)。
    const SAMPLE: &str = r#"{
        "tag_name": "v0.5.1",
        "html_url": "https://github.com/liuluopeng/glaspen2/releases/tag/v0.5.1",
        "name": "",
        "published_at": "2026-09-20T08:00:00Z",
        "draft": false,
        "prerelease": false,
        "body": "……"
    }"#;

    #[test]
    fn test_parse_release_reads_fields() {
        let r = parse_release(SAMPLE).unwrap();
        assert_eq!(r.tag, "v0.5.1");
        assert_eq!(
            r.url,
            "https://github.com/liuluopeng/glaspen2/releases/tag/v0.5.1"
        );
        assert_eq!(r.name, "");
        assert_eq!(r.published_at, "2026-09-20T08:00:00Z");
    }

    #[test]
    fn test_parse_release_falls_back_to_download_page_without_url() {
        let r = parse_release(r#"{"tag_name":"v1.0.0"}"#).unwrap();
        assert_eq!(r.url, RELEASES_PAGE);
        assert_eq!(r.published_at, "");
    }

    #[test]
    fn test_parse_release_rejects_garbage_and_missing_tag() {
        assert!(parse_release("not json").is_err());
        assert!(parse_release("{}").is_err());
        assert!(parse_release(r#"{"tag_name":"  "}"#).is_err());
        // 没有 tag 的对象不能当成一个 release
        assert!(parse_release(r#"{"html_url":"https://example.com"}"#).is_err());
        // 非 http(s) 的 html_url 不采信, 退回下载页
        let r = parse_release(r#"{"tag_name":"v1.0.0","html_url":"javascript:alert(1)"}"#).unwrap();
        assert_eq!(r.url, RELEASES_PAGE);
    }

    #[test]
    fn test_parse_version_shapes() {
        assert_eq!(parse_version("v0.5.1"), vec![0, 5, 1]);
        assert_eq!(parse_version("0.5.1"), vec![0, 5, 1]);
        assert_eq!(parse_version("V1.2"), vec![1, 2]);
        assert_eq!(parse_version("1.2.3-beta.1"), vec![1, 2, 3]);
        assert_eq!(parse_version("0.6"), vec![0, 6]);
        assert_eq!(parse_version(""), vec![0]);
    }

    #[test]
    fn test_is_newer() {
        assert!(is_newer("v0.6.0", "0.5.1"));
        assert!(is_newer("v0.5.10", "0.5.9"));
        assert!(is_newer("v1.0.0", "0.9.9"));
        assert!(is_newer("0.5.2", "0.5.1"));
        // 同版本 / 开发版比发布版新 / tag 少一段且补 0 相等
        assert!(!is_newer("v0.5.1", "0.5.1"));
        assert!(!is_newer("v0.5.1", "0.5.2"));
        assert!(!is_newer("v0.5", "0.5.0"));
        assert!(!is_newer("", "0.5.1"));
    }

    #[test]
    fn test_current_version_is_semver_shaped() {
        assert!(!current_version().is_empty());
        assert!(current_version().contains('.'));
    }

    /// 真打一次 GitHub(发版前手动跑一次,验证 TLS/限流/解析整条链路):
    /// `cargo test update:: -- --ignored`
    #[test]
    #[ignore = "需要网络, 默认跳过; 手动: cargo test update:: -- --ignored"]
    fn test_fetch_latest_live() {
        let r = fetch_latest().expect("联网检查更新失败");
        assert!(!r.tag.is_empty(), "tag 为空");
        assert!(
            r.url
                .starts_with("https://github.com/liuluopeng/glaspen2/releases"),
            "意外的下载页地址: {}",
            r.url
        );
        // 当前版本是开发版时"有更新"可真可假,但 same-version 必须是 false
        assert!(!is_newer(current_version(), current_version()));
    }
}
