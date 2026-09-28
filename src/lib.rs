// glaspen2 — macOS/Windows 主程序 crate(壳)。
//
// 共享核心已拆至 `crates/glaspen-core`(存储/模型器/cairo/导出/更新/
// 手写消息与共享上行 FFI);Windows 覆盖层在 `crates/glaspen-windows`。
// 本 crate 保留:FRB 面板 API(api.rs + frb_generated.rs, macos 壳,
// 以及把 core 的模块与类型原路再导出 —— 让 `crate::db` 等既有路径
// 在 api.rs 里继续成立, ObjC 链接的 FFI 符号不受影响)。
#![allow(clippy::not_unsafe_ptr_arg_deref)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]

mod frb_generated; /* AUTO INJECTED BY flutter_rust_bridge */

pub mod api;

#[cfg(target_os = "macos")]
pub mod macos;

// ── core 再导出:api.rs 的 `crate::db` / `crate::export` 等旧路径照旧成立 ──
pub use glaspen_core::{cairo_dl, db, export, modeler, pdf, state, update, updater};
pub use glaspen_core::{desktop_path, pressure_to_width, runtime, timestamped_name,
                       timestamped_path, Stroke, STROKES};
