fn pipe_wide_name() -> Vec<u16> {
    OsStr::new(r"\\.\pipe\glaspen2_settings")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// 「立即更新」已下载(校验通过)的安装包;applyUpdate 优先用它,
/// 兜底 newest_installer(update_dir)。
static DOWNLOADED_INSTALLER: std::sync::Mutex<Option<std::path::PathBuf>> =
    std::sync::Mutex::new(None);

fn run_settings_pipe_server(hwnd: isize) {
    let name = pipe_wide_name();

    loop {
        let pipe = unsafe {
            CreateNamedPipeW(
                PCWSTR(name.as_ptr()),
                PIPE_ACCESS_DUPLEX,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                PIPE_UNLIMITED_INSTANCES,
                BUFFER_SIZE,
                BUFFER_SIZE,
                0,
                std::ptr::null(),
            )
        };
        if pipe == -1 || pipe == 0 {
            eprintln!("[pipe] CreateNamedPipeW failed");
            std::thread::sleep(std::time::Duration::from_secs(2));
            continue;
        }

        // Block until a client connects
        let ok = unsafe { ConnectNamedPipe(pipe, std::ptr::null_mut()) };
        if ok == 0 {
            let err = std::io::Error::last_os_error();
            // ERROR_PIPE_CONNECTED (535) means client connected before ConnectNamedPipe
            if err.raw_os_error() != Some(535) {
                eprintln!("[pipe] ConnectNamedPipe error: {}", err);
                close_pipe(pipe);
                continue;
            }
        }
        eprintln!("[pipe] Flutter settings client connected");

        handle_pipe_client(pipe, hwnd);

        eprintln!("[pipe] Flutter settings client disconnected");
    }
}

fn close_pipe(pipe: isize) {
    unsafe {
        DisconnectNamedPipe(pipe);
        let _ = windows::Win32::Foundation::CloseHandle(HANDLE(pipe as *mut _));
    }
}

fn handle_pipe_client(pipe: isize, hwnd: isize) {
    use std::io::Read;
    use std::os::windows::io::{FromRawHandle, IntoRawHandle};

    // Wrap pipe HANDLE in a single File for both read and write
    let mut stream = unsafe { std::fs::File::from_raw_handle(pipe as *mut std::ffi::c_void) };

    let mut buf = [0u8; 4096];
    let mut line_buf = Vec::new();

    loop {
        let n = match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                eprintln!("[pipe] client read error: {}", e);
                break;
            }
        };

        for &byte in &buf[..n] {
            if byte == b'\n' {
                if !line_buf.is_empty() {
                    let line = String::from_utf8_lossy(&line_buf).to_string();
                    process_pipe_message(&line, hwnd, &mut stream);
                    line_buf.clear();
                }
            } else {
                line_buf.push(byte);
            }
        }
    }

    // Prevent File from closing the handle; we close it ourselves
    let _ = stream.into_raw_handle();
    close_pipe(pipe);
}

/// 无限画布总览载荷(macOS canvas_overview_payload 同款):
/// 把当前画布内容按包围盒适配渲染成 PNG + 映射当前视口矩形,
/// 返回 data JSON 内容("png":"<b64>","rect":[x,y,w,h]);空画布返回空串。
/// 在管道线程调用:只读 STROKES / 镜头原子量,不动 UI 状态。
fn render_overview_json(ow: i32, oh: i32) -> String {
    let (mut bx, mut by, mut bx2, mut by2) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    if glaspen_core::export::glaspen2_stroke_bbox(&mut bx, &mut by, &mut bx2, &mut by2) == 0 {
        return String::new();
    }
    let (mut bw, mut bh) = (bx2 - bx, by2 - by);
    if bw < 1.0 {
        bw = 1.0;
    }
    if bh < 1.0 {
        bh = 1.0;
    }
    // 外扩 5%,笔迹不贴边
    let mx = bw * 0.05;
    let my = bh * 0.05;
    bx -= mx;
    by -= my;
    bw += mx * 2.0;
    bh += my * 2.0;

    let mut out_len: i32 = 0;
    let ptr = glaspen_core::export::glaspen2_render_canvas_overview(bx, by, bw, bh, ow, oh, &mut out_len);
    if ptr.is_null() || out_len <= 0 {
        return String::new();
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, out_len as usize) };
    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes);
    glaspen_core::export::glaspen2_free_rust_bytes(ptr, out_len);

    // 总览映射(scale/offset 必须与渲染一致),再映射当前视口矩形
    let ov_scale = ((ow as f64) / bw).min((oh as f64) / bh);
    let ov_ox = ((ow as f64) - bw * ov_scale) * 0.5;
    let ov_oy = ((oh as f64) - bh * ov_scale) * 0.5;
    let (px, py, z) = cam();
    let z = if z > ZOOM_MIN { z } else { 1.0 };
    let sw = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) } as f64;
    let sh = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) } as f64;
    let vr_w = sw / z * ov_scale;
    let vr_h = sh / z * ov_scale;
    let vx = ov_ox + (px - bx) * ov_scale;
    let vy = ov_oy + (py - by) * ov_scale;
    format!(
        "\"png\":\"{}\",\"rect\":[{},{},{},{}]",
        b64, vx, vy, vr_w, vr_h
    )
}

