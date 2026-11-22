//! 页信息 / 日期文案的唯一实现。
//!
//! "今天 第2页  第3/5页" 这套文案与日期比较曾在 macOS(ObjC) 与 Windows(Rust)
//! 各写一份(D4)。现在只在这里写: macOS 通过 FFI `glaspen2_page_info_text`,
//! Windows 直接调 [page_info_text]。i18n 全量下沉是 D10, 这里先把页信息收口。

use chrono::{Local, TimeZone};

fn local_date(ts: i64) -> chrono::NaiveDate {
    let tz = Local;
    tz.timestamp_opt(ts, 0)
        .single()
        .or_else(|| tz.timestamp_opt(ts, 0).earliest())
        .map(|d| d.date_naive())
        .unwrap_or_else(|| Local::now().date_naive())
}

/// 日期标签: 今天 / 昨天 / YYYY-MM-DD(按本地时区, 与系统日历一致)。
pub fn date_label_at(created: i64, now: i64, lang: &str) -> String {
    let en = lang == "en";
    let cd = local_date(created);
    let nd = local_date(now);
    if cd == nd {
        return if en { "Today" } else { "今天" }.to_string();
    }
    if nd.signed_duration_since(cd).num_days() == 1 {
        return if en { "Yesterday" } else { "昨天" }.to_string();
    }
    cd.format("%Y-%m-%d").to_string()
}

/// 页信息文案(纯函数, 便于两端共享与测试)。
/// zh: `今天 第2页  第3/5页`;en: `Today page 2 (3/5)`。
pub fn page_info_text_at(
    nth: i64,
    pos: i64,
    total: i64,
    created: i64,
    now: i64,
    lang: &str,
) -> String {
    let label = date_label_at(created, now, lang);
    if lang == "en" {
        format!("{label} page {nth} ({pos}/{total})")
    } else {
        format!("{label} 第{nth}页  第{pos}/{total}页")
    }
}

/// 读取某页的页信息并格式化。页不存在 → None。
/// 内部与既有 FFI 一样用 block_on: 调用方(mac ObjC 主线程 / Windows 管道线程)
/// 本来就不在 tokio 运行时里。
pub fn page_info_text(screen_id: i64, lang: &str) -> Option<String> {
    let (nth, _date_total, pos, total, created) =
        crate::runtime().block_on(crate::db::page_info(screen_id))?;
    Some(page_info_text_at(
        nth,
        pos,
        total,
        created as i64,
        Local::now().timestamp(),
        lang,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(y: i32, m: u32, d: u32) -> i64 {
        Local
            .with_ymd_and_hms(y, m, d, 12, 0, 0)
            .single()
            .expect("本地时间有效")
            .timestamp()
    }

    #[test]
    fn date_labels_zh_and_en() {
        let today = ts(2026, 11, 22);
        let yesterday = ts(2026, 11, 21);
        let older = ts(2026, 10, 1);
        assert_eq!(date_label_at(today, today, "zh"), "今天");
        assert_eq!(date_label_at(yesterday, today, "zh"), "昨天");
        assert_eq!(date_label_at(older, today, "zh"), "2026-10-01");
        assert_eq!(date_label_at(today, today, "en"), "Today");
        assert_eq!(date_label_at(yesterday, today, "en"), "Yesterday");
        assert_eq!(date_label_at(older, today, "en"), "2026-10-01");
    }

    #[test]
    fn date_boundary_uses_local_dates_not_elapsed_hours() {
        // 昨天 23:50 → 今天 00:10 只差 20 分钟, 但日期变了 → 昨天。
        let now = Local
            .with_ymd_and_hms(2026, 11, 22, 0, 10, 0)
            .single()
            .unwrap()
            .timestamp();
        let created = Local
            .with_ymd_and_hms(2026, 11, 21, 23, 50, 0)
            .single()
            .unwrap()
            .timestamp();
        assert_eq!(date_label_at(created, now, "zh"), "昨天");
        // 同一天跨 23 小时仍是今天。
        let created_same = Local
            .with_ymd_and_hms(2026, 11, 22, 23, 50, 0)
            .single()
            .unwrap()
            .timestamp();
        assert_eq!(date_label_at(created_same, now, "zh"), "今天");
    }

    #[test]
    fn page_text_format() {
        let today = ts(2026, 11, 22);
        assert_eq!(
            page_info_text_at(2, 3, 5, today, today, "zh"),
            "今天 第2页  第3/5页"
        );
        assert_eq!(
            page_info_text_at(2, 3, 5, today, today, "en"),
            "Today page 2 (3/5)"
        );
        let older = ts(2026, 10, 1);
        assert_eq!(
            page_info_text_at(1, 1, 1, older, today, "zh"),
            "2026-10-01 第1页  第1/1页"
        );
    }
}
