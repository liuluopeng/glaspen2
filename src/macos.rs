/// Entry point for the macOS version of glaspen2.
/// Calls the ObjC glaspen2_run() which handles all UI and input.
pub fn macos_run() {
    // 上一次自动更新的收尾(.old / dmg 清理)在进 GUI 之前做完;
    // 内部全是 best-effort,失败只记 updater.log,不影响启动。
    crate::updater::finish_pending();

    // 页面详情(面板圈选/移动/复制粘贴/删除)改到当前页时, core 经此回调
    // 在主线程重建玻璃(load_strokes + rebuild, 定义在 parts/menu.m)。
    unsafe extern "C" {
        fn glaspen2_macos_refresh_page(screen_id: i64);
    }
    glaspen_core::export::set_glass_refresh_hook(|screen_id| unsafe {
        glaspen2_macos_refresh_page(screen_id);
    });

    // 晾衣架: 气泡消息完结 → core 经此回调异步刷新挂卡(定义在 parts/line.m,
    // 内部 dispatch_async 回主线程再取数 —— 回调触发时 core 正持 bubbles 锁)。
    unsafe extern "C" {
        fn glaspen2_bubble_line_changed();
    }
    glaspen_core::export::set_clothesline_hook(|| unsafe {
        glaspen2_bubble_line_changed();
    });

    unsafe extern "C" {
        fn glaspen2_run();
    }
    unsafe {
        glaspen2_run();
    }
}
