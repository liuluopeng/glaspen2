//! Flutter ←→ Rust 桥接层(flutter_rust_bridge):macOS 设置面板的全部通信。
//!
//! 设置面板是嵌在同一进程里的 Flutter 视图,Rust 代码就在主可执行文件内,
//! 所以 Dart 侧用 `ExternalLibrary.process()` 直接解析符号即可 —— 不再需要
//! Flutter MethodChannel 中转。Windows 版设置是独立进程(命名管道),不使用本模块。
//!
//! 设置项 / 镜头 / 导出这些状态仍由 ObjC 持有(绘制、窗口、菜单都在那里),
//! 因此这里通过 `glaspen2_macos_*` C shim 转发;shim 内部负责切回主线程。

use std::sync::Mutex;

use crate::db;
use crate::frb_generated::StreamSink;
use flutter_rust_bridge::frb;

// ---------------------------------------------------------------------------
// ObjC C shim
// ---------------------------------------------------------------------------

/// macOS:真正的 ObjC 实现(见 src/macos/glaspen2.m)。
/// 内部实现细节,不镜像到 Dart(否则会绕过 run_blocking 的运行时保护)。
#[frb(ignore)]
#[cfg(target_os = "macos")]
pub(crate) mod shim {
    use std::os::raw::{c_char, c_int};

    unsafe extern "C" {
        fn glaspen2_macos_settings_json() -> *mut c_char;
        fn glaspen2_macos_free_c_string(p: *mut c_char);
        fn glaspen2_macos_set_setting(key: *const c_char, value_json: *const c_char);
        fn glaspen2_macos_delete_page(screen_id: i64) -> c_int;
        fn glaspen2_macos_navigate_to_page(screen_id: i64);
        fn glaspen2_macos_hotkey(key: *const c_char);
        fn glaspen2_macos_canvas_payload(
            w: c_int,
            h: c_int,
            action: c_int,
            rect_out: *mut f64,
            out_len: *mut c_int,
        ) -> *mut u8;
        fn glaspen2_macos_export_pdf() -> c_int;
        fn glaspen2_macos_export_gif() -> c_int;
        fn glaspen2_macos_free_bytes(p: *mut u8);
        fn glaspen2_macos_open_url(url: *const c_char);
    }

    pub fn settings_json() -> Option<String> {
        unsafe {
            let p = glaspen2_macos_settings_json();
            if p.is_null() {
                return None;
            }
            let s = std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned();
            glaspen2_macos_free_c_string(p);
            Some(s)
        }
    }

    pub fn set_setting(key: &str, value_json: &str) {
        if let (Ok(k), Ok(v)) = (
            std::ffi::CString::new(key),
            std::ffi::CString::new(value_json),
        ) {
            unsafe { glaspen2_macos_set_setting(k.as_ptr(), v.as_ptr()) };
        }
    }

    pub fn delete_page(screen_id: i64) -> bool {
        unsafe { glaspen2_macos_delete_page(screen_id) != 0 }
    }

    pub fn navigate_to_page(screen_id: i64) {
        unsafe { glaspen2_macos_navigate_to_page(screen_id) };
    }

    pub fn hotkey(key: &str) {
        if let Ok(k) = std::ffi::CString::new(key) {
            unsafe { glaspen2_macos_hotkey(k.as_ptr()) };
        }
    }

    /// 画布总览:返回 (PNG, 视口矩形)。空画布返回 None。
    pub fn canvas_payload(w: i32, h: i32, action: i32) -> Option<(Vec<u8>, Vec<f64>)> {
        let mut rect = [0.0f64; 4];
        let mut len: c_int = 0;
        unsafe {
            let png = glaspen2_macos_canvas_payload(w, h, action, rect.as_mut_ptr(), &mut len);
            if png.is_null() || len <= 0 {
                return None;
            }
            let bytes = std::slice::from_raw_parts(png, len as usize).to_vec();
            glaspen2_macos_free_bytes(png);
            Some((bytes, rect.to_vec()))
        }
    }

