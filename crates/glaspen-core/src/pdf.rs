//! PDF export — render strokes as vector paths.
//! Uses printpdf for vector rendering, lopdf for saving.

use std::path::PathBuf;

use printpdf::*;

use crate::{db, runtime};

/// 内嵌 glyphless 字体:所有字形为空, CID = Unicode 码位 —— 文本可选中/可复制/可搜索但不可见。
const GLYPHLESS_TTF: &[u8] = include_bytes!("../assets/glyphless.ttf");

/// Export all pages to a vector PDF on the desktop. Returns the file path.
pub fn export_all_pages() -> Option<String> {
    export_pages_by_ids(&[])
}

/// 导出指定页(空切片 = 全部页)。勾选页合成 PDF 用。
pub fn export_pages_by_ids(ids: &[i64]) -> Option<String> {
    let rt = runtime();

    let db_path = std::env::var("GLASPEN2_DB")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| db::db_path());

    // Open DB pool
    let pool = rt.block_on(async {
        sqlx::SqlitePool::connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&db_path)
                .read_only(true),
        )
        .await
    });
    let pool = match pool {
        Ok(p) => p,
        Err(e) => {
            tracing::info!("DB: {e}");
            return None;
        }
    };

    // Collect all screens
    let screens: Vec<(i64, i32, i32)> = rt.block_on(async {
        sqlx::query_as(
            "SELECT s.id, s.screen_w, s.screen_h FROM screens s \
             WHERE EXISTS (SELECT 1 FROM strokes WHERE screen_id = s.id) \
             ORDER BY s.id",
        )
        .fetch_all(&pool)
        .await
        .unwrap_or_default()
    });

    let screens: Vec<(i64, i32, i32)> = if ids.is_empty() {
        screens
    } else {
        let set: std::collections::HashSet<i64> = ids.iter().copied().collect();
        screens
            .into_iter()
            .filter(|(id, _, _)| set.contains(id))
            .collect()
    };
    if screens.is_empty() {
        tracing::info!("No pages");
        return None;
    }
    tracing::info!("Exporting {} pages", screens.len());

    let mut doc = PdfDocument::new("glaspen2");

    for (screen_id, sw, sh) in &screens {
        // Load strokes directly (single JOIN query, no N+1)
        let strokes: Vec<db::StrokeData> = rt.block_on(db::strokes_for_screen(*screen_id));
        tracing::info!(
            "Page {}: {}x{} ({} strokes)",
            screen_id,
            sw,
            sh,
            strokes.len()
        );

        // Page dimensions in mm (72 pt/inch → 25.4 mm/inch)
        let mm_w = *sw as f32 * 25.4 / 72.0;
        let mm_h = *sh as f32 * 25.4 / 72.0;

        let mut ops: Vec<Op> = Vec::new();
        for s in &strokes {
            push_stroke(&mut ops, s, 0.0, 0.0, *sh as f64);
        }

        doc.pages.push(PdfPage::new(Mm(mm_w), Mm(mm_h), ops));
    }

    // Close DB
    rt.block_on(pool.close());

    // 收集每页 OCR 全文(axum 服务识别, 启动/抬笔自动补全): 有则叠可复制文本层
    let pages_text: Vec<Option<String>> = {
        let mut v = Vec::with_capacity(screens.len());
        for (screen_id, _, _) in &screens {
            v.push(rt.block_on(db::latest_ocr_text(*screen_id)));
        }
        v
    };

    let opts = PdfSaveOptions::default();
    let mut warnings = Vec::new();
    let mut lopdf_doc = doc.to_lopdf_document(&opts, &mut warnings);

    // Post-process: glyphless CID 字体 + 隐形可复制文本层
    add_glyphless_text_layer(&mut lopdf_doc, &screens, &pages_text);

    // Save with lopdf
    let desktop = desktop_path();
    let path = desktop.join(timestamped_name("pdf"));
    let mut pdf_bytes = Vec::new();
    match lopdf_doc.save_to(&mut pdf_bytes) {
        Ok(()) => {
            std::fs::write(&path, &pdf_bytes).ok();
            if path.exists() {
                // 临时诊断: 桌面 PDF 莫名出现。调用栈 + 库路径, 定位触发链后移除。
                tracing::error!(
                    "PDF 写入(活页本全导, {} 页) → {} | 库: {} | 调用栈:\n{}",
                    screens.len(),
                    path.display(),
                    crate::db::db_path().display(),
                    std::backtrace::Backtrace::force_capture()
                );
                tracing::info!("Saved vector PDF to {}", path.display());
                return Some(path.to_string_lossy().to_string());
            }
        }
        Err(e) => tracing::error!("Save error: {e}"),
    }
    None
}

