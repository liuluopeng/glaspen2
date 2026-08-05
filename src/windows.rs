pub mod overlay;

/// Entry point for the Windows version of glaspen2 (pure Rust, no C#).
///
/// Launches the Flutter settings UI (non-blocking), then runs the fullscreen
/// transparent overlay (blocking — when it exits, the app exits).
pub fn win_main() {
    // Launch Flutter settings UI (non-blocking)
    if let Some(settings_path) = find_settings_exe() {
        eprintln!(
            "[glaspen2] Launching Flutter settings: {}",
            settings_path.display()
        );
        let _ = std::process::Command::new(settings_path).spawn();
    }

    // Run the overlay (blocking — message loop until quit)
    overlay::run();

    println!("[glaspen2] Exited");
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
