fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    let is_windows = target.contains("windows");
    let is_macos = target.contains("apple");

    if is_macos {
        println!("cargo:rerun-if-changed=src/macos/glaspen2.m");

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

        // Tell Cargo to re-run when any .cs file changes
        for f in &cs_files {
            println!("cargo:rerun-if-changed={}", f.path().display());
        }
        println!("cargo:rerun-if-changed={}", csharp_exe.display());

        // Auto-delete exe when .cs files are newer (force recompile)
        let exe_time = std::fs::metadata(&csharp_exe).and_then(|m| m.modified()).ok();
        let cs_newer = cs_files.iter().any(|f| {
            let cs_time = std::fs::metadata(f.path()).and_then(|m| m.modified()).ok();
            match (exe_time, cs_time) {
                (Some(e), Some(c)) => c > e,
                _ => false,
            }
        });
        if cs_newer && csharp_exe.exists() {
            println!("cargo:warning=C# source changed, recompiling...");
            let _ = std::fs::remove_file(&csharp_exe);
        }

        let needs_compile = !csharp_exe.exists();

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
                    "/target:exe",
                    &out_arg,
                    "/platform:x64",
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
    }
}
