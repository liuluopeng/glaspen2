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

/// 整笔轮廓重填 + 端点圆帽,返回脏矩形(仅新增段区域)。
///
/// D9c:pass 顺序/三档阴影/描边宽度/虚线相位/颜色全部由 core DrawPlan 决定
/// (见 crate::paint_plan),本函数只负责两件 adapter 的事:
/// 1. 把整条在写路径喂给计划 —— 与旧实现一致,每次报告重填整条路径
///    (在写笔迹尚未进 STROKES,计划里只有 `live` 一笔);
/// 2. 保留旧增量脏区语义 —— 公式仍是 core `PlanScope::WindowsLive`,adapter
///    只决定「传新增点 + 前一个衔接点」这一小段,不重刷旧区域。
///
/// 初始点圆帽(首份报告只有 1 点时旧实现的 fill_dot)现在由计划的 Fill 命令
/// 统一画出:阴影/描边命令在 < 2 点时由执行层跳过,Fill 画首尾圆点。
fn redraw_pen(state: &mut OverlayState, new_pts: &[(f32, f32, f32)]) -> Option<RECT> {
    let new_start = state.pen_path.len();
    state.pen_path.extend_from_slice(new_pts);
    let new_end = state.pen_path.len();

    let shadow = state.draw.soft_shadow;
    let outline = state.draw.outline_enabled;
    // 旧 canvas.color 的笔色来源(实时路径尚未提交,颜色不在 Stroke 里)。
    let color = (state.draw.pen_r, state.draw.pen_g, state.draw.pen_b);
    // pen_path 是「视图坐标 + 半径」;plan 要「视图坐标 + 直径」,恒等视图映射。
    let points = paint_plan::view_points(&state.pen_path);
    match paint_plan::live_plan(
        &points,
        color,
        state.canvas.w,
        state.canvas.h,
        shadow,
        outline,
    ) {
        Ok(plan) => state.canvas.execute_plan(&plan),
        Err(e) => tracing::warn!("实时笔迹计划生成失败: {e}"),
    }

    // dirty 只保留新增段区域(旧区域内容未变)。第 3 参数起 = 新增点 + 前一个
    // 衔接点,与旧实现 `[new_start.saturating_sub(1)..new_end]` 完全同段。
    let from = new_start.saturating_sub(1);
    paint_plan::live_dirty(
        &points[from..new_end],
        state.canvas.w,
        state.canvas.h,
        shadow,
        outline,
    )
    .map(|d| RECT {
        left: d.left,
        top: d.top,
        right: d.right,
        bottom: d.bottom,
    })
}

