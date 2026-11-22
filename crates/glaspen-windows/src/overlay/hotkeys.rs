// ── 可配置快捷键(对齐 macOS v0.6.0) ─────────────────────────────────
//
// chord 规范与 macOS 完全一致: cmd+ctrl+alt+shift+<Key>, 顺序固定, 至少含
// ctrl 或 alt; 空串 = 该动作未绑定。Windows 上 cmd 令牌 = Win 键。
// **动作 id / Windows 默认键 / 保留组合 / 强制绑定动作的唯一来源是
// `glaspen_core::ui`**(与 macOS 壳、Flutter 面板共用同一份 schema); 本文件
// 只保留历史迁移表(旧版本默认值), 不再维护第二份现行默认键表。
// 实体键与 macOS 不同: 核心用 Ctrl+Alt, 镜头用 Win+Alt, 不照搬 ⌘⌃。
//
// 实现差异: macOS 用 CGEventTap 自己匹配 chord; Windows 用 RegisterHotKey
// 让系统匹配(不挂低级键盘钩子 —— 钩子会同步卡系统输入管线)。因此:
//   - 录制快捷键期间必须把热键全部注销, 否则面板按下的组合先被覆盖层吃掉;
//   - 被系统/其它 App 占用的组合 RegisterHotKey 会失败, 只记 warn 不阻断。
//
// 线程: 注册表用互斥量保护; RegisterHotKey/UnregisterHotKey 只要有 hwnd
// 就能从任意线程调用(WM_HOTKEY 投递到窗口线程), 所以设置管道线程改键
// 直接改, 不用 PostMessage 中转。必须要求"与 macOS 同 id 同语义":
// 面板保存的是 action → chord 的完整表。

use glaspen_core::action_effect::{effect_for_action, ActionEffect, CameraDir, ZoomDir};
use glaspen_core::ui;

/// 本壳只跑 Windows; core 的 UI schema 按平台分列。
const UI_PLATFORM: ui::Platform = ui::Platform::Windows;

/// 生效绑定表的动作顺序 + Windows 默认 chord。
///
/// 动作清单来自 `ui::ACTIONS`(顺序 = 面板显示顺序), 默认值来自
/// `ui::default_chord(Windows, action)` —— 加动作/改默认键只改 core, 本文件
/// 不再维护第二份默认键表。
fn hotkey_actions() -> Vec<(&'static str, &'static str)> {
    ui::ACTIONS
        .iter()
        .map(|a| {
            let default = ui::default_chord(UI_PLATFORM, a.id).unwrap_or("");
            (a.id, default)
        })
        .collect()
}

/// 可配置动作的热键 id 基址(id = 基址 + 动作表下标)。
const HOTKEY_ID_ACTION_BASE: i32 = 100;
/// 固定热键(不进可配置表, 与 macOS 的"保留组合"对应): 三个按住类。
/// 退出已迁入可配置动作表(见 core `ui::ACTIONS` 的 "quit"), 不再是固定 id。
const HOTKEY_ID_GIF: i32 = 16;
const HOTKEY_ID_DRAFT: i32 = 17;
const HOTKEY_ID_SEND: i32 = 18;

/// Windows 保留组合: 按住类(录 GIF / 手写草稿 / 手写直发)。
/// 唯一来源是 `ui::reserved(Platform::Windows)`(与 macOS 的保留表同源),
/// 本文件不再维护副本; 用户不能用这些组合绑普通动作。
/// 退出默认键 Ctrl+Alt+Q 不在此列 —— 它属于「彻底退出」动作本身, 可以改键。

/// "默认键位方案"版本(user_settings 里的标记): 旧库没有这个键 = 1。
/// 2 = 主键(字母)与 macOS 对齐的第一版(个别键额外 Shift);
/// 3 = 与 macOS 字母完全一致: 核心 Ctrl+Alt+同字母, 镜头整套 Ctrl+Alt+Shift;
/// 4 = 全动作默认都有键: 镜头 Win+Alt+WASD/XZ(占用时降级 Alt+Shift), 其余 Ctrl+Alt 字母。
/// 5 = 镜头键"整套降级"版: 把 v4 可能已写进库的半套 Alt+Shift 镜头值归一回 Win+Alt,
///     启动时若主组合被占用再由注册逻辑整套降级(保证 A/S 与 W/D 修饰键一致)。
/// 6 = 降级组合改用 Ctrl+Alt+Shift(不再用会触发输入法语言切换的 Alt+Shift);
///     (该方案因 4 键难按在 v8 回退)
///     把 v5 落库的 Alt+Shift 镜头值同样归一回 Win+Alt。
/// 7 = 换粗细默认从 ⇧W 改成 ⇧T(⇧W 要让给镜头降级 Ctrl+Alt+Shift+W)。
/// 8 = 镜头键回到 Win+Alt(不再整套 Ctrl+Alt+Shift); 个别键被占时**单键**降级
///     Alt+Shift+同键(不做 4 键组合)。v7 落库的镜头值归一回 Win+Alt 重新决定。
/// 迁移只做一次 —— 用户之后自己改回旧组合, 不会在每次启动被反复改写。
const SHORTCUTS_DEFAULTS_VERSION_KEY: &str = "shortcuts_defaults_v";
const SHORTCUTS_DEFAULTS_VERSION: i32 = 8;

/// 规范化 chord: 规则/按键名表/错误分支的唯一实现在 `glaspen_core::chord`(D2)。
/// 空串 = 未绑定; 合法返回固定顺序 cmd→ctrl→alt→shift→Key; 非法返回 None。
fn normalize_chord(raw: &str) -> Option<String> {
    glaspen_core::chord::normalize(raw)
}

/// 按键名 → Win32 虚拟键码(RegisterHotKey 用)。字母/数字用 ASCII 惯用值,
/// 其余给显式数值(与 winuser.h 的 VK_* 一致), 免得为每个键都引一个常量。
fn vk_for_key(name: &str) -> Option<u32> {
    let b = name.as_bytes();
    if b.len() == 1 {
        let c = b[0];
        if c.is_ascii_uppercase() {
            return Some(c as u32); // VK_A..VK_Z = 'A'..'Z'
        }
        if c.is_ascii_digit() {
            return Some(c as u32); // VK_0..VK_9 = '0'..'9'
        }
    }
    if let Some(n) = name.strip_prefix('F')
        && let Ok(n) = n.parse::<u32>()
        && (1..=12).contains(&n)
    {
        return Some(0x70 + n - 1); // VK_F1 = 112
    }
    let vk = match name {
        "-" => 0xBD,       // VK_OEM_MINUS
        "=" => 0xBB,       // VK_OEM_PLUS
        "[" => 0xDB,       // VK_OEM_4
        "]" => 0xDD,       // VK_OEM_6
        "\\" => 0xDC,      // VK_OEM_5
        ";" => 0xBA,       // VK_OEM_1
        "'" => 0xDE,       // VK_OEM_7
        "`" => 0xC0,       // VK_OEM_3
        "," => 0xBC,       // VK_OEM_COMMA
        "." => 0xBE,       // VK_OEM_PERIOD
        "/" => 0xBF,       // VK_OEM_2
        "Left" => 0x25,    // VK_LEFT
        "Up" => 0x26,      // VK_UP
        "Right" => 0x27,   // VK_RIGHT
        "Down" => 0x28,    // VK_DOWN
        "PageUp" => 0x21,  // VK_PRIOR
        "PageDown" => 0x22, // VK_NEXT
        "Home" => 0x24,    // VK_HOME
        "End" => 0x23,     // VK_END
        "Delete" => 0x2E,  // VK_DELETE
        "Backspace" => 0x08, // VK_BACK
        "Return" => 0x0D,  // VK_RETURN
        "Escape" => 0x1B,  // VK_ESCAPE
        "Tab" => 0x09,     // VK_TAB
        "Space" => 0x20,   // VK_SPACE
        _ => return None,
    };
    Some(vk)
}

