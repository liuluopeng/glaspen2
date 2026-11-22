//! setSetting 的统一 effect 层(D1b 第一步;纯 core,两端 adapter 待接)。
//!
//! 背景:设置项的 key / 类型 / 落库键 / 平台可用性已由 `settings.rs` 单源(E3),
//! 数值区间已由 `presets.rs` 单源(FFI `glaspen2_clamp_setting_*`),但"面板发来
//! setSetting 后各自到底做什么"仍是 macOS ObjC 与 Windows 命名管道各写一份
//! `if key == ...` / `isEqualToString` 大 switch(见 `docs/duplication-backlog.md`
//! D1)。本模块把**解析 + 钳制**收敛到一处:面板键 + JSON 值 → 强类型
//! [`SettingEffect`];两端 adapter 下一步只需 `match SettingEffect` 做本平台的
//! 状态赋值 / 重绘 / 落库,不再各自 `[value boolValue]`、`v.clamp(..)`、解析
//! "装着 JSON 的字符串"。
//!
//! 边界(刻意, 避免一次切太大):
//! - **不碰数据库**:落库键与持久化仍由两端 adapter/FFI 负责,本切片不改持久化;
//! - **不判平台可用性**:`flipEffect` 仅 macOS、`ethereal` 仅 Windows 等由
//!   `settings::platforms` 决定;本模块对所有登记键都能产出 effect,不适用平台
//!   不消费即可(宁可空缺, 也不写半实现);
//! - **命令型 key 不走这里**:保存/导出/撤销等面板命令不是设置项,仍由
//!   `protocol.rs` 的命令分派处理。
//!
//! 两端现行规则若有差异,都写在对应分支/钳制注释里;钳制一律复用
//! `presets::clamp_setting_*` 的表,不在本文件另写数字。

use serde_json::Value;

