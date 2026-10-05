//! 活页本:页管理、新建守卫、导航、列表与画布模式切换。

use super::*;

/// Start a new canvas: a new page is created only when the notebook's last
/// page has strokes — 空白页之后不能再造空白页(要画就画在那页空白页上,
/// 不在末页时自动跳过去复用)。Returns 1 if a new page was created, 0 if
/// the existing blank tail page was reused (possibly after navigating to it).
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_clear_strokes(screen_w: c_int, screen_h: c_int) -> c_int {
    runtime().block_on(db::end_stroke()); // flush before checking — must block
    // 无限画布:全局只有一个画布,清空内容即可,不新建页。
    // 返回 1 表示"清掉了东西",0 表示本来就是空的。
    if state::canvas_kind() == state::CanvasKind::Infinite {
        let had = runtime().block_on(db::infinite_canvas_has_strokes());
        runtime().block_on(db::clear_infinite_canvas());
        STROKES.lock().unwrap().clear();
        return if had { 1 } else { 0 };
    }
    let current = state::current_screen_id();
    // 守卫按**当前几何组**判定: 组末页有笔迹才新建, 空白则复用组末页
    let last = runtime().block_on(async {
        match db::last_screen_with_geometry(screen_w, screen_h).await {
            Some(id) => Some((id, db::screen_has_strokes(id).await)),
            None => None,
        }
    });
    let (create, reuse) = plan_new_page(last);
    let mut created = 0;
    if create {
        runtime().block_on(db::new_screen(screen_w, screen_h));
        created = 1;
    } else if let Some(id) = reuse {
        // 末页已是空白:复用它,不再新建;不在那页就跳过去
        if id != current {
            glaspen2_load_strokes_for_screen(id);
        }
    }
    let mut strokes = STROKES.lock().unwrap();
    strokes.clear();
    created
}

/// Initialize the database and create the first screen record. Call once at app start.
/// 启动落在**最后一个有内容的页**:清理空白页 → 软删后仍有笔迹的末页
/// 直接沿用并载入内存(否则画布看着是空白, 新旧笔迹还会分家);
/// 空库才创建第一页。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_init_db(screen_w: c_int, screen_h: c_int) {
    runtime().block_on(db::init());
    let purged = runtime().block_on(db::purge_blank_screens());
    if purged > 0 {
        tracing::info!("清理空白页 {purged} 页");
    }
    // 启动落在**当前几何组**的末页: 页按分辨率分组, 玻璃什么尺寸就翻
    // 哪一本; 该几何从没出现过才建第一页。
    match runtime().block_on(db::last_screen_with_geometry(screen_w, screen_h)) {
        Some(id) => {
            state::set_current_screen_id(id);
            // 载入该页笔迹到 STROKES: 启动画布立即可见上次内容,
            // 新笔迹也与库中该页旧笔迹同处一份内存, 不会分家。
            glaspen2_load_strokes_for_screen(id);
        }
        None => runtime().block_on(db::new_screen(screen_w, screen_h)),
    }
    warm_thumbnail_cache();
}

/// Called when the display size/arrangement changed (防抖后的稳定值).
/// 页按分辨率分组: 分辨率切换 = **换一本** —— 落到目标几何组的末页
/// (回到离开时的那页, 游戏开关一个来回零垃圾页); 该几何从没出现过
/// 才建第一页。几何没变则什么都不做。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_on_display_change(screen_w: c_int, screen_h: c_int) {
    let current = state::current_screen_id();
    if let Some((cw, ch)) = runtime().block_on(db::screen_dims(current))
        && cw == screen_w
        && ch == screen_h
    {
        return; // 几何没变(防抖后协商回原分辨率等)
    }
    match runtime().block_on(db::last_screen_with_geometry(screen_w, screen_h)) {
        Some(id) => {
            dblog_display_enter(id, screen_w, screen_h);
            state::set_current_screen_id(id);
        }
        None => runtime().block_on(db::new_screen(screen_w, screen_h)),
    }
}

fn dblog_display_enter(id: i64, w: c_int, h: c_int) {
    tracing::info!("切换分辨率 → 进入 {w}x{h} 组末页 id={id}");
}

