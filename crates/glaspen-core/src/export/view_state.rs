//! 绘制/网格可见性状态 FFI(R9: Swift ViewSettings → Rust)。

use std::os::raw::{c_double, c_int};

use crate::view_state;

#[unsafe(no_mangle)]
pub extern "C" fn sw_set_infinite(on: c_int) {
    view_state::set_infinite(on != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn sw_set_minimap(on: c_int) {
    view_state::set_minimap(on != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_infinite() -> c_int {
    i32::from(view_state::infinite())
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_infinite_state(on: c_int) {
    view_state::set_infinite(on != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_strokes_visible() -> c_int {
    i32::from(view_state::strokes_visible())
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_strokes_visible(on: c_int) {
    view_state::set_strokes_visible(on != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_strokes_hidden() -> c_int {
    i32::from(!view_state::strokes_visible())
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_minimap_enabled() -> c_int {
    i32::from(view_state::minimap())
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_minimap(on: c_int) {
    view_state::set_minimap(on != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_show_grid() -> c_int {
    i32::from(view_state::show_grid())
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_grid_state(on: c_int) {
    view_state::set_show_grid(on != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_grid_follow(on: c_int) {
    view_state::set_grid_follow(on != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_grid_size(size: c_double) {
    view_state::set_grid_size(size);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_grid_divider(divider: c_int) {
    view_state::set_grid_divider(divider);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_startup_grid(
    show: c_int,
    size: c_double,
    follow: c_int,
    divider: c_int,
) {
    view_state::startup_grid(show != 0, size, follow != 0, divider);
}

// 新增只读 getter: Swift 渲染/探针读取网格参数用(旧实现是 Swift 静态字段)。
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_grid_size() -> c_double {
    view_state::grid_size()
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_grid_divider() -> c_int {
    view_state::grid_divider()
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_grid_follow() -> c_int {
    i32::from(view_state::grid_follow())
}
