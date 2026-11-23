# 平台边界与重复实现审计（2026-11-19）

> 目的：回答两件事——(1) 哪些代码是**真的**平台无关；(2) macOS 与 Windows
> 重复实现的逻辑，现在能不能只写一次。
>
> 方法：静态审计（读代码 + grep + 双人独立盘点交叉验证），**没有 Windows 真机**，
> Windows 侧行为结论均来自代码阅读。行数为 `wc -l` 实测，删除量是估算。

## 结论摘要

1. **core / chat / Flutter 面板是真的共享**：core 15,226 行、chat 1,969 行、
   面板 6,608 行只有 5 处 `Platform.is*` 分支；core 的 112 个 FFI 里 44 个被
   两个平台调用，Windows 还直接调了 79 处以 `glaspen_core::` 开头的 Rust API。
2. **但"共享 core"≠"Windows 上功能已实现"**：每个功能还需要一层 platform
   adapter（输入、窗口、渲染目标、剪贴板、通知、托盘、面板宿主、快捷键注册、
   单实例、开机自启）。过去的问题正是把 adapter 写成一次性 mac 实现，然后说
   "核心是平台无关的"。
3. **Windows 壳 5,611 行里约 45% 是平台无关逻辑的第二份实现**，与 macOS 侧
   合计约 1.5–1.7k（Windows）+ 0.9–1.1k（macOS/root）可以下沉到 core 或删掉。
4. **已经出现"假共享"实锤**：设置 DB 键名 9 个 mac/Win 不一致（注释却写
   "同键、可互换"）、Windows 笔迹渲染/模型器独立实现、面板 5 个桥方法空壳、
   `onSettingsChanged` 在 Windows 从未推送。详见 §4。

## 1. 分层现状（实测行数）

| 层 | 行数 | 平台属性 |
|---|---|---|
| `crates/glaspen-core` | 15,226 | 平台无关；41 处 `cfg(target_os/windows)` 只做 I/O（DB/日志/桌面路径、cairo 加载、更新解包、开机自启） |
| `chat/` | 1,969 | 平台无关（gRPC 客户端） |
| `flutter_settings/lib` | 6,608 | 一份 UI；5 处 `Platform.is*`（字号倍率、翻页动效区、GIF 提示、快捷键编辑器、桥工厂） |
| `src/api.rs`（FRB 面板面） | 1,201 | macOS 进程内面板专用；Windows 走命名管道，不经过它 |
| `src/macos/parts/*.m` + `glaspen2.m` | 6,662 | macOS adapter（ObjC） |
| `crates/glaspen-windows/src` | 5,611 | Windows adapter（Rust/Win32） |

**共享面实测**：core 共 112 个 `#[unsafe(no_mangle)]` FFI。

- 44 个被两个平台调用（DB/页、笔迹落库、导出格式、缩略图、更新检查、备份、chat/bubble 钩子等）。
- 52 个只有 macOS 调——其中一部分是**合法的**：Windows 是 Rust，直接调 core 的
  Rust API（`db::*`、`presets::*`、`update::*`、`page_thumbnails_blob` 等 79 处）。
- 另一部分**不合法**：core 有实现但 Windows 自己又写了一遍（§3 第 2、3 项）。
- 5 个死 FFI：`glaspen2_page_reorder`、`glaspen2_delete_last_stroke`、
  `glaspen2_now_secs`、`glaspen2_get_stroke_point_time`、`glaspen2_ocr_backfill_all`。

## 2. 真正可以复用的平台无关逻辑

这些已经在 core，且两个平台都该用（现状：有的用了，有的绕开了）：

- **存储/页策略**：SQLite 页/笔迹/点、软删、缩略图缓存、迁移与备份、页序、邻页 id。
- **笔迹模型**：`modeler.rs`（ink-stroke-modeler 封装）、压感→宽度、平滑。
- **预设与钳制**：`presets.rs`（色板/粗细/最近索引/设置值钳制）——**单源化的正面样板**。
- **导出媒体**：XOJ/PNG/SVG/GIF/PDF（`export/media.rs` 1,502 行）。
- **缩略图与总览渲染**：`export/thumbs.rs`（两个平台都在用，属于真正的共享）。
- **更新检查/下载/版本比较**：`update.rs`；解包/安装用 `cfg` 分段。
- **OCR 管线、NAS/本地备份、chat gRPC 客户端、device uuid**。
- **开机自启**：`export/mod.rs` 一个函数、两个 `cfg` 分支（plist vs HKCU\\Run）——
  **"单接口双实现"的样板**，比"两边各写一套"好。

