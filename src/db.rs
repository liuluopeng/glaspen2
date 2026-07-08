use std::sync::Mutex;
use rusqlite::{Connection, params};

static DB: Mutex<Option<Connection>> = Mutex::new(None);
static CURRENT_SCREEN_ID: Mutex<i64> = Mutex::new(0);
static PENDING_STROKE_ID: Mutex<Option<i64>> = Mutex::new(None);
static PENDING_POINTS: Mutex<Vec<(f64, f64, f64)>> = Mutex::new(Vec::new());

fn db_path() -> std::path::PathBuf {
    let exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let exe_dir = exe.parent().unwrap_or_else(|| std::path::Path::new("."));

    // Detect app bundle by finding Info.plist at Contents/Info.plist above the binary.
    // macOS bundle layout:  MyApp.app/Contents/MacOS/executable
    //                                    ↑              ↑ exe_dir
    //                           Info.plist
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
pub fn init() {
    let path = db_path();
    let conn = Connection::open(&path).expect("Failed to open glaspen2.db");

    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS screens (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at REAL NOT NULL,
            screen_w INTEGER NOT NULL,
            screen_h INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS strokes (
            id INTEGER PRIMARY KEY,
            screen_id INTEGER NOT NULL REFERENCES screens(id),
            color_r REAL NOT NULL,
            color_g REAL NOT NULL,
            color_b REAL NOT NULL,
            width_scale REAL NOT NULL DEFAULT 1.0,
            created_at REAL NOT NULL
        );
        CREATE TABLE IF NOT EXISTS points (
            stroke_id INTEGER NOT NULL REFERENCES strokes(id),
            seq INTEGER NOT NULL,
            x REAL NOT NULL,
            y REAL NOT NULL,
            width REAL NOT NULL,
            PRIMARY KEY (stroke_id, seq)
        );
        CREATE INDEX IF NOT EXISTS idx_strokes_screen ON strokes(screen_id);
        CREATE TABLE IF NOT EXISTS user_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
    ").expect("Failed to create tables");

    // Apply defaults for missing settings
    apply_defaults();

    *DB.lock().unwrap() = Some(conn);

    println!("[glaspen2] DB initialized at {}", path.display());
}

fn now_f64() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// Create a new screen record. Called on app start (via init) and on clear.
pub fn new_screen(screen_w: i32, screen_h: i32) {
    let db = DB.lock().unwrap();
    if let Some(ref conn) = *db {
        let now = now_f64();
        conn.execute(
            "INSERT INTO screens (created_at, screen_w, screen_h) VALUES (?1, ?2, ?3)",
            params![now, screen_w, screen_h],
        ).ok();
        let sid = conn.last_insert_rowid();
        *CURRENT_SCREEN_ID.lock().unwrap() = sid;
    }
}

/// Begin a stroke: insert a row, store id for point buffering.
pub fn begin_stroke(r: f64, g: f64, b: f64, width_scale: f64) {
    // Flush any unflushed stroke first
    flush_pending();

    let db = DB.lock().unwrap();
    if let Some(ref conn) = *db {
        let screen_id = *CURRENT_SCREEN_ID.lock().unwrap();
        let now = now_f64();
        let res = conn.execute(
            "INSERT INTO strokes (screen_id, color_r, color_g, color_b, width_scale, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![screen_id, r, g, b, width_scale, now],
        );
        if res.is_ok() {
            let stroke_id = conn.last_insert_rowid();
            *PENDING_STROKE_ID.lock().unwrap() = Some(stroke_id);
            PENDING_POINTS.lock().unwrap().clear();
        }
    }
}

/// Buffer a point. Will be flushed to DB on end_stroke.
pub fn add_point(x: f64, y: f64, width: f64) {
    PENDING_POINTS.lock().unwrap().push((x, y, width));
}

/// Flush buffered points to DB.
pub fn end_stroke() {
    flush_pending();
}

/// Flush pending points for the current stroke to the database.
fn flush_pending() {
    let stroke_id = {
        let mut pending = PENDING_STROKE_ID.lock().unwrap();
        match pending.take() {
            Some(id) => id,
            None => return,
        }
    };

    let points: Vec<(f64, f64, f64)> = {
        let mut p = PENDING_POINTS.lock().unwrap();
        std::mem::take(&mut *p)
    };

    if points.is_empty() {
        return;
    }

    let db = DB.lock().unwrap();
    if let Some(ref conn) = *db {
        let tx = conn.unchecked_transaction().ok();
        for (i, &(x, y, w)) in points.iter().enumerate() {
            conn.execute(
                "INSERT INTO points (stroke_id, seq, x, y, width) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![stroke_id, i as i64, x, y, w],
            ).ok();
        }
        if let Some(tx) = tx {
            tx.commit().ok();
        }
    }
}

/// Delete the last stroke from the current screen (for undo).
pub fn delete_last_stroke() {
    flush_pending();
    let db = DB.lock().unwrap();
    if let Some(ref conn) = *db {
        let screen_id = *CURRENT_SCREEN_ID.lock().unwrap();
        // Find the last stroke id for this screen
        let last_id: Option<i64> = conn.query_row(
            "SELECT id FROM strokes WHERE screen_id = ?1 ORDER BY id DESC LIMIT 1",
            params![screen_id],
            |row| row.get(0),
        ).ok();
        if let Some(sid) = last_id {
            conn.execute("DELETE FROM points WHERE stroke_id = ?1", params![sid]).ok();
            conn.execute("DELETE FROM strokes WHERE id = ?1", params![sid]).ok();
        }
    }
}

pub fn current_screen() -> i64 {
    *CURRENT_SCREEN_ID.lock().unwrap()
}

pub fn set_current_screen(id: i64) {
    *CURRENT_SCREEN_ID.lock().unwrap() = id;
}

/// Check if a screen has any strokes.
pub fn screen_has_strokes(screen_id: i64) -> bool {
    let db = DB.lock().unwrap();
    if let Some(ref conn) = *db {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM strokes WHERE screen_id = ?1)",
            params![screen_id],
            |row| row.get::<_, bool>(0),
        ).unwrap_or(false)
    } else {
        false
    }
}

