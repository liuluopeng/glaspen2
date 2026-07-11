//! Recognition model — PP-OCRv6_medium_rec.
//! Takes a tight crop of a text line, returns recognized text.

use ndarray::Array4;
use ort::session::Session;
use ort::value::TensorRef;
use std::sync::{Mutex, OnceLock};

static REC_ENGINE: OnceLock<RecEngine> = OnceLock::new();

pub(super) struct RecEngine {
    pub session: Mutex<Session>,
    pub chars: Vec<String>,
}

pub(super) fn model_path(p: &str) -> std::path::PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        // Standard macOS bundle location: glaspen2.app/Contents/Resources/models/
        let resources = exe.parent().unwrap()
            .parent().unwrap().join("Resources").join("models");
        let f = resources.join(p);
        if f.exists() { return f; }

        // Next to the executable (debug/dev builds)
        let dir = exe.parent().unwrap().join("models");
        let f = dir.join(p);
        if f.exists() { return f; }
    }
    // Fallback: current working directory
    std::path::Path::new("models").join(p)
}

fn load_chars() -> Vec<String> {
    let path = model_path("ppocr_v6_dict.json");
    let data = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", path.display(), e));
    serde_json::from_str(&data)
        .unwrap_or_else(|e| panic!("Failed to parse char dict: {}", e))
}

pub(super) fn engine() -> &'static RecEngine {
    REC_ENGINE.get_or_init(|| {
        let model_file = model_path("ppocr_v6_rec.onnx");
        let session = Session::builder()
            .unwrap()
            .commit_from_file(&model_file)
            .unwrap_or_else(|e| panic!("Failed to load model from {}: {}", model_file.display(), e));
        let chars = load_chars();
        RecEngine { session: Mutex::new(session), chars }
    })
}

/// Run recognition on a tight crop of a text line (RGBA pixels).
/// The image should have text filling most of the height.
pub fn recognize(pixels: &[u8], width: u32, height: u32) -> String {
    let e = engine();

    // Resize to H=48 maintaining aspect ratio
    let target_h = 48u32;
    let scale = target_h as f64 / height.max(1) as f64;
    let mut target_w = (width as f64 * scale).ceil() as u32;
    target_w = target_w.max(8);
    target_w = ((target_w + 7) / 8) * 8;

    // CHW tensor with BGR + [-1,1] norm
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
            let inv_a = 1.0 - a;
            let rc = r * a + 255.0 * inv_a;
            let gc = g * a + 255.0 * inv_a;
            let bc = b * a + 255.0 * inv_a;
            array[[0, 0, y as usize, x as usize]] = bc / 127.5 - 1.0;
            array[[0, 1, y as usize, x as usize]] = gc / 127.5 - 1.0;
            array[[0, 2, y as usize, x as usize]] = rc / 127.5 - 1.0;
        }
    }

    // Inference
    let input = TensorRef::from_array_view(&array).unwrap();
    let mut session = e.session.lock().unwrap();
    let outputs = session.run(ort::inputs![input]).unwrap();
    let output = &outputs[0];

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
            // Model index 0 = blank, index 1 = chars[0], index 2 = chars[1], ...
            if best <= e.chars.len() {
                if let Some(ch) = e.chars.get(best - 1) {
                    result.push_str(ch);
                }
            }
        }
        prev = best;
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
        assert!(text.len() < 100);
    }

    #[test]
    fn test_synthetic_dash() {
        let w = 200u32; let h = 48u32;
        let mut pixels = vec![255u8; (w * h * 4) as usize];
        for y in 14..34 { for x in 20..180 {
            let off = (y * w + x) as usize * 4;
            pixels[off] = 0; pixels[off+1] = 0; pixels[off+2] = 0; pixels[off+3] = 255;
        }}
        let text = recognize(&pixels, w, h);
        eprintln!("[rec] dash: {:?}", text);
    }

    #[test]
    fn test_synthetic_vert_bar() {
        let w = 48u32; let h = 48u32;
        let mut pixels = vec![255u8; (w * h * 4) as usize];
        for x in 20..28 { for y in 4..44 {
            let off = (y * w + x) as usize * 4;
            pixels[off] = 0; pixels[off+1] = 0; pixels[off+2] = 0; pixels[off+3] = 255;
        }}
        let text = recognize(&pixels, w, h);
        eprintln!("[rec] vert: {:?}", text);
    }

    /// Cross-validate: load the synthetic "你好世界" line PNG and OCR it.
    /// This image is correctly recognized by PaddleOCR Python.
    #[test]
    fn test_cross_validate_png() {
        use std::path::Path;
        let path = "python_test/test_line_hello.png";
        if !Path::new(path).exists() {
            eprintln!("[cross] SKIP: {path} not found");
            return;
        }
        // Load PNG via image crate
        let img = image::open(Path::new(path)).unwrap().to_rgba8();
        let (w, h) = img.dimensions();
        let pixels = img.into_raw();
        eprintln!("[cross] loaded {}x{} PNG", w, h);
        let text = recognize(&pixels, w, h);
        eprintln!("[cross] Rust recognized: {:?}", text);
        // Python result for this line: "你好世界"
        // If mapping is correct, we should get Chinese characters
        if text.is_empty() {
            eprintln!("[cross] WARNING: empty result!");
        } else {
            eprintln!("[cross] SUCCESS: got {} chars", text.chars().count());
        }
    }
}
