fn pipe_wide_name() -> Vec<u16> {
    OsStr::new(r"\\.\pipe\glaspen2_settings")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

use glaspen_ipc::panel_wire::{encode_response, PanelDecoder, PanelInput};
use std::io::Write as _;

/// 一个连接的协议感知写端: JSON 客户端照旧写行; protobuf 客户端把业务层
/// 产出的 JSON 响应行编码成 Panel 帧。
struct PanelOut {
    file: std::fs::File,
    pending: Vec<u8>,
}

impl PanelOut {
    fn new(file: std::fs::File) -> Self {
        Self {
            file,
            pending: Vec::new(),
        }
    }
}

impl std::io::Write for PanelOut {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.pending.extend_from_slice(buf);
        while let Some(pos) = self.pending.iter().position(|b| *b == b'\n') {
            let line = self.pending[..pos].to_vec();
            self.pending.drain(..=pos);
            let text = String::from_utf8_lossy(&line).trim().to_string();
            if text.is_empty() {
                continue;
            }
            let encoded = encode_response(&text);
            self.file.write_all(&encoded)?;
        }
        if self.pending.len() > 8 * 1024 * 1024 {
            tracing::warn!("面板 protobuf 响应缓冲超限, 丢弃");
            self.pending.clear();
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
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
            tracing::error!("CreateNamedPipeW failed");
            std::thread::sleep(std::time::Duration::from_secs(2));
            continue;
        }

        // Block until a client connects
        let ok = unsafe { ConnectNamedPipe(pipe, std::ptr::null_mut()) };
        if ok == 0 {
            let err = std::io::Error::last_os_error();
            // ERROR_PIPE_CONNECTED (535) means client connected before ConnectNamedPipe
            if err.raw_os_error() != Some(535) {
                tracing::error!("ConnectNamedPipe error: {}", err);
                close_pipe(pipe);
                continue;
            }
        }
        tracing::info!("Flutter settings client connected");

        // 每个连接一个线程继续接受下一个: 面板是独立进程, 旧面板/手动双击开的
        // 面板可能占着前一个连接 —— 单线程服务会让"后来那个看得见的面板"
        // 永远连不上(它的改设置/切画布模式全部静默丢失)。多客户端彼此独立,
        // 回推帧广播给所有连接(真正的面板 + 调试客户端; 见 PANEL_WRITERS)。
        std::thread::spawn(move || {
            handle_pipe_client(pipe, hwnd);
            tracing::info!("Flutter settings client disconnected");
        });
    }
}

fn close_pipe(pipe: isize) {
    unsafe {
        DisconnectNamedPipe(pipe);
        let _ = windows::Win32::Foundation::CloseHandle(HANDLE(pipe as *mut _));
    }
}

