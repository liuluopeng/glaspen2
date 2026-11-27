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
            // 记录"上一档"给 color_swap(可配置动作);同色重复点不变更历史
            if idx != state.draw.selected_color {
                state.draw.prev_color = state.draw.selected_color;
            }
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
            // 面板色板同步(macOS sync_settings_panel: 热键换色后圆点跟随)
            push_settings_changed(&format!(
                "{{\"color\":{},\"width\":{}}}",
                state.draw.selected_color, state.draw.selected_width
            ));
        }
    } else if cmd >= CMD_SELECT_WIDTH && cmd < CMD_SELECT_WIDTH + WIDTH_PRESETS.len() {
        let idx = cmd - CMD_SELECT_WIDTH;
        if idx < WIDTH_PRESETS.len() {
            // 记录"上一档"给 width_swap(可配置动作)
            if idx != state.draw.selected_width {
                state.draw.prev_width = state.draw.selected_width;
            }
            state.draw.width_scale = WIDTH_PRESETS[idx];
            state.draw.selected_width = idx;
            glaspen_core::export::glaspen2_save_settings(
                state.draw.pen_r,
                state.draw.pen_g,
                state.draw.pen_b,
                state.draw.width_scale,
            );
            push_settings_changed(&format!(
                "{{\"color\":{},\"width\":{}}}",
                state.draw.selected_color, state.draw.selected_width
            ));
        }
    } else {
        match cmd {
            x if x == CMD_SAVE_WITH_BG => {
                save_with_bg(state);
                hud_notify(i18n::text(Lang::Zh, &Msg::ScreenshotWithBackgroundSaved));
            }
            x if x == CMD_SAVE_DRAWING => {
                save_drawing(state);
                hud_notify(i18n::text(Lang::Zh, &Msg::ScreenshotSaved));
            }
            x if x == CMD_SAVE_XOJ => {
                glaspen_core::export::glaspen2_save_xoj();
                hud_notify(i18n::text(Lang::Zh, &Msg::NotesSaved));
            }
            x if x == CMD_CLEAR_SCREEN => clear_screen(state),
            x if x == CMD_UNDO => undo_last_stroke(state),
            x if x == CMD_TOGGLE_OUTLINE => {
                let on = param_on(state.draw.outline_enabled);
                state.draw.outline_enabled = on;
                OUTLINE_ENABLED.store(on, std::sync::atomic::Ordering::Relaxed);
                // macOS apply_outline 同款: core 同步(导出/缩略图带描边) +
                // 落库(重启保留) + 立即重绘已有笔迹
                glaspen_core::export::glaspen2_set_stroke_outline(on as i32);
                glaspen_core::runtime().block_on(glaspen_core::db::save_setting(
                    "outline",
                    if on { "1" } else { "0" },
                ));
                redraw_from_strokes(state);
            }
            x if x == CMD_SET_GRID_FOLLOW => {
                // 网格跟随涂鸦: 运行态立即生效(飘渺隐藏时决定网格是否陪葬)
                let on = param_on(state.draw.grid_follow_strokes);
                state.draw.grid_follow_strokes = on;
                glaspen_core::runtime().block_on(glaspen_core::db::save_setting(
                    "gridFollowStrokes",
                    if on { "1" } else { "0" },
                ));
                if state.draw.ethereal && !state.strokes_visible {
                    redraw_from_strokes(state);
                }
            }
            x if x == CMD_SET_GLASS_FOLLOW => {
                // 玻璃跟随涂鸦: 立即按新规则重估玻璃可见性(macOS gl_glass_apply)
                let on = param_on(state.draw.glass_follow_strokes);
                state.draw.glass_follow_strokes = on;
                glaspen_core::runtime().block_on(glaspen_core::db::save_setting(
                    "glassFollowStrokes",
                    if on { "1" } else { "0" },
                ));
                let hidden = state.draw.ethereal && !state.strokes_visible;
                apply_frosted(state.canvas.hwnd, state.draw.frosted && !hidden);
            }
            x if x == CMD_SET_SOFT_SHADOW => {
                // 软阴影: 运行态立即生效 + core 同步(导出/缩略图) + 重绘
                let on = param_on(state.draw.soft_shadow);
                state.draw.soft_shadow = on;
                glaspen_core::export::glaspen2_set_soft_shadow(on as i32);
                glaspen_core::runtime().block_on(glaspen_core::db::save_setting(
                    "softShadow",
                    if on { "1" } else { "0" },
                ));
                redraw_from_strokes(state);
            }
            x if x == CMD_TOGGLE_GRID => {
                let on = param_on(state.draw.show_grid);
                state.draw.show_grid = on;
                glaspen_core::runtime()
                    .block_on(glaspen_core::db::save_setting("grid", if on { "1" } else { "0" }));
                redraw_from_strokes(state);
            }
            x if x == CMD_SET_GRID_DIVIDER => {
                // 分栏:0=无 1=左右两栏 2=上下两栏 3=九宫格 4=康奈尔(与 macOS 同键同钳制)
                let v = (param as i64).clamp(0, 4) as i32;
                state.draw.grid_divider = v;
                glaspen_core::runtime()
                    .block_on(glaspen_core::db::save_setting("grid_divider", &v.to_string()));
                redraw_from_strokes(state);
            }
            x if x == CMD_SET_GRID_SIZE => {
                // 网格尺寸:core presets 单源钳制(10..200),落库后重绘
                set_grid_size(param as f64);
                glaspen_core::runtime().block_on(glaspen_core::db::save_setting(
                    "grid_size",
                    &format!("{:.1}", grid_size()),
                ));
                redraw_from_strokes(state);
            }
            x if x == CMD_TOGGLE_MINIMAP => {
                // 页面缩略图条:面板开关,状态 + 落库 + 重绘(仅活页本模式绘制)
                let on = param_on(MINIMAP_ENABLED.load(std::sync::atomic::Ordering::SeqCst));
                MINIMAP_ENABLED.store(on, std::sync::atomic::Ordering::SeqCst);
                glaspen_core::runtime().block_on(glaspen_core::db::save_setting(
                    "minimap",
                    if on { "1" } else { "0" },
                ));
                redraw_from_strokes(state);
            }
            x if x == CMD_MINIMAP_REFRESH => {
                // 后台工人渲染完一张缩略图:重绘让新图上屏
                if MINIMAP_ENABLED.load(std::sync::atomic::Ordering::SeqCst) {
                    redraw_from_strokes(state);
                }
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
                    glaspen_core::export::glaspen2_share_ink_set_active(0);
                }
                // D8b: 总开关镜像喂给手写消息状态机。关闭时按 effects 收尾录制:
                // 直发静默丢弃(Discard), 草稿走完整 StopChatDraft(提交在写笔画 +
                // half-close + 按 axum 裁决提示, 修复旧 Windows 静默 stop 的不一致)。
                recording_chat_integration_changed(state, on);
                // Ctrl+Alt+2/3: 集成开才注册, 关时把组合直通前台
                set_chat_integration(state.canvas.hwnd, on);
                push_settings_changed(&format!("{{\"chatIntegration\":{on}}}"));
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
                // 模式切换时作废未到期的 peek 隐藏计时(macOS peek_cancel_timer),
                // 否则刚切完模式会被上一轮的 1s 计时器藏掉
                let _ = unsafe { KillTimer(Some(state.canvas.hwnd), TIMER_PEEK) };
                state.peek_active = false;
                let on = param_on(state.draw.ethereal);
                state.draw.ethereal = on;
                glaspen_core::runtime().block_on(glaspen_core::db::save_setting(
                    "ethereal",
                    if on { "1" } else { "0" },
                ));
                if on {
                    hide_strokes(state);
                    hud_notify(i18n::text(Lang::Zh, &Msg::EtherealCanvasMode));
                } else {
                    show_strokes(state);
                    hud_notify(i18n::text(Lang::Zh, &Msg::FixedCanvasMode));
                }
                push_settings_changed(&format!("{{\"ethereal\":{on}}}"));
            }
            x if x == CMD_TOGGLE_PRESSURE_MONITOR => {
                let on = param_on(state.draw.pressure_monitor);
                state.draw.pressure_monitor = on;
                glaspen_core::runtime().block_on(glaspen_core::db::save_setting(
                    "pressureMonitor",
                    if on { "1" } else { "0" },
                ));
                hud_toggle_pressure(on);
                push_settings_changed(&format!("{{\"pressureMonitor\":{on}}}"));
                hud_notify(if on {
                    "压力监控已开启"
                } else {
                    "压力监控已关闭"
                });
            }
            x if x == CMD_NAVIGATE_TO_PAGE => {
                // 内容 tab 点击页面:恢复该页笔迹继续绘画
                navigate_to_page_from_panel(state, param as i64);
            }
            x if x == CMD_REFRESH_PAGE => {
                // 面板改动了数据(圈选移动/粘贴/删除/删页): 重载当前页并重绘。
                // macOS 同款: 只有"当前页"的数据改动才需要重建玻璃。
                let sid = param as i64;
                if infinite_on() {
                    glaspen_core::export::glaspen2_load_infinite_strokes();
                    redraw_from_strokes(state);
                } else {
                    let cur = glaspen_core::export::glaspen2_get_current_screen_id();
                    if sid <= 0 || sid == cur {
                        let _ = glaspen_core::export::glaspen2_load_strokes_for_screen(cur);
                        redraw_from_strokes(state);
                    }
                }
            }
            x if x == CMD_PAGE_PREV => navigate_page(state, false),
            x if x == CMD_PAGE_NEXT => navigate_page(state, true),
            x if x == CMD_EXPORT_SVG_GIF => export_svg_gif_clipboard(state),
            x if x == CMD_TOGGLE_ENABLED => toggle_enabled(state),
            x if x == CMD_TOGGLE_INFINITE_CANVAS => {
                apply_infinite_canvas(state, param_on(infinite_on()), param == usize::MAX)
            }
            x if x == CMD_CANVAS_HOME => {
                // 回到原点 + 100%(设置面板「回到原点」);HOME 常量单源在 core
                set_camera(Camera::HOME);
                persist_camera();
                redraw_from_strokes(state);
            }
            x if x == CMD_CANVAS_CENTER => {
                // 居中内容包围盒(设置面板「居中内容」)
                let (mut bx, mut by, mut bx2, mut by2) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
                if glaspen_core::export::glaspen2_stroke_bbox(&mut bx, &mut by, &mut bx2, &mut by2) != 0 {
                    let mut cam = camera();
                    let used_zoom = cam.centered_on(
                        (bx, by, bx2, by2),
                        (state.canvas.w as f64, state.canvas.h as f64),
                    );
                    // Windows 旧行为: 顺带把公式实际用的 zoom 写回(越界 zoom 被
                    // "修"成 1.0);macOS 只改 pan —— 差异保留, 见 camera.rs 模块
                    // 文档差异 5。
                    cam.zoom = used_zoom;
                    set_camera(cam);
                    persist_camera();
                }
                redraw_from_strokes(state);
            }
            x if x == CMD_CANVAS_NEW => {
                // 自由涂鸦按 macOS 设计**只有一张画布, 不存在清空/新建操作**:
                // 收到该动作只收笔 + 提示, 绝不清空数据(以前这里会清空整片自由涂鸦,
                // 一次误点就丢内容)。活页本模式下仍按原语义清空当前页。
                finish_active_stroke(state);
                if infinite_on() {
                    hud_notify("自由涂鸦只有一张画布，不存在清空操作");
                } else {
                    clear_screen(state);
                }
            }
            x if x == CMD_OPEN_SETTINGS => crate::show_settings_window(),
            x if x == CMD_QUIT => unsafe {
                let _ = DestroyWindow(state.canvas.hwnd);
            },
            _ => {}
        }
    }
}

