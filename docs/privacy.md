# 隐私说明（glasspen / 玻璃涂鸦）

glaspen2 默认是一个**纯本地**的涂鸦工具：笔迹、页面、设置、缩略图、OCR 结果
全部保存在本机 SQLite / 用户目录，不会自动上传。

## 会联网的功能（均为显式触发或需用户配置）

| 功能 | 目标 | 发送内容 | 默认 |
|---|---|---|---|
| 检查更新 / 自动更新 | GitHub Releases API | 当前版本号（匿名） | 手动触发；启动不联网 |
| 手写消息 / 共享画布 | 用户配置的服务地址 | 笔迹帧、消息内容 | 默认关闭（总开关） |
| OCR 文字识别 | 用户配置的服务地址 | 当前页渲染的 PNG | 未配置服务时不启用 |

- 以上功能都需要用户主动开启/配置；未配置时请求不会发出。
- 更新包会校验 SHA-256 与发布者签名；下载源固定为 GitHub。
- 不收集遥测、设备指纹、使用统计；没有崩溃自动上传。
- 崩溃报告与日志只写本机：
  - macOS `~/Library/Logs/glaspen2/`
  - Windows `%LOCALAPPDATA%\glaspen2\logs\`

## 凭据

手写消息/共享画布的账号与密码由用户主动填写，保存在本机数据库
（`user_settings`）。当前版本尚未接入系统钥匙串，请勿在共享电脑上保存
敏感账号；后续版本会迁移到 macOS Keychain / Windows DPAPI。

## 删除数据

退出应用后删除数据库文件即可清空全部笔迹与设置：

- macOS：`~/Library/Application Support/glaspen2/glaspen2.db`
- Windows：`%LOCALAPPDATA%\glaspen2\glaspen2.db`

删除前建议使用设置面板的「数据备份」导出 `.db` 文件。
