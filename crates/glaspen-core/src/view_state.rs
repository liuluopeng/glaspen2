//! 绘制/网格可见性状态(从 Swift `ViewSettings` 迁移)。
//!
//! 状态用原子量, 不涉及 AppKit; C ABI 符号与旧 Swift 导出一致。
#![cfg(target_os = "macos")]

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};

static INFINITE: AtomicBool = AtomicBool::new(false);
static STROKES_VISIBLE: AtomicBool = AtomicBool::new(true);
static MINIMAP: AtomicBool = AtomicBool::new(false);
static SHOW_GRID: AtomicBool = AtomicBool::new(false);
static GRID_SIZE_BITS: AtomicU64 = AtomicU64::new(40.0f64.to_bits());
static GRID_DIVIDER: AtomicI32 = AtomicI32::new(0);
static GRID_FOLLOW: AtomicBool = AtomicBool::new(false);

pub fn infinite() -> bool {
    INFINITE.load(Ordering::Relaxed)
}
pub fn set_infinite(on: bool) {
    INFINITE.store(on, Ordering::Relaxed);
}
pub fn strokes_visible() -> bool {
    STROKES_VISIBLE.load(Ordering::Relaxed)
}
pub fn set_strokes_visible(on: bool) {
    STROKES_VISIBLE.store(on, Ordering::Relaxed);
}
pub fn minimap() -> bool {
    MINIMAP.load(Ordering::Relaxed)
}
pub fn set_minimap(on: bool) {
    MINIMAP.store(on, Ordering::Relaxed);
}
pub fn show_grid() -> bool {
    SHOW_GRID.load(Ordering::Relaxed)
}
pub fn set_show_grid(on: bool) {
    SHOW_GRID.store(on, Ordering::Relaxed);
}
pub fn grid_size() -> f64 {
    f64::from_bits(GRID_SIZE_BITS.load(Ordering::Relaxed))
}
pub fn set_grid_size(size: f64) {
    GRID_SIZE_BITS.store(size.to_bits(), Ordering::Relaxed);
}
pub fn grid_divider() -> i32 {
    GRID_DIVIDER.load(Ordering::Relaxed)
}
pub fn set_grid_divider(divider: i32) {
    GRID_DIVIDER.store(divider, Ordering::Relaxed);
}
pub fn grid_follow() -> bool {
    GRID_FOLLOW.load(Ordering::Relaxed)
}
pub fn set_grid_follow(on: bool) {
    GRID_FOLLOW.store(on, Ordering::Relaxed);
}

/// 启动恢复: 尺寸 10..200 / 分隔 0..4 才写入, 其余保持默认/上次值(旧逻辑)。
pub fn startup_grid(show: bool, size: f64, follow: bool, divider: i32) {
    set_show_grid(show);
    if (10.0..=200.0).contains(&size) {
        set_grid_size(size);
    }
    set_grid_follow(follow);
    if (0..=4).contains(&divider) {
        set_grid_divider(divider);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_state_roundtrip_and_startup_clamps() {
        set_infinite(true);
        assert!(infinite());
        set_infinite(false);
        set_strokes_visible(false);
        assert!(!strokes_visible());
        set_strokes_visible(true);
        set_minimap(true);
        assert!(minimap());
        set_minimap(false);
        set_show_grid(false);
        set_grid_size(40.0);
        startup_grid(true, 55.0, true, 2);
        assert!(show_grid());
        assert_eq!(grid_size(), 55.0);
        assert!(grid_follow());
        assert_eq!(grid_divider(), 2);
        startup_grid(false, 999.0, false, 9);
        assert_eq!(grid_size(), 55.0);
        assert_eq!(grid_divider(), 2);
    }
}
