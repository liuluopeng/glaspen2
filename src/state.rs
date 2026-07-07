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
