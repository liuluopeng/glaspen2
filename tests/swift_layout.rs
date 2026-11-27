//! macOS Swift 原生层目录布局守护。
//!
//! 目标: 防止退回"103 个 .swift 平铺一层"的散落状态, 并保证 build.rs
//! 依赖的"basename 全局唯一"约束不被破坏。纯文件系统检查, 不启动 GUI,
//! 跨平台可跑(仓库里始终带有 macOS 源目录)。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

const ALLOWED_DIRS: &[&str] = &[
    "App",
    "Menu",
    "Input",
    "Surface",
    "Settings",
    "Features",
    "Diagnostics",
];

fn walk_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries =
        std::fs::read_dir(dir).unwrap_or_else(|e| panic!("读取 {} 失败: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("目录项").path();
        if path.is_dir() {
            walk_files(&path, out);
        } else {
            out.push(path);
        }
    }
}

#[test]
fn swift_sources_are_grouped_by_subsystem() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("crates")
        .join("glaspen-macos")
        .join("swift");

    assert!(root.join("Bridging.h").is_file(), "缺少 Bridging.h");
    assert!(root.join("rust_ffi.h").is_file(), "缺少 rust_ffi.h");
    assert!(root.join("README.md").is_file(), "缺少目录说明 README.md");

    let mut all = Vec::new();
    walk_files(&root, &mut all);

    let mut swift_stems: HashSet<std::ffi::OsString> = HashSet::new();
    let mut swift_count = 0usize;
    for path in &all {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        match ext {
            "swift" => {
                swift_count += 1;
                let rel = path.strip_prefix(&root).expect("Swift 路径在根目录下");
                let comps: Vec<_> = rel.components().collect();
                assert_eq!(
                    comps.len(),
                    2,
                    "Swift 文件必须正好位于一个子系统目录(App/Menu/Input/...): {}",
                    path.display()
                );
                let dir = comps[0].as_os_str().to_string_lossy();
                assert!(
                    ALLOWED_DIRS.contains(&dir.as_ref()),
                    "未知 Swift 子系统目录 {dir}: {}",
                    path.display()
                );
                let stem = path
                    .file_stem()
                    .expect("Swift 文件没有文件名")
                    .to_os_string();
                assert!(
                    swift_stems.insert(stem.clone()),
                    "Swift 文件名必须全局唯一(目标文件按 basename 命名): {:?}",
                    stem
                );
            }
            "m" | "mm" => panic!(
                "macOS 应用层不得出现 ObjC 源文件(door-step 规则): {}",
                path.display()
            ),
            "h" => assert!(
                path.parent() == Some(root.as_path()),
                "C ABI 头文件只允许放在 crates/glaspen-macos/swift/ 根目录: {}",
                path.display()
            ),
            _ => {}
        }
    }

    assert!(
        (3..=8).contains(&swift_count),
        "Swift 源文件数量异常(当前 {swift_count}); 迁移后应只余少量 AppKit/SCK 壳"
    );
}
