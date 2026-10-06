//! 缩略图渲染/缓存与画布总览载荷(面板活页本/自由涂鸦用)。

use super::*;

/// Background-fill the thumbnail cache so the 活页本 grid opens instantly.
/// Renders at the settings panel's size (280) only; pages already cached or
/// drawn later are filled on demand. Yields between pages to stay out of the
/// way of live drawing.
pub fn warm_thumbnail_cache() {
    std::thread::Builder::new()
        .name("thumb-warm".into())
        .spawn(|| {
            std::thread::sleep(std::time::Duration::from_secs(5));
            let outline = STROKE_OUTLINE.load(std::sync::atomic::Ordering::SeqCst);
            for (id, _w, _h) in runtime().block_on(db::list_screens()) {
                let (count, max_id) = runtime().block_on(db::screen_stroke_version(id));
                if count == 0 {
                    continue;
                }
                let cached =
                    runtime().block_on(db::thumbnail_lookup(id, 280, count, max_id, outline));
                if cached.is_some() {
                    continue;
                }
                let mut len: c_int = 0;
                let ptr = glaspen2_render_thumbnail(id, 0, 0, 280, &mut len);
                if !ptr.is_null() {
                    glaspen2_free_rust_bytes(ptr, len);
                }
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
        })
        .ok();
}

/// and render only that region — so the thumbnail tightly fits the content.
/// Render a page thumbnail cropped to the content bounding box.
/// Instead of rendering the full screen and downsampling (which makes small
/// doodles illegible), we find the stroke bounding box, add a small margin,
/// and render only that region — so the thumbnail tightly fits the content.
/// Never touches the global STROKES.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_render_thumbnail(
    screen_id: i64,
    _w: c_int,
    _h: c_int,
    max_size: c_int,
    out_len: *mut c_int,
) -> *mut c_uchar {
    if max_size <= 0 || out_len.is_null() {
        if !out_len.is_null() {
            unsafe {
                *out_len = 0;
            }
        }
        return std::ptr::null_mut();
    }
    unsafe {
        *out_len = 0;
    }

    // ── 0. 缓存:内容版本(笔迹数,最大笔迹id)+渲染参数未变 → 直接返回存库 PNG ──
    let outline = STROKE_OUTLINE.load(std::sync::atomic::Ordering::SeqCst);
    let (count, max_id) = runtime().block_on(db::screen_stroke_version(screen_id));
    if count > 0
        && let Some(png) = runtime().block_on(db::thumbnail_lookup(
            screen_id, max_size, count, max_id, outline,
        ))
    {
        return leak_png(png, out_len);
    }

    match render_and_store_thumbnail(screen_id, max_size, (count, max_id), outline) {
        Some(png) => leak_png(png, out_len),
        None => std::ptr::null_mut(),
    }
}

/// Load one page's strokes, render its thumbnail and cache it under
/// `version` = (live stroke count, max stroke id). `None` when the page has
/// nothing drawable or cairo is unavailable.
fn render_and_store_thumbnail(
    screen_id: i64,
    max_size: i32,
    version: (i64, i64),
    outline: bool,
) -> Option<Vec<u8>> {
    let strokes = runtime().block_on(db::strokes_for_screen(screen_id));
    if strokes.is_empty() {
        return None;
    }
    let (pw, ph) = match runtime().block_on(db::screen_dims(screen_id)) {
        Some(d) => d,
        None => return None,
    };
    let png = render_strokes_thumbnail(&strokes, max_size, pw, ph)?;
    runtime().block_on(db::thumbnail_store(
        screen_id, max_size, version.0, version.1, outline, &png,
    ));
    Some(png)
}

