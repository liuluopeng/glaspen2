//! 自动更新的"替换正在运行的旧版本"核心:帮手进程 (`glaspen2 --updater`)
//! 与新版首启收尾 ([`finish_pending`])。
//!
//! # 关键约束
//!
//! 帮手进程**正运行在要被替换的 bundle 里**,由此推出两条铁律:
//!
//! 1. 旧 bundle 只改名成 `.old`,**绝不由帮手删除** —— 删掉自己正在执行的
//!    文件后,后续缺页会 SIGBUS;
//! 2. `.old` 的清理交给**新版首启**的 [`finish_pending`]:先写 ack,再等
//!    帮手进程退出,然后按帮手留下的清理清单删 `.old` / dmg / 残留。
//!
//! # 时序
//!
//! ```text
//! 主程序: spawn --updater → 退出
//! 帮手:   写 helper.pid → 等主程序退出 → DB 快照
//!         → target 改名 target.old → ditto 暂存包 → target → open 拉起新版
//!         → 等 ack ──成功→ 写 cleanup.list → 直接退出(不碰 .old)
//!                  └─超时→ 回滚(删掉没起来的新版, .old 改回, 重新拉起)
//! 新版:   finish_pending(): 写 ack → 等 helper.pid 的进程退出
//!         → 执行 cleanup.list → 清标记
//! ```
//!
//! 失败回滚时 DB schema 不会向前迁移(新版根本没起来),所以二进制回滚是
//! 完整的;快照 (`glaspen2.db.before-update`) 是给"新版起来了但行为不对、
//! 用户事后想降级"这条更远的路准备的 —— 旧版**拒绝打开**新 schema。

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// 标记文件
// ---------------------------------------------------------------------------

fn pid_file(cache: &Path) -> PathBuf {
    cache.join("helper.pid")
}
fn ack_file(cache: &Path) -> PathBuf {
    cache.join("ack")
}
fn cleanup_file(cache: &Path) -> PathBuf {
    cache.join("cleanup.list")
}
/// 替换前的数据库快照(进程退出后拷,天然一致)。
pub fn db_snapshot_name() -> &'static str {
    "glaspen2.db.before-update"
}

/// 追加一行到 `updater.log`(冒烟/排障用:面板看不到帮手的输出)。
pub fn log_line(cache: &Path, msg: &str) {
    use std::io::Write;
    let _ = std::fs::create_dir_all(cache);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(cache.join("updater.log"))
    {
        let _ = writeln!(f, "[{}] {msg}", crate::update::current_version());
    }
}

// ---------------------------------------------------------------------------
// 可注入的副作用 / 任务参数
// ---------------------------------------------------------------------------

/// 帮手流程里仅有的两个"外部世界"操作;其余全是文件系统(测试里用真实临时目录)。
pub trait Effects {
    /// 进程是否还活着。
    fn alive(&self, pid: u32) -> bool;
    /// 拉起(或重新拉起)目标 `.app`。
    fn relaunch(&self, app: &Path) -> Result<(), String>;
}

/// 一次替换任务的全部输入(测试直接构造,不依赖真实环境)。
#[derive(Debug, Clone)]
pub struct Job {
    /// 将被替换的当前 `.app`。
    pub target: PathBuf,
    /// 暂存的新版 `.app`([`crate::update::stage_dmg`] 的产物)。
    pub staging: PathBuf,
    /// 主程序 pid —— 必须等它退出才能换 bundle。
    pub pid: u32,
    /// 更新工作目录([`crate::update::update_dir`])。
    pub cache: PathBuf,
    /// 数据库文件(退出后做快照)。
    pub db: PathBuf,
    /// 附加清理项(下载的 dmg 等);成功后进清理清单。
    pub extra: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct Timing {
    /// 等主程序退出的预算。
    pub exit_wait: Duration,
    /// 等新版 ack 的预算(冷启动 1-2s,留足余量)。
    pub handshake_wait: Duration,
    pub poll: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            exit_wait: Duration::from_secs(30),
            handshake_wait: Duration::from_secs(45),
            poll: Duration::from_millis(200),
        }
    }
}

/// 替换结果。`Updated` = 新版已起且 ack 到手;`RolledBack` = 新版没起来,
/// 已恢复旧版并重新拉起。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Outcome {
    Updated,
    RolledBack,
}

