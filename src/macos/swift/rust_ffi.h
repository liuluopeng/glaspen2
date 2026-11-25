#pragma once
// Auto-extracted Rust FFI declarations (from former glaspen2.m).
extern void glaspen2_save_drawing(const unsigned char *data, int width, int height, int stride);
extern void glaspen2_save_with_background(
    const unsigned char *drawing_data, int drawing_width, int drawing_height, int drawing_stride,
    const unsigned char *bg_data, int bg_width, int bg_height, int bg_stride);
extern void glaspen2_begin_stroke(double r, double g, double b, double width_scale);
extern void glaspen2_add_point(double x, double y, double width);
extern void glaspen2_add_point_t(double x, double y, double width, double t);
extern void glaspen2_end_stroke(void);
extern void glaspen2_save_xoj(void);
extern int glaspen2_clear_strokes(int screen_w, int screen_h);
extern void glaspen2_init_db(int screen_w, int screen_h);
extern void glaspen2_save_settings(double r, double g, double b, double width_scale);
extern void glaspen2_shutdown(void);
extern int  glaspen2_load_settings_parts(double *r, double *g, double *b, double *w);
extern void glaspen2_save_bool_setting(const char *key, int val);
extern int  glaspen2_load_bool_setting(const char *key);
extern void glaspen2_save_string_setting(const char *key, const char *value);
extern char* glaspen2_load_string_setting(const char *key);
// D1 设置 effect 桥(core/settings_apply.rs): 解析 + 钳制 + 落库一次完成,
// 返回 effect JSON({"kind":..., "bool"/"int"/"float"/"str"/"json":...} 或 {"error":...});
// 调用方用 glaspen2_free_c_string 释放。ObjC 只按 kind 做平台运行时副作用。
extern char* glaspen2_settings_apply(const char *key, const char *value_json);
// D12 面板设置快照桥(core/settings_snapshot.rs): 入参 = 运行时专属值 JSON 对象
// (launchAtLogin/shortcuts/flipEffect/language), 返回完整快照单行 JSON
// (字段集合/顺序/类型/缺省值/DB 回退全在 core);失败回 {"error":...}, 永不 NULL;
// 调用方用 glaspen2_free_c_string 释放。
extern char* glaspen2_settings_snapshot_json(const char *overrides_json);
// D3b 动作 effect 桥(core/action_effect.rs): action id → 平台无关 effect JSON
// ({"kind":"<ActionEffect 变体名>"} 或 {"error":"..."}); 调用方用
// glaspen2_free_c_string 释放。ObjC 只按 kind 做本平台运行时副作用。
extern char* glaspen2_action_effect_json(const char *action);
// D5b 镜头数学桥(core/camera.rs): 输入 ObjC 持有的镜头三值 + 本次操作参数,
// 返回 core 算好的新三值 JSON({"pan_x","pan_y","zoom"}[,"hit_max","hit_min",
// "changed"] 或 {"error":"..."}), 永不 NULL;调用方用 glaspen2_free_c_string 释放。
// ObjC 只负责写回 g_pan_x/g_pan_y/g_zoom 与本平台门控/提示/持久化/重绘。
extern char* glaspen2_camera_pan(double pan_x, double pan_y, double zoom, double dx, double dy);
extern char* glaspen2_camera_zoom_at(double pan_x, double pan_y, double zoom, double factor, double vx, double vy);
extern char* glaspen2_camera_zoom_at_center(double pan_x, double pan_y, double zoom, double factor, double viewport_w, double viewport_h);
extern char* glaspen2_camera_centered(double bx, double by, double bx2, double by2, double viewport_w, double viewport_h, double zoom);
// 标量(高频拖动/键盘步长, 避免走 JSON/堆分配)。
extern double glaspen2_camera_step_zoom(double zoom);
extern double glaspen2_camera_keyboard_pan_step(double zoom);
// 回原点 + 100%(out 参数可传 NULL)。
extern void glaspen2_camera_home(double *pan_x, double *pan_y, double *zoom);
// D6 网格/分栏几何桥(core/grid.rs): 输入视图尺寸/镜头/网格设置, 返回 core 算好的
// 线段表 JSON({"minor":[...],"screen":[...],"divider":[...],"follow":bool} 或
// {"error":"..."}), 线段是视图坐标(原点左上, y 向下), ObjC 只按既有 CGContext
// 样式描边。glaspen2_grid_json = 全视口规范入口(屏幕单位=视口尺寸, macOS 两种画布
// 模式都画屏幕分界线); glaspen2_grid_json_clipped = 笔迹脏矩形热路径(只算与 clip
// 相交的线, 额外带 show_grid 开关)。永不 NULL;调用方用 glaspen2_free_c_string 释放。
extern char* glaspen2_grid_json(double view_w, double view_h, double pan_x, double pan_y, double zoom, double grid_size, int divider, int follow);
extern char* glaspen2_grid_json_clipped(double view_w, double view_h, double pan_x, double pan_y, double zoom, double grid_size, int divider, int show_grid, double clip_x0, double clip_y0, double clip_x1, double clip_y1);
extern void glaspen2_chat_auth_reload(void); // 设置/启动时把 DB 账号配置推给 Rust auth
extern void glaspen2_share_ink_set_active(int active); // 共享画布上行开关(export.rs)
// S2 双向互调探针(env 门控, 默认 no-op; 定义在 src/macos/swift/InteropProbe.m)
extern void glaspen2_macos_swift_probe_maybe(void);
// (共享画布上行已无 ObjC 入口:生命周期完全由面板 tab 驱动,FRB 直达 Rust)

