pub mod overlay;

/// Entry point for the Windows version of glaspen2 (pure Rust, no C#).
///
/// Launches the Flutter settings UI (non-blocking), then runs the fullscreen
/// transparent overlay (blocking — when it exits, the app exits).
pub fn win_main() {
    set_app_user_model_id();

    // 清理上次退出残留的设置进程(崩溃/强杀也会留孤儿,积多了任务栏出现
    // 多个同图标窗口)。taskkill 是控制台程序,CREATE_NO_WINDOW 防闪黑框。
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let _ = std::process::Command::new("taskkill")
            .args(["/F", "/IM", "glaspen2_settings.exe"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }

    // Launch Flutter settings UI (non-blocking); keep the handle so quitting
    // the overlay takes the settings window down with it.
    let mut settings_child = find_settings_exe()
        .and_then(|p| {
            eprintln!("[glaspen2] Launching Flutter settings: {}", p.display());
            std::process::Command::new(p).spawn().ok()
        });

    // Run the overlay (blocking — message loop until quit)
    overlay::run();

    if let Some(child) = settings_child.as_mut() {
        let _ = child.kill();
        let _ = child.wait();
    }

    println!("[glaspen2] Exited");
}

/// 与 Flutter 设置进程共用同一个 AppUserModelID:任务栏把两个进程的
/// 窗口归组为同一个 glaspen2 图标,而不是各占一格。
fn set_app_user_model_id() {
    #[link(name = "shell32")]
    unsafe extern "system" {
        fn SetCurrentProcessExplicitAppUserModelID(appid: *const u16) -> i32;
    }
    let wide: Vec<u16> = "glaspen2.app"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        SetCurrentProcessExplicitAppUserModelID(wide.as_ptr());
    }
}

fn find_settings_exe() -> Option<std::path::PathBuf> {
    let name = "glaspen2_settings.exe";

    // 1) Next to the Rust binary (for distribution)
    if let Ok(exe_path) = std::env::current_exe() {
        let sibling = exe_path
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join(name);
        if sibling.exists() {
            return Some(sibling);
        }
    }

    // 2) Compile-time env var (set by build.rs)
    if let Some(path) = option_env!("GLASPEN2_FLUTTER_EXE") {
        let p = std::path::Path::new(path);
        if p.exists() {
            return Some(p.to_owned());
        }
    }

    // 3) flutter_settings build directory (dev builds)
    let dev_paths = [
        "flutter_settings/build/windows/x64/runner/Release/glaspen2_settings.exe",
        "flutter_settings/build/windows/x64/runner/Debug/glaspen2_settings.exe",
    ];
    for dp in &dev_paths {
        let p = std::path::Path::new(dp);
        if p.exists() {
            return Some(p.to_owned());
        }
    }

    None
}
