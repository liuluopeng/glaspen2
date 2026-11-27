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

### P1 Windows 命名管道

- 覆盖层与 Flutter 面板改为二进制帧;
- 请求/响应消息按 `Frame.body` 分型(逐步从 settings 扩展到 page/thumbnail/bubble/export);
- Dart 侧生成 protobuf 代码; 大图/缩略图直接放 `bytes`, 不再 base64;
- 双栈期: 旧 JSON 行协议只保留一个版本用于回滚。

### P2 Dart <-> Rust FRB

- `settings_snapshot` / `set_setting` / bubbles / copy-paste payload 改为 `Vec<u8>`;
- 其余 FRB 函数本来已经 typed, 保持不动;
- 用生成的 Dart 模型替代 map/JSON 解析。

### P3 macOS Swift C ABI

- 新符号 `*_pb` 接收/返回 `(ptr,len)`, 旧 `*_json` 保留兼容壳;
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
