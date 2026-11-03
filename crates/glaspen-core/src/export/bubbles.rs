//! 气泡消息: 把一段时间内的连续笔迹聚合成一条"消息", 供面板的
//! 「气泡消息」tab 以聊天窗样式回顾(右侧 = 用户口吻)。
//!
//! 分段规则(消息边界):
//! - 空闲超时: 距最近落笔 > [`IDLE`], 视为一次表达结束;
//! - 上下文切换: 翻页/清空/载入(检测到 STROKES 被重置)时, 未完成的段
//!   先结为一条消息;
//! - 容量护栏: 单段 > [`MAX_PENDING_STROKES`] 笔或 > [`MAX_PENDING_POINTS`]
//!   点时截断成下一条(防一次超长涂鸦撑爆内存)。
//!
//! 数据结构对齐手写消息的 ShareStroke 语义(color/width_scale/points),
//! 以 JSON 出口交给面板, Flutter CustomPainter 负责把折线画进气泡。
//! 纯内存、会话级: 进程退出即清空, 不落库。

use serde_json::json;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// 空闲分段阈值: 停笔超过此时长, 当前段结为一条消息。
const IDLE: Duration = Duration::from_secs(4);
/// 保留的消息条数上限(最旧的丢弃)。
const MAX_MSGS: usize = 50;
/// 单段笔数上限。
const MAX_PENDING_STROKES: usize = 60;
/// 单段点数上限。
const MAX_PENDING_POINTS: usize = 8000;

#[derive(Clone)]
struct BubbleStroke {
    /// 0xRRGGBB。
    rgb: u32,
    /// 宽度倍率(面板粗细预设, proto width_scale 同义)。
    width_scale: f64,
    /// 折线 (x, y, w) —— 画布坐标, w = 压感换算后的绝对线宽。
    pts: Vec<(f64, f64, f64)>,
}

struct BubbleMsg {
    id: u64,
    /// 完成时刻(unix 毫秒)。
    ts_ms: u64,
    strokes: Vec<BubbleStroke>,
}

struct Inner {
    /// 已捕获的 STROKES 前缀长度; STROKES 变短 = 被重置(翻页/清空)。
    captured: usize,
    pending: Vec<BubbleStroke>,
    pending_points: usize,
    last_activity: Option<Instant>,
    msgs: Vec<BubbleMsg>,
    next_id: u64,
}

static INNER: Mutex<Inner> = Mutex::new(Inner {
    captured: 0,
    pending: Vec::new(),
    pending_points: 0,
    last_activity: None,
    msgs: Vec::new(),
    next_id: 1,
});

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 提交钩子(抬笔扇出, STROKES 锁已释放): 捕获新笔迹 + 维护分段。
pub fn on_stroke_committed() {
    let mut g = INNER.lock().unwrap_or_else(|e| e.into_inner());
    let strokes = crate::STROKES.lock().unwrap_or_else(|e| e.into_inner());

    // STROKES 被重置(翻页/清空/载入): 未完结的段先结为消息再重同步。
    if strokes.len() < g.captured {
        finish_pending(&mut g);
        g.captured = strokes.len();
    }
    // 空闲超时: 上次落笔距今超过阈值, 当前段结为消息, 本笔开新段。
    if !g.pending.is_empty()
        && g.last_activity.is_some_and(|t| t.elapsed() > IDLE)
    {
        finish_pending(&mut g);
    }
    while g.captured < strokes.len() {
        let s = &strokes[g.captured];
        g.captured += 1;
        if s.points.is_empty() {
            continue;
        }
        let rgb = ((s.r.clamp(0.0, 1.0) * 255.0).round() as u32) << 16
            | ((s.g.clamp(0.0, 1.0) * 255.0).round() as u32) << 8
            | (s.b.clamp(0.0, 1.0) * 255.0).round() as u32;
        let pts: Vec<(f64, f64, f64)> = s
            .points
            .iter()
            .map(|&(x, y, w, _)| ((x * 10.0).round() / 10.0, (y * 10.0).round() / 10.0, w))
            .collect();
        g.pending_points += pts.len();
        g.pending.push(BubbleStroke {
            rgb,
            width_scale: 1.0,
            pts,
        });
        // 容量护栏: 超长段截断, 新笔迹开下一段。
        if g.pending.len() > MAX_PENDING_STROKES || g.pending_points > MAX_PENDING_POINTS {
            finish_pending(&mut g);
        }
    }
    g.last_activity = Some(Instant::now());
}

/// 把进行中的段结为一条消息(容量护栏路径)。
fn finish_pending(g: &mut Inner) {
    if g.pending.is_empty() {
        return;
    }
    let msg = BubbleMsg {
        id: g.next_id,
        ts_ms: now_ms(),
        strokes: std::mem::take(&mut g.pending),
    };
    g.next_id += 1;
    g.pending_points = 0;
    g.msgs.push(msg);
    if g.msgs.len() > MAX_MSGS {
        g.msgs.remove(0);
    }
}

/// 惰性过期: 取数据前调用, 把已超空闲阈值的未完结段结为消息。
fn flush_expired(g: &mut Inner) {
    if !g.pending.is_empty() && g.last_activity.is_some_and(|t| t.elapsed() > IDLE) {
        finish_pending(g);
    }
}

