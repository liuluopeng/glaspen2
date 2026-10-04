pub fn run() {
    unsafe {
        // 初始化数据库(创建首个屏幕记录),必须在任何 DB 访问之前
        {
            let sw = GetSystemMetrics(SM_CXVIRTUALSCREEN);
            let sh = GetSystemMetrics(SM_CYVIRTUALSCREEN);
            glaspen_core::export::glaspen2_init_db(sw, sh);
        }

        let hwnd = create_overlay_window();

        let mut pen_r = 1.0;
        let mut pen_g = 0.0;
        let mut pen_b = 0.0;
        let mut width_scale = 0.3;
        glaspen_core::export::glaspen2_load_settings_parts(
            &mut pen_r,
            &mut pen_g,
            &mut pen_b,
            &mut width_scale,
        );
        let outline_enabled = OUTLINE_ENABLED.load(std::sync::atomic::Ordering::Relaxed);
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
            .clamp(0, 3);
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
        let soft_shadow = glaspen_core::runtime()
            .block_on(glaspen_core::db::load_setting("softShadow"))
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0)
            != 0;
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
            enabled: true,
            show_rainbow: false,
            outline_enabled,
            show_grid,
            grid_divider,
            frosted,
            ethereal,
            grid_follow_strokes,
            glass_follow_strokes,
            soft_shadow,
            pressure_monitor,
            chat_integration,
        };

        let mut state = OverlayState {
            canvas,
            draw,
            pen_path: Vec::new(),
            stroke_modeler: StrokeModeler::default(),
            start_time: Instant::now(),
            in_stroke: false,
            strokes_visible: true,
            gif_recording: false,
            gif_record_start: -1,
            msg_record_start: -1,
            ink_draft_active: false,
        };
        let _ = state.stroke_modeler.reset_w_params(modeler_params());
        // 按当前画布模式绘制网格与笔迹(无限画布重启后恢复镜头与内容)
        redraw_from_strokes(&mut state);
        // 初始穿透(WS_EX_TRANSPARENT),鼠标可正常操作;笔事件到达时自动唤醒拦截
        set_input_blocking(hwnd, false);
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
        println!("[overlay] RegisterRawInputDevices: {:?}", r);

        // 热键(README 快捷键表):Ctrl+Alt+C 新建画布 / V 开关 / Z 撤销 /
        // ` / 1 翻页(与 macOS b7cb22d 同改:J/K → 左手单手可及的 ~/1) /
        // G 导出 / B 模糊背景 / Q 退出 / X 固定↔飘渺;
        // 无限画布:方向键平移 / PageUp·PageDown 键盘缩放
        let mods = HOT_KEY_MODIFIERS(MOD_CONTROL.0 | MOD_ALT.0);
        RegisterHotKey(Some(hwnd), 1, mods, 'C' as u32).ok();
        RegisterHotKey(Some(hwnd), 2, mods, 'V' as u32).ok();
        RegisterHotKey(Some(hwnd), 3, mods, 'Z' as u32).ok();
        RegisterHotKey(Some(hwnd), 4, mods, VK_OEM_3.0 as u32).ok(); // ` 上一页
        RegisterHotKey(Some(hwnd), 5, mods, '1' as u32).ok(); // 1 下一页
        RegisterHotKey(Some(hwnd), 6, mods, 'G' as u32).ok();
        RegisterHotKey(Some(hwnd), 7, mods, 'B' as u32).ok();
        RegisterHotKey(Some(hwnd), 8, mods, 'Q' as u32).ok();
        RegisterHotKey(Some(hwnd), 9, mods, 'X' as u32).ok();
        RegisterHotKey(Some(hwnd), 10, mods, VK_LEFT.0 as u32).ok();
        RegisterHotKey(Some(hwnd), 11, mods, VK_UP.0 as u32).ok();
        RegisterHotKey(Some(hwnd), 12, mods, VK_RIGHT.0 as u32).ok();
        RegisterHotKey(Some(hwnd), 13, mods, VK_DOWN.0 as u32).ok();
        RegisterHotKey(Some(hwnd), 14, mods, VK_PRIOR.0 as u32).ok(); // PageUp
        RegisterHotKey(Some(hwnd), 15, mods, VK_NEXT.0 as u32).ok(); // PageDown
        RegisterHotKey(Some(hwnd), 16, mods, 'R' as u32).ok(); // 按住录 GIF
        RegisterHotKey(Some(hwnd), 17, mods, '2' as u32).ok(); // 按住手写草稿(集成开)
        RegisterHotKey(Some(hwnd), 18, mods, '3' as u32).ok(); // 按住手写消息直发(集成开)

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

        println!("[overlay] 全屏透明涂鸦已启动(WM_INPUT + ink-stroke-modeler + cairo)。");
        println!(
            "[overlay] 快捷键: Ctrl+Alt+C 新建 / V 开关 / Z 撤销 / `·1 翻页 / G 导出 / B 模糊 / X 固定↔飘渺 / Q 退出 / 2 按住手写草稿 / 3 按住手写直发(集成开时);无限画布: 方向键平移 / PageUp·Down 缩放 / Ctrl+Alt+滚轮缩放"
        );
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
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
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
            let mut h = OVERLAY_HWND.lock().unwrap();
            *h = hwnd.0 as isize;
        }
        hwnd
    }
}

fn set_input_blocking(hwnd: HWND, blocking: bool) {
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let want_transparent = !blocking;
        let has_transparent = (style & (WS_EX_TRANSPARENT.0 as isize)) != 0;
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
    }
}

// ── 无限画布:Ctrl+Alt+滚轮 缩放(鼠标 Raw Input) ──
// overlay 平时 WS_EX_TRANSPARENT 穿透且无焦点,滚轮只会发给前台窗口。
// 不用 WH_MOUSE_LL 低级钩子:LL 钩子是同步回调,系统原始输入线程(RIT)
// 要等我们处理完才放行输入;书写时本进程线程繁忙会造成钩子超时,
// 拖慢整条输入管线,笔报告成批延迟(表现为书写周期性卡顿)。
// 鼠标同样走 Raw Input(RIDEV_INPUTSINK 排队通知,不阻塞 RIT),
// 与数位笔同一条 WM_INPUT 通道,只取滚轮,其余忽略。

