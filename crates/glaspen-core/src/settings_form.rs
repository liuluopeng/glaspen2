//! 设置面板「表单」元数据(FormMeta / SECTIONS)—— D11a。
//!
//! 背景: 面板(flutter_settings)的表单结构(分区/顺序/标签/控件类型/可选值/
//! 单位/范围/特性门控)此前全部手写在 Dart 里(`main.settings_widgets.dart`
//! 1309 行 + `main.settings_state.dart` + `main.dart` 的 settings tab 组装)。
//! D11 要把表单也收口到 core: 新增设置项只改 core registry + 两端 effect,
//! Dart 不再按 key 手写控件。本切片(D11a)只加**元数据**并经 uiConfig 下发,
//! Dart 仍按旧代码渲染 —— 用户可见行为与文案零变化; D11b 再让 Dart 从
//! schema 生成控件并删掉逐键手写代码。
//!
//! 事实源与口径:
//! - 键集合 = `settings::SETTINGS`(38 条登记项)中 Dart 表单真实渲染的项;
//!   本文件为**全部** 38 条登记项给出分类(表单控件 / implicit / readOnly /
//!   hidden), 登记表新增键却漏分类会让 `cargo test -p glaspen-core
//!   settings_form::` 变红, 不会静默消失。
//! - `label_id` / `description_id` / 选项 `label_id` 都是 **Flutter l10n 键**
//!   (`app_en.arb` / `app_zh.arb` 的键名 = gen-l10n 生成的 getter 名), 不是
//!   文案; 测试会校验两套 ARB 都含该键。单位是稳定 token(见 [`Unit`]),
//!   具体显示格式由 Dart 决定。
//! - `order` 只是**分区内**的显示顺序(0 起连续); 分区显示顺序见 [`SECTIONS`]。
//!
//! ── 表单盘点(以当前 Dart 代码为准)──
//! 38 条登记项 = 34 条表单可见 + 4 条非表单:
//! - implicit: `infiniteCanvas` —— 由面板 tab(活页本/自由涂鸦)切换时写入,
//!   没有独立控件;
//! - readOnly: `chatHasPassword` —— 派生值, 只在登录区当状态文案用;
//! - hidden: `frostedGlass` —— 面板没有开关(只由动作/热键切换玻璃), 但仍在
//!   `_settingsToMap` 与 uiConfig 契约里; `ethereal` —— Windows 壳运行时状态,
//!   面板不消费。
//!
//! 含设置项的分区共 10 个(见 [`SECTIONS`]): pen / canvasLook / notebookLook /
//! recordExport / dataBackup / sharedCanvas / doodleIdentity / systemAndAssist /
//! language / shortcuts。Dart 里还有 `about` 区块(版本/检查更新), 不含任何
//! 设置项, 故不登记为设置分区。
//!
//! ── 不是设置的表单字段(D11b 再纳入)──
//! - `chatPassword`: 密码输入框走协议特殊键 `chatPassword`(不在 SETTINGS
//!   登记表, 见 `settings_apply::effect_for`), 不属于设置项; D11b 需要把
//!   "非持久化动作字段"也纳入 schema, 或在 Dart 保留这一块手写。
//! - 备份/导出/检查更新/登录按钮: 是命令(protocol/FFI 调用), 不是设置项。
//! - `showBubbleTab` 下的气泡重播控件、`grid` 下的网格尺寸、
//!   `chatIntegration` 下的登录字段/共享画布开关: 是**设置值驱动**的条件显示
//!   (不是 uiConfig.features)。本切片的 `visible_when_feature` 只表达 features
//!   门控(invertInk / pageFlipEffect), 设置值门控留给 D11b 扩展 schema 或
//!   Dart 保留条件。
//!
//! ── 盘点中发现的差异(D11a 只记录, 不改行为)──
//! 1. `gridSize`: registry `Kind::Float`、core 钳制 10..200, 但表单只给
//!    20/40/80 三档整数。元数据按**表单**登记整数档位; registry 的 Float 口径
//!    不改(快照/落库类型由 D12 决定)。
//! 2. `bubbleReplaySpeed`: core 钳制 1..8, 表单只给 1/2/4 三档。
//! 3. `gifFps` / `gifResolution` / `gifSpeed`: 表单档位是 core 钳制区间的
//!    **子集**(10..50 / 0.25..1.0 / 0.5..20); 测试断言所有档位经 core 钳制后
//!    不变。
//! 4. `invertFps`: 表单 10/30/60/100 与 `settings_apply` 白名单完全一致。
//! 5. `shortcuts` 分区标题: Dart `_shortcutSectionTitle()` 硬编码
//!    「快捷键 / Shortcuts」, 没走 l10n; ARB 里已有 `hotkeys`(⌘⌃…)/
//!    `hotkeysCtrlAlt`(Ctrl+Alt+…) 两条**未被使用**的键。本文件分区 label_id
//!    记 `hotkeys`(稳定键), D11b 应统一标题并决定用哪条(或补一条
//!    `shortcutsTitle`)。
//! 6. 颜色/粗细显示名: Flutter ARB(`cRed`…/`wUltraFine`…) 与 core i18n
//!    (`Msg::ColorRed`…/`WidthHair`…) 各有一份。D11a 的 label_id 指向面板
//!    l10n(表单由 Dart 渲染); 颜色/宽度档位顺序与 `presets::COLOR_PRESETS` /
//!    `WIDTH_PRESETS` 一致(测试锁定), D11b 再评估是否由 core i18n 供面板复用。
//! 7. 除 shortcuts 分区标题外, Dart 表单的 label/description 都走 ARB, 无缺键。
//!
//! 新增/修改分区或控件: 只改本文件 + 测试里的 EXPECTED 迁移表(见 tests),
//! 不要把 label 文案搬进来。

use serde_json::{Value, json};

/// 表单控件类型; 非表单的三种分类也放在这里, 便于"每条登记键都有分类"。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Toggle,
    Select,
    Text,
    Password,
    Shortcuts,
    ColorGrid,
    WidthGrid,
    /// 由其它 UI(如面板 tab 切换)写入, 没有独立控件。
    Implicit,
    /// 派生只读值, 可能作为状态文案出现, 不可写。
    ReadOnly,
    /// 面板完全不展示(只有动作/热键或其它平台壳会改)。
    Hidden,
}

impl Control {
    pub fn as_str(self) -> &'static str {
        match self {
            Control::Toggle => "toggle",
            Control::Select => "select",
            Control::Text => "text",
            Control::Password => "password",
            Control::Shortcuts => "shortcuts",
            Control::ColorGrid => "colorGrid",
            Control::WidthGrid => "widthGrid",
            Control::Implicit => "implicit",
            Control::ReadOnly => "readOnly",
            Control::Hidden => "hidden",
        }
    }

    /// 是否为可渲染的表单控件(其余三种是分类, 不出控件)。
    pub fn is_form(self) -> bool {
        matches!(
            self,
            Control::Toggle
                | Control::Select
                | Control::Text
                | Control::Password
                | Control::Shortcuts
                | Control::ColorGrid
                | Control::WidthGrid
        )
    }

    /// 数值型控件(有档位/范围可校验)。
    pub fn is_numeric(self) -> bool {
        matches!(
            self,
            Control::Select | Control::ColorGrid | Control::WidthGrid
        )
    }
}

/// 数值显示单位 token(Dart 侧据此格式化; 不是文案)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Fps,
    Percent,
    /// 倍率(`2x` / `2×`)。
    Times,
    Hz,
}

impl Unit {
    pub fn as_str(self) -> &'static str {
        match self {
            Unit::Fps => "fps",
            Unit::Percent => "percent",
            Unit::Times => "x",
            Unit::Hz => "hz",
        }
    }
}

/// 数值范围; `step == 0.0` 表示档位离散但步长不均匀(以 `options` 为准)。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Range {
    pub min: f64,
    pub max: f64,
    pub step: f64,
}

impl Range {
    pub const fn new(min: f64, max: f64, step: f64) -> Self {
        Range { min, max, step }
    }

    pub fn to_json(self) -> Value {
        json!({ "min": self.min, "max": self.max, "step": self.step })
    }
}

/// 一个可选档位的值(bool 开关型 select 也用它)。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OptionValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(&'static str),
}

impl OptionValue {
    pub fn to_json(self) -> Value {
        match self {
            OptionValue::Bool(v) => json!(v),
            OptionValue::Int(v) => json!(v),
            OptionValue::Float(v) => json!(v),
            OptionValue::Str(v) => json!(v),
        }
    }