fn clear_screen(state: &mut OverlayState) {
    // macOS 清空前先 finish_active_stroke(提交 + 抬笔扇出), 无论哪个分支
    finish_active_stroke(state);
    // 无限画布不清空(内容多,误清损失大):新建走设置面板的「新建画布」
    if infinite_on() {
        hud_notify(i18n::text(Lang::Zh, &Msg::InfiniteCanvasKept));
        return;
    }
    state.pen_path.clear();
    state.reset_pen();
    state.canvas.clear();
    // 网格与分栏是两个独立开关, draw_grid 内部各画各层(无条件调用)
    state.canvas
        .draw_grid(state.draw.show_grid, state.draw.grid_divider);
    state.canvas.set_bg_alpha(BG_BLOCK);
    let created = glaspen_core::export::glaspen2_clear_strokes(state.canvas.w, state.canvas.h);
    if created == 0 {
        // 当前页从未涂鸦:不能连续创建空白画布(macOS 同款提示)
        hud_notify(i18n::text(Lang::Zh, &Msg::BlankCanvasGuard));
    } else {
        // macOS 成功后显示页信息("今天 第2页 第3/5页"), 不另发"已新建"
        let cur = glaspen_core::export::glaspen2_get_current_screen_id();
        hud_notify(&page_info_text(cur));
    }
}

