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
    // Connect to C# log pipe in background thread
    if let Some(ref path) = csharp_exe {
        eprintln!("[glaspen2] Launching C# overlay: {}", path.display());
        match std::process::Command::new(path)
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(mut child) => {
                // Forward C# stderr
                if let Some(stderr) = child.stderr.take() {
                    std::thread::spawn(move || {
                        use std::io::BufRead;
                        let reader = std::io::BufReader::new(stderr);
                        for line in reader.lines() {
                            if let Ok(l) = line { eprintln!("[C#] {}", l); }
                        }
                    });
                }
                // Connect to C# log pipe
                std::thread::spawn(|| {
                    use std::io::BufRead;
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    for attempt in 0..50 {
                        match std::fs::OpenOptions::new()
                            .read(true)
                            .open(r"\\.\pipe\glaspen2_log")
                        {
                            Ok(pipe) => {
                                eprintln!("[glaspen2] Connected to C# log pipe (attempt {})", attempt);
                                let reader = std::io::BufReader::new(pipe);
                                for line in reader.lines() {
                                    if let Ok(l) = line { eprintln!("{}", l); }
                                }
                                eprintln!("[glaspen2] C# log pipe disconnected");
                                return;
                            }
                            Err(_) => {
                                std::thread::sleep(std::time::Duration::from_millis(200));
                            }
                        }
                    }
                    eprintln!("[glaspen2] Could not connect to C# log pipe");
                });
                match child.wait() {
                    Ok(status) => eprintln!("[glaspen2] C# overlay exited: {:?}", status),
                    Err(e) => eprintln!("[glaspen2] C# overlay wait error: {}", e),
                }
            }
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
    // 1) Compile-time env var (set by build.rs after auto-building Flutter)
    if let Some(path) = option_env!("GLASPEN2_FLUTTER_EXE") {
        let p = std::path::Path::new(path);
        if p.exists() {
            return Some(p.to_owned());
        }
    }

    // 2) Next to the Rust binary (for distribution)
    if let Ok(exe_path) = std::env::current_exe() {
        let sibling = exe_path.parent().unwrap_or(std::path::Path::new("."))
            .join("glaspen2_settings.exe");
        if sibling.exists() {
            return Some(sibling);
        }
    }

    // 3) Flutter build output (for development)
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
