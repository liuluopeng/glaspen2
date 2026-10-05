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
    // 调试/测试用:GLASPEN2_DB_PATH=/tmp/x.db 指定库文件,
    // 虚拟笔性能剖析、迁移试验等场景就不会写进真实数据。
    if let Some(p) = std::env::var_os("GLASPEN2_DB_PATH") {
        return std::path::PathBuf::from(p);
    }

    let exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let exe_dir = exe.parent().unwrap_or_else(|| std::path::Path::new("."));

    // Windows: cargo 构建产物(target/debug、target/release)里的数据库会被
    // cargo clean 连锅端掉。dev/本地构建改用固定的 %LOCALAPPDATA%\glaspen2\
    // glaspen2-dev.db(与 macOS dev 的隔离原则一致),首次运行把 target 里的
    // 旧库整体搬过去。安装版 exe 不在 target 下,维持 exe 同目录不变。
    #[cfg(target_os = "windows")]
    if exe_dir
        .ancestors()
        .any(|a| a.file_name() == Some(std::ffi::OsStr::new("target")))
        && let Some(app_data) = windows_app_data_dir()
    {
        let new_path = app_data.join("glaspen2-dev.db");
        let legacy = exe_dir.join("glaspen2.db");
        migrate_legacy_db(&legacy, &new_path);
        return new_path;
    }

    let is_bundled = exe_dir
        .ancestors()
        .any(|a| a.join("Contents").join("Info.plist").exists());

    // 非 bundled(dev cargo run)也用稳定的库路径: 曾经放在 exe 同目录
    // (target/debug/), 一次 cargo clean 就把整个开发库连备份清掉了。
    // dev 库与安装版隔离(glaspen2-dev.db), 但路径不再随构建产物消失。
    if !is_bundled && let Some(app_support) = app_support_dir() {
        std::fs::create_dir_all(&app_support).ok();
        return app_support.join("glaspen2-dev.db");
    }

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

#[cfg(target_os = "windows")]
fn windows_app_data_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|base| {
        let dir = std::path::PathBuf::from(base).join("glaspen2");
        std::fs::create_dir_all(&dir).ok();
        dir
    })
}

/// target 里的旧库首次遇到持久位置时整体搬过去(.db + -wal + -shm)。
/// 持久位置已存在则一律以它为准,不用 target 里的旧数据覆盖。
#[cfg(target_os = "windows")]
fn migrate_legacy_db(legacy: &std::path::Path, new_path: &std::path::Path) {
    if !legacy.exists() || new_path.exists() {
        return;
    }
    if std::fs::copy(legacy, new_path).is_ok() {
        for suffix in ["-wal", "-shm"] {
            let mut src_name = legacy.as_os_str().to_os_string();
            src_name.push(suffix);
            let src = std::path::PathBuf::from(src_name);
            if src.exists() {
                let mut dst_name = new_path.as_os_str().to_os_string();
                dst_name.push(suffix);
                let _ = std::fs::copy(&src, std::path::PathBuf::from(dst_name));
            }
        }
        eprintln!(
            "[db] 已迁移开发库 {} -> {}",
            legacy.display(),
            new_path.display()
        );
    }
}

fn now_f64() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// 诊断日志:全部 CRUD 一行一条,走 stderr(与 NSLog 的 [pen]/[ethereal]
/// 同一流,cargo run 终端里按时间顺序交错可见)。只加在公开包装函数上,
/// 测试用的 `_with` 变体不打,免得 cargo test 输出刷屏。
macro_rules! dblog {
    ($($arg:tt)*) => {{
        // 默认静默: 反色模式下每次整笔重绘都会查相邻页, 日志会滚进屏幕上
        // 可见的终端 → SCStream 检测到"画面变化"再送帧 → 自激循环(日志
        // 自己制造自己的触发源)。需要排查时设 GLASPEN2_DB_LOG=1 恢复。
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if *ON.get_or_init(|| std::env::var("GLASPEN2_DB_LOG").is_ok()) {
            eprintln!("[db] {}", format_args!($($arg)*));
        }
    }};
}

// ---------------------------------------------------------------------------
// async sqlx (all platforms — the module itself is platform-agnostic;
// gating it to macOS/Windows broke Linux CI compilation of `pub use platform::*`)
// ---------------------------------------------------------------------------
mod platform {
    use crate::state;
    use sqlx::{Connection, Row, SqlitePool};
    use std::collections::HashMap;
    use std::sync::OnceLock;

    use super::{StrokeData, db_path, now_f64};

    static DB: OnceLock<SqlitePool> = OnceLock::new();

    /// 当前 schema 版本。**任何改变表结构的改动都要 +1**, 并在 `migrate_with`
    /// 里补一段迁移。
    ///
    /// 两个作用: 旧版程序打开新版写过的库时直接拒绝(而不是按旧 schema 读写出
    /// 错、把数据写坏); 迁移失败时定位到底停在哪一版。
    pub(crate) const SCHEMA_VERSION: i32 = 2;

