//! 气泡消息 → 晾衣架(Tendedero): 每条完结的消息渲染成透明底 PNG, 写入
//! 用户配置的监视文件夹(设置 bubbleHangFolder), 晾衣架应用监到新图片即
//! 挂上 —— 面板气泡 tab 照常显示, 两处同看。
//!
//! 渲染走纯软件路径(pagerender, 不依赖 cairo), 管线与缩略图同源:
//! 预乘 BGRA → 换通道 + 反预乘 → encode_png_rgba。
//! 铁律: 增强功能, 一切失败只记日志, 永不阻断涂鸦。

use super::bubbles::BubbleMsg;

/// 输出长边上限(px): 晾衣架卡片最大 ~320px, 留足清晰度即可, 不必全尺寸。
const MAX_EDGE: f64 = 1200.0;
/// 画布四周留白(px), 逃离软边抗锯齿与圆帽溢出。
const PAD: f64 = 8.0;

/// 锁内零 IO: finish_pending 结出消息后调用, 渲染/读设置/写盘交给 runtime。
pub(super) fn spawn_hang(msg: BubbleMsg) {
    crate::runtime().spawn(async move {
        hang_message(&msg).await;
    });
}

/// 消息完结后调用(runtime 线程): 读设置 → 渲染 → 写盘。
async fn hang_message(msg: &BubbleMsg) {
    let Some(folder) = crate::db::load_setting("bubble_hang_folder").await else {
        return;
    };
    let folder = folder.trim().to_string();
    if folder.is_empty() {
        return; // 未配置 = 关闭
    }
    let Some((png, w, h)) = render_message_png(msg) else {
        return;
    };
    let path = std::path::Path::new(&folder)
        .join(format!("bubble-{}-{}.png", msg.id, msg.ts_ms));
    match std::fs::write(&path, &png) {
        Ok(_) => tracing::info!(?path, w, h, size = png.len(), "气泡已挂上晾衣架"),
        Err(e) => tracing::warn!("晾衣架写盘失败({path:?}): {e}"),
    }
}

/// 消息 → 透明底 PNG(纯函数, 可测)。空消息或渲染异常返回 None。
pub(super) fn render_message_png(msg: &BubbleMsg) -> Option<(Vec<u8>, i32, i32)> {
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
    let scale = (MAX_EDGE / bw.max(bh)).min(1.0);
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
        let (png, w, h) = render_message_png(&m).expect("应渲染出 PNG");
        assert_eq!(&png[..4], &[0x89, b'P', b'N', b'G'], "PNG 魔数");
        // 内容 90×30 + 四周 8px 留白
        assert_eq!(w, 90 + 16);
        assert_eq!(h, 30 + 16);
    }

    #[test]
    fn long_message_is_scaled_down() {
        // 3000px 宽的一条 → 长边钳到 1200(缩放后 1200, 再加留白)
        let m = msg_with(&[(0.0, 0.0, 4.0), (3000.0, 0.0, 4.0)]);
        let (_, w, _) = render_message_png(&m).expect("应渲染出 PNG");
        assert_eq!(w, 1200 + 16);
    }

    #[test]
    fn empty_message_returns_none() {
        let m = BubbleMsg { id: 1, ts_ms: 0, strokes: vec![] };
        assert!(render_message_png(&m).is_none());
        let m2 = msg_with(&[]);
        assert!(render_message_png(&m2).is_none());
    }
}
