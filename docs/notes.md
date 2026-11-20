# 讨论笔记 (docs/notes.md)

> 记录"讨论过 / 考虑过但未实施"的决策, 避免以后重复讨论。
> 每条记录: 日期、主题、结论、状态。

---

## 2026-08-04 — macOS 与 Windows 共用一份 Flutter UI (只存在一种 UI)

### 现状 (已确认)

1. **UI 代码库已经是唯一的**: macOS 和 Windows 都从同一份 `flutter_settings/` 构建。
2. **传输层已抽象, 但不平等**:
   - `_SettingsBridge` 抽象类: macOS = `_MethodChannelBridge` (内嵌 FlutterEngine, 进程内);
     Windows = `_NamedPipeBridge` (独立 `glaspen2_settings.exe`, 命名管道, main.dart:76)
   - Windows 管道桥**只实现了最小协议** (getSettings/setSetting/onSettingsChanged),
     高级功能走不通。
3. **高级功能直接调 MethodChannel** (main.dart:453-1110):
   `listPages` / `getPageThumbnail` / `searchText` / `navigateToPage` / `deletePage` /
   `recognizeText` / `ocrBackfill` / `exportAnimatedGif` / `exportPdf` —
   **Windows 上会抛 MissingPluginException**, 内容页在 Windows 实际不可用。
4. **UI 平台分支 6 处** (main.dart:368/387/435/570/936 + createBridge):
   OCR 区域仅 macOS 显示; 动画 GIF 按钮 Windows 显示"仅 macOS 可用";
   resizeToFit 窗口自适应仅 macOS。
