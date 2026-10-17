# Third-Party Notices / 第三方组件声明

glaspen2 以 MIT 许可发布（见 `LICENSE`）。分发包还包含以下第三方组件，
其版权归各自作者所有：

## 应用框架与运行时

| 组件 | 许可 | 说明 |
|---|---|---|
| Flutter / Dart | BSD-3-Clause | 设置面板 UI 与运行时 |
| flutter_rust_bridge | MIT | Dart ↔ Rust 同进程桥 |
| Rust 依赖（Cargo.lock） | MIT / Apache-2.0（逐 crate） | 以 `cargo metadata` 为准 |

## 渲染与图像

| 组件 | 许可 | 说明 |
|---|---|---|
| cairo | LGPL-2.1 / MPL-1.1 | 笔迹渲染（macOS 链接 Homebrew 版本；Windows 随包分发 DLL） |
| pixman | MIT | cairo 像素后端 |
| freetype | FTL / GPL-2.0（二选一） | 字体栅格化 |
| fontconfig | MIT-like | 字体匹配 |
| harfbuzz | MIT | 文本整形 |
| libpng / zlib | libpng-2.0 / Zlib | PNG 编解码 |
| glib / gobject | LGPL-2.1 | fontconfig/harfbuzz 依赖 |

Windows 包内 `vendor/win/cairo/*.dll` 为 MSYS2/MinGW 构建产物；其源码与
许可文本可从 <https://www.msys2.org/> 与各上游项目获取。分发时请保留本
声明；LGPL 组件允许用户替换 DLL（同目录替换即可）。

## 字体

| 组件 | 许可 | 说明 |
|---|---|---|
| LXGW WenKai Mono / 霞鹜文楷等宽 | SIL Open Font License 1.1 | 设置面板字体（`flutter_settings/assets/LXGWWenKaiMono-Regular.ttf`） |
| glyphless.ttf | 项目内生成 | PDF 隐形文本层占位字形（无可见字形） |

OFL 要求随字体分发许可文本；完整文本见
<https://scripts.sil.org/OFL>。

## 图标 / 插画

`glaspen2.icns`、`glaspen2.ico`、`introduct/` 与 `flutter_settings/assets/*.jpg`
为项目自带素材，随 MIT 许可发布。
