//! 单笔模型器会话(D9b):压感 → 平滑点列的**唯一实现**。
//!
//! 背景(`docs/duplication-backlog.md` D9b):同一套 ink-stroke-modeler 接入在两端
//! 各写了一份 ——
//! - macOS 走 core 的全局路径:`modeler.rs:4-10` 的 `STATE`(模型器 + 输出缓冲)
//!   由 `export/mod.rs:820-895` 的 `glaspen2_modeler_{begin,move,end}` 驱动,
//!   笔宽映射在全局函数里完成;抬笔后 `glaspen2_modeler_commit_to_strokes`
//!   (`export/mod.rs:897-922`)把缓冲并进 `STROKES` 再扇出钩子。
//! - Windows 覆盖层自带一份: `overlay/state.rs:218-222` 持 `StrokeModeler` +
//!   `start_time`,`overlay/pen.rs:144-234` 手写 begin/move/end 的事件类型判定与
//!   绘制路径,`pen.rs:252-261` 又抄了一份 `modeler_params()`,笔宽映射走本地
//!   `width_r`(`pen.rs:1-7`,f32 收窄 + 半径包装)。
//!
//! 本模块把「一笔的生命周期」收敛成平台无关的 [`ModelerSession`]:落笔
//! ([`ModelerSession::begin`])/送点([`ModelerSession::push`])/收笔
//! ([`ModelerSession::finish`])/取消([`ModelerSession::reset`]) + 输出缓冲 +
//! 模型器参数/笔宽公式单源。**会话本身不碰**全局 `STROKES`、DB、抬笔钩子
//! (草稿/共享/OCR)、FFI、渲染与存储语义 —— 这些留在 adapter(macOS 的
//! `export/mod.rs`,Windows 的 `overlay/{pen,actions}.rs`)。
//!
//! # 模型
//!
//! - [`modeler_params`]:ink-stroke-modeler 参数单源(两端历史上同值:
//!   `sampling_min_output_rate=120` / `sampling_end_of_stroke_stopping_distance=0.01` /
//!   `sampling_end_of_stroke_max_iterations=20` / `sampling_max_outputs_per_call=200` /
//!   `stylus_state_modeler_max_input_samples=20`,其余取 `ModelerParams::suggested()`)。
//! - [`pressure_to_width`]:压力 → 墨迹**宽度(直径)**的唯一公式
//!   (原 `modeler.rs:218`;macOS 即时反馈点与 Windows 半径包装都指向这里)。
//! - [`PenSample`][]:一个原始笔输入(位置/压力/绝对时间/当前笔宽倍率)。
//! - [`ModelerPoint`][]:模型器输出(平滑位置/映射后宽度/平滑压力/绝对时间)。
//! - [`StrokeStyle`]:会话元数据(笔色 + 橡皮标记);模型器数学不消费,收笔时
//!   随 [`FinishedStroke`] 带出。
//! - [`ModelerSession`]:一笔一实例(或长寿命实例反复 begin/reset);`begin` 重置
//!   模型器并清空缓冲,`push` 追加输出,`finish` 收笔并给出元数据,`reset` 取消。
//!
//! # 两端现状审计(读码结论;行号以 D9b 第一步提交时的 main 为准)
//!
//! ## 已经一致、直接由会话承载
//!
//! 1. **同一模型器与同一参数**:两端都从 `ink_stroke_modeler_rs` 建
//!    `StrokeModeler`,参数逐字段同值(`modeler.rs:12-22` ↔ `pen.rs:252-261`);
//!    两端都是「落笔重置模型器、Move 喂中间点、Up 收敛收笔」。
//! 2. **同一输出到笔宽映射**:`pressure_to_width(r.pressure, width_scale)`。
//!    macOS 在每次输出时映射(`modeler.rs:49`);Windows 在读取结果时经
//!    `width_r` 映射(`pen.rs:205`/`:219`),而 `width_r` 内部调的就是同一个
//!    core 公式(`pen.rs:6`),不是第二份公式。
//! 3. **平滑输出即实时绘制源**:两端都用模型器输出画实时笔迹(macOS 落笔先画
//!    原始点、之后画缓冲点;Windows 直接用每次 update 的返回),没有用原始点
//!    当墨迹中心线。
//! 4. **无预测调用**:两端都没有接 `StrokeModeler::predict`(只读输入序列);
//!    本会话同样不暴露 predict,不发明行为。
//!
//! ## 真实差异(会话只显式化,不静默统一)
//!
//! 1. **时间原点**:macOS 落笔时记绝对时间戳,模型器输入相对时间(`t = ts -
//!    start`),输出还原成绝对时间(`start + r.time`,`modeler.rs:54`);Windows 从
//!    自己的 `Instant`(`start_time`,在每次收笔/清屏/隐藏时重置)取 elapsed 直接
//!    当模型器输入时间,输出即该单调时钟值(`pen.rs:162`)。两者数学相同,
//!    只差一个常量平移 —— 会话统一「begin 记原点 + 输出绝对时间」,Windows
//!    adapter 继续用自己的 `Instant` 传时间,落库时间轴保持原样(tests
//!    `platform_time_origins_only_shift_times` 锁死:位置/宽度逐点一致,时间差
//!    恒等于原点差)。
//! 2. **错误处理**:macOS 全局函数把模型器错误记日志后吞掉(`modeler.rs:46-58`
//!    等),缓冲保持原样;Windows 把 `Err` 当作「这条 HID 报告无效」整条丢弃
//!    (`pen.rs:165-168`,含 Duplicate/负时间/TooFarApart)。会话返回
//!    [`Result`],由 adapter 各自保持原策略;引擎拒绝输入时不改内部状态
//!    (ink-stroke-modeler 的行为),所以两条路径语义一致。
//! 3. **橡皮**:仅 macOS 有(事件 tap 判定 `g_eraser_mode`,抬笔走
//!    `glaspen2_modeler_erase_finish`;Windows 无橡皮输入路径)。会话只持有
//!    [`StrokeStyle::erase`] 标记,不做命中/删除 —— 擦除命中
//!    (`export/mod.rs:1016-1028`)与存储语义留在 adapter。
//! 4. **笔宽倍率变化时机**:macOS 书写中换粗细只记 pending、抬笔后才生效
//!    (`menu.m:1370-1382`),所以整笔同一倍率;Windows 没有书写中守卫
//!    (`actions.rs:60-80` 直接改 `width_scale`),后续输出会用新倍率。会话按
//!    **每次送点携带 `width_scale`** 的语义映射,两端各自现状都不变。
//! 5. **缓冲生命周期**:macOS 的输出缓冲跨 begin/move/end 累积,抬笔后由
//!    `commit_to_strokes` 用 `take_buffer` 取走再 `clear`/下一次 begin 清空
//!    (`modeler.rs:33-37`/`:116-123`);Windows 不缓冲(直接消费 update 返回)。
//!    会话两用:返回值 = 本次新增输出,内部缓冲 = 整笔累积(macOS 兼容路径);
//!    Windows 只用返回值。
//!
//! # 使用契约
//!
//! 1. 会话不读全局、不落库、不调钩子、不做 FFI —— 同一输入序列必得同一输出。
//! 2. `begin` 会重置模型器并清空输出缓冲(与旧全局 `begin_stroke` 一致);
//!    落笔失败(输入含 NaN/Inf)不改状态。
//! 3. `push`/`finish` 前必须有 `begin`,否则 [`SessionError::NotStarted`];
//!    `finish` 成功后会话回到 `Idle`(缓冲保留,供提交路径 `take_points`)。
//! 4. 输出点的 `width` 已按**当前调用**的 `width_scale` 映射,是直径;adapter
//!    不要再做二次映射。
//! 5. 时间戳一律「绝对秒」;会话内部以 `begin` 的时间为原点换算相对时间,
//!    输出再还原为绝对时间(Windows 用自己的单调时钟原点即可)。

