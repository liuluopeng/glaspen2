// Debug 构建(cargo run / cargo build)保留控制台日志;
// Release 构建(正式运行)不显示控制台窗口。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // 统一日志(tracing): 全仓 eprintln/NSLog 的替代。stderr(cargo run
    // 终端直读)之外增加按天滚动文件层(~/Library/Logs/glaspen2/), 并安装
    // panic hook 落盘崩溃 backtrace —— 见 src/logging.rs。
    // 过滤优先级: RUST_LOG(标准 EnvFilter 语法) >
    //   GLASPEN2_DB_LOG=1 → "debug"(兼容旧的 DB 日志开关) >
    //   默认 "info"(db 的 debug 日志静默)。
    glaspen2::logging::init();
    glaspen2::logging::install_panic_hook();

    // 自动更新的帮手进程:不进任何 GUI,等主程序退出 → 换 bundle → 拉起新版。
    // 它跑在**即将被替换的 bundle 里**,所以必须在一切初始化之前截胡,
    // 只做文件操作(见 src/updater.rs 的两条铁律)。
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--updater") {
        std::process::exit(glaspen2::updater::updater_main(&args[2..]));
    }

    // S2 Swift↔ObjC 互调自检(隐藏调试入口, 不启动 GUI): 只跑一次双向探针
    // (ObjC→Swift→ObjC, 各打一行 NSLog)后退出。正常启动/用户路径不经过这里。
    // 定义在 src/macos/swift/InteropProbe.m;验证命令:
    //     target/debug/glaspen2 --swift-probe
    if args.get(1).map(String::as_str) == Some("--swift-probe") {
        #[cfg(target_os = "macos")]
        unsafe {
            unsafe extern "C" {
                fn glaspen2_macos_swift_probe_selftest();
            }
            glaspen2_macos_swift_probe_selftest();
        }
        std::process::exit(0);
    }

    // S3a 状态栏创建自检(隐藏调试入口, 不启动 GUI): Swift 创建一个 NSStatusItem
    // 并检查 button/image 后退出。菜单构建仍由 ObjC 负责(S3b)。
    if args.get(1).map(String::as_str) == Some("--swift-statusbar-probe") {
        #[cfg(target_os = "macos")]
        unsafe {
            unsafe extern "C" {
                fn glaspen2_macos_swift_statusbar_selftest() -> i32;
            }
            let code = glaspen2_macos_swift_statusbar_selftest();
            std::process::exit(code);
        }
        #[cfg(not(target_os = "macos"))]
        std::process::exit(2);
    }

    // S3b-1 菜单小图标 parity 自检(隐藏调试入口, 不启动 GUI):
    // 逐字节比较旧 ObjC 图标与 Swift 图标(TIFF), 0 = 一致。
    if args.get(1).map(String::as_str) == Some("--swift-icon-probe") {
        #[cfg(target_os = "macos")]
        unsafe {
            unsafe extern "C" {
                fn glaspen2_macos_swift_icon_parity() -> i32;
            }
            let code = glaspen2_macos_swift_icon_parity();
            std::process::exit(code);
        }
        #[cfg(not(target_os = "macos"))]
        std::process::exit(2);
    }

    #[cfg(target_os = "macos")]
    glaspen2::macos::macos_run();

    #[cfg(target_os = "windows")]
    glaspen_windows::win_main();
}