/// Export the single infinite canvas to a PDF, split into screen-sized pages.
///
/// The canvas content is tiled into a grid of `page_w` x `page_h` (points)
/// rectangles; each tile becomes one PDF page. Strokes that cross a tile edge
/// are drawn on every tile they touch and clipped by the page's MediaBox.
/// Returns the file path, or None when there is nothing to export / too many
/// pages would be produced.
pub fn export_infinite_paged(page_w: i32, page_h: i32) -> Option<String> {
    let rt = runtime();

    let db_path = std::env::var("GLASPEN2_DB")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| db::db_path());
    let pool = rt.block_on(async {
        sqlx::SqlitePool::connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&db_path)
                .read_only(true),
        )
        .await
    });
    let pool = match pool {
        Ok(p) => p,
        Err(e) => {
            tracing::info!("DB: {e}");
            return None;
        }
    };
    let strokes = rt.block_on(db::load_infinite_strokes_with(&pool));
    rt.block_on(pool.close());

    if strokes.is_empty() {
        tracing::info!("Infinite canvas is empty");
        return None;
    }

    // Content bounding box.
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for s in &strokes {
        for &(x, y, w, _) in &s.points {
            let r = w * 0.5;
            x0 = x0.min(x - r);
            y0 = y0.min(y - r);
            x1 = x1.max(x + r);
            y1 = y1.max(y + r);
        }
    }
    if x0 > x1 || y0 > y1 {
        return None;
    }
    let pad = 10.0;
    x0 -= pad;
    y0 -= pad;
    x1 += pad;
    y1 += pad;

    let pw = page_w.max(64) as f64;
    let ph = page_h.max(64) as f64;
    let cols = (((x1 - x0) / pw).ceil() as i64).max(1);
    let rows = (((y1 - y0) / ph).ceil() as i64).max(1);
    const MAX_PAGES: i64 = 400;
    if cols * rows > MAX_PAGES {
        tracing::info!(
            "Infinite canvas needs {} pages (> {}) — reduce content or page size",
            cols * rows,
            MAX_PAGES
        );
        return None;
    }
    tracing::info!(
        "Infinite canvas: {} strokes, bbox {:.0}x{:.0}, {}x{} = {} pages",
        strokes.len(),
        x1 - x0,
        y1 - y0,
        cols,
        rows,
        cols * rows
    );

    let mm_w = pw as f32 * 25.4 / 72.0;
    let mm_h = ph as f32 * 25.4 / 72.0;
    let mut doc = PdfDocument::new("glaspen2-infinite");

    for row in 0..rows {
        for col in 0..cols {
            let ox = x0 + col as f64 * pw;
            let oy = y0 + row as f64 * ph;
            let mut ops: Vec<Op> = Vec::new();
            for s in &strokes {
                if stroke_intersects_rect(s, ox, oy, pw, ph) {
                    push_stroke(&mut ops, s, ox, oy, ph);
                }
            }
            doc.pages.push(PdfPage::new(Mm(mm_w), Mm(mm_h), ops));
        }
    }

    let opts = PdfSaveOptions::default();
    let mut warnings = Vec::new();
    let mut lopdf_doc = doc.to_lopdf_document(&opts, &mut warnings);
    let path = desktop_path().join(timestamped_name("pdf"));
    let mut pdf_bytes = Vec::new();
    match lopdf_doc.save_to(&mut pdf_bytes) {
        Ok(()) => {
            std::fs::write(&path, &pdf_bytes).ok();
            if path.exists() {
                // 临时诊断: 桌面 PDF 莫名出现。调用栈 + 库路径, 定位触发链后移除。
                tracing::error!(
                    "PDF 写入(无限画布分页) → {} | 库: {} | 调用栈:\n{}",
                    path.display(),
                    crate::db::db_path().display(),
                    std::backtrace::Backtrace::force_capture()
                );
                tracing::info!("Saved infinite-canvas PDF to {}", path.display());
                return Some(path.to_string_lossy().to_string());
            }
        }
        Err(e) => tracing::error!("Save error: {e}"),
    }
    None
}