/// 一次 setSetting 对应的、已解析/已钳制的平台无关副作用。
///
/// 每个变体都对应一个面板协议键(见变体文档与 [`SettingEffect::panel_key`])。
/// 变体里的值已是最终值:bool 已解析、数值已按两端现行区间钳制、字符串已归一,
/// adapter 直接使用即可。
#[derive(Debug, Clone, PartialEq)]
pub enum SettingEffect {
    /// 面板键 `bubbleAutoNewPage`:写完气泡消息自动新建一页(默认关)。
    BubbleAutoNewPage(bool),
    /// 面板键 `bubbleReplayAuto`:打开气泡 tab 自动重播最新一条(默认关)。
    BubbleReplayAuto(bool),
    /// 面板键 `bubbleReplayHoldLast`:重播播完停在最后一帧(默认开)。
    BubbleReplayHoldLast(bool),
    /// 面板键 `bubbleReplaySpeed`:气泡重播速度倍率,钳制 1..=8
    /// (macOS menu.m:1113-1115 ↔ Windows pipe.rs:1178,两端一致)。
    BubbleReplaySpeed(i64),
    /// 面板键 `chatApiBase`:聊天/共享 API 地址(落库后 adapter 重载 auth 缓存)。
    ChatApiBase(String),
    /// 面板键 `chatIntegration`:手写消息/共享总开关(关 = 热键直通、面板收起)。
    ChatIntegration(bool),
    /// 面板键 `chatUser`:涂鸦身份用户名。
    ChatUser(String),
    /// 面板键 `color`:选中第 idx 个颜色预设。**不钳制**:两端都按"预设表边界"
    /// 在应用时忽略越界值(macOS `gl_settings_set_color` / Windows CMD_SELECT_COLOR),
    /// 本层不擅自改语义。
    Color(i64),
    /// 面板键 `flipEffect`:翻页动效,钳制 0..=1。
    /// 仅 macOS 实现(settings.rs platforms=MAC;Windows 无此键, 面板隐藏)。
    FlipEffect(i64),
    /// 面板键 `frostedGlass`:磨砂玻璃开关(落库键 glass_enabled)。
    FrostedGlass(bool),
    /// 面板键 `gifEndMode`:GIF 结束模式,钳制 0..=2(presets 区间表,两端同源)。
    GifEndMode(i64),
    /// 面板键 `gifFps`:GIF 帧率,钳制 1..=50(presets 区间表,两端同源)。
    GifFps(i64),
    /// 面板键 `gifResolution`:GIF 分辨率缩放,钳制 0.1..=1.0(presets 区间表)。
    GifResolution(f64),
    /// 面板键 `gifSpeed`:GIF 播放速度,钳制 0.25..=20.0(presets 区间表)。
    GifSpeed(f64),
    /// 面板键 `glassFollowStrokes`:玻璃只在笔迹附近显示(跟随笔迹)。
    GlassFollowStrokes(bool),
    /// 面板键 `grid`:网格开关。
    Grid(bool),
    /// 面板键 `gridDivider`:分栏 0=无 1=左右 2=上下 3=九宫格 4=康奈尔,
    /// 钳制 0..=4(macOS menu.m:1059-1061 ↔ Windows pipe.rs:1083,两端一致)。
    GridDivider(i64),
    /// 面板键 `gridFollowStrokes`:网格跟随笔迹。
    GridFollowStrokes(bool),
    /// 面板键 `gridSize`:网格尺寸,钳制 10.0..=200.0(presets 区间表,两端同源)。
    GridSize(f64),
    /// 面板键 `infiniteCanvas`:活页本 ↔ 无限画布模式切换。
    InfiniteCanvas(bool),
    /// 面板键 `invertFps`:反色捕获帧率,**白名单 {10,30,60,100}** 而非区间。
    /// 两端今天的做法都是"非白名单值直接忽略"(不落库、不重启捕获流),
    /// 所以本层返回 `Err` 让 adapter 记录并跳过,不擅自改成别的帧率。
    InvertFps(i64),
    /// 面板键 `invertInk`:反色模式开关(历史 gap: Windows 存了但渲染不消费)。
    InvertInk(bool),
    /// 面板键 `language`:面板/菜单语言。已按两端规则归一为 `"en"` 或 `"zh"`:
    /// 只有 `"en"` 是英文,其余一律中文(macOS menu.m:1144 / Windows pipe.rs:1227)。
    Language(String),
    /// 面板键 `launchAtLogin`:开机自启(平台存储:LaunchAgent plist / HKCU Run)。
    LaunchAtLogin(bool),
    /// 面板键 `minimap`:页面缩略图条开关。
    Minimap(bool),
    /// 面板键 `nasUrl`:NAS 备份服务地址(空 = 未启用)。
    NasUrl(String),
    /// 面板键 `notebookStyle`:活页本拟物风格。
    NotebookStyle(bool),
    /// 面板键 `outline`:笔迹描边开关。
    Outline(bool),
    /// 面板键 `panelTransparent`:面板窗口真穿透(实验)。
    PanelTransparent(bool),
    /// 面板键 `pressureMonitor`:压感监视浮层开关。
    PressureMonitor(bool),
    /// 面板键 `shareCanvas`:共享画布上行开关(仅在 chatIntegration 开启时生效)。
    ShareCanvas(bool),
    /// 面板键 `shortcuts`:整张 action → chord 表,始终是 JSON **对象**
    /// (`serde_json::Value::Object`;面板可能发对象,也可能发"装着 JSON 的字符串")。
    /// 规范化/补默认/挡冲突仍在两端现有实现里,本层只做解析。
    Shortcuts(Value),
    /// 面板键 `showBubbleTab`:「气泡消息」tab 显隐。
    ShowBubbleTab(bool),
    /// 面板键 `showFreeCanvas`:「自由涂鸦」tab 显隐。
    ShowFreeCanvas(bool),
    /// 面板键 `softShadow`:笔迹柔和阴影。
    SoftShadow(bool),
    /// 面板键 `width`:选中第 idx 个粗细预设。**不钳制**(同 `Color`, 应用时按
    /// 预设表边界忽略越界值)。
    Width(i64),
    /// 特殊键 `chatPassword`:**不在 SETTINGS 登记表**(密码本体永不进快照),
    /// 但两端 setSetting 都处理。空串语义两端今天不一致:macOS = 保持已存密码
    /// 不变(menu.m:1097-1103),Windows = 直接写空(pipe.rs:1115-1124);
    /// 本层原样携带,由 adapter 保留各自规则(接线时再对齐,见 D1b 进度记录)。
    ChatPassword(String),
    /// 特殊键 `hotkeyRecording`:**不在 SETTINGS 登记表**(控制消息, 不落库),
    /// 但两端 setSetting 都处理:录制快捷键期间放行/注销全局热键,
    /// macOS 20s 自动恢复、Windows 60s 兜底, 由各自 adapter 计时。
    HotkeyRecording(bool),
}