/// 新建页守卫的纯决策:活页本末页(未删除页中最新的一页)已有笔迹 →
/// 允许新建;**末页空白 → 不允许**(空白页之后不能再造空白页,要画就画
/// 在那页上);空库 → 新建第一页。返回 (是否新建, 可复用的末页 id)。
/// 新建页守卫的纯决策:活页本末页(未删除页中最新的一页)已有笔迹 →
/// 允许新建;**末页空白 → 不允许**(空白页之后不能再造空白页,要画就画
/// 在那页上);空库 → 新建第一页。返回 (是否新建, 可复用的末页 id)。
pub(crate) fn plan_new_page(last: Option<(i64, bool)>) -> (bool, Option<i64>) {
    match last {
        Some((_id, true)) => (true, None),
        Some((id, false)) => (false, Some(id)),
        None => (true, None),
    }
}

// ---------------------------------------------------------------------------
// Modeler FFI
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_load_strokes_for_screen(screen_id: i64) -> c_int {
    // Flush any stroke whose pen-up was still queued, so page navigation
    // never races the async point flush.
    runtime().block_on(db::end_stroke());
    let data = runtime().block_on(db::strokes_for_screen(screen_id));
    let count = data.len() as c_int;
    let mut strokes = STROKES.lock().unwrap();
    strokes.clear();
    for s in data {
        strokes.push(Stroke {
            id: s.id,
            r: s.r,
            g: s.g,
            b: s.b,
            points: s.points,
        });
    }
    // Update current screen in DB
    state::set_current_screen_id(screen_id);
    count
}

