use std::cell::UnsafeCell;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Format { ARGB32 }

pub struct ImageSurface {
    data: UnsafeCell<Vec<u8>>,
    w: i32,
    h: i32,
    stride: i32,
}

unsafe impl Send for ImageSurface {}
unsafe impl Sync for ImageSurface {}

impl ImageSurface {
    pub fn create(_format: Format, width: i32, height: i32) -> Result<Self, ()> {
        let stride = ((width as usize * 4 + 3) & !3) as i32;
        let data = vec![0u8; stride as usize * height as usize];
        Ok(Self { data: UnsafeCell::new(data), w: width, h: height, stride })
    }
    pub fn width(&self) -> i32 { self.w }
    pub fn height(&self) -> i32 { self.h }
    pub fn stride(&self) -> i32 { self.stride }
    pub fn data(&self) -> Result<&[u8], ()> {
        Ok(unsafe { &*self.data.get() })
    }
    pub fn pixels_mut(&self) -> &mut Vec<u8> {
        unsafe { &mut *self.data.get() }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum LineCap { Butt, Round, Square }
#[derive(Clone, Copy, PartialEq)]
pub enum LineJoin { Miter, Round, Bevel }
#[derive(Clone, Copy, PartialEq)]
pub enum Operator { Clear, Over }
#[derive(Clone, Copy)]
pub enum FontSlant { Normal, Italic, Oblique }
#[derive(Clone, Copy)]
pub enum FontWeight { Normal, Bold }

pub struct TextExtents {
    xb: f64, yb: f64, w: f64, h: f64, xa: f64, ya: f64,
}
impl TextExtents {
    pub fn x_bearing(&self) -> f64 { self.xb }
    pub fn y_bearing(&self) -> f64 { self.yb }
    pub fn width(&self) -> f64 { self.w }
    pub fn height(&self) -> f64 { self.h }
    pub fn x_advance(&self) -> f64 { self.xa }
    pub fn y_advance(&self) -> f64 { self.ya }
}

#[derive(Clone, Copy)]
struct Pt { x: f64, y: f64 }

pub struct Context<'a> {
    surface: &'a ImageSurface,
    st: UnsafeCell<CState>,
}

struct CState {
    r: f64, g: f64, b: f64, a: f64,
    lw: f64,
    cap: LineCap,
    join: LineJoin,
    op: Operator,
    sub: Vec<Vec<Pt>>,
    cur: usize,
    pos: Pt,
    has_pos: bool,
}

impl<'a> Context<'a> {
    pub fn new(surface: &'a ImageSurface) -> Result<Self, ()> {
        Ok(Self {
            surface,
            st: UnsafeCell::new(CState {
                r: 0.0, g: 0.0, b: 0.0, a: 1.0,
                lw: 1.0, cap: LineCap::Butt, join: LineJoin::Round,
                op: Operator::Over,
                sub: Vec::new(), cur: 0,
                pos: Pt { x: 0.0, y: 0.0 }, has_pos: false,
            }),
        })
    }

    fn st(&self) -> &mut CState { unsafe { &mut *self.st.get() } }

    pub fn set_source_rgba(&self, r: f64, g: f64, b: f64, a: f64) {
        let s = self.st(); s.r = r; s.g = g; s.b = b; s.a = a;
    }
    pub fn set_line_width(&self, w: f64) { self.st().lw = w; }
    pub fn set_line_cap(&self, cap: LineCap) { self.st().cap = cap; }
    pub fn set_line_join(&self, j: LineJoin) { self.st().join = j; }
    pub fn set_operator(&self, op: Operator) { self.st().op = op; }

    pub fn move_to(&self, x: f64, y: f64) {
        let s = self.st();
        s.sub.push(vec![Pt { x, y }]);
        s.cur = s.sub.len() - 1;
        s.pos = Pt { x, y };
        s.has_pos = true;
    }

