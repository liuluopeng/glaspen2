/// 线宽半径(线性压力映射 × 线宽倍率):(0.75 + p*1.75) * scale(直径 1.5..5px × scale)
fn width_r(p: f32, scale: f32) -> f32 {
    // 半径版包装:单源直径公式在 core(macOS 同一二次曲线 —— 轻压极细、
    // 重压粗,27 倍动态范围)。本地线性式 (0.75+p*1.75)*scale 与它漂移:
    // 下限粗 2.5 倍、范围仅 3.3 倍,轻压看起来像一根不变的最细线。
    (glaspen_core::modeler::pressure_to_width(p as f64, scale.max(0.05) as f64) * 0.5) as f32
}

/// 由中心线点列(x, y, 半径)构建可变宽度轮廓:
/// 每点沿法线 ±偏移半径,返回闭合轮廓点列(左边界正向 + 右边界反向)。
/// 法线用中点差分(前后点方向),保证相邻带在共享点处偏移一致,无缝衔接。
fn build_outline(pts: &[(f32, f32, f32)]) -> Vec<(f32, f32)> {
    let n = pts.len();
    if n < 2 {
        return vec![];
    }
    let mut left = Vec::with_capacity(n);
    let mut right = Vec::with_capacity(n);
    for i in 0..n {
        let (x, y, r) = pts[i];
        let (dx, dy) = if i == 0 {
            (pts[1].0 - x, pts[1].1 - y)
        } else if i == n - 1 {
            (x - pts[i - 1].0, y - pts[i - 1].1)
        } else {
            (pts[i + 1].0 - pts[i - 1].0, pts[i + 1].1 - pts[i - 1].1)
        };
        let l = (dx * dx + dy * dy).sqrt();
        let (nx, ny) = if l > 1e-6 {
            (-dy / l, dx / l)
        } else {
            (1.0, 0.0)
        };
        left.push((x + nx * r, y + ny * r));
        right.push((x - nx * r, y - ny * r));
    }
    let mut outline = left;
    outline.extend(right.iter().rev());
    outline
}

/// 合并脏矩形
fn merge_rect(dirty: &mut Option<RECT>, r: &RECT) {
    match dirty {
        None => *dirty = Some(*r),
        Some(d) => {
            d.left = d.left.min(r.left);
            d.top = d.top.min(r.top);
            d.right = d.right.max(r.right);
            d.bottom = d.bottom.max(r.bottom);
        }
    }
}

/// 用给定点列填充整笔轮廓 + 端点圆帽(带可选描边),返回脏矩形
fn fill_stroke_path(canvas: &mut OverlayCanvas, path: &[(f32, f32, f32)], ol: f32) -> Option<RECT> {
    if path.len() < 2 {
        return None;
    }
    let mut dirty: Option<RECT> = None;

    // 描边层: 黑白相间 1px 虚线(marching ants, macOS 同款)。
    // 相位沿整笔累计弧长连续 —— 任何背景上恒有一半虚线可见。
    if ol > 0.0 {
        let mut cum = 0.0f64;
        for w in path.windows(2) {
            let (x0, y0, r0) = w[0];
            let (x1, y1, r1) = w[1];
            // 描边直径 = 墨迹直径 + 2·ol(取相邻两点较粗者, 避免细段露墨)
            let segw = (r0.max(r1) + ol) * 2.0;
            let rect = canvas.stroke_outline_seg(x0, y0, x1, y1, segw as f64, cum);
            merge_rect(&mut dirty, &rect);
            cum += (((x1 - x0) * (x1 - x0) + (y1 - y0) * (y1 - y0)) as f64).sqrt();
        }
    }

    // 主体
    let outline = build_outline(path);
    if outline.len() >= 3 {
        let rect = canvas.fill_outline(&outline);
        merge_rect(&mut dirty, &rect);
    }
    if let Some(&(cx, cy, r)) = path.first() {
        let rect = canvas.fill_dot(cx, cy, r);
        merge_rect(&mut dirty, &rect);
    }
    if let Some(&(cx, cy, r)) = path.last() {
        let rect = canvas.fill_dot(cx, cy, r);
        merge_rect(&mut dirty, &rect);
    }
    dirty
}

