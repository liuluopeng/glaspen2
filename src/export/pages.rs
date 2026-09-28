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
    let last = runtime().block_on(async {
        match db::last_screen_id().await {
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
/// 沿用活页本末页作为当前页(不再每次启动新建一页——那会积累大量空白页);
/// 只有空库才创建第一页。启动时顺带软删所有历史空白页。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_init_db(screen_w: c_int, screen_h: c_int) {
    runtime().block_on(db::init());
    let purged = runtime().block_on(db::purge_blank_screens());
    if purged > 0 {
        eprintln!("[init] 清理空白页 {purged} 页");
    }
    match runtime().block_on(db::last_screen_id()) {
        Some(id) => state::set_current_screen_id(id),
        None => runtime().block_on(db::new_screen(screen_w, screen_h)),
    }
    warm_thumbnail_cache();
}

/// Called when the display size/arrangement changed. Only starts a new page
/// when the current page already has strokes; otherwise the current page is
/// kept (avoiding silent page switches from resolution changes).
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_on_display_change(screen_w: c_int, screen_h: c_int) {
    let current = state::current_screen_id();
    if !runtime().block_on(db::screen_has_strokes(current)) {
        return;
    }
    let last = runtime().block_on(async {
        match db::last_screen_id().await {
            Some(id) => Some((id, db::screen_has_strokes(id).await)),
            None => None,
        }
    });
    let (create, _) = plan_new_page(last);
    if create {
        runtime().block_on(db::new_screen(screen_w, screen_h));
    }
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

/// Delete a screen (page) and all its data (strokes, points).
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

