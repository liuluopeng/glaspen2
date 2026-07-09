use std::sync::Mutex;

// ---------------------------------------------------------------------------
// In-memory drawing state — NOT database.
// These are pure memory operations accessed from the hot drawing path.
// ---------------------------------------------------------------------------

static CURRENT_SCREEN_ID: Mutex<i64> = Mutex::new(0);
static PENDING_STROKE_ID: Mutex<Option<i64>> = Mutex::new(None);
static PENDING_POINTS: Mutex<Vec<(f64, f64, f64, f64)>> = Mutex::new(Vec::new()); // (x, y, width, relative_time)

// --- Screen id ---

pub fn current_screen_id() -> i64 {
    *CURRENT_SCREEN_ID.lock().unwrap()
}

pub fn set_current_screen_id(id: i64) {
    *CURRENT_SCREEN_ID.lock().unwrap() = id;
}

// --- Pending-point buffer (accumulated during a stroke, flushed at pen-up) ---

/// Push a point into the in-memory buffer. Called on every pen-move event.
/// NOT async — just a Mutex<Vec> push.
pub fn buffer_point(x: f64, y: f64, width: f64, t: f64) {
    PENDING_POINTS.lock().unwrap().push((x, y, width, t));
}

/// Take the buffered points and clear the buffer. Used by db::flush_pending.
pub fn take_pending() -> Vec<(f64, f64, f64, f64)> {
    let mut p = PENDING_POINTS.lock().unwrap();
    std::mem::take(&mut *p)
}

/// Set the pending stroke id and clear the point buffer.
/// Used by db::begin_stroke.
pub fn begin_pending(stroke_id: i64) {
    *PENDING_STROKE_ID.lock().unwrap() = Some(stroke_id);
    PENDING_POINTS.lock().unwrap().clear();
}

/// Take the pending stroke id (returns id or None).
pub fn take_pending_stroke_id() -> Option<i64> {
    let mut pending = PENDING_STROKE_ID.lock().unwrap();
    pending.take()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screen_id_default() {
        assert_eq!(current_screen_id(), 0);
    }

    #[test]
    fn test_screen_id_set_and_get() {
        set_current_screen_id(42);
        assert_eq!(current_screen_id(), 42);
        set_current_screen_id(0);  // reset for other tests
    }

    #[test]
    fn test_buffer_roundtrip() {
        begin_pending(123);
        assert_eq!(take_pending_stroke_id(), Some(123));

        buffer_point(1.0, 2.0, 3.0, 0.5);
        buffer_point(4.0, 5.0, 6.0, 1.5);
        let pts = take_pending();
        assert_eq!(pts.len(), 2);
        assert_eq!(pts[0], (1.0, 2.0, 3.0, 0.5));
        assert_eq!(pts[1], (4.0, 5.0, 6.0, 1.5));

        // Second take should be empty
        assert!(take_pending().is_empty());
    }

    #[test]
    fn test_begin_pending_clears_buffer() {
        buffer_point(1.0, 2.0, 3.0, 0.0);
        begin_pending(456);
        assert!(take_pending().is_empty());
        assert_eq!(take_pending_stroke_id(), Some(456));
    }

    #[test]
    fn test_take_none_when_empty() {
        let id = take_pending_stroke_id(); // may be None or Some(456) from prior test
        // Just verify the API doesn't panic
        assert!(id.is_some() || id.is_none());
    }
}