fn process_pipe_message(line: &str, hwnd: isize, writer: &mut std::fs::File) {
    use std::io::Write;

    let msg_type = json_get_str(line, "type");

    if msg_type == "listPages" {
        // 页面列表(内容 tab)
        let req_id = json_get_i64(line, "reqId").unwrap_or(0);
        let ptr = glaspen_core::export::glaspen2_list_screens_json();
        if ptr.is_null() {
            let _ = writer.write_all(
                format!(
                    "{{\"type\":\"listPages_response\",\"reqId\":{},\"data\":[]}}\n",
                    req_id
                )
                .as_bytes(),
            );
        } else {
            let s = unsafe { std::ffi::CStr::from_ptr(ptr) }
                .to_string_lossy()
                .to_string();
            glaspen_core::export::glaspen2_free_c_string(ptr);
            let resp = format!(
                "{{\"type\":\"listPages_response\",\"reqId\":{},\"data\":{}}}\n",
                req_id, s
            );
            let _ = writer.write_all(resp.as_bytes());
        }
        let _ = writer.flush();
    } else if msg_type == "getPageThumbnail" {
        // 页面缩略图(PNG,base64 编码)
        let screen_id = json_get_i64(line, "screenId").unwrap_or(0);
        let w = json_get_i64(line, "w").unwrap_or(0) as i32;
        let h = json_get_i64(line, "h").unwrap_or(0) as i32;
        let max_size = json_get_i64(line, "maxSize").unwrap_or(280) as i32;
        let req_id = json_get_i64(line, "reqId").unwrap_or(0);
        let mut out_len: i32 = 0;
        let ptr = glaspen_core::export::glaspen2_render_thumbnail(screen_id, w, h, max_size, &mut out_len);
        if ptr.is_null() || out_len <= 0 {
            let _ = writer.write_all(
                format!("{{\"type\":\"getPageThumbnail_response\",\"reqId\":{},\"data\":{{\"png\":\"\"}}}}\n", req_id).as_bytes(),
            );
        } else {
            let bytes = unsafe { std::slice::from_raw_parts(ptr, out_len as usize) };
            let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes);
            glaspen_core::export::glaspen2_free_rust_bytes(ptr, out_len);
            let resp = format!(
                "{{\"type\":\"getPageThumbnail_response\",\"reqId\":{},\"data\":{{\"png\":\"{}\"}}}}\n",
                req_id, b64
            );
            let _ = writer.write_all(resp.as_bytes());
        }
        let _ = writer.flush();
    } else if msg_type == "getPageThumbnails" {
        // 活页本整屏一次取图:避免每页一次管道往返 + 一次 setState。
        // 回传与 macOS 通道相同的自描述二进制块,这里按 base64 走 JSON。
        let ids = json_get_i64_array(line, "ids");
        let max_size = json_get_i64(line, "maxSize").unwrap_or(280) as i32;
        let req_id = json_get_i64(line, "reqId").unwrap_or(0);
        let blob = glaspen_core::export::page_thumbnails_blob(&ids, max_size);
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &blob);
        let _ = writer.write_all(
            format!(
                "{{\"type\":\"getPageThumbnails_response\",\"reqId\":{},\"data\":{{\"blob\":\"{}\"}}}}\n",
                req_id, b64
            )
            .as_bytes(),
        );
        let _ = writer.flush();
    } else if msg_type == "deletePage" {
        // 删除一页(内容 tab)。删掉当前页时 Rust 侧会清空内存笔迹,
        // 这里再切到相邻页并让覆盖层重载。
        let screen_id = json_get_i64(line, "screenId").unwrap_or(0);
        let req_id = json_get_i64(line, "reqId").unwrap_or(0);
        let ok = glaspen_core::export::glaspen2_delete_screen(screen_id);
        if ok != 0 {
            let mut next = glaspen_core::export::glaspen2_next_screen_id();
            if next == 0 {
                next = glaspen_core::export::glaspen2_prev_screen_id();
            }
            if next > 0 {
                let _ = unsafe {
                    PostMessageW(
                        Some(HWND(hwnd as *mut _)),
                        WM_TRAY_COMMAND,
                        WPARAM(CMD_NAVIGATE_TO_PAGE),
                        LPARAM(next as isize),
                    )
                };
            }
        }
        let _ = writer.write_all(
            format!(
                "{{\"type\":\"deletePage_response\",\"reqId\":{},\"data\":{{\"ok\":{}}}}}\n",
                req_id, ok
            )
            .as_bytes(),
        );
        let _ = writer.flush();
    } else if msg_type == "exportPdf" {
        // 导出全部页面为 PDF(纯 Rust,不需要覆盖层配合)
        let req_id = json_get_i64(line, "reqId").unwrap_or(0);
        let ok = glaspen_core::export::glaspen2_export_pdf();
        let _ = writer.write_all(
            format!(
                "{{\"type\":\"exportPdf_response\",\"reqId\":{},\"data\":{{\"ok\":{}}}}}\n",
                req_id, ok
            )
            .as_bytes(),
        );
        let _ = writer.flush();
    } else if msg_type == "hotkey" {
        // Flutter 快捷键按钮:转发为对应命令
        let key = json_get_str(line, "key");
        let cmd = match key {
            "Q" => Some(CMD_QUIT),
            "G" => Some(CMD_EXPORT_SVG_GIF),
            "`" => Some(CMD_PAGE_PREV),
            "1" => Some(CMD_PAGE_NEXT),
            "Z" => Some(CMD_UNDO),
            "X" => Some(CMD_TOGGLE_ETHEREAL),
            "C" => Some(CMD_CLEAR_SCREEN),
            "V" => Some(CMD_TOGGLE_ENABLED),
            "B" => Some(CMD_TOGGLE_FROSTED),
            _ => None,
        };
        if let Some(cmd) = cmd {
            // param = usize::MAX:handle_command 的开关类命令按"切换"处理
            // (与键盘 Ctrl+Alt+键 行为一致,而不是被 0 强制关闭)
            let _ = unsafe {
                PostMessageW(
                    Some(HWND(hwnd as *mut _)),
                    WM_TRAY_COMMAND,
                    WPARAM(cmd),
                    LPARAM(usize::MAX as isize),
                )
            };
        }
    } else if msg_type == "navigateToPage" {
        // 内容 tab 点击页面 → 恢复该页笔迹(指定页面跳转)
        let screen_id = json_get_i64(line, "screenId").unwrap_or(0);
        if screen_id > 0 {
            let _ = unsafe {
                PostMessageW(
                    Some(HWND(hwnd as *mut _)),
                    WM_TRAY_COMMAND,
                    WPARAM(CMD_NAVIGATE_TO_PAGE),
                    LPARAM(screen_id as isize),
                )
            };
        }
    } else if msg_type == "canvasOverview" || msg_type == "canvasNew" {
        // 无限画布总览 tab:镜头动作(home/center/new)+ 包围盒总览 PNG。
        // 动作发给消息循环线程应用;总览渲染在管道线程(只读 STROKES/镜头)。
        let req_id = json_get_i64(line, "reqId").unwrap_or(0);
        let cmd = if msg_type == "canvasNew" {
            Some((CMD_CANVAS_NEW, 0usize))
        } else if json_get_bool(line, "home") == Some(true) {
            Some((CMD_CANVAS_HOME, 0))
        } else if json_get_bool(line, "center") == Some(true) {
            Some((CMD_CANVAS_CENTER, 0))
        } else {
            None
        };
        if let Some((cmd, param)) = cmd {
            let _ = unsafe {
                PostMessageW(
                    Some(HWND(hwnd as *mut _)),
                    WM_TRAY_COMMAND,
                    WPARAM(cmd),
                    LPARAM(param as isize),
                )
            };
            // 等消息循环应用动作后再渲染总览
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
        let data = render_overview_json(1024, 768);
        let _ = writer.write_all(
            format!(
                "{{\"type\":\"canvasOverview_response\",\"reqId\":{},\"data\":{}}}\n",
                req_id, data
            )
            .as_bytes(),
        );
        let _ = writer.flush();
    } else if msg_type == "getSettings" {
        // Respond with current settings from DB
        let (r, g, b, w) = glaspen_core::runtime()
            .block_on(glaspen_core::db::load_settings())
            .unwrap_or((1.0, 0.0, 0.0, 1.0));
        let color = closest_color_index(r, g, b);
        let width = closest_width_index(w);
        let outline = if OUTLINE_ENABLED.load(std::sync::atomic::Ordering::Relaxed) {
            1
        } else {
            0
        };
        let grid = glaspen_core::runtime()
            .block_on(glaspen_core::db::load_setting("grid"))
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0);
        let frosted = glaspen_core::runtime()
            .block_on(glaspen_core::db::load_setting("frostedGlass"))
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0);
        let pressure_monitor = glaspen_core::runtime()
            .block_on(glaspen_core::db::load_setting("pressureMonitor"))
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0);
        let grid_follow = glaspen_core::runtime()
            .block_on(glaspen_core::db::load_setting("gridFollowStrokes"))
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0);
        let ethereal = glaspen_core::runtime()
            .block_on(glaspen_core::db::load_setting("ethereal"))
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0);
        let grid_divider = glaspen_core::runtime()
            .block_on(glaspen_core::db::load_setting("grid_divider"))
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0);
        let chat_integration = glaspen_core::runtime()
            .block_on(glaspen_core::db::load_setting("chat_integration"))
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0);
        let share_canvas = glaspen_core::runtime()
            .block_on(glaspen_core::db::load_setting("share_ink"))
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0);
        let chat_api_base = glaspen_core::runtime()
            .block_on(glaspen_core::db::load_setting("chat_api_base"))
            .unwrap_or_default();
        let chat_user = glaspen_core::runtime()
            .block_on(glaspen_core::db::load_setting("chat_user"))
            .unwrap_or_default();
        let infinite_canvas = if infinite_on() { 1 } else { 0 };
        let (gfps, gres, gspd, gem) = gif_settings();
        // 密码不回读(macOS 同款):面板只在输入时发送
        let resp = format!(
            "{{\"type\":\"getSettings_response\",\"data\":{{\"color\":{},\"width\":{},\"outline\":{},\"grid\":{},\"gridDivider\":{},\"gridFollowStrokes\":{},\"frostedGlass\":{},\"pressureMonitor\":{},\"ethereal\":{},\"infiniteCanvas\":{},\"gifFps\":{},\"gifResolution\":{:.2},\"gifSpeed\":{:.2},\"gifEndMode\":{},\"rainbow\":false,\"launchAtLogin\":false,\"chatIntegration\":{},\"shareCanvas\":{},\"chatApiBase\":\"{}\",\"chatUser\":\"{}\"}}}}\n",
            color,
            width,
            outline,
            grid,
            grid_divider.clamp(0, 3),
            grid_follow,
            frosted,
            pressure_monitor,
            ethereal,
            infinite_canvas,
            gfps,
            gres,
            gspd,
            gem,
            chat_integration,
            share_canvas,
            json_escape(&chat_api_base),
            json_escape(&chat_user),
        );
        let _ = writer.write_all(resp.as_bytes());
        let _ = writer.flush();
    } else if msg_type == "setSetting" {
        let key = json_get_str(line, "key");
        if key == "save_drawing" {
            let _ = unsafe {
                PostMessageW(
                    Some(HWND(hwnd as *mut _)),
                    WM_TRAY_COMMAND,
                    WPARAM(CMD_SAVE_DRAWING),
                    LPARAM(0),
                )
            };
        } else if key == "save_with_bg" {
            let _ = unsafe {
                PostMessageW(
                    Some(HWND(hwnd as *mut _)),
                    WM_TRAY_COMMAND,
                    WPARAM(CMD_SAVE_WITH_BG),
                    LPARAM(0),
                )
            };
        } else if key == "save_xoj" {
            let _ = unsafe {
                PostMessageW(
                    Some(HWND(hwnd as *mut _)),
                    WM_TRAY_COMMAND,
                    WPARAM(CMD_SAVE_XOJ),
                    LPARAM(0),
                )
            };
        } else if key == "undo" {
            let _ = unsafe {
                PostMessageW(
                    Some(HWND(hwnd as *mut _)),
                    WM_TRAY_COMMAND,
                    WPARAM(CMD_UNDO),
                    LPARAM(0),
                )
            };
        } else if key == "export_animated_gif" {
            let result = glaspen_core::export::glaspen2_save_animated_gif(15, 0.5, 2.0, 1);
            eprintln!(
                "[pipe] animated GIF export: {}",
                if result != 0 { "OK" } else { "FAILED" }
            );
            hud_notify(if result != 0 {
                "动画 GIF 已保存到桌面"
            } else {
                "动画 GIF 导出失败"
            });
        } else if key == "export_pdf" {
            let result = glaspen_core::export::glaspen2_export_pdf();
            eprintln!(
                "[pipe] PDF export: {}",
                if result != 0 { "OK" } else { "FAILED" }
            );
            hud_notify(if result != 0 {
                "PDF 已保存到桌面"
            } else {
                "PDF 导出失败"
            });
        } else if key == "color" {
            if let Some(val) = json_get_i64(line, "value") {
                let idx = val as usize;
                let cmd = CMD_SELECT_COLOR + idx;
                let _ = unsafe {
                    PostMessageW(
                        Some(HWND(hwnd as *mut _)),
                        WM_TRAY_COMMAND,
                        WPARAM(cmd),
                        LPARAM(0),
                    )
                };
            }
        } else if key == "width" {
            if let Some(val) = json_get_i64(line, "value") {
                let idx = val as usize;
                let cmd = CMD_SELECT_WIDTH + idx;
                let _ = unsafe {
                    PostMessageW(
                        Some(HWND(hwnd as *mut _)),
                        WM_TRAY_COMMAND,
                        WPARAM(cmd),
                        LPARAM(0),
                    )
                };
            }
        } else if key == "outline" {
            if let Some(on) = json_get_bool(line, "value") {
                let cmd = CMD_TOGGLE_OUTLINE;
                let _ = unsafe {
                    PostMessageW(
                        Some(HWND(hwnd as *mut _)),
                        WM_TRAY_COMMAND,
                        WPARAM(cmd),
                        LPARAM(if on { 1 } else { 0 }),
                    )
                };
            }
        } else if key == "grid" {
            if let Some(on) = json_get_bool(line, "value") {
                let cmd = CMD_TOGGLE_GRID;
                let _ = unsafe {
                    PostMessageW(
                        Some(HWND(hwnd as *mut _)),
                        WM_TRAY_COMMAND,
                        WPARAM(cmd),
                        LPARAM(if on { 1 } else { 0 }),
                    )
                };
            }
        } else if key == "gridDivider" {
            // 分栏:0..3,经消息循环改状态 + 落库 + 重绘(与 macOS 同键)
            if let Some(v) = json_get_i64(line, "value") {
                let _ = unsafe {
                    PostMessageW(
                        Some(HWND(hwnd as *mut _)),
                        WM_TRAY_COMMAND,
                        WPARAM(CMD_SET_GRID_DIVIDER),
                        LPARAM(v.clamp(0, 3) as isize),
                    )
                };
            }
        } else if key == "chatIntegration" {
            // 手写消息集成总开关:经消息循环改状态(终止进行中会话/恢复共享上行)
            if let Some(on) = json_get_bool(line, "value") {
                let _ = unsafe {
                    PostMessageW(
                        Some(HWND(hwnd as *mut _)),
                        WM_TRAY_COMMAND,
                        WPARAM(CMD_TOGGLE_CHAT_INTEGRATION),
                        LPARAM(if on { 1 } else { 0 }),
                    )
                };
            }
        } else if key == "shareCanvas" {
            // 共享画布上行开关:落库 + 仅在集成开启时生效(与 macOS 同规则)
            if let Some(on) = json_get_bool(line, "value") {
                glaspen_core::runtime().block_on(glaspen_core::db::save_setting(
                    "share_ink",
                    if on { "1" } else { "0" },
                ));
                let integration_on = glaspen_core::runtime()
                    .block_on(glaspen_core::db::load_setting("chat_integration"))
                    .and_then(|v| v.parse::<i32>().ok())
                    .unwrap_or(0)
                    != 0;
                if integration_on {
                    glaspen_core::export::glaspen2_share_ink_set_active(if on { 1 } else { 0 });
                }
            }
        } else if key == "chatApiBase" || key == "chatUser" || key == "chatPassword" {
            // 涂鸦身份配置:落库(键名与 macOS 一致)+ 重载 auth 缓存
            let value = json_get_str(line, "value").to_string();
            let db_key = match key {
                "chatApiBase" => "chat_api_base",
                "chatUser" => "chat_user",
                _ => "chat_password",
            };
            glaspen_core::runtime().block_on(glaspen_core::db::save_setting(db_key, &value));
            glaspen_core::export::glaspen2_chat_auth_reload();
        } else if key == "frostedGlass" {
            if let Some(on) = json_get_bool(line, "value") {
                let cmd = CMD_TOGGLE_FROSTED;
                let _ = unsafe {
                    PostMessageW(
                        Some(HWND(hwnd as *mut _)),
                        WM_TRAY_COMMAND,
                        WPARAM(cmd),
                        LPARAM(if on { 1 } else { 0 }),
                    )
                };
            }
        } else if key == "gridFollowStrokes" {
            if let Some(on) = json_get_bool(line, "value") {
                glaspen_core::runtime().block_on(glaspen_core::db::save_setting(
                    "gridFollowStrokes",
                    if on { "1" } else { "0" },
                ));
            }
        } else if key == "pressureMonitor" {
            if let Some(on) = json_get_bool(line, "value") {
                let cmd = CMD_TOGGLE_PRESSURE_MONITOR;
                let _ = unsafe {
                    PostMessageW(
                        Some(HWND(hwnd as *mut _)),
                        WM_TRAY_COMMAND,
                        WPARAM(cmd),
                        LPARAM(if on { 1 } else { 0 }),
                    )
                };
            }
        } else if key == "infiniteCanvas" {
            // 模式 tab 切换:活页本 ↔ 无限画布(参数 0/1 显式设置)
            if let Some(on) = json_get_bool(line, "value") {
                let _ = unsafe {
                    PostMessageW(
                        Some(HWND(hwnd as *mut _)),
                        WM_TRAY_COMMAND,
                        WPARAM(CMD_TOGGLE_INFINITE_CANVAS),
                        LPARAM(if on { 1 } else { 0 }),
                    )
                };
            }
        } else if key == "gifFps" {
            if let Some(v) = json_get_i64(line, "value") {
                let (_, res, spd, em) = gif_settings();
                persist_gif_settings(v as i32, res, spd, em);
            }
        } else if key == "gifResolution" {
            if let Some(v) = json_get_f64(line, "value") {
                let (fps, _, spd, em) = gif_settings();
                persist_gif_settings(fps, v, spd, em);
            }
        } else if key == "gifSpeed" {
            if let Some(v) = json_get_f64(line, "value") {
                let (fps, res, _, em) = gif_settings();
                persist_gif_settings(fps, res, v, em);
            }
        } else if key == "gifEndMode" {
            if let Some(v) = json_get_i64(line, "value") {
                let (fps, res, spd, _) = gif_settings();
                persist_gif_settings(fps, res, spd, v as i32);
            }
        }
    } else if msg_type == "appVersion" {
        // 「关于」区显示的当前版本(与发布物一致,取自 Cargo.toml)
        let req_id = json_get_i64(line, "reqId").unwrap_or(0);
        let data = serde_json::json!({ "version": glaspen_core::update::current_version() });
        let _ = writer.write_all(
            format!("{{\"type\":\"appVersion_response\",\"reqId\":{req_id},\"data\":{data}}}\n")
                .as_bytes(),
        );
        let _ = writer.flush();
    } else if msg_type == "checkUpdate" {
        // 手动「检查更新」:阻塞的网络调用就跑在管道线程上(面板侧 15s 超时,
        // Rust 侧 10s 超时)。期间其它面板请求会排队 —— 按钮是手动触发的,可接受。
        let req_id = json_get_i64(line, "reqId").unwrap_or(0);
        let current = glaspen_core::update::current_version().to_string();
        let data = match glaspen_core::update::fetch_latest() {
            Ok(r) => {
                let has = glaspen_core::update::is_newer(&r.tag, &current);
                serde_json::json!({
                    "ok": true,
                    "current": current,
                    "latest": r.tag,
                    "hasUpdate": has,
                    "url": r.url,
                    "notes": r.notes,
                    "error": "",
                })
            }
            Err(e) => serde_json::json!({
                "ok": false,
                "current": current,
                "latest": "",
                "hasUpdate": false,
                "url": "",
                "error": e,
            }),
        };
        let _ = writer.write_all(
            format!("{{\"type\":\"checkUpdate_response\",\"reqId\":{req_id},\"data\":{data}}}\n")
                .as_bytes(),
        );
        let _ = writer.flush();
    } else if msg_type == "openUrl" {
        // 「打开下载页」:http/https 白名单在 open_url_checked 里, 缺 url 会被拒绝
        open_url_checked(json_get_str(line, "url"));
    } else if msg_type == "testChatLogin" {
        // 涂鸦身份「测试登录」:面板已先经 setSetting 保存最新配置,
        // 这里强制登录一次(阻塞网络调用跑在管道线程,与 checkUpdate 同理)
        let req_id = json_get_i64(line, "reqId").unwrap_or(0);
        let (ok, message) = match glaspen_core::export::chat_auth_test_login_blocking() {
            Ok(()) => (1, String::new()),
            Err(e) => (0, e),
        };
        let _ = writer.write_all(
            format!(
                "{{\"type\":\"testChatLogin_response\",\"reqId\":{req_id},\"data\":{{\"ok\":{ok},\"message\":\"{}\"}}}}\n",
                json_escape(&message)
            )
            .as_bytes(),
        );
        let _ = writer.flush();
    } else if msg_type == "backupNow" {
        // 一键全量备份到桌面(库在覆盖层进程里,必须在这边执行 —— macOS 走 FRB 同进程)
        let req_id = json_get_i64(line, "reqId").unwrap_or(0);
        let (ok, message) = match glaspen_core::runtime().block_on(glaspen_core::db::backup_now())
        {
            Ok(path) => (1, path),
            Err(e) => (0, e),
        };
        let _ = writer.write_all(
            format!(
                "{{\"type\":\"backupNow_response\",\"reqId\":{req_id},\"data\":{{\"ok\":{ok},\"message\":\"{}\"}}}}\n",
                json_escape(&message)
            )
            .as_bytes(),
        );
        let _ = writer.flush();
    } else if msg_type == "restoreLatestBackup" {
        // 从桌面最新备份合并恢复(不删新增页,同名 id 以备份为准)
        let req_id = json_get_i64(line, "reqId").unwrap_or(0);
        let (ok, message) = match glaspen_core::runtime()
            .block_on(glaspen_core::db::restore_latest_backup())
        {
            Ok((path, pages)) => (1, format!("{path}(当前共 {pages} 页)")),
            Err(e) => (0, e),
        };
        let _ = writer.write_all(
            format!(
                "{{\"type\":\"restoreLatestBackup_response\",\"reqId\":{req_id},\"data\":{{\"ok\":{ok},\"message\":\"{}\"}}}}\n",
                json_escape(&message)
            )
            .as_bytes(),
        );
        let _ = writer.flush();
    } else if msg_type == "downloadUpdate" {
        // 「立即更新」下载:fetch → 挑当前平台安装包 → 流式下载。
        // 进度帧(~100ms 一帧)直接写回管道;取消 = 面板发 cancelDownload,
        // on_progress 里偷看管道输入(单写者:此期间没有其它帧会发出)。
        let req_id = json_get_i64(line, "reqId").unwrap_or(0);
        use std::io::{Read as _, Write as _};
        use std::os::windows::io::AsRawHandle;
        let handle = HANDLE(writer.as_raw_handle());
        let mut frame_at = std::time::Instant::now() - std::time::Duration::from_millis(200);
        let result = glaspen_core::update::download_to_cache(|received, total| {
            if total == 0 || received == total || frame_at.elapsed() >= std::time::Duration::from_millis(100) {
                frame_at = std::time::Instant::now();
                let _ = writer.write_all(
                    format!(
                        "{{\"type\":\"downloadUpdate_frame\",\"reqId\":{req_id},\"data\":{{\"received\":{received},\"total\":{total},\"done\":false,\"error\":\"\",\"path\":\"\"}}}}\n",
                    )
                    .as_bytes(),
                );
                let _ = writer.flush();
            }
            // 偷看取消请求:有 cancelDownload 就停(消费掉这行,core 会删 .part)
            let mut avail: u32 = 0;
            if unsafe {
                PeekNamedPipe(handle, None, 0, None, Some(&mut avail), None)
            }
            .is_ok()
                && avail > 0
            {
                let mut buf = vec![0u8; avail as usize];
                let mut read: u32 = 0;
                if unsafe { ReadFile(handle, Some(&mut buf), Some(&mut read), None) }.is_ok() {
                    let s = String::from_utf8_lossy(&buf[..read as usize]);
                    if s.contains("cancelDownload") {
                        return false;
                    }
                }
            }
            true
        });
        match result {
            Ok((path, received, total)) => {
                eprintln!("[pipe] 更新包已下载: {}", path.display());
                *DOWNLOADED_INSTALLER.lock().unwrap() = Some(path.clone());
                let p = path.to_string_lossy().replace('\\', "/");
                let _ = writer.write_all(
                    format!(
                        "{{\"type\":\"downloadUpdate_frame\",\"reqId\":{req_id},\"data\":{{\"received\":{received},\"total\":{total},\"done\":true,\"error\":\"\",\"path\":\"{p}\"}}}}\n",
                    )
                    .as_bytes(),
                );
            }
            Err(e) => {
                // Cancelled 的 Display 就是「已取消」,Failed 直接带原因
                let _ = writer.write_all(
                    format!(
                        "{{\"type\":\"downloadUpdate_frame\",\"reqId\":{req_id},\"data\":{{\"received\":0,\"total\":0,\"done\":true,\"error\":\"{}\",\"path\":\"\"}}}}\n",
                        json_escape(&e.to_string())
                    )
                    .as_bytes(),
                );
            }
        }
        let _ = writer.flush();
    } else if msg_type == "stageUpdate" {
        // Windows 无 DMG 解包:安装包下载(校验)完即就绪
        let req_id = json_get_i64(line, "reqId").unwrap_or(0);
        let _ = writer.write_all(
            format!(
                "{{\"type\":\"stageUpdate_response\",\"reqId\":{req_id},\"data\":{{\"ok\":1,\"message\":\"安装包已就绪\"}}}}\n"
            )
            .as_bytes(),
        );
        let _ = writer.flush();
    } else if msg_type == "applyUpdate" {
        // 拉起安装包并退出本程序:安装器解压到 %LOCALAPPDATA%\glaspen2 并
        // 自动启动新版。先记下路径 + 回包,再走正常退出(管道断开时设置
        // 面板也会被带走);win_main 在进程收尾时用 cmd 延迟 2 秒启动安装器,
        // 确保文件锁全部释放。
        let req_id = json_get_i64(line, "reqId").unwrap_or(0);
        let installer = DOWNLOADED_INSTALLER
            .lock()
            .unwrap()
            .clone()
            .or_else(|| glaspen_core::update::newest_installer(&glaspen_core::update::update_dir()));
        let Some(installer) = installer else {
            let _ = writer.write_all(
                format!(
                    "{{\"type\":\"applyUpdate_response\",\"reqId\":{req_id},\"data\":{{\"ok\":0,\"message\":\"找不到已下载的安装包,请重新下载\"}}}}\n"
                )
                .as_bytes(),
            );
            let _ = writer.flush();
            return;
        };
        crate::set_pending_installer(installer);
        eprintln!("[pipe] 退出并启动更新安装器");
        let _ = writer.write_all(
            format!(
                "{{\"type\":\"applyUpdate_response\",\"reqId\":{req_id},\"data\":{{\"ok\":1,\"message\":\"\"}}}}\n"
            )
            .as_bytes(),
        );
        let _ = writer.flush();
        let _ = unsafe {
            PostMessageW(
                Some(HWND(hwnd as *mut _)),
                WM_TRAY_COMMAND,
                WPARAM(CMD_QUIT),
                LPARAM(0),
            )
        };
    }
}

