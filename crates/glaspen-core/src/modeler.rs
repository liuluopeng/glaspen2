//! macOS 全局模型器路径(FFI 用)与离线平滑辅助。
//!
//! D9b 第三步:全局 STATE 已改为 core modeler_session 的委托 —— 参数表 /
//! 压感→笔宽公式 / begin-move-end 平滑的唯一实现都在 modeler_session,本文件
//! 只保留 macOS FFI(glaspen2_modeler_*)依赖的函数签名与全局缓冲语义:
//! - 进程级单例 STATE(Mutex<Option<...>>)由 FFI 按事件顺序驱动;
//! - 输出缓冲是 (x, y, width, 绝对时间) 元组,供 export/mod.rs 逐点读回;
//! - 错误处理保持旧策略:模型器拒绝(重复点/负时间差/间隔过大)时记日志、
//!   缓冲与阶段不变,不 panic;
//! - smooth_points 是「整段原始点离线平滑」的独立入口(export/pages.rs 用,
//!   导入/迁移场景),不属于单笔会话的实时语义,仍是自己的临时模型器实例。
//!
//! 会话内部历史语义见 modeler_session 模块文档(时间原点/宽度映射/两端差异)。

use std::sync::Mutex;

use crate::modeler_session::{ModelerPoint, ModelerSession, PenSample, StrokeStyle};

pub use crate::modeler_session::pressure_to_width;

struct GlobalState {
    session: ModelerSession,
}

static STATE: Mutex<Option<GlobalState>> = Mutex::new(None);

fn sample(x: f64, y: f64, pressure: f64, timestamp: f64, width_scale: f64) -> PenSample {
    PenSample::new(x, y, pressure, timestamp, width_scale)
}

/// Initialize the modeler for a new stroke. Called on pen down.
///
/// 参数非法/模型器拒绝时只记日志(与旧实现一致),缓冲由 begin 清空。
pub fn begin_stroke(x: f64, y: f64, pressure: f64, timestamp: f64, width_scale: f64) {
    let mut state_lock = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let state = state_lock.get_or_insert_with(|| GlobalState {
        session: ModelerSession::new(),
    });
    if let Err(e) = state.session.begin(
        StrokeStyle::default(),
        sample(x, y, pressure, timestamp, width_scale),
    ) {
        tracing::error!("Down error: {e}");
    }
}

/// Feed a pen move event into the modeler. Smoothed output goes into the buffer.
pub fn pen_move(x: f64, y: f64, pressure: f64, timestamp: f64, width_scale: f64) {
    let mut state_lock = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let Some(state) = state_lock.as_mut() else {
        tracing::info!("Move: no state!");
        return;
    };
    if let Err(e) = state
        .session
        .push(sample(x, y, pressure, timestamp, width_scale))
    {
        tracing::error!("Move error: {e}");
    }
}

/// Finalize the stroke. Modeler converges to the final position.
pub fn end_stroke(x: f64, y: f64, pressure: f64, timestamp: f64, width_scale: f64) {
    let mut state_lock = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let Some(state) = state_lock.as_mut() else {
        return;
    };
    if let Err(e) = state
        .session
        .finish(sample(x, y, pressure, timestamp, width_scale))
    {
        tracing::error!("Up error: {e}");
    }
}

/// Take the buffered smoothed points. Clears the buffer.
pub fn take_buffer() -> Vec<(f64, f64, f64, f64)> {
    let mut state_lock = STATE.lock().unwrap_or_else(|e| e.into_inner());
    match state_lock.as_mut() {
        Some(state) => state
            .session
            .take_points()
            .into_iter()
            .map(ModelerPoint::to_tuple)
            .collect(),
        None => Vec::new(),
    }
}

/// Get the current buffer length.
pub fn buffer_len() -> usize {
    let state_lock = STATE.lock().unwrap_or_else(|e| e.into_inner());
    state_lock.as_ref().map_or(0, |s| s.session.point_count())
}

/// Get a single point from the buffer by index.
pub fn get_buffer_point(idx: usize) -> Option<(f64, f64, f64, f64)> {
    let state_lock = STATE.lock().unwrap_or_else(|e| e.into_inner());
    state_lock
        .as_ref()
        .and_then(|s| s.session.point(idx))
        .map(ModelerPoint::to_tuple)
}

/// Clear the buffer.
pub fn clear_buffer() {
    let mut state_lock = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(state) = state_lock.as_mut() {
        state.session.clear_points();
    }
}

