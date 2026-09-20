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
}

impl Settings {
    /// 宽松解析:推送方只发部分字段(旧 sync_settings_panel 只发 12 项)。
    fn from_json(json: &str) -> Option<Settings> {
        let v: serde_json::Value = serde_json::from_str(json).ok()?;
        let b = |k: &str| v.get(k).and_then(|x| x.as_bool()).unwrap_or(false);
        let i = |k: &str| v.get(k).and_then(|x| x.as_i64()).unwrap_or(0) as i32;
        let f = |k: &str| v.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0);
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
