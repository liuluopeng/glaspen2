pub mod overlay;
pub mod overlay_gtk;
pub mod tray;
pub mod render;

/// Entry point for the Windows version of glaspen2.
pub fn win_main() {
    crate::db::init();
    overlay_gtk::run_gtk();
    println!("[glaspen2] Exited");
}
