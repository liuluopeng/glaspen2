//! 气泡消息: 把一段时间内的连续笔迹聚合成一条"消息", 供面板的
//! 「气泡消息」tab 以聊天窗样式回顾(右侧 = 用户口吻)。
//!
//! 分段规则(消息边界, 以"冷启动"为主):
//! - **冷启动**: 笔离开感应区且**持续 >= COLD_START_MIN_MS(3s)** 后, 下一次
//!   落笔开新消息(放笔/离开 = macOS proximity exit; 无 proximity 平板用真
//!   鼠标移动回退)。抬笔换气、字与字之间、笔↔橡皮切换、proximity 抖动都在
//!   秒级以内, 不拆 —— 旧实现一离开就置冷, 把 hello 拆成 h/e/l/l/o。
//!   笔回到感应区时按离开时长结算: 短离开撤销, 长离开置 pen_cold, 在下一次
//!   落笔提交时消费; 面板刷新(flush_expired)也只认"已离开 3s", 不会在
//!   字间抬笔时提前结段;
//! - **静默超时**: 距最近一次笔事件 >= 节奏阈值(EMA 自适应
//!   clamp(5×EMA, 60s, 90s))时兜底分段 —— 一分钟之内换区域写/短思考都
//!   算同一条(空间跳变规则已废: dev DB 实证它把连续书写拦腰拆开,
//!   边界只认时间与冷启动, 不认位置); 悬停完全不动(无事件)与"平台
//!   无显式离开信号"只能靠它区分; 纯鼠标用户(从无笔信号)沿用旧的
//!   笔迹间隔规则;
//! - **上下文切换**: 翻页/清空/载入(检测到 STROKES 被重置)时, 未完成的段
//!   先结为一条消息;
//! - **容量护栏**: 单段 > MAX_PENDING_STROKES 笔或 > MAX_PENDING_POINTS
//!   点时截断成下一条(防一次超长涂鸦撑爆内存)。
//!
//! 数据结构对齐手写消息的 ShareStroke 语义(color/width_scale/points),
//! 以 JSON 出口交给面板, Flutter CustomPainter 负责把折线画进气泡。
//! 已完结的消息落 SQLite 表 `bubble_messages`(整条 strokes JSON 一行),
//! 启动时恢复最近 MAX_MSGS 条; 进程退出时把未完结段 flush 落库。

use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// 分段阈值下限: 一分钟之内的连续书写(换区域/短思考)都是同一条
/// (2026-11-08 dev DB 实证: 105s 内 15 笔最大间隔 4.2s 被拆成 5 条,
/// 元凶是 8s 下限 + 空间跳变 —— 两者均已废)。
const THRESHOLD_MIN_MS: f64 = 60_000.0;
/// 上限: 边想边写的人也不会超过 90 秒。
const THRESHOLD_MAX_MS: f64 = 90_000.0;
/// 阈值 = EMA × 此系数。
const THRESHOLD_FACTOR: f64 = 5.0;
/// EMA 平滑系数。
const EMA_ALPHA: f64 = 0.3;
/// EMA 初值(2s ≈ 连贯书写的典型间隔)。
const INITIAL_TYPICAL_MS: f64 = 2_000.0;
/// 内存/面板保留的消息条数上限(最旧的不在面板显示; DB 里仍保留历史)。
const MAX_MSGS: usize = 50;
/// 单段笔数上限。
const MAX_PENDING_STROKES: usize = 60;
/// 单段点数上限。
const MAX_PENDING_POINTS: usize = 8000;
/// 冷启动最小离开时长: 笔离开感应区 >= 3s 才算"放下笔", 下一次落笔开新
/// 消息。手写时抬笔/字间/换笔尖的 proximity 中断都在 1-2s 内; 3s 是
/// "停笔思考"的保守分界。短于它的离开一律并入当前消息(面板刷新也不会
/// 提前结段)。
const COLD_START_MIN_MS: u64 = 3_000;

