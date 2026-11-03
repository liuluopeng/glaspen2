//! 气泡消息: 把一段时间内的连续笔迹聚合成一条"消息", 供面板的
//! 「气泡消息」tab 以聊天窗样式回顾(右侧 = 用户口吻)。
//!
//! 分段规则(消息边界, 自适应而非固定超时):
//! - **节奏学习**: 段内相邻笔迹的间隔做 EMA, 分段阈值 = clamp(5×EMA,
//!   8s, 90s)。写得快的人 8 秒停顿就算一条; 边想边写的人 40 秒的思考
//!   也不会被拦腰截断 —— 阈值跟着用户自己的节奏走;
//! - **空间跳变**: 新笔起点距当前段包围盒的间隙 > max(2×对角线, 400px)
//!   时立即分段 —— "写完一个主题移到别处"不需要干等计时器;
//! - **上下文切换**: 翻页/清空/载入(检测到 STROKES 被重置)时, 未完成的段
//!   先结为一条消息;
//! - **容量护栏**: 单段 > MAX_PENDING_STROKES 笔或 > MAX_PENDING_POINTS
//!   点时截断成下一条(防一次超长涂鸦撑爆内存)。
//!
//! 数据结构对齐手写消息的 ShareStroke 语义(color/width_scale/points),
//! 以 JSON 出口交给面板, Flutter CustomPainter 负责把折线画进气泡。
//! 纯内存、会话级: 进程退出即清空, 不落库。

use serde_json::json;
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// 分段阈值下限: 再快的人, 8 秒停顿也算一条。
const THRESHOLD_MIN_MS: f64 = 8_000.0;
/// 上限: 边想边写的人也不会超过 90 秒。
const THRESHOLD_MAX_MS: f64 = 90_000.0;
/// 阈值 = EMA × 此系数。
const THRESHOLD_FACTOR: f64 = 5.0;
/// EMA 平滑系数。
const EMA_ALPHA: f64 = 0.3;
/// EMA 初值(2s ≈ 连贯书写的典型间隔)。
const INITIAL_TYPICAL_MS: f64 = 2_000.0;
/// 空间跳变最小间隙(px): 防止第一笔很小导致到处都算"跳变"。
const SPATIAL_MIN_GAP: f64 = 400.0;
/// 空间跳变相对项: 间隙 > 2×当前段对角线。
const SPATIAL_DIAG_FACTOR: f64 = 2.0;
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
    /// 段内相邻笔迹间隔的 EMA(ms) —— 用户书写节奏。
    typical_gap_ms: f64,
    msgs: Vec<BubbleMsg>,
    next_id: u64,
}

static INNER: Mutex<Inner> = Mutex::new(Inner {
    captured: 0,
    pending: Vec::new(),
    pending_points: 0,
    last_activity: None,
    typical_gap_ms: INITIAL_TYPICAL_MS,
    msgs: Vec::new(),
    next_id: 1,
});

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 当前分段阈值(毫秒): 随节奏 EMA 自适应。
fn threshold_ms(typical_gap_ms: f64) -> f64 {
    (typical_gap_ms * THRESHOLD_FACTOR).clamp(THRESHOLD_MIN_MS, THRESHOLD_MAX_MS)
}

/// EMA 更新(只喂段内间隔, 调用方保证 gap < 阈值)。
fn next_typical(gap_ms: u64, typical: f64) -> f64 {
    typical * (1.0 - EMA_ALPHA) + (gap_ms as f64) * EMA_ALPHA
}

/// 点到轴对齐矩形的间隙(内部为 0)。
fn gap_to_rect(px: f64, py: f64, x0: f64, y0: f64, x1: f64, y1: f64) -> f64 {
    let dx = (x0 - px).max(px - x1).max(0.0);
    let dy = (y0 - py).max(py - y1).max(0.0);
    (dx * dx + dy * dy).sqrt()
}

/// 分段判定(纯函数, 可测):
/// 1. 间隔 >= 节奏阈值 → 分;
/// 2. 空间跳变: 新笔起点距当前段包围盒间隙 > max(2×对角线, 400px) → 分。
/// bbox = None(段空)时只看时间。
fn should_split(
    gap_ms: u64,
    typical_gap_ms: f64,
    start: (f64, f64),
    bbox: Option<((f64, f64), (f64, f64))>,
) -> bool {
    if gap_ms as f64 >= threshold_ms(typical_gap_ms) {
        return true;
    }
    if let Some(((x0, y0), (x1, y1))) = bbox {
        let gap = gap_to_rect(start.0, start.1, x0, y0, x1, y1);
        let diag = ((x1 - x0) * (x1 - x0) + (y1 - y0) * (y1 - y0)).sqrt();
        return gap > (SPATIAL_DIAG_FACTOR * diag).max(SPATIAL_MIN_GAP);
    }
    false
}