    pub async fn init() {
        let path = db_path();
        dblog!("库文件: {}", path.display());
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

        // 规模日志: 页数/笔迹数一眼可见, "全新的数据库"一类问题当场暴露
        let (screens, strokes): (i64, i64) = sqlx::query_as(
            "SELECT (SELECT COUNT(*) FROM screens WHERE deleted_at IS NULL), \
                    (SELECT COUNT(*) FROM strokes WHERE deleted_at IS NULL)",
        )
        .fetch_one(&pool)
        .await
        .unwrap_or((0, 0));
        dblog!("现有 {screens} 页 / {strokes} 笔");

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
                edited INTEGER NOT NULL DEFAULT 0,
                order_index INTEGER NOT NULL DEFAULT 0
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
            // OCR 识别结果(axum PP-OCRv6 服务, docs/ocr-api.md):
            // 每页一条最新结果, 历史结果软删保留; boxes 表为将来
            // 带坐标的文本层预留(当前 HTTP API 只返回整页文本)。
            "CREATE TABLE IF NOT EXISTS ocr_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                screen_id INTEGER NOT NULL REFERENCES screens(id),
                full_text TEXT NOT NULL,
                created_at REAL NOT NULL,
                deleted_at REAL
            )",
            "CREATE TABLE IF NOT EXISTS ocr_boxes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                result_id INTEGER NOT NULL REFERENCES ocr_results(id),
                box_index INTEGER NOT NULL,
                text TEXT NOT NULL,
                x REAL NOT NULL, y REAL NOT NULL,
                w REAL NOT NULL, h REAL NOT NULL,
                confidence REAL NOT NULL DEFAULT 0.0
            )",
            "CREATE INDEX IF NOT EXISTS idx_ocr_results_screen ON ocr_results(screen_id)",
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
        // 页序 v2: 活页本可重排/插页。回填 = id 序(与迁移前完全一致,
        // 老库行为零变化); 之后新页追加到全局序列末尾, 面板可调序。
        add_column_if_missing(pool, "screens", "order_index", "INTEGER NOT NULL DEFAULT 0").await?;
        sqlx::query("UPDATE screens SET order_index = id WHERE order_index = 0")
            .execute(pool)
            .await
            .map_err(|e| format!("回填 order_index 失败: {e}"))?;

        // 注:旧版本曾在 screens 上存 per-page 镜头(pan_x/pan_y/zoom)。
        // 现在无限画布独立存储且全局只有一个画布,镜头改存 user_settings,
        // 这几列不再读写(旧库中残留的列保持不动,无副作用)。

        // 缩略图形态 v2:内容包围盒裁剪 → 整页等比(与统一卡片尺寸冲突,
        // 已放弃裁剪)。一次性清掉旧裁剪缓存, 标记防重跑(每次启动都走
        // 本函数, 不能无条件 DELETE 白费缓存)。
        let purged: Option<String> = sqlx::query_scalar(
            "SELECT value FROM user_settings WHERE key = 'thumbs_purged_v2'",
        )
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
        if purged.is_none() {
            sqlx::query("DELETE FROM screen_thumbnails")
                .execute(pool)
                .await
                .map_err(|e| format!("清理旧缩略图失败: {e}"))?;
            sqlx::query(
                "INSERT OR REPLACE INTO user_settings (key, value) VALUES ('thumbs_purged_v2', '1')",
            )
            .execute(pool)
            .await
            .map_err(|e| format!("写缩略图清理标记失败: {e}"))?;
            dblog!("缩略图缓存已一次性清理(裁剪 → 整页形态)");
        }

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
        match sqlx::query_scalar::<_, i64>(
            "INSERT INTO screens (created_at, screen_w, screen_h, order_index)              VALUES (?1, ?2, ?3,              COALESCE((SELECT MAX(order_index) FROM screens WHERE deleted_at IS NULL), 0) + 1)              RETURNING id",
        )
        .bind(now)
        .bind(screen_w)
        .bind(screen_h)
        .fetch_one(pool)
        .await
        {
            Ok(sid) => {
                state::set_current_screen_id(sid);
                dblog!("页+ id={sid} ({screen_w}x{screen_h})");
            }
            Err(e) => dblog!("ERR 新建页失败: {e}"),
        }
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
                    dblog!("笔迹+ id={id} 画布=无限");
                    id
                }
                Ok(None) => {
                    dblog!("ERR 落笔 INSERT 无返回行 (无限)");
                    0
                }
                Err(e) => {
                    dblog!("ERR 落笔 INSERT 失败 (无限): {e}");
                    0
                }
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
                dblog!("笔迹+ id={id} screen={screen_id}");
                // 标记"这页被编辑过"与笔画行无关, 没人需要它立刻可见 →
                // 后台写掉。原来紧跟 INSERT 同步跑, 落笔多付一整次 SQL 往返
                // (实测 pen_down 969µs, 其中约一半是这条 UPDATE)。
                crate::runtime().spawn(async move {
                    if let Some(pool) = DB.get() {
                        sqlx::query("UPDATE screens SET edited = 1 WHERE id = ?1")
                            .bind(screen_id)
                            .execute(pool)
                            .await
                            .ok();
                    }
                });
                id
            }
            Ok(None) => {
                dblog!("ERR 落笔 INSERT 无返回行 screen={screen_id}");
                0
            }
            Err(e) => {
                dblog!("ERR 落笔 INSERT 失败 screen={screen_id}: {e}");
                0
            }
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
            // 空笔迹:表行已建但一个点都没有 —— "幽灵落笔"(漏 UP、驱动
            // 抖动)的签名,诊断时必须可见。
            dblog!("点+ stroke={stroke_id} n=0 (空笔迹,仅表行)");
            return;
        }
        let pool = match DB.get() {
            Some(p) => p,
            None => {
                dblog!(
                    "ERR flush stroke={stroke_id}: DB 未初始化, {n} 个点丢失",
                    n = points.len()
                );
                return;
            }
        };
        let mut tx = match pool.begin().await {
            Ok(t) => t,
            Err(e) => {
                dblog!("ERR flush 开事务 stroke={stroke_id}: {e}");
                return;
            }
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
            if let Err(e) = q.execute(&mut *tx).await {
                dblog!("ERR flush 批量插入 stroke={stroke_id} 行={}: {e}", done);
            }
            done = end;
        }
        match tx.commit().await {
            Ok(_) => dblog!("点+ stroke={stroke_id} n={} ({})", points.len(), table),
            Err(e) => dblog!("ERR flush 提交 stroke={stroke_id}: {e}"),
        }
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

    /// 活页本末页 = 未删除页中 id 最大者(含空白页)。空库返回 None。
    /// "末页空白就不能再建空白页"的守卫与启动沿用末页都用它。
    pub async fn last_screen_id() -> Option<i64> {
        let pool = match DB.get() {
            Some(p) => p,
            None => return None,
        };
        let r: Option<i64> = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT MAX(id) FROM screens WHERE deleted_at IS NULL",
        )
        .fetch_one(pool)
        .await
        .ok()
        .flatten();
        dblog!("末页 id → {:?}", r);
        r
    }

    /// 软删活页本的所有空白页(没有任何未删除笔迹的页)。返回清理的页数。
    /// 启动时调用一次:配合"末页空白不能再建空白页"守卫,空白页只在
    /// 当前会话内瞬时存在,不会跨启动积累。
    pub async fn purge_blank_screens() -> u64 {
        let pool = match DB.get() {
            Some(p) => p,
            None => return 0,
        };
        match sqlx::query(
            "UPDATE screens SET deleted_at = ?1 \
             WHERE deleted_at IS NULL \
             AND NOT EXISTS (SELECT 1 FROM strokes \
                             WHERE strokes.screen_id = screens.id AND strokes.deleted_at IS NULL)",
        )
        .bind(now_f64())
        .execute(pool)
        .await
        {
            Ok(r) => {
                if r.rows_affected() > 0 {
                    dblog!("清理空白页 {} 页", r.rows_affected());
                }
                r.rows_affected()
            }
            Err(e) => {
                dblog!("ERR 清理空白页: {e}");
                0
            }
        }
    }

    /// 保存某页的 OCR 识别结果:软删该页旧结果后插入新结果(axum OCR 服务)。
    pub async fn save_ocr_result(screen_id: i64, full_text: &str) {
        let pool = match DB.get() {
            Some(p) => p,
            None => return,
        };
        let now = now_f64();
        if let Err(e) = sqlx::query(
            "UPDATE ocr_results SET deleted_at = ?2 WHERE screen_id = ?1 AND deleted_at IS NULL",
        )
        .bind(screen_id)
        .bind(now)
        .execute(pool)
        .await
        {
            dblog!("ERR OCR 旧结果软删 screen={screen_id}: {e}");
            return;
        }
        match sqlx::query(
            "INSERT INTO ocr_results (screen_id, full_text, created_at) VALUES (?1, ?2, ?3)",
        )
        .bind(screen_id)
        .bind(full_text)
        .bind(now)
        .execute(pool)
        .await
        {
            Ok(_) => dblog!("OCR存 screen={screen_id} 字数={}", full_text.len()),
            Err(e) => dblog!("ERR OCR 存结果 screen={screen_id}: {e}"),
        }
    }

    /// OCR 全文搜索: 返回匹配页 id(升序)。查询按子串匹配(LIKE)。
    pub async fn ocr_search_ids(query: &str) -> Vec<i64> {
        let Some(pool) = DB.get() else {
            return Vec::new();
        };
        if query.trim().is_empty() {
            return Vec::new();
        }
        let r = sqlx::query_scalar::<_, i64>(
            "SELECT DISTINCT screen_id FROM ocr_results \
             WHERE deleted_at IS NULL AND full_text LIKE '%' || ?1 || '%' \
             ORDER BY screen_id",
        )
        .bind(query.trim())
        .fetch_all(pool)
        .await
        .unwrap_or_default();
        dblog!("OCR搜索 {:?} → {} 页", query, r.len());
        r
    }

    /// 某页最新的 OCR 全文(未删除的最新一条);无则 None。
    pub async fn latest_ocr_text(screen_id: i64) -> Option<String> {
        let pool = match DB.get() {
            Some(p) => p,
            None => return None,
        };
        let r = sqlx::query_scalar::<_, String>(
            "SELECT full_text FROM ocr_results              WHERE screen_id = ?1 AND deleted_at IS NULL ORDER BY id DESC LIMIT 1",
        )
        .bind(screen_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);
        dblog!(
            "OCR读 screen={screen_id} → {}",
            r.as_ref()
                .map(|t| t.len())
                .map_or("无".into(), |n| format!("{n}字"))
        );
        r
    }

    /// 还没有 OCR 结果的页(未删除、有笔迹、无未删除 OCR 行),按 id 升序。
    /// 供 PDF 导出前的批量补全与启动 backfill 使用。
    pub async fn pages_missing_ocr() -> Vec<(i64, i32, i32)> {
        let pool = match DB.get() {
            Some(p) => p,
            None => return Vec::new(),
        };
        let r = sqlx::query_as(
            "SELECT s.id, s.screen_w, s.screen_h FROM screens s              WHERE s.deleted_at IS NULL              AND EXISTS (SELECT 1 FROM strokes WHERE screen_id = s.id)              AND NOT EXISTS (SELECT 1 FROM ocr_results WHERE screen_id = s.id AND deleted_at IS NULL)              ORDER BY s.id",
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default();
        dblog!("OCR缺页 → {} 页", r.len());
        r
    }

    /// 某页的尺寸(补占位页沿用当前页尺寸; OCR 渲染整页用)。
    pub async fn screen_dims(screen_id: i64) -> Option<(i32, i32)> {
        let pool = match DB.get() {
            Some(p) => p,
            None => return None,
        };
        let r = sqlx::query_as("SELECT screen_w, screen_h FROM screens WHERE id = ?1")
            .bind(screen_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);
        dblog!("页尺寸 {screen_id} → {:?}", r);
        r
    }

    pub async fn screen_has_strokes(screen_id: i64) -> bool {
        let pool = match DB.get() {
            Some(p) => p,
            None => return false,
        };
        let r = sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM strokes WHERE screen_id = ?1 AND deleted_at IS NULL)",
        )
        .bind(screen_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0)
            != 0;
        dblog!("页有无笔迹 {screen_id} → {r}");
        r
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
        .map(|v| v != 0)
        .inspect(|r| dblog!("页曾编辑 {screen_id} → {r}"))
        .unwrap_or_else(|| {
            dblog!("页曾编辑 {screen_id} → 无此页");
            false
        })
    }

    /// Delete a stroke by id. Returns true if the stroke existed.
    pub async fn delete_stroke_by_id(stroke_id: i64) -> bool {
        let pool = match DB.get() {
            Some(p) => p,
            None => return false,
        };
        // 软删除:标记而非物理删除(数据可恢复)
        let now = now_f64();
        match sqlx::query("UPDATE strokes SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL")
            .bind(stroke_id)
            .bind(now)
            .execute(pool)
            .await
        {
            Ok(r) => {
                dblog!("笔迹- id={stroke_id} ({})", r.rows_affected());
                r.rows_affected() > 0
            }
            Err(e) => {
                dblog!("ERR 删笔迹 id={stroke_id}: {e}");
                false
            }
        }
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
            _ => {
                dblog!("撤销末笔 screen={screen_id} → 无笔迹");
                return false;
            }
        };
        delete_stroke_by_id(stroke_id).await
    }

    pub async fn delete_screen(target_id: i64) -> bool {
        let pool = match DB.get() {
            Some(p) => p,
            None => return false,
        };
        // 软删除:标记 screens + strokes + ocr_results 而非物理删除
        let now = now_f64();
        let ocr_del = sqlx::query(
            "UPDATE ocr_results SET deleted_at = ?2 WHERE screen_id = ?1 AND deleted_at IS NULL",
        )
        .bind(target_id)
        .bind(now)
        .execute(pool)
        .await;
        let screen_del =
            sqlx::query("UPDATE screens SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL")
                .bind(target_id)
                .bind(now)
                .execute(pool)
                .await;
        let stroke_del = sqlx::query(
            "UPDATE strokes SET deleted_at = ?2 WHERE screen_id = ?1 AND deleted_at IS NULL",
        )
        .bind(target_id)
        .bind(now)
        .execute(pool)
        .await;
        thumbnails_purge_screen(target_id).await;
        match (&screen_del, &stroke_del) {
            (Ok(s), Ok(k)) => dblog!(
                "页- id={target_id} (页{}, 笔迹{}, OCR {})",
                s.rows_affected(),
                k.rows_affected(),
                ocr_del.as_ref().map(|r| r.rows_affected()).unwrap_or(0)
            ),
            (Err(e), _) | (_, Err(e)) => dblog!("ERR 删页 id={target_id}: {e}"),
        }
        screen_del.is_ok() || stroke_del.is_ok()
    }

    /// 上一页/下一页: **只在同几何组内翻**(页 = 某块玻璃的快照, 不同
    /// 分辨率是不同的本子; 翻页不跨玻璃)。组 = 当前页的 screen_w×screen_h。
    pub async fn prev_screen(current: i64) -> Option<i64> {
        match DB.get() {
            Some(p) => prev_screen_with(p, current).await,
            None => None,
        }
    }

    pub(crate) async fn prev_screen_with(pool: &SqlitePool, current: i64) -> Option<i64> {
        let r = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM screens WHERE deleted_at IS NULL \
             AND screen_w = (SELECT screen_w FROM screens WHERE id = ?1) \
             AND screen_h = (SELECT screen_h FROM screens WHERE id = ?1) \
             AND order_index < (SELECT order_index FROM screens WHERE id = ?1) \
             ORDER BY order_index DESC LIMIT 1",
        )
        .bind(current)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
        dblog!("上一页 cur={current} → {:?}", r);
        r
    }

    pub async fn next_screen(current: i64) -> Option<i64> {
        match DB.get() {
            Some(p) => next_screen_with(p, current).await,
            None => None,
        }
    }

    pub(crate) async fn next_screen_with(pool: &SqlitePool, current: i64) -> Option<i64> {
        let r = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM screens WHERE deleted_at IS NULL \
             AND screen_w = (SELECT screen_w FROM screens WHERE id = ?1) \
             AND screen_h = (SELECT screen_h FROM screens WHERE id = ?1) \
             AND order_index > (SELECT order_index FROM screens WHERE id = ?1) \
             ORDER BY order_index ASC LIMIT 1",
        )
        .bind(current)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
        dblog!("下一页 cur={current} → {:?}", r);
        r
    }

    /// 指定几何组的末页(没有则 None)。分辨率切换"进入对应本子"用。
    pub async fn last_screen_with_geometry(w: i32, h: i32) -> Option<i64> {
        match DB.get() {
            Some(p) => last_screen_with_geometry_with(p, w, h).await,
            None => None,
        }
    }

    pub(crate) async fn last_screen_with_geometry_with(
        pool: &SqlitePool,
        w: i32,
        h: i32,
    ) -> Option<i64> {
        let r = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT id FROM screens WHERE deleted_at IS NULL AND screen_w = ?1 AND screen_h = ?2 \
             ORDER BY order_index DESC LIMIT 1",
        )
        .bind(w)
        .bind(h)
        .fetch_one(pool)
        .await
        .ok()
        .flatten();
        dblog!("组末页 {w}x{h} → {:?}", r);
        r
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
            dblog!("载入页 {screen_id} → 0 笔/0 点");
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
        let total_pts = pts.len();
        dblog!("载入页 {screen_id} → {} 笔/{} 点", strokes.len(), total_pts);
        attach_points(strokes, pts)
    }

    // ── 全量备份 / 回导 ──────────────────────────────────────────
    // 备份用 SQLite 的 VACUUM INTO: 产出一个内容一致、已整理的独立库文件
    // (不受 WAL 影响), 换机时直接替换 glaspen2.db 即可。
    // 回导是**合并**(INSERT OR REPLACE): 不会删掉备份之后新画的内容。
    // user_settings 与 screen_thumbnails 不参与(前者是偏好, 后者是派生缓存)。

    const BACKUP_PREFIX: &str = "glaspen2_backup_";
    const BACKUP_SUFFIX: &str = ".db";

    /// 参与回导的表与列。显式列出而不是 SELECT *: 老库可能多出历史遗留列
    /// (例如 screens.pan_x/pan_y/zoom), 列数不一致会让合并失败。
    const RESTORE_TABLES: [(&str, &str); 5] = [
        (
            "screens",
            "id, created_at, screen_w, screen_h, edited, deleted_at",
        ),
        (
            "strokes",
            "id, screen_id, color_r, color_g, color_b, width_scale, created_at, deleted_at",
        ),
        ("points", "stroke_id, seq, x, y, width, t"),
        (
            "infinite_strokes",
            "id, color_r, color_g, color_b, width_scale, created_at, deleted_at",
        ),
        ("infinite_points", "stroke_id, seq, x, y, width, t"),
    ];

    fn backup_file_name() -> String {
        format!(
            "{BACKUP_PREFIX}{}{BACKUP_SUFFIX}",
            chrono::Local::now().format("%Y%m%d_%H%M%S")
        )
    }

    /// 备份到指定路径(目录自动创建; 目标已存在先删除, VACUUM INTO 要求目标不存在)。
    pub(crate) async fn backup_to_with(
        pool: &SqlitePool,
        path: &std::path::Path,
    ) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("创建目录失败: {e}"))?;
        }
        if path.exists() {
            std::fs::remove_file(path).map_err(|e| format!("覆盖旧备份失败: {e}"))?;
        }
        sqlx::query("VACUUM INTO ?1")
            .bind(path.to_string_lossy().to_string())
            .execute(pool)
            .await
            .map_err(|e| format!("备份失败: {e}"))?;
        Ok(())
    }

    /// 备份到桌面, 返回文件路径。
    pub async fn backup_now() -> Result<String, String> {
        let pool = DB.get().ok_or("数据库未初始化")?;
        let path = crate::desktop_path().join(backup_file_name());
        backup_to_with(pool, &path).await?;
        dblog!("备份 → {}", path.display());
        Ok(path.to_string_lossy().into_owned())
    }

    /// 桌面上最新的备份文件(没有则 None)。
    pub fn newest_backup() -> Option<std::path::PathBuf> {
        let mut best: Option<(std::time::SystemTime, std::path::PathBuf)> = None;
        for entry in std::fs::read_dir(crate::desktop_path()).ok()?.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with(BACKUP_PREFIX) || !name.ends_with(BACKUP_SUFFIX) {
                continue;
            }
            let Ok(modified) = entry.metadata().and_then(|m| m.modified()) else {
                continue;
            };
            if best.as_ref().is_none_or(|(t, _)| modified > *t) {
                best = Some((modified, entry.path()));
            }
        }
        best.map(|(_, path)| path)
    }

    /// 从备份合并恢复, 返回库里恢复后的页数。
    ///
    /// 备份文件先复制一份再迁移到当前 schema —— **绝不改动用户手上的备份**;
    /// 这一步同时验证它确实是个 glaspen2 库(表结构能被迁移)。
    pub(crate) async fn restore_merge_from_with(
        pool: &SqlitePool,
        path: &std::path::Path,
    ) -> Result<usize, String> {
        let tmp = std::env::temp_dir().join(format!(
            "glaspen2_restore_{}_{}.db",
            std::process::id(),
            chrono::Local::now().timestamp_millis().unsigned_abs() % 1_000_000
        ));
        let _ = std::fs::remove_file(&tmp);
        std::fs::copy(path, &tmp).map_err(|e| format!("读取备份失败: {e}"))?;

        let upgrade = async {
            let tmp_pool =
                SqlitePool::connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&tmp))
                    .await
                    .map_err(|e| format!("打开备份失败: {e}"))?;
            let result = migrate_with(&tmp_pool).await;
            tmp_pool.close().await;
            result
        }
        .await;
        if let Err(e) = upgrade {
            let _ = std::fs::remove_file(&tmp);
            return Err(format!("备份文件不可用: {e}"));
        }

        // 必须全程用同一条连接: ATTACH 是"连接级"的, 池里换一条连接就看不到
        // backup.* 了(测试里就是这么抓到 no such table: backup.screens 的)。
        let mut conn = match pool.acquire().await {
            Ok(c) => c,
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                return Err(format!("获取连接失败: {e}"));
            }
        };

        if let Err(e) = sqlx::query("ATTACH DATABASE ?1 AS backup")
            .bind(tmp.to_string_lossy().to_string())
            .execute(&mut *conn)
            .await
        {
            let _ = std::fs::remove_file(&tmp);
            return Err(format!("挂载备份失败: {e}"));
        }

        let merged = async {
            let mut tx = conn
                .begin()
                .await
                .map_err(|e| format!("开启事务失败: {e}"))?;
            for (table, cols) in RESTORE_TABLES {
                sqlx::query(&format!(
                    "INSERT OR REPLACE INTO {table} ({cols}) SELECT {cols} FROM backup.{table}"
                ))
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("合并 {table} 失败: {e}"))?;
            }
            tx.commit().await.map_err(|e| format!("提交失败: {e}"))
        }
        .await;

        let _ = sqlx::query("DETACH DATABASE backup")
            .execute(&mut *conn)
            .await;
        let _ = std::fs::remove_file(&tmp);
        merged?;

        let pages: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM screens WHERE deleted_at IS NULL")
                .fetch_one(pool)
                .await
                .unwrap_or(0);
        Ok(pages as usize)
    }

    /// 从桌面上最新的备份合并恢复, 返回 (备份路径, 页数)。
    pub async fn restore_latest_backup() -> Result<(String, usize), String> {
        let pool = DB.get().ok_or("数据库未初始化")?;
        let path = newest_backup().ok_or("桌面上没有 glaspen2_backup_*.db 备份文件")?;
        match restore_merge_from_with(pool, &path).await {
            Ok(pages) => {
                dblog!("回导 ← {} ({pages} 页)", path.display());
                Ok((path.to_string_lossy().into_owned(), pages))
            }
            Err(e) => {
                dblog!("ERR 回导 ← {}: {e}", path.display());
                Err(e)
            }
        }
    }

    // ── 缩略图缓存 ────────────────────────────────────────────────
    // 内容版本 = (非删除笔迹数, 最大笔迹 id)。笔迹只追加/软删,
    // 该二元组足以识别内容变化;配合 outline/max_size 一起判定缓存新鲜度。

    /// Per-screen content version for thumbnail freshness checks.
    pub async fn screen_stroke_version(screen_id: i64) -> (i64, i64) {
        let r = match DB.get() {
            Some(p) => screen_stroke_version_with(p, screen_id).await,
            None => (0, 0),
        };
        dblog!("内容版本 {screen_id} → ({}笔, max={})", r.0, r.1);
        r
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
        let r = thumbnail_lookup_with(
            DB.get()?,
            screen_id,
            max_size,
            stroke_count,
            max_stroke_id,
            outline,
        )
        .await;
        dblog!(
            "缩略图查 {screen_id} ({max_size}px{}) → {}",
            if outline { ",描边" } else { "" },
            r.as_ref().map_or("未命中", |_| "命中")
        );
        r
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
            dblog!("缩略图存 {screen_id} ({max_size}px) {} 字节", png.len());
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
        let r = match DB.get() {
            Some(pool) => stroke_versions_many_with(pool, ids).await,
            None => HashMap::new(),
        };
        dblog!("版本批量 {} 页 → {} 条", ids.len(), r.len());
        r
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
        let r = match DB.get() {
            Some(pool) => thumbnails_many_with(pool, ids, max_size, outline).await,
            None => HashMap::new(),
        };
        dblog!(
            "缩略图批量 {} 页 ({}px) → {} 条缓存",
            ids.len(),
            max_size,
            r.len()
        );
        r
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
            let n = thumbnails_purge_screen_with(pool, screen_id).await;
            dblog!("缩略图清 {screen_id} → {n} 条");
        }
    }

    pub(crate) async fn thumbnails_purge_screen_with(pool: &SqlitePool, screen_id: i64) -> u64 {
        match sqlx::query("DELETE FROM screen_thumbnails WHERE screen_id = ?1")
            .bind(screen_id)
            .execute(pool)
            .await
        {
            Ok(r) => r.rows_affected(),
            Err(e) => {
                dblog!("ERR 清缩略图 {screen_id}: {e}");
                0
            }
        }
    }

    pub async fn list_screens() -> Vec<(i64, i32, i32)> {
        let pool = match DB.get() {
            Some(p) => p,
            None => return Vec::new(),
        };
        let r = sqlx::query_as(
            "SELECT s.id, s.screen_w, s.screen_h FROM screens s \
             WHERE deleted_at IS NULL \
             AND EXISTS (SELECT 1 FROM strokes WHERE screen_id = s.id) \
             ORDER BY s.order_index, s.id",
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default();
        dblog!("页列表 → {} 页", r.len());
        r
    }

    /// 活页本重排: 把某页移到锚点页前/后(面板内前后移)。全库活页序列
    /// 重编号为 1..N。锚点不存在时报错(跨笔记本误传的调用拦在这里)。
    pub async fn reorder_screen(
        screen_id: i64,
        anchor_id: i64,
        before: bool,
    ) -> Result<(), String> {
        match DB.get() {
            Some(p) => reorder_screen_with(p, screen_id, anchor_id, before).await,
            None => Err("DB not initialized".into()),
        }
    }

    pub(crate) async fn reorder_screen_with(
        pool: &SqlitePool,
        screen_id: i64,
        anchor_id: i64,
        before: bool,
    ) -> Result<(), String> {
        if screen_id == anchor_id {
            return Ok(());
        }
        let mut tx = pool.begin().await.map_err(|e| format!("开启事务失败: {e}"))?;
        let ids: Vec<i64> = sqlx::query_scalar(
            "SELECT id FROM screens WHERE deleted_at IS NULL ORDER BY order_index, id",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| format!("读取页序失败: {e}"))?;
        let Some(from) = ids.iter().position(|&id| id == screen_id) else {
            return Err(format!("页 {screen_id} 不存在或已删除"));
        };
        let mut ordered = ids;
        ordered.remove(from);
        match ordered.iter().position(|&id| id == anchor_id) {
            Some(pos) => {
                ordered.insert(if before { pos } else { pos + 1 }, screen_id);
            }
            None => return Err(format!("锚点页 {anchor_id} 不存在或已删除")),
        }
        for (i, &id) in ordered.iter().enumerate() {
            sqlx::query("UPDATE screens SET order_index = ?1 WHERE id = ?2")
                .bind(i as i64 + 1)
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("写入页序失败: {e}"))?;
        }
        tx.commit().await.map_err(|e| format!("提交事务失败: {e}"))?;
        dblog!("页重排 {screen_id} → 锚点 {anchor_id} {}", if before { "前" } else { "后" });
        Ok(())
    }

    /// 同页平移所选笔迹(面板圈选移动)。points 行内 x/y 直接加偏移。
    pub async fn move_strokes_translate(
        screen_id: i64,
        ids: &[i64],
        dx: f64,
        dy: f64,
    ) -> u64 {
        let Some(pool) = DB.get() else { return 0 };
        let n = ids.len();
        if n == 0 {
            return 0;
        }
        // 动态占位符: ids 是面板圈选结果, 数量小且受信
        let in_clause = ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        let sql = format!(
            "UPDATE points SET x = x + ?1, y = y + ?2 \
             WHERE stroke_id IN ({in_clause}) AND EXISTS (SELECT 1 FROM strokes \
             WHERE strokes.id = points.stroke_id AND strokes.screen_id = ?3 AND strokes.deleted_at IS NULL)"
        );
        match sqlx::query(&sql).bind(dx).bind(dy).bind(screen_id).execute(pool).await {
            Ok(r) => {
                dblog!("笔迹平移 {} 笔 (页 {screen_id}, dx={dx:.1} dy={dy:.1})", n);
                r.rows_affected()
            }
            Err(e) => {
                dblog!("ERR 笔迹平移: {e}");
                0
            }
        }
    }

    /// 跨页搬移所选笔迹: 改 screen_id + 平移。保 id 保时间(移动 = 保实体)。
    pub async fn move_strokes_to_screen(
        screen_id: i64,
        ids: &[i64],
        target_screen_id: i64,
        dx: f64,
        dy: f64,
    ) -> u64 {
        let Some(pool) = DB.get() else { return 0 };
        if ids.is_empty() || screen_id == target_screen_id {
            return 0;
        }
        let in_clause = ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        let sql = format!("UPDATE strokes SET screen_id = ?1 WHERE id IN ({in_clause}) \
             AND screen_id = ?2 AND deleted_at IS NULL");
        let Ok(r) = sqlx::query(&sql)
            .bind(target_screen_id)
            .bind(screen_id)
            .execute(pool)
            .await
        else {
            dblog!("ERR 笔迹搬页失败");
            return 0;
        };
        let moved = r.rows_affected();
        if moved > 0 {
            // 平移只作用于真正搬过去的笔迹
            let sql2 = format!(
                "UPDATE points SET x = x + ?1, y = y + ?2 WHERE stroke_id IN ({in_clause})"
            );
            if let Err(e) = sqlx::query(&sql2).bind(dx).bind(dy).execute(pool).await {
                dblog!("ERR 笔迹搬页平移: {e}");
            }
            dblog!("笔迹搬页 {} 笔 ({} → {})", moved, screen_id, target_screen_id);
        }
        moved
    }

    /// 软删所选笔迹(面板圈选删除)。返回软删笔数。
    pub async fn delete_strokes(screen_id: i64, ids: &[i64]) -> u64 {
        let Some(pool) = DB.get() else { return 0 };
        if ids.is_empty() {
            return 0;
        }
        let in_clause = ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        let sql = format!("UPDATE strokes SET deleted_at = ?1 \
             WHERE id IN ({in_clause}) AND screen_id = ?2 AND deleted_at IS NULL");
        match sqlx::query(&sql)
            .bind(now_f64())
            .bind(screen_id)
            .execute(pool)
            .await
        {
            Ok(r) => {
                dblog!("笔迹软删 {} 笔 (页 {screen_id})", r.rows_affected());
                r.rows_affected()
            }
            Err(e) => {
                dblog!("ERR 笔迹软删: {e}");
                0
            }
        }
    }

    /// 粘贴笔迹(新 id 新时间 = 新实体; 点内相对 t 重算起点为 0)。
    /// strokes = (颜色 r,g,b, 点列)。颜色随载荷保留 —— 复制红块粘出来
    /// 还是红色; 呈现宽度逐点随载荷(width_scale 档位存 1.0)。
    pub async fn paste_strokes(
        screen_id: i64,
        strokes: &[(f64, f64, f64, Vec<(f64, f64, f64)>)],
    ) -> usize {
        let Some(pool) = DB.get() else { return 0 };
        let now = now_f64();
        let mut created = 0usize;
        for &(r, g, b, ref pts) in strokes {
            let Ok(id) = sqlx::query_scalar::<_, i64>(
                "INSERT INTO strokes (screen_id, color_r, color_g, color_b, width_scale, created_at) \
                 VALUES (?1, ?2, ?3, ?4, 1.0, ?5) RETURNING id",
            )
            .bind(screen_id)
            .bind(r)
            .bind(g)
            .bind(b)
            .bind(now)
            .fetch_one(pool)
            .await
            else {
                dblog!("ERR 粘贴 INSERT 失败");
                continue;
            };
            let mut seq = 0i64;
            for &(x, y, w) in pts {
                if sqlx::query("INSERT INTO points (stroke_id, seq, x, y, width, t) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)")
                    .bind(id)
                    .bind(seq)
                    .bind(x)
                    .bind(y)
                    .bind(w)
                    .bind(0.0)
                    .execute(pool)
                    .await
                    .is_err()
                {
                    dblog!("ERR 粘贴点写入失败 stroke={id}");
                }
                seq += 1;
            }
            created += 1;
            dblog!("笔迹+ id={id} screen={screen_id} (粘贴, {} 点)", pts.len());
        }
        created
    }

    /// Page info for the 新建画布/翻页 notification:
    /// (nth-of-date, date_total, position, total, created_at).
    /// Date grouping uses local time (same calendar date = 今天/昨天/…).
    pub async fn page_info(screen_id: i64) -> Option<(i64, i64, i64, i64, f64)> {
        let pool = DB.get()?;
        let r = page_info_with(pool, screen_id).await;
        dblog!("页信息 {screen_id} → {:?}", r.as_ref().map(|i| (i.2, i.3)));
        r
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
                   AND s.order_index <= (SELECT order_index FROM screens WHERE id = ?1)) AS nth, \
               (SELECT COUNT(*) FROM screens s WHERE \
                   date(datetime(s.created_at,'unixepoch','localtime')) = \
                   (SELECT date(datetime(created_at,'unixepoch','localtime')) FROM screens WHERE id = ?1)) AS date_total, \
               (SELECT COUNT(*) FROM screens WHERE order_index <= \
                   (SELECT order_index FROM screens WHERE id = ?1) AND deleted_at IS NULL) AS pos, \
               (SELECT COUNT(*) FROM screens WHERE deleted_at IS NULL) AS total, \
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
        dblog!("载入无限 → {} 笔/{} 点", strokes.len(), pts.len());
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
        .await;
        match &deleted {
            Ok(r) => dblog!("无限笔迹- id={stroke_id} ({})", r.rows_affected()),
            Err(e) => dblog!("ERR 删无限笔迹 id={stroke_id}: {e}"),
        }
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
            _ => {
                dblog!("无限撤销末笔 → 无笔迹");
                return false;
            }
        };
        delete_infinite_stroke_by_id(stroke_id).await
    }

    pub async fn infinite_canvas_has_strokes() -> bool {
        let pool = match DB.get() {
            Some(p) => p,
            None => return false,
        };
        let r = sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM infinite_strokes WHERE deleted_at IS NULL)",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(0)
            != 0;
        dblog!("无限有无笔迹 → {r}");
        r
    }

    /// 清空无限画布的全部内容(不删画布本身,画布只有一个)。
    pub async fn clear_infinite_canvas() {
        let pool = match DB.get() {
            Some(p) => p,
            None => return,
        };
        // 软删除:标记而非物理删除
        let now = now_f64();
        match sqlx::query("UPDATE infinite_strokes SET deleted_at = ?1 WHERE deleted_at IS NULL")
            .bind(now)
            .execute(pool)
            .await
        {
            Ok(r) => dblog!("无限清空 {} 条", r.rows_affected()),
            Err(e) => dblog!("ERR 无限清空: {e}"),
        }
    }

    /// 当前页高度(逻辑 px)
    pub async fn page_height(screen_id: i64) -> Option<f64> {
        let pool = DB.get()?;
        let r = sqlx::query_scalar::<_, i32>("SELECT screen_h FROM screens WHERE id = ?1")
            .bind(screen_id)
            .fetch_optional(pool)
            .await
            .ok()?
            .map(|v| v as f64);
        dblog!("页高 {screen_id} → {:?}", r);
        r
    }

    /// 当前页的笔迹数（排除软删除）
    pub async fn page_stroke_count(screen_id: i64) -> u64 {
        let pool = match DB.get() {
            Some(p) => p,
            None => return 0,
        };
        let r = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM strokes WHERE screen_id = ?1 AND deleted_at IS NULL",
        )
        .bind(screen_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0) as u64;
        dblog!("页笔迹数 {screen_id} → {r}");
        r
    }

    /// 无限画布镜头变换(全局唯一,存 user_settings)。
    pub async fn set_infinite_transform(pan_x: f64, pan_y: f64, zoom: f64) {
        dblog!("无限镜头存 pan=({pan_x:.1},{pan_y:.1}) zoom={zoom:.3}");
        save_setting("infinite_pan_x", &format!("{pan_x:.6}")).await;
        save_setting("infinite_pan_y", &format!("{pan_y:.6}")).await;
        save_setting("infinite_zoom", &format!("{zoom:.6}")).await;
    }

    /// 无限画布镜头变换(未设置 → None,调用方用 0,0,1)。
    pub async fn get_infinite_transform() -> Option<(f64, f64, f64)> {
        let x = load_setting("infinite_pan_x").await?.parse::<f64>().ok()?;
        let y = load_setting("infinite_pan_y").await?.parse::<f64>().ok()?;
        let z = load_setting("infinite_zoom").await?.parse::<f64>().ok()?;
        dblog!("无限镜头读 pan=({x:.1},{y:.1}) zoom={z:.3}");
        Some((x, y, z))
    }

    pub async fn save_setting(key: &str, value: &str) {
        let pool = match DB.get() {
            Some(p) => p,
            None => {
                dblog!("ERR 设置写 {key}: DB 未初始化");
                return;
            }
        };
        match sqlx::query("INSERT OR REPLACE INTO user_settings (key, value) VALUES (?1, ?2)")
            .bind(key)
            .bind(value)
            .execute(pool)
            .await
        {
            Ok(_) => dblog!("设置写 {key}={value}"),
            Err(e) => dblog!("ERR 设置写 {key}: {e}"),
        }
    }

    pub async fn load_setting(key: &str) -> Option<String> {
        let pool = DB.get()?;
        let r = sqlx::query_scalar::<_, String>("SELECT value FROM user_settings WHERE key = ?1")
            .bind(key)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
        dblog!("设置读 {key} → {:?}", r);
        r
    }

    pub async fn save_settings(pen_r: f64, pen_g: f64, pen_b: f64, width_scale: f64) {
        let pool = match DB.get() {
            Some(p) => p,
            None => {
                dblog!("ERR 设置写画笔: DB 未初始化");
                return;
            }
        };
        for &(k, v) in &[
            ("pen_r", pen_r),
            ("pen_g", pen_g),
            ("pen_b", pen_b),
            ("width_scale", width_scale),
        ] {
            if let Err(e) =
                sqlx::query("INSERT OR REPLACE INTO user_settings (key, value) VALUES (?1, ?2)")
                    .bind(k)
                    .bind(format!("{:.6}", v))
                    .execute(pool)
                    .await
            {
                dblog!("ERR 设置写 {k}: {e}");
            }
        }
        dblog!("设置写画笔 rgb=({pen_r:.2},{pen_g:.2},{pen_b:.2}) w={width_scale:.2}");
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
        dblog!("设置读画笔 rgb=({r:.2},{g:.2},{b:.2}) w={ws:.2}");
        Some((r, g, b, ws))
    }
}