/// Smooth all loaded strokes in STROKES through the modeler. Call after loading.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_smooth_loaded_strokes() {
    // Snapshot under lock; run CPU-heavy smoothing outside the lock.
    let snapshot: Vec<(f64, f64, f64, Vec<(f64, f64, f64)>)> = {
        let strokes = STROKES.lock().unwrap();
        strokes
            .iter()
            .map(|s| {
                (
                    s.r,
                    s.g,
                    s.b,
                    s.points.iter().map(|&(x, y, w, _)| (x, y, w)).collect(),
                )
            })
            .collect()
    };

    let mut smoothed_all: Vec<Vec<(f64, f64, f64, f64)>> = Vec::with_capacity(snapshot.len());
    for (_, _, _, raw) in snapshot.iter() {
        let smoothed = modeler::smooth_points(raw);
        smoothed_all.push(smoothed);
    }

    let mut strokes = STROKES.lock().unwrap();
    for (stroke, smoothed) in strokes.iter_mut().zip(smoothed_all) {
        if !smoothed.is_empty() {
            stroke.points = decimate(&smoothed);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_prev_screen_id() -> i64 {
    runtime()
        .block_on(db::prev_screen(state::current_screen_id()))
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_next_screen_id() -> i64 {
    runtime()
        .block_on(db::next_screen(state::current_screen_id()))
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_get_current_screen_id() -> i64 {
    state::current_screen_id()
}

/// 当前页的几何(创建时的屏幕尺寸)。壳层用它做 scale-to-fit 观看:
/// 页几何 ≠ 当前屏幕时, 等比缩放居中显示而非 1:1 裁在角落。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_page_dims(screen_id: i64, w: *mut c_int, h: *mut c_int) {
    if w.is_null() || h.is_null() {
        return;
    }
    if let Some((sw, sh)) = runtime().block_on(db::screen_dims(screen_id)) {
        unsafe {
            *w = sw;
            *h = sh;
        }
    }
}

/// ── 翻页动效(时光隧道)的页缓存 ─────────────────────────────────────
///
/// 动画开始时壳层把要用到的页(目标页 + 前后各几张)一次性预热进来,
/// 逐帧渲染走缓存 —— 每帧去 SQLite 拉点列既卡又会和书写落库抢锁。
/// 只读缓存:动画期间用户仍在旧页上, STROKES / 当前页 id 都不动。
pub(crate) static PAGE_PREVIEW: std::sync::Mutex<Vec<PreviewPage>> =
    std::sync::Mutex::new(Vec::new());

/// 缓存里的一页(笔迹 + 该页几何)。
pub(crate) struct PreviewPage {
    // 页 id: 预载时写入, 供日志/调试对照缓存槽与真实页
    #[allow(dead_code)]
    pub(crate) screen_id: i64,
    pub(crate) w: i32,
    pub(crate) h: i32,
    pub(crate) strokes: Vec<db::StrokeData>,
}

/// 某页相邻的第 n 页(n 沿 id 顺序, 页按分辨率分组 —— 与
/// `glaspen2_prev_screen_id` / `glaspen2_next_screen_id` 同一邻接语义)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_page_neighbor(screen_id: i64, n: c_int) -> i64 {
    if screen_id <= 0 || n == 0 {
        return 0;
    }
    let mut id = screen_id;
    let steps = n.unsigned_abs();
    for _ in 0..steps {
        let next = if n > 0 {
            runtime().block_on(db::next_screen(id))
        } else {
            runtime().block_on(db::prev_screen(id))
        };
        match next {
            Some(v) => id = v,
            None => return 0,
        }
    }
    id
}

/// 预热翻页动效的页缓存:从 `center` 起向前 `after` 页、向后 `before` 页
/// (含 center 本身), 一次载入并平滑。返回载入的页数。
///
/// 载入顺序 = 动画里的叠放顺序(隧道向后翻时前页在上), 壳层按序取用。
/// 期间**不切当前页、不动 STROKES**。
/// 诊断用:直接插入 n 页(绕过新建守卫与空白页清理 —— probe 翻页测试
/// 只需要页存在)。正常功能**永不**调用。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_debug_insert_pages(n: c_int, w: c_int, h: c_int) -> c_int {
    let mut made = 0;
    for _ in 0..n.max(0) {
        runtime().block_on(db::new_screen(w, h));
        made += 1;
    }
    made
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_preload_flip_pages(
    center: i64,
    going_next: c_int,
    before: c_int,
    after: c_int,
) -> c_int {
    if center <= 0 {
        return 0;
    }
    // 抬笔还在排队时先把点冲刷掉, 否则快照会缺最后一笔。
    runtime().block_on(db::end_stroke());

    // 采集链:叠放顺序从上到下 = [before 页…, center, after 页…]
    let mut ids: Vec<i64> = Vec::new();
    for k in (1..=before.max(0)).rev() {
        let id = glaspen2_page_neighbor(center, -k);
        if id > 0 {
            ids.push(id);
        }
    }
    ids.push(center);
    for k in 1..=after.max(0) {
        let id = glaspen2_page_neighbor(center, k);
        if id > 0 {
            ids.push(id);
        }
    }
    let _ = going_next; // 叠放顺序恒为"近的在上", 方向只影响动画, 不影响缓存

    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        let mut strokes = runtime().block_on(db::strokes_for_screen(id));
        for s in strokes.iter_mut() {
            let raw: Vec<(f64, f64, f64)> =
                s.points.iter().map(|&(x, y, w, _)| (x, y, w)).collect();
            let smoothed = modeler::smooth_points(&raw);
            if !smoothed.is_empty() {
                s.points = decimate(&smoothed);
            }
        }
        let (w, h) = runtime().block_on(db::screen_dims(id)).unwrap_or((0, 0));
        out.push(PreviewPage {
            screen_id: id,
            w,
            h,
            strokes,
        });
    }
    let n = out.len() as c_int;
    *PAGE_PREVIEW.lock().unwrap() = out;
    n
}

/// 把预热缓存里第 `slot` 页(0 起)画进外部 cairo 表面(透明底)。
///
/// 与画布渲染同一条坐标管线:`scale` 是视口 backing scale(点 → 像素),
/// `ox/oy/pscale` 是 scale-to-fit 的"页像素 → 逻辑点"变换(不 fit 时 0/0/1),
/// `alpha` 是整页不透明度(隧道卡片的渐显/渐隐),`white_bg` 供导出用。
/// 返回 1 成功, 0 = 槽位不存在 / 表面绑定失败。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_paint_preview_into_surface(
    surface_ptr: *mut std::ffi::c_void,
    slot: c_int,
    scale: c_double,
    ox: c_double,
    oy: c_double,
    pscale: c_double,
    alpha: c_double,
    white_bg: c_int,
    depth: c_double,
) -> c_int {
    let Some(r) = crate::cairo_dl::CairoRenderer::from_surface(surface_ptr) else {
        return 0;
    };
    let buf = PAGE_PREVIEW.lock().unwrap();
    let Some(page) = buf.get(slot.max(0) as usize) else {
        return 0;
    };
    render_page_into(
        &r,
        &page.strokes,
        scale,
        ox,
        oy,
        pscale,
        page.w as f64,
        page.h as f64,
        alpha,
        white_bg != 0,
        depth,
    )
}

