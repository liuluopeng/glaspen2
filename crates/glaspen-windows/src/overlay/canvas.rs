use glaspen_core::draw_spec::{Dash, DrawCommand, DrawPlan, PaintPass};

use crate::paint_plan;

static STATE: AtomicPtr<OverlayState> = AtomicPtr::new(std::ptr::null_mut());

// ── 全屏透明 overlay 画布(UpdateLayeredWindowIndirect + 32bit BGRA DIB) ──
// cairo 渲染统一走 glaspen_core::cairo_dl(动态加载 libcairo-2.dll,直接画到 DIB 内存)

struct OverlayCanvas {
    hwnd: HWND,
    dib_dc: HDC,
    dib: HBITMAP,
    old: HGDIOBJ,
    bits: *mut u8,
    screen_dc: HDC,
    w: i32,
    h: i32,
    pos: POINT,
    cairo: Option<CairoRenderer>,
    /// 笔迹颜色 (R, G, B)
    color: (u8, u8, u8),
}

/// 网格一层线段 → DIB:轴对齐线段写 1px 列/行(旧 draw_grid 的直写样式)。
/// 越界线段跳过(旧代码同款 `gx >= 0 && gx < w`)。几何来自 core::grid。
unsafe fn grid_blit_layer(
    bits: *mut u8,
    w: i32,
    h: i32,
    segments: &[Segment],
    gr: u8,
    ga: u8,
) {
    for seg in segments {
        if seg.is_vertical() {
            grid_blit_column(bits, w, h, seg.x0.round() as i32, gr, ga);
        } else if seg.is_horizontal() {
            grid_blit_row(bits, w, h, seg.y0.round() as i32, gr, ga);
        }
    }
}

/// 分栏参考线:主列全 alpha + 邻列半 alpha(旧 draw_col/draw_row 同款 1.5px 覆盖)。
unsafe fn grid_blit_divider(
    bits: *mut u8,
    w: i32,
    h: i32,
    segments: &[Segment],
    main_gr: u8,
    main_ga: u8,
    half_gr: u8,
    half_ga: u8,
) {
    for seg in segments {
        if seg.is_vertical() {
            let gx = seg.x0.round() as i32;
            grid_blit_column(bits, w, h, gx, main_gr, main_ga);
            grid_blit_column(bits, w, h, gx + 1, half_gr, half_ga);
        } else if seg.is_horizontal() {
            let gy = seg.y0.round() as i32;
            grid_blit_row(bits, w, h, gy, main_gr, main_ga);
            grid_blit_row(bits, w, h, gy + 1, half_gr, half_ga);
        }
    }
}

/// 一列像素填色(越界整体跳过;直线坐标已由 core 给出、这里只取整)。
unsafe fn grid_blit_column(bits: *mut u8, w: i32, h: i32, gx: i32, gr: u8, ga: u8) {
    if !(0..w).contains(&gx) {
        return;
    }
    for y in 0..h {
        let i = ((y as usize) * (w as usize) + gx as usize) * 4;
        *bits.add(i) = gr;
        *bits.add(i + 1) = gr;
        *bits.add(i + 2) = gr;
        *bits.add(i + 3) = ga;
    }
}

/// 一行像素填色。
unsafe fn grid_blit_row(bits: *mut u8, w: i32, h: i32, gy: i32, gr: u8, ga: u8) {
    if !(0..h).contains(&gy) {
        return;
    }
    for x in 0..w {
        let i = ((gy as usize) * (w as usize) + x as usize) * 4;
        *bits.add(i) = gr;
        *bits.add(i + 1) = gr;
        *bits.add(i + 2) = gr;
        *bits.add(i + 3) = ga;
    }
}

