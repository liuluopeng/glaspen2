//! Detection model — PP-OCRv6_medium_det (DBNet).
//! Finds text regions in an image, then recognizes each with the rec model.

use ndarray::Array4;
use ort::session::Session;
use ort::value::TensorRef;
use std::sync::{Mutex, OnceLock};

use super::rec;

static DET_ENGINE: OnceLock<Mutex<Session>> = OnceLock::new();

fn det_session() -> &'static Mutex<Session> {
    DET_ENGINE.get_or_init(|| {
        let model_file = rec::model_path("ppocr_v6_det.onnx");
        let session = Session::builder()
            .unwrap()
            .commit_from_file(&model_file)
            .unwrap_or_else(|e| panic!("Failed to load det model: {}", e));
        Mutex::new(session)
    })
}

// ── Preprocessing (ImageNet normalization: mean, std, BGR) ──

const DET_MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const DET_STD: [f32; 3] = [0.229, 0.224, 0.225];
const DET_MAX_SIDE: u32 = 960;

/// Resize image so the longest side ≤ DET_MAX_SIDE, maintaining aspect ratio,
/// and dimensions are multiples of 32 (required by PPLCNetV4 backbone).
fn det_resize(w: u32, h: u32) -> (u32, u32, f32) {
    let max = w.max(h) as f32;
    let scale = if max <= DET_MAX_SIDE as f32 { 1.0 } else { DET_MAX_SIDE as f32 / max };
    let nw = ((w as f32 * scale).ceil() as u32 + 31) / 32 * 32;
    let nh = ((h as f32 * scale).ceil() as u32 + 31) / 32 * 32;
    (nw.max(32), nh.max(32), scale)
}

/// Preprocess RGBA image for detection model: composite onto white, BGR, resize, normalise.
fn det_preprocess(pixels: &[u8], w: u32, h: u32) -> (Array4<f32>, u32, u32, f32) {
    let (nw, nh, scale) = det_resize(w, h);
    let mut array = Array4::<f32>::zeros((1, 3, nh as usize, nw as usize));

    for y in 0..nh {
        let src_y = ((y as f32) / scale).min((h - 1) as f32) as u32;
        for x in 0..nw {
            let src_x = ((x as f32) / scale).min((w - 1) as f32) as u32;
            let off = (src_y * w + src_x) as usize * 4;
            let r = pixels[off + 2] as f32;
            let g = pixels[off + 1] as f32;
            let b = pixels[off] as f32;
            let a = pixels[off + 3] as f32 / 255.0;
            let inv_a = 1.0 - a;
            let rc = (r * a + 255.0 * inv_a) / 255.0;
            let gc = (g * a + 255.0 * inv_a) / 255.0;
            let bc = (b * a + 255.0 * inv_a) / 255.0;
            // BGR + ImageNet normalize
            array[[0, 0, y as usize, x as usize]] = (bc - DET_MEAN[0]) / DET_STD[0];
            array[[0, 1, y as usize, x as usize]] = (gc - DET_MEAN[1]) / DET_STD[1];
            array[[0, 2, y as usize, x as usize]] = (rc - DET_MEAN[2]) / DET_STD[2];
        }
    }
    (array, nw, nh, scale)
}

// ── DB post-processing ──

/// A detected text region
#[derive(Debug, Clone)]
pub struct TextBox {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub score: f32,
}