    pub fn export_pdf() -> bool {
        unsafe { glaspen2_macos_export_pdf() != 0 }
    }

    pub fn export_gif() -> bool {
        unsafe { glaspen2_macos_export_gif() != 0 }
    }

    /// 用系统默认浏览器打开 URL(实现见 src/macos/glaspen2.m)。
    pub fn open_url(url: &str) {
        if let Ok(u) = std::ffi::CString::new(url) {
            unsafe { glaspen2_macos_open_url(u.as_ptr()) };
        }
    }
}

/// 其它平台没有 ObjC 侧(Windows 设置面板是独立进程,走命名管道)。
#[frb(ignore)]
#[cfg(not(target_os = "macos"))]
pub(crate) mod shim {
    pub fn settings_json() -> Option<String> {
        None
    }
    pub fn set_setting(_key: &str, _value_json: &str) {}
    pub fn delete_page(_screen_id: i64) -> bool {
        false
    }
    pub fn navigate_to_page(_screen_id: i64) {}
    pub fn hotkey(_key: &str) {}
    pub fn canvas_payload(_w: i32, _h: i32, _action: i32) -> Option<(Vec<u8>, Vec<f64>)> {
        None
    }
    pub fn export_pdf() -> bool {
        false
    }
    pub fn export_gif() -> bool {
        false
    }

    /// 用系统默认浏览器打开 URL:Windows 走 `cmd /c start`,不开控制台窗口。
    pub fn open_url(url: &str) {
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            let _ = std::process::Command::new("cmd")
                .args(["/c", "start", "", url])
                .creation_flags(CREATE_NO_WINDOW)
                .spawn();
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = url;
        }
    }
}

/// 打开一个 http(s) URL(系统默认浏览器)。设置面板与 Windows 命名管道共用,
/// 只接受 http/https —— URL 可能来自网络响应, 先在这里挡掉其它 scheme。
pub(crate) fn open_url_checked(url: &str) {
    if url.starts_with("https://") || url.starts_with("http://") {
        shim::open_url(url);
    } else {
        eprintln!("[api] 拒绝打开非 http(s) URL: {url}");
    }
}

// ---------------------------------------------------------------------------
// 设置
// ---------------------------------------------------------------------------

/// 在独立线程上执行阻塞工作。
///
/// FRB 的 async 任务跑在 tokio 运行时的工作线程上,而本项目所有 FFI 入口
/// (ObjC shim、`page_thumbnails_blob` 等)内部都用 `runtime().block_on(...)`
/// 桥接 async SQLite —— 在运行时上下文里再次 block_on 会 panic:
/// "Cannot start a runtime from within a runtime"。
/// 换一个全新线程就脱离了运行时上下文;代价是每次调用几微秒的线程开销。
pub(crate) fn run_blocking<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|scope| match scope.spawn(f).join() {
        Ok(value) => value,
        // 保留原始 panic 信息,交给 FRB 的 catch_unwind 上报给 Dart
        Err(panic) => std::panic::resume_unwind(panic),
    })
}

/// 设置快照。字段与旧 MethodChannel 字典一一对应,Dart 侧 `_FrbBridge`
/// 会转回原来的 Map 形状,UI 代码无需改动。
#[frb]
#[derive(Debug, Clone)]
pub struct Settings {
    pub color: i32,
    pub width: i32,
    pub rainbow: bool,
    pub launch_at_login: bool,
    pub frosted_glass: bool,
    pub grid: bool,
    pub grid_follow_strokes: bool,
    pub pressure_monitor: bool,
    pub outline: bool,
    pub infinite_canvas: bool,
    pub minimap: bool,
    pub grid_size: f64,
    pub gif_fps: i32,
    pub gif_resolution: f64,
    pub gif_speed: f64,
    pub gif_end_mode: i32,
    // 涂鸦身份(手写消息登录)—— 密码不进快照,只回是否已保存
    pub chat_api_base: String,
    pub chat_user: String,
    pub chat_has_password: bool,
    // 手写消息集成总开关(默认关):关 = 隐藏登录/共享界面,⌘⌃2/⌘⌃3 直通
    pub chat_integration: bool,
    // 「自由涂鸦」tab 显隐(默认关:最小面板只有 设置+活页本)
    pub show_free_canvas: bool,
    // 共享画布上行开关(活页本 tab;仅集成开启时生效)
    pub share_canvas: bool,
}

