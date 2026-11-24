//! 页面缩略图条(minimap)的**页窗与条带布局**唯一实现(D7)。
//!
//! 背景: 两端壳曾经各写一份同一套页窗/条带几何(`docs/duplication-backlog.md` D7):
//!
//! - macOS: `src/macos/swift/run_glue.swift` 的 `draw_minimap`(从
//!   `glaspen2_list_screens_json` 里按当前页 ±5 取窗口、按 `g_pageview_pw`
//!   过滤几何组、条带贴右缘 8pt / 顶端 8pt、卡片 64×40、间距 4、页号 =
//!   全列表 1 基序号),状态在 `src/macos/swift/run_glue.swift`。
//! - Windows: `crates/glaspen-windows/src/overlay/actions.rs` 的
//!   `minimap_page_ids` / `minimap_pump` / `draw_minimap`(同一页窗 5/10,
//!   但无几何过滤、条带垂直居中且顶边不小于 4px、不画页号文本)。
//!
//! 本模块只算**页窗 + 条带几何**(取哪些页、每张卡片的 y/高度/页号文本);
//! 缩略图渲染/缓存/后台队列/条带合成/描边/颜色/字体仍留在各平台 adapter。
//!
//! # 页窗公式(两端逐字相同)
//!
//! `~text
//! rows  = db::list_screens()
//!         (screens.deleted_at IS NULL 且 EXISTS(该页有 strokes 行);
//!          按 order_index, id;macOS page-view-fit 模式额外只保留
//!          screen_w == 当前页宽的页)
//! idx   = rows.index_of(current)(找不到 → 无 minimap, 见 page_window)
//! start = idx.saturating_sub(5)
//! end   = min(start + 10, rows.len())
//! `~
//!
//! 注意: `list_screens` 的 EXISTS 不看 strokes.deleted_at —— 页的软删以
//! screens.deleted_at 为准(两端旧实现一致);只删光笔迹的页仍算"有笔迹的页"。
//!
//! # 条带布局公式(两端逐字)
//!
//! `~text
//! n       = 窗口内页数
//! total_h = n * item_h + (n - 1) * gap                    (n = 0 → 0)
//! strip_x = view_w - item_w - margin_right                (右缘 8pt = 64px)
//! macOS   : strip_y = 8                                   (顶端对齐, 8pt)
//! Windows : strip_y = max((view_h - total_h) / 2, 4)      (垂直居中, 顶边 >= 4px)
//! item i  : y = strip_y + i * (item_h + gap), h = item_h
//!           label = (start + i + 1).to_string()           (全列表 1 基序号)
//! `~
//!
//! # 坐标约定
//!
//! 视图坐标 = 逻辑点, 原点左上, **y 向下**(与 [crate::grid] 一致)。Windows
//! 位图(像素行向下)直接消费;macOS 的 NSImage/CGContext 是 y 向上, adapter
//! 合成条带时翻转(core 不再算第二套公式):
//!
//! `~text
//! local_y_up = strip_h - ((item.y - strip_y) + item.h)     // 条带图内坐标
//! dst_y      = view_h - strip_y - strip_h                  // 条带贴图位置
//! `~
//!
//! # 两端真实差异(只显式化, 不静默统一)
//!
//! 1. **垂直锚点**: macOS 条带顶端距视图顶 8pt([MACOS_METRICS],
//!    [VerticalPlacement::Top]);Windows 垂直居中且顶边不小于 4px
//!    ([WINDOWS_METRICS], [VerticalPlacement::Centered])。
//! 2. **页窗过滤**: macOS 在 page-view-fit 模式(`g_pageview_pw > 0`, 即当前页
//!    几何与屏幕不同)只显示当前几何组(按页宽过滤);Windows 无此过滤、显示
//!    全部活页。→ [PageFilter]。
//! 3. **页号文本**: macOS 画页号(monospaced 9pt, 卡片左上 +3/+3);Windows
//!    分层窗口画文本需另建 GDI DC, 首版不画。core 两端都下发
//!    [MinimapItem::label], Windows 忽略。
//! 4. **绘制样式**: 卡片底色/占位底色/边框颜色与线宽/当前页高亮(macOS
//!    systemRed 2pt;Windows 当前笔色 2px)/缩略图等比缩放算法全部留在 adapter
//!    (与 D6 同规则: 样式不进 core)。
//!
//! 窗口大小(当前页前后各 5 页、最多 10 项)两端一致, 上收为
//! [WINDOW_HALF] / [WINDOW_COUNT], adapter 不再写 5/10 字面量。
//!
//! # 异常输入(边界)
//!
//! - 视图宽/高非有限或非正 → [MinimapError::ViewSize];
//! - 卡片宽/高/gap/边距非有限(或宽高非正、间距为负) → [MinimapError::Metrics];
//! - ids 为空是**合法**状态(当前页没有笔迹/空库): 回空 items 的布局;
//! - 当前页不在窗口里是**合法**状态(新建空白页): [page_window] 回 None。