/// Get the previous screen id (lower id with strokes), or None.
pub fn prev_screen(current: i64) -> Option<i64> {
    let db = DB.lock().unwrap();
    let conn = db.as_ref()?;
    conn.query_row(
        "SELECT s.id FROM screens s WHERE s.id < ?1 AND EXISTS (SELECT 1 FROM strokes WHERE screen_id = s.id) ORDER BY s.id DESC LIMIT 1",
        params![current],
        |row| row.get(0),
    ).ok()
}

/// Get the next screen id (higher id with strokes), or None.
pub fn next_screen(current: i64) -> Option<i64> {
    let db = DB.lock().unwrap();
    let conn = db.as_ref()?;
    conn.query_row(
        "SELECT s.id FROM screens s WHERE s.id > ?1 AND EXISTS (SELECT 1 FROM strokes WHERE screen_id = s.id) ORDER BY s.id ASC LIMIT 1",
        params![current],
        |row| row.get(0),
    ).ok()
}

pub struct StrokeData {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub width_scale: f64,
    pub points: Vec<(f64, f64, f64)>,
}

/// Load all strokes for a screen into the STROKES vec.
pub fn strokes_for_screen(screen_id: i64) -> Vec<StrokeData> {
    let db = DB.lock().unwrap();
    let conn = match db.as_ref() {
        Some(c) => c,
        None => return Vec::new(),
    };

    let mut strokes = Vec::new();
    let mut stmt = match conn.prepare(
        "SELECT id, color_r, color_g, color_b, width_scale FROM strokes WHERE screen_id = ?1 ORDER BY id"
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let rows = stmt.query_map(params![screen_id], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?, row.get::<_, f64>(2)?, row.get::<_, f64>(3)?, row.get::<_, f64>(4)?))
    });

    if let Ok(rows) = rows {
        for row in rows.flatten() {
            let (stroke_id, r, g, b, width_scale) = row;
            let mut points = Vec::new();
            if let Ok(mut ps) = conn.prepare("SELECT x, y, width FROM points WHERE stroke_id = ?1 ORDER BY seq") {
                if let Ok(pr) = ps.query_map(params![stroke_id], |prow| {
                    Ok((prow.get::<_, f64>(0)?, prow.get::<_, f64>(1)?, prow.get::<_, f64>(2)?))
                }) {
                    for p in pr.flatten() {
                        points.push(p);
                    }
                }
            }
            strokes.push(StrokeData { r, g, b, width_scale, points });
        }
    }

    strokes
}

// --- User settings ---

fn apply_defaults() {
    let db = DB.lock().unwrap();
    if let Some(ref conn) = *db {
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
            conn.execute(
                "INSERT OR IGNORE INTO user_settings (key, value) VALUES (?1, ?2)",
                params![key, val],
            ).ok();
        }
    }
}

pub fn save_setting(key: &str, value: &str) {
    let db = DB.lock().unwrap();
    if let Some(ref conn) = *db {
        conn.execute(
            "INSERT OR REPLACE INTO user_settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        ).ok();
    }
}

pub fn load_setting(key: &str) -> Option<String> {
    let db = DB.lock().unwrap();
    let conn = db.as_ref()?;
    conn.query_row(
        "SELECT value FROM user_settings WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    ).ok()
}

pub fn save_settings(pen_r: f64, pen_g: f64, pen_b: f64, width_scale: f64) {
    let db = DB.lock().unwrap();
    if let Some(ref conn) = *db {
        let pairs = [
            ("pen_r", pen_r),
            ("pen_g", pen_g),
            ("pen_b", pen_b),
            ("width_scale", width_scale),
        ];
        for &(key, val) in &pairs {
            conn.execute(
                "INSERT OR REPLACE INTO user_settings (key, value) VALUES (?1, ?2)",
                params![key, format!("{:.6}", val)],
            ).ok();
        }
    }
}

pub fn load_settings() -> Option<(f64, f64, f64, f64)> {
    let db = DB.lock().unwrap();
    let conn = db.as_ref()?;
    let r: f64 = conn.query_row(
        "SELECT value FROM user_settings WHERE key = 'pen_r'",
        [], |row| row.get::<_, String>(0),
    ).ok()?.parse().ok()?;
    let g: f64 = conn.query_row(
        "SELECT value FROM user_settings WHERE key = 'pen_g'",
        [], |row| row.get::<_, String>(0),
    ).ok()?.parse().ok()?;
    let b: f64 = conn.query_row(
        "SELECT value FROM user_settings WHERE key = 'pen_b'",
        [], |row| row.get::<_, String>(0),
    ).ok()?.parse().ok()?;
    let ws: f64 = conn.query_row(
        "SELECT value FROM user_settings WHERE key = 'width_scale'",
        [], |row| row.get::<_, String>(0),
    ).ok()?.parse().ok()?;
    Some((r, g, b, ws))
}