impl Settings {
    /// 宽松解析:推送方只发部分字段(旧 sync_settings_panel 只发 12 项)。
    fn from_json(json: &str) -> Option<Settings> {
        let v: serde_json::Value = serde_json::from_str(json).ok()?;
        let b = |k: &str| v.get(k).and_then(|x| x.as_bool()).unwrap_or(false);
        let i = |k: &str| v.get(k).and_then(|x| x.as_i64()).unwrap_or(0) as i32;
        let f = |k: &str| v.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0);
        let s = |k: &str| {
            v.get(k)
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string()
        };
        Some(Settings {
            color: i("color"),
            width: i("width"),
            rainbow: b("rainbow"),
            launch_at_login: b("launchAtLogin"),
            frosted_glass: b("frostedGlass"),
            grid: b("grid"),
            grid_follow_strokes: b("gridFollowStrokes"),
            pressure_monitor: b("pressureMonitor"),
            outline: b("outline"),
            infinite_canvas: b("infiniteCanvas"),
            minimap: b("minimap"),
            grid_size: f("gridSize"),
            gif_fps: i("gifFps"),
            gif_resolution: f("gifResolution"),
            gif_speed: f("gifSpeed"),
            gif_end_mode: i("gifEndMode"),
            chat_api_base: s("chatApiBase"),
            chat_user: s("chatUser"),
            chat_has_password: b("chatHasPassword"),
            chat_integration: b("chatIntegration"),
            show_free_canvas: b("showFreeCanvas"),
            share_canvas: b("shareCanvas"),
        })
    }
}

/// 设置变化推送的订阅者。ObjC 侧(菜单/快捷键改了状态)会调用
/// `glaspen2_notify_settings_changed` 往这里推一份新快照。
static SETTINGS_SINKS: Mutex<Vec<StreamSink<Settings>>> = Mutex::new(Vec::new());

/// 当前设置。
#[frb]
pub async fn get_settings() -> Option<Settings> {
    run_blocking(current_settings)
}

/// 写入一项设置。`value_json` 是 JSON 标量(`true` / `3` / `2.5`),
/// ObjC 侧按 key 解析,布尔/整数/小数共用一条通道。
#[frb]
pub async fn set_setting(key: String, value_json: String) {
    run_blocking(move || shim::set_setting(&key, &value_json));
}

/// 设置面板「测试登录」:用当前已保存的涂鸦身份配置强制登录一次
/// (成功则缓存 token,后续 DraftInk/AppendMessages 立即携带身份)。
/// 返回空串 = 成功,否则为可读失败原因。
#[frb]
pub async fn test_chat_login() -> String {
    run_blocking(|| match crate::export::chat_auth_test_login_blocking() {
        Ok(()) => String::new(),
        Err(e) => e,
    })
}

// ---------------------------------------------------------------------------
// 共享画布上行(活页本 tab 的「共享画布」开关;转发语义见 docs/canvas-share-grpc.md)
// ---------------------------------------------------------------------------

/// 设置变化推送流:面板订阅它,菜单/快捷键的改动能实时同步到 UI。
#[frb]
pub fn settings_changed(sink: StreamSink<Settings>) {
    if let Some(s) = current_settings() {
        let _ = sink.add(s);
    }
    SETTINGS_SINKS.lock().unwrap().push(sink);
}

