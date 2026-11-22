pub fn run() {
    unsafe {
        // 初始化数据库(创建首个屏幕记录),必须在任何 DB 访问之前
        {
            let sw = GetSystemMetrics(SM_CXVIRTUALSCREEN);
            let sh = GetSystemMetrics(SM_CYVIRTUALSCREEN);
            glaspen_core::export::glaspen2_init_db(sw, sh);
        }

        let hwnd = create_overlay_window();

        // 首启引导: macOS 启动不弹面板(只在菜单/热键里打开), Windows 没有
        // 菜单栏, 所以仅在**第一次**启动时拉起一次设置面板, 之后靠
        // Ctrl+Alt+D(可配置 settings 动作)。用 settings_opened 键记住。
        {
            let opened = glaspen_core::runtime()
                .block_on(glaspen_core::db::load_setting("settings_opened"))
                .is_some();
            if !opened {
                glaspen_core::runtime().block_on(glaspen_core::db::save_setting(
                    "settings_opened",
                    "1",
                ));
                crate::show_settings_window();
            }
        }

        let mut pen_r = 1.0;
        let mut pen_g = 0.0;
        let mut pen_b = 0.0;
        // 默认线宽与 macOS 一致 = 1.0(宽档预设索引 3);有存档时下面的
        // load_settings_parts 会覆盖它
        let mut width_scale = 1.0;
        glaspen_core::export::glaspen2_load_settings_parts(
            &mut pen_r,
            &mut pen_g,
            &mut pen_b,
            &mut width_scale,
        );
        // 描边: 从库恢复(macOS load_bool_setting("outline") 同款, 重启保留)
        let outline_enabled = glaspen_core::runtime()
            .block_on(glaspen_core::db::load_setting("outline"))
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0)
            != 0;
        OUTLINE_ENABLED.store(
            outline_enabled,
            std::sync::atomic::Ordering::Relaxed,
        );
        glaspen_core::export::glaspen2_set_stroke_outline(outline_enabled as i32);
        // 软阴影 / 气泡捕获: 与 macOS 启动恢复段对齐(core 侧导出/缩略图要用)
        let soft_shadow = glaspen_core::runtime()
            .block_on(glaspen_core::db::load_setting("softShadow"))
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0)
            != 0;
        glaspen_core::export::glaspen2_set_soft_shadow(soft_shadow as i32);
        let bubble_capture = glaspen_core::runtime()
            .block_on(glaspen_core::db::load_setting("show_bubble_tab"))
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0)
            != 0;
        glaspen_core::export::glaspen2_bubble_set_capture(bubble_capture as i32);
        let frosted = glaspen_core::runtime()
            .block_on(glaspen_core::db::load_setting("frostedGlass"))
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0)
            != 0;
        let show_grid = glaspen_core::runtime()
            .block_on(glaspen_core::db::load_setting("grid"))
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0)
            != 0;
        let grid_divider = glaspen_core::runtime()
            .block_on(glaspen_core::db::load_setting("grid_divider"))
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0)
            .clamp(0, 4);
        let ethereal = glaspen_core::runtime()
            .block_on(glaspen_core::db::load_setting("ethereal"))
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0)
            != 0;
        let grid_follow_strokes = glaspen_core::runtime()
            .block_on(glaspen_core::db::load_setting("gridFollowStrokes"))
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0)
            != 0;
        let invert_ink = glaspen_core::runtime()
            .block_on(glaspen_core::db::load_setting("invertInk"))
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0)
            != 0;
        let invert_fps = glaspen_core::runtime()
            .block_on(glaspen_core::db::load_setting("invertFps"))
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(30);
        let glass_follow_strokes = glaspen_core::runtime()
            .block_on(glaspen_core::db::load_setting("glassFollowStrokes"))
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(1) // 缺省 = 跟随(macOS 历史行为)
            != 0;
        let pressure_monitor = glaspen_core::runtime()
            .block_on(glaspen_core::db::load_setting("pressureMonitor"))
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0)
            != 0;
        // 手写消息集成(与 macOS 同库键):总开关 + 共享上行恢复
        let chat_integration = glaspen_core::runtime()
            .block_on(glaspen_core::db::load_setting("chat_integration"))
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0)
            != 0;
        if chat_integration {
            let share_on = glaspen_core::runtime()
                .block_on(glaspen_core::db::load_setting("share_ink"))
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap_or(0)
                != 0;
            if share_on {
                glaspen_core::export::glaspen2_share_ink_set_active(1);
            }
        }
        // 网格尺寸(面板「网格大小」)与页面缩略图条(macOS 同库键)
        load_grid_size();
        let minimap = glaspen_core::runtime()
            .block_on(glaspen_core::db::load_setting("minimap"))
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0)
            != 0;
        MINIMAP_ENABLED.store(minimap, std::sync::atomic::Ordering::SeqCst);

        // 恢复画布模式与镜头(与 macOS 一致):两种模式独立存储,
        // 无限画布全局仅一个;重启后回到离开时的镜头位置。
        let infinite = glaspen_core::runtime()
            .block_on(glaspen_core::db::load_setting("infinite_canvas"))
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0)
            != 0;
        // 快捷录制 GIF 的质量设置(与 macOS 同键名)
        load_gif_settings();
        INFINITE_CANVAS.store(infinite, std::sync::atomic::Ordering::SeqCst);
        glaspen_core::export::glaspen2_set_canvas_kind(if infinite { 1 } else { 0 });
        if infinite {
            glaspen_core::export::glaspen2_load_infinite_strokes();
            let (mut px, mut py, mut pz) = (0.0f64, 0.0f64, 0.0f64);
            glaspen_core::export::glaspen2_get_infinite_transform(&mut px, &mut py, &mut pz);
            set_cam(px, py, if pz > 0.0 { pz } else { 1.0 });
        }

        // HUD 窗口(通知居中 + 压力监控左上角)
        {
            let (notif_hwnd, pm_hwnd) = hud_create();
            HUD = Box::into_raw(Box::new(HudState {
                notif_hwnd,
                pm_hwnd,
                notif: None,
                pm_text: String::new(),
                pm_visible: pressure_monitor,
            }));
            hud_toggle_pressure(pressure_monitor);
        }

        let mut canvas = OverlayCanvas::create(hwnd);

        // 存档色吸附到最近预设(与 macOS 同策略): 色板调亮这类更新后,
        // 旧存档值自动迁移; DB 无存档时默认纯红也会归到当前红预设。
        use glaspen_core::presets::COLOR_PRESETS;
        let selected_color = closest_color_index(pen_r, pen_g, pen_b);
        let (pen_r, pen_g, pen_b) = COLOR_PRESETS[selected_color];
        canvas.color = (pen_r as u8, pen_g as u8, pen_b as u8);

        let draw = DrawState {
            pen_r,
            pen_g,
            pen_b,
            width_scale,
            selected_color,
            selected_width: closest_width_index(width_scale),
            // 换色/换粗细的历史起点(与 macOS 初始值一致: 黑 / 细)
            prev_color: 9,
            prev_width: 2,
            enabled: true,
            outline_enabled,
            show_grid,
            grid_divider,
            frosted,
            ethereal,
            grid_follow_strokes,
            glass_follow_strokes,
            soft_shadow,
            invert_ink,
            invert_fps,
            pressure_monitor,
            chat_integration,
        };

        let mut state = OverlayState {
            canvas,
            draw,
            pen_path: Vec::new(),
            stroke_modeler: StrokeModeler::default(),
            start_time: Instant::now(),
            last_report: Instant::now(),
            last_activity: Instant::now(),
            last_hover: None,
            in_stroke: false,
            strokes_visible: true,
            peek_active: false,
            last_no_next: None,
            camera_dragging: false,
            camera_last_cursor: (0, 0),
            camera_pending_dx: 0.0,
            camera_pending_dy: 0.0,
            camera_pan_timer: false,
            camera_blocked: false,
            gif_recording: false,
            gif_record_start: -1,
            msg_record_start: -1,
            ink_draft_active: false,
        };
        let _ = state.stroke_modeler.reset_w_params(modeler_params());
        // 按当前画布模式绘制网格与笔迹(无限画布重启后恢复镜头与内容)
        redraw_from_strokes(&mut state);
        // 飘渺模式: 启动就落位到"隐藏"态(macOS gl_ethereal_restore_now 同款),
        // 等笔靠近/落笔再露出; 否则重启后笔迹会先闪一下
        if state.draw.ethereal {
            hide_strokes(&mut state);
        }
        // 初始穿透(WS_EX_TRANSPARENT),鼠标可正常操作;笔事件到达时自动唤醒拦截
        set_input_blocking(hwnd, false, "init");
        // 鼠标输入看门狗(独立线程): 拦截态 + 笔 1.5s 无报告 → 强制恢复穿透
        spawn_input_watchdog(hwnd);
        STATE.store(Box::into_raw(Box::new(state)), Ordering::SeqCst);

        // 注册 Raw Input 设备收报告(WM_INPUT 不依赖 hit test,穿透时也能收到):
        // 数位笔(悬空+落笔) + 鼠标(滚轮缩放) + 键盘(录制 GIF 检测松键)
        let mut devices = [
            RAWINPUTDEVICE {
                usUsagePage: 0x0D,
                usUsage: 0x01,
                dwFlags: RIDEV_INPUTSINK,
                hwndTarget: hwnd,
            },
            RAWINPUTDEVICE {
                usUsagePage: 0x0D,
                usUsage: 0x02,
                dwFlags: RIDEV_INPUTSINK,
                hwndTarget: hwnd,
            },
            RAWINPUTDEVICE {
                usUsagePage: 0x01,
                usUsage: 0x02,
                dwFlags: RIDEV_INPUTSINK,
                hwndTarget: hwnd,
            },
            RAWINPUTDEVICE {
                usUsagePage: 0x01,
                usUsage: 0x06,
                dwFlags: RIDEV_INPUTSINK,
                hwndTarget: hwnd,
            },
        ];
        let r = RegisterRawInputDevices(&mut devices, std::mem::size_of::<RAWINPUTDEVICE>() as u32);
        tracing::info!("RegisterRawInputDevices: {:?}", r);

        // 可配置快捷键(对齐 macOS v0.6.0): 从 user_settings.shortcuts 载入,
        // 合并默认/去冲突后注册; 面板改键经设置管道(setSetting shortcuts)
        // 热更新, 录制期间自动注销/恢复。固定热键(按住类)也在里面;
        // 「彻底退出」已从固定热键迁入可配置动作表(默认仍是 Ctrl+Alt+Q)。
        init(hwnd, chat_integration);

        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _ = UpdateWindow(hwnd);
        if frosted {
            apply_frosted(hwnd, true);
        }

        // Flutter 设置管道线程
        {
            let pipe_hwnd = hwnd.0 as isize;
            std::thread::spawn(move || {
                run_settings_pipe_server(pipe_hwnd);
            });
        }

        tracing::info!("全屏透明涂鸦已启动(WM_INPUT + ink-stroke-modeler + cairo)。");
        tracing::info!(
            "快捷键: 面板「设置 → 快捷键」可逐动作录制;核心/开关类 Ctrl+Alt+字母 —— Z 撤销 / `·1 翻页 / V 开关 / D 设置 / Shift+Q 退出 / C 清空 / G 导出 / S 复制 / B 磨砂 / X 飘渺 / I 自由涂鸦 / Shift+C 换色 / T 换粗细, 无限画布镜头 Win+Alt+W/A/S/D、X 放大 / Z 缩小(某个键被其它软件占用时只把该键降级为 Alt+Shift+同键, 其余仍是 Win+Alt; 鼠标手势固定用 Win+Alt, 不会切输入法; 面板显示实际值);R 按住录 GIF / 2·3 按住手写消息(集成开时)为固定保留组合"
        );
        // Windows 没有菜单栏/托盘: 设置面板只能靠 settings 热键呼出。启动时把
        // 当前键位提示一下(1 秒), 免得用户看到"只有玻璃"以为面板不见了 ——
        // 默认键位刚改过一轮(设置从 Ctrl+Alt+D 变成 Ctrl+Alt+Shift+D)尤其需要。
        let settings_chord = chord_display("settings");
        if !settings_chord.is_empty() {
            hud_notify(&format!("按 {settings_chord} 打开设置面板"));
        }
        run_loop();

        let p = STATE.swap(std::ptr::null_mut(), Ordering::SeqCst);
        if !p.is_null() {
            drop(Box::from_raw(p));
        }
        if !HUD.is_null() {
            drop(Box::from_raw(HUD));
            HUD = std::ptr::null_mut();
        }
    }
}