fn handle_pipe_client(pipe: isize, hwnd: isize) {
    use std::io::{Read, Write};
    use std::os::windows::io::{FromRawHandle, IntoRawHandle};

    // Wrap pipe HANDLE in a single File for both read and write
    let mut stream = unsafe { std::fs::File::from_raw_handle(pipe as *mut std::ffi::c_void) };
    let mut decoder = PanelDecoder::new();
    let mut buf = [0u8; 4096];

    let (mut writer, my_gen): (PanelOut, u64) = match (
        stream.try_clone(),
        stream.try_clone(),
    ) {
        (Ok(reg), Ok(out)) => {
            let conn_gen = PANEL_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            PANEL_WRITERS
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push((conn_gen, reg));
            (PanelOut::new(out), conn_gen)
        }
        _ => {
            tracing::warn!("面板回推通道创建失败");
            let _ = stream.into_raw_handle();
            close_pipe(pipe);
            return;
        }
    };

    // **不要在这个线程上挂一个阻塞的 ReadFile**: 同步(非 OVERLAPPED)命名管道在
    // 有挂起读的时候, 另一个线程(panel-push)的 WriteFile 会被一起卡住 ——
    // 回推 onSettingsChanged 就永远写不出去(面板 tab/开关不同步的根因)。
    // 这里改成 PeekNamedPipe 轮询, 只有确认有数据才 ReadFile(瞬时返回)。
    loop {
        let mut avail: u32 = 0;
        let peek_ok = unsafe {
            PeekNamedPipe(
                HANDLE(pipe as *mut _),
                None,
                0,
                None,
                Some(&mut avail as *mut u32),
                None,
            )
        };
        if peek_ok.is_err() {
            tracing::info!("Flutter settings client peek 断开");
            break;
        }
        if avail == 0 {
            std::thread::sleep(std::time::Duration::from_millis(5));
            continue;
        }
        let to_read = (avail as usize).min(buf.len());
        let n = match stream.read(&mut buf[..to_read]) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                tracing::error!("client read error: {}", e);
                break;
            }
        };

        let inputs = match decoder.feed(&buf[..n]) {
            Ok(inputs) => inputs,
            Err(err) => {
                tracing::warn!("面板管道帧错误: {err}");
                break;
            }
        };

        for input in inputs {
            match input {
                PanelInput::Json(line) => {
                    process_pipe_message(&line, hwnd, &mut writer);
                }
                PanelInput::SetSetting {
                    request_id,
                    key,
                    value,
                } => {
                    process_set_setting(&key, value, request_id as i64, hwnd, &mut writer);
                }
                PanelInput::GetSettings { request_id } => {
                    let overrides = settings_snapshot_overrides();
                    if let Ok(data) = glaspen_core::settings_snapshot::snapshot_json(
                        glaspen_core::ui::Platform::Windows,
                        None,
                        &overrides,
                    ) {
                        let frame = glaspen_ipc::encode_settings_frame(request_id, &data);
                        let _ = writer.write_frame(&frame);
                    }
                }
                PanelInput::ListPages { request_id } => {
                    let pages = glaspen_core::export::list_screens_summary();
                    let frame = glaspen_ipc::encode_page_list_frame(request_id, &pages);
                    let _ = writer.write_frame(&frame);
                }
                PanelInput::PageThumbnails {
                    request_id,
                    ids,
                    max_size,
                } => {
                    let entries = glaspen_core::export::page_thumbnails_entries(&ids, max_size);
                    let frame = glaspen_ipc::encode_page_thumbnails_frame(request_id, &entries);
                    let _ = writer.write_frame(&frame);
                }
                PanelInput::PageOp {
                    request_id,
                    request,
                } => {
                    process_page_op(request_id, &request, hwnd, &mut writer);
                }
                PanelInput::GetBubbles { request_id } => {
                    let messages = glaspen_core::export::bubbles::messages_snapshot();
                    let frame = glaspen_ipc::encode_bubble_messages_frame(request_id, messages);
                    let _ = writer.write_frame(&frame);
                }
                PanelInput::NavigateToPage { screen_id } => {
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
                }
                PanelInput::Hotkey { key } => {
                    let cmd = match key.as_str() {
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
                        let _ = unsafe {
                            PostMessageW(
                                Some(HWND(hwnd as *mut _)),
                                WM_TRAY_COMMAND,
                                WPARAM(cmd),
                                LPARAM(usize::MAX as isize),
                            )
                        };
                    }
                }
                PanelInput::OpenUrl { url } => open_url_checked(url),
                PanelInput::FlushBubbles { request_id } => {
                    glaspen_core::export::bubbles::flush_now();
                    let frame = glaspen_ipc::encode_page_op_frame(
                        request_id,
                        "flushBubbles_response",
                        glaspen_ipc::PageOpValue::Ok(true),
                    );
                    let _ = writer.write_frame(&frame);
                }
                PanelInput::AppVersion { request_id } => {
                    let frame = glaspen_ipc::encode_app_version_frame(
                        request_id,
                        glaspen_core::update::current_version(),
                    );
                    let _ = writer.write_frame(&frame);
                }
                PanelInput::CanvasOverview {
                    request_id,
                    w,
                    h,
                    action,
                } => {
                    process_canvas_overview(request_id, w, h, action, hwnd, &mut writer);
                }
                PanelInput::BackupCommand {
                    request_id,
                    command,
                } => {
                    use glaspen_ipc::proto::BackupCommand;
                    let (kind, ok, message) = match command {
                        BackupCommand::BackupNow => {
                            let (ok, message) = match glaspen_core::runtime()
                                .block_on(glaspen_core::db::backup_now())
                            {
                                Ok(path) => (true, path),
                                Err(e) => (false, e),
                            };
                            ("backupNow_response", ok, message)
                        }
                        BackupCommand::RestoreLatest => {
                            let (ok, message) = match glaspen_core::runtime()
                                .block_on(glaspen_core::db::restore_latest_backup())
                            {
                                Ok((path, pages)) => (true, format!("{path}(当前共 {pages} 页)")),
                                Err(e) => (false, e),
                            };
                            ("restoreLatestBackup_response", ok, message)
                        }
                    };
                    let frame = glaspen_ipc::encode_backup_outcome_frame(
                        request_id,
                        kind,
                        ok,
                        &message,
                    );
                    let _ = writer.write_frame(&frame);
                }
                PanelInput::ChatLogin { request_id } => {
                    let (ok, message) = match glaspen_core::export::chat_auth_test_login_blocking() {
                        Ok(()) => (true, String::new()),
                        Err(e) => (false, e),
                    };
                    let frame = glaspen_ipc::encode_chat_login_outcome_frame(
                        request_id,
                        ok,
                        &message,
                    );
                    let _ = writer.write_frame(&frame);
                }
                PanelInput::DownloadUpdate { request_id } => {
                    process_download_update(request_id, &mut writer);
                }
                PanelInput::CancelDownload => {}
                PanelInput::ApplyUpdate { request_id } => {
                    let installer = DOWNLOADED_INSTALLER
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .clone()
                        .or_else(|| {
                            glaspen_core::update::newest_installer(
                                &glaspen_core::update::update_dir(),
                            )
                        });
                    match installer {
                        Some(installer) => {
                            crate::set_pending_installer(installer);
                            tracing::info!("退出并启动更新安装器");
                            let frame = glaspen_ipc::encode_apply_update_frame(
                                request_id,
                                true,
                                "",
                            );
                            let _ = writer.write_frame(&frame);
                            let _ = unsafe {
                                PostMessageW(
                                    Some(HWND(hwnd as *mut _)),
                                    WM_TRAY_COMMAND,
                                    WPARAM(CMD_QUIT),
                                    LPARAM(0),
                                )
                            };
                        }
                        None => {
                            let frame = glaspen_ipc::encode_apply_update_frame(
                                request_id,
                                false,
                                "找不到已下载的安装包,请重新下载",
                            );
                            let _ = writer.write_frame(&frame);
                        }
                    }
                }
                PanelInput::NasBackup { request_id } => {
                    let text = glaspen_core::export::nas_backup::backup_now();
                    let value: serde_json::Value =
                        serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
                    let frame = glaspen_ipc::encode_nas_backup_frame(
                        request_id,
                        value["ok"].as_bool().unwrap_or(false),
                        value["size_bytes"].as_u64().unwrap_or(0),
                        value["stroke_count"].as_u64().unwrap_or(0),
                        value["error"].as_str().unwrap_or(""),
                        value["hint"].as_str().unwrap_or(""),
                    );
                    let _ = writer.write_frame(&frame);
                }
                PanelInput::CheckUpdate { request_id } => {
                    let current = glaspen_core::update::current_version().to_string();
                    let (ok, latest, has_update, url, notes, error) =
                        match glaspen_core::update::fetch_latest() {
                            Ok(r) => {
                                let has =
                                    glaspen_core::update::is_newer(&r.tag, &current);
                                (true, r.tag, has, r.url, r.notes, String::new())
                            }
                            Err(e) => (
                                false,
                                String::new(),
                                false,
                                String::new(),
                                String::new(),
                                e,
                            ),
                        };
                    let frame = glaspen_ipc::encode_update_info_frame(
                        request_id,
                        ok,
                        &current,
                        &latest,
                        has_update,
                        &url,
                        &notes,
                        &error,
                    );
                    let _ = writer.write_frame(&frame);
                }
                PanelInput::ExportPdf { request_id } => {
                    let ok = glaspen_core::export::glaspen2_export_pdf() != 0;
                    let frame = glaspen_ipc::encode_page_op_frame(
                        request_id,
                        "exportPdf_response",
                        glaspen_ipc::PageOpValue::Ok(ok),
                    );
                    let _ = writer.write_frame(&frame);
                }
            }
        }
        let _ = writer.flush();
    }

    let _ = writer.flush();

    // Prevent File from closing the handle; we close it ourselves
    let _ = stream.into_raw_handle();
    // 只移除"自己这一代"的回推句柄: 广播表里其它连接(如真正的面板)不动
    PANEL_WRITERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|(g, _)| *g != my_gen);
    close_pipe(pipe);
    // 面板进程断开: 若当时正在录制快捷键, 立即恢复全局热键
    // (Windows 面板是独立进程, 崩溃/被杀不该让快捷键永久失效)。
    let hwnd = HWND(hwnd as *mut std::ffi::c_void);
    resume_if_paused(hwnd);
}

