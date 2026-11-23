//! Windows 覆盖层的单笔模型器适配层(D9b 第二步):平滑/重采样/笔宽公式的
//! **唯一实现**是 glaspen_core::modeler_session::ModelerSession。
//!
//! 背景(docs/duplication-backlog.md D9b):D9b 第一步已把「一笔的生命周期」
//! (begin/push/finish/reset + 参数 + 压感→宽度)收进 core modeler_session。
//! 本模块是 Windows 侧剩下的一层薄适配器 —— 旧 overlay/state.rs 的
//! StrokeModeler 实例、overlay/pen.rs 的 modeler_params() 与手写
//! Down/Move/Up 事件判定全部删掉,只保留 Windows adapter 真正需要的东西:
//!
//! 1. **事件语义**(与旧 overlay/pen.rs::handle_point 逐分支对应):
//!    落笔报告中未落笔 → begin;落笔中 → push;非落笔报告中落笔中 →
//!    finish;未落笔的悬空报告 → 忽略(旧实现把 Up 喂给模型器被拒绝,等价)。
//! 2. **时间轴**:adapter 传入自己的单调时钟绝对秒(Windows =
//!    OverlayState::stroke_origin.elapsed()),会话输出点的 time 与旧
//!    Windows 模型器输出 r.time 同轴 —— 落笔时刻取会话原点,后续点 =
//!    原点 + 模型器内部相对时间,逐位等于旧实现(time_axis_matches_legacy_*
//!    测试对照 core 旧全局路径)。
//! 3. **展示/落库映射**:会话输出 width 是已按当次倍率映射的直径,adapter
//!    只做旧路径的收窄/缩放包装 —— 落库宽度走旧 width_r()*2.0 的 f32 收窄
//!    (db_width),实时绘制半径走旧 width_r()*zoom 的 f32 运算
//!    (view_radius);笔宽公式本身不再有第二份。
//! 4. **无橡皮**:Windows 没有橡皮输入路径(审计确认,全仓 0 处 Windows 橡皮
//!    判定),会话样式恒为 StrokeStyle::default()(非橡皮)。擦除命中与存储
//!    语义只在 macOS adapter。
//!
//! **不碰** Win32 句柄、全局 STROKES/DB、抬笔钩子扇出 —— 这些仍在
//! overlay/pen.rs(落库点)与 overlay/actions.rs(抬笔仍走
//! end_stroke → decimate_last_stroke → notify_stroke_committed 原顺序)。
//!
//! 本模块不依赖 Win32,所以在 macOS 开发机上也能编译并跑单测
//! (cargo test -p glaspen-windows pen_modeler::);Windows overlay 只在
//! cfg(windows) 下 include 它。

use glaspen_core::modeler_session::{
    FinishedStroke, ModelerPoint, ModelerSession, PenSample, SessionError, StrokeStyle,
};

/// 旧 overlay/pen.rs::width_r 的笔宽倍率下限(scale.max(0.05)):
/// 面板预设最小 0.15,这里只兜底历史库里的脏值(0/负数/极小值)。
pub const MIN_WIDTH_SCALE: f64 = 0.05;

/// 构造送进模型器会话的原始样本,保留旧 Windows 路径的两处收窄:
/// - 压力先收窄到 f32(旧 HID 归一化本身就是 f32,模型器收到的是 p as f64);
/// - 笔宽倍率走 f32 且下限 MIN_WIDTH_SCALE(旧 width_r 的 scale.max(0.05))。
///
/// 位置/时间保持 f64(旧实现同样由 f32 位置转 f64、单调时钟 f64 直接传入)。
pub fn sample(x: f64, y: f64, pressure: f64, time: f64, width_scale: f64) -> PenSample {
    PenSample::new(
        x,
        y,
        pressure as f32 as f64,
        time,
        (width_scale as f32).max(MIN_WIDTH_SCALE as f32) as f64,
    )
}

