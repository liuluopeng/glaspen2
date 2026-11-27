fn pipe_wide_name() -> Vec<u16> {
    OsStr::new(r"\\.\pipe\glaspen2_settings")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

use glaspen_ipc::panel_wire::{encode_response, PanelDecoder};
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

        let lines = match decoder.feed(&buf[..n]) {
            Ok(lines) => lines,
            Err(err) => {
                tracing::warn!("面板管道帧错误: {err}");
                break;
            }
        };

        for line in lines {
            process_pipe_message(&line, hwnd, &mut writer);
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
fn render_overview_json(ow: i32, oh: i32) -> String {
    // **总览永远画数据库里的无限画布**: 面板点「自由涂鸦」tab 时"切画布模式"和
    // "拉总览"是两条异步管道消息, 覆盖层可能还停在活页本 —— 这时若用 STROKES
    // (页笔迹)渲染, 就会出现"库里明明有无限画布涂鸦, 面板总览却是空/画着活页本"。
    let mut bbox = [0.0f64; 4];
    if glaspen_core::export::glaspen2_infinite_canvas_bbox(bbox.as_mut_ptr()) == 0 {
        return String::new();
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
        return String::new();
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, out_len as usize) };
    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes);
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
    format!(
        "\"png\":\"{}\",\"rect\":[{},{},{},{}]",
        b64, vx, vy, vr_w, vr_h
    )
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
    } else if msg_type == "reorderPage" {
        // 活页本重排(面板前后移)
        let req_id = json_get_i64(line, "reqId").unwrap_or(0);
        let screen_id = json_get_i64(line, "screenId").unwrap_or(0);
        let anchor_id = json_get_i64(line, "anchorId").unwrap_or(0);
        let before = json_get_bool(line, "before").unwrap_or(false);
        let ok = glaspen_core::runtime()
            .block_on(glaspen_core::db::reorder_screen(screen_id, anchor_id, before))
            .is_ok();
        // Dart 侧统一从 data 取结果(pending reqId 通道); 顶层 ok 保留给
        // 旧客户端。此前只有顶层 ok, 面板永远读不到、回滚并误报失败,
        // 而数据库其实已经改了。
        let resp = format!(
            "{{\"type\":\"reorderPage_response\",\"reqId\":{},\"ok\":{},\"data\":{{\"ok\":{}}}}}
",
            req_id, ok, ok
        );
        let _ = writer.write_all(resp.as_bytes());
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
        // 删除一页(内容 tab)。delete_screen 会把 current 置 0, 所以"相邻页"
        // 必须在**删除前**按当前页取好(macOS glaspen2_macos_delete_page 同款),
        // 否则删完 next/prev 都查不到, 外壳永远进不了切页分支。
        let screen_id = json_get_i64(line, "screenId").unwrap_or(0);
        let req_id = json_get_i64(line, "reqId").unwrap_or(0);
        let was_current =
            screen_id == glaspen_core::export::glaspen2_get_current_screen_id();
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
        let ok = glaspen_core::export::glaspen2_delete_screen(screen_id);
        if ok != 0 {
            if infinite_on() {
                // 无限画布独立存储, 不受页删除影响: 重载重绘即可
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
                if target > 0 {
                    let _ = unsafe {
                        PostMessageW(
                            Some(HWND(hwnd as *mut _)),
                            WM_TRAY_COMMAND,
                            WPARAM(CMD_NAVIGATE_TO_PAGE),
                            LPARAM(target as isize),
                        )
                    };
                } else {
                    // 没有相邻页: 新建一页(macOS 删光后走 clear_screen)
                    let _ = unsafe {
                        PostMessageW(
                            Some(HWND(hwnd as *mut _)),
                            WM_TRAY_COMMAND,
                            WPARAM(CMD_CLEAR_SCREEN),
                            LPARAM(usize::MAX as isize),
                        )
                    };
                }
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
    } else if msg_type == "lassoSelect"
        || msg_type == "moveStrokes"
        || msg_type == "moveStrokesToPage"
        || msg_type == "deleteStrokes"
        || msg_type == "copyStrokes"
        || msg_type == "pasteStrokes"
    {
        // 页面详情: 圈选/移动/复制粘贴/删除所选(纯 DB 数据操作, 同 macOS)
        let req_id = json_get_i64(line, "reqId").unwrap_or(0);
        let screen_id = json_get_i64(line, "screenId").unwrap_or(0);
        let nums = |v: &serde_json::Value| -> Vec<(f64, f64)> {
            v.as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|p| {
                            let arr = p.as_array()?;
                            Some((arr.first()?.as_f64()?, arr.get(1)?.as_f64()?))
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        let ids_json = |v: &serde_json::Value| -> Vec<i64> {
            v.as_array()
                .map(|a| a.iter().filter_map(|x| x.as_i64()).collect())
                .unwrap_or_default()
        };
        let val = json_get_str(line, "value");
        let parsed: Option<serde_json::Value> = serde_json::from_str(val).ok();
        let resp_val = match msg_type {
            "lassoSelect" => {
                // Dart 发的是 value: [[x,y],...]; 早期实现读 poly, 永远为空。
                let poly = parsed
                    .as_ref()
                    .map(|v| nums(v.get("poly").unwrap_or_else(|| &v["value"])))
                    .unwrap_or_default();
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
                    unsafe { std::ffi::CStr::from_ptr(ptr) }.to_string_lossy().into_owned()
                };
                if !ptr.is_null() {
                    glaspen_core::export::glaspen2_free_c_string(ptr);
                }
                let ids: Vec<i64> = s.split(',').filter_map(|t| t.trim().parse().ok()).collect();
                serde_json::json!({ "ids": ids })
            }
            "moveStrokes" => {
                let ids = parsed.as_ref().map(|v| ids_json(&v["ids"])).unwrap_or_default();
                let (dx, dy) = parsed
                    .as_ref()
                    .map(|v| (v["dx"].as_f64().unwrap_or(0.0), v["dy"].as_f64().unwrap_or(0.0)))
                    .unwrap_or((0.0, 0.0));
                let csv = std::ffi::CString::new(
                    ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","),
                )
                .unwrap_or_default();
                let ok = glaspen_core::export::glaspen2_move_strokes(
                    screen_id, csv.as_ptr(), dx, dy,
                ) != 0;
                serde_json::json!({ "ok": ok })
            }
            "moveStrokesToPage" => {
                let ids = parsed.as_ref().map(|v| ids_json(&v["ids"])).unwrap_or_default();
                let target = parsed
                    .as_ref()
                    .and_then(|v| v["targetScreenId"].as_i64())
                    .unwrap_or(0);
                let (dx, dy) = parsed
                    .as_ref()
                    .map(|v| (v["dx"].as_f64().unwrap_or(0.0), v["dy"].as_f64().unwrap_or(0.0)))
                    .unwrap_or((0.0, 0.0));
                let csv = std::ffi::CString::new(
                    ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","),
                )
                .unwrap_or_default();
                let ok = glaspen_core::export::glaspen2_move_strokes_to_page(
                    screen_id, csv.as_ptr(), target, dx, dy,
                ) != 0;
                serde_json::json!({ "ok": ok })
            }
            "deleteStrokes" => {
                let ids = parsed.as_ref().map(|v| ids_json(&v["ids"])).unwrap_or_default();
                let csv = std::ffi::CString::new(
                    ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","),
                )
                .unwrap_or_default();
                let ok =
                    glaspen_core::export::glaspen2_delete_strokes(screen_id, csv.as_ptr()) != 0;
                serde_json::json!({ "ok": ok })
            }
            "copyStrokes" => {
                let ids = parsed.as_ref().map(|v| ids_json(&v["ids"])).unwrap_or_default();
                let csv = std::ffi::CString::new(
                    ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","),
                )
                .unwrap_or_default();
                let ptr =
                    glaspen_core::export::glaspen2_copy_strokes_payload(screen_id, csv.as_ptr());
                let s = if ptr.is_null() {
                    String::new()
                } else {
                    unsafe { std::ffi::CStr::from_ptr(ptr) }.to_string_lossy().into_owned()
                };
                if !ptr.is_null() {
                    glaspen_core::export::glaspen2_free_c_string(ptr);
                }
                serde_json::json!({ "payload": s })
            }
            _ => {
                // pasteStrokes: 载荷坐标以包围盒中心为原点, 按 (cx, cy) 平移。
                let (payload, cx, cy) = parsed
                    .as_ref()
                    .map(|v| {
                        (
                            v["payload"].as_str().unwrap_or("").to_string(),
                            v["cx"].as_f64().unwrap_or(0.0),
                            v["cy"].as_f64().unwrap_or(0.0),
                        )
                    })
                    .unwrap_or_default();
                let shifted = glaspen_core::export::shift_stroke_payload(&payload, cx, cy);
                let c = std::ffi::CString::new(shifted).unwrap_or_default();
                let n = glaspen_core::export::glaspen2_paste_strokes(screen_id, c.as_ptr());
                serde_json::json!({ "count": n })
            }
        };
        let resp = format!(
            "{{\"type\":\"{}_response\",\"reqId\":{},\"data\":{}}}\n",
            msg_type, req_id, resp_val
        );
        let _ = writer.write_all(resp.as_bytes());
        let _ = writer.flush();
        // 数据改动(移动/粘贴/删除所选)后让覆盖层重载当前页并重绘 ——
        // macOS 侧由 core 的 refresh hook 调 glaspen2_macos_refresh_page 完成,
        // Windows 走 PostMessage 回消息循环线程。
        if matches!(
            msg_type,
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
    } else if msg_type == "getBubbles" {
        // 气泡消息: 全部已完结消息 JSON(先惰性过期)。体量小(折线点列), 直接内嵌 JSON 回复
        let req_id = json_get_i64(line, "reqId").unwrap_or(0);
        let json = glaspen_core::export::bubbles::messages_json();
        // 必须以 \n 结尾: 面板按行(\n)分帧解析, 少了换行这帧永远留在缓冲区,
        // 请求 completer 只能等 15s 超时 —— 表现就是「气泡消息一直转圈」。
        let resp = format!(
            "{{\"type\":\"getBubbles_response\",\"reqId\":{},\"data\":{}}}\n",
            req_id, json
        );
        let _ = writer.write_all(resp.as_bytes());
        let _ = writer.flush();
    } else if msg_type == "flushBubbles" {
        let req_id = json_get_i64(line, "reqId").unwrap_or(0);
        glaspen_core::export::bubbles::flush_now();
        let resp = format!(
            "{{\"type\":\"flushBubbles_response\",\"reqId\":{}}}\n",
            req_id
        );
        let _ = writer.write_all(resp.as_bytes());
        let _ = writer.flush();
    } else if msg_type == "nasBackup" {
        // NAS 备份: 整库快照 + 上传(耗时数秒~分钟, 管道线程同步执行;
        // 失败原因在返回 JSON 里, 面板直接展示)
        let req_id = json_get_i64(line, "reqId").unwrap_or(0);
        let json = glaspen_core::export::nas_backup::backup_now();
        // 同样要 \n 结尾(面板按行分帧), 否则 NAS 备份完成的响应也收不到。
        let resp = format!(
            "{{\"type\":\"nasBackup_response\",\"reqId\":{},\"data\":{}}}\n",
            req_id, json
        );
        let _ = writer.write_all(resp.as_bytes());
        let _ = writer.flush();
    } else if msg_type == "canvasOverview" || msg_type == "canvasNew" {
        // 无限画布总览 tab:镜头动作(home/center/new)+ 包围盒总览 PNG。
        // 动作发给消息循环线程应用;总览渲染在管道线程(只读 STROKES/镜头)。
        let req_id = json_get_i64(line, "reqId").unwrap_or(0);
        // 面板切 tab 与动作是异步消息: 动作必须在无限画布模式下执行。
        // 否则「新建画布」会走到活页本分支把当前页清掉(数据事故)。
        if !infinite_on() {
            post_tray_command(hwnd, CMD_TOGGLE_INFINITE_CANVAS, 1);
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
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
        // data 内容是 ""png":"...","rect":[...]" (不带外层花括号) —— 这里必须
        // 补上一对花括号, 否则面板 jsonDecode 直接失败, 总览永远空白。
        let _ = writer.write_all(
            format!(
                "{{\"type\":\"canvasOverview_response\",\"reqId\":{},\"data\":{{{data}}}}}\n",
                req_id
            )
            .as_bytes(),
        );
        let _ = writer.flush();
    } else if msg_type == "uiConfig" {
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
    } else if msg_type == "getSettings" {
        // D12: 面板设置快照单源在 core settings_snapshot —— 字段集合/顺序/类型/
        // 缺省值/DB 回退都在那里; 覆盖层只提供运行时专属值(launchAtLogin /
        // shortcuts, 见 settings_snapshot_overrides)。帧形状与旧实现一致:
        // {"type":"getSettings_response","data":{...}}\n。
        let overrides = settings_snapshot_overrides();
        match glaspen_core::settings_snapshot::snapshot_json(
            glaspen_core::ui::Platform::Windows,
            None,
            &overrides,
        ) {
            Ok(data) => {
                let resp =
                    format!("{{\"type\":\"getSettings_response\",\"data\":{data}}}\n");
                let _ = writer.write_all(resp.as_bytes());
                let _ = writer.flush();
            }
            Err(e) => {
                // 旧实现永远能拼出 JSON; core 报错只可能是 adapter 的 overrides
                // 坏了 —— 记录并回协议错误帧, 不交付半份快照。
                tracing::warn!("getSettings 快照构建失败: {e}");
                let _ = writer.write_all(
                    glaspen_core::protocol::error_line(0, "getSettings", &e.to_string()).as_bytes(),
                );
                let _ = writer.flush();
            }
        }
    } else if msg_type == "setSetting" {
        // D1:设置 apply 单源。core settings_apply::apply_setting 负责宽容解析 +
        // 区间钳制 + 落库(规范 DB 键);本分支只做 Windows 运行时副作用
        // (PostMessage 到消息循环 / 状态原子量 / 热键注册 / auth 重载)。
        let req: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("setSetting 请求不是合法 JSON: {e}");
                let _ = writer.write_all(
                    glaspen_core::protocol::error_line(0, "setSetting", "请求不是合法 JSON").as_bytes(),
                );
                let _ = writer.flush();
                return;
            }
        };
        let key = req
            .get("key")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let req_id = req.get("reqId").and_then(|v| v.as_i64()).unwrap_or(0);

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
            let mut value = req.get("value").cloned().unwrap_or(serde_json::Value::Null);
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
            if unsafe { PeekNamedPipe(handle, None, 0, None, Some(&mut avail), None) }.is_ok()
                && avail > 0
            {
                let mut buf = vec![0u8; avail as usize];
                // PeekNamedPipe 已确认有数据,同步读立即返回
                if let Ok(n) = writer.read(&mut buf) {
                    let s = String::from_utf8_lossy(&buf[..n]);
                    if s.contains("cancelDownload") {
                        return false;
                    }
                }
            }
            true
        });
        match result {
            Ok((path, received, total)) => {
                tracing::info!("更新包已下载: {}", path.display());
                *DOWNLOADED_INSTALLER.lock().unwrap_or_else(|e| e.into_inner()) = Some(path.clone());
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
        tracing::info!("退出并启动更新安装器");
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
