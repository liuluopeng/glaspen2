//! 气泡消息: 把一段时间内的连续笔迹聚合成一条"消息", 供面板的
//! 「气泡消息」tab 以聊天窗样式回顾(右侧 = 用户口吻)。
//!
//! 分段规则(消息边界, 以"冷启动"为主):
//! - **冷启动**: 笔离开感应区后下一次落笔必开新消息(放笔/离开 = macOS
//!   proximity exit; 无 proximity 平板用真鼠标移动回退)。悬停在空中构思
//!   (笔还热)再久也不拆 —— 事件层经 on_pen_activity/_pen_back/_pen_gone
//!   喂笔状态(FFI: glaspen2_bubble_pen_*);
//! - **笔静默超时**: 距最近一次笔事件 >= 节奏阈值(EMA 自适应
//!   clamp(5×EMA, 8s, 90s))时兜底分段 —— 悬停完全不动(无事件)与"平台
//!   无显式离开信号"只能靠它区分; 纯鼠标用户(从无笔信号)沿用旧的
//!   笔迹间隔规则;
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
/// 笔↔橡皮切换宽限(ms): proximity exit+enter 在此之内 = 切换笔尖, 不算离开。
const PEN_SWITCH_GRACE_MS: u64 = 300;

#[derive(Clone)]
pub(crate) struct BubbleStroke {
    /// 0xRRGGBB。
    pub(crate) rgb: u32,
    /// 宽度倍率(面板粗细预设, proto width_scale 同义)。
    pub(crate) width_scale: f64,
    /// 折线 (x, y, w) —— 画布坐标, w = 压感换算后的绝对线宽。
    pub(crate) pts: Vec<(f64, f64, f64)>,
    /// 与上一笔提交的间隔(ms, 段内首笔 = 0)—— 重播时间线的笔间停顿。
    pub(crate) gap_ms: u64,
}

#[derive(Clone)]
pub(crate) struct BubbleMsg {
    pub(crate) id: u64,
    /// 完成时刻(unix 毫秒)。
    pub(crate) ts_ms: u64,
    pub(crate) strokes: Vec<BubbleStroke>,
}

struct Inner {
    /// 已捕获的 STROKES 前缀长度; STROKES 变短 = 被重置(翻页/清空)。
    captured: usize,
    pending: Vec<BubbleStroke>,
    pending_points: usize,
    last_activity: Option<Instant>,
    /// 段内相邻笔迹间隔的 EMA(ms) —— 用户书写节奏。
    typical_gap_ms: f64,
    /// 最近一次笔事件(悬空移动/落笔/抬笔); None = 本会话无笔信号(纯鼠标)。
    pen_last_seen: Option<Instant>,
    /// 笔已离开感应区(macOS proximity exit / 无 proximity 平板的鼠标回退)。
    /// 下一次落笔提交时消费(清掉并分段); 悬停不清它 —— 冷标记必须活到
    /// 下一次真正的落笔才生效, "笔回到感应区"只由 on_pen_back 依切换宽限撤销。
    pen_cold: bool,
    /// pen_cold 置位时刻 —— 识别笔↔橡皮切换(exit+enter 几乎同时)。
    pen_cold_at: Option<Instant>,
    /// 段内上一笔的提交时刻(记每笔间隔用); None = 新段的第一笔。
    last_capture: Option<Instant>,
    msgs: Vec<BubbleMsg>,
    next_id: u64,
}

static INNER: Mutex<Inner> = Mutex::new(Inner {
    captured: 0,
    pending: Vec::new(),
    pending_points: 0,
    last_activity: None,
    typical_gap_ms: INITIAL_TYPICAL_MS,
    pen_last_seen: None,
    pen_cold: false,
    pen_cold_at: None,
    last_capture: None,
    msgs: Vec::new(),
    next_id: 1,
});

