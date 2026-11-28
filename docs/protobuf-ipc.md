# protobuf 机器间通信改造(分阶段)

> 目标: 把所有**跨机器边界**的 JSON 文本换成 protobuf 二进制帧。
> 注意: 数据库里的 JSON(笔迹/气泡/快捷键)不是机器通信, 不在这条线内;
> Rust <-> Rust 同进程调用也不再序列化, 直接 typed Rust API。

## 边界盘点

| 边界 | 当前格式 | 迁移目标 |
|---|---|---|
| Dart/Flutter 设置面板 <-> Rust FRB | 部分 `String` JSON(settings/snapshot/bubbles/copy-paste) | `Vec<u8>` protobuf 帧 |
| Windows 覆盖层 <-> Flutter 面板(命名管道) | 换行分隔 JSON | u32 LE 长度 + `Frame` protobuf |
| macOS Swift <-> Rust C ABI | 多个 `*_json` 字符串 | `(ptr,len)` protobuf + SwiftProtobuf |
| Rust core <-> macOS Rust shell | 实时 plan/录制 JSON(历史边界) | 已完成 typed Rust API, 不走 protobuf |
| SQLite | `strokes_json` / 设置值 | 不迁移(持久化, 非机器通信) |

## 第一阶段(已完成)

- 新增 `proto/glaspen/ipc/v1/ipc.proto`:
  - `Frame { protocol_version, request_id, oneof body }`
  - `SettingsSnapshot` / `SetSetting` / `SettingValue`(覆盖 bool/int/double/string/Struct/List/bytes/null)
  - `Ack` / `Error`
  - `Panel` 过渡信封(`kind` + `google.protobuf.Struct data` + `ok`), 供 Windows
    面板管道逐消息迁移;
  - Rust 侧 `json_to_prost_value` / `prost_value_to_json` / `json_to_prost_struct` /
    `prost_struct_to_json` 兼容转换, 以及 `panel_frame` helper。
- 新增 `crates/glaspen-ipc`:
  - `prost-build` 生成 Rust 类型;
  - `encode_frame` / `decode_frame` / `take_frame`;
  - 4 字节 LE 长度前缀, 单帧上限 8 MiB, 版本握手辅助。
- 尚未切换任何客户端: 旧 JSON 通路继续工作, 这是兼容期的契约层。

## 后续阶段

### P1 Windows 命名管道(已完成, 无 JSON 兼容期)

- ✅ 服务端只接受 `GPB1` 魔数 + `u32 LE 长度 + Frame`; JSON 行协议已删除;
- ✅ `PanelOut` 把现有业务 handler 的响应转成 Panel protobuf 帧;
- ✅ Dart `_NamedPipeBridge` 已生成 protobuf 模型, 连接后先发 `GPB1`,
  请求/响应/广播全部走 length-delimited Frame;
- ✅ Flutter `protobuf`/`fixnum` 依赖和 `lib/src/proto/` 生成文件已入库;
- 待真机验证: Windows 面板连接、getSettings/onSettingsChanged、缩略图搬运。

### P2 Dart <-> Rust FRB

- ✅ `set_setting` 已改为 `Vec<u8>`(`glaspen.ipc.v1.SettingValue`); Dart 侧
  `encodeSettingValue` 编码; Rust 解码后再喂 macOS C ABI(该 C ABI 的 JSON 属 P3);
- ✅ `uiConfig` / `bubbleMessages` / `nasBackup` 改为 `Vec<u8>` Panel protobuf;
  Dart 端 `decodePanelMessage` 解包(UI 边界仍是 dynamic map, 但线上不再传 JSON 文本);
- ⏳ `copyStrokes` / `pasteStrokes` 的自定义 line payload 不是 JSON; typed 化留待后;
- 其余 FRB 函数本来已经 typed, 保持不动。

### P3 macOS Swift C ABI

- ✅ settings 快照 C ABI 已改为 **Rust 直调**: `settings_snapshot_pb()` 返回 Panel
  protobuf; `set_setting_pb(key, bytes)` 解码 `SettingValue`; `glaspen2_macos_settings_json`
  和 `glaspen2_macos_set_setting` C 符号删除; probe 改为解码 protobuf;
