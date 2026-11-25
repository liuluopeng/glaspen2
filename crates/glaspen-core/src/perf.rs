//! 性能日志(从 Swift `PerfLog` 迁移)。
//!
//! 只有 `GLASPEN2_PERF_LOG=1` 时启用, 写 `~/Library/Logs/glaspen2/perf.log`;
//! 时间戳/耗时用 mach 绝对时间, 与迁移前 Swift 实现同单位(微秒/毫秒)。
#![cfg(target_os = "macos")]

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

#[repr(C)]
struct MachTimebaseInfo {
    numer: u32,
    denom: u32,
}

unsafe extern "C" {
    fn mach_absolute_time() -> u64;
    fn mach_timebase_info(info: *mut MachTimebaseInfo) -> i32;
}

fn timebase() -> (u64, u64) {
    static TB: OnceLock<(u64, u64)> = OnceLock::new();
    *TB.get_or_init(|| {
        let mut info = MachTimebaseInfo { numer: 0, denom: 0 };
        let rc = unsafe { mach_timebase_info(&mut info) };
        if rc == 0 && info.numer > 0 && info.denom > 0 {
            (info.numer as u64, info.denom as u64)
        } else {
            (1, 1)
        }
    })
}

fn now_ticks() -> u64 {
    unsafe { mach_absolute_time() }
}

/// mach 绝对时间差 → 微秒(旧 Swift `PerfLog.elapsedUs` 同公式)。
pub fn elapsed_us(since: u64) -> u64 {
    let (numer, denom) = timebase();
    now_ticks().saturating_sub(since).saturating_mul(numer) / denom / 1000
}

fn timestamp_ms() -> f64 {
    let (numer, denom) = timebase();
    now_ticks() as f64 * numer as f64 / denom as f64 / 1e6
}

fn perf_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Logs/glaspen2/perf.log"))
}

struct PerfState {
    file: Option<BufWriter<File>>,
    total: u64,
    slow: u64,
}

static ENABLED: AtomicBool = AtomicBool::new(false);
static STATE: Mutex<PerfState> = Mutex::new(PerfState {
    file: None,
    total: 0,
    slow: 0,
});

fn begin() {
    let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if st.file.is_some() {
        return;
    }
    let Some(path) = perf_path() else { return };
    if let Some(dir) = path.parent()
        && let Err(e) = std::fs::create_dir_all(dir)
    {
        eprintln!("[glaspen2] perf log dir error: {e}");
    }
    match File::create(&path) {
        Ok(f) => {
            let mut w = BufWriter::new(f);
            let _ = writeln!(w, "ts_ms\ttype\tdur_us\tnotes");
            let _ = w.flush();
            st.file = Some(w);
            eprintln!("[glaspen2] performance log: {}", path.display());
        }
        Err(e) => eprintln!("[glaspen2] perf log open failed: {}: {e}", path.display()),
    }
}

/// `GLASPEN2_PERF_LOG` 非空且非 "0" 时启用并打开文件。
pub fn init_from_env() {
    let Some(v) = std::env::var_os("GLASPEN2_PERF_LOG") else {
        return;
    };
    let s = v.to_string_lossy();
    if s.is_empty() || s == "0" {
        return;
    }
    ENABLED.store(true, Ordering::SeqCst);
    begin();
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}

pub fn record(name: &str, dur_us: u64, notes: Option<&str>) {
    if !is_enabled() {
        return;
    }
    let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let total = st.total.wrapping_add(1);
    st.total = total;
    if dur_us > 16000 {
        st.slow = st.slow.wrapping_add(1);
    }
    let note = notes.unwrap_or(if dur_us > 16000 { "SLOW" } else { "" });
    if let Some(file) = st.file.as_mut() {
        let _ = writeln!(
            file,
            "{:.3}\t{}\t{}\t{}",
            timestamp_ms(),
            name,
            dur_us,
            note
        );
        if total % 100 == 0 {
            let _ = file.flush();
        }
    }
}

pub fn summary() {
    let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let Some(mut file) = st.file.take() else {
        return;
    };
    let pct = if st.total > 0 {
        100.0 * st.slow as f64 / st.total as f64
    } else {
        0.0
    };
    let _ = write!(
        file,
        "\n# SUMMARY: total={} slow={} ({:.1}%)\n",
        st.total, st.slow, pct
    );
    let _ = file.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elapsed_us_of_now_is_small() {
        let t = now_ticks();
        let e = elapsed_us(t);
        assert!(e < 5_000, "同一时刻的耗时异常: {e}us");
    }

    #[test]
    fn disabled_recording_is_noop() {
        // 测试进程不设 GLASPEN2_PERF_LOG; record/summary 必须安全 no-op。
        assert!(!is_enabled());
        record("test", 1, None);
        summary();
    }
}