## 3. 重复实现清单（按可单源化价值排序）

| # | 重复项 | macOS 证据 | Windows 证据 | 估算可删 | 做法 | 风险 |
|---|---|---|---|---|---|---|
| 1 | **设置 schema / 键分派 / 快照 / 启动恢复** | `menu.m:975-1046`（37 键快照）、`menu.m:1094-1273`（40 分支）、`run.m:2979-3152`（恢复） | `pipe.rs:521-687`（手写 40 键 JSON）、`pipe.rs:688-1043`（30 分支）、`run.rs:14-125`（恢复）；第三份 `api.rs:187-319` | Win −500，root −150，ObjC −150 | core 定义 `Settings{key, db_key, ty, default, clamp}` + `snapshot_json()` + `apply(key,val)->Effect`；两端只做 effect | 中高；顺带修 §4 的 DB 键漂移 |
| 2 | **笔迹光栅化** | 走 core `glaspen2_draw_rebuild_view` → `paint_strokes_into`（`export/mod.rs`，显式标注"仅 macOS 消费；Windows 用覆盖层自己的实现"） | `canvas.rs:71-270`（阴影/描边/填充/圆点）、`pen.rs` 轮廓、`actions.rs:292` 全量重绘 | Win −250~300 | Windows 把 DIB 的 cairo surface 交给 core 渲染入口（或抽出共享的逐段绘制原语） | 中；增量绘制性能需 Windows 真机实测 |
| 3 | **网格 + 分栏几何** | `run.m:110-230`（细网格/分界线/半屏/九宫格/康奈尔、吸附、粗细） | `canvas.rs:329-505`（同参数：GA/GR、140/70、166/83、0.3/0.75） | 约 −120（合计） | core `grid::lines(pan,zoom,w,h,grid,divider,screen)` 返回线段表，两端只画 | 低 |
| 4 | **镜头（camera）+ 无限画布切换** | `run.m:387-540`（pan/zoom/锚点/持久化/鼠标拖动） | `state.rs:81-98`、`actions.rs:428-538` | 约 −130（合计） | core 持有 pan/zoom/钳制/锚点，`pan_by/zoom_at/home/center`；两端只重绘 | 低中；注意两端线程模型（主线程 vs 消息循环） |
| 5 | **动作目录 + 快捷键 chord** | `menu.m:652-972`（19 动作、默认 chord、解析/规范化/迁移/保留键） | `run.rs:210-233` 硬编码 19 个 `RegisterHotKey`；面板 `main.shortcuts.dart:332` 直接禁用编辑 | Win −200，ObjC −250 | core/共享层定义动作表（id、默认 chord、显示名）；Dart 经 FRB/管道取；Windows 补注册逻辑 | 中；同时补上 Windows"可配置快捷键"缺口 |
| 6 | **录制状态机**（GIF/草稿/直发） | `run.m:1836-1967`（按住窗口、异步收尾、互斥） | `actions.rs:557-700`（同状态、同文案） | 约 −100（合计） | core 录制 FSM（状态 + 起止 + 错误文案），平台只做剪贴板/截图 | 低 |
| 7 | **缩略图条 + 总览载荷** | `run.m:546-689`（±5 页、64×40、gap 4、高亮）、`menu.m:579-619`（bbox +5%） | `actions.rs:1121-1355`、`pipe.rs:103-148`（同参数） | 约 −300（合计） | core 出页 id 列表/条带布局/bbox 映射；绘制与 PNG 编码平台做 | 低 |
| 8 | **页导航 + 页信息/日期文案** | `run.m:1564-1648`、`glaspen2.m:506-542`（今天/昨天、"第N页 第P/T页"） | `actions.rs:333-390`、`hud.rs:9-28` | 约 −100（合计） | core 出页信息字符串（含 locale）；保留"再按一次建页"策略差异 | 低 |
| 9 | **通知文案 / i18n** | 93 组双语 `L()` | 107 条中文 HUD 文案（无英文） | 约 −100 | core 文案表（zh/en），平台只负责显示 | 低；Windows 顺带补齐英文 |
| 10 | **单实例 / 开机自启** | `run.m:2742-2769`（flock+NSWorkspace） | `lib.rs:48-60`（命名互斥体） | 0 | 保持平台实现（自启已有 core 双分支，可照抄到单实例） | 低 |

**合并后的判断**：真正"纯逻辑"、可直接下沉的是 #1、#3、#4、#6、#7、#8、#9
（约 800–1,000 行）；#2 是最大单笔（渲染），但需要 Windows 真机过性能；
#5 涉及面板与两端键位，收益大、跨度也大。