fn toggle_enabled(state: &mut OverlayState) {
    state.draw.enabled = !state.draw.enabled;
    let hwnd = state.canvas.hwnd;
    if !state.draw.enabled {
        // macOS: 关闭涂鸦 = 临时隐藏整个覆盖层窗口(笔迹/网格/磨砂全不可见),
        // 干净地让位给其它绘图软件; 在写的笔画先落库, 镜头抓手中止。
        finish_active_stroke(state);
        cancel_camera_drag(state);
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
        if state.draw.pressure_monitor {
            hud_toggle_pressure(false);
        }
        set_input_blocking(hwnd, false, "draw-disabled");
    } else {
        // macOS: 重新启用后, 飘渺隐藏态维持"窗口不出场"(等笔靠近再露出);
        // 其余情况把窗口拉回来并重绘。
        if state.draw.ethereal && !state.strokes_visible {
            redraw_from_strokes(state);
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
        } else {
            state.strokes_visible = true;
            redraw_from_strokes(state);
            unsafe {
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            }
            if state.draw.pressure_monitor {
                hud_toggle_pressure(true);
            }
        }
    }
    // 停用涂鸦时把其余动作热键注销, 组合键透传给前台应用(macOS 同语义);
    // 只留 启停涂鸦/自由涂鸦/打开设置 三个。
    on_draw_enabled_changed(hwnd, state.draw.enabled);
    hud_notify(if state.draw.enabled {
        i18n::text(Lang::Zh, &Msg::DrawingEnabled)
    } else {
        i18n::text(Lang::Zh, &Msg::DrawingDisabled)
    });
}

fn undo_last_stroke(state: &mut OverlayState) {
    // macOS: 书写中按下只提示, 不动库
    if state.in_stroke() {
        hud_notify(i18n::text(Lang::Zh, &Msg::StrokeInProgress));
        return;
    }
    let remaining = glaspen_core::export::glaspen2_undo_last_stroke();
    if remaining < 0 {
        hud_notify(i18n::text(Lang::Zh, &Msg::NothingToUndo));
        return;
    }
    // 清空画布,从 STROKES 重绘全部剩余笔画
    redraw_from_strokes(state);
    hud_notify(i18n::text(Lang::Zh, &Msg::Undone));
}

/// 清空画布并从 STROKES 重绘全部笔画(撤销/翻页/网格开关后使用)
fn redraw_from_strokes(state: &mut OverlayState) {
    // 飘渺隐藏态: 只画网格/分栏/缩略图, 绝不画笔迹 —— 等价 macOS drawRect 的
    // `if (image && g_strokes_visible)` 门控。没有这道门控时, 改网格/撤销/
    // 切模式/镜头移动等"重建类操作"都会把整片笔迹重新露出来。
    if state.draw.ethereal && !state.strokes_visible {
        state.pen_path.clear();
        // 重建路径作废进行中的笔(会话阶段是唯一来源;旧实现只清 in_stroke
        // 标志、留下模型器状态,本笔后续报告会失效)。
        state.pen.reset();
        state.canvas.clear();
        if !state.draw.grid_follow_strokes {
            state.canvas
                .draw_grid(state.draw.show_grid, state.draw.grid_divider);
        }
        draw_minimap(state);
        state.canvas.set_bg_alpha(BG_BLOCK);
        state.canvas.present_all();
        return;
    }
    state.pen_path.clear();
    state.reset_pen();
    state.canvas.clear();
    // 网格与分栏是两个独立开关, draw_grid 内部各画各层(无条件调用)
    state.canvas
        .draw_grid(state.draw.show_grid, state.draw.grid_divider);
    // D9c:整页重建的 pass 顺序(逐笔三档阴影 → 描边 → 墨迹)/宽度/透明度/
    // 颜色全部由 core DrawPlan 决定(Windows 参数见 crate::paint_plan);
    // 本函数不再自己分层。计划在 STROKES 锁内生成、锁外执行 —— 落笔线程
    // 不用等一整轮 cairo 光栅化。
    let plan = {
        let strokes = glaspen_core::STROKES
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        paint_plan::rebuild_plan(
            &strokes,
            paint_plan::view(infinite_on(), camera()),
            state.canvas.w as f64,
            state.canvas.h as f64,
            state.draw.soft_shadow,
            state.draw.outline_enabled,
        )
    };
    match plan {
        Ok(plan) => state.canvas.execute_plan(&plan),
        Err(e) => tracing::warn!("整页重建计划生成失败: {e}"),
    }
    draw_minimap(state);
    state.canvas.set_bg_alpha(BG_BLOCK);
    state.canvas.present_all();
}

/// 末页"再按一次新建"的判定窗口(macOS perform_hotkey 0x12 用 5 秒)
const NO_NEXT_SECONDS: std::time::Duration = std::time::Duration::from_secs(5);