/// Append one stroke's vector ops. Canvas (x,y) maps to page ((x-ox), page_h-(y-oy)).
fn push_stroke(ops: &mut Vec<Op>, s: &db::StrokeData, ox: f64, oy: f64, page_h: f64) {
    let pts = &s.points;
    if pts.len() < 2 {
        return;
    }
    let color = Color::Rgb(Rgb {
        r: s.r as f32,
        g: s.g as f32,
        b: s.b as f32,
        icc_profile: None,
    });
    let map = |x: f64, y: f64| (Pt((x - ox) as f32), Pt((page_h - (y - oy)) as f32));

    for i in 0..pts.len() {
        let (x, y, w, _t) = pts[i];
        let (px, py) = map(x, y);
        if i == 0 {
            // First point: round dot (zero-length line with round cap).
            ops.push(Op::SaveGraphicsState);
            ops.push(Op::SetFillColor { col: color.clone() });
            ops.push(Op::SetOutlineThickness { pt: Pt(w as f32) });
            ops.push(Op::SetLineCapStyle {
                cap: LineCapStyle::Round,
            });
            ops.push(Op::DrawLine {
                line: Line {
                    points: vec![
                        LinePoint {
                            p: Point { x: px, y: py },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point { x: px, y: py },
                            bezier: false,
                        },
                    ],
                    is_closed: false,
                },
            });
            ops.push(Op::RestoreGraphicsState);
        } else {
            let (px_prev, py_prev, _pw, _pt) = pts[i - 1];
            let (ppx, ppy) = map(px_prev, py_prev);
            ops.push(Op::SaveGraphicsState);
            ops.push(Op::SetOutlineColor { col: color.clone() });
            ops.push(Op::SetOutlineThickness { pt: Pt(w as f32) });
            ops.push(Op::SetLineCapStyle {
                cap: LineCapStyle::Round,
            });
            ops.push(Op::SetLineJoinStyle {
                join: LineJoinStyle::Round,
            });
            ops.push(Op::DrawLine {
                line: Line {
                    points: vec![
                        LinePoint {
                            p: Point { x: ppx, y: ppy },
                            bezier: false,
                        },
                        LinePoint {
                            p: Point { x: px, y: py },
                            bezier: false,
                        },
                    ],
                    is_closed: false,
                },
            });
            ops.push(Op::RestoreGraphicsState);
        }
    }
}

/// Whether a stroke's bounding box (grown by its widest point) overlaps a tile.
fn stroke_intersects_rect(s: &db::StrokeData, ox: f64, oy: f64, w: f64, h: f64) -> bool {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    let mut max_w: f64 = 0.0;
    for &(x, y, pw, _) in &s.points {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
        max_w = max_w.max(pw);
    }
    if x0 > x1 {
        return false;
    }
    let r = max_w * 0.5;
    x1 + r >= ox && x0 - r <= ox + w && y1 + r >= oy && y0 - r <= oy + h
}

fn desktop_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string()))
            .join("Desktop")
    }
    #[cfg(not(target_os = "windows"))]
    {
        PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string())).join("Desktop")
    }
}