5. **宿主形态不同**: macOS 内嵌 FlutterEngine (build.rs 链 Flutter 框架);
   Windows 独立 Flutter 进程 (C# SettingsPipeServer + Process.Start)。

### 结论: "一个代码库"已是事实, 缺的是功能对等

三级路径 (均未实施):

- **方案 A — 功能对等 (最小正解)**: 把 9 个高级调用从 `_channel` 收进
  `_SettingsBridge` 抽象方法; macOS 实现走 MethodChannel, Windows 实现走管道 +
  宿主补命令; 删掉 UI 里的 Platform 分支 (OCR/动画 GIF 两平台都显示 —
  Rust 侧 `glaspen2_save_animated_gif` 等 FFI 全是共享的)。
  结果: 一份 UI 完全一致, 只剩 `createBridge` 一行平台判断。
- **方案 B — 宿主统一 (更彻底)**: macOS 也弃用内嵌引擎, 改为独立 Flutter 进程 +
  管道。传输层只剩一种, `createBridge` 可删, mac 主程序不再依赖 Flutter 框架,
  两平台宿主对称。代价: ObjC 侧写进程启动 + 管道服务器; 设置窗口的
  App activation policy 切换逻辑需搬到独立进程。
- **方案 C — 全 Flutter 重写**: 整个 app 用 Flutter 做 UI+主进程, 平台通道只留
  输入捕获薄壳。愿景级, 工程量大。

### 倾向

A 先行 (一份 UI 成立), 中长期走向 B (宿主对称 + mac 构建瘦身)。
实施会涉及 Windows 宿主 (C# 管道服务器) — 需 Windows 侧确认后再动。

### 状态更新 (2026-09-20): 方案 A 已落地, 但 macOS 侧不再用 MethodChannel

- 9 个高级调用已全部收进 `_SettingsBridge` 抽象 (`listPages` / `deletePage` /
  `navigateToPage` / `triggerHotkey` / `exportPdf` / `canvasOverview` 系列等),
  UI 里的平台分支已消除; OCR 相关功能整体删除。
- **macOS 侧改用 flutter_rust_bridge, FlutterMethodChannel 已删除**: 面板与 Rust
  同进程, Dart 用 `ExternalLibrary.process()` 直接 dlsym Rust 符号(`src/api.rs`
  是唯一接口面, ObjC 状态经 `glaspen2_macos_*` C shim 反向调用)。
- Windows 仍是独立进程 + 命名管道(FRB 跨进程用不上), 管道补齐了 `deletePage` /
  `exportPdf` / `pageThumbnails`。
- 方案 B 的动机之一"mac 主程序不再依赖 Flutter 框架"因此不再成立 —— 现在反而更
  依赖内嵌引擎了。
- 调试方式见 `docs/debugging.md`。

---

## 2026-11-20 — 导入 rnote / Xournal++ 旧笔记为"历史涂鸦" (可行性讨论, 未实施)

### 问题
能否把 `/Volumes/six/note` 里的 `.rnote`(rnote 0.13/0.14) 与 `.xopp`(Xournal++ 1.3.2)
笔记, 作为"历史上的涂鸦"导入 glaspen2 的既有数据模型? 效果与代价如何?

### 语料实测 (2026-11-20 全量扫描; 源码参考 ~/xx/rnote、~/xx/xournalpp)

- rnote: 22 个 0.13/0.14 文件 + 1 个 0.9.0 旧格式; 共 116,798 笔 / 7.49M 点段。
  - 所有可解析笔迹的段全是 `lineto`(无贝塞尔), 每段 = 1 点, 直接映射即可。
  - 布局: continuous_vertical 14 个, infinite 6 个、semi_infinite 2 个。
  - 页面 format: 1587.4×2245.0 (@96/150dpi, 即 A4@192dpi 等价) 最多; 另有 2245×1587、1123×1587。
  - 非笔迹元素极少: shapestroke 14、textstroke 1 → 丢弃损失可忽略 (0.9 旧文件另有 1 text + 3 image, 不导入)。
  - 按 format 网格切页, 有笔迹的页格 ~245 个 (actual 切页会略多)。
- xopp: 10 个文件, 381 页 / 15,320 笔 / 829,736 点。
  - 页面 595.28×841.89pt @72dpi(A4); 除 1 个 327 页 PDF 批注书外, 背景全是 solid。
  - 全部 `tool="pen"`; 单页最多 52,557 点 (当前 glaspen2 最密页 4,793 点, 约 10 倍)。
- 合计: 约 132k 笔 / 8.3M 点 / ~300 个内容页 (另 327 页 PDF 书可跳过)。

### 映射结论: 可行, 且笔迹几乎没有保真损失

- 页: `screens(screen_w=format.width, screen_h=format.height)`, 按 format 尺寸的
  原点对齐网格切分(row-major), 只建有笔迹的格; 跨页笔迹在页边界切开。
- 笔/点: `strokes(color_rgb=笔色, width_scale=1)` + `points(x, y, width)`。
  - rnote: `width = style.smooth.stroke_width × pressure` (linear);
    `x,y = path.lineto.end.pos`, 逐点 pressure 已内嵌在 JSON。
  - xopp: fileversion 4 的 `width` 属性 = [基准宽(pt), 各点压力...];
    `width_px = 基准宽 × pressure[i]`, 坐标是 72dpi pt — **1:1 直接导入, 不做 dpi 换算**,
    相对比例自然正确(显示端 scale-to-fit 负责大小)。
- 颜色: 语料全为实色 pen; 高亮笔的半透明色会丢 alpha(现有 schema 无 alpha 列)。
- z 序: 按文件内顺序插入, `strokes.id` 升序即绘制序。
- 不能导入: shapestroke / text / PDF 背景 / 0.9.0 旧 schema (各占极少数)。

### 实测视觉样本 (用上述映射把样本页画成 PNG)

- `JB/runs/20261120-note-preview/xopp-13-p1.png` (13.xopp 第 1 页, 50,477 点)
- `JB/runs/20261120-note-preview/rnote-quick-p1.png` (quick_note.rnote 第 1 页)
- 观感: 字形、粗细、压力变化基本保留, 与原文肉眼一致。

### 可能的效果

- 活页本会按 `w×h` 自动多出"本子"卡片(1587×2245 / 2245×1587 / 595×842 …), 翻页、
  时光隧道、缩略图、PDF 导出、备份/回导全部直接复用, 无需新管线。
- **同尺寸的所有文件会合并进同一个本子**(面板分组只认 w×h), 无法按文件名区分来源;
  首次导入后组序按"组内最大 id"排, 历史本子会顶到书架最前。
- 玻璃是透明覆盖层: 旧笔记只带墨迹, 不带纸面(白底/网格/PDF 背景), 阅读观感取决于
  当前玻璃背景。页几何 ≠ 屏幕时已有 scale-to-fit: 3440×1440 屏上 A4 纵向页显示为
  1017×1440 居中窄条, 两侧留白; 在 fit 页上继续书写已有坐标映射, 可直接批注。
- 翻页只在本子(同几何)内; 历史本子与日常本子互不串页, 符合"历史"定位。

### 代价与风险

- 数据量: 8.3M 点 → points 表约 +0.4–0.5GB (现 1.46M 点 / 101MB 全库), 导入后库约
  0.55–0.6GB; 单事务批量插入预计数十秒 (20–90s), 需要进度提示。导入时套用现有
  `decimate(min_dist=0.8, width_ratio=0.12)` 可再降 30–50% 体积。
- 渲染: 最密页 4–5 万点 vs 现最大 4.8k 点, 页重建/翻页预热可能到 0.3–1s 级,
  会放大"翻到最密页"的卡顿 → 导入时降采样 / 按页点数设上限。
- 编辑安全: 导入笔迹会成为普通笔迹, 橡皮/撤销会误删旧笔记(撤销取 id 最大的一笔,
  导入后若新画一笔则先删自己的, 但空页上按撤销就会删旧笔)。建议 `screens` 加
  `import_source`/`locked` 列, 橡皮/撤销/圈选/删除遇到 locked 页跳过。
- 其它: 导入页不会自动登记 OCR(worker 只登记新落笔); 备份 VACUUM INTO 自动包含、
  体积同步变大; Windows 端需另补导入入口(核心解析可共享)。
- 许可: glaspen2 = MIT, rnote = GPLv3, xournalpp = GPLv2。**不能嵌入/移植其解析代码**;
  JSON/XML 格式本身是接口, 只按文件格式独立实现读取器。

### 方案取舍

- **A(推荐)**: 矢量导入为原生几何页 + 只读/来源标记 + 导入降采样。保真最好, 既有功能全复用。
- B: 先把每页缩放居中"烤"进屏幕几何页(3440×1440), 混入日常本子时间线; 实现最省,
  但旧笔记与日常页混排、导出/页码混杂, 不推荐。
- C: 位图背景导入(rnote-cli / xournalpp 先导 PNG/PDF, glaspen2 加页背景图): 文本/形状/
  PDF 背景全保真, 但需新表 + 渲染 + 导出, 不可编辑, 体积更大; 适合作为非笔迹内容的补充。
- D: 导入无限画布: 保留 rnote infinite 原貌且可缩放, 但全局只有一个无限画布, 会与
  涂鸦混杂, 不适合多本历史笔记。

### 实施路径 (估 4–7 人日)

1. 核心 `glaspen-core` 新模块 `noteimport`(flate2/serde_json 已在依赖, xopp 加 roxmltree):
   parse → 归一化 `Page{w,h,strokes[{rgb, points}]}` → 事务插入;
   `screens` 加 `import_source`/`locked` 列(add_column_if_missing)。
2. 入口: ObjC 菜单"导入历史笔记…"(NSOpenPanel 多选 .rnote/.xopp) → 后台线程 →
   完成通知 + 面板刷新; 单文件失败跳过并汇总。
3. 体验: 导入进度/统计; locked 页的橡皮/撤销/圈选/删除拦截; (可选)本子卡片显示来源名。
4. 验证: 只在 `glaspen2-dev.db` 或 `GLASPEN2_DB_PATH` 指定的副本上导入, 绝不碰真实库;
   回归 `cargo build` / `cargo test` / `fvm flutter analyze`。

### 状态: 未实施, 方案 A 待确认 (讨论日期 2026-11-20)