/// 落库/STROKES 用的墨迹宽度(**直径**),保留旧路径的 f32 收窄:
/// 旧代码 (width_r(pressure, scale) * 2.0) as f64,width_r 返回 f32 半径,
/// 所以 Windows 历史笔迹在库里一直是 f32 精度的直径;这里维持同一精度,
/// 不因为换成 core 会话就悄悄升级成 f64。
pub fn db_width(width: f64) -> f64 {
    (width as f32) as f64
}

/// 实时绘制用的墨迹**半径**(视图坐标):旧 (width_r(pressure, scale) * zoom)
/// .max(0.5) 的逐行等价实现;zoom 是视图缩放(翻页模式恒 1.0)。
pub fn view_radius(width: f64, zoom: f32) -> f32 {
    ((width * 0.5) as f32 * zoom).max(0.5)
}

/// 一次笔 HID 报告的处理结果(与旧 handle_point 的四个分支一一对应)。
#[derive(Debug)]
pub enum PenReport {
    /// 未落笔的悬空报告:旧实现把 Up 喂给模型器被拒绝后整条忽略
    /// (唯一副作用 glaspen2_bubble_pen_activity 在调用本适配层之前完成)。
    Ignored,
    /// 落笔:新笔首点(模型器 Down 恒返回 1 点)。
    Began(Vec<ModelerPoint>),
    /// 书写中移动:本次新增平滑点(模型器保证非空)。
    Moved(Vec<ModelerPoint>),
    /// 抬笔:本次收敛点 + 本笔元数据。
    Finished(FinishedStroke),
}

/// Windows 覆盖层的单笔模型器(D9b):只做事件判定,平滑数学全在 core 会话。
pub struct PenModeler {
    session: ModelerSession,
}

impl Default for PenModeler {
    fn default() -> Self {
        Self::new()
    }
}

impl PenModeler {
    /// 用 core 单源参数(ModelerSession::new)建一个空闲适配器。
    pub fn new() -> Self {
        Self {
            session: ModelerSession::new(),
        }
    }

    /// 喂一个 HID 笔报告。
    ///
    /// down = 报告的笔尖按下位;sample 的时间必须是本笔时间轴上的绝对秒
    /// (Windows adapter = stroke_origin.elapsed().as_secs_f64())。
    ///
    /// 错误处理保持旧策略:模型器拒绝(重复点/负时间差/间隔过大)时整条报告
    /// 丢弃,会话状态与缓冲不变,调用方等待下一条报告(旧实现 Err(_) => return None)。
    pub fn feed(&mut self, down: bool, sample: PenSample) -> Result<PenReport, SessionError> {
        if down {
            if self.session.is_active() {
                self.session.push(sample).map(PenReport::Moved)
            } else {
                self.session
                    .begin(StrokeStyle::default(), sample)
                    .map(PenReport::Began)
            }
        } else if self.session.is_active() {
            self.session.finish(sample).map(PenReport::Finished)
        } else {
            Ok(PenReport::Ignored)
        }
    }

    /// 当前是否有进行中的笔(旧 OverlayState::in_stroke 字段的唯一来源)。
    pub fn is_active(&self) -> bool {
        self.session.is_active()
    }

    /// 取消进行中的笔并清空输出缓冲(旧 reset_w_params + 清缓冲)。
    pub fn reset(&mut self) {
        self.session.reset();
    }

    /// 本笔落笔时间(秒;未落笔时 0)。
    pub fn start_time(&self) -> f64 {
        self.session.start_time()
    }

    /// 整笔累计输出点数。
    pub fn point_count(&self) -> usize {
        self.session.point_count()
    }