// Modeler FFI
extern void glaspen2_modeler_begin(double r, double g, double b, double x, double y, double pressure, double timestamp, double width_scale);
extern void glaspen2_modeler_move(double x, double y, double pressure, double timestamp, double width_scale);
extern void glaspen2_modeler_end(double x, double y, double pressure, double timestamp, double width_scale);
extern int glaspen2_modeler_point_count(void);
extern void glaspen2_modeler_get_point(int idx, double *x, double *y, double *w);
extern void glaspen2_modeler_clear_buffer(void);
extern void glaspen2_modeler_commit_to_strokes(double r, double g, double b);

// 气泡消息「冷启动」分段的笔状态喂入(bubbles.rs):
// 活动心跳 = 悬空移动/落笔/拖动/抬笔;pen_gone = proximity 离开;
// pen_back = proximity 进入(极近的退+进 = 笔↔橡皮切换,撤销冷标记)
extern void glaspen2_bubble_set_capture(int on);
extern void glaspen2_bubble_pen_activity(void);
extern void glaspen2_bubble_pen_back(void);
extern void glaspen2_bubble_pen_gone(void);
// 「写完自动新建一页」: 落笔瞬间调用, 若这笔将开新消息则建页并切 current id,
// 返回新页 id(0 = 无动作); shell 据此翻页载入(见 run.m bubble_auto_page_flip)
extern long long glaspen2_bubble_auto_page_if_due(void);
extern void glaspen2_modeler_erase_finish(void);
extern int glaspen2_stroke_bbox(double *x_min, double *y_min, double *x_max, double *y_max);
extern void glaspen2_save_svg(void);
extern char* glaspen2_get_cropped_svg(void);
extern void glaspen2_free_c_string(char *ptr);
// D9c 实时绘制计划桥(core export/mod.rs): 本段笔迹 JSON → 单行 DrawPlan JSON
// (commands / dirty / seam; 或 {"error":...}), 永不 NULL;
// 返回值用 glaspen2_free_c_string 释放。ObjC 侧光栅化见 parts/panel.m 的
// gl_live_plan_draw_segment(失败回退旧逐段自绘)。
extern char* glaspen2_live_plan_json(const char *input);
// D10b 文案桥(core/i18n.rs): 按稳定 key 取当前语言文案, 未知 key 回显 key
// 本身(便于发现拼错), 返回缓冲用 glaspen2_free_c_string 释放; 每次调用现查
// 语言, 因此切换语言后新提示/菜单立即生效(见 gl_text/gl_textf)。
extern char* glaspen2_i18n(const char *msg_key, const char *lang);
extern int glaspen2_save_gif_cropped(const unsigned char *surface_data, int w, int h, int stride, double surface_scale);
extern int glaspen2_save_animated_gif(int fps, double resolution, double speed, int end_mode);
extern unsigned char * glaspen2_gif_record_end(int start_index, int end_index, int fps, double resolution, double speed, int end_mode, int *out_len);
extern void glaspen2_draw_rebuild(void *surface_ptr, double scale);
extern void glaspen2_page_dims(long long screen_id, int *w, int *h);
extern void glaspen2_draw_rebuild_view(void *surface_ptr, double scale, double ox, double oy, double pscale);
extern int glaspen2_paint_page_into_surface(void *surface_ptr, long long screen_id, double scale, int white_bg);
extern int glaspen2_export_pdf(void);
// 无限画布导出(独立存储):分页 PDF / 整幅 SVG
extern int glaspen2_export_infinite_pdf_paged(int page_w, int page_h);
extern int glaspen2_export_infinite_svg(void);
extern void glaspen2_on_display_change(int screen_w, int screen_h);
// D7 缩略图条布局桥(core/minimap.rs): 输入当前页/视图尺寸/几何组过滤宽度,
// 返回 core 算好的页窗 + 条带布局 JSON({"strip_x","strip_y","strip_w","strip_h",
// "items":[{"screen_id","index","y","h","label","current"},...]} 或
// {"error":"..."}), 坐标是视图坐标(原点左上, y 向下; 合成条带时翻转 y)。
// geometry_width > 0 = page-view-fit 模式, 只保留当前几何组(按页宽过滤)。
// 永不 NULL; 调用方用 glaspen2_free_c_string 释放。
extern char* glaspen2_minimap_layout_json(long long current_screen_id, double view_w, double view_h, int geometry_width);
extern unsigned char* glaspen2_render_thumbnail(long long screen_id, int w, int h, int max_size, int *out_len);
/// 批量缩略图:一次调用返回多页的 PNG(自描述二进制块,见 export.rs)。
extern unsigned char* glaspen2_page_thumbnails(const long long *ids, int count, int max_size, int *out_len);
extern void glaspen2_free_rust_bytes(unsigned char *ptr, int len);
/// 设置面板改为 flutter_rust_bridge 通信:菜单/快捷键改了状态后通知 Rust,
/// 由 Rust 推给订阅了设置流的 Dart 侧(取代旧的 MethodChannel 回调)。
extern void glaspen2_notify_settings_changed(void);
extern int glaspen2_delete_screen(long long screen_id);
extern char* glaspen2_page_info_text(long long screen_id, const char *lang);
// chord 单源(core/chord.rs, D2): 动作表/默认值/规范化/保留/旧默认迁移
extern char* glaspen2_chord_actions_json(void);
extern char* glaspen2_chord_defaults_json(void);
extern char* glaspen2_chord_normalize(const char *raw);
extern char* glaspen2_chord_migrate_value(const char *action, const char *raw);
extern int glaspen2_chord_reserved(const char *chord);
extern int glaspen2_chat_send_strokes(int start_index, int end_index);
// 手写消息草稿通道(⌘⌃2):开启/关闭 DraftInk gRPC 流,实现见 export.rs
extern int glaspen2_ink_draft_start(int canvas_w, int canvas_h);
extern int glaspen2_ink_draft_stop(void);
extern const char *glaspen2_ink_draft_last_error(void);
// D8b 录制会话状态机桥(core/recording.rs): ObjC 不能持 Rust 结构体, 状态机
// 活在 core 的两个槽位里(adapter 只在主 runloop 线程调用, 后台结果回主线程再喂回):
//   槽 0 = GIF(⌘⌃R);槽 1 = 手写消息(⌘⌃2 草稿 / ⌘⌃3 直发, 同一机器内互斥)。
// glaspen2_recording_event_json 喂单行事件 JSON, 回 must-execute-in-order 的
// effects JSON 数组(单行, 永不 NULL, 非法输入回 {"error":...})。
// glaspen2_recording_mode 回 0=空闲/1=GIF/2=草稿/3=直发(-1=非法槽位),
// 供 key-up 分支判断"这次抬起是否属于本槽位"。
// 字符串一律用 glaspen2_free_c_string 释放。
extern int glaspen2_recording_init(int slot, int chat_integration);
extern char* glaspen2_recording_event_json(int slot, const char *event_json);
extern int glaspen2_recording_reset(int slot);
extern int glaspen2_recording_mode(int slot);
extern void glaspen2_set_stroke_outline(int enabled);