fn timestamped_name(ext: &str) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600 + 8) % 24;
    let days = secs / 86400;
    let y = 1970 + days / 365;
    let d = days % 365;
    format!(
        "glaspen2_{:04}-{:03}_{:02}-{:02}-{:02}.{}",
        y, d, h, m, s, ext
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

use lopdf::{Dictionary, Object, Stream, dictionary};

const IDENTITY_CMAP: &[u8] = b"/CIDInit /ProcSet findresource begin
12 dict begin
begincmap
/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def
/CMapName /Adobe-Identity-UCS def
/CMapType 2 def
1 begincodespacerange
<0000><FFFF>
endcodespacerange
1 beginbfrange
<0000><FFFF><0000>
endbfrange
endcmap
CMapName currentdict /CMap defineresource pop
end
end";

// ---------------------------------------------------------------------------
// lopdf 后处理: glyphless CID 字体 + 隐形可复制 Unicode 文本层
// ---------------------------------------------------------------------------

/// 给有 OCR 全文的页叠加隐形文本层:逐行放置, 文本可选中/复制/搜索
/// 但不可见(渲染模式 3)。OCR 无坐标框, 按行等距排在页面左侧。
fn add_glyphless_text_layer(
    doc: &mut lopdf::Document,
    screens: &[(i64, i32, i32)],
    pages_text: &[Option<String>],
) {
    let type0_font_id = add_glyphless_font_objects(doc);
    let font_name = b"C1";
    let page_ids: Vec<lopdf::ObjectId> = doc.get_pages().into_values().collect();

    for (page_num, page_id) in page_ids.iter().enumerate() {
        if page_num >= pages_text.len() {
            break;
        }
        let Some(full_text) = pages_text[page_num].as_ref() else {
            continue;
        };
        let (_, _, sh) = screens.get(page_num).copied().unwrap_or((0, 0, 1080));
        let content = build_text_content(sh as f32, full_text);
        if content.is_empty() {
            continue;
        }

        let old_content = get_page_content_bytes(doc, *page_id);
        let mut new_content = content;
        new_content.extend_from_slice(&old_content);
        let new_stream_id = doc.add_object(Stream::new(Dictionary::new(), new_content));
        add_font_to_page_resources(doc, *page_id, font_name, type0_font_id);
        if let Ok(page_dict) = doc.get_dictionary_mut(*page_id) {
            page_dict.set(b"Contents", Object::Reference(new_stream_id));
        }
    }
}

/// glyphless 字体三件套: 字体文件流 → FontDescriptor → CIDFontType2 →
/// ToUnicode CMap → Type0 根字体。返回 Type0 字体对象 id。
fn add_glyphless_font_objects(doc: &mut lopdf::Document) -> lopdf::ObjectId {
    let font_stream_id = doc.add_object(Stream::new(
        {
            let mut d = Dictionary::new();
            d.set("Length", GLYPHLESS_TTF.len() as i64);
            d
        },
        GLYPHLESS_TTF.to_vec(),
    ));

    let font_desc_id = doc.add_object(dictionary! {
        b"Type" => Object::Name(b"FontDescriptor".to_vec()),
        b"FontName" => Object::Name(b"GLYPHLESS+GlyphLessFont".to_vec()),
        b"Flags" => Object::Integer(4),
        b"FontBBox" => Object::Array(vec![
            Object::Integer(0), Object::Integer(0),
            Object::Integer(0), Object::Integer(0),
        ]),
        b"ItalicAngle" => Object::Integer(0),
        b"Ascent" => Object::Integer(0),
        b"Descent" => Object::Integer(0),
        b"CapHeight" => Object::Integer(0),
        b"StemV" => Object::Integer(0),
        b"FontFile2" => Object::Reference(font_stream_id),
    });

    let mut cid_system_info = Dictionary::new();
    cid_system_info.set(b"Registry", Object::string_literal("Adobe"));
    cid_system_info.set(b"Ordering", Object::string_literal("Identity"));
    cid_system_info.set(b"Supplement", Object::Integer(0));

    let cid_font_id = doc.add_object(dictionary! {
        b"Type" => Object::Name(b"Font".to_vec()),
        b"Subtype" => Object::Name(b"CIDFontType2".to_vec()),
        b"BaseFont" => Object::Name(b"GLYPHLESS+GlyphLessFont".to_vec()),
        b"CIDSystemInfo" => Object::Dictionary(cid_system_info),
        b"DW" => Object::Integer(1000),
        b"FontDescriptor" => Object::Reference(font_desc_id),
    });

    let cmap_stream_id = doc.add_object(Stream::new(Dictionary::new(), IDENTITY_CMAP.to_vec()));

    doc.add_object(dictionary! {
        b"Type" => Object::Name(b"Font".to_vec()),
        b"Subtype" => Object::Name(b"Type0".to_vec()),
        b"BaseFont" => Object::Name(b"GLYPHLESS+GlyphLessFont-Identity-H".to_vec()),
        b"Encoding" => Object::Name(b"Identity-H".to_vec()),
        b"DescendantFonts" => Object::Array(vec![Object::Reference(cid_font_id)]),
        b"ToUnicode" => Object::Reference(cmap_stream_id),
    })
}

/// 由整页全文生成隐形文本内容流(UTF-16BE hex CID, 渲染模式 3 不可见)。
/// 逐行放置:左边距 20pt, 首行基线 = 页高 - 40, 行距 28pt。
fn build_text_content(sh: f32, full_text: &str) -> Vec<u8> {
    let mut content = Vec::new();
    let lines: Vec<&str> = full_text.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return content;
    }

    content.extend_from_slice(b"q\nBT\n3 Tr\n");
    let mut y = sh - 40.0;
    for line in &lines {
        let hex_cids: String = line
            .encode_utf16()
            .map(|cp| format!("{cp:04X}"))
            .collect::<Vec<_>>()
            .join("");
        use std::io::Write as _;
        let _ = writeln!(&mut content, "/C1 20.0 Tf");
        let _ = writeln!(&mut content, "1 0 0 1 20.0 {y:.1} Tm");
        let _ = writeln!(&mut content, "<{hex_cids}> Tj");
        y -= 28.0;
    }
    content.extend_from_slice(b"ET\nQ\n");
    content
}