/// 面板 setSetting 的 (key, value) → 强类型 effect(解析 + 钳制一次完成)。
///
/// 返回 `Err` 的场景(adapter 应记录并跳过, 等价于两端今天的"这一项不生效"):
/// - 未登记且非特殊键的 key(防面板/后端版本漂移时静默写错项);
/// - [`crate::settings::READONLY_KEYS`](只读/派生值, 不允许 setSetting 写);
/// - 值类型或取值不合法(bool 收到 null、int 收到 1.5、invertFps 不在白名单…)。
///
/// 宽容规则见各 `parse_*` 函数:面板历史上出现过 bool/0/1/"true"/"false"/
/// 数字字符串等形态,这里统一收口。
pub fn effect_for(key: &str, value: &Value) -> Result<SettingEffect, String> {
    // 控制型 key:面板 setSetting 通道里的"非持久化动作",不在 SETTINGS 登记表
    // (不是设置项、不进快照),但两端 setSetting 都处理,所以也在这里单源解析。
    match key {
        "chatPassword" => return Ok(SettingEffect::ChatPassword(parse_string(key, value)?)),
        "hotkeyRecording" => return Ok(SettingEffect::HotkeyRecording(parse_bool(key, value)?)),
        _ => {}
    }

    let def = crate::settings::get(key).ok_or_else(|| format!("未知设置键: {key}"))?;
    if crate::settings::READONLY_KEYS.contains(&key) {
        return Err(format!(
            "设置键 {key} 是只读/派生值, 不能通过 setSetting 写入"
        ));
    }

    // 按 SETTINGS 的字母序排列, 方便与登记表逐行对照。
    match key {
        "bubbleAutoNewPage" => Ok(SettingEffect::BubbleAutoNewPage(parse_bool(key, value)?)),
        "bubbleReplayAuto" => Ok(SettingEffect::BubbleReplayAuto(parse_bool(key, value)?)),
        "bubbleReplayHoldLast" => Ok(SettingEffect::BubbleReplayHoldLast(parse_bool(key, value)?)),
        "bubbleReplaySpeed" => Ok(SettingEffect::BubbleReplaySpeed(
            parse_int(key, value)?.clamp(1, 8),
        )),
        "chatApiBase" => Ok(SettingEffect::ChatApiBase(parse_string(key, value)?)),
        "chatIntegration" => Ok(SettingEffect::ChatIntegration(parse_bool(key, value)?)),
        "chatUser" => Ok(SettingEffect::ChatUser(parse_string(key, value)?)),
        "color" => Ok(SettingEffect::Color(parse_int(key, value)?)),
        "flipEffect" => Ok(SettingEffect::FlipEffect(
            parse_int(key, value)?.clamp(0, 1),
        )),
        "frostedGlass" => Ok(SettingEffect::FrostedGlass(parse_bool(key, value)?)),
        "gifEndMode" => Ok(SettingEffect::GifEndMode(clamp_int_key(
            key,
            parse_int(key, value)?,
        ))),
        "gifFps" => Ok(SettingEffect::GifFps(clamp_int_key(
            key,
            parse_int(key, value)?,
        ))),
        "gifResolution" => Ok(SettingEffect::GifResolution(
            crate::presets::clamp_setting_double(key, parse_float(key, value)?),
        )),
        "gifSpeed" => Ok(SettingEffect::GifSpeed(
            crate::presets::clamp_setting_double(key, parse_float(key, value)?),
        )),
        "glassFollowStrokes" => Ok(SettingEffect::GlassFollowStrokes(parse_bool(key, value)?)),
        "grid" => Ok(SettingEffect::Grid(parse_bool(key, value)?)),
        "gridDivider" => Ok(SettingEffect::GridDivider(
            parse_int(key, value)?.clamp(0, 4),
        )),
        "gridFollowStrokes" => Ok(SettingEffect::GridFollowStrokes(parse_bool(key, value)?)),
        "gridSize" => Ok(SettingEffect::GridSize(
            crate::presets::clamp_setting_double(key, parse_float(key, value)?),
        )),
        "infiniteCanvas" => Ok(SettingEffect::InfiniteCanvas(parse_bool(key, value)?)),
        "invertFps" => {
            let fps = parse_int(key, value)?;
            if matches!(fps, 10 | 30 | 60 | 100) {
                Ok(SettingEffect::InvertFps(fps))
            } else {
                Err(format!(
                    "设置 invertFps 只接受 10/30/60/100, 收到 {fps}(两端现行行为均为忽略该值)"
                ))
            }
        }
        "invertInk" => Ok(SettingEffect::InvertInk(parse_bool(key, value)?)),
        "language" => Ok(SettingEffect::Language(language_from(parse_string(
            key, value,
        )?))),
        "launchAtLogin" => Ok(SettingEffect::LaunchAtLogin(parse_bool(key, value)?)),
        "minimap" => Ok(SettingEffect::Minimap(parse_bool(key, value)?)),
        "nasUrl" => Ok(SettingEffect::NasUrl(parse_string(key, value)?)),
        "notebookStyle" => Ok(SettingEffect::NotebookStyle(parse_bool(key, value)?)),
        "outline" => Ok(SettingEffect::Outline(parse_bool(key, value)?)),
        "panelTransparent" => Ok(SettingEffect::PanelTransparent(parse_bool(key, value)?)),
        "pressureMonitor" => Ok(SettingEffect::PressureMonitor(parse_bool(key, value)?)),
        "shareCanvas" => Ok(SettingEffect::ShareCanvas(parse_bool(key, value)?)),
        "shortcuts" => Ok(SettingEffect::Shortcuts(parse_shortcuts(value)?)),
        "showBubbleTab" => Ok(SettingEffect::ShowBubbleTab(parse_bool(key, value)?)),
        "showFreeCanvas" => Ok(SettingEffect::ShowFreeCanvas(parse_bool(key, value)?)),
        "softShadow" => Ok(SettingEffect::SoftShadow(parse_bool(key, value)?)),
        "width" => Ok(SettingEffect::Width(parse_int(key, value)?)),
        // 登记表新增了 key 但本模块忘了加分支:宁可报错, 也不要静默吞掉。
        _ => Err(format!(
            "设置键 {key} 已在登记表(kind={})但 settings_apply 未接入 effect",
            def.kind.as_str()
        )),
    }
}