use ink_stroke_modeler_rs::{
    ModelerError, ModelerInput, ModelerInputEventType, ModelerParams, StrokeModeler,
};

/// ink-stroke-modeler 参数单源(两端历史上同值,改这里即两端同时生效)。
///
/// 逐字段理由:
/// - `sampling_min_output_rate = 120`:输出最低 120Hz,低于输入率时上采样;
/// - `sampling_end_of_stroke_stopping_distance = 0.01`:抬笔收敛停止距离;
/// - `sampling_end_of_stroke_max_iterations = 20`:抬笔收敛迭代上限;
/// - `sampling_max_outputs_per_call = 200`:单次调用输出上限(防长间隔刷爆);
/// - `stylus_state_modeler_max_input_samples = 20`:压力插值回看窗口;
/// - 其余(wobble/spring/drag)沿用 crate 建议值,与 rnote 一致。
pub fn modeler_params() -> ModelerParams {
    ModelerParams {
        sampling_min_output_rate: 120.0,
        sampling_end_of_stroke_stopping_distance: 0.01,
        sampling_end_of_stroke_max_iterations: 20,
        sampling_max_outputs_per_call: 200,
        stylus_state_modeler_max_input_samples: 20,
        ..ModelerParams::suggested()
    }
}

/// 压力 → 墨迹**宽度(直径)**的唯一实现。
///
/// `pressure <= 0.01`(含无压感的普通鼠标/触控板)退回固定最细宽度 1.0×倍率;
/// 否则走二次曲线 `(0.3 + p² × 7.7) × 倍率` —— 轻压极细、重压粗,动态范围约
/// 27 倍。macOS 落笔即时反馈点(经 FFI `glaspen2_pressure_raw_width`)、两端
/// 模型器输出、Windows 半径包装全部复用本函数,不许再写第二份公式。
pub fn pressure_to_width(pressure: f64, width_scale: f64) -> f64 {
    if pressure > 0.01 {
        (0.3 + pressure * pressure * 7.7) * width_scale
    } else {
        1.0 * width_scale
    }
}

/// 一个原始笔输入样本(未平滑)。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PenSample {
    /// 画布坐标(Windows 无限画布下是经镜头逆变换后的画布系;macOS 同理)。
    pub x: f64,
    pub y: f64,
    /// 归一化压力(0..=1;无压感设备给固定值)。
    pub pressure: f64,
    /// 绝对时间(秒)。会话内部以 `begin` 的时间为原点换算相对时间。
    pub time: f64,
    /// 当前笔宽倍率(压力 → 宽度的唯一乘数)。书写中换粗细立即对后续输出生效。
    pub width_scale: f64,
}

impl PenSample {
    pub const fn new(x: f64, y: f64, pressure: f64, time: f64, width_scale: f64) -> Self {
        Self {
            x,
            y,
            pressure,
            time,
            width_scale,
        }
    }

    /// 所有字段都是有限数(模型器不拒绝 NaN,但 NaN 一旦进缓冲就会污染
    /// STROKES/DB/导出;会话在入口挡住)。
    pub fn is_finite(&self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.pressure.is_finite()
            && self.time.is_finite()
            && self.width_scale.is_finite()
    }
}

/// 一个模型器输出点(平滑后)。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelerPoint {
    /// 平滑后的画布坐标。
    pub x: f64,
    pub y: f64,
    /// 墨迹宽度(**直径**):`pressure_to_width(平滑压力, 当次 width_scale)`。
    pub width: f64,
    /// 模型器插值出的平滑压力(0..=1 附近;原样保留,压力显示/未来预测可用)。
    pub pressure: f64,
    /// 绝对时间(秒)= `begin` 时间 + 模型器相对时间。
    pub time: f64,
}