/// 当前页前后各取几页(macOS `idx > 5` / Windows `idx.saturating_sub(5)` 上收)。
pub const WINDOW_HALF: usize = 5;
/// 窗口最多几项(两端 `start + 10` 上收, = 2 × [WINDOW_HALF])。
pub const WINDOW_COUNT: usize = 10;

/// 卡片宽(macOS `tw = 64` / Windows `MM_TW = 64`)。
pub const ITEM_W: f64 = 64.0;
/// 卡片高(macOS `th = 40` / Windows `MM_TH = 40`)。
pub const ITEM_H: f64 = 40.0;
/// 卡片间距(macOS `gap = 4` / Windows `MM_GAP = 4`)。
pub const ITEM_GAP: f64 = 4.0;
/// 条带距右缘(macOS/Windows 都是 8)。
pub const MARGIN_RIGHT: f64 = 8.0;
/// macOS 条带顶端距视图顶(macOS `bounds.height - total_h - 8` 的 8)。
pub const MACOS_TOP_MARGIN: f64 = 8.0;
/// Windows 条带顶边最小值(Windows `((h - total_h) / 2).max(4)` 的 4)。
pub const WINDOWS_MIN_TOP: f64 = 4.0;

/// 条带的垂直锚点(两端差异, 见模块文档差异 1)。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VerticalPlacement {
    /// 条带顶边距视图顶边固定 `margin`(macOS: 8pt)。
    Top { margin: f64 },
    /// 条带垂直居中, 顶边不小于 `min_top`(Windows: 4px)。
    Centered { min_top: f64 },
}

/// 条带布局参数(卡片尺寸/间距/右缘边距/垂直锚点)。
///
/// 两端常量见 [MACOS_METRICS] / [WINDOWS_METRICS];测试可自定义。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MinimapMetrics {
    pub item_w: f64,
    pub item_h: f64,
    pub gap: f64,
    pub margin_right: f64,
    pub placement: VerticalPlacement,
}

/// macOS 现有条带样式: 卡片 64×40 / 间距 4 / 右缘 8 / 顶端对齐 8pt。
pub const MACOS_METRICS: MinimapMetrics = MinimapMetrics {
    item_w: ITEM_W,
    item_h: ITEM_H,
    gap: ITEM_GAP,
    margin_right: MARGIN_RIGHT,
    placement: VerticalPlacement::Top {
        margin: MACOS_TOP_MARGIN,
    },
};

/// Windows 现有条带样式: 卡片 64×40 / 间距 4 / 右缘 8 / 垂直居中且顶边 >= 4px。
pub const WINDOWS_METRICS: MinimapMetrics = MinimapMetrics {
    item_w: ITEM_W,
    item_h: ITEM_H,
    gap: ITEM_GAP,
    margin_right: MARGIN_RIGHT,
    placement: VerticalPlacement::Centered {
        min_top: WINDOWS_MIN_TOP,
    },
};

/// 页窗的页来源过滤(两端差异, 见模块文档差异 2)。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageFilter {
    /// 全部活页(Windows 现状;macOS page-view-fit 关闭时也是这个)。
    All,
    /// 额外只保留 `screen_w == width` 的页(macOS page-view-fit 模式, 只显示
    /// 当前几何组)。
    Width(i32),
}

