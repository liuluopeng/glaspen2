# AGENTS.md — glaspen2 仓库规则(任何 AI 助手均应遵守)

## Git 纪律:每次修改都提交到本地

- **每完成一处修改(功能点/修复/重构/微调),立即提交到本地 git**,不要攒着不提交
- 提交前先 `git status --short` 检查改动范围,**只提交与本次修改相关的文件**
- 提交粒度:一个功能点或一次修复 = 一个 commit;连续的小调整(调参数、改文案)可合并为一次
- commit message 用中文,格式:
  - `feat: 描述`(新功能)/ `fix: 描述`(修复)/ `refactor: 描述`(重构)
  - `perf: 描述`(性能)/ `chore: 描述`(构建/配置)/ `docs: 描述`(文档)
- message 简洁但说清做了什么,例如 `fix: 菜单颜色项在浅色模式下不可见`、
  `perf: 落笔时的 edited 标记改后台写`
- **不要推送远程**,除非用户明确要求
- 大改动(重命名、跨文件重构)前先提交当前基线,便于回滚

## 构建与调试命令(FVM)

- **Flutter 一律用 `fvm flutter ...`**(本机 Flutter 由 fvm 管理,裸 `flutter` 版本可能不对):
  - `fvm flutter analyze`(在 `flutter_settings/` 下跑,查 Dart 错误)
  - `fvm flutter run -d macos` 不用于主程序;调试 Dart 侧用
    `GLASPEN2_FLUTTER_CONFIG=Debug cargo run`(Debug 的 App.framework 走 JIT,
    可被 flutter attach;默认 Release 是 AOT)
- **Rust/整体构建**:`cargo build`(build.rs 会自动:编译 `glaspen2.m` →
  fvm 构建 Flutter macos-framework → 链接);运行:`cargo run`
- **测试**:`cargo test`(全仓)、`cargo test -p glaspen-chat`(聊天协议层)
- **FRB 改了 Rust API 后**:`cd flutter_settings && flutter_rust_bridge_codegen generate`,
  然后**必须 `cargo build`**(build.rs 监视生成的 Dart,会重建 framework;
  不重建就会运行时报 "Content hash … different from Rust side",面板全挂)
- **lint**:`sh scripts/lint.sh`;打包:`sh scripts/build-dmg.sh`

## 架构总览(当前状态)

Cargo workspace 多 crate 结构(`crates/` 下为核心与 Windows 两个新 crate):

```
┌─ macOS 主程序 glaspen2(根 crate = 壳)───────────────────────────────┐
│                                                                      │
│  ObjC 层  src/macos/glaspen2.m(唯一 ObjC 文件)                      │
│   ├ CGEventTap(挂主 runloop):笔/鼠标/键盘/滚轮事件统一入口           │
│   │   ├ 笔: modeler_begin/move/end → cairo 实时画 → 抬笔 commit      │
│   │   ├ 热键: J/K、⌥⌘↑/↓ 整页翻页(setIsVisible 系统动效)            │
│   │   │       ⌘⌃R GIF、⌘⌃2 草稿、⌘⌃3 直发、⌘⌃X 飘渺、⌘⌃V 直通        │
│   │   └ 飘渺模式: 隐藏/重现笔迹 = setIsVisible(窗口级系统动效)        │
│   ├ 覆盖层窗口(笔迹 layer + 网格 + 磨砂玻璃)+ 菜单栏(NSStatusItem)  │
│   │   └ 菜单图标: attributedTitle + NSTextAttachment(image 不渲染)   │
│   ├ C shim: glaspen2_macos_*(settings_json / set_setting / 导出…)   │
│   └ main.rs / macos.rs(壳): 入口分发, 依赖 glaspen-core/windows     │
│                                                                      │
│  crates/glaspen-core(共享核心, 平台无关)──────────────────────────── │
│   ├ lib.rs      Stroke / STROKES / TEST_LOCK + tokio runtime()       │
│   ├ modeler.rs  ink-stroke-modeler(平滑/预测/橡皮)                   │
│   ├ cairo_dl.rs 动态加载 libcairo(无链接依赖)                        │
│   ├ db.rs       SQLite: screens/strokes/points(软删)/缩略图缓存/     │
│   │              user_settings(键值设置);无限画布独立存储             │
│   ├ export/     FFI 大本营(拆分子模块):                              │
│   │   ├ mod.rs     绘图与模型器 FFI/描边/镜头/设置持久化/开机自启      │
│   │   ├ pages.rs   活页本:页管理/新建守卫/导航/列表/画布模式           │
│   │   ├ chat_glue.rs ⌘⌃2 草稿、⌘⌃3 直发、ShareInk 共享上行、涂鸦身份   │
│   │   ├ media.rs   文件导出:XOJ/PNG/SVG/GIF/PDF                      │
│   │   └ thumbs.rs  缩略图渲染缓存与画布总览载荷                        │
│   ├ api.rs? 不在这里 —— FRB 面板 API(api.rs)留在根 crate:           │
│   │   yaml/生成代码/链接参数零改动; 通过根的再导出用 crate::db 等旧路径│
│   └ update.rs / updater.rs  GitHub 检查更新 / 自更新帮手              │
│                                                                      │
│  crates/glaspen-windows  Windows 覆盖层(Win32 窗口/笔输入/管道服务)  │
│   └ 非 Windows 平台编译为空壳; 经 core 的 FFI 共享存储与逻辑          │
└──────────┬──────────────────────────────┬────────────────────────────┘
           │ FRB(DynamicLibrary.process,  │ │ tonic gRPC(客户端)
           │  同进程直调, api.rs 在根)     │ │ 127.0.0.1:50051
           ▼                              ▼ │(GLASPEN_CHAT_ENDPOINT)
┌─ flutter_settings 设置面板 ─┐   ┌─ axum(kongde 生态, 独立工程)─────┐
│ 默认 2 tabs: 设置 / 活页本   │   │ ChatStore: AppendMessages(⌘⌃3)  │
│ + 自由涂鸦 tab(开关,默认关) │   │            DraftInk(⌘⌃2 草稿)   │
│ + 共享画布开关(活页本内,    │   │            ShareInk(共享画布上行)│
│   仅集成开时可见)            │   │ /api/user/login、/api/chat/ink-route│
└─────────────────────────────┘   └───────────────────────────────────┘

┌─ chat/ = glaspen-chat crate(gRPC 客户端,glaspen2 的全部"后端交互")─┐
│  pb/           proto 生成(build.rs 监视 proto 文件)                  │
│  auth.rs       登录换 token(ureq, 自签放行)+ 内存缓存 + 失效        │
│  draft.rs      ⌘⌃2 草稿通道(DraftInk 客户端流)                      │
│  canvas.rs     共享画布上行(ShareInk 客户端流, tab 生命周期)          │
│  lib.rs        Sink(⌘⌃3 直发 AppendMessages)+ mock 模式              │
└──────────────────────────────────────────────────────────────────────┘
proto/glaspen/chat/v1/chat.proto = 与 axum 的唯一协议契约(演进规则见
docs/axum-chat-store.md §4: 字段号永不复用, axum 侧为源头, 整文件复制回来)
```