impl SettingEffect {
    /// 该 effect 对应的面板协议键(日志/测试/适配器排障用)。
    ///
    /// 与 `settings.rs` 的登记键一一对应;控制型特殊键也返回面板实际发送的键。
    pub fn panel_key(&self) -> &'static str {
        match self {
            Self::BubbleAutoNewPage(_) => "bubbleAutoNewPage",
            Self::BubbleReplayAuto(_) => "bubbleReplayAuto",
            Self::BubbleReplayHoldLast(_) => "bubbleReplayHoldLast",
            Self::BubbleReplaySpeed(_) => "bubbleReplaySpeed",
            Self::ChatApiBase(_) => "chatApiBase",
            Self::ChatIntegration(_) => "chatIntegration",
            Self::ChatUser(_) => "chatUser",
            Self::Color(_) => "color",
            Self::FlipEffect(_) => "flipEffect",
            Self::FrostedGlass(_) => "frostedGlass",
            Self::GifEndMode(_) => "gifEndMode",
            Self::GifFps(_) => "gifFps",
            Self::GifResolution(_) => "gifResolution",
            Self::GifSpeed(_) => "gifSpeed",
            Self::GlassFollowStrokes(_) => "glassFollowStrokes",
            Self::Grid(_) => "grid",
            Self::GridDivider(_) => "gridDivider",
            Self::GridFollowStrokes(_) => "gridFollowStrokes",
            Self::GridSize(_) => "gridSize",
            Self::InfiniteCanvas(_) => "infiniteCanvas",
            Self::InvertFps(_) => "invertFps",
            Self::InvertInk(_) => "invertInk",
            Self::Language(_) => "language",
            Self::LaunchAtLogin(_) => "launchAtLogin",
            Self::Minimap(_) => "minimap",
            Self::NasUrl(_) => "nasUrl",
            Self::NotebookStyle(_) => "notebookStyle",
            Self::Outline(_) => "outline",
            Self::PanelTransparent(_) => "panelTransparent",
            Self::PressureMonitor(_) => "pressureMonitor",
            Self::ShareCanvas(_) => "shareCanvas",
            Self::Shortcuts(_) => "shortcuts",
            Self::ShowBubbleTab(_) => "showBubbleTab",
            Self::ShowFreeCanvas(_) => "showFreeCanvas",
            Self::SoftShadow(_) => "softShadow",
            Self::Width(_) => "width",
            Self::ChatPassword(_) => "chatPassword",
            Self::HotkeyRecording(_) => "hotkeyRecording",
        }
    }
}