/// 整页等比缩略图(不裁剪): 页原生尺寸 → 最长边 = max_size, 透明底。
/// 同一几何组的所有页输出同尺寸卡片(按屏幕尺寸展示每页), 页内内容
/// 位置真实保留 —— 不再做内容包围盒裁剪(裁剪与统一卡片尺寸冲突,
/// 同本子缩略图大小不一, 已放弃该形态)。
fn render_strokes_thumbnail(
    strokes: &[db::StrokeData],
    max_size: i32,
    page_w: i32,
    page_h: i32,
) -> Option<Vec<u8>> {
    let pw = page_w.max(1) as f64;
    let ph = page_h.max(1) as f64;
    // 整页 fit: 最长边 = max_size(纵横比 = 页的真实比例)
    let fit = (max_size as f64 / pw.max(ph)).min(1.0);
    let ow = ((pw * fit).ceil() as i32).max(1);
    let oh = ((ph * fit).ceil() as i32).max(1);

    // 渲染(整页坐标直接缩放, 透明底)
    let renderer = crate::cairo_dl::CairoRenderer::create_owned(ow, oh)?;
    renderer.clear();
    for s in strokes {
        if s.points.len() < 2 {
            continue;
        }
        let color = (
            (s.r.clamp(0.0, 1.0) * 255.0) as u8,
            (s.g.clamp(0.0, 1.0) * 255.0) as u8,
            (s.b.clamp(0.0, 1.0) * 255.0) as u8,
        );
        for i in 0..s.points.len() {
            let (x, y, wd, _t) = s.points[i];
            let px = (x * fit) as f32;
            let py = (y * fit) as f32;
            let pw_pt = (wd * fit) as f32;
            if i == 0 {
                renderer.fill_circle(px, py, pw_pt * 0.5, color);
            } else {
                let (qx, qy, _qw, _qt) = s.points[i - 1];
                renderer.stroke_line((qx * fit) as f32, (qy * fit) as f32, px, py, pw_pt, color);
            }
        }
    }
    renderer.flush();

    // 4. BGRA → RGBA + PNG 编码(已是目标尺寸,无需降采样)
    let bits = renderer.bits();
    let stride = ow as usize;
    let n = stride * oh as usize * 4;
    let mut rgba: Vec<u8> = unsafe {
        std::slice::from_raw_parts(bits, n)
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|px| [px[2], px[1], px[0], px[3]]) // BGRA → RGBA
            .collect()
    };
    unpremultiply_rgba(&mut rgba); // 预乘 alpha 反解, 避免边缘发暗
    encode_png_rgba(&rgba, ow as u32, oh as u32)
}

/// Magic + per-entry framing for `glaspen2_page_thumbnails` (see there).
pub const THUMB_BLOB_MAGIC: u32 = 0x3148_5447; // "GTH1"

/// Batched page thumbnails in a single self-describing blob — one FFI call and
/// one channel round trip for a whole screenful of the 活页本 grid, instead of
/// two SQL queries plus a PNG transfer per page.
///
/// Layout, little endian: magic u32, entry count u32, then per entry
/// `id i64, len u32, png bytes`. Pages with no drawable strokes are omitted, so
/// the blob may describe fewer pages than requested. Caller frees the buffer
/// with `glaspen2_free_rust_bytes`; NULL means "no thumbnails at all".
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_page_thumbnails(
    ids: *const i64,
    count: c_int,
    max_size: c_int,
    out_len: *mut c_int,
) -> *mut c_uchar {
    if out_len.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        *out_len = 0;
    }
    if ids.is_null() || count <= 0 || max_size <= 0 {
        return std::ptr::null_mut();
    }
    let ids = unsafe { std::slice::from_raw_parts(ids, count as usize) };
    let blob = page_thumbnails_blob(ids, max_size);
    if blob.is_empty() {
        return std::ptr::null_mut();
    }
    leak_png(blob, out_len)
}

/// Build the blob described by `glaspen2_page_thumbnails`: one batched version
/// query, one batched cache read, and a render only for the pages whose cache
/// entry is missing or stale.
pub fn page_thumbnails_blob(ids: &[i64], max_size: i32) -> Vec<u8> {
    let mut seen = std::collections::HashSet::with_capacity(ids.len());
    let ids: Vec<i64> = ids.iter().copied().filter(|id| seen.insert(*id)).collect();
    if ids.is_empty() {
        return Vec::new();
    }

    let outline = STROKE_OUTLINE.load(std::sync::atomic::Ordering::SeqCst);
    // 分批查询:SQLite 的绑定变量有上限,超长 IN 列表会整条失败
    const CHUNK: usize = 400;
    let mut versions = std::collections::HashMap::new();
    let mut cached = std::collections::HashMap::new();
    for chunk in ids.chunks(CHUNK) {
        versions.extend(runtime().block_on(db::stroke_versions_many(chunk)));
        cached.extend(runtime().block_on(db::thumbnails_many(chunk, max_size, outline)));
    }

    let mut entries: Vec<(i64, Vec<u8>)> = Vec::with_capacity(ids.len());
    for &id in &ids {
        let version = versions.get(&id).copied().unwrap_or((0, 0));
        // A cached PNG counts only when it was rendered from this exact version.
        let png = match cached.remove(&id) {
            Some((count, max_id, png)) if (count, max_id) == version => Some(png),
            _ if version.0 == 0 => None,
            _ => render_and_store_thumbnail(id, max_size, version, outline),
        };
        if let Some(png) = png {
            entries.push((id, png));
        }
    }

    encode_thumb_blob(&entries)
}