    /// 数值型档位; bool/字符串返回 None。
    pub fn as_f64(self) -> Option<f64> {
        match self {
            OptionValue::Int(v) => Some(v as f64),
            OptionValue::Float(v) => Some(v),
            OptionValue::Bool(_) | OptionValue::Str(_) => None,
        }
    }
}

/// 一个可选档位: 值 + 显示用的 l10n 键。`label_id` 为空串 = 纯数值档位,
/// 直接显示原值(可带 [`Unit`]), 没有 l10n 文案。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FormOption {
    pub value: OptionValue,
    pub label_id: &'static str,
}

const fn opt_int(value: i64, label_id: &'static str) -> FormOption {
    FormOption {
        value: OptionValue::Int(value),
        label_id,
    }
}

const fn opt_float(value: f64, label_id: &'static str) -> FormOption {
    FormOption {
        value: OptionValue::Float(value),
        label_id,
    }
}

const fn opt_bool(value: bool, label_id: &'static str) -> FormOption {
    FormOption {
        value: OptionValue::Bool(value),
        label_id,
    }
}

const fn opt_str(value: &'static str, label_id: &'static str) -> FormOption {
    FormOption {
        value: OptionValue::Str(value),
        label_id,
    }
}

/// 一个表单可见设置项的元数据。
#[derive(Debug, Clone, Copy)]
pub struct FormMeta {
    pub key: &'static str,
    /// [`SECTIONS`] 里的分区 id。
    pub section: &'static str,
    /// 分区内显示顺序(0 起连续)。
    pub order: u16,
    /// 标签 l10n 键(空串不允许; 数值档位选项才允许空)。
    pub label_id: &'static str,
    pub description_id: Option<&'static str>,
    pub control: Control,
    pub unit: Option<Unit>,
    pub range: Option<Range>,
    pub options: &'static [FormOption],
    /// 依赖 `uiConfig.features` 里的同名特性才显示(如 `invertInk`)。
    pub visible_when_feature: Option<&'static str>,
    /// 是否位于「更多设置」折叠区(与所属分区的 advanced 一致)。
    pub advanced: bool,
}

impl FormMeta {
    /// 单条元数据的 JSON 形状(uiConfig `settings[].form`)。
    pub fn form_json(&self) -> Value {
        json!({
            "key": self.key,
            "section": self.section,
            "order": self.order,
            "labelId": self.label_id,
            "descriptionId": self.description_id,
            "control": self.control.as_str(),
            "unit": self.unit.map(Unit::as_str),
            "range": self.range.map(Range::to_json),
            "options": self
                .options
                .iter()
                .map(|o| json!({ "value": o.value.to_json(), "labelId": o.label_id }))
                .collect::<Vec<_>>(),
            "visibleWhenFeature": self.visible_when_feature,
            "advanced": self.advanced,
        })
    }
}

/// 非表单登记项: 有分类, 不出控件。
#[derive(Debug, Clone, Copy)]
pub struct NonFormMeta {
    pub key: &'static str,
    pub control: Control,
}

/// 表单分区(数组顺序 = 面板显示顺序)。
#[derive(Debug, Clone, Copy)]
pub struct Section {
    pub id: &'static str,
    pub label_id: &'static str,
    pub advanced: bool,
}

pub const SECTIONS: &[Section] = &[
    Section {
        id: "pen",
        label_id: "pen",
        advanced: false,
    },
    Section {
        id: "canvasLook",
        label_id: "canvasLook",
        advanced: true,
    },
    Section {
        id: "notebookLook",
        label_id: "notebookLook",
        advanced: true,
    },
    Section {
        id: "recordExport",
        label_id: "recordExport",
        advanced: true,
    },
    Section {
        id: "dataBackup",
        label_id: "dataBackup",
        advanced: true,
    },
    Section {
        id: "sharedCanvas",
        label_id: "sharedCanvas",
        advanced: true,
    },
    Section {
        id: "doodleIdentity",
        label_id: "doodleIdentity",
        advanced: true,
    },
    Section {
        id: "systemAndAssist",
        label_id: "systemAndAssist",
        advanced: true,
    },
    Section {
        id: "language",
        label_id: "language",
        advanced: true,
    },
    Section {
        id: "shortcuts",
        // Dart 当前硬编码「快捷键 / Shortcuts」; 这里记 ARB 已有的稳定键,
        // D11b 统一标题时用它(见模块文档差异 5)。
        label_id: "hotkeys",
        advanced: true,
    },
];

// ── 档位表 ─────────────────────────────────────────────────────
// 顺序与 Dart 表单一致(D11b 迁移 oracle); label_id 为空 = 纯数值档位。

/// 颜色 10 档(索引 0..9); 顺序与 `presets::COLOR_PRESETS` 一致。
const COLOR_OPTIONS: &[FormOption] = &[
    opt_int(0, "cRed"),
    opt_int(1, "cOrange"),
    opt_int(2, "cYellow"),
    opt_int(3, "cGreen"),
    opt_int(4, "cCyan"),
    opt_int(5, "cBlue"),
    opt_int(6, "cPurple"),
    opt_int(7, "cPink"),
    opt_int(8, "cWhite"),
    opt_int(9, "cBlack"),
];

/// 粗细 8 档(索引 0..7); 顺序与 `presets::WIDTH_PRESETS` 一致。
const WIDTH_OPTIONS: &[FormOption] = &[
    opt_int(0, "wUltraFine"),
    opt_int(1, "wVeryFine"),
    opt_int(2, "wFine"),
    opt_int(3, "wMedium"),
    opt_int(4, "wThick"),
    opt_int(5, "wVeryThick"),
    opt_int(6, "wExtraThick"),
    opt_int(7, "wUltraThick"),
];

/// 气泡重播速度: 表单三档 1/2/4(core 钳制 1..8)。
const BUBBLE_REPLAY_SPEED_OPTIONS: &[FormOption] =
    &[opt_int(1, ""), opt_int(2, ""), opt_int(4, "")];

/// 网格尺寸: 表单三档 20/40/80(registry Kind::Float, core 钳制 10..200)。
const GRID_SIZE_OPTIONS: &[FormOption] = &[opt_int(20, ""), opt_int(40, ""), opt_int(80, "")];

const GRID_DIVIDER_OPTIONS: &[FormOption] = &[
    opt_int(0, "none"),
    opt_int(1, "twoColumnsSideBy"),
    opt_int(2, "twoColumnsStacked"),
    opt_int(3, "grid3x3"),
    opt_int(4, "cornell"),
];

const INVERT_FPS_OPTIONS: &[FormOption] = &[
    opt_int(10, ""),
    opt_int(30, ""),
    opt_int(60, ""),
    opt_int(100, ""),
];

const NOTEBOOK_STYLE_OPTIONS: &[FormOption] =
    &[opt_bool(true, "skeuoGlass"), opt_bool(false, "notePaper")];

const FLIP_EFFECT_OPTIONS: &[FormOption] =
    &[opt_int(0, "systemAnimation"), opt_int(1, "timeTunnel")];

const GIF_FPS_OPTIONS: &[FormOption] = &[
    opt_int(10, ""),
    opt_int(15, ""),
    opt_int(20, ""),
    opt_int(24, ""),
    opt_int(30, ""),
    opt_int(50, ""),
];

const GIF_RESOLUTION_OPTIONS: &[FormOption] = &[
    opt_float(0.25, ""),
    opt_float(0.5, ""),
    opt_float(0.75, ""),
    opt_float(1.0, ""),
];

const GIF_SPEED_OPTIONS: &[FormOption] = &[
    opt_float(0.5, ""),
    opt_float(1.0, ""),
    opt_float(2.0, ""),
    opt_float(3.0, ""),
    opt_float(4.0, ""),
    opt_float(8.0, ""),
    opt_float(10.0, ""),
    opt_float(15.0, ""),
    opt_float(20.0, ""),
];

const GIF_END_MODE_OPTIONS: &[FormOption] = &[
    opt_int(0, "stopOnLastFrame"),
    opt_int(1, "hold1SThen"),
    opt_int(2, "loopImmediately"),
];

const LANGUAGE_OPTIONS: &[FormOption] = &[opt_str("zh", "chinese"), opt_str("en", "english")];

// ── 构造器(让表里每行字段都显式, 不用 Default)────────────────────

