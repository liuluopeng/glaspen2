//! 日志落盘与崩溃记录。
//!
//! stderr 层(`cargo run` 终端直读)之外增加**按天滚动**的文件层 ——
//! 从 Finder 启动时 stderr 直接丢掉, 文件是排查线上问题的唯一现场:
//! - macOS: `~/Library/Logs/glaspen2/glaspen2.log.<日期>`
//! - Windows: `%LOCALAPPDATA%/glaspen2/logs/…`
//!
//! panic hook 把 backtrace 写成 `glaspen2-crash-*.log`(保留最近 10 份)。
//! 只覆盖 Rust 侧 panic;ObjC NSException/段错误走系统崩溃报告,不在此列。
//!
//! 目录可用 `GLASPEN2_LOG_DIR` 覆盖(测试与排障)。文件层创建失败只降级
//! 不 panic —— HOME 只读也不能挡着程序启动。

use std::path::PathBuf;

/// 日志目录。测试/排障可经 `GLASPEN2_LOG_DIR` 覆盖。
pub fn log_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("GLASPEN2_LOG_DIR") {
        return PathBuf::from(d);
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join("Library/Logs/glaspen2")
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA")
            .map(|d| PathBuf::from(d).join("glaspen2").join("logs"))
            .unwrap_or_else(|| PathBuf::from("glaspen2-logs"))
    }
}

/// 初始化 tracing:stderr 层(行为与旧版一致)+ 按天滚动文件层。
///
/// 过滤优先级不变:`RUST_LOG` > `GLASPEN2_DB_LOG=1` → debug > 默认 info。
/// 同一过滤同时作用于两层。进程内只能调用一次(`.init()` panic 语义)。
pub fn init() {
    use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

    let default = if std::env::var_os("GLASPEN2_DB_LOG").is_some() {
        "debug"
    } else {
        "info"
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default));

    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_target(true);

    let file_layer = build_appender(&log_dir()).map(|appender| {
        let (writer, guard) = tracing_appender::non_blocking(appender);
        // guard 泄漏到进程退出:非阻塞写线程的生命周期 = 进程生命周期,
        // 本应用没有"中途关闭日志"的需求, 泄漏即正确。
        std::mem::forget(guard);
        tracing_subscriber::fmt::layer()
            .with_writer(writer)
            .with_ansi(false)
            .with_target(true)
    });

    tracing_subscriber::registry()
        .with(filter)
        .with(stderr_layer)
        .with(file_layer)
        .init();
}

/// 构建按天滚动的文件 appender;失败返回 None(降级为只有 stderr)。
/// 此时 subscriber 尚未初始化, 只能 eprintln(引导期特例)。
fn build_appender(dir: &std::path::Path) -> Option<tracing_appender::rolling::RollingFileAppender> {
    if let Err(e) = std::fs::create_dir_all(dir) {
        eprintln!("日志目录创建失败({e}), 本次运行不落盘:{}", dir.display());
        return None;
    }
    match tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("glaspen2.log")
        .build(dir)
    {
        Ok(a) => Some(a),
        Err(e) => {
            eprintln!("日志文件打开失败({e}), 本次运行不落盘:{}", dir.display());
            None
        }
    }
}

/// 安装 panic hook:崩溃信息(含 backtrace)写入 log_dir,然后调用原 hook
/// (保留 stderr 行为)。被 `catch_unwind` 接住的 panic 也会触发 hook ——
/// 文件里多一份记录无害。
pub fn install_panic_hook() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let thread = thread.name().unwrap_or("<unnamed>").to_string();
        let location = info.location().map(|l| l.to_string()).unwrap_or_default();
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "<非字符串 payload>".to_string()
        };
        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        write_crash_log(
            &log_dir(),
            &thread,
            &location,
            &payload,
            &backtrace,
            env!("CARGO_PKG_VERSION"),
        );
        prev(info);
    }));
}

