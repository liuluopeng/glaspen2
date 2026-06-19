// pub mod overlay; // inactive — using hook_overlay instead
pub mod overlay_gtk;
pub mod tray;
pub mod render;
pub mod hook_overlay;

/// Entry point for the Windows version of glaspen2.
pub fn win_main() {
    crate::db::init();
    hook_overlay::run();
    println!("[glaspen2] Exited");
}
