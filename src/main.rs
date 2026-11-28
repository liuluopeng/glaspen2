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
    // 定义在 Swift(菜单构建自检);验证命令:
    //     target/debug/glaspen2 --swift-menu-probe
    if args.get(1).map(String::as_str) == Some("--swift-menu-probe") {
        #[cfg(target_os = "macos")]
        unsafe {
            unsafe extern "C" {
                fn glaspen2_macos_swift_menu_selftest() -> i32;
            }
            let code = glaspen2_macos_swift_menu_selftest();
            std::process::exit(code);
        }
        #[cfg(not(target_os = "macos"))]
        std::process::exit(2);
    }

    // S3b-2b-3 完整菜单构建自检(隐藏调试入口, 不启动 GUI): Swift
    // MenuBuilder 按真实建菜单顺序(颜色→分隔符→粗细→顶层其余项+子菜单)
    // 建出整份菜单, 校验项数/分隔符位置/顶层 action·tag·state·target/两个子
    // 菜单后退出。普通启动/用户路径不经过这里。验证命令:
    //     target/debug/glaspen2 --swift-pageframe-parity
    if args.get(1).map(String::as_str) == Some("--swift-pageframe-parity") {
        #[cfg(target_os = "macos")]
        unsafe {
            unsafe extern "C" {
                fn glaspen2_macos_swift_pageframe_parity_selftest() -> i32;
            }
            std::process::exit(glaspen2_macos_swift_pageframe_parity_selftest());
        }
        #[cfg(not(target_os = "macos"))]
        std::process::exit(2);
    }

    //     target/debug/glaspen2 --swift-tunnel-frame-parity
    if args.get(1).map(String::as_str) == Some("--swift-tunnel-frame-parity") {
        #[cfg(target_os = "macos")]
        unsafe {
            unsafe extern "C" {
                fn glaspen2_macos_swift_tunnel_frame_parity_selftest() -> i32;
            }
            std::process::exit(glaspen2_macos_swift_tunnel_frame_parity_selftest());
        }
        #[cfg(not(target_os = "macos"))]
        std::process::exit(2);
    }

    //     target/debug/glaspen2 --swift-card-factory-probe
    if args.get(1).map(String::as_str) == Some("--swift-card-factory-probe") {
        #[cfg(target_os = "macos")]
        unsafe {
            unsafe extern "C" {
                fn cairo_image_surface_create(format: i32, w: i32, h: i32)
                -> *mut std::ffi::c_void;
                fn cairo_surface_destroy(s: *mut std::ffi::c_void);
                fn cairo_surface_status(s: *mut std::ffi::c_void) -> i32;
                fn glaspen2_rust_surface_set(s: *mut std::ffi::c_void);
                fn glaspen2_rust_surface_set_scale(v: f64);
                fn glaspen2_rust_tunnel_set_surf_w(v: i32);
                fn glaspen2_rust_tunnel_set_surf_h(v: i32);
                fn glaspen2_rust_make_card(
                    is_current: i32,
                    depth: i32,
                    cache_slot: i32,
                    screen_id: i64,
                    main_out: *mut *mut std::ffi::c_void,
                    mip_out: *mut *mut std::ffi::c_void,
                ) -> i32;
            }
            let surf = cairo_image_surface_create(0, 800, 600);
            if surf.is_null() || cairo_surface_status(surf) != 0 {
                println!("CARD_FACTORY_PROBE_FAIL surface");
                std::process::exit(1);
            }
            glaspen2_rust_surface_set(surf);
            glaspen2_rust_surface_set_scale(2.0);
            glaspen2_rust_tunnel_set_surf_w(800);
            glaspen2_rust_tunnel_set_surf_h(600);
            let (mut main, mut mip): (*mut std::ffi::c_void, *mut std::ffi::c_void) =
                (std::ptr::null_mut(), std::ptr::null_mut());
            let ok = glaspen2_rust_make_card(1, 0, 0, 0, &mut main, &mut mip);
            println!(
                "CARD_FACTORY_PROBE ok={ok} main={} mip={}",
                !main.is_null(),
                !mip.is_null()
            );
            if !main.is_null() {
                cairo_surface_destroy(main);
            }
            if !mip.is_null() {
                cairo_surface_destroy(mip);
            }
            cairo_surface_destroy(surf);
            std::process::exit(if ok != 0 && !main.is_null() { 0 } else { 1 });
        }
        #[cfg(not(target_os = "macos"))]
        std::process::exit(2);
    }

    //     target/debug/glaspen2 --swift-tunnel-card-parity
    if args.get(1).map(String::as_str) == Some("--swift-tunnel-card-parity") {
        #[cfg(target_os = "macos")]
        unsafe {
            unsafe extern "C" {
                fn glaspen2_macos_swift_tunnel_card_parity_selftest() -> i32;
            }
            std::process::exit(glaspen2_macos_swift_tunnel_card_parity_selftest());
        }
        #[cfg(not(target_os = "macos"))]
        std::process::exit(2);
    }

    //     target/debug/glaspen2 --swift-tunnel-card-probe
    if args.get(1).map(String::as_str) == Some("--swift-tunnel-card-probe") {
        #[cfg(target_os = "macos")]
        unsafe {
            unsafe extern "C" {
                fn glaspen2_macos_swift_tunnel_card_selftest() -> i32;
            }
            std::process::exit(glaspen2_macos_swift_tunnel_card_selftest());
        }
        #[cfg(not(target_os = "macos"))]
        std::process::exit(2);
    }

    //     target/debug/glaspen2 --swift-notification-probe
    if args.get(1).map(String::as_str) == Some("--swift-notification-probe") {
        #[cfg(target_os = "macos")]
        unsafe {
            unsafe extern "C" {
                fn glaspen2_macos_swift_notification_selftest() -> i32;
            }
            let code = glaspen2_macos_swift_notification_selftest();
            std::process::exit(code);
        }
        #[cfg(not(target_os = "macos"))]
        std::process::exit(2);
    }

    //     target/debug/glaspen2 --swift-menu-full-probe
    if args.get(1).map(String::as_str) == Some("--swift-menu-full-probe") {
        #[cfg(target_os = "macos")]
        unsafe {
            unsafe extern "C" {
                fn glaspen2_macos_swift_menu_full_selftest() -> i32;
            }
            let code = glaspen2_macos_swift_menu_full_selftest();
            std::process::exit(code);
        }
        #[cfg(not(target_os = "macos"))]
        std::process::exit(2);
    }

    // S3a 状态栏创建自检(隐藏调试入口, 不启动 GUI): Swift 创建一个 NSStatusItem
    // 并检查 button/image 后退出。菜单构建也已在 Swift(S3b)。
    if args.get(1).map(String::as_str) == Some("--swift-statusbar-probe") {
        #[cfg(target_os = "macos")]
        unsafe {
            unsafe extern "C" {
                fn glaspen2_rust_statusbar_selftest() -> i32;
            }
            std::process::exit(glaspen2_rust_statusbar_selftest());
        }
        #[cfg(not(target_os = "macos"))]
        std::process::exit(2);
    }

    // S8 自检: 设置应用分派/状态存储/设置快照(隐藏调试入口, 不启动 GUI)。
    if args.get(1).map(String::as_str) == Some("--swift-hotkey-perform-probe") {
        #[cfg(target_os = "macos")]
        unsafe {
            unsafe extern "C" {
                fn glaspen2_router_hotkey_perform(key: u16) -> i32;
            }
            let a = glaspen2_router_hotkey_perform(0x09);
            let b = glaspen2_router_hotkey_perform(0x09);
            let c = glaspen2_router_hotkey_perform(0x06);
            println!("HOTKEY_PERFORM_PROBE draw1={a} draw2={b} undo={c}");
            std::process::exit(if a == 1 && b == 1 { 0 } else { 1 });
        }
        #[cfg(not(target_os = "macos"))]
        std::process::exit(2);
    }

    if args.get(1).map(String::as_str) == Some("--swift-macos-api-probe") {
        #[cfg(target_os = "macos")]
        let bytes = glaspen_macos::macos_shell::settings_snapshot_pb();
        let Ok(data) = glaspen_ipc::decode_settings_frame(&bytes) else {
            println!("MACOS_API_PROBE pb=invalid");
            std::process::exit(1);
        };
        let grid = data.get("gridSize").is_some();
        println!("MACOS_API_PROBE len={} kind=settings grid={grid}", bytes.len());
        std::process::exit(if grid { 0 } else { 1 });
        #[cfg(not(target_os = "macos"))]
        std::process::exit(2);
    }

    if args.get(1).map(String::as_str) == Some("--swift-settings-probe") {
        #[cfg(target_os = "macos")]
        unsafe {
            unsafe extern "C" {
                fn glaspen2_rust_settings_probe() -> i32;
            }
            std::process::exit(glaspen2_rust_settings_probe());
        }
        #[cfg(not(target_os = "macos"))]
        std::process::exit(2);
    }

    // S8 自检: 页面导航/空画布载荷(隐藏调试入口; 需临时 DB)。
    if args.get(1).map(String::as_str) == Some("--swift-pages-probe") {
        #[cfg(target_os = "macos")]
        unsafe {
            unsafe extern "C" {
                fn glaspen2_rust_pages_probe() -> i32;
            }
            std::process::exit(glaspen2_rust_pages_probe());
        }
        #[cfg(not(target_os = "macos"))]
        std::process::exit(2);
    }

    // S8 自检: 模型器/笔画提交/画布总览(隐藏调试入口; 需临时 DB)。
    if args.get(1).map(String::as_str) == Some("--swift-render-probe") {
        #[cfg(target_os = "macos")]
        unsafe {
            unsafe extern "C" {
                fn glaspen2_rust_render_probe() -> i32;
            }
            std::process::exit(glaspen2_rust_render_probe());
        }
        #[cfg(not(target_os = "macos"))]
        std::process::exit(2);
    }

    // S8 自检: 快捷键默认表安装/反查(隐藏调试入口; 需临时 DB)。
    if args.get(1).map(String::as_str) == Some("--swift-hotkeys-probe") {
        #[cfg(target_os = "macos")]
        unsafe {
            unsafe extern "C" {
                fn glaspen2_rust_hotkeys_probe() -> i32;
            }
            std::process::exit(glaspen2_rust_hotkeys_probe());
        }
        #[cfg(not(target_os = "macos"))]
        std::process::exit(2);
    }

    // 诊断: 模型器一笔 + 异步 flush, 外部查 DB points。
    if args.get(1).map(String::as_str) == Some("--swift-persist-probe") {
        #[cfg(target_os = "macos")]
        unsafe {
            unsafe extern "C" {
                fn glaspen2_rust_persist_probe() -> i32;
            }
            std::process::exit(glaspen2_rust_persist_probe());
        }
        #[cfg(not(target_os = "macos"))]
        std::process::exit(2);
    }

    #[cfg(target_os = "macos")]
    glaspen2::macos::macos_run();

    #[cfg(target_os = "windows")]
    glaspen_windows::win_main();
}
