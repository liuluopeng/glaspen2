fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    let is_windows = target.contains("windows");
    let is_macos = target.contains("apple");

    if is_macos {
        // Watch sources for incremental rebuild (macOS 层已全 Swift)。

        // Swift 迁移层(S1):目录级监视,cargo 会递归扫目录,新增/改 .swift 都会重跑
        println!("cargo:rerun-if-changed=src/macos/swift");
        // 桥接头/Swift 头生成输入(S2):目录监视已覆盖,显式列一条防未来挪目录漏掉
        println!("cargo:rerun-if-changed=src/macos/swift/Bridging.h");
        println!("cargo:rerun-if-changed=flutter_settings/lib/main.dart");
        // main.dart 的 part 文件(main.*.dart)也是 Dart 源: 只盯 main.dart
        // 会在改 part 时不重建 App.framework, 面板改动静默失效。
        for e in std::fs::read_dir("flutter_settings/lib").unwrap().flatten() {
            if e.path().extension().is_some_and(|x| x == "dart") {
                println!("cargo:rerun-if-changed={}", e.path().display());
            }
        }
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

        // ── Swift 原生层:src/macos/swift/**/*.swift → .o,与 ObjC 同一个归档 ──
        // 部署目标取发布包 Info.plist 的 LSMinimumSystemVersion = 12.0
        // (scripts/build-dmg.sh;README 也写 macOS 12+)。S0 报告要求显式设置:
        // swiftc 默认 target 是宿主系统版本(这里 27.2),不设会让产物只能在
        // 开发机上跑。注意 aarch64 macOS 的理论下限是 11.0,12.0 与产品一致。
        // 代价:12.0 早于 Swift 5.6 runtime(12.3),新编译器会自动 force_load
        // libswiftCompatibility56 等兼容静态库 —— 它们只在工具链的
        // lib/swift/macosx 下,所以链接时必须带该 -L(见下方)。若将来把最低系统
        // 抬到 13.0,兼容库会消失,那几行 -L 可以精简。
        const SWIFT_DEPLOYMENT_TARGET: &str = "12.0";

        fn run_xcrun(args: &[&str]) -> String {
            let out = std::process::Command::new("xcrun")
                .args(args)
                .output()
                .unwrap_or_else(|e| panic!("xcrun {args:?} 执行失败: {e}"));
            assert!(
                out.status.success(),
                "xcrun {args:?} 失败: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        }

        let sdk_path = run_xcrun(&["--show-sdk-path"]);
        let swiftc = run_xcrun(&["-f", "swiftc"]);
        // 工具链根: .../XcodeDefault.xctoolchain/usr/bin/swiftc → .../usr
        let toolchain_usr = std::path::PathBuf::from(&swiftc)
            .parent()
            .and_then(|p| p.parent())
            .unwrap_or_else(|| panic!("无法从 swiftc 路径推导工具链目录: {swiftc}"))
            .to_path_buf();
        let swift_runtime_dir = toolchain_usr.join("lib").join("swift").join("macosx");
        assert!(
            swift_runtime_dir.exists(),
            "找不到 Swift 运行时库目录 {}(Xcode 工具链不完整?)",
            swift_runtime_dir.display()
        );

        let swift_src_dir = std::path::Path::new(&manifest_dir)
            .join("src")
            .join("macos")
            .join("swift");
        // 按子系统分目录(App/Menu/Input/Surface/Settings/Features/Diagnostics),
        // 递归收集; Swift 侧仍编译为同一个模块, 目录只影响工程组织。
        fn collect_swift_sources(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            let entries = std::fs::read_dir(dir)
                .unwrap_or_else(|e| panic!("读取 {} 失败: {e}", dir.display()));
            for entry in entries {
                let path = entry.expect("目录项").path();
                if path.is_dir() {
                    collect_swift_sources(&path, out);
                } else if path.extension().is_some_and(|x| x == "swift") {
                    out.push(path);
                }
            }
        }
        let mut swift_sources: Vec<std::path::PathBuf> = Vec::new();
        collect_swift_sources(&swift_src_dir, &mut swift_sources);
        swift_sources.sort();
        assert!(
            !swift_sources.is_empty(),
            "{} 下没有 .swift 文件(Swift 迁移层不应为空)",
            swift_src_dir.display()
        );
        // swiftc -c 把每个源文件的目标文件写到当前目录、文件名只取 basename;
        // 分目录后若出现同名 .swift 会互相覆盖, 这里提前报错而不是静默少链。
        let mut swift_stems = std::collections::HashSet::new();
        for src in &swift_sources {
            let stem = src.file_stem().expect("swift 源文件没有文件名");
            assert!(
                swift_stems.insert(stem.to_os_string()),
                "Swift 文件名必须全局唯一(目标文件按 basename 命名): {}",
                src.display()
            );
        }
        // 目录级 rerun-if-changed 之外逐文件显式列出: 嵌套目录/新增文件都能可靠触发重建。
        for src in &swift_sources {
            println!("cargo:rerun-if-changed={}", src.display());
        }

        let swift_obj_dir = std::path::Path::new(&out_dir).join("swift");
        std::fs::create_dir_all(&swift_obj_dir).expect("创建 Swift 目标文件目录失败");

        // bridging header: Swift 侧唯一的 C/FFI 声明入口(无 ObjC 实现侧)。
        let bridging_header = std::path::Path::new(&manifest_dir)
            .join("src")
            .join("macos")
            .join("swift")
            .join("Bridging.h");
        assert!(
            bridging_header.exists(),
            "缺少 Swift 桥接头 {}(Swift 调 ObjC 声明的唯一入口)",
            bridging_header.display()
        );
        let swift_target = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
            Ok("aarch64") => format!("arm64-apple-macosx{SWIFT_DEPLOYMENT_TARGET}"),
            Ok("x86_64") => format!("x86_64-apple-macosx{SWIFT_DEPLOYMENT_TARGET}"),
            other => panic!("不支持的 macOS 架构 {other:?}"),
        };
        let mut swiftc_cmd = std::process::Command::new(&swiftc);
        // 一次编整个模块(-c 多文件):将来多文件之间的符号/类型可直接互引,
        // 目标文件按源文件名落到 OUT_DIR/swift。固定 -module-name 避免模块名
        // 随文件名漂移。刻意钉 -swift-version 5:不让 CI 换新版 Swift 时默认
        // 切到 Swift 6 语言模式(Swift 6 并发检查是 S2+ 的独立决策)。
        swiftc_cmd
            .args([
                "-c",
                "-parse-as-library",
                "-module-name",
                "glaspen2_macos_swift",
                "-swift-version",
                "5",
            ])
            .args(["-target", &swift_target])
            .args(["-sdk", &sdk_path])
            // 导入 C/FFI 声明(应用层 ObjC 清零后不再 emit ObjC 头)。
            .arg("-import-objc-header")
            .arg(&bridging_header)
            .arg("-O")
            .current_dir(&swift_obj_dir);
        // Flutter 模块: 设置面板宿主(FlutterHost)在 Swift 里直接 import FlutterMacOS。
        let flutter_swift_search = format!(
            "{}/FlutterMacOS.xcframework/macos-arm64_x86_64",
            flutter_fw_dir
        );
        swiftc_cmd.arg("-F").arg(&flutter_swift_search);
        // cairo 头: Bridging.h 需要 <cairo/cairo.h>, pkg-config 给的是 .../include/cairo。
        for p in &cairo.include_paths {
            swiftc_cmd.arg("-I").arg(p);
            if let Some(parent) = p.parent() {
                swiftc_cmd.arg("-I").arg(parent);
            }
        }
        if std::env::var("PROFILE").as_deref() == Ok("debug") {
            swiftc_cmd.arg("-g");
        }
        swiftc_cmd.args(&swift_sources);
        let status = swiftc_cmd.status().expect("Failed to run swiftc");
        assert!(
            status.success(),
            "swiftc failed to compile src/macos/swift/**/*.swift"
        );

        let swift_objects: Vec<std::path::PathBuf> = swift_sources
            .iter()
            .map(|src| {
                let stem = src.file_stem().expect("swift 源文件没有文件名");
                let obj = swift_obj_dir.join(format!("{}.o", stem.to_string_lossy()));
                assert!(
                    obj.exists(),
                    "swiftc 未产出目标文件 {}(源自 {})",
                    obj.display(),
                    src.display()
                );
                obj
            })
            .collect();

        // Tell cargo about the object file
        println!("cargo:rustc-link-search=native={}", out_dir);
        println!("cargo:rustc-link-lib=static=glaspen2_objc");

        // Create one archive from the Swift objects. Swift 目标文件里带
        // LC_LINKER_OPTION autolink 信息, 从归档成员里拉出来时链接器也会处理.
        let lib_path = format!("{}/libglaspen2_objc.a", out_dir);
        let _ = std::fs::remove_file(&lib_path);
        let mut ar = std::process::Command::new("ar");
        ar.arg("crus").arg(&lib_path);
        for obj in &swift_objects {
            ar.arg(obj);
        }
        let status = ar.status().expect("Failed to run ar");
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

        // Swift 运行时链接(与上面 ObjC 产物链进同一个可执行文件):
        // - 工具链 lib/swift/macosx:部署目标 12.0 所需 libswiftCompatibility56.a
        //   等兼容库只在这里;缺了会报 undefined __swift_FORCE_LOAD_$_swiftCompatibility56。
        // - SDK/usr/lib/swift:libswiftCore/Foundation/AppKit 的 .tbd 链接桩。
        // - /usr/lib/swift:旧系统上 Swift 运行时 dylib 的实际目录。
        // Swift 对象自带的 autolink 会按 import 自动补 -lswiftCore /
        // -lswiftFoundation / -framework AppKit,这里刻意不手写库列表,
        // 避免库清单与 Swift 源码 import 漂移;只提供搜索路径 + rpath。
        println!(
            "cargo:rustc-link-search=native={}",
            swift_runtime_dir.display()
        );
        println!("cargo:rustc-link-search=native={sdk_path}/usr/lib/swift");
        println!("cargo:rustc-link-search=native=/usr/lib/swift");
        // 现代 macOS 的 Swift dylib 在 dyld shared cache 里,该 rpath 主要是
        // 兼容老系统/非标准运行时;保留它可避免未来 Swift 版本要求 rpath 时
        // 才发现产物起不来。
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
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
