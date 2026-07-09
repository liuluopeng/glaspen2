fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    let is_windows = target.contains("windows");
    let is_macos = target.contains("apple");

    if is_macos {
        // Watch sources for incremental rebuild
        println!("cargo:rerun-if-changed=src/macos/glaspen2.m");
        println!("cargo:rerun-if-changed=flutter_settings/lib/main.dart");
        println!("cargo:rerun-if-changed=flutter_settings/pubspec.yaml");
        for entry in std::fs::read_dir("flutter_settings/assets").unwrap() {
            if let Ok(e) = entry {
                println!("cargo:rerun-if-changed={}", e.path().display());
            }
        }

        // Auto-rebuild Flutter macOS framework if Dart sources or assets changed.
        // fvm flutter build macos-framework is fast (incremental), so this adds
        // minimal overhead when nothing changed.
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let flutter_dir = format!("{}/flutter_settings", manifest_dir);
        let flutter_framework = format!(
            "{}/build/macos/framework/Release/App.xcframework/macos-arm64_x86_64/App.framework/App",
            flutter_dir
        );

        // Rebuild if the framework doesn't exist yet
        let needs_build = !std::path::Path::new(&flutter_framework).exists();
        if !needs_build {
            // Compare mtimes of main.dart vs framework binary
            if let (Ok(dart_meta), Ok(fw_meta)) = (
                std::fs::metadata(format!("{}/lib/main.dart", flutter_dir)),
                std::fs::metadata(&flutter_framework),
            ) {
                if let (Ok(dart_mtime), Ok(fw_mtime)) = (dart_meta.modified(), fw_meta.modified()) {
                    if dart_mtime >= fw_mtime {
                        std::process::Command::new("fvm")
                            .args(["flutter", "build", "macos-framework"])
                            .current_dir(&flutter_dir)
                            .status().ok();
                    }
                }
            }
        }

        // cc crate adds -O2/-O3 from OPT_LEVEL in release mode, which breaks NSEvent tablet data.
        // Compile ObjC directly with clang -O0 to guarantee no optimization.
        let out_dir = std::env::var("OUT_DIR").unwrap();
        let obj_path = format!("{}/glaspen2.o", out_dir);

        // Flutter framework paths
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let flutter_fw_dir = format!(
            "{}/flutter_settings/build/macos/framework/Release",
            manifest_dir
        );

        let status = std::process::Command::new("clang")
            .args(&["-c", "src/macos/glaspen2.m", "-o", &obj_path])
            .args(&["-fobjc-arc", "-O0"])
            .arg("-I/opt/homebrew/Cellar/cairo/1.18.4/include")
            .arg(format!("-F{}/FlutterMacOS.xcframework/macos-arm64_x86_64", flutter_fw_dir))
            .status()
            .expect("Failed to run clang");

        assert!(status.success(), "clang failed to compile glaspen2.m");

        // Tell cargo about the object file
        println!("cargo:rustc-link-search=native={}", out_dir);
        println!("cargo:rustc-link-lib=static=glaspen2_objc");

        // Create an archive from the object file using ar
        let lib_path = format!("{}/libglaspen2_objc.a", out_dir);
        let status = std::process::Command::new("ar")
            .args(&["crus", &lib_path, &obj_path])
            .status()
            .expect("Failed to run ar");

        assert!(status.success(), "ar failed to create archive");

        // Link Flutter frameworks
        // -F needs the directory CONTAINING the .framework, not the .framework itself
        let flutter_search = format!(
            "{}/FlutterMacOS.xcframework/macos-arm64_x86_64",
            flutter_fw_dir
        );
        let app_search = format!(
            "{}/App.xcframework/macos-arm64_x86_64",
            flutter_fw_dir
        );
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

        println!("cargo:rustc-link-lib=cairo");
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
        let csharp_dir = std::path::Path::new("glaspen2_csharp");

        // Output C# exe to Cargo target dir (same dir as glaspen2.dll)
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
        let target_debug = std::path::Path::new(&manifest_dir).join("target").join(&profile);
        let csharp_exe = target_debug.join("glaspen2_app.exe");

        // Auto-build Flutter Windows app (like macOS does)
        let flutter_dir = std::path::Path::new(&manifest_dir).join("flutter_settings");
        let flutter_exe = flutter_dir.join("build").join("windows").join("x64")
            .join("runner").join("Release").join("glaspen2_settings.exe");
        let flutter_debug_exe = flutter_dir.join("build").join("windows").join("x64")
            .join("runner").join("Debug").join("glaspen2_settings.exe");

        // Determine flutter command (prefer fvm)
        let flutter_cmd = if std::process::Command::new("fvm")
            .args(&["flutter", "--version"])
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
                    println!("cargo:warning=Flutter build failed (exit code {:?})", s.code());
                }
                Err(e) => {
                    println!("cargo:warning=Failed to run flutter build: {}", e);
                }
            }
        }

        // Tell Rust where to find the Flutter settings exe
        if flutter_exe.exists() {
            println!("cargo:rustc-env=GLASPEN2_FLUTTER_EXE={}", flutter_exe.display());
            println!("cargo:warning=Flutter settings: {}", flutter_exe.display());
        } else if flutter_debug_exe.exists() {
            println!("cargo:rustc-env=GLASPEN2_FLUTTER_EXE={}", flutter_debug_exe.display());
            println!("cargo:warning=Flutter settings (debug): {}", flutter_debug_exe.display());
        }

        let cs_files: Vec<_> = std::fs::read_dir(csharp_dir)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "cs"))
            .collect();

        // Atomic lock: create a .lock file. First build.rs wins, second skips.
        // The lock persists — delete it (along with the exe) to force recompilation.
        let lock_file = target_debug.join(".csharp_compile.lock");
        // Tell Cargo to re-run if the exe or lock file is missing
        println!("cargo:rerun-if-changed={}", csharp_exe.display());
        println!("cargo:rerun-if-changed={}", lock_file.display());
        let has_lock = lock_file.exists()
            || std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_file)
                .is_ok();
        let needs_compile = !csharp_exe.exists() && has_lock;

        if needs_compile && !cs_files.is_empty() && !csharp_exe.exists() {
            // Find csc.exe — prefer .NET Framework 64-bit
            let csc_candidates = [
                r"C:\Windows\Microsoft.NET\Framework64\v4.0.30319\csc.exe",
                r"C:\Windows\Microsoft.NET\Framework\v4.0.30319\csc.exe",
            ];
            let csc = csc_candidates.iter().find(|p| std::path::Path::new(p).exists());

            if let Some(csc_path) = csc {
                // Compile to a temp file first, then move into place.
                // This avoids CS0016 "file in use" when the exe is locked
                // (e.g. by Windows Defender or a previous run).
                // Use unique temp name to avoid collision between cdylib/binary build.rs
                let tmp_exe = target_debug.join(format!("glaspen2_app_{}.exe", std::process::id()));
                let out_arg = format!("/out:{}", tmp_exe.display());
                let mut cmd = std::process::Command::new(csc_path);
                cmd.args(&[
                    "/target:winexe",
                    &out_arg,
                    "/platform:x64",
                    "/unsafe",
                ]);
                for f in &cs_files {
                    let abs = std::fs::canonicalize(f.path())
                        .unwrap_or_else(|_| f.path())
                        .display()
                        .to_string()
                        .replace("\\\\?\\", "");
                    cmd.arg(abs);
                }

                match cmd.output() {
                    Ok(output) => {
                        if output.status.success() {
                            // Only move if exe doesn't already exist (another build.rs
                            // may have created it between our check and now).
                            let moved = if !csharp_exe.exists() {
                                std::fs::rename(&tmp_exe, &csharp_exe).is_ok()
                            } else {
                                false
                            };
                            let _ = std::fs::remove_file(&tmp_exe);
                            if moved {
                                println!("cargo:warning=Compiled C# overlay → {}", csharp_exe.display());
                            }
                        } else {
                            let _ = std::fs::remove_file(&tmp_exe);
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            let stdout = String::from_utf8_lossy(&output.stdout);
                            println!("cargo:warning=C# compilation FAILED:");
                            for line in stderr.lines().chain(stdout.lines()) {
                                if !line.trim().is_empty() {
                                    println!("cargo:warning=  {}", line);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        println!("cargo:warning=Failed to run csc.exe: {}", e);
                    }
                }
            } else {
                println!("cargo:warning=csc.exe not found — C# overlay not compiled");
            }
        }

        // Tell Rust where to find the C# exe at runtime
        if csharp_exe.exists() {
            println!("cargo:rustc-env=GLASPEN2_CSHARP_EXE={}", csharp_exe.display());
            println!("cargo:warning=C# overlay: {}", csharp_exe.display());
        } else {
            println!("cargo:warning=glaspen2_app.exe not found — Rust fallback will be used");
        }

        // ── Copy Cairo DLLs from MSYS2 to target dir ──
        let msys_bin = std::path::Path::new("C:/msys64/mingw64/bin");
        if msys_bin.exists() {
            let cairo_dlls = [
                "libcairo-2.dll", "libpixman-1-0.dll", "libpng16-16.dll",
                "zlib1.dll", "libfontconfig-1.dll", "libfreetype-6.dll",
                "libexpat-1.dll", "libglib-2.0-0.dll", "libharfbuzz-0.dll",
                "libiconv-2.dll", "libintl-8.dll", "libpcre2-8-0.dll",
                "libbz2-1.dll", "libbrotlicommon.dll", "libbrotlidec.dll",
                "libffi-8.dll", "libgraphite2.dll",
                "libgcc_s_seh-1.dll", "libwinpthread-1.dll", "libstdc++-6.dll",
                "libdatrie-1.dll", "libfribidi-0.dll",
            ];
            for dll in &cairo_dlls {
                let src = msys_bin.join(dll);
                let dst = target_debug.join(dll);
                if src.exists() {
                    if dst.exists() {
                        // Only copy if source is newer
                        let src_time = std::fs::metadata(&src).and_then(|m| m.modified()).ok();
                        let dst_time = std::fs::metadata(&dst).and_then(|m| m.modified()).ok();
                        if src_time > dst_time {
                            if let Err(e) = std::fs::copy(&src, &dst) {
                                println!("cargo:warning=Failed to copy {}: {}", dll, e);
                            }
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
}
