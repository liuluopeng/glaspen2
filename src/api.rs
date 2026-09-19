//! Flutter ←→ Rust 桥接层：暴露给设置面板的查询函数。
//! 只放读操作和轻量操作；笔迹实时绘制/事件拦截留在覆盖层。

use crate::db;

/// 活页本概览：每页的 id 和笔迹数（用于列表展示 + 空页标识）。
#[flutter_rust_bridge::frb]
pub async fn list_pages() -> Vec<PageSummary> {
    let screens = db::list_screens().await;
    let mut out = Vec::with_capacity(screens.len());
    for (id, w, h) in screens {
        let stroke_count = db::page_stroke_count(id).await;
        out.push(PageSummary {
            id,
            width: w,
            height: h,
            stroke_count,
        });
    }
    out
}

/// 当前镜头状态（无限画布）。
#[flutter_rust_bridge::frb]
pub async fn get_lens() -> LensState {
    let cur = crate::state::current_screen_id();
    let (pan_x, pan_y, zoom) = db::get_infinite_transform().await.unwrap_or((0.0, 0.0, 1.0));
    LensState {
        page_id: cur,
        pan_x,
        pan_y,
        zoom: if zoom > 0.05 { zoom } else { 1.0 },
    }
}

/// 页号跟随：当前页在所有页中的序号（1 起）。
#[flutter_rust_bridge::frb]
pub async fn get_page_ordinal() -> u64 {
    let cur = crate::state::current_screen_id();
    db::page_info(cur).await.map(|i| i.2 as u64).unwrap_or(0)
}

// ── 数据结构 ──

#[flutter_rust_bridge::frb]
#[derive(Debug, Clone)]
pub struct PageSummary {
    pub id: i64,
    pub width: i32,
    pub height: i32,
    pub stroke_count: u64,
}

#[flutter_rust_bridge::frb]
#[derive(Debug, Clone)]
pub struct LensState {
    pub page_id: i64,
    pub pan_x: f64,
    pub pan_y: f64,
    pub zoom: f64,
}
