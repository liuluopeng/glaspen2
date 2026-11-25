//! 画布输入坐标变换(从 Swift `canvas_input_x/y` 迁移)。
//!
//! 视图坐标 → 画布坐标; 无限画布用镜头(pan/zoom), 翻页模式用页偏移,
//! 缩放适配页时再反解 paper 变换。纯公式, 不依赖 AppKit。

/// 无限画布: view → canvas。
pub fn infinite_input(view: f64, zoom: f64, pan: f64) -> f64 {
    view / zoom + pan
}

/// 翻页模式: view → canvas(页偏移 + 可选缩放适配)。
pub fn page_input(
    view: f64,
    page_off: f64,
    is_fit: bool,
    paper_origin: f64,
    paper_scale: f64,
) -> f64 {
    if is_fit {
        (view - page_off - paper_origin) / paper_scale
    } else {
        view - page_off
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infinite_applies_zoom_then_pan() {
        assert_eq!(infinite_input(200.0, 2.0, 50.0), 150.0);
        assert_eq!(infinite_input(0.0, 1.0, 10.0), 10.0);
    }

    #[test]
    fn page_plain_and_fit() {
        assert_eq!(page_input(200.0, 20.0, false, 0.0, 1.0), 180.0);
        assert_eq!(page_input(200.0, 20.0, true, 10.0, 2.0), 85.0);
        assert_eq!(page_input(20.0, 0.0, true, 0.0, 0.5), 40.0);
    }
}