/// 晾衣架钩子(shell 注入, REFRESH_HOOK 同款): 消息完结时通知 shell 挂新卡。
/// 调用时持有 INNER 锁 —— 实现必须异步回主线程再取数据, 严禁同步回调进 core。
static LINE_HOOK: Mutex<Option<fn()>> = Mutex::new(None);

/// shell(macOS)启动时注入晾衣架刷新回调;未注入(测试/Windows)为无操作。
pub fn set_bubble_line_hook(f: fn()) {
    *LINE_HOOK.lock().unwrap_or_else(|e| e.into_inner()) = Some(f);
}

/// 晾衣架挂卡列表(最近 `max` 条已完结消息, 旧→新):
/// `{"cards":[{"id":7,"ts_ms":...}, ...]}`。空笔消息跳过(渲染必为 None)。
pub fn line_cards_json(max: i32) -> String {
    let g = INNER.lock().unwrap_or_else(|e| e.into_inner());
    let max = max.clamp(1, 64) as usize;
    // 先从新往旧取 max 条, 再反回旧→新(晾衣架从左往右按时间挂)
    let mut picked: Vec<&BubbleMsg> = g
        .msgs
        .iter()
        .rev()
        .filter(|m| m.strokes.iter().any(|s| !s.pts.is_empty()))
        .take(max)
        .collect();
    picked.reverse();
    let cards: Vec<serde_json::Value> = picked
        .iter()
        .map(|m| json!({ "id": m.id, "ts_ms": m.ts_ms }))
        .collect();
    json!({ "cards": cards }).to_string()
}

