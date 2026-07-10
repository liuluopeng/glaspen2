//! OCR module — PP-OCRv6 detection + recognition via ONNX Runtime.
//!
//! Requires model files at runtime:
//!   models/ppocr_v6_rec.onnx    (recognition, 73 MB)
//!   models/ppocr_v6_det.onnx    (detection, 62 MB)
//!   models/ppocr_v6_dict.json   (character dict)

pub mod det;
pub mod rec;

pub use det::detect_and_recognize;
pub use rec::recognize;
