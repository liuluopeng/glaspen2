use std::sync::Mutex;
use std::sync::OnceLock;
use sqlx::SqlitePool;

static DB: OnceLock<SqlitePool> = OnceLock::new();
static CURRENT_SCREEN_ID: Mutex<i64> = Mutex::new(0);
static PENDING_STROKE_ID: Mutex<Option<i64>> = Mutex::new(None);
static PENDING_POINTS: Mutex<Vec<(f64, f64, f64, f64)>> = Mutex::new(Vec::new()); // (x, y, width, relative_time)

fn db_path() -> std::path::PathBuf {
    let exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let exe_dir = exe.parent().unwrap_or_else(|| std::path::Path::new("."));

    // Detect app bundle by finding Info.plist at Contents/Info.plist above the binary.
    let is_bundled = exe_dir
        .ancestors()
        .any(|a| a.join("Contents").join("Info.plist").exists());

    if is_bundled {
        if let Some(app_support) = app_support_dir() {
            std::fs::create_dir_all(&app_support).ok();
            return app_support.join("glaspen2.db");
        }
    }

    // Dev / cargo run: DB next to the binary
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

/// Initialize the database. Call once at app start.
pub async fn init() {
    let path = db_path();

    // Create pool with a single connection for simplicity
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

    // Migration: add t column to existing points table
    sqlx::query("ALTER TABLE points ADD COLUMN t REAL NOT NULL DEFAULT 0.0")
        .execute(&pool).await.ok();

    // Apply defaults for missing settings
    apply_defaults(&pool).await;

    DB.set(pool).ok();
    println!("[glaspen2] DB initialized at {}", path.display());
}

fn now_f64() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// Create a new screen record. Called on app start (via init) and on clear.
pub async fn new_screen(screen_w: i32, screen_h: i32) {
    let pool = DB.get().expect("DB not initialized");
    let now = now_f64();
    sqlx::query("INSERT INTO screens (created_at, screen_w, screen_h) VALUES (?1, ?2, ?3)")
        .bind(now).bind(screen_w).bind(screen_h)
        .execute(pool).await.ok();
    let sid = sqlx::query_scalar::<_, i64>("SELECT last_insert_rowid()")
        .fetch_one(pool).await.unwrap_or(0);
    *CURRENT_SCREEN_ID.lock().unwrap() = sid;
}

/// Begin a stroke: insert a row, store id for point buffering.
pub async fn begin_stroke(r: f64, g: f64, b: f64, width_scale: f64) {
    // Flush any unflushed stroke first
    flush_pending().await;

    let pool = DB.get().expect("DB not initialized");
    let screen_id = *CURRENT_SCREEN_ID.lock().unwrap();
    let now = now_f64();
    let res = sqlx::query(
        "INSERT INTO strokes (screen_id, color_r, color_g, color_b, width_scale, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
    ).bind(screen_id).bind(r).bind(g).bind(b).bind(width_scale).bind(now)
        .execute(pool).await;
    if let Ok(_) = res {
        let stroke_id = sqlx::query_scalar::<_, i64>("SELECT last_insert_rowid()")
            .fetch_one(pool).await.unwrap_or(0);
        *PENDING_STROKE_ID.lock().unwrap() = Some(stroke_id);
        PENDING_POINTS.lock().unwrap().clear();
    }
}

/// Buffer a point. Will be flushed to DB on end_stroke.
/// This is NOT async — pure memory operation (Mutex<Vec> push).
pub fn add_point(x: f64, y: f64, width: f64, t: f64) {
    PENDING_POINTS.lock().unwrap().push((x, y, width, t));
}

/// Flush buffered points to DB.
pub async fn end_stroke() {
    flush_pending().await;
}

/// Flush pending points for the current stroke to the database.
async fn flush_pending() {
    let stroke_id = {
        let mut pending = PENDING_STROKE_ID.lock().unwrap();
        match pending.take() {
            Some(id) => id,
            None => return,
        }
    };

    let points: Vec<(f64, f64, f64, f64)> = {
        let mut p = PENDING_POINTS.lock().unwrap();
        std::mem::take(&mut *p)
    };

    if points.is_empty() {
        return;
    }

    let pool = DB.get().expect("DB not initialized");
    let mut tx = pool.begin().await.ok();
    for (i, &(x, y, w, t)) in points.iter().enumerate() {
        sqlx::query(
            "INSERT INTO points (stroke_id, seq, x, y, width, t) VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
        ).bind(stroke_id).bind(i as i64).bind(x).bind(y).bind(w).bind(t)
            .execute(&*pool).await.ok();
    }
    if let Some(tx) = tx {
        tx.commit().await.ok();
    }
}

pub fn current_screen() -> i64 {
    *CURRENT_SCREEN_ID.lock().unwrap()
}

pub fn set_current_screen(id: i64) {
    *CURRENT_SCREEN_ID.lock().unwrap() = id;
}

/// Check if a screen has any strokes.
pub async fn screen_has_strokes(screen_id: i64) -> bool {
    let pool = match DB.get() { Some(p) => p, None => return false };
    sqlx::query_scalar::<_, i64>(
        "SELECT EXISTS(SELECT 1 FROM strokes WHERE screen_id = ?1)"
    ).bind(screen_id).fetch_one(pool).await.unwrap_or(0) != 0
}

/// Delete the last stroke on the current screen from the database.
/// Returns true if a stroke was found and deleted.
pub async fn delete_last_stroke() -> bool {
    let pool = match DB.get() { Some(p) => p, None => return false };
    let screen_id = *CURRENT_SCREEN_ID.lock().unwrap();

    let stroke_id = match sqlx::query_scalar::<_, i64>(
        "SELECT id FROM strokes WHERE screen_id = ?1 ORDER BY id DESC LIMIT 1"
    ).bind(screen_id).fetch_optional(pool).await {
        Ok(Some(id)) => id,
        _ => return false,
    };

    sqlx::query("DELETE FROM points WHERE stroke_id = ?1")
        .bind(stroke_id).execute(pool).await.ok();
    sqlx::query("DELETE FROM strokes WHERE id = ?1")
        .bind(stroke_id).execute(pool).await.ok();
    true
}

/// Get the previous screen id (lower id with strokes), or None.
pub async fn prev_screen(current: i64) -> Option<i64> {
    let pool = DB.get()?;
    sqlx::query_scalar::<_, i64>(
        "SELECT s.id FROM screens s WHERE s.id < ?1 AND EXISTS (SELECT 1 FROM strokes WHERE screen_id = s.id) ORDER BY s.id DESC LIMIT 1"
    ).bind(current).fetch_optional(pool).await.ok()?
}

/// Get the next screen id (higher id with strokes), or None.
pub async fn next_screen(current: i64) -> Option<i64> {
    let pool = DB.get()?;
    sqlx::query_scalar::<_, i64>(
        "SELECT s.id FROM screens s WHERE s.id > ?1 AND EXISTS (SELECT 1 FROM strokes WHERE screen_id = s.id) ORDER BY s.id ASC LIMIT 1"
    ).bind(current).fetch_optional(pool).await.ok()?
}

pub struct StrokeData {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub width_scale: f64,
    pub points: Vec<(f64, f64, f64, f64)>, // (x, y, width, relative_time)
}

/// Load all strokes for a screen into the STROKES vec.
pub async fn strokes_for_screen(screen_id: i64) -> Vec<StrokeData> {
    let pool = match DB.get() { Some(p) => p, None => return Vec::new() };

    type Row = (i64, f64, f64, f64, f64);
    let rows: Vec<(i64, f64, f64, f64, f64)> = sqlx::query_as::<_, Row>(
        "SELECT id, color_r, color_g, color_b, width_scale FROM strokes WHERE screen_id = ?1 ORDER BY id"
    ).bind(screen_id).fetch_all(pool).await.unwrap_or_default();

    let mut strokes = Vec::new();
    for (stroke_id, r, g, b, width_scale) in rows {
        type Pt = (f64, f64, f64, f64);
        let points: Vec<Pt> = sqlx::query_as::<_, Pt>(
            "SELECT x, y, width, t FROM points WHERE stroke_id = ?1 ORDER BY seq"
        ).bind(stroke_id).fetch_all(pool).await.unwrap_or_default();
        strokes.push(StrokeData { r, g, b, width_scale, points });
    }
    strokes
}

// --- User settings ---

async fn apply_defaults(pool: &SqlitePool) {
    let defaults = [
        ("pen_r", "1.0"),
        ("pen_g", "0.0"),
        ("pen_b", "0.0"),
        ("width_scale", "1.0"),
        ("outline_enabled", "0"),
        ("inverse_enabled", "0"),
        ("glass_alpha", "0"),
        ("glass_enabled", "0"),
    ];
    for &(key, val) in &defaults {
        sqlx::query("INSERT OR IGNORE INTO user_settings (key, value) VALUES (?1, ?2)")
            .bind(key).bind(val)
            .execute(pool).await.ok();
    }
}

pub async fn save_setting(key: &str, value: &str) {
    let pool = match DB.get() { Some(p) => p, None => return };
    sqlx::query("INSERT OR REPLACE INTO user_settings (key, value) VALUES (?1, ?2)")
        .bind(key).bind(value)
        .execute(pool).await.ok();
}

pub async fn load_setting(key: &str) -> Option<String> {
    let pool = DB.get()?;
    sqlx::query_scalar::<_, String>(
        "SELECT value FROM user_settings WHERE key = ?1"
    ).bind(key).fetch_optional(pool).await.ok()?
}

pub async fn save_settings(pen_r: f64, pen_g: f64, pen_b: f64, width_scale: f64) {
    let pool = match DB.get() { Some(p) => p, None => return };
    let pairs = [
        ("pen_r", pen_r),
        ("pen_g", pen_g),
        ("pen_b", pen_b),
        ("width_scale", width_scale),
    ];
    for &(key, val) in &pairs {
        sqlx::query("INSERT OR REPLACE INTO user_settings (key, value) VALUES (?1, ?2)")
            .bind(key).bind(format!("{:.6}", val))
            .execute(pool).await.ok();
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
