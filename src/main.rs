// NOTE: windows_subsystem = "windows" is intentionally NOT set.
// We need console output for debugging. When launched from cargo run,
// stderr appears in the terminal. When launched as a packaged app,
// a console window appears briefly — acceptable for now.

fn main() {
    #[cfg(target_os = "macos")]
    {
        glaspen2::ws::start_server();
        glaspen2::macos::macos_run();
    }

    #[cfg(target_os = "windows")]
    glaspen2::windows::win_main();
}