fn create_overlay_window() -> HWND {
    unsafe {
        let class_name = wide_string("Glaspen2OverlayV2");
        let hinst: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
        let wc = WNDCLASSW {
            style: WNDCLASS_STYLES(0),
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: HICON::default(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: HBRUSH::default(),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
        };
        let _ = RegisterClassW(&wc);

        let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let cx = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let cy = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        let hwnd = CreateWindowExW(
            // WS_EX_NOACTIVATE: 镜头拖动/落笔拦截时会临时清掉 WS_EX_TRANSPARENT
            // 让窗口可命中 —— 没有 NOACTIVATE 的话点一下就把前台 App 顶掉,
            // 之后键盘全进覆盖层被吞。macOS 用全局 tap 消费点击也不抢焦点。
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            PCWSTR(class_name.as_ptr()),
            PCWSTR::null(),
            WS_POPUP,
            x,
            y,
            cx,
            cy,
            None,
            None,
            Some(hinst),
            None,
        )
        .expect("CreateWindowExW failed");
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            cx,
            cy,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        {
            let mut h = OVERLAY_HWND.lock().unwrap_or_else(|e| e.into_inner());
            *h = hwnd.0 as isize;
        }
        hwnd
    }
}

/// 拦截/放行鼠标(WS_EX_TRANSPARENT 的反向操作)。
/// blocking=true 时窗口可命中, 驱动为笔合成的鼠标事件会被覆盖层吃掉
/// (不会点穿到下层窗口); blocking=false 时窗口穿透, 系统鼠标/笔都直达
/// 下层窗口 —— 这就是"键鼠/笔分离"的实现方式。
/// `why` 只进日志, 用于排查拦截态卡住。
fn set_input_blocking(hwnd: HWND, blocking: bool, why: &str) {
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let want_transparent = !blocking;
        let has_transparent = (style & (WS_EX_TRANSPARENT.0 as isize)) != 0;
        INPUT_BLOCKED.store(blocking, std::sync::atomic::Ordering::SeqCst);
        // 面板在覆盖层上面时, 数位板驱动合成的鼠标事件会被 Flutter 面板当成
        // 鼠标(点按钮/悬停)—— 笔落下或镜头交互(拦截态)期间把覆盖层抬到面板
        // 上面: 合成鼠标被覆盖层吃掉, 面板保持纯鼠标交互。放行后再把面板放回
        // 上面(没有面板时是无害的空操作)。
        crate::set_overlay_above_panel(hwnd, blocking);
        if want_transparent == has_transparent {
            // 样式未变化时不调用 SetWindowLongPtr:书写中每个笔事件都走到这里,
            // 200 次/秒的冗余 win32k 调用会加剧输入管线压力(卡顿嫌疑之一)
            return;
        }
        let transparent = if blocking {
            0
        } else {
            WS_EX_TRANSPARENT.0 as isize
        };
        let new_style = (style & !(WS_EX_TRANSPARENT.0 as isize)) | transparent;
        let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style);
        tracing::debug!(
            "鼠标拦截{} ({why})",
            if blocking { "开" } else { "关" }
        );
    }
}

