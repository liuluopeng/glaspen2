//! 方向闸门: 两端平移方向只能调用 `Camera::drag_pan_from_screen_delta` /
//! `Camera::keyboard_pan_delta`, 不允许 adapter 再写第二份符号。
use std::path::Path;

fn read(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("读取 {rel} 失败: {e}"))
}

#[test]
fn adapters_use_core_pan_direction_only() {
    let core = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = core.parent().unwrap().parent().unwrap();

    let macos = read(root, "crates/glaspen-macos/src/macos_shell.rs");
    let win_mouse = read(root, "crates/glaspen-windows/src/overlay/mouse.rs");
    let win_hotkeys = read(root, "crates/glaspen-windows/src/overlay/hotkeys.rs");

    for (name, src, token) in [
        (
            "macos mouse",
            macos.as_str(),
            "Camera::drag_pan_from_screen_delta",
        ),
        (
            "macos keyboard",
            macos.as_str(),
            "Camera::keyboard_pan_delta",
        ),
        (
            "windows mouse",
            win_mouse.as_str(),
            "Camera::drag_pan_from_screen_delta",
        ),
        (
            "windows keyboard",
            win_hotkeys.as_str(),
            "Camera::keyboard_pan_delta",
        ),
    ] {
        assert!(src.contains(token), "{name} 未走 core 方向函数 {token}");
    }

    assert!(
        !win_mouse.contains("camera_pending_dx -="),
        "Windows 鼠标又写了本地方向符号"
    );
    assert!(
        !macos.contains("camera_last_x()) - x)"),
        "macOS 鼠标又写了本地方向符号"
    );
}