// 镜头数学在 core::camera。生产 Swift 走 R5 的 *_values 直出接口; 下面的 JSON 版本保留给 core 测试/兼容。
// ObjC 侧使用的 Swift 薄桥前向声明(Bridging.h 只给 Swift 编译单元)
void glaspen2_router_update_status_icon_state(void);
extern void glaspen2_set_soft_shadow(int enabled);
extern void glaspen2_set_stroke_invert(int enabled);      // 反色突出开关
extern void glaspen2_set_invert_background(void *surface); // 反相背景 cairo surface
extern int  glaspen2_has_strokes(void);                    // 捕获循环空转判断
void invert_ink_apply(int on);                             // 定义在 parts/panel.m
void invert_stream_restart(void);                          // 帧率变更后重启捕获流
void gl_panel_transparent_apply_full(int on);             // 面板真穿透开关(定义在 parts/panel.m)
extern void glaspen2_set_invert_dirty(int x, int y, int w, int h); // 局部重绘脏区
extern unsigned char* glaspen2_render_canvas_overview(double bx, double by, double bw, double bh, int out_w, int out_h, int *out_len);
extern void glaspen2_set_view_transform(double pan_x, double pan_y, double zoom);
// 画布存储切换 + 无限画布(独立存储,全局仅一个画布)
extern void glaspen2_set_canvas_kind(int infinite);
extern int glaspen2_load_infinite_strokes(void);
extern void glaspen2_set_infinite_transform(double pan_x, double pan_y, double zoom);
extern void glaspen2_get_infinite_transform(double *pan_x, double *pan_y, double *zoom);

