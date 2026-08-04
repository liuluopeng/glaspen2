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
