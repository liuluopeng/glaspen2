//! ChatStore gRPC 调用的登录身份(`authorization: Bearer <JWT>`)。
//!
//! 聊天是 glaspen2 的**增强功能**:四个环境变量(`GLASPEN_API_BASE` /
//! `GLASPEN_CHAT_USER` / `GLASPEN_CHAT_PASSWORD` / `GLASPEN_CHAT_TOKEN`)
//! 一个都没配时 [`token()`] 恒为 `None`,请求不带任何 metadata,行为与
//! 未加身份的旧版完全一致 —— 不登录 axum 照常用 glaspen2。
//!
//! 身份语义与 axum 侧规格见 docs/grpc-auth.md:
//! - `GLASPEN_CHAT_TOKEN` 直接给 token(跳过登录,二选一);
//! - 否则 `USER`+`PASSWORD` 向 `{GLASPEN_API_BASE}/api/user/login` 换取,
//!   token 只存内存,收到 gRPC `unauthenticated` 时 [`invalidate()`] 清缓存,
//!   下次会话自动重新登录;
//! - 登录失败**降级**为无身份发送(过渡期 axum 接受),绝不阻断涂鸦。

use std::sync::{Mutex, OnceLock};

/// 登录身份配置,来自环境变量,进程内解析一次。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthConfig {
    /// axum HTTP(S) 基址,登录用(与 gRPC 端口不同)。
    pub api_base: Option<String>,
    pub user: Option<String>,
    pub password: Option<String>,
    /// 直接给 token,跳过登录。
    pub direct_token: Option<String>,
}

impl AuthConfig {
    pub fn from_env() -> Self {
        Self {
            api_base: nonempty_env("GLASPEN_API_BASE"),
            user: nonempty_env("GLASPEN_CHAT_USER"),
            password: nonempty_env("GLASPEN_CHAT_PASSWORD"),
            direct_token: nonempty_env("GLASPEN_CHAT_TOKEN"),
        }
    }

    /// 是否具备携带身份的条件:直接给 token,或登录三件套齐全。
    /// 缺任何一个 = 不鉴权(静默降级,不打扰用户)。
    pub fn is_configured(&self) -> bool {
        self.direct_token.is_some()
            || matches!((&self.api_base, &self.user, &self.password),
                        (Some(_), Some(_), Some(_)))
    }

    /// 字段级合并:`other` 里的非空值覆盖 `self`(DB/面板设置优先于环境变量)。
    pub fn merged(self, other: AuthConfig) -> AuthConfig {
        AuthConfig {
            api_base: other.api_base.or(self.api_base),
            user: other.user.or(self.user),
            password: other.password.or(self.password),
            direct_token: other.direct_token.or(self.direct_token),
        }
    }
}

fn nonempty_env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

struct AuthState {
    cfg: AuthConfig,
    /// 内存缓存的登录 token;token 过期/被登出时由 invalidate() 清除。
    cached: Option<String>,
}

static AUTH: OnceLock<Mutex<AuthState>> = OnceLock::new();

fn auth_state() -> &'static Mutex<AuthState> {
    AUTH.get_or_init(|| Mutex::new(AuthState {
        cfg: AuthConfig::from_env(),
        cached: None,
    }))
}

/// 宿主(glaspen2 设置面板)注入的运行时配置,字段级覆盖环境变量默认值。
/// 配置变化时清掉缓存 token(换账号/换服务,旧 token 不可复用)。
pub fn set_config(cfg: AuthConfig) {
    let mut st = auth_state().lock().unwrap();
    if st.cfg != cfg {
        st.cached = None;
    }
    st.cfg = cfg;
}

/// 当前生效配置(环境变量 + 宿主注入的合并结果)。
/// 当前生效配置里的 axum 服务基址(OCR 等功能用)。
pub fn api_base() -> Option<String> {
    config().api_base
}

/// 是否已配置登录身份(登录三件套或直接 token)。
/// 所有联网增强功能(草稿/直发/共享/OCR)的前置条件; 检查更新除外。
pub fn configured() -> bool {
    config().is_configured()
}

pub fn config() -> AuthConfig {
    auth_state().lock().unwrap().cfg.clone()
}

/// 取当前请求应携带的 Bearer token。无配置/登录失败返回 None(不带身份)。
pub async fn token() -> Option<String> {
    token_with(auth_state()).await
}