/// chord → (MOD_* 掩码, VK)。空串/非法返回 None。
fn parse_chord(chord: &str) -> Option<(u32, u32)> {
    let norm = normalize_chord(chord)?;
    if norm.is_empty() {
        return None;
    }
    let parts: Vec<&str> = norm.split('+').collect();
    let key = parts.last()?;
    let mut mods = 0u32;
    for m in &parts[..parts.len() - 1] {
        mods |= match *m {
            "cmd" => MOD_WIN.0 as u32,
            "ctrl" => MOD_CONTROL.0 as u32,
            "alt" => MOD_ALT.0 as u32,
            "shift" => MOD_SHIFT.0 as u32,
            _ => return None,
        };
    }
    Some((mods, vk_for_key(key)?))
}

/// 该组合是否被系统/按住类功能保留(唯一来源: core 的 `ui::reserved`)。
fn is_reserved(chord: &str) -> bool {
    ui::reserved(UI_PLATFORM).contains(&chord)
}

#[derive(Default)]
struct HotkeyRegistry {
    hwnd: isize,
    /// action id → 生效 chord(已补默认/去冲突; 空串 = 未绑定)
    effective: Vec<(&'static str, String)>,
    /// 已成功注册的可配置热键 id
    registered: Vec<i32>,
    /// 固定热键(退出 + 按住类)是否已注册
    fixed_registered: bool,
    /// 录制快捷键期间暂停全部热键
    paused: bool,
    /// 录制暂停的代数(60s 兜底定时器用: 代数变了说明已恢复)
    pause_gen: u64,
    /// 涂鸦已停用: 除「启停涂鸦 / 自由涂鸦 / 打开设置 / 彻底退出」外的动作
    /// 热键全部注销, 键位透传给前台应用(macOS event tap 只在 tap 里"放行",
    /// Windows 的 RegisterHotKey 会吞键, 只能靠反注册复刻同一体验)。
    draw_disabled: bool,
    /// 是否处于无限画布模式: 镜头类动作只在无限画布下注册。
    /// macOS 在翻页模式下对 canvas_* 组合是"放行给系统", 而 RegisterHotKey
    /// 注册即吞键(会顶掉浏览器 Ctrl+Alt+PgUp 切 tab 之类), 所以非无限画布
    /// 时不注册这些键。
    canvas_mode: bool,
    /// 手写消息集成总开关: 关时 Ctrl+Alt+2/3 不注册(直通前台)
    chat_integration: bool,
}

static HOTKEYS: std::sync::LazyLock<std::sync::Mutex<HotkeyRegistry>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(HotkeyRegistry::default()));

fn lock_registry() -> std::sync::MutexGuard<'static, HotkeyRegistry> {
    HOTKEYS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Windows 上没有菜单栏/托盘兜底、必须始终可用的动作。
/// 唯一来源: core 的 `ui::force_bound`(当前 = settings / quit)。
fn force_bound(action: &str) -> bool {
    ui::force_bound(UI_PLATFORM).contains(&action)
}

/// 旧 Windows 默认 → 新默认(与 macOS `gl_shortcut_migrate_saved` 同思路):
/// 只有存档值**仍等于某个旧默认**时才迁移, 用户自定义组合不动。
///
/// 版本沿革(一个动作可能有多个旧默认, 都要能迁到最终值):
///   v1: 镜头=方向键/PgUp·PgDn, 设置/退出无 Shift;
///   v2: 镜头 WASD/XZ, 设置 ⇧D、缩小 ⇧Z(个别键额外 Shift);
///   v3: 字母与 macOS 完全一致 —— 核心 Ctrl+Alt+同字母, 镜头整套 Ctrl+Alt+Shift;
///   v4: 镜头 Win+Alt+WASD/XZ, 其余动作全部补上 Ctrl+Alt 字母默认。
///
/// 对整张存档表做旧默认迁移, 返回迁移条数(启动时调用一次, 见 init)。
/// 历史默认表与"只迁仍等于旧默认的项"规则单源在 `core::chord`(D2)。
fn migrate_legacy_defaults(saved: &mut HashMap<String, String>) -> usize {
    let mut changed = 0;
    for (action, raw) in saved.iter_mut() {
        let Some(new) = glaspen_core::chord::migrate_value(UI_PLATFORM, action, raw) else {
            continue;
        };
        tracing::info!("[hotkey] 旧默认 {action}={raw} 迁移为 {new}");
        *raw = new;
        changed += 1;
    }
    changed
}