// ── 翻页动效(时光隧道)的页快照 ──
// 预热缓存: 从 center 起前后若干页一次载入+平滑(不切当前页、不动 STROKES),
// 逐帧渲染走缓存而不是每帧查库。返回载入的页数。
extern int glaspen2_debug_insert_pages(int n, int w, int h);
extern int glaspen2_preload_flip_pages(long long center, int going_next, int before, int after);
// 把缓存里第 slot 页画进 cairo 表面(透明底)。scale = 视口 backing scale;
// ox/oy/pscale = scale-to-fit 的"页像素→逻辑点"变换(不 fit 传 0,0,1);
// alpha = 整页不透明度(渐显/渐隐); white_bg = 白底(导出用)。返回 1 成功。
// depth = 该页在隧道里的页距(0=当前页); >=0 时快照被磨砂玻璃化
// (越深越暗/越模糊, 层次一眼可读), <0 = 不磨砂(原样)。
extern int glaspen2_paint_preview_into_surface(void *surface_ptr, int slot, double scale,
                                               double ox, double oy, double pscale,
                                               double alpha, int white_bg, double depth);

// Page navigation FFI
extern long long glaspen2_prev_screen_id(void);
extern long long glaspen2_next_screen_id(void);
extern long long glaspen2_create_next_page(long long current_id); // 末页后新建一页(守卫同新建画布)
extern long long glaspen2_get_current_screen_id(void);
// 某页相邻的第 n 页(n>0 向后/较新, n<0 向前/较旧; 0 = 自身)。
// 页按分辨率分组, 与 prev/next 同一邻接语义。找不到返回 0。
extern long long glaspen2_page_neighbor(long long screen_id, int n);
extern int glaspen2_load_strokes_for_screen(long long screen_id);
extern void glaspen2_smooth_loaded_strokes(void);
extern int  glaspen2_set_launch_at_login(int enable);
extern int  glaspen2_is_launch_at_login(void);
extern int glaspen2_stroke_count(void);
extern int glaspen2_get_stroke_point_count(int idx);
// 笔预设/设置钳制(glaspen-core presets 单一事实源)
extern double glaspen2_pressure_raw_width(double pressure, double width_scale);
extern int glaspen2_color_preset_count(void);
extern void glaspen2_color_preset_rgb(int i, double *r, double *g, double *b);
extern int glaspen2_width_preset_count(void);
extern double glaspen2_width_preset_value(int i);
extern int glaspen2_nearest_color_index(double r, double g, double b);
extern int glaspen2_nearest_width_index(double w);
extern double glaspen2_clamp_setting_double(const char *key, double v);
extern int glaspen2_clamp_setting_int(const char *key, int v);
extern void glaspen2_get_stroke_color(int idx, double *r, double *g, double *b);
extern double glaspen2_get_stroke_avg_width(int idx);
extern void glaspen2_get_stroke_point(int idx, int pidx, double *x, double *y);
extern double glaspen2_get_stroke_point_width(int idx, int pidx);
extern int glaspen2_undo_last_stroke(void);