## 4. 实锤："说共享、其实没共享"的三处证据

**(a) 设置 DB 键名在 mac/Win 之间是两套（9 个键不一致）**

macOS 写 snake_case，Windows 写面板 camelCase：

| 面板键 | macOS 存的键 | Windows 存的键 |
|---|---|---|
| frostedGlass | `glass_enabled` | `frostedGlass` |
| glassFollowStrokes | `glass_follow_strokes` | `glassFollowStrokes` |
| gridFollowStrokes | `grid_follow_strokes` | `gridFollowStrokes` |
| softShadow | `soft_shadow` | `softShadow` |
| invertInk | `invert_ink` | `invertInk` |
| invertFps | `invert_fps` | `invertFps` |
| pressureMonitor | `pressure_monitor` | `pressureMonitor` |
| notebookStyle | `notebook_style` | `notebookStyle` |
| panelTransparent | `panel_transparent` | `panelTransparent` |

后果：`db::restore_latest_backup` 是共享的，**把 macOS 的备份恢复到 Windows
会静默丢掉这 9 项**。另外 `outline` macOS 落库（`run.m:435`）、Windows 只存内存
（`state.rs:36-39`）；`invertInk/invertFps` Windows 存了但渲染不消费。
而 Windows 代码注释写着"键名与 macOS 一致，设置数据库可互换"（`state.rs:138`、
`pipe.rs:615`）——这正是需要杜绝的"假共享"。

**(b) Windows 笔迹渲染仍是独立第二实现(模型器已单源,D9b)**

- core 的 `paint_strokes_into`（阴影/描边/填充/alpha 合成）只服务 macOS，注释
  自己写明"Windows 用覆盖层自己的实现"；
- Windows 在 `canvas.rs` 用 cairo（同一份 `cairo_dl`）又画了一遍同样的 pass；
- 模型器：已单源（D9b）—— macOS 走 core `glaspen2_modeler_*`（其全局 STATE
  内部委托 `modeler_session`），Windows 走同一 `ModelerSession` 的薄适配层
  `pen_modeler.rs`；平滑数学不再有第二份，adapter 只保留事件判定与展示映射。

**(c) Flutter 面板在 Windows 上是"半壳"**

`main.bridge.dart` 的 37 个抽象方法两端都 override（编译期强制），但 Windows
实现里 5 个是空壳：`exportPagePngBytes`(`:666`)、`exportPagePng`(`:758`)、
`exportPageSvg`(`:764`)、`exportSelectedPdf`(`:770`)、`ocrSearch`(`:776`)。
另外 `onSettingsChanged` 在 Windows 侧**从未被推送**（`crates/glaspen-windows`
0 处引用）——用热键改设置后面板不会刷新，只能重开面板。
快捷键编辑器在 Windows 直接显示"仅支持 macOS"（`main.shortcuts.dart:332`）。

## 5. 明确的功能缺口（不是重复，但同属"mac 写完忘了 win"）

- 面板：单页 PNG/SVG 导出、勾选页合成 PDF、OCR 搜索（见上，空壳）。
- 设置推送：`onSettingsChanged`（见上）。
- 可配置快捷键：Windows 硬编码 19 个热键，不能改、不能解绑。
- 外观：`invertInk/invertFps` 不消费；`outline` 不持久；磨砂玻璃透明度、
  翻页动效、真穿透、飘渺模式的个别分支仅 macOS。
- 翻页动画/时光隧道（`run.m:697-1448`，约 750 行）仅 macOS；Windows 是直接
  切页。这是**有意差距**还是待补，需要产品决定。

## 6. 今后如何判断"平台无关"（规则）

1. **一个功能 = core 逻辑 + adapter 接口 + 两个平台的 adapter 实现**。只有
   adapter 才是"平台相关"的；core 里不应该出现 UI 状态机、设置键名、用户文案。
2. 平台层只允许出现：OS API 调用、坐标/单位换算、渲染目标绑定、剪贴板/通知/
   托盘/权限等原生资源操作。
3. **状态单源**：运行期状态（颜色/粗细/网格/镜头/录制中）要么放 core，要么在
   文档里明确列为 per-platform，禁止两个壳各存一份"看起来一样"的字段。
4. **设置键单源**：core 维护 `key → db_key/type/default/clamp`；新增设置键必须
   一次改 core + 两个 adapter（编译期强制），Dart 侧从 core 拉 schema。
5. **验收口径**：在 mac 上做完功能后，必须回答"Windows 现在会怎样"——能跑 /
   有 gap（列入 §5）/ 明确 unsupported。禁止只说"核心是平台无关的"。
