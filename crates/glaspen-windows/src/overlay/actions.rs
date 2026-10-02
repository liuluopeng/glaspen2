fn run_loop() {
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            let _ = DispatchMessageW(&msg);
        }
    }
}

fn wide_string(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

// ── 命令处理(设置管道 / 热键) ──

fn handle_command(state: &mut OverlayState, cmd: usize, param: usize) {
    // 开关类命令:管道传入 0/1 时按值设置,否则翻转(热键/无参调用)
    let param_on = |cur: bool| -> bool {
        if param == 0 || param == 1 {
            param == 1
        } else {
            !cur
        }
    };
    if cmd >= CMD_SELECT_COLOR && cmd < CMD_SELECT_COLOR + 10 {
        let idx = cmd - CMD_SELECT_COLOR;
        if idx < COLOR_PRESETS.len() {
            state.draw.pen_r = COLOR_PRESETS[idx].0;
            state.draw.pen_g = COLOR_PRESETS[idx].1;
            state.draw.pen_b = COLOR_PRESETS[idx].2;
            state.draw.selected_color = idx;
            state.canvas.color = (
                (state.draw.pen_r * 255.0) as u8,
                (state.draw.pen_g * 255.0) as u8,
                (state.draw.pen_b * 255.0) as u8,
            );
            glaspen_core::export::glaspen2_save_settings(
                state.draw.pen_r,
                state.draw.pen_g,
                state.draw.pen_b,
                state.draw.width_scale,
            );
        }
    } else if cmd >= CMD_SELECT_WIDTH && cmd < CMD_SELECT_WIDTH + WIDTH_PRESETS.len() {
        let idx = cmd - CMD_SELECT_WIDTH;
        if idx < WIDTH_PRESETS.len() {
            state.draw.width_scale = WIDTH_PRESETS[idx];
            state.draw.selected_width = idx;
            glaspen_core::export::glaspen2_save_settings(
                state.draw.pen_r,
                state.draw.pen_g,
                state.draw.pen_b,
                state.draw.width_scale,
            );
        }
    } else {
        match cmd {
            x if x == CMD_SAVE_WITH_BG => {
                save_with_bg(state);
                hud_notify("截图成功(含背景)");
            }
            x if x == CMD_SAVE_DRAWING => {
                save_drawing(state);
                hud_notify("截图成功");
            }
            x if x == CMD_SAVE_XOJ => {
                glaspen_core::export::glaspen2_save_xoj();
                hud_notify("笔记已保存");
            }
            x if x == CMD_CLEAR_SCREEN => clear_screen(state),
            x if x == CMD_UNDO => undo_last_stroke(state),
            x if x == CMD_TOGGLE_RAINBOW => {
                state.draw.show_rainbow = !state.draw.show_rainbow;
                if state.draw.show_rainbow {
                    draw_rainbow_indicator(state);
                } else {
                    clear_screen(state);
                }
            }
            x if x == CMD_TOGGLE_OUTLINE => {
                let on = param_on(state.draw.outline_enabled);
                state.draw.outline_enabled = on;
                OUTLINE_ENABLED.store(on, std::sync::atomic::Ordering::Relaxed);
            }
            x if x == CMD_TOGGLE_GRID => {
                let on = param_on(state.draw.show_grid);
                state.draw.show_grid = on;
                glaspen_core::runtime()
                    .block_on(glaspen_core::db::save_setting("grid", if on { "1" } else { "0" }));
                redraw_from_strokes(state);
            }
            x if x == CMD_SET_GRID_DIVIDER => {
                // 分栏:0=无 1=左右两栏 2=上下两栏 3=九宫格(与 macOS 同键同钳制)
                let v = (param as i64).clamp(0, 3) as i32;
                state.draw.grid_divider = v;
                glaspen_core::runtime()
                    .block_on(glaspen_core::db::save_setting("grid_divider", &v.to_string()));
                redraw_from_strokes(state);
            }
            x if x == CMD_TOGGLE_CHAT_INTEGRATION => {
                // 手写消息集成总开关(macOS gl_settings_set_chat_integration 同语义):
                // 关 = 静默终止进行中的录制/草稿 + 停共享上行;
                // 重开 = share_ink 若此前开着则一并恢复
                let on = param_on(state.draw.chat_integration);
                state.draw.chat_integration = on;
                glaspen_core::runtime().block_on(glaspen_core::db::save_setting(
                    "chat_integration",
                    if on { "1" } else { "0" },
                ));
                if on {
                    let share_on = glaspen_core::runtime()
                        .block_on(glaspen_core::db::load_setting("share_ink"))
                        .and_then(|v| v.parse::<i32>().ok())
                        .unwrap_or(0)
                        != 0;
                    if share_on {
                        glaspen_core::export::glaspen2_share_ink_set_active(1);
                    }
                } else {
                    state.msg_record_start = -1;
                    if state.ink_draft_active {
                        state.ink_draft_active = false;
                        std::thread::spawn(|| {
                            let _ = glaspen_core::export::glaspen2_ink_draft_stop();
                        });
                    }
                    glaspen_core::export::glaspen2_share_ink_set_active(0);
                }
            }
            x if x == CMD_TOGGLE_FROSTED => {
                let on = param_on(state.draw.frosted);
                state.draw.frosted = on;
                glaspen_core::runtime().block_on(glaspen_core::db::save_setting(
                    "frostedGlass",
                    if on { "1" } else { "0" },
                ));
                apply_frosted(state.canvas.hwnd, on);
            }
            x if x == CMD_TOGGLE_ETHEREAL => {
                let on = param_on(state.draw.ethereal);
                state.draw.ethereal = on;
                glaspen_core::runtime().block_on(glaspen_core::db::save_setting(
                    "ethereal",
                    if on { "1" } else { "0" },
                ));
                if on {
                    hide_strokes(state);
                    hud_notify("飘渺画布涂鸦模式 (悬空/落笔显示)");
                } else {
                    show_strokes(state);
                    hud_notify("固定画布涂鸦模式");
                }
            }
            x if x == CMD_TOGGLE_PRESSURE_MONITOR => {
                let on = param_on(state.draw.pressure_monitor);
                state.draw.pressure_monitor = on;
                glaspen_core::runtime().block_on(glaspen_core::db::save_setting(
                    "pressureMonitor",
                    if on { "1" } else { "0" },
                ));
                hud_toggle_pressure(on);
                hud_notify(if on {
                    "压力监控已开启"
                } else {
                    "压力监控已关闭"
                });
            }
            x if x == CMD_NAVIGATE_TO_PAGE => {
                // 内容 tab 点击页面:恢复该页笔迹继续绘画
                navigate_to(state, param as i64, "已切换到该页面");
            }
            x if x == CMD_PAGE_PREV => navigate_page(state, false),
            x if x == CMD_PAGE_NEXT => navigate_page(state, true),
            x if x == CMD_EXPORT_SVG_GIF => export_svg_gif_clipboard(state),
            x if x == CMD_TOGGLE_ENABLED => toggle_enabled(state),
            x if x == CMD_TOGGLE_INFINITE_CANVAS => {
                apply_infinite_canvas(state, param_on(infinite_on()))
            }
            x if x == CMD_CANVAS_HOME => {
                // 回到原点 + 100%(设置面板「回到原点」)
                set_cam(0.0, 0.0, 1.0);
                persist_camera();
                redraw_from_strokes(state);
            }
            x if x == CMD_CANVAS_CENTER => {
                // 居中内容包围盒(设置面板「居中内容」)
                let (mut bx, mut by, mut bx2, mut by2) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
                if glaspen_core::export::glaspen2_stroke_bbox(&mut bx, &mut by, &mut bx2, &mut by2) != 0 {
                    let (_, _, z) = cam();
                    let z = if z > ZOOM_MIN { z } else { 1.0 };
                    let (w, h) = (state.canvas.w as f64, state.canvas.h as f64);
                    set_cam(
                        (bx + bx2) * 0.5 - w * 0.5 / z,
                        (by + by2) * 0.5 - h * 0.5 / z,
                        z,
                    );
                    persist_camera();
                }
                redraw_from_strokes(state);
            }
            x if x == CMD_CANVAS_NEW => {
                // 手动新建:清空内容,镜头回原点 + 100%(活页本退化为普通新建)
                if infinite_on() {
                    let _ = glaspen_core::export::glaspen2_clear_strokes(state.canvas.w, state.canvas.h);
                    set_cam(0.0, 0.0, 1.0);
                    persist_camera();
                    redraw_from_strokes(state);
                    hud_notify("已新建画布");
                } else {
                    clear_screen(state);
                }
            }
            x if x == CMD_QUIT => unsafe {
                let _ = DestroyWindow(state.canvas.hwnd);
            },
            _ => {}
        }
    }
}

fn clear_screen(state: &mut OverlayState) {
    // 无限画布不清空(内容多,误清损失大):新建走设置面板的「新建画布」
    if infinite_on() {
        hud_notify("无限画布不会被清除 · 新建请到设置面板手动新建");
        return;
    }
    state.pen_path.clear();
    state.in_stroke = false;
    let params = modeler_params();
    let _ = state.stroke_modeler.reset_w_params(params);
    state.start_time = Instant::now();
    state.canvas.clear();
    if state.draw.show_grid {
        state.canvas.draw_grid(state.draw.grid_divider);
    }
    state.canvas.set_bg_alpha(BG_BLOCK);
    let created = glaspen_core::export::glaspen2_clear_strokes(state.canvas.w, state.canvas.h);
    if state.draw.show_rainbow {
        draw_rainbow_indicator(state);
    }
    if created == 0 {
        // 当前页从未涂鸦:不能连续创建空白画布(macOS 同款提示)
        hud_notify("不能连续创建空白画布,请先涂鸦");
    } else {
        hud_notify("已新建画布");
    }
}

fn toggle_enabled(state: &mut OverlayState) {
    state.draw.enabled = !state.draw.enabled;
    if !state.draw.enabled {
        // 立即恢复穿透
        set_input_blocking(state.canvas.hwnd, false);
    }
    hud_notify(if state.draw.enabled {
        "涂鸦已开启"
    } else {
        "涂鸦已关闭"
    });
}

fn undo_last_stroke(state: &mut OverlayState) {
    let remaining = glaspen_core::export::glaspen2_undo_last_stroke();
    if remaining < 0 {
        hud_notify("没有可撤销的笔画");
        return;
    }
    // 清空画布,从 STROKES 重绘全部剩余笔画
    redraw_from_strokes(state);
    hud_notify("已撤销上一笔");
}

/// 清空画布并从 STROKES 重绘全部笔画(撤销/翻页/网格开关后使用)
fn redraw_from_strokes(state: &mut OverlayState) {
    state.pen_path.clear();
    state.in_stroke = false;
    let params = modeler_params();
    let _ = state.stroke_modeler.reset_w_params(params);
    state.start_time = Instant::now();
    state.canvas.clear();
    if state.draw.show_grid {
        state.canvas.draw_grid(state.draw.grid_divider);
    }
    let ol = if state.draw.outline_enabled { 1.0 } else { 0.0 };
    let z = if infinite_on() { cam().2 as f32 } else { 1.0 };
    {
        let strokes = glaspen_core::STROKES.lock().unwrap();
        for s in strokes.iter() {
            if s.points.is_empty() {
                continue;
            }
            state.canvas.color = (
                (s.r * 255.0) as u8,
                (s.g * 255.0) as u8,
                (s.b * 255.0) as u8,
            );
            // 笔迹存画布坐标:重绘时经镜头变换到视图,线宽同步缩放
            let path: Vec<(f32, f32, f32)> = s
                .points
                .iter()
                .map(|&(x, y, w, _)| {
                    let (vx, vy) = view_from_canvas(x, y);
                    (vx as f32, vy as f32, ((w as f32 * 0.5) * z).max(0.5))
                })
                .collect();
            fill_stroke_path(&mut state.canvas, &path, ol);
        }
    }
    state.canvas.set_bg_alpha(BG_BLOCK);
    state.canvas.present_all();
    if state.draw.show_rainbow {
        draw_rainbow_indicator(state);
    }
}

/// 上一页/下一页(加载目标页笔画并重绘)
fn navigate_page(state: &mut OverlayState, next: bool) {
    let target = if next {
        glaspen_core::export::glaspen2_next_screen_id()
    } else {
        glaspen_core::export::glaspen2_prev_screen_id()
    };
    navigate_to(state, target, if next { "下一页" } else { "上一页" });
}

/// 跳转到指定页面:加载该页笔画并重绘,可继续绘画。
/// 无限画布下先切回活页本(否则会把页笔迹塞进无限画布内存,macOS 同款)。
fn navigate_to(state: &mut OverlayState, target: i64, _label: &str) {
    if infinite_on() {
        apply_infinite_canvas(state, false);
    }
    let current = glaspen_core::export::glaspen2_get_current_screen_id();
    if target <= 0 || target == current {
        eprintln!(
            "[overlay] 没有更多页面 (current={}, target={})",
            current, target
        );
        hud_notify("没有可跳转的页面");
        return;
    }
    let count = glaspen_core::export::glaspen2_load_strokes_for_screen(target);
    redraw_from_strokes(state);
    eprintln!("[overlay] 已切换到页面 {} ({} 笔)", target, count);
    hud_notify(&page_info_text(target));
    // 飘渺模式:短暂显示目标页,随后自动隐藏(除非笔在活动)
    if state.draw.ethereal {
        let hwnd = state.canvas.hwnd;
        let _ = unsafe { SetTimer(Some(hwnd), TIMER_PEEK, PEEK_DELAY_MS, None) };
    }
}

/// 页数+日期通知文本,如 "今天 第2页  第3/5页"(与 macOS 一致)
fn page_info_text(screen_id: i64) -> String {
    let ptr = glaspen_core::export::glaspen2_page_info_json(screen_id);
    if ptr.is_null() {
        return format!("第 {} 页", screen_id);
    }
    let s = unsafe { std::ffi::CStr::from_ptr(ptr) }
        .to_string_lossy()
        .to_string();
    glaspen_core::export::glaspen2_free_c_string(ptr);
    let v: serde_json::Value = match serde_json::from_str(&s) {
        Ok(v) => v,
        Err(_) => return format!("第 {} 页", screen_id),
    };
    let nth = v["nth"].as_i64().unwrap_or(screen_id);
    let pos = v["pos"].as_i64().unwrap_or(0);
    let total = v["total"].as_i64().unwrap_or(0);
    let created = v["created"].as_f64().unwrap_or(0.0) as u64;
    let label = date_label(created);
    format!("{} 第{}页  第{}/{}页", label, nth, pos, total)
}

/// 飘渺模式:隐藏笔迹(保留数据,仅清空显示;网格按"网格跟随涂鸦"决定)
fn hide_strokes(state: &mut OverlayState) {
    // 提交未完成的笔画,避免悬空
    if state.in_stroke {
        glaspen_core::export::glaspen2_end_stroke();
        state.in_stroke = false;
        state.pen_path.clear();
    }
    state.strokes_visible = false;
    state.canvas.clear();
    // 网格跟随涂鸦 → 一起隐藏;否则网格保持可见
    if state.draw.show_grid && !state.draw.grid_follow_strokes {
        state.canvas.draw_grid(state.draw.grid_divider);
    }
    state.canvas.set_bg_alpha(BG_BLOCK);
    state.canvas.present_all();
}

/// 飘渺模式:显示笔迹(从 STROKES 重绘)
fn show_strokes(state: &mut OverlayState) {
    if !state.strokes_visible {
        state.strokes_visible = true;
        redraw_from_strokes(state);
    }
}

// ── 无限画布(自由涂鸦):模式切换与镜头(macOS 同款行为) ──

/// 镜头持久化到 user_settings(仅无限模式有意义)
fn persist_camera() {
    let (px, py, z) = cam();
    glaspen_core::export::glaspen2_set_infinite_transform(px, py, z);
}

/// 应用镜头变换到渲染 + 节流持久化(0.5s 一次,与 macOS 一致)
fn canvas_apply_transform(state: &mut OverlayState) {
    static LAST_SAVE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    redraw_from_strokes(state);
    let now = now_millis();
    if now.saturating_sub(LAST_SAVE.load(std::sync::atomic::Ordering::Relaxed))
        > CAM_SAVE_INTERVAL_MS
    {
        LAST_SAVE.store(now, std::sync::atomic::Ordering::Relaxed);
        persist_camera();
    }
}

/// 镜头平移(⌘⌃方向键同款:Ctrl+Alt+方向键)。书写中忽略(模型器坐标系不能中途跳变)
fn canvas_pan_by(state: &mut OverlayState, dx: f64, dy: f64) {
    if state.in_stroke {
        return;
    }
    let (px, py, z) = cam();
    set_cam(px - dx, py - dy, z);
    canvas_apply_transform(state);
}

/// 缩放镜头:以视图点 (vx, vy) 为锚,zoom ∈ (0.05, 1.0]。
/// 锚点的画布坐标在缩放前后保持同一屏幕位置(view = (canvas−pan)×zoom)。
fn canvas_zoom_at(state: &mut OverlayState, factor: f64, vx: f64, vy: f64) {
    if state.in_stroke {
        return;
    }
    let (px, py, z) = cam();
    let ccx = vx / z + px;
    let ccy = vy / z + py;
    let mut nz = z * factor;
    if nz > ZOOM_MAX {
        nz = ZOOM_MAX;
        if z < ZOOM_MAX {
            static LAST_HINT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let now = now_millis();
            if now.saturating_sub(LAST_HINT.load(std::sync::atomic::Ordering::Relaxed))
                > ZOOM_HINT_MS
            {
                LAST_HINT.store(now, std::sync::atomic::Ordering::Relaxed);
                hud_notify("已达最大缩放 100%");
            }
        }
    }
    if nz < ZOOM_MIN {
        nz = ZOOM_MIN;
    }
    set_cam(ccx - vx / nz, ccy - vy / nz, nz);
    canvas_apply_transform(state);
}

/// 应用无限画布开关(设置面板与热键共用的唯一入口)。
/// 两种模式各自独立存储:活页本用 screens/strokes,无限画布用全局唯一的
/// infinite_strokes。切换时冲刷当前笔画、切存储、载入对应画布的笔迹与镜头。
fn apply_infinite_canvas(state: &mut OverlayState, on: bool) {
    if infinite_on() == on {
        return;
    }
    // 先把在写的笔画落库到"旧"存储
    if state.in_stroke {
        glaspen_core::export::glaspen2_end_stroke();
        state.in_stroke = false;
        state.pen_path.clear();
        let params = modeler_params();
        let _ = state.stroke_modeler.reset_w_params(params);
        state.start_time = Instant::now();
    }
    if infinite_on() {
        persist_camera(); // 离开无限画布前存镜头
    }
    INFINITE_CANVAS.store(on, std::sync::atomic::Ordering::SeqCst);
    glaspen_core::runtime().block_on(glaspen_core::db::save_setting(
        "infinite_canvas",
        if on { "1" } else { "0" },
    ));
    glaspen_core::export::glaspen2_set_canvas_kind(if on { 1 } else { 0 });
    if on {
        glaspen_core::export::glaspen2_load_infinite_strokes();
        let mut px = 0.0f64;
        let mut py = 0.0f64;
        let mut pz = 0.0f64;
        glaspen_core::export::glaspen2_get_infinite_transform(&mut px, &mut py, &mut pz);
        set_cam(px, py, if pz > 0.0 { pz } else { 1.0 });
    } else {
        // 回到活页本:载入当前页;没有页就建一页;镜头恒为原点 + 100%
        let cur = glaspen_core::export::glaspen2_get_current_screen_id();
        if cur > 0 {
            glaspen_core::export::glaspen2_load_strokes_for_screen(cur);
        } else {
            glaspen_core::export::glaspen2_clear_strokes(state.canvas.w, state.canvas.h);
        }
        set_cam(0.0, 0.0, 1.0);
    }
    // 载入什么就显示什么(与 macOS 一致):load_strokes_for_screen /
    // load_infinite_strokes 都是"清空再填充"的替换语义,内存与库天然一致,
    // undo/导出所见即所得。此前的"切换后清空内存副本"会把刚载入的内容
    // 抹成空白,看起来像切换失败,且让 undo/导出与页面数据脱节。
    redraw_from_strokes(state);
    hud_notify(if on {
        "无限画布已开启 (Ctrl+Alt+滚轮缩放 · Ctrl+Alt+方向键平移)"
    } else {
        "无限画布已关闭,回到活页本"
    });
}

// ── 快捷录制 GIF(Ctrl+Alt+R 按住,松开生成并复制剪贴板;macOS ⌘⌃R 同款) ──

/// 提交在写的笔画并复位模型器(录制边界必须落在笔画边界上)
fn finish_active_stroke(state: &mut OverlayState) {
    if state.in_stroke {
        glaspen_core::export::glaspen2_end_stroke();
        // 抬笔扇出(草稿/共享/OCR):与 pen.rs 的正常抬笔路径一致
        glaspen_core::export::glaspen2_notify_stroke_committed();
        state.in_stroke = false;
        state.pen_path.clear();
        let params = modeler_params();
        let _ = state.stroke_modeler.reset_w_params(params);
        state.start_time = Instant::now();
    }
}

/// 按键按下:记录起始笔画序号,之后画的每一笔都进这段 GIF
fn gif_record_start(state: &mut OverlayState) {
    if state.gif_recording {
        return;
    }
    if !state.draw.enabled {
        hud_notify("涂鸦已关闭, 无法录制 GIF");
        return;
    }
    finish_active_stroke(state);
    state.gif_record_start = glaspen_core::export::glaspen2_stroke_count();
    state.gif_recording = true;
    hud_notify("按住绘制, 松开生成 GIF");
}

/// 按键松开:固定笔画区间,后台线程回放渲染编码 GIF → 剪贴板
fn gif_record_stop(state: &mut OverlayState) {
    if !state.gif_recording {
        return;
    }
    let start = state.gif_record_start;
    state.gif_recording = false;
    state.gif_record_start = -1;
    finish_active_stroke(state);
    let end = glaspen_core::export::glaspen2_stroke_count();
    let (fps, resolution, speed, end_mode) = gif_settings();
    // HWND 裸指针不能跨线程,转 isize 传递
    let hwnd = state.canvas.hwnd.0 as isize;
    // 回放渲染 + GIF 压缩可能上百毫秒,不阻塞消息循环;
    // 区间在主线程钉死,新录制不会覆盖尚在编码的这一次
    std::thread::spawn(move || {
        let mut len: i32 = 0;
        let ptr = glaspen_core::export::glaspen2_gif_record_end(
            start, end, fps, resolution, speed, end_mode, &mut len,
        );
        let result = if ptr.is_null() || len <= 0 {
            2 // 没有笔迹或导出失败
        } else {
            let gif = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
            let ok = unsafe { copy_gif_bytes_to_clipboard(gif) };
            glaspen_core::export::glaspen2_free_rust_bytes(ptr, len);
            if ok { 1 } else { 0 }
        };
        unsafe {
            let _ = PostMessageW(
                Some(HWND(hwnd as *mut core::ffi::c_void)),
                WM_RECORD_DONE,
                WPARAM(result as usize),
                LPARAM(0),
            );
        }
    });
}

// ── 手写消息集成(macOS ⌘⌃2/⌘⌃3 同款语义,Windows = Ctrl+Alt+2/3)──
// ⌘⌃3 直发:按住钉住起点,松开把 [start,end) 一次性发送;
// ⌘⌃2 草稿:按住经 DraftInk 流实时推送,松开 half-close 由 axum 决定。
// 总开关 chat_integration 关闭时热键不劫持;两者互斥。

/// Ctrl+Alt+3 按下:钉住录制起点(先把手写中的笔画落定)
fn chat_record_start(state: &mut OverlayState) {
    if state.msg_record_start >= 0 || state.ink_draft_active {
        return;
    }
    finish_active_stroke(state);
    state.msg_record_start = glaspen_core::export::glaspen2_stroke_count();
    hud_notify("书写手写消息… 松开 Ctrl+Alt+3 发送");
}

/// Ctrl+Alt+3 松开:后台线程发送,gRPC 阻塞不进消息循环
fn chat_record_stop(state: &mut OverlayState) {
    let start = state.msg_record_start;
    state.msg_record_start = -1;
    if start < 0 {
        return;
    }
    finish_active_stroke(state);
    let end = glaspen_core::export::glaspen2_stroke_count();
    if end <= start {
        hud_notify("没有新手写内容");
        return;
    }
    std::thread::spawn(move || {
        let sent = glaspen_core::export::glaspen2_chat_send_strokes(start, end);
        if sent >= 0 {
            hud_notify(&format!("手写消息已发送 ({sent} 笔)"));
        } else {
            hud_notify("手写消息发送失败");
        }
    });
}

/// Ctrl+Alt+2 按下:打开草稿通道(失败原因经 FFI 取回)
fn ink_draft_start(state: &mut OverlayState) {
    if state.ink_draft_active || state.msg_record_start >= 0 {
        return;
    }
    finish_active_stroke(state);
    let (w, h) = (state.canvas.w, state.canvas.h);
    if glaspen_core::export::glaspen2_ink_draft_start(w, h) != 0 {
        state.ink_draft_active = true;
        hud_notify("书写手写消息(草稿)… 松开 Ctrl+Alt+2 交给对方");
    } else {
        hud_notify("手写通道开启失败");
    }
}

/// Ctrl+Alt+2 松开:后台线程 half-close 并等 axum 的决定(阻塞 FFI)
fn ink_draft_stop(state: &mut OverlayState) {
    if !state.ink_draft_active {
        return;
    }
    state.ink_draft_active = false;
    finish_active_stroke(state);
    std::thread::spawn(|| {
        let r = glaspen_core::export::glaspen2_ink_draft_stop();
        // 失败原因在后台线程取好再通知,避免与下一次会话竞争(macOS 同款)
        let ptr = glaspen_core::export::glaspen2_ink_draft_last_error();
        let detail = if ptr.is_null() {
            String::new()
        } else {
            unsafe { std::ffi::CStr::from_ptr(ptr) }
                .to_string_lossy()
                .to_string()
        };
        if r > 0 {
            hud_notify(&format!("对方已接收手写草稿 ({r} 笔)"));
        } else if r == 0 {
            hud_notify("对方未采用这份手写草稿");
        } else if !detail.is_empty() {
            hud_notify(&detail);
        } else {
            hud_notify("手写通道失败(未连接或中断)");
        }
    });
}

/// GIF 字节写入剪贴板:注册格式 "GIF"(微信/QQ 粘贴动画时识别此格式)
/// GIF 写入剪贴板,双格式保证粘贴可用:
///  1) CF_HDROP:GIF 落临时文件后按"文件"粘贴(微信/QQ 识别为动画,
///     与 macOS 写文件 URL 到剪贴板同思路);
///  2) 注册格式 "GIF":原始字节,截图类工具按此读动画。
/// 返回是否至少写入了一种格式。
unsafe fn copy_gif_bytes_to_clipboard(gif: &[u8]) -> bool {
    let cf_gif = RegisterClipboardFormatA(b"GIF\0".as_ptr());
    if cf_gif == 0 {
        return false;
    }

    // GIF 落临时文件(剪贴板只存路径引用,文件保留在 %TEMP% 由系统清理)
    let path = std::env::temp_dir().join(format!(
        "glaspen2_record_{}.gif",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    if std::fs::write(&path, gif).is_err() {
        return false;
    }
    let mut path_wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    path_wide.push(0); // 双 NUL 结尾

    // CF_HDROP 缓冲:DROPFILES 头(20B) + 宽字符路径列表
    let hdrop_bytes = 20 + path_wide.len() * 2;
    let mem_hdrop = GlobalAlloc(GMEM_MOVEABLE, hdrop_bytes);
    if mem_hdrop.0.is_null() {
        return false;
    }
    {
        let p = GlobalLock(mem_hdrop) as *mut u8;
        if p.is_null() {
            let _ = GlobalFree(mem_hdrop);
            return false;
        }
        // DROPFILES { pFiles=20, pt=(0,0), fNC=0, fWide=1 }
        std::ptr::write_bytes(p, 0, hdrop_bytes);
        (p as *mut u32).write_unaligned(20);
        (p.add(16) as *mut i32).write_unaligned(1); // fWide = TRUE
        std::ptr::copy_nonoverlapping(
            path_wide.as_ptr() as *const u8,
            p.add(20),
            path_wide.len() * 2,
        );
        let _ = GlobalUnlock(mem_hdrop);
    }

    // "GIF" 注册格式的原始字节
    let mem_gif = GlobalAlloc(GMEM_MOVEABLE, gif.len());
    if mem_gif.0.is_null() {
        let _ = GlobalFree(mem_hdrop);
        return false;
    }
    {
        let p = GlobalLock(mem_gif);
        if p.is_null() {
            let _ = GlobalFree(mem_gif);
            let _ = GlobalFree(mem_hdrop);
            return false;
        }
        std::ptr::copy_nonoverlapping(gif.as_ptr(), p as *mut u8, gif.len());
        let _ = GlobalUnlock(mem_gif);
    }

    const CF_HDROP: u32 = 15;
    if OpenClipboard(HWND::default()) == 0 {
        let _ = GlobalFree(mem_hdrop);
        let _ = GlobalFree(mem_gif);
        return false;
    }
    EmptyClipboard();
    let h1 = SetClipboardData(CF_HDROP, mem_hdrop);
    let h2 = SetClipboardData(cf_gif, mem_gif);
    CloseClipboard();
    if h1.0.is_null() {
        let _ = GlobalFree(mem_hdrop);
    }
    if h2.0.is_null() {
        let _ = GlobalFree(mem_gif);
    }
    !(h1.0.is_null() && h2.0.is_null())
}

/// 键盘 Raw Input:录制中检测 Ctrl+Alt+R 松开。
/// RegisterHotKey 只给按下不给抬起;键盘 Raw Input 与数位笔/鼠标
/// 同走 WM_INPUT 排队通道,不阻塞系统输入线程。
unsafe fn handle_keyboard_raw(buf: &[u64], state: &mut OverlayState) {
    let raw = buf.as_ptr() as *const RAWINPUT;
    if (*raw).header.dwType != RIM_TYPEKEYBOARD.0 {
        return;
    }
    let kb = &(*raw).data.keyboard;
    if state.gif_recording && kb.VKey == 0x52 && kb.Message == WM_KEYUP {
        // R 松开(无需再验修饰键:录制只在热键按下时开启)
        gif_record_stop(state);
        return;
    }
    // 手写消息:'2'/'3' 松开结束草稿/直发 —— 与 GIF 的 R 同机制,
    // 不验修饰键,防止先松 Ctrl/Alt 把会话卡死(macOS 同款容错)
    if kb.Message == WM_KEYUP {
        if kb.VKey == 0x32 && state.ink_draft_active {
            ink_draft_stop(state);
        } else if kb.VKey == 0x33 && state.msg_record_start >= 0 {
            chat_record_stop(state);
        }
    }
}

/// Ctrl+Alt+G:导出 SVG + GIF,并把当前画布复制到系统剪贴板(CF_DIB)
fn export_svg_gif_clipboard(state: &mut OverlayState) {
    glaspen_core::export::glaspen2_save_svg();
    // Default GIF quality/speed (fps, resolution, playback speed); the macOS
    // settings panel exposes these for the Cmd+Ctrl+R recording flow.
    let ok = glaspen_core::export::glaspen2_save_animated_gif(15, 0.5, 2.0, 1);
    eprintln!(
        "[overlay] SVG 已导出;GIF 导出: {}",
        if ok != 0 { "OK" } else { "FAILED" }
    );
    copy_canvas_to_clipboard(state);
    hud_notify("已导出 SVG + GIF,并复制到剪贴板");
}

/// 把当前画布(32bit BGRA 预乘)复制为 CF_DIB 到系统剪贴板
fn copy_canvas_to_clipboard(state: &mut OverlayState) {
    let w = state.canvas.w as usize;
    let h = state.canvas.h as usize;
    let row = (w * 4 + 3) & !3;
    let header = 40usize;
    let total = header + row * h;
    let mem = unsafe { GlobalAlloc(GMEM_MOVEABLE, total) };
    if mem.0.is_null() {
        return;
    }
    let ptr = unsafe { GlobalLock(mem) };
    if ptr.is_null() {
        unsafe {
            let _ = GlobalFree(mem);
        }
        return;
    }
    let snap = state.canvas.snapshot();
    unsafe {
        let p = ptr as *mut u8;
        // BITMAPINFOHEADER(40 字节,bottom-up 32bpp BGRA)
        std::slice::from_raw_parts_mut(p, 40).fill(0);
        std::slice::from_raw_parts_mut(p.add(0), 4).copy_from_slice(&40u32.to_le_bytes());
        std::slice::from_raw_parts_mut(p.add(4), 4).copy_from_slice(&(w as i32).to_le_bytes());
        std::slice::from_raw_parts_mut(p.add(8), 4).copy_from_slice(&(h as i32).to_le_bytes()); // 正高度 = bottom-up
        std::slice::from_raw_parts_mut(p.add(12), 2).copy_from_slice(&1u16.to_le_bytes()); // biPlanes
        std::slice::from_raw_parts_mut(p.add(14), 2).copy_from_slice(&32u16.to_le_bytes()); // biBitCount
        std::slice::from_raw_parts_mut(p.add(16), 4).copy_from_slice(&0u32.to_le_bytes()); // biCompression = BI_RGB
        // 像素:源为 top-down,写入 bottom-up(行反转)
        for y in 0..h {
            let src_off = y * w * 4;
            std::ptr::copy_nonoverlapping(
                snap.as_ptr().add(src_off),
                p.add(header + (h - 1 - y) * row),
                w * 4,
            );
        }
        let _ = GlobalUnlock(mem);
    }
    unsafe {
        if OpenClipboard(state.canvas.hwnd) != 0 {
            EmptyClipboard();
            let h = SetClipboardData(CF_DIB, mem);
            CloseClipboard();
            if h.0.is_null() {
                let _ = GlobalFree(mem);
            }
            eprintln!(
                "[overlay] 画布已复制到剪贴板 ({}x{})",
                state.canvas.w, state.canvas.h
            );
        } else {
            let _ = GlobalFree(mem);
            eprintln!("[overlay] 剪贴板打开失败,未复制");
        }
    }
}

/// 应用/取消模糊背景(磨砂玻璃)。SetWindowCompositionAttribute 是
/// undocumented API,user32 导入库中没有,故用 libloading 动态加载。
/// 注:对 UpdateLayeredWindow 窗口,Windows 可能忽略该效果(与 ULW 合成冲突),
/// 调用不失败即可。
fn apply_frosted(hwnd: HWND, on: bool) {
    type FnSetWca = unsafe extern "system" fn(HWND, *mut WindowCompositionAttributeData) -> i32;
    let lib = match unsafe { libloading::Library::new("user32.dll") } {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[overlay] user32.dll 加载失败: {}", e);
            return;
        }
    };
    let Ok(f) = (unsafe { lib.get::<FnSetWca>(b"SetWindowCompositionAttribute") }) else {
        eprintln!("[overlay] 系统不支持 SetWindowCompositionAttribute");
        return;
    };
    let f: FnSetWca = *f;
    let mut accent = AccentPolicy {
        accent_state: if on {
            ACCENT_ENABLE_ACRYLICBLURBEHIND
        } else {
            0
        },
        flags: 0,
        color: 0,
        animation_id: 0,
    };
    let mut data = WindowCompositionAttributeData {
        attribute: WCA_ACCENT_POLICY,
        data: (&mut accent as *mut AccentPolicy).cast(),
        size: std::mem::size_of::<AccentPolicy>(),
    };
    let ret = unsafe { f(hwnd, &mut data) };
    eprintln!(
        "[overlay] SetWindowCompositionAttribute(blur={}) -> {}",
        on, ret
    );
}

/// Ctrl+Alt+B:模糊背景(磨砂玻璃)开关
fn toggle_frosted(state: &mut OverlayState) {
    state.draw.frosted = !state.draw.frosted;
    glaspen_core::runtime().block_on(glaspen_core::db::save_setting(
        "frostedGlass",
        if state.draw.frosted { "1" } else { "0" },
    ));
    apply_frosted(state.canvas.hwnd, state.draw.frosted);
    eprintln!(
        "[overlay] 模糊背景: {}",
        if state.draw.frosted { "开" } else { "关" }
    );
    hud_notify(if state.draw.frosted {
        "模糊背景已开启"
    } else {
        "模糊背景已关闭"
    });
}

/// 显示分辨率/排列变化:重建画布并重绘已保存笔画
fn on_display_change(state: &mut OverlayState) {
    let hwnd = state.canvas.hwnd;
    let x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
    let w = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
    let h = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            w,
            h,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
    glaspen_core::export::glaspen2_on_display_change(w, h);
    let color = state.canvas.color;
    state.canvas = OverlayCanvas::create(hwnd);
    state.canvas.color = color;
    redraw_from_strokes(state);
}

fn draw_rainbow_indicator(state: &mut OverlayState) {
    for col in 0..14 {
        let h = col as f64 / 14.0;
        let (r, g, b) = hsv_to_rgb(h);
        let color = ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8);
        state
            .canvas
            .fill_rect(col as f32 * 2.0, 0.0, 2.0, 4.0, color);
    }
    state.canvas.present_all();
}

fn hsv_to_rgb(h: f64) -> (f64, f64, f64) {
    let i = (h * 6.0) as i32;
    let f = h * 6.0 - i as f64;
    let q = 1.0 - f;
    match i % 6 {
        0 => (1.0, f, 0.0),
        1 => (q, 1.0, 0.0),
        2 => (0.0, 1.0, f),
        3 => (0.0, q, 1.0),
        4 => (f, 0.0, 1.0),
        5 => (1.0, 0.0, q),
        _ => (0.0, 0.0, 0.0),
    }
}

// ── 保存导出 ──

fn save_drawing(state: &mut OverlayState) {
    let snap = state.canvas.snapshot();
    glaspen_core::export::glaspen2_save_drawing(
        snap.as_ptr(),
        state.canvas.w,
        state.canvas.h,
        state.canvas.w * 4,
    );
}

fn save_with_bg(state: &mut OverlayState) {
    unsafe {
        let screen_dc = GetDC(None);
        let bw = state.canvas.w;
        let bh = state.canvas.h;
        let bg_dc = CreateCompatibleDC(Some(screen_dc));
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: bw,
                biHeight: -bh,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bg_bits: *mut std::ffi::c_void = ptr::null_mut();
        let bg_bmp =
            CreateDIBSection(Some(bg_dc), &bmi, DIB_RGB_COLORS, &mut bg_bits, None, 0).unwrap();
        let old = SelectObject(bg_dc, bg_bmp.into());
        let _ = BitBlt(bg_dc, 0, 0, bw, bh, Some(screen_dc), 0, 0, SRCCOPY);
        let snap = state.canvas.snapshot();
        glaspen_core::export::glaspen2_save_with_background(
            snap.as_ptr(),
            state.canvas.w,
            state.canvas.h,
            state.canvas.w * 4,
            bg_bits as *const u8,
            bw,
            bh,
            bw * 4,
        );
        let _ = SelectObject(bg_dc, old);
        let _ = DeleteObject(bg_bmp.into());
        let _ = DeleteDC(bg_dc);
        let _ = ReleaseDC(None, screen_dc);
    }
}

// ── 颜色/线宽匹配(设置管道用;实现在 core presets,macOS 同一实现) ──

fn closest_color_index(r: f64, g: f64, b: f64) -> usize {
    glaspen_core::presets::nearest_color_index(r, g, b)
}

fn closest_width_index(w: f64) -> usize {
    glaspen_core::presets::nearest_width_index(w)
}

// ── Settings Pipe Server(Flutter UI) ──

const PIPE_ACCESS_DUPLEX: u32 = 0x00000003;
const PIPE_TYPE_BYTE: u32 = 0x00000000;
const PIPE_READMODE_BYTE: u32 = 0x00000000;
const PIPE_WAIT: u32 = 0x00000000;
const PIPE_UNLIMITED_INSTANCES: u32 = 255;
const BUFFER_SIZE: u32 = 4096;

unsafe extern "system" {
    fn CreateNamedPipeW(
        lp_name: PCWSTR,
        dw_open_mode: u32,
        dw_pipe_mode: u32,
        n_max_instances: u32,
        n_out_buffer_size: u32,
        n_in_buffer_size: u32,
        n_default_time_out: u32,
        lp_security_attributes: *const std::ffi::c_void,
    ) -> isize;

    fn ConnectNamedPipe(h_named_pipe: isize, lp_overlapped: *mut std::ffi::c_void) -> i32;

    fn DisconnectNamedPipe(h_named_pipe: isize) -> i32;
}

// ── 剪贴板(CF_DIB 复制画布) ──

const CF_DIB: u32 = 8;
const GMEM_MOVEABLE: u32 = 0x0002;

#[link(name = "user32")]
unsafe extern "system" {
    fn OpenClipboard(hwnd: HWND) -> i32;
    fn EmptyClipboard() -> i32;
    fn SetClipboardData(uformat: u32, hmem: HANDLE) -> HANDLE;
    fn CloseClipboard() -> i32;
    fn RegisterClipboardFormatA(lpsz_format: *const u8) -> u32;
    fn GlobalAlloc(uflags: u32, dw_bytes: usize) -> HANDLE;
    fn GlobalFree(hmem: HANDLE) -> HANDLE;
    fn GlobalLock(hmem: HANDLE) -> *mut std::ffi::c_void;
    fn GlobalUnlock(hmem: HANDLE) -> i32;
}

// ── 模糊背景(SetWindowCompositionAttribute,Win10 1809+) ──

/// WCA_ACCENT_POLICY
const WCA_ACCENT_POLICY: i32 = 19;
/// ACCENT_ENABLE_ACRYLICBLURBEHIND(亚克力,模糊较轻);3 = BLURBEHIND 全屏高斯模糊(过糊)
const ACCENT_ENABLE_ACRYLICBLURBEHIND: i32 = 4;

#[repr(C)]
struct AccentPolicy {
    accent_state: i32,
    flags: i32,
    color: u32,
    animation_id: i32,
}

#[repr(C)]
struct WindowCompositionAttributeData {
    attribute: i32,
    data: *mut std::ffi::c_void,
    size: usize,
}