/// 上一页/下一页(热键专用,macOS perform_hotkey 同款语义):
/// - 无限画布: 只有一块画布, 只提示不切模式;
/// - 有目标页: 切页 + 飘渺 peek + 页信息 HUD;
/// - 末页再按: 5 秒内二次按 = 新建一页并翻过去。
fn navigate_page(state: &mut OverlayState, next: bool) {
    if infinite_on() {
        hud_notify(i18n::text(Lang::Zh, &Msg::SingleInfiniteCanvas));
        return;
    }
    finish_active_stroke(state);
    let target = if next {
        glaspen_core::export::glaspen2_next_screen_id()
    } else {
        glaspen_core::export::glaspen2_prev_screen_id()
    };
    if target > 0 {
        if switch_to_page(state, target) {
            peek_strokes(state, 1.0);
            hud_notify(&page_info_text(target));
        }
        return;
    }
    if !next {
        hud_notify(i18n::text(Lang::Zh, &Msg::NoPreviousPage));
        return;
    }
    // 没有下一页: 5 秒内再按一次 = 新建一页并翻过去(写完接着写)
    let now = Instant::now();
    let second = state
        .last_no_next
        .map(|t| now.duration_since(t) < NO_NEXT_SECONDS)
        .unwrap_or(false);
    state.last_no_next = Some(now);
    if !second {
        hud_notify(i18n::text(Lang::Zh, &Msg::NoNextPage));
        return;
    }
    // core 建页时会顺手把 current 切到新页; 翻页前的页必须先抓下来
    let before = glaspen_core::export::glaspen2_get_current_screen_id();
    let page_id = glaspen_core::export::glaspen2_create_next_page(before);
    if page_id > 0 && page_id != before {
        // 注意: core 已把 current 切到 page_id, 不能再走 switch_to_page
        // (target == current 会早退, 导致不载入/不重绘, 内存还是旧页)。
        // macOS 用 page_flip_swap_from(before) 显式 load 正是为此。
        let count = glaspen_core::export::glaspen2_load_strokes_for_screen(page_id);
        redraw_from_strokes(state);
        tracing::info!("已新建并切换到页面 {page_id} ({count} 笔)");
        peek_strokes(state, 1.0);
        // macOS 先报页信息再报"已新建了一页"(HUD 后一条覆盖前一条)
        hud_notify(&page_info_text(page_id));
        hud_notify(i18n::text(Lang::Zh, &Msg::PageCreated));
    } else if page_id == before {
        hud_notify(i18n::text(Lang::Zh, &Msg::PageAlreadyLatest));
    } else {
        hud_notify(i18n::text(Lang::Zh, &Msg::PageCreateFailed));
    }
}

/// 切到指定页并重绘(不含提示/peek, 调用方按场景决定)。
/// 无限画布下先切回活页本(否则会把页笔迹塞进无限画布内存,macOS 同款)。
/// 成功返回 true; target 非法/与当前页相同返回 false。
fn switch_to_page(state: &mut OverlayState, target: i64) -> bool {
    if infinite_on() {
        apply_infinite_canvas(state, false, false);
    }
    let current = glaspen_core::export::glaspen2_get_current_screen_id();
    if target <= 0 || target == current {
        tracing::info!("没有更多页面 (current={current}, target={target})");
        return false;
    }
    let count = glaspen_core::export::glaspen2_load_strokes_for_screen(target);
    redraw_from_strokes(state);
    tracing::info!("已切换到页面 {target} ({count} 笔)");
    true
}

/// 设置面板点页面跳转: 只切页, 不弹 HUD/peek
/// (macOS glaspen2_macos_navigate_to_page 就是这么做的)。
fn navigate_to_page_from_panel(state: &mut OverlayState, screen_id: i64) {
    if screen_id > 0 {
        // macOS glaspen2_macos_navigate_to_page 也会先把在写的笔收掉
        finish_active_stroke(state);
        let _ = switch_to_page(state, screen_id);
    }
}

/// 飘渺模式: 短暂显示笔迹 `seconds` 秒后自动隐回(macOS peek_strokes 同款)。
/// 每次调用重新计时; 笔在书写中不隐藏。
fn peek_strokes(state: &mut OverlayState, seconds: f64) {
    if !state.draw.ethereal {
        return; // 固定画布模式: 笔迹本来就一直可见
    }
    if !state.strokes_visible {
        show_strokes(state);
    }
    state.peek_active = true;
    let ms = (seconds.max(0.05) * 1000.0).round() as u32;
    let _ = unsafe { SetTimer(Some(state.canvas.hwnd), TIMER_PEEK, ms, None) };
}

/// 飘渺模式下镜头平移/缩放时短暂露出笔迹, 让新镜头可见(macOS peek_lens_change 同款)。
/// 笔正悬停(笔迹已可见且没有 peek 计时器)时不启动隐藏计时器, 否则悬浮看画会
/// 1 秒后被藏起来。
fn peek_lens_change(state: &mut OverlayState) {
    if !state.draw.ethereal || !infinite_on() {
        return;
    }
    if state.strokes_visible && !state.peek_active {
        return;
    }
    peek_strokes(state, 1.0);
}

/// 页数+日期通知文本,如 "今天 第2页  第3/5页"。
/// 文案/日期比较单源在 core::pageinfo(macOS 走同一实现), 这里只负责显示。
fn page_info_text(screen_id: i64) -> String {
    glaspen_core::pageinfo::page_info_text(screen_id, "zh")
        .unwrap_or_else(|| format!("第 {} 页", screen_id))
}

/// 飘渺模式:隐藏笔迹(保留数据,仅清空显示;网格按"网格跟随涂鸦"决定)
fn hide_strokes(state: &mut OverlayState) {
    // 提交未完成的笔画(含抬笔扇出,不能只 end_stroke)
    finish_active_stroke(state);
    state.strokes_visible = false;
    state.canvas.clear();
    // 网格跟随涂鸦 → 网格与分栏线一起隐藏;否则按各自开关保持可见
    if !state.draw.grid_follow_strokes {
        state.canvas
            .draw_grid(state.draw.show_grid, state.draw.grid_divider);
    }
    // 玻璃跟随涂鸦 → 磨砂背景一起关;否则保持
    if state.draw.glass_follow_strokes && state.draw.frosted {
        apply_frosted(state.canvas.hwnd, false);
    }
    // 压力监控跟随笔迹显隐(macOS ethereal_hide_now: pm_hide)
    if state.draw.pressure_monitor {
        hud_toggle_pressure(false);
    }
    state.canvas.set_bg_alpha(BG_BLOCK);
    state.canvas.present_all();
}

/// 飘渺模式:显示笔迹(从 STROKES 重绘)
fn show_strokes(state: &mut OverlayState) {
    if !state.strokes_visible {
        state.strokes_visible = true;
        redraw_from_strokes(state);
        // 重现时把跟随隐藏的磨砂背景按自身开关恢复
        if state.draw.glass_follow_strokes && state.draw.frosted {
            apply_frosted(state.canvas.hwnd, true);
        }
        // 压力监控跟着笔迹出现(macOS auto_show_canvas: pm_show)
        if state.draw.pressure_monitor {
            hud_toggle_pressure(true);
        }
        // 之前可能因"关闭涂鸦"隐藏了窗口: 这里把窗口拉回来
        unsafe {
            let _ = ShowWindow(state.canvas.hwnd, SW_SHOWNOACTIVATE);
        }
    }
}

// ── 无限画布(自由涂鸦):模式切换与镜头(macOS 同款行为) ──