/// Smooth a set of raw points through a fresh modeler instance.
/// Returns smoothed (x, y, width, estimated_t) points. Each smoothed point
/// inherits the width of the nearest raw input point, and time is estimated
/// at 120 Hz sampling rate.
pub fn smooth_points(points: &[(f64, f64, f64)]) -> Vec<(f64, f64, f64, f64)> {
    if points.len() < 2 {
        return points.iter().map(|&(x, y, w)| (x, y, w, 0.0)).collect();
    }

    let params = crate::modeler_session::modeler_params();
    let mut modeler = match ink_stroke_modeler_rs::StrokeModeler::new(params) {
        Ok(m) => m,
        Err(_) => return points.iter().map(|&(x, y, w)| (x, y, w, 0.0)).collect(),
    };

    // Down with first point
    let (x0, y0, w0) = points[0];
    let down = ink_stroke_modeler_rs::ModelerInput {
        event_type: ink_stroke_modeler_rs::ModelerInputEventType::Down,
        pos: (x0, y0),
        time: 0.0,
        pressure: 0.5,
    };
    if modeler.update(down).is_err() {
        return points.iter().map(|&(x, y, w)| (x, y, w, 0.0)).collect();
    }

    let mut result = vec![(x0, y0, w0, 0.0)];
    let mut input_idx = vec![0usize];

    // Move for each subsequent point
    for (i, &(x, y, _)) in points.iter().enumerate().skip(1) {
        let t = i as f64 / 120.0;
        let mv = ink_stroke_modeler_rs::ModelerInput {
            event_type: ink_stroke_modeler_rs::ModelerInputEventType::Move,
            pos: (x, y),
            time: t,
            pressure: 0.5,
        };
        if let Ok(results) = modeler.update(mv) {
            for r in results {
                result.push((r.pos.0, r.pos.1, 0.0, r.time));
                input_idx.push(i);
            }
        }
    }

    // Up with last point
    let last = points.last().unwrap();
    let t = points.len() as f64 / 120.0;
    let up = ink_stroke_modeler_rs::ModelerInput {
        event_type: ink_stroke_modeler_rs::ModelerInputEventType::Up,
        pos: (last.0, last.1),
        time: t,
        pressure: 0.5,
    };
    if let Ok(results) = modeler.update(up) {
        for r in results {
            result.push((r.pos.0, r.pos.1, 0.0, r.time));
            input_idx.push(points.len() - 1);
        }
    }

    // Map widths: each smoothed point inherits width from nearest raw input point
    for (i, &idx) in input_idx.iter().enumerate() {
        result[i].2 = points[idx].2;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pressure_to_width_values() {
        assert!((pressure_to_width(0.0, 1.0) - 1.0).abs() < 0.01);
        assert!((pressure_to_width(0.009, 1.0) - 1.0).abs() < 0.01);
        // at threshold 0.01, pressure formula kicks in
        let w0 = pressure_to_width(0.1, 1.0);
        let w1 = pressure_to_width(0.5, 1.0);
        assert!(w1 > w0, "higher pressure should give larger width");
        assert!(w0 > 0.3, "formula result should be reasonable");
    }

    #[test]
    fn test_smooth_points_few_points() {
        // 1 point — should return it unchanged
        let one = vec![(5.0, 5.0, 2.0)];
        let r = smooth_points(&one);
        assert_eq!(r.len(), 1);
        assert!((r[0].0 - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_smooth_points_produces_valid_output() {
        // A short diagonal stroke
        let raw = vec![(0.0, 0.0, 2.0), (10.0, 0.0, 3.0), (20.0, 0.0, 2.0)];
        let result = smooth_points(&raw);
        assert!(
            result.len() >= 3,
            "should produce at least same number of points"
        );
        // All points should have positive width
        for &(_, _, w, _) in &result {
            assert!(w > 0.0, "each point must have width");
        }
    }

    #[test]
    fn test_modeler_begin_end_buffer_cleared() {
        // 全局 STATE 是进程级状态: 与 modeler_session 的特征化测试串行。
        let _guard = crate::tests::TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let _ = STATE.lock().unwrap_or_else(|e| e.into_inner()).take();
        begin_stroke(0.0, 0.0, 0.5, 0.0, 1.0);
        let buf = take_buffer();
        assert!(
            !buf.is_empty(),
            "begin_stroke should produce modeler output"
        );
        // take 之后缓冲清空; begin 重新开始一笔(旧 STATE 语义)
        assert_eq!(buffer_len(), 0);
        begin_stroke(10.0, 10.0, 0.5, 1.0, 1.0);
        assert_eq!(buffer_len(), 1, "Down 恒 1 点");
        assert_eq!(get_buffer_point(0).map(|p| p.3), Some(1.0));
        clear_buffer();
        assert_eq!(buffer_len(), 0);
        assert_eq!(get_buffer_point(0), None);
    }

    #[test]
    fn global_path_matches_session_time_and_width_semantics() {
        // D9b 第三步:全局函数只是会话委托 —— 逐点与直接会话一致(时间轴/宽度/
        // 缓冲 take 语义),FFI 读回顺序不变。
        let _guard = crate::tests::TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let _ = STATE.lock().unwrap_or_else(|e| e.into_inner()).take();
        let samples: Vec<PenSample> = (0..8)
            .map(|i| {
                PenSample::new(
                    100.0 + i as f64 * 4.0,
                    200.0 + (i as f64 * 0.7).sin() * 10.0,
                    0.3 + i as f64 * 0.05,
                    1000.0 + i as f64 * 0.005,
                    1.0,
                )
            })
            .collect();
        begin_stroke(
            samples[0].x,
            samples[0].y,
            samples[0].pressure,
            samples[0].time,
            samples[0].width_scale,
        );
        for s in &samples[1..samples.len() - 1] {
            pen_move(s.x, s.y, s.pressure, s.time, s.width_scale);
        }
        let last = *samples.last().unwrap();
        end_stroke(last.x, last.y, last.pressure, last.time, last.width_scale);
        let via_global = take_buffer();

        let mut session = ModelerSession::new();
        let mut direct = session
            .begin(StrokeStyle::default(), samples[0])
            .expect("session begin");
        for s in &samples[1..samples.len() - 1] {
            direct.extend(session.push(*s).expect("session push"));
        }
        direct.extend(session.finish(last).expect("session finish").points);
        let via_session: Vec<(f64, f64, f64, f64)> = direct.iter().map(|p| p.to_tuple()).collect();

        assert_eq!(via_global, via_session);
    }
}