/// 把存档表合并成生效表(与 macOS gl_shortcuts_apply 同规则):
/// 缺失/非法 → 默认; 保留组合 → 默认; 同组合后者解绑。
fn build_effective(saved: &HashMap<String, String>) -> Vec<(&'static str, String)> {
    let mut out: Vec<(&'static str, String)> = Vec::new();
    let mut used: Vec<String> = Vec::new();
    for (action, default) in hotkey_actions() {
        let mut chord = match saved.get(action) {
            Some(raw) => {
                let norm = normalize_chord(raw);
                if norm.is_none() && !raw.trim().is_empty() {
                    tracing::warn!("[hotkey] 非法组合 {action}={raw}, 回退默认");
                }
                norm.filter(|c| !c.is_empty()).unwrap_or_else(|| {
                    // 缺失/非法/空串都按"回退默认"处理: 只有面板显式保存的空串
                    // 才代表"用户解绑", 这里用 save 的语义区分不了, 交给
                    // build_effective_saved 的"空串=解绑"分支。
                    default.to_string()
                })
            }
            None => default.to_string(),
        };
        if !chord.is_empty() && is_reserved(&chord) {
            tracing::warn!("[hotkey] 保留组合 {action}={chord}, 回退默认");
            chord = default.to_string();
        }
        if !chord.is_empty() && used.contains(&chord) {
            tracing::warn!("[hotkey] 冲突 {chord} → {action} 解绑(本表里已有动作占用)");
            chord.clear();
        }
        // Windows 没有菜单栏/托盘: 「打开设置」是重开面板的唯一入口,
        // 「彻底退出」是退出进程的唯一入口(任务管理器除外)。允许解绑 = 用户
        // 可能把自己锁在外面, 所以这两个动作强制不可解绑。
        if force_bound(action) && chord.is_empty() {
            tracing::warn!("[hotkey] {action} 动作在 Windows 不允许解绑, 回退默认");
            chord = default.to_string();
            // 回退后的默认组合若已被前面的动作占用(手改库/跨平台恢复), 强制绑定
            // 动作优先: 让前面那个解绑, 否则两个动作抢同一组合, 注册时必有一个失败。
            if let Some((prev_action, prev_chord)) =
                out.iter_mut().find(|(_, c)| c == &chord)
            {
                tracing::warn!(
                    "[hotkey] {chord} 让位给强制绑定动作 {action}, {prev_action} 解绑"
                );
                prev_chord.clear();
                used.retain(|c| c != &chord);
            }
        }
        if !chord.is_empty() {
            used.push(chord.clone());
        }
        out.push((action, chord));
    }
    out
}

/// 用户显式解绑(空串)与"缺键 = 默认"要区分: 面板保存的是完整表,
/// 空串代表解绑, 所以 merged 表里出现空串时保持空串。
fn build_effective_saved(saved: &HashMap<String, String>) -> Vec<(&'static str, String)> {
    let mut out = build_effective(saved);
    if saved.is_empty() {
        return out;
    }
    for (action, chord) in out.iter_mut() {
        if force_bound(action) {
            continue; // Windows 上不可解绑(没有菜单栏/托盘的备用入口)
        }
        if let Some(raw) = saved.get(*action)
            && raw.trim().is_empty()
        {
            *chord = String::new();
        }
    }
    out
}

/// 固定热键: R 按住录 GIF 永远注册; 2/3 只在手写消息集成开启时
/// 注册 —— 集成关必须直通前台(AGENTS 铁律: 关 = ⌘⌃2/⌘⌃3 热键直通不劫持),
/// RegisterHotKey 注册即吞键, dispatch 里再判断是来不及的。
/// (退出走可配置动作表的 "quit", 不再在这里重复注册。)
fn register_fixed(hwnd: HWND, chat_integration: bool) {
    let mods = HOT_KEY_MODIFIERS(MOD_CONTROL.0 | MOD_ALT.0);
    let mut keys = vec![(HOTKEY_ID_GIF, b'R' as u32)];
    if chat_integration {
        keys.push((HOTKEY_ID_DRAFT, b'2' as u32));
        keys.push((HOTKEY_ID_SEND, b'3' as u32));
    }
    for (id, vk) in keys {
        if unsafe { RegisterHotKey(Some(hwnd), id, mods, vk) }.is_err() {
            tracing::warn!("[hotkey] 固定热键 id={id} 注册失败(可能被其它程序占用)");
        }
    }
}

fn unregister_fixed(hwnd: HWND) {
    for id in [HOTKEY_ID_GIF, HOTKEY_ID_DRAFT, HOTKEY_ID_SEND] {
        let _ = unsafe { UnregisterHotKey(Some(hwnd), id) };
    }
}

/// 注册失败时的降级组合: 镜头键的 Win+Alt+键 在不少机器上被常驻软件占用
/// (实测 Win+Alt+W/D 会返回 1409), 单独换成 Alt+Shift+同键。
///
/// 只降级失败的那个键, 不做"整套降级": 整套要么得用 Ctrl+Alt+Shift(4 个键,
/// 手按不动), 要么全用 Alt+Shift(按住会切输入法)。鼠标手势另用 Win+Alt 掩码,
/// 不依赖被降级的键, 所以不会因为 W/D 降级而把语言切走。
fn registration_fallback(action: &str, chord: &str) -> Option<String> {
    if !action.starts_with("canvas_") {
        return None;
    }
    let key = chord.strip_prefix("cmd+alt+")?;
    Some(format!("alt+shift+{key}"))
}

/// 注销并重新注册可配置动作的热键(固定热键不动)。
fn register_actions_locked(reg: &mut HotkeyRegistry, hwnd: HWND) {
    for id in reg.registered.drain(..) {
        let _ = unsafe { UnregisterHotKey(Some(hwnd), id) };
    }
    // 拷贝一份再遍历: 降级注册时要改写 reg.effective[idx](面板/日志显示实际值)
    let effective = reg.effective.clone();
    let mut fallback_used = false;
    for (idx, (action, chord)) in effective.iter().enumerate() {
        if chord.is_empty() {
            continue;
        }
        // 涂鸦停用时只有这几个动作仍消费按键(启停/自由涂鸦/开面板/彻底退出),
        // 其余注销后透传给前台应用。退出必须始终可用 —— Windows 没有菜单栏,
        // 按不出来就只能任务管理器强杀。
        if reg.draw_disabled
            && !matches!(
                *action,
                "toggle_draw" | "toggle_free_canvas" | "settings" | "quit"
            )
        {
            continue;
        }
        // 镜头动作只在无限画布模式注册(翻页模式下放行给系统, macOS 同语义)
        if action.starts_with("canvas_") && !reg.canvas_mode {
            continue;
        }
        let Some((mods, vk)) = parse_chord(chord) else {
            tracing::warn!("[hotkey] {action}={chord} 解析失败, 跳过注册");
            continue;
        };
        // 镜头平移/缩放要支持"按住连续"(WM_HOTKEY 自动重复), 其余一次动作
        // 加 MOD_NOREPEAT: 否则按住翻页键会连翻好几页(macOS 用 isRepeat 过滤)。
        let repeatable = action.starts_with("canvas_pan_")
            || matches!(*action, "canvas_zoom_in" | "canvas_zoom_out");
        let flags = if repeatable {
            HOT_KEY_MODIFIERS(mods)
        } else {
            HOT_KEY_MODIFIERS(mods | MOD_NOREPEAT.0 as u32)
        };
        let id = HOTKEY_ID_ACTION_BASE + idx as i32;
        match unsafe { RegisterHotKey(Some(hwnd), id, flags, vk) } {
            Ok(()) => reg.registered.push(id),
            Err(e) => {
                // 镜头键被其它程序占用(Win+Alt+W/D 很常见): 单键降级 Alt+Shift+同键
                let fallback = registration_fallback(action, chord)
                    .filter(|alt| parse_chord(alt).is_some());
                let Some(alt) = fallback else {
                    tracing::warn!(
                        "[hotkey] {action}={chord} 注册失败(被系统/其它程序占用?): {e}"
                    );
                    continue;
                };
                let (mods2, vk2) = parse_chord(&alt).expect("fallback 已在上面校验");
                let flags2 = if repeatable {
                    HOT_KEY_MODIFIERS(mods2)
                } else {
                    HOT_KEY_MODIFIERS(mods2 | MOD_NOREPEAT.0 as u32)
                };
                match unsafe { RegisterHotKey(Some(hwnd), id, flags2, vk2) } {
                    Ok(()) => {
                        tracing::info!(
                            "[hotkey] {action}={chord} 被占用, 单独降级为 {alt}"
                        );
                        reg.effective[idx].1 = alt;
                        reg.registered.push(id);
                        fallback_used = true;
                    }
                    Err(e2) => tracing::warn!(
                        "[hotkey] {action}={chord} 注册失败, 降级 {alt} 也失败: {e2}"
                    ),
                }
            }
        }
    }
    // 生效表(含未绑定)写进日志, 排障时一眼看到底注册了什么
    let summary: Vec<String> = reg
        .effective
        .iter()
        .map(|(a, c)| {
            if c.is_empty() {
                format!("{a}=-")
            } else {
                format!("{a}={c}")
            }
        })
        .collect();
    tracing::info!("[hotkey] 生效快捷键: {}", summary.join(" "));
    if fallback_used {
        // 回推实际生效表: 否则面板"恢复默认/保存"后仍显示默认值, 和真实热键不一致
        let table = effective_json_locked(reg);
        if let Ok(quoted) = serde_json::to_string(&table) {
            push_settings_changed(&format!("\"shortcuts\":{quoted}"));
        }
    }
}

fn effective_json_locked(reg: &HotkeyRegistry) -> String {
    let map: std::collections::BTreeMap<&str, &str> = reg
        .effective
        .iter()
        .map(|(a, c)| (*a, c.as_str()))
        .collect();
    serde_json::to_string(&map).unwrap_or_else(|_| "{}".to_string())
}

/// **只在窗口所属线程**执行热键(重)注册。
/// RegisterHotKey/UnregisterHotKey 对"别的线程的窗口"会直接失败
/// (0x80070580 无效窗口;它属于另一线程 —— v0.6 的管道线程就踩过这个坑),
/// 所以管道线程/定时器线程只改状态, 由它们投递 WM_HOTKEY_RELOAD 到这里。
fn apply_reload(hwnd: HWND) {
    let mut reg = lock_registry();
    if reg.hwnd == 0 {
        reg.hwnd = hwnd.0 as isize;
    }
    if reg.paused {
        // 录制快捷键: 注销全部(含退出/按住类), 让面板能收到任意组合
        for id in reg.registered.drain(..) {
            let _ = unsafe { UnregisterHotKey(Some(hwnd), id) };
        }
        if reg.fixed_registered {
            unregister_fixed(hwnd);
            reg.fixed_registered = false;
        }
        return;
    }
    if !reg.fixed_registered {
        register_fixed(hwnd, reg.chat_integration);
        reg.fixed_registered = true;
    }
    register_actions_locked(&mut reg, hwnd);
}

/// 从任意线程请求在窗口线程重新注册热键(投递自定义消息, 不阻塞调用方)。
fn post_reload(hwnd: HWND) {
    unsafe {
        let _ = PostMessageW(Some(hwnd), WM_HOTKEY_RELOAD, WPARAM(0), LPARAM(0));
    }
}

/// 启动: 从 user_settings 读 "shortcuts" → 合并默认 → 注册。
/// 必须在窗口创建后、run_loop 之前(窗口线程)调用。
pub(crate) fn init(hwnd: HWND, chat_integration: bool) {
    let raw = glaspen_core::runtime()
        .block_on(glaspen_core::db::load_setting("shortcuts"))
        .unwrap_or_default();
    let mut saved: HashMap<String, String> = serde_json::from_str::<serde_json::Value>(&raw)
        .ok()
        .and_then(|v| v.as_object().cloned())
        .map(|o| {
            o.into_iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_string())))
                .collect()
        })
        .unwrap_or_default();
    // 旧默认迁移只做一次(版本标记): 升级前存档里的旧默认键位换成本轮与 macOS
    // 对齐的新默认; 用户自定义的组合不动。
    let defaults_version = glaspen_core::runtime()
        .block_on(glaspen_core::db::load_setting(SHORTCUTS_DEFAULTS_VERSION_KEY))
        .and_then(|v| v.trim().parse::<i32>().ok())
        .unwrap_or(1);
    if defaults_version < SHORTCUTS_DEFAULTS_VERSION {
        let migrated = migrate_legacy_defaults(&mut saved);
        tracing::info!(
            "[hotkey] 默认键位升级到 v{SHORTCUTS_DEFAULTS_VERSION}: 迁移 {migrated} 个旧默认组合"
        );
    }
    let effective = build_effective_saved(&saved);
    {
        let mut reg = lock_registry();
        reg.hwnd = hwnd.0 as isize;
        reg.effective = effective;
        reg.canvas_mode = infinite_on();
        reg.chat_integration = chat_integration;
    } // 先放锁: apply_reload 内部自己取锁
    apply_reload(hwnd);
    // 首次启动即把"补齐默认后的完整表"落库: 面板读取的永远是完整表,
    // 之后缺键(old 存档)也能稳定回退默认(与 macOS settings_json 回显一致)。
    let json = effective_json();
    glaspen_core::runtime().block_on(glaspen_core::db::save_setting("shortcuts", &json));
    if defaults_version < SHORTCUTS_DEFAULTS_VERSION {
        glaspen_core::runtime().block_on(glaspen_core::db::save_setting(
            SHORTCUTS_DEFAULTS_VERSION_KEY,
            &SHORTCUTS_DEFAULTS_VERSION.to_string(),
        ));
    }
}

