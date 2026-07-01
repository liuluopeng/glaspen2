// pub mod overlay; // inactive — using hook_overlay instead
// pub mod overlay_gtk; // inactive — requires GTK4 runtime DLLs
pub mod tray;
pub mod render;
pub mod hook_overlay;

/// Entry point for the Windows version of glaspen2.
///
/// Launches glaspen2_app.exe — the C# transparent overlay (main app).
/// The Rust process waits for the C# overlay to exit (app lifecycle).
pub fn win_main() {
    // Initialize the database
    crate::db::init();

    let csharp_exe = find_csharp_exe();

    // Launch C# overlay (blocking — when it exits, the app exits)
    if let Some(ref path) = csharp_exe {
        eprintln!("[glaspen2] Launching C# overlay: {}", path.display());
        // Connect to C# log pipe in background thread
        std::thread::spawn(|| {
            use std::io::BufRead;
            for _ in 0..50 {
                match std::fs::OpenOptions::new()
                    .read(true)
                    .open(r"\\.\pipe\glaspen2_log")
                {
                    Ok(pipe) => {
                        eprintln!("[glaspen2] Connected to C# log pipe");
                        let reader = std::io::BufReader::new(pipe);
                        for line in reader.lines() {
                            match line {
                                Ok(l) => { let _ = l; eprintln!("{}", l); }
                                Err(_) => break,
                            }
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
