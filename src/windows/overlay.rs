//! glaspen2 Windows overlay — 纯 Rust 实现(无 C#)。
//!
//! 架构(基于已验证原型 raw_input_trans_draw.rs):
//!   - 单层全屏 WS_EX_LAYERED 窗口 + UpdateLayeredWindowIndirect(32bit BGRA alpha) 合成
//!   - 输入:WM_INPUT(Raw Input)手工解析 HID 报告,任何窗口状态都能收到
//!   - 笔迹:ink-stroke-modeler(位置/压力双平滑 + 120Hz 重采样)→ 可变宽度轮廓
//!     (法线偏移 + cairo 抗锯齿填充 + 端点圆帽)
//!   - 拦截:笔事件到达清除 WS_EX_TRANSPARENT,笔离开 500ms 后恢复穿透
//!   - 集成 glaspen2 功能:Flutter 设置管道、撤销/清屏/保存导出、热键
//!
//! 依赖版本与 wAPItry 原型一致(windows 0.62、libloading 0.9)。

#![allow(unsafe_op_in_unsafe_fn)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::time::Instant;

use ink_stroke_modeler_rs::{ModelerInput, ModelerInputEventType, ModelerParams, StrokeModeler};
use windows::core::PCWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL,
};
use windows::Win32::UI::Input::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::cairo_dl::CairoRenderer;

const WM_INPUT: u32 = 0x00FF;
const TIMER_UNBLOCK: usize = 1;
const TIMER_PEEK: usize = 2;
// 笔事件停止后的延迟:恢复穿透 + 飘渺模式隐藏笔迹
const UNBLOCK_DELAY_MS: u32 = 50;
// 飘渺模式下翻页后短暂显示笔迹的时长(ms)
const PEEK_DELAY_MS: u32 = 2000;
// 近透明底色 alpha=2(0.01*255):肉眼不可见,但窗口可命中/可收指针消息
const BG_BLOCK: u8 = 2;

// XP-Pen 板面逻辑范围(Generic X/Y logical max)
const MAX_RAW_X: i64 = 25400;
const MAX_RAW_Y: i64 = 15875;

// ── 自定义消息与命令 ID(Flutter 设置管道用) ──

pub const WM_TRAY_COMMAND: u32 = WM_USER + 1;

pub const CMD_SELECT_COLOR: usize = 100;
pub const CMD_SELECT_WIDTH: usize = 200;
pub const CMD_SAVE_WITH_BG: usize = 300;
pub const CMD_SAVE_DRAWING: usize = 301;
pub const CMD_SAVE_XOJ: usize = 302;
pub const CMD_CLEAR_SCREEN: usize = 400;
pub const CMD_TOGGLE_RAINBOW: usize = 500;
pub const CMD_TOGGLE_ENABLED: usize = 600;
pub const CMD_TOGGLE_OUTLINE: usize = 650;
pub const CMD_TOGGLE_GRID: usize = 652;
pub const CMD_TOGGLE_FROSTED: usize = 653;
pub const CMD_TOGGLE_ETHEREAL: usize = 654;
pub const CMD_TOGGLE_PRESSURE_MONITOR: usize = 655;
pub const CMD_TOGGLE_OCR: usize = 660;
pub const CMD_OCR_PROGRESS: usize = 870;
pub const CMD_NAVIGATE_TO_PAGE: usize = 810;
pub const CMD_PAGE_PREV: usize = 720;
pub const CMD_PAGE_NEXT: usize = 721;
pub const CMD_EXPORT_SVG_GIF: usize = 722;
pub const CMD_UNDO: usize = 800;
pub const CMD_QUIT: usize = 999;

// ── 颜色 & 线宽预设 ──
pub const COLOR_PRESETS: [(f64, f64, f64); 10] = [
    (1.0, 0.0, 0.0), (1.0, 0.5, 0.0), (1.0, 1.0, 0.0), (0.0, 0.8, 0.0), (0.0, 0.8, 0.8),
    (0.0, 0.4, 1.0), (0.6, 0.0, 0.8), (1.0, 0.4, 0.7), (1.0, 1.0, 1.0), (0.0, 0.0, 0.0),
];
pub const COLOR_NAMES_ZH: [&str; 10] = ["红", "橙", "黄", "绿", "青", "蓝", "紫", "粉", "白", "黑"];
// 8 档线宽倍率,与 Flutter 设置 UI 的 8 档一一对应
pub const WIDTH_PRESETS: [f64; 8] = [0.15, 0.3, 0.6, 1.0, 1.5, 2.0, 2.5, 3.5];
pub const WIDTH_NAMES_ZH: [&str; 8] = ["极细", "很细", "细", "中", "粗", "很粗", "超粗", "极粗"];

pub struct DrawState {
    pub pen_r: f64, pub pen_g: f64, pub pen_b: f64,
    pub width_scale: f64,
    pub selected_color: usize, pub selected_width: usize,
    pub enabled: bool, pub show_rainbow: bool,
    pub outline_enabled: bool,
    pub show_grid: bool,
    pub frosted: bool,
    /// 飘渺画布涂鸦模式:悬空/落笔显示笔迹,笔离开后隐藏
    pub ethereal: bool,
    /// 网格跟随涂鸦:飘渺模式下隐藏笔迹时网格也隐藏
    pub grid_follow_strokes: bool,
    /// 压力监控 HUD 是否开启
    pub pressure_monitor: bool,
    /// OCR 是否开启(首次开启时下载模型)
    pub ocr_enabled: bool,
}

// ── 共享状态(仅消息循环线程访问) ──

pub static OVERLAY_HWND: std::sync::Mutex<isize> = std::sync::Mutex::new(0);

struct OverlayState {
    canvas: OverlayCanvas,
    draw: DrawState,
    /// 当前笔的曲线点列(像素坐标 + 半径),用于轮廓填充
    pen_path: Vec<(f32, f32, f32)>,
    /// ink-stroke-modeler(位置/压力平滑 + 120Hz 重采样)
    stroke_modeler: StrokeModeler,
    start_time: Instant,
    /// 当前是否处于笔画中(首事件必须发 Down)
    in_stroke: bool,
    /// 笔迹当前是否可见(飘渺模式)
    strokes_visible: bool,
}

static STATE: AtomicPtr<OverlayState> = AtomicPtr::new(std::ptr::null_mut());