/// 由 ObjC 在 `sync_settings_panel()` 里调用(运行在主线程)。
#[frb(ignore)]
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_notify_settings_changed() {
    let Some(settings) = current_settings() else {
        return;
    };
    let mut sinks = SETTINGS_SINKS.lock().unwrap();
    // 订阅端已断开时 add 会失败,顺手清理
    sinks.retain(|s| s.add(settings.clone()).is_ok());
}

fn current_settings() -> Option<Settings> {
    shim::settings_json()
        .as_deref()
        .and_then(Settings::from_json)
}

// ---------------------------------------------------------------------------
// 活页本
// ---------------------------------------------------------------------------

/// 活页本概览:每页的 id、尺寸和笔迹数。
#[frb]
pub async fn list_pages() -> Vec<PageSummary> {
    let screens = db::list_screens().await;
    let ids: Vec<i64> = screens.iter().map(|(id, _, _)| *id).collect();
    // 一次批量查询替代每页一条 COUNT
    let versions = db::stroke_versions_many(&ids).await;
    screens
        .into_iter()
        .map(|(id, width, height)| PageSummary {
            id,
            width,
            height,
            stroke_count: versions.get(&id).map(|(c, _)| *c as u64).unwrap_or(0),
        })
        .collect()
}

/// 一页的缩略图。
#[frb]
#[derive(Debug, Clone)]
pub struct PageThumb {
    pub id: i64,
    pub png: Vec<u8>,
}

/// 批量缩略图:一次调用取回整屏(版本查询、缓存读取、缺失渲染都在 Rust 侧
/// 批量化)。没有笔迹的页不会出现在结果里。
#[frb]
pub async fn page_thumbnails(ids: Vec<i64>, max_size: i32) -> Vec<PageThumb> {
    run_blocking(move || {
        let blob = crate::export::page_thumbnails_blob(&ids, max_size);
        decode_thumb_blob(&blob)
    })
}

/// 删除一页及其笔迹。
#[frb]
pub async fn delete_page(screen_id: i64) -> bool {
    run_blocking(move || shim::delete_page(screen_id))
}

/// 跳转到指定页继续绘画。
#[frb]
pub async fn navigate_to_page(screen_id: i64) {
    run_blocking(move || shim::navigate_to_page(screen_id));
}

/// 触发一个快捷键动作(与物理 ⌃⌘<key> 等价)。
#[frb]
pub async fn trigger_hotkey(key: String) {
    run_blocking(move || shim::hotkey(&key));
}

// ---------------------------------------------------------------------------
// 无限画布
// ---------------------------------------------------------------------------

/// 镜头动作。
#[frb]
#[derive(Debug, Clone, Copy)]
pub enum CanvasAction {
    /// 只取当前总览
    Current,
    /// 镜头回原点 + 100%
    Home,
    /// 镜头居中到内容包围盒
    Center,
    /// 清空内容 + 镜头回原点
    New,
}

/// 总览载荷:PNG + 当前视口矩形 [x, y, w, h]。
#[frb]
#[derive(Debug, Clone)]
pub struct CanvasPayload {
    pub png: Vec<u8>,
    pub rect: Vec<f64>,
}

/// 无限画布总览。`w`/`h` 小于 400 时按 1024×768 处理。空画布返回 None。
#[frb]
pub async fn canvas_overview(w: i32, h: i32, action: CanvasAction) -> Option<CanvasPayload> {
    let code = match action {
        CanvasAction::Current => 0,
        CanvasAction::Home => 1,
        CanvasAction::Center => 2,
        CanvasAction::New => 3,
    };
    run_blocking(move || {
        shim::canvas_payload(w, h, code).map(|(png, rect)| CanvasPayload { png, rect })
    })
}