/// Frame `(id, png)` entries into the blob the settings panel parses.
/// Kept separate from the DB work so the wire format is unit-testable.
pub fn encode_thumb_blob(entries: &[(i64, Vec<u8>)]) -> Vec<u8> {
    let mut out =
        Vec::with_capacity(8 + entries.iter().map(|(_, png)| png.len() + 12).sum::<usize>());
    out.extend_from_slice(&THUMB_BLOB_MAGIC.to_le_bytes());
    out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for (id, png) in entries {
        out.extend_from_slice(&id.to_le_bytes());
        out.extend_from_slice(&(png.len() as u32).to_le_bytes());
        out.extend_from_slice(png);
    }
    out
}

/// Hand ownership of a PNG buffer to the caller via out_len (leak pattern).
fn leak_png(png: Vec<u8>, out_len: *mut c_int) -> *mut c_uchar {
    let len = png.len() as c_int;
    let ptr = png.as_ptr() as *mut c_uchar;
    std::mem::forget(png);
    unsafe {
        *out_len = len;
    }
    ptr
}

/// Free a buffer returned by glaspen2_render_thumbnail./// Free a buffer returned by glaspen2_render_thumbnail.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_free_rust_bytes(ptr: *mut c_uchar, len: c_int) {
    if !ptr.is_null() && len > 0 {
        unsafe {
            let _ = Vec::from_raw_parts(ptr, len as usize, len as usize);
        }
    }
}

/// 无限画布总览:把当前 STROKES 按包围盒 [bx,by,bw,bh] 适配进
/// out_w×out_h(居中、透明底),返回 PNG 字节,由 glaspen2_free_rust_bytes
/// 释放。失败返回 NULL 且 *out_len = 0。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_render_canvas_overview(
    bx: c_double,
    by: c_double,
    bw: c_double,
    bh: c_double,
    out_w: c_int,
    out_h: c_int,
    out_len: *mut c_int,
) -> *mut c_uchar {
    if out_len.is_null() || out_w <= 0 || out_h <= 0 || bw <= 0.0 || bh <= 0.0 {
        if !out_len.is_null() {
            unsafe {
                *out_len = 0;
            }
        }
        return std::ptr::null_mut();
    }
    unsafe {
        *out_len = 0;
    }
    let scale = (out_w as f64 / bw).min(out_h as f64 / bh);
    let off_x = (out_w as f64 - bw * scale) * 0.5;
    let off_y = (out_h as f64 - bh * scale) * 0.5;
    let Some(r) = crate::cairo_dl::CairoRenderer::create_owned(out_w, out_h) else {
        return std::ptr::null_mut();
    };
    r.clear();
    let strokes = STROKES.lock().unwrap();
    for s in strokes.iter() {
        let pts = &s.points;
        if pts.len() < 2 {
            continue;
        }
        let color = (
            (s.r.clamp(0.0, 1.0) * 255.0) as u8,
            (s.g.clamp(0.0, 1.0) * 255.0) as u8,
            (s.b.clamp(0.0, 1.0) * 255.0) as u8,
        );
        for i in 0..pts.len() {
            let (x, y, w, _t) = pts[i];
            let sx = ((x - bx) * scale + off_x) as f32;
            let sy = ((y - by) * scale + off_y) as f32;
            let sw = (w * scale).max(1.0) as f32;
            if i == 0 {
                r.fill_circle(sx, sy, sw * 0.5, color);
            } else {
                let (px, py, _pw, _pt) = pts[i - 1];
                r.stroke_line(
                    ((px - bx) * scale + off_x) as f32,
                    ((py - by) * scale + off_y) as f32,
                    sx,
                    sy,
                    sw,
                    color,
                );
            }
        }
    }
    drop(strokes);
    r.flush();

    let bits = r.bits();
    let n = (out_w as usize) * (out_h as usize) * 4;
    let mut bgra = unsafe { std::slice::from_raw_parts(bits, n).to_vec() };
    unpremultiply_rgba(&mut bgra); // 预乘 alpha 反解
    let Some(png) = encode_png_bgra(&bgra, out_w as u32, out_h as u32) else {
        return std::ptr::null_mut();
    };
    let len = png.len() as c_int;
    let ptr = png.as_ptr() as *mut c_uchar;
    std::mem::forget(png);
    unsafe {
        *out_len = len;
    }
    ptr
}

