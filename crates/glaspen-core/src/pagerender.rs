//! 页快照渲染(纯函数,可单测)。
//!
//! 翻页动效(时光隧道)与单页 PNG 导出共用:把**某一页**的笔迹画进一个
//! 独立的像素缓冲。壳层再把这张缓冲画成隧道里的一块"卡片"。
//!
//! 关键点(都是踩过的坑):
//! - 页几何 ≠ 当前屏幕时, 页按等比缩放 + 居中渲染(scale-to-fit),
//!   坐标换算集中在 [`layout`] 一处, 出错会整页跑出画面;
//! - 缓冲按调用方给的容量分配, 渲染矩形必须落在容量内(越界写 = 段错误);
//! - 笔迹画在透明底上(玻璃上本来就是墨迹悬浮), 白底是上层合成的选项。

/// 页在输出缓冲里的等比缩放布局(全部为输出缓冲的像素单位)。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageLayout {
    /// 页内容缩放(页几何像素 → 输出像素)。
    pub scale: f64,
    /// 页左上角在输出缓冲中的偏移。
    pub ox: f64,
    pub oy: f64,
    /// 页内容在输出缓冲中的尺寸(= 页几何 × scale)。
    pub w: f64,
    pub h: f64,
}

/// 把页几何(`pw × ph`)等比缩放并居中进 `out_w × out_h`。
pub fn layout(pw: i32, ph: i32, out_w: i32, out_h: i32) -> PageLayout {
    let (pw, ph) = (pw.max(1) as f64, ph.max(1) as f64);
    let (ow, oh) = (out_w.max(1) as f64, out_h.max(1) as f64);
    let scale = (ow / pw).min(oh / ph);
    let (w, h) = (pw * scale, ph * scale);
    PageLayout {
        scale,
        ox: ((ow - w) * 0.5).floor(),
        oy: ((oh - h) * 0.5).floor(),
        w,
        h,
    }
}

/// 32bit BGRA 缓冲(预乘),`stride` 以字节计(通常 = w × 4)。
pub struct PageSurface<'a> {
    pub data: &'a mut [u8],
    pub w: i32,
    pub h: i32,
    pub stride: usize,
}