/// Delete a screen (page) and all of its data (strokes, points).
/// Returns 1 on success, 0 on failure.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_delete_screen(screen_id: i64) -> c_int {
    let ok = runtime().block_on(db::delete_screen(screen_id));
    // 只有翻页模式才把"被删的是当前页"反映到内存;无限画布与页存储无关。
    if ok
        && state::canvas_kind() == state::CanvasKind::Page
        && screen_id == state::current_screen_id()
    {
        state::set_current_screen_id(0);
        STROKES.lock().unwrap().clear();
    }
    ok as c_int
}

// ---------------------------------------------------------------------------
// Stroke introspection
// ---------------------------------------------------------------------------

/// Undo the last stroke: remove from both STROKES (memory) and DB.
/// Returns the number of remaining strokes, or -1 if there was nothing to undo.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_undo_last_stroke() -> c_int {
    let id = {
        let mut strokes = STROKES.lock().unwrap();
        if strokes.is_empty() {
            return -1;
        }
        strokes.pop().map(|s| s.id).unwrap_or(0)
    };
    if id > 0 {
        if state::canvas_kind() == state::CanvasKind::Infinite {
            runtime().block_on(db::delete_infinite_stroke_by_id(id));
        } else {
            runtime().block_on(db::delete_stroke_by_id(id));
        }
    }
    STROKES.lock().unwrap().len() as c_int
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_delete_last_stroke() {
    if state::canvas_kind() == state::CanvasKind::Infinite {
        runtime().block_on(db::delete_last_infinite_stroke());
    } else {
        runtime().block_on(db::delete_last_stroke());
    }
    STROKES.lock().unwrap().pop();
}

// ---------------------------------------------------------------------------
// PDF export
// ---------------------------------------------------------------------------

/// Export all pages to a PDF on the desktop.  Returns 1 on success, 0 on failure.
/// List all screens as JSON.
/// Returns a C string (caller must free via glaspen2_free_c_string).
/// JSON: [{"id":1,"w":1920,"h":1080}, ...]
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_list_screens_json() -> *mut c_char {
    let rows = runtime().block_on(db::list_screens());
    let list: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|(id, w, h)| serde_json::json!({ "id": id, "w": w, "h": h }))
        .collect();
    let json = serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string());
    CString::new(json).unwrap_or_default().into_raw()
}

/// 活页本重排: 把某页移到锚点页前/后(面板内前后移/拖拽)。
/// 返回 1 成功, 0 失败(页不存在/锚点不存在)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_page_reorder(screen_id: i64, anchor_id: i64, before: c_int) -> c_int {
    match runtime().block_on(db::reorder_screen(screen_id, anchor_id, before != 0)) {
        Ok(()) => 1,
        Err(e) => {
            tracing::error!("重排失败: {e}");
            0
        }
    }
}

// ── 页面详情: 圈选 / 移动 / 复制粘贴 / 删除所选(面板内鼠标操作) ──
// 设计原则: 玻璃上的笔只做生成动作, 一切治理动作归面板。本组 FFI 全是
// 纯 DB 数据操作; 涉及当前页时由 ObjC 壳刷新玻璃(rebuild)。

/// 圈选命中: 多边形任一点在内部的笔迹(整笔为单位)。
/// poly = 逗号分隔的 "x,y" 对(页面坐标)。返回入选 stroke id 列表
/// (逗号分隔字符串, 空串 = 无), 调用方负责 free。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_lasso_select(screen_id: i64, poly: *const c_char) -> *mut c_char {
    let Some(poly) = (unsafe { poly.as_ref() }).and_then(|p| {
        unsafe { CStr::from_ptr(p) }
            .to_str()
            .ok()
            .map(|s| s.to_string())
    }) else {
        return CString::new("").unwrap_or_default().into_raw();
    };
    // 线格式 "x1,y1,x2,y2,...": 数值流两两成对(此前按对取, 全部滤空
    // → 圈选恒返回"圈内没有笔迹")
    let vals: Vec<f64> = poly
        .split(',')
        .filter_map(|t| t.trim().parse::<f64>().ok())
        .collect();
    let pts: Vec<(f64, f64)> = vals
        .chunks(2)
        .filter(|c| c.len() == 2)
        .map(|c| (c[0], c[1]))
        .collect();
    if pts.len() < 3 {
        return CString::new("").unwrap_or_default().into_raw();
    }
    let strokes = runtime().block_on(db::strokes_for_screen(screen_id));
    let ids: Vec<String> = strokes
        .iter()
        .filter(|s| {
            s.points
                .iter()
                .any(|&(x, y, _, _)| point_in_polygon(x, y, &pts))
        })
        .map(|s| s.id.to_string())
        .collect();
    CString::new(ids.join(",")).unwrap_or_default().into_raw()
}

