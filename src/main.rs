// Debug 构建(cargo run / cargo build)保留控制台日志;
// Release 构建(正式运行)不显示控制台窗口。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(target_os = "macos")]
    glaspen2::macos::macos_run();

    #[cfg(target_os = "windows")]
    glaspen2::windows::win_main();
}