impl OverlayCanvas {
    fn create(hwnd: HWND) -> Self {
        unsafe {
            let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
            let w = GetSystemMetrics(SM_CXVIRTUALSCREEN);
            let h = GetSystemMetrics(SM_CYVIRTUALSCREEN);
            let wdc = GetDC(Some(hwnd));
            let dib_dc = CreateCompatibleDC(Some(wdc));
            let _ = ReleaseDC(Some(hwnd), wdc);

            let mut bmi = BITMAPINFO::default();
            bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = w.max(1);
            bmi.bmiHeader.biHeight = -h.max(1); // top-down
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = BI_RGB.0;

            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let dib = CreateDIBSection(Some(dib_dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0)
                .expect("CreateDIBSection failed");
            let old = SelectObject(dib_dc, dib.into());

            // 初始全透明(alpha=0)
            let n = (w.max(1) as usize) * (h.max(1) as usize) * 4;
            std::slice::from_raw_parts_mut(bits as *mut u8, n).fill(0);

            let screen_dc = GetDC(None);
            let pos = POINT { x, y };
            // 加载 cairo(画到同一像素缓冲),失败则回退自绘
            let cairo = CairoRenderer::load(bits as *mut u8, w.max(1), h.max(1));
            Self {
                hwnd,
                dib_dc,
                dib,
                old,
                bits: bits as *mut u8,
                screen_dc,
                w: w.max(1),
                h: h.max(1),
                pos,
                cairo,
                color: (0, 0, 0),
            }
        }
    }

    /// 软阴影 pass: 整条折线加宽 spread×2 的半透明黑(cairo alpha 合成;
    /// 无 cairo 的回退自绘不支持半透明, 直接跳过)。返回脏矩形。
    fn stroke_shadow_pass(
        &mut self,
        path: &[(f32, f32, f32)],
        spread: f64,
        alpha: f64,
    ) -> Option<RECT> {
        let c = self.cairo.as_ref()?;
        let mut half = 0.0f32;
        for &(x, y, r) in path {
            let _ = (x, y);
            half = half.max(r);
        }
        half += spread as f32 + 1.0;
        let (mut l, mut t, mut rr, mut b) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for &(x, y, _) in path {
            l = l.min(x - half);
            t = t.min(y - half);
            rr = rr.max(x + half);
            b = b.max(y + half);
        }
        for w in path.windows(2) {
            let width = (w[0].2.max(w[1].2)) * 2.0 + (spread * 2.0) as f32;
            c.stroke_line_alpha(w[0].0, w[0].1, w[1].0, w[1].1, width, (0, 0, 0), alpha);
        }
        Some(RECT {
            left: (l.max(0.0)) as i32,
            top: (t.max(0.0)) as i32,
            right: (rr.min(self.w as f32)) as i32,
            bottom: (b.min(self.h as f32)) as i32,
        })
    }

    /// 黑白相间虚线段(marching ants 描边, macOS 同款): 黑偶相位白奇相位
    /// 各描一遍, 平头(圆帽会把相邻 1px 黑白段互相吞掉);
    /// offset = 段起点处整笔累计弧长(cairo 每次 stroke 重置相位, 逐段拨)。
    /// `dash_len` 来自 core DrawPlan 的 [Dash](glaspen_core::draw_spec::Dash)
    /// (Windows 恒 1.0 视图像素), 本函数只做 cairo 调用。
    fn stroke_outline_seg(
        &mut self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        width: f64,
        dash: Dash,
    ) -> RECT {
        let Dash {
            len: dash_len,
            phase: offset,
        } = dash;
        let half = (width * 0.5) as f32;
        let rect = RECT {
            left: (x0.min(x1) - half - 1.0).max(0.0) as i32,
            top: (y0.min(y1) - half - 1.0).max(0.0) as i32,
            right: (x0.max(x1) + half + 2.0).min(self.w as f32) as i32,
            bottom: (y0.max(y1) + half + 2.0).min(self.h as f32) as i32,
        };
        if let Some(c) = &self.cairo {
            c.set_line_cap_butt();
            c.set_dash(dash_len, offset.rem_euclid(2.0 * dash_len));
            c.stroke_line(x0, y0, x1, y1, width as f32, (0, 0, 0));
            c.set_dash(dash_len, (offset + dash_len).rem_euclid(2.0 * dash_len));
            c.stroke_line(x0, y0, x1, y1, width as f32, (255, 255, 255));
            c.set_dash(0.0, 0.0); // 墨迹本体绝不能被虚线化
            c.set_line_cap_round();
        } else {
            // 无 cairo 回退: 实心单段(近似)
            let _ = self.draw_soft_line(x0, y0, x1, y1, half);
        }
        rect
    }

    /// 填充闭合轮廓多边形(cairo 抗锯齿,可变宽度笔迹),返回脏矩形
    fn fill_outline(&mut self, outline: &[(f32, f32)]) -> RECT {
        let mut left = f32::MAX;
        let mut top = f32::MAX;
        let mut right = f32::MIN;
        let mut bottom = f32::MIN;
        for p in outline {
            left = left.min(p.0);
            top = top.min(p.1);
            right = right.max(p.0);
            bottom = bottom.max(p.1);
        }
        let rect = RECT {
            left: (left as i32 - 1).max(0),
            top: (top as i32 - 1).max(0),
            right: (right as i32 + 2).min(self.w),
            bottom: (bottom as i32 + 2).min(self.h),
        };
        if let Some(c) = &self.cairo {
            c.fill_outline(outline, self.color);
            c.flush();
            return rect;
        }
        // fallback:轮廓边逐段软线(近似)
        if outline.len() >= 2 {
            for w in outline.windows(2) {
                let _ = self.draw_soft_line(w[0].0, w[0].1, w[1].0, w[1].1, 0.5);
            }
        }
        rect
    }

    /// 填充实心圆点(笔迹端点圆帽),返回脏矩形
    fn fill_dot(&mut self, cx: f32, cy: f32, r: f32) -> RECT {
        let dirty = RECT {
            left: (cx as i32 - r.ceil() as i32 - 1).max(0),
            top: (cy as i32 - r.ceil() as i32 - 1).max(0),
            right: (cx as i32 + r.ceil() as i32 + 2).min(self.w),
            bottom: (cy as i32 + r.ceil() as i32 + 2).min(self.h),
        };
        if let Some(c) = &self.cairo {
            c.fill_circle(cx, cy, r, self.color);
            c.flush();
            return dirty;
        }
        let _ = self.draw_soft_line(cx, cy, cx, cy, r.max(0.5));
        dirty
    }

    /// 执行 core DrawPlan —— **pass 顺序/宽度/透明度/颜色的唯一来源**。
    ///
    /// 本函数不再决定「三档阴影 → 描边 → 墨迹」的顺序与宽度(D9c:计划由
    /// core `draw_spec::plan` 产出,Windows 参数见 crate::paint_plan),只把
    /// 每条命令翻译成本文件的画法原语。Windows 光栅化差异保留在执行层:
    /// - 差异 D 描边取相邻两点较粗者(paint_plan::outline_segment_width);
    /// - 差异 F 墨迹仍是 build_outline 法线轮廓 + 一次填充 + 端点圆点;
    /// - 差异 K 半径下限 0.5px(paint_plan::command_path);
    /// - 差异 O 无 cairo 回退仍由各原语内部近似(plan 不驱动回退路径)。
    fn execute_plan(&mut self, plan: &DrawPlan) {
        for layer in &plan.layers {
            // 旧行为:每笔开始处把 canvas.color 设为该笔颜色(无 cairo 回退
            // 路径的软线用它)。颜色来源 = 计划里该笔的 Fill 命令。
            if let Some(fill) = layer.iter().find(|c| c.pass == PaintPass::Fill) {
                self.color = paint_plan::color_u8(fill.color);
            }
            for cmd in layer {
                self.execute_command(plan, cmd);
            }
        }
    }

    /// 执行一条计划命令:点列/宽度/透明度/虚线全部来自计划。
    fn execute_command(&mut self, plan: &DrawPlan, cmd: &DrawCommand) {
        let path = paint_plan::command_path(plan, cmd);
        match cmd.pass {
            // 阴影层:三档 spread/alpha 由计划的 3 条 Shadow 命令给出
            // (差异 B:Windows spread 固定视图像素,不随 zoom 缩放)。
            PaintPass::Shadow => {
                // < 2 点在旧 fill_stroke_path 里整笔早退(实时首点圆点由 Fill 画)。
                if path.len() >= 2 {
                    let spread = paint_plan::shadow_spread(cmd);
                    let _ = self.stroke_shadow_pass(&path, spread, cmd.alpha);
                }
            }
            // 描边层:相位沿整笔累计弧长连续(cairo 每次 stroke 重置相位)。
            PaintPass::Outline => {
                if path.len() < 2 {
                    return;
                }
                let dash = paint_plan::dash(cmd);
                let mut cum = dash.phase;
                for i in 1..path.len() {
                    let Some(width) = paint_plan::outline_segment_width(plan, cmd, i) else {
                        break;
                    };
                    let (x0, y0, _) = path[i - 1];
                    let (x1, y1, _) = path[i];
                    let seg_dash = Dash {
                        len: dash.len,
                        phase: cum,
                    };
                    let _ = self.stroke_outline_seg(x0, y0, x1, y1, width, seg_dash);
                    let (dx, dy) = (x1 - x0, y1 - y0);
                    cum += ((dx * dx + dy * dy) as f64).sqrt();
                }
            }
            // 墨迹本体:多边形一次填充 + 首尾圆点(差异 F,Windows 光栅化算法)。
            // 1 点路径(实时首份报告)只画圆点 = 旧实现的初始点 fill_dot。
            PaintPass::Fill => {
                self.color = paint_plan::color_u8(cmd.color);
                let outline = build_outline(&path);
                if outline.len() >= 3 {
                    let _ = self.fill_outline(&outline);
                }
                if let Some(&(cx, cy, r)) = path.first() {
                    let _ = self.fill_dot(cx, cy, r);
                }
                if let Some(&(cx, cy, r)) = path.last() {
                    let _ = self.fill_dot(cx, cy, r);
                }
            }
            // Windows 全仓无橡皮输入路径(审计差异 H);core 的 Erase 只服务
            // macOS 实时 CLEAR,本 adapter 不会收到。
            PaintPass::Erase => {}
        }
    }

    /// 填充实心矩形
    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: (u8, u8, u8)) {
        if let Some(c) = &self.cairo {
            c.fill_rect(x, y, w, h, color);
            c.flush();
        } else {
            // fallback:四边软线近似
            let _ = self.draw_soft_line(x, y, x + w, y, h * 0.5);
            let _ = self.draw_soft_line(x, y, x, y + h, w * 0.5);
            let _ = self.draw_soft_line(x + w, y, x + w, y + h, w * 0.5);
            let _ = self.draw_soft_line(x, y + h, x + w, y + h, h * 0.5);
        }
    }

