//! On-demand download of the PaddleOCR models (PaddleOCR, hosted on
//! HuggingFace). Models are stored in the app-support models dir and are
//! verified against known sha256 hashes before being activated, so an
//! interrupted or corrupted download can never break OCR silently.

use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Mutex;

pub const DET_FILENAME: &str = "ppocr_v6_det.onnx";
pub const REC_FILENAME: &str = "ppocr_v6_rec.onnx";

// sha256 of the original model files (compute from your local copies).
// If the HuggingFace files are byte-identical to yours, the download
// passes verification automatically.
pub const DET_SHA256: &str = "eb13b44b25bb36f89528b68720af8a61d9cf381176107f465db1757b65d086e1";
pub const REC_SHA256: &str = "9c09abf0957f7968c7586464b7397b84ad2387a0497a351af40e9acc71b673ba";

// Fallback expected sizes (bytes) for progress estimation when a server
// does not send Content-Length.
pub const DET_EXPECTED_BYTES: u64 = 62032837;
pub const REC_EXPECTED_BYTES: u64 = 76554979;

// ── HuggingFace download URLs (set these to your model repo) ──
// Use "resolve/main" so the server redirects to the actual file.
const DET_URL: &str = "https://huggingface.co/<your-repo>/resolve/main/ppocr_v6_det.onnx";
const REC_URL: &str = "https://huggingface.co/<your-repo>/resolve/main/ppocr_v6_rec.onnx";

// ── Download state (read by ObjC/C# via FFI) ──
// PROGRESS: -200 failed, -100 idle, 0..=100 percent (100 = done)
const PROGRESS_IDLE: i32 = -100;
const PROGRESS_FAILED: i32 = -200;
static PROGRESS: AtomicI32 = AtomicI32::new(PROGRESS_IDLE);
// DL_STATE: 0 idle, 1 running, 2 done, 3 failed
static DL_STATE: AtomicI32 = AtomicI32::new(0);
static LAST_ERROR: Mutex<String> = Mutex::new(String::new());

/// Directory the downloaded models live in.
pub fn models_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("glaspen2")
                .join("models");
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(profile) = std::env::var("USERPROFILE") {
            return PathBuf::from(profile)
                .join("AppData")
                .join("Roaming")
                .join("glaspen2")
                .join("models");
        }
    }
    PathBuf::from(".")
}

pub fn models_present() -> bool {
    models_dir().join(DET_FILENAME).exists() && models_dir().join(REC_FILENAME).exists()
}

/// Result of an ensure_models() call.
#[derive(Debug, PartialEq)]
pub enum EnsureResult {
    Ready,          // models already present
    Started,        // download was started by this call
    AlreadyRunning, // a download is already in progress
    Failed,         // could not start the download
}

/// Kick off the download if the models are missing. Safe to call from any
/// thread and repeatedly — the compare_exchange guard prevents duplicates.
pub fn ensure_models() -> EnsureResult {
    if models_present() {
        return EnsureResult::Ready;
    }
    // 0 -> 1 only if nothing is running or finished.
    let prev = DL_STATE
        .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
        .unwrap_or_else(|v| v);
    if prev != 0 {
        return if prev == 1 {
            EnsureResult::AlreadyRunning
        } else if prev == 2 && models_present() {
            // finished successfully earlier in this session
            EnsureResult::Ready
        } else {
            // finished with failure — allow a retry
            if DL_STATE.compare_exchange(prev, 1, Ordering::AcqRel, Ordering::Acquire).is_err() {
                return EnsureResult::AlreadyRunning;
            }
            EnsureResult::Started
        };
    }

    PROGRESS.store(0, Ordering::Relaxed);
    std::thread::spawn(|| {
        let result = download_all();
        match result {
            Ok(()) => {
                PROGRESS.store(100, Ordering::Relaxed);
                DL_STATE.store(2, Ordering::Release);
            }
            Err(e) => {
                *LAST_ERROR.lock().unwrap() = e.clone();
                PROGRESS.store(PROGRESS_FAILED, Ordering::Relaxed);
                DL_STATE.store(3, Ordering::Release);
            }
        }
    });
    EnsureResult::Started
}

/// Current overall download progress: -1 idle, -2 failed, 0..100 percent.
pub fn progress() -> f64 {
    match PROGRESS.load(Ordering::Relaxed) {
        PROGRESS_IDLE => -1.0,
        PROGRESS_FAILED => -2.0,
        p => p as f64,
    }
}

pub fn last_error() -> String {
    LAST_ERROR.lock().unwrap().clone()
}

fn download_all() -> Result<(), String> {
    let dir = models_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("无法创建模型目录 {}: {}", dir.display(), e))?;

    download_one(DET_URL, DET_FILENAME, DET_SHA256, DET_EXPECTED_BYTES, &dir, 0.0, 0.5)?;
    download_one(REC_URL, REC_FILENAME, REC_SHA256, REC_EXPECTED_BYTES, &dir, 0.5, 1.0)?;
    Ok(())
}

fn download_one(
    url: &str,
    filename: &str,
    expected_sha256: &str,
    expected_bytes: u64,
    dir: &PathBuf,
    span_start: f64,
    span_end: f64,
) -> Result<(), String> {
    let tmp = dir.join(format!("{}.part", filename));
    let dest = dir.join(filename);

    let resp = ureq::get(url)
        .call()
        .map_err(|e| format!("{} 下载失败: {}", filename, e))?;
    let total: u64 = resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or(expected_bytes);
    let mut reader = resp.into_body().into_reader();

    let mut file = std::fs::File::create(&tmp)
        .map_err(|e| format!("无法写入 {}: {}", tmp.display(), e))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut written: u64 = 0;
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("{} 读取失败: {}", filename, e))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .map_err(|e| format!("{} 写入失败: {}", filename, e))?;
        hasher.update(&buf[..n]);
        written += n as u64;
        if total > 0 {
            let frac = (written as f64 / total as f64).min(1.0);
            let pct = span_start + (span_end - span_start) * frac;
            PROGRESS.store((pct * 100.0) as i32, Ordering::Relaxed);
        }
    }

    if written != total {
        return Err(format!("{} 下载不完整: 期望 {} 字节, 实际 {}", filename, total, written));
    }
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|b| format!("{:02x}", b)).collect();
    if hex != expected_sha256 {
        std::fs::remove_file(&tmp).ok();
        return Err(format!("{} 校验失败 (sha256 不匹配): 期望 {} 实际 {}", filename, expected_sha256, hex));
    }

    std::fs::rename(&tmp, &dest)
        .map_err(|e| format!("无法移动 {}: {}", tmp.display(), e))?;
    Ok(())
}
