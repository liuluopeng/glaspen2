//! 笔外观设置(颜色/粗细/选中/历史/暂存; 从 Swift `PenSettingsStore` 迁移)。
//!
//! 颜色/宽度预设直接用 core 的 `presets`(与旧 Swift `PresetStore` 由 core 填充一致)。
#![cfg(target_os = "macos")]

use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};

static PEN_R_BITS: AtomicU64 = AtomicU64::new(1.0f64.to_bits());
static PEN_G_BITS: AtomicU64 = AtomicU64::new(0.0f64.to_bits());
static PEN_B_BITS: AtomicU64 = AtomicU64::new(0.0f64.to_bits());
static WIDTH_SCALE_BITS: AtomicU64 = AtomicU64::new(1.0f64.to_bits());
static SELECTED_COLOR: AtomicI32 = AtomicI32::new(0);
static SELECTED_WIDTH: AtomicI32 = AtomicI32::new(3);
// 旧 ObjC: 交换键初值给"可用的另一端"(黑/细), 无历史时可直接换过去。
static PREV_COLOR: AtomicI32 = AtomicI32::new(9);
static PREV_WIDTH: AtomicI32 = AtomicI32::new(2);
static PENDING_COLOR: AtomicI32 = AtomicI32::new(-1);
static PENDING_WIDTH: AtomicI32 = AtomicI32::new(-1);

pub fn pen_rgb() -> (f64, f64, f64) {
    (
        f64::from_bits(PEN_R_BITS.load(Ordering::Relaxed)),
        f64::from_bits(PEN_G_BITS.load(Ordering::Relaxed)),
        f64::from_bits(PEN_B_BITS.load(Ordering::Relaxed)),
    )
}
pub fn width_scale() -> f64 {
    f64::from_bits(WIDTH_SCALE_BITS.load(Ordering::Relaxed))
}
pub fn selected_color() -> i32 {
    SELECTED_COLOR.load(Ordering::Relaxed)
}
pub fn selected_width() -> i32 {
    SELECTED_WIDTH.load(Ordering::Relaxed)
}
pub fn prev_color() -> i32 {
    PREV_COLOR.load(Ordering::Relaxed)
}
pub fn prev_width() -> i32 {
    PREV_WIDTH.load(Ordering::Relaxed)
}
pub fn pending_color() -> i32 {
    PENDING_COLOR.load(Ordering::Relaxed)
}
pub fn pending_width() -> i32 {
    PENDING_WIDTH.load(Ordering::Relaxed)
}

pub fn set_rgb(r: f64, g: f64, b: f64) {
    PEN_R_BITS.store(r.to_bits(), Ordering::Relaxed);
    PEN_G_BITS.store(g.to_bits(), Ordering::Relaxed);
    PEN_B_BITS.store(b.to_bits(), Ordering::Relaxed);
}
pub fn set_width_scale(width: f64) {
    WIDTH_SCALE_BITS.store(width.to_bits(), Ordering::Relaxed);
}
pub fn set_selected_color(idx: i32) {
    SELECTED_COLOR.store(idx, Ordering::Relaxed);
}
pub fn set_selected_width(idx: i32) {
    SELECTED_WIDTH.store(idx, Ordering::Relaxed);
}
pub fn set_prev_color(idx: i32) {
    PREV_COLOR.store(idx, Ordering::Relaxed);
}
pub fn set_prev_width(idx: i32) {
    PREV_WIDTH.store(idx, Ordering::Relaxed);
}
pub fn set_pending_color(idx: i32) {
    PENDING_COLOR.store(idx, Ordering::Relaxed);
}
pub fn set_pending_width(idx: i32) {
    PENDING_WIDTH.store(idx, Ordering::Relaxed);
}
pub fn clear_pending_color() {
    set_pending_color(-1);
}
pub fn clear_pending_width() {
    set_pending_width(-1);
}

pub fn apply_color(idx: i32) {
    let (r, g, b) = if idx >= 0 {
        crate::presets::COLOR_PRESETS
            .get(idx as usize)
            .copied()
            .unwrap_or((0.0, 0.0, 0.0))
    } else {
        (0.0, 0.0, 0.0)
    };
    set_rgb(r, g, b);
    set_selected_color(idx);
}

pub fn apply_width(idx: i32) {
    let width = if idx >= 0 {
        crate::presets::WIDTH_PRESETS
            .get(idx as usize)
            .copied()
            .unwrap_or(0.0)
    } else {
        0.0
    };
    set_width_scale(width);
    set_selected_width(idx);
}

pub fn set_pen_settings(r: f64, g: f64, b: f64, width: f64, color_idx: i32, width_idx: i32) {
    set_rgb(r, g, b);
    set_width_scale(width);
    set_selected_color(color_idx);
    set_selected_width(width_idx);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_updates() {
        assert_eq!(pen_rgb(), (1.0, 0.0, 0.0));
        assert_eq!(width_scale(), 1.0);
        assert_eq!((selected_color(), selected_width()), (0, 3));
        assert_eq!((prev_color(), prev_width()), (9, 2));
        assert_eq!((pending_color(), pending_width()), (-1, -1));
        set_rgb(0.1, 0.2, 0.3);
        set_width_scale(2.0);
        set_selected_color(4);
        set_selected_width(5);
        set_prev_color(1);
        set_prev_width(2);
        set_pending_color(3);
        set_pending_width(4);
        assert_eq!(pen_rgb(), (0.1, 0.2, 0.3));
        assert_eq!(width_scale(), 2.0);
        assert_eq!((selected_color(), selected_width()), (4, 5));
        assert_eq!((prev_color(), prev_width()), (1, 2));
        assert_eq!((pending_color(), pending_width()), (3, 4));
        clear_pending_color();
        clear_pending_width();
        assert_eq!((pending_color(), pending_width()), (-1, -1));
        apply_color(0);
        assert_eq!(selected_color(), 0);
        assert_eq!(pen_rgb(), crate::presets::COLOR_PRESETS[0]);
        apply_width(0);
        assert_eq!(selected_width(), 0);
        assert_eq!(width_scale(), crate::presets::WIDTH_PRESETS[0]);
    }
}
