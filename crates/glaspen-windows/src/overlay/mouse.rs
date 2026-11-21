/// 处理鼠标 Raw Input:镜头修饰键 + 滚轮 → 镜头缩放;其余鼠标输入忽略。
/// 镜头修饰键**跟随「自由涂鸦: 放大」当前绑定的修饰键**(与 macOS
/// gl_shortcut_modifier_mask_for 一致: 面板改键后鼠标自动跟随; 该动作
/// 未绑定时鼠标不控制镜头)。
/// 返回 true 表示吞掉了滚轮(已应用缩放)。
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
    let mask = modifier_mask_for("canvas_zoom_in");
    if mask == 0 || !modifier_mask_pressed(mask) {
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

/// MOD_* 掩码里的修饰键当前是否都按着。掩码来自可配置快捷键的 chord,
/// 因此鼠标滚轮跟着面板里「放大」的绑定走(cmd 令牌 = Win 键)。
fn modifier_mask_pressed(mask: u32) -> bool {
    let down = |vk: u16| unsafe { GetAsyncKeyState(vk as i32) < 0 };
    if mask & (MOD_CONTROL.0 as u32) != 0 && !down(VK_CONTROL.0) {
        return false;
    }
    if mask & (MOD_ALT.0 as u32) != 0 && !down(VK_MENU.0) {
        return false;
    }
    if mask & (MOD_SHIFT.0 as u32) != 0 && !down(VK_SHIFT.0) {
        return false;
    }
    if mask & (MOD_WIN.0 as u32) != 0 && !(down(VK_LWIN.0) || down(VK_RWIN.0)) {
        return false;
    }
    true
}

// ── HUD 窗口:屏幕中央通知 + 左上角压力监控(macOS 一致) ──
// 黑色半透明底 + 白字,主消息循环线程访问。