/// 写一份崩溃报告并轮转(保留最近 [`CRASH_KEEP`] 份)。
/// 独立成函数以便单测直接驱动,不依赖真实 panic。
fn write_crash_log(
    dir: &std::path::Path,
    thread: &str,
    location: &str,
    payload: &str,
    backtrace: &str,
    version: &str,
) -> Option<PathBuf> {
    use std::fmt::Write as _;
    if std::fs::create_dir_all(dir).is_err() {
        return None;
    }
    let name = format!("glaspen2-crash-{}", crate::timestamped_name("log"));
    let path = dir.join(name);
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut body = String::new();
    let _ = write!(
        body,
        "glaspen2 {version} 崩溃报告 (unix 时间 {secs})\n\
         线程: {thread}\n\
         位置: {location}\n\
         信息: {payload}\n\
         \nbacktrace:\n{backtrace}\n"
    );
    std::fs::write(&path, body).ok()?;

    // 轮转:文件名含 年-年内天号_时分秒, 字典序即时间序
    let mut crashes: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("glaspen2-crash-"))
        })
        .collect();
    crashes.sort();
    while crashes.len() > CRASH_KEEP {
        let oldest = crashes.remove(0);
        let _ = std::fs::remove_file(oldest);
    }
    Some(path)
}

const CRASH_KEEP: usize = 10;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crash_log_written_and_rotated() {
        let dir = std::env::temp_dir().join(format!("glaspen2_crash_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // 先铺 12 份"旧"报告(文件名字典序 = 时间序)
        for i in 0..12 {
            std::fs::write(
                dir.join(format!(
                    "glaspen2-crash-glaspen2_2026-200_{:02}-00-00.log",
                    i
                )),
                "old",
            )
            .unwrap();
        }
        let path = write_crash_log(&dir, "main", "src/x.rs:1", "boom", "frame", "0.5.2")
            .expect("应写出崩溃文件");
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("boom") && body.contains("src/x.rs:1") && body.contains("0.5.2"));

        let remaining: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(remaining.len(), CRASH_KEEP, "应只保留最近 10 份");
        // 新写的这份(时间最新)必须还在;
        // 字典序最小的 3 份(200_00..200_02)应被清掉
        assert!(path.exists());
        assert!(
            !dir.join("glaspen2-crash-glaspen2_2026-200_00-00-00.log")
                .exists()
        );
        assert!(
            !dir.join("glaspen2-crash-glaspen2_2026-200_02-00-00.log")
                .exists()
        );
        assert!(
            dir.join("glaspen2-crash-glaspen2_2026-200_03-00-00.log")
                .exists()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn init_writes_file_and_env_override_works() {
        // 合成一个测试:env 覆盖与 init() 都动全局状态, 分开写会互相竞争。
        let dir = std::env::temp_dir().join(format!("glaspen2_log_init_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // SAFETY: 本测试是唯一动 GLASPEN2_LOG_DIR、唯一调 init() 的测试。
        unsafe { std::env::set_var("GLASPEN2_LOG_DIR", &dir) };
        assert_eq!(log_dir(), dir, "GLASPEN2_LOG_DIR 应覆盖默认目录");

        init();
        tracing::info!("落盘冒烟: hello file layer");
        tracing::warn!("落盘冒烟: warn level");
        // 非阻塞写线程需要一点时间把队列刷进文件
        std::thread::sleep(std::time::Duration::from_millis(300));

        let files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .expect("日志目录应已创建")
            .flatten()
            .map(|e| e.path())
            .collect();
        assert!(!files.is_empty(), "应有按天滚动的日志文件");
        let body = std::fs::read_to_string(&files[0]).unwrap();
        assert!(
            body.contains("hello file layer"),
            "info 应写入文件: {body:?}"
        );
        assert!(
            body.contains("WARN"),
            "文件层应有级别字段(无 ANSI): {body:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