// ── 覆盖层 → 设置面板回推(等价 macOS sync_settings_panel) ──
// 面板是独立进程, 只能经命名管道推。连接时保存一份写句柄克隆;
// 消息循环线程(热键/菜单改设置)用 push_settings_changed 推局部字段,
// Dart 侧缺省字段沿用当前值, 于是开关/圆点能实时跟上。
//
// **写必须在独立线程做**: 面板若是暂停读(忙碌/挂起/被杀但句柄未断),
// 窗口线程上的 write_all 会阻塞 → 整个消息循环卡死 → 通知不消失、热键失灵。
// 所以窗口线程只往队列里投帧, 由 panel-push 线程负责真正的阻塞写。
/// 当前所有连接的写句柄(连接代号, 句柄)。回推采用**广播**: 每个连接都推,
/// 断开时按代号只移除自己那份。
/// 以前只保留"最新连接"的句柄: 调试客户端/第二个面板断开时会把它清掉, 真正的
/// 面板从此收不到 onSettingsChanged —— 表现就是面板 tab/圆点与玻璃不同步。
static PANEL_WRITERS: std::sync::LazyLock<
    std::sync::Mutex<Vec<(u64, std::fs::File)>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(Vec::new()));
static PANEL_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// 回推帧队列(有界): 窗口线程 try_send 永不阻塞; 面板不读时最多积压 64 帧,
/// 再多的直接丢弃(局部字段帧丢一帧无妨, 下次改设置会重新推)。
static PANEL_TX: std::sync::LazyLock<std::sync::mpsc::SyncSender<String>> =
    std::sync::LazyLock::new(|| {
        let (tx, rx) = std::sync::mpsc::sync_channel::<String>(64);
        std::thread::Builder::new()
            .name("panel-push".into())
            .spawn(move || {
                use std::io::Write;
                for frame in rx {
                    // 取所有连接句柄的克隆后**立即放锁**: 阻塞写不能占着锁
                    // (否则面板重连时管道线程会卡在 lock 上)
                    let conns: Vec<(u64, std::fs::File)> = {
                        let guard =
                            PANEL_WRITERS.lock().unwrap_or_else(|e| e.into_inner());
                        guard
                            .iter()
                            .filter_map(|(g, f)| f.try_clone().ok().map(|c| (*g, c)))
                            .collect()
                    };
                    let mut dead: Vec<u64> = Vec::new();
                    for (conn_gen, mut f) in conns {
                        let encoded = encode_response(&frame);
                        if f.write_all(&encoded).is_err() || f.flush().is_err() {
                            dead.push(conn_gen); // 写失败 = 该连接已断开
                        }
                    }
                    if !dead.is_empty() {
                        PANEL_WRITERS
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .retain(|(g, _)| !dead.contains(g));
                    }
                }
            })
            .ok();
        tx
    });

