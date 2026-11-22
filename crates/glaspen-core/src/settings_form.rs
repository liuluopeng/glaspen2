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
        Some(Range::new(20.0, 80.0, 20.0)),
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
