# 跨平台重复实现清单与进度表

> 目的: macOS/Windows 两份实现中,**可以只写一次**的部分集中登记在这里。
> 这个文件是**持久台账**,不是一次性审计: 每开始/完成一项都改状态并追加进度,
> 即使中途停下, 下一个人(或下一次会话)从表格就能接着做 —— 不依赖聊天记录。
>
> 配套规则见 [platform-parity.md](platform-parity.md) §6/§9:
> 一个功能 = core 逻辑 + 两个 adapter;未实现的平台宁可**空缺并隐藏**,
> 不写半实现。core 里不要出现 UI 状态机/设置键名/用户文案。

## 状态图例

| 标记 | 含义 |
|---|---|
| ⬜ TODO | 未开始 |
| 🟨 DOING | 进行中(必须写"正在改的文件/下一步") |
| ✅ DONE | 完成(填 commit) |
| ⛔ 保留 | 评估后确认属于 OS adapter,不合并(写明原因) |

## 使用规则

1. 开始一项: 状态改 🟨,在《进度记录》写日期 + 范围 + 下一步。
2. 完成一项: 状态改 ✅,写 commit hash + 验收命令结果;若同时改了两端,确认
   `cargo test --workspace --exclude glaspen2_installer`、`flutter test`、
   `scripts/check-platform-parity.sh` 全绿。
3. 中途停止: 状态保持 🟨,把"当前进度/卡点/下一步"写进《进度记录》,
   不要删除表格行;未验证的部分标出来。
4. 新增重复项: 在表尾加 D-ID,按同样格式补"证据/方案/验收"。

## 已完成(单一实现,勿再复制)

| ID | 已单源的内容 | 唯一来源 | 防漂移措施 | commit |
|---|---|---|---|---|
| E1 | 动作 id / 默认键位 / 保留组合 / 强制绑定 / 修饰键显示 / 键盘示意图 / 字号 / 功能可用性 | `core/ui.rs` + `uiConfig` | Flutter `platform_parity_test`;Rust catalog 对称测试 | 358c941 / 3a10a98 |
| E2 | Windows 热键默认表/保留/强制绑定 | `core/ui.rs` | hotkeys 单测断言 effective 表 == `ui::ACTIONS` | 8bef542 |
| E3 | 设置 key / 落库键 / 类型 / 平台可用性 | `core/settings.rs` | 扫描 Flutter `_settingsToMap` / `api.rs Settings` / Windows 管道;跨平台键归一 | b80f05b |
| E4 | 面板命令表 / 响应帧 / 协议版本 | `core/protocol.rs` | 扫描管道 dispatch 与 Flutter 请求集合;版本握手 | 953d74e |
| E5 | 面板平台行为分支 | Flutter 无分支(仅桥工厂) | parity 脚本 + Dart 守卫测试 | 3a10a98 / 39206c3 |
| E6 | 跨平台 CI 行为验证(Windows 壳单测/Flutter Windows 构建/core 契约) | `.github/workflows/ci.yml` | windows-check 硬门 | 39206c3 |

## 待办总表

> 规模是"可删/可省的重复行数"估算,不是精确行数;证据行号以当前 main 为准。
> 优先级:P0 最便宜且防漂移收益最高;P1 行为漂移最大;P2/P3 需要更多设计或真机。

