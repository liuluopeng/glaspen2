# D9d 绘制管线验证记录(2026-11-23)

> 结论:**macOS 自动验证通过;Windows 部分按用户决定(2026-11-23)延期,作为已知风险记录**。
> 本文件是 D9d 的验收凭证;延期不等于 Windows 已验证,后续有 Windows 环境时按文末清单补做。

## 验证对象

D9c 之后两端绘制全部消费 core::draw_spec::DrawPlan:

- Windows:重建 overlay/actions.rs::redraw_from_strokes + 实时 overlay/pen.rs::redraw_pen,
  经 crates/glaspen-windows/src/paint_plan.rs 适配(像素直写,无 cairo)。
- macOS:整页重建 core export/mod.rs::paint_strokes_into(cairo) + 实时
  src/macos/parts/panel.m raw_draw_dot/raw_draw_segment(FFI glaspen2_live_plan_json)。

## macOS(本机已执行)

| 检查 | 命令 | 结果 |
|---|---|---|
| 离屏像素回归(旧实现 oracle vs plan 光栅化,5 组固定笔迹:Retina 2x / zoom 0.7 / 仅描边 / 仅阴影 / 邻页 y_shift) | cargo test -p glaspen-core export::tests::plan_rasterization_matches_legacy_pixels | **通过,逐字节 0 差异** |
| 整页命令序列 = D9a 特征化快照 | cargo test -p glaspen-core export::tests::macos_paint_plan_matches_d9a_characterization_snapshot | 通过 |
| 实时计划 FFI(3 点命令流/相位/接缝/非法输入/单行 JSON) | cargo test -p glaspen-core live_plan | 4/4 通过 |
| ObjC 编译(实时 dot/segment 消费计划) | cargo build(build.rs 监视 parts/*.m) | 退出 0,clang 成功 |

**未执行(本环境无 GUI 会话)**:真实鼠标/笔输入下的肉眼效果、橡皮、飘渺、缩放、
多屏;这些依赖人工在 macOS 桌面手测(建议清单:书写/橡皮/描边/阴影/缩放/翻页/多屏)。

## Windows(按用户决定延期,未验证)

2026-11-23 用户明确:"不用验证 Windows 了",因此本项不阻塞 0.7.0。
**风险声明**:Windows 像素/性能/笔输入均未真机验证;D9c 删除了一份手写 pass 实现,
所以"Windows 渲染与旧版一致"目前只有编译 + 几何/参数级测试支撑,不能声称已验证。

后续补做清单(有 Windows 环境时):

1. **CI 离屏像素**(无需 GUI):把固定笔迹渲染到内存 DIB,与入库 golden 逐字节比较;
   建议覆盖:重建/实时、阴影三档、描边取相邻较粗、0.5px 下限、ceil 脏区、初始点。
2. **VM/真机功能冒烟**:覆盖层窗口/DWM/亚克力、多显示器 DPI、托盘、剪贴板、
   实时书写/笔压/擦除(Windows 无橡皮路径,确认与设计一致)。
3. **帧时间**:perf_log_event 等现有计时,整页重建 + 每 HID 报告各采样;
   注意实时路径每报告现在会分配一个 DrawPlan(旧实现为零分配),需确认可接受。
4. 在台账 D9d 行记录结果,并把本节"未验证"改为"已补验"。

## 相关提交

- D9a b596c28、D9b 10f1459..1a68725、D9c 8b34ffb/db2575b/d0924a5/0790253。