/// 面板保存快捷键(完整表 action→chord): 合并/校验 → 落库 → 请求重新注册。
/// 可从管道线程调用: 注册动作投递到窗口线程执行。
pub(crate) fn apply_saved_map(hwnd: HWND, saved: &HashMap<String, String>) {
    let effective = build_effective_saved(saved);
    let json = {
        let mut reg = lock_registry();
        if reg.hwnd == 0 {
            reg.hwnd = hwnd.0 as isize;
        }
        reg.effective = effective;
        effective_json_locked(&reg)
    };
    glaspen_core::runtime().block_on(glaspen_core::db::save_setting("shortcuts", &json));
    tracing::info!("[hotkey] 面板已保存新绑定: {json}");
    post_reload(hwnd);
}

/// 当前生效表的 JSON(含默认补齐); 设置面板 getSettings 回显用。
pub(crate) fn effective_json() -> String {
    let reg = lock_registry();
    if reg.effective.is_empty() {
        // 尚未 init(理论上不会): 至少给默认表, 面板不至于空
        let saved = HashMap::new();
        return serde_json::to_string(
            &build_effective_saved(&saved)
                .into_iter()
                .collect::<std::collections::BTreeMap<&str, String>>(),
        )
        .unwrap_or_else(|_| "{}".to_string());
    }
    effective_json_locked(&reg)
}