/// `target` 的旧版本占位路径(`glaspen2.app` → `glaspen2.app.old`)。
pub fn backup_path(target: &Path) -> PathBuf {
    let mut s = target.as_os_str().to_owned();
    s.push(".old");
    PathBuf::from(s)
}

fn wait_until(mut cond: impl FnMut() -> bool, budget: Duration, poll: Duration) -> bool {
    let deadline = Instant::now() + budget;
    loop {
        if cond() {
            return true;
        }
        if Instant::now() >= deadline {
            return cond();
        }
        std::thread::sleep(poll);
    }
}

// ---------------------------------------------------------------------------
// 主流程
// ---------------------------------------------------------------------------

pub fn run_job(job: &Job, fx: &impl Effects, t: &Timing) -> Result<Outcome, String> {
    let cache = &job.cache;
    std::fs::create_dir_all(cache).map_err(|e| format!("创建更新目录失败:{e}"))?;
    std::fs::write(pid_file(cache), std::process::id().to_string())
        .map_err(|e| format!("写 helper.pid 失败:{e}"))?;
    log_line(
        cache,
        &format!("helper start target={}", job.target.display()),
    );

    // 1. 主程序还活着不能换 bundle
    if !wait_until(|| !fx.alive(job.pid), t.exit_wait, t.poll) {
        log_line(cache, "等待主程序退出超时");
        return Err("等待主程序退出超时,已放弃更新".into());
    }

    // 2. DB 快照(进程已退,一致);失败不阻断更新,只记录
    if job.db.exists() {
        let snap = cache.join(db_snapshot_name());
        match std::fs::copy(&job.db, &snap) {
            Ok(n) => log_line(cache, &format!("db snapshot {n} bytes")),
            Err(e) => log_line(cache, &format!("db snapshot FAILED: {e}")),
        }
    }

    // 3. 换 bundle:旧的只改名(它里面有我们自己),新的 ditto 进来
    if !job.staging.exists() {
        return Err("暂存的新版本不存在,请重新下载".into());
    }
    let backup = backup_path(&job.target);
    if backup.exists() {
        std::fs::remove_dir_all(&backup).map_err(|e| format!("清理旧备份失败:{e}"))?;
    }
    std::fs::rename(&job.target, &backup).map_err(|e| format!("改名旧版本失败(权限?):{e}"))?;
    if let Err(e) = copy_bundle(&job.staging, &job.target) {
        // 半途失败:把旧的放回去
        let _ = std::fs::remove_dir_all(&job.target);
        let _ = std::fs::rename(&backup, &job.target);
        log_line(cache, &format!("放入新版本失败,已回滚:{e}"));
        return Err(format!("放入新版本失败:{e}"));
    }
    // 暂存副本不是执行体,可以删(我们跑在 target.old 里)
    let _ = std::fs::remove_dir_all(&job.staging);

    // 4. 拉起新版
    if let Err(e) = fx.relaunch(&job.target) {
        let back = restore(fx, &job.target, &backup);
        log_line(cache, &format!("拉起新版失败:{e};回滚:{back:?}"));
        return Err(format!("拉起新版失败:{e}"));
    }

    // 5. 等新版首启 ack
    if wait_until(|| ack_file(cache).exists(), t.handshake_wait, t.poll) {
        // 成功:清理清单交给新版执行 —— .old 里跑着我们,不能自己删
        let mut list = String::new();
        list.push_str(&backup.to_string_lossy());
        list.push('\n');
        for p in &job.extra {
            list.push_str(&p.to_string_lossy());
            list.push('\n');
        }
        std::fs::write(cleanup_file(cache), list).map_err(|e| format!("写清理清单失败:{e}"))?;
        log_line(cache, "updated ok, waiting new app to clean up");
        return Ok(Outcome::Updated);
    }

    // 握手超时:新版没起来(多半启动即崩)。它不会在跑,直接删掉换回旧版。
    log_line(cache, "handshake timeout → rollback");
    let _ = std::fs::remove_file(ack_file(cache));
    if let Err(e) = restore(fx, &job.target, &backup) {
        log_line(cache, &format!("rollback FAILED: {e}"));
        return Err(format!("新版没有启动,且回滚失败:{e}"));
    }
    log_line(cache, "rolled back to old version");
    Ok(Outcome::RolledBack)
}

