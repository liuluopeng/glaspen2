# Windows 笔压感问题诊断与解决方案

## 问题描述

- **Windows Ink 开启时**：笔能穿透 overlay 到下层窗口，但有压感
- **Windows Ink 关闭时**：笔不穿透（好），但没有压感（坏）

## 架构分析

当前 C# 代码有三条获取笔输入的路径：

### 路径 1: WM_POINTER（Windows Ink 专属）

```
笔 → Windows Ink → WM_POINTERDOWN/UPDATE/UP → GetPointerPenInfo() → pressure
```

- Windows Ink 关闭时不触发
- 坐标和压感都来自系统

### 路径 2: WH_MOUSE_LL Hook（PenInterceptor）

```
笔 → WM_LBUTTONDOWN/WM_MOUSEMOVE → HookCallback()
  → GetMessageExtraInfo 检查 PEN_SIGNATURE
  → 有签名: suppress + 触发 PenDown/PenMove/PenUp
  → 无签名: 依赖 HidTipDown 时序检测
```

- Windows Ink 开启时：笔事件带 PEN_SIGNATURE，可靠识别
- Windows Ink 关闭时：**没有 PEN_SIGNATURE**，退化为时序检测

### 路径 3: Raw Input HID（最底层）

```
笔 → USB HID 报告 → RegisterRawInputDevices → WM_INPUT → ProcessHidInput()
  → 解析 [switches, x, y, pressure] → _lastPointerPressure
```

- Windows Ink 开/关都有数据
- 但只有 overlay 窗口收到 WM_INPUT 时才触发
- 如果 hook 没拦截住鼠标事件，事件去了下层窗口，overlay 收不到 WM_INPUT

## 根本问题

### 问题 1: 竞态条件

Hook 线程和 UI 线程不同步：

```csharp
// PenInterceptor.cs — Hook 线程 (WH_MOUSE_LL 回调)
bool isFromPen = msSincePen < 80ms || OverlayForm.HidTipDown;
//                                          ↑ 可能还没被 UI 线程设置

// OverlayForm.cs — UI 线程 (WndProc 回调)
HidTipDown = tipDown; // 设晚了，hook 已经放行了事件
```

时序：
```
T=0   HID 报告到达（UI 线程队列）
T=0   WM_LBUTTONDOWN 到达（hook 线程立即处理）
T=0   hook 检查 HidTipDown → false → 放行 ❌
T=1ms UI 线程处理 HID → HidTipDown = true（太晚了）
```

### 问题 2: WS_EX_TRANSPARENT 未使用

当前用 hook 来拦截笔事件，但 hook 的时序检测不可靠。Windows 有现成的窗口属性 `WS_EX_TRANSPARENT` 可以让鼠标事件自动穿透，不需要 hook。

### 问题 3: HID 坐标依赖

HID 的 x/y 是逻辑坐标（0-65535），需要映射到屏幕坐标。当前映射用 `VirtualScreen`，在多显示器下可能有偏移。

## 建议方案

### 方案 A: 纯 HID + 窗口穿透（推荐）

彻底去掉 hook 的笔检测逻辑，改为：

```
1. overlay 窗口设为 WS_EX_TRANSPARENT（鼠标事件自动穿透）
2. 只从 Raw Input HID 获取笔数据（坐标 + 压感 + tipDown）
3. HID 的 tipDown 直接触发 OnPenDown/OnPenMove/OnPenUp
4. 不依赖 hook 线程和 UI 线程的同步
```

优点：
- Windows Ink 开/关都一样工作
- 没有竞态条件
- 压感始终可用（HID 始终有数据）
- 代码更简单

关键代码改动：

```csharp
// 1. 窗口创建时设置穿透
protected override CreateParams CreateParams {
    get {
        var cp = base.CreateParams;
        cp.ExStyle |= 0x00000020; // WS_EX_TRANSPARENT
        return cp;
    }
}

// 2. HID 回调直接驱动绘图（已有，确保不被跳过）
private void ProcessHidInput(...) {
    // 解析 x, y, pressure, tipDown（已有）
    if (tipDown && pressure > 0) {
        SetPressure(pressure);
        if (!_isDrawing) OnPenDown(sx, sy);
        else OnPenMove(_lastPoint.X, _lastPoint.Y, sx, sy);
    } else if (!tipDown && _isDrawing) {
        OnPenUp(sx, sy);
    }
}

// 3. PenInterceptor 只处理真正的鼠标事件
// 不再需要笔检测逻辑，只保留快捷键拦截
```

### 方案 B: 修复现有 hook 时序

如果不想大改架构，可以：

```csharp
// PenInterceptor 中改用 volatile 字段 + 自旋等待
private volatile bool _hidTipDown;

// hook 回调中等待 HID 状态更新（最多等 2ms）
if (!isFromPen && extraVal == 0) {
    for (int i = 0; i < 20; i++) {
        if (OverlayForm.HidTipDown) { isFromPen = true; break; }
        Thread.Sleep(0); // yield, ~100μs
    }
}
```

缺点：增加了延迟，不够优雅。

## 需要验证的事