#[derive(Clone)]
struct BubbleStroke {
    /// 0xRRGGBB。
    rgb: u32,
    /// 宽度倍率(面板粗细预设, proto width_scale 同义)。
    width_scale: f64,
    /// 折线 (x, y, w) —— 画布坐标, w = 压感换算后的绝对线宽。
    pts: Vec<(f64, f64, f64)>,
    /// 与上一笔提交的间隔(ms, 段内首笔 = 0)—— 重播时间线的笔间停顿。
    gap_ms: u64,
}

struct BubbleMsg {
    /// DB 行 id(写入失败时退回内存自增 id)。
    id: u64,
    /// 完成时刻(unix 毫秒)。
    ts_ms: u64,
    /// 作者设备 UUIDv7(未来他人消息按它分左右栏)。
    device: String,
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
    /// 最近一次笔事件(悬空移动/落笔/抬笔); None = 本会话无笔信号(纯鼠标)。
    pen_last_seen: Option<Instant>,
    /// 笔离开感应区的时刻(macOS proximity exit / 无 proximity 平板的鼠标
    /// 回退); None = 笔在感应区或未知。离开满 COLD_START_MIN_MS 才升级为
    /// pen_cold, 短离开在笔回来时直接丢弃 —— 字与字之间的抬笔不拆消息。
    pen_away_since: Option<Instant>,
    /// 离开已满冷启动阈值, 下一次真正落笔提交时开新消息(提交时消费)。
    pen_cold: bool,
    /// 段内上一笔的提交时刻(记每笔间隔用); None = 新段的第一笔。
    last_capture: Option<Instant>,
    msgs: Vec<BubbleMsg>,
    next_id: u64,
    /// 是否已从 DB 恢复过历史(load_from_db 幂等)。
    loaded: bool,
}