impl PageSurface<'_> {
    /// 直接按 cairo 的数据格式合成一个带 alpha 的颜色(源 over)。
    ///
    /// cairo 的 `set_source_rgba` 在同一 surface 的多次绘制间不累积 alpha,
    /// 隧道里"整页渐隐/渐实"只能在像素上做, 故这里自己实现合成。
    fn blend_px(&mut self, x: i32, y: i32, rgb: (u8, u8, u8), alpha: f64) {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return;
        }
        let a = alpha.clamp(0.0, 1.0);
        if a <= 0.0 {
            return;
        }
        let (r, g, b) = (
            rgb.0 as f64 / 255.0,
            rgb.1 as f64 / 255.0,
            rgb.2 as f64 / 255.0,
        );
        let off = y as usize * self.stride + x as usize * 4;
        let px = &mut self.data[off..off + 4];
        // 缓冲为 BGRA 预乘
        let (db, dg, dr, da) = (
            px[0] as f64 / 255.0,
            px[1] as f64 / 255.0,
            px[2] as f64 / 255.0,
            px[3] as f64 / 255.0,
        );
        let out_a = a + da * (1.0 - a);
        if out_a <= 0.0 {
            px.copy_from_slice(&[0, 0, 0, 0]);
            return;
        }
        let (or, og, ob) = (
            (r * a + dr * (1.0 - a)) / out_a,
            (g * a + dg * (1.0 - a)) / out_a,
            (b * a + db * (1.0 - a)) / out_a,
        );
        px[0] = (ob.clamp(0.0, 1.0) * 255.0).round() as u8;
        px[1] = (og.clamp(0.0, 1.0) * 255.0).round() as u8;
        px[2] = (or.clamp(0.0, 1.0) * 255.0).round() as u8;
        px[3] = (out_a.clamp(0.0, 1.0) * 255.0).round() as u8;
    }

    /// 实心圆(alpha 合成, 面积覆盖法做抗锯齿)。
    fn fill_disc(&mut self, cx: f64, cy: f64, radius: f64, rgb: (u8, u8, u8), alpha: f64) {
        let r = radius.max(0.5);
        let x0 = (cx - r - 1.0).floor() as i32;
        let x1 = (cx + r + 1.0).ceil() as i32;
        let y0 = (cy - r - 1.0).floor() as i32;
        let y1 = (cy + r + 1.0).ceil() as i32;
        for y in y0..=y1 {
            for x in x0..=x1 {
                let dx = x as f64 + 0.5 - cx;
                let dy = y as f64 + 0.5 - cy;
                let d = (dx * dx + dy * dy).sqrt();
                let cover = (r - d + 0.5).clamp(0.0, 1.0);
                if cover > 0.0 {
                    self.blend_px(x, y, rgb, alpha * cover);
                }
            }
        }
    }

    /// 页外压暗(scale-to-fit 观看/快照卡片):页外区域叠一层黑,
    /// 页边界一圈细白线 —— 与画布上的 pageview_frame_draw 同一视觉。
    pub fn dim_outside(&mut self, x: f64, y: f64, w: f64, h: f64, dim: f64) {
        let (x1, y1) = (x + w, y + h);
        let x0i = x.floor() as i32 - 1;
        let x1i = x1.ceil() as i32 + 1;
        let y0i = y.floor() as i32 - 1;
        let y1i = y1.ceil() as i32 + 1;
        for py in y0i..=y1i {
            for px in x0i..=x1i {
                let cx = px as f64 + 0.5;
                let cy = py as f64 + 0.5;
                let inside = cx >= x && cx <= x1 && cy >= y && cy <= y1;
                if !inside {
                    self.blend_px(px, py, (0, 0, 0), dim);
                } else if (cx - x < 1.0) || (x1 - cx < 1.0) || (cy - y < 1.0) || (y1 - cy < 1.0) {
                    self.blend_px(px, py, (255, 255, 255), 0.55);
                }
            }
        }
    }

    /// 磨砂玻璃化:两遍可分离盒式模糊(半径 r, 近似高斯) + 与冷灰蓝底色
    /// 混合(不透明) + 按深度压暗。
    ///
    /// 隧道卡片要一眼看出"哪一页在前、哪一页在后":每张卡片都铺一层
    /// 不透明的磨砂玻璃(类似系统磨砂窗口), 页快照的墨迹浮在这层玻璃上 ——
    /// 越深的页玻璃越暗、模糊越重, 层次与透视关系就清楚了。
    ///
    /// 实现要点:
    /// - 盒式模糊**加权求和累积在 f32**, 不做逐次 u8 四舍五入, 避免重模糊发灰;
    /// - 透明像素按 premultiplied 处理(blur 完再归一), 边缘不产生"黑晕";
    /// - 一遍 O(N·r), 两遍 O(N·r) —— 3440×1440 r=8 约 2.4 亿次加法, 1~2ms。
    pub fn frost_and_tint(&mut self, radius: i32, depth: f64) {
        let (w, h) = (self.w, self.h);
        if w <= 0 || h <= 0 {
            return;
        }
        let r = radius.clamp(1, 32);
        let wu = w as usize;
        let hu = h as usize;
        let stride = self.stride;

        // 读出 BGRA → 4 个以 f32 累积的通道(仅有一个通道非零时可跳过)
        let mut ch: Vec<[f32; 4]> = vec![[0.0; 4]; wu * hu];
        for y in 0..hu {
            let row = y * stride;
            for x in 0..wu {
                let o = row + x * 4;
                let px = &self.data[o..o + 4];
                let a = px[3];
                let (ir, ig, ib) = if a > 0 {
                    // 反预乘: v = ink×a/255 → ink = v×255/a(读出阶段归一,
                    // 模糊后各通道始终是"墨水颜色"的面积平均, 不会因 alpha
                    // 稀释把白色描边洗成灰)。
                    (
                        px[2] as f32 * 255.0 / a as f32,
                        px[1] as f32 * 255.0 / a as f32,
                        px[0] as f32 * 255.0 / a as f32,
                    )
                } else {
                    (0.0, 0.0, 0.0)
                };
                ch[y * wu + x] = [ir, ig, ib, a as f32];
            }
        }

        // 可分离盒式模糊:横扫 + 竖扫, 各通道独立累加
        let mut tmp = vec![[0.0f32; 4]; wu * hu];
        let inv = 1.0 / (r as f32 * 2.0 + 1.0);
        for y in 0..hu {
            let base = y * wu;
            let mut acc = [0.0f32; 4];
            for k in -r..=r {
                let x = (k + r).min(w - 1) as usize;
                let v = ch[base + x];
                for c in 0..4 {
                    acc[c] += v[c];
                }
            }
            for x in 0..wu {
                let lo = (x as i32 - r).max(0) as usize;
                let hi = (x as i32 + r).min(w - 1) as usize;
                tmp[base + x] = [acc[0] * inv, acc[1] * inv, acc[2] * inv, acc[3] * inv];
                let vin = ch[base + hi];
                let vout = ch[base + lo];
                for c in 0..4 {
                    acc[c] += vin[c] - vout[c];
                }
            }
        }
        for x in 0..wu {
            let mut acc = [0.0f32; 4];
            for k in -r..=r {
                let y = (k + r).min(h - 1) as usize;
                let v = tmp[y * wu + x];
                for c in 0..4 {
                    acc[c] += v[c];
                }
            }
            for y in 0..hu {
                let lo = (y as i32 - r).max(0) as usize;
                let hi = (y as i32 + r).min(h - 1) as usize;
                ch[y * wu + x] = [acc[0] * inv, acc[1] * inv, acc[2] * inv, acc[3] * inv];
                let vin = tmp[hi * wu + x];
                let vout = tmp[lo * wu + x];
                for c in 0..4 {
                    acc[c] += vin[c] - vout[c];
                }
            }
        }

        // 磨砂玻璃底色(冷灰蓝)与深度压暗: 墨迹 = 归一后的 rgb, 玻璃 = tint
        let depth_d = depth.max(0.0);
        let shade = (1.0 - 0.16 * depth_d).clamp(0.45, 1.0);
        let (tr, tg, tb) = (216.0 * shade, 227.0 * shade, 240.0 * shade);
        // 玻璃底**低不透明度**(≈0.5, 随深度略增): 多片 OVER 叠加时
        // 1-(1-a1)(1-a2)… 自然累积, 中心几片一叠逐渐变实 ——
        // "几片透明玻璃叠起来"的观感靠这里, 单片绝不能近不透明。
        let ta = (150.0 - 20.0 * depth_d).clamp(90.0, 150.0);
        for y in 0..hu {
            let row = y * stride;
            for x in 0..wu {
                let v = ch[y * wu + x];
                let a = v[3];
                let o = row + x * 4;
                let out = &mut self.data[o..o + 4];
                if a <= 0.5 {
                    // 空玻璃
                    out[0] = (tb as f32).round().clamp(0.0, 255.0) as u8;
                    out[1] = (tg as f32).round().clamp(0.0, 255.0) as u8;
                    out[2] = (tr as f32).round().clamp(0.0, 255.0) as u8;
                    out[3] = ta.round().clamp(0.0, 255.0) as u8;
                } else {
                    // 读出阶段已反预乘, 模糊后 v 即墨水颜色
                    let ir = v[0].clamp(0.0, 255.0);
                    let ig = v[1].clamp(0.0, 255.0);
                    let ib = v[2].clamp(0.0, 255.0);
                    let k = a / 255.0; // 墨迹覆盖度(模糊后的边缘羽化)
                    let (trf, tgf, tbf) = (tr as f32, tg as f32, tb as f32);
                    out[0] = (ib + (tbf - ib) * (1.0 - k)).round().clamp(0.0, 255.0) as u8;
                    out[1] = (ig + (tgf - ig) * (1.0 - k)).round().clamp(0.0, 255.0) as u8;
                    out[2] = (ir + (trf - ir) * (1.0 - k)).round().clamp(0.0, 255.0) as u8;
                    out[3] = ta.round().clamp(0.0, 255.0) as u8;
                }
            }
        }
    }

    /// 圆头线段(alpha 合成, 距离场覆盖)。
    fn stroke_seg(
        &mut self,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        width: f64,
        rgb: (u8, u8, u8),
        alpha: f64,
    ) {
        let r = (width * 0.5).max(0.5);
        let minx = x0.min(x1) - r - 1.0;
        let maxx = x0.max(x1) + r + 1.0;
        let miny = y0.min(y1) - r - 1.0;
        let maxy = y0.max(y1) + r + 1.0;
        let (x0i, x1i) = (minx.floor() as i32, maxx.ceil() as i32);
        let (y0i, y1i) = (miny.floor() as i32, maxy.ceil() as i32);
        let dx = x1 - x0;
        let dy = y1 - y0;
        let len2 = dx * dx + dy * dy;
        for y in y0i..=y1i {
            for x in x0i..=x1i {
                let px = x as f64 + 0.5;
                let py = y as f64 + 0.5;
                let t = if len2 > 0.0 {
                    (((px - x0) * dx + (py - y0) * dy) / len2).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let qx = x0 + dx * t;
                let qy = y0 + dy * t;
                let d = ((px - qx).powi(2) + (py - qy).powi(2)).sqrt();
                let cover = (r - d + 0.5).clamp(0.0, 1.0);
                if cover > 0.0 {
                    self.blend_px(x, y, rgb, alpha * cover);
                }
            }
        }
    }
}

