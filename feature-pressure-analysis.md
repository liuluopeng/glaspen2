# 笔压分析

## 当前数据流

```
硬件 → HID/WM_POINTER → _lastPointerPressure (0-1024, 整数)
                              ↓
OnPenDown/OnPenMove/OnPenUp → pressure = _lastPointerPressure / 1024.0
                              ↓
Rust modeler → pressure_to_width(p, widthScale) → (0.3 + p² × 7.7) × widthScale
                              ↓
DrawModelerBuffer → 用 modeler 输出的 width 画线
```

## 发现的问题

### 1. 压力在整笔中是静态的（最严重）
`_lastPointerPressure` 只在 WM_POINTER 消息中设置，**OnPenMove 不更新**。
→ modeler 每个 move 事件收到相同压力 → 笔迹粗细无变化。

### 2. HID 路径不喂压力给 modeler
`ProcessHidInput` 读到了 HID 压力，调了 `SetPressure()` 给 C# 宽度，
但**没更新 `_lastPointerPressure`** → modeler 收不到。

### 3. Raw Input 路径无压力
`ProcessMouseInput` 处理绝对坐标鼠标事件，没有压力数据。

### 4. smooth_points 硬编码 pressure=0.5
加载历史笔迹时，`modeler::smooth_points()` 所有点用固定 0.5 压力。
→ 历史笔迹丢失粗细变化。

## 修复方案

| 问题 | 修复 |
|------|------|
| 压力静态 | OnPenMove 从 WM_POINTER 读实时压力 |
| HID 不喂 modeler | HID 路径同时更新 `_lastPointerPressure` |
| smooth_points | 保留原始 pressure 数据，传入 modeler |

## 压力映射公式

当前: `(0.3 + p² × 7.7) × widthScale`

| 压力 (0-1) | 宽度倍数 |
|------------|----------|
| 0.0        | 0.30x    |
| 0.25       | 0.78x    |
| 0.5        | 2.23x    |
| 0.75       | 4.63x    |
| 1.0        | 8.00x    |

这个映射是平方关系，轻笔触变化小，重笔触变化大。合理。
