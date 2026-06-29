// pub mod overlay; // inactive — using hook_overlay instead
// pub mod overlay_gtk; // inactive — requires GTK4 runtime DLLs
pub mod tray;
pub mod render;
pub mod hook_overlay;

/// Entry point for the Windows version of glaspen2.
///
/// Launches two child processes:
///   1. glaspen2_app.exe — the C# transparent overlay (main app)
///   2. glaspen2_settings.exe — the Flutter settings window
///
/// The Rust process waits for the C# overlay to exit (app lifecycle).
/// The Flutter settings window is optional — if not found, it's skipped.
pub fn win_main() {
    // Initialize the database
    crate::db::init();

    let csharp_exe = find_csharp_exe();
    let flutter_exe = find_flutter_exe();

    // Launch Flutter settings window (optional, non-blocking)
    let mut _flutter_child = None;
    if let Some(ref path) = flutter_exe {
        eprintln!("[glaspen2] Launching Flutter settings: {}", path.display());
        match std::process::Command::new(path).spawn() {
            Ok(child) => {
                _flutter_child = Some(child);
            }
            Err(e) => {
                eprintln!("[glaspen2] Failed to launch Flutter settings: {}", e);
            }
        }
    } else {
        eprintln!("[glaspen2] Flutter settings not found — skipping");
    }

    // Launch C# overlay (blocking — when it exits, the app exits)
    if let Some(ref path) = csharp_exe {
        eprintln!("[glaspen2] Launching C# overlay: {}", path.display());
        match std::process::Command::new(path).spawn() {
            Ok(mut child) => {
                match child.wait() {
                    Ok(status) => eprintln!("[glaspen2] C# overlay exited: {:?}", status),
                    Err(e) => eprintln!("[glaspen2] C# overlay wait error: {}", e),
                }
            }
            Err(e) => {
                eprintln!("[glaspen2] Failed to launch C# overlay: {} — falling back to Rust", e);
                hook_overlay::run();
            }
        }
    } else {
        eprintln!("[glaspen2] glaspen2_app.exe not found — using Rust fallback");
        hook_overlay::run();
    }

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

fn find_flutter_exe() -> Option<std::path::PathBuf> {
    // 1) Next to the Rust binary (for distribution)
    if let Ok(exe_path) = std::env::current_exe() {
        let sibling = exe_path.parent().unwrap_or(std::path::Path::new("."))
            .join("glaspen2_settings.exe");
        if sibling.exists() {
            return Some(sibling);
        }
    }

    // 2) Flutter build output (for development)
    let flutter_debug = std::path::Path::new("flutter_settings")
        .join("build").join("windows").join("x64")
        .join("runner").join("Debug").join("glaspen2_settings.exe");
    if flutter_debug.exists() {
        return Some(flutter_debug);
    }

    let flutter_release = std::path::Path::new("flutter_settings")
        .join("build").join("windows").join("x64")
        .join("runner").join("Release").join("glaspen2_settings.exe");
    if flutter_release.exists() {
        return Some(flutter_release);
    }

    None
}