6. **CI**：adapter trait 每平台必须实现（缺实现编译不过）；Windows 编译门至少
   覆盖 core + windows crate；Flutter 的 `Platform.is*` 白名单化。
7. 本文档随功能更新维护成"功能 × 平台"矩阵；新增 feature 若只写了一个平台，
   必须在本文件登记。

## 7. 建议执行顺序

- **Phase 0（1–2 天，低风险，先做）**：core 统一设置键/DB 键表 + 迁移（修 §4a
  的跨平台丢设置）；core camera、grid 几何、页信息字符串；删 5 个死 FFI。
- **Phase 1（约 3–5 天）**：core `Settings` schema + 两端 adapter（消灭两份
  键分派/快照/恢复）；动作目录 + Windows 可配置快捷键；通知文案单源。
- **Phase 2（约 1 周，需 Windows 真机）**：Windows 渲染/模型器切到 core 实现，
  过性能与压感回归；缩略图条/总览载荷下沉。
- **Phase 3**：面板 5 个空壳接通（导出/OCR）+ `onSettingsChanged` 推送 +
  §6.6 的 CI 门。

每阶段：`cargo test` / `sh scripts/lint.sh` / mac 手测；涉及 Windows 的必须
在 Windows 真机上验证后再合。

## 8. 审计局限

- 无 Windows 真机：Windows 侧结论来自代码阅读，未做运行时验证。
- "可删行数"是静态估算，实际取决于抽出的 trait/接口粒度。
- 少数差异可能是有意为之（例如 invertInk 注释写"macOS v1 独有"），执行前需确认。

## 9. 执行现状(2026-11-22): 从"文档规则"变成"门禁"

单源化已落地并被测试/CI 强制:

- **动作/面板平台差异**: `crates/glaspen-core/src/ui.rs` 是唯一来源(默认键位/保留
  组合/强制绑定/修饰键显示/键盘示意图/字号/功能可用性)。macOS 走 FRB
  `uiConfigJson`, Windows 走命名管道 `uiConfig`, Flutter 无平台行为分支
  (`platform_parity_test.dart` 守卫)。
- **设置项契约**: `crates/glaspen-core/src/settings.rs` 登记 key / db_key / 类型 /
  平台可用性。`cargo test` 用 `include_str!` 扫描 Flutter `_settingsToMap`、
  `api.rs Settings`、Windows 管道 `getSettings`, 任一方少/多一个键即失败
  (本规则已抓到 `nasUrl` 面板漏映射的真实 bug)。`user_settings` 落库键也由该
  登记表归一(旧 camelCase 行在启动时合并)。
- **Windows 壳**: 热键默认/保留/强制绑定从 `ui.rs` 取, 不再有第二份表。
- **CI 硬门**: `windows-check` 不可 `continue-on-error`; 现在跑
  `cargo test -p glaspen-windows`(Windows-only 单测)、core 的 db/settings/ui
  测试、`flutter build windows`、`flutter test`(Windows)。macOS job 追加
  `scripts/check-platform-parity.sh`(无平台分支/无静默空壳/无 todo!/unimplemented!/
  版本一致)。
- **面板协议唯一契约**: `crates/glaspen-core/src/protocol.rs` 定义命令表、请求
  解析、响应帧(单行 + 换行)与 `PROTOCOL_VERSION`; Windows 管道拒绝未登记命令,
  `uiConfig` 做面板版本握手(独立 exe 新旧混跑时明确报错, 不再静默错乱)。`cargo test`
  扫描管道 dispatch 与 Flutter 请求, 少一方/多一方都失败; Flutter 的
  `kPanelProtocolVersion` 也被测试锁定必须与 core 一致。
- **故意空缺原则**: 某平台未实现的能力必须由 `ui_config_json` 的 `features` 隐藏
  入口、管道/FRB 返回 `UnsupportedError`, **禁止半实现或静默空壳**; 真实缺口因此
  在真机上一眼可见, 而不是"看起来有按钮、点了没反应"。

尚未单源化的债务: 见 **[`duplication-backlog.md`](duplication-backlog.md)**
(带 ID 的持久清单 + 进度记录;每项含证据/单源方案/验收标准/状态)。
设置 effect 分派(D1)、动作语义(D3)、chord(D2)、页信息(D4)、camera(D5)、
grid(D6)、minimap(D7)、录制 FSM(D8)、模型器/绘制(D9)、i18n(D10)、
Flutter 设置表单(D11)、快照构建(D12)、死代码(D13)。