/// 镜头持久化到 user_settings(仅无限模式有意义)。{:.6} 落库格式单源在
/// core camera::Camera::to_db_values(经 db::set_infinite_transform, D5b)。
fn persist_camera() {
    let cam = camera();
    glaspen_core::export::glaspen2_set_infinite_transform(cam.pan_x, cam.pan_y, cam.zoom);
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

/// 镜头平移(键盘镜头键/鼠标抓手共用; 方向由调用方按"直觉"取号)。书写中忽略
/// (模型器坐标系不能中途跳变)。数学(pan -= d)单源在 core Camera::pan_by。
fn canvas_pan_by(state: &mut OverlayState, dx: f64, dy: f64) {
    if state.in_stroke() {
        return;
    }
    let mut cam = camera();
    cam.pan_by(dx, dy);
    set_camera(cam);
    canvas_apply_transform(state);
    // 飘渺模式下笔迹是隐的: 短暂露出, 让镜头变化可见(macOS peek_lens_change)
    peek_lens_change(state);
}

/// 缩放结果统一处理(锚点缩放/中心缩放共用): 端点提示(仅 Windows 有下限提示)、
/// 无变化早退、写回 + 重绘/peek。
///
/// 提示/早退规则与旧内联实现逐行一致: 只看"钳制后是否真的变了";没变且本次
/// 撞到上/下限 → 1.5s 节流弹提示, 不重建/不 peek(否则到顶后每次滚轮都会全屏
/// 重绘, 飘渺模式下还会闪一下)。macOS 只在撞上限时提示(差异 2)。
fn apply_camera_zoom(state: &mut OverlayState, cam: Camera, out: ZoomOutcome) {
    if !out.changed() {
        if out.hit_max || out.hit_min {
            static LAST_HINT: std::sync::atomic::AtomicU64 =
                std::sync::atomic::AtomicU64::new(0);
            let now = now_millis();
            if now.saturating_sub(LAST_HINT.load(std::sync::atomic::Ordering::Relaxed))
                > ZOOM_HINT_MS
            {
                LAST_HINT.store(now, std::sync::atomic::Ordering::Relaxed);
                hud_notify(if out.hit_max {
                    i18n::text(Lang::Zh, &Msg::ZoomMax100)
                } else {
                    "已达最小缩放 5%"
                });
            }
        }
        return;
    }
    set_camera(cam);
    canvas_apply_transform(state);
    // 飘渺模式下笔迹是隐的: 短暂露出, 让缩放变化可见(macOS peek_lens_change)
    peek_lens_change(state);
}

/// 缩放镜头:以视图点 (vx, vy) 为锚,zoom ∈ (0.05, 1.0]。
/// 锚点的画布坐标在缩放前后保持同一屏幕位置(view = (canvas−pan)×zoom);
/// 数学(锚点不变/限幅/hit 标记)单源在 core Camera::zoom_at。
fn canvas_zoom_at(state: &mut OverlayState, factor: f64, vx: f64, vy: f64) {
    if state.in_stroke() {
        return;
    }
    let mut cam = camera();
    let out = cam.zoom_at(factor, vx, vy);
    apply_camera_zoom(state, cam, out);
}

/// 以视口中心为锚缩放(键盘镜头缩放两端同款; 数学单源 core zoom_at_center)。
fn canvas_zoom_at_center(state: &mut OverlayState, factor: f64) {
    if state.in_stroke() {
        return;
    }
    let mut cam = camera();
    let out = cam.zoom_at_center(factor, state.canvas.w as f64, state.canvas.h as f64);
    apply_camera_zoom(state, cam, out);
}

/// 应用无限画布开关(设置面板与热键共用的唯一入口)。
/// 两种模式各自独立存储:活页本用 screens/strokes,无限画布用全局唯一的
/// infinite_strokes。切换时冲刷当前笔画、切存储、载入对应画布的笔迹与镜头。
/// `notify`: 面板切换静默、热键/菜单切换才弹提示(macOS apply_infinite_canvas 同款)。
fn apply_infinite_canvas(state: &mut OverlayState, on: bool, notify: bool) {
    if infinite_on() == on {
        return;
    }
    // 模式切换: 结束任何鼠标抓手, 丢弃未应用的镜头拖动位移(macOS 同款)
    cancel_camera_drag(state);
    // 先把在写的笔画落库到"旧"存储; 必须走 finish_active_stroke 才会触发
    // 抬笔扇出(草稿/共享/OCR), 直接 end_stroke 会漏掉钩子(macOS 同款)
    finish_active_stroke(state);
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
        let (mut px, mut py, mut pz) = (0.0f64, 0.0f64, 1.0f64);
        glaspen_core::export::glaspen2_get_infinite_transform(&mut px, &mut py, &mut pz);
        // zoom 兜底已在 FFI 边界完成(from_db_values 语义, > 0 不钳上限), 直接写状态
        set_camera(Camera::new(px, py, pz));
    } else {
        // 回到活页本:载入当前页;没有页就建一页;镜头恒为原点 + 100%
        let cur = glaspen_core::export::glaspen2_get_current_screen_id();
        if cur > 0 {
            glaspen_core::export::glaspen2_load_strokes_for_screen(cur);
        } else {
            glaspen_core::export::glaspen2_clear_strokes(state.canvas.w, state.canvas.h);
        }
        set_camera(Camera::HOME);
    }
    // 载入什么就显示什么(与 macOS 一致):load_strokes_for_screen /
    // load_infinite_strokes 都是"清空再填充"的替换语义,内存与库天然一致,
    // undo/导出所见即所得。此前的"切换后清空内存副本"会把刚载入的内容
    // 抹成空白,看起来像切换失败,且让 undo/导出与页面数据脱节。
    redraw_from_strokes(state);
    // 镜头热键只在无限画布模式注册(翻页模式放行给系统, macOS 同语义)
    set_canvas_mode(state.canvas.hwnd, on);
    // 面板开关/圆点同步(macOS sync_settings_panel 的等价物)
    let show_free = glaspen_core::runtime()
        .block_on(glaspen_core::db::load_setting("show_free_canvas"))
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0)
        != 0;
    push_settings_changed(&format!(
        "{{\"infiniteCanvas\":{},\"showFreeCanvas\":{}}}",
        if on { "true" } else { "false" },
        if show_free { "true" } else { "false" }
    ));
    // macOS: 从面板切换是静默的(NO), 只有热键/菜单切换才提示
    if notify {
        hud_notify(if on {
            i18n::text(Lang::Zh, &Msg::InfiniteCanvasOn)
        } else {
            i18n::text(Lang::Zh, &Msg::InfiniteCanvasOff)
        });
    }
}