/// 整笔轮廓重填 + 端点圆帽,返回脏矩形(仅新增段区域)
fn redraw_pen(state: &mut OverlayState, new_pts: &[(f32, f32, f32)]) -> Option<RECT> {
    let ol = if state.draw.outline_enabled { 1.0 } else { 0.0 };

    if state.pen_path.is_empty() && !new_pts.is_empty() {
        let (cx, cy, r) = new_pts[0];
        let _ = state.canvas.fill_dot(cx, cy, r);
    }
    let new_start = state.pen_path.len();
    state.pen_path.extend_from_slice(new_pts);
    let new_end = state.pen_path.len();

    // 整笔轮廓填充(单轮廓,非零环绕)+ 可选描边层
    fill_stroke_path(&mut state.canvas, &state.pen_path, ol);

    // dirty 只保留新增段区域(旧区域内容未变)
    let mut new_dirty: Option<RECT> = None;
    for &(px, py, r) in &state.pen_path[new_start.saturating_sub(1)..new_end] {
        let rr = r.ceil() as i32 + 1 + ol as i32;
        let rect = RECT {
            left: (px as i32 - rr).max(0),
            top: (py as i32 - rr).max(0),
            right: (px as i32 + rr + 1).min(state.canvas.w),
            bottom: (py as i32 + rr + 1).min(state.canvas.h),
        };
        merge_rect(&mut new_dirty, &rect);
    }
    new_dirty
}

/// 处理一个采样点:喂给 ink-stroke-modeler,输出平滑点列后轮廓填充。
/// x/y 为画布坐标(无限画布下 = 视图坐标经镜头逆变换);笔迹以画布坐标
/// 入库,绘制时再经镜头变换回视图。
fn handle_point(state: &mut OverlayState, x: f32, y: f32, p: f32, down: bool) -> Option<RECT> {
    // 模型器要求首事件 Down,后续 Move,抬起 Up
    let event_type = if !down {
        ModelerInputEventType::Up
    } else if state.in_stroke {
        ModelerInputEventType::Move
    } else {
        ModelerInputEventType::Down
    };
    let input = ModelerInput {
        event_type,
        pos: (x as f64, y as f64),
        time: state.start_time.elapsed().as_secs_f64(),
        pressure: p as f64,
    };
    let results = match state.stroke_modeler.update(input) {
        Ok(r) => r,
        Err(_) => return None, // Duplicate/负时间等,忽略
    };

    // 落笔时确定笔迹颜色
    if down && !state.in_stroke {
        state.canvas.color = (
            (state.draw.pen_r * 255.0) as u8,
            (state.draw.pen_g * 255.0) as u8,
            (state.draw.pen_b * 255.0) as u8,
        );
    }

    // 记录笔画到 STROKES/DB(用于撤销、导出、XOJ 保存)
    if down && !state.in_stroke {
        glaspen_core::export::glaspen2_begin_stroke(
            state.draw.pen_r,
            state.draw.pen_g,
            state.draw.pen_b,
            state.draw.width_scale,
        );
    }
    if !results.is_empty() {
        let scale = state.draw.width_scale as f32;
        for r in &results {
            // 相对时间必须带上:GIF 回放时间线按它展开,t=0 会让导出永远为空
            glaspen_core::export::glaspen2_add_point_t(
                r.pos.0,
                r.pos.1,
                (width_r(r.pressure as f32, scale) * 2.0) as f64,
                r.time,
            );
        }
    }
    let z = if infinite_on() { cam().2 as f32 } else { 1.0 };
    // 模型器输出是画布坐标;绘制前经镜头变换回视图,线宽同步缩放
    let pts: Vec<(f32, f32, f32)> = results
        .iter()
        .map(|r| {
            let (vx, vy) = view_from_canvas(r.pos.0, r.pos.1);
            (
                vx as f32,
                vy as f32,
                (width_r(r.pressure as f32, state.draw.width_scale as f32) * z).max(0.5),
            )
        })
        .collect();

    if !down {
        // 抬起:补最后一段轮廓 + 终点圆帽,清空并重置模型器
        state.in_stroke = false;
        let dirty = redraw_pen(state, &pts);
        glaspen_core::export::glaspen2_end_stroke();
        // 抬笔扇出:草稿推送/共享上行/OCR 登记(macOS 在 modeler_commit 里做)
        glaspen_core::export::glaspen2_notify_stroke_committed();
        state.pen_path.clear();
        let params = modeler_params();
        let _ = state.stroke_modeler.reset_w_params(params);
        state.start_time = Instant::now();
        return dirty;
    }

    state.in_stroke = true;
    redraw_pen(state, &pts)
}

fn modeler_params() -> ModelerParams {
    ModelerParams {
        sampling_min_output_rate: 120.0,
        sampling_end_of_stroke_stopping_distance: 0.01,
        sampling_end_of_stroke_max_iterations: 20,
        sampling_max_outputs_per_call: 200,
        stylus_state_modeler_max_input_samples: 20,
        ..ModelerParams::suggested()
    }
}