| ID | 重复项 | 现状证据(macOS ↔ Windows) | 规模(约) | 单源方案 | 验收标准 | 优先级 | 估时 | 状态 |
|---|---|---|---|---|---|---|---|---|
| D1 | **设置 apply 分派** | `src/macos/parts/menu.m:1104-1283`(180 行,40 个 `isEqualToString`) ↔ `crates/glaspen-windows/src/overlay/pipe.rs:940-1348`(~408 行,41 个 `key ==`) | 450–550 行 | core 维护 `(key,value) -> [Effect]`;两端只实现 `apply(effect)`(窗口/重绘/菜单/热键等 OS 动作)。key/类型/默认/落库已在 D 前置(E3) | core 单测覆盖每个 effect;两端 `apply` 穷举 match;新增设置项不实现两端编译/测试即失败 | P1 | 3–5 d | ⬜ TODO |
| D2 | **chord 解析/规范化/校验/旧默认迁移** | `src/macos/parts/menu.m` 的 `gl_shortcut_*` 区块(~660–1010;defaults:679,migrate:888) ↔ `crates/glaspen-windows/src/overlay/hotkeys.rs` normalize:97 / parse:187 / migrate:330 | 250–350 行 | `core/chord.rs`: 令牌规范化、key→VK/NSKeyCode 映射表、保留/冲突校验、旧默认迁移;两端只做 OS 注册与按压分派 | 同一 chord 表在两端产出相同规范化结果;迁移用例双向跑;两端只允许 OS 映射差异 | P0 | 1–2 d | ⬜ TODO |
| D3 | **动作语义(20 个 action 的 effect)** | `run.m:1739` `perform_action` ↔ `hotkeys.rs:848` `dispatch_hotkey` + `actions.rs` 各 action 实现 | 200–300 行 | 与 D1 同形的 `ActionEffect` 枚举:camera/page/record/export/clipboard/…;两端 `match action -> effect` 后执行 OS 侧动作 | 动作 id 一致性已有;补 effect 层单测;新增动作缺一端实现即失败 | P1 | 2–3 d | ⬜ TODO |
| D4 | **页信息/日期文案** | `src/macos/glaspen2.m:527` `show_page_info`(+ run.m:1445/1587/1612/1640) ↔ `actions.rs:587-606` `page_info_text`、`hud.rs:9` `date_label` | 50–80 行 + 文案 | `core/pageinfo.rs` 纯函数(今天/昨天/N 页/P/T 页、locale),两端只显示 | 固定时钟/页序的表驱动测试;两端输出同字符串 | P0 | 0.5–1 d | ⬜ TODO |
| D5 | **camera 数学(pan/zoom/clamp/持久化)** | `run.m:461` `canvas_pan_by`、`run.m:507` `canvas_zoom_at`、鼠标抓手/持久化 ↔ `actions.rs:654/674/687/2596/2603` | 130–200 行 | `core/camera.rs`(pan_by/zoom_at/anchor/clamp/home);两端只提供画布尺寸/重绘 | 纯函数测试(边界/锚点/100% 钳制);两端同一输入同结果 | P2 | 1–2 d | ⬜ TODO |
| D6 | **网格/分栏几何** | `src/macos/glaspen2.m` grid 变量与绘制(≈385–435 及绘制函数) ↔ `canvas.rs:329` `draw_grid` + actions 参数 | 120–180 行 | `core/grid.rs::lines(pan,zoom,w,h,mode)` 返回线段表;两端只画 | 线段表快照测试(无/两栏/上下/九宫格/康奈尔) | P2 | 1 d | ⬜ TODO |
| D7 | **minimap 缩略图条布局/载荷** | `run.m:541+` + `glaspen2.m` `draw_minimap`(声明:214) ↔ `actions.rs:2709` `minimap_page_ids`、`2740` `minimap_pump`、`2805` `draw_minimap` | 150–250 行 | core 出 page ids + 条带布局(尺寸/间距/页号);两端做 PNG/绘制/缓存 | 页 id 序列与布局表测试(首末页/软删页) | P2 | 1–2 d | ⬜ TODO |
| D8 | **录制 FSM(GIF/草稿/直发)** | `run.m:1847/1861/1890/1902` ↔ `actions.rs:815/827/871/881` | ~100 行 | `core/recording.rs` 状态机(状态/起止/错误/计时);两端做截图/剪贴板/窗口 | 状态迁移表测试;错误文案同源 | P2 | 1–2 d | ⬜ TODO |
| D9 | **模型器接入 + 绘制 pass 规范** | core `glaspen2_modeler_*` + `export/mod.rs:630` `paint_strokes_into` ↔ `state.rs:218` 自有 StrokeModeler、`pen.rs:165/248`、`canvas.rs`(545 行;shadow:71 / outline:106) | 600–900 行 | 短期: core 提供 modeler 薄封装统一接入;中期: core 输出 **draw commands**(halo→fill→stroke、脏矩形规则),两端保留各自表面 | 同输入同绘制指令序列;Windows 真机性能回归 | P3 | 1–2 w | ⬜ TODO |
| D10 | **i18n/通知文案** | mac `L(@"zh",@"en")` 共 109 对 ↔ Windows overlay 中约 190 条中/英字面量 | ~300 条 | `core/i18n.rs`: 文案 key + zh/en + 占位符;两端只显示 | 键集合测试:两端使用 ⊆ core 表;缺文案即失败 | P1 | 2–3 d | ⬜ TODO |
| D11 | **Flutter 设置表单元数据(手写)** | `main.settings_state.dart`(321 行)+ `main.settings_widgets.dart`(1309 行)按 key 手写控件/labels/default/clamp | 400–600 行 | settings registry 扩展 section/label/range/unit;Dart 从 schema 生成控件;平台可用性已由 E3 推导 | 新增设置项只改 core registry + 两端 effect;Dart 无 per-key 手写 | P3 | 2–3 d | ⬜ TODO |
| D12 | **面板设置快照构建** | `menu.m:985` `glaspen2_macos_settings_json` ↔ `pipe.rs:767/869` `getSettings` 的巨型 format! | ~100 行 + 类型错漏风险 | 形状/排序/类型由 core schema 决定;两端只提供"值"(活状态 + DB);统一用 serde/schema 序列化 | 两端快照键集合已有测试;补值类型/默认一致性测试 | P2 | 1–2 d | ⬜ TODO |
| D13 | **死代码/历史命令清理** | 4 个零调用 FFI: `glaspen2_page_reorder`/`glaspen2_delete_last_stroke`/`glaspen2_now_secs`/`glaspen2_get_stroke_point_time`;Windows 管道 `stageUpdate`(pipe.rs:1501,Dart 本地实现)、`getPageThumbnail`(已被批量接口取代) | ~100 行 | 直接删除;protocol 表里标 Legacy 的条目一并删(先确认一个 release 无回滚需求) | 全仓 grep 零引用;cargo/flutter 全绿 | P0 | 0.5 d | ⬜ TODO |