/// 一条笔迹的可渲染表示(与 [`crate::Stroke`] 同构, 便于测试构造)。
pub struct PageStroke<'a> {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    /// (x, y, width, t),坐标为页几何像素。
    pub points: &'a [(f64, f64, f64, f64)],
}

/// 渲染选项。
#[derive(Clone, Copy)]
pub struct RenderOpts {
    /// 整体不透明度(隧道的卡片渐显/渐隐用)。
    pub alpha: f64,
    /// 是否绘制白底(页快照/导出用;隧道卡片保持透明底)。
    pub white_bg: bool,
}

impl Default for RenderOpts {
    fn default() -> Self {
        Self {
            alpha: 1.0,
            white_bg: false,
        }
    }
}

/// 把一页的笔迹画进 `surf`(透明或白底, 见 [`RenderOpts`])。
///
/// `layout` 由 [`layout`] 算出;笔迹坐标是页几何像素, 渲染时统一走
/// `(p - pan) * zoom * layout.scale + layout.ox`(无限画布的 pan/zoom 也
/// 由此折叠, 翻页模式下恒为 pan=0、zoom=1)。
pub fn render_strokes(
    surf: &mut PageSurface<'_>,
    lay: &PageLayout,
    strokes: &[PageStroke<'_>],
    pan_x: f64,
    pan_y: f64,
    zoom: f64,
    opts: &RenderOpts,
) {
    let alpha = opts.alpha.clamp(0.0, 1.0);
    if opts.white_bg {
        for y in 0..surf.h.max(0) {
            for x in 0..surf.w.max(0) {
                surf.blend_px(x, y, (255, 255, 255), alpha);
            }
        }
    }
    if alpha <= 0.0 {
        return;
    }
    let map = |x: f64, y: f64| -> (f64, f64) {
        (
            (x - pan_x) * zoom * lay.scale + lay.ox,
            (y - pan_y) * zoom * lay.scale + lay.oy,
        )
    };
    for s in strokes {
        let pts = s.points;
        if pts.len() < 2 {
            continue;
        }
        let rgb = (
            (s.r.clamp(0.0, 1.0) * 255.0).round() as u8,
            (s.g.clamp(0.0, 1.0) * 255.0).round() as u8,
            (s.b.clamp(0.0, 1.0) * 255.0).round() as u8,
        );
        for (i, &(x, y, w, _t)) in pts.iter().enumerate() {
            let (sx, sy) = map(x, y);
            let sw = w * zoom * lay.scale;
            if i == 0 {
                surf.fill_disc(sx, sy, sw * 0.5, rgb, alpha);
            } else {
                let (px, py, _pw, _pt) = pts[i - 1];
                let (pxx, pyy) = map(px, py);
                surf.stroke_seg(pxx, pyy, sx, sy, sw, rgb, alpha);
            }
        }
    }
}

/// 按笔色亮度选对比描边色(BT.601, 阈值 128), 与画布渲染同参数。
fn outline_contrast_color(r: f64, g: f64, b: f64) -> (u8, u8, u8) {
    let lum = 0.299 * r + 0.587 * g + 0.114 * b;
    if lum > 0.5 {
        (0, 0, 0)
    } else {
        (255, 255, 255)
    }
}

/// 描边比笔迹宽出的半径(px), 与画布渲染同参数。
const OUTLINE_PAD: f64 = 1.0;

/// [`render_strokes`] 的描边层版本:同路径加宽 + 对比色垫在笔迹之下
/// (先画描边再画笔迹), 与玻璃上的观感一致。
pub fn render_strokes_with_outline(
    surf: &mut PageSurface<'_>,
    lay: &PageLayout,
    strokes: &[PageStroke<'_>],
    pan_x: f64,
    pan_y: f64,
    zoom: f64,
    opts: &RenderOpts,
    outline: bool,
) {
    if outline {
        for s in strokes {
            if s.points.len() < 2 {
                continue;
            }
            let ol = outline_contrast_color(s.r, s.g, s.b);
            let widened: Vec<(f64, f64, f64, f64)> = s
                .points
                .iter()
                .map(|&(x, y, w, t)| (x, y, w + OUTLINE_PAD * 2.0, t))
                .collect();
            let shim = PageStroke {
                r: ol.0 as f64 / 255.0,
                g: ol.1 as f64 / 255.0,
                b: ol.2 as f64 / 255.0,
                points: &widened,
            };
            render_strokes(surf, lay, &[shim], pan_x, pan_y, zoom, opts);
        }
    }
    render_strokes(surf, lay, strokes, pan_x, pan_y, zoom, opts);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn surface<'a>(data: &'a mut [u8], w: i32, h: i32) -> PageSurface<'a> {
        let stride = w as usize * 4;
        assert_eq!(data.len(), stride * h as usize);
        PageSurface { data, w, h, stride }
    }

    #[test]
    fn layout_same_geometry_is_identity() {
        let lay = layout(1920, 1080, 1920, 1080);
        assert_eq!(lay.scale, 1.0);
        assert_eq!((lay.ox, lay.oy), (0.0, 0.0));
        assert_eq!((lay.w, lay.h), (1920.0, 1080.0));
    }

    #[test]
    fn layout_fits_letterboxed_page_into_viewport() {
        // 3440×1440 的页放进 1920×1080 的视口: 宽向贴满、上下留白
        let lay = layout(3440, 1440, 1920, 1080);
        assert!((lay.scale - 1920.0 / 3440.0).abs() < 1e-9);
        assert_eq!(lay.w, 1920.0);
        assert!(lay.h < 1080.0);
        assert_eq!(lay.ox, 0.0);
        assert!(lay.oy > 0.0);
        // 渲染矩形不得越出缓冲
        assert!(lay.ox + lay.w <= 1920.0 + 1e-9);
        assert!(lay.oy + lay.h <= 1080.0 + 1e-9);
    }

    #[test]
    fn layout_is_centered() {
        let lay = layout(500, 500, 1000, 400);
        assert_eq!(lay.scale, 0.8);
        assert_eq!((lay.w, lay.h), (400.0, 400.0));
        assert_eq!(lay.ox, 300.0);
        assert_eq!(lay.oy, 0.0);
    }

    #[test]
    fn render_keeps_strokes_inside_capacity() {
        // 回归: 老实现把表面画成 2 倍大、笔迹写到分配内存之外(段错误)。
        let w = 64;
        let h = 48;
        let mut data = vec![0u8; w as usize * h as usize * 4];
        let lay = layout(1920, 1080, w, h);
        let stroke = PageStroke {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            points: &[
                (0.0, 0.0, 20.0, 0.0),
                (1919.0, 1079.0, 20.0, 0.1),
                (960.0, 540.0, 40.0, 0.2),
            ],
        };
        let mut surf = surface(&mut data, w, h);
        render_strokes(
            &mut surf,
            &lay,
            &[stroke],
            0.0,
            0.0,
            1.0,
            &RenderOpts::default(),
        );
        // 画到了缓冲里(角落与中心都有像素), 且没有越界写(跑不崩即证明)
        assert!(data.chunks(4).any(|px| px[2] > 0), "应有红色像素");
    }

    #[test]
    fn render_connects_samples_along_path() {
        // 回归: 整页重绘路径曾把线段起点 y 误写成当前点 y, 每段退化成
        // 水平短划, 保存后回看笔迹呈"散点"。两采样点连线的中点必须被覆盖。
        let w = 32;
        let h = 32;
        let mut data = vec![0u8; w as usize * h as usize * 4];
        let lay = layout(w, h, w, h);
        let stroke = PageStroke {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            points: &[(4.0, 4.0, 4.0, 0.0), (26.0, 26.0, 4.0, 0.1)],
        };
        let mut surf = surface(&mut data, w, h);
        render_strokes(
            &mut surf,
            &lay,
            &[stroke],
            0.0,
            0.0,
            1.0,
            &RenderOpts::default(),
        );
        let red_at = |x: usize, y: usize| {
            let i = (y * w as usize + x) * 4;
            data[i + 2] > 0
        };
        // (4,4)→(26,26) 的中点 (15,15): 水平短划 bug 下此处必为空白
        let mid_covered = (14..=16).any(|yy| (14..=16).any(|xx| red_at(xx, yy)));
        assert!(mid_covered, "采样点之间的线段中点应被着色");
    }

    #[test]
    fn render_alpha_fades_everything() {
        let w = 32;
        let h = 32;
        let mut data = vec![0u8; w as usize * h as usize * 4];
        let lay = layout(32, 32, w, h);
        let stroke = PageStroke {
            r: 0.0,
            g: 0.0,
            b: 1.0,
            points: &[(4.0, 16.0, 8.0, 0.0), (28.0, 16.0, 8.0, 0.1)],
        };
        let mut surf = surface(&mut data, w, h);
        render_strokes(
            &mut surf,
            &lay,
            &[stroke],
            0.0,
            0.0,
            1.0,
            &RenderOpts {
                alpha: 0.25,
                white_bg: false,
            },
        );
        // 与不透明渲染(α=1)对比: 每个像素都只能更淡或相同,
        // 且最深处确实被压到 1/4 附近(重叠处会略高)。
        let mut full = vec![0u8; w as usize * h as usize * 4];
        let mut surf2 = surface(&mut full, w, h);
        let stroke2 = PageStroke {
            r: 0.0,
            g: 0.0,
            b: 1.0,
            points: &[(4.0, 16.0, 8.0, 0.0), (28.0, 16.0, 8.0, 0.1)],
        };
        render_strokes(
            &mut surf2,
            &lay,
            &[stroke2],
            0.0,
            0.0,
            1.0,
            &RenderOpts::default(),
        );
        let max_a = data.chunks(4).map(|px| px[3]).max().unwrap();
        let full_a = full.chunks(4).map(|px| px[3]).max().unwrap();
        assert!(max_a > 0, "应有像素");
        assert!(max_a <= 120, "整体不透明度应按 alpha 衰减, got {max_a}");
        assert!(max_a < full_a, "α=0.25 必须比 α=1 淡({max_a} vs {full_a})");
        for (faded, opaque) in data.chunks(4).zip(full.chunks(4)) {
            assert!(faded[3] <= opaque[3], "逐像素不得比不透明渲染更浓");
        }
    }

    #[test]
    fn render_white_bg_covers_whole_surface() {
        let w = 8;
        let h = 8;
        let mut data = vec![0u8; w as usize * h as usize * 4];
        let lay = layout(8, 8, w, h);
        let mut surf = surface(&mut data, w, h);
        render_strokes(
            &mut surf,
            &lay,
            &[],
            0.0,
            0.0,
            1.0,
            &RenderOpts {
                alpha: 1.0,
                white_bg: true,
            },
        );
        assert!(data.chunks(4).all(|px| px == [255, 255, 255, 255]));
    }

    #[test]
    fn frost_tints_glass_and_keeps_ink() {
        // 空像素 → 冷灰蓝玻璃底; 墨迹 → 仍可见的前景
        let w = 24;
        let h = 24;
        let mut data = vec![0u8; w as usize * h as usize * 4];
        let lay = layout(w, h, w, h);
        // 粗一点的墨迹:磨砂(r=2)才不会把细节抹掉
        let stroke = PageStroke {
            r: 0.0,
            g: 0.0,
            b: 1.0,
            points: &[(4.0, 12.0, 16.0, 0.0), (20.0, 12.0, 16.0, 0.1)],
        };
        let mut r_only = vec![0u8; w as usize * h as usize * 4];
        {
            let mut surf = surface(&mut r_only, w, h);
            render_strokes(
                &mut surf,
                &lay,
                &[stroke],
                0.0,
                0.0,
                1.0,
                &RenderOpts::default(),
            );
        }
        {
            let surf = surface(&mut data, w, h);
            surf.data.copy_from_slice(&r_only);
        }
        {
            let mut surf = surface(&mut data, w, h);
            surf.frost_and_tint(2, 1.0);
        }
        // 空玻璃: BGRA = 冷灰蓝 + 不透明
        let corner = &data[0..4];
        assert_eq!(corner[3], 130, "磨砂玻璃应为半透明(alpha≈0.51)");
        assert!(corner[0] > corner[2], "玻璃应偏冷蓝(B > R)");
        // 墨迹: 蓝墨水应让 B 相对玻璃抬升、R 压低(没被磨砂磨掉)
        let mid = (12 * w as usize + 12) * 4;
        let ink = &data[mid..mid + 4];
        assert!(
            ink[0] as i32 > corner[0] as i32 + 10,
            "蓝墨迹应抬高 B, got {} vs 玻璃 {}",
            ink[0],
            corner[0]
        );
        assert!(
            ink[2] < corner[2],
            "蓝墨迹应压低 R, got {} vs 玻璃 {}",
            ink[2],
            corner[2]
        );
    }

    #[test]
    fn frost_depth_darkens_glass() {
        let w = 8;
        let h = 8;
        let mut near = vec![0u8; w as usize * h as usize * 4];
        let mut far = vec![0u8; w as usize * h as usize * 4];
        {
            let mut s1 = surface(&mut near, w, h);
            s1.frost_and_tint(1, 0.0);
        }
        {
            let mut s2 = surface(&mut far, w, h);
            s2.frost_and_tint(1, 3.0);
        }
        assert!(
            far[0] < near[0],
            "深处的玻璃应更暗(深度压暗), {} vs {}",
            far[0],
            near[0]
        );
    }

    #[test]
    fn render_respects_view_transform() {
        // 无限画布: pan/zoom 折叠进同一公式, 页内容不应跑出缓冲
        let w = 40;
        let h = 40;
        let mut data = vec![0u8; w as usize * h as usize * 4];
        let lay = layout(400, 400, w, h);
        let stroke = PageStroke {
            r: 0.0,
            g: 1.0,
            b: 0.0,
            points: &[(0.0, 0.0, 10.0, 0.0), (400.0, 400.0, 10.0, 0.1)],
        };
        let mut surf = surface(&mut data, w, h);
        render_strokes(
            &mut surf,
            &lay,
            &[stroke],
            100.0,
            100.0,
            0.5,
            &RenderOpts::default(),
        );
        assert!(data.chunks(4).any(|px| px[1] > 0), "应有绿色像素");
    }
}
