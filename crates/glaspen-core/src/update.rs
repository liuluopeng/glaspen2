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
    /// release notes(确认对话框里展示)。
    pub notes: String,
    /// 安装包资产(供「立即更新」挑包下载)。
    pub assets: Vec<Asset>,
}

/// release 里的一个安装包(GitHub `assets[]` 子集)。
#[derive(Debug, Clone, PartialEq)]
pub struct Asset {
    pub name: String,
    pub url: String,
    /// 字节数(进度条总量;GitHub 可能给 0)。
    pub size: u64,
    /// sha256 十六进制小写。GitHub 的 `digest` 字段(`sha256:...`)提供,
    /// 缺失时为 `None` —— 下载仍进行,但没有校验。
    pub sha256: Option<String>,
}

/// 当前版本:编译时取自 `Cargo.toml`,与发布物一一对应。
pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

// ---------------------------------------------------------------------------
// 网络
// ---------------------------------------------------------------------------

/// 检查端点。`GLASPEN2_UPDATE_API` 可覆盖 —— 冒烟时指到本地 JSON 服务,
/// 就能在没有"更高版本 release"的情况下把全链路(下载/解包/替换/重启)演练
/// 一遍(见 docs/auto-update.md)。显式覆盖时允许 http://(本机服务)。
pub fn api_url() -> String {
    std::env::var("GLASPEN2_UPDATE_API").unwrap_or_else(|_| RELEASES_API.to_string())
}

/// 请求 `releases/latest` 并解析。失败时 `Err` 是**给用户看的中文原因**。
pub fn fetch_latest() -> Result<LatestRelease, String> {
    let url = api_url();
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .https_only(url.starts_with("https://"))
        .build();
    let mut resp = match ureq::Agent::new_with_config(config)
        .get(&url)
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
    let assets = v
        .get("assets")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| {
                    Some(Asset {
                        name: a.get("name")?.as_str()?.to_string(),
                        url: a.get("browser_download_url")?.as_str()?.to_string(),
                        size: a.get("size").and_then(|s| s.as_u64()).unwrap_or(0),
                        sha256: a
                            .get("digest")
                            .and_then(|d| d.as_str())
                            .and_then(|d| d.strip_prefix("sha256:"))
                            .map(|s| s.to_ascii_lowercase()),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(LatestRelease {
        tag,
        url,
        name: field("name"),
        published_at: field("published_at"),
        notes: field("body"),
        assets,
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
// 选包 / 缓存目录
// ---------------------------------------------------------------------------

/// 为 `os` + `arch` 挑安装包资产;挑不到返回 `None`(调用方降级为打开下载页)。
///
/// 资产命名在历史上并不统一(`glaspen2-0.5.0-arm64.dmg` vs
/// `glaspen2-v0.5.0-windows-x64-setup.exe`),这里按后缀 + 关键字宽松匹配:
///
/// - macOS: `.dmg`、不含 windows、arch 匹配(`arm64` dmg / `x86_64` dmg,
///   `universal` 两边都收);
/// - Windows: `.exe`、含 `windows` 与 64 位标记。
pub fn pick_asset_for<'a>(assets: &'a [Asset], os: &str, arch: &str) -> Option<&'a Asset> {
    assets.iter().find(|a| {
        let n = a.name.to_ascii_lowercase();
        match os {
            "macos" => {
                n.ends_with(".dmg")
                    && !n.contains("windows")
                    && match arch {
                        "arm64" => {
                            n.contains("arm64") || n.contains("aarch64") || n.contains("universal")
                        }
                        "x86_64" => {
                            n.contains("x86_64") || n.contains("amd64") || n.contains("universal")
                        }
                        _ => n.contains("universal"),
                    }
            }
            "windows" => {
                n.ends_with(".exe")
                    && n.contains("windows")
                    && (n.contains("x64") || n.contains("amd64"))
            }
            _ => false,
        }
    })
}

/// 为当前编译目标挑安装包。
pub fn pick_asset(assets: &[Asset]) -> Option<&Asset> {
    // consts::OS 本来就是 "macos" / "windows",直接用
    let os = std::env::consts::OS;
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        other => other, // "x86_64" 原样
    };
    pick_asset_for(assets, os, arch)
}

/// 更新工作目录:dmg / 暂存 .app / 握手标记 / 日志都在这里。
///
/// - macOS: `~/Library/Caches/glaspen2/updates`
/// - Windows: `%LOCALAPPDATA%\glaspen2\updates`
/// - 其它: 系统临时目录
pub fn update_dir() -> std::path::PathBuf {
    let base = if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .map(|h| h.join("Library/Caches/glaspen2"))
    } else if cfg!(target_os = "windows") {
        std::env::var_os("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .map(|h| h.join("glaspen2"))
    } else {
        None
    };
    let dir = base.unwrap_or_else(std::env::temp_dir).join("updates");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

// ---------------------------------------------------------------------------
// 下载
// ---------------------------------------------------------------------------

/// 下载失败的两种形态:用户主动取消不算故障。
#[derive(Debug, PartialEq)]
pub enum DownloadError {
    /// 用户在面板上取消(回调返回 `false`)。
    Cancelled,
    /// 真故障(网络/写盘/校验),`String` 是给用户看的原因。
    Failed(String),
}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadError::Cancelled => write!(f, "已取消"),
            DownloadError::Failed(m) => write!(f, "{m}"),
        }
    }
}

/// 下载用的 agent:连接/响应快超时,整体 30 分钟兜底(45MB 移动网络也够)。
fn download_agent() -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .timeout_recv_body(Some(Duration::from_secs(30 * 60)))
        .https_only(true)
        .build();
    ureq::Agent::new_with_config(config)
}