1. `WS_EX_TRANSPARENT` 窗口是否仍能收到 `WM_INPUT` 消息
2. HID 坐标在多显示器下的映射是否正确
3. 没有 hook 后，快捷键（Cmd+C 等）是否还能正常工作

## HID 数据格式定义

### Raw Input 注册

```csharp
// OverlayForm.cs — RegisterRawInput()
RAWINPUTDEVICE[] devices = {
    { usUsagePage=0x0001, usUsage=0x0002, dwFlags=RIDEV_INPUTSINK },  // Mouse
    { usUsagePage=0x000D, usUsage=0x0002, dwFlags=RIDEV_INPUTSINK },  // Pen (Digitizer)
    { usUsagePage=0x000D, usUsage=0x0001, dwFlags=RIDEV_INPUTSINK },  // Stylus (Digitizer)
};
```

| Usage Page | Usage | 含义 |
|---|---|---|
| `0x0001` | `0x0002` | Generic Desktop → Mouse |
| `0x000D` | `0x0001` | Digitizer → Stylus |
| `0x000D` | `0x0002` | Digitizer → Pen |
| `0x000D` | `0x0004` | Digitizer → Touch Screen（未注册） |

### HID 报告结构（标准数位板）

当前代码解析格式（`ProcessHidInput`）：

```
字节偏移    大小    字段          说明
──────────────────────────────────────────────
 0          1      reportId      HID 报告 ID（通常 0x01）
 1          1      switches      开关状态位
                                  bit 0: tipDown（笔尖触碰）
                                  bit 1: barrelButton（侧按钮）
                                  bit 2: eraser（橡皮擦）
                                  bit 3: invert（笔倒置）
                                  bit 4: inRange（在感应范围内）
 2-3        2      X             X 坐标，Little-Endian 16-bit
                                  逻辑范围: 0-65535
                                  映射: screenX = virtualScreen.left + (x * screenWidth / 65536)
 4-5        2      Y             Y 坐标，Little-Endian 16-bit
                                  逻辑范围: 0-65535
                                  映射: screenY = virtualScreen.top + (y * screenHeight / 65536)
 6-7        2      pressure      压感，Little-Endian 16-bit
                                  范围: 0-1024（Wacom 标准）
                                  转换: float_p = pressure / 1024.0
```

### switches 字段位定义

```
bit 4    bit 3    bit 2    bit 1    bit 0
inRange  invert   eraser   barrel   tipDown
```

| 位 | 名称 | 含义 |
|---|---|---|
| bit 0 | tipDown | 笔尖接触数位板 = 1，抬起 = 0 |
| bit 1 | barrelButton | 笔身侧按钮按下 = 1 |
| bit 2 | eraser | 当前使用橡皮擦端 = 1 |
| bit 3 | invert | 笔倒置（非所有设备支持） |
| bit 4 | inRange | 笔在数位板感应范围内 = 1 |

### 解析代码

```csharp
// 当前实现（OverlayForm.cs:494-558）
int baseOff = offset + 8; // 跳过 RAWINPUTHEADER (dwSizeHid=4 + dwCount=4)
byte reportId = Marshal.ReadByte(buffer, baseOff);
byte switches = Marshal.ReadByte(buffer, baseOff + 1);
uint x        = Marshal.ReadByte(buffer, baseOff + 2) | (Marshal.ReadByte(buffer, baseOff + 3) << 8);
uint y        = Marshal.ReadByte(buffer, baseOff + 4) | (Marshal.ReadByte(buffer, baseOff + 5) << 8);
uint pressure = Marshal.ReadByte(buffer, baseOff + 6) | (Marshal.ReadByte(buffer, baseOff + 7) << 8);

bool tipDown     = (switches & 0x01) != 0;
bool barrelBtn   = (switches & 0x02) != 0;
bool isEraser    = (switches & 0x04) != 0;
bool inRange     = (switches & 0x10) != 0;
```

### 完整 HID 报告可能包含的额外字段

标准数位板 HID 报告可能超过 8 字节，常见扩展：

```
 8-9        2      tiltX         X 轴倾斜（有符号 16-bit，-9000 到 +9000，单位 0.01 度）
10-11       2      tiltY         Y 轴倾斜（同上）
12-13       2      twist         笔旋转（0-35999，单位 0.01 度）
14-15       2      barrelPressure 侧按钮压力（部分设备）
```

当前代码只解析前 8 字节（reportId + switches + x + y + pressure）。如果需要倾斜或旋转，需要扩展解析逻辑并检查 `dataLen`。

### WM_POINTER 中的压力格式

WM_POINTER 路径的压力来自 `GetPointerPenInfo()`：

```csharp
POINTER_PEN_INFO penInfo;
GetPointerPenInfo(pointerId, ref penInfo);
uint pressure = penInfo.pressure; // 0-1024，与 HID 一致
```

范围相同（0-1024），两条路径的压力值可以直接比较。

## 参考

- [WM_POINTER 文档](https://learn.microsoft.com/en-us/windows/win32/inputmsg/wm-pointerdown)
- [Raw Input 文档](https://learn.microsoft.com/en-us/windows/win32/inputdev/raw-input)
- [WS_EX_TRANSPARENT](https://learn.microsoft.com/en-us/windows/win32/winmsg/extended-window-styles)