/// 鼠标输入看门狗: 不依赖消息循环的最后一层保险。
/// 只要覆盖层处于拦截态且笔已 1.5s 没有任何 HID 报告, 就强制恢复穿透 ——
/// 鼠标永久点不动比"少画一笔"严重得多(之前消息循环/计时器出岔子时卡死过)。
fn spawn_input_watchdog(hwnd: HWND) {
    let hwnd_val = hwnd.0 as isize;
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let overlay = HWND(hwnd_val as *mut std::ffi::c_void);
            // 通知窗口兜底隐藏(计时器丢消息/窗口线程短暂阻塞都能自愈)
            hud_watchdog_check(overlay);
            let now = now_millis();
            let last = LAST_PEN_REPORT_MS.load(std::sync::atomic::Ordering::SeqCst);
            let blocked = INPUT_BLOCKED.load(std::sync::atomic::Ordering::SeqCst);
            // 拦截态没有继续的理由就恢复穿透: 笔已 1.5s 无报告, 且镜头修饰键
            // 也没按着(key-up 万一丢了不会永久拦截鼠标)。
            if blocked
                && !camera_mods_held_now()
                && (last == 0 || now.saturating_sub(last) > 1500)
            {
                set_input_blocking(
                    HWND(hwnd_val as *mut std::ffi::c_void),
                    false,
                    "watchdog",
                );
                tracing::warn!(
                    "[看门狗] 笔已 {}ms 无报告且镜头键未按仍拦截鼠标, 已强制恢复穿透",
                    now.saturating_sub(last)
                );
            }
        }
    });
}

// ── 无限画布:Ctrl+Alt+滚轮 缩放(鼠标 Raw Input) ──
// overlay 平时 WS_EX_TRANSPARENT 穿透且无焦点,滚轮只会发给前台窗口。
// 不用 WH_MOUSE_LL 低级钩子:LL 钩子是同步回调,系统原始输入线程(RIT)
// 要等我们处理完才放行输入;书写时本进程线程繁忙会造成钩子超时,
// 拖慢整条输入管线,笔报告成批延迟(表现为书写周期性卡顿)。
// 鼠标同样走 Raw Input(RIDEV_INPUTSINK 排队通知,不阻塞 RIT),
// 与数位笔同一条 WM_INPUT 通道,只取滚轮,其余忽略。