/// 单条消息的卡片 PNG(按 id;ObjC 用 glaspen2_free_rust_bytes 释放)。
/// 找不到 / 消息为空 / out_len 为空 → NULL。
pub fn line_card_png(id: u64, max_edge: i32, out_len: *mut std::os::raw::c_int) -> *mut u8 {
    if out_len.is_null() {
        return std::ptr::null_mut();
    }
    let g = INNER.lock().unwrap_or_else(|e| e.into_inner());
    let Some(msg) = g.msgs.iter().find(|m| m.id == id) else {
        return std::ptr::null_mut();
    };
    let Some(png) = super::line::render_message_png(msg, max_edge.max(64) as f64) else {
        return std::ptr::null_mut();
    };
    drop(g);
    super::thumbs::leak_png(png.0, out_len)
}

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
/// 1. **冷启动**: pen_cold(笔离开过感应区) → 必分, 新消息从"笔回来"开始;
/// 2. **笔静默超时**: 距最近一次笔事件(悬空移动/落笔/抬笔) >= 节奏阈值 → 分。
///    悬空移动持续刷新心跳, 所以"悬停构思"再久也不断; 无笔信号(纯鼠标,
///    activity_gap_ms = None)回退旧的笔迹间隔规则;
/// 3. 空间跳变: 新笔起点距当前段包围盒间隙 > max(2×对角线, 400px) → 分。
/// bbox = None(段空)时只看时间。
fn should_split(
    pen_cold: bool,
    activity_gap_ms: Option<u64>,
    stroke_gap_ms: u64,
    typical_gap_ms: f64,
    start: (f64, f64),
    bbox: Option<((f64, f64), (f64, f64))>,
) -> bool {
    if pen_cold {
        return true;
    }
    let gap_ms = activity_gap_ms.unwrap_or(stroke_gap_ms);
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
    // 冷启动标记在此消费(只认真落笔): Windows 悬空报告也会扇出空提交
    // (STROKES 未增长提前返回), 不能在那儿把标记白吃掉。
    let pen_cold = std::mem::take(&mut g.pen_cold);
    let activity_gap_ms = g.pen_last_seen.map(|t| t.elapsed().as_millis() as u64);
    let new_start = strokes[g.captured]
        .points
        .first()
        .map(|&(x, y, _, _)| (x, y))
        .unwrap_or((0.0, 0.0));
    let bbox = pending_bbox(g.pending.as_slice());
    if !g.pending.is_empty()
        && should_split(
            pen_cold,
            activity_gap_ms,
            gap_ms,
            g.typical_gap_ms,
            new_start,
            bbox,
        )
    {
        finish_pending(&mut g);
    } else if !g.pending.is_empty() && (gap_ms as f64) < threshold_ms(g.typical_gap_ms) {
        // 段内间隔: 喂节奏 EMA(未触发分段且间隔没超阈值才配叫"节奏";
        // 悬停桥接的长间隔喂进去会把阈值带飞, 自我放大)
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
        // 与上一笔提交的间隔(段内首笔 = 0): 重播时间线的笔间停顿。
        let gap_ms = g
            .last_capture
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0);
        g.last_capture = Some(Instant::now());
        g.pending_points += pts.len();
        g.pending.push(BubbleStroke {
            rgb,
            width_scale: 1.0,
            pts,
            gap_ms,
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
    g.last_capture = None; // 新段第一笔间隔记 0
    // 晾衣架钩子: 通知 shell 挂新卡(此刻持 INNER 锁, 实现必须异步回主线程
    // 再取数据, 严禁同步回调进 core)
    if let Some(f) = LINE_HOOK.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        f();
    }
    g.msgs.push(msg);
    if g.msgs.len() > MAX_MSGS {
        g.msgs.remove(0);
    }
}

/// 惰性过期: 取数据前调用。冷启动(笔离开)或笔静默超节奏阈值的未完结段
/// 结为消息; 笔还热(悬停构思中)时保持 pending = "未发送"语义, 打开
/// 气泡 tab 的 flush_now 会立即结。
fn flush_expired(g: &mut Inner) {
    if g.pending.is_empty() {
        return;
    }
    if g.pen_cold {
        finish_pending(g);
        return;
    }
    let stale = match g.pen_last_seen {
        Some(t) => t.elapsed().as_millis() as f64 >= threshold_ms(g.typical_gap_ms),
        None => g
            .last_activity
            .is_some_and(|t| t.elapsed().as_millis() as f64 >= threshold_ms(g.typical_gap_ms)),
    };
    if stale {
        finish_pending(g);
    }
}

/// 强制结段(打开气泡 tab 时调用): 进行中的段立即成为一条消息。
pub fn flush_now() {
    let mut g = INNER.lock().unwrap_or_else(|e| e.into_inner());
    finish_pending(&mut g);
}

/// 笔活动心跳(事件层喂入: 悬空移动/落笔/拖动/抬笔, macOS 与 Windows 同
/// 语义)。悬停构思期间持续刷新, "笔静默超时"永远追不上, 消息不被切断。
pub fn on_pen_activity() {
    let mut g = INNER.lock().unwrap_or_else(|e| e.into_inner());
    g.pen_last_seen = Some(Instant::now());
}

/// 笔回到感应区(macOS proximity enter)。距离开不足 PEN_SWITCH_GRACE_MS =
/// 笔↔橡皮切换, 撤销冷标记(擦改不该把消息拦腰拆开); 真离开后再回(间隔
/// 远大于宽限)不撤销, 冷启动照常在下一次落笔生效。
pub fn on_pen_back() {
    let mut g = INNER.lock().unwrap_or_else(|e| e.into_inner());
    if g.pen_cold
        && g.pen_cold_at
            .is_some_and(|t| t.elapsed().as_millis() < PEN_SWITCH_GRACE_MS as u128)
    {
        g.pen_cold = false;
    }
    g.pen_last_seen = Some(Instant::now());
}

/// 笔离开感应区(macOS proximity exit / 无 proximity 平板上的真鼠标移动
/// 回退)。只置冷标记不立即结段: 结段交给下一次落笔提交(提交时消费)或
/// 惰性过期, 避免误拆。
pub fn on_pen_gone() {
    let mut g = INNER.lock().unwrap_or_else(|e| e.into_inner());
    g.pen_cold = true;
    g.pen_cold_at = Some(Instant::now());
}

/// 落笔前判定: 这一笔会不会开启新消息?(分段规则的时间类子集 —— 空间
/// 跳变无法在得知落点前判定, 排除; 空间跳变只切消息不翻页。)
/// 冷启动(笔离开过感应区)或笔静默超节奏阈值; 纯鼠标用户回退笔迹间隔。
fn will_split_soon(g: &Inner, stroke_gap_ms: u64) -> bool {
    if g.pen_cold {
        return true;
    }
    let activity_gap_ms = g.pen_last_seen.map(|t| t.elapsed().as_millis() as u64);
    match activity_gap_ms {
        Some(ms) => ms as f64 >= threshold_ms(g.typical_gap_ms),
        None => stroke_gap_ms as f64 >= threshold_ms(g.typical_gap_ms),
    }
}

/// 「写完自动新建一页」(设置 bubbleAutoNewPage × 气泡 tab 开启, 仅翻页
/// 模式): 落笔瞬间、笔画尚未开始时由 shell 调用。若这一笔将开新消息
/// (冷启动/静默超时), 先建新页并切换 current id, shell 随后翻页载入,
/// 笔画即落在新页上 —— 消息边界 = 页边界。
///
/// 返回新页 id(>0, shell 据此翻页);0 = 无动作。绕开 WRITE_LOCK(无墨迹
/// 在飞, 见 create_next_page_inner); 保留 2s 建页频率闸, 末页空白复用
/// 场景返回 0(与手动路径"已是空白末页"同语义, 不做中途跳页)。
pub fn auto_page_if_due() -> i64 {
    // 设置门槛: 开关 + 气泡 tab 显示, 均落笔低频读, 不缓存
    let enabled = runtime_block_on_setting("bubble_auto_new_page").as_deref() == Some("1")
        && runtime_block_on_setting("show_bubble_tab").as_deref() == Some("1");
    if !enabled {
        return 0;
    }
    if crate::state::canvas_kind() != crate::state::CanvasKind::Page {
        return 0; // 无限画布没有"页"
    }
    {
        // 落笔瞬间无提交扇出在跑, 两把锁都空闲; 借出锁再建页
        let strokes = crate::STROKES.lock().unwrap_or_else(|e| e.into_inner());
        let g = INNER.lock().unwrap_or_else(|e| e.into_inner());
        if strokes.len() < g.captured {
            return 0; // 手动翻页/清空已发生, 待提交钩子自行重同步
        }
        if g.pending.is_empty() {
            return 0;
        }
        let stroke_gap_ms = g
            .last_activity
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0);
        if !will_split_soon(&g, stroke_gap_ms) {
            return 0;
        }
    }
    let current = crate::state::current_screen_id();
    let new_id = super::pages::create_next_page_inner(current, false);
    // 复用(末页空白)返回 current_id: 对自动翻页而言 = 不动, 返回 0
    if new_id > current {
        tracing::info!("气泡自动新建一页: {current} → {new_id}");
        new_id
    } else {
        0
    }
}