// ── Minimal JSON helpers ──

/// JSON 字符串转义(getSettings 回读 chatApiBase/chatUser 等用户输入)
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn json_get_str<'a>(json: &'a str, key: &str) -> &'a str {
    let pattern = format!("\"{}\":\"", key);
    if let Some(start) = json.find(&pattern) {
        let val_start = start + pattern.len();
        if let Some(end) = json[val_start..].find('"') {
            return &json[val_start..val_start + end];
        }
    }
    ""
}

fn json_get_i64(json: &str, key: &str) -> Option<i64> {
    let pattern = format!("\"{}\":", key);
    if let Some(start) = json.find(&pattern) {
        let val_start = start + pattern.len();
        let rest = &json[val_start..].trim_start();
        let end = rest
            .find(|c: char| !c.is_ascii_digit() && c != '-')
            .unwrap_or(rest.len());
        if end > 0 {
            return rest[..end].parse::<i64>().ok();
        }
    }
    None
}

/// 解析 JSON 整数数组(如 ids);缺失或类型不符时返回空数组。
fn json_get_i64_array(json: &str, key: &str) -> Vec<i64> {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| v.get(key)?.as_array().cloned())
        .map(|a| a.iter().filter_map(|x| x.as_i64()).collect())
        .unwrap_or_default()
}

