unsafe fn handle_mouse_raw(buf: &[u64], state: &mut OverlayState) -> bool {
    let raw = buf.as_ptr() as *const RAWINPUT;
    if (*raw).header.dwType != RIM_TYPEMOUSE.0 {
        return false;
    }
    let mouse = &(*raw).data.mouse;
    let btn_flags = mouse.Anonymous.Anonymous.usButtonFlags;
    if (btn_flags & RI_MOUSE_WHEEL as u16) == 0 {
        return false; // 非滚轮(移动/按键)全部忽略
    }
    let delta = mouse.Anonymous.Anonymous.usButtonData as i16;
    if delta == 0 || !infinite_on() || !state.draw.enabled || state.in_stroke {
        return false;
    }
    let ctrl = GetAsyncKeyState(VK_CONTROL.0 as i32) < 0;
    let alt = GetAsyncKeyState(VK_MENU.0 as i32) < 0;
    if !ctrl || !alt {
        return false;
    }
    let factor = if delta > 0 { 1.1 } else { 1.0 / 1.1 };
    // 光标屏幕坐标(虚拟桌面) → 视图坐标(虚拟屏左上为原点)
    let mut pt = POINT::default();
    let _ = GetCursorPos(&mut pt);
    let vx = (pt.x - GetSystemMetrics(SM_XVIRTUALSCREEN)) as f32;
    let vy = (pt.y - GetSystemMetrics(SM_YVIRTUALSCREEN)) as f32;
    canvas_zoom_at(state, factor, vx as f64, vy as f64);
    true
}

// ── HUD 窗口:屏幕中央通知 + 左上角压力监控(macOS 一致) ──
// 黑色半透明底 + 白字,主消息循环线程访问。