    /// 软边线段(抗锯齿):优先 cairo 渲染,回退自绘。返回脏矩形。
    fn draw_soft_line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> RECT {
        let pad = r.ceil() as i32 + 1;
        let left = (x0.min(x1) as i32 - pad).max(0);
        let top = (y0.min(y1) as i32 - pad).max(0);
        let right = (x0.max(x1) as i32 + pad + 1).min(self.w);
        let bottom = (y0.max(y1) as i32 + pad + 1).min(self.h);

        if let Some(c) = &self.cairo {
            c.fill_circle(x0, y0, r, self.color);
            return RECT {
                left,
                top,
                right,
                bottom,
            };
        }

        let dx = x1 - x0;
        let dy = y1 - y0;
        let l2 = dx * dx + dy * dy;
        let l2 = if l2 < 1e-6 { 1.0 } else { l2 };
        let inv_l2 = 1.0 / l2;
        let edge = r + 0.5; // 实心半径 + 0.5px 抗锯齿边

        unsafe {
            let bits = self.bits;
            let w = self.w;
            for py in top..bottom {
                for px in left..right {
                    let fx = px as f32;
                    let fy = py as f32;
                    let t = ((fx - x0) * dx + (fy - y0) * dy) * inv_l2;
                    let t = if t < 0.0 {
                        0.0
                    } else if t > 1.0 {
                        1.0
                    } else {
                        t
                    };
                    let nx = x0 + t * dx;
                    let ny = y0 + t * dy;
                    let d2 = (fx - nx) * (fx - nx) + (fy - ny) * (fy - ny);
                    let d = d2.sqrt();
                    let cov = edge - d;
                    if cov > 0.0 {
                        let a = if cov >= 1.0 { 255 } else { (cov * 255.0) as u8 };
                        let i = ((py as usize) * (w as usize) + px as usize) * 4;
                        let cur = *bits.add(i + 3);
                        if a > cur {
                            let (r, g, b) = self.color;
                            *bits.add(i) = b;
                            *bits.add(i + 1) = g;
                            *bits.add(i + 2) = r;
                            *bits.add(i + 3) = a;
                        }
                    }
                }
            }
        }

        RECT {
            left,
            top,
            right,
            bottom,
        }
    }

