fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    let is_windows = target.contains("windows");
    let is_macos = target.contains("apple");

    if is_macos {
        // Watch sources for incremental rebuild
        println!("cargo:rerun-if-changed=src/macos/glaspen2.m");
        println!("cargo:rerun-if-changed=flutter_settings/lib/main.dart");
        println!("cargo:rerun-if-changed=flutter_settings/pubspec.yaml");
        // flutter_rust_bridge 生成的 Dart 绑定:codegen 只改这里而不动
        // main.dart 时,不监听会导致 App.framework 里打进旧 Dart(与 Rust
        // 侧 frb_generated.rs 的 content hash 不一致,面板初始化即报
        // "Content hash ... different from Rust side")。
        for e in std::fs::read_dir("flutter_settings/lib/src/rust")
            .unwrap()
            .flatten()
        {
            println!("cargo:rerun-if-changed={}", e.path().display());
        }
        for e in std::fs::read_dir("flutter_settings/assets")
            .unwrap()
            .flatten()
        {
            println!("cargo:rerun-if-changed={}", e.path().display());
        }

        // Auto-rebuild Flutter macOS framework whenever this build script
        // runs (triggered by rerun-if-changed on main.dart/pubspec/assets).
        // fvm flutter build macos-framework is fast (incremental), so this
        // adds minimal overhead. No mtime comparison — a rerun always means
        // Dart sources changed, so always rebuild.
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let flutter_dir = format!("{}/flutter_settings", manifest_dir);

        // macOS 27 lipo rejects the multi-arch `-verify_arch a b` form that
        // Flutter's macOS build validation uses; scripts/lipo-shim/lipo splits
        // it into per-arch checks. Must be on PATH or the framework build fails
        // (and this match would silently swallow it, shipping stale Dart).
        let lipo_shim = format!("{}/scripts/lipo-shim", manifest_dir);
        let path_env = std::env::var("PATH").unwrap_or_default();

        // 链哪一份 Flutter framework。默认 Release(发布/DMG 用);调 Dart 侧
        // (JIT + VM service + 热重载)时切到 Debug:
        //     GLASPEN2_FLUTTER_CONFIG=Debug cargo run
        // Debug/Profile 的 App.framework 含 kernel_blob.bin,引擎跑 JIT,
        // 于是 flutter attach 能连上;Release 是 AOT,连不上也改不了代码。
        println!("cargo:rerun-if-env-changed=GLASPEN2_FLUTTER_CONFIG");
        let flutter_config = std::env::var("GLASPEN2_FLUTTER_CONFIG").unwrap_or_default();
        // Release 保持历史行为(不带额外开关,构建全部配置);Debug/Profile
        // 只构建需要的那一个,省掉另外两个的编译时间。
        let (config, fw_args): (&str, &[&str]) = match flutter_config.as_str() {
            "Debug" => ("Debug", &["--debug", "--no-profile", "--no-release"]),
            "Profile" => ("Profile", &["--profile", "--no-debug", "--no-release"]),
            _ => ("Release", &["--release"]),
        };

        // fvm 是本机的 Flutter 版本管理器;CI 或只装了 Flutter SDK 的环境没有它,
        // 退化成直接用 flutter(否则这一步会静默失败, 只剩旧 Dart)。
        let (flutter_exe, flutter_args): (&str, &[&str]) = if std::process::Command::new("fvm")
            .arg("--version")
            .output()
            .is_ok()
        {
            ("fvm", &["flutter", "build", "macos-framework"])
        } else {
            ("flutter", &["build", "macos-framework"])
        };

        let mut fw_cmd = std::process::Command::new(flutter_exe);
        fw_cmd
            .args(flutter_args)
            .args(fw_args)
            .current_dir(&flutter_dir)
            .env("PATH", format!("{lipo_shim}:{path_env}"));
        let status = fw_cmd.status();
        match status {
            Ok(s) if s.success() => {}
            other => eprintln!(
                "[build.rs] {flutter_exe} flutter build macos-framework {fw_args:?} failed: {other:?}"
            ),
        }

        // Note: historically compiled with -O0 due to a suspected "optimization
        // breaks NSEvent tablet data" issue. Compiler optimization cannot change
        // semantics of correct code; the underlying bug was fixed separately
        // (pressure is read from CGEvent tablet fields). -O2 gives a large
        // speedup to the per-event ObjC hot path.
        let out_dir = std::env::var("OUT_DIR").unwrap();
        let obj_path = format!("{}/glaspen2.o", out_dir);

        // flutter_rust_bridge 的 C 入口。Rust 是以 rlib 静态链进主可执行文件
        // 的:没有任何引用的归档成员会被链接器丢掉,而这些符号**只**由 Dart 侧
        // 在运行时 dlsym(ExternalLibrary.process)解析 —— 丢一个,面板就在用户
        // 机器上打不开(编译、链接、签名全都正常)。所以逐个用 -u 强制保留。
        //
        // 这 14 个就是 FRB 2.12 的全部 _frb_* 导出,与 Dart 侧
        // flutter_rust_bridge 包里的 lookup 一一对应。**升级 FRB 或重新生成绑定
        // 后必须重新核对**:漏项会被 tests/frb_entry_points.rs 的对比测试
        // (拿 cdylib 的导出集合作基准)以及 scripts/build-dmg.sh 的断言拦下来。
        //
        // 只作用于 bin 目标:测试二进制里没有这些符号(链接参数按 cargo 的定义
        // 不覆盖 rlib 成员),加上去会让 cargo test 直接链接失败。
        const FRB_SYMBOLS: [&str; 14] = [
            "frb_pde_ffi_dispatcher_primary",
            "frb_pde_ffi_dispatcher_sync",
            "frb_dart_fn_deliver_output",
            "frb_get_rust_content_hash",
            "frb_init_frb_dart_api_dl",
            "frb_create_shutdown_callback",
            "frb_free_wire_sync_rust2dart_sse",
            "frb_free_wire_sync_rust2dart_dco",
            "frb_rust_vec_u8_new",
            "frb_rust_vec_u8_resize",
            "frb_rust_vec_u8_free",
            "frb_dart_opaque_dart2rust_encode",
            "frb_dart_opaque_rust2dart_decode",
            "frb_dart_opaque_drop_thread_box_persistent_handle",
        ];
        for symbol in FRB_SYMBOLS {
            println!("cargo:rustc-link-arg-bins=-Wl,-u,_{symbol}");
        }

        // Flutter framework paths(配置由上面的 GLASPEN2_FLUTTER_CONFIG 决定)
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let flutter_fw_dir = format!(
            "{}/flutter_settings/build/macos/framework/{config}",
            manifest_dir
        );
        if !std::path::Path::new(&flutter_fw_dir).exists() {
            panic!(
                "{flutter_fw_dir} 不存在:先跑一次 \
                 `cd flutter_settings && fvm flutter build macos-framework --{0}`",
                config.to_lowercase()
            );
        }

        // cairo via pkg-config — portable across Homebrew versions / CI runners
        // (pkg_config::probe also emits the cargo link-search/lib directives).
        let cairo = pkg_config::Config::new()
            .probe("cairo")
            .expect("cairo not found via pkg-config (brew install cairo)");

        let mut clang = std::process::Command::new("clang");
        clang.args(["-c", "src/macos/glaspen2.m", "-o", &obj_path]);
        clang.args(["-fobjc-arc", "-O2"]);
        // debug 构建带上调试信息:否则 lldb 只能按符号名下断点,
        // 看不到 glaspen2.m 的行号(backtrace 也只有地址)。
        if std::env::var("PROFILE").as_deref() == Ok("debug") {
            clang.arg("-g");
        }
        for p in &cairo.include_paths {
            clang.arg(format!("-I{}", p.display()));
            // glaspen2.m 用 <cairo/cairo.h> 风格: pkg-config 给的是 .../include/cairo,
            // 需要父目录 .../include 才能解析 <cairo/...>。
            if let Some(parent) = p.parent() {
                clang.arg(format!("-I{}", parent.display()));
            }
        }
        clang.arg(format!(
            "-F{}/FlutterMacOS.xcframework/macos-arm64_x86_64",
            flutter_fw_dir
        ));
        let status = clang.status().expect("Failed to run clang");

        assert!(status.success(), "clang failed to compile glaspen2.m");

        // Tell cargo about the object file
        println!("cargo:rustc-link-search=native={}", out_dir);
        println!("cargo:rustc-link-lib=static=glaspen2_objc");

        // Create an archive from the object file using ar
        let lib_path = format!("{}/libglaspen2_objc.a", out_dir);
        let status = std::process::Command::new("ar")
            .args(["crus", &lib_path, &obj_path])
            .status()
            .expect("Failed to run ar");

        assert!(status.success(), "ar failed to create archive");

        // Link Flutter frameworks
        // -F needs the directory CONTAINING the .framework, not the .framework itself
        let flutter_search = format!(
            "{}/FlutterMacOS.xcframework/macos-arm64_x86_64",
            flutter_fw_dir
        );
        let app_search = format!("{}/App.xcframework/macos-arm64_x86_64", flutter_fw_dir);
        println!("cargo:rustc-link-search=framework={}", flutter_search);
        println!("cargo:rustc-link-search=framework={}", app_search);
        println!("cargo:rustc-link-lib=framework=FlutterMacOS");
        println!("cargo:rustc-link-lib=framework=App");

        // Set rpath so the binary can find Flutter frameworks at runtime
        println!(
            "cargo:rustc-link-arg=-Wl,-rpath,{}/FlutterMacOS.xcframework/macos-arm64_x86_64",
            flutter_fw_dir
        );
        println!(
            "cargo:rustc-link-arg=-Wl,-rpath,{}/App.xcframework/macos-arm64_x86_64",
            flutter_fw_dir
        );

        // cairo link directives are emitted by the pkg-config probe above
        println!("cargo:rustc-link-lib=framework=Cocoa");
        println!("cargo:rustc-link-lib=framework=QuartzCore");
        println!("cargo:rustc-link-lib=framework=ScreenCaptureKit");
        println!("cargo:rustc-link-lib=framework=CoreMedia");
        println!("cargo:rustc-link-lib=framework=CoreVideo");
        println!("cargo:rustc-link-lib=framework=IOSurface");
        println!("cargo:rustc-link-lib=framework=Carbon");
        println!("cargo:rustc-link-lib=framework=ApplicationServices");
    }

    if is_windows {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();

        // ── 应用图标(macOS 同款素材转出的 ico)嵌入 exe 资源 ──
        println!("cargo:rerun-if-changed=glaspen2.ico");
        let mut res = winresource::WindowsResource::new();
        res.set_icon("glaspen2.ico");
        res.compile()
            .expect("failed to compile Windows resources (icon)");

        // Auto-build Flutter Windows app (like macOS does)
        let flutter_dir = std::path::Path::new(&manifest_dir).join("flutter_settings");
        let flutter_exe = flutter_dir
            .join("build")
            .join("windows")
            .join("x64")
            .join("runner")
            .join("Release")
            .join("glaspen2_settings.exe");
        let flutter_debug_exe = flutter_dir
            .join("build")
            .join("windows")
            .join("x64")
            .join("runner")
            .join("Debug")
            .join("glaspen2_settings.exe");

        // Determine flutter command (prefer fvm)
        let flutter_cmd = if std::process::Command::new("fvm")
            .args(["flutter", "--version"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            "fvm"
        } else {
            "flutter"
        };

        // Check if Flutter exe already exists and is newer than lib/main.dart
        let main_dart = flutter_dir.join("lib").join("main.dart");
        let needs_flutter_build = if flutter_exe.exists() {
            // Check if main.dart is newer than the exe
            let exe_time = std::fs::metadata(&flutter_exe)
                .and_then(|m| m.modified())
                .ok();
            let dart_time = std::fs::metadata(&main_dart)
                .and_then(|m| m.modified())
                .ok();
            match (exe_time, dart_time) {
                (Some(e), Some(d)) => d > e,
                _ => true,
            }
        } else if flutter_debug_exe.exists() {
            false // Debug build exists, good enough
        } else {
            true // No exe at all
        };

        if needs_flutter_build {
            println!("cargo:warning=Building Flutter Windows app...");
            let flutter_args = if flutter_cmd == "fvm" {
                vec!["flutter", "build", "windows"]
            } else {
                vec!["build", "windows"]
            };
            let status = std::process::Command::new(flutter_cmd)
                .args(&flutter_args)
                .current_dir(&flutter_dir)
                .status();
            match status {
                Ok(s) if s.success() => {
                    println!("cargo:warning=Flutter build succeeded");
                }
                Ok(s) => {
                    println!(
                        "cargo:warning=Flutter build failed (exit code {:?})",
                        s.code()
                    );
                }
                Err(e) => {
                    println!("cargo:warning=Failed to run flutter build: {}", e);
                }
            }
        }

        // Tell Rust where to find the Flutter settings exe
        if flutter_exe.exists() {
            println!(
                "cargo:rustc-env=GLASPEN2_FLUTTER_EXE={}",
                flutter_exe.display()
            );
            println!("cargo:warning=Flutter settings: {}", flutter_exe.display());
        } else if flutter_debug_exe.exists() {
            println!(
                "cargo:rustc-env=GLASPEN2_FLUTTER_EXE={}",
                flutter_debug_exe.display()
            );
            println!(
                "cargo:warning=Flutter settings (debug): {}",
                flutter_debug_exe.display()
            );
        }

        // ── Copy Cairo DLLs to target dir (next to the exe) ──
        // 优先项目内自带 vendor/win/cairo(不依赖用户安装 Rnote/MSYS2),
        // 缺失时回退到 MSYS2 目录。
        let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
        let target_dir = std::path::Path::new(&manifest_dir)
            .join("target")
            .join(&profile);
        let vendor_dir = std::path::Path::new(&manifest_dir)
            .join("vendor")
            .join("win")
            .join("cairo");
        let msys_bin = std::path::Path::new("C:/msys64/mingw64/bin");
        let cairo_dlls = [
            "libcairo-2.dll",
            "libpixman-1-0.dll",
            "libpng16-16.dll",
            "zlib1.dll",
            "libfontconfig-1.dll",
            "libfreetype-6.dll",
            "libexpat-1.dll",
            "libglib-2.0-0.dll",
            "libharfbuzz-0.dll",
            "libiconv-2.dll",
            "libintl-8.dll",
            "libpcre2-8-0.dll",
            "libbz2-1.dll",
            "libbrotlicommon.dll",
            "libbrotlidec.dll",
            "libffi-8.dll",
            "libgraphite2.dll",
            "libgcc_s_seh-1.dll",
            "libwinpthread-1.dll",
            "libstdc++-6.dll",
            "libdatrie-1.dll",
            "libfribidi-0.dll",
        ];
        let src_root: &std::path::Path = if vendor_dir.exists() {
            println!("cargo:warning=Cairo DLLs from vendor/win/cairo");
            &vendor_dir
        } else if msys_bin.exists() {
            println!("cargo:warning=Cairo DLLs from MSYS2 (vendor/win/cairo missing)");
            msys_bin
        } else {
            println!("cargo:warning=Cairo DLLs not found (vendor/win/cairo or MSYS2 required)");
            return;
        };
        for dll in &cairo_dlls {
            let src = src_root.join(dll);
            let dst = target_dir.join(dll);
            if src.exists() {
                if dst.exists() {
                    // Only copy if source is newer
                    let src_time = std::fs::metadata(&src).and_then(|m| m.modified()).ok();
                    let dst_time = std::fs::metadata(&dst).and_then(|m| m.modified()).ok();
                    if src_time > dst_time
                        && let Err(e) = std::fs::copy(&src, &dst)
                    {
                        println!("cargo:warning=Failed to copy {}: {}", dll, e);
                    }
                } else {
                    if let Err(e) = std::fs::copy(&src, &dst) {
                        println!("cargo:warning=Failed to copy {}: {}", dll, e);
                    }
                }
            }
        }
    }
}
