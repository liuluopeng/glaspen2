// ── 鼠标 Raw Input: 无限画布镜头(拖动平移 + 滚轮缩放) ──
// 行为对齐 macOS run.m 的事件 tap:
//   - 修饰键 = 对应动作的 chord 修饰键(平移用 canvas_pan_up, 缩放用
//     canvas_zoom_in; 默认 Ctrl+Alt), 且要求**恰好**是这套修饰键
//     (macOS 用 (smods & relevant) == cameraMods 的相等比较);
//   - Ctrl+Alt + 左键拖动 = 临时抓手, 内容跟随鼠标(自然滚动);
//   - 滚轮缩放以鼠标位置为锚, 缩放上下限与提示同 macOS;
//   - 书写中 / 非无限画布 / 涂鸦停用 → 全部忽略, 事件透传。
//
// 与 macOS 的差别: macOS 的事件 tap 能"消费"鼠标事件; Windows 的覆盖层
// 平时是 WS_EX_TRANSPARENT(为了键鼠/笔分离), 所以要在镜头修饰键按住时
// 先清穿透(把鼠标交给覆盖层), 否则左键会先点到下层窗口。修饰键释放后
// (键盘 raw input 收到 key-up)再还原穿透。

/// 当前按住的修饰键 → MOD_* 掩码(与 hotkeys.rs 的 chord 掩码同一编码)
fn current_mod_mask() -> u32 {
    let down = |vk: u16| unsafe { GetAsyncKeyState(vk as i32) < 0 };
    let mut m = 0u32;
    if down(VK_CONTROL.0) || down(VK_LCONTROL.0) || down(VK_RCONTROL.0) {
        m |= MOD_CONTROL.0 as u32;
    }
    if down(VK_MENU.0) {
        m |= MOD_ALT.0 as u32;
    }
    if down(VK_SHIFT.0) || down(VK_LSHIFT.0) || down(VK_RSHIFT.0) {
        m |= MOD_SHIFT.0 as u32;
    }
    if down(VK_LWIN.0) || down(VK_RWIN.0) {
        m |= MOD_WIN.0 as u32;
    }
    m
}

/// 组合键要求的修饰键是否"恰好"按住(macOS 用相等比较, 多按不算)
fn mods_match_exact(mask: u32) -> bool {
    mask != 0 && current_mod_mask() == mask
}

fn camera_pan_mods() -> u32 {
    modifier_mask_for("canvas_pan_up")
}

fn camera_zoom_mods() -> u32 {
    modifier_mask_for("canvas_zoom_in")
}

/// 镜头手势(抓手拖动 / 滚轮缩放)用的修饰键掩码: **固定取主组合家族
/// (Win+Alt)**, 不取被降级的 Alt+Shift —— Alt+Shift 是 Windows 语言切换热键,
/// 按住拖/滚会切输入法。个别镜头键被别人占用时只降级键盘键, 鼠标手势仍用
/// 主组合家族; 若用户把每个镜头键都改得没有主组合了, 再退回缩放键的掩码。
fn camera_gesture_mods() -> u32 {
    for action in [
        "canvas_zoom_in",
        "canvas_pan_up",
        "canvas_pan_left",
        "canvas_pan_right",
        "canvas_pan_down",
        "canvas_zoom_out",
    ] {
        if crate::overlay::effective_chord(action).starts_with("cmd+alt+") {
            return modifier_mask_for(action);
        }
    }
    camera_zoom_mods()
}

/// 镜头手势判定: 修饰键恰好等于手势掩码(与 macOS 一样, 多按不算)。
fn camera_mods_match() -> bool {
    mods_match_exact(camera_gesture_mods())
}

/// 镜头操作可用条件(macOS: g_infinite_canvas && g_enabled && !g_stroke_active)
fn camera_available(state: &OverlayState) -> bool {
    infinite_on() && state.draw.enabled && !state.in_stroke()
}

/// 清空并应用累积的镜头拖动位移(每 16ms 合并一次, 与 macOS 节流一致)
fn flush_camera_pan(state: &mut OverlayState) {
    state.camera_pan_timer = false;
    let (dx, dy) = (state.camera_pending_dx, state.camera_pending_dy);
    state.camera_pending_dx = 0.0;
    state.camera_pending_dy = 0.0;
    if dx != 0.0 || dy != 0.0 {
        canvas_pan_by(state, dx, dy);
    }
}

/// 取消镜头拖动/镜头拦截(模式切换、停用涂鸦、开始书写时调用)。
/// 与 macOS camera_mouse_cancel_pan 同义: 丢弃未应用的位移并还原穿透。
fn cancel_camera_drag(state: &mut OverlayState) {
    if state.camera_pan_timer {
        let _ = unsafe { KillTimer(Some(state.canvas.hwnd), TIMER_CAMERA_PAN) };
    }
    state.camera_pan_timer = false;
    state.camera_dragging = false;
    state.camera_pending_dx = 0.0;
    state.camera_pending_dy = 0.0;
    if state.camera_blocked && !state.in_stroke() {
        set_input_blocking(state.canvas.hwnd, false, "camera-cancel");
        state.camera_blocked = false;
    }
}

/// 修饰键释放时还原穿透(键盘 raw input 的 key-up 分支调用)
fn camera_mods_maybe_released(state: &mut OverlayState) {
    if state.camera_blocked
        && !state.camera_dragging
        && !mods_match_exact(camera_pan_mods())
    {
        set_input_blocking(state.canvas.hwnd, false, "camera-mods-up");
        state.camera_blocked = false;
    }
}