/// Simple 8-direction connected-component labelling on a binary map.
fn connected_components(map: &[u8], w: usize, h: usize) -> Vec<TextBox> {
    let mut labels = vec![0u32; w * h];
    let mut next_label = 1u32;
    // Equivalent classes for union-find
    let mut eq = Vec::new();

    // First pass
    for y in 0..h {
        for x in 0..w {
            if map[y * w + x] == 0 { continue; }
            let idx = y * w + x;

            // Check 4-connected neighbours (up, left)
            let mut neighbor_labels = Vec::new();
            if y > 0 && labels[(y - 1) * w + x] != 0 {
                neighbor_labels.push(labels[(y - 1) * w + x]);
            }
            if x > 0 && labels[y * w + (x - 1)] != 0 {
                neighbor_labels.push(labels[y * w + (x - 1)]);
            }

            if neighbor_labels.is_empty() {
                labels[idx] = next_label;
                eq.push(next_label);
                next_label += 1;
            } else {
                let min_lab = *neighbor_labels.iter().min().unwrap();
                labels[idx] = min_lab;
                for &nl in &neighbor_labels {
                    if nl != min_lab {
                        // Union: make nl's root point to min_lab's root
                        let root_nl = find_root(nl, &mut eq);
                        let root_min = find_root(min_lab, &mut eq);
                        if root_nl != root_min {
                            eq[root_nl as usize - 1] = root_min;
                        }
                    }
                }
            }
        }
    }

    // Second pass: resolve labels
    for y in 0..h {
        for x in 0..w {
            if map[y * w + x] == 0 { continue; }
            let idx = y * w + x;
            labels[idx] = find_root(labels[idx], &mut eq);
        }
    }

    // Collect bbox per component
    use std::collections::HashMap;
    let mut comps: HashMap<u32, (u32, u32, u32, u32, u32)> = HashMap::new();
    for y in 0..h {
        for x in 0..w {
            let lab = labels[y * w + x];
            if lab == 0 { continue; }
            let e = comps.entry(lab).or_insert((x as u32, y as u32, x as u32, y as u32, 0));
            e.0 = e.0.min(x as u32);
            e.1 = e.1.min(y as u32);
            e.2 = e.2.max(x as u32);
            e.3 = e.3.max(y as u32);
            e.4 += 1;
        }
    }

    // Filter small components and convert to TextBox
    let min_area = 9; // 3x3 pixels minimum
    let mut boxes: Vec<TextBox> = comps.into_iter()
        .filter(|(_, (_, _, _, _, area))| *area >= min_area)
        .map(|(_, (x1, y1, x2, y2, _))| {
            let bw = x2 - x1 + 1;
            let bh = y2 - y1 + 1;
            TextBox { x: x1, y: y1, w: bw, h: bh, score: 0.0 }
        })
        .collect();

    // Merge overlapping boxes
    merge_overlapping(&mut boxes);
    boxes
}

fn find_root(mut lab: u32, eq: &mut [u32]) -> u32 {
    let idx = lab as usize - 1;
    while eq[idx] != lab {
        eq[idx] = eq[eq[idx] as usize - 1];
        lab = eq[idx];
    }
    lab
}