// ── 全屏透明 overlay 画布(UpdateLayeredWindowIndirect + 32bit BGRA DIB) ──
// cairo 渲染统一走 crate::cairo_dl(动态加载 libcairo-2.dll,直接画到 DIB 内存)

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
            let dib = CreateDIBSection(
                Some(dib_dc),
                &bmi,
                DIB_RGB_COLORS,
                &mut bits,
                None,
                0,
            )
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
                hwnd, dib_dc, dib, old, bits: bits as *mut u8, screen_dc,
                w: w.max(1), h: h.max(1), pos, cairo, color: (0, 0, 0),
            }
        }
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
            return RECT { left, top, right, bottom };
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
                    let t = if t < 0.0 { 0.0 } else if t > 1.0 { 1.0 } else { t };
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

        RECT { left, top, right, bottom }
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
            let size = SIZE { cx: self.w, cy: self.h };
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
        let dirty = RECT { left: 0, top: 0, right: self.w, bottom: self.h };
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
    fn draw_grid(&mut self) {
        const GAP: i32 = 40;
        const GA: u8 = 38; // 0.15 * 255
        const GR: u8 = 19; // 0.5 * 0.15 * 255 (50% 灰,premultiplied)
        let w = self.w;
        let h = self.h;
        unsafe {
            let bits = self.bits;
            // 竖线
            let mut gx = 0;
            while gx <= w {
                for y in 0..h {
                    let i = ((y as usize) * (w as usize) + gx as usize) * 4;
                    *bits.add(i) = GR;       // B
                    *bits.add(i + 1) = GR;   // G
                    *bits.add(i + 2) = GR;   // R
                    *bits.add(i + 3) = GA;   // A
                }
                gx += GAP;
            }
            // 横线
            let mut gy = 0;
            while gy <= h {
                for x in 0..w {
                    let i = ((gy as usize) * (w as usize) + x as usize) * 4;
                    *bits.add(i) = GR;
                    *bits.add(i + 1) = GR;
                    *bits.add(i + 2) = GR;
                    *bits.add(i + 3) = GA;
                }
                gy += GAP;
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

/// 线宽半径(线性压力映射 × 线宽倍率):(0.75 + p*1.75) * scale(直径 1.5..5px × scale)
fn width_r(p: f32, scale: f32) -> f32 {
    (0.75 + p.clamp(0.0, 1.0) * 1.75) * scale.max(0.05)
}

/// 由中心线点列(x, y, 半径)构建可变宽度轮廓:
/// 每点沿法线 ±偏移半径,返回闭合轮廓点列(左边界正向 + 右边界反向)。
/// 法线用中点差分(前后点方向),保证相邻带在共享点处偏移一致,无缝衔接。
fn build_outline(pts: &[(f32, f32, f32)]) -> Vec<(f32, f32)> {
    let n = pts.len();
    if n < 2 {
        return vec![];
    }
    let mut left = Vec::with_capacity(n);
    let mut right = Vec::with_capacity(n);
    for i in 0..n {
        let (x, y, r) = pts[i];
        let (dx, dy) = if i == 0 {
            (pts[1].0 - x, pts[1].1 - y)
        } else if i == n - 1 {
            (x - pts[i - 1].0, y - pts[i - 1].1)
        } else {
            (pts[i + 1].0 - pts[i - 1].0, pts[i + 1].1 - pts[i - 1].1)
        };
        let l = (dx * dx + dy * dy).sqrt();
        let (nx, ny) = if l > 1e-6 {
            (-dy / l, dx / l)
        } else {
            (1.0, 0.0)
        };
        left.push((x + nx * r, y + ny * r));
        right.push((x - nx * r, y - ny * r));
    }
    let mut outline = left;
    outline.extend(right.iter().rev());
    outline
}

/// 合并脏矩形
fn merge_rect(dirty: &mut Option<RECT>, r: &RECT) {
    match dirty {
        None => *dirty = Some(*r),
        Some(d) => {
            d.left = d.left.min(r.left);
            d.top = d.top.min(r.top);
            d.right = d.right.max(r.right);
            d.bottom = d.bottom.max(r.bottom);
        }
    }
}

/// 模型器输出点转 pen_path 点列(带半径)
fn modeler_pts_to_path(results: &[ink_stroke_modeler_rs::ModelerResult], scale: f32) -> Vec<(f32, f32, f32)> {
    results
        .iter()
        .map(|r| (r.pos.0 as f32, r.pos.1 as f32, width_r(r.pressure as f32, scale)))
        .collect()
}

/// 轮廓色(黑/白,根据笔迹亮度取对比色,用于描边增强)
fn contrast_color(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let lum = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
    if lum > 128.0 { (0, 0, 0) } else { (255, 255, 255) }
}

/// 用给定点列填充整笔轮廓 + 端点圆帽(带可选描边),返回脏矩形
fn fill_stroke_path(canvas: &mut OverlayCanvas, path: &[(f32, f32, f32)], ol: f32) -> Option<RECT> {
    if path.len() < 2 {
        return None;
    }
    let mut dirty: Option<RECT> = None;

    // 描边层:整笔轮廓放大 ol 后用对比色填充
    if ol > 0.0 {
        let ol_color = contrast_color(canvas.color.0, canvas.color.1, canvas.color.2);
        let saved = canvas.color;
        canvas.color = ol_color;
        let wide: Vec<(f32, f32, f32)> =
            path.iter().map(|&(x, y, r)| (x, y, r + ol)).collect();
        let outline = build_outline(&wide);
        if outline.len() >= 3 {
            let rect = canvas.fill_outline(&outline);
            merge_rect(&mut dirty, &rect);
        }
        if let Some(&(cx, cy, r)) = path.first() {
            let rect = canvas.fill_dot(cx, cy, r + ol);
            merge_rect(&mut dirty, &rect);
        }
        if let Some(&(cx, cy, r)) = path.last() {
            let rect = canvas.fill_dot(cx, cy, r + ol);
            merge_rect(&mut dirty, &rect);
        }
        canvas.color = saved;
    }

    // 主体
    let outline = build_outline(path);
    if outline.len() >= 3 {
        let rect = canvas.fill_outline(&outline);
        merge_rect(&mut dirty, &rect);
    }
    if let Some(&(cx, cy, r)) = path.first() {
        let rect = canvas.fill_dot(cx, cy, r);
        merge_rect(&mut dirty, &rect);
    }
    if let Some(&(cx, cy, r)) = path.last() {
        let rect = canvas.fill_dot(cx, cy, r);
        merge_rect(&mut dirty, &rect);
    }
    dirty
}

/// 整笔轮廓重填 + 端点圆帽,返回脏矩形(仅新增段区域)
fn redraw_pen(state: &mut OverlayState, new_pts: &[(f32, f32, f32)]) -> Option<RECT> {
    let ol = if state.draw.outline_enabled { 1.0 } else { 0.0 };

    if state.pen_path.is_empty() && !new_pts.is_empty() {
        let (cx, cy, r) = new_pts[0];
        let _ = state.canvas.fill_dot(cx, cy, r);
    }
    let new_start = state.pen_path.len();
    state.pen_path.extend_from_slice(new_pts);
    let new_end = state.pen_path.len();

    // 整笔轮廓填充(单轮廓,非零环绕)+ 可选描边层
    fill_stroke_path(&mut state.canvas, &state.pen_path, ol);

    // dirty 只保留新增段区域(旧区域内容未变)
    let mut new_dirty: Option<RECT> = None;
    for &(px, py, r) in &state.pen_path[new_start.saturating_sub(1)..new_end] {
        let rr = r.ceil() as i32 + 1 + ol as i32;
        let rect = RECT {
            left: (px as i32 - rr).max(0),
            top: (py as i32 - rr).max(0),
            right: (px as i32 + rr + 1).min(state.canvas.w),
            bottom: (py as i32 + rr + 1).min(state.canvas.h),
        };
        merge_rect(&mut new_dirty, &rect);
    }
    new_dirty
}

/// 处理一个采样点:喂给 ink-stroke-modeler,输出平滑点列后轮廓填充
fn handle_point(state: &mut OverlayState, x: f32, y: f32, p: f32, down: bool) -> Option<RECT> {
    // 模型器要求首事件 Down,后续 Move,抬起 Up
    let event_type = if !down {
        ModelerInputEventType::Up
    } else if state.in_stroke {
        ModelerInputEventType::Move
    } else {
        ModelerInputEventType::Down
    };
    let input = ModelerInput {
        event_type,
        pos: (x as f64, y as f64),
        time: state.start_time.elapsed().as_secs_f64(),
        pressure: p as f64,
    };
    let results = match state.stroke_modeler.update(input) {
        Ok(r) => r,
        Err(_) => return None, // Duplicate/负时间等,忽略
    };

    // 落笔时确定笔迹颜色
    if down && !state.in_stroke {
        state.canvas.color = (
            (state.draw.pen_r * 255.0) as u8,
            (state.draw.pen_g * 255.0) as u8,
            (state.draw.pen_b * 255.0) as u8,
        );
    }

    // 记录笔画到 STROKES/DB(用于撤销、导出、XOJ 保存)
    if down && !state.in_stroke {
        crate::export::glaspen2_begin_stroke(
            state.draw.pen_r,
            state.draw.pen_g,
            state.draw.pen_b,
            state.draw.width_scale,
        );
    }
    if !results.is_empty() {
        let scale = state.draw.width_scale as f32;
        for r in &results {
            crate::export::glaspen2_add_point(
                r.pos.0,
                r.pos.1,
                (width_r(r.pressure as f32, scale) * 2.0) as f64,
            );
        }
    }
    let pts = modeler_pts_to_path(&results, state.draw.width_scale as f32);

    if !down {
        // 抬起:补最后一段轮廓 + 终点圆帽,清空并重置模型器
        state.in_stroke = false;
        let dirty = redraw_pen(state, &pts);
        crate::export::glaspen2_end_stroke();
        state.pen_path.clear();
        let params = modeler_params();
        let _ = state.stroke_modeler.reset_w_params(params);
        state.start_time = Instant::now();
        return dirty;
    }

    state.in_stroke = true;
    redraw_pen(state, &pts)
}

fn modeler_params() -> ModelerParams {
    ModelerParams {
        sampling_min_output_rate: 120.0,
        sampling_end_of_stroke_stopping_distance: 0.01,
        sampling_end_of_stroke_max_iterations: 20,
        sampling_max_outputs_per_call: 200,
        stylus_state_modeler_max_input_samples: 20,
        ..ModelerParams::suggested()
    }
}

/// 从 GetRawInputData 原始 buffer 手工解析 HID 报告
/// 布局: [RAWINPUTHEADER 24B][dwSizeHid 4B][dwCount 4B][报告 dwSizeHid*dwCount 字节]
unsafe fn process_raw_hid(buf: &[u64]) -> Option<RECT> {
    let raw = buf.as_ptr() as *const u8;
    let dw_type = u32::from_le_bytes(std::slice::from_raw_parts(raw, 4).try_into().unwrap());
    if dw_type != 2 {
        return None; // 不是 RIM_TYPEHID
    }
    let dw_size_hid =
        u32::from_le_bytes(std::slice::from_raw_parts(raw.add(24), 4).try_into().unwrap()) as usize;
    let dw_count =
        u32::from_le_bytes(std::slice::from_raw_parts(raw.add(28), 4).try_into().unwrap()) as usize;
    if dw_size_hid == 0 || dw_count == 0 {
        return None;
    }
    let base = raw.add(32);

    let state = &mut *STATE.load(Ordering::SeqCst);

    let mut dirty: Option<RECT> = None;
    for i in 0..dw_count {
        let data = std::slice::from_raw_parts(base.add(i * dw_size_hid), dw_size_hid);
        if data.len() < 8 {
            continue;
        }
        let switches = data[1];
        let x = (data[2] as u32) | ((data[3] as u32) << 8);
        let y = (data[4] as u32) | ((data[5] as u32) << 8);
        let press = (data[6] as u32) | ((data[7] as u32) << 8);
        if x > 0x1_0000 || y > 0x1_0000 {
            continue;
        }
        let sx = (x.min(MAX_RAW_X as u32) as i64 * (state.canvas.w - 1) as i64 / MAX_RAW_X) as f32;
        let sy = (y.min(MAX_RAW_Y as u32) as i64 * (state.canvas.h - 1) as i64 / MAX_RAW_Y) as f32;
        let pnorm = (press as f32 / 16383.0).clamp(0.0, 1.0);
        let down = (switches & 0x05) != 0;
        // 压力监控:每帧刷新
        if state.draw.pressure_monitor {
            hud_update_pressure(press as i32, down, sx as i32, sy as i32);
        }
        if let Some(rect) = handle_point(state, sx, sy, pnorm, down) {
            merge_rect(&mut dirty, &rect);
        }
    }
    dirty
}

// ── 窗口过程 ──

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if STATE.load(Ordering::SeqCst).is_null() {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }

    match msg {
        WM_INPUT => {
            let state = &mut *STATE.load(Ordering::SeqCst);
            if !state.draw.enabled {
                // 涂鸦关闭:保持穿透,不拦截、不处理
                set_input_blocking(hwnd, false);
                return LRESULT(0);
            }
            // 笔报告到达 = 笔在范围内:拦截输入(清除 WS_EX_TRANSPARENT),重置离开计时
            set_input_blocking(hwnd, true);
            let _ = KillTimer(Some(hwnd), TIMER_UNBLOCK);
            let _ = KillTimer(Some(hwnd), TIMER_PEEK);

            // 飘渺模式:笔悬空/落笔 → 显示笔迹(仅首次,避免高频全量重绘)
            if state.draw.ethereal && !state.strokes_visible {
                show_strokes(state);
            }

            let hraw = HRAWINPUT(lparam.0 as *mut core::ffi::c_void);
            let mut size: u32 = 0;
            let _ = GetRawInputData(
                hraw,
                RID_INPUT,
                None,
                &mut size,
                std::mem::size_of::<RAWINPUTHEADER>() as u32,
            );
            if size > 0 {
                let n = ((size as usize) + 7) / 8;
                let mut buf = vec![0u64; n];
                let written = GetRawInputData(
                    hraw,
                    RID_INPUT,
                    Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
                    &mut size,
                    std::mem::size_of::<RAWINPUTHEADER>() as u32,
                );
                if written > 0 {
                    if let Some(dirty) = process_raw_hid(&buf) {
                        let state = &mut *STATE.load(Ordering::SeqCst);
                        state.canvas.present_rect(&dirty);
                    }
                }
            }
            let _ = SetTimer(Some(hwnd), TIMER_UNBLOCK, UNBLOCK_DELAY_MS, None);
            LRESULT(0)
        }
        WM_TIMER => {
            let state = &mut *STATE.load(Ordering::SeqCst);
            match wparam.0 as usize {
                TIMER_UNBLOCK => {
                    let _ = KillTimer(Some(hwnd), TIMER_UNBLOCK);
                    // 笔离开:设 WS_EX_TRANSPARENT 恢复穿透
                    set_input_blocking(hwnd, false);
                    // 飘渺模式:隐藏笔迹
                    if state.draw.ethereal && state.strokes_visible && !state.in_stroke {
                        hide_strokes(state);
                    }
                }
                TIMER_PEEK => {
                    let _ = KillTimer(Some(hwnd), TIMER_PEEK);
                    // 飘渺模式翻页 peek 到期:无笔活动时隐藏
                    if state.draw.ethereal && !state.in_stroke && state.strokes_visible {
                        hide_strokes(state);
                    }
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_HOTKEY => {
            let state = &mut *STATE.load(Ordering::SeqCst);
            match wparam.0 as i32 {
                // Ctrl+Alt+C 新建画布 / Ctrl+Alt+V 开关涂鸦 / Ctrl+Alt+Z 撤销
                1 => clear_screen(state),
                2 => toggle_enabled(state),
                3 => undo_last_stroke(state),
                // Ctrl+Alt+J/K 上一页/下一页
                4 => navigate_page(state, false),
                5 => navigate_page(state, true),
                // Ctrl+Alt+G 导出 SVG + GIF 并复制到剪贴板
                6 => export_svg_gif_clipboard(state),
                // Ctrl+Alt+B 模糊背景(磨砂玻璃)
                7 => toggle_frosted(state),
                // Ctrl+Alt+X 切换固定/飘渺画布模式
                9 => handle_command(state, CMD_TOGGLE_ETHEREAL, usize::MAX),
                // Ctrl+Alt+Q 退出
                8 => {
                    unsafe { let _ = DestroyWindow(hwnd); }
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_TRAY_COMMAND => {
            let state = &mut *STATE.load(Ordering::SeqCst);
            handle_command(state, wparam.0, lparam.0 as usize);
            LRESULT(0)
        }
        WM_KEYDOWN => DefWindowProcW(hwnd, msg, wparam, lparam),
        WM_DISPLAYCHANGE => {
            let state = &mut *STATE.load(Ordering::SeqCst);
            on_display_change(state);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// ── 入口 ──

pub fn run() {
    unsafe {
        // 初始化数据库(创建首个屏幕记录),必须在任何 DB 访问之前
        {
            let sw = GetSystemMetrics(SM_CXVIRTUALSCREEN);
            let sh = GetSystemMetrics(SM_CYVIRTUALSCREEN);
            crate::export::glaspen2_init_db(sw, sh);
        }

        let hwnd = create_overlay_window();

        let mut pen_r = 1.0; let mut pen_g = 0.0; let mut pen_b = 0.0; let mut width_scale = 0.3;
        crate::export::glaspen2_load_settings_parts(&mut pen_r, &mut pen_g, &mut pen_b, &mut width_scale);
        let outline_enabled = crate::runtime().block_on(crate::db::load_setting("outline_enabled"))
            .and_then(|v| v.parse::<i32>().ok()).unwrap_or(0) != 0;
        let frosted = crate::runtime().block_on(crate::db::load_setting("frostedGlass"))
            .and_then(|v| v.parse::<i32>().ok()).unwrap_or(0) != 0;
        let show_grid = crate::runtime().block_on(crate::db::load_setting("grid"))
            .and_then(|v| v.parse::<i32>().ok()).unwrap_or(0) != 0;
        let ethereal = crate::runtime().block_on(crate::db::load_setting("ethereal"))
            .and_then(|v| v.parse::<i32>().ok()).unwrap_or(0) != 0;
        let grid_follow_strokes = crate::runtime().block_on(crate::db::load_setting("gridFollowStrokes"))
            .and_then(|v| v.parse::<i32>().ok()).unwrap_or(0) != 0;
        let pressure_monitor = crate::runtime().block_on(crate::db::load_setting("pressureMonitor"))
            .and_then(|v| v.parse::<i32>().ok()).unwrap_or(0) != 0;
        let ocr_enabled = crate::runtime().block_on(crate::db::load_setting("ocrEnabled"))
            .and_then(|v| v.parse::<i32>().ok()).unwrap_or(0) != 0;

        // HUD 窗口(通知居中 + 压力监控左上角)
        {
            let (notif_hwnd, pm_hwnd) = hud_create();
            HUD = Box::into_raw(Box::new(HudState {
                notif_hwnd,
                pm_hwnd,
                notif: None,
                pm_text: String::new(),
                pm_visible: pressure_monitor,
            }));
            hud_toggle_pressure(pressure_monitor);
        }

        let mut canvas = OverlayCanvas::create(hwnd);
        canvas.color = (pen_r as u8, pen_g as u8, pen_b as u8);

        let draw = DrawState {
            pen_r, pen_g, pen_b, width_scale,
            selected_color: closest_color_index(pen_r, pen_g, pen_b),
            selected_width: closest_width_index(width_scale),
            enabled: true, show_rainbow: false,
            outline_enabled, show_grid, frosted,
            ethereal, grid_follow_strokes,
            pressure_monitor, ocr_enabled,
        };

        let mut state = OverlayState {
            canvas,
            draw,
            pen_path: Vec::new(),
            stroke_modeler: StrokeModeler::default(),
            start_time: Instant::now(),
            in_stroke: false,
            strokes_visible: true,
        };
        let _ = state.stroke_modeler.reset_w_params(modeler_params());
        if show_grid {
            state.canvas.draw_grid();
        }
        state.canvas.set_bg_alpha(BG_BLOCK);
        // 初始穿透(WS_EX_TRANSPARENT),鼠标可正常操作;笔事件到达时自动唤醒拦截
        set_input_blocking(hwnd, false);
        STATE.store(Box::into_raw(Box::new(state)), Ordering::SeqCst);

        // 注册 Digitizer 设备收笔报告(WM_INPUT 不依赖 hit test,穿透时也能收到)
        let mut devices = [
            RAWINPUTDEVICE {
                usUsagePage: 0x0D,
                usUsage: 0x01,
                dwFlags: RIDEV_INPUTSINK,
                hwndTarget: hwnd,
            },
            RAWINPUTDEVICE {
                usUsagePage: 0x0D,
                usUsage: 0x02,
                dwFlags: RIDEV_INPUTSINK,
                hwndTarget: hwnd,
            },
        ];
        let r = RegisterRawInputDevices(&mut devices, std::mem::size_of::<RAWINPUTDEVICE>() as u32);
        println!("[overlay] RegisterRawInputDevices: {:?}", r);

        // 热键(README 快捷键表):Ctrl+Alt+C 新建画布 / V 开关 / Z 撤销 /
        // J/K 翻页 / G 导出 / B 模糊背景 / Q 退出
        let mods = HOT_KEY_MODIFIERS(MOD_CONTROL.0 | MOD_ALT.0);
        RegisterHotKey(Some(hwnd), 1, mods, 'C' as u32).ok();
        RegisterHotKey(Some(hwnd), 2, mods, 'V' as u32).ok();
        RegisterHotKey(Some(hwnd), 3, mods, 'Z' as u32).ok();
        RegisterHotKey(Some(hwnd), 4, mods, 'J' as u32).ok();
        RegisterHotKey(Some(hwnd), 5, mods, 'K' as u32).ok();
        RegisterHotKey(Some(hwnd), 6, mods, 'G' as u32).ok();
        RegisterHotKey(Some(hwnd), 7, mods, 'B' as u32).ok();
        RegisterHotKey(Some(hwnd), 8, mods, 'Q' as u32).ok();
        RegisterHotKey(Some(hwnd), 9, mods, 'X' as u32).ok();

        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _ = UpdateWindow(hwnd);
        if frosted {
            apply_frosted(hwnd, true);
        }

        // Flutter 设置管道线程
        {
            let pipe_hwnd = hwnd.0 as isize;
            std::thread::spawn(move || {
                run_settings_pipe_server(pipe_hwnd);
            });
        }

        println!("[overlay] 全屏透明涂鸦已启动(WM_INPUT + ink-stroke-modeler + cairo)。");
        println!("[overlay] 快捷键: Ctrl+Alt+C 新建画布 / V 开关 / Z 撤销 / J/K 翻页 / G 导出 / B 模糊背景 / X 固定↔飘渺 / Q 退出");
        run_loop();

        let p = STATE.swap(std::ptr::null_mut(), Ordering::SeqCst);
        if !p.is_null() {
            drop(Box::from_raw(p));
        }
        if !HUD.is_null() {
            drop(Box::from_raw(HUD));
            HUD = std::ptr::null_mut();
        }
    }
}

fn create_overlay_window() -> HWND {
    unsafe {
        let class_name = wide_string("Glaspen2OverlayV2");
        let hinst: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
        let wc = WNDCLASSW {
            style: WNDCLASS_STYLES(0),
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: HICON::default(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: HBRUSH::default(),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
        };
        let _ = RegisterClassW(&wc);

        let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let cx = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let cy = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            PCWSTR(class_name.as_ptr()),
            PCWSTR::null(),
            WS_POPUP,
            x,
            y,
            cx,
            cy,
            None,
            None,
            Some(hinst),
            None,
        )
        .expect("CreateWindowExW failed");
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            cx,
            cy,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        { let mut h = OVERLAY_HWND.lock().unwrap(); *h = hwnd.0 as isize; }
        hwnd
    }
}

fn set_input_blocking(hwnd: HWND, blocking: bool) {
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let transparent = if blocking { 0 } else { WS_EX_TRANSPARENT.0 as isize };
        let new_style = (style & !(WS_EX_TRANSPARENT.0 as isize)) | transparent;
        let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style);
    }
}

// ── HUD 窗口:屏幕中央通知 + 左上角压力监控(macOS 一致) ──
// 黑色半透明底 + 白字,主消息循环线程访问。

const HUD_NOTIF_W: i32 = 460;
const HUD_NOTIF_H: i32 = 48;
const HUD_PM_W: i32 = 240;
const HUD_PM_H: i32 = 32;
const HUD_NOTIF_TIMER: usize = 10;
const HUD_NOTIF_MS: u32 = 1200;

/// 日期标签:今天 / 昨天 / YYYY-MM-DD(本地时区,chrono 计算)
fn date_label(unix_secs: u64) -> String {
    use chrono::{Local, TimeZone};
    let now = Local::now();
    let ts = Local
        .timestamp_opt(unix_secs as i64, 0)
        .single()
        .unwrap_or(now);
    if ts.date_naive() == now.date_naive() {
        "今天".to_string()
    } else if now.date_naive().signed_duration_since(ts.date_naive()).num_days() == 1 {
        "昨天".to_string()
    } else {
        ts.format("%Y-%m-%d").to_string()
    }
}

// ── Flutter 字体(LXGWWenKaiMono) ──

#[link(name = "gdi32")]
unsafe extern "system" {
    fn AddFontResourceExW(name: PCWSTR, flags: u32, res: *const std::ffi::c_void) -> i32;
    fn GetTextFaceW(hdc: HDC, cchcount: i32, lpname: *mut u16) -> i32;
}

// ── GDI+ 文字渲染(灰度抗锯齿,消除 GDI 文本的颗粒感) ──

use libloading::{Library, Symbol};

const TEXT_RENDERING_HINT_ANTIALIAS: i32 = 4; // AntiAlias
const STRING_ALIGNMENT_CENTER: i32 = 1;
const STATUS_OK: i32 = 0;
const UNIT_PIXEL: i32 = 3;

#[repr(C)]
struct GdiplusStartupInput {
    gdiplus_version: u32,
    debug_event_callback: *mut std::ffi::c_void,
    suppress_background_thread: i32,
    suppress_external_codecs: i32,
}

#[repr(C)]
struct GdiplusStartupOutput {
    notification_hook: *mut std::ffi::c_void,
    notification_unhook: *mut std::ffi::c_void,
}

#[repr(C)]
struct RectF {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

struct Gdiplus {
    _lib: Library,
    token: usize,
    create_from_hdc: unsafe extern "C" fn(HDC, *mut *mut std::ffi::c_void) -> i32,
    delete_graphics: unsafe extern "C" fn(*mut std::ffi::c_void) -> i32,
    set_text_rendering_hint: unsafe extern "C" fn(*mut std::ffi::c_void, i32) -> i32,
    new_private_font_collection: unsafe extern "C" fn(*mut *mut std::ffi::c_void) -> i32,
    private_add_font_file: unsafe extern "C" fn(*mut std::ffi::c_void, *const u16) -> i32,
    create_font_family_from_name: unsafe extern "C" fn(*const u16, *mut std::ffi::c_void, *mut *mut std::ffi::c_void) -> i32,
    delete_font_family: unsafe extern "C" fn(*mut std::ffi::c_void) -> i32,
    create_font: unsafe extern "C" fn(*mut std::ffi::c_void, f32, i32, i32, *mut *mut std::ffi::c_void) -> i32,
    delete_font: unsafe extern "C" fn(*mut std::ffi::c_void) -> i32,
    create_solid_fill: unsafe extern "C" fn(u32, *mut *mut std::ffi::c_void) -> i32,
    delete_brush: unsafe extern "C" fn(*mut std::ffi::c_void) -> i32,
    create_string_format: unsafe extern "C" fn(i32, u16, *mut *mut std::ffi::c_void) -> i32,
    set_string_align: unsafe extern "C" fn(*mut std::ffi::c_void, i32) -> i32,
    set_line_align: unsafe extern "C" fn(*mut std::ffi::c_void, i32) -> i32,
    delete_string_format: unsafe extern "C" fn(*mut std::ffi::c_void) -> i32,
    measure_string: unsafe extern "C" fn(
        *mut std::ffi::c_void,
        *const u16,
        i32,
        *mut std::ffi::c_void,
        *const RectF,
        *mut std::ffi::c_void,
        *mut RectF,
        *mut i32,
        *mut i32,
    ) -> i32,
    draw_string: unsafe extern "C" fn(
        *mut std::ffi::c_void,
        *const u16,
        i32,
        *mut std::ffi::c_void,
        *const RectF,
        *mut std::ffi::c_void,
        *mut std::ffi::c_void,
    ) -> i32,
}

fn gdiplus() -> Option<&'static Gdiplus> {
    use std::sync::OnceLock;
    static GP: OnceLock<Option<Gdiplus>> = OnceLock::new();
    GP.get_or_init(|| {
        let lib = unsafe { libloading::Library::new("gdiplus.dll") }.ok()?;
        unsafe fn sym<T: Copy>(lib: &Library, name: &[u8]) -> Option<T> {
            let s: Symbol<T> = lib.get(name).ok()?;
            Some(*s)
        }
        let startup: unsafe extern "C" fn(*mut usize, *const GdiplusStartupInput, *mut GdiplusStartupOutput) -> i32 =
            unsafe { sym(&lib, b"GdiplusStartup") }?;
        let shutdown: unsafe extern "C" fn(usize) = unsafe { sym(&lib, b"GdiplusShutdown") }?;
        let _ = shutdown;

        let mut input = GdiplusStartupInput {
            gdiplus_version: 1,
            debug_event_callback: std::ptr::null_mut(),
            suppress_background_thread: 0,
            suppress_external_codecs: 0,
        };
        let mut token: usize = 0;
        let status = unsafe { startup(&mut token, &input, std::ptr::null_mut()) };
        if status != STATUS_OK {
            return None;
        }
        let create_from_hdc = unsafe { sym(&lib, b"GdipCreateFromHDC") }?;
        let delete_graphics = unsafe { sym(&lib, b"GdipDeleteGraphics") }?;
        let set_text_rendering_hint = unsafe { sym(&lib, b"GdipSetTextRenderingHint") }?;
        let new_private_font_collection = unsafe { sym(&lib, b"GdipNewPrivateFontCollection") }?;
        let private_add_font_file = unsafe { sym(&lib, b"GdipPrivateAddFontFile") }?;
        let create_font_family_from_name = unsafe { sym(&lib, b"GdipCreateFontFamilyFromName") }?;
        let delete_font_family = unsafe { sym(&lib, b"GdipDeleteFontFamily") }?;
        let create_font = unsafe { sym(&lib, b"GdipCreateFont") }?;
        let delete_font = unsafe { sym(&lib, b"GdipDeleteFont") }?;
        let create_solid_fill = unsafe { sym(&lib, b"GdipCreateSolidFill") }?;
        let delete_brush = unsafe { sym(&lib, b"GdipDeleteBrush") }?;
        let create_string_format = unsafe { sym(&lib, b"GdipCreateStringFormat") }?;
        let set_string_align = unsafe { sym(&lib, b"GdipSetStringFormatAlign") }?;
        let set_line_align = unsafe { sym(&lib, b"GdipSetStringFormatLineAlign") }?;
        let delete_string_format = unsafe { sym(&lib, b"GdipDeleteStringFormat") }?;
        let measure_string = unsafe { sym(&lib, b"GdipMeasureString") }?;
        let draw_string = unsafe { sym(&lib, b"GdipDrawString") }?;
        Some(Gdiplus {
            _lib: lib,
            token,
            create_from_hdc,
            delete_graphics,
            set_text_rendering_hint,
            new_private_font_collection,
            private_add_font_file,
            create_font_family_from_name,
            delete_font_family,
            create_font,
            delete_font,
            create_solid_fill,
            delete_brush,
            create_string_format,
            set_string_align,
            set_line_align,
            delete_string_format,
            measure_string,
            draw_string,
        })
    })
    .as_ref()
}

const FR_PRIVATE: u32 = 0x10;

/// 查找 Flutter 打包的字体文件路径(exe 旁 → 开发构建目录 → 源码 assets)
fn find_font_file() -> Option<std::path::PathBuf> {
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    // 1) exe 旁(打包分发:data/flutter_assets/assets/)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(
                dir.join("data").join("flutter_assets").join("assets")
                    .join("LXGWWenKaiMono-Regular.ttf"),
            );
        }
    }
    // 2) 开发构建目录 / 源码 assets(CARGO_MANIFEST_DIR 仅编译期可用)
    if let Some(manifest) = option_env!("CARGO_MANIFEST_DIR") {
        let m = std::path::Path::new(manifest);
        for sub in [
            "flutter_settings/build/windows/x64/runner/Release/data/flutter_assets/assets",
            "flutter_settings/build/windows/x64/runner/Debug/data/flutter_assets/assets",
            "flutter_settings/assets",
        ] {
            candidates.push(m.join(sub).join("LXGWWenKaiMono-Regular.ttf"));
        }
    }
    candidates.into_iter().find(|p| p.exists())
}

/// 注册 Flutter 打包的 LXGWWenKaiMono 字体(仅当前进程 FR_PRIVATE,
/// 不污染系统,供 GDI 压力窗口使用)。返回 true 表示可用。
fn register_flutter_font() -> bool {
    let Some(path) = find_font_file() else {
        return false;
    };
    let wide: Vec<u16> = path
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let ret = unsafe { AddFontResourceExW(PCWSTR(wide.as_ptr()), FR_PRIVATE, std::ptr::null()) };
    if ret > 0 {
        eprintln!("[overlay] 已注册 Flutter 字体: {}", path.display());
        return true;
    }
    false
}

/// 字体是否已注册(LXGWWenKaiMono);仅主线程在 hud_create 时设置
static mut HUD_FONT_OK: bool = false;

/// GDI+ 私人字体集合(通知渲染用,系统字体不经过 GDI 注册表),仅主线程访问
static mut GP_FONT_COLLECTION: *mut std::ffi::c_void = std::ptr::null_mut();

/// 把 Flutter 字体文件加入 GDI+ 私人字体集合。
/// System.Drawing 的 PrivateFontCollection 同路径(该字体经
/// AddFontResourceExW + CreateFontFromLogfont 会报 NotTrueTypeFont)。
unsafe fn gdiplus_load_font_file() -> bool {
    if !GP_FONT_COLLECTION.is_null() {
        return true;
    }
    let gp = match gdiplus() {
        Some(g) => g,
        None => return false,
    };
    let mut col: *mut std::ffi::c_void = std::ptr::null_mut();
    if (gp.new_private_font_collection)(&mut col) != STATUS_OK || col.is_null() {
        return false;
    }
    let Some(path) = find_font_file() else {
        return false;
    };
    let wide: Vec<u16> = path
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let st = (gp.private_add_font_file)(col, wide.as_ptr());
    if st == STATUS_OK {
        GP_FONT_COLLECTION = col;
        eprintln!("[overlay] GDI+ 私人字体已加载: {}", path.display());
        true
    } else {
        false
    }
}

/// 缓存的 GDI+ 通知字体(48px,创建一次全程复用)。
/// 不能每次渲染创建/删除:GDI+ 对同一 collection 重复
/// create_font_family_from_name 会返回已释放的同一地址(悬垂),
/// 第二次渲染即崩溃。
static mut GP_NOTIF_FONT: *mut std::ffi::c_void = std::ptr::null_mut();
static mut GP_NOTIF_FAMILY: *mut std::ffi::c_void = std::ptr::null_mut();

/// 获取缓存的 GDI+ 通知字体(48px,霞鹜文楷等宽;失败回退微软雅黑)
unsafe fn gdiplus_get_notif_font(gp: &Gdiplus) -> *mut std::ffi::c_void {
    if !GP_NOTIF_FONT.is_null() {
        return GP_NOTIF_FONT;
    }
    let collection = if gdiplus_load_font_file() {
        GP_FONT_COLLECTION
    } else {
        std::ptr::null_mut()
    };
    let names: [&str; 2] = ["霞鹜文楷等宽", "Microsoft YaHei UI"];
    for name in names {
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let mut family: *mut std::ffi::c_void = std::ptr::null_mut();
        let st = (gp.create_font_family_from_name)(wide.as_ptr(), collection, &mut family);
        if st != STATUS_OK || family.is_null() {
            continue;
        }
        let mut font: *mut std::ffi::c_void = std::ptr::null_mut();
        let st2 = (gp.create_font)(family, 48.0, 0, UNIT_PIXEL, &mut font);
        if st2 == STATUS_OK && !font.is_null() {
            GP_NOTIF_FAMILY = family;
            GP_NOTIF_FONT = font;

            return font;
        }
        let _ = (gp.delete_font_family)(family);
    }
    GP_NOTIF_FONT
}

struct HudState {
    /// 通知窗口(屏幕中央)
    notif_hwnd: HWND,
    /// 压力监控窗口(左上角,macOS 同位置)
    pm_hwnd: HWND,
    /// 短暂通知文本(1.2s 后清除)
    notif: Option<String>,
    /// 压力监控文本(常驻,开关控制)
    pm_text: String,
    pm_visible: bool,
}

static mut HUD: *mut HudState = std::ptr::null_mut();

fn hud_ref() -> &'static mut HudState {
    unsafe { &mut *HUD }
}

unsafe extern "system" fn hud_wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            let hud = hud_ref();
            // 通知窗口内容由 UpdateLayeredWindow 直接呈现,这里只清无效区;
            // 压力窗口保持 GDI 绘制(黑底)
            if hwnd == hud.pm_hwnd {
                let text = if hud.pm_visible { hud.pm_text.clone() } else { String::new() };
                if !text.is_empty() {
                    let font_name = if HUD_FONT_OK { "霞鹜文楷等宽" } else { "Microsoft YaHei UI" };
                    let font = CreateFontW(
                        -13, 0, 0, 0, 400, 0, 0, 0,
                        DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS,
                        CLEARTYPE_QUALITY, 0x40, // FF_DONTCARE
                        PCWSTR(wide_string(font_name).as_ptr()),
                    );
                    let old_font = SelectObject(hdc, font.into());
                    let _ = SetBkMode(hdc, TRANSPARENT);
                    let _ = SetTextColor(hdc, COLORREF(0x00_FF_FF_FF));
                    let mut rc = RECT { left: 4, top: 0, right: HUD_PM_W - 4, bottom: HUD_PM_H };
                    let mut wide: Vec<u16> = text.encode_utf16().collect();
                    let _ = DrawTextW(hdc, wide.as_mut_slice(), &mut rc, DT_VCENTER | DT_SINGLELINE);
                    let _ = SelectObject(hdc, old_font);
                    let _ = DeleteObject(font.into());
                }
            }
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_TIMER => {
            if wparam.0 as usize == HUD_NOTIF_TIMER {
                let _ = KillTimer(Some(hwnd), HUD_NOTIF_TIMER);
                let hud = hud_ref();
                hud.notif = None;
                let _ = ShowWindow(hud.notif_hwnd, SW_HIDE);
            }
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_DESTROY => LRESULT(0),
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// 创建一个小 HUD 窗口。压力窗口带黑色半透明底(SetLayeredWindowAttributes),
/// 通知窗口不用(内容用 UpdateLayeredWindow per-pixel alpha 直接呈现,无背景)。
unsafe fn hud_create_window(
    class: PCWSTR,
    hinst: HINSTANCE,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    black_bg: bool,
) -> HWND {
    let hwnd = CreateWindowExW(
        WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
        class,
        PCWSTR::null(),
        WS_POPUP,
        x,
        y,
        w,
        h,
        None,
        None,
        Some(hinst),
        None,
    )
    .expect("CreateWindowExW (HUD) failed");
    if black_bg {
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 220, LWA_ALPHA);
    }
    hwnd
}

/// 创建 HUD 窗口(通知:屏幕正中央透明;压力:左上角 macOS 同位置黑底)
fn hud_create() -> (HWND, HWND) {
    unsafe {
        HUD_FONT_OK = register_flutter_font();
        if gdiplus().is_some() {
            eprintln!("[overlay] GDI+ 已初始化");
        } else {
            eprintln!("[overlay] GDI+ 初始化失败!");
        }
        let class_name = wide_string("Glaspen2Hud");
        let hinst: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
        let wc = WNDCLASSW {
            style: WNDCLASS_STYLES(0),
            lpfnWndProc: Some(hud_wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: HICON::default(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: HBRUSH::default(),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
        };
        let _ = RegisterClassW(&wc);

        let sw = GetSystemMetrics(SM_CXSCREEN);
        let sh = GetSystemMetrics(SM_CYSCREEN);
        let class = PCWSTR(class_name.as_ptr());
        // 通知:屏幕正中央,透明(无背景)
        let notif_hwnd = hud_create_window(
            class,
            hinst,
            (sw - HUD_NOTIF_W) / 2,
            (sh - HUD_NOTIF_H) / 2,
            HUD_NOTIF_W,
            HUD_NOTIF_H,
            false,
        );
        // 压力监控:左上角 (10, 40),macOS 为 (10, height-40),黑底
        let pm_hwnd = hud_create_window(class, hinst, 10, 40, HUD_PM_W, HUD_PM_H, true);
        (notif_hwnd, pm_hwnd)
    }
}

/// 在通知窗口渲染文字:GDI+ 抗锯齿,透明背景 + 柔和黑阴影 + 白字。
/// 与 macOS 一样文字直接浮在屏幕上,没有黑底条。
unsafe fn hud_render_notif_text(hwnd: HWND, text: &str) {
    let pad = 16;
    let mut wide: Vec<u16> = text.encode_utf16().collect();

    // 测量文字尺寸(GDI+ 测量)
    let mut layout = RectF { x: 0.0, y: 0.0, width: 10000.0, height: 10000.0 };
    let mut measured = RectF { x: 0.0, y: 0.0, width: 0.0, height: 0.0 };
    let w;
    let h;
    {
        let gp = match gdiplus() {
            Some(g) => g,
            None => {
                eprintln!("[overlay] GDI+ 不可用,通知渲染失败");
                return;
            }
        };
        let (font, _family) = (gdiplus_get_notif_font(gp), GP_NOTIF_FAMILY);
        if font.is_null() {
            eprintln!("[overlay] GDI+ 字体创建失败");
            return;
        }
        let screen_dc = GetDC(None);
        let mut graphics: *mut std::ffi::c_void = std::ptr::null_mut();
        let st1 = (gp.create_from_hdc)(screen_dc, &mut graphics);
        if st1 != STATUS_OK || graphics.is_null() {
            eprintln!("[overlay] GDI+ create_from_hdc 失败: st={}", st1);
            let _ = ReleaseDC(None, screen_dc);
            return;
        }
        let _ = (gp.set_text_rendering_hint)(graphics, TEXT_RENDERING_HINT_ANTIALIAS);
        let mut format: *mut std::ffi::c_void = std::ptr::null_mut();
        let _ = (gp.create_string_format)(0, 0, &mut format);
        if !format.is_null() {
            let _ = (gp.set_string_align)(format, STRING_ALIGNMENT_CENTER);
            let _ = (gp.set_line_align)(format, STRING_ALIGNMENT_CENTER);
        }
        let mut fitted = 0i32;
        let mut lines = 0i32;
        let _ = (gp.measure_string)(
            graphics,
            wide.as_ptr(),
            wide.len() as i32,
            font,
            &layout,
            format,
            &mut measured,
            &mut fitted,
            &mut lines,
        );
        // 窗口比文字四周多出 pad*2 余量,绘制区域再留 pad 内边距,
        // 保证最后一个字符不被 GDI+ 在区域边缘裁剪
        w = (measured.width as i32) + pad * 4;
        h = (measured.height as i32) + pad * 4;
        let _ = (gp.delete_string_format)(format);
        let _ = (gp.delete_graphics)(graphics);
        let _ = ReleaseDC(None, screen_dc);
    }

    // 居中置顶
    let sw = GetSystemMetrics(SM_CXSCREEN);
    let sh = GetSystemMetrics(SM_CYSCREEN);
    let _ = SetWindowPos(
        hwnd,
        Some(HWND_TOPMOST),
        (sw - w) / 2,
        (sh - h) / 2,
        w.max(1),
        h.max(1),
        SWP_NOACTIVATE | SWP_SHOWWINDOW,
    );

    // DIB:32bpp 全透明
    let screen_dc = GetDC(None);
    let dib_dc = CreateCompatibleDC(Some(screen_dc));
    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w.max(1),
            biHeight: -h.max(1),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
    let hbmp = CreateDIBSection(Some(dib_dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0).unwrap();
    let old = SelectObject(dib_dc, hbmp.into());
    std::slice::from_raw_parts_mut(bits as *mut u8, (w as usize) * (h as usize) * 4).fill(0);

    // GDI+ 绘制:柔和黑阴影(多层偏移模拟 blur) + 白字
    {
        let gp = match gdiplus() {
            Some(g) => g,
            None => {
                let _ = SelectObject(dib_dc, old);
                let _ = DeleteObject(hbmp.into());
                let _ = DeleteDC(dib_dc);
                let _ = ReleaseDC(None, screen_dc);
                return;
            }
        };
        let font = gdiplus_get_notif_font(gp);
        if font.is_null() {
            let _ = SelectObject(dib_dc, old);
            let _ = DeleteObject(hbmp.into());
            let _ = DeleteDC(dib_dc);
            let _ = ReleaseDC(None, screen_dc);
            return;
        }
        let mut graphics: *mut std::ffi::c_void = std::ptr::null_mut();
        if (gp.create_from_hdc)(dib_dc, &mut graphics) != STATUS_OK || graphics.is_null() {
            let _ = SelectObject(dib_dc, old);
            let _ = DeleteObject(hbmp.into());
            let _ = DeleteDC(dib_dc);
            let _ = ReleaseDC(None, screen_dc);
            return;
        }
        let _ = (gp.set_text_rendering_hint)(graphics, TEXT_RENDERING_HINT_ANTIALIAS);
        let mut format: *mut std::ffi::c_void = std::ptr::null_mut();
        let _ = (gp.create_string_format)(0, 0, &mut format);
        if !format.is_null() {
            let _ = (gp.set_string_align)(format, STRING_ALIGNMENT_CENTER);
            let _ = (gp.set_line_align)(format, STRING_ALIGNMENT_CENTER);
        }
        let area = RectF {
            x: pad as f32,
            y: pad as f32,
            // 绘制区域比文字实际宽度多出 pad*2,文字居中,末字不会被裁剪
            width: (w - pad * 2) as f32,
            height: (h - pad * 2) as f32,
        };
        // 阴影:黑色半透明刷子,多层偏移模拟 blur
        let mut shadow_brush: *mut std::ffi::c_void = std::ptr::null_mut();
        let _ = (gp.create_solid_fill)(0xB0000000, &mut shadow_brush); // ARGB: alpha 176, 黑
        if !shadow_brush.is_null() {
            for (dx, dy) in [(2.0, 2.0), (1.0, 1.0), (2.0, 0.0), (0.0, 2.0), (1.0, 2.0), (2.0, 1.0)] {
                let sr = RectF {
                    x: area.x + dx,
                    y: area.y + dy,
                    width: area.width,
                    height: area.height,
                };
                let _ = (gp.draw_string)(graphics, wide.as_ptr(), wide.len() as i32, font, &sr, format, shadow_brush);
            }
            let _ = (gp.delete_brush)(shadow_brush);
        }
        // 主文字:白
        let mut white_brush: *mut std::ffi::c_void = std::ptr::null_mut();
        let _ = (gp.create_solid_fill)(0xFFFFFFFF, &mut white_brush);
        if !white_brush.is_null() {
            let _ = (gp.draw_string)(graphics, wide.as_ptr(), wide.len() as i32, font, &area, format, white_brush);
            let _ = (gp.delete_brush)(white_brush);
        }
        let _ = (gp.delete_string_format)(format);
        let _ = (gp.delete_graphics)(graphics);
    }

    // GDI+ 输出到 32bpp DIB 为非预乘 ARGB,ULW 需要预乘:RGB *= alpha/255
    {
        let n = (w as usize) * (h as usize);
        let p = bits as *mut u8;
        for i in 0..n {
            let off = i * 4;
            let a = *p.add(off + 3);
            if a != 0 && a != 255 {
                let r = (*p.add(off + 2) as u32 * a as u32 + 127) / 255;
                let g = (*p.add(off + 1) as u32 * a as u32 + 127) / 255;
                let b = (*p.add(off) as u32 * a as u32 + 127) / 255;
                *p.add(off) = b as u8;
                *p.add(off + 1) = g as u8;
                *p.add(off + 2) = r as u8;
            }
        }
    }

    // ULW 呈现(透明背景 + 文字)
    let blend = BLENDFUNCTION {
        BlendOp: 0,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: 1, // AC_SRC_ALPHA
    };
    let size = SIZE { cx: w.max(1), cy: h.max(1) };
    let src = POINT { x: 0, y: 0 };
    let _ = UpdateLayeredWindow(hwnd, None, None, Some(&size), Some(dib_dc), Some(&src), COLORREF(0), Some(&blend), ULW_ALPHA);

    // 清理
    let _ = SelectObject(dib_dc, old);
    let _ = DeleteObject(hbmp.into());
    let _ = DeleteDC(dib_dc);
    let _ = ReleaseDC(None, screen_dc);
}

/// 显示短暂通知(1.2s 后自动消失)
fn hud_notify(text: &str) {
    unsafe {
        if HUD.is_null() {
            return;
        }
        let hud = hud_ref();
        hud.notif = Some(text.to_string());
        hud_render_notif_text(hud.notif_hwnd, text);
        let _ = SetTimer(Some(hud.notif_hwnd), HUD_NOTIF_TIMER, HUD_NOTIF_MS, None);
    }
}

/// 更新压力监控文本(悬空/按下事件每帧调用)
fn hud_update_pressure(pressure: i32, down: bool, x: i32, y: i32) {
    unsafe {
        if HUD.is_null() {
            return;
        }
        let hud = hud_ref();
        if !hud.pm_visible {
            return;
        }
        let evtype = if down { "按下" } else { "悬空" };
        hud.pm_text = format!("压力 P={:<5}  {}  ({},{})", pressure, evtype, x, y);
        let _ = InvalidateRect(Some(hud.pm_hwnd), None, false);
    }
}

/// 开关压力监控
fn hud_toggle_pressure(on: bool) {
    unsafe {
        if HUD.is_null() {
            return;
        }
        let hud = hud_ref();
        hud.pm_visible = on;
        if on {
            hud.pm_text = "压力 P=-----  悬空  (---,---)".to_string();
            let _ = ShowWindow(hud.pm_hwnd, SW_SHOWNOACTIVATE);
        } else {
            let _ = ShowWindow(hud.pm_hwnd, SW_HIDE);
        }
        let _ = InvalidateRect(Some(hud.pm_hwnd), None, false);
    }
}

fn run_loop() {
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            let _ = DispatchMessageW(&msg);
        }
    }
}

fn wide_string(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

// ── 命令处理(设置管道 / 热键) ──

fn handle_command(state: &mut OverlayState, cmd: usize, param: usize) {
    // 开关类命令:管道传入 0/1 时按值设置,否则翻转(热键/无参调用)
    let param_on = |cur: bool| -> bool { if param == 0 || param == 1 { param == 1 } else { !cur } };
    if cmd >= CMD_SELECT_COLOR && cmd < CMD_SELECT_COLOR + 10 {
        let idx = cmd - CMD_SELECT_COLOR;
        if idx < COLOR_PRESETS.len() {
            state.draw.pen_r = COLOR_PRESETS[idx].0;
            state.draw.pen_g = COLOR_PRESETS[idx].1;
            state.draw.pen_b = COLOR_PRESETS[idx].2;
            state.draw.selected_color = idx;
            state.canvas.color = (
                (state.draw.pen_r * 255.0) as u8,
                (state.draw.pen_g * 255.0) as u8,
                (state.draw.pen_b * 255.0) as u8,
            );
            crate::export::glaspen2_save_settings(
                state.draw.pen_r,
                state.draw.pen_g,
                state.draw.pen_b,
                state.draw.width_scale,
            );
        }
    } else if cmd >= CMD_SELECT_WIDTH && cmd < CMD_SELECT_WIDTH + WIDTH_PRESETS.len() {
        let idx = cmd - CMD_SELECT_WIDTH;
        if idx < WIDTH_PRESETS.len() {
            state.draw.width_scale = WIDTH_PRESETS[idx];
            state.draw.selected_width = idx;
            crate::export::glaspen2_save_settings(
                state.draw.pen_r,
                state.draw.pen_g,
                state.draw.pen_b,
                state.draw.width_scale,
            );
        }
    } else {
        match cmd {
            x if x == CMD_SAVE_WITH_BG => {
                save_with_bg(state);
                hud_notify("截图成功(含背景)");
            }
            x if x == CMD_SAVE_DRAWING => {
                save_drawing(state);
                hud_notify("截图成功");
            }
            x if x == CMD_SAVE_XOJ => {
                crate::export::glaspen2_save_xoj();
                hud_notify("笔记已保存");
            }
            x if x == CMD_CLEAR_SCREEN => clear_screen(state),
            x if x == CMD_UNDO => undo_last_stroke(state),
            x if x == CMD_TOGGLE_RAINBOW => {
                state.draw.show_rainbow = !state.draw.show_rainbow;
                if state.draw.show_rainbow {
                    draw_rainbow_indicator(state);
                } else {
                    clear_screen(state);
                }
            }
            x if x == CMD_TOGGLE_OUTLINE => {
                let on = param_on(state.draw.outline_enabled);
                state.draw.outline_enabled = on;
                crate::runtime().block_on(crate::db::save_setting("outline_enabled", if on { "1" } else { "0" }));
            }
            x if x == CMD_TOGGLE_GRID => {
                let on = param_on(state.draw.show_grid);
                state.draw.show_grid = on;
                crate::runtime().block_on(crate::db::save_setting("grid", if on { "1" } else { "0" }));
                redraw_from_strokes(state);
            }
            x if x == CMD_TOGGLE_FROSTED => {
                let on = param_on(state.draw.frosted);
                state.draw.frosted = on;
                crate::runtime().block_on(crate::db::save_setting("frostedGlass", if on { "1" } else { "0" }));
                apply_frosted(state.canvas.hwnd, on);
            }
            x if x == CMD_TOGGLE_ETHEREAL => {
                let on = param_on(state.draw.ethereal);
                state.draw.ethereal = on;
                crate::runtime().block_on(crate::db::save_setting("ethereal", if on { "1" } else { "0" }));
                if on {
                    hide_strokes(state);
                    hud_notify("飘渺画布涂鸦模式 (悬空/落笔显示)");
                } else {
                    show_strokes(state);
                    hud_notify("固定画布涂鸦模式");
                }
            }
            x if x == CMD_TOGGLE_PRESSURE_MONITOR => {
                let on = param_on(state.draw.pressure_monitor);
                state.draw.pressure_monitor = on;
                crate::runtime().block_on(crate::db::save_setting(
                    "pressureMonitor",
                    if on { "1" } else { "0" },
                ));
                hud_toggle_pressure(on);
                hud_notify(if on { "压力监控已开启" } else { "压力监控已关闭" });
            }
            x if x == CMD_TOGGLE_OCR => {
                let on = param_on(state.draw.ocr_enabled);
                state.draw.ocr_enabled = on;
                crate::runtime().block_on(crate::db::save_setting(
                    "ocrEnabled",
                    if on { "1" } else { "0" },
                ));
                if on {
                    // 首次开启 OCR:后台确保模型存在(存在则立即就绪,缺失则下载)
                    hud_notify("正在准备 OCR 模型...");
                    let hwnd = state.canvas.hwnd.0 as isize;
                    std::thread::spawn(move || {
                        let r = crate::export::glaspen2_ocr_ensure_models();
                        if r == 0 {
                            return; // 模型已就绪
                        }
                        if r < 0 {
                            unsafe {
                                let _ = PostMessageW(
                                    Some(HWND(hwnd as *mut _)),
                                    WM_TRAY_COMMAND,
                                    WPARAM(CMD_OCR_PROGRESS),
                                    LPARAM(1),
                                );
                            }
                            return;
                        }
                        // 轮询下载进度并通知主线程
                        loop {
                            let p = crate::export::glaspen2_ocr_download_progress();
                            let param: usize = if p >= 100.0 {
                                2 // done
                            } else if p <= -2.0 {
                                1 // failed
                            } else if p >= 0.0 {
                                100 + p as usize // progress
                            } else {
                                0 // idle, keep waiting
                            };
                            if param == 2 || param == 1 {
                                unsafe {
                                    let _ = PostMessageW(
                                        Some(HWND(hwnd as *mut _)),
                                        WM_TRAY_COMMAND,
                                        WPARAM(CMD_OCR_PROGRESS),
                                        LPARAM(param as isize),
                                    );
                                }
                                break;
                            }
                            if param >= 100 {
                                unsafe {
                                    let _ = PostMessageW(
                                        Some(HWND(hwnd as *mut _)),
                                        WM_TRAY_COMMAND,
                                        WPARAM(CMD_OCR_PROGRESS),
                                        LPARAM(param as isize),
                                    );
                                }
                            }
                            std::thread::sleep(std::time::Duration::from_millis(500));
                        }
                    });
                }
            }
            x if x == CMD_OCR_PROGRESS => {
                match param {
                    1 => {
                        let err = crate::ocr::download::last_error();
                        if err.is_empty() {
                            hud_notify("OCR 模型下载失败");
                        } else {
                            hud_notify(&format!("OCR 模型下载失败: {}", err));
                        }
                    }
                    2 => hud_notify("OCR 模型下载完成"),
                    p if p >= 100 => hud_notify(&format!("下载 OCR 模型 {}%", p - 100)),
                    _ => {}
                }
            }
            x if x == CMD_NAVIGATE_TO_PAGE => {
                // 内容 tab 点击页面:恢复该页笔迹继续绘画
                navigate_to(state, param as i64, "已切换到该页面");
            }
            x if x == CMD_PAGE_PREV => navigate_page(state, false),
            x if x == CMD_PAGE_NEXT => navigate_page(state, true),
            x if x == CMD_EXPORT_SVG_GIF => export_svg_gif_clipboard(state),
            x if x == CMD_TOGGLE_ENABLED => toggle_enabled(state),
            x if x == CMD_QUIT => {
                unsafe { let _ = DestroyWindow(state.canvas.hwnd); }
            }
            _ => {}
        }
    }
}

fn clear_screen(state: &mut OverlayState) {
    state.pen_path.clear();
    state.in_stroke = false;
    let params = modeler_params();
    let _ = state.stroke_modeler.reset_w_params(params);
    state.start_time = Instant::now();
    state.canvas.clear();
    if state.draw.show_grid {
        state.canvas.draw_grid();
    }
    state.canvas.set_bg_alpha(BG_BLOCK);
    let created = crate::export::glaspen2_clear_strokes(state.canvas.w, state.canvas.h);
    if state.draw.show_rainbow {
        draw_rainbow_indicator(state);
    }
    if created == 0 {
        // 当前页从未涂鸦:不能连续创建空白画布(macOS 同款提示)
        hud_notify("不能连续创建空白画布,请先涂鸦");
    } else {
        hud_notify("已新建画布");
    }
}

fn toggle_enabled(state: &mut OverlayState) {
    state.draw.enabled = !state.draw.enabled;
    if !state.draw.enabled {
        // 立即恢复穿透
        set_input_blocking(state.canvas.hwnd, false);
    }
    hud_notify(if state.draw.enabled { "涂鸦已开启" } else { "涂鸦已关闭" });
}

fn undo_last_stroke(state: &mut OverlayState) {
    let remaining = crate::export::glaspen2_undo_last_stroke();
    if remaining < 0 {
        hud_notify("没有可撤销的笔画");
        return;
    }
    // 清空画布,从 STROKES 重绘全部剩余笔画
    redraw_from_strokes(state);
    hud_notify("已撤销上一笔");
}

/// 清空画布并从 STROKES 重绘全部笔画(撤销/翻页/网格开关后使用)
fn redraw_from_strokes(state: &mut OverlayState) {
    state.pen_path.clear();
    state.in_stroke = false;
    let params = modeler_params();
    let _ = state.stroke_modeler.reset_w_params(params);
    state.start_time = Instant::now();
    state.canvas.clear();
    if state.draw.show_grid {
        state.canvas.draw_grid();
    }
    let ol = if state.draw.outline_enabled { 1.0 } else { 0.0 };
    {
        let strokes = crate::STROKES.lock().unwrap();
        for s in strokes.iter() {
            if s.points.is_empty() {
                continue;
            }
            state.canvas.color = (
                (s.r * 255.0) as u8,
                (s.g * 255.0) as u8,
                (s.b * 255.0) as u8,
            );
            let path: Vec<(f32, f32, f32)> = s.points
                .iter()
                .map(|&(x, y, w, _)| (x as f32, y as f32, (w as f32 * 0.5).max(0.5)))
                .collect();
            fill_stroke_path(&mut state.canvas, &path, ol);
        }
    }
    state.canvas.set_bg_alpha(BG_BLOCK);
    state.canvas.present_all();
    if state.draw.show_rainbow {
        draw_rainbow_indicator(state);
    }
}

/// 上一页/下一页(加载目标页笔画并重绘)
fn navigate_page(state: &mut OverlayState, next: bool) {
    let target = if next {
        crate::export::glaspen2_next_screen_id()
    } else {
        crate::export::glaspen2_prev_screen_id()
    };
    navigate_to(state, target, if next { "下一页" } else { "上一页" });
}

/// 跳转到指定页面:加载该页笔画并重绘,可继续绘画
fn navigate_to(state: &mut OverlayState, target: i64, _label: &str) {
    let current = crate::export::glaspen2_get_current_screen_id();
    if target <= 0 || target == current {
        eprintln!("[overlay] 没有更多页面 (current={}, target={})", current, target);
        hud_notify("没有可跳转的页面");
        return;
    }
    let count = crate::export::glaspen2_load_strokes_for_screen(target);
    redraw_from_strokes(state);
    eprintln!("[overlay] 已切换到页面 {} ({} 笔)", target, count);
    hud_notify(&page_info_text(target));
    // 飘渺模式:短暂显示目标页,随后自动隐藏(除非笔在活动)
    if state.draw.ethereal {
        let hwnd = state.canvas.hwnd;
        let _ = unsafe { SetTimer(Some(hwnd), TIMER_PEEK, PEEK_DELAY_MS, None) };
    }
}

/// 页数+日期通知文本,如 "今天 第2页  第3/5页"(与 macOS 一致)
fn page_info_text(screen_id: i64) -> String {
    let ptr = crate::export::glaspen2_page_info_json(screen_id);
    if ptr.is_null() {
        return format!("第 {} 页", screen_id);
    }
    let s = unsafe { std::ffi::CStr::from_ptr(ptr) }.to_string_lossy().to_string();
    crate::export::glaspen2_free_c_string(ptr);
    let v: serde_json::Value = match serde_json::from_str(&s) {
        Ok(v) => v,
        Err(_) => return format!("第 {} 页", screen_id),
    };
    let nth = v["nth"].as_i64().unwrap_or(screen_id);
    let pos = v["pos"].as_i64().unwrap_or(0);
    let total = v["total"].as_i64().unwrap_or(0);
    let created = v["created"].as_f64().unwrap_or(0.0) as u64;
    let label = date_label(created);
    format!("{} 第{}页  第{}/{}页", label, nth, pos, total)
}

/// 飘渺模式:隐藏笔迹(保留数据,仅清空显示;网格按"网格跟随涂鸦"决定)
fn hide_strokes(state: &mut OverlayState) {
    // 提交未完成的笔画,避免悬空
    if state.in_stroke {
        crate::export::glaspen2_end_stroke();
        state.in_stroke = false;
        state.pen_path.clear();
    }
    state.strokes_visible = false;
    state.canvas.clear();
    // 网格跟随涂鸦 → 一起隐藏;否则网格保持可见
    if state.draw.show_grid && !state.draw.grid_follow_strokes {
        state.canvas.draw_grid();
    }
    state.canvas.set_bg_alpha(BG_BLOCK);
    state.canvas.present_all();
}

/// 飘渺模式:显示笔迹(从 STROKES 重绘)
fn show_strokes(state: &mut OverlayState) {
    if !state.strokes_visible {
        state.strokes_visible = true;
        redraw_from_strokes(state);
    }
}

/// Ctrl+Alt+G:导出 SVG + GIF,并把当前画布复制到系统剪贴板(CF_DIB)
fn export_svg_gif_clipboard(state: &mut OverlayState) {
    crate::export::glaspen2_save_svg();
    let ok = crate::export::glaspen2_save_animated_gif();
    eprintln!("[overlay] SVG 已导出;GIF 导出: {}", if ok != 0 { "OK" } else { "FAILED" });
    copy_canvas_to_clipboard(state);
    hud_notify("已导出 SVG + GIF,并复制到剪贴板");
}

/// 把当前画布(32bit BGRA 预乘)复制为 CF_DIB 到系统剪贴板
fn copy_canvas_to_clipboard(state: &mut OverlayState) {
    let w = state.canvas.w as usize;
    let h = state.canvas.h as usize;
    let row = (w * 4 + 3) & !3;
    let header = 40usize;
    let total = header + row * h;
    let mem = unsafe { GlobalAlloc(GMEM_MOVEABLE, total) };
    if mem.0.is_null() {
        return;
    }
    let ptr = unsafe { GlobalLock(mem) };
    if ptr.is_null() {
        unsafe { let _ = GlobalFree(mem); }
        return;
    }
    let snap = state.canvas.snapshot();
    unsafe {
        let p = ptr as *mut u8;
        // BITMAPINFOHEADER(40 字节,bottom-up 32bpp BGRA)
        std::slice::from_raw_parts_mut(p, 40).fill(0);
        std::slice::from_raw_parts_mut(p.add(0), 4).copy_from_slice(&40u32.to_le_bytes());
        std::slice::from_raw_parts_mut(p.add(4), 4).copy_from_slice(&(w as i32).to_le_bytes());
        std::slice::from_raw_parts_mut(p.add(8), 4).copy_from_slice(&(h as i32).to_le_bytes()); // 正高度 = bottom-up
        std::slice::from_raw_parts_mut(p.add(12), 2).copy_from_slice(&1u16.to_le_bytes()); // biPlanes
        std::slice::from_raw_parts_mut(p.add(14), 2).copy_from_slice(&32u16.to_le_bytes()); // biBitCount
        std::slice::from_raw_parts_mut(p.add(16), 4).copy_from_slice(&0u32.to_le_bytes()); // biCompression = BI_RGB
        // 像素:源为 top-down,写入 bottom-up(行反转)
        for y in 0..h {
            let src_off = y * w * 4;
            std::ptr::copy_nonoverlapping(
                snap.as_ptr().add(src_off),
                p.add(header + (h - 1 - y) * row),
                w * 4,
            );
        }
        let _ = GlobalUnlock(mem);
    }
    unsafe {
        if OpenClipboard(state.canvas.hwnd) != 0 {
            EmptyClipboard();
            let h = SetClipboardData(CF_DIB, mem);
            CloseClipboard();
            if h.0.is_null() {
                let _ = GlobalFree(mem);
            }
            eprintln!("[overlay] 画布已复制到剪贴板 ({}x{})", state.canvas.w, state.canvas.h);
        } else {
            let _ = GlobalFree(mem);
            eprintln!("[overlay] 剪贴板打开失败,未复制");
        }
    }
}

/// 应用/取消模糊背景(磨砂玻璃)。SetWindowCompositionAttribute 是
/// undocumented API,user32 导入库中没有,故用 libloading 动态加载。
/// 注:对 UpdateLayeredWindow 窗口,Windows 可能忽略该效果(与 ULW 合成冲突),
/// 调用不失败即可。
fn apply_frosted(hwnd: HWND, on: bool) {
    type FnSetWca = unsafe extern "system" fn(HWND, *mut WindowCompositionAttributeData) -> i32;
    let lib = match unsafe { libloading::Library::new("user32.dll") } {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[overlay] user32.dll 加载失败: {}", e);
            return;
        }
    };
    let Ok(f) = (unsafe { lib.get::<FnSetWca>(b"SetWindowCompositionAttribute") }) else {
        eprintln!("[overlay] 系统不支持 SetWindowCompositionAttribute");
        return;
    };
    let f: FnSetWca = *f;
    let mut accent = AccentPolicy {
        accent_state: if on { ACCENT_ENABLE_ACRYLICBLURBEHIND } else { 0 },
        flags: 0,
        color: 0,
        animation_id: 0,
    };
    let mut data = WindowCompositionAttributeData {
        attribute: WCA_ACCENT_POLICY,
        data: (&mut accent as *mut AccentPolicy).cast(),
        size: std::mem::size_of::<AccentPolicy>(),
    };
    let ret = unsafe { f(hwnd, &mut data) };
    eprintln!("[overlay] SetWindowCompositionAttribute(blur={}) -> {}", on, ret);
}

/// Ctrl+Alt+B:模糊背景(磨砂玻璃)开关
fn toggle_frosted(state: &mut OverlayState) {
    state.draw.frosted = !state.draw.frosted;
    crate::runtime().block_on(crate::db::save_setting(
        "frostedGlass",
        if state.draw.frosted { "1" } else { "0" },
    ));
    apply_frosted(state.canvas.hwnd, state.draw.frosted);
    eprintln!("[overlay] 模糊背景: {}", if state.draw.frosted { "开" } else { "关" });
    hud_notify(if state.draw.frosted { "模糊背景已开启" } else { "模糊背景已关闭" });
}

/// 显示分辨率/排列变化:重建画布并重绘已保存笔画
fn on_display_change(state: &mut OverlayState) {
    let hwnd = state.canvas.hwnd;
    let x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
    let w = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
    let h = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            w,
            h,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
    crate::export::glaspen2_on_display_change(w, h);
    let color = state.canvas.color;
    state.canvas = OverlayCanvas::create(hwnd);
    state.canvas.color = color;
    redraw_from_strokes(state);
}

fn draw_rainbow_indicator(state: &mut OverlayState) {
    for col in 0..14 {
        let h = col as f64 / 14.0;
        let (r, g, b) = hsv_to_rgb(h);
        let color = ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8);
        state.canvas.fill_rect(col as f32 * 2.0, 0.0, 2.0, 4.0, color);
    }
    state.canvas.present_all();
}

fn hsv_to_rgb(h: f64) -> (f64, f64, f64) {
    let i = (h * 6.0) as i32;
    let f = h * 6.0 - i as f64;
    let q = 1.0 - f;
    match i % 6 {
        0 => (1.0, f, 0.0),
        1 => (q, 1.0, 0.0),
        2 => (0.0, 1.0, f),
        3 => (0.0, q, 1.0),
        4 => (f, 0.0, 1.0),
        5 => (1.0, 0.0, q),
        _ => (0.0, 0.0, 0.0),
    }
}

// ── 保存导出 ──

fn save_drawing(state: &mut OverlayState) {
    let snap = state.canvas.snapshot();
    crate::export::glaspen2_save_drawing(snap.as_ptr(), state.canvas.w, state.canvas.h, state.canvas.w * 4);
}

fn save_with_bg(state: &mut OverlayState) {
    unsafe {
        let screen_dc = GetDC(None);
        let bw = state.canvas.w;
        let bh = state.canvas.h;
        let bg_dc = CreateCompatibleDC(Some(screen_dc));
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: bw,
                biHeight: -bh,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bg_bits: *mut std::ffi::c_void = ptr::null_mut();
        let bg_bmp = CreateDIBSection(Some(bg_dc), &bmi, DIB_RGB_COLORS, &mut bg_bits, None, 0).unwrap();
        let old = SelectObject(bg_dc, bg_bmp.into());
        let _ = BitBlt(bg_dc, 0, 0, bw, bh, Some(screen_dc), 0, 0, SRCCOPY);
        let snap = state.canvas.snapshot();
        crate::export::glaspen2_save_with_background(
            snap.as_ptr(),
            state.canvas.w,
            state.canvas.h,
            state.canvas.w * 4,
            bg_bits as *const u8,
            bw,
            bh,
            bw * 4,
        );
        let _ = SelectObject(bg_dc, old);
        let _ = DeleteObject(bg_bmp.into());
        let _ = DeleteDC(bg_dc);
        let _ = ReleaseDC(None, screen_dc);
    }
}

// ── 颜色/线宽匹配(设置管道用) ──

fn closest_color_index(r: f64, g: f64, b: f64) -> usize {
    let mut best = 0;
    let mut best_dist = f64::MAX;
    for (i, &(cr, cg, cb)) in COLOR_PRESETS.iter().enumerate() {
        let d = (r - cr).powi(2) + (g - cg).powi(2) + (b - cb).powi(2);
        if d < best_dist {
            best_dist = d;
            best = i;
        }
    }
    best
}

fn closest_width_index(w: f64) -> usize {
    let mut best = 0;
    let mut best_dist = f64::MAX;
    for (i, &ww) in WIDTH_PRESETS.iter().enumerate() {
        let d = (w - ww).powi(2);
        if d < best_dist {
            best_dist = d;
            best = i;
        }
    }
    best
}

// ── Settings Pipe Server(Flutter UI) ──

const PIPE_ACCESS_DUPLEX: u32 = 0x00000003;
const PIPE_TYPE_BYTE: u32 = 0x00000000;
const PIPE_READMODE_BYTE: u32 = 0x00000000;
const PIPE_WAIT: u32 = 0x00000000;
const PIPE_UNLIMITED_INSTANCES: u32 = 255;
const BUFFER_SIZE: u32 = 4096;

unsafe extern "system" {
    fn CreateNamedPipeW(
        lp_name: PCWSTR,
        dw_open_mode: u32,
        dw_pipe_mode: u32,
        n_max_instances: u32,
        n_out_buffer_size: u32,
        n_in_buffer_size: u32,
        n_default_time_out: u32,
        lp_security_attributes: *const std::ffi::c_void,
    ) -> isize;

    fn ConnectNamedPipe(h_named_pipe: isize, lp_overlapped: *mut std::ffi::c_void) -> i32;

    fn DisconnectNamedPipe(h_named_pipe: isize) -> i32;
}

// ── 剪贴板(CF_DIB 复制画布) ──

const CF_DIB: u32 = 8;
const GMEM_MOVEABLE: u32 = 0x0002;

#[link(name = "user32")]
unsafe extern "system" {
    fn OpenClipboard(hwnd: HWND) -> i32;
    fn EmptyClipboard() -> i32;
    fn SetClipboardData(uformat: u32, hmem: HANDLE) -> HANDLE;
    fn CloseClipboard() -> i32;
    fn GlobalAlloc(uflags: u32, dw_bytes: usize) -> HANDLE;
    fn GlobalFree(hmem: HANDLE) -> HANDLE;
    fn GlobalLock(hmem: HANDLE) -> *mut std::ffi::c_void;
    fn GlobalUnlock(hmem: HANDLE) -> i32;
}

// ── 模糊背景(SetWindowCompositionAttribute,Win10 1809+) ──

/// WCA_ACCENT_POLICY
const WCA_ACCENT_POLICY: i32 = 19;
/// ACCENT_ENABLE_ACRYLICBLURBEHIND(亚克力,模糊较轻);3 = BLURBEHIND 全屏高斯模糊(过糊)
const ACCENT_ENABLE_ACRYLICBLURBEHIND: i32 = 4;

#[repr(C)]
struct AccentPolicy {
    accent_state: i32,
    flags: i32,
    color: u32,
    animation_id: i32,
}

#[repr(C)]
struct WindowCompositionAttributeData {
    attribute: i32,
    data: *mut std::ffi::c_void,
    size: usize,
}

fn pipe_wide_name() -> Vec<u16> {
    OsStr::new(r"\\.\pipe\glaspen2_settings")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn run_settings_pipe_server(hwnd: isize) {
    let name = pipe_wide_name();

    loop {
        let pipe = unsafe {
            CreateNamedPipeW(
                PCWSTR(name.as_ptr()),
                PIPE_ACCESS_DUPLEX,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                PIPE_UNLIMITED_INSTANCES,
                BUFFER_SIZE,
                BUFFER_SIZE,
                0,
                std::ptr::null(),
            )
        };
        if pipe == -1 || pipe == 0 {
            eprintln!("[pipe] CreateNamedPipeW failed");
            std::thread::sleep(std::time::Duration::from_secs(2));
            continue;
        }

        // Block until a client connects
        let ok = unsafe { ConnectNamedPipe(pipe, std::ptr::null_mut()) };
        if ok == 0 {
            let err = std::io::Error::last_os_error();
            // ERROR_PIPE_CONNECTED (535) means client connected before ConnectNamedPipe
            if err.raw_os_error() != Some(535) {
                eprintln!("[pipe] ConnectNamedPipe error: {}", err);
                close_pipe(pipe);
                continue;
            }
        }
        eprintln!("[pipe] Flutter settings client connected");

        handle_pipe_client(pipe, hwnd);

        eprintln!("[pipe] Flutter settings client disconnected");
    }
}

fn close_pipe(pipe: isize) {
    unsafe {
        DisconnectNamedPipe(pipe);
        let _ = windows::Win32::Foundation::CloseHandle(HANDLE(pipe as *mut _));
    }
}

fn handle_pipe_client(pipe: isize, hwnd: isize) {
    use std::io::Read;
    use std::os::windows::io::{FromRawHandle, IntoRawHandle};

    // Wrap pipe HANDLE in a single File for both read and write
    let mut stream = unsafe {
        std::fs::File::from_raw_handle(pipe as *mut std::ffi::c_void)
    };

    let mut buf = [0u8; 4096];
    let mut line_buf = Vec::new();

    loop {
        let n = match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => break,
        };

        for &byte in &buf[..n] {
            if byte == b'\n' {
                if !line_buf.is_empty() {
                    let line = String::from_utf8_lossy(&line_buf).to_string();
                    process_pipe_message(&line, hwnd, &mut stream);
                    line_buf.clear();
                }
            } else {
                line_buf.push(byte);
            }
        }
    }

    // Prevent File from closing the handle; we close it ourselves
    let _ = stream.into_raw_handle();
    close_pipe(pipe);
}

fn process_pipe_message(line: &str, hwnd: isize, writer: &mut std::fs::File) {
    use std::io::Write;

    let msg_type = json_get_str(line, "type");

    if msg_type == "listPages" {
        // 页面列表(内容 tab)
        let req_id = json_get_i64(line, "reqId").unwrap_or(0);
        let ptr = crate::export::glaspen2_list_screens_json();
        if ptr.is_null() {
            let _ = writer.write_all(
                format!("{{\"type\":\"listPages_response\",\"reqId\":{},\"data\":[]}}\n", req_id).as_bytes(),
            );
        } else {
            let s = unsafe { std::ffi::CStr::from_ptr(ptr) }.to_string_lossy().to_string();
            crate::export::glaspen2_free_c_string(ptr);
            let resp = format!("{{\"type\":\"listPages_response\",\"reqId\":{},\"data\":{}}}\n", req_id, s);
            let _ = writer.write_all(resp.as_bytes());
        }
        let _ = writer.flush();
    } else if msg_type == "getPageThumbnail" {
        // 页面缩略图(PNG,base64 编码)
        let screen_id = json_get_i64(line, "screenId").unwrap_or(0);
        let w = json_get_i64(line, "w").unwrap_or(0) as i32;
        let h = json_get_i64(line, "h").unwrap_or(0) as i32;
        let max_size = json_get_i64(line, "maxSize").unwrap_or(280) as i32;
        let req_id = json_get_i64(line, "reqId").unwrap_or(0);
        let mut out_len: i32 = 0;
        let ptr = crate::export::glaspen2_render_thumbnail(screen_id, w, h, max_size, &mut out_len);
        if ptr.is_null() || out_len <= 0 {
            let _ = writer.write_all(
                format!("{{\"type\":\"getPageThumbnail_response\",\"reqId\":{},\"data\":{{\"png\":\"\"}}}}\n", req_id).as_bytes(),
            );
        } else {
            let bytes = unsafe { std::slice::from_raw_parts(ptr, out_len as usize) };
            let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes);
            crate::export::glaspen2_free_rust_bytes(ptr, out_len);
            let resp = format!("{{\"type\":\"getPageThumbnail_response\",\"reqId\":{},\"data\":{{\"png\":\"{}\"}}}}\n", req_id, b64);
            let _ = writer.write_all(resp.as_bytes());
        }
        let _ = writer.flush();
    } else if msg_type == "searchText" {
        // OCR 文本搜索(内容 tab)
        let req_id = json_get_i64(line, "reqId").unwrap_or(0);
        let query = json_get_str(line, "query");
        let q = std::ffi::CString::new(query).unwrap_or_default();
        let ptr = crate::export::glaspen2_search_ocr_json(q.as_ptr());
        if ptr.is_null() {
            let _ = writer.write_all(
                format!("{{\"type\":\"searchText_response\",\"reqId\":{},\"data\":[]}}\n", req_id).as_bytes(),
            );
        } else {
            let s = unsafe { std::ffi::CStr::from_ptr(ptr) }.to_string_lossy().to_string();
            crate::export::glaspen2_free_c_string(ptr);
            let resp = format!("{{\"type\":\"searchText_response\",\"reqId\":{},\"data\":{}}}\n", req_id, s);
            let _ = writer.write_all(resp.as_bytes());
        }
        let _ = writer.flush();
    } else if msg_type == "hotkey" {
        // Flutter 快捷键按钮:转发为对应命令
        let key = json_get_str(line, "key");
        let cmd = match key {
            "Q" => Some(CMD_QUIT),
            "G" => Some(CMD_EXPORT_SVG_GIF),
            "J" => Some(CMD_PAGE_PREV),
            "K" => Some(CMD_PAGE_NEXT),
            "Z" => Some(CMD_UNDO),
            "X" => Some(CMD_TOGGLE_ETHEREAL),
            "C" => Some(CMD_CLEAR_SCREEN),
            "V" => Some(CMD_TOGGLE_ENABLED),
            "B" => Some(CMD_TOGGLE_FROSTED),
            _ => None,
        };
        if let Some(cmd) = cmd {
            // param = usize::MAX:handle_command 的开关类命令按"切换"处理
            // (与键盘 Ctrl+Alt+键 行为一致,而不是被 0 强制关闭)
            let _ = unsafe {
                PostMessageW(
                    Some(HWND(hwnd as *mut _)),
                    WM_TRAY_COMMAND,
                    WPARAM(cmd),
                    LPARAM(usize::MAX as isize),
                )
            };
        }
    } else if msg_type == "navigateToPage" {
        // 内容 tab 点击页面 → 恢复该页笔迹(指定页面跳转)
        let screen_id = json_get_i64(line, "screenId").unwrap_or(0);
        if screen_id > 0 {
            let _ = unsafe {
                PostMessageW(
                    Some(HWND(hwnd as *mut _)),
                    WM_TRAY_COMMAND,
                    WPARAM(CMD_NAVIGATE_TO_PAGE),
                    LPARAM(screen_id as isize),
                )
            };
        }
    } else if msg_type == "getSettings" {
        // Respond with current settings from DB
        let (r, g, b, w) = crate::runtime().block_on(crate::db::load_settings()).unwrap_or((1.0, 0.0, 0.0, 1.0));
        let color = closest_color_index(r, g, b);
        let width = closest_width_index(w);
        let outline = crate::runtime().block_on(crate::db::load_setting("outline_enabled"))
            .and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
        let grid = crate::runtime().block_on(crate::db::load_setting("grid"))
            .and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
        let frosted = crate::runtime().block_on(crate::db::load_setting("frostedGlass"))
            .and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
        let pressure_monitor = crate::runtime().block_on(crate::db::load_setting("pressureMonitor"))
            .and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
        let grid_follow = crate::runtime().block_on(crate::db::load_setting("gridFollowStrokes"))
            .and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
        let ocr_enabled = crate::runtime().block_on(crate::db::load_setting("ocrEnabled"))
            .and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
        let ethereal = crate::runtime().block_on(crate::db::load_setting("ethereal"))
            .and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
        let resp = format!(
            "{{\"type\":\"getSettings_response\",\"data\":{{\"color\":{},\"width\":{},\"outline\":{},\"grid\":{},\"gridFollowStrokes\":{},\"frostedGlass\":{},\"pressureMonitor\":{},\"ocrEnabled\":{},\"ethereal\":{},\"rainbow\":false,\"launchAtLogin\":false}}}}\n",
            color, width, outline, grid, grid_follow, frosted, pressure_monitor, ocr_enabled, ethereal
        );
        let _ = writer.write_all(resp.as_bytes());
        let _ = writer.flush();
    } else if msg_type == "setSetting" {
        let key = json_get_str(line, "key");
        if key == "save_drawing" {
            let _ = unsafe {
                PostMessageW(Some(HWND(hwnd as *mut _)), WM_TRAY_COMMAND, WPARAM(CMD_SAVE_DRAWING), LPARAM(0))
            };
        } else if key == "save_with_bg" {
            let _ = unsafe {
                PostMessageW(Some(HWND(hwnd as *mut _)), WM_TRAY_COMMAND, WPARAM(CMD_SAVE_WITH_BG), LPARAM(0))
            };
        } else if key == "save_xoj" {
            let _ = unsafe {
                PostMessageW(Some(HWND(hwnd as *mut _)), WM_TRAY_COMMAND, WPARAM(CMD_SAVE_XOJ), LPARAM(0))
            };
        } else if key == "undo" {
            let _ = unsafe {
                PostMessageW(
                    Some(HWND(hwnd as *mut _)),
                    WM_TRAY_COMMAND,
                    WPARAM(CMD_UNDO),
                    LPARAM(0),
                )
            };
        } else if key == "export_animated_gif" {
            let result = crate::export::glaspen2_save_animated_gif();
            eprintln!("[pipe] animated GIF export: {}", if result != 0 { "OK" } else { "FAILED" });
            hud_notify(if result != 0 { "动画 GIF 已保存到桌面" } else { "动画 GIF 导出失败" });
        } else if key == "export_pdf" {
            let result = crate::export::glaspen2_export_pdf();
            eprintln!("[pipe] PDF export: {}", if result != 0 { "OK" } else { "FAILED" });
            hud_notify(if result != 0 { "PDF 已保存到桌面" } else { "PDF 导出失败" });
        } else if key == "color" {
            if let Some(val) = json_get_i64(line, "value") {
                let idx = val as usize;
                let cmd = CMD_SELECT_COLOR + idx;
                let _ = unsafe {
                    PostMessageW(Some(HWND(hwnd as *mut _)), WM_TRAY_COMMAND, WPARAM(cmd), LPARAM(0))
                };
            }
        } else if key == "width" {
            if let Some(val) = json_get_i64(line, "value") {
                let idx = val as usize;
                let cmd = CMD_SELECT_WIDTH + idx;
                let _ = unsafe {
                    PostMessageW(Some(HWND(hwnd as *mut _)), WM_TRAY_COMMAND, WPARAM(cmd), LPARAM(0))
                };
            }
        } else if key == "outline" {
            if let Some(on) = json_get_bool(line, "value") {
                let cmd = CMD_TOGGLE_OUTLINE;
                let _ = unsafe {
                    PostMessageW(Some(HWND(hwnd as *mut _)), WM_TRAY_COMMAND, WPARAM(cmd), LPARAM(if on { 1 } else { 0 }))
                };
            }
        } else if key == "grid" {
            if let Some(on) = json_get_bool(line, "value") {
                let cmd = CMD_TOGGLE_GRID;
                let _ = unsafe {
                    PostMessageW(Some(HWND(hwnd as *mut _)), WM_TRAY_COMMAND, WPARAM(cmd), LPARAM(if on { 1 } else { 0 }))
                };
            }
        } else if key == "frostedGlass" {
            if let Some(on) = json_get_bool(line, "value") {
                let cmd = CMD_TOGGLE_FROSTED;
                let _ = unsafe {
                    PostMessageW(Some(HWND(hwnd as *mut _)), WM_TRAY_COMMAND, WPARAM(cmd), LPARAM(if on { 1 } else { 0 }))
                };
            }
        } else if key == "gridFollowStrokes" {
            if let Some(on) = json_get_bool(line, "value") {
                crate::runtime().block_on(crate::db::save_setting(
                    "gridFollowStrokes",
                    if on { "1" } else { "0" },
                ));
            }
        } else if key == "ocrEnabled" {
            if let Some(on) = json_get_bool(line, "value") {
                let cmd = CMD_TOGGLE_OCR;
                let _ = unsafe {
                    PostMessageW(Some(HWND(hwnd as *mut _)), WM_TRAY_COMMAND, WPARAM(cmd), LPARAM(if on { 1 } else { 0 }))
                };
            }
        } else if key == "pressureMonitor" {
            if let Some(on) = json_get_bool(line, "value") {
                let cmd = CMD_TOGGLE_PRESSURE_MONITOR;
                let _ = unsafe {
                    PostMessageW(Some(HWND(hwnd as *mut _)), WM_TRAY_COMMAND, WPARAM(cmd), LPARAM(if on { 1 } else { 0 }))
                };
            }
        }
    }
}

// ── Minimal JSON helpers ──

fn json_get_str<'a>(json: &'a str, key: &str) -> &'a str {
    let pattern = format!("\"{}\":\"", key);
    if let Some(start) = json.find(&pattern) {
        let val_start = start + pattern.len();
        if let Some(end) = json[val_start..].find('"') {
            return &json[val_start..val_start + end];
        }
    }
    ""
}

fn json_get_i64(json: &str, key: &str) -> Option<i64> {
    let pattern = format!("\"{}\":", key);
    if let Some(start) = json.find(&pattern) {
        let val_start = start + pattern.len();
        let rest = &json[val_start..].trim_start();
        let end = rest.find(|c: char| !c.is_ascii_digit() && c != '-').unwrap_or(rest.len());
        if end > 0 {
            return rest[..end].parse::<i64>().ok();
        }
    }
    None
}

/// 解析 JSON bool 值(Flutter 发送的开关为 true/false)
fn json_get_bool(json: &str, key: &str) -> Option<bool> {
    let pattern = format!("\"{}\":", key);
    let start = json.find(&pattern)?;
    let rest = json[start + pattern.len()..].trim_start();
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}
