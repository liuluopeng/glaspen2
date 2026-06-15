use tray_icon::menu::{Menu, MenuItem, Submenu, PredefinedMenuItem, MenuEvent};
use tray_icon::{TrayIconBuilder, Icon};

use super::overlay;

// Menu item ID constants
const COLOR_ID_PREFIX: &str = "color_";
const WIDTH_ID_PREFIX: &str = "width_";
const ID_SAVE_BG: &str = "save_bg";
const ID_SAVE_DRAWING: &str = "save_drawing";
const ID_SAVE_XOJ: &str = "save_xoj";
const ID_CLEAR: &str = "clear";
const ID_RAINBOW: &str = "rainbow";
const ID_OUTLINE: &str = "outline";
const ID_INVERSE: &str = "inverse";
const ID_TOGGLE: &str = "toggle";
const ID_LANG: &str = "lang";
const ID_QUIT: &str = "quit";

// Raw pointers to menu items for live updates (all on main thread)
static mut TOGGLE_ITEM_PTR: usize = 0;
static mut LANG_ITEM_PTR: usize = 0;

/// Run the system tray on the current thread. Blocks until Quit is clicked.
pub fn run(hwnd: isize) {
    // Store hwnd for sending commands to overlay
    {
        let mut h = overlay::OVERLAY_HWND.lock().unwrap();
        if *h == 0 {
            *h = hwnd;
        }
    }

    // Build color submenu
    let color_menu = Submenu::new("颜色", true);
    for i in 0..overlay::COLOR_PRESETS.len() {
        let item = MenuItem::with_id(
            format!("{}{}", COLOR_ID_PREFIX, i),
            overlay::COLOR_NAMES_ZH[i],
            true,
            None,
        );
        color_menu.append(&item).ok();
    }

    // Build width submenu
    let width_menu = Submenu::new("线宽", true);
    for i in 0..overlay::WIDTH_PRESETS.len() {
        let item = MenuItem::with_id(
            format!("{}{}", WIDTH_ID_PREFIX, i),
            overlay::WIDTH_NAMES_ZH[i],
            true,
            None,
        );
        width_menu.append(&item).ok();
    }

    // Build main menu
    let menu = Menu::new();
    menu.append(&color_menu).ok();
    menu.append(&width_menu).ok();
    menu.append(&PredefinedMenuItem::separator()).ok();
    menu.append(&MenuItem::with_id(ID_SAVE_BG, "保存(含背景)", true, None)).ok();
    menu.append(&MenuItem::with_id(ID_SAVE_DRAWING, "保存(涂鸦)", true, None)).ok();
    menu.append(&MenuItem::with_id(ID_SAVE_XOJ, "保存笔记 (Xournal)", true, None)).ok();
    menu.append(&MenuItem::with_id(ID_CLEAR, "清屏", true, None)).ok();
    menu.append(&MenuItem::with_id(ID_RAINBOW, "彩虹指示器", true, None)).ok();
    menu.append(&MenuItem::with_id(ID_OUTLINE, "描边增强", true, None)).ok();
    menu.append(&MenuItem::with_id(ID_INVERSE, "反色模式", true, None)).ok();
    menu.append(&PredefinedMenuItem::separator()).ok();

    let toggle_item = MenuItem::with_id(ID_TOGGLE, "开启涂鸦", true, None);
    menu.append(&toggle_item).ok();

    let lang_item = MenuItem::with_id(ID_LANG, "English", true, None);
    menu.append(&lang_item).ok();

    menu.append(&PredefinedMenuItem::separator()).ok();
    menu.append(&MenuItem::with_id(ID_QUIT, "退出", true, None)).ok();

    // Store pointers for updates (safe: all on main thread)
    unsafe {
        TOGGLE_ITEM_PTR = &toggle_item as *const _ as usize;
        LANG_ITEM_PTR = &lang_item as *const _ as usize;
    }

    let icon = create_icon(1.0, 0.0, 0.0, true);
    let _tray = TrayIconBuilder::new()
        .with_tooltip("glaspen2")
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .build()
        .expect("Failed to create tray icon");

    // Keep tray alive
    let _tray = Box::leak(Box::new(_tray));

    // Event loop
    let receiver = MenuEvent::receiver();
    loop {
        match receiver.recv_timeout(std::time::Duration::from_millis(500)) {
            Ok(event) => {
                if handle_menu_id(event.id().0.as_str()) {
                    break;
                }
            }
            Err(_) => {
                // Timeout or channel closed — check if overlay window is still alive
                let alive = unsafe {
                    use windows::Win32::UI::WindowsAndMessaging::IsWindow;
                    use windows::Win32::Foundation::HWND;
                    IsWindow(HWND(hwnd as *mut _))
                }.0 != 0;
                if !alive {
                    break;
                }
            }
        }
    }
}