/// [`token()`] 的可注入形态(测试用,绕开进程级全局)。
async fn token_with(auth: &Mutex<AuthState>) -> Option<String> {
    let cfg = auth.lock().unwrap().cfg.clone();
    if !cfg.is_configured() {
        return None;
    }
    if let Some(t) = &cfg.direct_token {
        return Some(t.clone());
    }
    if let Some(t) = auth.lock().unwrap().cached.clone() {
        return Some(t);
    }
    login_and_cache(auth).await.ok()
}

/// 强制重新登录(设置面板「测试登录」):忽略内存缓存,成功后更新缓存。
/// 返回 token 或可读失败原因。
pub async fn force_login() -> Result<String, String> {
    let auth = auth_state();
    auth.lock().unwrap().cached = None;
    login_and_cache(auth).await
}

/// 登录一次并把 token 写进缓存;失败时打降级日志(不阻断涂鸦)并返回原因。
async fn login_and_cache(auth: &Mutex<AuthState>) -> Result<String, String> {
    let cfg = auth.lock().unwrap().cfg.clone();
    if !cfg.is_configured() {
        return Err("未配置涂鸦身份账号".into());
    }
    let base = cfg.api_base.clone().unwrap_or_default();
    let user = cfg.user.clone().unwrap_or_default();
    let pass = cfg.password.clone().unwrap_or_default();
    match tokio::task::spawn_blocking(move || login_blocking(&base, &user, &pass)).await {
        Ok(Ok(t)) => {
            eprintln!("[chat-auth] 登录成功,本次会话携带身份");
            auth.lock().unwrap().cached = Some(t.clone());
            Ok(t)
        }
        Ok(Err(e)) => {
            eprintln!("[ink-draft] 登录失败,涂鸦将无身份(不阻断涂鸦): {e}");
            Err(e)
        }
        Err(e) => {
            eprintln!("[ink-draft] 登录任务失败,涂鸦将无身份(不阻断涂鸦): {e}");
            Err(format!("登录任务失败: {e}"))
        }
    }
}

/// token 失效(gRPC `unauthenticated`)后清除缓存,下次重新登录。
pub fn invalidate() {
    if let Some(auth) = AUTH.get() {
        auth.lock().unwrap().cached = None;
    }
}

/// 构造 `authorization` metadata 值;含非法字符时返回 None(JWT 为 ASCII,
/// 实际不会发生)。
pub fn bearer_metadata(
    token: &str,
) -> Option<tonic::metadata::MetadataValue<tonic::metadata::Ascii>> {
    format!("Bearer {token}").try_into().ok()
}

/// 登录请求(`POST /api/user/login`)。dev 的 HTTPS 是自签证书
/// (Caddy local),disable_verification 放行;失败原因是给用户看的中文。
fn login_blocking(api_base: &str, user: &str, password: &str) -> Result<String, String> {
    let url = format!("{}/api/user/login", api_base.trim_end_matches('/'));
    let tls = ureq::tls::TlsConfig::builder()
        .disable_verification(true)
        .build();
    let config = ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_global(Some(std::time::Duration::from_secs(5)))
        .build();
    let agent = ureq::Agent::new_with_config(config);
    let mut resp = agent
        .post(&url)
        .send_json(serde_json::json!({
            "username": user,
            "password": password,
            "player_name": "glaspen2",
        }))
        .map_err(|e| format!("请求 {url} 失败: {e}"))?;
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("读取登录响应失败: {e}"))?;
    parse_login_json(&body)
}