const fn toggle(
    key: &'static str,
    section: &'static str,
    order: u16,
    label_id: &'static str,
    description_id: Option<&'static str>,
    visible_when_feature: Option<&'static str>,
    advanced: bool,
) -> FormMeta {
    FormMeta {
        key,
        section,
        order,
        label_id,
        description_id,
        control: Control::Toggle,
        unit: None,
        range: None,
        options: &[],
        visible_when_feature,
        advanced,
    }
}

const fn select(
    key: &'static str,
    section: &'static str,
    order: u16,
    label_id: &'static str,
    description_id: Option<&'static str>,
    options: &'static [FormOption],
    range: Option<Range>,
    unit: Option<Unit>,
    visible_when_feature: Option<&'static str>,
    advanced: bool,
) -> FormMeta {
    FormMeta {
        key,
        section,
        order,
        label_id,
        description_id,
        control: Control::Select,
        unit,
        range,
        options,
        visible_when_feature,
        advanced,
    }
}

const fn text(
    key: &'static str,
    section: &'static str,
    order: u16,
    label_id: &'static str,
    description_id: Option<&'static str>,
    advanced: bool,
) -> FormMeta {
    FormMeta {
        key,
        section,
        order,
        label_id,
        description_id,
        control: Control::Text,
        unit: None,
        range: None,
        options: &[],
        visible_when_feature: None,
        advanced,
    }
}

const fn shortcuts(
    key: &'static str,
    section: &'static str,
    order: u16,
    label_id: &'static str,
    advanced: bool,
) -> FormMeta {
    FormMeta {
        key,
        section,
        order,
        label_id,
        description_id: None,
        control: Control::Shortcuts,
        unit: None,
        range: None,
        options: &[],
        visible_when_feature: None,
        advanced,
    }
}

/// 网格控件: 档位是索引 0..len-1, 所以范围由档位数派生。
const fn grid(
    key: &'static str,
    section: &'static str,
    order: u16,
    label_id: &'static str,
    options: &'static [FormOption],
    control: Control,
    advanced: bool,
) -> FormMeta {
    FormMeta {
        key,
        section,
        order,
        label_id,
        description_id: None,
        control,
        unit: None,
        range: Some(Range::new(0.0, (options.len() - 1) as f64, 1.0)),
        options,
        visible_when_feature: None,
        advanced,
    }
}

/// 表单可见设置项(数组顺序 = 面板展示顺序: 按 [`SECTIONS`] 分区, 分区内按
/// `order`)。D11b 的 Dart 渲染按这个顺序生成控件。
pub const FORM_META: &[FormMeta] = &[
    // ── pen(笔; 首屏)──
    grid(
        "color",
        "pen",
        0,
        "color",
        COLOR_OPTIONS,
        Control::ColorGrid,
        false,
    ),
    grid(
        "width",
        "pen",
        1,
        "width",
        WIDTH_OPTIONS,
        Control::WidthGrid,
        false,
    ),
    // ── canvasLook(画布外观)──
    toggle(
        "showFreeCanvas",
        "canvasLook",
        0,
        "freeDoodleMode",
        Some("showTheFreeDoodle"),
        None,
        true,
    ),
    toggle(
        "showBubbleTab",
        "canvasLook",
        1,
        "bubbleTab",
        Some("showBubbleTabSub"),
        None,
        true,
    ),
    select(
        "bubbleReplaySpeed",
        "canvasLook",
        2,
        "bubbleReplaySpeed",
        None,
        BUBBLE_REPLAY_SPEED_OPTIONS,
        Some(Range::new(1.0, 4.0, 0.0)),
        Some(Unit::Times),
        None,
        true,
    ),
    toggle(
        "bubbleReplayHoldLast",
        "canvasLook",
        3,
        "bubbleReplayHoldLast",
        Some("bubbleReplayHoldLastSub"),
        None,
        true,
    ),
    toggle(
        "bubbleReplayAuto",
        "canvasLook",
        4,
        "bubbleReplayAuto",
        Some("bubbleReplayAutoSub"),
        None,
        true,
    ),
    toggle(
        "bubbleAutoNewPage",
        "canvasLook",
        5,
        "bubbleAutoNewPage",
        Some("bubbleAutoNewPageSub"),
        None,
        true,
    ),
    toggle(
        "grid",
        "canvasLook",
        6,
        "showGrid",
        Some("helpsAlignWhileDoodling"),
        None,
        true,
    ),
    select(
        "gridSize",
        "canvasLook",
        7,
        "gridSize",
        None,
        GRID_SIZE_OPTIONS,
        Some(Range::new(20.0, 80.0, 0.0)),
        None,
        None,
        true,
    ),
    select(
        "gridDivider",
        "canvasLook",
        8,
        "columns",
        None,
        GRID_DIVIDER_OPTIONS,
        Some(Range::new(0.0, 4.0, 1.0)),
        None,
        None,
        true,
    ),
    toggle(
        "gridFollowStrokes",
        "canvasLook",
        9,
        "gridFollowsDoodles",
        Some("whenOnTheGrid"),
        None,
        true,
    ),
    toggle(
        "glassFollowStrokes",
        "canvasLook",
        10,
        "glassFollowsDoodles",
        Some("whenOnTheGlass"),
        None,
        true,
    ),
    toggle(
        "outline",
        "canvasLook",
        11,
        "inkOutline",
        Some("dashedContrastOutlineVisible"),
        None,
        true,
    ),
    toggle(
        "softShadow",
        "canvasLook",
        12,
        "softShadow",
        Some("softDarkShadowUnder"),
        None,
        true,
    ),
    toggle(
        "invertInk",
        "canvasLook",
        13,
        "invertedInk",
        Some("strokesTakeThePer"),
        Some("invertInk"),
        true,
    ),
    select(
        "invertFps",
        "canvasLook",
        14,
        "invertTrackingFpsCap",
        None,
        INVERT_FPS_OPTIONS,
        Some(Range::new(10.0, 100.0, 0.0)),
        Some(Unit::Hz),
        Some("invertInk"),
        true,
    ),
    // ── notebookLook(活页本外观)──
    select(
        "notebookStyle",
        "notebookLook",
        0,
        "style",
        None,
        NOTEBOOK_STYLE_OPTIONS,
        None,
        None,
        None,
        true,
    ),
    select(
        "flipEffect",
        "notebookLook",
        1,
        "pageFlipEffect",
        None,
        FLIP_EFFECT_OPTIONS,
        Some(Range::new(0.0, 1.0, 1.0)),
        None,
        Some("pageFlipEffect"),
        true,
    ),
    toggle(
        "minimap",
        "notebookLook",
        2,
        "show10NearbyPages",
        Some("verticalMinimapOnThe"),
        None,
        true,
    ),
    toggle(
        "panelTransparent",
        "notebookLook",
        3,
        "truePassThroughExperimental",
        Some("cardsRevealTheDesktop"),
        None,
        true,
    ),
    // ── recordExport(录制与导出)──
    select(
        "gifFps",
        "recordExport",
        0,
        "frameRate",
        None,
        GIF_FPS_OPTIONS,
        Some(Range::new(10.0, 50.0, 0.0)),
        Some(Unit::Fps),
        None,
        true,
    ),
    select(
        "gifResolution",
        "recordExport",
        1,
        "resolution",
        None,
        GIF_RESOLUTION_OPTIONS,
        Some(Range::new(0.25, 1.0, 0.25)),
        Some(Unit::Percent),
        None,
        true,
    ),
    select(
        "gifSpeed",
        "recordExport",
        2,
        "speed",
        None,
        GIF_SPEED_OPTIONS,
        Some(Range::new(0.5, 20.0, 0.0)),
        Some(Unit::Times),
        None,
        true,
    ),
    select(
        "gifEndMode",
        "recordExport",
        3,
        "ending",
        None,
        GIF_END_MODE_OPTIONS,
        Some(Range::new(0.0, 2.0, 1.0)),
        None,
        None,
        true,
    ),
    // ── dataBackup(数据备份)──
    text("nasUrl", "dataBackup", 0, "nasBackupUrl", None, true),
    // ── sharedCanvas(共享画布; 依赖 chatIntegration 打开)──
    toggle(
        "shareCanvas",
        "sharedCanvas",
        0,
        "streamDoodlesLive",
        Some("pushedToThePeer"),
        None,
        true,
    ),
    // ── doodleIdentity(涂鸦身份; 登录字段依赖 chatIntegration)──
    toggle(
        "chatIntegration",
        "doodleIdentity",
        0,
        "enableHandwrittenMessagesSharing",
        Some("whenOff23"),
        None,
        true,
    ),
    text(
        "chatApiBase",
        "doodleIdentity",
        1,
        "serverAddress",
        Some("eGHttps192"),
        true,
    ),
    text("chatUser", "doodleIdentity", 2, "username", None, true),
    // ── systemAndAssist(系统与辅助)──
    toggle(
        "pressureMonitor",
        "systemAndAssist",
        0,
        "pressureMonitor",
        None,
        None,
        true,
    ),
    toggle(
        "launchAtLogin",
        "systemAndAssist",
        1,
        "launchAtLogin",
        None,
        None,
        true,
    ),
    // ── language(语言)──
    select(
        "language",
        "language",
        0,
        "language",
        None,
        LANGUAGE_OPTIONS,
        None,
        None,
        None,
        true,
    ),
    // ── shortcuts(快捷键)──
    shortcuts("shortcuts", "shortcuts", 0, "hotkeys", true),
];

