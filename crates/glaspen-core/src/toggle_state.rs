//! 功能开关与 GIF 记录参数(从 Swift `ToggleSettings` 迁移)。
//!
//! 状态用原子量, 不涉及 AppKit; C ABI 符号与旧 Swift 导出一致。
#![cfg(target_os = "macos")]

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(true);
static ETHEREAL: AtomicBool = AtomicBool::new(false);
static PRESSURE_MONITOR: AtomicBool = AtomicBool::new(false);
static CHAT_INTEGRATION: AtomicBool = AtomicBool::new(false);
static GIF_FPS: AtomicI32 = AtomicI32::new(15);
static GIF_RESOLUTION_BITS: AtomicU64 = AtomicU64::new(0.5f64.to_bits());
static GIF_SPEED_BITS: AtomicU64 = AtomicU64::new(2.0f64.to_bits());
static GIF_END_MODE: AtomicI32 = AtomicI32::new(1);

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}
pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}
pub fn ethereal() -> bool {
    ETHEREAL.load(Ordering::Relaxed)
}
pub fn set_ethereal(on: bool) {
    ETHEREAL.store(on, Ordering::Relaxed);
}
pub fn pressure_monitor() -> bool {
    PRESSURE_MONITOR.load(Ordering::Relaxed)
}
pub fn set_pressure_monitor(on: bool) {
    PRESSURE_MONITOR.store(on, Ordering::Relaxed);
}
pub fn chat_integration() -> bool {
    CHAT_INTEGRATION.load(Ordering::Relaxed)
}
pub fn set_chat_integration(on: bool) {
    CHAT_INTEGRATION.store(on, Ordering::Relaxed);
}
pub fn gif_fps() -> i32 {
    GIF_FPS.load(Ordering::Relaxed)
}
pub fn gif_resolution() -> f64 {
    f64::from_bits(GIF_RESOLUTION_BITS.load(Ordering::Relaxed))
}
pub fn gif_speed() -> f64 {
    f64::from_bits(GIF_SPEED_BITS.load(Ordering::Relaxed))
}
pub fn gif_end_mode() -> i32 {
    GIF_END_MODE.load(Ordering::Relaxed)
}

pub fn set_gif(fps: i32, resolution: f64, speed: f64, end_mode: i32) {
    GIF_FPS.store(fps, Ordering::Relaxed);
    GIF_RESOLUTION_BITS.store(resolution.to_bits(), Ordering::Relaxed);
    GIF_SPEED_BITS.store(speed.to_bits(), Ordering::Relaxed);
    GIF_END_MODE.store(end_mode, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_and_gif_roundtrip() {
        assert!(enabled());
        set_enabled(false);
        assert!(!enabled());
        set_enabled(true);
        set_ethereal(true);
        assert!(ethereal());
        set_ethereal(false);
        set_pressure_monitor(true);
        assert!(pressure_monitor());
        set_pressure_monitor(false);
        set_chat_integration(true);
        assert!(chat_integration());
        set_chat_integration(false);
        assert_eq!(gif_fps(), 15);
        set_gif(30, 0.75, 3.0, 2);
        assert_eq!(
            (gif_fps(), gif_resolution(), gif_speed(), gif_end_mode()),
            (30, 0.75, 3.0, 2)
        );
    }
}
