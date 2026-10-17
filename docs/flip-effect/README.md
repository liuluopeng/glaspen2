# 玻璃隧道翻页：现状与问题示意

> 这两张图是**离屏模拟**（按 `src/macos/parts/run.m` 的 `tunnel_place` /
> `tunnel_draw_card` 参数 1:1 重画，卡片内容用示意笔迹），不是屏幕截图。
> 用途是排查几何/透明度问题，不代表最终视觉。

## 现状（`current.png`，动画中点）

- 所有卡片**同心**、只按 `pow(0.92, zc)` 等比缩小；每层只小 8%，实际同屏
  只有 3–4 张，看起来像"缩放叠框"，不像纵深。
- 卡片快照**只有笔迹**，没有页面表面；玻璃面只有 8% 灰。于是后面几页的
  笔迹全部透出来，叠成一片。
- 侧面是两块纯色平行四边形，阴影是两条硬边矩形；没有圆角、渐变、
  Fresnel 高光/软阴影。
- 背景实际是**全透明**（代码注释写的是"深色磨砂玻璃"）。

## 一个更接近玻璃的方向（`proposed.png`，示意）

- 真透视（`1/(1+kz)`）+ 轻微上移，让层与层有可读的间距；
- 圆角、面渐变、侧面渐变、顶部高光、软阴影；
- 深色背景 + 快速深度雾，远处页面只留轮廓；
- 落定帧必须与真实页完全对齐（已先修：见 commit `693b44e`）。

## 关键代码位置

| 问题 | 位置 |
|---|---|
| 等比缩小、所有卡同心 | `src/macos/parts/run.m` `tunnel_place` |
| 0.92 落定缩放 | 同上（已修为满屏） |
| 快照只有笔迹、8% 玻璃面 | `crates/glaspen-core/src/export/mod.rs` `render_page_into`；`tunnel_draw_card` |
| 纯色侧面/硬阴影 | `tunnel_draw_card` |
| 透明背景 | `tunnel_frame` 的 `cairo_paint` |
| 12 张卡只显示 3–4 张 | `page_flip_tunnel` 的 `before=12`（已改 4） |
| 60Hz 定时器 | `page_flip_tunnel` 的 `dispatch_source_set_timer` |