// ── 快捷录制 GIF(Ctrl+Alt+R 按住,松开生成并复制剪贴板;macOS ⌘⌃R 同款) ──

/// 提交在写的笔画并复位模型器(录制边界必须落在笔画边界上)
fn finish_active_stroke(state: &mut OverlayState) {
    if state.in_stroke() {
        glaspen_core::export::glaspen2_end_stroke();
        glaspen_core::export::glaspen2_decimate_last_stroke();
        // 抬笔扇出(草稿/共享/OCR):与 pen.rs 的正常抬笔路径一致
        glaspen_core::export::glaspen2_notify_stroke_committed();
        state.pen_path.clear();
        state.reset_pen();
    }
}

// (D8b) 录制会话的起止/决策/effect 分派已迁到 overlay/recording.rs:
// 热键/Raw Input 只喂 Start/Release 事件, GIF 编码/剪贴板/通道/直发/HUD 全部按
// core 状态机下发的 effects 顺序执行。本文件保留 finish_active_stroke(笔画
// 边界)与 copy_gif_bytes_to_clipboard(剪贴板实现)供其复用。

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
    let key_up = kb.Message == WM_KEYUP || kb.Message == WM_SYSKEYUP;
    let is_modifier = matches!(
        kb.VKey,
        0x11 | 0xA2 | 0xA3 // Ctrl: VK_CONTROL / VK_LCONTROL / VK_RCONTROL
        | 0x12 | 0xA4 | 0xA5 // Alt: VK_MENU / VK_LMENU / VK_RMENU
        | 0x10 | 0xA0 | 0xA1 // Shift: VK_SHIFT / VK_LSHIFT / VK_RSHIFT
        | 0x5B | 0x5C // Win
    );
    // 镜头修饰键按下: 提前清穿透把鼠标交给覆盖层, 这样 Ctrl+Alt+滚轮从第一格
    // 起就被吞掉(不会同时缩放下层浏览器), 左键拖动也不会先点到下层窗口。
    if !key_up
        && is_modifier
        && infinite_on()
        && state.draw.enabled
        && !state.in_stroke()
        && !state.camera_blocked
        && (mods_match_exact(camera_pan_mods()) || mods_match_exact(camera_zoom_mods()))
    {
        set_input_blocking(state.canvas.hwnd, true, "camera-mods");
        state.camera_blocked = true;
    }
    // 镜头修饰键释放 → 还原穿透(否则 Ctrl+Alt 按住时清掉的穿透会一直留着)
    if key_up && is_modifier {
        camera_mods_maybe_released(state);
    }
    // 录制会话的抬起交给 core 状态机(起止/窗口/效果顺序全在 core):
    // 不验修饰键, 防止先松 Ctrl/Alt 把会话卡死(macOS 同款容错), 但只在本
    // 槽位确实处于该模式时才消费 —— Raw Input 看得到所有按键, 不能把普通
    // r/2/3 抬键也吃掉。
    if kb.Message == WM_KEYUP {
        use glaspen_core::recording::RecordingMode;
        let gif_mode = state.gif_machine.state().mode();
        let chat_mode = state.chat_machine.state().mode();
        if kb.VKey == 0x52 && gif_mode == Some(RecordingMode::Gif) {
            recording_release(state, RecordingMode::Gif);
            return;
        }
        if kb.VKey == 0x32 && chat_mode == Some(RecordingMode::Draft) {
            recording_release(state, RecordingMode::Draft);
        } else if kb.VKey == 0x33 && chat_mode == Some(RecordingMode::Send) {
            recording_release(state, RecordingMode::Send);
        }
    }
}

/// Ctrl+Alt+G(可配置 export): 导出 SVG + 按内容裁切的动画 GIF 到桌面, 并把
/// 最新 GIF 以**文件**形式复制到剪贴板(macOS perform_hotkey(kVK_ANSI_G) 同款:
/// 写 SVG → 存裁切 GIF → pboard 写文件 URL → 成功/失败通知)。
fn export_svg_gif_clipboard(state: &mut OverlayState) {
    glaspen_core::export::glaspen2_save_svg();
    // 画布快照是预乘 BGRA(top-down, stride = w*4), 与 core 期望的
    // cairo ARGB32 小端布局一致; Windows 无 Retina 缩放(scale=1.0)。
    let snap = state.canvas.snapshot();
    let ok = glaspen_core::export::glaspen2_save_gif_cropped(
        snap.as_ptr(),
        state.canvas.w,
        state.canvas.h,
        state.canvas.w * 4,
        1.0,
    );
    if ok != 0 {
        if let Some(path) = newest_desktop_gif() {
            if let Ok(bytes) = std::fs::read(&path) {
                let _ = unsafe { copy_gif_bytes_to_clipboard(&bytes) };
                tracing::info!("已导出 SVG + GIF: {}", path.display());
            }
        }
        hud_notify(i18n::text(Lang::Zh, &Msg::SvgGifSaved));
    } else {
        tracing::warn!("GIF 导出失败(没有笔迹?)");
        hud_notify(i18n::text(Lang::Zh, &Msg::ExportFailed));
    }
}

/// 桌面上最新的一个 glaspen2_*.gif(刚导出写完的那份)
fn newest_desktop_gif() -> Option<std::path::PathBuf> {
    let dir = glaspen_core::desktop_path();
    let mut best: Option<(std::time::SystemTime, std::path::PathBuf)> = None;
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let lower = name.to_ascii_lowercase();
        if !lower.starts_with("glaspen2_") || !lower.ends_with(".gif") {
            continue;
        }
        let Ok(meta) = e.metadata() else { continue };
        let m = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
        if best.as_ref().map(|(t, _)| m > *t).unwrap_or(true) {
            best = Some((m, e.path()));
        }
    }
    best.map(|(_, p)| p)
}

