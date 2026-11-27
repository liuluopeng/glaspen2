// Windows 托盘图标 + 右键菜单。
//
// 菜单内容/顺序/勾选/文案来自 core 的共享 platform_menu(与 macOS 菜单同源)，
// 这里只做 Win32 渲染：Shell_NotifyIconW + CreatePopupMenu/TrackPopupMenu，
// 选中的命令仍投给 handle_command（与设置管道/热键同一条命令总线）。

use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};

static TRAY_ICON_INSTALLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
const TRAY_ICON_ID: u32 = 1;

fn wide_z(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn install_tray_icon(hwnd: HWND) {
    if TRAY_ICON_INSTALLED.load(Ordering::SeqCst) {
        return;
    }
    let hinst: HINSTANCE = unsafe { GetModuleHandleW(None).unwrap_or_default().into() };
    // 优先可执行文件内嵌的应用图标(id=1), 取不到再用系统默认图标。
    let icon = unsafe { LoadIconW(Some(hinst), PCWSTR(1 as _)) }
        .or_else(|_| unsafe { LoadIconW(None, IDI_APPLICATION) })
        .unwrap_or_default();
    let mut nid = NOTIFYICONDATAW::default();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ICON_ID;
    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    nid.uCallbackMessage = WM_TRAY_ICON;
    nid.hIcon = icon;
    let tip = wide_z("glaspen2");
    let n = tip.len().min(nid.szTip.len());
    nid.szTip[..n].copy_from_slice(&tip[..n]);
    let ok = unsafe { Shell_NotifyIconW(NIM_ADD, &nid).as_bool() };
    if ok {
        TRAY_ICON_INSTALLED.store(true, Ordering::SeqCst);
        tracing::info!("Windows 托盘图标已创建");
    } else {
        tracing::warn!("Windows 托盘图标创建失败");
    }
}

pub fn remove_tray_icon(hwnd: HWND) {
    if !TRAY_ICON_INSTALLED.swap(false, Ordering::SeqCst) {
        return;
    }
    let mut nid = NOTIFYICONDATAW::default();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ICON_ID;
    let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &nid) };
}

fn menu_command(action: glaspen_core::platform_menu::MenuAction) -> usize {
    use glaspen_core::platform_menu::MenuAction as A;
    match action {
        A::SelectColor(i) => CMD_SELECT_COLOR + i,
        A::SelectWidth(i) => CMD_SELECT_WIDTH + i,
        A::ToggleDraw => CMD_TOGGLE_ENABLED,
        A::Undo => CMD_UNDO,
        A::Clear => CMD_CLEAR_SCREEN,
        A::ToggleGrid => CMD_TOGGLE_GRID,
        A::ToggleGlass => CMD_TOGGLE_FROSTED,
        A::ToggleOutline => CMD_TOGGLE_OUTLINE,
        A::ToggleEthereal => CMD_TOGGLE_ETHEREAL,
        A::ToggleInfinite => CMD_TOGGLE_INFINITE_CANVAS,
        A::ToggleMinimap => CMD_TOGGLE_MINIMAP,
        A::TogglePressureMonitor => CMD_TOGGLE_PRESSURE_MONITOR,
        A::ToggleChatIntegration => CMD_TOGGLE_CHAT_INTEGRATION,
        A::SaveWithBackground => CMD_SAVE_WITH_BG,
        A::SaveDrawing => CMD_SAVE_DRAWING,
        A::SaveXoj => CMD_SAVE_XOJ,
        A::ExportSvgGif => CMD_EXPORT_SVG_GIF,
        A::OpenSettings => CMD_OPEN_SETTINGS,
        A::Quit => CMD_QUIT,
    }
}

fn append_menu_entries(
    menu: HMENU,
    entries: &[glaspen_core::platform_menu::MenuEntry],
) {
    for entry in entries {
        if entry.is_separator() {
            let _ = unsafe { AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()) };
            continue;
        }
        if !entry.children.is_empty() {
            let Ok(submenu) = (unsafe { CreatePopupMenu() }) else {
                continue;
            };
            append_menu_entries(submenu, &entry.children);
            let label = wide_z(&entry.label);
            let _ = unsafe {
                AppendMenuW(
                    menu,
                    MF_POPUP,
                    submenu.0 as usize,
                    PCWSTR(label.as_ptr()),
                )
            };
            continue;
        }
        let Some(action) = entry.action else {
            continue;
        };
        let flags = if entry.checked { MF_CHECKED } else { MENU_ITEM_FLAGS(0) };
        let label = wide_z(&entry.label);
        let _ = unsafe {
            AppendMenuW(
                menu,
                flags,
                menu_command(action),
                PCWSTR(label.as_ptr()),
            )
        };
    }
}

fn tray_menu_state(state: &OverlayState) -> glaspen_core::platform_menu::MenuState {
    glaspen_core::platform_menu::MenuState {
        draw_enabled: state.draw.enabled,
        grid: state.draw.show_grid,
        glass: state.draw.frosted,
        outline: state.draw.outline_enabled,
        ethereal: state.draw.ethereal,
        infinite: infinite_on(),
        minimap: MINIMAP_ENABLED.load(Ordering::SeqCst),
        pressure_monitor: state.draw.pressure_monitor,
        chat_integration: state.draw.chat_integration,
        color_index: state.draw.selected_color,
        width_index: state.draw.selected_width,
    }
}

fn show_tray_menu(hwnd: HWND, state: &mut OverlayState) {
    use glaspen_core::i18n::Lang;
    let menu_state = tray_menu_state(state);
    let entries = glaspen_core::platform_menu::build(Lang::Zh, &menu_state);
    let Ok(menu) = (unsafe { CreatePopupMenu() }) else {
        return;
    };
    append_menu_entries(menu, &entries);

    let mut pt = POINT::default();
    let _ = unsafe { GetCursorPos(&mut pt) };
    let _ = unsafe { SetForegroundWindow(hwnd) };
    let flags = TRACK_POPUP_MENU_FLAGS(TPM_RETURNCMD.0 | TPM_RIGHTBUTTON.0);
    let cmd = unsafe {
        TrackPopupMenu(menu, flags, pt.x, pt.y, None, hwnd, None)
    }
    .0 as usize;
    let _ = unsafe { DestroyMenu(menu) };
    // 菜单必须回到窗口消息循环；否则点空白处菜单不会消失。
    let _ = unsafe { PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0)) };
    if cmd != 0 {
        // handle_command 的开关类命令把 0/1 当显式布尔值; 菜单点击必须是
        // "翻转"(usize::MAX), 否则点一次只会设成 false。颜色/粗细/保存等
        // 没有翻转语义的命令仍传 0。
        let param = if matches!(
            cmd,
            CMD_TOGGLE_ENABLED
                | CMD_TOGGLE_GRID
                | CMD_TOGGLE_FROSTED
                | CMD_TOGGLE_OUTLINE
                | CMD_TOGGLE_ETHEREAL
                | CMD_TOGGLE_INFINITE_CANVAS
                | CMD_TOGGLE_MINIMAP
                | CMD_TOGGLE_PRESSURE_MONITOR
                | CMD_TOGGLE_CHAT_INTEGRATION
        ) {
            usize::MAX
        } else {
            0
        };
        handle_command(state, cmd, param);
    }
}
