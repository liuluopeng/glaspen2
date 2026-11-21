#[cfg(windows)]
pub mod overlay;

/// 「立即更新」待安装的安装包路径(管道 applyUpdate 记录,进程收尾时消费)
#[cfg(windows)]
static PENDING_INSTALLER: std::sync::Mutex<Option<std::path::PathBuf>> =
    std::sync::Mutex::new(None);

#[cfg(windows)]
pub(crate) fn set_pending_installer(path: std::path::PathBuf) {
    *PENDING_INSTALLER.lock().unwrap_or_else(|e| e.into_inner()) = Some(path);
}

/// 非 Windows 平台的空实现:让 `cargo check --workspace` 在 macOS CI 上
/// 也能通过(本 crate 的实质代码全部是 Win32)。
#[cfg(not(windows))]
pub fn win_main() {}

/// 本进程拉起过的设置面板进程(macOS 面板是内嵌视图, Windows 是独立进程)。
/// 退出时要显式结束它们 —— 见 kill_settings_processes。
#[cfg(windows)]
static SETTINGS_CHILDREN: std::sync::Mutex<Vec<std::process::Child>> =
    std::sync::Mutex::new(Vec::new());

/// 设置面板(独立进程)的单实例查询/置顶/拉起。
///
/// **面板必须单实例**: 设置管道同一时刻只服务一个客户端。若同时存在两个面板,
/// 先开的那个(可能被后开的挡在下面、用户看不见)会一直占着连接, 后开的"看得见
/// 的那个"连不上覆盖层 —— 用户在它里面切画布模式/改设置全部无效(典型症状:
/// 切到自由涂鸦后涂鸦仍然落在活页本里)。已有活面板时只把它提到最前, 绝不新开。
#[cfg(windows)]
pub(crate) fn show_settings_window() {
    // 清理已自行关闭(X 按钮/崩溃)的; 历史遗留的多面板只留最后一个, 其余结束
    let existing_pid = {
        let mut children = SETTINGS_CHILDREN.lock().unwrap_or_else(|e| e.into_inner());
        children.retain_mut(|c| !matches!(c.try_wait(), Ok(Some(_))));
        let keep_from = children.len().saturating_sub(1);
        for mut stale in children.drain(..keep_from) {
            let pid = stale.id();
            if stale.kill().is_ok() {
                let _ = stale.wait();
                tracing::info!("结束多余的设置面板进程(pid={pid})");
            }
        }
        children.first().map(std::process::Child::id)
    };

    if let Some(pid) = existing_pid {
        if let Some(hwnd) = find_settings_window(pid) {
            activate_settings_window(hwnd, pid);
            tracing::info!("设置面板已在运行, 置顶显示(pid={pid})");
        } else {
            // 进程在但 Flutter 窗口还没创建(刚拉起/首帧中): 等它出来再置顶
            std::thread::spawn(move || raise_settings_window(pid));
        }
        return;
    }

    let Some(p) = find_settings_exe() else {
        tracing::warn!("找不到 glaspen2_settings.exe, 无法打开设置面板");
        return;
    };
    match std::process::Command::new(&p).spawn() {
        Ok(child) => {
            let pid = child.id();
            tracing::info!("设置面板已拉起: {} (pid={pid})", p.display());
            SETTINGS_CHILDREN
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(child);
            // 覆盖层是置顶窗口, 面板会被压在玻璃下面: 等 Flutter 窗口出现后置顶。
            std::thread::spawn(move || raise_settings_window(pid));
        }
        Err(e) => tracing::warn!("拉起设置面板失败({}): {e}", p.display()),
    }
}

/// 按 pid 找面板进程的 Flutter 顶层窗口(FLUTTER_RUNNER_WIN32_WINDOW)。
#[cfg(windows)]
fn find_settings_window(pid: u32) -> Option<windows::Win32::Foundation::HWND> {
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetWindowThreadProcessId,
    };
    use windows::core::BOOL;

    unsafe extern "system" fn find_panel(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ctx = &mut *(lparam.0 as *mut (u32, Option<HWND>));
        let mut win_pid = 0u32;
        let _ = GetWindowThreadProcessId(hwnd, Some(&mut win_pid as *mut u32));
        if win_pid != ctx.0 {
            return BOOL(1); // 不是这个进程的窗口, 继续找
        }
        let mut class_name = [0u16; 64];
        let n = GetClassNameW(hwnd, &mut class_name);
        if n > 0
            && String::from_utf16_lossy(&class_name[..n as usize]) == "FLUTTER_RUNNER_WIN32_WINDOW"
        {
            ctx.1 = Some(hwnd);
            return BOOL(0); // 找到 Flutter 主窗口, 停止枚举
        }
        BOOL(1)
    }

    let mut ctx: (u32, Option<HWND>) = (pid, None);
    let _ = unsafe { EnumWindows(Some(find_panel), LPARAM(&mut ctx as *mut _ as isize)) };
    ctx.1
}