/// 镜头修饰键当前是否按着(看门狗线程用它兜底释放拦截; 不碰 OverlayState)
pub(crate) fn camera_mods_held_now() -> bool {
    mods_match_exact(camera_pan_mods()) || mods_match_exact(camera_zoom_mods())
}

/// 处理鼠标 Raw Input:镜头拖动平移 / 滚轮缩放;其余鼠标输入忽略透传。
/// 返回 true 表示吞掉了滚轮(已应用缩放)。
unsafe fn handle_mouse_raw(buf: &[u64], state: &mut OverlayState) -> bool {
    let raw = buf.as_ptr() as *const RAWINPUT;
    if (*raw).header.dwType != RIM_TYPEMOUSE.0 {
        return false;
    }
    let mouse = &(*raw).data.mouse;
    let btn_flags = mouse.Anonymous.Anonymous.usButtonFlags;

    // ── 1. 镜头修饰键按住 → 先清穿透, 让紧接着的左键按下被覆盖层接住 ──
    // (否则左键会先点到下层窗口; macOS 事件 tap 是直接消费的)
    let pan_held = camera_mods_match();
    if camera_available(state) {
        if pan_held && !state.camera_blocked {
            set_input_blocking(state.canvas.hwnd, true, "camera-mods");
            state.camera_blocked = true;
        } else if !pan_held && state.camera_blocked && !state.camera_dragging {
            set_input_blocking(state.canvas.hwnd, false, "camera-mods-up");
            state.camera_blocked = false;
        }
    } else if state.camera_blocked && !state.camera_dragging {
        // 退出无限画布/停用涂鸦/开始书写: 放弃镜头拦截, 还原穿透
        set_input_blocking(state.canvas.hwnd, false, "camera-unavailable");
        state.camera_blocked = false;
    }

    // ── 2. 左键按下 + 镜头修饰键 → 开始拖动平移 ──
    if (btn_flags & RI_MOUSE_LEFT_BUTTON_DOWN as u16) != 0
        && camera_available(state)
        && camera_mods_match()
    {
        state.camera_dragging = true;
        state.camera_pending_dx = 0.0;
        state.camera_pending_dy = 0.0;
        let mut start = POINT::default();
        let _ = GetCursorPos(&mut start);
        state.camera_last_cursor = (start.x, start.y);
        if !state.camera_blocked {
            set_input_blocking(state.canvas.hwnd, true, "camera-drag");
            state.camera_blocked = true;
        }
        tracing::debug!("镜头拖动开始");
        return false;
    }

    // ── 3. 拖动中: 累积位移, 每 16ms 合并重建一帧 ──
    if state.camera_dragging {
        // 用系统光标前后差(含指针加速), 与 macOS locationInWindow 同语义;
        // raw mouse 的 lLastX/lLastY 是设备 mickey, 与屏幕位移比例不一致。
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let dx = (pt.x - state.camera_last_cursor.0) as f64;
        let dy = (pt.y - state.camera_last_cursor.1) as f64;
        state.camera_last_cursor = (pt.x, pt.y);
        if dx != 0.0 || dy != 0.0 {
            let z = camera().step_zoom();
            // 抓手拖动方向 = 常规直觉: 鼠标向右拖 → 画面内容跟着向右(像抓住纸拖),
            // 上下同理。canvas_pan_by 内部 pan -= d, 所以这里直接传正的屏幕位移。
            // (键盘 WASD / 滚轮缩放的方向仍在 perform_action / 滚轮分支里单独取反。)
            state.camera_pending_dx += dx / z;
            state.camera_pending_dy += dy / z;
            if !state.camera_pan_timer {
                state.camera_pan_timer = true;
                let _ = SetTimer(
                    Some(state.canvas.hwnd),
                    TIMER_CAMERA_PAN,
                    CAMERA_PAN_INTERVAL_MS,
                    None,
                );
            }
        }
        if (btn_flags & RI_MOUSE_LEFT_BUTTON_UP as u16) != 0 {
            state.camera_dragging = false;
            flush_camera_pan(state);
            tracing::debug!("镜头拖动结束");
            // 修饰键还在: 保持拦截以便继续做镜头操作; 松开由键盘 key-up 还原
            camera_mods_maybe_released(state);
        }
        return false;
    }

    // ── 4. 滚轮缩放(修饰键跟随「自由涂鸦: 放大」; 以光标为锚) ──
    if (btn_flags & RI_MOUSE_WHEEL as u16) == 0 {
        return false; // 非滚轮(移动/按键)全部忽略
    }
    let delta = mouse.Anonymous.Anonymous.usButtonData as i16;
    if delta == 0 || !camera_available(state) || !camera_mods_match() {
        return false;
    }
    // 滚轮方向按用户直觉(2026-11 要求取反): 前滚(正, 远离自己) → 放大,
    // 后滚(负) → 缩小。
    let factor = if delta > 0 { 1.1 } else { 1.0 / 1.1 };
    // 光标屏幕坐标(虚拟桌面) → 视图坐标(虚拟屏左上为原点)
    let mut pt = POINT::default();
    let _ = GetCursorPos(&mut pt);
    let vx = (pt.x - GetSystemMetrics(SM_XVIRTUALSCREEN)) as f32;
    let vy = (pt.y - GetSystemMetrics(SM_YVIRTUALSCREEN)) as f32;
    canvas_zoom_at(state, factor, vx as f64, vy as f64);
    true
}