/// 解析 JSON 数值(整数或小数;gifResolution/gifSpeed 用)
fn json_get_f64(json: &str, key: &str) -> Option<f64> {
    let pattern = format!("\"{}\":", key);
    let start = json.find(&pattern)?;
    let rest = json[start + pattern.len()..].trim_start();
    let end = rest
        .find(|c: char| !c.is_ascii_digit() && c != '-' && c != '.')
        .unwrap_or(rest.len());
    if end > 0 {
        return rest[..end].parse::<f64>().ok();
    }
    None
}

/// 解析 JSON bool 值(Flutter 发送的开关为 true/false)
fn json_get_bool(json: &str, key: &str) -> Option<bool> {
    let pattern = format!("\"{}\":", key);
    let start = json.find(&pattern)?;
    let rest = json[start + pattern.len()..].trim_start();
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

/// http/https 白名单校验(与主 crate api::open_url_checked 同规则):
/// URL 可能来自网络响应, 只放行 http/https, 其它 scheme 直接拒绝。
fn open_url_checked(url: &str) {
    if url.starts_with("https://") || url.starts_with("http://") {
        // Windows 上用 cmd start 打开默认浏览器;CREATE_NO_WINDOW 防闪黑框
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            let _ = std::process::Command::new("cmd")
                .args(["/c", "start", "", url])
                .creation_flags(CREATE_NO_WINDOW)
                .spawn();
        }
        #[cfg(not(windows))]
        {
            let _ = url;
        }
    } else {
        eprintln!("[overlay] 拒绝打开非 http(s) URL: {url}");
    }
}