/// 当前镜头状态(无限画布)。
#[frb]
pub async fn get_lens() -> LensState {
    let cur = crate::state::current_screen_id();
    let (pan_x, pan_y, zoom) = db::get_infinite_transform()
        .await
        .unwrap_or((0.0, 0.0, 1.0));
    LensState {
        page_id: cur,
        pan_x,
        pan_y,
        zoom: if zoom > 0.05 { zoom } else { 1.0 },
    }
}

/// 页号跟随:当前页在所有页中的序号(1 起)。
#[frb]
pub async fn get_page_ordinal() -> u64 {
    let cur = crate::state::current_screen_id();
    db::page_info(cur).await.map(|i| i.2 as u64).unwrap_or(0)
}

// ---------------------------------------------------------------------------
// 数据备份 / 回导
// ---------------------------------------------------------------------------

/// 备份/回导结果:成败 + 给用户看的一句话(成功时是文件路径)。
#[frb]
#[derive(Debug, Clone)]
pub struct BackupOutcome {
    pub ok: bool,
    pub message: String,
}

/// 把全部数据备份到桌面。产出单个 .db 文件(一致快照), 换机时可直接替换
/// glaspen2.db 使用。
#[frb]
pub async fn backup_now() -> BackupOutcome {
    match db::backup_now().await {
        Ok(path) => BackupOutcome {
            ok: true,
            message: path,
        },
        Err(e) => BackupOutcome {
            ok: false,
            message: e,
        },
    }
}

/// 从桌面上最新的备份**合并**恢复: 不会删除备份之后新画的页/笔迹, 同名 id
/// 以备份为准。恢复后需要重启才能看到覆盖层上的变化。
#[frb]
pub async fn restore_latest_backup() -> BackupOutcome {
    match db::restore_latest_backup().await {
        Ok((path, pages)) => BackupOutcome {
            ok: true,
            message: format!("{path}（当前共 {pages} 页）"),
        },
        Err(e) => BackupOutcome {
            ok: false,
            message: e,
        },
    }
}

// ---------------------------------------------------------------------------
// 检查更新
// ---------------------------------------------------------------------------

/// release 里的一个安装包(挑包下载用)。
#[frb]
#[derive(Debug, Clone)]
pub struct UpdateAsset {
    pub name: String,
    pub url: String,
    /// 字节数(进度条总量;GitHub 可能给 0)。
    pub size: u64,
    /// sha256(小写无前缀);GitHub 没给 `digest` 时为 None。
    pub sha256: Option<String>,
}

/// 「检查更新」结果。`ok=false` 时 `error` 是给用户看的原因;
/// `has_update=true` 时 `url` 指向最新发布的下载页。
#[frb]
#[derive(Debug, Clone)]
pub struct UpdateCheck {
    /// 检查是否成功(网络/解析)。
    pub ok: bool,
    /// 当前运行的版本(编译时取自 Cargo.toml),检查失败时也总有值。
    pub current: String,
    /// GitHub 最新正式版 tag(如 `v0.5.1`);失败时为空。
    pub latest: String,
    /// 最新版本是否比当前新。
    pub has_update: bool,
    /// 最新发布的页面地址;没有更新或检查失败时为空。
    pub url: String,
    /// 失败原因;成功时为空。
    pub error: String,
    /// release notes(确认对话框展示;可能为空)。
    pub notes: String,
    /// 安装包资产;挑不到本平台的包时 `下载并更新` 应降级为打开下载页。
    pub assets: Vec<UpdateAsset>,
}

/// 下载进度帧(最后一帧 `done=true`;`error` 非空 = 失败)。
#[frb]
#[derive(Debug, Clone)]
pub struct UpdateProgress {
    pub received: u64,
    pub total: u64,
    pub done: bool,
    pub error: String,
    /// 成功时 = 下载完成的安装包路径。
    pub path: String,
}

/// 解包 / 退出替换这类"一句话结果"。
#[frb]
#[derive(Debug, Clone)]
pub struct UpdateOutcome {
    pub ok: bool,
    /// 成功时是给用户看的路径,失败时是原因。
    pub message: String,
}

