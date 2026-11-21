unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if STATE.load(Ordering::SeqCst).is_null() {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }

    match msg {
        WM_INPUT => {
            // 先分流:鼠标输入(滚轮缩放)与数位笔(HID)走不同处理;
            // 鼠标事件绝不能触碰笔状态/穿透样式(否则会打断下层操作)
            let hraw = HRAWINPUT(lparam.0 as *mut core::ffi::c_void);
            let mut size: u32 = 0;
            let _ = GetRawInputData(
                hraw,
                RID_INPUT,
                None,
                &mut size,
                std::mem::size_of::<RAWINPUTHEADER>() as u32,
            );
            if size == 0 {
                return LRESULT(0);
            }
            let n = ((size as usize) + 7) / 8;
            let mut buf = vec![0u64; n];
            let written = GetRawInputData(
                hraw,
                RID_INPUT,
                Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
                &mut size,
                std::mem::size_of::<RAWINPUTHEADER>() as u32,
            );
            if written == 0 {
                return LRESULT(0);
            }
            let dw_type = u32::from_le_bytes(
                std::slice::from_raw_parts(buf.as_ptr() as *const u8, 4)
                    .try_into()
                    .unwrap(),
            );
            if dw_type == RIM_TYPEMOUSE.0 {
                let state = &mut *STATE.load(Ordering::SeqCst);
                handle_mouse_raw(&buf, state);
                return LRESULT(0);
            }
            if dw_type == RIM_TYPEKEYBOARD.0 {
                // 键盘仅用于录制 GIF 时检测 Ctrl+Alt+R 松开,其余忽略
                let state = &mut *STATE.load(Ordering::SeqCst);
                handle_keyboard_raw(&buf, state);
                return LRESULT(0);
            }
            if dw_type != RIM_TYPEHID.0 {
                return LRESULT(0); // 其余设备不经此窗口
            }
            let state = &mut *STATE.load(Ordering::SeqCst);
            if !state.draw.enabled {
                // 涂鸦关闭:保持穿透,不拦截、不处理
                cancel_camera_drag(state);
                set_input_blocking(hwnd, false, "draw-disabled");
                return LRESULT(0);
            }
            // 记录"笔还在活动"的时间戳:穿透计时器用它区分"按住不动"
            // 和"抬笔报告丢了/笔已离开感应区"; 看门狗线程也读它。
            state.last_report = std::time::Instant::now();
            LAST_PEN_REPORT_MS.store(now_millis(), Ordering::SeqCst);
            let _ = KillTimer(Some(hwnd), TIMER_UNBLOCK);
            let _ = KillTimer(Some(hwnd), TIMER_PEEK);
            // peek 计时器被笔活动取消时同步清标志(否则 peek_lens_change 会误判
            // "有计时器在跑", 笔悬停移动镜头后 1s 把笔迹藏掉)
            state.peek_active = false;

            // 飘渺模式:笔悬空/落笔 → 显示笔迹(仅首次,避免高频全量重绘)
            if state.draw.ethereal && !state.strokes_visible {
                show_strokes(state);
            }

            if let Some(dirty) = process_raw_hid(&buf) {
                let state = &mut *STATE.load(Ordering::SeqCst);
                state.canvas.present_rect(&dirty);
            }

            // 拦截策略(兼顾"第一笔不穿透"与"笔搁板上时鼠标可用"):
            //  - 落笔中, 或笔最近 400ms 内在悬空移动 → 清 WS_EX_TRANSPARENT,
            //    让驱动为笔合成的鼠标事件被覆盖层吃掉, 不会点穿到下层窗口;
            //  - 笔静止悬停/搁在板上(驱动即使仍在发报告) → 不刷新 activity,
            //    计时器到期就恢复穿透, 系统鼠标照常工作(键鼠/笔分离)。
            // 恢复计时器**每份报告都要重新武装**: 抬笔报告的 in_stroke 已经是
            // false, 若只在书写中武装, 上面两行 KillTimer 就会把唯一一次恢复
            // 穿透的机会杀掉 —— 全屏窗口从此不透明(v0.6.0 的"鼠标卡死", 见 44fac24)。
            let state = &mut *STATE.load(Ordering::SeqCst);
            let active = state.in_stroke || state.last_activity.elapsed() < PEN_ACTIVE_WINDOW;
            if active {
                set_input_blocking(
                    hwnd,
                    true,
                    if state.in_stroke {
                        "pen-down"
                    } else {
                        "pen-hover-move"
                    },
                );
            }
            let _ = SetTimer(Some(hwnd), TIMER_UNBLOCK, UNBLOCK_DELAY_MS, None);
            LRESULT(0)
        }
        WM_TIMER => {
            let state = &mut *STATE.load(Ordering::SeqCst);
            match wparam.0 as usize {
                TIMER_UNBLOCK => {
                    let _ = KillTimer(Some(hwnd), TIMER_UNBLOCK);
                    // 笔还按着但报告停了(按住不动): 续一个计时器继续拦截。
                    // 超过 STALE_STROKE 没有报告 = 抬笔报告丢了/笔离开了感应区,
                    // 强制收笔, 绝不让 in_stroke 卡在 true 把鼠标永久拦住。
                    if state.in_stroke {
                        if state.last_report.elapsed() < STALE_STROKE {
                            let _ = SetTimer(Some(hwnd), TIMER_UNBLOCK, UNBLOCK_DELAY_MS, None);
                            return LRESULT(0);
                        }
                        tracing::warn!(
                            "笔报告已 {:.0}ms 未更新仍处于落笔状态, 强制收笔并恢复鼠标穿透",
                            state.last_report.elapsed().as_secs_f64() * 1000.0
                        );
                        end_stroke_cleanup(state);
                    } else if state.last_activity.elapsed() < PEN_ACTIVE_WINDOW {
                        // 笔还在悬空移动(正要落笔): 继续拦截一小会儿,
                        // 保证接下来的落笔不穿透(启动后第一笔就是靠这个挡住)。
                        let _ = SetTimer(Some(hwnd), TIMER_UNBLOCK, UNBLOCK_DELAY_MS, None);
                        return LRESULT(0);
                    } else if state.camera_blocked || state.camera_dragging {
                        // 镜头修饰键还按着/正在拖动: 拦截权归镜头, 别在这里还原穿透
                        return LRESULT(0);
                    }
                    // 笔已静止/离开: 设 WS_EX_TRANSPARENT 恢复穿透
                    set_input_blocking(hwnd, false, "idle");
                    // 飘渺模式:隐藏笔迹
                    if state.draw.ethereal && state.strokes_visible {
                        hide_strokes(state);
                    }
                }
                TIMER_CAMERA_PAN => {
                    let _ = KillTimer(Some(hwnd), TIMER_CAMERA_PAN);
                    flush_camera_pan(state);
                }
                TIMER_DISPLAY => {
                    let _ = KillTimer(Some(hwnd), TIMER_DISPLAY);
                    // 到期重读真实几何(on_display_change 内部读 GetSystemMetrics)
                    on_display_change(state);
                }
                TIMER_PEEK => {
                    let _ = KillTimer(Some(hwnd), TIMER_PEEK);
                    state.peek_active = false;
                    // 飘渺模式 peek 到期(翻页/镜头):无笔活动时隐藏
                    if state.draw.ethereal && !state.in_stroke && state.strokes_visible {
                        hide_strokes(state);
                    }
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_HOTKEY => {
            // 热键 id → 动作的统一分发在 hotkeys.rs:
            //   8=退出 / 16·17·18=按住类(录 GIF·草稿·直发) 固定;
            //   100+=面板里可配置的 19 个动作(动作表下标 + 基址)。
            let state = &mut *STATE.load(Ordering::SeqCst);
            dispatch_hotkey(state, wparam.0 as i32);
            LRESULT(0)
        }
        WM_RECORD_DONE => {
            // 后台 GIF 编码/剪贴板完成(0=复制失败, 1=成功, 2=没有笔迹)
            match wparam.0 as usize {
                1 => hud_notify("GIF 已复制到剪贴板"),
                2 => hud_notify("没有笔迹或导出失败"),
                _ => hud_notify("GIF 复制失败"),
            }
            LRESULT(0)
        }
        WM_TRAY_COMMAND => {
            let state = &mut *STATE.load(Ordering::SeqCst);
            handle_command(state, wparam.0, lparam.0 as usize);
            LRESULT(0)
        }
        WM_HUD_NOTIFY => {
            // 后台线程投递的通知: 在窗口线程弹(避免跨线程碰 HUD 状态)
            let texts: Vec<String> = HUD_QUEUE
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .drain(..)
                .collect();
            for t in texts {
                hud_notify(&t);
            }
            LRESULT(0)
        }
        WM_HOTKEY_RELOAD => {
            // 设置管道线程/录制超时线程投递的"重新注册热键"请求:
            // 只有窗口所属线程能注册(否则 0x80070580 无效窗口)。
            apply_reload(hwnd);
            LRESULT(0)
        }
        WM_KEYDOWN => DefWindowProcW(hwnd, msg, wparam, lparam),
        WM_MOUSEACTIVATE => {
            // 覆盖层可命中时(落笔拦截/镜头拖动)也不抢前台焦点
            LRESULT(MA_NOACTIVATE as isize)
        }
        WM_DISPLAYCHANGE => {
            // 防抖(macOS panel.m 同款): 事件成串到达时只重置计时, 稳定 2.5s 后
            // 再处理; 否则每个瞬态分辨率都会凭空建一页。
            let _ = SetTimer(Some(hwnd), TIMER_DISPLAY, DISPLAY_DEBOUNCE_MS, None);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_DESTROY => {
            // 无限画布退出前存镜头(节流持久化可能丢最后一次拖动)
            if infinite_on() {
                persist_camera();
            }
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// ── 入口 ──

