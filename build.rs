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
        let csharp_exe = csharp_dir.join("glaspen2_app.exe");

        // Auto-compile C# overlay if exe is missing or any .cs file changed
        let cs_files: Vec<_> = std::fs::read_dir(csharp_dir)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "cs"))
            .collect();

        for f in &cs_files {
            println!("cargo:rerun-if-changed={}", f.path().display());
        }

        let needs_compile = !csharp_exe.exists()
            || cs_files.iter().any(|f| {
                let cs_meta = f.metadata().ok();
                let exe_meta = std::fs::metadata(&csharp_exe).ok();
                match (cs_meta, exe_meta) {
                    (Some(cs), Some(exe)) => cs.modified().ok() > exe.modified().ok(),
                    _ => true,
                }
            });

        if needs_compile && !cs_files.is_empty() {
            // Find csc.exe — prefer .NET Framework 64-bit
            let csc_candidates = [
                r"C:\Windows\Microsoft.NET\Framework64\v4.0.30319\csc.exe",
                r"C:\Windows\Microsoft.NET\Framework\v4.0.30319\csc.exe",
            ];
            let csc = csc_candidates.iter().find(|p| std::path::Path::new(p).exists());

            if let Some(csc_path) = csc {
                let mut cmd = std::process::Command::new(csc_path);
                cmd.args(&[
                    "/target:winexe",
                    "/out:glaspen2_app.exe",
                    "/platform:x64",
                ]);
                for f in &cs_files {
                    // Use filename only since current_dir is already glaspen2_csharp
                    cmd.arg(f.file_name());
                }
                cmd.current_dir(csharp_dir);

                match cmd.output() {
                    Ok(output) => {
                        if output.status.success() {
                            println!("cargo:warning=Compiled C# overlay ({} files)", cs_files.len());
                        } else {
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

        // Set env var so Rust can find the C# exe at runtime
        if csharp_exe.exists() {
            let abs = std::fs::canonicalize(&csharp_exe).unwrap_or_else(|_| csharp_exe.to_owned());
            println!("cargo:rustc-env=GLASPEN2_CSHARP_EXE={}", abs.display());
            println!("cargo:warning=C# overlay: {}", abs.display());
        } else {
            println!("cargo:warning=glaspen2_app.exe not found — Rust fallback will be used");
        }
    }
}
