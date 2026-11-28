# 发版清单

> 适用：macOS 正式包、Windows 安装包。每次打 tag 前逐项打勾，
> 未过的项要么修掉，要么在 release notes 里明确写成已知限制。

## 0. 版本与锁文件

- [ ] `Cargo.toml` 的 `[workspace.package] version` 是唯一版本源；
      所有 crate 用 `version.workspace = true`
- [ ] `flutter_settings/pubspec.yaml` 的 `version` 与上面一致
- [ ] `Cargo.lock` 按当前项目约定不提交（已加入 `.gitignore`）；`flutter_settings/pubspec.lock` 仍提交
- [ ] `CHANGELOG.md` 有对应版本段落，日期与 tag 一致
- [ ] 每次 bump 的版本都要有对应 tag 与 CHANGELOG 段落（0.5.2 曾只 bump 未发，勿重演）

## 1. 质量门

在仓库根目录：

```bash
sh scripts/lint.sh
```

必须全绿：

- [ ] `cargo fmt --check`
- [ ] `cargo clippy --workspace --exclude glaspen2_installer --all-targets -- -D warnings`
- [ ] `cargo test --workspace --exclude glaspen2_installer`
- [ ] `cd flutter_settings && fvm flutter analyze`（0 issue）
- [ ] `cd flutter_settings && fvm flutter test`

> CI 未固定 Rust 版本（用最新 stable）。升级 stable 后先本地跑一遍
> `sh scripts/lint.sh`，让新 fmt/clippy lint 导致的红在本地暴露，别等 CI。

Windows 侧（有真机时）：

- [ ] `cargo check --workspace --exclude glaspen2_installer --all-targets`（本地/CI 双绿）
- [ ] 真机跑一遍：面板打开/关闭/重开、笔压、翻页、导出、更新
- [ ] CI 的 `windows-check` 是**硬门**（已移除 `continue-on-error`）：
      它红了就说明 cfg(windows) 代码编不过，**不允许打 tag** —— 历史上 v0.6.0
      的 Windows 侧就是这样带着编译错误发出去的

## 2. 功能冒烟

macOS：

- [ ] 冷启动 → 权限引导 → 能落笔
- [ ] 翻页（⌥⌘↑/↓、⌘⌃`/1）整页动效正常，笔迹不串页
- [ ] 关掉设置面板再打开（⌘⌃D），状态还在
- [ ] 保存（涂鸦 / 含背景）各一次；macOS 12/13 也要试含背景
- [ ] GIF 录制、PDF 导出（OCR 文本层需配置 axum OCR 服务；未配置时 PDF 无文本层属预期）
- [ ] 活页本：缩略图、minimap、重排、多选删除、页面详情圈选
- [ ] 检查更新 → 立即更新 → 自动重启 → 版本号变化
- [ ] 睡眠唤醒、双屏热插拔、4K/缩放屏各一次
- [ ] 连续书写 30 分钟，内存无持续增长

Windows：

- [ ] 覆盖层不挡鼠标（笔悬停时仍能点下层应用）
- [ ] 压感逐点、橡皮/笔键（如设备支持）
- [ ] 设置面板所有 tab 可用；关掉后 Ctrl+Alt+Shift+D 能重开
- [ ] 安装 → 卸载 → 重装；升级后版本号变化

## 3. 数据安全

- [ ] 用 0.5.x 的真实库升级到本版，笔迹/页序/设置都在
- [ ] 迁移前备份文件已生成：`glaspen2.db.before-vN`
- [ ] 设置面板「数据备份」生成 `.db`，再「从最新备份恢复」可读
- [ ] 更新中断（拔网/强杀）后旧版仍能启动，数据不丢
- [ ] 磁盘满 / 只读目录下不崩溃、有可读错误提示

## 4. 签名与分发

macOS：

- [ ] `scripts/create-signing-cert.sh` 或 Developer ID 证书就绪
- [ ] `GLASPEN2_DEVELOPER_ID` + `GLASPEN2_NOTARY_PROFILE` 已配置
- [ ] `scripts/build-dmg.sh` 通过签名自检
- [ ] `spctl -a -vv` / `codesign --verify --deep --strict` 通过
- [ ] 干净机器（未装过）能直接打开，无需右键
- [ ] DMG sha256 写进 release notes

Windows：

- [ ] `signtool` 对 exe 签名（OV/EV 证书）
- [ ] 安装器在干净 Windows 10/11 上不被 SmartScreen 拦截
- [ ] 安装器写卸载注册表、带卸载入口
- [ ] VC++ 运行库/ cairo DLL 完整，无缺失依赖

## 5. 发布

- [ ] `git tag vX.Y.Z`（本地）
- [ ] GitHub Release 上传 DMG / setup.exe / sha256
- [ ] release notes 写明：平台支持（macOS arm64 / Windows x64）、
      已知限制、数据兼容性、回滚方式
- [ ] 更新 `README` 的版本/平台说明
