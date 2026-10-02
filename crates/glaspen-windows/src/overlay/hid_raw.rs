/// 从 GetRawInputData 原始 buffer 手工解析 HID 报告
/// 布局: [RAWINPUTHEADER 24B][dwSizeHid 4B][dwCount 4B][报告 dwSizeHid*dwCount 字节]
unsafe fn process_raw_hid(buf: &[u64]) -> Option<RECT> {
    let raw = buf.as_ptr() as *const u8;
    let dw_type = u32::from_le_bytes(std::slice::from_raw_parts(raw, 4).try_into().unwrap());
    if dw_type != 2 {
        return None; // 不是 RIM_TYPEHID
    }
    let dw_size_hid = u32::from_le_bytes(
        std::slice::from_raw_parts(raw.add(24), 4)
            .try_into()
            .unwrap(),
    ) as usize;
    let dw_count = u32::from_le_bytes(
        std::slice::from_raw_parts(raw.add(28), 4)
            .try_into()
            .unwrap(),
    ) as usize;
    if dw_size_hid == 0 || dw_count == 0 {
        return None;
    }
    let base = raw.add(32);
    // RAWINPUTHEADER.hDevice 在偏移 8(类型 4B + 大小 4B 之后):标识上报设备,
    // 用于按设备选择归一化量程(驱动更新后物理板/虚拟板并存)
    let hdev = usize::from_le_bytes(
        std::slice::from_raw_parts(raw.add(8), 8)
            .try_into()
            .unwrap(),
    ) as isize;

    let state = &mut *STATE.load(Ordering::SeqCst);
    let ctx = ctx_for(hdev);

    let mut dirty: Option<RECT> = None;
    for i in 0..dw_count {
        let data = std::slice::from_raw_parts(base.add(i * dw_size_hid), dw_size_hid);
        if data.len() < 8 {
            continue;
        }
        let switches = data[1];
        let x = (data[2] as u32) | ((data[3] as u32) << 8);
        let y = (data[4] as u32) | ((data[5] as u32) << 8);
        let press = (data[6] as u32) | ((data[7] as u32) << 8);
        if x as f64 > ctx.x_max * 1.5 || y as f64 > ctx.y_max * 1.5 {
            continue; // 明显超出量程的坏报告
        }
        let sx = (x.min(ctx.x_max as u32) as f64 / ctx.x_max * (state.canvas.w - 1) as f64) as f32;
        let sy = (y.min(ctx.y_max as u32) as f64 / ctx.y_max * (state.canvas.h - 1) as f64) as f32;
        let pnorm = ((press as f64 / ctx.p_max).clamp(0.0, 1.0)) as f32;
        let down = (switches & 0x05) != 0;
        // 压力监控:每帧刷新(视图坐标 = 屏幕像素位置)
        if state.draw.pressure_monitor {
            hud_update_pressure(press as i32, down, sx as i32, sy as i32);
        }
        // 无限画布:视图坐标 → 画布坐标(笔迹存画布系,镜头平移/缩放不影响已写笔画)
        let (cx, cy) = canvas_from_view(sx as f64, sy as f64);
        if let Some(rect) = handle_point(state, cx as f32, cy as f32, pnorm, down) {
            merge_rect(&mut dirty, &rect);
        }
    }
    dirty
}

// ── 窗口过程 ──