/// 提交钩子(抬笔扇出, STROKES 锁已释放): 捕获新笔迹 + 维护分段。
pub fn on_stroke_committed() {
    let mut g = INNER.lock().unwrap_or_else(|e| e.into_inner());
    let strokes = crate::STROKES.lock().unwrap_or_else(|e| e.into_inner());

    // STROKES 被重置(翻页/清空): 未完结的段先结为消息再重同步。
    if strokes.len() < g.captured {
        finish_pending(&mut g);
        g.captured = strokes.len();
    }
    if g.captured >= strokes.len() {
        return; // 没有新笔迹(重置路径到此为止)
    }
    // 分段判定用"本次新笔"的起点; 间隔 = 距上次落笔的时长。
    let gap_ms = g
        .last_activity
        .map(|t| t.elapsed().as_millis() as u64)
        .unwrap_or(0);
    let new_start = strokes[g.captured]
        .points
        .first()
        .map(|&(x, y, _, _)| (x, y))
        .unwrap_or((0.0, 0.0));
    let bbox = pending_bbox(g.pending.as_slice());
    if !g.pending.is_empty() && should_split(gap_ms, g.typical_gap_ms, new_start, bbox) {
        finish_pending(&mut g);
    } else if !g.pending.is_empty() {
        // 段内间隔: 喂节奏 EMA(只有未触发分段的间隔才算"节奏")
        g.typical_gap_ms = next_typical(gap_ms, g.typical_gap_ms);
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

/// 当前进行中段的包围盒(空段 = None)。
fn pending_bbox(pending: &[BubbleStroke]) -> Option<((f64, f64), (f64, f64))> {
    let mut it = pending.iter().flat_map(|s| s.pts.iter());
    let Some(&(x, y, _)) = it.next() else {
        return None;
    };
    let mut x0 = x;
    let mut y0 = y;
    let mut x1 = x;
    let mut y1 = y;
    for &(x, y, _) in it {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    Some(((x0, y0), (x1, y1)))
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

/// 惰性过期: 取数据前调用, 把已超节奏阈值的未完结段结为消息。
fn flush_expired(g: &mut Inner) {
    if !g.pending.is_empty()
        && g
            .last_activity
            .is_some_and(|t| t.elapsed().as_millis() as f64 >= threshold_ms(g.typical_gap_ms))
    {
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
        g.typical_gap_ms = INITIAL_TYPICAL_MS;
    }

    /// 翻页/清空(STROKES 重置)强制结段; 空笔不撑爆段。
    #[test]
    fn segmentation_reset() {
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
    }

    /// 时间规则: 阈值 = clamp(5×EMA, 8s, 90s), 快/慢两种节奏都验证。
    #[test]
    fn split_time_adaptive() {
        // 快手: EMA=2s → 阈值 10s; 9.5s 不分, 10.5s 分
        assert_eq!(threshold_ms(2_000.0), 10_000.0);
        assert!(!should_split(9_500, 2_000.0, (0.0, 0.0), None));
        assert!(should_split(10_500, 2_000.0, (0.0, 0.0), None));

        // 慢思考: EMA=25s → 阈值 90s(顶到上限); 50s 不分
        assert_eq!(threshold_ms(25_000.0), THRESHOLD_MAX_MS);
        assert!(!should_split(50_000, 25_000.0, (0.0, 0.0), None));

        // 快到极致: EMA=0.1s → 阈值 8s(下限兜底)
        assert_eq!(threshold_ms(100.0), THRESHOLD_MIN_MS);
        assert!(!should_split(7_900, 100.0, (0.0, 0.0), None));
        assert!(should_split(8_100, 100.0, (0.0, 0.0), None));
    }

    /// 空间规则: 间隙 > max(2×对角线, 400px) 即分, 与时间无关。
    #[test]
    fn split_space_jump() {
        let bbox = Some(((0.0, 0.0), (100.0, 100.0))); // 对角线 ≈141 → 限额 400px
        assert!(!should_split(0, 2_000.0, (300.0, 50.0), bbox), "近处落笔不因空间分段");
        assert!(should_split(0, 2_000.0, (550.0, 50.0), bbox), "移到远处 = 新的一条");

        // 大段(对角线 2154 → 限额 4308): 半屏外不分, 更远才分
        let big = Some(((0.0, 0.0), (2000.0, 800.0)));
        assert!(!should_split(0, 2_000.0, (5000.0, 400.0), big));
        assert!(should_split(0, 2_000.0, (7000.0, 400.0), big));

        // 段空时无空间信号, 只看时间
        assert!(!should_split(0, 2_000.0, (99999.0, 99999.0), None));
    }

    /// 节奏学习: 段内间隔把 EMA 拉向用户节奏(α=0.3)。
    #[test]
    fn typical_gap_learns() {
        let mut ema = INITIAL_TYPICAL_MS;
        for gap in [700u64; 5] {
            ema = next_typical(gap, ema);
        }
        assert!(ema < 1_200.0, "快手连续 0.7s 间隔应把 EMA 拉低, 实得 {ema}");
        // 单个长间隔(平滑): 只推高一截, 不会带飞
        ema = next_typical(6_000, ema);
        assert!(ema < 2_500.0, "单个长间隔不应带飞 EMA, 实得 {ema}");
    }
}
