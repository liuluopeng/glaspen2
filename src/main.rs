#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() {
    #[cfg(target_os = "macos")]
    glaspen2::macos::macos_run();

    #[cfg(target_os = "windows")]
    glaspen2::windows::win_main();
}