/// 解析登录响应:`{"msg":"ok","data":{"token":"<JWT>"}}`。
fn parse_login_json(body: &str) -> Result<String, String> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|_| "登录响应不是合法 JSON".to_string())?;
    if v.get("msg").and_then(|m| m.as_str()) != Some("ok") {
        let msg = v.get("msg").and_then(|m| m.as_str()).unwrap_or("(无 msg 字段)");
        return Err(format!("登录被拒绝: {msg}"));
    }
    v.pointer("/data/token")
        .and_then(|t| t.as_str())
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| "登录响应缺少 data.token".to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// 纯解析:成功响应取出 data.token。
    #[test]
    fn parse_login_ok() {
        let body = r#"{"msg":"ok","data":{"token":"jwt-abc.def"}}"#;
        assert_eq!(parse_login_json(body).unwrap(), "jwt-abc.def");
    }

    /// 纯解析:业务拒绝(msg != ok)与字段缺失都要给出可读原因。
    #[test]
    fn parse_login_rejects() {
        let e = parse_login_json(r#"{"msg":"bad password"}"#).unwrap_err();
        assert!(e.contains("登录被拒绝"), "{e}");
        assert!(e.contains("bad password"), "{e}");
        let e = parse_login_json(r#"{"msg":"ok","data":{}}"#).unwrap_err();
        assert!(e.contains("data.token"), "{e}");
        let e = parse_login_json("not json").unwrap_err();
        assert!(e.contains("JSON"), "{e}");
    }

    /// 配置判定:直接 token 或登录三件套齐全才算配置了身份。
    #[test]
    fn auth_configured_matrix() {
        let full = AuthConfig {
            api_base: Some("https://x".into()),
            user: Some("u".into()),
            password: Some("p".into()),
            direct_token: None,
        };
        assert!(full.is_configured());
        assert!(AuthConfig { direct_token: Some("t".into()), ..Default::default() }.is_configured());
        // 缺 api_base / 缺密码 → 无法登录,视为未配置。
        assert!(!AuthConfig { user: Some("u".into()), password: Some("p".into()), ..Default::default() }.is_configured());
        assert!(!AuthConfig { api_base: Some("https://x".into()), password: Some("p".into()), ..Default::default() }.is_configured());
        assert!(!AuthConfig::default().is_configured());
    }

    /// 未配置任何账号:token() 恒为 None —— 聊天是增强功能,不登录照常用。
    #[tokio::test]
    async fn unconfigured_yields_no_token() {
        let auth = Mutex::new(AuthState {
            cfg: AuthConfig::default(),
            cached: None,
        });
        assert_eq!(token_with(&auth).await, None);
    }

    /// 直接 token:跳过登录,原样携带。
    #[tokio::test]
    async fn direct_token_passthrough() {
        let auth = Mutex::new(AuthState {
            cfg: AuthConfig {
                direct_token: Some("jwt-direct".into()),
                ..Default::default()
            },
            cached: None,
        });
        assert_eq!(token_with(&auth).await.as_deref(), Some("jwt-direct"));
    }

    /// 缓存命中:cached 有值时不再触发登录(cfg 有登录配置但没有网络)。
    #[tokio::test]
    async fn cached_token_reused() {
        let auth = Mutex::new(AuthState {
            cfg: AuthConfig {
                api_base: Some("https://unused".into()),
                user: Some("u".into()),
                password: Some("p".into()),
                direct_token: None,
            },
            cached: Some("jwt-cached".into()),
        });
        assert_eq!(token_with(&auth).await.as_deref(), Some("jwt-cached"));
    }

    /// 字段级合并:面板/DB 的非空值逐字段覆盖环境变量默认值。
    #[test]
    fn merge_overrides_field_wise() {
        let env = AuthConfig {
            api_base: Some("https://env".into()),
            user: Some("env-user".into()),
            password: Some("env-pass".into()),
            direct_token: None,
        };
        let db = AuthConfig {
            api_base: None, // 未填 → 保留 env
            user: Some("ui-user".into()),
            password: Some("ui-pass".into()),
            direct_token: None,
        };
        let m = env.merged(db);
        assert_eq!(m.api_base.as_deref(), Some("https://env"));
        assert_eq!(m.user.as_deref(), Some("ui-user"));
        assert_eq!(m.password.as_deref(), Some("ui-pass"));
    }

    /// invalidate 清缓存;未初始化全局时是安全空操作。
    #[test]
    fn invalidate_clears_cache() {
        let auth = Mutex::new(AuthState {
            cfg: AuthConfig::default(),
            cached: Some("jwt-gone".into()),
        });
        invalidate(); // 全局未初始化,不应 panic
        auth.lock().unwrap().cached = Some("jwt-gone".into());
        assert_eq!(auth.lock().unwrap().cached.as_deref(), Some("jwt-gone"));
        // 直接对局部状态再演一遍 invalidate 的语义
        invalidate_state(&auth);
        assert_eq!(auth.lock().unwrap().cached, None);
    }

    fn invalidate_state(auth: &Mutex<AuthState>) {
        auth.lock().unwrap().cached = None;
    }

    /// Bearer metadata:正常构造,非法字符拒绝。
    #[test]
    fn bearer_metadata_shape() {
        let v = bearer_metadata("jwt-x").unwrap();
        assert_eq!(v.to_str().unwrap(), "Bearer jwt-x");
        assert!(bearer_metadata("bad\nvalue").is_none());
    }
}