static INNER: Mutex<Inner> = Mutex::new(Inner {
    captured: 0,
    pending: Vec::new(),
    pending_points: 0,
    last_activity: None,
    typical_gap_ms: INITIAL_TYPICAL_MS,
    pen_last_seen: None,
    pen_away_since: None,
    pen_cold: false,
    last_capture: None,
    msgs: Vec::new(),
    next_id: 1,
    loaded: false,
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

/// 分段判定(纯函数, 可测):
/// 1. **冷启动**: pen_cold(笔离开过感应区) → 必分, 新消息从"笔回来"开始;
/// 2. **笔静默超时**: 距最近一次笔事件(悬空移动/落笔/抬笔) >= 节奏阈值 → 分。
///    悬空移动持续刷新心跳, 所以"悬停构思"再久也不断; 无笔信号(纯鼠标,
///    activity_gap_ms = None)回退旧的笔迹间隔规则。
/// (空间跳变规则已废 —— 位置不构成消息边界, 见 THRESHOLD_MIN_MS 注释。)
fn should_split(
    pen_cold: bool,
    activity_gap_ms: Option<u64>,
    stroke_gap_ms: u64,
    typical_gap_ms: f64,
) -> bool {
    if pen_cold {
        return true;
    }
    let gap_ms = activity_gap_ms.unwrap_or(stroke_gap_ms);
    gap_ms as f64 >= threshold_ms(typical_gap_ms)
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
    // 平台没发回 enter(或事件层没补 pen_back)时, 用离开时长兜底结算:
    // 离开满阈值 = 这一笔属于新消息; 短离开保留 pen_away_since 继续计时
    // (可能刚抬笔, 面板刷新要能等到满 3s 再结段)。
    if let Some(since) = g.pen_away_since
        && since.elapsed().as_millis() as u64 >= COLD_START_MIN_MS
    {
        g.pen_cold = true;
        g.pen_away_since = None;
    }
    // 冷启动标记在此消费(只认真落笔): Windows 悬空报告也会扇出空提交
    // (STROKES 未增长提前返回), 不能在那儿把标记白吃掉。
    let pen_cold = std::mem::take(&mut g.pen_cold);
    let activity_gap_ms = g.pen_last_seen.map(|t| t.elapsed().as_millis() as u64);
    if !g.pending.is_empty() && should_split(pen_cold, activity_gap_ms, gap_ms, g.typical_gap_ms) {
        tracing::debug!(
            "气泡分段: cold={pen_cold} activity_gap={activity_gap_ms:?}ms stroke_gap={gap_ms}ms pending={}",
            g.pending.len()
        );
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

/// 一笔气泡笔迹 → 面板/DB 共用的 JSON 形状(rgb/w/g/pts 平铺)。
fn stroke_value(s: &BubbleStroke) -> Value {
    json!({
        "rgb": s.rgb,
        "w": s.width_scale,
        "g": s.gap_ms,
        "pts": s.pts.iter().flat_map(|&(x, y, w)| [x, y, w]).collect::<Vec<f64>>(),
    })
}

/// 一条消息的 strokes 数组 → 紧凑 JSON(DB 的 strokes_json 列)。
fn encode_strokes(strokes: &[BubbleStroke]) -> String {
    let arr: Vec<Value> = strokes.iter().map(stroke_value).collect();
    serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string())
}

/// 反向解析; 损坏/不完整的数据跳过(不阻断其余历史消息)。
fn decode_strokes(payload: &str) -> Vec<BubbleStroke> {
    let Ok(v) = serde_json::from_str::<Value>(payload) else {
        return Vec::new();
    };
    let Some(arr) = v.as_array() else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|s| {
            let rgb = s.get("rgb")?.as_u64()? as u32;
            let width_scale = s.get("w").and_then(Value::as_f64).unwrap_or(1.0);
            let gap_ms = s.get("g").and_then(Value::as_u64).unwrap_or(0);
            let pts_v = s.get("pts")?.as_array()?;
            let mut pts = Vec::with_capacity(pts_v.len() / 3);
            for chunk in pts_v.chunks_exact(3) {
                pts.push((chunk[0].as_f64()?, chunk[1].as_f64()?, chunk[2].as_f64()?));
            }
            if pts.is_empty() {
                None
            } else {
                Some(BubbleStroke {
                    rgb,
                    width_scale,
                    pts,
                    gap_ms,
                })
            }
        })
        .collect()
}

/// 把进行中的段结为一条消息, 并落库(bubble_messages)。
///
/// DB 写入失败时仍保留内存副本(仅本会话可见); 返回 DB 行 id 作为稳定消息 id,
/// 失败退回内存自增 id。写入是同步的: 落笔/翻页路径本来就在这个线程上写
/// SQLite, 单行 INSERT 的代价远小于丢消息。
fn finish_pending(g: &mut Inner) {
    if g.pending.is_empty() {
        return;
    }
    let ts_ms = now_ms();
    let device = crate::device::device_uuid();
    let payload = encode_strokes(&g.pending);
    let db_id =
        crate::runtime().block_on(crate::db::insert_bubble_message(&device, ts_ms, &payload));
    let id = match db_id {
        Some(id) if id > 0 => id as u64,
        _ => g.next_id,
    };
    if id >= g.next_id {
        g.next_id = id.saturating_add(1);
    }
    let msg = BubbleMsg {
        id,
        ts_ms,
        device,
        strokes: std::mem::take(&mut g.pending),
    };
    g.pending_points = 0;
    g.last_capture = None; // 新段第一笔间隔记 0
    g.msgs.push(msg);
    if g.msgs.len() > MAX_MSGS {
        g.msgs.remove(0);
    }
}

/// 从 DB 恢复最近 MAX_MSGS 条消息(启动时调用; 幂等)。
pub fn load_from_db() {
    {
        let g = INNER.lock().unwrap_or_else(|e| e.into_inner());
        if g.loaded {
            return;
        }
    }
    let rows = crate::runtime().block_on(crate::db::load_bubble_messages(MAX_MSGS as i64));
    let mut g = INNER.lock().unwrap_or_else(|e| e.into_inner());
    if g.loaded {
        return;
    }
    g.loaded = true;
    // 期间可能已有新消息先落库/进内存; 按 id 合并去重。
    let mut by_id: BTreeMap<u64, BubbleMsg> = std::mem::take(&mut g.msgs)
        .into_iter()
        .map(|m| (m.id, m))
        .collect();
    for row in rows {
        if row.id <= 0 {
            continue;
        }
        let id = row.id as u64;
        g.next_id = g.next_id.max(id.saturating_add(1));
        if by_id.contains_key(&id) {
            continue;
        }
        let strokes = decode_strokes(&row.strokes_json);
        if strokes.is_empty() {
            continue;
        }
        by_id.insert(
            id,
            BubbleMsg {
                id,
                ts_ms: row.ts_ms.max(0) as u64,
                device: row.device,
                strokes,
            },
        );
    }
    let mut msgs: Vec<BubbleMsg> = by_id.into_values().collect();
    if msgs.len() > MAX_MSGS {
        let drop_n = msgs.len() - MAX_MSGS;
        msgs.drain(..drop_n);
    }
    g.msgs = msgs;
}

/// 惰性过期: 取数据前调用。笔已离开满冷启动阈值, 或笔静默超节奏阈值的
/// 未完结段结为消息; 笔还热(悬停构思中)或只是短促抬笔时保持 pending =
/// "未发送"语义, 打开气泡 tab 的 flush_now 会立即结。
fn flush_expired(g: &mut Inner) {
    if g.pending.is_empty() {
        return;
    }
    let away_long = g
        .pen_away_since
        .is_some_and(|t| t.elapsed().as_millis() as u64 >= COLD_START_MIN_MS);
    if g.pen_cold || away_long {
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

/// 笔回到感应区/有活动: 按离开时长结算 pen_away_since。
/// 短离开(< COLD_START_MIN_MS)直接丢弃 —— 字间抬笔、笔↔橡皮切换、
/// proximity 抖动都不拆; 长离开置 pen_cold, 下一笔提交时开新消息。
fn resolve_pen_back(g: &mut Inner) {
    if let Some(since) = g.pen_away_since.take()
        && since.elapsed().as_millis() as u64 >= COLD_START_MIN_MS
    {
        g.pen_cold = true;
    }
}

/// 笔活动心跳(只刷新"最近笔事件"): 抬笔事件也可能跟在 proximity exit 后面
/// 到达, 不能把它当成"笔回来了"; 回来信号由 on_pen_back(proximity enter,
/// 或平台不发 enter 时的悬停/落笔回退)负责。Windows 没有 proximity, 所有
/// 笔事件都走这里, 由静默阈值兜底。
pub fn on_pen_activity() {
    let mut g = INNER.lock().unwrap_or_else(|e| e.into_inner());
    g.pen_last_seen = Some(Instant::now());
}

/// 笔回到感应区(macOS proximity enter; 平台不发 enter 时由悬停/落笔事件
/// 补发)。按离开时长结算: 不足 COLD_START_MIN_MS 撤销, 超阈值置冷; 不再
/// 需要单独的笔↔橡皮切换宽限。
pub fn on_pen_back() {
    let mut g = INNER.lock().unwrap_or_else(|e| e.into_inner());
    resolve_pen_back(&mut g);
    g.pen_last_seen = Some(Instant::now());
}

/// 笔离开感应区(macOS proximity exit / 无 proximity 平板上的真鼠标移动
/// 回退)。**不立即置冷** —— 只记下离开时刻; 是否算冷启动要等笔回来
/// (或面板刷新时)按离开时长结算, 否则字与字之间的抬笔会把 hello 拆成
/// 五个消息。
pub fn on_pen_gone() {
    let mut g = INNER.lock().unwrap_or_else(|e| e.into_inner());
    // 纯鼠标(本会话从无笔信号)没有"笔离开感应区"语义; 鼠标移动回退
    // 不能给纯鼠标用户设冷, 否则每一笔都被拆开。纯鼠标走静默阈值。
    if g.pen_last_seen.is_none() {
        return;
    }
    if g.pen_away_since.is_none() {
        g.pen_away_since = Some(Instant::now());
    }
}

/// 落笔前判定: 这一笔会不会开启新消息?(分段规则的时间类子集 —— 空间
/// 跳变无法在得知落点前判定, 排除; 空间跳变只切消息不翻页。)
/// 冷启动(离开满 COLD_START_MIN_MS)或笔静默超节奏阈值; 纯鼠标用户回退
/// 笔迹间隔。
fn will_split_soon(g: &Inner, stroke_gap_ms: u64) -> bool {
    if g.pen_cold {
        return true;
    }
    if let Some(since) = g.pen_away_since
        && since.elapsed().as_millis() as u64 >= COLD_START_MIN_MS
    {
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
    load_from_db();
    let mut g = INNER.lock().unwrap_or_else(|e| e.into_inner());
    flush_expired(&mut g);
    let device = crate::device::device_uuid();
    let msgs: Vec<Value> = g
        .msgs
        .iter()
        .map(|m| {
            json!({
                "id": m.id,
                "ts_ms": m.ts_ms,
                // 逐条携带作者设备: 历史消息的 device 可能不是当前设备(未来他人消息)
                "device": m.device,
                "strokes": m.strokes.iter().map(stroke_value).collect::<Vec<_>>(),
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
        g.pen_away_since = None;
        g.pen_cold = false;
        g.last_capture = None;
        // 测试默认不读全局 DB 历史(其他测试的 finish_pending 会写库);
        // 持久化专项测试自行置 false 并清表。
        g.loaded = true;
    }

    /// strokes JSON 与 DB 列同构: encode → decode 保真, 坏数据不 panic。
    #[test]
    fn strokes_json_roundtrip() {
        let s = BubbleStroke {
            rgb: 0xFF0038,
            width_scale: 1.5,
            pts: vec![(1.0, 2.0, 3.0), (4.5, 5.5, 6.5)],
            gap_ms: 42,
        };
        let encoded = encode_strokes(std::slice::from_ref(&s));
        let back = decode_strokes(&encoded);
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].rgb, 0xFF0038);
        assert_eq!(back[0].width_scale, 1.5);
        assert_eq!(back[0].gap_ms, 42);
        assert_eq!(back[0].pts, s.pts);
        assert!(decode_strokes("{not json").is_empty());
        assert!(decode_strokes("[]").is_empty());
    }

    /// 端到端: 结段落库 → 清空内存 → load_from_db 恢复(同一条 id/内容)。
    #[test]
    fn bubble_messages_persist_and_reload() {
        let _g = crate::tests::TEST_LOCK.lock().unwrap();
        if !crate::db::is_init() {
            let path =
                std::env::temp_dir().join(format!("glaspen2_bubble_{}.db", std::process::id()));
            let _ = std::fs::remove_file(&path);
            // SAFETY: 测试单线程初始化(TEST_LOCK 串行化)
            unsafe { std::env::set_var("GLASPEN2_DB_PATH", &path) };
            crate::runtime().block_on(crate::db::init());
        }
        crate::runtime().block_on(crate::db::clear_bubble_messages_for_test());
        reset_fixture();
        on_pen_activity();
        commit_stroke(1);
        flush_now();
        let inserted = {
            let g = INNER.lock().unwrap();
            assert_eq!(g.msgs.len(), 1);
            (g.msgs[0].id, g.msgs[0].ts_ms, g.msgs[0].strokes.len())
        };
        // 模拟重启: 清内存, 允许重新从 DB 读
        {
            let mut g = INNER.lock().unwrap();
            g.msgs.clear();
            g.loaded = false;
            g.next_id = 1;
        }
        load_from_db();
        let g = INNER.lock().unwrap();
        assert_eq!(g.msgs.len(), 1, "重启后应恢复最近一条");
        assert_eq!(g.msgs[0].id, inserted.0, "消息 id = DB 行 id");
        assert_eq!(g.msgs[0].ts_ms, inserted.1);
        assert_eq!(g.msgs[0].strokes.len(), inserted.2);
        assert!(g.msgs[0].id > 0);
        assert!(!g.msgs[0].device.is_empty());
    }

    /// 追加一笔并走提交钩子(测试用)。
    fn commit_stroke(id: i64) {
        {
            let mut strokes = crate::STROKES.lock().unwrap();
            strokes.push(crate::Stroke {
                id,
                r: 1.0,
                g: 0.0,
                b: 0.0,
                points: vec![(id as f64, id as f64, 2.0, 0.0)],
            });
        }
        on_stroke_committed();
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
        let v: serde_json::Value = serde_json::from_str(&messages_json()).expect("应为合法 JSON");
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

    /// 时间规则(无笔信号 = 纯鼠标回退路径): 阈值 = clamp(5×EMA, 60s, 90s)。
    #[test]
    fn split_time_adaptive() {
        // 快手: EMA=2s → 阈值顶到下限 60s; 一分钟之内连续 = 同一条
        assert_eq!(threshold_ms(2_000.0), THRESHOLD_MIN_MS);
        assert!(!should_split(false, None, 59_500, 2_000.0));
        assert!(should_split(false, None, 60_500, 2_000.0));

        // 慢思考: EMA=25s → 阈值 90s(顶到上限); 50s 不分
        assert_eq!(threshold_ms(25_000.0), THRESHOLD_MAX_MS);
        assert!(!should_split(false, None, 50_000, 25_000.0));

        // 快到极致: EMA=0.1s → 阈值 60s(下限兜底)
        assert_eq!(threshold_ms(100.0), THRESHOLD_MIN_MS);
        assert!(!should_split(false, None, 59_900, 100.0));
        assert!(should_split(false, None, 60_100, 100.0));
    }

    /// 冷启动规则: 悬停桥接再长的笔迹间隔也不拆; 短促抬笔(proximity exit
    /// 后很快又回来)不拆; 只有离开满 COLD_START_MIN_MS 才在新一笔开消息。
    #[test]
    fn split_cold_start() {
        let _g = crate::tests::TEST_LOCK.lock().unwrap();
        reset_fixture();
        on_pen_activity(); // 本会话有笔信号(不是纯鼠标)

        commit_stroke(1);
        assert_eq!(INNER.lock().unwrap().pending.len(), 1);

        // 悬停构思 60s: 笔活动心跳持续刷新(刚刚还在悬空移动),
        // 笔迹间隔虽远超阈值, 消息不拆
        {
            let mut g = INNER.lock().unwrap();
            g.last_activity = Some(Instant::now() - std::time::Duration::from_secs(60));
            g.pen_last_seen = Some(Instant::now());
        }
        commit_stroke(2);
        assert_eq!(
            INNER.lock().unwrap().pending.len(),
            2,
            "悬停构思不拆: 两笔仍同段"
        );

        // 字间抬笔: proximity exit + 800ms 后 enter(远小于 3s)→ 不拆
        on_pen_gone();
        INNER.lock().unwrap().pen_away_since =
            Some(Instant::now() - std::time::Duration::from_millis(800));
        on_pen_back();
        commit_stroke(3);
        assert_eq!(
            INNER.lock().unwrap().pending.len(),
            3,
            "短促抬笔不拆: hello 的字母不能各成一条"
        );

        // 真离开: 离开满 3s 后笔回来 → 冷标记, 下一笔开新消息
        on_pen_gone();
        INNER.lock().unwrap().pen_away_since =
            Some(Instant::now() - std::time::Duration::from_millis(COLD_START_MIN_MS + 100));
        on_pen_back();
        assert!(INNER.lock().unwrap().pen_cold, "离开满阈值应置冷");
        commit_stroke(4);
        let g = INNER.lock().unwrap();
        assert_eq!(g.msgs.len(), 1, "冷启动结出第一条(3 笔)");
        assert_eq!(g.msgs[0].strokes.len(), 3);
        assert_eq!(g.pending.len(), 1, "笔回来后的笔开新段");
        assert!(!g.pen_cold, "冷标记应在提交时消费");
    }

    /// 笔静默超时(悬停完全不动/平台无显式离开信号)按心跳超时分段;
    /// 纯鼠标(无笔信号)回退旧的笔迹间隔规则; 短离开不置冷、满 3s 才置冷。
    #[test]
    fn pen_silent_and_mouse_fallback() {
        let _g = crate::tests::TEST_LOCK.lock().unwrap();
        // 纯函数层:
        // 心跳 0.5s 前 + 笔迹间隔 60s → 悬停桥接, 不分
        assert!(!should_split(false, Some(500), 60_000, 2_000.0));
        // 心跳 60s 前(悬停完全不动) → 静默超阈值(60s 下限) → 分
        assert!(should_split(false, Some(60_500), 500, 2_000.0));
        // 无笔信号 → 回退笔迹间隔规则(与 split_time_adaptive 同判)
        assert!(!should_split(false, None, 59_000, 2_000.0));
        assert!(should_split(false, None, 60_500, 2_000.0));

        // 短离开(笔↔橡皮切换/字间抬笔)不置冷; 离开满 3s 才置冷
        reset_fixture();
        on_pen_activity();
        on_pen_gone();
        INNER.lock().unwrap().pen_away_since =
            Some(Instant::now() - std::time::Duration::from_millis(800));
        on_pen_back();
        assert!(!INNER.lock().unwrap().pen_cold, "短离开不应置冷");
        on_pen_gone();
        INNER.lock().unwrap().pen_away_since =
            Some(Instant::now() - std::time::Duration::from_millis(COLD_START_MIN_MS + 100));
        on_pen_back();
        assert!(INNER.lock().unwrap().pen_cold, "离开满 3s 才置冷");
    }

    /// 回归: "hello" 的字母/笔画之间只是短促抬笔, 不能各成一条消息。
    /// 模拟每笔后 proximity exit + enter(常见于抬笔即离开感应区的平板)。
    #[test]
    fn hello_short_lifts_stay_one_message() {
        let _g = crate::tests::TEST_LOCK.lock().unwrap();
        reset_fixture();
        on_pen_activity(); // 笔进入感应区, 本会话有笔信号
        for i in 1..=5 {
            commit_stroke(i);
            on_pen_gone();
            // 抬笔后 800ms 才把笔挪回感应区: 旧实现 300ms 宽限会拆,
            // 新实现按 3s 冷启动阈值并入同一条。
            INNER.lock().unwrap().pen_away_since =
                Some(Instant::now() - std::time::Duration::from_millis(800));
            on_pen_back();
        }
        let g = INNER.lock().unwrap();
        assert_eq!(g.pending.len(), 5, "hello 应是一条 5 笔的消息");
        assert!(g.msgs.is_empty(), "短抬笔不应提前结段");
        assert!(!g.pen_cold, "短离开不置冷");
    }

    /// 抬笔/迟到的活动事件不能撤销 proximity exit 的离开计时; 否则冷启动
    /// 永远不生效。真正"笔回来"只能由 on_pen_back(proximity enter, 或
    /// 平台的悬停/落笔回退)结算。
    #[test]
    fn late_activity_does_not_cancel_away_timer() {
        let _g = crate::tests::TEST_LOCK.lock().unwrap();
        reset_fixture();
        on_pen_activity();
        commit_stroke(1);
        on_pen_gone();
        on_pen_activity(); // 迟到的抬笔/活动心跳
        assert!(
            INNER.lock().unwrap().pen_away_since.is_some(),
            "活动事件不应清掉离开计时"
        );
        // 离开满 3s 后回来 → 冷标记, 下一笔开新消息
        INNER.lock().unwrap().pen_away_since =
            Some(Instant::now() - std::time::Duration::from_millis(COLD_START_MIN_MS + 100));
        on_pen_back();
        assert!(INNER.lock().unwrap().pen_cold);
        commit_stroke(2);
        let g = INNER.lock().unwrap();
        assert_eq!(g.msgs.len(), 1, "旧消息结段");
        assert_eq!(g.pending.len(), 1, "新消息只含新笔");
        assert!(!g.pen_cold, "冷标记已消费");
    }

    /// 面板刷新(flush_expired)不能把字间短抬笔提前结段; 离开满 3s 才结。
    #[test]
    fn flush_only_after_sustained_absence() {
        let _g = crate::tests::TEST_LOCK.lock().unwrap();
        reset_fixture();
        on_pen_activity();
        commit_stroke(1);
        on_pen_gone();
        INNER.lock().unwrap().pen_away_since =
            Some(Instant::now() - std::time::Duration::from_millis(800));
        let _ = messages_json();
        assert!(
            INNER.lock().unwrap().msgs.is_empty(),
            "短离开时面板刷新不结段"
        );
        on_pen_back();
        commit_stroke(2);
        assert_eq!(INNER.lock().unwrap().pending.len(), 2);

        // 真离开 3s: 刷新即结段, 笔回来后下一笔开新消息
        on_pen_gone();
        INNER.lock().unwrap().pen_away_since =
            Some(Instant::now() - std::time::Duration::from_millis(COLD_START_MIN_MS + 100));
        let _ = messages_json();
        {
            let g = INNER.lock().unwrap();
            assert_eq!(g.msgs.len(), 1, "离开满 3s 刷新应结段");
            assert!(g.pending.is_empty());
        }
        on_pen_back();
        commit_stroke(3);
        assert_eq!(INNER.lock().unwrap().pending.len(), 1, "新消息只含新笔");
    }

    /// 纯鼠标(从无笔信号): 鼠标移动回退的 on_pen_gone 不能设冷。
    #[test]
    fn pure_mouse_ignores_pen_gone() {
        let _g = crate::tests::TEST_LOCK.lock().unwrap();
        reset_fixture();
        on_pen_gone();
        assert!(INNER.lock().unwrap().pen_away_since.is_none());
        commit_stroke(1);
        on_pen_gone(); // pen_last_seen 仍为 None
        assert!(INNER.lock().unwrap().pen_away_since.is_none());
        commit_stroke(2);
        assert_eq!(INNER.lock().unwrap().pending.len(), 2);
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
            pen_away_since: None,
            pen_cold: false,
            last_capture: None,
            msgs: Vec::new(),
            next_id: 1,
            loaded: true,
        };
        // 笔冷 = 必分
        let mut g = base();
        g.pen_cold = true;
        assert!(will_split_soon(&g, 0));
        // 离开满 3s = 分(即使平台没发 enter)
        let mut g = base();
        g.pen_away_since =
            Some(Instant::now() - std::time::Duration::from_millis(COLD_START_MIN_MS + 100));
        assert!(will_split_soon(&g, 0));
        // 短离开 = 不分(字间抬笔)
        let mut g = base();
        g.pen_away_since = Some(Instant::now() - std::time::Duration::from_millis(500));
        assert!(!will_split_soon(&g, 0));
        // 心跳新鲜(0.5s 前还在悬停) = 不分(悬停构思桥接)
        let mut g = base();
        g.pen_last_seen = Some(Instant::now() - std::time::Duration::from_millis(500));
        assert!(!will_split_soon(&g, 60_000));
        // 心跳超阈值(61s 没有任何笔事件) = 分
        let mut g = base();
        g.pen_last_seen = Some(Instant::now() - std::time::Duration::from_secs(61));
        assert!(will_split_soon(&g, 0));
        // 纯鼠标: 回退笔迹间隔(60s 阈值)
        let g = base();
        assert!(!will_split_soon(&g, 59_000));
        assert!(will_split_soon(&g, 60_500));
    }
}