    /// 整笔累计输出点(缓冲)。
    pub fn points(&self) -> &[ModelerPoint] {
        self.session.points()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glaspen_core::modeler_session::pressure_to_width;

    /// 与 core modeler_session 特征化测试同一组固定输入:8 点右手弧线 +
    /// 压力起伏。时间从 0.4s 起 —— Windows adapter 用自己的单调时钟,首点
    /// 时间不为 0 是常态(旧实现同样以 elapsed 作模型器输入时间)。
    fn fixed_samples() -> Vec<PenSample> {
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
            .map(|(i, &(x, y, p))| sample(x, y, p, 0.4 + i as f64 * 0.005, 1.0))
            .collect()
    }

    /// 按旧 Windows 事件序列走完一笔,返回整笔平滑输出。
    fn run_stroke(modeler: &mut PenModeler) -> Vec<ModelerPoint> {
        let samples = fixed_samples();
        let mut out = match modeler.feed(true, samples[0]).expect("Down 应成功") {
            PenReport::Began(points) => points,
            other => panic!("首个落笔报告应为 Began: {other:?}"),
        };
        for sample in &samples[1..samples.len() - 1] {
            match modeler.feed(true, *sample).expect("Move 应成功") {
                PenReport::Moved(points) => out.extend(points),
                other => panic!("书写中的落笔报告应为 Moved: {other:?}"),
            }
        }
        match modeler
            .feed(false, *samples.last().unwrap())
            .expect("Up 应成功")
        {
            PenReport::Finished(finished) => out.extend(finished.points),
            other => panic!("抬笔报告应为 Finished: {other:?}"),
        }
        out
    }

    // ------------------------------------------------------------------
    // 事件序列 / 活跃阶段(旧 handle_point 分支)
    // ------------------------------------------------------------------

    #[test]
    fn feed_sequence_matches_old_windows_event_branches() {
        let mut modeler = PenModeler::new();
        let samples = fixed_samples();

        // 悬空报告(未落笔):旧实现喂 Up 被模型器拒绝,整条忽略。
        assert!(matches!(
            modeler.feed(false, samples[0]),
            Ok(PenReport::Ignored)
        ));
        assert!(!modeler.is_active());
        assert_eq!(modeler.point_count(), 0);

        // 落笔:Down 恒 1 点,位置/时间原样(Down 不做平滑)。
        let began = match modeler.feed(true, samples[0]).expect("begin") {
            PenReport::Began(points) => points,
            other => panic!("应为 Began: {other:?}"),
        };
        assert_eq!(began.len(), 1);
        assert_eq!(began[0].x, samples[0].x);
        assert_eq!(began[0].y, samples[0].y);
        assert_eq!(began[0].pressure, samples[0].pressure);
        assert_eq!(began[0].time, samples[0].time);
        // Down 输出宽度 = 单源公式(输入压力是 HID 的 f32 值转 f64)。
        assert_eq!(began[0].width, pressure_to_width(samples[0].pressure, 1.0));
        assert!(modeler.is_active());
        assert_eq!(modeler.start_time(), samples[0].time);

        // 书写中的落笔报告 = Move,每次至少 1 点。
        for sample in &samples[1..samples.len() - 1] {
            match modeler.feed(true, *sample).expect("move") {
                PenReport::Moved(points) => assert!(!points.is_empty(), "Move 必须产出点"),
                other => panic!("应为 Moved: {other:?}"),
            }
            assert!(modeler.is_active());
        }

        // 非落笔报告 = Up:收敛出点、带元数据、会话回空闲。
        let finished = match modeler.feed(false, *samples.last().unwrap()).expect("end") {
            PenReport::Finished(finished) => finished,
            other => panic!("应为 Finished: {other:?}"),
        };
        assert!(!finished.points.is_empty(), "Up 必须收敛出点");
        assert_eq!(finished.start_time, samples[0].time);
        assert_eq!(finished.total_points, modeler.point_count());
        assert!(!finished.style.erase, "Windows 无橡皮路径,样式恒非橡皮");
        assert!(!modeler.is_active());

        // 抬笔后的悬空报告回到忽略分支(旧实现同样被模型器拒绝)。
        assert!(matches!(
            modeler.feed(false, *samples.last().unwrap()),
            Ok(PenReport::Ignored)
        ));
    }

    #[test]
    fn hover_reports_are_ignored_and_do_not_touch_session() {
        let mut modeler = PenModeler::new();
        for i in 0..20 {
            let hover = sample(300.0 + i as f64, 400.0, 0.0, i as f64 * 0.008, 1.0);
            assert!(matches!(modeler.feed(false, hover), Ok(PenReport::Ignored)));
        }
        assert!(!modeler.is_active());
        assert_eq!(modeler.point_count(), 0);
        assert!(modeler.points().is_empty());

        // 悬空报告之后正常落笔仍然是新笔首点(不被悬空报告污染)。
        let down = sample(10.0, 20.0, 0.5, 1.0, 1.0);
        match modeler.feed(true, down).expect("begin after hover") {
            PenReport::Began(points) => {
                assert_eq!(points.len(), 1);
                assert_eq!(points[0].x, 10.0);
                assert_eq!(points[0].time, 1.0);
            }
            other => panic!("应为 Began: {other:?}"),
        }
    }

    #[test]
    fn duplicate_report_is_ignored_and_stroke_continues() {
        let mut modeler = PenModeler::new();
        let samples = fixed_samples();
        modeler.feed(true, samples[0]).expect("begin");
        modeler.feed(true, samples[1]).expect("first move");
        let before = modeler.point_count();

        // 完全相同的报告 = 模型器 Duplicate:整条丢弃,会话仍活跃,之后的点可继续。
        assert!(matches!(
            modeler.feed(true, samples[1]),
            Err(SessionError::Modeler(_))
        ));
        assert!(modeler.is_active());
        assert_eq!(modeler.point_count(), before);

        assert!(matches!(
            modeler.feed(true, samples[2]),
            Ok(PenReport::Moved(_))
        ));
        assert!(matches!(
            modeler
                .feed(false, *samples.last().unwrap())
                .expect("finish"),
            PenReport::Finished(_)
        ));
    }

    #[test]
    fn reset_cancels_stroke_and_allows_new_one() {
        let mut modeler = PenModeler::new();
        let samples = fixed_samples();
        modeler.feed(true, samples[0]).expect("begin");
        modeler.feed(true, samples[1]).expect("move");
        assert!(modeler.point_count() >= 2);

        // 清屏/隐藏/翻页等重建路径:取消当前笔,回到干净状态。
        modeler.reset();
        assert!(!modeler.is_active());
        assert_eq!(modeler.point_count(), 0);
        assert!(modeler.points().is_empty());
        match modeler.feed(true, samples[0]).expect("re-begin") {
            PenReport::Began(points) => assert_eq!(points.len(), 1),
            other => panic!("应为 Began: {other:?}"),
        }
    }

    // ------------------------------------------------------------------
    // 输出点 / 时间轴 / 宽度映射
    // ------------------------------------------------------------------

    #[test]
    fn live_points_are_smoothed_modeler_output() {
        let samples = fixed_samples();
        let mut modeler = PenModeler::new();
        let points = run_stroke(&mut modeler);

        // 120Hz 重采样:输出点数不少于原始输入点数。
        assert!(
            points.len() >= samples.len(),
            "输出 {} 点 < 输入 {} 点",
            points.len(),
            samples.len()
        );
        // 实时绘制源 = 模型器平滑输出(不是原始点直连):至少一个插值点。
        assert!(
            points
                .iter()
                .any(|p| !samples.iter().any(|s| s.x == p.x && s.y == p.y)),
            "应至少有一个平滑插值点(实时笔迹来自模型器输出)"
        );
        // 全部点有限、宽度为正、时间单调不减(与旧实现同语义)。
        assert!(points.iter().all(ModelerPoint::is_finite));
        assert!(points.iter().all(|p| p.width > 0.0));
        for pair in points.windows(2) {
            assert!(
                pair[0].time <= pair[1].time + 1e-12,
                "输出时间必须非递减: {} -> {}",
                pair[0].time,
                pair[1].time
            );
        }
        // 缓冲与输出一致(finish 后仍可用读取接口)。
        assert_eq!(modeler.point_count(), points.len());
        assert_eq!(modeler.points(), points.as_slice());
    }

    /// 旧 Windows 模型器的输出 r.time 存进 DB;core 旧全局路径
    /// (modeler::{begin_stroke,pen_move,end_stroke,take_buffer},macOS 的
    /// glaspen2_modeler_* 底层)在 begin 记 start_time 后输出
    /// start_time + r.time,与本适配层会话输出同一时间轴。用同一组样本对照:
    /// 位置/宽度/绝对时间必须逐位一致 —— 证明换 core 会话没有换掉输出语义。
    #[test]
    fn time_axis_matches_legacy_core_global_path() {
        let samples = fixed_samples();
        let last = *samples.last().unwrap();

        // 旧全局路径是进程级单例缓冲;本测试二进制里只有这里使用它。
        glaspen_core::modeler::begin_stroke(
            samples[0].x,
            samples[0].y,
            samples[0].pressure,
            samples[0].time,
            samples[0].width_scale,
        );
        for sample in &samples[1..samples.len() - 1] {
            glaspen_core::modeler::pen_move(
                sample.x,
                sample.y,
                sample.pressure,
                sample.time,
                sample.width_scale,
            );
        }
        glaspen_core::modeler::end_stroke(
            last.x,
            last.y,
            last.pressure,
            last.time,
            last.width_scale,
        );
        let legacy = glaspen_core::modeler::take_buffer();

        let mut modeler = PenModeler::new();
        let points = run_stroke(&mut modeler);

        assert_eq!(legacy.len(), points.len(), "点数必须一致");
        for (old, new) in legacy.iter().zip(points.iter()) {
            assert_eq!(
                *old,
                new.to_tuple(),
                "旧全局路径与 Windows 会话适配层逐点(含绝对时间)一致"
            );
        }
    }

    #[test]
    fn db_width_and_view_radius_keep_legacy_f32_wrapping() {
        // 旧 overlay/pen.rs::width_r 的原式(单源公式 + f32 收窄 + 倍率下限)。
        fn legacy_width_r(pressure: f32, scale: f32) -> f32 {
            (pressure_to_width(pressure as f64, scale.max(0.05) as f64) * 0.5) as f32
        }

        let mut modeler = PenModeler::new();
        let points = run_stroke(&mut modeler);
        assert!(!points.is_empty());

        for point in &points {
            // 落库宽度:旧 (width_r * 2.0) as f64。
            let legacy_db = (legacy_width_r(point.pressure as f32, 1.0) * 2.0) as f64;
            let new_db = db_width(point.width);
            // 唯一差异:模型器输出压力在旧实现里先收窄到 f32,会话用 f64;
            // 差值上界约半个 f32 ulp 乘曲线斜率(< 1e-6 px)。
            assert!(
                (new_db - legacy_db).abs() <= 2e-6,
                "DB 宽度漂移: {new_db} vs {legacy_db} (pressure={})",
                point.pressure
            );

            // 实时半径:旧 (width_r * zoom).max(0.5)。
            for zoom in [1.0_f32, 0.5, 2.0] {
                let legacy_radius = (legacy_width_r(point.pressure as f32, 1.0) * zoom).max(0.5);
                let new_radius = view_radius(point.width, zoom);
                assert!(
                    (new_radius - legacy_radius).abs() <= 1e-5,
                    "绘制半径漂移: {new_radius} vs {legacy_radius} (zoom={zoom})"
                );
            }
        }
        // 半径包装与 DB 宽度包装同源(旧 width_r 半径 × 2 = 直径),但绘制半径
        // 有 0.5px 下限(旧实现同款),DB 宽度没有 —— 这里区分两条路径。
        for point in &points {
            let raw_radius = (point.width * 0.5) as f32;
            if raw_radius >= 0.5 {
                assert_eq!(
                    db_width(point.width),
                    (view_radius(point.width, 1.0) * 2.0) as f64
                );
            } else {
                assert_eq!(view_radius(point.width, 1.0), 0.5);
            }
        }
    }

    #[test]
    fn width_scale_is_applied_per_report() {
        // 旧 Windows 行为:书写中换粗细(actions.rs 直接改 width_scale)对后续
        // 输出立即生效;会话按「每次送点携带 width_scale」映射,adapter 照旧逐条传。
        let mut modeler = PenModeler::new();
        let mut samples = fixed_samples();
        let first = modeler.feed(true, samples[0]).expect("begin");
        let first_width = match first {
            PenReport::Began(points) => points[0].width,
            other => panic!("应为 Began: {other:?}"),
        };
        assert_eq!(first_width, pressure_to_width(samples[0].pressure, 1.0));

        samples[1] = sample(samples[1].x, samples[1].y, 0.42, samples[1].time, 2.5);
        match modeler.feed(true, samples[1]).expect("move") {
            PenReport::Moved(points) => {
                assert!(!points.is_empty());
                for point in &points {
                    assert_eq!(
                        point.width,
                        pressure_to_width(point.pressure, 2.5),
                        "后续输出必须用当次 width_scale"
                    );
                }
            }
            other => panic!("应为 Moved: {other:?}"),
        }
        modeler.feed(false, *samples.last().unwrap()).expect("end");
    }

    #[test]
    fn sample_keeps_old_f32_narrowing_and_width_scale_floor() {
        // 压力先收窄 f32(旧 HID 归一化输出就是 f32)。
        let s = sample(1.0, 2.0, 0.3, 10.0, 1.0);
        assert_eq!(s.pressure, 0.3_f32 as f64);
        assert_eq!(s.width_scale, 1.0);
        // 倍率下限:旧 width_r 的 scale.max(0.05),防止历史库脏值把线宽缩成 0。
        // (下限本身是 f32 值再转 f64,与旧 width_r 的 scale.max(0.05) 同表示)
        let floor = MIN_WIDTH_SCALE as f32 as f64;
        let tiny = sample(0.0, 0.0, 0.5, 0.0, 0.001);
        assert_eq!(tiny.width_scale, floor);
        let negative = sample(0.0, 0.0, 0.5, 0.0, -3.0);
        assert_eq!(negative.width_scale, floor);
        // 位置/时间不做收窄。
        assert_eq!(s.x, 1.0);
        assert_eq!(s.time, 10.0);

        // 首点宽度 = core 单源公式(不再有本地 width 公式)。
        let mut modeler = PenModeler::new();
        match modeler
            .feed(true, sample(4.0, 5.0, 0.5, 2.0, 1.0))
            .expect("begin")
        {
            PenReport::Began(points) => {
                assert_eq!(points[0].width, pressure_to_width(0.5, 1.0));
            }
            other => panic!("应为 Began: {other:?}"),
        }
    }

    #[test]
    fn single_point_stroke_has_modeler_output_and_metadata() {
        // 一点笔:落笔后同位置稍晚抬笔 —— 旧实现同样得到 Down 1 点 + Up 收敛点。
        let down = sample(5.0, 6.0, 0.5, 12.0, 1.0);
        let up = sample(5.0, 6.0, 0.01, 12.05, 1.0);
        let mut modeler = PenModeler::new();
        let begin_points = match modeler.feed(true, down).expect("begin") {
            PenReport::Began(points) => points,
            other => panic!("应为 Began: {other:?}"),
        };
        assert_eq!(begin_points.len(), 1);

        let finished = match modeler.feed(false, up).expect("finish") {
            PenReport::Finished(finished) => finished,
            other => panic!("应为 Finished: {other:?}"),
        };
        assert!(!finished.points.is_empty());
        assert_eq!(finished.total_points, modeler.point_count());
        assert!(finished.total_points >= 2, "Down + Up 至少 2 点");
        assert_eq!(finished.start_time, 12.0);
        assert!((finished.duration - 0.05).abs() < 1e-12);
        assert!(finished.points.iter().all(ModelerPoint::is_finite));
        assert!(!modeler.is_active());

        // 0 压力(鼠标/无压感设备):宽度退回固定最细档,不是 0。
        assert!(db_width(begin_points[0].width) > 0.0);
    }
}
