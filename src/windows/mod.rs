pub mod overlay;
pub mod input;
pub mod tray;
pub mod render;

/// Entry point for the Windows version of glaspen2.
pub fn win_main() {
    // Set DPI awareness BEFORE getting screen dimensions
    unsafe {
        use windows::Win32::UI::HiDpi::{SetProcessDpiAwareness, PROCESS_DPI_AWARENESS};
        let _ = SetProcessDpiAwareness(PROCESS_DPI_AWARENESS(2));
    }

    // Get screen dimensions
    let screen_w;
    let screen_h;
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
        screen_w = GetSystemMetrics(SM_CXSCREEN);
        screen_h = GetSystemMetrics(SM_CYSCREEN);
    }

    // Initialize database and create first screen record
    crate::db::init();
    crate::db::new_screen(screen_w, screen_h);

    // Spawn overlay window on a dedicated thread (needs its own message loop for hooks/hotkeys)
    let overlay_thread = std::thread::spawn(move || {
        overlay::run(screen_w, screen_h);
    });

    // Wait a moment for the overlay window to be created
    std::thread::sleep(std::time::Duration::from_millis(200));

    // Get the overlay window handle for the tray to send commands to
    let hwnd = {
        let h = overlay::OVERLAY_HWND.lock().unwrap();
        *h
    };

    if hwnd == 0 {
        eprintln!("[glaspen2] Failed to create overlay window");
        return;
    }

    // Run system tray on the main thread (blocks until quit)
    tray::run(hwnd);

    // Wait for overlay thread to finish
    overlay_thread.join().ok();

    println!("[glaspen2] Exited");
}