pub use platform::*;

#[cfg(test)]
mod tests {
    use super::StrokeData;
    use super::platform::{
        SCHEMA_VERSION, attach_points, backup_to_with, last_screen_with_geometry_with,
        migrate_with, next_screen_with, page_info_with, prev_screen_with,
        reorder_screen_with, restore_merge_from_with, screen_stroke_version_with,
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
                edited INTEGER NOT NULL DEFAULT 0,
                order_index INTEGER NOT NULL DEFAULT 0
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        (pool, path)
    }

    async fn add_screen(pool: &SqlitePool, ts: f64) -> i64 {
        sqlx::query_scalar::<_, i64>(
            "INSERT INTO screens (created_at, screen_w, screen_h, order_index) \
             VALUES (?1, 1920, 1080, COALESCE((SELECT MAX(order_index) FROM screens), 0) + 1) \
             RETURNING id",
        )
        .bind(ts)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn add_screen_at(pool: &SqlitePool, ts: f64, w: i32, h: i32) -> i64 {
        sqlx::query_scalar::<_, i64>(
            "INSERT INTO screens (created_at, screen_w, screen_h, order_index) \
             VALUES (?1, ?2, ?3, COALESCE((SELECT MAX(order_index) FROM screens), 0) + 1) \
             RETURNING id",
        )
        .bind(ts)
        .bind(w)
        .bind(h)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    #[test]
    fn test_page_nav_grouped_by_geometry() {
        let _g = crate::tests::TEST_LOCK.lock().unwrap();
        runtime().block_on(async {
            let (pool, path) = temp_pool().await;
            // temp_pool 刻意建旧 schema(给迁移测试用); 本测试需要
            // deleted_at → 先迁移到当前版本
            migrate_with(&pool).await.unwrap();
            // 页序列: 1(1920) 2(3440) 3(1920) 4(3440) —— 交错, 组内 id 不连续
            let p1 = add_screen_at(&pool, 1.0, 1920, 1080).await;
            let p2 = add_screen_at(&pool, 2.0, 3440, 1440).await;
            let p3 = add_screen_at(&pool, 3.0, 1920, 1080).await;
            let p4 = add_screen_at(&pool, 4.0, 3440, 1440).await;

            // 1920 组内: 1 ↔ 3 互为前后, 不串到 3440
            assert_eq!(prev_screen_with(&pool, p3).await, Some(p1));
            assert_eq!(next_screen_with(&pool, p1).await, Some(p3));
            assert_eq!(prev_screen_with(&pool, p1).await, None);
            // 3440 组内: 2 ↔ 4
            assert_eq!(prev_screen_with(&pool, p4).await, Some(p2));
            assert_eq!(next_screen_with(&pool, p4).await, None);

            // 组末页查询
            assert_eq!(
                last_screen_with_geometry_with(&pool, 1920, 1080).await,
                Some(p3)
            );
            assert_eq!(
                last_screen_with_geometry_with(&pool, 3440, 1440).await,
                Some(p4)
            );
            assert_eq!(
                last_screen_with_geometry_with(&pool, 1280, 1024).await,
                None
            );

            let _ = sqlx::query(&format!("DELETE FROM screens WHERE id = {}", p1))
                .execute(&pool)
                .await;
            let _ = std::fs::remove_file(&path);
        });
    }

    #[test]
    fn test_page_info_grouping_local_dates() {
        let _g = crate::tests::TEST_LOCK.lock().unwrap();
        runtime().block_on(async {
            let (pool, path) = temp_pool().await;
            // temp_pool 建旧 schema; page_info 的 SQL 引用 deleted_at /
            // order_index → 先迁移到当前版本
            migrate_with(&pool).await.unwrap();
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
                ("screens", "order_index"),
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
    #[test]
    fn test_reorder_screen_and_navigation() {
        let _g = crate::tests::TEST_LOCK.lock().unwrap();
        runtime().block_on(async {
            let (pool, path) = temp_pool().await;
            migrate_with(&pool).await.unwrap();
            let p1 = add_screen(&pool, 1000.0).await;
            let p2 = add_screen(&pool, 2000.0).await;
            let p3 = add_screen(&pool, 3000.0).await;

            // 初始序 [1,2,3]: p3 移到 p1 前 → [3,1,2]
            reorder_screen_with(&pool, p3, p1, true).await.unwrap();
            let order: Vec<i64> = sqlx::query_scalar(
                "SELECT id FROM screens WHERE deleted_at IS NULL ORDER BY order_index",
            )
            .fetch_all(&pool)
            .await
            .unwrap();
            assert_eq!(order, vec![p3, p1, p2], "重排后顺序");

            // 翻页跟随新序: p1 的上一页 = p3, 下一页 = p2
            assert_eq!(prev_screen_with(&pool, p1).await, Some(p3));
            assert_eq!(next_screen_with(&pool, p1).await, Some(p2));
            assert_eq!(prev_screen_with(&pool, p3).await, None, "p3 已是首页");

            // 移到末尾: p3 移到 p2 后 → [1,2,3]
            reorder_screen_with(&pool, p3, p2, false).await.unwrap();
            let order: Vec<i64> = sqlx::query_scalar(
                "SELECT id FROM screens WHERE deleted_at IS NULL ORDER BY order_index",
            )
            .fetch_all(&pool)
            .await
            .unwrap();
            assert_eq!(order, vec![p1, p2, p3]);

            // 序号密集重编号(1..N, 无空洞)
            let ois: Vec<i64> = sqlx::query_scalar(
                "SELECT order_index FROM screens WHERE deleted_at IS NULL ORDER BY order_index",
            )
            .fetch_all(&pool)
            .await
            .unwrap();
            assert_eq!(ois, vec![1, 2, 3]);

            // 软删页不参与: 删 p2 → [1,3], p1 的下一页 = p3
            sqlx::query("UPDATE screens SET deleted_at = 1 WHERE id = ?1")
                .bind(p2)
                .execute(&pool)
                .await
                .unwrap();
            assert_eq!(next_screen_with(&pool, p1).await, Some(p3));

            // 锚点不存在 → 报错(跨笔记本误传拦截)
            assert!(reorder_screen_with(&pool, p1, 9999, true).await.is_err());

            pool.close().await;
            let _ = std::fs::remove_file(&path);
        });
    }

    /// 备份 → 继续画 → 从备份合并恢复: 现有数据不丢, 备份里的内容回来。
    /// 这是"库损坏/换机不丢笔迹史"那条承诺的具体形式。
    #[test]
    fn test_backup_then_restore_merge() {
        let _g = crate::tests::TEST_LOCK.lock().unwrap();
        runtime().block_on(async {
            let (pool, path) = raw_temp_pool().await;
            migrate_with(&pool).await.unwrap();

            // 线上库: 2 页, 第 1 页一笔一点
            for _ in 0..2 {
                sqlx::query(
                    "INSERT INTO screens (created_at, screen_w, screen_h) VALUES (1.0, 100, 100)",
                )
                .execute(&pool)
                .await
                .unwrap();
            }
            sqlx::query(
                "INSERT INTO strokes (id, screen_id, color_r, color_g, color_b, width_scale, created_at) \
                 VALUES (1, 1, 0.0, 0.0, 0.0, 1.0, 1.0)",
            )
            .execute(&pool)
            .await
            .unwrap();
            sqlx::query("INSERT INTO points (stroke_id, seq, x, y, width, t) VALUES (1, 0, 1.0, 2.0, 3.0, 0.0)")
                .execute(&pool)
                .await
                .unwrap();

            // 备份
            let backup = path.with_extension("bak.db");
            backup_to_with(&pool, &backup).await.expect("备份应成功");
            assert!(backup.exists());

            // 备份文件本身必须是可打开的 glaspen2 库, 且带着当前 schema 版本
            let backup_pool = SqlitePool::connect_with(
                sqlx::sqlite::SqliteConnectOptions::new().filename(&backup),
            )
            .await
            .unwrap();
            assert_eq!(user_version(&backup_pool).await, SCHEMA_VERSION);
            let backup_screens: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM screens")
                .fetch_one(&backup_pool)
                .await
                .unwrap();
            assert_eq!(backup_screens, 2);
            backup_pool.close().await;

            // 备份之后又画了一页一笔(恢复时不能被抹掉)
            sqlx::query(
                "INSERT INTO screens (created_at, screen_w, screen_h) VALUES (1.0, 100, 100)",
            )
            .execute(&pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO strokes (id, screen_id, color_r, color_g, color_b, width_scale, created_at) \
                 VALUES (2, 3, 0.0, 0.0, 0.0, 1.0, 1.0)",
            )
            .execute(&pool)
            .await
            .unwrap();

            let pages = restore_merge_from_with(&pool, &backup)
                .await
                .expect("恢复应成功");
            assert_eq!(pages, 3, "合并恢复: 备份的 2 页 + 备份后新画的 1 页");
            let strokes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM strokes")
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(strokes, 2, "两边的笔迹都要在");
            let points: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM points")
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(points, 1, "备份里的点要回来");

            // 损坏/非备份文件必须被拒绝, 而不是把库写坏
            let junk = path.with_extension("junk.db");
            std::fs::write(&junk, b"this is not a database").unwrap();
            assert!(
                restore_merge_from_with(&pool, &junk).await.is_err(),
                "非数据库文件必须被拒绝"
            );

            // 来自更新版本的备份同样拒绝
            let future = path.with_extension("future.db");
            {
                let fp = SqlitePool::connect_with(
                    sqlx::sqlite::SqliteConnectOptions::new()
                        .filename(&future)
                        .create_if_missing(true),
                )
                .await
                .unwrap();
                sqlx::query(&format!("PRAGMA user_version = {}", SCHEMA_VERSION + 1))
                    .execute(&fp)
                    .await
                    .unwrap();
                fp.close().await;
            }
            let err = restore_merge_from_with(&pool, &future)
                .await
                .expect_err("更新版本的备份必须被拒绝");
            assert!(err.contains("请升级程序"), "错误信息要能指导用户: {err}");

            pool.close().await;
            for f in [&path, &backup, &junk, &future] {
                let _ = std::fs::remove_file(f);
            }
        });
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn migrate_legacy_db_copies_once_and_never_clobbers() {
        let dir = std::env::temp_dir().join(format!("glaspen2_migrate_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let legacy = dir.join("legacy.db");
        let new_path = dir.join("persistent.db");

        // 无旧库:什么都不做
        super::migrate_legacy_db(&legacy, &new_path);
        assert!(!new_path.exists());

        // 有旧库(含 -wal):整体搬过去
        std::fs::write(&legacy, b"db-bytes").unwrap();
        std::fs::write(dir.join("legacy.db-wal"), b"wal-bytes").unwrap();
        super::migrate_legacy_db(&legacy, &new_path);
        assert_eq!(std::fs::read(&new_path).unwrap(), b"db-bytes");
        assert_eq!(
            std::fs::read(dir.join("persistent.db-wal")).unwrap(),
            b"wal-bytes"
        );

        // 持久库已存在:旧库不得覆盖它
        std::fs::write(&legacy, b"stale").unwrap();
        super::migrate_legacy_db(&legacy, &new_path);
        assert_eq!(std::fs::read(&new_path).unwrap(), b"db-bytes");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
