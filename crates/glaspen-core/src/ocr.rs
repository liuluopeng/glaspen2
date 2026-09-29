//! OCR 识别(axum PP-OCRv6 服务, docs/ocr-api.md)。
//!
//! glaspen2 把当前页渲染成白底 PNG,POST 到 axum 的 `/api/ocr/images`
//! (multipart,多图可重复),取回识别全文后按页存入本地库 —— 供 PDF
//! 导出叠加可复制的隐形文本层。服务不可达/未配置一律静默跳过,
//! 绝不影响涂鸦本身。
//!
//! 触发(闲时模式):抬笔提交只做「待识别登记」;常驻 worker 每几秒
//! 检查一次**整机输入空闲**(macOS CGEventSource, 系统级含笔/鼠标/键盘),
//! 空闲 ≥10s 才逐页识别 —— 用户一有操作立即让路, 不占正在使用的电脑。
//! 另有 `glaspen2_ocr_backfill_all` 批量补全历史页面(显式触发)。

use crate::db;
use crate::runtime;
use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

/// 整机输入空闲阈值:最后一个键鼠/笔事件距今超过该秒数才开 OCR。
const IDLE_INPUT_SECS: f64 = 10.0;
/// 单次 HTTP 超时:OCR 是 CPU 重活(检测+识别两次推理)。
const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

/// 解析 axum 服务基址(与登录/涂鸦身份同一来源:DB chat_api_base >
/// GLASPEN_API_BASE 环境变量)。未配置 → None(不启用 OCR)。
fn api_base() -> Option<String> {
    glaspen_chat::auth::api_base()
}

/// 把整页笔迹渲染成白底 PNG(长边 ≤ 2000,等比缩放):
/// 画在透明底上再逐像素合成到白 —— PP-OCR 对浅底深字最稳。
fn render_page_png(strokes: &[db::StrokeData], sw: i32, sh: i32) -> Option<Vec<u8>> {
    let long = (sw.max(sh)) as f64;
    let scale = (2000.0 / long).min(1.0);
    let rw = ((sw as f64) * scale).ceil() as i32;
    let rh = ((sh as f64) * scale).ceil() as i32;

    let renderer = crate::cairo_dl::CairoRenderer::create_owned(rw, rh)?;
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
            let (x, y, w, _t) = s.points[i];
            if i == 0 {
                renderer.fill_circle(
                    (x * scale) as f32,
                    (y * scale) as f32,
                    (w * 0.5 * scale) as f32,
                    color,
                );
            } else {
                let (px, py, _pw, _pt) = s.points[i - 1];
                renderer.stroke_line(
                    (px * scale) as f32,
                    (py * scale) as f32,
                    (x * scale) as f32,
                    (y * scale) as f32,
                    (w * scale) as f32,
                    color,
                );
            }
        }
    }
    renderer.flush();

    // 透明底 → 白底合成(cairo ARGB32 预乘;bits 内存序 B,G,R,A)
    unsafe {
        let n = (rw as usize) * (rh as usize) * 4;
        let bits = std::slice::from_raw_parts_mut(renderer.bits(), n);
        for px in bits.chunks_exact_mut(4) {
            let a = px[3] as u16;
            if a < 255 {
                px[0] = (px[0] as u16 + (255 - a) * 255 / 255) as u8; // B + (1-a)*白
                px[1] = (px[1] as u16 + (255 - a)) as u8;
                px[2] = (px[2] as u16 + (255 - a)) as u8;
                px[3] = 255;
            }
        }
    }

    crate::export::thumbs::encode_png_rgba(
        unsafe { std::slice::from_raw_parts(renderer.bits(), (rw as usize) * (rh as usize) * 4) },
        rw as u32,
        rh as u32,
    )
}

/// 调 axum OCR 服务识别一组图片,按顺序返回每张的识别文本。
/// 单张失败用空串占位(不拖垮整批)。`api_base` 为空 → Err。
pub fn ocr_images(api_base: &str, images: &[Vec<u8>]) -> Result<Vec<String>, String> {
    if images.is_empty() {
        return Err("没有图片".into());
    }
    let boundary = format!("glaspen-ocr-{}", std::process::id());
    let mut body: Vec<u8> = Vec::new();
    for (i, img) in images.iter().enumerate() {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; \
                 name=\"image\"; filename=\"img{i}.png\"\r\n\
                 Content-Type: image/png\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(img);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    let tls = ureq::tls::TlsConfig::builder()
        .disable_verification(true) // dev 自签证书(Caddy local)
        .build();
    let config = ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_global(Some(REQUEST_TIMEOUT))
        .build();
    let agent = ureq::Agent::new_with_config(config);
    let resp = agent
        .post(&format!("{api_base}/api/ocr/images"))
        .header(
            "Content-Type",
            &format!("multipart/form-data; boundary={boundary}"),
        )
        .send(body.as_slice())
        .map_err(|e| format!("OCR 请求失败: {e}"))?;
    let text = resp
        .into_body()
        .read_to_string()
        .map_err(|e| format!("读取 OCR 响应失败: {e}"))?;
    let v: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("OCR 响应解析失败: {e}"))?;
    if v.get("code").and_then(|c| c.as_i64()) != Some(200) {
        return Err(format!("OCR 服务报错: {v}"));
    }
    let mut texts = Vec::new();
    if let Some(arr) = v.get("results").and_then(|r| r.as_array()) {
        for r in arr {
            texts.push(r.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string());
        }
    }
    Ok(texts)
}