/// 回滚:删掉(没起来的)新版 → `.old` 改回去 → 重新拉起。
fn restore(fx: &impl Effects, target: &Path, backup: &Path) -> Result<(), String> {
    if !backup.exists() {
        return Err("旧版本占位不存在".into());
    }
    if target.exists() {
        std::fs::remove_dir_all(target).map_err(|e| format!("删除失败:{e}"))?;
    }
    std::fs::rename(backup, target).map_err(|e| format!("改名失败:{e}"))?;
    fx.relaunch(target)
}

/// 拷整个 bundle:macOS 用 `ditto`(保权限/xattr/framework 符号链接);
/// 其它平台(与单测)用纯 Rust 递归拷贝。
#[cfg(target_os = "macos")]
fn copy_bundle(from: &Path, to: &Path) -> Result<(), String> {
    let st = std::process::Command::new("/usr/bin/ditto")
        .arg(from)
        .arg(to)
        .status()
        .map_err(|e| format!("调用 ditto 失败:{e}"))?;
    if st.success() {
        Ok(())
    } else {
        Err("ditto 拷贝失败".into())
    }
}

#[cfg(not(target_os = "macos"))]
fn copy_bundle(from: &Path, to: &Path) -> Result<(), String> {
    copy_tree(from, to)
}

