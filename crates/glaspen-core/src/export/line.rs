//! 晾衣架卡片渲染: 把一条完结的气泡消息渲染成透明底 PNG(macOS 晾衣架
//! 卡片/预览共用, 见 parts/line.m)。纯软件路径(pagerender, 不依赖
//! cairo), 管线与缩略图同源: 预乘 BGRA → 换通道 + 反预乘 →
//! encode_png_rgba。

use super::bubbles::BubbleMsg;

/// 卡片长边上限(px): 晾衣架卡片高约 104pt, 留足清晰度。
pub(crate) const CARD_EDGE: i32 = 320;
/// 预览长边上限(px): 点击卡片的大图。
pub(crate) const PREVIEW_EDGE: i32 = 1100;
/// 画布四周留白(px), 逃离软边抗锯齿与圆帽溢出。
const PAD: f64 = 8.0;

/// 消息 → 透明底 PNG(纯函数, 可测)。空消息或渲染异常返回 None。
/// 消息 → 透明底 PNG(纯函数, 可测)。空消息或渲染异常返回 None。
/// `max_edge` = 输出长边上限(等比缩小, 不放大)。
pub(super) fn render_message_png(msg: &BubbleMsg, max_edge: f64) -> Option<(Vec<u8>, i32, i32)> {
    // 包围盒(画布坐标)
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    let mut any = false;
    for s in &msg.strokes {
        for &(x, y, _) in &s.pts {
            any = true;
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    if !any || x1 < x0 || y1 < y0 {
        return None;
    }
    let bw = (x1 - x0).max(1.0);
    let bh = (y1 - y0).max(1.0);
    // 等比适配(不放大), 四周留白
    let scale = (max_edge / bw.max(bh)).min(1.0);
    let w = ((bw * scale) as i32 + PAD as i32 * 2).max(1);
    let h = ((bh * scale) as i32 + PAD as i32 * 2).max(1);
    let lay = crate::pagerender::PageLayout {
        scale,
        ox: PAD,
        oy: PAD,
        w: bw * scale,
        h: bh * scale,
    };

    // BubbleStroke(rgb + (x,y,w)) → PageStroke(r,g,b + (x,y,w,t))。
    // 单点笔画会被 render_strokes(要求 ≥2 点)跳过, 复制成两点 → fill_disc 圆点。
    let quads: Vec<Vec<(f64, f64, f64, f64)>> = msg
        .strokes
        .iter()
        .map(|s| match s.pts.len() {
            0 => Vec::new(),
            1 => {
                let (x, y, w) = s.pts[0];
                vec![(x, y, w, 0.0), (x, y, w, 0.0)]
            }
            _ => s.pts.iter().map(|&(x, y, w)| (x, y, w, 0.0)).collect(),
        })
        .collect();
    let views: Vec<crate::pagerender::PageStroke<'_>> = msg
        .strokes
        .iter()
        .zip(&quads)
        .map(|(s, pts)| crate::pagerender::PageStroke {
            r: ((s.rgb >> 16) & 0xFF) as f64 / 255.0,
            g: ((s.rgb >> 8) & 0xFF) as f64 / 255.0,
            b: (s.rgb & 0xFF) as f64 / 255.0,
            points: pts,
        })
        .collect();

    let mut data = vec![0u8; w as usize * h as usize * 4];
    let stride = w as usize * 4;
    {
        let mut surf = crate::pagerender::PageSurface {
            data: &mut data,
            w,
            h,
            stride,
        };
        crate::pagerender::render_strokes(
            &mut surf,
            &lay,
            &views,
            x0, // pan = 包围盒原点: 画布坐标折叠到缓冲左上角
            y0,
            1.0,
            &crate::pagerender::RenderOpts {
                alpha: 1.0,
                white_bg: false, // 透明底: 晾衣架玻璃卡片直接衬底
            },
        );
    }
    // PageSurface = 预乘 BGRA → RGBA + 反预乘 → PNG
    let mut rgba: Vec<u8> = data
        .chunks_exact(4)
        .flat_map(|px| [px[2], px[1], px[0], px[3]])
        .collect();
    super::unpremultiply_rgba(&mut rgba);
    let png = super::thumbs::encode_png_rgba(&rgba, w as u32, h as u32)?;
    Some((png, w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg_with(points: &[(f64, f64, f64)]) -> BubbleMsg {
        BubbleMsg {
            id: 7,
            ts_ms: 1_700_000_000_000,
            strokes: vec![super::super::bubbles::BubbleStroke {
                rgb: 0xFF8800,
                width_scale: 1.0,
                pts: points.to_vec(),
                gap_ms: 0,
            }],
        }
    }

    #[test]
    fn renders_png_with_transparent_pad() {
        let m = msg_with(&[(10.0, 10.0, 3.0), (60.0, 40.0, 5.0), (100.0, 20.0, 3.0)]);
        let (png, w, h) = render_message_png(&m, 1200.0).expect("应渲染出 PNG");
        assert_eq!(&png[..4], &[0x89, b'P', b'N', b'G'], "PNG 魔数");
        // 内容 90×30 + 四周 8px 留白
        assert_eq!(w, 90 + 16);
        assert_eq!(h, 30 + 16);
    }

    #[test]
    fn long_message_is_scaled_down() {
        // 3000px 宽的一条 → 长边钳到 1200(缩放后 1200, 再加留白)
        let m = msg_with(&[(0.0, 0.0, 4.0), (3000.0, 0.0, 4.0)]);
        let (_, w, _) = render_message_png(&m, 1200.0).expect("应渲染出 PNG");
        assert_eq!(w, 1200 + 16);
    }

    #[test]
    fn empty_message_returns_none() {
        let m = BubbleMsg { id: 1, ts_ms: 0, strokes: vec![] };
        assert!(render_message_png(&m, 320.0).is_none());
        let m2 = msg_with(&[]);
        assert!(render_message_png(&m2, 320.0).is_none());
    }
}
