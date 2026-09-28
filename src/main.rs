// Debug 构建(cargo run / cargo build)保留控制台日志;
// Release 构建(正式运行)不显示控制台窗口。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // 自动更新的帮手进程:不进任何 GUI,等主程序退出 → 换 bundle → 拉起新版。
    // 它跑在**即将被替换的 bundle 里**,所以必须在一切初始化之前截胡,
    // 只做文件操作(见 src/updater.rs 的两条铁律)。
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--updater") {
        std::process::exit(glaspen2::updater::updater_main(&args[2..]));
    }

    #[cfg(target_os = "macos")]
    glaspen2::macos::macos_run();

    #[cfg(target_os = "windows")]
    glaspen_windows::win_main();
}
