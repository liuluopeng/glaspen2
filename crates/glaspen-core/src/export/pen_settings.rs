//! 笔外观设置 FFI(R12: Swift PenSettingsStore → Rust)。

use std::os::raw::{c_double, c_int};

use crate::pen_settings;

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_pen_r() -> c_double {
    pen_settings::pen_rgb().0
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_pen_g() -> c_double {
    pen_settings::pen_rgb().1
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_pen_b() -> c_double {
    pen_settings::pen_rgb().2
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_width_scale() -> c_double {
    pen_settings::width_scale()
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_selected_color() -> c_int {
    pen_settings::selected_color()
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_selected_color_index() -> c_int {
    pen_settings::selected_color()
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_selected_width() -> c_int {
    pen_settings::selected_width()
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_selected_width_index() -> c_int {
    pen_settings::selected_width()
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_prev_color_index() -> c_int {
    pen_settings::prev_color()
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_prev_width_index() -> c_int {
    pen_settings::prev_width()
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_prev_color(idx: c_int) {
    pen_settings::set_prev_color(idx);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_prev_width(idx: c_int) {
    pen_settings::set_prev_width(idx);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_pending_color() -> c_int {
    pen_settings::pending_color()
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_pending_width() -> c_int {
    pen_settings::pending_width()
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_pending_color(idx: c_int) {
    pen_settings::set_pending_color(idx);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_pending_width(idx: c_int) {
    pen_settings::set_pending_width(idx);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_clear_pending_color() {
    pen_settings::clear_pending_color();
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_clear_pending_width() {
    pen_settings::clear_pending_width();
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_apply_color(idx: c_int) {
    pen_settings::apply_color(idx);
}
#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_apply_width(idx: c_int) {
    pen_settings::apply_width(idx);
}

#[unsafe(no_mangle)]
pub extern "C" fn sw_set_pen_settings(
    r: c_double,
    g: c_double,
    b: c_double,
    width: c_double,
    color_idx: c_int,
    width_idx: c_int,
) {
    pen_settings::set_pen_settings(r, g, b, width, color_idx, width_idx);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_save_pen_settings() {
    let (r, g, b) = pen_settings::pen_rgb();
    super::glaspen2_save_settings(r, g, b, pen_settings::width_scale());
}
