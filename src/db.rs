// ---------------------------------------------------------------------------
// Shared across both platforms
// ---------------------------------------------------------------------------

pub struct StrokeData {
    pub id: i64,
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub width_scale: f64,
    pub points: Vec<(f64, f64, f64, f64)>, // (x, y, width, relative_time)
}

pub fn db_path() -> std::path::PathBuf {
    let exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let exe_dir = exe.parent().unwrap_or_else(|| std::path::Path::new("."));

    let is_bundled = exe_dir
        .ancestors()
        .any(|a| a.join("Contents").join("Info.plist").exists());

    if is_bundled && let Some(app_support) = app_support_dir() {
        std::fs::create_dir_all(&app_support).ok();
        return app_support.join("glaspen2.db");
    }

    exe_dir.join("glaspen2.db")
}

fn app_support_dir() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").ok()?;
        Some(
            std::path::PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("glaspen2"),
        )
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
// async sqlx (all platforms — the module itself is platform-agnostic;
// gating it to macOS/Windows broke Linux CI compilation of `pub use platform::*`)
// ---------------------------------------------------------------------------
mod platform {
    use crate::state;
    use sqlx::{Row, SqlitePool};
    use std::collections::HashMap;
    use std::sync::OnceLock;

    use super::{StrokeData, db_path, now_f64};

    static DB: OnceLock<SqlitePool> = OnceLock::new();

    /// 当前 schema 版本。**任何改变表结构的改动都要 +1**, 并在 `migrate_with`
    /// 里补一段迁移。
    ///
    /// 两个作用: 旧版程序打开新版写过的库时直接拒绝(而不是按旧 schema 读写出
    /// 错、把数据写坏); 迁移失败时定位到底停在哪一版。
    pub(crate) const SCHEMA_VERSION: i32 = 1;