/// 识别一页并把结果写入本地库(软删旧结果后插入)。
/// 返回识别全文或失败原因。
static PENDING: OnceLock<Mutex<VecDeque<i64>>> = OnceLock::new();
static WORKER: OnceLock<()> = OnceLock::new();

pub fn ocr_screen(screen_id: i64) -> Result<String, String> {
    let strokes = runtime().block_on(db::strokes_for_screen(screen_id));
    if strokes.is_empty() {
        return Err("该页没有笔迹".into());
    }
    let (sw, sh) = runtime()
        .block_on(db::screen_dims(screen_id))
        .unwrap_or((1920, 1080));
    let png = render_page_png(&strokes, sw, sh).ok_or("页面渲染失败")?;
    let base = api_base().ok_or_else(|| "未配置 axum 服务地址".to_string())?;
    let texts = ocr_images(&base, &[png])?;
    let text = texts.first().cloned().unwrap_or_default();
    if text.trim().is_empty() {
        return Err("未识别出文字".into());
    }
    runtime().block_on(async {
        db::save_ocr_result(screen_id, &text).await;
    });
    Ok(text)
}

/// 抬笔提交后的登记:把当前页放进「待识别」队列并确保常驻 worker
/// 已启动。不发生任何网络/计算 —— 真正的识别在整机输入空闲时进行。
pub fn on_stroke_committed(screen_id: i64) {
    if api_base().is_none() {
        return;
    }
    {
        let mut q = pending_queue().lock().unwrap();
        if !q.contains(&screen_id) {
            q.push_back(screen_id);
        }
    }
    ensure_worker();
}

fn pending_queue() -> &'static Mutex<VecDeque<i64>> {
    PENDING.get_or_init(|| Mutex::new(VecDeque::new()))
}

fn ensure_worker() {
    WORKER.get_or_init(|| {
        let _ = std::thread::Builder::new()
            .name("ocr-idle".into())
            .spawn(idle_worker);
    });
}

/// 常驻闲时 worker:每 5s 醒一次;整机输入空闲 ≥ IDLE_INPUT_SECS 时
/// 逐页识别待识别队列(一次一页)。用户一有输入立即让路。
fn idle_worker() {
    loop {
        std::thread::sleep(Duration::from_secs(5));
        if user_idle_secs() < IDLE_INPUT_SECS {
            continue;
        }
        let Some(screen_id) = pending_queue().lock().unwrap().pop_front() else {
            continue;
        };
        match ocr_screen(screen_id) {
            Ok(_) => eprintln!("[ocr] 闲时识别完成 页面 {screen_id}"),
            Err(e) => eprintln!("[ocr] 闲时识别跳过({screen_id}): {e}"),
        }
    }
}

/// 整机输入空闲秒数(键鼠/笔的一切输入都算"在使用电脑")。
#[cfg(target_os = "macos")]
fn user_idle_secs() -> f64 {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(state: u32, mask: u64) -> f64;
    }
    // kCGEventSourceStateCombinedSessionState = 0; kCGAnyInputEventType = u64::MAX
    unsafe { CGEventSourceSecondsSinceLastEventType(0, u64::MAX) }
}

/// 非 macOS 平台暂无系统空闲检测:视为一直空闲, 识别节奏退化为
/// 队列驱动(每 5s 最多一页)。
#[cfg(not(target_os = "macos"))]
fn user_idle_secs() -> f64 {
    f64::INFINITY
}

/// 批量补全:为所有还没有 OCR 结果的页面识别(阻塞,逐页)。
/// 返回成功识别的页数。
pub fn backfill_missing() -> usize {
    let Some(base) = api_base() else {
        eprintln!("[ocr] 未配置服务地址,跳过批量补全");
        return 0;
    };
    let pages = runtime().block_on(db::pages_missing_ocr());
    if pages.is_empty() {
        eprintln!("[ocr] 所有页面都已有 OCR 结果");
        return 0;
    }
    eprintln!("[ocr] 批量补全 {} 页", pages.len());
    let mut ok = 0;
    for (screen_id, sw, sh) in &pages {
        let strokes = runtime().block_on(db::strokes_for_screen(*screen_id));
        if strokes.is_empty() {
            continue;
        }
        let Some(png) = render_page_png(&strokes, *sw, *sh) else {
            continue;
        };
        let texts = match ocr_images(&base, &[png]) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("[ocr] 页面 {screen_id} 识别失败: {e}");
                continue;
            }
        };
        let text = texts.first().cloned().unwrap_or_default();
        runtime().block_on(async {
            db::save_ocr_result(*screen_id, &text).await;
        });
        ok += 1;
        eprintln!("[ocr] 页面 {screen_id} 完成({} 字)", text.chars().count());
    }
    ok
}