### 评估后确认"保留两份"的(不要合并)

| 项 | 原因 |
|---|---|
| 输入捕获(CGEventTap ↔ WM_INPUT/Raw HID) | OS 事件模型本质不同;共同部分已下沉(模型器/笔迹数据),adapter 保留 |
| 窗口/焦点/z-order/taskbar/DPI | Win32 与 AppKit 语义不同;靠真机清单验证 |
| RegisterHotKey 注册/IME 冲突/占用降级 | OS 注册机制不同;chord 规则合并(D2)后剩余注册适配保留 |
| 安装器/自动更新解包/权限(TCC) | 平台机制不同;更新协议/版本策略可共享(E 系列已含版本单源) |
| 文件锁/rename 语义等 | 已抽出 helper(core 内的平台分支),不再要求跨平台同实现 |
| 剪贴板/通知展示 | 原生 API 不同;只共享内容生成 |

## 每项的统一验收模板

```
[ ] 一个 core 模块承载规则/状态/文案(纯函数优先)
[ ] 两端 adapter 只执行 effect,不再包含规则/文案/几何
[ ] 契约测试: 至少一个"两端输入相同 → 输出相同"或用例表
[ ] cargo test --workspace --exclude glaspen2_installer 通过
[ ] flutter analyze / flutter test 通过
[ ] scripts/check-platform-parity.sh 通过(无分支/空壳/todo!)
[ ] Windows CI(windows-check)通过;需要真机的项在表里注明
[ ] 本文件: 状态改 ✅、填 commit、进度记录追加一行
```

## 进度记录(append-only)

| 日期 | 项 | 进度/决定 | 下一步 | commit |
|---|---|---|---|---|
| 2026-11-22 | E1–E6 | 动作/UI 配置、Windows 热键默认、设置 registry、协议契约、面板分支、Windows CI 行为门均已单源化 | — | 358c941/8bef542/3a10a98/b80f05b/39206c3/953d74e |
| 2026-11-22 | D1–D13 | 建立本清单;当前全部 TODO(除 E 系列) | 建议从 P0(D2 chord、D4 页信息、D13 死代码)开始 | — |