/// 处理一个采样点:喂 core 模型器会话(D9b 单源),输出平滑点列后轮廓填充。
/// x/y 为画布坐标(无限画布下 = 视图坐标经镜头逆变换);笔迹以画布坐标
/// 入库,绘制时再经镜头变换回视图。
///
/// 事件语义与旧自研模型器版本逐分支一致:
/// - 落笔报告且未落笔 = 新笔(begin,会话 Down 恒 1 点);
/// - 落笔报告且书写中 = Move;
/// - 非落笔报告且书写中 = Up(收笔);
/// - 非落笔报告且未落笔 = 悬空,忽略(旧实现喂 Up 被模型器拒绝,等价)。
fn handle_point(state: &mut OverlayState, x: f32, y: f32, p: f32, down: bool) -> Option<RECT> {
    // 气泡消息冷启动分段: 每个 HID 报告(含悬空)都算笔活动心跳, 悬停构思
    // 不拆消息; Windows 无显式"笔离开"信号, 冷启动由"笔静默超阈值"覆盖。
    glaspen_core::export::glaspen2_bubble_pen_activity();

    let sample = pen_modeler::sample(
        x as f64,
        y as f64,
        p as f64,
        state.stroke_origin.elapsed().as_secs_f64(),
        state.draw.width_scale,
    );

    // 落笔边沿的 adapter 副作用(旧实现顺序: 取消镜头抓手 → 定色 →
    // 气泡自动建页 → DB 起笔)。放在喂会话之前: 气泡自动建页会走
    // redraw_from_strokes(取消进行中的笔), 先喂 Down 会被清掉、本笔后续
    // Move 全部丢失(旧自研模型器的潜在缺陷, 这里顺手规避)。
    if down && !state.in_stroke() {
        cancel_camera_drag(state);
        state.canvas.color = (
            (state.draw.pen_r * 255.0) as u8,
            (state.draw.pen_g * 255.0) as u8,
            (state.draw.pen_b * 255.0) as u8,
        );
        // 气泡「写完自动新建一页」: 这笔若开新消息(冷启动/静默超时), 先建页
        // 载入再重绘 —— 笔画落在新页, 消息边界 = 页边界(设置 × 气泡 tab 开启)
        let auto_page = glaspen_core::export::glaspen2_bubble_auto_page_if_due();
        if auto_page > 0 {
            glaspen_core::export::glaspen2_load_strokes_for_screen(auto_page);
            redraw_from_strokes(state);
            hud_notify(i18n::text(Lang::Zh, &Msg::PageAutoCreated));
        }
        glaspen_core::export::glaspen2_begin_stroke(
            state.draw.pen_r,
            state.draw.pen_g,
            state.draw.pen_b,
            state.draw.width_scale,
        );
    }

    // 喂会话:错误(重复点/负时间差/间隔过大)整条报告忽略,保持旧策略。
    let report = match state.pen.feed(down, sample) {
        Ok(report) => report,
        Err(_) => return None,
    };
    let (points, finished) = match &report {
        PenReport::Ignored => return None,
        PenReport::Began(points) | PenReport::Moved(points) => (points.as_slice(), false),
        PenReport::Finished(finished) => (finished.points.as_slice(), true),
    };

    // 记录笔画到 STROKES/DB(用于撤销、导出、XOJ 保存)。宽度走旧 f32 收窄,
    // 时间就是会话输出(与旧 Windows 模型器 r.time 同轴: GIF 回放时间线按它
    // 展开, t=0 会让导出永远为空; adapter 时间原点由 stroke_origin 保证)。
    for point in points {
        glaspen_core::export::glaspen2_add_point_t(
            point.x,
            point.y,
            pen_modeler::db_width(point.width),
            point.time,
        );
    }

    let z = if infinite_on() { camera().zoom as f32 } else { 1.0 };
    // 模型器输出是画布坐标;绘制前经镜头变换回视图,线宽同步缩放
    let pts: Vec<(f32, f32, f32)> = points
        .iter()
        .map(|point| {
            let (vx, vy) = view_from_canvas(point.x, point.y);
            (
                vx as f32,
                vy as f32,
                pen_modeler::view_radius(point.width, z),
            )
        })
        .collect();

    if finished {
        // 抬起:补最后一段轮廓 + 终点圆帽,清空并复位模型器会话
        let dirty = redraw_pen(state, &pts);
        end_stroke_cleanup(state);
        return dirty;
    }
    redraw_pen(state, &pts)
}

/// 收笔收尾(DB 落笔 + 抬笔扇出 + 模型器会话复位)。
/// 正常抬笔走这里;抬笔报告丢失(笔离开感应区)时由穿透计时器超时强制调用,
/// 否则会话会永远停在 Active,全屏覆盖层一直拦截鼠标。
fn end_stroke_cleanup(state: &mut OverlayState) {
    glaspen_core::export::glaspen2_end_stroke();
    // 长笔点列抽稀(macOS 在 modeler_commit_to_strokes 内部做同样的事)
    glaspen_core::export::glaspen2_decimate_last_stroke();
    // 抬笔扇出:草稿推送/共享上行/OCR 登记(macOS 在 modeler_commit 里做)
    glaspen_core::export::glaspen2_notify_stroke_committed();
    state.pen_path.clear();
    state.reset_pen();
}