/// db::load_setting 的同步包装(auto_page 落笔路径用)。
fn runtime_block_on_setting(key: &str) -> Option<String> {
    crate::runtime().block_on(crate::db::load_setting(key))
}

/// 全部消息的 JSON(先惰性过期)。形如:
/// `{"device":"0192…","messages":[{"id":1,"ts_ms":...,"device":"0192…",
///   "strokes":[{"rgb":16711680,"w":2.0,"g":0,"pts":[x,y,w, ...]}]}]}`
/// (g = 与上一笔的间隔 ms, 重播时间线用;device = 作者设备 UUIDv7,
/// 当前全为本机, 未来他人消息按它分左右栏)
pub fn messages_json() -> String {
    let mut g = INNER.lock().unwrap_or_else(|e| e.into_inner());
    flush_expired(&mut g);
    let device = crate::device::device_uuid();
    let msgs: Vec<serde_json::Value> = g
        .msgs
        .iter()
        .map(|m| {
            json!({
                "id": m.id,
                "ts_ms": m.ts_ms,
                "device": device,
                "strokes": m.strokes.iter().map(|s| json!({
                    "rgb": s.rgb,
                    "w": s.width_scale,
                    "g": s.gap_ms,
                    "pts": s.pts.iter().flat_map(|&(x, y, w)| [x, y, w]).collect::<Vec<f64>>(),
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({ "device": device, "messages": msgs }).to_string()
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
        g.pen_last_seen = None;
        g.pen_cold = false;
        g.pen_cold_at = None;
        g.last_capture = None;
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
        // g = 与上一笔的间隔(ms): 段内首笔为 0, 重播时间线用
        assert_eq!(msgs[0]["strokes"][0]["g"], 0, "段内首笔间隔为 0");
        // device = 作者设备 UUIDv7(根级 + 消息级, 未来他人气泡分左右栏)
        let dev = v["device"].as_str().expect("根级 device");
        assert_eq!(dev.len(), 36, "UUID 格式: {dev}");
        assert_eq!(dev.as_bytes()[14], b'7', "v7 版本位: {dev}");
        assert_eq!(msgs[0]["device"].as_str(), Some(dev), "消息级 = 根级");
    }

    /// 设备 UUID: 同进程稳定(OnceLock 缓存; 持久化路径由 device.rs 生成时
    /// save_setting 保证 —— 测试间 DB 会被重建, 不在此断言 DB 内容)。
    #[test]
    fn device_uuid_stable() {
        let a = crate::device::device_uuid();
        let b = crate::device::device_uuid();
        assert_eq!(a, b);
        assert_eq!(a.len(), 36);
        assert_eq!(a.as_bytes()[14], b'7');
    }

    /// 晾衣架挂卡列表: 最近 N 条、旧→新、空笔消息跳过;卡片 PNG 按 id 取。
    #[test]
    fn line_cards_and_png() {
        let _g = crate::tests::TEST_LOCK.lock().unwrap();
        reset_fixture();
        // 消息 1: 两笔(可渲染)
        {
            let mut strokes = crate::STROKES.lock().unwrap();
            strokes.push(crate::Stroke {
                id: 1,
                r: 1.0,
                g: 0.0,
                b: 0.0,
                points: vec![(0.0, 0.0, 2.0, 0.0), (30.0, 20.0, 2.0, 0.1)],
            });
        }
        on_stroke_committed();
        // 冷标记强制分段: 笔 2 开新消息(否则两笔并作一条)
        INNER.lock().unwrap().pen_cold = true;
        // 消息 2: 一笔(可渲染)
        {
            let mut strokes = crate::STROKES.lock().unwrap();
            strokes.push(crate::Stroke {
                id: 2,
                r: 0.0,
                g: 0.0,
                b: 1.0,
                points: vec![(5.0, 5.0, 2.0, 0.0)],
            });
        }
        on_stroke_committed();
        crate::STROKES.lock().unwrap().clear();
        on_stroke_committed(); // 重置结出两条消息
        {
            let v: serde_json::Value =
                serde_json::from_str(&line_cards_json(8)).unwrap();
            let cards = v["cards"].as_array().unwrap();
            assert_eq!(cards.len(), 2, "两条消息两张卡");
            assert_eq!(cards[0]["id"], 1, "旧→新");
            assert_eq!(cards[1]["id"], 2);
        }
        // 卡片 PNG: 有效 id → PNG 魔数;未知 id → NULL
        let mut len: std::os::raw::c_int = 0;
        let png = line_card_png(1, 320, &mut len);
        assert!(!png.is_null() && len > 4);
        let magic = unsafe { std::slice::from_raw_parts(png, 4) };
        assert_eq!(magic, &[0x89, b'P', b'N', b'G']);
        unsafe { crate::export::thumbs::glaspen2_free_rust_bytes(png, len) };
        assert!(line_card_png(999, 320, &mut len).is_null(), "未知 id → NULL");
        assert!(line_card_png(1, 320, std::ptr::null_mut()).is_null(), "out_len NULL 防御");
        reset_fixture();
    }

    /// 时间规则(无笔信号 = 纯鼠标回退路径): 阈值 = clamp(5×EMA, 8s, 90s)。
    #[test]
    fn split_time_adaptive() {
        // 快手: EMA=2s → 阈值 10s; 9.5s 不分, 10.5s 分
        assert_eq!(threshold_ms(2_000.0), 10_000.0);
        assert!(!should_split(false, None, 9_500, 2_000.0, (0.0, 0.0), None));
        assert!(should_split(false, None, 10_500, 2_000.0, (0.0, 0.0), None));

        // 慢思考: EMA=25s → 阈值 90s(顶到上限); 50s 不分
        assert_eq!(threshold_ms(25_000.0), THRESHOLD_MAX_MS);
        assert!(!should_split(false, None, 50_000, 25_000.0, (0.0, 0.0), None));

        // 快到极致: EMA=0.1s → 阈值 8s(下限兜底)
        assert_eq!(threshold_ms(100.0), THRESHOLD_MIN_MS);
        assert!(!should_split(false, None, 7_900, 100.0, (0.0, 0.0), None));
        assert!(should_split(false, None, 8_100, 100.0, (0.0, 0.0), None));
    }

    /// 空间规则: 间隙 > max(2×对角线, 400px) 即分, 与时间无关。
    #[test]
    fn split_space_jump() {
        let bbox = Some(((0.0, 0.0), (100.0, 100.0))); // 对角线 ≈141 → 限额 400px
        assert!(
            !should_split(false, None, 0, 2_000.0, (300.0, 50.0), bbox),
            "近处落笔不因空间分段"
        );
        assert!(
            should_split(false, None, 0, 2_000.0, (550.0, 50.0), bbox),
            "移到远处 = 新的一条"
        );

        // 大段(对角线 2154 → 限额 4308): 半屏外不分, 更远才分
        let big = Some(((0.0, 0.0), (2000.0, 800.0)));
        assert!(!should_split(false, None, 0, 2_000.0, (5000.0, 400.0), big));
        assert!(should_split(false, None, 0, 2_000.0, (7000.0, 400.0), big));

        // 段空时无空间信号, 只看时间
        assert!(!should_split(false, None, 0, 2_000.0, (99999.0, 99999.0), None));
    }

    /// 冷启动规则: 悬停桥接(心跳新鲜)再长的笔迹间隔也不拆;
    /// 笔离开感应区(pen_cold)后下一次提交必分段。
    #[test]
    fn split_cold_start() {
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
        }
        on_stroke_committed();
        assert_eq!(INNER.lock().unwrap().pending.len(), 1);

        // 悬停构思 60s: 笔活动心跳持续刷新(刚刚还在悬空移动),
        // 笔迹间隔虽远超阈值, 消息不拆
        {
            let mut g = INNER.lock().unwrap();
            g.last_activity = Some(Instant::now() - std::time::Duration::from_secs(60));
            g.pen_last_seen = Some(Instant::now());
        }
        {
            let mut strokes = crate::STROKES.lock().unwrap();
            strokes.push(crate::Stroke {
                id: 2,
                r: 1.0,
                g: 0.0,
                b: 0.0,
                points: vec![(15.0, 15.0, 2.0, 0.0)],
            });
        }
        on_stroke_committed();
        assert_eq!(
            INNER.lock().unwrap().pending.len(),
            2,
            "悬停构思不拆: 两笔仍同段"
        );

        // 笔离开感应区(proximity exit → 冷标记) → 下一次落笔必分段
        on_pen_gone();
        {
            let mut strokes = crate::STROKES.lock().unwrap();
            strokes.push(crate::Stroke {
                id: 3,
                r: 1.0,
                g: 0.0,
                b: 0.0,
                points: vec![(20.0, 20.0, 2.0, 0.0)],
            });
        }
        on_stroke_committed();
        let g = INNER.lock().unwrap();
        assert_eq!(g.msgs.len(), 1, "冷启动结出第一条(2 笔)");
        assert_eq!(g.msgs[0].strokes.len(), 2);
        assert_eq!(g.pending.len(), 1, "笔回来后的笔开新段");
        assert!(!g.pen_cold, "冷标记应在提交时消费");
    }

    /// 笔静默超时(悬停完全不动/平台无显式离开信号)按心跳超时分段;
    /// 纯鼠标(无笔信号)回退旧的笔迹间隔规则; 笔↔橡皮切换宽限撤销冷标记。
    #[test]
    fn pen_silent_and_mouse_fallback() {
        let _g = crate::tests::TEST_LOCK.lock().unwrap();
        // 纯函数层:
        // 心跳 0.5s 前 + 笔迹间隔 60s → 悬停桥接, 不分
        assert!(!should_split(false, Some(500), 60_000, 2_000.0, (0.0, 0.0), None));
        // 心跳 60s 前(悬停完全不动) → 静默超阈值(10s) → 分
        assert!(should_split(false, Some(60_000), 500, 2_000.0, (0.0, 0.0), None));
        // 无笔信号 → 回退笔迹间隔规则(与 split_time_adaptive 同判)
        assert!(!should_split(false, None, 7_000, 2_000.0, (0.0, 0.0), None));
        assert!(should_split(false, None, 10_500, 2_000.0, (0.0, 0.0), None));

        // pen_back 宽限: 刚离开就回 = 切橡皮, 撤销冷标记
        reset_fixture();
        on_pen_gone();
        on_pen_back();
        assert!(
            !INNER.lock().unwrap().pen_cold,
            "切换宽限内笔回 = 撤销冷标记"
        );
        // 真离开后再回(超宽限, 直接回拨置位时刻模拟等待) → 冷标记保留
        on_pen_gone();
        INNER.lock().unwrap().pen_cold_at =
            Some(Instant::now() - std::time::Duration::from_millis(400));
        on_pen_back();
        assert!(INNER.lock().unwrap().pen_cold, "超宽限笔回不撤销冷标记");
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

    /// 自动翻页判定: 冷启动/笔静默超阈值/纯鼠标回退笔迹间隔。
    #[test]
    fn will_split_soon_rules() {
        let base = || Inner {
            captured: 0,
            pending: vec![BubbleStroke {
                rgb: 0xFF0000,
                width_scale: 1.0,
                pts: vec![(0.0, 0.0, 2.0), (5.0, 5.0, 2.0)],
                gap_ms: 0,
            }],
            pending_points: 2,
            last_activity: Some(Instant::now()),
            typical_gap_ms: INITIAL_TYPICAL_MS, // 阈值 10s
            pen_last_seen: None,
            pen_cold: false,
            pen_cold_at: None,
            last_capture: None,
            msgs: Vec::new(),
            next_id: 1,
        };
        // 笔冷 = 必分
        let mut g = base();
        g.pen_cold = true;
        assert!(will_split_soon(&g, 0));
        // 心跳新鲜(0.5s 前还在悬停) = 不分(悬停构思桥接)
        let mut g = base();
        g.pen_last_seen = Some(Instant::now() - std::time::Duration::from_millis(500));
        assert!(!will_split_soon(&g, 60_000));
        // 心跳超阈值(11s 没有任何笔事件) = 分
        let mut g = base();
        g.pen_last_seen = Some(Instant::now() - std::time::Duration::from_secs(11));
        assert!(will_split_soon(&g, 0));
        // 纯鼠标: 回退笔迹间隔(10s 阈值)
        let g = base();
        assert!(!will_split_soon(&g, 9_000));
        assert!(will_split_soon(&g, 10_500));
    }
}