/// 向设置面板推一帧 onSettingsChanged;未连接时静默丢弃。
/// **可从任意线程调用**(含窗口线程): 只入队, 绝不阻塞。
fn push_settings_changed(data_json: &str) {
    let frame = format!("{{\"type\":\"onSettingsChanged\",\"data\":{data_json}}}\n");
    match PANEL_TX.try_send(frame) {
        Ok(()) => {}
        Err(std::sync::mpsc::TrySendError::Full(_)) => {
            tracing::debug!("面板回推积压已满, 丢弃一帧局部设置");
        }
        Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {}
    }
}

/// 管道线程 → 消息循环线程: 投递一个 WM_TRAY_COMMAND(改运行态设置用)
fn post_tray_command(hwnd: isize, cmd: usize, param: usize) {
    let _ = unsafe {
        PostMessageW(
            Some(HWND(hwnd as *mut std::ffi::c_void)),
            WM_TRAY_COMMAND,
            WPARAM(cmd),
            LPARAM(param as isize),
        )
    };
}

/// 无限画布总览载荷(macOS canvas_overview_payload 同款):
/// 把当前画布内容按包围盒适配渲染成 PNG + 映射当前视口矩形,
/// 返回 data JSON 内容("png":"<b64>","rect":[x,y,w,h]);空画布返回空串。
/// 在管道线程调用:只读 STROKES / 镜头原子量,不动 UI 状态。
fn render_overview(ow: i32, oh: i32) -> Option<(Vec<u8>, [f64; 4])> {
    // **总览永远画数据库里的无限画布**: 面板点「自由涂鸦」tab 时"切画布模式"和
    // "拉总览"是两条异步管道消息, 覆盖层可能还停在活页本 —— 这时若用 STROKES
    // (页笔迹)渲染, 就会出现"库里明明有无限画布涂鸦, 面板总览却是空/画着活页本"。
    let mut bbox = [0.0f64; 4];
    if glaspen_core::export::glaspen2_infinite_canvas_bbox(bbox.as_mut_ptr()) == 0 {
        return None;
    }
    let (mut bx, mut by, mut bx2, mut by2) = (bbox[0], bbox[1], bbox[2], bbox[3]);
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
    let ptr = glaspen_core::export::glaspen2_render_infinite_canvas_overview(
        bx,
        by,
        bw,
        bh,
        ow,
        oh,
        &mut out_len,
    );
    if ptr.is_null() || out_len <= 0 {
        return None;
    }
    let png = unsafe { std::slice::from_raw_parts(ptr, out_len as usize) }.to_vec();
    glaspen_core::export::glaspen2_free_rust_bytes(ptr, out_len);

    // 总览映射(scale/offset 必须与渲染一致),再映射当前视口矩形
    let ov_scale = ((ow as f64) / bw).min((oh as f64) / bh);
    let ov_ox = ((ow as f64) - bw * ov_scale) * 0.5;
    let ov_oy = ((oh as f64) - bh * ov_scale) * 0.5;
    // 视口矩形: 无限画布模式下用实时镜头; 还停在活页本时用库里保存的无限画布镜头
    // (切换动作还没被窗口线程处理完时, camera() 是活页本的 0,0,1, 会画错视口框)。
    // 镜头数学(交互 zoom 兜底)单源 core Camera;载入兜底同 Camera::from_stored。
    let cam = if infinite_on() {
        camera()
    } else {
        glaspen_core::runtime()
            .block_on(glaspen_core::db::get_infinite_transform())
            .map(|(px, py, z)| Camera::from_stored(px, py, z))
            .unwrap_or(Camera::HOME)
    };
    let z = cam.step_zoom();
    let sw = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) } as f64;
    let sh = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) } as f64;
    let vr_w = sw / z * ov_scale;
    let vr_h = sh / z * ov_scale;
    let vx = ov_ox + (cam.pan_x - bx) * ov_scale;
    let vy = ov_oy + (cam.pan_y - by) * ov_scale;
    Some((png, [vx, vy, vr_w, vr_h]))
}

/// D12: 设置快照的运行时专属值(其余字段/顺序/类型/缺省值/DB 回退单源在
/// core settings_snapshot)。这里只放"不是持久化设置"的值:
/// - launchAtLogin: HKCU Run 注册表才是平台存储, 不落 user_settings;
/// - shortcuts: 生效表(含默认补齐/保留组合/冲突消解), 由热键注册表在运行时算出。
fn settings_snapshot_overrides() -> serde_json::Map<String, serde_json::Value> {
    let mut overrides = serde_json::Map::new();
    overrides.insert(
        "launchAtLogin".to_string(),
        serde_json::Value::Bool(glaspen_core::export::glaspen2_is_launch_at_login() == 1),
    );
    overrides.insert(
        "shortcuts".to_string(),
        serde_json::Value::String(effective_json()),
    );
    overrides
}

