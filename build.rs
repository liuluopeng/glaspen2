fn main() {
    println!("cargo:rerun-if-changed=src/glaspen2.m");

    // cc crate adds -O2/-O3 from OPT_LEVEL in release mode, which breaks NSEvent tablet data.
    // Compile ObjC directly with clang -O0 to guarantee no optimization.
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let obj_path = format!("{}/glaspen2.o", out_dir);

    let status = std::process::Command::new("clang")
        .args(&["-c", "src/glaspen2.m", "-o", &obj_path])
        .args(&["-fobjc-arc", "-O0"])
        .arg("-I/opt/homebrew/Cellar/cairo/1.18.4/include")
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

    println!("cargo:rustc-link-lib=cairo");
    println!("cargo:rustc-link-lib=framework=Cocoa");
    println!("cargo:rustc-link-lib=framework=QuartzCore");
    println!("cargo:rustc-link-lib=framework=ScreenCaptureKit");
    println!("cargo:rustc-link-lib=framework=Carbon");
}
