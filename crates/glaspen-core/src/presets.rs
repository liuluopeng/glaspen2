//! 平台无关的"笔"预设与设置数值约束 —— macOS(ObjC)与 Windows(覆盖层)
//! 的单一事实源。此前色板/粗细表在两侧各存一份、压感→宽度公式在 ObjC 里
//! 硬编码了一份拷贝、GIF 钳制范围两侧已经漂移(0.05 vs 0.1),全部收拢到这里。
//!
//! 名字(颜色中文名/英文名)是平台 UI 字符串,不在这里:macOS 走双语
//! 本地化,Windows 只有中文 HUD。

/// 全饱和色板(对齐 rnote 实测色值),索引即「颜色 N」语义。
pub const COLOR_PRESETS: [(f64, f64, f64); 10] = [
    (0.839, 0.000, 0.227), // Red    #D6003A
    (1.000, 0.302, 0.000), // Orange #FF4D00
    (0.988, 0.718, 0.000), // Yellow #FCB700
    (0.000, 0.694, 0.431), // Green  #00B16E
    (0.431, 0.769, 0.957), // Cyan   #6EC4F4
    (0.000, 0.439, 0.741), // Blue   #0070BD
    (0.541, 0.000, 0.902), // Purple #8A00E6
    (1.000, 0.000, 0.502), // Pink   #FF0080
    (1.0, 1.0, 1.0),       // White
    (0.0, 0.0, 0.0),       // Black
];

/// 8 档线宽倍率,与 Flutter 设置面板的 8 档一一对应。
pub const WIDTH_PRESETS: [f64; 8] = [0.15, 0.3, 0.6, 1.0, 1.5, 2.0, 2.5, 3.5];

/// 落笔即时反馈用的原始笔宽(与 modeler 落笔平滑后的宽度同公式)。
pub fn pressure_raw_width(pressure: f64, width_scale: f64) -> f64 {
    crate::modeler::pressure_to_width(pressure, width_scale)
}

/// 最近颜色预设(平方欧氏距离;平手取靠前者 —— 与两侧原实现一致)。
pub fn nearest_color_index(r: f64, g: f64, b: f64) -> usize {
    let mut best = 0;
    let mut best_dist = f64::MAX;
    for (i, &(cr, cg, cb)) in COLOR_PRESETS.iter().enumerate() {
        let d = (r - cr) * (r - cr) + (g - cg) * (g - cg) + (b - cb) * (b - cb);
        if d < best_dist {
            best_dist = d;
            best = i;
        }
    }
    best
}

/// 最近粗细预设(平方距离)。
pub fn nearest_width_index(w: f64) -> usize {
    let mut best = 0;
    let mut best_dist = f64::MAX;
    for (i, &ww) in WIDTH_PRESETS.iter().enumerate() {
        let d = (w - ww) * (w - ww);
        if d < best_dist {
            best_dist = d;
            best = i;
        }
    }
    best
}

// ── 数值设置的合法区间 ───────────────────────────────────────────
// 键名用面板/FFI 的驼峰键(设置写入通道用同一套名字)。区间取两侧
// 现行实现中较宽者 —— 面板 UI 能产出的值都在区间内,历史存库值不会被
// 新钳制拒绝;作用只是挡住越界的垃圾输入。

struct Limit {
    key: &'static str,
    min: f64,
    max: f64,
    int: bool,
}

const LIMITS: [Limit; 5] = [
    Limit {
        key: "gridSize",
        min: 10.0,
        max: 200.0,
        int: false,
    },
    Limit {
        key: "gifFps",
        min: 1.0,
        max: 50.0,
        int: true,
    },
    Limit {
        key: "gifResolution",
        min: 0.1,
        max: 1.0,
        int: false,
    },
    Limit {
        key: "gifSpeed",
        min: 0.25,
        max: 20.0,
        int: false,
    },
    Limit {
        key: "gifEndMode",
        min: 0.0,
        max: 2.0,
        int: true,
    },
];

fn limit_for(key: &str) -> Option<&'static Limit> {
    LIMITS.iter().find(|l| l.key == key)
}

/// 钳制 double 型设置值(未知键原样返回)。
pub fn clamp_setting_double(key: &str, v: f64) -> f64 {
    match limit_for(key) {
        Some(l) if !l.int => v.clamp(l.min, l.max),
        _ => v,
    }
}

/// 钳制 int 型设置值(未知键原样返回)。
pub fn clamp_setting_int(key: &str, v: i32) -> i32 {
    match limit_for(key) {
        Some(l) if l.int => (v as f64).clamp(l.min, l.max) as i32,
        _ => v,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pressure_width_matches_modeler_formula() {
        // 与 modeler::pressure_to_width 的历史值一致(ObjC 拷贝的源头)
        assert!((pressure_raw_width(0.0, 1.0) - 1.0).abs() < 0.01);
        assert!((pressure_raw_width(0.5, 1.0) - (0.3 + 0.25 * 7.7)).abs() < 1e-9);
        assert!((pressure_raw_width(0.8, 2.0) - (0.3 + 0.64 * 7.7) * 2.0).abs() < 1e-9);
    }

    #[test]
    fn nearest_matches_expected_preset() {
        assert_eq!(nearest_color_index(0.839, 0.0, 0.227), 0); // 红
        assert_eq!(nearest_color_index(1.0, 1.0, 1.0), 8); // 白
        assert_eq!(nearest_color_index(0.0, 0.0, 0.0), 9); // 黑
        assert_eq!(nearest_color_index(1.0, 0.1, 0.45), 7); // 粉(比红/白近)
        assert_eq!(nearest_width_index(1.05), 3); // 1.0
        assert_eq!(nearest_width_index(2.6), 6); // 2.5
    }

    #[test]
    fn clamps_match_platform_histories() {
        assert_eq!(clamp_setting_double("gridSize", 5.0), 10.0);
        assert_eq!(clamp_setting_double("gridSize", 999.0), 200.0);
        assert_eq!(clamp_setting_int("gifFps", 0), 1);
        assert_eq!(clamp_setting_int("gifFps", 120), 50);
        assert_eq!(clamp_setting_double("gifResolution", 0.05), 0.1);
        assert_eq!(clamp_setting_double("gifSpeed", 0.1), 0.25);
        assert_eq!(clamp_setting_int("gifEndMode", 7), 2);
        // 未知键不钳
        assert_eq!(clamp_setting_double("unknownKey", -1.0), -1.0);
        assert_eq!(clamp_setting_int("unknownKey", -1), -1);
    }
}