/// 把面板窗口提到置顶并激活。
///
/// 覆盖层是 `WS_EX_TOPMOST` 全屏窗口, 而设置面板(独立进程的普通窗口)默认会被
/// 压在它下面 —— 开磨砂玻璃时面板直接看不清/点不到, 用户会以为"只有玻璃、
/// 面板没出来"。
#[cfg(windows)]
fn activate_settings_window(hwnd: windows::Win32::Foundation::HWND, pid: u32) {
    use windows::Win32::UI::WindowsAndMessaging::{
        HWND_TOPMOST, SW_SHOW, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SetForegroundWindow,
        SetWindowPos, ShowWindow,
    };
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        );
        let _ = ShowWindow(hwnd, SW_SHOW);
        // 覆盖层不抢焦点, 正常能被提到前台; 失败也不影响显示(只是不激活)。
        let _ = SetForegroundWindow(hwnd);
    }
    tracing::info!("设置面板窗口已置顶并激活(pid={pid})");
}

/// 新拉起的面板: Flutter 窗口要等首帧就绪才创建, 轮询等它出现再置顶(最多 6s)。
#[cfg(windows)]
fn raise_settings_window(pid: u32) {
    for _ in 0..60 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if let Some(hwnd) = find_settings_window(pid) {
            activate_settings_window(hwnd, pid);
            return;
        }
    }
    tracing::warn!("设置面板窗口 6s 内未出现, 无法置顶(pid={pid})");
}

/// 彻底退出:结束所有由本进程拉起的设置面板进程。
///
/// macOS 的面板是主程序里的 Flutter 视图, terminate 一并带走; Windows 的面板
/// 是**独立进程**, 只销毁覆盖层窗口的话它仍会活着、每 2 秒重连已经消失的命名
/// 管道, 用户看到的是"退出后设置窗口还在"——不是彻底退出。另外
/// glaspen2_settings.exe 的文件锁会挡住自动更新替换, 所以更新前也必须先结束它。
#[cfg(windows)]
pub(crate) fn kill_settings_processes() {
    let mut children = SETTINGS_CHILDREN.lock().unwrap_or_else(|e| e.into_inner());
    if children.is_empty() {
        return;
    }
    for mut child in children.drain(..) {
        let pid = child.id();
        match child.kill() {
            Ok(()) => {
                let _ = child.wait(); // 等句柄/文件锁真正释放
                tracing::info!("已结束设置面板进程(pid={pid})");
            }
            // 面板可能已自行关闭(X 按钮/崩溃), 不算错误
            Err(e) => tracing::debug!("设置面板进程 pid={pid} 无需结束: {e}"),
        }
    }
}

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
    // GLASPEN2_ALLOW_MULTI=1 旁路(与 macOS 同款,并行调试用)。
    if std::env::var("GLASPEN2_ALLOW_MULTI").is_err() {
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

    // 设置面板不再随启动无脑拉起(macOS 只在菜单/热键里打开)。
    // 首次启动由 overlay::run 里的"首启引导"拉起一次并记住;
    // 之后用 Ctrl+Alt+D(可配置 settings 动作)打开; 每次拉起的 pid 都记在
    // SETTINGS_CHILDREN 里, 退出时统一结束(见 kill_settings_processes)。

    // Run the overlay (blocking — message loop until quit)
    overlay::run();

    // 写完排队点 + checkpoint WAL, 让更新器的数据库快照完整。
    glaspen_core::export::glaspen2_shutdown();

    // 彻底退出:连设置面板进程一起结束(macOS 面板是内嵌视图, terminate 自带)。
    kill_settings_processes();

    // 自动更新收尾:applyUpdate 已记下安装包路径。cmd 延迟 2 秒再启动,
    // 确保本进程(含设置面板)完全退出、文件锁释放;安装器解压到
    // %LOCALAPPDATA%\glaspen2 后自动拉起新版。
    if let Some(installer) = PENDING_INSTALLER
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        // DETACHED_PROCESS:不随本控制台/进程消亡
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        let path = installer.to_string_lossy().to_string();
        tracing::info!("2 秒后启动更新安装器: {path}");
        let _ = std::process::Command::new("cmd")
            .args([
                "/c", "timeout", "/t", "2", "/nobreak", ">nul", "&", "start", "", &path,
            ])
            .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
            .spawn();
    }

    tracing::info!("Exited");
}

#[cfg(windows)]
fn already_running_message() {
    use windows::Win32::UI::WindowsAndMessaging::{MB_ICONWARNING, MB_OK, MessageBoxW};
    use windows::core::PCWSTR;
    let text: Vec<u16> =
        "glaspen2 已在运行,请使用现有实例(彻底退出: 默认 Ctrl+Alt+Q, 可在 设置→快捷键 改键)"
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
// 只在 Windows 主流程调用。必须整体 cfg 掉: 函数体里的
// `#[link(name = "shell32")]` 在 macOS 上也会被链接器看到, 导致
// `cargo test --workspace` 找不到 shell32。
#[cfg(windows)]
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
pub(crate) fn find_settings_exe() -> Option<std::path::PathBuf> {
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