// ── 解析(宽容, 收敛面板历史上出现过的 JSON 形态) ──────────────────────

/// bool:JSON bool / 0|1 数字 / "true"|"false"|"1"|"0" 字符串。
/// (两端进来的是 JSON 标量;字符串形态来自旧面板/测试,宽进避免静默写 false。)
fn parse_bool(key: &str, value: &Value) -> Result<bool, String> {
    let parsed = match value {
        Value::Bool(b) => Some(*b),
        Value::Number(n) => match n.as_i64() {
            Some(1) => Some(true),
            Some(0) => Some(false),
            _ => None,
        },
        Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "true" | "1" => Some(true),
            "false" | "0" => Some(false),
            _ => None,
        },
        _ => None,
    };
    parsed.ok_or_else(|| format!("设置 {key} 需要 bool(true/false/1/0), 收到 {value}"))
}

/// 整数:JSON 整数(含整数值的浮点) / 整数值数字字符串。
fn parse_int(key: &str, value: &Value) -> Result<i64, String> {
    let parsed = match value {
        Value::Number(n) => n.as_i64().or_else(|| {
            let f = n.as_f64()?;
            integral_i64(f)
        }),
        Value::String(s) => {
            let s = s.trim();
            s.parse::<i64>()
                .ok()
                .or_else(|| integral_i64(s.parse::<f64>().ok()?))
        }
        _ => None,
    };
    parsed.ok_or_else(|| format!("设置 {key} 需要整数, 收到 {value}"))
}

/// 有限浮点整数且能放进 i64 时才返回(挡掉 1.5 / NaN / 1e300 这类值)。
fn integral_i64(f: f64) -> Option<i64> {
    (f.is_finite() && f.fract() == 0.0 && f >= i64::MIN as f64 && f <= i64::MAX as f64)
        .then_some(f as i64)
}

/// 浮点:JSON 数字 / 有限数字字符串。NaN/inf 一律拒绝(JSON 也表达不了)。
fn parse_float(key: &str, value: &Value) -> Result<f64, String> {
    let parsed = match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    };
    match parsed {
        Some(f) if f.is_finite() => Ok(f),
        _ => Err(format!("设置 {key} 需要有限数字, 收到 {value}")),
    }
}

/// 字符串:只收 JSON 字符串。
///
/// 两端对"非字符串值"的旧行为不一致(macOS `%@` 会 stringify,Windows
/// `json_get_str` 得空串);本层取最严格的"只收字符串",面板正常路径发的都是
/// 字符串,不会改变真实行为。
fn parse_string(key: &str, value: &Value) -> Result<String, String> {
    match value {
        Value::String(s) => Ok(s.clone()),
        _ => Err(format!("设置 {key} 需要字符串, 收到 {value}")),
    }
}

/// 快捷键整表:接受 JSON 对象(macOS NSDictionary / 管道内联对象)或
/// "装着 JSON 的字符串"(FRB 侧 `jsonEncode(value)` 的形状)。
/// 统一产出 `Value::Object`;空对象的语义差异留给 adapter(macOS = 全回默认并
/// 落库,Windows 今天 = 忽略),接线时再对齐。
fn parse_shortcuts(value: &Value) -> Result<Value, String> {
    match value {
        Value::Object(_) => Ok(value.clone()),
        Value::String(s) => match serde_json::from_str::<Value>(s) {
            Ok(v) if v.is_object() => Ok(v),
            Ok(v) => Err(format!("设置 shortcuts 的字符串不是 JSON 对象: {v}")),
            Err(e) => Err(format!("设置 shortcuts 的字符串不是合法 JSON: {e}")),
        },
        other => Err(format!(
            "设置 shortcuts 需要 JSON 对象或装着 JSON 对象的字符串, 收到 {other}"
        )),
    }
}

/// 语言归一:只有 `"en"` 是英文, 其余一律 `"zh"`
/// (macOS menu.m:1144-1145 / Windows pipe.rs:1227-1228 同一规则)。
fn language_from(raw: String) -> String {
    if raw == "en" {
        "en".to_string()
    } else {
        "zh".to_string()
    }
}