/// 当前版本号(与发布物一致,取自 Cargo.toml)。
#[frb]
pub async fn app_version() -> String {
    crate::update::current_version().to_string()
}

/// 检查 GitHub Releases 上的最新正式版(手动触发;阻塞网络放在独立线程,
/// 见 [`run_blocking`])。
#[frb]
pub async fn check_update() -> UpdateCheck {
    run_blocking(|| {
        let current = crate::update::current_version().to_string();
        match crate::update::fetch_latest() {
            Ok(r) => UpdateCheck {
                ok: true,
                has_update: crate::update::is_newer(&r.tag, &current),
                latest: r.tag,
                url: r.url,
                current,
                error: String::new(),
                notes: r.notes,
                assets: r
                    .assets
                    .into_iter()
                    .map(|a| UpdateAsset {
                        name: a.name,
                        url: a.url,
                        size: a.size,
                        sha256: a.sha256,
                    })
                    .collect(),
            },
            Err(error) => UpdateCheck {
                ok: false,
                current,
                latest: String::new(),
                has_update: false,
                url: String::new(),
                error,
                notes: String::new(),
                assets: Vec::new(),
            },
        }
    })
}

/// 下载最新安装包到缓存,**流式推进度**。
///
/// 重新查一次 release 并由 Rust 侧挑本平台的包(与检查结果解耦,不依赖
/// UI 传参)。已存在且校验通过的包直接复用。取消订阅流即取消下载:
/// 下一帧推不进去 → 回调返回 false → 删除 `.part`。
#[frb]
pub fn download_update(sink: StreamSink<UpdateProgress>) {
    std::thread::spawn(move || {
        let send = |received: u64, total: u64, done: bool, error: &str, path: &str| {
            sink.add(UpdateProgress {
                received,
                total,
                done,
                error: error.to_string(),
                path: path.to_string(),
            })
            .is_ok()
        };
        let result = download_to_cache(|rec, tot| send(rec, tot, false, "", ""));
        match result {
            Ok((path, received, total)) => {
                let p = path.to_string_lossy().into_owned();
                send(received, total, true, "", &p);
            }
            Err(e) => {
                send(0, 0, true, &e, "");
            }
        }
    });
}

/// 查最新 release → 挑包 → 下载(或复用已下载的)。返回 (路径, received, total)。
fn download_to_cache(
    mut on_progress: impl FnMut(u64, u64) -> bool,
) -> Result<(std::path::PathBuf, u64, u64), String> {
    let rel = crate::update::fetch_latest()?;
    let asset = crate::update::pick_asset(&rel.assets)
        .ok_or("当前平台没有对应的安装包,请打开下载页手动更新")?;
    let dest = crate::update::update_dir().join(&asset.name);
    if crate::update::cached_asset_is_valid(&dest, asset.sha256.as_deref()) {
        let total = dest.metadata().map(|m| m.len()).unwrap_or(asset.size);
        on_progress(total, total);
        return Ok((dest, total, total));
    }
    crate::update::download(&asset.url, &dest, asset.sha256.as_deref(), on_progress)
        .map_err(|e| e.to_string())?;
    let total = dest.metadata().map(|m| m.len()).unwrap_or(asset.size);
    Ok((dest, total, total))
}

/// 解包缓存里最新的 DMG(挂载 → ditto → 卸载 → 验签 → 剥 quarantine)。
/// 在主程序退出前完成,失败原因能直接显示在面板上。`tag` 用于命名暂存目录。
#[frb]
pub async fn stage_update(tag: String) -> UpdateOutcome {
    run_blocking(|| {
        let dir = crate::update::update_dir();
        let Some(dmg) = crate::update::newest_dmg(&dir) else {
            return UpdateOutcome {
                ok: false,
                message: "找不到已下载的安装包,请重新下载".into(),
            };
        };
        match crate::update::stage_dmg(&dmg, &tag) {
            Ok(path) => UpdateOutcome {
                ok: true,
                message: path.to_string_lossy().into_owned(),
            },
            Err(e) => UpdateOutcome {
                ok: false,
                message: e,
            },
        }
    })
}

