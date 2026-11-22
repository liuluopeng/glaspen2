//! glaspen2 自解压安装器(仅依赖 flate2)。
//!
//! 文件格式:[installer.exe][payload.tar.gz][len u64 LE][MAGIC 12B]
//! MAGIC 位于文件绝对末尾,其前 8 字节为 payload 长度,
//! 精确定位,与 payload 内部字节序列无关。
//!
//! 打包:scripts/make_installer.ps1。
//! 运行时:读取自身尾部 tar.gz,解压到 %LOCALAPPDATA%\glaspen2,
//! 创建开始菜单快捷方式,并启动 glaspen2.exe。

#![windows_subsystem = "windows"]

use std::io::Read;

const MAGIC: &[u8; 12] = b"GLASPEN2PKGX";
const SHOW_NORMAL: i32 = 1;

#[link(name = "user32")]
unsafe extern "system" {
    fn MessageBoxW(hwnd: usize, text: *const u16, caption: *const u16, flags: u32) -> i32;
}

#[link(name = "shell32")]
unsafe extern "system" {
    fn ShellExecuteW(
        hwnd: usize,
        op: *const u16,
        file: *const u16,
        params: *const u16,
        dir: *const u16,
        show: i32,
    ) -> isize;
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn msgbox(text: &str, caption: &str) {
    unsafe {
        let t = wide(text);
        let c = wide(caption);
        MessageBoxW(0, t.as_ptr(), c.as_ptr(), 0x10 /* MB_ICONERROR */);
    }
}

fn append_log(msg: &str) {
    use std::io::Write;
    let path = std::env::temp_dir().join("glaspen_installer.log");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{msg}");
    }
}

/// 请求正在运行的旧版本退出(WM_CLOSE → 覆盖层收尾并结束面板进程)。
fn request_running_instances_close() {
    for image in ["glaspen2.exe", "glaspen2_settings.exe"] {
        let _ = std::process::Command::new("taskkill")
            .args(["/IM", image])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        append_log(&format!("request close: {image}"));
    }
}

/// 强杀残留进程(正常关闭超时后兜底)。
fn force_kill_running_instances() {
    for image in ["glaspen2.exe", "glaspen2_settings.exe"] {
        let _ = std::process::Command::new("taskkill")
            .args(["/F", "/IM", image])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        append_log(&format!("force kill: {image}"));
    }
}

/// 文件能否以写方式打开(= 没有被旧进程占用)。不存在也算可用。
fn file_writable(path: &std::path::Path) -> bool {
    match std::fs::OpenOptions::new().write(true).open(path) {
        Ok(_) => true,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
        Err(_) => false,
    }
}