    pub fn line_to(&self, x: f64, y: f64) {
        let s = self.st();
        if !s.has_pos {
            self.move_to(x, y);
            return;
        }
        if s.sub.is_empty() {
            s.sub.push(vec![s.pos]);
            s.cur = s.sub.len() - 1;
        }
        s.sub[s.cur].push(Pt { x, y });
        s.pos = Pt { x, y };
    }

    pub fn arc(&self, cx: f64, cy: f64, r: f64, start: f64, end: f64) {
        let steps = 48;
        let da = (end - start) / steps as f64;
        for i in 0..=steps {
            let a = start + da * i as f64;
            let x = cx + r * a.cos();
            let y = cy + r * a.sin();
            if i == 0 { self.move_to(x, y); }
            else { self.line_to(x, y); }
        }
    }

    pub fn rectangle(&self, x: f64, y: f64, w: f64, h: f64) {
        self.move_to(x, y);
        self.line_to(x + w, y);
        self.line_to(x + w, y + h);
        self.line_to(x, y + h);
        self.line_to(x, y);
    }

    pub fn stroke(&self) -> Result<(), ()> {
        let s = self.st();
        let lw = s.lw;
        let cap = s.cap;
        let (cr, cg, cb, ca) = (s.r, s.g, s.b, s.a);
        let subs: Vec<Vec<Pt>> = s.sub.clone();
        let px = self.surface.pixels_mut();
        let stride = self.surface.stride() as usize;
        let sw = self.surface.w;
        let sh = self.surface.h;

        for sub in &subs {
            if sub.len() < 2 { continue; }
            for seg in sub.windows(2) {
                draw_aa_line(px, stride, sw, sh, seg[0].x, seg[0].y, seg[1].x, seg[1].y, lw, cr, cg, cb, ca);
            }
            if cap == LineCap::Round {
                let first = sub[0];
                let last = sub[sub.len() - 1];
                draw_filled_circle(px, stride, sw, sh, first.x, first.y, lw * 0.5, cr, cg, cb, ca);
                if sub.len() > 2 || (first.x - last.x).abs() > 0.01 || (first.y - last.y).abs() > 0.01 {
                    draw_filled_circle(px, stride, sw, sh, last.x, last.y, lw * 0.5, cr, cg, cb, ca);
                }
            }
        }
        self.st().sub.clear();
        Ok(())
    }

    pub fn fill(&self) -> Result<(), ()> {
        let s = self.st();
        let (cr, cg, cb, ca) = (s.r, s.g, s.b, s.a);
        let subs: Vec<Vec<Pt>> = s.sub.clone();
        let px = self.surface.pixels_mut();
        let stride = self.surface.stride() as usize;
        let sw = self.surface.w;
        let sh = self.surface.h;

        for sub in &subs {
            if sub.is_empty() { continue; }
            if sub.len() == 1 {
                let p = sub[0];
                let radius = s.lw * 0.5;
                draw_filled_circle(px, stride, sw, sh, p.x, p.y, radius, cr, cg, cb, ca);
            } else {
                fill_polygon(px, stride, sw, sh, sub, cr, cg, cb, ca);
            }
        }
        self.st().sub.clear();
        Ok(())
    }

    pub fn paint(&self) -> Result<(), ()> {
        let s = self.st();
        let px = self.surface.pixels_mut();
        let stride = self.surface.stride() as usize;
        let sw = self.surface.w as usize;
        let sh = self.surface.h as usize;
        match s.op {
            Operator::Clear => {
                for b in px.iter_mut() { *b = 0; }
            }
            Operator::Over => {
                let (cr, cg, cb, ca) = (s.r, s.g, s.b, s.a);
                for y in 0..sh {
                    for x in 0..sw {
                        let off = y * stride + x * 4;
                        if off + 3 < px.len() {
                            blend_over(&mut px[off..off+4], cr, cg, cb, ca);
                        }
                    }
                }
            }
        }
        self.st().sub.clear();
        Ok(())
    }