/// 启动 `--updater` 帮手并退出本程序(用户点「立即重启更新」)。
///
/// **成功的路径不返回**:`shim::hotkey("Q")` → `[NSApp terminate:]` 会在
/// 主线程把进程结束掉;只有每一步失败才回到这里,带着 `ok=false`。
#[frb]
pub async fn apply_update() -> UpdateOutcome {
    #[cfg(not(target_os = "macos"))]
    {
        UpdateOutcome {
            ok: false,
            message: "当前平台暂不支持自动更新,请打开下载页手动更新".into(),
        }
    }
    #[cfg(target_os = "macos")]
    run_blocking(|| {
        let fail = |m: String| UpdateOutcome {
            ok: false,
            message: m,
        };

        let Ok(exe) = std::env::current_exe() else {
            return fail("定位当前程序失败".into());
        };
        // 当前 .app(不是裸可执行文件时退回可执行文件所在目录)
        let target = exe
            .ancestors()
            .find(|a| a.join("Contents").join("Info.plist").exists())
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| {
                exe.parent()
                    .unwrap_or(std::path::Path::new("."))
                    .to_path_buf()
            });
        let cache = crate::update::update_dir();
        let Some(staging) = crate::update::staged_app(&cache) else {
            return fail("找不到解包后的新版本,请重新下载".into());
        };

        let mut cmd = std::process::Command::new(&exe);
        cmd.args(["--updater", "--target"])
            .arg(&target)
            .arg("--staging")
            .arg(&staging)
            .arg("--pid")
            .arg(std::process::id().to_string())
            .arg("--cache")
            .arg(&cache)
            .arg("--db")
            .arg(crate::db::db_path());
        if let Some(dmg) = crate::update::newest_dmg(&cache) {
            cmd.arg("--dmg").arg(dmg);
        }
        // 脱离会话:主程序退出后帮手不收信号、不被会话回收
        #[cfg(target_os = "macos")]
        unsafe {
            use std::os::unix::process::CommandExt;
            cmd.pre_exec(|| {
                if libc::setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        if let Err(e) = cmd.spawn() {
            return fail(format!("启动更新进程失败:{e}"));
        }
        eprintln!("[update] helper spawned, quitting…");

        // 给在途的后台写(落笔 edited 标记等)一点时间落地, 再走正常退出
        std::thread::sleep(std::time::Duration::from_millis(300));
        // ⌘⌃Q 的同一条路径:gl_run_on_main_sync → [NSApp terminate:nil]。
        // terminate 结束进程, 正常情况下这行之后什么都不会执行。
        shim::hotkey("Q");
        // 走到这里说明 quit 没生效 —— 帮手还在等主进程, 硬退。
        UpdateOutcome {
            ok: true,
            message: "已退出".into(),
        }
    })
}

/// 用系统默认浏览器打开一个 http(s) URL(「打开下载页」按钮)。
#[frb]
pub async fn open_url(url: String) {
    run_blocking(move || open_url_checked(&url));
}

// ---------------------------------------------------------------------------
// 导出
// ---------------------------------------------------------------------------

/// 导出当前页为 PDF。
#[frb]
pub async fn export_pdf() -> bool {
    run_blocking(shim::export_pdf)
}

/// 导出动画 GIF 并复制到剪贴板。
#[frb]
pub async fn export_animated_gif() -> bool {
    run_blocking(shim::export_gif)
}

// ---------------------------------------------------------------------------
// 数据结构 / 内部
// ---------------------------------------------------------------------------

#[frb]
#[derive(Debug, Clone)]
pub struct PageSummary {
    pub id: i64,
    pub width: i32,
    pub height: i32,
    pub stroke_count: u64,
}

#[frb]
#[derive(Debug, Clone)]
pub struct LensState {
    pub page_id: i64,
    pub pan_x: f64,
    pub pan_y: f64,
    pub zoom: f64,
}

/// 解析 `page_thumbnails_blob` 的自描述二进制块(与 Dart 侧同一格式)。
fn decode_thumb_blob(blob: &[u8]) -> Vec<PageThumb> {
    if blob.len() < 8
        || u32::from_le_bytes(blob[0..4].try_into().unwrap()) != crate::export::THUMB_BLOB_MAGIC
    {
        return Vec::new();
    }
    let count = u32::from_le_bytes(blob[4..8].try_into().unwrap());
    let mut out = Vec::new();
    let mut off = 8usize;
    for _ in 0..count {
        if off + 12 > blob.len() {
            break;
        }
        let id = i64::from_le_bytes(blob[off..off + 8].try_into().unwrap());
        let len = u32::from_le_bytes(blob[off + 8..off + 12].try_into().unwrap()) as usize;
        off += 12;
        if off + len > blob.len() {
            break;
        }
        out.push(PageThumb {
            id,
            png: blob[off..off + len].to_vec(),
        });
        off += len;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_from_json_tolerates_partial_payload() {
        let s = Settings::from_json(r#"{"color":3,"grid":true,"gridSize":42.5}"#).unwrap();
        assert_eq!(s.color, 3);
        assert!(s.grid);
        assert_eq!(s.grid_size, 42.5);
        assert!(!s.outline);
        assert_eq!(s.gif_fps, 0);
        // 涂鸦身份:缺字段 = 空/未保存
        assert_eq!(s.chat_api_base, "");
        assert_eq!(s.chat_user, "");
        assert!(!s.chat_has_password);
    }

    /// 涂鸦身份字段:字符串与 has_password 保真;密码本体永不进快照。
    #[test]
    fn test_settings_from_json_chat_auth() {
        let s = Settings::from_json(
            r#"{"chatApiBase":"https://192.168.31.58:23001","chatUser":"abc","chatHasPassword":true}"#,
        )
        .unwrap();
        assert_eq!(s.chat_api_base, "https://192.168.31.58:23001");
        assert_eq!(s.chat_user, "abc");
        assert!(s.chat_has_password);
        let j = serde_json::json!({
            "chatApiBase": "x", "chatUser": "u",
            // 即便推送方误发密码字段,from_json 也不解析它
            "chatPassword": "oops",
        });
        assert!(!Settings::from_json(&j.to_string()).unwrap().chat_has_password);
    }

    #[test]
    fn test_settings_from_json_empty_is_all_defaults() {
        let s = Settings::from_json("{}").unwrap();
        assert_eq!(s.color, 0);
        assert_eq!(s.width, 0);
        assert!(!s.infinite_canvas);
    }

    #[test]
    fn test_decode_thumb_blob_roundtrip() {
        let entries = vec![(7i64, vec![1u8, 2, 3]), (569i64, vec![9u8])];
        let blob = crate::export::encode_thumb_blob(&entries);
        let parsed = decode_thumb_blob(&blob);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].id, 7);
        assert_eq!(parsed[0].png, vec![1u8, 2, 3]);
        assert_eq!(parsed[1].id, 569);
        assert_eq!(parsed[1].png, vec![9u8]);
    }

    #[test]
    fn test_decode_thumb_blob_rejects_garbage() {
        assert!(decode_thumb_blob(&[]).is_empty());
        assert!(decode_thumb_blob(b"not a blob").is_empty());
        // 魔数正确但被截断:保留已解析部分而不是越界
        let mut blob = crate::export::encode_thumb_blob(&[(7i64, vec![1u8, 2, 3])]);
        blob.truncate(blob.len() - 1);
        assert!(decode_thumb_blob(&blob).is_empty());
    }
}
