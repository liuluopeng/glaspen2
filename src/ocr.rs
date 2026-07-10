//! OCR module — PP-OCRv6 recognition via ONNX Runtime.
//!
//! Requires model files at runtime:
//!   models/ppocr_v6_rec.onnx
//!   models/ppocr_v6_dict.json

use ndarray::Array4;
use ort::session::Session;
use ort::value::TensorRef;
use std::sync::{Mutex, OnceLock};

static ENGINE: OnceLock<OcrEngine> = OnceLock::new();

struct OcrEngine {
    session: Mutex<Session>,
    chars: Vec<String>,
}

fn model_path(p: &str) -> std::path::PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        let dir = exe.parent().unwrap().join("models");
        let f = dir.join(p);
        if f.exists() { return f; }
    }
    std::path::Path::new("models").join(p)
}

fn load_chars() -> Vec<String> {
    let path = model_path("ppocr_v6_dict.json");
    let data = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));
    serde_json::from_str(&data)
        .unwrap_or_else(|e| panic!("Failed to parse char dict: {}", e))
}

fn engine() -> &'static OcrEngine {
    ENGINE.get_or_init(|| {
        let model_file = model_path("ppocr_v6_rec.onnx");
        let session = Session::builder()
            .unwrap()
            .commit_from_file(&model_file)
            .unwrap_or_else(|e| panic!("Failed to load model from {}: {}", model_file.display(), e));
        let chars = load_chars();
        OcrEngine { session: Mutex::new(session), chars }
    })
}

/// Run OCR on a cropped RGBA image buffer. Returns recognized text.
pub fn recognize(pixels: &[u8], width: u32, height: u32) -> String {
    let e = engine();

    // Preprocess: resize to H=48 maintaining aspect ratio
    let target_h = 48u32;
    let scale = target_h as f64 / height.max(1) as f64;
    let mut target_w = (width as f64 * scale).ceil() as u32;
    target_w = target_w.max(8);
    target_w = ((target_w + 7) / 8) * 8;

    // Build CHW tensor with PP-OCR preprocessing:
    // 1. Composite onto white background (CAIRO_ARGB32 has alpha)
    // 2. BGR channel order (model expects BGR)
    // 3. Normalize to [-1, 1]: (pixel/255 - 0.5) / 0.5 = pixel/127.5 - 1.0
    let mut array = Array4::<f32>::zeros((1, 3, target_h as usize, target_w as usize));
    for y in 0..target_h {
        let src_y = ((y as f64) / scale).min((height - 1) as f64) as u32;
        for x in 0..target_w {
            let src_x = ((x as f64) / scale).min((width - 1) as f64) as u32;
            let off = (src_y * width + src_x) as usize * 4;
            let r = pixels[off + 2] as f32;
            let g = pixels[off + 1] as f32;
            let b = pixels[off] as f32;
            let a = pixels[off + 3] as f32 / 255.0;
            // Alpha composite onto white background
            let inv_a = 1.0 - a;
            let rc = r * a + 255.0 * inv_a;
            let gc = g * a + 255.0 * inv_a;
            let bc = b * a + 255.0 * inv_a;
            // BGR order + normalize to [-1, 1]
            array[[0, 0, y as usize, x as usize]] = bc / 127.5 - 1.0;
            array[[0, 1, y as usize, x as usize]] = gc / 127.5 - 1.0;
            array[[0, 2, y as usize, x as usize]] = rc / 127.5 - 1.0;
        }
    }

    // Run inference
    let input = TensorRef::from_array_view(&array).unwrap();
    let mut session = e.session.lock().unwrap();
    let outputs = session.run(ort::inputs![input]).unwrap();
    let output = &outputs[0];

    // Extract output array: shape [1, T, 18710]
    let arr = output.try_extract_array::<f32>().unwrap();
    let shape = arr.shape();
    let seq_len = shape[1];
    let num_classes = shape[2];

    // CTC greedy decode
    let blank = 0usize;
    let mut prev = blank;
    let mut result = String::new();

    for t in 0..seq_len {
        let mut best = blank;
        let mut best_val = f32::NEG_INFINITY;
        for c in 0..num_classes {
            let v = arr[[0, t, c]];
            if v > best_val {
                best_val = v;
                best = c;
            }
        }
        if best != blank && best != prev {
            if let Some(ch) = e.chars.get(best) {
                result.push_str(ch);
            }
        }
        prev = best;
    }

    // Debug: if result is empty, show first few argmax values
    if result.is_empty() {
        for t in 0..seq_len.min(5) {
            let mut best = blank;
            let mut best_val = f32::NEG_INFINITY;
            for c in 0..num_classes {
                let v = arr[[0, t, c]];
                if v > best_val {
                    best_val = v;
                    best = c;
                }
            }
            eprintln!("[ocr] t={}: best_idx={} best_val={:.4}", t, best, best_val);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recognize_empty() {
        let pixels = vec![0u8; 50 * 30 * 4];
        let text = recognize(&pixels, 50, 30);
        assert!(text.len() < 100, "should not explode");
    }

    /// Feed a synthetic image with a bold black dash on white to test model output.
    #[test]
    fn test_synthetic_dash() {
        let w = 200u32;
        let h = 48u32;
        let mut pixels = vec![255u8; (w * h * 4) as usize]; // white
        // Draw a bold black horizontal line (like a dash) across the middle
        for y in 14..34 {
            for x in 20..180 {
                let off = (y * w + x) as usize * 4;
                pixels[off] = 0;     // B
                pixels[off + 1] = 0; // G
                pixels[off + 2] = 0; // R
                pixels[off + 3] = 255; // A
            }
        }
        let text = recognize(&pixels, w, h);
        eprintln!("[ocr_synthetic] dash recognized: {:?}", text);
        // At minimum should not panic; check if we got some characters
        if text.is_empty() {
            eprintln!("[ocr_synthetic] WARNING: empty result for bold dash!");
        }
    }

    /// Feed a synthetic image with vertical bar (like 'l' or '1')
    #[test]
    fn test_synthetic_vert_bar() {
        let w = 48u32;
        let h = 48u32;
        let mut pixels = vec![255u8; (w * h * 4) as usize];
        // Draw a bold vertical bar at center
        for x in 20..28 {
            for y in 4..44 {
                let off = (y * w + x) as usize * 4;
                pixels[off] = 0;
                pixels[off + 1] = 0;
                pixels[off + 2] = 0;
                pixels[off + 3] = 255;
            }
        }
        let text = recognize(&pixels, w, h);
        eprintln!("[ocr_synthetic] vert bar recognized: {:?}", text);
    }
}
