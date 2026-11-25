//! 渲染设置 FFI(R11: Swift RenderSettings → Rust)。

use std::os::raw::{c_double, c_int};

use crate::render_state;

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_glass_enabled() -> c_int {
    i32::from(render_state::glass_enabled())
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_glass_state(on: c_int) {
    render_state::set_glass_enabled(on != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_glass_opacity() -> c_double {
    render_state::glass_opacity()
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_glass_follow() -> c_int {
    i32::from(render_state::glass_follow())
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_glass_follow(on: c_int) {
    render_state::set_glass_follow(on != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_startup_glass(opacity: c_double, enabled: c_int) {
    render_state::startup_glass(opacity, enabled != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_startup_set_glass_follow(on: c_int) {
    render_state::set_glass_follow(on != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_soft_shadow() -> c_int {
    i32::from(render_state::soft_shadow())
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_startup_shadow(on: c_int) {
    render_state::set_soft_shadow(on != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_soft_shadow_flag(on: c_int) {
    render_state::set_soft_shadow(on != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_outline_enabled() -> c_int {
    i32::from(render_state::outline())
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_outline_state(on: c_int) {
    render_state::set_outline(on != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_startup_outline(on: c_int) {
    render_state::set_outline(on != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_flip_effect() -> c_int {
    render_state::flip_effect()
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_startup_flip(effect: c_int) {
    render_state::set_flip_effect(effect);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_set_flip(effect: c_int) {
    render_state::set_flip_effect(effect);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_invert_enabled() -> c_int {
    i32::from(render_state::invert_ink())
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_invert_set_ink(on: c_int) {
    render_state::set_invert_ink(on != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_startup_invert(fps: c_int, ink: c_int) {
    render_state::startup_invert(fps, ink != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_invert_fps() -> c_int {
    render_state::invert_fps()
}

#[unsafe(no_mangle)]
pub extern "C" fn glaspen2_router_invert_set_fps(fps: c_int) {
    render_state::set_invert_fps(fps);
}