/// Merge boxes that overlap significantly.
fn merge_overlapping(boxes: &mut Vec<TextBox>) {
    if boxes.len() < 2 { return; }
    loop {
        let mut merged = false;
        let mut i = 0;
        while i < boxes.len() {
            let mut j = i + 1;
            while j < boxes.len() {
                let a = &boxes[i];
                let b = &boxes[j];
                let ix = a.x.max(b.x);
                let iy = a.y.max(b.y);
                let iw = (a.x + a.w).min(b.x + b.w).saturating_sub(ix);
                let ih = (a.y + a.h).min(b.y + b.h).saturating_sub(iy);
                let overlap = (iw * ih) as f32;
                let area_a = (a.w * a.h) as f32;
                let area_b = (b.w * b.h) as f32;
                // Merge if overlap > 40% of either box or if vertically close & horizontally overlapping
                let vert_close = (a.y.max(b.y) as i32 - (a.y as i32 + a.h as i32).min(b.y as i32 + b.h as i32)).abs() < (a.h.max(b.h) as i32 / 2);
                let horiz_overlap = iw > 0;
                if overlap > area_a * 0.4 || overlap > area_b * 0.4 || (vert_close && horiz_overlap) {
                    // Merge j into i
                    let new_x = boxes[i].x.min(boxes[j].x);
                    let new_y = boxes[i].y.min(boxes[j].y);
                    let new_x2 = (boxes[i].x + boxes[i].w).max(boxes[j].x + boxes[j].w);
                    let new_y2 = (boxes[i].y + boxes[i].h).max(boxes[j].y + boxes[j].h);
                    boxes[i] = TextBox {
                        x: new_x, y: new_y,
                        w: new_x2 - new_x, h: new_y2 - new_y,
                        score: boxes[i].score.max(boxes[j].score),
                    };
                    boxes.remove(j);
                    merged = true;
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
        if !merged { break; }
    }
}

/// Run detection on RGBA image, return list of text boxes.
/// Box coordinates are in the original image space.
pub fn detect_text_regions(pixels: &[u8], width: u32, height: u32) -> Vec<TextBox> {
    let session = det_session();
    let (input_tensor, _nw, _nh, _scale) = det_preprocess(pixels, width, height);
    eprintln!("[det] input {}x{} resized to {}x{} scale={:.3}", width, height, _nw, _nh, _scale);

    let input = TensorRef::from_array_view(&input_tensor).unwrap();
    let mut sess = session.lock().unwrap();
    let outputs = sess.run(ort::inputs![input]).unwrap();
    let output = &outputs[0];

    let arr = output.try_extract_array::<f32>().unwrap();
    let shape = arr.shape(); // [1, 1, H/4, W/4]
    let oh = shape[2];
    let ow = shape[3];

    // Model output is already sigmoid-ed probability map in [0, 1]
    let thresh = 0.3;
    let mut binmap = vec![0u8; (ow * oh) as usize];
    let mut max_prob = 0.0f32;
    let mut nonzero = 0u32;
    for y in 0..oh {
        for x in 0..ow {
            let v = arr[[0, 0, y, x]];
            if v > max_prob { max_prob = v; }
            if v > thresh { nonzero += 1; }
            binmap[y * ow + x] = if v > thresh { 1 } else { 0 };
        }
    }
    eprintln!("[det] output {}x{}, max_prob={:.4}, pixels_above_thresh={}/{}", ow, oh, max_prob, nonzero, ow * oh);

    // Find connected components
    let boxes_downscaled = connected_components(&binmap, ow, oh);
    eprintln!("[det] connected components before filter: {}", boxes_downscaled.len());

    // Scale boxes back to original image coordinates
    let ups_x = width as f32 / ow as f32;
    let ups_y = height as f32 / oh as f32;

    let mut text_boxes: Vec<TextBox> = boxes_downscaled.into_iter()
        .filter(|b| {
            // Filter by aspect ratio (typical text is wider than tall)
            b.w >= b.h / 2
        })
        .map(|b| {
            let bx = (b.x as f32 * ups_x).floor() as u32;
            let by = (b.y as f32 * ups_y).floor() as u32;
            let bw = ((b.x + b.w) as f32 * ups_x).ceil() as u32 - bx;
            let bh = ((b.y + b.h) as f32 * ups_y).ceil() as u32 - by;
            TextBox { x: bx, y: by, w: bw, h: bh, score: b.score }
        })
        .collect();

    // Sort top-to-bottom, then left-to-right
    text_boxes.sort_by(|a, b| a.y.cmp(&b.y).then(a.x.cmp(&b.x)));
    text_boxes
}

/// Crop a rectangular region from RGBA pixels.
fn crop_pixels(pixels: &[u8], src_w: u32, x: u32, y: u32, w: u32, h: u32) -> Vec<u8> {
    let mut cropped = Vec::with_capacity((w * h * 4) as usize);
    for row in y..y + h {
        let src_off = (row * src_w + x) as usize * 4;
        let end = src_off + w as usize * 4;
        cropped.extend_from_slice(&pixels[src_off..end.min(pixels.len())]);
    }
    cropped
}

/// Full pipeline: detect text regions → recognize each → return concatenated text.
pub fn detect_and_recognize(pixels: &[u8], width: u32, height: u32) -> String {
    let boxes = detect_text_regions(pixels, width, height);
    if boxes.is_empty() {
        // Fallback: try recognition on the full image
        return rec::recognize(pixels, width, height);
    }

    let mut results = Vec::new();
    for tb in &boxes {
        // Add padding
        let pad = 4u32;
        let x = tb.x.saturating_sub(pad);
        let y = tb.y.saturating_sub(pad);
        let w = (tb.w + pad * 2).min(width - x);
        let h = (tb.h + pad * 2).min(height - y);
        if w < 4 || h < 4 { continue; }

        let crop = crop_pixels(pixels, width, x, y, w, h);
        let text = rec::recognize(&crop, w, h);
        if !text.is_empty() {
            results.push(text);
        }
    }

    results.join("\n")
}