fn wait_file_writable(path: &std::path::Path, timeout: std::time::Duration) -> bool {
    let start = std::time::Instant::now();
    loop {
        if file_writable(path) {
            return true;
        }
        if start.elapsed() > timeout {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// 解压时文件可能刚被释放锁, 给几次重试机会(以前直接 continue 会静默跳过,
/// 结果就是"安装了正式版还是旧版")。
fn create_file_retry(path: &std::path::Path) -> std::io::Result<std::fs::File> {
    let mut last = None;
    for _ in 0..20 {
        match std::fs::File::create(path) {
            Ok(f) => return Ok(f),
            Err(e) => {
                last = Some(e);
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    }
    Err(last.unwrap_or_else(|| std::io::Error::other("create failed")))
}

fn octal_field(field: &[u8]) -> u64 {
    let mut v: u64 = 0;
    for &b in field {
        if b == 0 || b == b' ' {
            break;
        }
        if b == b'0' && v == 0 {
            continue;
        }
        if !(b'0'..=b'7').contains(&b) {
            break;
        }
        v = v * 8 + (b - b'0') as u64;
    }
    v
}

/// 解压 tar.gz 到 dest。Err = 有关键文件写不进去(通常是旧版本还在运行占着 exe)。
fn extract_tar_gz(data: &[u8], dest: &std::path::Path) -> Result<(), String> {
    use std::io::Write;

    // Gzip 解压
    let mut out = Vec::new();
    {
        let mut decoder = flate2::read::GzDecoder::new(data);
        if decoder.read_to_end(&mut out).is_err() {
            return Err("解压失败".to_string());
        }
    }

    // 解析 tar(512 字节块)
    let mut cur = 0usize;
    let mut files = 0u32;
    let mut failed: Vec<String> = Vec::new();
    while cur + 512 <= out.len() {
        let header = &out[cur..cur + 512];
        // 全零块 = 结尾
        if header.iter().all(|&b| b == 0) {
            break;
        }
        let name_end = header[..100].iter().position(|&b| b == 0).unwrap_or(100);
        let name = String::from_utf8_lossy(&header[..name_end]).to_string();
        let typeflag = header[156];
        let size = octal_field(&header[124..136]) as usize;
        cur += 512;
        let padded = (size + 511) & !511;

        if name.is_empty() || name.contains("..") {
            cur += padded;
            continue;
        }
        let rel = std::path::Path::new(&name);
        let out_path = dest.join(rel);

        match typeflag {
            b'5' => {
                let _ = std::fs::create_dir_all(&out_path);
            }
            _ => {
                if let Some(parent) = out_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if cur + size <= out.len() {
                    match create_file_retry(&out_path) {
                        Ok(mut f) => {
                            let _ = f.write_all(&out[cur..cur + size]);
                            files += 1;
                        }
                        Err(e) => failed.push(format!("{name}: {e}")),
                    }
                }
            }
        }
        cur += padded;
    }
    if !failed.is_empty() {
        let critical = failed.iter().any(|s| {
            s.starts_with("glaspen2.exe")
                || s.starts_with("glaspen2.dll")
                || s.starts_with("glaspen2_settings.exe")
        });
        append_log(&format!("extract failed files: {failed:?}"));
        if critical {
            return Err(format!(
                "无法覆盖程序文件(旧版本可能仍在运行): {}",
                failed.join("; ")
            ));
        }
    }
    append_log(&format!("extract ok: {files} files"));
    Ok(())
}

fn main() {
    let log_path = std::env::temp_dir().join("glaspen_installer.log");
    let _ = std::fs::remove_file(&log_path);

    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(_) => {
            msgbox("无法定位安装程序", "glaspen2 安装失败");
            return;
        }
    };
    let mut data = Vec::new();
    if std::fs::File::open(&exe)
        .and_then(|mut f| f.read_to_end(&mut data))
        .is_err()
    {
        msgbox("无法读取安装程序自身", "glaspen2 安装失败");
        return;
    }

    // 定位 payload:文件末尾 12 字节 = MAGIC,其前 8 字节 = payload 长度
    let total = data.len();
    if total < 12 + 8 {
        msgbox("安装包数据缺失或损坏", "glaspen2 安装失败");
        return;
    }
    if &data[total - 12..] != MAGIC {
        msgbox("安装包数据缺失或损坏", "glaspen2 安装失败");
        return;
    }
    let payload_len = u64::from_le_bytes(data[total - 20..total - 12].try_into().unwrap()) as usize;
    let payload_start = total - 20 - payload_len;
    if payload_start < 12 || payload_start + payload_len > total - 20 {
        msgbox("安装包数据损坏", "glaspen2 安装失败");
        return;
    }
    let payload = &data[payload_start..payload_start + payload_len];
    if payload.len() < 40 {
        msgbox("安装包数据损坏", "glaspen2 安装失败");
        return;
    }

    // 目标目录 %LOCALAPPDATA%\glaspen2
    let local = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
    let dest = std::path::Path::new(&local).join("glaspen2");
    if std::fs::create_dir_all(&dest).is_err() {
        msgbox("无法创建安装目录", "glaspen2 安装失败");
        return;
    }

    // 先结束正在运行的旧版本: Windows 不允许覆盖正在运行的 exe/dll, 不处理的话
    // 解压会静默跳过旧文件, 安装器再启动旧 exe —— 用户看到"装了正式版还是旧版"。
    // 先请它正常收尾(WM_CLOSE), 超时再强杀, 然后等文件锁释放。
    let app_exe = dest.join("glaspen2.exe");
    if app_exe.exists() {
        append_log("found existing install, stopping it");
        request_running_instances_close();
        if !wait_file_writable(&app_exe, std::time::Duration::from_secs(4)) {
            append_log("old instance still running, force kill");
            force_kill_running_instances();
            let _ = wait_file_writable(&app_exe, std::time::Duration::from_secs(5));
        }
    }
    if let Err(e) = extract_tar_gz(payload, &dest) {
        append_log(&format!("install failed: {e}"));
        msgbox(&e, "glaspen2 安装失败");
        return;
    }

    let app_exe = dest.join("glaspen2.exe");
    if !app_exe.exists() {
        msgbox("安装后未找到 glaspen2.exe", "glaspen2 安装失败");
        return;
    }

    // 创建开始菜单快捷方式(通过 PowerShell 的 WScript.Shell)
    {
        let start_menu = std::env::var("APPDATA")
            .unwrap_or_else(|_| local.clone())
            .replace('\\', "/");
        let lnk = format!(
            "{}/Microsoft/Windows/Start Menu/Programs/glaspen2.lnk",
            start_menu
        );
        let app = app_exe.to_string_lossy().replace('\\', "/");
        let wd = dest.to_string_lossy().replace('\\', "/");
        let script = format!(
            "$ws=New-Object -ComObject WScript.Shell;$s=$ws.CreateShortcut('{}');$s.TargetPath='{}';$s.WorkingDirectory='{}';$s.Save()",
            lnk, app, wd
        );
        let _ = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", &script])
            .spawn();
    }

    // 启动 glaspen2
    unsafe {
        let app = wide(&app_exe.to_string_lossy());
        ShellExecuteW(
            0,
            wide("open").as_ptr(),
            app.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SHOW_NORMAL,
        );
    }
}