    pub fn select_font_face(&self, _n: &str, _s: FontSlant, _w: FontWeight) {}
    pub fn set_font_size(&self, _s: f64) {}
    pub fn text_extents(&self, text: &str) -> TextExtents {
        let w = text.len() as f64 * 9.0;
        TextExtents { xb: 0.0, yb: 0.0, w, h: 16.0, xa: w, ya: 16.0 }
    }
    pub fn show_text(&self, text: &str) -> Result<(), ()> {
        // Render simple bitmap text
        let s = self.st();
        let (cr, cg, cb, ca) = (s.r, s.g, s.b, s.a);
        let px = self.surface.pixels_mut();
        let stride = self.surface.stride() as usize;
        let sw = self.surface.w;
        let sh = self.surface.h;
        let chars: Vec<char> = text.chars().collect();
        let mut cur_x = s.pos.x;
        let cur_y = s.pos.y;
        for ch in &chars {
            render_glyph(px, stride, sw, sh, *ch, cur_x, cur_y, 12.0, cr, cg, cb, ca);
            cur_x += 9.0;
        }
        Ok(())
    }
    pub fn set_source_surface(&self, _s: &ImageSurface, _x: f64, _y: f64) -> Result<(), ()> { Ok(()) }
}

fn blend_over(dst: &mut [u8], sr: f64, sg: f64, sb: f64, sa: f64) {
    let sa255 = (sa * 255.0) as i32;
    let inv_a = 255 - sa255;
    let da = dst[3] as i32;
    let out_a = sa255 + da * inv_a / 255;
    if out_a < 1 { return; }
    let out_r = (sr * sa * 255.0) as i32 + dst[2] as i32 * inv_a / 255;
    let out_g = (sg * sa * 255.0) as i32 + dst[1] as i32 * inv_a / 255;
    let out_b = (sb * sa * 255.0) as i32 + dst[0] as i32 * inv_a / 255;
    dst[0] = (out_b * 255 / out_a).min(255).max(0) as u8;
    dst[1] = (out_g * 255 / out_a).min(255).max(0) as u8;
    dst[2] = (out_r * 255 / out_a).min(255).max(0) as u8;
    dst[3] = out_a.min(255).max(0) as u8;
}

fn dist_sq_point_to_seg(px: f64, py: f64, x0: f64, y0: f64, x1: f64, y1: f64) -> f64 {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-10 {
        return (px - x0).powi(2) + (py - y0).powi(2);
    }
    let t = ((px - x0) * dx + (py - y0) * dy / len_sq).max(0.0).min(1.0);
    let proj_x = x0 + t * dx;
    let proj_y = y0 + t * dy;
    (px - proj_x).powi(2) + (py - proj_y).powi(2)
}

fn draw_aa_line(px: &mut [u8], stride: usize, sw: i32, sh: i32,
                 x0: f64, y0: f64, x1: f64, y1: f64,
                 width: f64, r: f64, g: f64, b: f64, a: f64) {
    let half_w = width * 0.5;
    let hw_inner = (half_w - 0.5).max(0.0);
    let hw_outer = half_w + 0.5;
    let hw_inner_sq = hw_inner * hw_inner;
    let hw_outer_sq = hw_outer * hw_outer;
    let x_min = (x0.min(x1) - hw_outer - 1.0).floor() as i32;
    let x_max = (x0.max(x1) + hw_outer + 1.0).ceil() as i32;
    let y_min = (y0.min(y1) - hw_outer - 1.0).floor() as i32;
    let y_max = (y0.max(y1) + hw_outer + 1.0).ceil() as i32;

    for py in y_min.max(0)..=y_max.min(sh - 1) {
        for px_x in x_min.max(0)..=x_max.min(sw - 1) {
            let dsq = dist_sq_point_to_seg(px_x as f64 + 0.5, py as f64 + 0.5, x0, y0, x1, y1);
            let alpha = if dsq < hw_inner_sq {
                a
            } else if dsq < hw_outer_sq {
                let dist = dsq.sqrt();
                a * (0.5 + half_w - dist)
            } else {
                continue;
            };
            if alpha < 1.0 / 255.0 { continue; }
            let off = py as usize * stride + px_x as usize * 4;
            if off + 3 < px.len() {
                blend_over(&mut px[off..off+4], r, g, b, alpha);
            }
        }
    }
}

fn draw_filled_circle(px: &mut [u8], stride: usize, sw: i32, sh: i32,
                       cx: f64, cy: f64, radius: f64,
                       r: f64, g: f64, b: f64, a: f64) {
    if radius < 0.5 { return; }
    let r_inner = (radius - 0.5).max(0.0);
    let r_outer = radius + 0.5;
    let r_inner_sq = r_inner * r_inner;
    let r_outer_sq = r_outer * r_outer;
    let x_min = (cx - r_outer - 1.0).floor() as i32;
    let x_max = (cx + r_outer + 1.0).ceil() as i32;
    let y_min = (cy - r_outer - 1.0).floor() as i32;
    let y_max = (cy + r_outer + 1.0).ceil() as i32;

    for py in y_min.max(0)..=y_max.min(sh - 1) {
        for px_x in x_min.max(0)..=x_max.min(sw - 1) {
            let dx = px_x as f64 + 0.5 - cx;
            let dy = py as f64 + 0.5 - cy;
            let dsq = dx * dx + dy * dy;
            let alpha = if dsq < r_inner_sq {
                a
            } else if dsq < r_outer_sq {
                a * (0.5 + radius - dsq.sqrt())
            } else {
                continue;
            };
            if alpha < 1.0 / 255.0 { continue; }
            let off = py as usize * stride + px_x as usize * 4;
            if off + 3 < px.len() {
                blend_over(&mut px[off..off+4], r, g, b, alpha);
            }
        }
    }
}

fn fill_polygon(px: &mut [u8], stride: usize, sw: i32, sh: i32,
                pts: &[Pt], r: f64, g: f64, b: f64, a: f64) {
    if pts.len() < 3 { return; }
    let y_min = pts.iter().map(|p| p.y as i32).min().unwrap().max(0);
    let y_max = pts.iter().map(|p| p.y as i32).max().unwrap().min(sh - 1);

    for y in y_min..=y_max {
        let mut intersections: Vec<f64> = Vec::new();
        let n = pts.len();
        for i in 0..n {
            let j = (i + 1) % n;
            let yi = pts[i].y;
            let yj = pts[j].y;
            if (yi <= y as f64 && yj > y as f64) || (yj <= y as f64 && yi > y as f64) {
                let t = (y as f64 - yi) / (yj - yi);
                intersections.push(pts[i].x + t * (pts[j].x - pts[i].x));
            }
        }
        intersections.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for pair in intersections.windows(2) {
            let x0 = pair[0].ceil() as i32;
            let x1 = pair[1].floor() as i32;
            for x in x0.max(0)..=x1.min(sw - 1) {
                let off = y as usize * stride + x as usize * 4;
                if off + 3 < px.len() {
                    blend_over(&mut px[off..off+4], r, g, b, a);
                }
            }
        }
    }
}

fn render_glyph(px: &mut [u8], stride: usize, sw: i32, sh: i32,
                 ch: char, x: f64, y: f64, size: f64,
                 r: f64, g: f64, b: f64, a: f64) {
    let bitmap = glyph_bitmap(ch);
    let scale = size / 16.0;
    for (row, &bits) in bitmap.iter().enumerate() {
        for col in 0..8 {
            if bits & (0x80 >> col) != 0 {
                let px_x = x as i32 + (col as f64 * scale) as i32;
                let px_y = y as i32 + (row as f64 * scale) as i32;
                let sw_r = (scale.ceil() as i32).max(1);
                for dy in 0..sw_r {
                    for dx in 0..sw_r {
                        let xx = px_x + dx;
                        let yy = px_y + dy;
                        if xx >= 0 && xx < sw && yy >= 0 && yy < sh {
                            let off = yy as usize * stride + xx as usize * 4;
                            if off + 3 < px.len() {
                                blend_over(&mut px[off..off+4], r, g, b, a);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn glyph_bitmap(ch: char) -> &'static [u8; 16] {
    static SPACE: [u8; 16] = [0; 16];
    static A: [u8; 16] = [
        0x00, 0x00, 0x18, 0x3C, 0x66, 0x66, 0x7E, 0x66,
        0x66, 0x66, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static B: [u8; 16] = [
        0x00, 0x00, 0x7C, 0x66, 0x66, 0x7C, 0x66, 0x66,
        0x66, 0x7C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static C: [u8; 16] = [
        0x00, 0x00, 0x3C, 0x66, 0x60, 0x60, 0x60, 0x60,
        0x66, 0x3C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static D: [u8; 16] = [
        0x00, 0x00, 0x78, 0x6C, 0x66, 0x66, 0x66, 0x66,
        0x6C, 0x78, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static E: [u8; 16] = [
        0x00, 0x00, 0x7E, 0x60, 0x60, 0x7C, 0x60, 0x60,
        0x60, 0x7E, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static F: [u8; 16] = [
        0x00, 0x00, 0x7E, 0x60, 0x60, 0x7C, 0x60, 0x60,
        0x60, 0x60, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static G: [u8; 16] = [
        0x00, 0x00, 0x3C, 0x66, 0x60, 0x60, 0x6E, 0x66,
        0x66, 0x3C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static H: [u8; 16] = [
        0x00, 0x00, 0x66, 0x66, 0x66, 0x7E, 0x66, 0x66,
        0x66, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static I: [u8; 16] = [
        0x00, 0x00, 0x3C, 0x18, 0x18, 0x18, 0x18, 0x18,
        0x18, 0x3C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static J: [u8; 16] = [
        0x00, 0x00, 0x0E, 0x06, 0x06, 0x06, 0x06, 0x66,
        0x66, 0x3C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static K: [u8; 16] = [
        0x00, 0x00, 0x66, 0x6C, 0x78, 0x70, 0x78, 0x6C,
        0x66, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static L: [u8; 16] = [
        0x00, 0x00, 0x60, 0x60, 0x60, 0x60, 0x60, 0x60,
        0x60, 0x7E, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static M: [u8; 16] = [
        0x00, 0x00, 0x63, 0x77, 0x7F, 0x6B, 0x63, 0x63,
        0x63, 0x63, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static N: [u8; 16] = [
        0x00, 0x00, 0x66, 0x76, 0x7E, 0x7E, 0x6E, 0x66,
        0x66, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static O: [u8; 16] = [
        0x00, 0x00, 0x3C, 0x66, 0x66, 0x66, 0x66, 0x66,
        0x66, 0x3C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static P: [u8; 16] = [
        0x00, 0x00, 0x7C, 0x66, 0x66, 0x7C, 0x60, 0x60,
        0x60, 0x60, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static Q: [u8; 16] = [
        0x00, 0x00, 0x3C, 0x66, 0x66, 0x66, 0x66, 0x6B,
        0x6E, 0x3C, 0x06, 0x03, 0x00, 0x00, 0x00, 0x00,
    ];
    static R: [u8; 16] = [
        0x00, 0x00, 0x7C, 0x66, 0x66, 0x7C, 0x6C, 0x66,
        0x66, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static S: [u8; 16] = [
        0x00, 0x00, 0x3C, 0x66, 0x60, 0x3C, 0x06, 0x06,
        0x66, 0x3C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static T: [u8; 16] = [
        0x00, 0x00, 0x7E, 0x18, 0x18, 0x18, 0x18, 0x18,
        0x18, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static U: [u8; 16] = [
        0x00, 0x00, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
        0x66, 0x3C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static V: [u8; 16] = [
        0x00, 0x00, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C,
        0x3C, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static W: [u8; 16] = [
        0x00, 0x00, 0x63, 0x63, 0x63, 0x6B, 0x7F, 0x77,
        0x63, 0x63, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static X_Y: [u8; 16] = [
        0x00, 0x00, 0x66, 0x66, 0x3C, 0x18, 0x18, 0x3C,
        0x66, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static Z: [u8; 16] = [
        0x00, 0x00, 0x7E, 0x06, 0x0C, 0x18, 0x30, 0x60,
        0x60, 0x7E, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static N0: [u8; 16] = [
        0x00, 0x00, 0x3C, 0x66, 0x6E, 0x76, 0x66, 0x66,
        0x66, 0x3C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static N1: [u8; 16] = [
        0x00, 0x00, 0x18, 0x38, 0x18, 0x18, 0x18, 0x18,
        0x18, 0x7E, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static N2: [u8; 16] = [
        0x00, 0x00, 0x3C, 0x66, 0x06, 0x0C, 0x18, 0x30,
        0x60, 0x7E, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static N3: [u8; 16] = [
        0x00, 0x00, 0x3C, 0x66, 0x06, 0x1C, 0x06, 0x06,
        0x66, 0x3C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static N4: [u8; 16] = [
        0x00, 0x00, 0x0C, 0x1C, 0x3C, 0x6C, 0x7E, 0x0C,
        0x0C, 0x0C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static N5: [u8; 16] = [
        0x00, 0x00, 0x7E, 0x60, 0x7C, 0x06, 0x06, 0x06,
        0x66, 0x3C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static N6: [u8; 16] = [
        0x00, 0x00, 0x3C, 0x60, 0x60, 0x7C, 0x66, 0x66,
        0x66, 0x3C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static N7: [u8; 16] = [
        0x00, 0x00, 0x7E, 0x06, 0x0C, 0x18, 0x30, 0x30,
        0x30, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static N8: [u8; 16] = [
        0x00, 0x00, 0x3C, 0x66, 0x66, 0x3C, 0x66, 0x66,
        0x66, 0x3C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static N9: [u8; 16] = [
        0x00, 0x00, 0x3C, 0x66, 0x66, 0x3E, 0x06, 0x06,
        0x06, 0x3C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static SLASH: [u8; 16] = [
        0x00, 0x00, 0x02, 0x06, 0x0C, 0x18, 0x30, 0x60,
        0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    static COLON: [u8; 16] = [
        0x00, 0x00, 0x00, 0x18, 0x18, 0x00, 0x00, 0x18,
        0x18, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    match ch {
        'a' | 'A' => &A, 'b' | 'B' => &B, 'c' | 'C' => &C,
        'd' | 'D' => &D, 'e' | 'E' => &E, 'f' | 'F' => &F,
        'g' | 'G' => &G, 'h' | 'H' => &H, 'i' | 'I' => &I,
        'j' | 'J' => &J, 'k' | 'K' => &K, 'l' | 'L' => &L,
        'm' | 'M' => &M, 'n' | 'N' => &N, 'o' | 'O' => &O,
        'p' | 'P' => &P, 'q' | 'Q' => &Q, 'r' | 'R' => &R,
        's' | 'S' => &S, 't' | 'T' => &T, 'u' | 'U' => &U,
        'v' | 'V' => &V, 'w' | 'W' => &W,
        'x' | 'X' | 'y' | 'Y' => &X_Y, 'z' | 'Z' => &Z,
        '0' => &N0, '1' => &N1, '2' => &N2, '3' => &N3, '4' => &N4,
        '5' => &N5, '6' => &N6, '7' => &N7, '8' => &N8, '9' => &N9,
        '/' => &SLASH, ':' => &COLON,
        _ => &SPACE,
    }
}
