// pub mod overlay; // inactive — using hook_overlay instead
// pub mod overlay_gtk; // inactive — requires GTK4 runtime DLLs
pub mod tray;
pub mod render;
pub mod hook_overlay;

/// Entry point for the Windows version of glaspen2.
///
/// On Windows, we prefer to launch the C# GlasPen2 overlay,
/// which is the proven, working implementation. The Rust hook_overlay is a fallback.
///
/// Search order for glaspen2_app.exe:
///   1) Next to the Rust binary (for distribution packages)
///   2) Compile-time path (for development builds)
pub fn win_main() {
    // Search for glaspen2_app.exe
    let csharp_exe = find_csharp_exe();

    if let Some(ref path) = csharp_exe {
        eprintln!("[glaspen2] Launching C# overlay: {}", path.display());
        match std::process::Command::new(path).spawn() {
            Ok(mut child) => {
                match child.wait() {
                    Ok(status) => eprintln!("[glaspen2] C# overlay exited: {:?}", status),
                    Err(e) => eprintln!("[glaspen2] C# overlay wait error: {}", e),
                }
                return;
            }
            Err(e) => {
                eprintln!("[glaspen2] Failed to launch C# overlay: {} — falling back to Rust", e);
            }
        }
    } else {
        eprintln!("[glaspen2] glaspen2_app.exe not found — using Rust fallback");
    }

    // Fallback: Rust implementation
    crate::db::init();
    hook_overlay::run();
    println!("[glaspen2] Exited");
}

fn find_csharp_exe() -> Option<std::path::PathBuf> {
    // 1) Next to the Rust binary (for distribution)
    if let Ok(exe_path) = std::env::current_exe() {
        let sibling = exe_path.parent().unwrap_or(std::path::Path::new(".")).join("glaspen2_app.exe");
        if sibling.exists() {
            return Some(sibling);
        }
    }

    // 2) Compile-time env var (for development builds via build.rs)
    if let Some(path) = option_env!("GLASPEN2_CSHARP_EXE") {
        let p = std::path::Path::new(path);
        if p.exists() {
            return Some(p.to_owned());
        }
    }

    // 3) glaspen2_csharp/ in project root
    let dev_path = std::path::Path::new("glaspen2_csharp").join("glaspen2_app.exe");
    if dev_path.exists() {
        return Some(dev_path);
    }

    None
}
