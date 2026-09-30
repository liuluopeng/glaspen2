const WM_INPUT: u32 = 0x00FF;
const TIMER_UNBLOCK: usize = 1;
const TIMER_PEEK: usize = 2;
// 笔事件停止后的延迟:恢复穿透 + 飘渺模式隐藏笔迹
const UNBLOCK_DELAY_MS: u32 = 50;
// 飘渺模式下翻页后短暂显示笔迹的时长(ms)
const PEEK_DELAY_MS: u32 = 2000;
// 近透明底色 alpha=2(0.01*255):肉眼不可见,但窗口可命中/可收指针消息
const BG_BLOCK: u8 = 2;

// caps 不可用时的回退量程(XP-Pen 板面历史值,Generic X/Y logical max)
const FALLBACK_MAX_X: f64 = 25400.0;
const FALLBACK_MAX_Y: f64 = 15875.0;
const FALLBACK_MAX_P: f64 = 16383.0;

// ── 动态量程(修复驱动更新后笔迹偏移):按 hDevice 缓存 caps 读出的逻辑范围 ──
// 驱动更新后系统多出虚拟数位板(VID_00FF/BACC),笔事件改道到它那里,
// 坐标系从 XP-Pen 的 25400x15875 变成全屏归一化的 32767x32767,
// 写死量程导致笔迹越往右下偏得越多(WM_POINTER 不受影响)。
// 修复:按 hDevice 缓存 preparsed data,从 HID value caps 动态读
// GX/GY/压力的 LogicalMax 归一化;物理板/虚拟板各用各的量程。
// 已在 wAPItry/raw_input_trans_draw_dbg 验证:与 WM_POINTER 对照偏差 ≤7px。

struct DevCtx {
    x_max: f64,
    y_max: f64,
    p_max: f64,
}

static CTX: AtomicPtr<HashMap<isize, DevCtx>> = AtomicPtr::new(std::ptr::null_mut());

fn ctx_map() -> &'static mut HashMap<isize, DevCtx> {
    unsafe {
        let p = CTX.load(Ordering::SeqCst);
        if p.is_null() {
            let b = Box::into_raw(Box::new(HashMap::new()));
            let _ =
                CTX.compare_exchange(std::ptr::null_mut(), b, Ordering::SeqCst, Ordering::SeqCst);
        }
        &mut *CTX.load(Ordering::SeqCst)
    }
}

fn device_name(h: HANDLE) -> Option<String> {
    unsafe {
        let mut len: u32 = 0;
        let _ = GetRawInputDeviceInfoW(Some(h), RIDI_DEVICENAME, None, &mut len);
        if len == 0 {
            return None;
        }
        let mut buf = vec![0u16; len as usize];
        let n = GetRawInputDeviceInfoW(
            Some(h),
            RIDI_DEVICENAME,
            Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
            &mut len,
        );
        if n == u32::MAX || n == 0 {
            return None;
        }
        buf.truncate(n as usize);
        Some(String::from_utf16_lossy(&buf))
    }
}

/// 获取设备 preparsed data:RIDI_PREPARSEDDATA → 设备路径+CreateFile+HidD → 直接 HidD
fn get_preparsed(h: HANDLE) -> Option<PHIDP_PREPARSED_DATA> {
    unsafe {
        let mut prep = PHIDP_PREPARSED_DATA(0);
        let mut size = std::mem::size_of::<PHIDP_PREPARSED_DATA>() as u32;
        let n = GetRawInputDeviceInfoW(
            Some(h),
            RIDI_PREPARSEDDATA,
            Some(&mut prep as *mut PHIDP_PREPARSED_DATA as *mut core::ffi::c_void),
            &mut size,
        );
        if n != u32::MAX && n != 0 && prep.0 != 0 {
            return Some(prep);
        }

        if let Some(name) = device_name(h) {
            let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            let path = PCWSTR(wide.as_ptr());
            let access_modes = [(GENERIC_READ | GENERIC_WRITE).0, (GENERIC_READ).0, 0];
            for access in access_modes {
                if let Ok(handle) = CreateFileW(
                    path,
                    access,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    None,
                    OPEN_EXISTING,
                    FILE_FLAG_OVERLAPPED,
                    None,
                ) {
                    let mut pp = PHIDP_PREPARSED_DATA(0);
                    if HidD_GetPreparsedData(handle, &mut pp) && pp.0 != 0 {
                        return Some(pp);
                    }
                    let _ = CloseHandle(handle);
                }
            }
        }

        let mut pp = PHIDP_PREPARSED_DATA(0);
        if HidD_GetPreparsedData(h, &mut pp) && pp.0 != 0 {
            return Some(pp);
        }
        None
    }
}

/// 从 value caps 提取 GX/GY/压力的逻辑上限
fn load_caps_ranges(ptr: PHIDP_PREPARSED_DATA) -> (f64, f64, f64) {
    unsafe {
        let mut x_max = FALLBACK_MAX_X;
        let mut y_max = FALLBACK_MAX_Y;
        let mut p_max = FALLBACK_MAX_P;

        let mut n: u16 = 0;
        let _ = HidP_GetValueCaps(HidP_Input, std::ptr::null_mut(), &mut n, ptr);
        if n > 0 {
            let mut vc = vec![HIDP_VALUE_CAPS::default(); n as usize];
            let _ = HidP_GetValueCaps(HidP_Input, vc.as_mut_ptr(), &mut n, ptr);
            for c in vc {
                let usage = c.Anonymous.NotRange.Usage;
                if c.UsagePage == 0x01 && usage == 0x30 && c.LogicalMax > 1000 {
                    x_max = c.LogicalMax as f64;
                } else if c.UsagePage == 0x01 && usage == 0x31 && c.LogicalMax > 1000 {
                    y_max = c.LogicalMax as f64;
                } else if c.UsagePage == 0x0D && usage == 0x30 && c.LogicalMax > 255 {
                    p_max = c.LogicalMax as f64;
                }
            }
        }
        (x_max, y_max, p_max)
    }
}

/// 首次见到该设备时构建量程上下文(并打印一行量程来源)
fn ctx_for(hdev: isize) -> &'static mut DevCtx {
    let map = ctx_map();
    if !map.contains_key(&hdev) {
        let h = HANDLE(hdev as *mut core::ffi::c_void);
        let (x_max, y_max, p_max, from_caps) = match get_preparsed(h) {
            Some(ptr) => {
                let (x, y, p) = load_caps_ranges(ptr);
                (x, y, p, true)
            }
            None => (FALLBACK_MAX_X, FALLBACK_MAX_Y, FALLBACK_MAX_P, false),
        };
        let src = if from_caps {
            "caps"
        } else {
            "⚠ preparsed 不可用,回退常量"
        };
        eprintln!(
            "[overlay] [新输入设备 hDev=0x{:X}] 量程({}): X 0..{:.0} Y 0..{:.0} P 0..{:.0}",
            hdev, src, x_max, y_max, p_max
        );
        map.insert(
            hdev,
            DevCtx {
                x_max,
                y_max,
                p_max,
            },
        );
    }
    map.get_mut(&hdev).unwrap()
}