impl ModelerPoint {
    /// 转成 STROKES/DB 的点元组 `(x, y, width, relative_time)`。
    /// **注意**元组第 4 项在旧全局路径里是绝对时间,调用方沿用原语义。
    pub const fn to_tuple(self) -> (f64, f64, f64, f64) {
        (self.x, self.y, self.width, self.time)
    }

    pub fn is_finite(&self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.pressure.is_finite()
            && self.time.is_finite()
    }
}

/// 单笔会话样式/元数据。
///
/// 模型器数学不消费颜色/橡皮标记;它们随会话保存,收笔时经 [`FinishedStroke`]
/// 带出,供 adapter 提交 STROKES/DB/钩子时使用(避免 adapter 再维护一份平行的
/// 单笔状态)。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrokeStyle {
    /// 笔色 `(r, g, b)`,各分量 0..=1。
    pub color: (f64, f64, f64),
    /// 橡皮笔标记:只记录语义,擦除命中/删除由 adapter 负责。
    pub erase: bool,
}

impl Default for StrokeStyle {
    fn default() -> Self {
        Self {
            color: (0.0, 0.0, 0.0),
            erase: false,
        }
    }
}

impl StrokeStyle {
    pub const fn new(color: (f64, f64, f64), erase: bool) -> Self {
        Self { color, erase }
    }

    /// 颜色分量都是有限数。
    pub fn is_finite(&self) -> bool {
        self.color.0.is_finite() && self.color.1.is_finite() && self.color.2.is_finite()
    }
}

/// 会话阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionPhase {
    /// 没有进行中的笔(初始 / 收笔后 / 取消后)。
    Idle,
    /// 已落笔、尚未收笔。
    Active,
}

impl SessionPhase {
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }
}

/// 会话错误。
#[derive(Debug, Clone)]
pub enum SessionError {
    /// 没有 `begin` 就 `push`/`finish`(旧全局路径对应「无 STATE」/模型器顺序错误)。
    NotStarted,
    /// 输入样本或样式含 NaN/Inf:拒绝,模型器与缓冲都不动(防 NaN 进 STROKES/DB)。
    NonFinite(&'static str),
    /// ink-stroke-modeler 拒绝输入(重复点/负时间差/事件顺序/间隔过大)。
    /// 引擎保证校验失败时内部状态不变,会话同样不改缓冲/阶段。
    Modeler(ModelerError),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotStarted => write!(f, "模型器会话尚未落笔(begin)"),
            Self::NonFinite(field) => write!(f, "模型器输入含非有限数: {field}"),
            Self::Modeler(e) => write!(f, "模型器拒绝输入: {e}"),
        }
    }
}

impl std::error::Error for SessionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Modeler(e) => Some(e),
            _ => None,
        }
    }
}

impl From<ModelerError> for SessionError {
    fn from(e: ModelerError) -> Self {
        Self::Modeler(e)
    }
}

/// 收笔结果:本次 Up 新增的平滑点 + 本笔元数据。
///
/// `points` 只含**本次 finish 调用新收敛出的点**(begin/push 的输出已在缓冲里,
/// GIF/DB 按事件流逐点记录过);整笔点列用 [`ModelerSession::take_points`] 取。
#[derive(Debug, Clone, PartialEq)]
pub struct FinishedStroke {
    /// Up 事件新收敛出的平滑点(模型器保证 Up 至少产出 1 点)。
    pub points: Vec<ModelerPoint>,
    /// 本笔样式(颜色/橡皮标记)。
    pub style: StrokeStyle,
    /// 落笔时间(秒,`begin` 的绝对时间戳)。
    pub start_time: f64,
    /// 本笔时长(秒)= Up 绝对时间 − 落笔时间。
    pub duration: f64,
    /// 本笔累计输出点数(含 Up 段;= 缓冲长度)。
    pub total_points: usize,
}

/// 单笔 ink-stroke-modeler 会话(D9b 核心门面)。
///
/// 生命周期:
///
/// ```text
/// Idle --begin(style, sample)--> Active --push(sample)+--> Active
///                                     \--finish(sample)--> Idle(缓冲保留)
///      <--reset()/取消-- 任意状态 ---------------------/
/// ```
///
/// 两端都在单线程持有(macOS 在全局锁内、Windows 在消息循环线程),所以本类型
/// 不额外加锁,也不要求 adapter 之外的并发语义。
pub struct ModelerSession {
    modeler: StrokeModeler,
    phase: SessionPhase,
    style: StrokeStyle,
    start_time: f64,
    buffer: Vec<ModelerPoint>,
}

impl std::fmt::Debug for ModelerSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelerSession")
            .field("phase", &self.phase)
            .field("style", &self.style)
            .field("start_time", &self.start_time)
            .field("buffer_len", &self.buffer.len())
            .finish_non_exhaustive()
    }
}

