// Debug 构建(cargo run / cargo build)保留控制台日志;
// Release 构建(正式运行)不显示控制台窗口。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // 统一日志(tracing): 全仓 eprintln/NSLog 的替代。输出走 stderr
    // (cargo run 终端直读), 格式 = 层级 时间 目标 消息。
    // 过滤优先级: RUST_LOG(标准 EnvFilter 语法) >
    //   GLASPEN2_DB_LOG=1 → "debug"(兼容旧的 DB 日志开关) >
    //   默认 "info"(db 的 debug 日志静默)。
    {
        use tracing_subscriber::EnvFilter;
        let default = if std::env::var_os("GLASPEN2_DB_LOG").is_some() {
            "debug"
        } else {
            "info"
        };
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default));
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            .with_target(true)
            .init();
    }

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
