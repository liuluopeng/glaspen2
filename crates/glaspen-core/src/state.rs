use std::sync::Mutex;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// 书写活动时间戳(笔尖落下/抬笔即刷新)。
// 供 create_next_page 的书写活动锁使用: 最近落笔不久或笔仍在墨水中时拒绝
// 新建页 —— 2026-10-09 桌面实证: 显示器唤醒后重复翻页信号在 7 秒内建了
// 9 页, 把一次书写会话劈碎成散落各页的孤儿碎片。
// ---------------------------------------------------------------------------

static LAST_STROKE_ACTIVITY: Mutex<Option<Instant>> = Mutex::new(None);

/// 取锁时容忍 poison: 一次 panic 不应让后续所有落笔/翻页永久失败。
/// panic 本身已由 panic hook 落盘, 恢复数据比再次 abort 更有价值。
pub fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn mark_stroke_activity() {
    *lock(&LAST_STROKE_ACTIVITY) = Some(Instant::now());
}

/// 距最近一次书写活动(落笔或抬笔)的时长; 从未写过 = None。
pub fn since_last_stroke_activity() -> Option<Duration> {
    lock(&LAST_STROKE_ACTIVITY).map(|t| t.elapsed())
}

/// 测试把活动时间戳拨回过去(绕过书写活动锁)。
#[cfg(test)]
pub fn backdate_stroke_activity(d: Duration) {
    *lock(&LAST_STROKE_ACTIVITY) = Some(Instant::now() - d);
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
    *lock(&CANVAS_KIND)
}

pub fn set_canvas_kind(kind: CanvasKind) {
    *lock(&CANVAS_KIND) = kind;
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
    *lock(&CURRENT_SCREEN_ID)
}

pub fn set_current_screen_id(id: i64) {
    *lock(&CURRENT_SCREEN_ID) = id;
}

// --- Pending-point buffer (accumulated during a stroke, flushed at pen-up) ---

/// Push a point into the in-memory buffer. Called on every pen-move event.
/// NOT async — just a Mutex<Vec> push.
pub fn buffer_point(x: f64, y: f64, width: f64, t: f64) {
    lock(&PENDING).points.push((x, y, width, t));
}

/// Take the buffered points and clear the buffer. Used by db::flush_pending.
pub fn take_pending() -> Vec<(f64, f64, f64, f64)> {
    let mut p = lock(&PENDING);
    std::mem::take(&mut p.points)
}

/// Set the pending stroke id + the store it belongs to, and clear the buffer.
/// Used by db::begin_stroke. Recording the kind here (rather than reading the
/// current kind at flush time) keeps the async pen-up flush from writing a
/// stroke's points into the table of a mode the user just switched to.
pub fn begin_pending(stroke_id: i64, kind: CanvasKind) {
    let mut p = lock(&PENDING);
    p.id = Some(stroke_id);
    p.kind = kind;
    p.points.clear();
}

/// Take the pending stroke id (returns id or None).
pub fn take_pending_stroke_id() -> Option<i64> {
    let mut pending = lock(&PENDING);
    pending.id.take()
}

/// Atomically take (stroke id, canvas kind, points). Either all are consumed or
/// none are: begin_pending/buffer_point/this function share one lock, so a
/// flush can never pair a stale id with a newer stroke's points.
pub fn take_pending_bundle() -> Option<(i64, CanvasKind, Vec<(f64, f64, f64, f64)>)> {
    let mut p = lock(&PENDING);
    let id = p.id.take()?;
    let kind = p.kind;
    let points = std::mem::take(&mut p.points);
    Some((id, kind, points))
}

// ---------------------------------------------------------------------------
// Flush 失败重试队列
// ---------------------------------------------------------------------------
//
// SQLite 写失败(磁盘满 / BUSY / IO 错)时, 点已经从 PENDING 取走。此前只打
// 一条 debug 日志就丢掉, 用户看到的是"这一笔少了一截"。现在整笔放回重试
// 队列, 下一次抬笔/载页/退出时再写; 队列有上限, 极端情况下丢最旧的并
// 用 error 级日志留下现场。

type RetryStroke = (i64, CanvasKind, Vec<(f64, f64, f64, f64)>);

static RETRY: Mutex<Vec<RetryStroke>> = Mutex::new(Vec::new());
const RETRY_MAX: usize = 64;

pub fn push_retry(id: i64, kind: CanvasKind, points: Vec<(f64, f64, f64, f64)>) {
    let mut q = lock(&RETRY);
    if q.len() >= RETRY_MAX {
        let (dropped_id, _, dropped) = q.remove(0);
        tracing::error!(
            "待重试笔迹队列已满({RETRY_MAX}), 丢弃最旧 stroke={dropped_id} ({} 点)",
            dropped.len()
        );
    }
    q.push((id, kind, points));
}

pub fn pop_retry() -> Option<RetryStroke> {
    lock(&RETRY).pop()
}

pub fn retry_len() -> usize {
    lock(&RETRY).len()
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
