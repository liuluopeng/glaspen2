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

    /// 黑白相间虚线段(marching ants 描边, macOS 同款): 黑偶相位白奇相位
    /// 各描一遍, 平头(圆帽会把相邻 1px 黑白段互相吞掉);
    /// offset = 段起点处整笔累计弧长(cairo 每次 stroke 重置相位, 逐段拨)。
    fn stroke_outline_seg(
        &mut self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        width: f64,
        offset: f64,
    ) -> RECT {
        let half = (width * 0.5) as f32;
        let rect = RECT {
            left: (x0.min(x1) - half - 1.0).max(0.0) as i32,
            top: (y0.min(y1) - half - 1.0).max(0.0) as i32,
            right: (x0.max(x1) + half + 2.0).min(self.w as f32) as i32,
            bottom: (y0.max(y1) + half + 2.0).min(self.h as f32) as i32,
        };
        if let Some(c) = &self.cairo {
            const DASH: f64 = 1.0;
            c.set_line_cap_butt();
            c.set_dash(DASH, offset.rem_euclid(2.0 * DASH));
            c.stroke_line(x0, y0, x1, y1, width as f32, (0, 0, 0));
            c.set_dash(DASH, (offset + DASH).rem_euclid(2.0 * DASH));
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

    /// 填充实心矩形(彩虹指示器)
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

    /// 绘制网格线(与 macOS 一致:40px 间距、50% 灰半透明、画在笔画下方)。
    /// macOS 为 colorWithWhite:0.5 alpha:0.15 → premultiplied 像素
    /// RGB = 128*0.15 ≈ 19,alpha = 38。
    /// 网格画在画布坐标系:无限画布下跟随镜头平移/缩放,并在「屏幕尺寸」
    /// 整数倍处加粗(每屏一条参考线);翻页模式 pan=0、zoom=1,与从前一致。
    /// `divider`:分栏参考线(macOS 同款)——0=无 1=左右两栏 2=上下两栏
    /// 3=九宫格;切分点吸附到最近的网格线,线宽 1.5px(主列全 alpha + 邻列半 alpha)。
    fn draw_grid(&mut self, divider: i32) {
        // 网格尺寸:面板「网格大小」20/40/80,10..200 钳制(macOS 同键)
        let GAP: f64 = grid_size();
        const GA: u8 = 38; // 0.15 * 255
        const GR: u8 = 19; // 0.5 * 0.15 * 255 (50% 灰,premultiplied)
        // 加粗参考线(仅无限画布):colorWithWhite:0.5 alpha:0.55
        const BOLD_GA: u8 = 140; // 0.55 * 255
        const BOLD_GR: u8 = 70; // 0.5 * 0.55 * 255
        // 分栏参考线:colorWithWhite:0.5 alpha:0.65,宽 1.5px
        const DIV_GA: u8 = 166; // 0.65 * 255
        const DIV_GR: u8 = 83; // 0.5 * 0.65 * 255
        const DIV_HALF_GA: u8 = 83; // 邻列 ~半覆盖
        const DIV_HALF_GR: u8 = 41;

        let infinite = infinite_on();
        let (pan_x, pan_y, zoom) = if infinite { cam() } else { (0.0, 0.0, 1.0) };
        let w = self.w;
        let h = self.h;
        // 可见画布范围(view = (canvas − pan) × zoom)
        let cx0 = pan_x;
        let cx1 = pan_x + w as f64 / zoom;
        let cy0 = pan_y;
        let cy1 = pan_y + h as f64 / zoom;

        unsafe {
            let bits = self.bits;

            // 细网格:每 GAP 一格,统一淡细线
            let kx0 = (cx0 / GAP).floor() as i64 - 1;
            let kx1 = (cx1 / GAP).floor() as i64 + 1;
            let mut k = kx0;
            while k <= kx1 {
                let gx = ((k as f64 * GAP - pan_x) * zoom).round() as i32;
                if gx >= 0 && gx < w {
                    for y in 0..h {
                        let i = ((y as usize) * (w as usize) + gx as usize) * 4;
                        *bits.add(i) = GR; // B
                        *bits.add(i + 1) = GR; // G
                        *bits.add(i + 2) = GR; // R
                        *bits.add(i + 3) = GA; // A
                    }
                }
                k += 1;
            }
            let ky0 = (cy0 / GAP).floor() as i64 - 1;
            let ky1 = (cy1 / GAP).floor() as i64 + 1;
            let mut k = ky0;
            while k <= ky1 {
                let gy = ((k as f64 * GAP - pan_y) * zoom).round() as i32;
                if gy >= 0 && gy < h {
                    for x in 0..w {
                        let i = ((gy as usize) * (w as usize) + x as usize) * 4;
                        *bits.add(i) = GR;
                        *bits.add(i + 1) = GR;
                        *bits.add(i + 2) = GR;
                        *bits.add(i + 3) = GA;
                    }
                }
                k += 1;
            }

            // 分界线:只在「屏幕尺寸」整数倍处加深加粗(无限画布 = 每屏一条参考线)
            if infinite {
                let bw = w as f64;
                let bh = h as f64;
                let mut k = ((cx0 / bw).floor() as i64) - 1;
                while k <= ((cx1 / bw).floor() as i64) + 1 {
                    let gx = ((k as f64 * bw - pan_x) * zoom).round() as i32;
                    if gx >= 0 && gx < w {
                        for y in 0..h {
                            let i = ((y as usize) * (w as usize) + gx as usize) * 4;
                            *bits.add(i) = BOLD_GR;
                            *bits.add(i + 1) = BOLD_GR;
                            *bits.add(i + 2) = BOLD_GR;
                            *bits.add(i + 3) = BOLD_GA;
                        }
                    }
                    k += 1;
                }
                let mut k = ((cy0 / bh).floor() as i64) - 1;
                while k <= ((cy1 / bh).floor() as i64) + 1 {
                    let gy = ((k as f64 * bh - pan_y) * zoom).round() as i32;
                    if gy >= 0 && gy < h {
                        for x in 0..w {
                            let i = ((gy as usize) * (w as usize) + x as usize) * 4;
                            *bits.add(i) = BOLD_GR;
                            *bits.add(i + 1) = BOLD_GR;
                            *bits.add(i + 2) = BOLD_GR;
                            *bits.add(i + 3) = BOLD_GA;
                        }
                    }
                    k += 1;
                }
            }

            // 分栏参考线(纯视觉,macOS 同款):左右两栏/上下两栏 = 每屏单位
            // 1/2 处,九宫格 = 1/3、2/3 处。切分点吸附到最近的网格线(保证
            // 加粗的永远是真实网格线),每屏单位各自吸附;两种画布模式都画。
            if divider > 0 {
                let halves: [f64; 1] = [0.5];
                let thirds: [f64; 2] = [1.0 / 3.0, 2.0 / 3.0];
                let (fx, fy): (&[f64], &[f64]) = match divider {
                    1 => (&halves[..], &[][..]),
                    2 => (&[][..], &halves[..]),
                    _ => (&thirds[..], &thirds[..]),
                };
                let bw = w as f64;
                let bh = h as f64;
                let draw_col = |bits: *mut u8, gx: i32, main_ga: u8, main_gr: u8, half_ga: u8, half_gr: u8| {
                    if gx >= 0 && gx < w {
                        for y in 0..h {
                            let i = ((y as usize) * (w as usize) + gx as usize) * 4;
                            *bits.add(i) = main_gr;
                            *bits.add(i + 1) = main_gr;
                            *bits.add(i + 2) = main_gr;
                            *bits.add(i + 3) = main_ga;
                        }
                    }
                    // 1.5px 线宽:邻列 ~半覆盖
                    if gx + 1 >= 0 && gx + 1 < w {
                        for y in 0..h {
                            let i = ((y as usize) * (w as usize) + (gx + 1) as usize) * 4;
                            *bits.add(i) = half_gr;
                            *bits.add(i + 1) = half_gr;
                            *bits.add(i + 2) = half_gr;
                            *bits.add(i + 3) = half_ga;
                        }
                    }
                };
                let draw_row = |bits: *mut u8, gy: i32, main_ga: u8, main_gr: u8, half_ga: u8, half_gr: u8| {
                    if gy >= 0 && gy < h {
                        for x in 0..w {
                            let i = ((gy as usize) * (w as usize) + x as usize) * 4;
                            *bits.add(i) = main_gr;
                            *bits.add(i + 1) = main_gr;
                            *bits.add(i + 2) = main_gr;
                            *bits.add(i + 3) = main_ga;
                        }
                    }
                    if gy + 1 >= 0 && gy + 1 < h {
                        for x in 0..w {
                            let i = (((gy + 1) as usize) * (w as usize) + x as usize) * 4;
                            *bits.add(i) = half_gr;
                            *bits.add(i + 1) = half_gr;
                            *bits.add(i + 2) = half_gr;
                            *bits.add(i + 3) = half_ga;
                        }
                    }
                };
                for frac in fx {
                    let mut i = ((cx0 / bw).floor() as i64) - 1;
                    while i <= ((cx1 / bw).floor() as i64) + 1 {
                        // 吸附到最近的网格线(macOS lround 同款)
                        let k = ((i as f64 * bw + bw * frac) / GAP).round() as i64;
                        let gx = ((k as f64 * GAP - pan_x) * zoom).round() as i32;
                        draw_col(bits, gx, DIV_GA, DIV_GR, DIV_HALF_GA, DIV_HALF_GR);
                        i += 1;
                    }
                }
                for frac in fy {
                    let mut i = ((cy0 / bh).floor() as i64) - 1;
                    while i <= ((cy1 / bh).floor() as i64) + 1 {
                        let k = ((i as f64 * bh + bh * frac) / GAP).round() as i64;
                        let gy = ((k as f64 * GAP - pan_y) * zoom).round() as i32;
                        draw_row(bits, gy, DIV_GA, DIV_GR, DIV_HALF_GA, DIV_HALF_GR);
                        i += 1;
                    }
                }
            }
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