/// 射线法: 点是否在多边形内(边界不算内部, 圈选容差由外扩处理)
fn point_in_polygon(px: f64, py: f64, poly: &[(f64, f64)]) -> bool {
    let n = poly.len();
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        if ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// 刷新当前页玻璃(若 screen_id 是当前页且在翻页模式)。
/// 由壳注入回调(ObjC 侧 load+rebuild 必须在主线程做 surface 操作);
/// 未注入(测试/Windows 管道场景自己刷新)则退化为仅载入 STROKES。
static REFRESH_HOOK: std::sync::Mutex<Option<fn(i64)>> = std::sync::Mutex::new(None);

pub fn set_glass_refresh_hook(f: fn(i64)) {
    *REFRESH_HOOK.lock().unwrap() = Some(f);
}

fn refresh_glass_if_current(screen_id: i64) {
    if state::current_screen_id() != screen_id {
        return;
    }
    if state::canvas_kind() == state::CanvasKind::Infinite {
        return;
    }
    match REFRESH_HOOK.lock().unwrap().as_ref() {
        Some(f) => f(screen_id),
        None => {
            glaspen2_load_strokes_for_screen(screen_id);
        }
    }
}

/// 平移所选笔迹(同页)。返回 1 成功。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_move_strokes(
    screen_id: i64,
    ids_csv: *const c_char,
    dx: c_double,
    dy: c_double,
) -> c_int {
    let Some(ids) = parse_ids_csv(ids_csv) else {
        return 0;
    };
    if ids.is_empty() || (dx == 0.0 && dy == 0.0) {
        return 1; // 空操作视为成功
    }
    let n = runtime().block_on(db::move_strokes_translate(screen_id, &ids, dx, dy));
    if n > 0 {
        runtime().block_on(db::thumbnails_purge_screen(screen_id));
        refresh_glass_if_current(screen_id);
    }
    (n > 0) as c_int
}

/// 把所选笔迹搬到目标页并平移(跨页复制语义的"移动"变体: 保 id 保时间)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_move_strokes_to_page(
    screen_id: i64,
    ids_csv: *const c_char,
    target_screen_id: i64,
    dx: c_double,
    dy: c_double,
) -> c_int {
    let Some(ids) = parse_ids_csv(ids_csv) else {
        return 0;
    };
    if ids.is_empty() || target_screen_id == screen_id {
        return if target_screen_id == screen_id {
            glaspen2_move_strokes(screen_id, ids_csv, dx, dy)
        } else {
            0
        };
    }
    let n = runtime().block_on(db::move_strokes_to_screen(
        screen_id,
        &ids,
        target_screen_id,
        dx,
        dy,
    ));
    if n > 0 {
        runtime().block_on(db::thumbnails_purge_screen(screen_id));
        runtime().block_on(db::thumbnails_purge_screen(target_screen_id));
        refresh_glass_if_current(screen_id);
        refresh_glass_if_current(target_screen_id);
    }
    (n > 0) as c_int
}

/// 删除所选笔迹(软删)。返回 1 成功。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_delete_strokes(screen_id: i64, ids_csv: *const c_char) -> c_int {
    let Some(ids) = parse_ids_csv(ids_csv) else {
        return 0;
    };
    if ids.is_empty() {
        return 1;
    }
    let n = runtime().block_on(db::delete_strokes(screen_id, &ids));
    if n > 0 {
        runtime().block_on(db::thumbnails_purge_screen(screen_id));
        refresh_glass_if_current(screen_id);
    }
    (n > 0) as c_int
}

