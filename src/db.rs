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
// macOS: async sqlx + separate state module
// ---------------------------------------------------------------------------
#[cfg(target_os = "macos")]
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
        // Flush any pending stroke first
        let _ = sqlx::query("DELETE FROM points WHERE stroke_id IN (SELECT id FROM strokes WHERE screen_id = ?1 AND id > ?2)")
            .bind(screen_id).bind(0).execute(pool).await;
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
}

// ---------------------------------------------------------------------------
// Windows: sync rusqlite (wrapped in async fn for API compatibility)
// ---------------------------------------------------------------------------
#[cfg(windows)]
mod platform {
    use std::sync::OnceLock;
    use std::sync::Mutex;
    use rusqlite::{params, Connection};

    use super::{db_path, now_f64, StrokeData};

    static DB: OnceLock<Mutex<Option<Connection>>> = OnceLock::new();

    fn db() -> &'static Mutex<Option<Connection>> {
        DB.get().expect("DB not initialized")
    }

    pub async fn init() {
        let path = db_path();
        let conn = Connection::open(&path).expect("Failed to open glaspen2.db");

        conn.execute_batch("
            CREATE TABLE IF NOT EXISTS screens (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at REAL NOT NULL,
                screen_w INTEGER NOT NULL, screen_h INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS strokes (
                id INTEGER PRIMARY KEY,
                screen_id INTEGER NOT NULL REFERENCES screens(id),
                color_r REAL NOT NULL, color_g REAL NOT NULL, color_b REAL NOT NULL,
                width_scale REAL NOT NULL DEFAULT 1.0, created_at REAL NOT NULL
            );
            CREATE TABLE IF NOT EXISTS points (
                stroke_id INTEGER NOT NULL REFERENCES strokes(id),
                seq INTEGER NOT NULL,
                x REAL NOT NULL, y REAL NOT NULL, width REAL NOT NULL, t REAL NOT NULL DEFAULT 0.0,
                PRIMARY KEY (stroke_id, seq)
            );
            CREATE INDEX IF NOT EXISTS idx_strokes_screen ON strokes(screen_id);
            CREATE TABLE IF NOT EXISTS user_settings (
                key TEXT PRIMARY KEY, value TEXT NOT NULL
            );
        ").expect("Failed to create tables");

        conn.execute_batch("ALTER TABLE points ADD COLUMN t REAL NOT NULL DEFAULT 0.0;").ok();

        apply_defaults(&conn);
        _ = DB.get_or_init(|| Mutex::new(Some(conn)));
        println!("[glaspen2] DB initialized at {}", path.display());
    }

    fn apply_defaults(conn: &Connection) {
        let defaults = [
            ("pen_r", "1.0"), ("pen_g", "0.0"), ("pen_b", "0.0"),
            ("width_scale", "1.0"), ("glass_alpha", "0"), ("glass_enabled", "0"),
        ];
        for &(key, val) in &defaults {
            conn.execute("INSERT OR IGNORE INTO user_settings (key, value) VALUES (?1, ?2)",
                params![key, val]).ok();
        }
    }

    pub async fn new_screen(screen_w: i32, screen_h: i32) {
        let db_lock = db().lock().unwrap();
        if let Some(ref conn) = *db_lock {
            let now = now_f64();
            conn.execute("INSERT INTO screens (created_at, screen_w, screen_h) VALUES (?1, ?2, ?3)",
                params![now, screen_w, screen_h]).ok();
            crate::state::set_current_screen_id(conn.last_insert_rowid());
        }
    }

    pub async fn begin_stroke(r: f64, g: f64, b: f64, width_scale: f64) {
        flush_pending_impl();
        let db_lock = db().lock().unwrap();
        if let Some(ref conn) = *db_lock {
            let screen_id = crate::state::current_screen_id();
            let now = now_f64();
            conn.execute(
                "INSERT INTO strokes (screen_id, color_r, color_g, color_b, width_scale, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![screen_id, r, g, b, width_scale, now],
            ).ok();
            crate::state::begin_pending(conn.last_insert_rowid());
        }
    }

    pub async fn end_stroke() { flush_pending_impl(); }

    pub fn end_stroke_spawned() { flush_pending_impl(); }

    fn flush_pending_impl() {
        let stroke_id = match crate::state::take_pending_stroke_id() { Some(id) => id, None => return };
        let points = crate::state::take_pending();
        if points.is_empty() { return; }
        let db_lock = db().lock().unwrap();
        if let Some(ref conn) = *db_lock {
            let tx = conn.unchecked_transaction().ok();
            for (i, &(x, y, w, t)) in points.iter().enumerate() {
                conn.execute(
                    "INSERT INTO points (stroke_id, seq, x, y, width, t) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![stroke_id, i as i64, x, y, w, t],
                ).ok();
            }
            if let Some(tx) = tx { tx.commit().ok(); }
        }
    }

    pub async fn screen_has_strokes(screen_id: i64) -> bool {
        let db_lock = db().lock().unwrap();
        if let Some(ref conn) = *db_lock {
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM strokes WHERE screen_id = ?1)",
                params![screen_id], |row| row.get::<_, bool>(0),
            ).unwrap_or(false)
        } else { false }
    }

    pub async fn delete_last_stroke() -> bool {
        let db_lock = db().lock().unwrap();
        if let Some(ref conn) = *db_lock {
            let screen_id = crate::state::current_screen_id();
            let last_id: Option<i64> = conn.query_row(
                "SELECT id FROM strokes WHERE screen_id = ?1 ORDER BY id DESC LIMIT 1",
                params![screen_id], |row| row.get(0),
            ).ok();
            if let Some(sid) = last_id {
                conn.execute("DELETE FROM points WHERE stroke_id = ?1", params![sid]).ok();
                conn.execute("DELETE FROM strokes WHERE id = ?1", params![sid]).ok();
                true
            } else { false }
        } else { false }
    }

    pub async fn prev_screen(current: i64) -> Option<i64> {
        let db_lock = db().lock().unwrap();
        let conn = db_lock.as_ref()?;
        conn.query_row(
            "SELECT s.id FROM screens s WHERE s.id < ?1 AND EXISTS (SELECT 1 FROM strokes WHERE screen_id = s.id) ORDER BY s.id DESC LIMIT 1",
            params![current], |row| row.get(0),
        ).ok()
    }

    pub async fn next_screen(current: i64) -> Option<i64> {
        let db_lock = db().lock().unwrap();
        let conn = db_lock.as_ref()?;
        conn.query_row(
            "SELECT s.id FROM screens s WHERE s.id > ?1 AND EXISTS (SELECT 1 FROM strokes WHERE screen_id = s.id) ORDER BY s.id ASC LIMIT 1",
            params![current], |row| row.get(0),
        ).ok()
    }

    pub async fn strokes_for_screen(screen_id: i64) -> Vec<StrokeData> {
        let db_lock = db().lock().unwrap();
        let conn = match db_lock.as_ref() { Some(c) => c, None => return Vec::new() };
        let mut stmt = conn.prepare(
            "SELECT id, color_r, color_g, color_b, width_scale FROM strokes WHERE screen_id = ?1 ORDER BY id"
        ).ok()?;
        let rows = stmt.query_map(params![screen_id], |row| {
            let sid: i64 = row.get(0)?;
            let r: f64 = row.get(1)?;
            let g: f64 = row.get(2)?;
            let b: f64 = row.get(3)?;
            let ws: f64 = row.get(4)?;
            Ok((sid, r, g, b, ws))
        }).ok()?;
        let mut strokes = Vec::new();
        for row in rows.flatten() {
            let (sid, r, g, b, ws) = row;
            let mut pt_stmt = conn.prepare(
                "SELECT x, y, width, t FROM points WHERE stroke_id = ?1 ORDER BY seq"
            ).ok();
            let points: Vec<(f64,f64,f64,f64)> = pt_stmt.and_then(|mut s| {
                s.query_map(params![sid], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
                }).ok().map(|i| i.flatten().collect())
            }).unwrap_or_default();
            strokes.push(StrokeData { r, g, b, width_scale: ws, points });
        }
        strokes
    }

    pub async fn save_setting(key: &str, value: &str) {
        let db_lock = db().lock().unwrap();
        if let Some(ref conn) = *db_lock {
            conn.execute("INSERT OR REPLACE INTO user_settings (key, value) VALUES (?1, ?2)",
                params![key, value]).ok();
        }
    }

    pub async fn load_setting(key: &str) -> Option<String> {
        let db_lock = db().lock().unwrap();
        let conn = db_lock.as_ref()?;
        conn.query_row("SELECT value FROM user_settings WHERE key = ?1",
            params![key], |row| row.get(0)).ok()
    }

    pub async fn save_settings(pen_r: f64, pen_g: f64, pen_b: f64, width_scale: f64) {
        let pairs = [("pen_r", pen_r), ("pen_g", pen_g), ("pen_b", pen_b), ("width_scale", width_scale)];
        let db_lock = db().lock().unwrap();
        if let Some(ref conn) = *db_lock {
            for &(k, v) in &pairs {
                conn.execute("INSERT OR REPLACE INTO user_settings (key, value) VALUES (?1, ?2)",
                    params![k, format!("{:.6}", v)]).ok();
            }
        }
    }

    pub async fn load_settings() -> Option<(f64, f64, f64, f64)> {
        let db_lock = db().lock().unwrap();
        let conn = db_lock.as_ref()?;
        let r: f64 = conn.query_row("SELECT value FROM user_settings WHERE key = 'pen_r'", [], |row| row.get(0)).ok()?;
        let g: f64 = conn.query_row("SELECT value FROM user_settings WHERE key = 'pen_g'", [], |row| row.get(0)).ok()?;
        let b: f64 = conn.query_row("SELECT value FROM user_settings WHERE key = 'pen_b'", [], |row| row.get(0)).ok()?;
        let ws: f64 = conn.query_row("SELECT value FROM user_settings WHERE key = 'width_scale'", [], |row| row.get(0)).ok()?;
        Some((r, g, b, ws))
    }
}

pub use platform::*;