    pub async fn init() {
        let path = db_path();
        let pool = SqlitePool::connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true)
                // WAL + synchronous=NORMAL: commits don't fsync on every
                // write, so the per-stroke begin INSERT (running on the main
                // thread via block_on) no longer hitches pen-down.
                .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
                .synchronous(sqlx::sqlite::SqliteSynchronous::Normal),
        )
        .await
        .expect("Failed to open glaspen2.db");

        if let Err(e) = migrate_with(&pool).await {
            // 迁移失败绝不能带着半个 schema 继续跑: 报清楚原因并中止启动,
            // 用户至少知道该从哪个备份恢复, 而不是用着用着丢数据。
            panic!("[glaspen2] 数据库迁移失败, 已中止启动:\n{e}");
        }

        apply_defaults(&pool).await;

        DB.set(pool).ok();
        println!("[glaspen2] DB initialized at {}", path.display());
    }

    async fn exec(pool: &SqlitePool, sql: &str) -> Result<(), String> {
        sqlx::query(sql)
            .execute(pool)
            .await
            .map(|_| ())
            .map_err(|e| {
                format!(
                    "{e}\n  SQL: {}",
                    sql.split_whitespace().collect::<Vec<_>>().join(" ")
                )
            })
    }

    /// 加列迁移:先查 PRAGMA 再决定要不要 ALTER。
    ///
    /// 原来这些语句是 `ALTER TABLE ... .ok()`, 列已存在时确实要忽略, 但
    /// **真正的失败也被一起吞掉了** —— 结果是一个缺列的库继续跑, 直到某次
    /// 查询报错。现在只有"列已存在"会静默跳过, 别的错误一律上报。
    async fn add_column_if_missing(
        pool: &SqlitePool,
        table: &str,
        column: &str,
        decl: &str,
    ) -> Result<(), String> {
        let rows = sqlx::query(&format!("PRAGMA table_info({table})"))
            .fetch_all(pool)
            .await
            .map_err(|e| format!("读取 {table} 表结构失败: {e}"))?;
        let exists = rows.iter().any(|r| {
            r.try_get::<String, _>("name")
                .map(|n| n == column)
                .unwrap_or(false)
        });
        if exists {
            return Ok(());
        }
        exec(
            pool,
            &format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"),
        )
        .await
    }

    /// 建表 + 迁移。拆出来是为了能在测试里对指定 pool 跑, 而不是只能走全局单例。
    ///
    /// 全部语句都是幂等的(`CREATE ... IF NOT EXISTS` / 加列前先查), 所以
    /// 中途失败后下次启动重跑是安全的。
    pub(crate) async fn migrate_with(pool: &SqlitePool) -> Result<(), String> {
        let version: i32 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("读取 schema 版本失败: {e}"))?;
        if version > SCHEMA_VERSION {
            return Err(format!(
                "数据库 schema 版本为 {version}, 高于本程序支持的 {SCHEMA_VERSION} —— \
                 这份数据由更新版本的 glaspen2 写入, 请升级程序后再打开。\n  库文件: {}",
                db_path().display()
            ));
        }

        for sql in [
            "CREATE TABLE IF NOT EXISTS screens (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at REAL NOT NULL,
                screen_w INTEGER NOT NULL,
                screen_h INTEGER NOT NULL,
                edited INTEGER NOT NULL DEFAULT 0
            )",
            "CREATE TABLE IF NOT EXISTS strokes (
                id INTEGER PRIMARY KEY,
                screen_id INTEGER NOT NULL REFERENCES screens(id),
                color_r REAL NOT NULL,
                color_g REAL NOT NULL,
                color_b REAL NOT NULL,
                width_scale REAL NOT NULL DEFAULT 1.0,
                created_at REAL NOT NULL
            )",
            "CREATE TABLE IF NOT EXISTS points (
                stroke_id INTEGER NOT NULL REFERENCES strokes(id),
                seq INTEGER NOT NULL,
                x REAL NOT NULL,
                y REAL NOT NULL,
                width REAL NOT NULL,
                t REAL NOT NULL DEFAULT 0.0,
                PRIMARY KEY (stroke_id, seq)
            )",
            "CREATE INDEX IF NOT EXISTS idx_strokes_screen ON strokes(screen_id)",
            // 无限画布模式:独立存储,与翻页模式完全分开。
            // 目前全局只有一个无限画布,故这两张表不再按页(screen_id)分组。
            "CREATE TABLE IF NOT EXISTS infinite_strokes (
                id INTEGER PRIMARY KEY,
                color_r REAL NOT NULL,
                color_g REAL NOT NULL,
                color_b REAL NOT NULL,
                width_scale REAL NOT NULL DEFAULT 1.0,
                created_at REAL NOT NULL
            )",
            "CREATE TABLE IF NOT EXISTS infinite_points (
                stroke_id INTEGER NOT NULL REFERENCES infinite_strokes(id),
                seq INTEGER NOT NULL,
                x REAL NOT NULL,
                y REAL NOT NULL,
                width REAL NOT NULL,
                t REAL NOT NULL DEFAULT 0.0,
                PRIMARY KEY (stroke_id, seq)
            )",
            "CREATE TABLE IF NOT EXISTS user_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            // 缩略图缓存:渲染结果(PNG)按页存库,内容未变时直接复用,
            // 避免每次打开活页本都全量拉笔迹+渲染。新鲜度由
            // (stroke_count, max_stroke_id, outline, max_size) 四元组判定。
            "CREATE TABLE IF NOT EXISTS screen_thumbnails (
                screen_id INTEGER NOT NULL REFERENCES screens(id),
                max_size INTEGER NOT NULL,
                stroke_count INTEGER NOT NULL,
                max_stroke_id INTEGER NOT NULL,
                outline INTEGER NOT NULL DEFAULT 0,
                png BLOB NOT NULL,
                generated_at REAL NOT NULL,
                PRIMARY KEY (screen_id, max_size)
            )",
        ] {
            exec(pool, sql).await?;
        }

        // 历史迁移(老库缺这些列)。软删除:deleted_at 非 NULL 表示已删除
        // (不参与查询,可恢复);三张实体表各自记录,points/infinite_points
        // 跟随父级笔迹即可。
        add_column_if_missing(pool, "screens", "edited", "INTEGER NOT NULL DEFAULT 0").await?;
        add_column_if_missing(pool, "points", "t", "REAL NOT NULL DEFAULT 0.0").await?;
        add_column_if_missing(pool, "screens", "deleted_at", "REAL").await?;
        add_column_if_missing(pool, "strokes", "deleted_at", "REAL").await?;
        add_column_if_missing(pool, "infinite_strokes", "deleted_at", "REAL").await?;

        // 注:旧版本曾在 screens 上存 per-page 镜头(pan_x/pan_y/zoom)。
        // 现在无限画布独立存储且全局只有一个画布,镜头改存 user_settings,
        // 这几列不再读写(旧库中残留的列保持不动,无副作用)。

        // 迁移全部成功才写版本号:中途失败时下次启动会整套重跑(语句都幂等)
        sqlx::query(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))
            .execute(pool)
            .await
            .map_err(|e| format!("写入 schema 版本失败: {e}"))?;
        Ok(())
    }

    async fn apply_defaults(pool: &SqlitePool) {
        let defaults = [
            ("pen_r", "1.0"),
            ("pen_g", "0.0"),
            ("pen_b", "0.0"),
            ("width_scale", "1.0"),
            ("glass_alpha", "0"),
            ("glass_enabled", "0"),
        ];
        for &(key, val) in &defaults {
            sqlx::query("INSERT OR IGNORE INTO user_settings (key, value) VALUES (?1, ?2)")
                .bind(key)
                .bind(val)
                .execute(pool)
                .await
                .ok();
        }
    }

    pub async fn new_screen(screen_w: i32, screen_h: i32) {
        let pool = DB.get().expect("DB not initialized");
        let now = now_f64();
        let sid = sqlx::query_scalar::<_, i64>(
            "INSERT INTO screens (created_at, screen_w, screen_h) VALUES (?1, ?2, ?3) RETURNING id",
        )
        .bind(now)
        .bind(screen_w)
        .bind(screen_h)
        .fetch_one(pool)
        .await
        .unwrap_or(0);
        state::set_current_screen_id(sid);
    }

    /// Begin a stroke in the DB. Returns the new stroke id (0 on failure).
    /// Routed by the current canvas kind: page mode writes to strokes (+ marks
    /// the page edited); infinite mode writes to the single infinite canvas.
    pub async fn begin_stroke(r: f64, g: f64, b: f64, width_scale: f64) -> i64 {
        use crate::state;
        let pool = DB.get().expect("DB not initialized");
        let now = now_f64();
        if state::canvas_kind() == state::CanvasKind::Infinite {
            let stroke_id = sqlx::query_scalar::<_, i64>(
                "INSERT INTO infinite_strokes (color_r, color_g, color_b, width_scale, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5) RETURNING id",
            )
            .bind(r)
            .bind(g)
            .bind(b)
            .bind(width_scale)
            .bind(now)
            .fetch_optional(pool)
            .await;
            return match stroke_id {
                Ok(Some(id)) => {
                    state::begin_pending(id, state::CanvasKind::Infinite);
                    id
                }
                _ => 0,
            };
        }

        let screen_id = state::current_screen_id();
        let stroke_id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO strokes (screen_id, color_r, color_g, color_b, width_scale, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6) RETURNING id"
        ).bind(screen_id).bind(r).bind(g).bind(b).bind(width_scale).bind(now)
            .fetch_optional(pool).await;
        match stroke_id {
            Ok(Some(id)) => {
                state::begin_pending(id, state::CanvasKind::Page);
                // Mark the canvas as edited — even if all strokes are later
                // cleared/undone, the canvas counts as used.
                sqlx::query("UPDATE screens SET edited = 1 WHERE id = ?1")
                    .bind(screen_id)
                    .execute(pool)
                    .await
                    .ok();
                id
            }
            _ => 0,
        }
    }

    pub async fn end_stroke() {
        flush_pending().await;
    }

    async fn flush_pending() {
        use crate::state;
        let (stroke_id, kind, points) = match state::take_pending_bundle() {
            Some(b) => b,
            None => return,
        };
        if points.is_empty() {
            return;
        }
        let pool = DB.get().expect("DB not initialized");
        let mut tx = match pool.begin().await {
            Ok(t) => t,
            Err(_) => return,
        };
        // Route by the kind recorded at stroke begin (NOT the current kind) so
        // an async pen-up flush can't land in a mode the user just switched to.
        let table = match kind {
            state::CanvasKind::Infinite => "infinite_points",
            state::CanvasKind::Page => "points",
        };
        // 批量多行 INSERT(每 100 行一条语句):逐行 INSERT 每行都要 await
        // 一轮连接池,一整笔几百个点的写事务占用 SQLite 写锁十几~几十毫秒,
        // 下一笔落笔的同步 INSERT(begin_stroke)在 UI 线程排队等待,
        // 表现为"每画一笔卡一下"。批量后写事务 ~1-3ms,落笔无感。
        const CHUNK: usize = 100; // 100 行 × 6 参数,远低于 SQLITE_MAX_VARIABLE_NUMBER
        let mut done = 0usize;
        while done < points.len() {
            let end = (done + CHUNK).min(points.len());
            let n = end - done;
            let mut sql = String::with_capacity(n * 48);
            sql.push_str("INSERT INTO ");
            sql.push_str(table);
            sql.push_str(" (stroke_id, seq, x, y, width, t) VALUES ");
            for k in 0..n {
                if k > 0 {
                    sql.push(',');
                }
                let b = k * 6;
                sql.push_str(&format!(
                    " (?{}, ?{}, ?{}, ?{}, ?{}, ?{})",
                    b + 1,
                    b + 2,
                    b + 3,
                    b + 4,
                    b + 5,
                    b + 6
                ));
            }
            let mut q = sqlx::query(&sql);
            for (j, &(x, y, w, t)) in points[done..end].iter().enumerate() {
                q = q
                    .bind(stroke_id)
                    .bind((done + j) as i64)
                    .bind(x)
                    .bind(y)
                    .bind(w)
                    .bind(t);
            }
            q.execute(&mut *tx).await.ok();
            done = end;
        }
        tx.commit().await.ok();
    }

    pub fn end_stroke_spawned() {
        let rt = crate::runtime();
        // 后台任务由 tokio runtime 立即执行; JoinHandle 丢弃即分离,
        // 不是"绑定但从不运行"的 future, 故豁免该 lint。
        #[allow(clippy::let_underscore_future)]
        let _ = rt.spawn(async {
            flush_pending().await;
        });
    }

    pub async fn screen_has_strokes(screen_id: i64) -> bool {
        let pool = match DB.get() {
            Some(p) => p,
            None => return false,
        };
        sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM strokes WHERE screen_id = ?1 AND deleted_at IS NULL)",
        )
        .bind(screen_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0)
            != 0
    }

    /// Whether the canvas was ever edited (a stroke was started on it).
    /// True even if every stroke was later cleared/undone. Strokes in the
    /// table also imply edited (covers pre-migration databases).
    pub async fn screen_edited(screen_id: i64) -> bool {
        let pool = match DB.get() {
            Some(p) => p,
            None => return false,
        };
        if screen_id <= 0 {
            return false;
        }
        sqlx::query_scalar::<_, i64>(
            "SELECT CASE WHEN edited = 1 OR EXISTS \
             (SELECT 1 FROM strokes WHERE screen_id = screens.id AND deleted_at IS NULL) \
             THEN 1 ELSE 0 END FROM screens WHERE id = ?1",
        )
        .bind(screen_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .unwrap_or(0)
            != 0
    }

    /// Delete a stroke by id. Returns true if the stroke existed.
    pub async fn delete_stroke_by_id(stroke_id: i64) -> bool {
        let pool = match DB.get() {
            Some(p) => p,
            None => return false,
        };
        // 软删除:标记而非物理删除(数据可恢复)
        let now = now_f64();
        let deleted =
            sqlx::query("UPDATE strokes SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL")
                .bind(stroke_id)
                .bind(now)
                .execute(pool)
                .await
                .ok();
        deleted.map(|r| r.rows_affected() > 0).unwrap_or(false)
    }

    pub async fn delete_last_stroke() -> bool {
        use crate::state;
        let pool = match DB.get() {
            Some(p) => p,
            None => return false,
        };
        let screen_id = state::current_screen_id();
        let stroke_id = match sqlx::query_scalar::<_, i64>(
            "SELECT id FROM strokes WHERE screen_id = ?1 ORDER BY id DESC LIMIT 1",
        )
        .bind(screen_id)
        .fetch_optional(pool)
        .await
        {
            Ok(Some(id)) => id,
            _ => return false,
        };
        delete_stroke_by_id(stroke_id).await
    }

    pub async fn delete_screen(target_id: i64) -> bool {
        let pool = match DB.get() {
            Some(p) => p,
            None => return false,
        };
        // 软删除:标记 screens + strokes 而非物理删除
        let now = now_f64();
        let screen_del =
            sqlx::query("UPDATE screens SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL")
                .bind(target_id)
                .bind(now)
                .execute(pool)
                .await
                .ok();
        let stroke_del = sqlx::query(
            "UPDATE strokes SET deleted_at = ?2 WHERE screen_id = ?1 AND deleted_at IS NULL",
        )
        .bind(target_id)
        .bind(now)
        .execute(pool)
        .await
        .ok();
        thumbnails_purge_screen(target_id).await;
        screen_del.is_some() || stroke_del.is_some()
    }

    pub async fn prev_screen(current: i64) -> Option<i64> {
        let pool = DB.get()?;
        sqlx::query_scalar::<_, i64>(
            "SELECT id FROM screens WHERE id < ?1 AND deleted_at IS NULL ORDER BY id DESC LIMIT 1",
        )
        .bind(current)
        .fetch_optional(pool)
        .await
        .ok()?
    }

    pub async fn next_screen(current: i64) -> Option<i64> {
        let pool = DB.get()?;
        sqlx::query_scalar::<_, i64>(
            "SELECT id FROM screens WHERE id > ?1 AND deleted_at IS NULL ORDER BY id ASC LIMIT 1",
        )
        .bind(current)
        .fetch_optional(pool)
        .await
        .ok()?
    }

    /// Group point rows (stroke_id, seq, x, y, width, t) into stroke records.
    /// Strokes and points are both ordered by id/stroke_id ascending, so the
    /// grouping advances monotonically; orphan point rows are ignored.
    pub(super) fn attach_points(
        mut strokes: Vec<StrokeData>,
        pts: Vec<(i64, i64, f64, f64, f64, f64)>,
    ) -> Vec<StrokeData> {
        let mut idx: usize = 0;
        for (stroke_id, _seq, x, y, w, t) in pts {
            while idx < strokes.len() && strokes[idx].id != stroke_id {
                idx += 1;
            }
            if idx < strokes.len() {
                strokes[idx].points.push((x, y, w, t));
            }
        }
        strokes
    }

    pub async fn strokes_for_screen(screen_id: i64) -> Vec<StrokeData> {
        let pool = match DB.get() {
            Some(p) => p,
            None => return Vec::new(),
        };
        let rows: Vec<(i64, f64, f64, f64, f64)> = sqlx::query_as(
            "SELECT id, color_r, color_g, color_b, width_scale FROM strokes WHERE screen_id = ?1 AND deleted_at IS NULL ORDER BY id"
        ).bind(screen_id).fetch_all(pool).await.unwrap_or_default();
        if rows.is_empty() {
            return Vec::new();
        }

        // Single query for all points of the screen (avoids N+1 per stroke).
        let pts: Vec<(i64, i64, f64, f64, f64, f64)> = sqlx::query_as(
            "SELECT p.stroke_id, p.seq, p.x, p.y, p.width, p.t \
             FROM points p JOIN strokes s ON s.id = p.stroke_id \
             WHERE s.screen_id = ?1 AND s.deleted_at IS NULL \
             ORDER BY p.stroke_id, p.seq",
        )
        .bind(screen_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let strokes: Vec<StrokeData> = rows
            .into_iter()
            .map(|(id, r, g, b, ws)| StrokeData {
                id,
                r,
                g,
                b,
                width_scale: ws,
                points: Vec::new(),
            })
            .collect();
        attach_points(strokes, pts)
    }

    // ── 缩略图缓存 ────────────────────────────────────────────────
    // 内容版本 = (非删除笔迹数, 最大笔迹 id)。笔迹只追加/软删,
    // 该二元组足以识别内容变化;配合 outline/max_size 一起判定缓存新鲜度。

    /// Per-screen content version for thumbnail freshness checks.
    pub async fn screen_stroke_version(screen_id: i64) -> (i64, i64) {
        match DB.get() {
            Some(p) => screen_stroke_version_with(p, screen_id).await,
            None => (0, 0),
        }
    }

    pub(crate) async fn screen_stroke_version_with(
        pool: &SqlitePool,
        screen_id: i64,
    ) -> (i64, i64) {
        let r: (i64, Option<i64>) = sqlx::query_as(
            "SELECT COUNT(*), MAX(id) FROM strokes WHERE screen_id = ?1 AND deleted_at IS NULL",
        )
        .bind(screen_id)
        .fetch_one(pool)
        .await
        .unwrap_or((0, None));
        (r.0, r.1.unwrap_or(0))
    }

    pub async fn thumbnail_lookup(
        screen_id: i64,
        max_size: i32,
        stroke_count: i64,
        max_stroke_id: i64,
        outline: bool,
    ) -> Option<Vec<u8>> {
        thumbnail_lookup_with(
            DB.get()?,
            screen_id,
            max_size,
            stroke_count,
            max_stroke_id,
            outline,
        )
        .await
    }

    pub(crate) async fn thumbnail_lookup_with(
        pool: &SqlitePool,
        screen_id: i64,
        max_size: i32,
        stroke_count: i64,
        max_stroke_id: i64,
        outline: bool,
    ) -> Option<Vec<u8>> {
        let png: Option<(Vec<u8>,)> = sqlx::query_as(
            "SELECT png FROM screen_thumbnails \
             WHERE screen_id = ?1 AND max_size = ?2 \
             AND stroke_count = ?3 AND max_stroke_id = ?4 AND outline = ?5",
        )
        .bind(screen_id)
        .bind(max_size)
        .bind(stroke_count)
        .bind(max_stroke_id)
        .bind(outline as i64)
        .fetch_optional(pool)
        .await
        .ok()?;
        png.map(|(p,)| p)
    }

    pub async fn thumbnail_store(
        screen_id: i64,
        max_size: i32,
        stroke_count: i64,
        max_stroke_id: i64,
        outline: bool,
        png: &[u8],
    ) {
        if let Some(pool) = DB.get() {
            thumbnail_store_with(
                pool,
                screen_id,
                max_size,
                stroke_count,
                max_stroke_id,
                outline,
                png,
            )
            .await;
        }
    }

    pub(crate) async fn thumbnail_store_with(
        pool: &SqlitePool,
        screen_id: i64,
        max_size: i32,
        stroke_count: i64,
        max_stroke_id: i64,
        outline: bool,
        png: &[u8],
    ) {
        sqlx::query(
            "INSERT INTO screen_thumbnails \
             (screen_id, max_size, stroke_count, max_stroke_id, outline, png, generated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
             ON CONFLICT(screen_id, max_size) DO UPDATE SET \
             stroke_count = ?3, max_stroke_id = ?4, outline = ?5, png = ?6, generated_at = ?7",
        )
        .bind(screen_id)
        .bind(max_size)
        .bind(stroke_count)
        .bind(max_stroke_id)
        .bind(outline as i64)
        .bind(png)
        .bind(now_f64())
        .execute(pool)
        .await
        .ok();
    }

    /// Batched content versions for many screens in one query.
    /// Screens with no live strokes are absent from the map.
    pub async fn stroke_versions_many(ids: &[i64]) -> HashMap<i64, (i64, i64)> {
        match DB.get() {
            Some(pool) => stroke_versions_many_with(pool, ids).await,
            None => HashMap::new(),
        }
    }

    pub(crate) async fn stroke_versions_many_with(
        pool: &SqlitePool,
        ids: &[i64],
    ) -> HashMap<i64, (i64, i64)> {
        if ids.is_empty() {
            return HashMap::new();
        }
        let mut qb = sqlx::QueryBuilder::new(
            "SELECT screen_id, COUNT(*), MAX(id) FROM strokes \
             WHERE deleted_at IS NULL AND screen_id IN (",
        );
        let mut sep = qb.separated(", ");
        for id in ids {
            sep.push_bind(*id);
        }
        sep.push_unseparated(") GROUP BY screen_id");
        let rows: Vec<(i64, i64, Option<i64>)> = qb
            .build_query_as()
            .fetch_all(pool)
            .await
            .unwrap_or_default();
        rows.into_iter()
            .map(|(id, count, max_id)| (id, (count, max_id.unwrap_or(0))))
            .collect()
    }

    /// Batched cached-thumbnail lookup: one row per screen that has a cache
    /// entry for this size/outline, as (stroke_count, max_stroke_id, png).
    /// The caller compares the stored version with the live one.
    pub async fn thumbnails_many(
        ids: &[i64],
        max_size: i32,
        outline: bool,
    ) -> HashMap<i64, (i64, i64, Vec<u8>)> {
        match DB.get() {
            Some(pool) => thumbnails_many_with(pool, ids, max_size, outline).await,
            None => HashMap::new(),
        }
    }

    pub(crate) async fn thumbnails_many_with(
        pool: &SqlitePool,
        ids: &[i64],
        max_size: i32,
        outline: bool,
    ) -> HashMap<i64, (i64, i64, Vec<u8>)> {
        if ids.is_empty() {
            return HashMap::new();
        }
        let mut qb = sqlx::QueryBuilder::new(
            "SELECT screen_id, stroke_count, max_stroke_id, png FROM screen_thumbnails \
             WHERE max_size = ",
        );
        qb.push_bind(max_size);
        qb.push(" AND outline = ");
        qb.push_bind(outline as i64);
        qb.push(" AND screen_id IN (");
        let mut sep = qb.separated(", ");
        for id in ids {
            sep.push_bind(*id);
        }
        sep.push_unseparated(")");
        let rows: Vec<(i64, i64, i64, Vec<u8>)> = qb
            .build_query_as()
            .fetch_all(pool)
            .await
            .unwrap_or_default();
        rows.into_iter()
            .map(|(id, count, max_id, png)| (id, (count, max_id, png)))
            .collect()
    }

    /// Drop cached thumbnails for one screen (page deleted).
    pub async fn thumbnails_purge_screen(screen_id: i64) {
        if let Some(pool) = DB.get() {
            thumbnails_purge_screen_with(pool, screen_id).await;
        }
    }

    pub(crate) async fn thumbnails_purge_screen_with(pool: &SqlitePool, screen_id: i64) {
        sqlx::query("DELETE FROM screen_thumbnails WHERE screen_id = ?1")
            .bind(screen_id)
            .execute(pool)
            .await
            .ok();
    }

    pub async fn list_screens() -> Vec<(i64, i32, i32)> {
        let pool = match DB.get() {
            Some(p) => p,
            None => return Vec::new(),
        };
        sqlx::query_as(
            "SELECT s.id, s.screen_w, s.screen_h FROM screens s \
             WHERE deleted_at IS NULL \
             AND EXISTS (SELECT 1 FROM strokes WHERE screen_id = s.id) \
             ORDER BY s.id",
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    }

    /// Page info for the 新建画布/翻页 notification:
    /// (nth-of-date, date_total, position, total, created_at).
    /// Date grouping uses local time (same calendar date = 今天/昨天/…).
    pub async fn page_info(screen_id: i64) -> Option<(i64, i64, i64, i64, f64)> {
        let pool = DB.get()?;
        page_info_with(pool, screen_id).await
    }

    pub(super) async fn page_info_with(
        pool: &SqlitePool,
        screen_id: i64,
    ) -> Option<(i64, i64, i64, i64, f64)> {
        if screen_id <= 0 {
            return None;
        }
        // The outer WHERE id = ?1 guarantees an unknown id yields no row
        // (otherwise the subqueries would still produce a NULL-created row).
        sqlx::query_as(
            "SELECT \
               (SELECT COUNT(*) FROM screens s WHERE \
                   date(datetime(s.created_at,'unixepoch','localtime')) = \
                   (SELECT date(datetime(created_at,'unixepoch','localtime')) FROM screens WHERE id = ?1) \
                   AND s.id <= ?1) AS nth, \
               (SELECT COUNT(*) FROM screens s WHERE \
                   date(datetime(s.created_at,'unixepoch','localtime')) = \
                   (SELECT date(datetime(created_at,'unixepoch','localtime')) FROM screens WHERE id = ?1)) AS date_total, \
               (SELECT COUNT(*) FROM screens WHERE id <= ?1) AS pos, \
               (SELECT COUNT(*) FROM screens) AS total, \
               (SELECT created_at FROM screens WHERE id = ?1) AS created \
             FROM screens WHERE id = ?1"
        ).bind(screen_id).fetch_optional(pool).await.ok()?
    }

    // ── 无限画布(独立存储,全局仅一个画布) ──

    /// 读取整个无限画布的笔迹(全局 DB)。
    pub async fn load_infinite_strokes() -> Vec<StrokeData> {
        match DB.get() {
            Some(pool) => load_infinite_strokes_with(pool).await,
            None => Vec::new(),
        }
    }

    /// 读取整个无限画布的笔迹(指定连接池;导出用只读池时走这个)。
    pub async fn load_infinite_strokes_with(pool: &SqlitePool) -> Vec<StrokeData> {
        let rows: Vec<(i64, f64, f64, f64, f64)> = sqlx::query_as(
            "SELECT id, color_r, color_g, color_b, width_scale FROM infinite_strokes WHERE deleted_at IS NULL ORDER BY id",
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default();
        if rows.is_empty() {
            return Vec::new();
        }
        let pts: Vec<(i64, i64, f64, f64, f64, f64)> = sqlx::query_as(
            "SELECT p.stroke_id, p.seq, p.x, p.y, p.width, p.t \
             FROM infinite_points p ORDER BY p.stroke_id, p.seq",
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default();
        let strokes: Vec<StrokeData> = rows
            .into_iter()
            .map(|(id, r, g, b, ws)| StrokeData {
                id,
                r,
                g,
                b,
                width_scale: ws,
                points: Vec::new(),
            })
            .collect();
        attach_points(strokes, pts)
    }

    pub async fn delete_infinite_stroke_by_id(stroke_id: i64) -> bool {
        let pool = match DB.get() {
            Some(p) => p,
            None => return false,
        };
        // 软删除:标记而非物理删除
        let now = now_f64();
        let deleted = sqlx::query(
            "UPDATE infinite_strokes SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
        )
        .bind(stroke_id)
        .bind(now)
        .execute(pool)
        .await
        .ok();
        deleted.map(|r| r.rows_affected() > 0).unwrap_or(false)
    }

    pub async fn delete_last_infinite_stroke() -> bool {
        let pool = match DB.get() {
            Some(p) => p,
            None => return false,
        };
        let stroke_id = match sqlx::query_scalar::<_, i64>(
            "SELECT id FROM infinite_strokes ORDER BY id DESC LIMIT 1",
        )
        .fetch_optional(pool)
        .await
        {
            Ok(Some(id)) => id,
            _ => return false,
        };
        delete_infinite_stroke_by_id(stroke_id).await
    }

    pub async fn infinite_canvas_has_strokes() -> bool {
        let pool = match DB.get() {
            Some(p) => p,
            None => return false,
        };
        sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM infinite_strokes WHERE deleted_at IS NULL)",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(0)
            != 0
    }

    /// 清空无限画布的全部内容(不删画布本身,画布只有一个)。
    pub async fn clear_infinite_canvas() {
        let pool = match DB.get() {
            Some(p) => p,
            None => return,
        };
        // 软删除:标记而非物理删除
        let now = now_f64();
        sqlx::query("UPDATE infinite_strokes SET deleted_at = ?1 WHERE deleted_at IS NULL")
            .bind(now)
            .execute(pool)
            .await
            .ok();
    }

    /// 当前页高度(逻辑 px)
    pub async fn page_height(screen_id: i64) -> Option<f64> {
        let pool = DB.get()?;
        sqlx::query_scalar::<_, i32>("SELECT screen_h FROM screens WHERE id = ?1")
            .bind(screen_id)
            .fetch_optional(pool)
            .await
            .ok()?
            .map(|v| v as f64)
    }

    /// 当前页的笔迹数（排除软删除）
    pub async fn page_stroke_count(screen_id: i64) -> u64 {
        let pool = match DB.get() {
            Some(p) => p,
            None => return 0,
        };
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM strokes WHERE screen_id = ?1 AND deleted_at IS NULL",
        )
        .bind(screen_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0) as u64
    }

    /// 无限画布镜头变换(全局唯一,存 user_settings)。
    pub async fn set_infinite_transform(pan_x: f64, pan_y: f64, zoom: f64) {
        save_setting("infinite_pan_x", &format!("{pan_x:.6}")).await;
        save_setting("infinite_pan_y", &format!("{pan_y:.6}")).await;
        save_setting("infinite_zoom", &format!("{zoom:.6}")).await;
    }

    /// 无限画布镜头变换(未设置 → None,调用方用 0,0,1)。
    pub async fn get_infinite_transform() -> Option<(f64, f64, f64)> {
        let x = load_setting("infinite_pan_x").await?.parse::<f64>().ok()?;
        let y = load_setting("infinite_pan_y").await?.parse::<f64>().ok()?;
        let z = load_setting("infinite_zoom").await?.parse::<f64>().ok()?;
        Some((x, y, z))
    }

    pub async fn save_setting(key: &str, value: &str) {
        let pool = match DB.get() {
            Some(p) => p,
            None => return,
        };
        sqlx::query("INSERT OR REPLACE INTO user_settings (key, value) VALUES (?1, ?2)")
            .bind(key)
            .bind(value)
            .execute(pool)
            .await
            .ok();
    }

    pub async fn load_setting(key: &str) -> Option<String> {
        let pool = DB.get()?;
        sqlx::query_scalar::<_, String>("SELECT value FROM user_settings WHERE key = ?1")
            .bind(key)
            .fetch_optional(pool)
            .await
            .ok()?
    }

    pub async fn save_settings(pen_r: f64, pen_g: f64, pen_b: f64, width_scale: f64) {
        let pool = match DB.get() {
            Some(p) => p,
            None => return,
        };
        for &(k, v) in &[
            ("pen_r", pen_r),
            ("pen_g", pen_g),
            ("pen_b", pen_b),
            ("width_scale", width_scale),
        ] {
            sqlx::query("INSERT OR REPLACE INTO user_settings (key, value) VALUES (?1, ?2)")
                .bind(k)
                .bind(format!("{:.6}", v))
                .execute(pool)
                .await
                .ok();
        }
    }

    pub async fn load_settings() -> Option<(f64, f64, f64, f64)> {
        let pool = DB.get()?;
        let r: f64 =
            sqlx::query_scalar::<_, String>("SELECT value FROM user_settings WHERE key = 'pen_r'")
                .fetch_optional(pool)
                .await
                .ok()??
                .parse()
                .ok()?;
        let g: f64 =
            sqlx::query_scalar::<_, String>("SELECT value FROM user_settings WHERE key = 'pen_g'")
                .fetch_optional(pool)
                .await
                .ok()??
                .parse()
                .ok()?;
        let b: f64 =
            sqlx::query_scalar::<_, String>("SELECT value FROM user_settings WHERE key = 'pen_b'")
                .fetch_optional(pool)
                .await
                .ok()??
                .parse()
                .ok()?;
        let ws: f64 = sqlx::query_scalar::<_, String>(
            "SELECT value FROM user_settings WHERE key = 'width_scale'",
        )
        .fetch_optional(pool)
        .await
        .ok()??
        .parse()
        .ok()?;
        Some((r, g, b, ws))
    }
}

pub use platform::*;

#[cfg(test)]
mod tests {
    use super::StrokeData;
    use super::platform::{
        SCHEMA_VERSION, attach_points, migrate_with, page_info_with, screen_stroke_version_with,
        stroke_versions_many_with, thumbnail_lookup_with, thumbnail_store_with,
        thumbnails_many_with, thumbnails_purge_screen_with,
    };
    use crate::runtime;
    use sqlx::SqlitePool;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static DB_COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// Fresh temp-file DB per test (pool + :memory: would fragment the DB
    /// across connections).
    /// 只创建连接、不建任何表 —— 迁移测试要从空库或老 schema 起步
    /// (temp_pool 会先把新 schema 建好, 那样测不到迁移)。
    async fn raw_temp_pool() -> (SqlitePool, std::path::PathBuf) {
        let n = DB_COUNTER.fetch_add(1, Ordering::SeqCst);
        let path =
            std::env::temp_dir().join(format!("glaspen2_db_raw_{}_{}.db", std::process::id(), n));
        let _ = std::fs::remove_file(&path);
        let pool = SqlitePool::connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true),
        )
        .await
        .unwrap();
        (pool, path)
    }

    async fn temp_pool() -> (SqlitePool, std::path::PathBuf) {
        let n = DB_COUNTER.fetch_add(1, Ordering::SeqCst);
        let path =
            std::env::temp_dir().join(format!("glaspen2_db_test_{}_{}.db", std::process::id(), n));
        let _ = std::fs::remove_file(&path);
        let pool = SqlitePool::connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true),
        )
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE screens (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at REAL NOT NULL,
                screen_w INTEGER NOT NULL,
                screen_h INTEGER NOT NULL,
                edited INTEGER NOT NULL DEFAULT 0
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        (pool, path)
    }

    async fn add_screen(pool: &SqlitePool, ts: f64) -> i64 {
        sqlx::query_scalar::<_, i64>(
            "INSERT INTO screens (created_at, screen_w, screen_h) VALUES (?1, 1920, 1080) RETURNING id",
        )
        .bind(ts)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    #[test]
    fn test_page_info_grouping_local_dates() {
        let _g = crate::tests::TEST_LOCK.lock().unwrap();
        runtime().block_on(async {
            let (pool, path) = temp_pool().await;
            // Fixed reference (2025-01-01 08:00 local): grouping is relative
            // to the stored rows, never to the wall clock.
            let a = 1735689600.0; // 2025-01-01 08:00 (UTC+8)
            let prev_day = add_screen(&pool, a - 86400.0).await; // 2024-12-31
            let _today1 = add_screen(&pool, a).await; // 2025-01-01
            let today2 = add_screen(&pool, a + 3600.0).await; // 2025-01-01
            let next_day = add_screen(&pool, a + 86400.0).await; // 2025-01-02

            let (nth, date_total, pos, total, _created) =
                page_info_with(&pool, today2).await.unwrap();
            assert_eq!((nth, date_total, pos, total), (2, 2, 3, 4));

            let (nth, date_total, pos, total, _created) =
                page_info_with(&pool, prev_day).await.unwrap();
            assert_eq!((nth, date_total, pos, total), (1, 1, 1, 4));

            let (nth, date_total, pos, total, _created) =
                page_info_with(&pool, next_day).await.unwrap();
            assert_eq!((nth, date_total, pos, total), (1, 1, 4, 4));

            // Invalid ids
            assert!(page_info_with(&pool, 0).await.is_none());
            assert!(page_info_with(&pool, 9999).await.is_none());

            pool.close().await;
            let _ = std::fs::remove_file(&path);
        });
    }

    #[test]
    fn test_attach_points_groups_and_ignores_orphans() {
        let _g = crate::tests::TEST_LOCK.lock().unwrap();
        let strokes = vec![
            StrokeData {
                id: 2,
                r: 1.0,
                g: 0.0,
                b: 0.0,
                width_scale: 1.0,
                points: vec![],
            },
            StrokeData {
                id: 5,
                r: 0.0,
                g: 1.0,
                b: 0.0,
                width_scale: 1.5,
                points: vec![],
            },
        ];
        let pts = vec![
            (2, 0, 0.0, 0.0, 2.0, 0.0),
            (2, 1, 1.0, 1.0, 3.0, 0.1),
            (5, 0, 10.0, 10.0, 2.0, 0.0),
            (5, 1, 20.0, 20.0, 2.0, 0.5),
            (99, 0, 9.0, 9.0, 9.0, 9.0), // orphan (only possible after the known strokes in the real JOIN query)
        ];
        let out = attach_points(strokes, pts);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].points.len(), 2);
        assert_eq!(out[0].points[1], (1.0, 1.0, 3.0, 0.1));
        assert_eq!(out[1].points.len(), 2);
        assert_eq!(out[1].points[0], (10.0, 10.0, 2.0, 0.0));
        // stroke without points keeps an empty list
        let empty = attach_points(
            vec![StrokeData {
                id: 1,
                r: 0.0,
                g: 0.0,
                b: 0.0,
                width_scale: 1.0,
                points: vec![],
            }],
            vec![],
        );
        assert!(empty[0].points.is_empty());
    }

    /// Strokes + thumbnails tables mirroring the real migration, for tests
    /// that exercise the thumbnail cache without touching the global DB.
    async fn add_thumb_tables(pool: &SqlitePool) {
        sqlx::query(
            "CREATE TABLE strokes (
                id INTEGER PRIMARY KEY,
                screen_id INTEGER NOT NULL REFERENCES screens(id),
                color_r REAL NOT NULL,
                color_g REAL NOT NULL,
                color_b REAL NOT NULL,
                width_scale REAL NOT NULL DEFAULT 1.0,
                created_at REAL NOT NULL,
                deleted_at REAL
            )",
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE screen_thumbnails (
                screen_id INTEGER NOT NULL REFERENCES screens(id),
                max_size INTEGER NOT NULL,
                stroke_count INTEGER NOT NULL,
                max_stroke_id INTEGER NOT NULL,
                outline INTEGER NOT NULL DEFAULT 0,
                png BLOB NOT NULL,
                generated_at REAL NOT NULL,
                PRIMARY KEY (screen_id, max_size)
            )",
        )
        .execute(pool)
        .await
        .unwrap();
    }

    async fn add_stroke(pool: &SqlitePool, screen_id: i64, deleted: bool) -> i64 {
        sqlx::query_scalar::<_, i64>(
            "INSERT INTO strokes (screen_id, color_r, color_g, color_b, width_scale, created_at, deleted_at) \
             VALUES (?1, 1.0, 0.0, 0.0, 1.0, 1.0, ?2) RETURNING id",
        )
        .bind(screen_id)
        .bind(if deleted { Some(1.0f64) } else { None })
        .fetch_one(pool)
        .await
        .unwrap()
    }

    #[test]
    fn test_thumbnail_cache_lookup_store_purge() {
        let _g = crate::tests::TEST_LOCK.lock().unwrap();
        runtime().block_on(async {
            let (pool, path) = temp_pool().await;
            add_thumb_tables(&pool).await;
            let sid = add_screen(&pool, 1.0).await;
            let s1 = add_stroke(&pool, sid, false).await;
            let _s2 = add_stroke(&pool, sid, true).await; // soft-deleted: excluded

            // Version counts only live strokes
            let (count, max_id) = screen_stroke_version_with(&pool, sid).await;
            assert_eq!((count, max_id), (1, s1));

            // Miss → store → hit
            let png_a = vec![1u8, 2, 3];
            assert!(
                thumbnail_lookup_with(&pool, sid, 280, count, max_id, false)
                    .await
                    .is_none()
            );
            thumbnail_store_with(&pool, sid, 280, count, max_id, false, &png_a).await;
            assert_eq!(
                thumbnail_lookup_with(&pool, sid, 280, count, max_id, false).await,
                Some(png_a.clone())
            );

            // Any version-key change invalidates: outline / max_size / content
            assert!(
                thumbnail_lookup_with(&pool, sid, 280, count, max_id, true)
                    .await
                    .is_none()
            );
            assert!(
                thumbnail_lookup_with(&pool, sid, 128, count, max_id, false)
                    .await
                    .is_none()
            );
            let _s3 = add_stroke(&pool, sid, false).await;
            let (count2, max_id2) = screen_stroke_version_with(&pool, sid).await;
            assert_ne!((count2, max_id2), (count, max_id));
            assert!(
                thumbnail_lookup_with(&pool, sid, 280, count2, max_id2, false)
                    .await
                    .is_none()
            );

            // Re-store same key replaces (upsert), other max_size variant coexists
            let png_b = vec![9u8, 8, 7];
            thumbnail_store_with(&pool, sid, 280, count2, max_id2, false, &png_b).await;
            thumbnail_store_with(&pool, sid, 128, count2, max_id2, false, &png_b).await;
            assert_eq!(
                thumbnail_lookup_with(&pool, sid, 280, count2, max_id2, false).await,
                Some(png_b)
            );
            assert!(
                thumbnail_lookup_with(&pool, sid, 128, count2, max_id2, false)
                    .await
                    .is_some()
            );

            // Purge drops both variants
            thumbnails_purge_screen_with(&pool, sid).await;
            assert!(
                thumbnail_lookup_with(&pool, sid, 280, count2, max_id2, false)
                    .await
                    .is_none()
            );
            assert!(
                thumbnail_lookup_with(&pool, sid, 128, count2, max_id2, false)
                    .await
                    .is_none()
            );

            pool.close().await;
            let _ = std::fs::remove_file(&path);
        });
    }

    /// The batched variants must agree with the per-screen ones, including
    /// soft-delete handling, size/outline keying and missing screens.
    #[test]
    fn test_thumbnail_batch_helpers() {
        let _g = crate::tests::TEST_LOCK.lock().unwrap();
        runtime().block_on(async {
            let (pool, path) = temp_pool().await;
            add_thumb_tables(&pool).await;
            let a = add_screen(&pool, 1.0).await;
            let b = add_screen(&pool, 1.0).await;
            let a1 = add_stroke(&pool, a, false).await;
            let _a2 = add_stroke(&pool, a, true).await; // soft-deleted: excluded
            let b1 = add_stroke(&pool, b, false).await;

            let ids = [a, b, 9999]; // 9999 has no strokes
            let versions = stroke_versions_many_with(&pool, &ids).await;
            assert_eq!(versions.get(&a), Some(&(1, a1)));
            assert_eq!(versions.get(&b), Some(&(1, b1)));
            assert!(!versions.contains_key(&9999));
            assert!(stroke_versions_many_with(&pool, &[]).await.is_empty());

            // Empty cache → nothing, and other sizes/outlines never leak in
            assert!(
                thumbnails_many_with(&pool, &ids, 280, false)
                    .await
                    .is_empty()
            );
            thumbnail_store_with(&pool, a, 280, 1, a1, false, &[1, 2, 3]).await;
            thumbnail_store_with(&pool, b, 128, 1, b1, false, &[4, 5, 6]).await;
            thumbnail_store_with(&pool, b, 280, 1, b1, true, &[7, 8, 9]).await;

            let got = thumbnails_many_with(&pool, &ids, 280, false).await;
            assert_eq!(got.len(), 1);
            assert_eq!(got.get(&a), Some(&(1, a1, vec![1u8, 2, 3])));
            assert!(
                thumbnails_many_with(&pool, &[b], 280, false)
                    .await
                    .is_empty()
            );
            assert_eq!(
                thumbnails_many_with(&pool, &[b], 128, false).await.get(&b),
                Some(&(1, b1, vec![4u8, 5, 6]))
            );
            assert_eq!(
                thumbnails_many_with(&pool, &[b], 280, true).await.get(&b),
                Some(&(1, b1, vec![7u8, 8, 9]))
            );

            // Purge is still per screen and drops the batch view too
            thumbnails_purge_screen_with(&pool, a).await;
            assert!(
                thumbnails_many_with(&pool, &ids, 280, false)
                    .await
                    .is_empty()
            );

            pool.close().await;
            let _ = std::fs::remove_file(&path);
        });
    }
    async fn user_version(pool: &SqlitePool) -> i32 {
        sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn has_column(pool: &SqlitePool, table: &str, column: &str) -> bool {
        use sqlx::Row;
        sqlx::query(&format!("PRAGMA table_info({table})"))
            .fetch_all(pool)
            .await
            .unwrap()
            .iter()
            .any(|r| {
                r.try_get::<String, _>("name")
                    .map(|n| n == column)
                    .unwrap_or(false)
            })
    }

    /// schema 版本与迁移: 全新库、老库升级、重复执行、以及"更新版本写过的库"。
    /// 这是数据安全的地基 —— 迁移失败必须中止, 而不是带着半个 schema 继续跑。
    #[test]
    fn test_schema_version_and_migrations() {
        let _g = crate::tests::TEST_LOCK.lock().unwrap();
        runtime().block_on(async {
            // 1) 全新库: 建表后版本号落到 SCHEMA_VERSION, 且重复执行不报错
            let (pool, path) = raw_temp_pool().await;
            migrate_with(&pool).await.expect("全新库迁移");
            assert_eq!(user_version(&pool).await, SCHEMA_VERSION);
            migrate_with(&pool).await.expect("重复迁移应幂等(每次启动都会跑)");
            pool.close().await;
            let _ = std::fs::remove_file(&path);

            // 2) 老库: 缺 edited / deleted_at / t, 版本 0 → 迁移补齐并升版本
            let (pool, path) = raw_temp_pool().await;
            for sql in [
                "CREATE TABLE screens (id INTEGER PRIMARY KEY AUTOINCREMENT, created_at REAL NOT NULL, screen_w INTEGER NOT NULL, screen_h INTEGER NOT NULL)",
                "CREATE TABLE strokes (id INTEGER PRIMARY KEY, screen_id INTEGER NOT NULL, color_r REAL NOT NULL, color_g REAL NOT NULL, color_b REAL NOT NULL, width_scale REAL NOT NULL DEFAULT 1.0, created_at REAL NOT NULL)",
                "CREATE TABLE points (stroke_id INTEGER NOT NULL, seq INTEGER NOT NULL, x REAL NOT NULL, y REAL NOT NULL, width REAL NOT NULL, PRIMARY KEY (stroke_id, seq))",
                "CREATE TABLE infinite_strokes (id INTEGER PRIMARY KEY, color_r REAL NOT NULL, color_g REAL NOT NULL, color_b REAL NOT NULL, width_scale REAL NOT NULL DEFAULT 1.0, created_at REAL NOT NULL)",
            ] {
                sqlx::query(sql).execute(&pool).await.unwrap();
            }
            migrate_with(&pool).await.expect("老库升级");
            assert_eq!(user_version(&pool).await, SCHEMA_VERSION);
            for (table, column) in [
                ("screens", "edited"),
                ("screens", "deleted_at"),
                ("strokes", "deleted_at"),
                ("infinite_strokes", "deleted_at"),
                ("points", "t"),
            ] {
                assert!(
                    has_column(&pool, table, column).await,
                    "迁移后仍缺列 {table}.{column}"
                );
            }
            pool.close().await;
            let _ = std::fs::remove_file(&path);

            // 3) 更新版本的 glaspen2 写过的库: 必须拒绝打开
            let (pool, path) = raw_temp_pool().await;
            sqlx::query(&format!("PRAGMA user_version = {}", SCHEMA_VERSION + 1))
                .execute(&pool)
                .await
                .unwrap();
            let err = migrate_with(&pool)
                .await
                .expect_err("来自更新版本的库必须被拒绝");
            assert!(
                err.contains("请升级程序"),
                "错误信息要能直接指导用户, 实际是: {err}"
            );
            pool.close().await;
            let _ = std::fs::remove_file(&path);
        });
    }
}