// ── 钳制 ─────────────────────────────────────────────────────────────
//
// 区间表在 core/presets.rs, 与 FFI `glaspen2_clamp_setting_int/double` 同源,
// 这里必须复用它, 不得另写一套数字:
//   gridSize        f64 10.0..=200.0   (macOS menu.m:1053 / Windows state.rs:195)
//   gifFps          i64 1..=50         (macOS menu.m:1076 / Windows state.rs:133)
//   gifResolution   f64 0.1..=1.0      (macOS menu.m:1080 / Windows state.rs:135)
//   gifSpeed        f64 0.25..=20.0    (macOS menu.m:1084 / Windows state.rs:138)
//   gifEndMode      i64 0..=2          (macOS menu.m:1088 / Windows state.rs:139)
//
// 其余在 adapter 里行内写死的规则, 本次上收到本模块(两端逐条核对过):
//   gridDivider        0..=4    macOS menu.m:1059-1061 ↔ Windows pipe.rs:1083 一致
//   flipEffect         0..=1    仅 macOS menu.m:1067-1069;Windows 无此键(platforms=MAC)
//   bubbleReplaySpeed  1..=8    macOS menu.m:1113-1115 ↔ Windows pipe.rs:1178 一致
//   invertFps          白名单 {10,30,60,100}
//                              macOS menu.m:1032 ↔ Windows pipe.rs:1261 一致(非法值忽略)
//
// 两端不一致时取并集/最严并在报告注明;当前以上规则两端完全一致, 没有取舍差异。
// 不钳制: color / width(应用时按预设表边界忽略越界, 钳制反而会改变语义)、
//         所有字符串项(url/用户名等无区间)。