/// 读取页内容流的解码字节(文本层拼在原内容之前)。
fn get_page_content_bytes(doc: &lopdf::Document, page_id: lopdf::ObjectId) -> Vec<u8> {
    let page_dict = match doc.get_dictionary(page_id) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let contents = match page_dict.get(b"Contents") {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    match contents {
        Object::Reference(stream_id) => get_stream_bytes(doc, *stream_id).unwrap_or_default(),
        Object::Array(refs) => {
            let mut result = Vec::new();
            for obj in refs {
                if let Ok(stream_id) = obj.as_reference()
                    && let Ok(content) = get_stream_bytes(doc, stream_id)
                {
                    result.extend_from_slice(&content);
                    result.push(b'\n');
                }
            }
            result
        }
        _ => Vec::new(),
    }
}

fn get_stream_bytes(doc: &lopdf::Document, stream_id: lopdf::ObjectId) -> lopdf::Result<Vec<u8>> {
    let obj = doc.get_object(stream_id)?;
    obj.as_stream()?.get_plain_content()
}

/// 把 /C1 字体引用挂进页 Resources 的 /Font 字典(字典或引用都兼容)。
fn add_font_to_page_resources(
    doc: &mut lopdf::Document,
    page_id: lopdf::ObjectId,
    font_name: &[u8],
    font_id: lopdf::ObjectId,
) {
    let res_id = {
        let page_dict = match doc.get_dictionary(page_id) {
            Ok(d) => d,
            Err(_) => return,
        };
        match page_dict.get(b"Resources") {
            Ok(Object::Reference(id)) => *id,
            _ => return,
        }
    };
    let res_dict = match doc.get_dictionary_mut(res_id) {
        Ok(d) => d,
        Err(_) => return,
    };
    match res_dict.get_mut(b"Font") {
        Ok(Object::Dictionary(font_dict)) => {
            font_dict.set(font_name, Object::Reference(font_id));
        }
        _ => {
            let mut font_dict = Dictionary::new();
            font_dict.set(font_name, Object::Reference(font_id));
            res_dict.set(b"Font", Object::Dictionary(font_dict));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Generate PDF (alias for export_all_pages)
    #[test]
    fn test_export_pdf() {
        export_all_pages();
    }
}
