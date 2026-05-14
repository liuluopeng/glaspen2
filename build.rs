fn main() {
    println!("cargo:rerun-if-changed=src/glaspen2.m");

    // Force -O0 for ObjC: NSEvent tablet data unreliable under -O2/-Os
    // CFLAGS env has highest priority, .flag() as backup
    std::env::set_var("CFLAGS", "-O0");

    cc::Build::new()
        .file("src/glaspen2.m")
        .flag("-fobjc-arc")
        .flag("-O0")
        .include("/opt/homebrew/Cellar/cairo/1.18.4/include")
        .compile("glaspen2_objc");

    println!("cargo:rustc-link-lib=cairo");
    println!("cargo:rustc-link-lib=framework=Cocoa");
    println!("cargo:rustc-link-lib=framework=QuartzCore");
    println!("cargo:rustc-link-lib=framework=ScreenCaptureKit");
    println!("cargo:rustc-link-lib=framework=Carbon");
}