/// 流式下载 `url` → `dest`(先写 `.part`,**校验通过才 rename 成正式名**)。
///
/// `on_progress(received, total)` 约每 64KB 一次,返回 `false` 表示取消。
pub fn download(
    url: &str,
    dest: &std::path::Path,
    expect_sha256: Option<&str>,
    on_progress: impl FnMut(u64, u64) -> bool,
) -> Result<(), DownloadError> {
    download_with_agent(&download_agent(), url, dest, expect_sha256, on_progress)
}

/// [`download`] 的可注入版本(单测用本地 HTTP 服务、不加 https_only)。
fn download_with_agent(
    agent: &ureq::Agent,
    url: &str,
    dest: &std::path::Path,
    expect_sha256: Option<&str>,
    mut on_progress: impl FnMut(u64, u64) -> bool,
) -> Result<(), DownloadError> {
    use sha2::Digest;
    use std::io::{Read, Write};

    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| DownloadError::Failed(format!("创建目录失败:{e}")))?;
    }
    let mut part_os = dest.as_os_str().to_owned();
    part_os.push(".part");
    let part = std::path::PathBuf::from(part_os);
    let _ = std::fs::remove_file(&part);

    let resp = agent
        .get(url)
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| DownloadError::Failed(format!("下载失败:{e}")))?;
    let total = resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let mut reader = resp.into_body().into_reader();
    let mut file = std::fs::File::create(&part)
        .map_err(|e| DownloadError::Failed(format!("写文件失败:{e}")))?;

    let mut hasher = sha2::Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut received = 0u64;
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| DownloadError::Failed(format!("下载中断:{e}")))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .map_err(|e| DownloadError::Failed(format!("写文件失败:{e}")))?;
        <sha2::Sha256 as sha2::Digest>::update(&mut hasher, &buf[..n]);
        received += n as u64;
        if !on_progress(received, total) {
            drop(file);
            let _ = std::fs::remove_file(&part);
            return Err(DownloadError::Cancelled);
        }
    }
    file.flush()
        .and_then(|_| file.sync_all())
        .map_err(|e| DownloadError::Failed(format!("落盘失败:{e}")))?;
    drop(file);

    let got = hex_lower(<sha2::Sha256 as sha2::Digest>::finalize(hasher).as_slice());
    match expect_sha256 {
        Some(want) if !want.is_empty() && want != got => {
            let _ = std::fs::remove_file(&part);
            return Err(DownloadError::Failed(format!(
                "校验失败(期望 {want},实际 {got}),已删除下载文件"
            )));
        }
        None => tracing::warn!("资产没有 sha256,跳过校验:{url}"),
        _ => {}
    }
    if total > 0 && received != total {
        let _ = std::fs::remove_file(&part);
        return Err(DownloadError::Failed(format!(
            "下载不完整({received}/{total} 字节)"
        )));
    }
    std::fs::rename(&part, dest).map_err(|e| DownloadError::Failed(format!("保存失败:{e}")))?;
    on_progress(received, total);
    Ok(())
}

/// 流式计算文件 sha256(十六进制小写)。
pub fn file_sha256(path: &std::path::Path) -> std::io::Result<String> {
    use sha2::Digest;
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut hasher = sha2::Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        <sha2::Sha256 as sha2::Digest>::update(&mut hasher, &buf[..n]);
    }
    Ok(hex_lower(
        <sha2::Sha256 as sha2::Digest>::finalize(hasher).as_slice(),
    ))
}

