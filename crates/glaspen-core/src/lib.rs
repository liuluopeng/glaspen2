// glaspen-core — 平台无关的共享核心:存储、笔迹模型、cairo 渲染、
// 导出、更新检查,以及手写消息/共享上行的 FFI(供 ObjC / Windows 覆盖层
// / FRB 面板三方调用)。
//
// 历史规则沿用:这些 FFI 是 extern "C" 裸指针接口,不标 unsafe fn;
// 对结构性 lint 做 crate 级豁免。
#![allow(clippy::not_unsafe_ptr_arg_deref)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// Tokio runtime for bridging sync FFI → async SQLite.
/// The runtime is lazily created on first use and lives for the app's lifetime.
pub fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| tokio::runtime::Runtime::new().expect("Failed to create tokio runtime"))
}

// ---------------------------------------------------------------------------
// Module declarations
// ---------------------------------------------------------------------------

// 可配置动作的语义 effect 层(动作 id → 平台无关副作用, D3a)
pub mod action_effect;

// Cairo 渲染:唯一实现是 cairo_dl(动态加载 libcairo,源自 wAPItry 已验证原型)
pub mod cairo_dl;
// 无限画布镜头数学(pan/zoom/锚点/限幅/居中/持久化格式, D5a)
pub mod camera;
pub mod canvas_input;
pub mod chord;

pub mod db;
pub mod device;
// 绘制指令规范(绘制 pass/分层/宽度/脏区;D9a 规范,不接线两端)
pub mod draw_spec;
pub mod export;
// 网格/分栏几何(细网格 + 屏幕分界线 + 分栏参考线线段表, D6)
pub mod grid;
// 用户可见文案表(中/英)单源 + 查表 API/审计 JSON(D10a; FFI 在 export)
pub mod i18n;
// 页面缩略图条的页窗与条带布局(当前页 ±5 页 + 右缘竖条几何, D7)
pub mod minimap;
pub mod modeler;
// 单笔模型器会话(参数/笔宽公式/begin-move-finish 生命周期单源, D9b)
pub mod modeler_session;
pub mod ocr;
pub mod pageinfo;
pub mod pagerender;
pub mod pdf;
#[cfg(target_os = "macos")]
pub mod perf;
pub mod presets;
pub mod protocol;
// 录制会话状态机(GIF/⌘⌃2 草稿/⌘⌃3 直发;状态/起止/事件/用户可见动作单源, D8a)
pub mod recording;
pub mod settings;
pub mod settings_apply;
// 设置面板表单元数据(分区/顺序/l10n 键/控件/档位/范围, D11a)
pub mod settings_form;
// 面板设置快照构建单源(字段集合/顺序/类型/缺省值/DB 回退, D12)
#[cfg(target_os = "macos")]
pub mod pen_settings;
#[cfg(target_os = "macos")]
pub mod render_state;
pub mod settings_snapshot;
#[cfg(target_os = "macos")]
pub mod shell_state;
#[cfg(target_os = "macos")]
pub mod shortcut_state;
pub mod state;
#[cfg(target_os = "macos")]
pub mod toggle_state;
pub mod tunnel;
pub mod ui;
pub mod update;
pub mod updater;
#[cfg(target_os = "macos")]
pub mod view_state;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Stroke data stored in STROKES — used for rendering, SVG/GIF export, and XOJ save.
/// `id` is the DB row id (0 when the stroke has no DB row yet).
#[derive(Clone)]
pub struct Stroke {
    pub id: i64,
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub points: Vec<(f64, f64, f64, f64)>, // (x, y, width, relative_time)
}

/// 载入历史笔迹时剔除模型器遗留的坏点(负相对时间/非正宽度/NaN)。
/// 这些点会让 GIF 的整笔 dur<=0 被丢弃, 表现为录制动画“断触”。
pub(crate) fn sanitize_loaded_points(
    points: Vec<(f64, f64, f64, f64)>,
) -> Vec<(f64, f64, f64, f64)> {
    points
        .into_iter()
        .filter(|&(x, y, w, t)| {
            x.is_finite() && y.is_finite() && w.is_finite() && w > 0.0 && t.is_finite() && t >= 0.0
        })
        .collect()
}

impl Stroke {
    pub fn avg_width(&self) -> f64 {
        if self.points.is_empty() {
            return 1.0;
        }
        self.points.iter().map(|p| p.2).sum::<f64>() / self.points.len() as f64
    }
}

/// All strokes in memory — used by rendering, export, and FFI.
pub static STROKES: Mutex<Vec<Stroke>> = Mutex::new(Vec::new());

/// Tracks the current stroke's start timestamp for the raw (DB) draw path.
pub(crate) static RAW_STROKE_START: Mutex<Option<f64>> = Mutex::new(None);

// ---------------------------------------------------------------------------
// Shared helpers (used by export module)
// ---------------------------------------------------------------------------

// 压感→笔宽唯一实现在 modeler;这里 re-export 保持既有 `crate::pressure_to_width`
// 路径(export 模块与本文件测试)不动。
pub use modeler::pressure_to_width;

pub fn desktop_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string()))
            .join("Desktop")
    }
    #[cfg(not(target_os = "windows"))]
    {
        PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string())).join("Desktop")
    }
}

pub fn timestamped_path() -> PathBuf {
    desktop_path().join(timestamped_name("png"))
}

pub fn timestamped_name(ext: &str) -> String {
    // chrono::Local: 闰年/时区正确(旧手写版没算闰年, 文件名里的"天"
    // 会比真实日期漂移约两周)
    format!(
        "glaspen2_{}_{}",
        chrono::Local::now().format("%Y-%j_%H-%M-%S"),
        ext
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #[test]
    fn sanitize_loaded_points_drops_bogus_tail() {
        let pts = vec![
            (0.0, 0.0, 2.0, 0.0),
            (1.0, 1.0, 2.0, 0.01),
            (1.0, 1.0, 1.0, -4.79),
        ];
        let clean = super::sanitize_loaded_points(pts);
        assert_eq!(clean.len(), 2);
        assert!(clean.iter().all(|p| p.3 >= 0.0));
        assert_eq!(clean[1].2, 2.0);
    }

    use super::*;

    /// 串行锁:STROKES/模型器是进程级全局,改它们的测试共用一把锁。
    /// glaspen2 根 crate 与本 crate 的导出测试都引用 crate::tests::TEST_LOCK。
    pub(crate) static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_stroke_avg_width() {
        let s = Stroke {
            id: 0,
            r: 0.0,
            g: 0.0,
            b: 0.0,
            points: vec![(0.0, 0.0, 2.0, 0.0), (10.0, 10.0, 4.0, 1.0)],
        };
        let avg = s.avg_width();
        assert!((avg - 3.0).abs() < 0.01);
    }

    #[test]
    fn test_empty_stroke_avg_width() {
        let s = Stroke {
            id: 0,
            r: 0.0,
            g: 0.0,
            b: 0.0,
            points: vec![],
        };
        assert!((s.avg_width() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_pressure_to_width_zero_pressure() {
        assert!((pressure_to_width(0.0, 1.0) - 1.0).abs() < 1e-6);
    }
}