- ✅ Swift 直接调用的 JSON 已迁:
  - `shortcut_store_install` / `shortcut_apply` / `shortcut_store_values` 改 `StringMap`;
  - `menu_add_top_level` 的 titles 改 `StringMap`;
  - `menu_update_texts` 的 top/export/more 改 `StringList`;
  - Swift 侧新增 `ProtoWire.swift` 手写最小 protobuf map/list 编解码, 不引入 SwiftProtobuf;
- ✅ Swift 头文件中不再声明 JSON C ABI; 剩余 `*_json` Rust 导出仅被同进程 Rust 内部调用或测试使用, 不属于机器边界;
- Swift 侧引入 SwiftProtobuf(或提交生成 `.pb.swift` + runtime);
- 先把 settings/menu/shortcut 三组迁完, 再迁 recording/grid/camera/minimap。

## 工具链

- Rust: `prost` / `prost-build`; `protoc` 已在本机 `/opt/homebrew/bin/protoc`;
- Dart: `protoc_plugin` + `protobuf` package, 生成到 `flutter_settings/lib/src/proto/`;
- Swift: SwiftProtobuf + `protoc-gen-swift`, 生成的 `.pb.swift` 纳入 build.rs 源列表;
- 生成代码提交仓库, 构建时不依赖网络; `build.rs` 监视 `.proto` 变更。

## 兼容与验收

1. 先加 `_pb` 端点, 旧 JSON 端点保持; 客户端逐个切换。
2. 每个边界迁移必须有: 帧 roundtrip 单测、非法/超大帧测试、旧客户端拒绝路径。
3. 全部切换后再删旧 JSON codegen/字符串 API。
4. 不做 big-bang: protobuf 字段号永不复用, 新字段只追加。


## typed payload 迁移(进行中)

已有：

- `AppSettings` / `SettingsSnapshot` / `SettingsChanged` typed 消息；
- macOS settings snapshot、Windows `getSettings`/`onSettingsChanged`、Dart 解码已切 typed；
- `Frame.settings_snapshot_response` / `Frame.settings_changed` 不再是 `Panel` 动态 Value。

待迁移：

- ✅ `PageSummary` / `PageListResponse` typed；Windows `listPages` 已切 typed，
  ✅ `PageThumbnail/PageThumbnailsResponse` typed，Windows 缩略图不再 base64/JSON；
  页面操作仍待迁；
- ✅ `BubbleMessages/BubbleMessage/BubbleStroke/BubblePoint` typed，FRB `bubbleMessages` 已切 typed（DB 仍存 JSON）；
- ✅ macOS 录制改 direct typed `RecordingEvent/RecordingEffect`（`recording_handle_typed`），
  删除 macOS 内部 `recording_event_json` 路径与 core 事件/effects JSON 编解码；
- ✅ macOS `ActionEffect` 已直接 typed 匹配，删除 `glaspen2_action_effect_json` FFI；
- ✅ macOS minimap 改 direct typed `minimap::layout`，删除 minimap layout JSON FFI；
- ✅ 删除无调用方的 grid JSON FFI 与测试（core grid 已 typed）；
- `GridLines` / `MinimapLayout` proto 仅在需要跨进程时再定义（当前 Windows 同进程 direct）；
- `Panel` 其余命令 oneof；
- macOS 内部 `*_json` FFI 改 typed Rust 调用。

- ✅ 删除最后的 `glaspen2_live_plan_json` 与输入/输出 JSON 编解码；实时计划只保留 typed `build_macos_live_plan`。

- ✅ 删除 `process_pipe_message` 中已被 typed 路径替代的死 Panel 分支；函数现在只剩 `uiConfig` 与 `downloadUpdate` 两条 JSON 命令。

- ✅ macOS settings apply 直接调用 core `settings_apply::apply_setting`，删除 `glaspen2_settings_apply` effect JSON FFI。
