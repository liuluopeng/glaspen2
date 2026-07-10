// ---------------------------------------------------------------------------
// Shared across both platforms
// ---------------------------------------------------------------------------

pub struct StrokeData {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub width_scale: f64,
    pub points: Vec<(f64, f64, f64, f64)>, // (x, y, width, relative_time)
}

/// A single recognized text line with its bounding box.
#[derive(Debug, Clone)]
pub struct OcrBox {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub confidence: f32,
}

/// Full OCR result for a screen.
#[derive(Debug, Clone)]
pub struct OcrResult {
    pub id: i64,
    pub screen_id: i64,
    pub full_text: String,
    pub boxes: Vec<OcrBox>,
    pub created_at: f64,
}

fn db_path() -> std::path::PathBuf {
    let exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let exe_dir = exe.parent().unwrap_or_else(|| std::path::Path::new("."));

    let is_bundled = exe_dir
        .ancestors()
        .any(|a| a.join("Contents").join("Info.plist").exists());

    if is_bundled {
        if let Some(app_support) = app_support_dir() {
            std::fs::create_dir_all(&app_support).ok();
            return app_support.join("glaspen2.db");
        }
    }

    exe_dir.join("glaspen2.db")
}

fn app_support_dir() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").ok()?;
        Some(std::path::PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("glaspen2"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

fn now_f64() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

// ---------------------------------------------------------------------------
// macOS + Windows: async sqlx
// ---------------------------------------------------------------------------
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod platform {
    use std::sync::OnceLock;
    use sqlx::SqlitePool;
    use crate::state;

    use super::{db_path, now_f64, StrokeData};

    static DB: OnceLock<SqlitePool> = OnceLock::new();

    pub async fn init() {
        let path = db_path();
        let pool = SqlitePool::connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true),
        ).await.expect("Failed to open glaspen2.db");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS screens (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at REAL NOT NULL,
                screen_w INTEGER NOT NULL,
                screen_h INTEGER NOT NULL
            )"
        ).execute(&pool).await.expect("Failed to create screens table");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS strokes (
                id INTEGER PRIMARY KEY,
                screen_id INTEGER NOT NULL REFERENCES screens(id),
                color_r REAL NOT NULL,
                color_g REAL NOT NULL,
                color_b REAL NOT NULL,
                width_scale REAL NOT NULL DEFAULT 1.0,
                created_at REAL NOT NULL
            )"
        ).execute(&pool).await.expect("Failed to create strokes table");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS points (
                stroke_id INTEGER NOT NULL REFERENCES strokes(id),
                seq INTEGER NOT NULL,
                x REAL NOT NULL,
                y REAL NOT NULL,
                width REAL NOT NULL,
                t REAL NOT NULL DEFAULT 0.0,
                PRIMARY KEY (stroke_id, seq)
            )"
        ).execute(&pool).await.expect("Failed to create points table");

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_strokes_screen ON strokes(screen_id)"
        ).execute(&pool).await.ok();

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS user_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )"
        ).execute(&pool).await.expect("Failed to create user_settings table");

        sqlx::query("ALTER TABLE points ADD COLUMN t REAL NOT NULL DEFAULT 0.0")
            .execute(&pool).await.ok();

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS ocr_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                screen_id INTEGER NOT NULL REFERENCES screens(id),
                full_text TEXT NOT NULL,
                created_at REAL NOT NULL
            )"
        ).execute(&pool).await.expect("Failed to create ocr_results table");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS ocr_boxes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                result_id INTEGER NOT NULL REFERENCES ocr_results(id),
                box_index INTEGER NOT NULL,
                text TEXT NOT NULL,
                x REAL NOT NULL, y REAL NOT NULL,
                w REAL NOT NULL, h REAL NOT NULL,
                confidence REAL NOT NULL DEFAULT 0.0
            )"
        ).execute(&pool).await.expect("Failed to create ocr_boxes table");

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_ocr_results_screen ON ocr_results(screen_id)"
        ).execute(&pool).await.ok();

        apply_defaults(&pool).await;

        DB.set(pool).ok();
        println!("[glaspen2] DB initialized at {}", path.display());
    }

    async fn apply_defaults(pool: &SqlitePool) {
        let defaults = [
            ("pen_r", "1.0"), ("pen_g", "0.0"), ("pen_b", "0.0"),
            ("width_scale", "1.0"), ("glass_alpha", "0"), ("glass_enabled", "0"),
        ];
        for &(key, val) in &defaults {
            sqlx::query("INSERT OR IGNORE INTO user_settings (key, value) VALUES (?1, ?2)")
                .bind(key).bind(val).execute(pool).await.ok();
        }
    }

    pub async fn new_screen(screen_w: i32, screen_h: i32) {
        let pool = DB.get().expect("DB not initialized");
        let now = now_f64();
        let sid = sqlx::query_scalar::<_, i64>(
            "INSERT INTO screens (created_at, screen_w, screen_h) VALUES (?1, ?2, ?3) RETURNING id"
        ).bind(now).bind(screen_w).bind(screen_h)
            .fetch_one(pool).await.unwrap_or(0);
        state::set_current_screen_id(sid);
    }

    pub async fn begin_stroke(r: f64, g: f64, b: f64, width_scale: f64) {
        use crate::state;
        let pool = DB.get().expect("DB not initialized");
        let screen_id = state::current_screen_id();
        let now = now_f64();
        let stroke_id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO strokes (screen_id, color_r, color_g, color_b, width_scale, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6) RETURNING id"
        ).bind(screen_id).bind(r).bind(g).bind(b).bind(width_scale).bind(now)
            .fetch_optional(pool).await;
        if let Ok(Some(stroke_id)) = stroke_id {
            state::begin_pending(stroke_id);
        }
    }

    pub async fn end_stroke() {
        flush_pending().await;
    }

    async fn flush_pending() {
        use crate::state;
        let stroke_id = match state::take_pending_stroke_id() { Some(id) => id, None => return };
        let points = state::take_pending();
        if points.is_empty() { return; }
        let pool = DB.get().expect("DB not initialized");
        let mut tx = match pool.begin().await { Ok(t) => t, Err(_) => return };
        for (i, &(x, y, w, t)) in points.iter().enumerate() {
            sqlx::query(
                "INSERT INTO points (stroke_id, seq, x, y, width, t) VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
            ).bind(stroke_id).bind(i as i64).bind(x).bind(y).bind(w).bind(t)
                .execute(&mut *tx).await.ok();
        }
        tx.commit().await.ok();
    }

    pub fn end_stroke_spawned() {
        let rt = crate::runtime();
        let _ = rt.spawn(async { flush_pending().await; });
    }

    pub async fn screen_has_strokes(screen_id: i64) -> bool {
        let pool = match DB.get() { Some(p) => p, None => return false };
        sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM strokes WHERE screen_id = ?1)"
        ).bind(screen_id).fetch_one(pool).await.unwrap_or(0) != 0
    }

    pub async fn delete_last_stroke() -> bool {
        use crate::state;
        let pool = match DB.get() { Some(p) => p, None => return false };
        let screen_id = state::current_screen_id();
        let stroke_id = match sqlx::query_scalar::<_, i64>(
            "SELECT id FROM strokes WHERE screen_id = ?1 ORDER BY id DESC LIMIT 1"
        ).bind(screen_id).fetch_optional(pool).await {
            Ok(Some(id)) => id, _ => return false,
        };
        sqlx::query("DELETE FROM points WHERE stroke_id = ?1").bind(stroke_id).execute(pool).await.ok();
        sqlx::query("DELETE FROM strokes WHERE id = ?1").bind(stroke_id).execute(pool).await.ok();
        true
    }

    pub async fn prev_screen(current: i64) -> Option<i64> {
        let pool = DB.get()?;
        sqlx::query_scalar::<_, i64>(
            "SELECT s.id FROM screens s WHERE s.id < ?1 AND EXISTS (SELECT 1 FROM strokes WHERE screen_id = s.id) ORDER BY s.id DESC LIMIT 1"
        ).bind(current).fetch_optional(pool).await.ok()?
    }

    pub async fn next_screen(current: i64) -> Option<i64> {
        let pool = DB.get()?;
        sqlx::query_scalar::<_, i64>(
            "SELECT s.id FROM screens s WHERE s.id > ?1 AND EXISTS (SELECT 1 FROM strokes WHERE screen_id = s.id) ORDER BY s.id ASC LIMIT 1"
        ).bind(current).fetch_optional(pool).await.ok()?
    }

    pub async fn strokes_for_screen(screen_id: i64) -> Vec<StrokeData> {
        let pool = match DB.get() { Some(p) => p, None => return Vec::new() };
        let rows: Vec<(i64, f64, f64, f64, f64)> = sqlx::query_as(
            "SELECT id, color_r, color_g, color_b, width_scale FROM strokes WHERE screen_id = ?1 ORDER BY id"
        ).bind(screen_id).fetch_all(pool).await.unwrap_or_default();
        let mut strokes = Vec::new();
        for (stroke_id, r, g, b, width_scale) in rows {
            let points: Vec<(f64,f64,f64,f64)> = sqlx::query_as(
                "SELECT x, y, width, t FROM points WHERE stroke_id = ?1 ORDER BY seq"
            ).bind(stroke_id).fetch_all(pool).await.unwrap_or_default();
            strokes.push(StrokeData { r, g, b, width_scale, points });
        }
        strokes
    }

    pub async fn save_setting(key: &str, value: &str) {
        let pool = match DB.get() { Some(p) => p, None => return };
        sqlx::query("INSERT OR REPLACE INTO user_settings (key, value) VALUES (?1, ?2)")
            .bind(key).bind(value).execute(pool).await.ok();
    }

    pub async fn load_setting(key: &str) -> Option<String> {
        let pool = DB.get()?;
        sqlx::query_scalar::<_, String>(
            "SELECT value FROM user_settings WHERE key = ?1"
        ).bind(key).fetch_optional(pool).await.ok()?
    }

    pub async fn save_settings(pen_r: f64, pen_g: f64, pen_b: f64, width_scale: f64) {
        let pool = match DB.get() { Some(p) => p, None => return };
        for &(k, v) in &[("pen_r", pen_r), ("pen_g", pen_g), ("pen_b", pen_b), ("width_scale", width_scale)] {
            sqlx::query("INSERT OR REPLACE INTO user_settings (key, value) VALUES (?1, ?2)")
                .bind(k).bind(format!("{:.6}", v)).execute(pool).await.ok();
        }
    }

    pub async fn load_settings() -> Option<(f64, f64, f64, f64)> {
        let pool = DB.get()?;
        let r: f64 = sqlx::query_scalar::<_, String>("SELECT value FROM user_settings WHERE key = 'pen_r'")
            .fetch_optional(pool).await.ok()??.parse().ok()?;
        let g: f64 = sqlx::query_scalar::<_, String>("SELECT value FROM user_settings WHERE key = 'pen_g'")
            .fetch_optional(pool).await.ok()??.parse().ok()?;
        let b: f64 = sqlx::query_scalar::<_, String>("SELECT value FROM user_settings WHERE key = 'pen_b'")
            .fetch_optional(pool).await.ok()??.parse().ok()?;
        let ws: f64 = sqlx::query_scalar::<_, String>("SELECT value FROM user_settings WHERE key = 'width_scale'")
            .fetch_optional(pool).await.ok()??.parse().ok()?;
        Some((r, g, b, ws))
    }

    pub async fn save_ocr_result(
        screen_id: i64, full_text: &str, boxes: &[super::OcrBox],
    ) {
        let pool = match DB.get() { Some(p) => p, None => return };
        let now = super::now_f64();
        let result_id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO ocr_results (screen_id, full_text, created_at) VALUES (?1, ?2, ?3) RETURNING id"
        ).bind(screen_id).bind(full_text).bind(now)
            .fetch_optional(pool).await;
        let Ok(Some(rid)) = result_id else { return };
        for (i, b) in boxes.iter().enumerate() {
            sqlx::query(
                "INSERT INTO ocr_boxes (result_id, box_index, text, x, y, w, h, confidence) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)"
            ).bind(rid).bind(i as i64).bind(&b.text)
                .bind(b.x).bind(b.y).bind(b.w).bind(b.h).bind(b.confidence as f64)
                .execute(pool).await.ok();
        }
    }

    pub async fn load_latest_ocr(screen_id: i64) -> Option<super::OcrResult> {
        let pool = DB.get()?;
        let row = sqlx::query_as::<_, (i64, String, f64)>(
            "SELECT id, full_text, created_at FROM ocr_results WHERE screen_id = ?1 ORDER BY id DESC LIMIT 1"
        ).bind(screen_id).fetch_optional(pool).await.ok()??;
        let boxes: Vec<(i64, String, f64, f64, f64, f64, f64)> = sqlx::query_as(
            "SELECT box_index, text, x, y, w, h, confidence FROM ocr_boxes WHERE result_id = ?1 ORDER BY box_index"
        ).bind(row.0).fetch_all(pool).await.unwrap_or_default();
        let ocr_boxes: Vec<super::OcrBox> = boxes.into_iter().map(|(_, t, x, y, w, h, c)| {
            super::OcrBox { text: t, x, y, w, h, confidence: c as f32 }
        }).collect();
        Some(super::OcrResult {
            id: row.0, screen_id,
            full_text: row.1, boxes: ocr_boxes,
            created_at: row.2,
        })
    }
}

pub use platform::*;

