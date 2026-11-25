//! 时光隧道翻页的纯几何(从 Swift `TunnelGeometry` 迁移)。
//!
//! 输入只有页距 `zc` 与动画参数/表面尺寸, 输出卡片在屏幕上的
//! (cx, cy, w, h, alpha)。不涉及 AppKit/cairo, 因此放在 core 里,
//! macOS Swift 壳通过 FFI 调用; Windows/测试也可复用同一公式。
//!
//! 公式逐项对应迁移前的 Swift 实现:
//! `s = 1/(1+k*zc)`, 越深越小; 卡片轻微左移/上移; 负页距线性淡出,
//! 正页距双曲淡出; 可视范围 `|-SPAN|..FRONT` 与缩放 `(0.06, 2.4]` 之外返回 None。

/// 隧道可向后观察的最大页距。
pub const SPAN: f64 = 2.5;
/// 隧道最前面的页距(动画起点)。
pub const FRONT: f64 = 4.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TunnelPlacement {
    pub cx: f64,
    pub cy: f64,
    pub w: f64,
    pub h: f64,
    pub alpha: f64,
}

/// 由页距计算卡片矩形/透明度; 不可见时返回 None。
pub fn place(
    zc: f64,
    k: f64,
    travel: f64,
    surf_w: f64,
    surf_h: f64,
) -> Option<TunnelPlacement> {
    if zc < -SPAN || zc > FRONT {
        return None;
    }
    let s = 1.0 / (1.0 + k * zc);
    if s <= 0.06 || s > 2.4 {
        return None;
    }
    let w = surf_w * s;
    let h = surf_h * s;
    let cx = surf_w * 0.5 + surf_w * 0.0045 * zc;
    let cy = surf_h * 0.5 - surf_h * 0.027 * zc;

    let mut alpha = if zc < 0.0 {
        let a = 1.0 + zc / travel;
        if a < 0.0 { 0.0 } else { a }
    } else {
        1.0 / (1.0 + 0.90 * zc)
    };
    if alpha > 1.0 {
        alpha = 1.0;
    }
    Some(TunnelPlacement { cx, cy, w, h, alpha })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    #[test]
    fn zero_depth_is_full_surface() {
        let p = place(0.0, 0.45, 1.0, 1000.0, 500.0).unwrap();
        approx(p.cx, 500.0);
        approx(p.cy, 250.0);
        approx(p.w, 1000.0);
        approx(p.h, 500.0);
        approx(p.alpha, 1.0);
    }

    #[test]
    fn positive_depth_sits_inside_and_fades() {
        let p = place(1.0, 0.45, 1.0, 1000.0, 500.0).unwrap();
        let s = 1.0 / 1.45;
        approx(p.w, 1000.0 * s);
        approx(p.h, 500.0 * s);
        approx(p.cx, 500.0 + 4.5);
        approx(p.cy, 250.0 - 13.5);
        approx(p.alpha, 1.0 / 1.9);
    }

    #[test]
    fn negative_depth_uses_linear_travel_fade() {
        let p = place(-0.5, 0.45, 1.0, 1000.0, 500.0).unwrap();
        approx(p.alpha, 0.5);
        approx(p.cx, 500.0 - 2.25);
        approx(p.cy, 250.0 + 6.75);
    }

    #[test]
    fn out_of_range_and_too_small_or_large_are_none() {
        assert!(place(-SPAN, 0.45, 1.0, 1000.0, 500.0).is_none());
        assert!(place(FRONT + 0.01, 0.45, 1.0, 1000.0, 500.0).is_none());
        assert!(place(-1.5, 0.45, 1.0, 1000.0, 500.0).is_none());
    }

    #[test]
    fn alpha_is_clamped_to_unit_and_travel_clamp_respected() {
        let p = place(-1.0, 0.0, 1.0, 100.0, 100.0).unwrap();
        approx(p.alpha, 0.0);
        let q = place(-0.1, 0.0, 1.0, 100.0, 100.0).unwrap();
        approx(q.alpha, 0.9);
    }
}