/// 走 core presets 区间表的 int 键。
/// 先把 i64 收进 i32 域再交表: `as i32` 对超界值是截断(不是饱和),
/// 直接转会得到负数/错值,所以必须先 clamp。
fn clamp_int_key(key: &str, v: i64) -> i64 {
    let v32 = v.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    crate::presets::clamp_setting_int(key, v32) as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{self, Kind, SETTINGS};
    use serde_json::json;

    fn eff(key: &str, value: Value) -> SettingEffect {
        match effect_for(key, &value) {
            Ok(effect) => effect,
            Err(err) => panic!("{key}={value} 应解析成功, 实际: {err}"),
        }
    }

    fn err(key: &str, value: Value) -> String {
        match effect_for(key, &value) {
            Ok(effect) => panic!("{key}={value} 应被拒绝, 却得到 {effect:?}"),
            Err(err) => err,
        }
    }

    /// 登记表里每个 key 的一个合法样例值(测试用)。
    fn sample(key: &str) -> Value {
        match key {
            // 有白名单/区间约束的键, 给一个合法值
            "bubbleReplaySpeed" => json!(1),
            "flipEffect" => json!(0),
            "gifEndMode" => json!(1),
            "gifFps" => json!(15),
            "gifResolution" => json!(0.5),
            "gifSpeed" => json!(2.0),
            "gridDivider" => json!(0),
            "gridSize" => json!(40.0),
            "invertFps" => json!(30),
            "language" => json!("zh"),
            "shortcuts" => json!({"prevPage": "cmd+ctrl+`"}),
            _ => match settings::get(key).expect("样例只用于登记表里的 key").kind {
                Kind::Bool => json!(true),
                Kind::Int => json!(0),
                Kind::Float => json!(1.0),
                Kind::Str => json!("x"),
            },
        }
    }

    /// 登记表里每个可设置项都必须能生成 effect, 且 effect 反查回同一个面板键。
    #[test]
    fn every_settable_registry_key_maps_to_effect() {
        let mut checked = 0;
        for def in SETTINGS.iter().filter(|d| settings::is_settable(d.key)) {
            let effect = eff(def.key, sample(def.key));
            assert_eq!(
                effect.panel_key(),
                def.key,
                "effect 与面板键不对应: {effect:?}"
            );
            checked += 1;
        }
        assert!(
            checked >= 30,
            "可设置项解析异常: 只检查了 {checked} 个(登记表被改坏?)"
        );
    }

    #[test]
    fn read_only_and_unknown_keys_are_rejected() {
        for key in settings::READONLY_KEYS {
            let msg = err(key, json!(true));
            assert!(msg.contains("只读"), "只读项错误信息不清晰: {msg}");
        }
        let msg = err("noSuchSetting", json!(1));
        assert!(msg.contains("未知设置键"), "未知键错误信息不清晰: {msg}");
        // 嵌套对象不是合法标量
        assert!(err("grid", json!({"on": true})).contains("bool"));
    }

    #[test]
    fn tolerant_bool_forms() {
        for value in [
            json!(true),
            json!(1),
            json!("true"),
            json!("TRUE"),
            json!("1"),
            json!(" 1 "),
        ] {
            assert_eq!(
                eff("frostedGlass", value.clone()),
                SettingEffect::FrostedGlass(true),
                "输入 {value}"
            );
        }
        for value in [json!(false), json!(0), json!("false"), json!("0")] {
            assert_eq!(
                eff("frostedGlass", value.clone()),
                SettingEffect::FrostedGlass(false),
                "输入 {value}"
            );
        }
        // 布尔项只认 true/false 与整数 0/1: 其它数字/小数/null 都拒绝
        for value in [json!(2), json!(-1), json!(1.0), json!("yes"), json!(null)] {
            let msg = err("frostedGlass", value.clone());
            assert!(msg.contains("bool"), "错误信息应说明需要 bool: {msg}");
        }
    }

    #[test]
    fn tolerant_numeric_forms() {
        // int: JSON 整数 / 整数值浮点 / 数字字符串
        assert_eq!(eff("gifFps", json!(20)), SettingEffect::GifFps(20));
        assert_eq!(eff("gifFps", json!(20.0)), SettingEffect::GifFps(20));
        assert_eq!(eff("gifFps", json!("20")), SettingEffect::GifFps(20));
        assert!(err("gifFps", json!(1.5)).contains("整数"));
        // float: JSON 数字 / 数字字符串
        assert_eq!(eff("gifSpeed", json!(2)), SettingEffect::GifSpeed(2.0));
        assert_eq!(eff("gifSpeed", json!("2.5")), SettingEffect::GifSpeed(2.5));
        assert!(err("gifSpeed", json!("NaN")).contains("数字"));
        assert!(err("gifSpeed", json!(true)).contains("数字"));
        // 字符串项只收 JSON 字符串
        assert_eq!(
            eff("chatUser", json!("alice")),
            SettingEffect::ChatUser("alice".to_string())
        );
        assert!(err("chatUser", json!(42)).contains("字符串"));
    }

    /// 每个钳制边界(逐条对应两端历史实现与 presets 区间表)。
    #[test]
    fn clamps_match_platform_histories() {
        use SettingEffect::*;

        // core presets 区间表(两端同源)
        assert_eq!(eff("gridSize", json!(5.0)), GridSize(10.0));
        assert_eq!(eff("gridSize", json!(999.0)), GridSize(200.0));
        assert_eq!(eff("gifFps", json!(0)), GifFps(1));
        assert_eq!(eff("gifFps", json!(120)), GifFps(50));
        // i64 超界先收进 i32 域再查表,不能被截断成负数
        assert_eq!(eff("gifFps", json!(i64::MAX)), GifFps(50));
        assert_eq!(eff("gifResolution", json!(0.05)), GifResolution(0.1));
        assert_eq!(eff("gifResolution", json!(2.0)), GifResolution(1.0));
        assert_eq!(eff("gifSpeed", json!(0.1)), GifSpeed(0.25));
        assert_eq!(eff("gifSpeed", json!(99.0)), GifSpeed(20.0));
        assert_eq!(eff("gifEndMode", json!(-1)), GifEndMode(0));
        assert_eq!(eff("gifEndMode", json!(7)), GifEndMode(2));

        // 两端行内钳制(本次上收, 规则一致)
        assert_eq!(eff("bubbleReplaySpeed", json!(0)), BubbleReplaySpeed(1));
        assert_eq!(eff("bubbleReplaySpeed", json!(99)), BubbleReplaySpeed(8));
        assert_eq!(eff("gridDivider", json!(-1)), GridDivider(0));
        assert_eq!(eff("gridDivider", json!(9)), GridDivider(4));
        // flipEffect 仅 macOS, 区间 0..=1
        assert_eq!(eff("flipEffect", json!(-1)), FlipEffect(0));
        assert_eq!(eff("flipEffect", json!(5)), FlipEffect(1));

        // invertFps 是白名单: 合法值原样通过, 其它拒绝(等价两端"忽略")
        for fps in [10, 30, 60, 100] {
            assert_eq!(eff("invertFps", json!(fps)), InvertFps(fps));
        }
        for fps in [0, 5, 25, 120] {
            let msg = err("invertFps", json!(fps));
            assert!(msg.contains("10/30/60/100"), "错误信息应列出白名单: {msg}");
        }
    }

    /// 区间仍由 presets 单源:本模块不得偷偷换一套数字。
    #[test]
    fn clamp_ranges_stay_shared_with_presets() {
        for raw in [5.0, 999.0] {
            let effect = eff("gridSize", json!(raw));
            let expected = crate::presets::clamp_setting_double("gridSize", raw);
            assert_eq!(effect, SettingEffect::GridSize(expected));
        }
        for (key, raw) in [
            ("gifFps", 0i32),
            ("gifFps", 120),
            ("gifEndMode", 7),
            ("gifEndMode", -1),
        ] {
            let clamped = match eff(key, json!(raw)) {
                SettingEffect::GifFps(v) | SettingEffect::GifEndMode(v) => v,
                other => panic!("{key} 生成了非预期 effect: {other:?}"),
            };
            assert_eq!(
                clamped,
                crate::presets::clamp_setting_int(key, raw) as i64,
                "{key} 未复用 presets 区间表"
            );
        }
    }

    #[test]
    fn shortcuts_accepts_object_and_json_string() {
        let table = json!({"prevPage": "cmd+ctrl+`", "nextPage": "cmd+ctrl+1"});
        assert_eq!(
            eff("shortcuts", table.clone()),
            SettingEffect::Shortcuts(table.clone())
        );
        assert_eq!(
            eff("shortcuts", json!(table.to_string())),
            SettingEffect::Shortcuts(table.clone())
        );
        // 空对象也合法(全回默认; 两端空表语义差异留给 adapter)
        assert_eq!(
            eff("shortcuts", json!({})),
            SettingEffect::Shortcuts(json!({}))
        );
        assert!(err("shortcuts", json!("not json")).contains("JSON"));
        assert!(err("shortcuts", json!("[1,2]")).contains("对象"));
        assert!(err("shortcuts", json!([1, 2])).contains("JSON 对象"));
        assert!(err("shortcuts", json!(42)).contains("JSON 对象"));
    }

    #[test]
    fn language_is_normalized_like_both_shells() {
        // 只有 "en" 是英文, 其余一律 zh(macOS menu.m:1144 / Windows pipe.rs:1227)
        assert_eq!(
            eff("language", json!("en")),
            SettingEffect::Language("en".to_string())
        );
        assert_eq!(
            eff("language", json!("zh")),
            SettingEffect::Language("zh".to_string())
        );
        assert_eq!(
            eff("language", json!("fr")),
            SettingEffect::Language("zh".to_string())
        );
        assert_eq!(
            eff("language", json!("EN")),
            SettingEffect::Language("zh".to_string())
        );
    }

    /// 特殊键(不在登记表, 但面板 setSetting 会发)也有强类型 effect。
    #[test]
    fn control_keys_are_special_variants() {
        // 密码: 空串 = 保持已存密码(macOS 语义; 落库/重载由 adapter 做)
        assert_eq!(
            eff("chatPassword", json!("")),
            SettingEffect::ChatPassword(String::new())
        );
        assert_eq!(
            eff("chatPassword", json!("s3cret")),
            SettingEffect::ChatPassword("s3cret".to_string())
        );
        assert_eq!(
            eff("hotkeyRecording", json!(true)),
            SettingEffect::HotkeyRecording(true)
        );
        assert_eq!(
            eff("hotkeyRecording", json!("0")),
            SettingEffect::HotkeyRecording(false)
        );
        assert_eq!(
            eff("hotkeyRecording", json!(true)).panel_key(),
            "hotkeyRecording"
        );
        assert_eq!(eff("chatPassword", json!("x")).panel_key(), "chatPassword");
        // 都不是登记表里的设置项(不进快照)
        assert!(!settings::is_settable("hotkeyRecording"));
        assert!(!settings::is_settable("chatPassword"));
    }
}