impl Default for ModelerSession {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelerSession {
    /// 用单源参数 [`modeler_params`] 建一个空闲会话。
    ///
    /// 参数常量自身非法属于编程错误(有测试守护);真出现时退回 crate 建议值并
    /// 记 error 日志,不让进程挂掉 —— 与旧全局路径「reset 失败只记日志继续」
    /// 同策略。
    pub fn new() -> Self {
        match Self::with_params(modeler_params()) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("模型器参数非法({e}), 退回 crate 建议值");
                // suggested() 一定合法,此分支实际不可达。
                Self::from_modeler(StrokeModeler::default())
            }
        }
    }

    /// 用指定参数建会话(测试/未来可配置平滑用);参数非法返回错误文本
    /// (ink-stroke-modeler 的 `validate` 原因)。
    pub fn with_params(params: ModelerParams) -> Result<Self, String> {
        let modeler = StrokeModeler::new(params)?;
        Ok(Self::from_modeler(modeler))
    }

    fn from_modeler(modeler: StrokeModeler) -> Self {
        Self {
            modeler,
            phase: SessionPhase::Idle,
            style: StrokeStyle::default(),
            start_time: 0.0,
            buffer: Vec::new(),
        }
    }

    /// 落笔:重置模型器、记样式/时间原点、喂 Down,返回首点输出。
    ///
    /// 与旧全局 `begin_stroke` 一致:会清空输出缓冲(上一笔未取走的缓冲丢弃)。
    /// 样本/样式非法时不改任何状态并返回错误。
    pub fn begin(
        &mut self,
        style: StrokeStyle,
        sample: PenSample,
    ) -> Result<Vec<ModelerPoint>, SessionError> {
        validate_sample(&sample)?;
        if !style.is_finite() {
            return Err(SessionError::NonFinite("style.color"));
        }
        self.modeler.reset();
        self.phase = SessionPhase::Idle;
        self.buffer.clear();
        self.style = style;
        self.start_time = sample.time;
        let points = self.feed(ModelerInputEventType::Down, &sample)?;
        self.phase = SessionPhase::Active;
        Ok(points)
    }

    /// 送一个移动点:喂 Move,返回本次新增的平滑输出(可能 1 个或多个)。
    ///
    /// 未落笔返回 [`SessionError::NotStarted`];模型器拒绝(重复点/负时间差/
    /// 间隔过大)时返回 [`SessionError::Modeler`] 且不改缓冲。
    pub fn push(&mut self, sample: PenSample) -> Result<Vec<ModelerPoint>, SessionError> {
        if !self.phase.is_active() {
            return Err(SessionError::NotStarted);
        }
        validate_sample(&sample)?;
        self.feed(ModelerInputEventType::Move, &sample)
    }

    /// 收笔:喂 Up(模型器收敛到末点),返回本段输出 + 元数据,会话转 `Idle`。
    ///
    /// 失败时(未落笔/非法输入/模型器拒绝)会话保持原阶段与缓冲,可重试或 reset。
    pub fn finish(&mut self, sample: PenSample) -> Result<FinishedStroke, SessionError> {
        if !self.phase.is_active() {
            return Err(SessionError::NotStarted);
        }
        validate_sample(&sample)?;
        let points = self.feed(ModelerInputEventType::Up, &sample)?;
        self.phase = SessionPhase::Idle;
        Ok(FinishedStroke {
            points,
            style: self.style,
            start_time: self.start_time,
            duration: sample.time - self.start_time,
            total_points: self.buffer.len(),
        })
    }

    /// 取消进行中的笔并清空输出缓冲(等价「reset/cancel」)。
    ///
    /// 不产生任何输出;下一次 `begin` 从干净模型器开始。adapter 在抬笔丢失、
    /// 清屏、隐藏笔迹、切换画布等「必须丢弃当前笔」的路径调用。
    pub fn reset(&mut self) {
        self.modeler.reset();
        self.phase = SessionPhase::Idle;
        self.style = StrokeStyle::default();
        self.start_time = 0.0;
        self.buffer.clear();
    }

    /// 当前阶段。
    pub const fn phase(&self) -> SessionPhase {
        self.phase
    }

    pub const fn is_active(&self) -> bool {
        self.phase.is_active()
    }

    /// 当前笔样式(未落笔时为默认样式)。
    pub const fn style(&self) -> StrokeStyle {
        self.style
    }

    /// 落笔时间(秒;未落笔时为 0)。
    pub const fn start_time(&self) -> f64 {
        self.start_time
    }

    /// 整笔累计输出点(缓冲)。
    pub fn points(&self) -> &[ModelerPoint] {
        &self.buffer
    }

    /// 取走整笔累计输出点并清空缓冲(macOS 旧 `take_buffer` 的替代)。
    pub fn take_points(&mut self) -> Vec<ModelerPoint> {
        std::mem::take(&mut self.buffer)
    }

    /// 清空输出缓冲(不动模型器/阶段;macOS 旧 `clear_buffer` 的替代)。
    pub fn clear_points(&mut self) {
        self.buffer.clear();
    }

    /// 累计输出点数。
    pub fn point_count(&self) -> usize {
        self.buffer.len()
    }

    /// 按索引取单个输出点。
    pub fn point(&self, idx: usize) -> Option<ModelerPoint> {
        self.buffer.get(idx).copied()
    }

    /// 喂一个模型器事件并收集输出:映射宽度/时间后写入缓冲并返回本次新增。
    fn feed(
        &mut self,
        event_type: ModelerInputEventType,
        sample: &PenSample,
    ) -> Result<Vec<ModelerPoint>, SessionError> {
        let input = ModelerInput {
            event_type,
            pos: (sample.x, sample.y),
            time: sample.time - self.start_time,
            pressure: sample.pressure,
        };
        let results = self.modeler.update(input).map_err(SessionError::Modeler)?;
        let points: Vec<ModelerPoint> = results
            .into_iter()
            .map(|r| ModelerPoint {
                x: r.pos.0,
                y: r.pos.1,
                width: pressure_to_width(r.pressure, sample.width_scale),
                pressure: r.pressure,
                time: self.start_time + r.time,
            })
            .collect();
        self.buffer.extend_from_slice(&points);
        Ok(points)
    }
}