/// 复制 SVG 到剪贴板(可配置动作 copy_svg,macOS perform_hotkey(kVK_ANSI_S) 同语义)。
/// macOS 放的是 `<img src="data:image/svg+xml;base64,…" />` 字符串(可直接粘进
/// 富文本/聊天框渲染), 不是裸 SVG 文本; 这里保持一致。
fn copy_svg_to_clipboard(state: &mut OverlayState) {
    let ptr = glaspen_core::export::glaspen2_get_cropped_svg();
    if ptr.is_null() {
        hud_notify(i18n::text(Lang::Zh, &Msg::NoStrokesToCopy));
        return;
    }
    let svg = unsafe { std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned() };
    glaspen_core::export::glaspen2_free_c_string(ptr);
    if svg.is_empty() {
        hud_notify(i18n::text(Lang::Zh, &Msg::NoStrokesToCopy));
        return;
    }
    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, svg.as_bytes());
    let html = format!("<img src=\"data:image/svg+xml;base64,{b64}\" />");
    // Windows 剪贴板文本必须是 UTF-16 + NUL 结尾
    let wide: Vec<u16> = html.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = wide.len() * std::mem::size_of::<u16>();
    unsafe {
        if OpenClipboard(state.canvas.hwnd) == 0 {
            tracing::error!("剪贴板打开失败,SVG 未复制");
            hud_notify("剪贴板打开失败");
            return;
        }
        EmptyClipboard();
        let mem = GlobalAlloc(GMEM_MOVEABLE, bytes);
        if !mem.0.is_null() {
            let p = GlobalLock(mem);
            if !p.is_null() {
                std::ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, p as *mut u8, bytes);
                let _ = GlobalUnlock(mem);
            }
            let h = SetClipboardData(CF_UNICODETEXT, mem);
            if h.0.is_null() {
                let _ = GlobalFree(mem);
            }
        }
        CloseClipboard();
    }
    tracing::info!("SVG(data-URI img) 已复制到剪贴板 ({} 字节)", svg.len());
    hud_notify(i18n::text(Lang::Zh, &Msg::SvgCopied));
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
            tracing::error!("user32.dll 加载失败: {}", e);
            return;
        }
    };
    let Ok(f) = (unsafe { lib.get::<FnSetWca>(b"SetWindowCompositionAttribute") }) else {
        tracing::info!("系统不支持 SetWindowCompositionAttribute");
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
    tracing::info!(
        "SetWindowCompositionAttribute(blur={}) -> {}",
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
    push_settings_changed(&format!(
        "{{\"frostedGlass\":{}}}",
        state.draw.frosted
    ));
    tracing::info!(
        "模糊背景: {}",
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
    // core 只切 current, STROKES 内存归壳层: 必须重载新页笔迹再重绘,
    // 否则旧页内容会被画到新页上(macOS display_change_apply 同款)
    let cur = glaspen_core::export::glaspen2_get_current_screen_id();
    if cur > 0 {
        glaspen_core::export::glaspen2_load_strokes_for_screen(cur);
        glaspen_core::export::glaspen2_smooth_loaded_strokes();
    }
    let color = state.canvas.color;
    state.canvas = OverlayCanvas::create(hwnd);
    state.canvas.color = color;
    redraw_from_strokes(state);
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

// ── 剪贴板(CF_UNICODETEXT 复制 SVG data-URI / GIF 文件) ──


const CF_UNICODETEXT: u32 = 13;
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

// ── 页面缩略图条(minimap,仅活页本模式;macOS 同键,库键 minimap)──
// 页窗(当前页前后各 5 页)与条带几何(卡片 y/高度/页号)的唯一来源 =
// glaspen_core::minimap(D7);本文件只保留缩略图渲染/缓存/单后台工人泵/
// 像素直写/样式(与 macOS 同策略)。两端差异见 core::minimap 模块文档:
// Windows 条带垂直居中(顶边 >= 4px)、不画页号文本(分层窗口画文本需另建
// GDI DC,页号看面板网格)。

struct MinimapThumb {
    /// 预乘 RGBA(与目标 BGRA 缓冲同布局交换通道即可)
    rgba: Vec<u8>,
    w: i32,
    h: i32,
}

static MINIMAP_THUMBS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<i64, MinimapThumb>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));
static MINIMAP_INFLIGHT: std::sync::Mutex<Vec<i64>> = std::sync::Mutex::new(Vec::new());
/// 页窗缓存(core::minimap::page_window 的结果;当前页变化时重算 —— 与 macOS
/// 的布局 JSON 缓存同策略, 不在重绘路径上反复查库)。
static MINIMAP_WINDOW: std::sync::Mutex<Option<glaspen_core::minimap::PageWindow>> =
    std::sync::Mutex::new(None);
static MINIMAP_WINDOW_FOR: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);
/// 单渲染工人的 CAS 门(同一时刻至多一个后台线程在逐张渲染)
static MINIMAP_PUMPING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// 页窗(当前页附近;过滤/排序/窗口公式唯一来源 = core::minimap::page_window)。
fn minimap_window(cur: i64) -> Option<glaspen_core::minimap::PageWindow> {
    if MINIMAP_WINDOW_FOR.load(std::sync::atomic::Ordering::SeqCst) == cur {
        let g = MINIMAP_WINDOW.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(window) = g.as_ref() {
            return Some(window.clone());
        }
    }
    let window = glaspen_core::minimap::page_window(cur, glaspen_core::minimap::PageFilter::All)?;
    *MINIMAP_WINDOW.lock().unwrap_or_else(|e| e.into_inner()) = Some(window.clone());
    MINIMAP_WINDOW_FOR.store(cur, std::sync::atomic::Ordering::SeqCst);
    Some(window)
}

