#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

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

// Cairo: real crate when available, stub for cross-compilation / Windows
#[cfg(all(feature = "cairo_real", not(target_os = "windows")))]
extern crate cairo;
#[cfg(any(not(feature = "cairo_real"), target_os = "windows"))]
#[path = "cairo_stub.rs"]
pub mod cairo;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(windows)]
pub mod cairo_renderer;

#[cfg(target_os = "windows")]
pub mod windows;

pub mod db;
#[cfg(all(feature = "cairo_real", not(target_os = "windows")))]
pub mod draw;
pub mod export;
pub mod modeler;
pub mod ocr;
pub mod pdf;
pub mod state;

// Re-export FFI functions from export module so crate::glaspen2_* paths work
pub use export::*;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Stroke data stored in STROKES — used for rendering, SVG/GIF export, and XOJ save.
pub struct Stroke {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub points: Vec<(f64, f64, f64, f64)>, // (x, y, width, relative_time)
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

pub(crate) fn pressure_to_width(pressure: f64, width_scale: f64) -> f64 {
    if pressure > 0.01 {
        (0.3 + pressure * pressure * 7.7) * width_scale
    } else {
        1.0 * width_scale
    }
}

pub(crate) fn desktop_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string()))
            .join("Desktop")
    }
    #[cfg(not(target_os = "windows"))]
    {
        PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
            .join("Desktop")
    }
}

pub(crate) fn timestamped_path() -> PathBuf {
    desktop_path().join(timestamped_name("png"))
}

pub(crate) fn timestamped_name(ext: &str) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600 + 8) % 24;
    let days = secs / 86400;
    let y = 1970 + days / 365;
    let d = days % 365;
    format!("glaspen2_{:04}-{:03}_{:02}-{:02}-{:02}.{}", y, d, h, m, s, ext)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn test_build_cropped_svg_empty() {
        STROKES.lock().unwrap().clear();
        assert!(crate::export::build_cropped_svg().is_none());
    }

    #[test]
    fn test_build_cropped_svg_one_stroke() {
        STROKES.lock().unwrap().clear();
        STROKES.lock().unwrap().push(Stroke {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            points: vec![(0.0, 0.0, 2.0, 0.0), (10.0, 10.0, 3.0, 1.0)],
        });
        let svg = crate::export::build_cropped_svg().unwrap();
        assert!(svg.starts_with("<svg"), "SVG should start with <svg tag");
        assert!(svg.contains("stroke-width"), "should have stroke-width attr");
        assert!(svg.contains("</svg>\n"), "should close svg tag");
        STROKES.lock().unwrap().clear();
    }

    #[test]
    fn test_stroke_avg_width() {
        let s = Stroke {
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
