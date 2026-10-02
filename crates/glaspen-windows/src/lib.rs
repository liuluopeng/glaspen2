#[cfg(windows)]
pub mod overlay;

/// 「立即更新」待安装的安装包路径(管道 applyUpdate 记录,进程收尾时消费)
#[cfg(windows)]
static PENDING_INSTALLER: std::sync::Mutex<Option<std::path::PathBuf>> =
    std::sync::Mutex::new(None);

#[cfg(windows)]
pub(crate) fn set_pending_installer(path: std::path::PathBuf) {
    *PENDING_INSTALLER.lock().unwrap() = Some(path);
}

/// 非 Windows 平台的空实现:让 `cargo check --workspace` 在 macOS CI 上
/// 也能通过(本 crate 的实质代码全部是 Win32)。
#[cfg(not(windows))]
pub fn win_main() {}

/// Entry point for the Windows version of glaspen2 (pure Rust, no C#).
///
/// Launches the Flutter settings UI (non-blocking), then runs the fullscreen
/// transparent overlay (blocking — when it exits, the app exits).
#[cfg(windows)]
pub fn win_main() {
    set_app_user_model_id();

    // ── 单实例守卫 ──
    // 设置管道 \\.\pipe\glaspen2_settings 是全局名字:两个 overlay 并存时,
    // 设置面板的开关/切 tab 消息会被另一个实例抢走,表现为"模式切不动"。
    // 命名互斥量随进程存活,持有期间第二个实例弹窗提示并退出。
    {
        use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
        use windows::Win32::System::Threading::CreateMutexW;
        use windows::core::PCWSTR;
        let name: Vec<u16> = "Local\\glaspen2.single_instance"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mutex = unsafe { CreateMutexW(None, false, PCWSTR(name.as_ptr())) };
        if let Ok(mutex) = mutex {
            // 故意不释放:互斥量随进程生命周期持有。
            // 不能按 clippy 建议改成 `let _ = mutex` —— 那会立刻 Drop 关掉
            // 句柄,守卫随进程存活的前提就不成立了。
            #[allow(clippy::mem_forget)]
            std::mem::forget(mutex);
            if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
                already_running_message();
                return;
            }
        }
    }

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
    let mut settings_child = find_settings_exe().and_then(|p| {
        eprintln!("[glaspen2] Launching Flutter settings: {}", p.display());
        std::process::Command::new(p).spawn().ok()
    });

    // Run the overlay (blocking — message loop until quit)
    overlay::run();

    if let Some(child) = settings_child.as_mut() {
        let _ = child.kill();
        let _ = child.wait();
    }

    // 自动更新收尾:applyUpdate 已记下安装包路径。cmd 延迟 2 秒再启动,
    // 确保本进程(含设置面板)完全退出、文件锁释放;安装器解压到
    // %LOCALAPPDATA%\glaspen2 后自动拉起新版。
    if let Some(installer) = PENDING_INSTALLER.lock().unwrap().take() {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        // DETACHED_PROCESS:不随本控制台/进程消亡
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        let path = installer.to_string_lossy().to_string();
        eprintln!("[glaspen2] 2 秒后启动更新安装器: {path}");
        let _ = std::process::Command::new("cmd")
            .args([
                "/c", "timeout", "/t", "2", "/nobreak", ">nul", "&", "start", "", &path,
            ])
            .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
            .spawn();
    }

    println!("[glaspen2] Exited");
}

#[cfg(windows)]
fn already_running_message() {
    use windows::Win32::UI::WindowsAndMessaging::{MB_ICONWARNING, MB_OK, MessageBoxW};
    use windows::core::PCWSTR;
    let text: Vec<u16> = "glaspen2 已在运行,请使用现有实例 (可从系统托盘或 Ctrl+Alt+Q 退出后重试)"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let caption: Vec<u16> = "glaspen2"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        let _ = MessageBoxW(
            None,
            PCWSTR(text.as_ptr()),
            PCWSTR(caption.as_ptr()),
            MB_ICONWARNING | MB_OK,
        );
    }
}

/// 与 Flutter 设置进程共用同一个 AppUserModelID:任务栏把两个进程的
/// 窗口归组为同一个 glaspen2 图标,而不是各占一格。
// 非 Windows 编译为空壳,这两个函数只在 Windows 主流程被调用 ——
// macOS 侧的 clippy -D warnings 会扫到 dead code,这里显式豁免。
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
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

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
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