fn validate_sample(sample: &PenSample) -> Result<(), SessionError> {
    if !sample.is_finite() {
        let field = if !sample.x.is_finite() {
            "x"
        } else if !sample.y.is_finite() {
            "y"
        } else if !sample.pressure.is_finite() {
            "pressure"
        } else if !sample.time.is_finite() {
            "time"
        } else {
            "width_scale"
        };
        return Err(SessionError::NonFinite(field));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke_style() -> StrokeStyle {
        StrokeStyle::new((1.0, 0.0, 0.5), false)
    }

    /// 固定笔迹(两端/新旧路径共用同一组输入):8 点右手弧线 + 压力起伏。
    fn fixed_stroke(origin: f64) -> Vec<PenSample> {
        const RAW: [(f64, f64, f64); 8] = [
            (100.0, 200.0, 0.30),
            (112.0, 203.0, 0.42),
            (123.0, 209.0, 0.55),
            (131.0, 218.0, 0.62),
            (136.0, 229.0, 0.70),
            (138.0, 241.0, 0.66),
            (137.0, 253.0, 0.52),
            (133.0, 264.0, 0.44),
        ];
        RAW.iter()
            .enumerate()
            .map(|(i, &(x, y, p))| PenSample::new(x, y, p, origin + i as f64 * 0.005, 1.0))
            .collect()
    }

    fn run_whole_with(
        session: &mut ModelerSession,
        style: StrokeStyle,
        samples: &[PenSample],
    ) -> Vec<ModelerPoint> {
        assert!(samples.len() >= 2, "固定笔迹至少 2 点");
        let mut out = session.begin(style, samples[0]).expect("begin 应成功");
        for sample in &samples[1..samples.len() - 1] {
            out.extend(session.push(*sample).expect("push 应成功"));
        }
        let finished = session
            .finish(*samples.last().unwrap())
            .expect("finish 应成功");
        out.extend(finished.points);
        out
    }

    fn run_whole(session: &mut ModelerSession, samples: &[PenSample]) -> Vec<ModelerPoint> {
        run_whole_with(session, stroke_style(), samples)
    }

    fn assert_close(a: f64, b: f64, tol: f64, what: &str) {
        assert!((a - b).abs() <= tol, "{what}: {a} vs {b} (tol {tol})");
    }

    fn snapshot(points: &[ModelerPoint]) -> Vec<String> {
        points
            .iter()
            .map(|p| {
                format!(
                    "{:.6} {:.6} {:.9} {:.6} {:.6}",
                    p.x, p.y, p.width, p.pressure, p.time
                )
            })
            .collect()
    }

    // ------------------------------------------------------------------
    // 参数 / 公式单源
    // ------------------------------------------------------------------

    #[test]
    fn modeler_params_are_the_historical_two_end_values() {
        let p = modeler_params();
        assert_eq!(p.sampling_min_output_rate, 120.0);
        assert_eq!(p.sampling_end_of_stroke_stopping_distance, 0.01);
        assert_eq!(p.sampling_end_of_stroke_max_iterations, 20);
        assert_eq!(p.sampling_max_outputs_per_call, 200);
        assert_eq!(p.stylus_state_modeler_max_input_samples, 20);
        // 其余字段沿用 crate 建议值(wobble/spring/drag 两端都没覆盖)
        let suggested = ModelerParams::suggested();
        assert_eq!(p.wobble_smoother_timeout, suggested.wobble_smoother_timeout);
        assert_eq!(
            p.wobble_smoother_speed_floor,
            suggested.wobble_smoother_speed_floor
        );
        assert_eq!(
            p.wobble_smoother_speed_ceiling,
            suggested.wobble_smoother_speed_ceiling
        );
        assert_eq!(
            p.position_modeler_spring_mass_constant,
            suggested.position_modeler_spring_mass_constant
        );
        assert_eq!(
            p.position_modeler_drag_constant,
            suggested.position_modeler_drag_constant
        );
        // 参数常量必须自身合法(new() 的守护)
        assert!(p.validate().is_ok());
    }

    #[test]
    fn pressure_to_width_contract() {
        // 无压感/极轻压: 固定最细宽度 1.0×倍率
        assert_eq!(pressure_to_width(0.0, 1.0), 1.0);
        assert_eq!(pressure_to_width(0.009, 1.0), 1.0);
        assert_eq!(pressure_to_width(0.01, 1.0), 1.0);
        assert_eq!(pressure_to_width(-1.0, 1.0), 1.0);
        // 二次曲线
        assert_eq!(pressure_to_width(0.5, 1.0), 0.3 + 0.25 * 7.7);
        assert_eq!(pressure_to_width(1.0, 1.0), 8.0);
        // 倍率线性
        for &scale in &[0.15, 1.0, 3.5] {
            for &p0 in &[0.0, 0.2, 0.5, 1.0] {
                assert_close(
                    pressure_to_width(p0, scale),
                    pressure_to_width(p0, 1.0) * scale,
                    1e-12,
                    "width_scale 应线性",
                );
            }
        }
        // 严格单调(压力越高越宽,越过 0.01 阈值后)
        assert!(pressure_to_width(0.2, 1.0) < pressure_to_width(0.5, 1.0));
        assert!(pressure_to_width(0.5, 1.0) < pressure_to_width(1.0, 1.0));
    }

    // ------------------------------------------------------------------
    // begin / push / finish 生命周期
    // ------------------------------------------------------------------

    #[test]
    fn begin_emits_down_point_and_records_state() {
        let style = stroke_style();
        let sample = PenSample::new(10.0, 20.0, 0.5, 100.0, 1.0);
        let mut session = ModelerSession::new();
        assert_eq!(session.phase(), SessionPhase::Idle);
        assert!(!session.is_active());

        let points = session.begin(style, sample).expect("begin");
        assert_eq!(points.len(), 1, "Down 恒返回 1 点");
        assert_eq!(points[0].x, 10.0);
        assert_eq!(points[0].y, 20.0);
        assert_eq!(points[0].pressure, 0.5);
        assert_eq!(points[0].time, 100.0, "输出时间是绝对时间");
        assert_eq!(points[0].width, pressure_to_width(0.5, 1.0));
        assert!(points[0].is_finite());
        assert_eq!(session.phase(), SessionPhase::Active);
        assert_eq!(session.style(), style);
        assert_eq!(session.start_time(), 100.0);
        assert_eq!(session.point_count(), 1);
        assert_eq!(session.point(0), Some(points[0]));
        assert_eq!(session.point(1), None);
    }

    #[test]
    fn begin_push_finish_keep_point_and_time_order() {
        let samples = fixed_stroke(1000.0);
        let mut session = ModelerSession::new();
        let out = run_whole(&mut session, &samples);

        assert_eq!(session.phase(), SessionPhase::Idle, "finish 后回 Idle");
        assert_eq!(session.point_count(), out.len());
        assert_eq!(session.points(), out.as_slice());
        assert!(out.len() > samples.len(), "Move 至少上采样到 1 点/事件");
        // 时间单调不减,且都落在绝对时间轴上
        for w in out.windows(2) {
            assert!(
                w[0].time <= w[1].time + 1e-12,
                "输出时间必须非递减: {} -> {}",
                w[0].time,
                w[1].time
            );
        }
        assert!(out[0].time >= samples[0].time);
        assert!(out.last().unwrap().time >= samples.last().unwrap().time);
        // 首点 = Down 原样
        assert_eq!(out[0].x, samples[0].x);
        assert_eq!(out[0].y, samples[0].y);
        // 整笔点列可被取走,取走后缓冲清空
        let taken = session.take_points();
        assert_eq!(taken, out);
        assert_eq!(session.point_count(), 0);
        assert!(session.points().is_empty());
    }

    #[test]
    fn finish_returns_metadata_and_final_segment() {
        let samples = fixed_stroke(500.0);
        let mut session = ModelerSession::new();
        session.begin(stroke_style(), samples[0]).expect("begin");
        for sample in &samples[1..samples.len() - 1] {
            session.push(*sample).expect("push");
        }
        let last = *samples.last().unwrap();
        let finished = session.finish(last).expect("finish");
        assert!(!finished.points.is_empty(), "Up 必须收敛出点");
        assert_eq!(finished.style, stroke_style());
        assert_eq!(finished.start_time, 500.0);
        assert_eq!(finished.duration, last.time - 500.0);
        assert_eq!(finished.total_points, session.point_count());
        assert!(finished.points.iter().all(ModelerPoint::is_finite));
        assert_eq!(session.phase(), SessionPhase::Idle);
        // Up 段是缓冲的尾部
        let tail = &session.points()[session.point_count() - finished.points.len()..];
        assert_eq!(tail, finished.points.as_slice());
        // 收笔后再 push/finish 都是 NotStarted
        assert!(matches!(session.push(last), Err(SessionError::NotStarted)));
        assert!(matches!(
            session.finish(last),
            Err(SessionError::NotStarted)
        ));
    }

    #[test]
    fn push_or_finish_without_begin_is_not_started() {
        let samples = fixed_stroke(0.0);
        let mut session = ModelerSession::new();
        assert!(matches!(
            session.push(samples[0]),
            Err(SessionError::NotStarted)
        ));
        assert!(matches!(
            session.finish(samples[0]),
            Err(SessionError::NotStarted)
        ));
        assert_eq!(session.point_count(), 0);
        assert_eq!(session.phase(), SessionPhase::Idle);
    }

    #[test]
    fn rejected_inputs_leave_state_untouched() {
        let samples = fixed_stroke(0.0);
        let mut session = ModelerSession::new();
        session.begin(stroke_style(), samples[0]).expect("begin");
        // 正常推进一次, 记下基线
        assert!(session.push(samples[1]).is_ok());
        let after_first_move = session.point_count();

        // 重复 Move(位置/压力/时间完全相同) —— 模型器拒绝, 缓冲不变
        assert!(matches!(
            session.push(samples[1]),
            Err(SessionError::Modeler(_))
        ));
        assert_eq!(session.point_count(), after_first_move);
        assert_eq!(session.phase(), SessionPhase::Active);

        // 负时间差(比上一条输入更早)
        let mut back = samples[2];
        back.time = samples[1].time - 0.001;
        assert!(matches!(session.push(back), Err(SessionError::Modeler(_))));
        assert_eq!(session.point_count(), after_first_move);

        // 间隔过大(> sampling_max_outputs_per_call / rate 秒)
        let mut far = samples[2];
        far.time = samples[1].time + 5.0;
        assert!(matches!(session.push(far), Err(SessionError::Modeler(_))));
        assert_eq!(session.point_count(), after_first_move);

        // 状态仍健康: 正常点可推进、可收笔
        assert!(session.push(samples[2]).is_ok());
        assert_eq!(session.phase(), SessionPhase::Active);
        assert!(session.finish(samples[3]).is_ok());
        assert_eq!(session.phase(), SessionPhase::Idle);
    }

    #[test]
    fn clear_points_keeps_stroke_active() {
        let samples = fixed_stroke(0.0);
        let mut session = ModelerSession::new();
        session.begin(stroke_style(), samples[0]).expect("begin");
        session.push(samples[1]).expect("push");
        assert!(session.point_count() > 0);
        session.clear_points();
        assert_eq!(session.point_count(), 0);
        assert_eq!(session.phase(), SessionPhase::Active);
        assert!(session.push(samples[2]).is_ok());
        assert!(session.finish(samples[3]).is_ok());
    }

    #[test]
    fn begin_twice_starts_a_new_stroke() {
        let samples = fixed_stroke(0.0);
        let mut session = ModelerSession::new();
        session.begin(stroke_style(), samples[0]).expect("begin");
        for sample in &samples[1..4] {
            session.push(*sample).expect("push");
        }
        assert!(session.point_count() > 1);

        let new_style = StrokeStyle::new((0.0, 0.0, 1.0), true);
        let first = session.begin(new_style, samples[0]).expect("第二次 begin");
        assert_eq!(first.len(), 1, "begin 会清空缓冲, 只留新笔首点");
        assert_eq!(session.point_count(), 1);
        assert_eq!(session.style(), new_style);
        assert_eq!(session.start_time(), samples[0].time);
    }

    #[test]
    fn reset_cancels_stroke_and_clears_buffer() {
        let samples = fixed_stroke(0.0);
        let mut session = ModelerSession::new();
        session.begin(stroke_style(), samples[0]).expect("begin");
        session.push(samples[1]).expect("push");
        assert!(session.point_count() >= 2);

        session.reset();
        assert_eq!(session.phase(), SessionPhase::Idle);
        assert_eq!(session.style(), StrokeStyle::default());
        assert_eq!(session.point_count(), 0);
        assert!(session.points().is_empty());
        assert!(matches!(
            session.push(samples[2]),
            Err(SessionError::NotStarted)
        ));

        // reset 之后可以重新落笔
        let out = session.begin(stroke_style(), samples[0]).expect("re-begin");
        assert_eq!(out.len(), 1);
        assert_eq!(session.phase(), SessionPhase::Active);
    }

    // ------------------------------------------------------------------
    // 确定性 / 平台时间原点 / 宽度映射 / 橡皮标记
    // ------------------------------------------------------------------

    #[test]
    fn same_input_same_output() {
        let mut a = ModelerSession::new();
        let mut b = ModelerSession::new();
        let out_a = run_whole(&mut a, &fixed_stroke(1000.0));
        let out_b = run_whole(&mut b, &fixed_stroke(1000.0));
        assert_eq!(out_a, out_b, "同一输入序列必须逐位一致(无随机/无时间依赖)");
    }

    #[test]
    fn platform_time_origins_only_shift_times() {
        // macOS: 绝对时间戳(如 CACurrentMediaTime);Windows: 自己的单调时钟 elapsed。
        let macos = run_whole(&mut ModelerSession::new(), &fixed_stroke(1000.0));
        let windows = run_whole(&mut ModelerSession::new(), &fixed_stroke(0.0));
        assert_eq!(macos.len(), windows.len());
        for (m, w) in macos.iter().zip(windows.iter()) {
            assert_close(m.x, w.x, 1e-9, "x 与时间原点无关");
            assert_close(m.y, w.y, 1e-9, "y 与时间原点无关");
            assert_close(m.width, w.width, 1e-9, "宽度与时间原点无关");
            assert_close(m.pressure, w.pressure, 1e-12, "压力与时间原点无关");
            assert_close(m.time - w.time, 1000.0, 1e-9, "时间差恒等于原点差");
        }
    }

    #[test]
    fn width_scale_is_applied_per_sample() {
        let samples = fixed_stroke(0.0);
        let mut session = ModelerSession::new();
        let first = session.begin(stroke_style(), samples[0]).expect("begin");
        assert_eq!(first[0].width, pressure_to_width(samples[0].pressure, 1.0));

        // 书写中换粗细(Windows 现行行为):后续输出立即用新倍率
        let mut scaled = samples[1];
        scaled.width_scale = 2.5;
        let moved = session.push(scaled).expect("push");
        assert!(!moved.is_empty());
        for point in &moved {
            assert_eq!(
                point.width,
                pressure_to_width(point.pressure, 2.5),
                "输出宽度必须用当次 width_scale 映射"
            );
        }
    }

    #[test]
    fn erase_style_is_metadata_only() {
        let samples = fixed_stroke(0.0);
        let erase_style = StrokeStyle::new((0.0, 0.0, 0.0), true);
        let mut erase_session = ModelerSession::new();
        let erase_points = run_whole_with(&mut erase_session, erase_style, &samples);
        let mut normal_session = ModelerSession::new();
        let normal_points = run_whole(&mut normal_session, &samples);

        // 橡皮标记不影响平滑数学: 点列逐位一致
        assert_eq!(erase_points, normal_points);

        // 标记随收笔元数据带出
        let mut session = ModelerSession::new();
        session.begin(erase_style, samples[0]).expect("begin");
        let finished = session.finish(samples[1]).expect("finish");
        assert!(finished.style.erase);
        assert_eq!(finished.style.color, (0.0, 0.0, 0.0));

        // 默认样式非橡皮
        assert!(!StrokeStyle::default().erase);
    }

    // ------------------------------------------------------------------
    // 退化笔 / 非法输入 / NaN
    // ------------------------------------------------------------------

    #[test]
    fn single_point_and_empty_strokes() {
        // 空: 未落笔
        let empty = ModelerSession::new();
        assert_eq!(empty.point_count(), 0);
        assert!(empty.points().is_empty());

        // 一点笔: 落笔后同位置稍晚收笔
        let down_sample = PenSample::new(5.0, 6.0, 0.5, 10.0, 1.0);
        let up_sample = PenSample {
            time: 10.05,
            ..down_sample
        };
        let mut session = ModelerSession::new();
        let down = session.begin(stroke_style(), down_sample).expect("begin");
        assert_eq!(down.len(), 1);
        let finished = session.finish(up_sample).expect("finish");
        assert!(!finished.points.is_empty());
        assert!(finished.total_points >= 2);
        assert!(finished.points.iter().all(ModelerPoint::is_finite));
        assert!(finished.duration > 0.0);

        // 时间倒退的 Up 被模型器拒绝(负时间差), 状态/缓冲不变, 之后可正常收笔
        let mut rejected = ModelerSession::new();
        rejected.begin(stroke_style(), down_sample).expect("begin");
        let mut back_in_time = down_sample;
        back_in_time.time = down_sample.time - 0.001;
        assert!(matches!(
            rejected.finish(back_in_time),
            Err(SessionError::Modeler(_))
        ));
        assert_eq!(rejected.phase(), SessionPhase::Active);
        assert_eq!(rejected.point_count(), 1);
        assert!(rejected.finish(up_sample).is_ok());
    }

    #[test]
    fn invalid_params_and_non_finite_samples_are_rejected() {
        // 非法模型器参数: 会话创建失败(不 panic)
        let mut bad = modeler_params();
        bad.sampling_min_output_rate = 0.0;
        assert!(ModelerSession::with_params(bad).is_err());

        let good = PenSample::new(1.0, 2.0, 0.5, 10.0, 1.0);
        // 落笔样本非有限: 拒绝且状态/缓冲不变
        for (bad_sample, field) in [
            (PenSample::new(f64::NAN, 2.0, 0.5, 10.0, 1.0), "x"),
            (PenSample::new(1.0, f64::INFINITY, 0.5, 10.0, 1.0), "y"),
            (PenSample::new(1.0, 2.0, f64::NAN, 10.0, 1.0), "pressure"),
            (
                PenSample::new(1.0, 2.0, 0.5, f64::NEG_INFINITY, 1.0),
                "time",
            ),
            (PenSample::new(1.0, 2.0, 0.5, 10.0, f64::NAN), "width_scale"),
        ] {
            let mut session = ModelerSession::new();
            match session.begin(stroke_style(), bad_sample) {
                Err(SessionError::NonFinite(actual)) => assert_eq!(actual, field),
                other => panic!("应拒绝非有限样本({field}): {other:?}"),
            }
            assert_eq!(session.phase(), SessionPhase::Idle);
            assert_eq!(session.point_count(), 0);
        }

        // 非法样式颜色: 拒绝
        let mut session = ModelerSession::new();
        assert!(matches!(
            session.begin(StrokeStyle::new((f64::NAN, 0.0, 0.0), false), good),
            Err(SessionError::NonFinite("style.color"))
        ));
        assert_eq!(session.phase(), SessionPhase::Idle);

        // 落笔后: push 非有限也拒绝, 缓冲不变, 之后仍可正常书写
        let samples = fixed_stroke(0.0);
        let mut session = ModelerSession::new();
        session.begin(stroke_style(), samples[0]).expect("begin");
        let before = session.point_count();
        let mut bad_pressure = samples[1];
        bad_pressure.pressure = f64::INFINITY;
        assert!(matches!(
            session.push(bad_pressure),
            Err(SessionError::NonFinite("pressure"))
        ));
        assert_eq!(session.point_count(), before);
        assert_eq!(session.phase(), SessionPhase::Active);
        assert!(session.push(samples[1]).is_ok());
        assert!(session.finish(samples[2]).is_ok());
    }

    #[test]
    fn outputs_never_contain_nan_or_inf() {
        for &origin in &[0.0_f64, 1000.0, 1_000_000.0] {
            let mut samples = fixed_stroke(origin);
            samples[1].pressure = 0.0;
            samples[2].pressure = 1.0;
            samples[3].pressure = 0.01;
            let mut session = ModelerSession::new();
            let out = run_whole(&mut session, &samples);
            assert!(!out.is_empty());
            for point in &out {
                assert!(point.is_finite(), "输出出现非有限数: {point:?}");
                assert!(point.width > 0.0, "宽度必须为正: {point:?}");
            }
        }
    }

    // ------------------------------------------------------------------
    // 特征化: 与旧全局路径逐位一致 + 固定快照
    // ------------------------------------------------------------------

    /// 特征化基线:同一输入序列下,新会话输出必须与旧全局 STATE 路径
    /// (modeler.rs 的 begin_stroke/pen_move/end_stroke/take_buffer)逐位一致。
    /// D9b 第三步把全局路径改为委托会话后,本测试仍守护 adapter 侧的
    /// 时间/宽度/缓冲语义没有漂移。
    #[test]
    fn characterization_session_matches_legacy_global_path() {
        let _guard = crate::tests::TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let samples = fixed_stroke(1000.0);
        let last = *samples.last().unwrap();

        crate::modeler::begin_stroke(
            samples[0].x,
            samples[0].y,
            samples[0].pressure,
            samples[0].time,
            samples[0].width_scale,
        );
        for sample in &samples[1..samples.len() - 1] {
            crate::modeler::pen_move(
                sample.x,
                sample.y,
                sample.pressure,
                sample.time,
                sample.width_scale,
            );
        }
        crate::modeler::end_stroke(last.x, last.y, last.pressure, last.time, last.width_scale);
        let legacy = crate::modeler::take_buffer();

        let mut session = ModelerSession::new();
        let fresh: Vec<(f64, f64, f64, f64)> = run_whole(&mut session, &samples)
            .iter()
            .map(|p| p.to_tuple())
            .collect();

        assert_eq!(legacy.len(), fresh.len(), "点数必须一致");
        assert_eq!(legacy, fresh, "旧全局路径与新会话必须逐位一致");
    }

    /// 特征化快照:固定笔迹的完整输出(位置/宽度/压力/时间)逐点锁死。
    /// 数值漂移 = 平滑/重采样/宽度映射行为变化,必须人工确认。
    #[test]
    fn characterization_fixed_stroke_snapshot() {
        let samples = fixed_stroke(1000.0);
        let mut session = ModelerSession::new();
        let points = run_whole(&mut session, &samples);
        let got = snapshot(&points);
        let expected: Vec<String> = vec![
            "100.000000 200.000000 0.993000000 0.300000 1000.000000".to_string(),
            "100.883636 200.220909 1.034425226 0.308836 1000.005000".to_string(),
            "103.077732 201.008751 1.145355869 0.331340 1000.010000".to_string(),
            "106.538048 202.764144 1.342982637 0.368038 1000.015000".to_string(),
            "110.922121 205.819508 1.685516908 0.424190 1000.020000".to_string(),
            "115.721844 210.365505 2.152488686 0.490493 1000.025000".to_string(),
            "120.360513 216.414392 2.819496281 0.572020 1000.030000".to_string(),
            "124.259987 223.789711 3.418842987 0.636431 1000.035000".to_string(),
            "128.647366 236.931422 3.842186624 0.678251 1000.043333".to_string(),
            "131.292629 247.724861 2.851201727 0.575608 1000.051667".to_string(),
            "132.699970 255.371242 2.184595200 0.494725 1000.060000".to_string(),
            "133.324275 260.194768 1.966363559 0.465200 1000.068333".to_string(),
            "133.507669 262.902521 1.847044570 0.448235 1000.076667".to_string(),
            "133.545896 263.906356 1.803462927 0.441877 1000.080833".to_string(),
            "133.548131 263.967938 1.800807862 0.441486 1000.081094".to_string(),
        ];
        assert_eq!(got, expected);
    }
}
