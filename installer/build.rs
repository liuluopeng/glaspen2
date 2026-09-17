// Embed the app icon into the installer stub.
// 仅在目标是 Windows 时编译资源(在 macOS 上开发/CI 时跳过)。
fn main() {
    if std::env::var("CARGO_CFG_WINDOWS").is_err() {
        return;
    }
    println!("cargo:rerun-if-changed=../glaspen2.ico");
    let icon = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../glaspen2.ico");
    let mut res = winresource::WindowsResource::new();
    res.set("FileDescription", "glaspen2 setup");
    res.set("ProductName", "glaspen2");
    res.set_icon(icon.to_str().expect("icon path"));
    res.compile().expect("failed to compile installer resources");
}
