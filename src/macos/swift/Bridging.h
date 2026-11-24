// glaspen2 macOS 原生层 —— Swift → ObjC/C 桥接头(Swift migration S2)
//
// build.rs 用 `swiftc -import-objc-header src/macos/swift/Bridging.h` 把它导入
// Swift 模块。这里只允许**C 兼容声明**:
//   - C 基础类型(char/int/double/指针)/ NSObject 子类 / @objc 协议;
//   - 不得出现 Swift 调用方无法理解的东西(Block 返回值、ObjC 泛型容器等
//     需要显式转换的声明请先确认 Swift 侧能表达, 见 docs/swift-boundary.md)。
//
// 本头文件只声明 ObjC/C 侧已有的函数;实现仍留在 ObjC(或 Rust C ABI)一侧。
// 迁移新模块时: 把该模块对外暴露的 C 入口按原签名追加到这里, Swift 侧
// 直接按 C 语义调用, 不要包一层 Swift 类型。

#ifndef GLASPEN2_SWIFT_BRIDGING_H
#define GLASPEN2_SWIFT_BRIDGING_H

#import <Foundation/Foundation.h>

#ifdef __cplusplus
extern "C" {
#endif

// S2 反向互调探针: ObjC 侧实现(src/macos/swift/InteropProbe.m), 无副作用
// (只打日志 + 返回常量 0x5A2)。SwiftInteropProbe.ping() 经本声明调用它,
// 验证 Swift→ObjC 方向。
int glaspen2_macos_objc_probe(const char *tag);

// S3b-2b-4b: 菜单文案桥(实现留在 menu.m, 内部调 gl_text; Swift 只按 key 取名)
NSString *glaspen2_menu_l10n(NSString *key);

// S3b-2b-7a: 菜单 delegate/退出薄桥(Swift MenuHandler 调用; 实现留在 menu.m)
void glaspen2_menu_will_open_bridge(void);
void glaspen2_menu_did_close_bridge(void);
void glaspen2_menu_quit_bridge(void);

// S3b-2b-7b: 语言切换所需的语言状态桥(实现留在 menu.m, g_lang 是 TU 内静态全局)
int glaspen2_menu_lang(void);
void glaspen2_menu_set_lang(int value);
void glaspen2_menu_refresh_texts(void);

// 既有 C FFI(实现在 core/Rust, 这里只补声明给 Swift 用)
void glaspen2_save_string_setting(const char *key, const char *value);
void glaspen2_notify_settings_changed(void);

// S3b-2b-7c: 开关类菜单动作的 getter/setter 薄桥(实现留在 menu.m)
int glaspen2_menu_launch_at_login(void);
void glaspen2_menu_set_launch(int on);
int glaspen2_menu_glass_enabled(void);
void glaspen2_menu_set_glass(int on);
int glaspen2_menu_outline_enabled(void);
void glaspen2_menu_apply_outline(int on);
int glaspen2_menu_infinite_enabled(void);
void glaspen2_menu_apply_infinite(int on);

// S3b-2b-7d: 保存/导出/设置/选择动作的薄桥(实现留在 menu.m)
void glaspen2_menu_save_with_bg(void);
void glaspen2_menu_save_only(void);
void glaspen2_menu_clear_screen(void);
void glaspen2_menu_toggle_enabled(void);
void glaspen2_menu_show_settings(void);
void glaspen2_menu_toggle_canvas_mode(void);
void glaspen2_menu_select_color(int idx);
void glaspen2_menu_select_width(int idx);
void glaspen2_menu_save_xoj_raw(void);
void glaspen2_menu_notify(NSString *text);

// S2 探针入口(ObjC 实现, 供 glaspen2_run / Rust --swift-probe 调用):
// 内部经生成的 glaspen2_macos_swift-Swift.h 调 [SwiftInteropProbe ping],
// 验证 ObjC→Swift 方向。普通迁移代码不需要这类"Swift 反向入口"。
void glaspen2_macos_swift_probe_selftest(void);
void glaspen2_macos_swift_probe_maybe(void);

#ifdef __cplusplus
}
#endif

#endif // GLASPEN2_SWIFT_BRIDGING_H
