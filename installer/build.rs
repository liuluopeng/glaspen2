// Embed the app icon (same asset as the main exe) into the installer stub.
fn main() {
    println!("cargo:rerun-if-changed=../glaspen2.ico");
    let icon = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../glaspen2.ico");
    let mut res = winresource::WindowsResource::new();
    res.set("FileDescription", "glaspen2 setup");
    res.set("ProductName", "glaspen2");
    res.set_icon(icon.to_str().expect("icon path"));
    res.compile().expect("failed to compile installer resources");
}
