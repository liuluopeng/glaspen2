# Windows 笔键隔离修复建议

## 当前问题
`src/windows/input.rs` 中的 `WH_MOUSE_LL` 钩子 + `WS_EX_TRANSPARENT` 无法可靠区分笔和鼠标：
- `is_pen_event()` 靠 `dwExtraInfo > 0x100` 判断笔，Huion 驱动可能不设这个值
- `WS_EX_TRANSPARENT` 让窗口自身收不到任何鼠标消息，完全依赖钩子
- 键盘事件完全没过滤

## 推荐方案：不用 `WS_EX_TRANSPARENT`，改用 `WM_NCHITTEST`

### 1. 修改 `overlay.rs` — 去掉 `WS_EX_TRANSPARENT`

```rust
// 改前：
let ex_style = WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW;

// 改后：去掉 TRANSPARENT
let ex_style = WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW;
```

### 2. 修改 `overlay.rs` — wnd_proc 加 `WM_NCHITTEST` 处理

```rust
unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_NCHITTEST => {
            let extra = unsafe { GetMessageExtraInfo() }.0 as usize;
            // 鼠标事件（extra==0）-> 穿透；笔事件 -> 留在窗口
            if extra == 0 {
                return LRESULT(HTTRANSPARENT as isize);
            }
            // 笔事件：正常处理
            return LRESULT(HTCLIENT as isize);
        }
        // ... 其他消息
    }
}
```

需要 `use windows::Win32::UI::WindowsAndMessaging::GetMessageExtraInfo;` 和 `HTTRANSPARENT`.

### 3. 简化 `input.rs` — 窗口处理笔事件，钩子只用于隐藏光标

核心思路：
- 窗口自己收笔事件 → 在 `wnd_proc` 里处理 `WM_LBUTTONDOWN` 等
- 钩子只用于：隐藏系统光标、追踪鼠标移动、反色模式取色
- 笔事件在窗口层被消费，鼠标事件被 `WM_NCHITTEST` 透传

### 4. 如果还是无法识别笔

可以用 `RegisterRawInputDevices` + `WM_INPUT` 直接接数位板的原始数据，绕过 `dwExtraInfo` 判断。这是最可靠的方式：

```rust
use windows::Win32::UI::Input::*;

let rid = RAWINPUTDEVICE {
    usUsagePage: 0x000D,           // HID digitizer
    usUsage: 0x0002,               // Pen
    dwFlags: RIDEV_INPUTSINK,
    hwndTarget: hwnd,
};
RegisterRawInputDevices(&[rid]);
```

然后在 `wnd_proc` 处理 `WM_INPUT` 消息，从 `RAWINPUT` 结构解析笔的压力、位置。

## 架构对比

| | macOS | Windows（推荐） |
|---|---|---|
| 窗口穿透 | `ignoresMouseEvents` | `WM_NCHITTEST → HTTRANSPARENT` |
| 笔输入 | `CGEventTap` 全局 hook | `WM_NCHITTEST` 区分 + 窗口收笔事件 |
| 笔识别 | NSEvent type 自动区分 | `GetMessageExtraInfo()` 或 `WM_INPUT` |
| 光标隐藏 | `CGDisplayHideCursor` | `ShowCursor(FALSE)` |

## 验证步骤

1. 去代码后在 `WM_NCHITTEST` 中加 `println!("extra_info: {:#x}", extra)`，看笔接触和鼠标分别输出什么值
2. 确认笔事件 `extra != 0` 后，去掉 TRANSPARENT，让窗口正常收消息
3. 确认鼠标点击能穿透到下层应用
