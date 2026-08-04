# Windows 笔输入架构 — 讨论备忘 (2026-08)

> 本文档在 macOS 侧整理, 供 Windows 侧开发时参考。核心结论:
> 输入源选择 (Wintab/WM_POINTER/HID) 与"是否穿透"是**两回事**,
> 穿透由驱动合成的鼠标事件造成, 必须单独拦截。

## 1. 三层模型

```
层1 输入数据源    Wintab 数据包 / WM_POINTER 消息 / RawInput HID 报告
                 → 只提供坐标/压力, 与"是否穿透"无关
层2 合成鼠标通道  笔按下时驱动额外发"假鼠标事件" (WM_LBUTTONDOWN 等)
                 → 这是穿透/选中下层文本的唯一元凶
层3 拦截手段      窗口命中拦截 (现状) / WH_MOUSE_LL 钩子 / 忽略合成鼠标
```

## 2. 历史尝试 (feat-async-ocr 分支) — 方案 B 已实测否决

`glaspen2_csharp/_bak/PenInterceptor.cs`: WH_MOUSE_LL 钩子 +
`GetMessageExtraInfo` 检查 `PEN_SIGNATURE = 0xFF515700` + 80ms HID 时序兜底。

**失败原因 (均有文档)**
- `.mimocode/plans/...jolly-island.md`:
  "The user's pen sets `dwExtraInfo=0x0` on legacy mouse messages.
  WH_MOUSE_LL CANNOT distinguish pen from mouse."
  → 本机笔驱动**不写签名**, 签名识别失效
- `docs/win-pen-pressure-diagnosis.md`: 时序兜底有竞态
  (hook 线程检查 HidTipDown 时 UI 线程尚未更新)

**当时的转向结论**: 只有 WM_POINTER 能显式识别笔, 但它要求
**非分层、可命中的窗口** → 演变成现在的两层窗口架构
(OverlayForm 近透明拦截层 + FakeStrokeForm 可见复刻层)。

## 3. 输入源对比

| 能力 | Wintab (wintab32.dll) | WM_POINTER (Win8+) | RawInput HID (现用) |
|---|---|---|---|
| 悬停移动 (准星) | ✅ | ✅ (运动即事件) | ✅ |
| 静止进出范围 (proximity) | ✅ pkProximity | ❌ 只能超时 | ✅ in-range 位 |
| 压力/倾斜/橡皮擦 | ✅ 含旋转 | ✅ GetPointerPenInfo | ✅ 自解析 |
| 维护方 | Wacom (规范 1.0→1.4) | 微软 | 无中间层 |
| 设备覆盖 | 专业板 ✅ / Surface ❌ | 最广 | 取决于驱动 |

结论: **HID 主路径不动** (信息最全, 含悬空); WM_POINTER 可作回退
(悬停可用, 离开需超时); Wintab 不必碰。

## 4. 防穿透三方案

| 方案 | 做法 | 效果 |
|---|---|---|
| a. 窗口命中拦截 (现状) | 近透明层 + WS_EX_TRANSPARENT AutoBlock | 笔被挡, 真鼠标也被挡, 双窗口 |
| b. 钩子掐合成鼠标 | WH_MOUSE_LL + 笔标志 → 丢弃 | 窗口可完全透明点穿, 下层永不被点; **本机签名=0x0, 需替代标志** |
| c. 可命中窗口 + 忽略合成鼠标 | 收 WM_POINTER 与合成鼠标, 忽略后者 (GTK4 做法) | 窗口必须可命中 (仍挡真鼠标) |

优雅化演进路线 (均未实施):
1. **单 ULW 窗口**: 笔迹真实 alpha; 笔在范围时铺 alpha=1 整屏底色拦截,
   离开清掉 → 删掉 OverlayForm/双份渲染/WS_EX_TRANSPARENT 切换
2. **钩子方案** (若找到替代标志): 单可见窗口 + 点穿, 最干净
3. **双窗口清理**: OverlayForm Opacity 0.01→0 + 删掉其不可见画布渲染

## 5. 待办: 替代标志探针 (Windows 上跑, 不动 C# 代码)

在 Windows 上临时写一个独立的小探针 (WH_MOUSE_LL 钩子), 记录每个鼠标事件的
`dwExtraInfo` 完整值 (hex) + `MSLLHOOKSTRUCT.flags` 的 `LLMHF_INJECTED` 位,
对比"真鼠标移动"与"笔悬停/落笔"的日志差异:
- 若笔事件带 INJECTED 位或写了某个非 0 的 extra info → 方案 b 有了可用标志
- 否则方案 b 对本机不可行, 维持方案 a / 单窗口演进 (方案 1)

探针要点: 编译用 `csc /out:PenEventProbe.exe PenEventProbe.cs` (单文件控制台,
不进仓库), 所有事件 pass through 不拦截, 秒级去重打日志。