fn process_set_setting(
    key: &str,
    mut value: serde_json::Value,
    req_id: i64,
    hwnd: isize,
    writer: &mut PanelOut,
) {
    // ① 命令型 key(不是设置项, 但仍走 setSetting 通道): 保持旧实现。
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
        // 用面板的 GIF 设置(macOS 同键), 不再硬编码默认值
        let (fps, resolution, speed, end_mode) = gif_settings();
        let result = glaspen_core::export::glaspen2_save_animated_gif(
            fps,
            resolution,
            speed,
            end_mode,
        );
        tracing::info!(
            "animated GIF export: {}",
            if result != 0 { "OK" } else { "FAILED" }
        );
        hud_notify_async(if result != 0 {
            "动画 GIF 已保存到桌面".to_string()
        } else {
            "动画 GIF 导出失败".to_string()
        });
    } else if key == "export_pdf" {
        let result = glaspen_core::export::glaspen2_export_pdf();
        tracing::info!(
            "PDF export: {}",
            if result != 0 { "OK" } else { "FAILED" }
        );
        hud_notify_async(if result != 0 {
            "PDF 已保存到桌面".to_string()
        } else {
            "PDF 导出失败".to_string()
        });
    } else {
        // ② 设置项 + 控制键: core effect 单源。
        // 旧行为(保留): invertFps 缺 value 时按 30 处理
        // (旧代码 json_get_int(line, "value").unwrap_or(30))。
        if value.is_null() && key == "invertFps" {
            value = serde_json::json!(30);
        }
        let effect = match glaspen_core::settings_apply::apply_setting(&key, &value) {
            Ok(effect) => effect,
            Err(e) => {
                // 未知/只读键、非法值: 明确回错误帧, 不再静默吞掉。
                tracing::warn!("setSetting 未应用: {key}={value}: {e}");
                let _ = writer.write_all(
                    glaspen_core::protocol::error_line(req_id, "setSetting", &e).as_bytes(),
                );
                let _ = writer.flush();
                return;
            }
        };
        use glaspen_core::settings_apply::SettingEffect;
        match effect {
            // 纯落库项: core 已写 user_settings, Windows 运行时无副作用。
            // (invertInk/invertFps/notebookStyle/panelTransparent 旧实现也只落库;
            //  flipEffect 是 macOS 专属键, 面板不会发给 Windows。)
            SettingEffect::BubbleAutoNewPage(_)
            | SettingEffect::BubbleReplayAuto(_)
            | SettingEffect::BubbleReplayHoldLast(_)
            | SettingEffect::BubbleReplaySpeed(_)
            | SettingEffect::NasUrl(_)
            | SettingEffect::NotebookStyle(_)
            | SettingEffect::ShowFreeCanvas(_)
            | SettingEffect::InvertInk(_)
            | SettingEffect::InvertFps(_)
            | SettingEffect::PanelTransparent(_)
            | SettingEffect::FlipEffect(_) => {}
            SettingEffect::Color(idx) => {
                // 越界按预设表忽略(与 macOS gl_settings_set_color 一致;
                // 旧实现把 idx 直接当命令号, 越界值可能撞到别的命令)。
                if idx >= 0 && (idx as usize) < glaspen_core::presets::COLOR_PRESETS.len() {
                    post_tray_command(hwnd, CMD_SELECT_COLOR + idx as usize, 0);
                }
            }
            SettingEffect::Width(idx) => {
                if idx >= 0 && (idx as usize) < glaspen_core::presets::WIDTH_PRESETS.len() {
                    post_tray_command(hwnd, CMD_SELECT_WIDTH + idx as usize, 0);
                }
            }
            SettingEffect::Grid(on) => {
                post_tray_command(hwnd, CMD_TOGGLE_GRID, on as usize);
            }
            SettingEffect::GridDivider(v) => {
                post_tray_command(hwnd, CMD_SET_GRID_DIVIDER, v as usize);
            }
            SettingEffect::GridSize(v) => {
                post_tray_command(hwnd, CMD_SET_GRID_SIZE, v as usize);
            }
            SettingEffect::GridFollowStrokes(on) => {
                post_tray_command(hwnd, CMD_SET_GRID_FOLLOW, on as usize);
            }
            SettingEffect::GlassFollowStrokes(on) => {
                post_tray_command(hwnd, CMD_SET_GLASS_FOLLOW, on as usize);
            }
            SettingEffect::SoftShadow(on) => {
                post_tray_command(hwnd, CMD_SET_SOFT_SHADOW, on as usize);
            }
            SettingEffect::Minimap(on) => {
                post_tray_command(hwnd, CMD_TOGGLE_MINIMAP, on as usize);
            }
            SettingEffect::FrostedGlass(on) => {
                post_tray_command(hwnd, CMD_TOGGLE_FROSTED, on as usize);
            }
            SettingEffect::PressureMonitor(on) => {
                post_tray_command(hwnd, CMD_TOGGLE_PRESSURE_MONITOR, on as usize);
            }
            SettingEffect::ChatIntegration(on) => {
                post_tray_command(hwnd, CMD_TOGGLE_CHAT_INTEGRATION, on as usize);
            }
            SettingEffect::Outline(on) => {
                post_tray_command(hwnd, CMD_TOGGLE_OUTLINE, on as usize);
            }
            SettingEffect::LaunchAtLogin(on) => {
                // HKCU Run 注册表即平台存储, 不落 DB(macOS 是 LaunchAgent plist)
                glaspen_core::export::glaspen2_set_launch_at_login(if on { 1 } else { 0 });
            }
            SettingEffect::ShowBubbleTab(on) => {
                glaspen_core::export::glaspen2_bubble_set_capture(if on { 1 } else { 0 });
            }
            SettingEffect::InfiniteCanvas(on) => {
                tracing::info!(
                    "面板请求切换画布模式: {}",
                    if on { "无限画布" } else { "活页本" }
                );
                post_tray_command(hwnd, CMD_TOGGLE_INFINITE_CANVAS, on as usize);
            }
            SettingEffect::ShareCanvas(on) => {
                // core 已写 share_ink; 仅集成开启时真正启停上行(旧 Windows 语义)
                let integration_on = glaspen_core::runtime()
                    .block_on(glaspen_core::db::load_setting("chat_integration"))
                    .and_then(|v| v.parse::<i32>().ok())
                    .unwrap_or(0)
                    != 0;
                if integration_on {
                    glaspen_core::export::glaspen2_share_ink_set_active(if on { 1 } else { 0 });
                }
            }
            SettingEffect::ChatApiBase(_) | SettingEffect::ChatUser(_) => {
                // core 已写 chat_api_base / chat_user; 这里只重载 auth 缓存
                glaspen_core::export::glaspen2_chat_auth_reload();
            }
            SettingEffect::ChatPassword(password) => {
                // 密码本体不进 settings 快照, core 不落库; Windows 旧语义 = 空串也写
                glaspen_core::runtime().block_on(glaspen_core::db::save_setting(
                    "chat_password",
                    &password,
                ));
                glaspen_core::export::glaspen2_chat_auth_reload();
            }
            SettingEffect::Language(_) => {
                // core 已把语言归一落库; Windows 面板是独立进程, 自己重读
            }
            SettingEffect::GifFps(v) => {
                let (_, res, spd, em) = gif_settings();
                persist_gif_settings(v as i32, res, spd, em);
            }
            SettingEffect::GifResolution(v) => {
                let (fps, _, spd, em) = gif_settings();
                persist_gif_settings(fps, v, spd, em);
            }
            SettingEffect::GifSpeed(v) => {
                let (fps, res, _, em) = gif_settings();
                persist_gif_settings(fps, res, v, em);
            }
            SettingEffect::GifEndMode(v) => {
                let (fps, res, spd, _) = gif_settings();
                persist_gif_settings(fps, res, spd, v as i32);
            }
            SettingEffect::Shortcuts(table) => {
                let saved: HashMap<String, String> = table
                    .as_object()
                    .map(|obj| {
                        obj.iter()
                            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                            .collect()
                    })
                    .unwrap_or_default();
                if saved.is_empty() {
                    // 旧 Windows 语义: 空表 = 忽略(不回默认、不落库);
                    // core 也不会为空表落库(见 settings_apply::apply_to_db)。
                    tracing::warn!("[hotkey] shortcuts 保存消息里没有可解析的 value");
                } else {
                    // 应用(补默认/挡保留/去冲突) → 落库生效表 → 回推面板
                    apply_saved_map(HWND(hwnd as *mut std::ffi::c_void), &saved);
                    let frame = format!(
                        "{{\"type\":\"onSettingsChanged\",\"data\":{{\"shortcuts\":\"{}\"}}}}\n",
                        json_escape(&effective_json())
                    );
                    let _ = writer.write_all(frame.as_bytes());
                    let _ = writer.flush();
                }
            }
            SettingEffect::HotkeyRecording(on) => {
                // 录制期间注销全部全局热键: RegisterHotKey 注册即吞键, 不注销
                // 面板根本收不到组合键。60s 兜底与面板断开恢复见 hotkeys.rs。
                set_recording(HWND(hwnd as *mut std::ffi::c_void), on);
            }
        }
    }
    // 写入成功后把落库后的完整快照回推给发起连接: 面板本地值和后端
    // 设置流可能有竞态, 以 core 快照为准再同步一次(局部推送仍保留)。
    let overrides = settings_snapshot_overrides();
    if let Ok(data) = glaspen_core::settings_snapshot::snapshot_json(
        glaspen_core::ui::Platform::Windows,
        None,
        &overrides,
    ) {
        let frame = glaspen_ipc::encode_settings_changed_frame(&data);
        let _ = writer.write_frame(&frame);
    }
}