/// 页窗: 当前页附近的有序页 id 列表(见模块文档"页窗公式")。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageWindow {
    /// 窗口内的页 id, 按活页本顺序(唯一的 id 列表来源)。
    pub ids: Vec<i64>,
    /// 窗口首项在全序列表中的 0 基下标(页号 = start + row + 1)。
    pub start: usize,
    /// 当前页在全序列表中的 0 基下标。
    pub current_index: usize,
    /// 全序列表的页数(过滤后)。
    pub total: usize,
}

/// 条带里的一张卡片(位置 + 页号文本 + 当前页标记)。
#[derive(Clone, Debug, PartialEq)]
pub struct MinimapItem {
    pub screen_id: i64,
    /// 全序列表中的 0 基下标(页号文本 = index + 1)。
    pub index: usize,
    /// 卡片 y(视图坐标, y 向下 = 卡片上沿)。
    pub y: f64,
    /// 卡片高(= metrics.item_h)。
    pub h: f64,
    /// 页号文本(全列表 1 基序号;macOS 画, Windows 首版忽略)。
    pub label: String,
    /// 是否当前页(两端高亮样式不同, 标记本身同源)。
    pub current: bool,
}

/// 条带布局: 右缘竖条的位置尺寸 + 每张卡片。
#[derive(Clone, Debug, PartialEq)]
pub struct MinimapLayout {
    /// 条带左缘 x(= 卡片左缘)。
    pub strip_x: f64,
    /// 条带上沿 y(y 向下;空窗口/空 ids 时仍按公式给值)。
    pub strip_y: f64,
    /// 条带宽(= metrics.item_w)。
    pub strip_w: f64,
    /// 条带高(= n*item_h + (n-1)*gap;空窗口 = 0)。
    pub strip_h: f64,
    pub items: Vec<MinimapItem>,
}

/// 布局参数非法(视图尺寸或样式参数)。两端 adapter 正常运行时不可达
/// (窗口尺寸恒为正、样式是编译期常量), 仅防御号/NaN 污染。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MinimapError {
    /// 视图宽/高不是有限正数。
    ViewSize,
    /// 卡片尺寸/间距/边距不是有限数(或宽高非正、间距为负)。
    Metrics,
}

impl MinimapError {
    pub fn message(self) -> &'static str {
        match self {
            MinimapError::ViewSize => "缩略图条视图尺寸非法(需要有限正数)",
            MinimapError::Metrics => "缩略图条样式参数非法(需要有限数)",
        }
    }
}

impl std::fmt::Display for MinimapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for MinimapError {}

/// 从页列表行(按活页本顺序)生成页窗 —— 纯函数, 便于单测。
///
/// `rows` = `(id, screen_w, screen_h)`, 与 `db::list_screens()` 同形;
/// `half` = 当前页前后各几页, `count` = 窗口最多几项(两端都是
/// [WINDOW_HALF]/[WINDOW_COUNT])。当前页不在过滤后的列表里 → None
/// (两端旧实现都直接跳过 minimap, 例如新建的空白页还没有笔迹)。
pub fn page_window_from_rows(
    rows: &[(i64, i32, i32)],
    current: i64,
    filter: PageFilter,
    half: usize,
    count: usize,
) -> Option<PageWindow> {
    let ids: Vec<i64> = rows
        .iter()
        .filter(|(_, w, _)| match filter {
            PageFilter::All => true,
            PageFilter::Width(width) => *w == width,
        })
        .map(|(id, _, _)| *id)
        .collect();
    let total = ids.len();
    let current_index = ids.iter().position(|&id| id == current)?;
    let start = current_index.saturating_sub(half);
    let end = start.saturating_add(count).min(total);
    Some(PageWindow {
        ids: ids[start..end].to_vec(),
        start,
        current_index,
        total,
    })
}

/// 读取当前活页本页序并生成页窗(生产入口;macOS 经 FFI / Windows 直接调)。
///
/// 页 id 的过滤/排序唯一来源 = `db::list_screens()`(见模块文档"页窗公式")。
/// 内部 `block_on` 与既有 FFI 一样: 调用方(mac ObjC 主线程 / Windows 消息
/// 线程)本就不在 tokio 运行时里;adapter 负责按"当前页变化"缓存结果, 不在
/// 绘制热路径上反复查库。
pub fn page_window(current: i64, filter: PageFilter) -> Option<PageWindow> {
    let rows = crate::runtime().block_on(crate::db::list_screens());
    page_window_from_rows(&rows, current, filter, WINDOW_HALF, WINDOW_COUNT)
}