// R1: 时光隧道几何(数学在 glaspen-core::tunnel, Swift 只做 FFI 调用/绘制)
extern int glaspen2_tunnel_place(double zc, double k, double travel,
                                 double surf_w, double surf_h, double *out);
extern double glaspen2_tunnel_front(void);

// R2: 预设显示名(索引/语言 -> UTF-8, 调用方 glaspen2_free_c_string)
extern char *glaspen2_color_name(int index, int lang);
extern char *glaspen2_width_name(int index, int lang);

// R3: 性能日志(Swift PerfLog → glaspen-core::perf)
extern void glaspen2_perf_init_from_env(void);
extern int glaspen2_perf_is_enabled(void);
extern unsigned long long glaspen2_perf_elapsed_us(unsigned long long since);
extern void glaspen2_perf_record(const char *name, unsigned long long dur_us, const char *notes);
extern void glaspen2_perf_summary(void);

// R4: 画布输入变换(view→canvas; 纯公式在 glaspen-core::canvas_input)
extern int glaspen2_canvas_input(double view_x, double view_y, int infinite,
                                 double zoom, double pan_x, double pan_y,
                                 double page_off_x, double page_off_y,
                                 int is_fit, double paper_ox, double paper_oy,
                                 double paper_scale, double *out_x, double *out_y);

// R5: 镜头结果直出(不再走 JSON 解析; 0 = 非法/NaN)
extern int glaspen2_camera_pan_values(double pan_x, double pan_y, double zoom,
                                      double dx, double dy,
                                      double *out_x, double *out_y, double *out_z);
extern int glaspen2_camera_zoom_at_values(double pan_x, double pan_y, double zoom,
                                          double factor, double vx, double vy,
                                          double *out_x, double *out_y, double *out_z,
                                          int *out_hit_max, int *out_hit_min, int *out_changed);
extern int glaspen2_camera_zoom_at_center_values(double pan_x, double pan_y, double zoom,
                                                 double factor, double viewport_w, double viewport_h,
                                                 double *out_x, double *out_y, double *out_z,
                                                 int *out_hit_max, int *out_hit_min, int *out_changed);
extern int glaspen2_camera_centered_values(double bx, double by, double bx2, double by2,
                                           double viewport_w, double viewport_h, double zoom,
                                           double *out_x, double *out_y, double *out_used_zoom);

// R6: 壳全局状态(Swift AppState → glaspen-core::shell_state)
extern void glaspen2_router_set_lang(int value);
extern int sw_app_screen_w(void);
extern int sw_app_screen_h(void);

// R7: chord → NSEvent.ModifierFlags.rawValue 掩码(解析在 core::chord)
extern unsigned long long glaspen2_chord_modifier_mask(const char *chord);

// R8: 快捷键生效表状态(values/lookup JSON; 调用方 free 返回的字符串)
extern void glaspen2_shortcut_store_install(const char *values_json, const char *lookup_json);
extern char *glaspen2_shortcut_store_chord(const char *action);
extern char *glaspen2_shortcut_store_action_for_chord(const char *chord);
extern char *glaspen2_shortcut_store_values_json(void);
extern int glaspen2_shortcut_store_is_empty(void);

// R9: 网格/绘制状态 getter(状态在 glaspen-core::view_state)
extern double glaspen2_router_grid_size(void);
extern int glaspen2_router_grid_divider(void);
extern int glaspen2_router_grid_follow(void);

// R9: ViewSettings 状态(旧 Swift 直调函数改为 C 入口)
extern void sw_set_infinite(int on);
extern void sw_set_minimap(int on);

// R12: 笔外观设置(Swift PenSettingsStore → Rust)
extern void sw_set_pen_settings(double r, double g, double b, double width,
                                int color_idx, int width_idx);