fn process_page_op(
    request_id: u64,
    req: &glaspen_ipc::proto::PageOpRequest,
    hwnd: isize,
    writer: &mut PanelOut,
) {
    let kind = req.kind.as_str();
    let screen_id = req.screen_id;
    let result = match kind {
        "reorderPage" => {
            let ok = glaspen_core::runtime()
                .block_on(glaspen_core::db::reorder_screen(
                    req.screen_id,
                    req.anchor_id,
                    req.before,
                ))
                .is_ok();
            glaspen_ipc::PageOpValue::Ok(ok)
        }
        "deletePage" => {
            let was_current = req.screen_id == glaspen_core::export::glaspen2_get_current_screen_id();
            let next_id = if was_current {
                glaspen_core::export::glaspen2_next_screen_id()
            } else {
                0
            };
            let prev_id = if was_current {
                glaspen_core::export::glaspen2_prev_screen_id()
            } else {
                0
            };
            let ok = glaspen_core::export::glaspen2_delete_screen(req.screen_id) != 0;
            if ok {
                if infinite_on() {
                    let _ = unsafe {
                        PostMessageW(
                            Some(HWND(hwnd as *mut _)),
                            WM_TRAY_COMMAND,
                            WPARAM(CMD_REFRESH_PAGE),
                            LPARAM(0),
                        )
                    };
                } else if was_current {
                    let target = if next_id != 0 { next_id } else { prev_id };
                    let (cmd, param) = if target > 0 {
                        (CMD_NAVIGATE_TO_PAGE, target as usize)
                    } else {
                        (CMD_CLEAR_SCREEN, usize::MAX)
                    };
                    let _ = unsafe {
                        PostMessageW(
                            Some(HWND(hwnd as *mut _)),
                            WM_TRAY_COMMAND,
                            WPARAM(cmd),
                            LPARAM(param as isize),
                        )
                    };
                }
            }
            glaspen_ipc::PageOpValue::Ok(ok)
        }
        "lassoSelect" => {
            let poly: Vec<(f64, f64)> = req
                .lasso
                .as_chunks::<2>()
                .0
                .iter()
                .map(|p| (p[0], p[1]))
                .collect();
            let payload = poly
                .iter()
                .map(|(x, y)| format!("{x},{y}"))
                .collect::<Vec<_>>()
                .join(",");
            let c = std::ffi::CString::new(payload).unwrap_or_default();
            let ptr = glaspen_core::export::glaspen2_lasso_select(screen_id, c.as_ptr());
            let s = if ptr.is_null() {
                String::new()
            } else {
                unsafe { std::ffi::CStr::from_ptr(ptr) }
                    .to_string_lossy()
                    .into_owned()
            };
            if !ptr.is_null() {
                glaspen_core::export::glaspen2_free_c_string(ptr);
            }
            let ids: Vec<i64> = s.split(',').filter_map(|t| t.trim().parse().ok()).collect();
            glaspen_ipc::PageOpValue::Ids(ids)
        }
        "moveStrokes" => {
            let csv = std::ffi::CString::new(
                req.ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","),
            )
            .unwrap_or_default();
            let ok = glaspen_core::export::glaspen2_move_strokes(
                screen_id,
                csv.as_ptr(),
                req.dx,
                req.dy,
            ) != 0;
            glaspen_ipc::PageOpValue::Ok(ok)
        }
        "moveStrokesToPage" => {
            let csv = std::ffi::CString::new(
                req.ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","),
            )
            .unwrap_or_default();
            let ok = glaspen_core::export::glaspen2_move_strokes_to_page(
                screen_id,
                csv.as_ptr(),
                req.target_screen_id,
                req.dx,
                req.dy,
            ) != 0;
            glaspen_ipc::PageOpValue::Ok(ok)
        }
        "deleteStrokes" => {
            let csv = std::ffi::CString::new(
                req.ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","),
            )
            .unwrap_or_default();
            let ok = glaspen_core::export::glaspen2_delete_strokes(screen_id, csv.as_ptr()) != 0;
            glaspen_ipc::PageOpValue::Ok(ok)
        }
        "copyStrokes" => {
            let csv = std::ffi::CString::new(
                req.ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","),
            )
            .unwrap_or_default();
            let ptr = glaspen_core::export::glaspen2_copy_strokes_payload(
                screen_id,
                csv.as_ptr(),
            );
            let s = if ptr.is_null() {
                String::new()
            } else {
                unsafe { std::ffi::CStr::from_ptr(ptr) }
                    .to_string_lossy()
                    .into_owned()
            };
            if !ptr.is_null() {
                glaspen_core::export::glaspen2_free_c_string(ptr);
            }
            glaspen_ipc::PageOpValue::Payload(s)
        }
        "pasteStrokes" => {
            let shifted = glaspen_core::export::shift_stroke_payload(
                &req.payload,
                req.cx,
                req.cy,
            );
            let c = std::ffi::CString::new(shifted).unwrap_or_default();
            let n = glaspen_core::export::glaspen2_paste_strokes(screen_id, c.as_ptr());
            glaspen_ipc::PageOpValue::Count(n)
        }
        other => {
            tracing::warn!("未知 typed 页面操作: {other}");
            glaspen_ipc::PageOpValue::Ok(false)
        }
    };
    let response_kind = format!("{kind}_response");
    let frame = glaspen_ipc::encode_page_op_frame(request_id, &response_kind, result);
    let _ = writer.write_frame(&frame);
    if matches!(
        kind,
        "moveStrokes" | "moveStrokesToPage" | "deleteStrokes" | "pasteStrokes"
    ) {
        let _ = unsafe {
            PostMessageW(
                Some(HWND(hwnd as *mut _)),
                WM_TRAY_COMMAND,
                WPARAM(CMD_REFRESH_PAGE),
                LPARAM(screen_id as isize),
            )
        };
    }
}