/// 强制结段(打开气泡 tab 时调用): 进行中的段立即成为一条消息。
pub fn flush_now() {
    let mut g = INNER.lock().unwrap_or_else(|e| e.into_inner());
    finish_pending(&mut g);
}

/// 全部消息的 JSON(先惰性过期)。形如:
/// `{"messages":[{"id":1,"ts_ms":...,"strokes":[{"rgb":16711680,"w":2.0,
///   "pts":[x,y,w, ...]}]}]}`
pub fn messages_json() -> String {
    let mut g = INNER.lock().unwrap_or_else(|e| e.into_inner());
    flush_expired(&mut g);
    let msgs: Vec<serde_json::Value> = g
        .msgs
        .iter()
        .map(|m| {
            json!({
                "id": m.id,
                "ts_ms": m.ts_ms,
                "strokes": m.strokes.iter().map(|s| json!({
                    "rgb": s.rgb,
                    "w": s.width_scale,
                    "pts": s.pts.iter().flat_map(|&(x, y, w)| [x, y, w]).collect::<Vec<f64>>(),
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({ "messages": msgs }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 分段规则: 空闲超时结段; STROKES 重置(翻页/清空)强制结段。
    /// 只操作 bubbles 自己的 INNER + 一个临时 STROKES 替身 —— 不碰真库。
    /// 测试前置: STROKES 清空 + 累积器归零(全量跑时其他测试会留残余,
    /// 不清场会把别人的尾巴吞进本测试的段里)。
    fn reset_fixture() {
        crate::STROKES.lock().unwrap().clear();
        let mut g = INNER.lock().unwrap();
        g.captured = 0;
        g.pending.clear();
        g.pending_points = 0;
        g.msgs.clear();
        g.next_id = 1;
        g.last_activity = None;
    }

    #[test]
    fn segmentation_idle_and_reset() {
        // STROKES 是全局静态, 直接推几条"笔迹"再清掉, 不影响其他测试
        // (其他测试用 TEST_LOCK 串行, 本测试同样加锁防交错)。
        let _g = crate::tests::TEST_LOCK.lock().unwrap();
        reset_fixture();
        {
            let mut strokes = crate::STROKES.lock().unwrap();
            strokes.push(crate::Stroke {
                id: 1,
                r: 1.0,
                g: 0.0,
                b: 0.0,
                points: vec![(0.0, 0.0, 2.0, 0.0), (10.0, 10.0, 2.0, 0.1)],
            });
            strokes.push(crate::Stroke {
                id: 2,
                r: 0.0,
                g: 0.0,
                b: 1.0,
                points: vec![(5.0, 5.0, 3.0, 0.0)],
            });
        }
        on_stroke_committed();
        assert_eq!(INNER.lock().unwrap().pending.len(), 2, "两笔进同一段");

        // 翻页/清空 → STROKES 变短 → 段强制完结
        crate::STROKES.lock().unwrap().clear();
        on_stroke_committed();
        {
            // 守卫块: INNER 不能跨 on_stroke_committed 持有(它内部再锁 INNER)
            let g = INNER.lock().unwrap();
            assert_eq!(g.msgs.len(), 1, "重置应结出 1 条消息");
            assert_eq!(g.msgs[0].strokes.len(), 2);
            assert_eq!(g.msgs[0].strokes[0].rgb, 0xFF0000, "红笔颜色应保留");
            assert_eq!(g.captured, 0, "重置后同步到 0");
            assert!(g.pending.is_empty());
        }

        // 容量护栏: 连续空笔不会撑爆段
        on_stroke_committed();
        assert_eq!(INNER.lock().unwrap().pending.len(), 0);

        // 清场: 恢复空 STROKES + 归零累积器
        INNER.lock().unwrap().msgs.clear();
        INNER.lock().unwrap().next_id = 1;
        INNER.lock().unwrap().captured = 0;
    }

    /// JSON 出口形状: messages[] / id / ts_ms / strokes[].rgb / pts 平铺。
    #[test]
    fn messages_json_shape() {
        let _g = crate::tests::TEST_LOCK.lock().unwrap();
        reset_fixture();
        {
            let mut strokes = crate::STROKES.lock().unwrap();
            strokes.push(crate::Stroke {
                id: 9,
                r: 0.0,
                g: 1.0,
                b: 0.0,
                points: vec![(1.0, 2.0, 3.0, 0.0)],
            });
        }
        on_stroke_committed();
        crate::STROKES.lock().unwrap().clear();
        on_stroke_committed();
        let v: serde_json::Value =
            serde_json::from_str(&messages_json()).expect("应为合法 JSON");
        let msgs = v["messages"].as_array().expect("messages 数组");
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["strokes"][0]["rgb"], 0x00FF00);
        let pts = msgs[0]["strokes"][0]["pts"].as_array().unwrap();
        assert_eq!(pts.len(), 3, "pts 平铺为 x,y,w 三元组");
        assert_eq!(pts[0], 1.0);
        assert!(msgs[0]["ts_ms"].as_u64().unwrap() > 0);

        // 清场
        let mut g = INNER.lock().unwrap();
        g.msgs.clear();
        g.next_id = 1;
        g.captured = 0;
    }
}