/// Encode RGBA pixel data as PNG bytes.
pub(crate) fn encode_png_rgba(rgba: &[u8], width: u32, height: u32) -> Option<Vec<u8>> {
    use image::{
        ImageEncoder,
        codecs::png::{CompressionType, FilterType, PngEncoder},
    };
    let mut buf = Vec::new();
    // Fast 压缩: debug 构建下 Default 档能把大图编码拖到秒级; 缩略图/
    // 详情位图对压缩率不敏感(尺寸本就不大), Fast 档全路径提速。
    PngEncoder::new_with_quality(&mut buf, CompressionType::Fast, FilterType::Adaptive)
        .write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
        .ok()?;
    Some(buf)
}

/// cairo 表面(BGRA 预乘)直接出 PNG: 换通道后编码。
/// 此前多个调用点把 BGRA 原样喂给 encode_png_rgba, R/B 互换
/// (红色笔迹导出/展示成蓝色), 统一收口到这里。
pub(crate) fn encode_png_bgra(bgra: &[u8], width: u32, height: u32) -> Option<Vec<u8>> {
    let swapped: Vec<u8> = bgra
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|px| [px[2], px[1], px[0], px[3]])
        .collect();
    encode_png_rgba(&swapped, width, height)
}

// ---------------------------------------------------------------------------
// 聊天流(⌘⌃3 录制的手写消息 → 本地 axum 存储)
// ---------------------------------------------------------------------------


#[cfg(test)]
mod png_channel_tests {
    use super::*;

    /// 红蓝互换回归: cairo BGRA 缓冲经 encode_png_bgra 出来的 PNG,
    /// 红像素必须是红(R=255, B=0)。此前四条路径直接喂 BGRA 给
    /// RGBA 编码器, 红笔显示成蓝笔。
    #[test]
    fn encode_png_bgra_channel_order() {
        // 1×2: 纯红 (B=0,G=0,R=255,A=255), 纯蓝 (B=255,G=0,R=0,A=255)
        let bgra = [0u8, 0, 255, 255, 255, 0, 0, 255];
        let png = encode_png_bgra(&bgra, 2, 1).expect("编码成功");
        let decoded = image::load_from_memory(&png).unwrap().to_rgba8();
        let px = decoded.get_pixel(0, 0); // 输入红
        assert_eq!((px[0], px[1], px[2]), (255, 0, 0), "红必须还是红");
        let px = decoded.get_pixel(1, 0); // 输入蓝
        assert_eq!((px[0], px[1], px[2]), (0, 0, 255), "蓝必须还是蓝");
    }

    /// 整页等比缩略图: 同组页输出同尺寸, 纵横比 = 页真实比例。
    #[test]
    fn thumbnail_full_page_uniform_size() {
        let strokes = vec![crate::db::StrokeData {
            id: 1,
            r: 1.0,
            g: 0.0,
            b: 0.0,
            width_scale: 1.0,
            points: vec![(100.0, 100.0, 2.0, 0.0), (300.0, 200.0, 2.0, 0.1)],
        }];
        // 同一页尺寸, 两次渲染 → 同尺寸
        let a = render_strokes_thumbnail(&strokes, 280, 3440, 1440).unwrap();
        let b = render_strokes_thumbnail(&strokes, 280, 3440, 1440).unwrap();
        assert_eq!(a.len(), b.len());
        let da = image::load_from_memory(&a).unwrap();
        assert_eq!((da.width(), da.height()), (280, 118), "3440x1440 等比 → 280x118");
        // 不同页尺寸 → 比例各自正确(不再裁剪)
        let c = render_strokes_thumbnail(&strokes, 280, 1920, 1080).unwrap();
        let dc = image::load_from_memory(&c).unwrap();
        assert_eq!((dc.width(), dc.height()), (280, 158), "1920x1080 → 280x158");
    }
}