/// 缓存里已有且校验通过的安装包可直接复用(重装/重复点更新不重下45MB)。
pub fn cached_asset_is_valid(dest: &std::path::Path, sha256: Option<&str>) -> bool {
    if !dest.exists() {
        return false;
    }
    match sha256 {
        Some(want) if !want.is_empty() => file_sha256(dest).is_ok_and(|got| got == want),
        _ => true,
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

// ---------------------------------------------------------------------------
// macOS: DMG 解包暂存
// ---------------------------------------------------------------------------

/// 缓存目录里最新的安装包(`.dmg`;按修改时间取最新)。
pub fn newest_dmg(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "dmg"))
        .max_by_key(|p| p.metadata().and_then(|m| m.modified()).ok())
}

/// 缓存目录里最新的 Windows 安装包(`glaspen2-*-setup.exe`;按修改时间取最新)。
/// apply 兜底用:优先取本次下载记录的路径。
#[cfg(target_os = "windows")]
pub fn newest_installer(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let n = p.file_name().map(|n| n.to_string_lossy().to_lowercase());
            n.is_some_and(|n| n.ends_with(".exe") && n.contains("glaspen2"))
        })
        .max_by_key(|p| p.metadata().and_then(|m| m.modified()).ok())
}

/// 查最新 release → 挑当前平台的安装包 → 下载(或复用缓存里校验通过的)。
/// 返回 (安装包路径, received, total)。macOS FRB(api.rs)与 Windows 管道共用。
pub fn download_to_cache(
    mut on_progress: impl FnMut(u64, u64) -> bool,
) -> Result<(std::path::PathBuf, u64, u64), String> {
    let rel = fetch_latest()?;
    let asset = pick_asset(&rel.assets).ok_or("当前平台没有对应的安装包,请打开下载页手动更新")?;
    let dest = update_dir().join(&asset.name);
    if cached_asset_is_valid(&dest, asset.sha256.as_deref()) {
        let total = dest.metadata().map(|m| m.len()).unwrap_or(asset.size);
        on_progress(total, total);
        return Ok((dest, total, total));
    }
    download(&asset.url, &dest, asset.sha256.as_deref(), on_progress).map_err(|e| e.to_string())?;
    let total = dest.metadata().map(|m| m.len()).unwrap_or(asset.size);
    Ok((dest, total, total))
}

/// 缓存目录里已解包的暂存 `.app`([`stage_dmg`] 的产物,`Glaspen2-*.app`)。
pub fn staged_app(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_dir()
                && p.extension().is_some_and(|x| x == "app")
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("Glaspen2-"))
        })
        .max_by_key(|p| p.metadata().and_then(|m| m.modified()).ok())
}

/// 挂载 dmg → 把 `.app` ditto 到 [`update_dir`] → 卸载 → 验签 → 剥 quarantine。
///
/// 在**主程序退出之前**调用(错误能直接显示在面板上);返回暂存的 `.app` 路径,
/// 后续换 bundle 由 `--updater` 帮手完成。
#[cfg(target_os = "macos")]
pub fn stage_dmg(dmg: &std::path::Path, tag: &str) -> Result<std::path::PathBuf, String> {
    use std::process::Command;

    if !dmg.exists() {
        return Err("安装包不存在,请重新下载".into());
    }
    let dir = update_dir();
    let mnt = dir.join(format!("mnt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&mnt);
    std::fs::create_dir_all(&mnt).map_err(|e| format!("创建挂载点失败:{e}"))?;

    let attached = Command::new("/usr/bin/hdiutil")
        .args(["attach", "-nobrowse", "-readonly", "-mountpoint"])
        .arg(&mnt)
        .arg(dmg)
        .status()
        .map_err(|e| format!("调用 hdiutil 失败:{e}"))?;
    if !attached.success() {
        let _ = std::fs::remove_dir_all(&mnt);
        return Err("挂载 DMG 失败(文件可能损坏)".into());
    }

    // 挂载期间的任何失败都必须先 detach 再返回
    let staged = stage_from_mount(&mnt, tag);
    let _ = Command::new("/usr/bin/hdiutil")
        .args(["detach", "-force"])
        .arg(&mnt)
        .status();
    let _ = std::fs::remove_dir_all(&mnt);
    staged
}

#[cfg(target_os = "macos")]
fn stage_from_mount(mnt: &std::path::Path, tag: &str) -> Result<std::path::PathBuf, String> {
    use std::process::Command;

    let src = find_app_in(mnt)?;
    let ver = tag.trim_start_matches(['v', 'V']);
    let staged = update_dir().join(format!("Glaspen2-{ver}.app"));
    let _ = std::fs::remove_dir_all(&staged);
    let st = Command::new("/usr/bin/ditto")
        .arg(&src)
        .arg(&staged)
        .status()
        .map_err(|e| format!("调用 ditto 失败:{e}"))?;
    if !st.success() {
        let _ = std::fs::remove_dir_all(&staged);
        return Err("从 DMG 拷贝应用失败".into());
    }
    // 自签未公证:自己下载的包不带 quarantine,但 ditto 从挂载卷拷贝可能带上
    let _ = Command::new("/usr/bin/xattr")
        .args(["-dr", "com.apple.quarantine"])
        .arg(&staged)
        .status();
    // 拷出来的包必须签名完好 —— 半截拷贝在这里就暴露,而不是替换之后
    let verify = Command::new("/usr/bin/codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(&staged)
        .status()
        .map_err(|e| format!("调用 codesign 失败:{e}"))?;
    if !verify.success() {
        let _ = std::fs::remove_dir_all(&staged);
        return Err("解包后的应用签名校验失败,已放弃".into());
    }
    Ok(staged)
}

/// 在挂载目录里找唯一的 `.app`。
#[cfg(target_os = "macos")]
fn find_app_in(dir: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("读取 DMG 内容失败:{e}"))?;
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "app") {
            return Ok(p);
        }
    }
    Err("DMG 里没有找到 .app".into())
}