/// 单后台工人:逐张渲染窗口里缺失的页,每张完成即投递刷新。
fn minimap_pump(ids: &[i64]) {
    use std::sync::atomic::Ordering;
    let want: Vec<i64> = ids
        .iter()
        .copied()
        .filter(|id| {
            !MINIMAP_THUMBS.lock().unwrap_or_else(|e| e.into_inner()).contains_key(id)
                && !MINIMAP_INFLIGHT.lock().unwrap_or_else(|e| e.into_inner()).contains(id)
        })
        .collect();
    if want.is_empty() {
        return;
    }
    if MINIMAP_PUMPING.swap(true, Ordering::SeqCst) {
        return; // 已有工人在跑:它会按最新列表继续,这里不重复起
    }
    let hwnd = *OVERLAY_HWND.lock().unwrap_or_else(|e| e.into_inner());
    std::thread::spawn(move || {
        let mut inflight = MINIMAP_INFLIGHT.lock().unwrap_or_else(|e| e.into_inner());
        inflight.extend_from_slice(&want);
        drop(inflight);
        for id in want {
            unsafe {
                let mut len: i32 = 0;
                let ptr =
                    glaspen_core::export::glaspen2_render_thumbnail(id, 0, 0, 128, &mut len);
                if !ptr.is_null() && len > 0 {
                    let png = std::slice::from_raw_parts(ptr, len as usize);
                    if let Ok(img) = image::load_from_memory(png) {
                        let rgba = img.to_rgba8();
                        let (w, h) = rgba.dimensions();
                        let mut buf = rgba.into_raw();
                        // 预乘 alpha(目标 BGRA 缓冲是预乘的)
                        for px in buf.chunks_exact_mut(4) {
                            let a = px[3] as u32;
                            px[0] = ((px[0] as u32 * a + 127) / 255) as u8;
                            px[1] = ((px[1] as u32 * a + 127) / 255) as u8;
                            px[2] = ((px[2] as u32 * a + 127) / 255) as u8;
                        }
                        MINIMAP_THUMBS.lock().unwrap_or_else(|e| e.into_inner()).insert(
                            id,
                            MinimapThumb {
                                rgba: buf,
                                w: w as i32,
                                h: h as i32,
                            },
                        );
                    }
                    glaspen_core::export::glaspen2_free_rust_bytes(ptr, len);
                }
                MINIMAP_INFLIGHT.lock().unwrap_or_else(|e| e.into_inner()).retain(|&x| x != id);
                let _ = PostMessageW(
                    Some(HWND(hwnd as *mut core::ffi::c_void)),
                    WM_TRAY_COMMAND,
                    WPARAM(CMD_MINIMAP_REFRESH),
                    LPARAM(0),
                );
            }
        }
        MINIMAP_PUMPING.store(false, std::sync::atomic::Ordering::SeqCst);
    });
}

/// 画右缘缩略图条(redraw_from_strokes 里、set_bg_alpha 之前调用:
/// 不透明卡片不会被背景 alpha 扫描改动)
fn draw_minimap(state: &mut OverlayState) {
    use std::sync::atomic::Ordering;
    if !MINIMAP_ENABLED.load(Ordering::SeqCst) || infinite_on() {
        return;
    }
    let cur = glaspen_core::export::glaspen2_get_current_screen_id();
    if cur <= 0 {
        return;
    }
    let Some(window) = minimap_window(cur) else {
        return;
    };

    let canvas = &mut state.canvas;
    let (w, h) = (canvas.w, canvas.h);
    // 页窗 + 条带几何唯一来源 = core::minimap(Windows 样式: 垂直居中)。
    let Ok(layout) = glaspen_core::minimap::layout(
        w as f64,
        h as f64,
        &window.ids,
        window.start,
        cur,
        &glaspen_core::minimap::WINDOWS_METRICS,
    ) else {
        return;
    };
    if layout.items.is_empty() {
        return;
    }
    let bits = canvas.bits;
    let accent = canvas.color; // 当前页描边用当前笔色
    let mw = layout.strip_w as i32;
    let tx = layout.strip_x as i32;
    unsafe {
        for item in &layout.items {
            let ty = item.y as i32;
            let mh = item.h as i32;
            if ty < 0 || ty + mh > h || tx < 0 {
                continue;
            }
            // 白底卡片(不透明)
            minimap_fill(bits, w, h, tx, ty, mw, mh, 252, 252, 252, 255);
            // 内容:等比缩放居中(最近邻,缩略图够清晰)
            let thumb = MINIMAP_THUMBS
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(&item.screen_id)
                .map(|t| (t.rgba.clone(), t.w, t.h));
            if let Some((src, sw, sh)) = thumb {
                let scale = (mw as f32 / sw as f32).min(mh as f32 / sh as f32);
                let dw = (sw as f32 * scale) as i32;
                let dh = (sh as f32 * scale) as i32;
                let ox = tx + (mw - dw) / 2;
                let oy = ty + (mh - dh) / 2;
                for dy in 0..dh {
                    let py = oy + dy;
                    if py < 0 || py >= h {
                        continue;
                    }
                    let sy = ((dy as f32 / scale) as i32).clamp(0, sh - 1);
                    for dx in 0..dw {
                        let px = ox + dx;
                        if px < 0 || px >= w {
                            continue;
                        }
                        let sx = ((dx as f32 / scale) as i32).clamp(0, sw - 1);
                        let so = ((sy * sw + sx) * 4) as usize;
                        let di = ((py as usize) * (w as usize) + px as usize) * 4;
                        *bits.add(di) = src[so + 2]; // B
                        *bits.add(di + 1) = src[so + 1]; // G
                        *bits.add(di + 2) = src[so]; // R
                        *bits.add(di + 3) = src[so + 3]; // A
                    }
                }
            }
            // 边框:当前页 2px 笔色,其余 1px 浅灰
            let (bc, bg_, bb, thick) = if item.current {
                (accent.0, accent.1, accent.2, 2)
            } else {
                (200u8, 200u8, 200u8, 1)
            };
            for t in 0..thick {
                // 上下
                minimap_fill(
                    bits,
                    w,
                    h,
                    tx - t,
                    ty - t,
                    mw + 2 * t + 1,
                    1,
                    bc,
                    bg_,
                    bb,
                    255,
                );
                minimap_fill(
                    bits,
                    w,
                    h,
                    tx - t,
                    ty + mh + t,
                    mw + 2 * t + 1,
                    1,
                    bc,
                    bg_,
                    bb,
                    255,
                );
                // 左右
                minimap_fill(bits, w, h, tx - t, ty, 1, mh, bc, bg_, bb, 255);
                minimap_fill(
                    bits,
                    w,
                    h,
                    tx + mw + t,
                    ty,
                    1,
                    mh,
                    bc,
                    bg_,
                    bb,
                    255,
                );
            }
        }
    }
    minimap_pump(&window.ids);
}

/// 矩形填色(预乘 BGRA,带边界裁剪)
unsafe fn minimap_fill(
    bits: *mut u8,
    w: i32,
    h: i32,
    x: i32,
    y: i32,
    rw: i32,
    rh: i32,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
) {
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + rw).min(w);
    let y1 = (y + rh).min(h);
    for py in y0..y1 {
        for px in x0..x1 {
            let i = ((py as usize) * (w as usize) + px as usize) * 4;
            *bits.add(i) = b;
            *bits.add(i + 1) = g;
            *bits.add(i + 2) = r;
            *bits.add(i + 3) = a;
        }
    }
}