/// 计算右缘缩略图条布局(纯函数, 公式见模块文档"条带布局公式")。
///
/// `ids` = 窗口内页 id(有序), `start` = 窗口首项在全列表中的 0 基下标
/// (页号文本/序号用), `current` = 当前页 id(高亮标记)。
/// 空 ids 合法: 回空 items 的布局(total_h = 0)。非法尺寸回 [MinimapError],
/// 绝不产出 NaN/Inf。
pub fn layout(
    view_w: f64,
    view_h: f64,
    ids: &[i64],
    start: usize,
    current: i64,
    metrics: &MinimapMetrics,
) -> Result<MinimapLayout, MinimapError> {
    // 输入校验: 视图尺寸 → 样式参数。
    if !(view_w.is_finite() && view_w > 0.0 && view_h.is_finite() && view_h > 0.0) {
        return Err(MinimapError::ViewSize);
    }
    if !(metrics.item_w.is_finite()
        && metrics.item_w > 0.0
        && metrics.item_h.is_finite()
        && metrics.item_h > 0.0
        && metrics.gap.is_finite()
        && metrics.gap >= 0.0
        && metrics.margin_right.is_finite())
    {
        return Err(MinimapError::Metrics);
    }

    let n = ids.len();
    let total_h = if n == 0 {
        0.0
    } else {
        n as f64 * metrics.item_h + (n - 1) as f64 * metrics.gap
    };
    let strip_x = view_w - metrics.item_w - metrics.margin_right;
    let strip_y = match metrics.placement {
        VerticalPlacement::Top { margin } if margin.is_finite() => margin,
        VerticalPlacement::Centered { min_top } if min_top.is_finite() => {
            ((view_h - total_h) / 2.0).max(min_top)
        }
        _ => return Err(MinimapError::Metrics),
    };

    let items = ids
        .iter()
        .enumerate()
        .map(|(row, &screen_id)| {
            let index = start + row;
            MinimapItem {
                screen_id,
                index,
                y: strip_y + row as f64 * (metrics.item_h + metrics.gap),
                h: metrics.item_h,
                label: (index + 1).to_string(),
                current: screen_id == current,
            }
        })
        .collect();

    Ok(MinimapLayout {
        strip_x,
        strip_y,
        strip_w: metrics.item_w,
        strip_h: total_h,
        items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(ids_and_widths: &[(i64, i32)]) -> Vec<(i64, i32, i32)> {
        ids_and_widths
            .iter()
            .map(|&(id, w)| (id, w, 1080))
            .collect()
    }

    fn ids_range(first: i64, last: i64) -> Vec<i64> {
        (first..=last).collect()
    }

    // ── 页窗: 首/中/末页、夹紧、过滤、缺页 ──

    #[test]
    fn window_first_page_anchors_at_zero() {
        let rows = rows(&(1..=20).map(|id| (id, 1920)).collect::<Vec<_>>());
        let w = page_window_from_rows(&rows, 1, PageFilter::All, WINDOW_HALF, WINDOW_COUNT)
            .expect("当前页在列表里");
        assert_eq!(w.start, 0);
        assert_eq!(w.current_index, 0);
        assert_eq!(w.total, 20);
        assert_eq!(w.ids, ids_range(1, 10));
    }

    #[test]
    fn window_middle_centers_current() {
        let rows = rows(&(1..=20).map(|id| (id, 1920)).collect::<Vec<_>>());
        // 索引 7(第 8 页): start = 7-5 = 2 → 3..=12, 当前页在窗口第 5 行。
        let w = page_window_from_rows(&rows, 8, PageFilter::All, WINDOW_HALF, WINDOW_COUNT)
            .expect("当前页在列表里");
        assert_eq!((w.start, w.current_index, w.total), (2, 7, 20));
        assert_eq!(w.ids, ids_range(3, 12));
        assert_eq!(w.current_index - w.start, WINDOW_HALF);
    }

    #[test]
    fn window_last_page_clamps_end() {
        let rows = rows(&(1..=20).map(|id| (id, 1920)).collect::<Vec<_>>());
        // 末页索引 19: start = 14, end = min(24, 20) = 20 → 只剩 6 项。
        let w = page_window_from_rows(&rows, 20, PageFilter::All, WINDOW_HALF, WINDOW_COUNT)
            .expect("当前页在列表里");
        assert_eq!((w.start, w.current_index, w.total), (14, 19, 20));
        assert_eq!(w.ids, ids_range(15, 20));
        assert_eq!(w.ids.len(), 6);
        assert_eq!(w.current_index - w.start, WINDOW_HALF);
    }

    #[test]
    fn window_small_book_is_whole_book() {
        let rows = rows(&[(1, 1920), (2, 1920), (3, 1920)]);
        let w = page_window_from_rows(&rows, 2, PageFilter::All, WINDOW_HALF, WINDOW_COUNT)
            .expect("当前页在列表里");
        assert_eq!(w.ids, vec![1, 2, 3]);
        assert_eq!((w.start, w.current_index, w.total), (0, 1, 3));
        // 末页: start 夹到 0, 窗口仍是整本(idx=2 < 5)。
        let w =
            page_window_from_rows(&rows, 3, PageFilter::All, WINDOW_HALF, WINDOW_COUNT).unwrap();
        assert_eq!(w.ids, vec![1, 2, 3]);
        assert_eq!((w.start, w.current_index), (0, 2));
        // 单页也合法(窗口就是那一页)。
        let one = [(7, 1920, 1080)];
        let w = page_window_from_rows(&one, 7, PageFilter::All, WINDOW_HALF, WINDOW_COUNT).unwrap();
        assert_eq!((w.ids, w.start, w.current_index), (vec![7], 0, 0));
    }

    #[test]
    fn window_radius_parameters_clamp() {
        // radius=0 → 只显示当前页; count=3 → 窗口 3 项(当前页居中偏后由 start 决定)。
        let rows = rows(&(1..=10).map(|id| (id, 1920)).collect::<Vec<_>>());
        let w = page_window_from_rows(&rows, 5, PageFilter::All, 0, 1).unwrap();
        assert_eq!(w.ids, vec![5]);
        assert_eq!((w.start, w.current_index), (4, 4));
        let w = page_window_from_rows(&rows, 5, PageFilter::All, 1, 3).unwrap();
        assert_eq!(w.ids, vec![4, 5, 6]);
        assert_eq!((w.start, w.current_index - w.start), (3, 1));
    }

    #[test]
    fn window_current_outside_list_is_none() {
        let rows = rows(&[(1, 1920), (2, 1920)]);
        // 新建的空页还没有 strokes 行 → 不在 list_screens 里;两端旧实现都跳过。
        assert!(
            page_window_from_rows(&rows, 99, PageFilter::All, WINDOW_HALF, WINDOW_COUNT).is_none()
        );
        assert!(
            page_window_from_rows(&[], 1, PageFilter::All, WINDOW_HALF, WINDOW_COUNT).is_none()
        );
    }

    #[test]
    fn window_geometry_filter_matches_macos_fit_mode() {
        // macOS page-view-fit: 只显示 screen_w 与当前页一致的页(按宽过滤)。
        let rows = vec![
            (1, 1920, 1080),
            (2, 3440, 1440),
            (3, 1920, 1080),
            (4, 3440, 1440),
            (5, 1920, 1080),
        ];
        let w = page_window_from_rows(&rows, 3, PageFilter::Width(1920), WINDOW_HALF, WINDOW_COUNT)
            .expect("当前页在 1920 组里");
        assert_eq!(w.ids, vec![1, 3, 5]);
        assert_eq!((w.start, w.current_index, w.total), (0, 1, 3));
        // 当前页不在该组(宽度不同)→ 不显示。Windows 走 PageFilter::All。
        assert!(
            page_window_from_rows(&rows, 2, PageFilter::Width(1920), WINDOW_HALF, WINDOW_COUNT)
                .is_none()
        );
        let all =
            page_window_from_rows(&rows, 2, PageFilter::All, WINDOW_HALF, WINDOW_COUNT).unwrap();
        assert_eq!(all.ids, vec![1, 2, 3, 4, 5]);
    }

    // ── 布局: 两端公式、y/标签/高亮、边界 ──

    #[test]
    fn layout_macos_top_anchored_matches_old_formula() {
        let ids = ids_range(3, 12); // 10 项, start = 2
        let l = layout(1440.0, 900.0, &ids, 2, 7, &MACOS_METRICS).expect("合法输入");
        // strip_x = 1440 - 64 - 8; strip_h = 10*40 + 9*4; 顶端 8。
        assert_eq!(
            (l.strip_x, l.strip_w, l.strip_y, l.strip_h),
            (1368.0, 64.0, 8.0, 436.0)
        );
        assert_eq!(l.items.len(), 10);
        // 旧 macOS: 条带贴图原点 y = height - total_h - 8, 条带内行 row 的
        // NSImage y(向上) = (n-1-row)*44;core 统一 y 向下 = height - (y_up + h)。
        let old_y_up = |row: usize| 900.0 - 436.0 - 8.0 + (9 - row) as f64 * 44.0;
        let old = |row: usize| 900.0 - (old_y_up(row) + 40.0);
        for (row, item) in l.items.iter().enumerate() {
            assert_eq!(item.y, old(row), "row {row}");
            assert_eq!(item.h, 40.0);
            assert_eq!(item.index, 2 + row);
            assert_eq!(item.label, (2 + row + 1).to_string());
        }
        // 卡片首尾: 顶部卡片正好距视图顶 8pt, 底部卡片 y = strip_y + 9*44。
        assert_eq!(l.items[0].y, 8.0);
        assert_eq!(l.items[9].y, 404.0);
        // 当前页 7(窗口首项 3 → row 4)只有一张高亮。
        assert_eq!(l.items.iter().filter(|i| i.current).count(), 1);
        assert_eq!(l.items[4].screen_id, 7);
        assert!(l.items[4].current);
    }

    #[test]
    fn layout_windows_centered_matches_old_formula() {
        let ids = ids_range(1, 10); // start = 0
        // (900 - 436) / 2 = 232(偶数差: 整数除与浮点一致)。
        let l = layout(1440.0, 900.0, &ids, 0, 5, &WINDOWS_METRICS).expect("合法输入");
        assert_eq!((l.strip_x, l.strip_y, l.strip_h), (1368.0, 232.0, 436.0));
        for (row, item) in l.items.iter().enumerate() {
            assert_eq!(item.y, 232.0 + row as f64 * 44.0);
        }
        // 奇数差: 旧 Windows i32 ((h - total_h) / 2) 截断 = floor;adapter
        // y as i32 截断后必须与旧值一致(465 / 2 = 232)。
        let l = layout(1440.0, 901.0, &ids, 0, 5, &WINDOWS_METRICS).expect("合法输入");
        assert_eq!(l.strip_y, 232.5);
        assert_eq!(l.strip_y as i32, 232);
        assert_eq!(l.items[1].y as i32, 232 + 44);
    }

    #[test]
    fn layout_windows_clamps_min_top_on_short_view() {
        let ids = ids_range(1, 10); // total_h = 436 > view_h
        let l = layout(200.0, 100.0, &ids, 0, 1, &WINDOWS_METRICS).expect("合法输入");
        assert_eq!(l.strip_y, WINDOWS_MIN_TOP);
        assert_eq!(l.items[0].y, WINDOWS_MIN_TOP);
        assert_eq!(l.strip_x, 200.0 - ITEM_W - MARGIN_RIGHT);
    }

    #[test]
    fn layout_empty_ids_is_valid_empty_strip() {
        let l = layout(800.0, 600.0, &[], 0, 0, &MACOS_METRICS).expect("空 ids 合法");
        assert!(l.items.is_empty());
        assert_eq!(
            (l.strip_x, l.strip_w, l.strip_h),
            (800.0 - 64.0 - 8.0, 64.0, 0.0)
        );
        // Windows 空条带: 居中公式分母仍是 total_h = 0 → (600-0)/2 = 300。
        let l = layout(800.0, 600.0, &[], 0, 0, &WINDOWS_METRICS).expect("空 ids 合法");
        assert_eq!(l.strip_y, 300.0);
    }

    #[test]
    fn layout_current_flag_and_label_use_global_index() {
        let ids = ids_range(11, 20); // start = 10
        let l = layout(800.0, 600.0, &ids, 10, 13, &MACOS_METRICS).expect("合法输入");
        assert_eq!(l.items[0].label, "11");
        assert_eq!(l.items[0].index, 10);
        assert_eq!(l.items[2].label, "13");
        assert!(l.items[2].current);
        // 当前页不在窗口里(理论上 adapter 已早退): 全部 false, 不报错。
        let l = layout(800.0, 600.0, &ids, 10, 99, &MACOS_METRICS).expect("合法输入");
        assert!(l.items.iter().all(|i| !i.current));
    }

    #[test]
    fn layout_rejects_invalid_sizes_and_metrics() {
        let ids = ids_range(1, 3);
        for (w, h) in [
            (0.0, 600.0),
            (-1.0, 600.0),
            (f64::NAN, 600.0),
            (800.0, f64::INFINITY),
        ] {
            assert_eq!(
                layout(w, h, &ids, 0, 1, &MACOS_METRICS).unwrap_err(),
                MinimapError::ViewSize
            );
        }
        let bad = [
            MinimapMetrics {
                item_w: 0.0,
                ..MACOS_METRICS
            },
            MinimapMetrics {
                item_h: f64::NAN,
                ..MACOS_METRICS
            },
            MinimapMetrics {
                gap: -1.0,
                ..MACOS_METRICS
            },
            MinimapMetrics {
                margin_right: f64::INFINITY,
                ..MACOS_METRICS
            },
            MinimapMetrics {
                placement: VerticalPlacement::Top { margin: f64::NAN },
                ..MACOS_METRICS
            },
            MinimapMetrics {
                placement: VerticalPlacement::Centered { min_top: f64::NAN },
                ..MACOS_METRICS
            },
        ];
        for metrics in bad {
            assert_eq!(
                layout(800.0, 600.0, &ids, 0, 1, &metrics).unwrap_err(),
                MinimapError::Metrics
            );
        }
        // 错误文案非空(FFI error JSON 用)。
        assert!(!MinimapError::ViewSize.to_string().is_empty());
        assert!(!MinimapError::Metrics.to_string().is_empty());
    }

    // ── D7: 两端 adapter minimap 残留闸门 ──
    //
    // core::minimap 是页窗(取哪些页/顺序)与条带几何(卡片位置/高度/页号)的
    // 唯一来源: macOS 经 export::glaspen2_minimap_layout_json FFI 拿布局 JSON,
    // Windows 同进程直接 minimap::page_window/layout。本闸门按文本扫描两端
    // adapter, 两类断言:
    // 1) **必须**出现 core minimap 入口 token(接线被退回/删调用即红);
    // 2) 旧页窗/条带几何的"指纹"**不得**出现(把公式抄回 adapter 即红)。
    //
    // 扫描范围 = 真正算页窗/画条带的文件:
    //   macOS   glaspen2.m(FFI 声明 + minimap 全局缓存) + parts/run.m(绘制);
    //   Windows overlay/{actions,state}.rs(actions = 页窗/布局/绘制, state =
    //   开关状态)。
    // 行注释剥掉再做匹配(注释里允许出现旧公式说明)。
    #[test]
    fn adapter_minimap_uses_core() {
        fn code_lines(src: &str) -> String {
            src.lines()
                .map(|line| match line.find("//") {
                    Some(at) => &line[..at],
                    None => line,
                })
                .collect::<Vec<_>>()
                .join("\n")
        }

        fn violations(name: &str, src: &str, forbidden: &[&str]) -> Vec<String> {
            let code = code_lines(src);
            forbidden
                .iter()
                .filter_map(|pat| {
                    code.find(pat).map(|at| {
                        format!(
                            "{name}:{} 仍含行内 minimap 页窗/条带几何 {:?}",
                            code[..at].matches('\n').count() + 1,
                            pat
                        )
                    })
                })
                .collect()
        }

        fn must_contain(name: &str, src: &str, tokens: &[&str]) -> Vec<String> {
            tokens
                .iter()
                .filter(|token| !src.contains(**token))
                .map(|token| format!("{name} 缺少 core minimap 入口 {token:?}"))
                .collect()
        }

        let mac_main = concat!(
            include_str!("../../../src/macos/swift/rust_ffi.h"),
            include_str!("../../../src/macos/swift/run_glue.swift")
        );
        let mac_run = concat!(
            include_str!("../../../src/macos/swift/run_glue.swift"),
            include_str!("../../../src/macos/swift/minimap.swift")
        );
        let mac_minimap_swift = include_str!("../../../src/macos/swift/minimap.swift");
        let mac_run_minimap = format!("{mac_run}\n{mac_minimap_swift}");
        let win_actions = include_str!("../../../crates/glaspen-windows/src/overlay/actions.rs");
        let win_state = include_str!("../../../crates/glaspen-windows/src/overlay/state.rs");
        // include_str! 指错文件/文件被清空时闸门不能静默变绿。
        for (name, src) in [
            ("glaspen2.m", mac_main),
            ("run.m", mac_run),
            ("actions.rs", win_actions),
            ("state.rs", win_state),
        ] {
            assert!(
                src.len() > 500,
                "闸门扫到的 {name} 过短, include_str! 路径可疑"
            );
        }

        // macOS 旧页窗/条带几何指纹(旧 run.m draw_minimap + glaspen2.m 全局量)。
        let mac_forbidden = [
            // 页窗: 旧本地过滤/切片(5/10 字面量)
            "g_minimap_ids",
            "glaspen2_list_screens_json(",
            "intValue] != g_pageview_pw",
            "idx > 5",
            "MIN(start + 10",
            // 条带: 旧卡片常量 + y/gap 公式 + 贴图位置
            "const CGFloat tw",
            "th = 40",
            "gap = 4",
            "(n - 1 - row) * (th + gap)",
            "bounds.size.width - tw - 8",
            "bounds.size.height - total_h - 8",
            "for (NSUInteger i = start; i < end; i++)",
        ];

        // Windows 旧页窗/条带几何指纹(旧 actions.rs minimap_* 区块)。
        let win_forbidden = [
            "MM_GAP",
            "MM_TW",
            "MM_TH",
            "minimap_page_ids",
            "MINIMAP_IDS",
            "glaspen2_list_screens_json(",
            "idx.saturating_sub(5)",
            "(start + 10)",
            "let total_h =",
            "ids[start..end]",
            "let y0 = ((h - total_h) / 2)",
        ];
        // state.rs 只放开关状态(D1/E3 已单源), 不该出现任何页窗/条带常量。
        let win_state_forbidden = ["MM_GAP", "MM_TW", "MM_TH", "minimap_page_ids"];

        let mut problems = Vec::new();
        problems.extend(violations("run.m", mac_run, &mac_forbidden));
        problems.extend(violations("glaspen2.m", mac_main, &mac_forbidden));
        problems.extend(violations("actions.rs", win_actions, &win_forbidden));
        problems.extend(violations("state.rs", win_state, &win_state_forbidden));

        // 必须接 core 入口(mac 经 FFI / Windows 直接调 minimap)。
        for (name, src, tokens) in [
            (
                "glaspen2.m",
                mac_main,
                &["extern char* glaspen2_minimap_layout_json("][..],
            ),
            (
                "run.m+minimap.swift",
                mac_run_minimap.as_str(),
                &["glaspen2_minimap_layout_json(", "layoutFromJSON("][..],
            ),
            (
                "actions.rs",
                win_actions,
                &[
                    "glaspen_core::minimap::page_window(",
                    "glaspen_core::minimap::layout(",
                    "minimap::WINDOWS_METRICS",
                    "minimap_pump(",
                ][..],
            ),
            ("state.rs", win_state, &["MINIMAP_ENABLED"][..]),
        ] {
            problems.extend(must_contain(name, src, tokens));
        }

        assert!(
            problems.is_empty(),
            "D7 minimap 页窗/布局接线闸门失败(两端必须只用 core::minimap):\n{}",
            problems.join("\n")
        );
    }
}
