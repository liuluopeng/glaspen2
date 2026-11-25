//! 渲染设置(玻璃/描边/软阴影/反色/翻转; 从 Swift `RenderSettings` 迁移)。
//!
//! 原子量存储; 启动恢复的钳制/保留语义与旧 Swift 实现一致。
#![cfg(target_os = "macos")]

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};

static GLASS_ENABLED: AtomicBool = AtomicBool::new(false);
static GLASS_OPACITY_BITS: AtomicU64 = AtomicU64::new(0.45f64.to_bits());
static GLASS_FOLLOW: AtomicBool = AtomicBool::new(true);
static OUTLINE: AtomicBool = AtomicBool::new(false);
static SOFT_SHADOW: AtomicBool = AtomicBool::new(false);
static INVERT_INK: AtomicBool = AtomicBool::new(false);
static INVERT_FPS: AtomicI32 = AtomicI32::new(30);
static FLIP_EFFECT: AtomicI32 = AtomicI32::new(0);

pub fn glass_enabled() -> bool {
    GLASS_ENABLED.load(Ordering::Relaxed)
}
pub fn set_glass_enabled(on: bool) {
    GLASS_ENABLED.store(on, Ordering::Relaxed);
}
pub fn glass_opacity() -> f64 {
    f64::from_bits(GLASS_OPACITY_BITS.load(Ordering::Relaxed))
}
pub fn glass_follow() -> bool {
    GLASS_FOLLOW.load(Ordering::Relaxed)
}
pub fn set_glass_follow(on: bool) {
    GLASS_FOLLOW.store(on, Ordering::Relaxed);
}
pub fn outline() -> bool {
    OUTLINE.load(Ordering::Relaxed)
}
pub fn set_outline(on: bool) {
    OUTLINE.store(on, Ordering::Relaxed);
}
pub fn soft_shadow() -> bool {
    SOFT_SHADOW.load(Ordering::Relaxed)
}
pub fn set_soft_shadow(on: bool) {
    SOFT_SHADOW.store(on, Ordering::Relaxed);
}
pub fn invert_ink() -> bool {
    INVERT_INK.load(Ordering::Relaxed)
}
pub fn set_invert_ink(on: bool) {
    INVERT_INK.store(on, Ordering::Relaxed);
}
pub fn invert_fps() -> i32 {
    INVERT_FPS.load(Ordering::Relaxed)
}
pub fn set_invert_fps(fps: i32) {
    INVERT_FPS.store(fps, Ordering::Relaxed);
}
pub fn flip_effect() -> i32 {
    FLIP_EFFECT.load(Ordering::Relaxed)
}
pub fn set_flip_effect(effect: i32) {
    FLIP_EFFECT.store(effect, Ordering::Relaxed);
}

/// 启动恢复: 库里没存 opacity(0) 时保留默认 0.45(历史 bug 修复点)。
pub fn startup_glass(opacity: f64, enabled: bool) {
    if opacity > 0.0 {
        GLASS_OPACITY_BITS.store(opacity.to_bits(), Ordering::Relaxed);
    }
    set_glass_enabled(enabled);
}

/// 启动恢复反色: fps 仅接受 10/30/60/100, ink 照写。
pub fn startup_invert(fps: i32, ink: bool) {
    if matches!(fps, 10 | 30 | 60 | 100) {
        set_invert_fps(fps);
    }
    set_invert_ink(ink);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_defaults_and_startup_semantics() {
        assert!(!glass_enabled());
        assert_eq!(glass_opacity(), 0.45);
        assert!(glass_follow());
        assert!(!outline());
        assert!(!soft_shadow());
        assert!(!invert_ink());
        assert_eq!(invert_fps(), 30);
        assert_eq!(flip_effect(), 0);
        startup_glass(0.0, true);
        assert!(glass_enabled());
        assert_eq!(glass_opacity(), 0.45);
        startup_glass(0.8, false);
        assert!(!glass_enabled());
        assert_eq!(glass_opacity(), 0.8);
        startup_invert(60, true);
        assert_eq!(invert_fps(), 60);
        assert!(invert_ink());
        startup_invert(99, false);
        assert_eq!(invert_fps(), 60);
        assert!(!invert_ink());
        set_flip_effect(1);
        assert_eq!(flip_effect(), 1);
    }
}