/// 录制快捷键: 暂停 = 注销全部热键(让面板能收到组合), 恢复 = 重新注册。
/// 可从任意线程调用(状态 + 投递 WM_HOTKEY_RELOAD)。
/// 60s 兜底: 面板异常退出(没发 false)时自动恢复, 不让全局热键永久失效
/// (与 macOS gl_set_hotkey_recording 一致)。
pub(crate) fn set_recording(hwnd: HWND, on: bool) {
    let generation = {
        let mut reg = lock_registry();
        if reg.hwnd == 0 {
            reg.hwnd = hwnd.0 as isize;
        }
        if on == reg.paused {
            return;
        }
        reg.paused = on;
        reg.pause_gen += 1;
        reg.pause_gen
    };
    post_reload(hwnd);
    tracing::info!(
        "[hotkey] {}",
        if on {
            "录制中: 已注销全部全局热键"
        } else {
            "录制结束: 已恢复全局热键"
        }
    );
    if on {
        let hwnd_val = hwnd.0 as isize;
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(60));
            let resume = {
                let mut reg = lock_registry();
                if reg.paused && reg.pause_gen == generation {
                    reg.paused = false;
                    true
                } else {
                    false
                }
            };
            if resume {
                post_reload(HWND(hwnd_val as *mut std::ffi::c_void));
                tracing::warn!("[hotkey] 录制超时 60s, 已自动恢复全局热键");
            }
        });
    }
}

