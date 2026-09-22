# 自动更新(替换正在运行的旧版本)

> 现状:**macOS 已实现**;Windows 待 P1.5(命名管道 + 安装桩静默参数,
> UI 已带降级话术)。检查更新本身(无替换)见 `src/update.rs` 顶部文档。

## 角色与铁律

三个参与者:

| 角色 | 是什么 | 代码 |
| --- | --- | --- |
| 主程序 | 正常的 glaspen2(GUI) | `src/macos.rs` / ObjC |
| 帮手 | `glaspen2 --updater …`,detach 出来的替换执行者 | `src/updater.rs` |
| 新版 | 被替换后重新拉起的主程序 | 同上,`finish_pending()` |

**帮手进程正运行在要被替换的 bundle 里**,由此推出两条铁律:

1. 旧 bundle 只 `rename` 成 `<app>.old`,**帮手绝不删除它** —— 删掉自己正在
   执行的文件后,后续缺页会 SIGBUS;
2. `.old` 的清理交给**新版首启**的 `finish_pending()`:写 `ack` → 等帮手进程
   真的退出 → 按帮手留下的 `cleanup.list` 删除 → 清标记。

## 时序

```text
用户点「检查更新」→ 结果有新版本 → 点「立即更新」→ 确认对话框(release notes)
  → 下载(进度条/可取消;.part 转正前先 sha256 校验,GitHub 资产自带 digest)
  → 解包 stage:hdiutil 挂载 → ditto → 卸载 → codesign --verify → 剥 quarantine
  → 点「立即重启更新」
      主程序: spawn `--updater`(setsid detach)→ 300ms 落盘 → ⌘⌃Q 同路退出
      帮手:   写 helper.pid → 等主程序退出 → DB 快照
               → rename target→target.old → ditto 暂存→target → open 拉起
               → 等 ack ── 45s 内等到 → 写 cleanup.list(含 .old、dmg)→ 退出
                        └─ 没等到 → 删掉没起来的新版,.old 改回,重新拉起旧版
新版首启: finish_pending(): 写 ack → 等 helper.pid 的进程退出 → 执行清单 → 清标记
```

标记文件都在 `~/Library/Caches/glaspen2/updates/`:`helper.pid`、`ack`、
`cleanup.list`、`glaspen2.db.before-update`(快照)、`updater.log`(排障)。

## 权限与不变式

- **管理员授权**:目标目录(如 `/Applications`)不可写时,替换序列经
  `osascript … with administrator privileges` 弹**一次**授权框(系统限制:
  每次更新都要输一次;想免密就装 `~/Applications`)。
- **TCC(辅助功能/录屏)**:同 bundle id + 同签名证书("Glaspen2
  Development"),原位替换后权限保留。**绝不能**落到 ad-hoc 签名。
- **DB schema 向前迁移**:新版起来后旧版会**拒绝打开**库文件。二进制回滚
  (握手超时)发生在新版起来之前,所以是完整的;更远的事后降级要用
  `glaspen2.db.before-update` 快照或「数据备份」功能。
- Gatekeeper:自签未公证,解包副本显式剥 `com.apple.quarantine`;公证留待 P3。

## 测试

自动(都在 `scripts/lint.sh` / CI 里):

```bash
cargo test update::      # 解析/选包/下载(本地 HTTP)/校验/取消
cargo test updater::     # 换 bundle/握手/回滚/清理,临时目录 + 注入 Effects
fvm flutter test         # 面板挂载 + FRB wire(含 appVersion)
cargo test update:: -- --ignored   # 真打一次 GitHub(手动)
```

## 手动冒烟清单(发版前跑一遍)

环境:已安装的正式版(如 0.5.1)在 `/Applications`,或直接对将要发布的
新版本自己跑一遍。

1. **检查**:面板「关于 → 检查更新」→ 出现「发现新版本」+ 两个按钮。
2. **取消下载**:点「立即更新」→ 确认 → 下载中点「取消」→ 回到按钮态;
   `updates/` 里**没有** `.part` 残留。
3. **完整更新**:再次「立即更新」→ 进度走完 → 解包 → 「立即重启更新」→
   - 装在 `/Applications` 时出现**一次**管理员授权框;
   - 应用退出、几秒后自动回来,「当前版本」变成新版本;
   - `updates/` 里没有 `*.app.old`、没有 dmg、没有 `helper.pid`/`ack`/
     `cleanup.list`;`updater.log` 最后是 `update finished`；
   - 系统设置里辅助功能/录屏权限**仍然勾着**。
4. **回滚路径**(可选):把 `updates/ack` 删不掉就模拟不了 —— 用
   `chmod 000` 暂存包让新版起不来,或本地把 `finish_pending` 临时改掉;
   预期:45s 后旧版被自动拉起,`updates/updater.log` 有 `rolled back`。
5. **登录项/快捷键**:更新后 ⌘⌃ 快捷键、开机自启、数据(最新笔迹)都在。

### 用本地 JSON 服务做全链路演练(不必等真发版)

`GLASPEN2_UPDATE_API` 可以覆盖检查端点。asset 的 URL 仍指真 GitHub 资产
(https、digest 对得上),只有"最新是哪个版本"是假的:

```bash
# 1. 造一份 "9.9.9" 的 release JSON(asset 指向真实存在的 0.5.0 dmg)
python3 -m http.server 8899 --directory /tmp/fake-update   # 服务 release.json
# 2. 启动应用前:
export GLASPEN2_UPDATE_API=http://127.0.0.1:8899/release.json
cargo run   # 或已安装的版本
# 3. 面板会显示"发现新版本 v9.9.9",走完 下载→解包→替换→重启 全流程;
#    替换后版本号是 dmg 里真实的那个(演练的是机制,不是内容)。
```

`release.json` 形状:`{ "tag_name": "v9.9.9", "html_url": "...",
"body": "...", "assets": [{ "name": "glaspen2-<ver>-arm64.dmg",
"browser_download_url": "<真实下载地址>", "size": <真实字节>,
"digest": "sha256:<真实 digest>" }] }`(digest 从 GitHub API 的同名字段抄)。

## 已知边界

- 更新期间不拦截落笔:确认对话框打开时用户自然停笔,退出前再等 300ms 让
  在途的后台写落地;正在录 GIF 时点更新会中断录制(GIF 按住快捷键,与
  对话框互斥)。
- 下载无断点续传(取消后重下;已下完且校验通过的包会复用)。
- 帮手日志在 `updates/updater.log`,面板看不到 —— 排障先看它。
