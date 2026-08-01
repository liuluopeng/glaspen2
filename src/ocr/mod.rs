//! OCR module — PP-OCRv6 detection + recognition via ONNX Runtime.
//!
//! The two ONNX models (~135 MB) are NOT bundled anymore; they are
//! downloaded on demand from HuggingFace into the app-support models dir
//! (see `download`). Only the small dict stays bundled.

pub mod det;
pub mod download;
pub mod rec;

pub use det::detect_and_recognize;
pub use rec::recognize;