/// 生效表里某动作的 chord → 面板同款可读文本("ctrl+alt+shift+D" → "Ctrl+Alt+Shift+D")。
/// 用于 Windows 启动提示: 没有菜单栏/托盘, 用户只能靠热键呼出设置面板。
pub(crate) fn chord_display(action: &str) -> String {
    let reg = lock_registry();
    let chord = reg
        .effective
        .iter()
        .find(|(a, _)| *a == action)
        .map(|(_, c)| c.as_str())
        .unwrap_or("");
    chord
        .split('+')
        .map(|part| match part {
            "cmd" => "Win".to_string(),
            "ctrl" => "Ctrl".to_string(),
            "alt" => "Alt".to_string(),
            "shift" => "Shift".to_string(),
            other => other.to_string(),
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// 某动作当前生效的 chord 文本(未绑定/未知返回空串)。
/// 鼠标镜头手势用它判断"主组合家族"是不是 Win+Alt。
pub(crate) fn effective_chord(action: &str) -> String {
    let reg = lock_registry();
    reg.effective
        .iter()
        .find(|(a, _)| *a == action)
        .map(|(_, c)| c.clone())
        .unwrap_or_default()
}

/// 某动作当前组合的修饰键掩码(MOD_*); 未绑定/非法返回 0。
/// 鼠标滚轮缩放用它: 改键后鼠标自动跟随(与 macOS gl_shortcut_modifier_mask_for 一致)。
pub(crate) fn modifier_mask_for(action: &str) -> u32 {
    let reg = lock_registry();
    let chord = reg
        .effective
        .iter()
        .find(|(a, _)| *a == action)
        .map(|(_, c)| c.as_str())
        .unwrap_or("");
    parse_chord(chord).map(|(m, _)| m).unwrap_or(0)
}

/// 模式切换(活页本 ↔ 无限画布)后刷新镜头热键注册。
/// 镜头动作只在无限画布模式注册(翻页模式下放行给系统, macOS 同语义)。
pub(crate) fn set_canvas_mode(hwnd: HWND, on: bool) {
    {
        let mut reg = lock_registry();
        if reg.canvas_mode == on {
            return;
        }
        reg.canvas_mode = on;
    }
    post_reload(hwnd);
}

/// 手写消息集成总开关变化: Ctrl+Alt+2/3 注册/注销。
/// 集成关时这两个组合必须直通前台(AGENTS 铁律), 只有反注册能做到。
pub(crate) fn set_chat_integration(hwnd: HWND, on: bool) {
    {
        let mut reg = lock_registry();
        if reg.chat_integration == on {
            return;
        }
        reg.chat_integration = on;
        if reg.fixed_registered {
            unregister_fixed(hwnd);
            reg.fixed_registered = false;
        }
    }
    post_reload(hwnd);
    tracing::info!(
        "[hotkey] 手写消息集成{}: Ctrl+Alt+2/3 {}",
        if on { "开" } else { "关" },
        if on {
            "已注册"
        } else {
            "已注销(直通前台)"
        }
    );
}

/// 涂鸦启停时调用(actions.rs toggle_enabled): 停用后除「启停涂鸦 / 自由/// 涂鸦 / 打开设置」外的动作热键全部注销, 组合键透传给前台应用 —— 复刻
/// macOS event tap「停用涂鸦时只消费那两个动作」的语义(Windows 的
/// RegisterHotKey 注册即吞键, 只能反注册)。
pub(crate) fn on_draw_enabled_changed(hwnd: HWND, enabled: bool) {
    {
        let mut reg = lock_registry();
        if reg.draw_disabled == !enabled {
            return;
        }
        reg.draw_disabled = !enabled;
    }
    // 注册/注销必须在窗口线程做 → 投递
    post_reload(hwnd);
    tracing::info!(
        "[hotkey] 涂鸦{}: {}",
        if enabled { "已启用" } else { "已停用" },
        if enabled {
            "动作热键已全部注册"
        } else {
            "仅保留 启停涂鸦/自由涂鸦/打开设置/彻底退出 四个热键, 其余透传给前台应用"
        }
    );
}

/// 设置面板断开(进程崩溃/被关闭)时调用: 若正在录制, 立即恢复全局热键,
/// 不等 60s 兜底(Windows 面板是独立进程, 崩溃概率高于 macOS 内嵌视图)。
pub(crate) fn resume_if_paused(hwnd: HWND) {
    let paused = lock_registry().paused;
    if paused {
        set_recording(hwnd, false);
        tracing::warn!("[hotkey] 设置面板断开, 已恢复全局热键");
    }
}

/// WM_HOTKEY 分发: 固定按住类 + 可配置动作。
fn dispatch_hotkey(state: &mut OverlayState, id: i32) {
    match id {
        HOTKEY_ID_GIF => gif_record_start(state),
        // 按住类与 macOS 一致受集成总开关守门
        HOTKEY_ID_DRAFT => {
            if state.draw.chat_integration {
                ink_draft_start(state);
            }
        }
        HOTKEY_ID_SEND => {
            if state.draw.chat_integration {
                chat_record_start(state);
            }
        }
        _ => {
            let idx = (id - HOTKEY_ID_ACTION_BASE) as usize;
            let action = ui::ACTIONS.get(idx).map(|a| a.id).unwrap_or("");
            if action.is_empty() {
                return;
            }
            perform_action(state, action);
        }
    }
}

/// 执行一个可配置动作(macOS perform_action 的 Windows 对照实现)。
///
/// D3b: action id → 语义的唯一来源是 core `action_effect::effect_for_action`;
/// 本函数不再按 action 字符串分支, 只 match `ActionEffect` 变体执行 Windows
/// 侧副作用(门控/镜头方向符号/提示文案按平台保留, 与旧字符串链一致)。
/// match 无 catch-all: core 新增变体时这里必须同步, 否则编译失败。
fn perform_action(state: &mut OverlayState, action: &str) {
    let effect = match effect_for_action(action) {
        Ok(effect) => effect,
        Err(err) => {
            // 旧实现静默忽略未识别的 action; 现在记一条日志, 行为等价。
            tracing::warn!("[hotkey] {err}");
            return;
        }
    };
    // 自由涂鸦镜头动作只在无限画布 + 涂鸦开启时生效(旧硬编码同规则)
    if matches!(
        effect,
        ActionEffect::CameraPan(_) | ActionEffect::CameraZoom(_)
    ) && (!infinite_on() || !state.draw.enabled)
    {
        return;
    }
    match effect {
        ActionEffect::Undo => undo_last_stroke(state),
        ActionEffect::PagePrev => navigate_page(state, false),
        ActionEffect::PageNext => navigate_page(state, true),
        ActionEffect::ColorSwap => color_swap(state),
        ActionEffect::WidthSwap => width_swap(state),
        ActionEffect::ToggleDraw => toggle_enabled(state),
        ActionEffect::ToggleFreeCanvas => {
            // 与 macOS 一致: 顺手把面板「自由涂鸦模式」打开, 否则面板没有
            // 自由涂鸦 tab, 模式切换既看不见落点又容易被 tab 逻辑推回去。
            if !infinite_on() {
                let rt = glaspen_core::runtime();
                let shown = rt
                    .block_on(glaspen_core::db::load_setting("show_free_canvas"))
                    .and_then(|v| v.parse::<i32>().ok())
                    .unwrap_or(0)
                    != 0;
                if !shown {
                    rt.block_on(glaspen_core::db::save_setting("show_free_canvas", "1"));
                }
            }
            apply_infinite_canvas(state, !infinite_on(), true);
        }
        // 方向按用户直觉: 按左 = 画面内容向左移动(内容跟着键走), 上下同理。
        // (2026-11: 用户反馈旧方向反直觉, WASD / 鼠标抓手 / 滚轮缩放统一取反。
        //  macOS 用"视野移动"方向, 与本平台符号相反是既有行为, 见 ActionEffect
        //  的 CameraPan 文档。)
        ActionEffect::CameraPan(CameraDir::Left) => pan_step(state, -1.0, 0.0),
        ActionEffect::CameraPan(CameraDir::Right) => pan_step(state, 1.0, 0.0),
        ActionEffect::CameraPan(CameraDir::Up) => pan_step(state, 0.0, -1.0),
        ActionEffect::CameraPan(CameraDir::Down) => pan_step(state, 0.0, 1.0),
        ActionEffect::CameraZoom(ZoomDir::In) => zoom_step(state, 1.15),
        ActionEffect::CameraZoom(ZoomDir::Out) => zoom_step(state, 1.0 / 1.15),
        ActionEffect::Clear => handle_command(state, CMD_CLEAR_SCREEN, usize::MAX),
        ActionEffect::Export => export_svg_gif_clipboard(state),
        ActionEffect::CopySvg => copy_svg_to_clipboard(state),
        ActionEffect::ToggleGlass => toggle_frosted(state),
        ActionEffect::ToggleEthereal => handle_command(state, CMD_TOGGLE_ETHEREAL, usize::MAX),
        ActionEffect::OpenSettings => crate::show_settings_window(),
        // 彻底退出: 与 macOS 菜单「退出」同语义 —— 收尾(写完排队点 +
        // checkpoint WAL, 见 wndproc WM_DESTROY → win_main)后整个进程退出,
        // 并结束设置面板进程(见 lib.rs kill_settings_processes)。
        ActionEffect::Quit => unsafe {
            let _ = DestroyWindow(state.canvas.hwnd);
        },
    }
}


fn pan_step(state: &mut OverlayState, dir_x: f64, dir_y: f64) {
    let (_, _, z) = cam();
    let z = if z > ZOOM_MIN { z } else { 1.0 };
    let step = 80.0 / z;
    canvas_pan_by(state, dir_x * step, dir_y * step);
}

fn zoom_step(state: &mut OverlayState, factor: f64) {
    let (w, h) = (state.canvas.w as f64, state.canvas.h as f64);
    canvas_zoom_at(state, factor, w * 0.5, h * 0.5);
}

/// 换回上一颜色(macOS color_swap 同语义: 无历史时给红/黑另一端)
fn color_swap(state: &mut OverlayState) {
    let cur = state.draw.selected_color;
    let mut prev = state.draw.prev_color;
    if prev >= COLOR_PRESETS.len() || prev == cur {
        prev = if cur == 0 { COLOR_PRESETS.len() - 1 } else { 0 };
    }
    handle_command(state, CMD_SELECT_COLOR + prev, usize::MAX);
    hud_notify(&format!("颜色: {}", COLOR_NAMES_ZH[prev.min(COLOR_NAMES_ZH.len() - 1)]));
}

/// 换回上一粗细(macOS width_swap 同语义: 无历史时给细/粗另一端)
fn width_swap(state: &mut OverlayState) {
    let cur = state.draw.selected_width;
    let mut prev = state.draw.prev_width;
    if prev >= WIDTH_PRESETS.len() || prev == cur {
        prev = if cur <= 3 { 6.min(WIDTH_PRESETS.len() - 1) } else { 2 };
    }
    handle_command(state, CMD_SELECT_WIDTH + prev, usize::MAX);
    hud_notify(&format!("粗细: {}", WIDTH_NAMES_ZH[prev.min(WIDTH_NAMES_ZH.len() - 1)]));
}


#[cfg(test)]
mod tests {
    use super::*;

    fn saved(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    fn chord_of(effective: &[(&'static str, String)], action: &str) -> String {
        effective
            .iter()
            .find(|(a, _)| *a == action)
            .unwrap_or_else(|| panic!("动作表里没有 {action}"))
            .1
            .clone()
    }

    /// v4 默认: **所有动作都有键**。核心/开关 = Ctrl+Alt+字母,
    /// 镜头/缩放 = Win+Alt+W/A/S/D、X/Z。
    #[test]
    fn windows_defaults_cover_all_actions() {
        let effective = build_effective_saved(&HashMap::new());
        let expected = [
            ("undo", "ctrl+alt+Z"),
            ("page_prev", "ctrl+alt+`"),
            ("page_next", "ctrl+alt+1"),
            ("color_swap", "ctrl+alt+shift+C"),
            ("width_swap", "ctrl+alt+T"),
            ("toggle_draw", "ctrl+alt+V"),
            ("toggle_free_canvas", "ctrl+alt+I"),
            ("canvas_pan_left", "cmd+alt+A"),
            ("canvas_pan_right", "cmd+alt+D"),
            ("canvas_pan_up", "cmd+alt+W"),
            ("canvas_pan_down", "cmd+alt+S"),
            ("canvas_zoom_in", "cmd+alt+X"),
            ("canvas_zoom_out", "cmd+alt+Z"),
            ("clear", "ctrl+alt+C"),
            ("export", "ctrl+alt+G"),
            ("copy_svg", "ctrl+alt+S"),
            ("toggle_glass", "ctrl+alt+B"),
            ("toggle_ethereal", "ctrl+alt+X"),
            ("settings", "ctrl+alt+D"),
            ("quit", "ctrl+alt+shift+Q"),
        ];
        for (action, chord) in expected {
            assert_eq!(chord_of(&effective, action), chord, "动作 {action} 默认键");
        }
        assert_eq!(ui::ACTIONS.len(), effective.len());
        // v4: 默认表里不再有"未绑定"的动作
        assert!(
            effective.iter().all(|(_, c)| !c.is_empty()),
            "默认表不应有空绑定: {effective:?}"
        );
    }

    /// 镜头默认必须是同一套前缀(cmd+alt), 否则"整套降级"无法覆盖全部镜头键;
    /// 降级后应得到 alt+shift+同一字母(整套一致)。
    #[test]
    fn camera_defaults_share_one_family() {
        let camera: Vec<_> = hotkey_actions()
            .into_iter()
            .filter(|(a, _)| a.starts_with("canvas_"))
            .collect();
        assert_eq!(camera.len(), 6);
        for (action, chord) in camera {
            assert!(
                chord.starts_with("cmd+alt+"),
                "镜头默认 {action}={chord} 不在 cmd+alt 家族"
            );
            let alt = registration_fallback(action, chord).expect("镜头应有降级组合");
            assert!(
                alt.starts_with("alt+shift+"),
                "降级 {action}={alt} 应为单键 Alt+Shift(不再用 4 键组合)"
            );
        }
    }

    /// 镜头键被占用时**单键**降级到 Alt+Shift+同键(2 个修饰键, 不做 4 键整套);
    /// 非镜头动作不降级。
    #[test]
    fn camera_fallback_chords() {
        assert_eq!(
            registration_fallback("canvas_pan_up", "cmd+alt+W").as_deref(),
            Some("alt+shift+W")
        );
        assert_eq!(
            registration_fallback("canvas_zoom_out", "cmd+alt+Z").as_deref(),
            Some("alt+shift+Z")
        );
        assert_eq!(registration_fallback("undo", "ctrl+alt+Z"), None);
        assert_eq!(registration_fallback("canvas_pan_up", "ctrl+alt+W"), None);
    }

    /// 各代旧默认(含"未绑定 = 空串")都能一次性迁到 v4; 迁移幂等。
    #[test]
    fn legacy_windows_defaults_are_migrated() {
        // v1~v3 的那 8 个"默认未绑定"动作: 空串 → v4 默认键
        let mut unbound = saved(&[
            ("color_swap", ""),
            ("width_swap", ""),
            ("toggle_free_canvas", ""),
            ("clear", ""),
            ("export", ""),
            ("copy_svg", ""),
            ("toggle_glass", ""),
            ("toggle_ethereal", ""),
        ]);
        assert_eq!(migrate_legacy_defaults(&mut unbound), 8);
        let effective_u = build_effective_saved(&unbound);
        assert_eq!(chord_of(&effective_u, "color_swap"), "ctrl+alt+shift+C");
        assert_eq!(chord_of(&effective_u, "width_swap"), "ctrl+alt+T");
        assert_eq!(chord_of(&effective_u, "toggle_free_canvas"), "ctrl+alt+I");
        assert_eq!(chord_of(&effective_u, "clear"), "ctrl+alt+C");
        assert_eq!(chord_of(&effective_u, "export"), "ctrl+alt+G");
        assert_eq!(chord_of(&effective_u, "copy_svg"), "ctrl+alt+S");
        assert_eq!(chord_of(&effective_u, "toggle_glass"), "ctrl+alt+B");
        assert_eq!(chord_of(&effective_u, "toggle_ethereal"), "ctrl+alt+X");

        // v1: 镜头=方向键/PgUp·PgDn, 退出无 Shift
        let mut v1 = saved(&[
            ("canvas_pan_left", "ctrl+alt+Left"),
            ("canvas_pan_right", "ctrl+alt+Right"),
            ("canvas_pan_up", "ctrl+alt+Up"),
            ("canvas_pan_down", "ctrl+alt+Down"),
            ("canvas_zoom_in", "ctrl+alt+PageUp"),
            ("canvas_zoom_out", "ctrl+alt+PageDown"),
            ("quit", "ctrl+alt+Q"),
        ]);
        assert_eq!(migrate_legacy_defaults(&mut v1), 7);
        assert_eq!(migrate_legacy_defaults(&mut v1), 0); // 幂等
        let effective = build_effective_saved(&v1);
        assert_eq!(chord_of(&effective, "canvas_pan_left"), "cmd+alt+A");
        assert_eq!(chord_of(&effective, "canvas_pan_right"), "cmd+alt+D");
        assert_eq!(chord_of(&effective, "canvas_pan_up"), "cmd+alt+W");
        assert_eq!(chord_of(&effective, "canvas_pan_down"), "cmd+alt+S");
        assert_eq!(chord_of(&effective, "canvas_zoom_in"), "cmd+alt+X");
        assert_eq!(chord_of(&effective, "canvas_zoom_out"), "cmd+alt+Z");
        assert_eq!(chord_of(&effective, "quit"), "ctrl+alt+shift+Q");
        assert_eq!(chord_of(&effective, "settings"), "ctrl+alt+D");

        // v4: 半套降级(个别镜头 alt+shift) → 归一回 Win+Alt, 再由注册逻辑整套决定
        let mut v4 = saved(&[
            ("canvas_pan_left", "cmd+alt+A"),
            ("canvas_pan_right", "alt+shift+D"),
            ("canvas_pan_up", "alt+shift+W"),
            ("canvas_pan_down", "cmd+alt+S"),
            ("canvas_zoom_in", "cmd+alt+X"),
            ("canvas_zoom_out", "cmd+alt+Z"),
        ]);
        assert_eq!(migrate_legacy_defaults(&mut v4), 2);
        let effective4 = build_effective_saved(&v4);
        for (action, chord) in [
            ("canvas_pan_left", "cmd+alt+A"),
            ("canvas_pan_right", "cmd+alt+D"),
            ("canvas_pan_up", "cmd+alt+W"),
            ("canvas_pan_down", "cmd+alt+S"),
            ("canvas_zoom_in", "cmd+alt+X"),
            ("canvas_zoom_out", "cmd+alt+Z"),
        ] {
            assert_eq!(chord_of(&effective4, action), chord, "v4 归一回 {action}");
        }

        // v3: 镜头 Ctrl+Alt+Shift+字母 → v4 Win+Alt+字母
        let mut v3 = saved(&[
            ("canvas_pan_left", "ctrl+alt+shift+A"),
            ("canvas_pan_right", "ctrl+alt+shift+D"),
            ("canvas_pan_up", "ctrl+alt+shift+W"),
            ("canvas_pan_down", "ctrl+alt+shift+S"),
            ("canvas_zoom_in", "ctrl+alt+shift+X"),
            ("canvas_zoom_out", "ctrl+alt+shift+Z"),
            ("settings", "ctrl+alt+D"),
            ("quit", "ctrl+alt+shift+Q"),
        ]);
        assert_eq!(migrate_legacy_defaults(&mut v3), 6);
        let effective3 = build_effective_saved(&v3);
        assert_eq!(chord_of(&effective3, "canvas_pan_left"), "cmd+alt+A");
        assert_eq!(chord_of(&effective3, "canvas_pan_right"), "cmd+alt+D");
        assert_eq!(chord_of(&effective3, "canvas_zoom_in"), "cmd+alt+X");
        assert_eq!(chord_of(&effective3, "canvas_zoom_out"), "cmd+alt+Z");
        assert_eq!(chord_of(&effective3, "settings"), "ctrl+alt+D");
        assert_eq!(chord_of(&effective3, "quit"), "ctrl+alt+shift+Q");
    }

    /// 只有"仍等于旧默认"的存档才迁移, 用户自己配的组合不动。
    #[test]
    fn custom_chords_are_not_migrated() {
        let mut map = saved(&[
            ("canvas_pan_left", "ctrl+alt+J"), // 用户自定义, 不是任何旧默认
            ("settings", "ctrl+alt+Q"),       // 用户自定义, 不是旧默认
            ("quit", "ctrl+alt+E"),
            ("toggle_ethereal", "ctrl+alt+U"),
        ]);
        assert_eq!(migrate_legacy_defaults(&mut map), 0);
        let effective = build_effective_saved(&map);
        assert_eq!(chord_of(&effective, "canvas_pan_left"), "ctrl+alt+J");
        assert_eq!(chord_of(&effective, "settings"), "ctrl+alt+Q");
        assert_eq!(chord_of(&effective, "quit"), "ctrl+alt+E");
        assert_eq!(chord_of(&effective, "toggle_ethereal"), "ctrl+alt+U");
    }

    /// Windows 没有菜单栏/托盘: 退出与打开设置都是强制绑定, 解绑必须回退默认。
    #[test]
    fn quit_and_settings_cannot_be_unbound() {
        let effective = build_effective_saved(&saved(&[("quit", ""), ("settings", "")]));
        assert_eq!(chord_of(&effective, "quit"), "ctrl+alt+shift+Q");
        assert_eq!(chord_of(&effective, "settings"), "ctrl+alt+D");
    }

    /// 退出可以改到别的组合(可配置), 原默认组合不再是保留组合。
    #[test]
    fn quit_can_be_rebound() {
        let effective = build_effective_saved(&saved(&[("quit", "ctrl+alt+E")]));
        assert_eq!(chord_of(&effective, "quit"), "ctrl+alt+E");
        assert!(!is_reserved("ctrl+alt+Q"));
    }

    /// 普通动作占了强制绑定动作的默认键时, 让位的必须是普通动作 —— 否则
    /// 「彻底退出/打开设置」可能注册不上(两个动作抢同一组合, 后注册的失败)。
    #[test]
    fn forced_action_wins_conflict() {
        let effective = build_effective_saved(&saved(&[("undo", "ctrl+alt+D")]));
        assert_eq!(chord_of(&effective, "settings"), "ctrl+alt+D");
        assert_eq!(chord_of(&effective, "undo"), "");
    }

    /// 默认表、旧默认迁移后的表、改过键的表都不能出现两个动作抢同一个组合。
    #[test]
    fn effective_table_never_has_duplicate_chords() {
        let mut legacy = saved(&[
            ("canvas_pan_left", "ctrl+alt+Left"),
            ("canvas_pan_right", "ctrl+alt+Right"),
            ("canvas_zoom_in", "ctrl+alt+PageUp"),
            ("canvas_zoom_out", "ctrl+alt+PageDown"),
            ("settings", "ctrl+alt+shift+D"),
            ("quit", "ctrl+alt+Q"),
        ]);
        migrate_legacy_defaults(&mut legacy);
        for map in [
            HashMap::new(),
            legacy,
            saved(&[("undo", "ctrl+alt+D")]), // 用户把撤销改到设置的默认键上
            saved(&[("undo", "ctrl+alt+Q")]),
        ] {
            let effective = build_effective_saved(&map);
            let mut seen: Vec<&str> = Vec::new();
            for (action, chord) in &effective {
                if chord.is_empty() {
                    continue;
                }
                assert!(
                    !seen.contains(&chord.as_str()),
                    "组合 {chord} 被多个动作占用: {action} (表: {:?})",
                    effective
                );
                seen.push(chord.as_str());
            }
        }
    }

    /// core 的 `ui::ACTIONS` 是动作/默认值的唯一来源: 生效表必须逐条覆盖 core
    /// 的全部动作, 默认值逐字等于 `ui::default_chord(Windows, action)`, 保留组合
    /// 不进默认表; 强制绑定集合与 core 完全一致(settings/quit)。
    /// core 加了动作而 Windows 壳没跟上时, 这个测试必须失败。
    #[test]
    fn core_catalog_drives_effective_table() {
        let effective = build_effective_saved(&HashMap::new());
        // 顺序 = 面板显示顺序 = core 顺序(HOTKEY_ID_ACTION_BASE + 下标依赖它)
        let expected_ids: Vec<&str> = hotkey_actions().into_iter().map(|(a, _)| a).collect();
        let actual_ids: Vec<&str> = effective.iter().map(|(a, _)| *a).collect();
        assert_eq!(actual_ids, expected_ids);

        for action in ui::ACTIONS {
            let chord = chord_of(&effective, action.id);
            assert_eq!(
                chord,
                ui::default_chord(UI_PLATFORM, action.id).unwrap_or(""),
                "{} 的默认值必须来自 core",
                action.id
            );
            assert!(!chord.is_empty(), "{} 在 Windows 必须有默认键", action.id);
            assert!(
                !is_reserved(&chord),
                "{} 的默认键 {chord} 不能是保留组合",
                action.id
            );
        }

        // 强制绑定动作: core 说是哪些就必须正好是哪些(不能漏也不能多)
        let mut forced: Vec<&str> = ui::force_bound(UI_PLATFORM);
        forced.sort_unstable();
        let mut ours: Vec<&str> = effective
            .iter()
            .filter(|(action, _)| force_bound(action))
            .map(|(action, _)| *action)
            .collect();
        ours.sort_unstable();
        assert_eq!(ours, forced);
        assert!(force_bound("settings"));
        assert!(force_bound("quit"));

        // 保留组合表与 core 同源(按住类三个), 且都不是可配置动作的默认键
        assert!(!ui::reserved(UI_PLATFORM).is_empty());
        for &chord in ui::reserved(UI_PLATFORM) {
            assert!(is_reserved(chord), "{chord} 应被 is_reserved 拦下");
        }
        // 用户把保留组合绑给普通动作 → 回退默认(core 的保留表真的生效)
        let rebound = build_effective_saved(&saved(&[("undo", "ctrl+alt+R")]));
        assert_eq!(chord_of(&rebound, "undo"), "ctrl+alt+Z");
    }
}
