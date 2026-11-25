//! 功能开关/GIF 参数 FFI(R10: Swift ToggleSettings → Rust)。

use std::os::raw::{c_double, c_int};

use crate::toggle_state;

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_enabled() -> c_int {
    i32::from(toggle_state::enabled())
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_enabled_state(on: c_int) {
    toggle_state::set_enabled(on != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_ethereal() -> c_int {
    i32::from(toggle_state::ethereal())
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_ethereal(on: c_int) {
    toggle_state::set_ethereal(on != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_pressure_monitor() -> c_int {
    i32::from(toggle_state::pressure_monitor())
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_pm_enabled() -> c_int {
    i32::from(toggle_state::pressure_monitor())
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_pressure_monitor(on: c_int) {
    toggle_state::set_pressure_monitor(on != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_chat_integration() -> c_int {
    i32::from(toggle_state::chat_integration())
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_chat_integration(on: c_int) {
    toggle_state::set_chat_integration(on != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_restore_chat_integration(on: c_int) {
    toggle_state::set_chat_integration(on != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_gif_fps() -> c_int {
    toggle_state::gif_fps()
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_gif_resolution() -> c_double {
    toggle_state::gif_resolution()
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_gif_speed() -> c_double {
    toggle_state::gif_speed()
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_gif_end_mode() -> c_int {
    toggle_state::gif_end_mode()
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_gif(
    fps: c_int,
    resolution: c_double,
    speed: c_double,
    end_mode: c_int,
) {
    toggle_state::set_gif(fps, resolution, speed, end_mode);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_restore_gif_settings(
    fps: c_int,
    resolution: c_double,
    speed: c_double,
    end_mode: c_int,
) {
    toggle_state::set_gif(fps, resolution, speed, end_mode);
}
