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

/// 解压 tar.gz 到 dest。返回 false 表示失败。
fn extract_tar_gz(data: &[u8], dest: &std::path::Path) -> bool {
    use std::io::Write;

    // Gzip 解压
    let mut out = Vec::new();
    {
        let mut decoder = flate2::read::GzDecoder::new(data);
        if decoder.read_to_end(&mut out).is_err() {
            return false;
        }
    }

    // 解析 tar(512 字节块)
    let mut cur = 0usize;
    let mut files = 0u32;
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
                    let mut f = match std::fs::File::create(&out_path) {
                        Ok(f) => f,
                        Err(_) => {
                            cur += padded;
                            continue;
                        }
                    };
                    let _ = f.write_all(&out[cur..cur + size]);
                    files += 1;
                }
            }
        }
        cur += padded;
    }
    let _ = files;
    true
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
    if std::fs::File::open(&exe).and_then(|mut f| f.read_to_end(&mut data)).is_err() {
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

    if !extract_tar_gz(payload, &dest) {
        msgbox("解压失败", "glaspen2 安装失败");
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
        let lnk = format!("{}/Microsoft/Windows/Start Menu/Programs/glaspen2.lnk", start_menu);
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
        ShellExecuteW(0, wide("open").as_ptr(), app.as_ptr(), std::ptr::null(), std::ptr::null(), SHOW_NORMAL);
    }
}