fn create_icon(r: f64, g: f64, b: f64, filled: bool) -> Icon {
    let size: u32 = 16;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let ir = (r * 255.0) as u8;
    let ig = (g * 255.0) as u8;
    let ib = (b * 255.0) as u8;

    let cx = 8.0f64;
    let cy = 8.0f64;
    let radius = 6.0f64;
    let radius_inner = if filled { 0.0 } else { 4.0 };

    for py in 0..size as i32 {
        for px in 0..size as i32 {
            let dx = px as f64 - cx + 0.5;
            let dy = py as f64 - cy + 0.5;
            let dist = (dx * dx + dy * dy).sqrt();
            let offset = ((py as u32 * size + px as u32) * 4) as usize;

            if dist <= radius && dist >= radius_inner {
                rgba[offset] = ir;
                rgba[offset + 1] = ig;
                rgba[offset + 2] = ib;
                rgba[offset + 3] = 255;
            } else if !filled && dist <= radius && dist >= radius - 1.5 {
                rgba[offset] = 128;
                rgba[offset + 1] = 128;
                rgba[offset + 2] = 128;
                rgba[offset + 3] = 200;
            }
        }
    }

    Icon::from_rgba(rgba, size, size).expect("Failed to create icon")
}

fn handle_menu_id(id: &str) -> bool {
    if id == ID_QUIT {
        send_command(overlay::CMD_QUIT, 0);
        return true;
    }

    if let Some(idx) = id.strip_prefix(COLOR_ID_PREFIX) {
        if let Ok(i) = idx.parse::<usize>() {
            send_command(overlay::CMD_SELECT_COLOR + i, 0);
        }
        return false;
    }

    if let Some(idx) = id.strip_prefix(WIDTH_ID_PREFIX) {
        if let Ok(i) = idx.parse::<usize>() {
            send_command(overlay::CMD_SELECT_WIDTH + i, 0);
        }
        return false;
    }

    match id {
        x if x == ID_SAVE_BG => send_command(overlay::CMD_SAVE_WITH_BG, 0),
        x if x == ID_SAVE_DRAWING => send_command(overlay::CMD_SAVE_DRAWING, 0),
        x if x == ID_SAVE_XOJ => send_command(overlay::CMD_SAVE_XOJ, 0),
        x if x == ID_CLEAR => send_command(overlay::CMD_CLEAR_SCREEN, 0),
        x if x == ID_RAINBOW => send_command(overlay::CMD_TOGGLE_RAINBOW, 0),
        x if x == ID_OUTLINE => send_command(overlay::CMD_TOGGLE_OUTLINE, 0),
        x if x == ID_INVERSE => send_command(overlay::CMD_TOGGLE_INVERSE, 0),
        x if x == ID_TOGGLE => send_command(overlay::CMD_TOGGLE_ENABLED, 0),
        x if x == ID_LANG => send_command(overlay::CMD_TOGGLE_LANG, 0),
        _ => {}
    }
    false
}

fn send_command(cmd: usize, param: usize) {
    let hwnd = *overlay::OVERLAY_HWND.lock().unwrap();
    if hwnd != 0 {
        unsafe {
            use windows::Win32::Foundation::*;
            use windows::Win32::UI::WindowsAndMessaging::*;
            PostMessageW(
                HWND(hwnd as *mut _),
                overlay::WM_TRAY_COMMAND,
                WPARAM(cmd),
                LPARAM(param as isize),
            ).ok();
        }
    }
}

// --- Public update functions ---

pub fn update_tray_icon(_state: &overlay::DrawState) {
    // tray-icon doesn't support updating icon after creation in all versions
    // This would require recreating the tray icon
}

pub fn update_enabled_item(enabled: bool, lang: i32) {
    unsafe {
        if TOGGLE_ITEM_PTR != 0 {
            let item = &*(TOGGLE_ITEM_PTR as *const MenuItem);
            let text = if enabled {
                if lang == 0 { "关闭涂鸦" } else { "Disable Drawing" }
            } else {
                if lang == 0 { "开启涂鸦" } else { "Enable Drawing" }
            };
            item.set_text(text);
        }
    }
}

pub fn update_rainbow_checkmark(_show: bool) {}
pub fn update_outline_checkmark(_show: bool) {}
pub fn update_inverse_checkmark(_show: bool) {}

pub fn update_menu_texts(_state: &overlay::DrawState) {
    // Full menu text update would require storing all item pointers
    // For now, only the toggle and lang items are updatable
}
