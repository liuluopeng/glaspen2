/// Entry point for the macOS version of glaspen2.
/// Calls the ObjC glaspen2_run() which handles all UI and input.
pub fn macos_run() {
    extern "C" {
        fn glaspen2_run();
    }
    unsafe { glaspen2_run(); }
}
