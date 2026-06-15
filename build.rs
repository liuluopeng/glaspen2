fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    let is_macos = target.contains("apple");

    if is_macos {
        println!("cargo:rerun-if-changed=src/macos/glaspen2.m");
        let out_dir = std::env::var("OUT_DIR").unwrap();
        let obj_path = format!("{}/glaspen2.o", out_dir);
        let status = std::process::Command::new("clang")
            .args(&["-c", "src/macos/glaspen2.m", "-o", &obj_path])
            .args(&["-fobjc-arc", "-O0"])
            .arg("-I/opt/homebrew/Cellar/cairo/1.18.4/include")
            .status()
            .expect("Failed to run clang");
        assert!(status.success(), "clang failed to compile glaspen2.m");
        println!("cargo:rustc-link-search=native={}", out_dir);
        println!("cargo:rustc-link-lib=static=glaspen2_objc");
        let lib_path = format!("{}/libglaspen2_objc.a", out_dir);
        let status = std::process::Command::new("ar")
            .args(&["crus", &lib_path, &obj_path])
            .status()
            .expect("Failed to run ar");
        assert!(status.success(), "ar failed to create archive");
        println!("cargo:rustc-link-lib=cairo");
        println!("cargo:rustc-link-lib=framework=Cocoa");
        println!("cargo:rustc-link-lib=framework=QuartzCore");
        println!("cargo:rustc-link-lib=framework=ScreenCaptureKit");
        println!("cargo:rustc-link-lib=framework=CoreMedia");
        println!("cargo:rustc-link-lib=framework=CoreVideo");
        println!("cargo:rustc-link-lib=framework=IOSurface");
        println!("cargo:rustc-link-lib=framework=Carbon");
    }
}
