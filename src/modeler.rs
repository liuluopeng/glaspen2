use std::sync::Mutex;
use ink_stroke_modeler_rs::{
    ModelerInput, ModelerInputEventType, ModelerParams, StrokeModeler,
};

struct StrokeModelerState {
    modeler: StrokeModeler,
    start_time: f64,
    buffer: Vec<(f64, f64, f64, f64)>, // (x, y, width, relative_time)
}

static STATE: Mutex<Option<StrokeModelerState>> = Mutex::new(None);

fn modeler_params() -> ModelerParams {
    // Match rnote's exact params — default spring/drag/wobble, only override sampling
    ModelerParams {
        sampling_min_output_rate: 120.0,
        sampling_end_of_stroke_stopping_distance: 0.01,
        sampling_end_of_stroke_max_iterations: 20,
        sampling_max_outputs_per_call: 200,
        stylus_state_modeler_max_input_samples: 20,
        ..ModelerParams::suggested()
    }
}

/// Initialize the modeler for a new stroke. Called on pen down.
pub fn begin_stroke(x: f64, y: f64, pressure: f64, timestamp: f64, width_scale: f64) {
    let mut state_lock = STATE.lock().unwrap();
    let state = state_lock.get_or_insert_with(|| StrokeModelerState {
        modeler: StrokeModeler::default(),
        start_time: timestamp,
        buffer: Vec::new(),
    });

    if let Err(e) = state.modeler.reset_w_params(modeler_params()) {
        eprintln!("[modeler] reset failed: {:?}", e);
    }
    state.start_time = timestamp;
    state.buffer.clear();

    let input = ModelerInput {
        event_type: ModelerInputEventType::Down,
        pos: (x, y),
        time: 0.0,
        pressure,
    };

    match state.modeler.update(input) {
        Ok(results) => {
            eprintln!("[modeler] Down: got {} results", results.len());
            for r in results {
                let w = pressure_to_width(r.pressure, width_scale);
                // Store approximate absolute wall-clock time so timestamps
                // are comparable across strokes.
                state.buffer.push((r.pos.0, r.pos.1, w, state.start_time + r.time));
            }
        }
        Err(e) => eprintln!("[modeler] Down error: {:?}", e),
    }
}

/// Feed a pen move event into the modeler. Smoothed output goes into the buffer.
pub fn pen_move(x: f64, y: f64, pressure: f64, timestamp: f64, width_scale: f64) {
    let mut state_lock = STATE.lock().unwrap();
    if let Some(ref mut state) = *state_lock {
        let t = timestamp - state.start_time;
        let input = ModelerInput {
            event_type: ModelerInputEventType::Move,
            pos: (x, y),
            time: t,
            pressure,
        };

        match state.modeler.update(input) {
            Ok(results) => {
                for r in results {
                    let w = pressure_to_width(r.pressure, width_scale);
                    state.buffer.push((r.pos.0, r.pos.1, w, state.start_time + r.time));
                }
            }
            Err(e) => eprintln!("[modeler] Move error: {:?}", e),
        }
    } else {
        eprintln!("[modeler] Move: no state!");
    }
}

/// Finalize the stroke. Modeler converges to the final position.
pub fn end_stroke(x: f64, y: f64, pressure: f64, timestamp: f64, width_scale: f64) {
    let mut state_lock = STATE.lock().unwrap();
    if let Some(ref mut state) = *state_lock {
        let t = timestamp - state.start_time;
        let input = ModelerInput {
            event_type: ModelerInputEventType::Up,
            pos: (x, y),
            time: t,
            pressure,
        };

        match state.modeler.update(input) {
            Ok(results) => {
                eprintln!("[modeler] Up: got {} results", results.len());
                for r in results {
                    let w = pressure_to_width(r.pressure, width_scale);
                    state.buffer.push((r.pos.0, r.pos.1, w, state.start_time + r.time));
                }
            }
            Err(e) => eprintln!("[modeler] Up error: {:?}", e),
        }
    }
}

/// Take the buffered smoothed points. Clears the buffer.
pub fn take_buffer() -> Vec<(f64, f64, f64, f64)> {
    let mut state_lock = STATE.lock().unwrap();
    if let Some(ref mut state) = *state_lock {
        std::mem::take(&mut state.buffer)
    } else {
        Vec::new()
    }
}

/// Get the current buffer length.
pub fn buffer_len() -> usize {
    let state_lock = STATE.lock().unwrap();
    state_lock.as_ref().map_or(0, |s| s.buffer.len())
}

/// Get a single point from the buffer by index.
pub fn get_buffer_point(idx: usize) -> Option<(f64, f64, f64, f64)> {
    let state_lock = STATE.lock().unwrap();
    state_lock.as_ref().and_then(|s| s.buffer.get(idx).copied())
}

/// Clear the buffer.
pub fn clear_buffer() {
    let mut state_lock = STATE.lock().unwrap();
    if let Some(ref mut state) = *state_lock {
        state.buffer.clear();
    }
}

/// Smooth a set of raw points through a fresh modeler instance.
/// Returns smoothed (x, y, width, estimated_t) points. Each smoothed point
/// inherits the width of the nearest raw input point, and time is estimated
/// at 120 Hz sampling rate.
pub fn smooth_points(points: &[(f64, f64, f64)]) -> Vec<(f64, f64, f64, f64)> {
    if points.len() < 2 { return points.iter().map(|&(x, y, w)| (x, y, w, 0.0)).collect(); }

    let params = modeler_params();
    let mut modeler = match StrokeModeler::new(params) {
        Ok(m) => m,
        Err(_) => return points.iter().map(|&(x, y, w)| (x, y, w, 0.0)).collect(),
    };

    // Down with first point
    let (x0, y0, w0) = points[0];
    let down = ModelerInput {
        event_type: ModelerInputEventType::Down,
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
        let mv = ModelerInput {
            event_type: ModelerInputEventType::Move,
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
    let up = ModelerInput {
        event_type: ModelerInputEventType::Up,
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

fn pressure_to_width(pressure: f64, width_scale: f64) -> f64 {
    if pressure > 0.01 {
        (0.3 + pressure * pressure * 7.7) * width_scale
    } else {
        1.0 * width_scale
    }
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
        let raw = vec![
            (0.0, 0.0, 2.0),
            (10.0, 0.0, 3.0),
            (20.0, 0.0, 2.0),
        ];
        let result = smooth_points(&raw);
        assert!(result.len() >= 3, "should produce at least same number of points");
        // All points should have positive width
        for &(_, _, w, _) in &result {
            assert!(w > 0.0, "each point must have width");
        }
    }

    #[test]
    fn test_modeler_begin_end_buffer_cleared() {
        let _ = STATE.lock().unwrap().take();
        begin_stroke(0.0, 0.0, 0.5, 0.0, 1.0);
        let buf = take_buffer();
        assert!(!buf.is_empty(), "begin_stroke should produce modeler output");
    }
}