### 数据流速记

- **一笔的旅程**:pen 事件(event tap)→ `glaspen2_modeler_begin/move/end`
  (Rust, 写 DB)→ 抬笔 `glaspen2_modeler_commit_to_strokes`(平滑点列进
  STROKES)→ **钩子扇出**:`ink_draft_on_stroke_committed`(⌘⌃2 草稿流)+
  `ink_share_on_stroke_committed`(共享上行)
- **设置项**:Dart `setSetting(key, json)` → FRB → ObjC
  `glaspen2_macos_set_setting` → 按 key 分派到 `gl_settings_set_*` + 落
  `user_settings` 表 → 启动时按同序恢复。**新增设置键三处同步**:ObjC
  handler、`api.rs Settings`(from_json)、Dart(`_settingsToMap` + 界面)
- **翻页**:整页翻转,动效 = `[g_window setIsVisible:]` 系统动效
  (`page_flip_swap`,参数不可调,勿改成手写 alpha 动画)

## 铁律:聊天/共享是增强功能,不登录不影响涂鸦

- **glaspen2 的核心是涂鸦工具**:笔迹本地绘制、本地保存,永不依赖 axum
  / kongde 存在。任何聊天/共享/身份功能出问题(未连接、登录失败、被拒)
  一律**静默降级 + stderr**,绝不阻断涂鸦、不弹错误打断书写
- **总开关 `g_chat_integration` / 面板「启用手写消息与共享」默认关**:
  关 = ⌘⌃2/⌘⌃3 热键直通(不劫持)、登录与共享 tab 收起。新加集成功能
  必须挂在这个开关下面;用户可见文案不得出现 kongde/NAS 等内部名
- gRPC 调用不允许在主线程阻塞(key-down 路径只做非阻塞 push;
  阻塞等待一律 dispatch 到后台队列)

## 踩坑记录(都是真踩过的,别再踩)

- **FRB content hash 不一致**(面板初始化全挂):Dart 与 Rust 两侧生成
  代码不同步。codegen 后必须 `cargo build`;build.rs 已监视
  `flutter_settings/lib/src/rust/`,别删那几行 rerun-if-changed
- **chat/build.rs 必须监视 proto**(`rerun-if-changed=../proto/...`):
  否则 proto 变了绑定不重生,运行时才炸
- **`NSMenuItem.image` 在新版系统菜单不渲染**:菜单图标一律用
  `attributedTitle` + `NSTextAttachment` 内嵌图(照
  `gl_color_item_text` / `gl_width_item_text` 的模式)
