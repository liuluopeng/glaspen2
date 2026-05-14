fn main() {
    let (screen_w, screen_h) = macos_get_screen_size();
    println!("[glaspen2] screen: {}x{}", screen_w, screen_h);

    // Run the entire app in our custom ObjC event loop (no winit event loop)
    macos_run_app(screen_w, screen_h);
}

#[cfg(target_os = "macos")]
fn macos_get_screen_size() -> (u32, u32) {
    extern "C" {
        fn macos_get_screen_size(out_w: *mut u32, out_h: *mut u32);
    }
    let mut w: u32 = 0;
    let mut h: u32 = 0;
    unsafe {
        macos_get_screen_size(&mut w, &mut h);
    }
    (w, h)
}

#[cfg(not(target_os = "macos"))]
fn macos_get_screen_size() -> (u32, u32) {
    (1920, 1080)
}

#[cfg(target_os = "macos")]
fn macos_run_app(screen_w: u32, screen_h: u32) {
    extern "C" {
        fn macos_run_app(screen_w: u32, screen_h: u32);
    }
    unsafe {
        macos_run_app(screen_w, screen_h);
    }
}

#[cfg(not(target_os = "macos"))]
fn macos_run_app(_screen_w: u32, _screen_h: u32) {
    println!("[glaspen2] non-macos: no-op");
}