/// 把剪贴板笔迹粘到某页(新 id 新时间 = 新实体; 点内相对 t 保速度曲线)。
/// payload = 手写消息同款墨迹序列化(逗号三元组 "x,y,w" 分号连接)。
/// 返回新 stroke 数。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_paste_strokes(screen_id: i64, payload: *const c_char) -> c_int {
    let Some(payload) = (unsafe { payload.as_ref() }).and_then(|p| {
        unsafe { CStr::from_ptr(p) }
            .to_str()
            .ok()
            .map(|s| s.to_string())
    }) else {
        return 0;
    };
    // payload: "r,g,b|x,y,w,x,y,w,...;..."(段前缀颜色, 坐标中心在原点)
    let mut strokes: Vec<(f64, f64, f64, Vec<(f64, f64, f64)>)> = Vec::new();
    for seg in payload.split(';') {
        let (head, body) = match seg.split_once('|') {
            Some((h, b)) => (h, b),
            None => continue,
        };
        let mut hc = head.split(',');
        let cr: f64 = hc.next().unwrap_or("1").parse().unwrap_or(1.0);
        let cg: f64 = hc.next().unwrap_or("0").parse().unwrap_or(0.0);
        let cb: f64 = hc.next().unwrap_or("0").parse().unwrap_or(0.0);
        // body = "x,y,w,x,y,w,...": 数值流三个一组(此前按组取, 全部 break 空)
        let vals: Vec<f64> = body
            .split(',')
            .filter_map(|t| t.trim().parse().ok())
            .collect();
        let mut pts = Vec::new();
        for c in vals.chunks(3) {
            if c.len() == 3 {
                pts.push((c[0], c[1], c[2]));
            }
        }
        if pts.len() >= 2 {
            strokes.push((cr, cg, cb, pts));
        }
    }
    if strokes.is_empty() {
        return 0;
    }
    let n = runtime().block_on(db::paste_strokes(screen_id, &strokes));
    if n > 0 {
        runtime().block_on(db::thumbnails_purge_screen(screen_id));
        refresh_glass_if_current(screen_id);
    }
    n as c_int
}

/// 取所选笔迹的剪贴板载荷(手写消息同款): "x,y,w;x,y,w;..."。
/// 坐标已平移为包围盒中心在原点(粘贴时按中心对齐鼠标)。
/// 返回 C 字符串(调用方 free), 空串 = 无。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_copy_strokes_payload(
    screen_id: i64,
    ids_csv: *const c_char,
) -> *mut c_char {
    let Some(ids) = parse_ids_csv(ids_csv) else {
        return CString::new("").unwrap_or_default().into_raw();
    };
    let strokes = runtime().block_on(db::strokes_for_screen(screen_id));
    let sel: Vec<&crate::db::StrokeData> = strokes.iter().filter(|s| ids.contains(&s.id)).collect();
    if sel.is_empty() {
        return CString::new("").unwrap_or_default().into_raw();
    }
    let mut minx = f64::MAX;
    let mut miny = f64::MAX;
    let mut maxx = f64::MIN;
    let mut maxy = f64::MIN;
    for s in &sel {
        for &(x, y, _, _) in &s.points {
            minx = minx.min(x);
            miny = miny.min(y);
            maxx = maxx.max(x);
            maxy = maxy.max(y);
        }
    }
    let cx = (minx + maxx) / 2.0;
    let cy = (miny + maxy) / 2.0;
    let mut segs: Vec<String> = Vec::new();
    for s in &sel {
        // 段前缀颜色(粘贴保原色 —— 复制红色块粘出来还是红色)
        let head = format!(
            "{:.3},{:.3},{:.3}|",
            s.r.clamp(0.0, 1.0),
            s.g.clamp(0.0, 1.0),
            s.b.clamp(0.0, 1.0)
        );
        let pts: Vec<String> = s
            .points
            .iter()
            .map(|&(x, y, w, _)| format!("{:.3},{:.3},{:.3}", x - cx, y - cy, w))
            .collect();
        segs.push(format!("{}{}", head, pts.join(",")));
    }
    CString::new(segs.join(";")).unwrap_or_default().into_raw()
}

fn parse_ids_csv(csv: *const c_char) -> Option<Vec<i64>> {
    let s = unsafe { csv.as_ref()? };
    let s = unsafe { CStr::from_ptr(s) }.to_str().ok()?;
    Some(
        s.split(',')
            .filter_map(|t| t.trim().parse::<i64>().ok())
            .collect(),
    )
}

/// Page info JSON for the 新建画布/翻页 notification:
/// {"nth":n,"date_total":m,"pos":x,"total":y,"created":unix_ts}
/// Caller frees with glaspen2_free_c_string. NULL when the page is unknown.
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_page_info_json(screen_id: i64) -> *mut c_char {
    let Some((nth, date_total, pos, total, created)) = runtime().block_on(db::page_info(screen_id))
    else {
        return std::ptr::null_mut();
    };
    let json = serde_json::json!({
        "nth": nth,
        "date_total": date_total,
        "pos": pos,
        "total": total,
        "created": created,
    })
    .to_string();
    match CString::new(json) {
        Ok(cs) => cs.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

// ---------------------------------------------------------------------------
// Thumbnail rendering (cairo_dl → scaled PNG)
// ---------------------------------------------------------------------------