    /// 把脏矩形合成到屏幕(ULW)
    fn present_rect(&self, dirty: &RECT) {
        unsafe {
            let blend = BLENDFUNCTION {
                BlendOp: 0, // AC_SRC_OVER
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: 1, // AC_SRC_ALPHA
            };
            let size = SIZE {
                cx: self.w,
                cy: self.h,
            };
            let src = POINT { x: 0, y: 0 };
            let info = UPDATELAYEREDWINDOWINFO {
                cbSize: std::mem::size_of::<UPDATELAYEREDWINDOWINFO>() as u32,
                hdcDst: self.screen_dc,
                pptDst: &self.pos,
                psize: &size,
                hdcSrc: self.dib_dc,
                pptSrc: &src,
                crKey: COLORREF(0),
                pblend: &blend,
                dwFlags: ULW_ALPHA,
                prcDirty: dirty,
            };
            let _ = UpdateLayeredWindowIndirect(self.hwnd, &info);
        }
    }

    /// 全屏刷新(清屏后用)
    fn present_all(&self) {
        let dirty = RECT {
            left: 0,
            top: 0,
            right: self.w,
            bottom: self.h,
        };
        self.present_rect(&dirty);
    }

    fn clear(&mut self) {
        unsafe {
            let n = (self.w as usize) * (self.h as usize) * 4;
            std::slice::from_raw_parts_mut(self.bits, n).fill(0);
        }
        self.present_all();
    }