fn process_canvas_overview(
    request_id: u64,
    w: i32,
    h: i32,
    action: glaspen_ipc::proto::CanvasAction,
    hwnd: isize,
    writer: &mut PanelOut,
) {
    use glaspen_ipc::proto::CanvasAction;
    if !infinite_on() {
        post_tray_command(hwnd, CMD_TOGGLE_INFINITE_CANVAS, 1);
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    let cmd = match action {
        CanvasAction::CanvasNew => Some((CMD_CANVAS_NEW, 0usize)),
        CanvasAction::CanvasHome => Some((CMD_CANVAS_HOME, 0)),
        CanvasAction::CanvasCenter => Some((CMD_CANVAS_CENTER, 0)),
        CanvasAction::CanvasCurrent => None,
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
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
    let (png, rect) = render_overview(w, h).unwrap_or_default();
    let frame = glaspen_ipc::encode_canvas_overview_frame(request_id, &png, &rect);
    let _ = writer.write_frame(&frame);
}

fn process_download_update(request_id: u64, writer: &mut PanelOut) {
    use std::io::{Read as _, Write as _};
    use std::os::windows::io::AsRawHandle;
    let handle = HANDLE(writer.as_raw_handle());
    let mut frame_at = std::time::Instant::now() - std::time::Duration::from_millis(200);
    let result = glaspen_core::update::download_to_cache(|received, total| {
        if total == 0
            || received == total
            || frame_at.elapsed() >= std::time::Duration::from_millis(100)
        {
            frame_at = std::time::Instant::now();
            let frame = glaspen_ipc::encode_download_progress_frame(
                request_id,
                received,
                total,
                false,
                "",
                "",
            );
            let _ = writer.write_frame(&frame);
        }
        let mut avail: u32 = 0;
        if unsafe { PeekNamedPipe(handle, None, 0, None, Some(&mut avail), None) }.is_ok()
            && avail > 0
        {
            let mut buf = vec![0u8; avail as usize];
            if let Ok(n) = writer.read(&mut buf) {
                if let Ok(inputs) = PanelDecoder::new().feed(&buf[..n]) {
                    if inputs
                        .iter()
                        .any(|i| matches!(i, PanelInput::CancelDownload))
                    {
                        return false;
                    }
                }
            }
        }
        true
    });
    match result {
        Ok((path, received, total)) => {
            tracing::info!("更新包已下载: {}", path.display());
            *DOWNLOADED_INSTALLER.lock().unwrap_or_else(|e| e.into_inner()) = Some(path.clone());
            let p = path.to_string_lossy().replace('\', "/");
            let frame = glaspen_ipc::encode_download_progress_frame(
                request_id, received, total, true, "", &p,
            );
            let _ = writer.write_frame(&frame);
        }
        Err(e) => {
            let frame = glaspen_ipc::encode_download_progress_frame(
                request_id,
                0,
                0,
                true,
                &e.to_string(),
                "",
            );
            let _ = writer.write_frame(&frame);
        }
    }
    let _ = writer.flush();
}

fn process_pipe_message(line: &str, hwnd: isize, writer: &mut PanelOut) {
    use std::io::Write;

    let msg_type = json_get_str(line, "type");

    // 协议契约(core/protocol.rs): 未登记的命令直接回错误。
    // 新增面板命令必须先进 COMMANDS 表, 否则这里拒绝 —— 不再静默丢弃。
    if !glaspen_core::protocol::known(&msg_type) {
        let req_id = json_get_i64(line, "reqId").unwrap_or(0);
        let _ = writer.write_all(
            glaspen_core::protocol::error_line(req_id, &msg_type, "未知面板命令").as_bytes(),
        );
        let _ = writer.flush();
        return;
    }

    if msg_type == "uiConfig" {
        // 面板启动时拉一次性平台配置: 与 macOS(FRB)是 core 同一份
        // `glaspen_core::ui` schema, Windows 走命名管道取。
        // 响应行由 core 的 `pipe_ui_config_response_json` 统一生成 —— 字段
        // 形状/内嵌对象/末尾 '\n'(面板按行分帧)都在 core 单测里锁住,
        // 这里不手拼 JSON。
        // 没有 reqId 时沿用其它命令的约定(reqId = 0)。
        let req_id = json_get_i64(line, "reqId").unwrap_or(0);
        // 独立 exe 面板可能与覆盖层版本错位: 版本不一致时明确报错, 让面板
        // 走 fallback 日志, 而不是拿着不认识的协议继续跑。
        let payload: serde_json::Value =
            serde_json::from_str(line.trim_end()).unwrap_or(serde_json::Value::Null);
        if let Err(msg) = glaspen_core::protocol::check_panel_version(&payload) {
            let _ = writer.write_all(
                glaspen_core::protocol::error_line(req_id, "uiConfig", &msg).as_bytes(),
            );
        } else {
            let resp = glaspen_core::ui::pipe_ui_config_response_json(req_id);
            let _ = writer.write_all(resp.as_bytes());
        }
        let _ = writer.flush();

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
        tracing::info!("拒绝打开非 http(s) URL: {url}");
    }
}