/// 非 macOS:FRB 接口仍然存在,但没有可执行的实现。
#[cfg(not(target_os = "macos"))]
pub fn stage_dmg(_dmg: &std::path::Path, _tag: &str) -> Result<std::path::PathBuf, String> {
    Err("当前平台暂不支持自动解包,请打开下载页手动更新".into())
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

    // ── 资产 ──

    fn assets_fixture() -> Vec<Asset> {
        vec![
            Asset {
                name: "glaspen2-0.6.0-arm64.dmg".into(),
                url: "https://example.com/a.dmg".into(),
                size: 100,
                sha256: Some("aa".into()),
            },
            Asset {
                name: "glaspen2-0.6.0-x86_64.dmg".into(),
                url: "https://example.com/b.dmg".into(),
                size: 200,
                sha256: None,
            },
            Asset {
                name: "glaspen2-v0.6.0-windows-x64-setup.exe".into(),
                url: "https://example.com/c.exe".into(),
                size: 300,
                sha256: Some("bb".into()),
            },
        ]
    }

    #[test]
    fn test_parse_release_reads_assets_notes_and_digest() {
        // 注意: body 以 `"##` 开头, 会撞上 r#"…"# 的终止符, 必须用双 #。
        let json = r###"{
            "tag_name": "v0.6.0",
            "html_url": "https://github.com/liuluopeng/glaspen2/releases/tag/v0.6.0",
            "body": "## 新增\n- 检查更新",
            "assets": [
                {"name": "glaspen2-0.6.0-arm64.dmg",
                 "browser_download_url": "https://github.com/x/a.dmg",
                 "size": 44886733,
                 "digest": "sha256:D3DDBAF4154322A27C714FC7A80C49F196D26C5AE18BACA3C2EBD95E75C440F6"},
                {"name": "no-digest.exe",
                 "browser_download_url": "https://github.com/x/b.exe",
                 "size": 7}
            ]
        }"###;
        let r = parse_release(json).unwrap();
        assert_eq!(r.notes, "## 新增\n- 检查更新");
        assert_eq!(r.assets.len(), 2);
        assert_eq!(r.assets[0].name, "glaspen2-0.6.0-arm64.dmg");
        assert_eq!(r.assets[0].size, 44886733);
        // digest 统一成小写无前缀
        assert_eq!(
            r.assets[0].sha256.as_deref(),
            Some("d3ddbaf4154322a27c714fc7a80c49f196d26c5ae18baca3c2ebd95e75c440f6")
        );
        assert_eq!(r.assets[1].sha256, None);
    }

    #[test]
    fn test_pick_asset_matrix() {
        let fx = assets_fixture();
        assert_eq!(
            pick_asset_for(&fx, "macos", "arm64").map(|a| a.name.as_str()),
            Some("glaspen2-0.6.0-arm64.dmg")
        );
        assert_eq!(
            pick_asset_for(&fx, "macos", "x86_64").map(|a| a.name.as_str()),
            Some("glaspen2-0.6.0-x86_64.dmg")
        );
        assert_eq!(
            pick_asset_for(&fx, "windows", "x86_64").map(|a| a.name.as_str()),
            Some("glaspen2-v0.6.0-windows-x64-setup.exe")
        );
        // 该平台/架构没有资产 → 降级打开下载页
        assert!(pick_asset_for(&[], "macos", "arm64").is_none());
        assert!(pick_asset_for(&fx, "linux", "x86_64").is_none());
        let only_arm = fx[..1].to_vec();
        assert!(pick_asset_for(&only_arm, "macos", "x86_64").is_none());
        assert!(pick_asset_for(&only_arm, "windows", "x86_64").is_none());
        // universal 两边都收
        let uni = &[Asset {
            name: "glaspen2-0.6.0-universal2.dmg".into(),
            url: "u".into(),
            size: 1,
            sha256: None,
        }];
        assert!(pick_asset_for(uni, "macos", "arm64").is_some());
        assert!(pick_asset_for(uni, "macos", "x86_64").is_some());
    }

    // ── 下载(本地一次性 HTTP 服务, 不碰外网) ──

    /// 起一个只服务一次请求的 HTTP/1.1 服务器, 返回下载 URL。
    fn serve_once(body: Vec<u8>) -> (String, std::thread::JoinHandle<()>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut req = [0u8; 4096];
            // 读到请求头结束
            loop {
                let n = sock.read(&mut req).unwrap_or(0);
                if n == 0 || req[..n].windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = sock.write_all(head.as_bytes());
            let _ = sock.write_all(&body);
        });
        (format!("http://{addr}/glaspen2-test.bin"), handle)
    }

    fn sha256_hex(bytes: &[u8]) -> String {
        use sha2::Digest;
        let mut h = sha2::Sha256::new();
        h.update(bytes);
        hex_lower(&h.finalize())
    }

    /// 每个测试一个独立目录(cargo test 并行跑, 只按 pid 化分会互相踩)。
    fn dl_dir(case: &str) -> std::path::PathBuf {
        let d =
            std::env::temp_dir().join(format!("glaspen2_dltest_{}_{}", std::process::id(), case));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn test_download_writes_file_verifies_sha_and_reports_progress() {
        let body = vec![7u8; 300 * 1024]; // > 一次 64KB 读, 至少触发 4 次进度
        let want = sha256_hex(&body);
        let (url, srv) = serve_once(body.clone());
        let dir = dl_dir("ok");
        let dest = dir.join("pkg.dmg");

        let agent = ureq::Agent::new_with_config(ureq::Agent::config_builder().build());
        let mut calls: Vec<(u64, u64)> = Vec::new();
        let r = download_with_agent(&agent, &url, &dest, Some(&want), |rec, total| {
            calls.push((rec, total));
            true
        });
        srv.join().unwrap();
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(std::fs::read(&dest).unwrap(), body);
        assert!(!dir.join("pkg.dmg.part").exists(), ".part 应该已经转正");
        assert!(calls.len() >= 4, "进度回调次数: {}", calls.len());
        assert_eq!(calls.last().unwrap().0, body.len() as u64);
        assert_eq!(calls.last().unwrap().1, body.len() as u64);
        assert!(cached_asset_is_valid(&dest, Some(&want)));
        assert!(!cached_asset_is_valid(&dest, Some("deadbeef")));
    }

    #[test]
    fn test_download_rejects_bad_sha_and_keeps_nothing() {
        let body = vec![1u8; 1024];
        let (url, srv) = serve_once(body);
        let dir = dl_dir("badsha");
        let dest = dir.join("pkg.dmg");
        let agent = ureq::Agent::new_with_config(ureq::Agent::config_builder().build());
        let r = download_with_agent(&agent, &url, &dest, Some("00deadbeef"), |_, _| true);
        srv.join().unwrap();
        assert!(matches!(r, Err(DownloadError::Failed(_))), "{r:?}");
        assert!(!dest.exists(), "校验失败不能留下正式文件");
        assert!(!dir.join("pkg.dmg.part").exists(), ".part 必须删掉");
    }

    #[test]
    fn test_download_cancel_removes_partial_file() {
        let body = vec![9u8; 512 * 1024];
        let (url, srv) = serve_once(body);
        let dir = dl_dir("cancel");
        let dest = dir.join("pkg.dmg");
        let agent = ureq::Agent::new_with_config(ureq::Agent::config_builder().build());
        let r = download_with_agent(&agent, &url, &dest, None, |_, _| false); // 第一次进度就取消
        srv.join().unwrap();
        assert_eq!(r, Err(DownloadError::Cancelled));
        assert!(!dest.exists());
        assert!(!dir.join("pkg.dmg.part").exists());
    }

    #[test]
    fn test_update_dir_exists() {
        let d = update_dir();
        assert!(d.is_dir(), "{d:?} 应该已创建");
        assert!(d.to_string_lossy().contains("updates"));
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
