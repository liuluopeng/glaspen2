/// Entry point for the macOS version of glaspen2.
/// Calls the ObjC glaspen2_run() which handles all UI and input.
pub fn macos_run() {
    // 上一次自动更新的收尾(.old / dmg 清理)在进 GUI 之前做完;
    // 内部全是 best-effort,失败只记 updater.log,不影响启动。
    crate::updater::finish_pending();

    unsafe extern "C" {
        fn glaspen2_run();
    }
    unsafe {
        glaspen2_run();
    }
}