    /// 绘制网格线(几何唯一来源 = core `grid::compute»;本函数只做像素样式)。
    ///
    /// 与 macOS 同一张线段表:细网格/屏幕分界线各 1px 直写预乘 BGRA,分栏线主列 +
    /// 邻列半 alpha(约 2px)。颜色常量保持旧值:
    /// 细网格 colorWithWhite:0.5 alpha:0.15 → 预乘 RGB=19/alpha=38;
    /// 屏幕分界线 alpha:0.55 → 70/140(仅无限画布, macOS 两种画布模式都画);
    /// 分栏 alpha:0.65 → 83/166,邻列 ~半覆盖。两个开关互不依赖, 各画各层,
    /// 绘制顺序 = 细网格 → 屏幕分界线 → 分栏(后画覆盖先画, 旧实现同序)。
    /// 几何(间距/吸附/屏幕单位/pan/zoom 变换)全部在 core::grid, 这里不再算。
    fn draw_grid(&mut self, show_grid: bool, divider: i32) {
        const GA: u8 = 38; // 0.15 * 255
        const GR: u8 = 19; // 0.5 * 0.15 * 255 (50% 灰, premultiplied)
        // 加粗参考线(仅无限画布):colorWithWhite:0.5 alpha:0.55
        const BOLD_GA: u8 = 140; // 0.55 * 255
        const BOLD_GR: u8 = 70; // 0.5 * 0.55 * 255
        // 分栏参考线:colorWithWhite:0.5 alpha:0.65,宽 1.5px
        const DIV_GA: u8 = 166; // 0.65 * 255
        const DIV_GR: u8 = 83; // 0.5 * 0.65 * 255
        const DIV_HALF_GA: u8 = 83; // 邻列 ~半覆盖
        const DIV_HALF_GR: u8 = 41;

        let infinite = infinite_on();
        let w = self.w;
        let h = self.h;
        // 翻页模式镜头恒为原点 + 100%(macOS 同款);屏幕单位 = 画布尺寸
        // (macOS 用 g_screen_w/h, 窗口 = 屏幕, 口径一致)。屏幕分界线仅无限
        // 画布生成(两端真实差异 1, 见 core/grid.rs 模块文档)。
        let cam = if infinite { camera() } else { Camera::HOME };
        let lines = glaspen_core::grid::compute(&glaspen_core::grid::GridParams {
            view_w: w as f64,
            view_h: h as f64,
            camera: cam,
            grid_size: grid_size(),
            divider,
            show_grid,
            screen_w: w as f64,
            screen_h: h as f64,
            screen_lines: infinite,
            clip: None,
        });

        unsafe {
            let bits = self.bits;
            grid_blit_layer(bits, w, h, &lines.minor, GR, GA);
            grid_blit_layer(bits, w, h, &lines.screen, BOLD_GR, BOLD_GA);
            grid_blit_divider(
                bits,
                w,
                h,
                &lines.divider,
                DIV_GR,
                DIV_GA,
                DIV_HALF_GR,
                DIV_HALF_GA,
            );
        }
    }

    /// 设置背景像素 alpha(只改完全透明的像素,保留笔迹及其软边抗锯齿像素):
    ///  a=0  → 整窗真正透明,系统视为不可见,输入穿透到下层
    ///  a>=2 → 肉眼几乎不可见,但窗口可命中,拦截笔/鼠标输入
    fn set_bg_alpha(&mut self, a: u8) {
        unsafe {
            let n = (self.w as usize) * (self.h as usize);
            let p = self.bits;
            for i in 0..n {
                let off = i * 4;
                if *p.add(off + 3) == 0 {
                    *p.add(off + 3) = a;
                }
            }
        }
        self.present_all();
    }

    /// 读取整个画布的像素副本(用于保存导出;BGRA 预乘)
    fn snapshot(&self) -> Vec<u8> {
        unsafe {
            let n = (self.w as usize) * (self.h as usize) * 4;
            std::slice::from_raw_parts(self.bits, n).to_vec()
        }
    }
}

impl Drop for OverlayCanvas {
    fn drop(&mut self) {
        unsafe {
            let _ = SelectObject(self.dib_dc, self.old);
            let _ = DeleteObject(self.dib.into());
            let _ = DeleteDC(self.dib_dc);
            let _ = ReleaseDC(None, self.screen_dc);
        }
    }
}

// ── 绘制核心(源自已验证原型) ──

