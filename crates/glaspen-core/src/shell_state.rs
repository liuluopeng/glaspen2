//! macOS 壳的少量全局状态(从 Swift `AppState` 迁移)。
//!
//! 屏幕尺寸在启动/显示器变化时设置; 语言由设置/菜单切换。
//! 用原子量即可, 不涉及 AppKit; C ABI 符号与旧 Swift 导出同名。
#![cfg(target_os = "macos")]

use std::sync::atomic::{AtomicI32, Ordering};

static SCREEN_W: AtomicI32 = AtomicI32::new(1920);
static SCREEN_H: AtomicI32 = AtomicI32::new(1080);
static LANG: AtomicI32 = AtomicI32::new(0);

pub fn screen_w() -> i32 {
    SCREEN_W.load(Ordering::Relaxed)
}

pub fn screen_h() -> i32 {
    SCREEN_H.load(Ordering::Relaxed)
}

pub fn set_screen_size(w: i32, h: i32) {
    SCREEN_W.store(w, Ordering::Relaxed);
    SCREEN_H.store(h, Ordering::Relaxed);
}

pub fn lang() -> i32 {
    LANG.load(Ordering::Relaxed)
}

/// 设置语言: 0=中文, 1=英文; 其它值按中文(与旧 Swift set 逻辑一致)。
pub fn set_lang(value: i32) {
    LANG.store(i32::from(value == 1), Ordering::Relaxed);
}

/// 启动恢复: 原样写入(旧实现允许非 0/1 值, 由读取方按 ==1 判定)。
pub fn startup_set_lang(value: i32) {
    LANG.store(value, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_and_lang_roundtrip() {
        set_screen_size(1234, 567);
        assert_eq!((screen_w(), screen_h()), (1234, 567));
        set_screen_size(1920, 1080);
        startup_set_lang(1);
        assert_eq!(lang(), 1);
        set_lang(0);
        assert_eq!(lang(), 0);
        set_lang(9);
        assert_eq!(lang(), 0);
    }
}
