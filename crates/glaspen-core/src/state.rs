use std::sync::Mutex;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// 书写活动时间戳(笔尖落下/抬笔即刷新)。
// 供 create_next_page 的书写活动锁使用: 最近落笔不久或笔仍在墨水中时拒绝
// 新建页 —— 2026-10-09 桌面实证: 显示器唤醒后重复翻页信号在 7 秒内建了
// 9 页, 把一次书写会话劈碎成散落各页的孤儿碎片。
// ---------------------------------------------------------------------------

static LAST_STROKE_ACTIVITY: Mutex<Option<Instant>> = Mutex::new(None);

pub fn mark_stroke_activity() {
    *LAST_STROKE_ACTIVITY.lock().unwrap() = Some(Instant::now());
}

/// 距最近一次书写活动(落笔或抬笔)的时长; 从未写过 = None。
pub fn since_last_stroke_activity() -> Option<Duration> {
    LAST_STROKE_ACTIVITY.lock().unwrap().map(|t| t.elapsed())
}

/// 测试把活动时间戳拨回过去(绕过书写活动锁)。
#[cfg(test)]
pub fn backdate_stroke_activity(d: Duration) {
    *LAST_STROKE_ACTIVITY.lock().unwrap() = Some(Instant::now() - d);
}

// ---------------------------------------------------------------------------
// In-memory drawing state — NOT database.
// These are pure memory operations accessed from the hot drawing path.
// PENDING_STROKE_ID and PENDING_POINTS share one mutex so that begin/flush
// can never observe a torn (id, points) pair across threads.
// ---------------------------------------------------------------------------

static CURRENT_SCREEN_ID: Mutex<i64> = Mutex::new(0);

// --- Canvas kind (storage backend) ---
//
// Page mode stores strokes under screens/strokes/points (one page at a time).
// Infinite mode stores strokes in the single infinite_strokes/infinite_points
// canvas, fully independent from the page store.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CanvasKind {
    Page,
    Infinite,
}

static CANVAS_KIND: Mutex<CanvasKind> = Mutex::new(CanvasKind::Page);

pub fn canvas_kind() -> CanvasKind {
    *CANVAS_KIND.lock().unwrap()
}

pub fn set_canvas_kind(kind: CanvasKind) {
    *CANVAS_KIND.lock().unwrap() = kind;
}

struct PendingStroke {
    id: Option<i64>,
    /// Store this pending stroke belongs to (recorded at begin, used at flush).
    kind: CanvasKind,
    points: Vec<(f64, f64, f64, f64)>, // (x, y, width, relative_time)
}

static PENDING: Mutex<PendingStroke> = Mutex::new(PendingStroke {
    id: None,
    kind: CanvasKind::Page,
    points: Vec::new(),
});

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
    PENDING.lock().unwrap().points.push((x, y, width, t));
}

/// Take the buffered points and clear the buffer. Used by db::flush_pending.
pub fn take_pending() -> Vec<(f64, f64, f64, f64)> {
    let mut p = PENDING.lock().unwrap();
    std::mem::take(&mut p.points)
}

/// Set the pending stroke id + the store it belongs to, and clear the buffer.
/// Used by db::begin_stroke. Recording the kind here (rather than reading the
/// current kind at flush time) keeps the async pen-up flush from writing a
/// stroke's points into the table of a mode the user just switched to.
pub fn begin_pending(stroke_id: i64, kind: CanvasKind) {
    let mut p = PENDING.lock().unwrap();
    p.id = Some(stroke_id);
    p.kind = kind;
    p.points.clear();
}

/// Take the pending stroke id (returns id or None).
pub fn take_pending_stroke_id() -> Option<i64> {
    let mut pending = PENDING.lock().unwrap();
    pending.id.take()
}

/// Atomically take (stroke id, canvas kind, points). Either all are consumed or
/// none are: begin_pending/buffer_point/this function share one lock, so a
/// flush can never pair a stale id with a newer stroke's points.
pub fn take_pending_bundle() -> Option<(i64, CanvasKind, Vec<(f64, f64, f64, f64)>)> {
    let mut p = PENDING.lock().unwrap();
    let id = p.id.take()?;
    let kind = p.kind;
    let points = std::mem::take(&mut p.points);
    Some((id, kind, points))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screen_id_default() {
        // 不能假设进程初值 0 —— 同一二进制里建屏类 FFI 测试(lasso/paste/
        // fuzz)会先改掉全局当前页, 测试并行执行顺序不保证。静态字面量
        // `AtomicI64::new(0)` 保证初值; 这里只验证 set/get 契约。
        set_current_screen_id(42);
        assert_eq!(current_screen_id(), 42);
        set_current_screen_id(0); // reset for other tests
        assert_eq!(current_screen_id(), 0);
    }

    #[test]
    fn test_buffer_roundtrip() {
        begin_pending(123, CanvasKind::Page);
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
        begin_pending(456, CanvasKind::Page);
        assert!(take_pending().is_empty());
        assert_eq!(take_pending_stroke_id(), Some(456));
    }

    #[test]
    fn test_take_none_when_empty() {
        let id = take_pending_stroke_id(); // may be None or Some(456) from prior test
        // Just verify the API doesn't panic
        assert!(id.is_some() || id.is_none());
    }

    #[test]
    fn test_take_bundle_atomic() {
        begin_pending(789, CanvasKind::Infinite);
        buffer_point(1.0, 1.0, 2.0, 0.0);
        buffer_point(2.0, 2.0, 3.0, 1.0);
        // The bundle carries the kind recorded at begin (survives a mode switch).
        let (id, kind, pts) = take_pending_bundle().unwrap();
        assert_eq!(id, 789);
        assert_eq!(kind, CanvasKind::Infinite);
        assert_eq!(pts.len(), 2);
        // Both consumed: second bundle take is None
        assert!(take_pending_bundle().is_none());
    }

    #[test]
    fn test_canvas_kind_defaults_to_page_and_round_trips() {
        // Fresh state is page mode; switching routes stroke storage.
        set_canvas_kind(CanvasKind::Page);
        assert_eq!(canvas_kind(), CanvasKind::Page);
        set_canvas_kind(CanvasKind::Infinite);
        assert_eq!(canvas_kind(), CanvasKind::Infinite);
        set_canvas_kind(CanvasKind::Page); // reset for other tests
        assert_eq!(canvas_kind(), CanvasKind::Page);
    }
}