/// 非表单登记项(有分类, 不出控件)。见模块文档的盘点说明。
pub const NON_FORM_META: &[NonFormMeta] = &[
    NonFormMeta {
        key: "chatHasPassword",
        control: Control::ReadOnly,
    },
    NonFormMeta {
        key: "ethereal",
        control: Control::Hidden,
    },
    NonFormMeta {
        key: "frostedGlass",
        control: Control::Hidden,
    },
    NonFormMeta {
        key: "infiniteCanvas",
        control: Control::Implicit,
    },
];

/// 某设置键的表单元数据(仅表单可见项; 非表单项见 [`non_form_meta`])。
pub fn meta_for(key: &str) -> Option<&'static FormMeta> {
    FORM_META.iter().find(|m| m.key == key)
}

/// 非表单登记项的分类; 未知/表单项返回 None。
pub fn non_form_meta(key: &str) -> Option<&'static NonFormMeta> {
    NON_FORM_META.iter().find(|m| m.key == key)
}

/// 全部可渲染的表单控件(按面板展示顺序)。
pub fn form_controls() -> &'static [FormMeta] {
    FORM_META
}

/// 分区表 JSON(uiConfig 顶层 `settingsSections`)。
pub fn sections_json() -> Value {
    Value::Array(
        SECTIONS
            .iter()
            .enumerate()
            .map(|(i, s)| {
                json!({
                    "id": s.id,
                    "labelId": s.label_id,
                    "advanced": s.advanced,
                    "order": i,
                })
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    //! D11a 漂移闸门。
    //!
    //! 本模块里的 EXPECTED_SECTIONS / EXPECTED 是**当前 Dart 表单的盘点快照**
    //! (分区、分区内顺序、label/description l10n 键、控件、单位、范围、档位、
    //! features 门控), 逐项抄自 main.settings_widgets.dart /
    //! main.settings_state.dart / main.dart / main.shortcuts.dart。
    //! 它是 D11b 的**迁移 oracle**:
    //! - 改 FORM_META(数据结构) → 必须同步改 EXPECTED, 并确认 Dart 表单也变了
    //!   (迁移期两边一起动);
    //! - 只改 Dart 表单(删控件/换档位/改文案键) → form_meta_matches_dart_inventory
    //!   或 dart_form_fingerprints_still_match 变红, 提示同步 core;
    //! - D11b 完成后 Dart 从 schema 渲染, 这个表即可删掉(oracle 完成使命)。
    //!
    //! 负向验证(交付说明里记录): 临时改 FORM_META 的某个 order 或 range 值,
    //! cargo test -p glaspen-core settings_form:: 必须变红且精确报出键名;
    //! 还原后绿。

    use super::*;
    use std::collections::HashSet;

    // ── 迁移 oracle: 当前 Dart 表单的盘点快照(按展示顺序) ──
    // 注意: 这是**独立副本**, 不从 FORM_META 派生; 两边不一致即漂移。

    struct ExpectedMeta {
        key: &'static str,
        section: &'static str,
        order: u16,
        label_id: &'static str,
        description_id: Option<&'static str>,
        control: Control,
        unit: Option<Unit>,
        range: Option<(f64, f64, f64)>,
        options: &'static [(OptionValue, &'static str)],
        feature: Option<&'static str>,
        advanced: bool,
    }

    const fn etoggle(
        key: &'static str,
        section: &'static str,
        order: u16,
        label_id: &'static str,
        description_id: Option<&'static str>,
        feature: Option<&'static str>,
        advanced: bool,
    ) -> ExpectedMeta {
        ExpectedMeta {
            key,
            section,
            order,
            label_id,
            description_id,
            control: Control::Toggle,
            unit: None,
            range: None,
            options: &[],
            feature,
            advanced,
        }
    }

    const fn eselect(
        key: &'static str,
        section: &'static str,
        order: u16,
        label_id: &'static str,
        description_id: Option<&'static str>,
        options: &'static [(OptionValue, &'static str)],
        range: Option<(f64, f64, f64)>,
        unit: Option<Unit>,
        feature: Option<&'static str>,
        advanced: bool,
    ) -> ExpectedMeta {
        ExpectedMeta {
            key,
            section,
            order,
            label_id,
            description_id,
            control: Control::Select,
            unit,
            range,
            options,
            feature,
            advanced,
        }
    }

    const fn etext(
        key: &'static str,
        section: &'static str,
        order: u16,
        label_id: &'static str,
        description_id: Option<&'static str>,
        advanced: bool,
    ) -> ExpectedMeta {
        ExpectedMeta {
            key,
            section,
            order,
            label_id,
            description_id,
            control: Control::Text,
            unit: None,
            range: None,
            options: &[],
            feature: None,
            advanced,
        }
    }

    const fn eshortcuts(
        key: &'static str,
        section: &'static str,
        order: u16,
        label_id: &'static str,
        advanced: bool,
    ) -> ExpectedMeta {
        ExpectedMeta {
            key,
            section,
            order,
            label_id,
            description_id: None,
            control: Control::Shortcuts,
            unit: None,
            range: None,
            options: &[],
            feature: None,
            advanced,
        }
    }

    const fn egrid(
        key: &'static str,
        section: &'static str,
        order: u16,
        label_id: &'static str,
        options: &'static [(OptionValue, &'static str)],
        control: Control,
        advanced: bool,
    ) -> ExpectedMeta {
        ExpectedMeta {
            key,
            section,
            order,
            label_id,
            description_id: None,
            control,
            unit: None,
            // 索引档位范围 = 0..档位数-1(与 FORM_META 的 grid() 同口径)
            range: Some((0.0, (options.len() - 1) as f64, 1.0)),
            options,
            feature: None,
            advanced,
        }
    }

    const EXP_COLOR: &[(OptionValue, &str)] = &[
        (OptionValue::Int(0), "cRed"),
        (OptionValue::Int(1), "cOrange"),
        (OptionValue::Int(2), "cYellow"),
        (OptionValue::Int(3), "cGreen"),
        (OptionValue::Int(4), "cCyan"),
        (OptionValue::Int(5), "cBlue"),
        (OptionValue::Int(6), "cPurple"),
        (OptionValue::Int(7), "cPink"),
        (OptionValue::Int(8), "cWhite"),
        (OptionValue::Int(9), "cBlack"),
    ];

    const EXP_WIDTH: &[(OptionValue, &str)] = &[
        (OptionValue::Int(0), "wUltraFine"),
        (OptionValue::Int(1), "wVeryFine"),
        (OptionValue::Int(2), "wFine"),
        (OptionValue::Int(3), "wMedium"),
        (OptionValue::Int(4), "wThick"),
        (OptionValue::Int(5), "wVeryThick"),
        (OptionValue::Int(6), "wExtraThick"),
        (OptionValue::Int(7), "wUltraThick"),
    ];

    const EXP_BUBBLE_REPLAY_SPEED: &[(OptionValue, &str)] = &[
        (OptionValue::Int(1), ""),
        (OptionValue::Int(2), ""),
        (OptionValue::Int(4), ""),
    ];

    const EXP_GRID_SIZE: &[(OptionValue, &str)] = &[
        (OptionValue::Int(20), ""),
        (OptionValue::Int(40), ""),
        (OptionValue::Int(80), ""),
    ];

    const EXP_GRID_DIVIDER: &[(OptionValue, &str)] = &[
        (OptionValue::Int(0), "none"),
        (OptionValue::Int(1), "twoColumnsSideBy"),
        (OptionValue::Int(2), "twoColumnsStacked"),
        (OptionValue::Int(3), "grid3x3"),
        (OptionValue::Int(4), "cornell"),
    ];

    const EXP_INVERT_FPS: &[(OptionValue, &str)] = &[
        (OptionValue::Int(10), ""),
        (OptionValue::Int(30), ""),
        (OptionValue::Int(60), ""),
        (OptionValue::Int(100), ""),
    ];

    const EXP_NOTEBOOK_STYLE: &[(OptionValue, &str)] = &[
        (OptionValue::Bool(true), "skeuoGlass"),
        (OptionValue::Bool(false), "notePaper"),
    ];

    const EXP_FLIP_EFFECT: &[(OptionValue, &str)] = &[
        (OptionValue::Int(0), "systemAnimation"),
        (OptionValue::Int(1), "timeTunnel"),
    ];

    const EXP_GIF_FPS: &[(OptionValue, &str)] = &[
        (OptionValue::Int(10), ""),
        (OptionValue::Int(15), ""),
        (OptionValue::Int(20), ""),
        (OptionValue::Int(24), ""),
        (OptionValue::Int(30), ""),
        (OptionValue::Int(50), ""),
    ];

    const EXP_GIF_RESOLUTION: &[(OptionValue, &str)] = &[
        (OptionValue::Float(0.25), ""),
        (OptionValue::Float(0.5), ""),
        (OptionValue::Float(0.75), ""),
        (OptionValue::Float(1.0), ""),
    ];

    const EXP_GIF_SPEED: &[(OptionValue, &str)] = &[
        (OptionValue::Float(0.5), ""),
        (OptionValue::Float(1.0), ""),
        (OptionValue::Float(2.0), ""),
        (OptionValue::Float(3.0), ""),
        (OptionValue::Float(4.0), ""),
        (OptionValue::Float(8.0), ""),
        (OptionValue::Float(10.0), ""),
        (OptionValue::Float(15.0), ""),
        (OptionValue::Float(20.0), ""),
    ];

    const EXP_GIF_END_MODE: &[(OptionValue, &str)] = &[
        (OptionValue::Int(0), "stopOnLastFrame"),
        (OptionValue::Int(1), "hold1SThen"),
        (OptionValue::Int(2), "loopImmediately"),
    ];

    const EXP_LANGUAGE: &[(OptionValue, &str)] = &[
        (OptionValue::Str("zh"), "chinese"),
        (OptionValue::Str("en"), "english"),
    ];

    /// 分区盘点(数组顺序 = main.dart 里的显示顺序)。
    const EXPECTED_SECTIONS: &[(&str, &str, bool)] = &[
        ("pen", "pen", false),
        ("canvasLook", "canvasLook", true),
        ("notebookLook", "notebookLook", true),
        ("recordExport", "recordExport", true),
        ("dataBackup", "dataBackup", true),
        ("sharedCanvas", "sharedCanvas", true),
        ("doodleIdentity", "doodleIdentity", true),
        ("systemAndAssist", "systemAndAssist", true),
        ("language", "language", true),
        // Dart 里该分区标题由 _shortcutSectionTitle() 硬编码; 这里记 ARB 稳定键
        ("shortcuts", "hotkeys", true),
    ];

    const EXPECTED: &[ExpectedMeta] = &[
        // ── pen ──
        egrid(
            "color",
            "pen",
            0,
            "color",
            EXP_COLOR,
            Control::ColorGrid,
            false,
        ),
        egrid(
            "width",
            "pen",
            1,
            "width",
            EXP_WIDTH,
            Control::WidthGrid,
            false,
        ),
        // ── canvasLook ──
        etoggle(
            "showFreeCanvas",
            "canvasLook",
            0,
            "freeDoodleMode",
            Some("showTheFreeDoodle"),
            None,
            true,
        ),
        etoggle(
            "showBubbleTab",
            "canvasLook",
            1,
            "bubbleTab",
            Some("showBubbleTabSub"),
            None,
            true,
        ),
        eselect(
            "bubbleReplaySpeed",
            "canvasLook",
            2,
            "bubbleReplaySpeed",
            None,
            EXP_BUBBLE_REPLAY_SPEED,
            Some((1.0, 4.0, 0.0)),
            Some(Unit::Times),
            None,
            true,
        ),
        etoggle(
            "bubbleReplayHoldLast",
            "canvasLook",
            3,
            "bubbleReplayHoldLast",
            Some("bubbleReplayHoldLastSub"),
            None,
            true,
        ),
        etoggle(
            "bubbleReplayAuto",
            "canvasLook",
            4,
            "bubbleReplayAuto",
            Some("bubbleReplayAutoSub"),
            None,
            true,
        ),
        etoggle(
            "bubbleAutoNewPage",
            "canvasLook",
            5,
            "bubbleAutoNewPage",
            Some("bubbleAutoNewPageSub"),
            None,
            true,
        ),
        etoggle(
            "grid",
            "canvasLook",
            6,
            "showGrid",
            Some("helpsAlignWhileDoodling"),
            None,
            true,
        ),
        eselect(
            "gridSize",
            "canvasLook",
            7,
            "gridSize",
            None,
            EXP_GRID_SIZE,
            Some((20.0, 80.0, 0.0)),
            None,
            None,
            true,
        ),
        eselect(
            "gridDivider",
            "canvasLook",
            8,
            "columns",
            None,
            EXP_GRID_DIVIDER,
            Some((0.0, 4.0, 1.0)),
            None,
            None,
            true,
        ),
        etoggle(
            "gridFollowStrokes",
            "canvasLook",
            9,
            "gridFollowsDoodles",
            Some("whenOnTheGrid"),
            None,
            true,
        ),
        etoggle(
            "glassFollowStrokes",
            "canvasLook",
            10,
            "glassFollowsDoodles",
            Some("whenOnTheGlass"),
            None,
            true,
        ),
        etoggle(
            "outline",
            "canvasLook",
            11,
            "inkOutline",
            Some("dashedContrastOutlineVisible"),
            None,
            true,
        ),
        etoggle(
            "softShadow",
            "canvasLook",
            12,
            "softShadow",
            Some("softDarkShadowUnder"),
            None,
            true,
        ),
        etoggle(
            "invertInk",
            "canvasLook",
            13,
            "invertedInk",
            Some("strokesTakeThePer"),
            Some("invertInk"),
            true,
        ),
        eselect(
            "invertFps",
            "canvasLook",
            14,
            "invertTrackingFpsCap",
            None,
            EXP_INVERT_FPS,
            Some((10.0, 100.0, 0.0)),
            Some(Unit::Hz),
            Some("invertInk"),
            true,
        ),
        // ── notebookLook ──
        eselect(
            "notebookStyle",
            "notebookLook",
            0,
            "style",
            None,
            EXP_NOTEBOOK_STYLE,
            None,
            None,
            None,
            true,
        ),
        eselect(
            "flipEffect",
            "notebookLook",
            1,
            "pageFlipEffect",
            None,
            EXP_FLIP_EFFECT,
            Some((0.0, 1.0, 1.0)),
            None,
            Some("pageFlipEffect"),
            true,
        ),
        etoggle(
            "minimap",
            "notebookLook",
            2,
            "show10NearbyPages",
            Some("verticalMinimapOnThe"),
            None,
            true,
        ),
        etoggle(
            "panelTransparent",
            "notebookLook",
            3,
            "truePassThroughExperimental",
            Some("cardsRevealTheDesktop"),
            None,
            true,
        ),
        // ── recordExport ──
        eselect(
            "gifFps",
            "recordExport",
            0,
            "frameRate",
            None,
            EXP_GIF_FPS,
            Some((10.0, 50.0, 0.0)),
            Some(Unit::Fps),
            None,
            true,
        ),
        eselect(
            "gifResolution",
            "recordExport",
            1,
            "resolution",
            None,
            EXP_GIF_RESOLUTION,
            Some((0.25, 1.0, 0.25)),
            Some(Unit::Percent),
            None,
            true,
        ),
        eselect(
            "gifSpeed",
            "recordExport",
            2,
            "speed",
            None,
            EXP_GIF_SPEED,
            Some((0.5, 20.0, 0.0)),
            Some(Unit::Times),
            None,
            true,
        ),
        eselect(
            "gifEndMode",
            "recordExport",
            3,
            "ending",
            None,
            EXP_GIF_END_MODE,
            Some((0.0, 2.0, 1.0)),
            None,
            None,
            true,
        ),
        // ── dataBackup ──
        etext("nasUrl", "dataBackup", 0, "nasBackupUrl", None, true),
        // ── sharedCanvas ──
        etoggle(
            "shareCanvas",
            "sharedCanvas",
            0,
            "streamDoodlesLive",
            Some("pushedToThePeer"),
            None,
            true,
        ),
        // ── doodleIdentity ──
        etoggle(
            "chatIntegration",
            "doodleIdentity",
            0,
            "enableHandwrittenMessagesSharing",
            Some("whenOff23"),
            None,
            true,
        ),
        etext(
            "chatApiBase",
            "doodleIdentity",
            1,
            "serverAddress",
            Some("eGHttps192"),
            true,
        ),
        etext("chatUser", "doodleIdentity", 2, "username", None, true),
        // ── systemAndAssist ──
        etoggle(
            "pressureMonitor",
            "systemAndAssist",
            0,
            "pressureMonitor",
            None,
            None,
            true,
        ),
        etoggle(
            "launchAtLogin",
            "systemAndAssist",
            1,
            "launchAtLogin",
            None,
            None,
            true,
        ),
        // ── language ──
        eselect(
            "language",
            "language",
            0,
            "language",
            None,
            EXP_LANGUAGE,
            None,
            None,
            None,
            true,
        ),
        // ── shortcuts ──
        eshortcuts("shortcuts", "shortcuts", 0, "hotkeys", true),
    ];

    // ── 1. 迁移 oracle: FORM_META/SECTIONS == Dart 表单盘点 ──

    #[test]
    fn form_meta_matches_dart_inventory() {
        assert_eq!(FORM_META.len(), EXPECTED.len(), "表单项数漂移");
        assert_eq!(FORM_META.len(), 34, "当前 Dart 表单可见设置项应为 34 条");
        for (i, (meta, exp)) in FORM_META.iter().zip(EXPECTED.iter()).enumerate() {
            assert_eq!(
                meta.key, exp.key,
                "第 {i} 项的键/顺序与 Dart 表单盘点不一致"
            );
            assert_eq!(meta.section, exp.section, "{}: 分区漂移", exp.key);
            assert_eq!(meta.order, exp.order, "{}: 分区内顺序漂移", exp.key);
            assert_eq!(meta.label_id, exp.label_id, "{}: label_id 漂移", exp.key);
            assert_eq!(
                meta.description_id, exp.description_id,
                "{}: description_id 漂移",
                exp.key
            );
            assert_eq!(meta.control, exp.control, "{}: 控件类型漂移", exp.key);
            assert_eq!(meta.unit, exp.unit, "{}: 单位漂移", exp.key);
            assert_eq!(
                meta.range.map(|r| (r.min, r.max, r.step)),
                exp.range,
                "{}: 范围漂移",
                exp.key
            );
            let got: Vec<(OptionValue, &'static str)> =
                meta.options.iter().map(|o| (o.value, o.label_id)).collect();
            assert_eq!(got.as_slice(), exp.options, "{}: 档位漂移", exp.key);
            assert_eq!(
                meta.visible_when_feature, exp.feature,
                "{}: features 门控漂移",
                exp.key
            );
            assert_eq!(meta.advanced, exp.advanced, "{}: advanced 漂移", exp.key);
        }
    }

    #[test]
    fn sections_match_dart_display_order() {
        assert_eq!(SECTIONS.len(), EXPECTED_SECTIONS.len());
        for (i, (s, (id, label_id, advanced))) in
            SECTIONS.iter().zip(EXPECTED_SECTIONS.iter()).enumerate()
        {
            assert_eq!(s.id, *id, "第 {i} 个分区 id/顺序漂移");
            assert_eq!(s.label_id, *label_id, "分区 {} 的 label_id 漂移", s.id);
            assert_eq!(s.advanced, *advanced, "分区 {} 的 advanced 漂移", s.id);
        }
        assert_eq!(
            SECTIONS.iter().filter(|s| !s.advanced).count(),
            1,
            "只有 pen 常在首屏"
        );
    }

    // ── 2. 登记表完整性: 每条 key 恰好一种分类, 不会静默消失 ──

    #[test]
    fn registry_is_fully_classified() {
        let registry: HashSet<&str> = crate::settings::SETTINGS.iter().map(|s| s.key).collect();
        assert_eq!(registry.len(), 38, "登记表项数变了: 同步盘点表单分类");

        let mut classified: HashSet<&str> = HashSet::new();
        for m in FORM_META {
            assert!(
                m.control.is_form(),
                "{} 在 FORM_META 里却是非表单控件",
                m.key
            );
            assert!(classified.insert(m.key), "{} 在 FORM_META 里重复", m.key);
            assert!(
                crate::settings::get(m.key).is_some(),
                "{} 不在 settings 登记表(Dart 表单渲染了未登记的键?)",
                m.key
            );
            assert!(
                crate::settings::is_settable(m.key),
                "{} 是只读项, 不该有可渲染控件",
                m.key
            );
        }
        for m in NON_FORM_META {
            assert!(!m.control.is_form(), "{} 应是无控件的分类", m.key);
            assert!(
                classified.insert(m.key),
                "{} 在 NON_FORM_META 里重复",
                m.key
            );
            assert!(
                crate::settings::get(m.key).is_some(),
                "{} 不在 settings 登记表",
                m.key
            );
        }
        let missing: Vec<&&str> = registry.difference(&classified).collect();
        let extra: Vec<&&str> = classified.difference(&registry).collect();
        assert!(
            missing.is_empty(),
            "这些登记键没有任何表单分类: {missing:?}"
        );
        assert!(extra.is_empty(), "分类表里有登记表之外的键: {extra:?}");
        assert_eq!(FORM_META.len() + NON_FORM_META.len(), registry.len());

        // 分类是显式盘点(不是猜的): 非表单项必须逐一对上模块文档。
        for (key, want) in [
            ("infiniteCanvas", Control::Implicit),
            ("chatHasPassword", Control::ReadOnly),
            ("frostedGlass", Control::Hidden),
            ("ethereal", Control::Hidden),
        ] {
            let got = non_form_meta(key)
                .unwrap_or_else(|| panic!("{key} 缺非表单分类"))
                .control;
            assert_eq!(got, want, "{key} 的分类漂移");
            assert!(meta_for(key).is_none(), "{key} 不应有表单控件");
        }
    }

    // ── 3. 元数据自洽: 分区/顺序/label/控件/范围/档位 ──

    #[test]
    fn form_meta_is_well_formed() {
        let cfg: serde_json::Value =
            serde_json::from_str(&crate::ui::ui_config_json(crate::ui::Platform::Macos))
                .expect("uiConfig 必须是合法 JSON");

        let mut keys = HashSet::new();
        let mut labels = HashSet::new();
        for m in FORM_META {
            assert!(keys.insert(m.key), "{} 重复", m.key);
            assert!(!m.label_id.is_empty(), "{} 缺 label_id", m.key);
            assert!(labels.insert(m.label_id), "label_id 重复: {}", m.label_id);

            let section = SECTIONS
                .iter()
                .find(|s| s.id == m.section)
                .unwrap_or_else(|| panic!("{} 的分区 {} 不在 SECTIONS", m.key, m.section));
            assert_eq!(
                m.advanced, section.advanced,
                "{} 的 advanced 与分区 {} 不一致",
                m.key, m.section
            );

            match m.control {
                Control::Toggle | Control::Text | Control::Password | Control::Shortcuts => {
                    assert!(m.options.is_empty(), "{} 非选择类控件不该有档位", m.key);
                    assert!(m.range.is_none(), "{} 非选择类控件不该有范围", m.key);
                    assert!(m.unit.is_none(), "{} 非数值控件不该有单位", m.key);
                }
                Control::Select | Control::ColorGrid | Control::WidthGrid => {
                    assert!(!m.options.is_empty(), "{} 选择类控件没有档位", m.key);
                    let numeric = m.options.iter().all(|o| o.value.as_f64().is_some());
                    if numeric {
                        assert!(m.range.is_some(), "{} 数值选择控件没有范围", m.key);
                    } else {
                        assert!(m.range.is_none(), "{} 非数值选择控件不该有范围", m.key);
                    }
                    for (i, a) in m.options.iter().enumerate() {
                        for b in &m.options[i + 1..] {
                            assert_ne!(a.value, b.value, "{} 档位值重复", m.key);
                        }
                    }
                }
                Control::Implicit | Control::ReadOnly | Control::Hidden => {
                    panic!("{} 是非表单分类, 不该出现在 FORM_META", m.key)
                }
            }
            if m.unit.is_some() {
                assert!(m.control.is_numeric(), "{} 非数值控件带单位", m.key);
            }
            if let Some(feature) = m.visible_when_feature {
                assert!(
                    cfg["features"]
                        .as_object()
                        .is_some_and(|o| o.contains_key(feature)),
                    "{} 的 visible_when_feature={feature} 不在 uiConfig.features 里",
                    m.key
                );
            }
        }

        // 分区内 order 从 0 连续, 且 FORM_META 数组顺序 = 分区顺序 + 分区内 order。
        for s in SECTIONS {
            let orders: Vec<u16> = FORM_META
                .iter()
                .filter(|m| m.section == s.id)
                .map(|m| m.order)
                .collect();
            assert!(!orders.is_empty(), "分区 {} 没有任何设置项", s.id);
            assert_eq!(
                orders,
                (0..orders.len() as u16).collect::<Vec<_>>(),
                "分区 {} 的 order 不连续/未排序",
                s.id
            );
        }
        let mut last = 0usize;
        for m in FORM_META {
            let idx = SECTIONS
                .iter()
                .position(|s| s.id == m.section)
                .expect("分区必须存在");
            assert!(idx >= last, "{} 的分区顺序与 SECTIONS 不一致", m.key);
            last = idx;
        }

        // 首屏(非 advanced)只有笔的两个网格。
        let base: Vec<&str> = FORM_META
            .iter()
            .filter(|m| !m.advanced)
            .map(|m| m.key)
            .collect();
        assert_eq!(base, ["color", "width"]);

        // password 控件留给 D11b 的非持久化字段(chatPassword 不在登记表)。
        assert!(
            FORM_META.iter().all(|m| m.control != Control::Password),
            "当前没有登记表内的密码项; chatPassword 是协议特殊键(见模块文档)"
        );

        // 辅助函数与表一致。
        assert_eq!(form_controls().len(), FORM_META.len());
        assert_eq!(meta_for("color").map(|m| m.key), Some("color"));
        assert!(meta_for("infiniteCanvas").is_none());
        assert!(non_form_meta("color").is_none());
        assert!(meta_for("noSuchSetting").is_none());
    }

    // ── 4. 范围与档位一致 ──

    #[test]
    fn range_agrees_with_options() {
        for m in FORM_META.iter().filter(|m| {
            m.control.is_numeric() && m.options.iter().all(|o| o.value.as_f64().is_some())
        }) {
            let r = m.range.unwrap_or_else(|| panic!("{} 缺范围", m.key));
            let nums: Vec<f64> = m
                .options
                .iter()
                .map(|o| {
                    o.value
                        .as_f64()
                        .unwrap_or_else(|| panic!("{} 的档位不是数值", m.key))
                })
                .collect();
            assert!(nums.len() >= 2, "{} 数值档位太少", m.key);
            assert!(
                nums.windows(2).all(|w| w[1] > w[0]),
                "{} 的档位必须严格递增",
                m.key
            );
            assert!(
                (nums[0] - r.min).abs() < 1e-9 && (nums[nums.len() - 1] - r.max).abs() < 1e-9,
                "{} 的 range({},{}) 与首尾档位({},{}) 不一致",
                m.key,
                r.min,
                r.max,
                nums[0],
                nums[nums.len() - 1]
            );
            let diffs: Vec<f64> = nums.windows(2).map(|w| w[1] - w[0]).collect();
            if r.step > 0.0 {
                assert!(
                    diffs.iter().all(|d| (d - r.step).abs() < 1e-9),
                    "{} 的 step={} 与档位间距 {:?} 不一致",
                    m.key,
                    r.step,
                    diffs
                );
            } else {
                let first = diffs[0];
                assert!(
                    diffs.iter().any(|d| (d - first).abs() > 1e-9),
                    "{} step=0 表示不均匀, 但档位等距({first}); 应填真实 step",
                    m.key
                );
            }
        }
    }

    // ── 5. l10n 键必须真实存在(en/zh 同步) ──

    fn arb_keys(src: &str) -> HashSet<String> {
        let v: serde_json::Value = serde_json::from_str(src).expect("ARB 必须是合法 JSON");
        v.as_object()
            .expect("ARB 顶层必须是对象")
            .keys()
            .filter(|k| !k.starts_with('@'))
            .cloned()
            .collect()
    }

    #[test]
    fn label_ids_exist_in_flutter_arb() {
        let en = arb_keys(include_str!(
            "../../../flutter_settings/lib/l10n/app_en.arb"
        ));
        let zh = arb_keys(include_str!(
            "../../../flutter_settings/lib/l10n/app_zh.arb"
        ));
        assert_eq!(en, zh, "中英 ARB 键集合必须一致");

        let mut need: Vec<(&str, &str)> = Vec::new();
        for s in SECTIONS {
            need.push(("分区", s.label_id));
        }
        for m in FORM_META {
            need.push(("label", m.label_id));
            if let Some(d) = m.description_id {
                need.push(("description", d));
            }
            for o in m.options {
                if o.label_id.is_empty() {
                    // 只有纯数值档位允许无 l10n 文案(由 unit 决定显示格式)
                    assert!(
                        o.value.as_f64().is_some(),
                        "{} 的非数值档位缺 label_id",
                        m.key
                    );
                } else {
                    need.push(("option", o.label_id));
                }
            }
        }
        let missing: Vec<&(&str, &str)> = need.iter().filter(|(_, k)| !en.contains(*k)).collect();
        assert!(missing.is_empty(), "面板 ARB 缺这些键: {missing:?}");
    }

    // ── 6. 网格档位与 core 预设同源 ──

    #[test]
    fn color_and_width_grids_match_core_presets() {
        let color = meta_for("color").expect("color 必须有表单元数据");
        assert_eq!(color.control, Control::ColorGrid);
        assert_eq!(
            color.options.len(),
            crate::presets::COLOR_PRESETS.len(),
            "色板档数与 presets::COLOR_PRESETS 不一致"
        );
        let color_idx: Vec<i64> = color
            .options
            .iter()
            .map(|o| match o.value {
                OptionValue::Int(v) => v,
                _ => panic!("颜色档位必须是索引"),
            })
            .collect();
        assert_eq!(
            color_idx,
            (0..color.options.len() as i64).collect::<Vec<_>>()
        );

        let width = meta_for("width").expect("width 必须有表单元数据");
        assert_eq!(width.control, Control::WidthGrid);
        assert_eq!(
            width.options.len(),
            crate::presets::WIDTH_PRESETS.len(),
            "粗细档数与 presets::WIDTH_PRESETS 不一致"
        );
        let width_idx: Vec<i64> = width
            .options
            .iter()
            .map(|o| match o.value {
                OptionValue::Int(v) => v,
                _ => panic!("粗细档位必须是索引"),
            })
            .collect();
        assert_eq!(
            width_idx,
            (0..width.options.len() as i64).collect::<Vec<_>>()
        );
    }

    // ── 7. 档位必须能被 core 钳制接受(表单是 core 区间的子集) ──

    #[test]
    fn numeric_options_survive_core_clamps() {
        for m in FORM_META.iter().filter(|m| m.control.is_numeric()) {
            for o in m.options {
                let Some(v) = o.value.as_f64() else {
                    continue;
                };
                let clamped = crate::presets::clamp_setting_double(m.key, v);
                assert!(
                    (clamped - v).abs() < 1e-9,
                    "{}={} 会被 presets 钳成 {}; 表单档位必须是 core 合法区间的子集",
                    m.key,
                    v,
                    clamped
                );
                if matches!(m.key, "gifFps" | "gifEndMode") {
                    let clamped_int = crate::presets::clamp_setting_int(m.key, v as i32);
                    assert_eq!(
                        clamped_int as f64, v,
                        "{}={} 会被 presets 整数钳制",
                        m.key, v
                    );
                }
            }
        }

        // settings_apply 的行内钳制/白名单与表单档位完全一致。
        for (key, want) in [
            ("bubbleReplaySpeed", vec![1i64, 2, 4]),
            ("gridDivider", vec![0i64, 1, 2, 3, 4]),
            ("flipEffect", vec![0i64, 1]),
            ("invertFps", vec![10i64, 30, 60, 100]),
        ] {
            let m = meta_for(key).unwrap_or_else(|| panic!("{key} 缺表单元数据"));
            let got: Vec<i64> = m
                .options
                .iter()
                .map(|o| match o.value {
                    OptionValue::Int(v) => v,
                    _ => panic!("{key} 的档位必须是整数"),
                })
                .collect();
            assert_eq!(got, want, "{key} 的档位与 core 白名单不一致");
        }
    }

    // ── 8. uiConfig 里下发的 JSON 形状 ──

    #[test]
    fn form_json_shape_is_stable() {
        let color = meta_for("color").unwrap();
        let j = color.form_json();
        for key in [
            "key",
            "section",
            "order",
            "labelId",
            "descriptionId",
            "control",
            "unit",
            "range",
            "options",
            "visibleWhenFeature",
            "advanced",
        ] {
            assert!(j.get(key).is_some(), "form_json 缺字段 {key}");
        }
        assert_eq!(j["key"], "color");
        assert_eq!(j["section"], "pen");
        assert_eq!(j["order"], 0);
        assert_eq!(j["labelId"], "color");
        assert!(j["descriptionId"].is_null());
        assert_eq!(j["control"], "colorGrid");
        assert_eq!(j["advanced"], false);
        assert_eq!(j["options"].as_array().unwrap().len(), 10);
        assert_eq!(j["options"][0]["value"], 0);
        assert_eq!(j["options"][0]["labelId"], "cRed");
        assert_eq!(
            j["range"],
            serde_json::json!({"min": 0.0, "max": 9.0, "step": 1.0})
        );

        let invert_fps = meta_for("invertFps").unwrap().form_json();
        assert_eq!(invert_fps["unit"], "hz");
        assert_eq!(invert_fps["visibleWhenFeature"], "invertInk");
        assert_eq!(invert_fps["control"], "select");

        let shortcuts = meta_for("shortcuts").unwrap().form_json();
        assert_eq!(shortcuts["control"], "shortcuts");
        assert!(shortcuts["options"].as_array().unwrap().is_empty());
        assert!(shortcuts["range"].is_null());

        let secs = sections_json();
        let arr = secs.as_array().expect("sections_json 必须是数组");
        assert_eq!(arr.len(), SECTIONS.len());
        assert_eq!(arr[0]["id"], "pen");
        assert_eq!(arr[0]["labelId"], "pen");
        assert_eq!(arr[0]["advanced"], false);
        assert_eq!(arr[0]["order"], 0);
        assert_eq!(arr[2]["id"], "notebookLook");
        assert_eq!(arr[arr.len() - 1]["id"], "shortcuts");
    }

    // ── 9. 每个平台的表单可见项(与 registry 平台标记叠加) ──

    #[test]
    fn form_visibility_follows_platform_registry() {
        let count = |mac: bool| {
            FORM_META
                .iter()
                .filter(|m| {
                    let def = crate::settings::get(m.key).expect("表单键必须在登记表");
                    if mac {
                        def.platforms.mac
                    } else {
                        def.platforms.win
                    }
                })
                .count()
        };
        assert_eq!(count(true), 34, "macOS 表单可见项数漂移");
        assert_eq!(
            count(false),
            33,
            "Windows 表单可见项数漂移(flipEffect 仅 macOS)"
        );
        assert!(
            FORM_META.iter().any(|m| m.key == "flipEffect"),
            "flipEffect 的控件仍在表单里(macOS), 平台可用性由 registry/features 决定"
        );
    }

    // ── 10. Dart 源码指纹: 控件/档位/分区标题仍与盘点一致 ──

    #[test]
    fn dart_form_fingerprints_still_match() {
        let state = include_str!("../../../flutter_settings/lib/main.settings_state.dart");
        let widgets = include_str!("../../../flutter_settings/lib/main.settings_widgets.dart");
        let main = include_str!("../../../flutter_settings/lib/main.dart");
        let shortcuts_src = include_str!("../../../flutter_settings/lib/main.shortcuts.dart");
        let controls = format!("{main}\n{widgets}\n{state}\n{shortcuts_src}");

        // 分区标题以 l10n getter 出现在 main.dart, 出现顺序 = SECTIONS 顺序。
        let mut last = 0usize;
        for s in SECTIONS.iter().filter(|s| s.id != "shortcuts") {
            let needle = format!("AppLocalizations.of(context)!.{}", s.label_id);
            let at = main
                .find(&needle)
                .unwrap_or_else(|| panic!("main.dart 里找不到分区 {} 的标题", s.id));
            assert!(
                at >= last,
                "分区 {} 在 main.dart 里的出现顺序与 SECTIONS 不一致",
                s.id
            );
            last = at;
        }
        // shortcuts 分区标题是硬编码 helper(模块文档差异 5)。
        assert!(
            main.contains("_shortcutSectionTitle(),"),
            "main.dart 不再用 _shortcutSectionTitle() 作 shortcuts 分区标题"
        );
        assert!(
            shortcuts_src.contains("String _shortcutSectionTitle() =>"),
            "main.shortcuts.dart 缺少 _shortcutSectionTitle()"
        );

        // 「更多设置」折叠: pen 在折叠器之前, 其余分区都在折叠区内。
        let pen_at = main
            .find("AppLocalizations.of(context)!.pen")
            .expect("缺 pen 分区");
        let adv_at = main
            .find("if (_showAdvanced) ...[")
            .expect("main.dart 缺「更多设置」折叠器");
        assert!(pen_at < adv_at, "pen 分区应始终显示(在折叠器之前)");
        for s in SECTIONS
            .iter()
            .filter(|s| s.advanced && s.id != "shortcuts")
        {
            let needle = format!("AppLocalizations.of(context)!.{}", s.label_id);
            let at = main.find(&needle).expect("高级分区标题必须存在");
            assert!(
                at > adv_at,
                "分区 {} 不在「更多设置」折叠区内(advanced 标记漂移)",
                s.id
            );
        }

        // 每个表单控件仍从同一处写设置(控件被删/改名则此处红)。
        for m in FORM_META {
            let needle = format!("setSetting('{}'", m.key);
            assert!(
                controls.contains(&needle),
                "Dart 表单里找不到 {} 的 setSetting 调用(控件被删/改名?)",
                m.key
            );
        }

        // 档位字面量指纹(保守几处: 故意改档位而不改 core 时能发现)。
        for (label, needle) in [
            (
                "gif fps 档位",
                "const fpsOptions = [10, 15, 20, 24, 30, 50];",
            ),
            (
                "gif 分辨率档位",
                "const resOptions = [0.25, 0.5, 0.75, 1.0];",
            ),
            (
                "gif 速度档位",
                "const speedOptions = [0.5, 1.0, 2.0, 3.0, 4.0, 8.0, 10.0, 15.0, 20.0];",
            ),
            (
                "网格尺寸档位",
                "[20, 40, 80].map((v) => ButtonSegment(value: v",
            ),
            (
                "气泡重播速度档位",
                "[1, 2, 4].map((v) => ButtonSegment(value: v",
            ),
            (
                "invert fps 档位 10",
                "ButtonSegment(value: 10, label: Text('10Hz'))",
            ),
            (
                "invert fps 档位 100",
                "ButtonSegment(value: 100, label: Text('100Hz'))",
            ),
            (
                "颜色名 ARB 引用",
                "[l.cRed, l.cOrange, l.cYellow, l.cGreen, l.cCyan, l.cBlue, l.cPurple, l.cPink, l.cWhite, l.cBlack]",
            ),
            (
                "粗细名 ARB 引用",
                "[l.wUltraFine, l.wVeryFine, l.wFine, l.wMedium, l.wThick, l.wVeryThick, l.wExtraThick, l.wUltraThick]",
            ),
            (
                "invertInk 特性门控",
                "uiConfig.features['invertInk'] == true",
            ),
            (
                "pageFlipEffect 特性门控",
                "uiConfig.features['pageFlipEffect'] == true",
            ),
            ("气泡 tab 条件显示", "if (_showBubbleTab)"),
            ("网格尺寸条件显示", "if (_showGrid)"),
            ("聊天集成条件显示", "if (_chatIntegration)"),
        ] {
            assert!(controls.contains(needle), "Dart 表单指纹漂移: {label}");
        }
    }
}