- **窗口动效只能用 setIsVisible**(`[g_window setIsVisible:NO/YES]`,
  系统内置不可调参);别用 NSAnimationContext 手写 alpha——用户已明确
  否掉过一次。渐隐到渐显的间隔保持最小(dispatch_async 一个 tick)
- **位图上下文**:设过 `rep.size` 后 CTM 已自带点→像素缩放,再手动
  `CGContextScaleCTM` 会双重放大画出界(全透明);小形状先描 halo 再
  fill 会被 halo 洗白——顺序:halo → fill → 细描边
- **`glaspen2_macos_set_setting` 的 JSON 标量解析**必须带
  `NSJSONReadingFragmentsAllowed`,否则每个设置都静默变成 false/0
- **主线程锁序**:STROKES/INK_DRAFT/CANVAS 等全局锁不做嵌套获取;
  提交钩子在放掉 STROKES 锁之后才调扇出钩子
- **ObjC 函数定义顺序**:C99 隐式声明是错误——被后文引用的 static 函数
  需要前向声明(文件头有 decl 区)

## 开发一个新功能的流程

### 0. 起手
- `git status --short` 应只有预期改动;大功能开分支 `git switch -c feat/xxx`
- 先判断改哪一层:

| 层 | 位置 | 说明 |
|---|---|---|
| ObjC 交互层 | `src/macos/glaspen2.m` | 事件/窗口/菜单/系统动效 |
| Rust 核心 | `crates/glaspen-core/src/`(lib/db/modeler/state) | 数据模型/SQLite/平滑 |
| FFI 面 | `crates/glaspen-core/src/export/` | ObjC ↔ Rust 的一切入口(mod.rs+pages/chat_glue/media/thumbs) |
| 面板桥 | `src/api.rs`(+ FRB codegen) | 设置面板 ↔ Rust |
| 聊天协议 | `chat/` + `proto/glaspen/chat/v1/chat.proto` | 与 axum 的一切交互 |
| 面板 UI | `flutter_settings/lib/main.dart` | 4 个 tab |

### 1. 改 Rust / ObjC
- ObjC 或 Rust 改完 `cargo build`——build.rs 会先编 ObjC,**ObjC 报错时
  整个 build 挂**,从 stderr 里找 `glaspen2.m:N` 行号
- 改了 FRB 面(api.rs 的 `#[frb]` 函数或 Settings 结构)→ codegen → build

### 2. 改 proto(与 axum 的协议)
- 只追加:新 RPC/新消息/新 oneof 分支,字段号永不复用
- `chat/build.rs` 会自动重生成;`cargo test -p glaspen-chat` 有往返测试
- 更新对应 docs 文档 + proto 头部变更记录

### 3. 改设置面板(Dart)
- 新设置项 = ObjC handler 分支 + `api.rs Settings` 字段 + Dart UI 三处
  同步;`cd flutter_settings && fvm flutter analyze` 过了再提交
- 改了 `#[frb]` API 记得 codegen(`flutter_rust_bridge_codegen generate`,
  在 `flutter_settings/` 下跑)

### 4. 提交前自检
- `cargo build`(必须过,含 ObjC 编译)
- `cargo test`(全仓)+ `cargo test -p glaspen-chat`
- `cd flutter_settings && fvm flutter analyze`(0 error/warning)
- 手测主路径:涂鸦、翻页(淡入淡出)、⌘⌃2/⌘⌃3(集成开时)、设置面板 4 tab

### 5. 提交
- `git status --short` → 只 add 相关文件 → 中文 message → 不 push
- mock 说明:`GLASPEN_CHAT_MOCK` 未配置时,聊天/共享走模拟(stderr 打印
  帧摘要);真连用 `GLASPEN_CHAT_MOCK=0 GLASPEN_CHAT_ENDPOINT=...`;
  测试依赖默认 mock,别在环境里永久 export

## 目录速查

| 路径 | 内容 |
|---|---|
| `src/macos/glaspen2.m` | macOS 全部 ObjC(交互/窗口/菜单/动效) |
| `src/*.rs` | 壳:FRB 面(api.rs)/入口/ObjC 链接 |
| `crates/glaspen-core/` | 共享核心(存储/模型/导出/更新) |
| `crates/glaspen-windows/` | Windows 覆盖层(Win32/笔输入/管道) |
| `chat/` | glaspen-chat:gRPC 客户端 + proto 生成 + mock |
| `proto/glaspen/chat/v1/chat.proto` | 与 axum 的协议契约(唯一) |
| `flutter_settings/` | 设置面板(Flutter;macOS 走 FRB,Windows 走命名管道) |
| `docs/` | axum-chat-store / ink-draft-grpc / canvas-share-grpc / grpc-auth 等 |
| `installer/` `scripts/` | 打包(DMG/安装器)与构建辅助(lipo-shim 等) |
| `src/windows.rs` + `installer/` | Windows 版(覆盖层 + 设置独立进程) |