// R14: Rust objc2 预研(默认关闭; GLASPEN2_RUST_STATUS_ITEM=1 时走这里)
extern void *glaspen2_rust_status_item_create(void);

// R15/R21: Rust 创建覆盖层主窗口(默认; GLASPEN2_SWIFT_OVERLAY=1 回退 Swift)
extern void *glaspen2_rust_overlay_window_create(double x, double y, double width, double height);

// R16/R21: Rust 创建覆盖层内容容器/磨砂玻璃视图(默认; GLASPEN2_SWIFT_OVERLAY=1 回退)
extern void *glaspen2_rust_overlay_content_view_create(double x, double y, double width, double height);
extern void *glaspen2_rust_overlay_glass_view_create(double x, double y, double width, double height,
                                                    int enabled, double opacity);

// R17: 状态栏图标 Rust 渲染(opt-in: GLASPEN2_RUST_STATUS_ICON=1)
extern void *glaspen2_rust_status_icon_render(int eraser, int enabled, int ethereal, int infinite,
                                              double width_scale, double pen_r, double pen_g, double pen_b);

// R18/R21: 覆盖层内容组装(默认; GLASPEN2_SWIFT_OVERLAY=1 回退)
extern void glaspen2_rust_overlay_assemble_content(void *content, void *glass,
                                                  void *draw_view, void *window);

// R19/R21: Rust NSView 子类绘制视图(默认; GLASPEN2_SWIFT_DRAW_VIEW=1 回退)
extern void *glaspen2_rust_draw_view_create(double x, double y, double width, double height);

// R22: 状态栏主菜单创建/挂载(默认; GLASPEN2_SWIFT_MENU=1 回退 Swift)
extern void *glaspen2_rust_menu_create(void);
extern void glaspen2_rust_status_item_set_menu(void *item, void *menu);

// R23: 批量追加菜单项(默认; GLASPEN2_SWIFT_MENU_ITEMS=1 回退 Swift)
extern int glaspen2_rust_menu_add_items(void *menu, void *handler, int count,
                                        void **titles, int title_count, const char *action);

// R24: 顶层其余项/导出▸/更多▸ 子菜单(默认; GLASPEN2_SWIFT_MENU_FULL=1 回退)
extern int glaspen2_rust_menu_add_top_level(void *menu, void *handler, const char *titles_json,
                                            int launch_at_login_on, int glass_on);

// R25: 颜色/粗细菜单勾选更新(默认; GLASPEN2_SWIFT_MENU_STATE=1 回退)
extern int glaspen2_rust_menu_update_checkmarks(void *menu, int color_count, int width_count,
                                                int color_index, int width_index);

// R26: 菜单动态文案刷新(默认; GLASPEN2_SWIFT_MENU_TEXTS=1 回退)
extern int glaspen2_rust_menu_update_texts(void *menu, void **color_titles, int color_count,
                                           void **width_titles, int width_count,
                                           const char *top_titles_json, const char *export_titles_json,
                                           const char *more_titles_json, const char *mode_title,
                                           const char *toggle_title, void *mode_item, void *toggle_item);

// R27: 菜单项递归查找/标题/state(默认; GLASPEN2_SWIFT_MENU_LOOKUP=1 回退)
extern void *glaspen2_rust_menu_item_with_tag(void *menu, long tag);
extern int glaspen2_rust_menu_set_item_title(void *menu, long tag, const char *title);
extern int glaspen2_rust_menu_item_state(void *menu, long tag);

// R28: 光标壳(默认; GLASPEN2_SWIFT_CURSOR=1 回退)
extern void *glaspen2_rust_blank_cursor_create(void);
extern void *glaspen2_rust_arrow_cursor_create(void);
extern void glaspen2_rust_draw_cursor(void *ctx, int visible, double x, double y, int eraser);

// R29: 设置面板窗口创建(默认; GLASPEN2_SWIFT_SETTINGS_PANEL=1 回退)
extern void *glaspen2_rust_settings_window_create(const char *title, void *view_controller,
                                                   double width, double height);

// R30: 笔迹 surface 合成(opt-in: GLASPEN2_RUST_SURFACE=1)
extern int glaspen2_rust_draw_surface(void *ctx, void *image, double rx, double ry,
                                      double rw, double rh, double bx, double by,
                                      double bw, double bh, double scale);