/// 纯 Rust 递归拷贝(不处理符号链接 —— 只给非 macOS 与测试用)。
#[cfg(any(not(target_os = "macos"), test))]
fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| format!("mkdir:{e}"))?;
    for entry in std::fs::read_dir(from).map_err(|e| format!("readdir:{e}"))? {
        let entry = entry.map_err(|e| format!("readdir:{e}"))?;
        let dst = to.join(entry.file_name());
        let ft = entry.file_type().map_err(|e| format!("stat:{e}"))?;
        if ft.is_dir() {
            copy_tree(&entry.path(), &dst)?;
        } else {
            std::fs::copy(entry.path(), &dst).map_err(|e| format!("copy:{e}"))?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 新版首启收尾
// ---------------------------------------------------------------------------

/// 启动时调用(任何版本都会走):如果上一次更新还在收尾,写 ack → 等帮手
/// 退出 → 执行清理清单 → 清标记。没有在途更新时只做两次 `exists`,近似零成本。
pub fn finish_pending() {
    finish_pending_with(crate::update::update_dir(), real_alive)
}

/// [`finish_pending`] 的可注入版本(单测:cache 指到临时目录, alive 用假的)。
fn finish_pending_with(cache: PathBuf, alive: impl Fn(u32) -> bool) {
    let pidf = pid_file(&cache);
    let ack = ack_file(&cache);
    let list = cleanup_file(&cache);
    let has_state = pidf.exists() || list.exists();
    if !has_state {
        return;
    }

    // 1. 立刻 ack —— 帮手在等这个才能收尾
    let _ = std::fs::write(&ack, crate::update::current_version());
    log_line(&cache, "new app: ack written");

    // 2. 等帮手退出(它跑在即将被删的 .old 里;删早了会 SIGBUS)
    if let Ok(s) = std::fs::read_to_string(&pidf)
        && let Ok(pid) = s.trim().parse::<u32>()
    {
        let _ = wait_until(
            || !alive(pid),
            Duration::from_secs(10),
            Duration::from_millis(100),
        );
    }

    // 3. 执行清单(帮手在写完清单之后才退出,所以这里读到的应该是完整的)
    if let Ok(content) = std::fs::read_to_string(&list) {
        for line in content.lines() {
            let p = PathBuf::from(line);
            if !p.exists() {
                continue;
            }
            let r = if p.is_dir() {
                std::fs::remove_dir_all(&p).map_err(|e| e.to_string())
            } else {
                std::fs::remove_file(&p).map_err(|e| e.to_string())
            };
            match r {
                Ok(()) => log_line(&cache, &format!("cleanup {}", p.display())),
                Err(e) => log_line(&cache, &format!("cleanup {} FAILED: {e}", p.display())),
            }
        }
        let _ = std::fs::remove_file(&list);
    }
    let _ = std::fs::remove_file(&pidf);
    let _ = std::fs::remove_file(&ack);
    log_line(&cache, "update finished, markers cleared");
}

/// 真实的进程存活探测。
fn real_alive(pid: u32) -> bool {
    #[cfg(target_os = "macos")]
    {
        // kill(pid, 0) 不发信号,只做存在性检查。主程序与帮手必然是同一用户,
        // 所以不存在 EPERM(他人进程)分支。
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = pid;
        false
    }
}

// ---------------------------------------------------------------------------
// --updater 入口
// ---------------------------------------------------------------------------

/// `glaspen2 --updater --target <app> --staging <app> --pid <n>
///           [--cache <dir>] [--db <file>] [--dmg <file>]`
///
/// 返回进程退出码:0 = 更新成功;2 = 回滚/不支持;1 = 出错。
#[cfg(target_os = "macos")]
pub fn updater_main(args: &[String]) -> i32 {
    fn val<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
        args.iter()
            .position(|a| a == key)
            .and_then(|i| args.get(i + 1))
            .map(String::as_str)
    }
    let (Some(target), Some(staging), Some(pid)) = (
        val(args, "--target"),
        val(args, "--staging"),
        val(args, "--pid"),
    ) else {
        tracing::info!("用法: --updater --target <app> --staging <app> --pid <n>");
        return 2;
    };
    let pid: u32 = match pid.parse() {
        Ok(p) => p,
        Err(_) => {
            tracing::info!("--pid 不是数字: {pid}");
            return 2;
        }
    };
    let job = Job {
        target: PathBuf::from(target),
        staging: PathBuf::from(staging),
        pid,
        cache: val(args, "--cache")
            .map(PathBuf::from)
            .unwrap_or_else(crate::update::update_dir),
        db: val(args, "--db")
            .map(PathBuf::from)
            .unwrap_or_else(crate::db::db_path),
        extra: val(args, "--dmg").map(PathBuf::from).into_iter().collect(),
    };

    struct MacEffects;
    impl Effects for MacEffects {
        fn alive(&self, pid: u32) -> bool {
            real_alive(pid)
        }
        fn relaunch(&self, app: &Path) -> Result<(), String> {
            let st = std::process::Command::new("/usr/bin/open")
                .arg(app)
                .status()
                .map_err(|e| format!("open 失败:{e}"))?;
            if st.success() {
                Ok(())
            } else {
                Err("open 返回失败".into())
            }
        }
    }

    match run_job(&job, &MacEffects, &Timing::default()) {
        Ok(Outcome::Updated) => 0,
        Ok(Outcome::RolledBack) => 2,
        Err(e) => {
            log_line(&job.cache, &format!("FAILED: {e}"));
            tracing::info!("{e}");
            1
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn updater_main(_args: &[String]) -> i32 {
    tracing::warn!("当前平台暂不支持自动更新");
    2
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// 每个用例独立的临时根目录(cargo test 并行,只按 pid 会互相踩)。
    fn root(case: &str) -> PathBuf {
        let r =
            std::env::temp_dir().join(format!("glaspen2_updtest_{}_{}", std::process::id(), case));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    /// 造一个假的 .app:marker 写进 Contents/marker.txt。
    fn make_app(dir: &Path, marker: &str) {
        std::fs::create_dir_all(dir.join("Contents")).unwrap();
        std::fs::write(dir.join("Contents/marker.txt"), marker).unwrap();
    }

    fn marker_of(app: &Path) -> String {
        std::fs::read_to_string(app.join("Contents/marker.txt")).unwrap_or_default()
    }

    /// 测试用 Effects:主程序"还活着"的次数可控;拉起时可选择性写 ack
    /// (模拟新版首启 finish_pending)。
    struct TestFx {
        alive_budget: AtomicU32,
        launched: RefCell<Vec<PathBuf>>,
        ack_path: PathBuf,
        handshake_on_launch: bool,
        fail_launch: bool,
    }

    impl TestFx {
        fn new(cache: &Path, handshake: bool) -> Self {
            TestFx {
                alive_budget: AtomicU32::new(3), // 头几次 alive 返回 true,然后主程序"退出"
                launched: RefCell::new(Vec::new()),
                ack_path: ack_file(cache),
                handshake_on_launch: handshake,
                fail_launch: false,
            }
        }
    }

    impl Effects for TestFx {
        fn alive(&self, _pid: u32) -> bool {
            loop {
                let cur = self.alive_budget.load(Ordering::SeqCst);
                if cur == 0 {
                    return false;
                }
                if self
                    .alive_budget
                    .compare_exchange(cur, cur - 1, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    return true;
                }
            }
        }
        fn relaunch(&self, app: &Path) -> Result<(), String> {
            self.launched.borrow_mut().push(app.to_path_buf());
            if self.fail_launch {
                return Err("open 挂了".into());
            }
            if self.handshake_on_launch {
                // 模拟新版首启 finish_pending():立刻 ack
                let _ = std::fs::write(&self.ack_path, "ok");
            }
            Ok(())
        }
    }

    fn fast_timing() -> Timing {
        Timing {
            exit_wait: Duration::from_secs(2),
            handshake_wait: Duration::from_millis(400),
            poll: Duration::from_millis(5),
        }
    }

    fn setup(case: &str, handshake: bool) -> (Job, TestFx) {
        let r = root(case);
        let target = r.join("Glaspen2.app");
        let staging = r.join("updates/Glaspen2-0.6.0.app");
        let cache = r.join("updates");
        std::fs::create_dir_all(&staging).unwrap();
        make_app(&target, "old");
        make_app(&staging, "new");
        let db = r.join("glaspen2.db");
        std::fs::write(&db, "DB-BYTES").unwrap();
        let dmg = cache.join("glaspen2-0.6.0-arm64.dmg");
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::write(&dmg, "DMG").unwrap();
        let job = Job {
            target,
            staging,
            pid: 424242,
            cache,
            db,
            extra: vec![dmg],
        };
        let fx = TestFx::new(&job.cache, handshake);
        (job, fx)
    }

    #[test]
    fn test_success_swaps_bundles_and_defers_old_cleanup() {
        let (job, fx) = setup("success", true);
        let out = run_job(&job, &fx, &fast_timing()).unwrap();
        assert_eq!(out, Outcome::Updated);

        // bundle 已换
        assert_eq!(marker_of(&job.target), "new");
        // 旧版本只改名不删除(帮手跑在里面)
        assert_eq!(marker_of(&backup_path(&job.target)), "old");
        // 暂存副本已消费
        assert!(!job.staging.exists());
        // DB 快照
        let snap = job.cache.join(db_snapshot_name());
        assert_eq!(std::fs::read_to_string(&snap).unwrap(), "DB-BYTES");
        // 拉起的是 target
        assert_eq!(
            fx.launched.borrow().as_slice(),
            std::slice::from_ref(&job.target)
        );
        // 清单包含 .old 与 dmg —— 由新版 finish_pending 执行
        let list = std::fs::read_to_string(cleanup_file(&job.cache)).unwrap();
        assert!(list.contains(".old"), "清单应包含 .old:{list}");
        assert!(list.contains("glaspen2-0.6.0-arm64.dmg"), "{list}");
        // 帮手不删自己的 pid 文件(新版要靠它确认帮手退出)
        assert!(pid_file(&job.cache).exists());
    }

    #[test]
    fn test_handshake_timeout_rolls_back_and_relaunches_old() {
        let (job, fx) = setup("rollback", false); // 新版起来但不 ack → 模拟启动即崩
        let out = run_job(&job, &fx, &fast_timing()).unwrap();
        assert_eq!(out, Outcome::RolledBack);

        // 旧版回来了,新版被删
        assert_eq!(marker_of(&job.target), "old");
        assert!(!backup_path(&job.target).exists());
        // 拉起两次:先新版,回滚后再拉旧版
        let launched = fx.launched.borrow();
        assert_eq!(launched.len(), 2);
        assert_eq!(launched[1], job.target);
        // ack 被清掉,清单不应存在
        assert!(!ack_file(&job.cache).exists());
        assert!(!cleanup_file(&job.cache).exists());
    }

    #[test]
    fn test_main_never_exits_gives_up() {
        let (job, mut fx) = setup("waitmain", true);
        fx.alive_budget = AtomicU32::new(u32::MAX); // 主程序永远活着
        let t = Timing {
            exit_wait: Duration::from_millis(50),
            handshake_wait: Duration::from_millis(100),
            poll: Duration::from_millis(5),
        };
        let err = run_job(&job, &fx, &t).unwrap_err();
        assert!(err.contains("等待主程序退出"), "{err}");
        // 没动过 bundle
        assert_eq!(marker_of(&job.target), "old");
        assert!(job.staging.exists());
    }

    #[test]
    fn test_missing_staging_bails_before_touching_target() {
        let (job, fx) = setup("nostaging", true);
        std::fs::remove_dir_all(&job.staging).unwrap();
        let err = run_job(&job, &fx, &fast_timing()).unwrap_err();
        assert!(err.contains("暂存"), "{err}");
        assert_eq!(marker_of(&job.target), "old");
        assert!(job.target.exists());
        assert!(!backup_path(&job.target).exists());
    }

    #[test]
    fn test_relaunch_failure_rolls_back_immediately() {
        let (job, mut fx) = setup("relaunchfail", true);
        fx.fail_launch = true;
        let err = run_job(&job, &fx, &fast_timing()).unwrap_err();
        assert!(err.contains("拉起新版失败"), "{err}");
        // 旧版已恢复
        assert_eq!(marker_of(&job.target), "old");
        assert!(!backup_path(&job.target).exists());
    }

    // ── finish_pending ──

    #[test]
    fn test_finish_pending_cleans_up_after_helper() {
        let (job, _fx) = setup("finish", true);
        // 模拟帮手留下的现场:.old + dmg 在清单里, pidfile 存在(进程已死)
        let backup = backup_path(&job.target);
        make_app(&backup, "old"); // 目录已存在(成功路径里就是它)
        // run_job 没跑,这里手工凑齐标记
        std::fs::write(pid_file(&job.cache), "999999").unwrap();
        std::fs::write(
            cleanup_file(&job.cache),
            format!("{}\n{}\n", backup.display(), job.extra[0].display()),
        )
        .unwrap();

        finish_pending_with(job.cache.clone(), |_| false); // "帮手已退出"

        assert!(!backup.exists(), ".old 应被清理");
        assert!(!job.extra[0].exists(), "dmg 应被清理");
        assert!(!pid_file(&job.cache).exists());
        assert!(!cleanup_file(&job.cache).exists());
        assert!(!ack_file(&job.cache).exists());
        // 目标没被动
        assert_eq!(marker_of(&job.target), "old");
    }

    #[test]
    fn test_finish_pending_noop_without_markers() {
        let r = root("finishnoop");
        let cache = r.join("updates");
        std::fs::create_dir_all(&cache).unwrap();
        // 唯一的副作用候选是 update_dir()(真实 HOME 缓存)里的标记;
        // 这里保证"无标记时不写 ack"的判断逻辑 —— 用一个带标记的私有目录验证反例:
        std::fs::write(ack_file(&cache), "stale").unwrap();
        // 无标记路径:finish_pending_with 在没有 pidfile/list 时直接 return,
        // 连 ack 都不会碰 —— 通过"删除 ack 后调用,ack 不被重建"验证:
        std::fs::remove_file(ack_file(&cache)).unwrap();
        finish_pending_with(cache.clone(), |_| false);
        // (真实 update_dir 没有标记,所以什么都不会发生)
        assert!(!pid_file(&cache).exists());
        assert!(!ack_file(&cache).exists());
    }

    #[test]
    fn test_backup_path_and_copy_tree() {
        let r = root("misc");
        assert_eq!(
            backup_path(Path::new("/Applications/Glaspen2.app")),
            PathBuf::from("/Applications/Glaspen2.app.old")
        );
        // copy_tree 递归
        let src = r.join("src");
        make_app(&src, "x");
        std::fs::write(src.join("Contents/deep.txt"), "deep").unwrap();
        let dst = r.join("dst");
        copy_tree(&src, &dst).unwrap();
        assert_eq!(marker_of(&dst), "x");
        assert_eq!(
            std::fs::read_to_string(dst.join("Contents/deep.txt")).unwrap(),
            "deep"
        );
    }
}
