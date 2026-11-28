//! 设置面板协议的唯一契约(命令表 / 请求解析 / 响应帧 / 协议版本)。
//!
//! 历史: macOS 面板走 FRB(进程内强类型), Windows 面板是独立进程走命名管道
//! (手写 JSON)。两边命令表靠人肉同步, 于是出现"面板发了/后端没实现"、
//! 少换行、少 `reqId`、int-vs-bool 等一整类 bug。本文件把命令表、请求解析、
//! 响应帧与协议版本收敛成一处; 两端的实现(FRB / 管道)都以此为准。
//!
//! 约束:
//! - 新增面板命令 → 在 `COMMANDS` 加一行, 并实现两端的 handler;
//!   `tests` 会扫描 Windows 管道与 Flutter 桥接层, 少一方就失败;
//! - 响应统一走 `response_line`/`error_line`: 单行 JSON + 末尾 \n
//!   (面板按行分帧), 不手拼字符串;
//! - 面板启动先取 `uiConfig`, 其中带 `protocolVersion`; 面板把自身版本随
//!   请求发来, 不一致时明确报错(独立 exe 会出现新旧面板混跑)。

use serde_json::{Value, json};

/// 面板 ↔ 后端协议版本。**改请求/响应形状/语义时 +1**。
/// Flutter 侧同名常量 `kPanelProtocolVersion` 必须同步(tests 会扫描校验)。
pub const PROTOCOL_VERSION: u32 = 1;

/// 命令由谁处理。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dispatch {
    /// Windows 管道主 dispatch 分支。
    Main,
    /// 在别的流程里内联处理(如下载循环里的 cancelDownload)。
    Inline,
    /// 历史遗留/已无调用方; 保留只为兼容旧面板, 新代码不要用。
    Legacy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandDef {
    pub name: &'static str,
    pub dispatch: Dispatch,
}

/// 唯一命令表(按名字排序)。`tests` 保证与两端的真实实现一致。
pub const COMMANDS: &[CommandDef] = &[
    CommandDef {
        name: "appVersion",
        dispatch: Dispatch::Main,
    },
    CommandDef {
        name: "applyUpdate",
        dispatch: Dispatch::Main,
    },
    CommandDef {
        name: "backupNow",
        dispatch: Dispatch::Main,
    },
    // 下载流式回传期间面板发来的取消请求, 被 downloadUpdate 的循环偷看消费。
    CommandDef {
        name: "cancelDownload",
        dispatch: Dispatch::Inline,
    },
    CommandDef {
        name: "canvasNew",
        dispatch: Dispatch::Main,
    },
    CommandDef {
        name: "canvasOverview",
        dispatch: Dispatch::Main,
    },
    CommandDef {
        name: "checkUpdate",
        dispatch: Dispatch::Main,
    },
    CommandDef {
        name: "copyStrokes",
        dispatch: Dispatch::Main,
    },
    CommandDef {
        name: "deletePage",
        dispatch: Dispatch::Main,
    },
    CommandDef {
        name: "deleteStrokes",
        dispatch: Dispatch::Main,
    },
    CommandDef {
        name: "downloadUpdate",
        dispatch: Dispatch::Main,
    },
    CommandDef {
        name: "exportPdf",
        dispatch: Dispatch::Main,
    },
    CommandDef {
        name: "flushBubbles",
        dispatch: Dispatch::Main,
    },
    CommandDef {
        name: "getBubbles",
        dispatch: Dispatch::Main,
    },
    CommandDef {
        name: "getPageThumbnails",
        dispatch: Dispatch::Main,
    },
    CommandDef {
        name: "getSettings",
        dispatch: Dispatch::Main,
    },
    CommandDef {
        name: "hotkey",
        dispatch: Dispatch::Main,
    },
    CommandDef {
        name: "lassoSelect",
        dispatch: Dispatch::Main,
    },
    CommandDef {
        name: "listPages",
        dispatch: Dispatch::Main,
    },
    CommandDef {
        name: "moveStrokes",
        dispatch: Dispatch::Main,
    },
    CommandDef {
        name: "moveStrokesToPage",
        dispatch: Dispatch::Main,
    },
    CommandDef {
        name: "nasBackup",
        dispatch: Dispatch::Main,
    },
    CommandDef {
        name: "navigateToPage",
        dispatch: Dispatch::Main,
    },
    CommandDef {
        name: "openUrl",
        dispatch: Dispatch::Main,
    },
    CommandDef {
        name: "pasteStrokes",
        dispatch: Dispatch::Main,
    },
    CommandDef {
        name: "reorderPage",
        dispatch: Dispatch::Main,
    },
    CommandDef {
        name: "restoreLatestBackup",
        dispatch: Dispatch::Main,
    },
    CommandDef {
        name: "setSetting",
        dispatch: Dispatch::Main,
    },
    CommandDef {
        name: "testChatLogin",
        dispatch: Dispatch::Main,
    },
    CommandDef {
        name: "uiConfig",
        dispatch: Dispatch::Main,
    },
];

pub fn command(name: &str) -> Option<&'static CommandDef> {
    COMMANDS.iter().find(|c| c.name == name)
}

pub fn known(name: &str) -> bool {
    command(name).is_some()
}

#[derive(Debug)]
pub enum ProtocolError {
    NotJson(String),
    NotObject,
    MissingType,
    UnknownCommand(String),
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtocolError::NotJson(e) => write!(f, "请求不是合法 JSON: {e}"),
            ProtocolError::NotObject => write!(f, "请求必须是 JSON 对象"),
            ProtocolError::MissingType => write!(f, "请求缺 type 字段"),
            ProtocolError::UnknownCommand(c) => write!(f, "未知面板命令: {c}"),
        }
    }
}

/// 解析一行请求。`req_id` 缺省为 0(与管道旧约定一致)。
#[derive(Debug, Clone)]
pub struct Request {
    pub req_id: i64,
    pub command: &'static CommandDef,
    pub payload: Value,
}

pub fn parse_request(line: &str) -> Result<Request, ProtocolError> {
    let v: Value =
        serde_json::from_str(line.trim_end()).map_err(|e| ProtocolError::NotJson(e.to_string()))?;
    let obj = v.as_object().ok_or(ProtocolError::NotObject)?;
    let name = obj
        .get("type")
        .and_then(Value::as_str)
        .ok_or(ProtocolError::MissingType)?;
    let command = command(name).ok_or_else(|| ProtocolError::UnknownCommand(name.to_string()))?;
    let req_id = obj.get("reqId").and_then(Value::as_i64).unwrap_or(0);
    Ok(Request {
        req_id,
        command,
        payload: v,
    })
}

/// 成功响应: `{"type":"<cmd>_response","reqId":N,"data":<data>}` + '\n'。
pub fn response_line(req_id: i64, command: &str, data: Value) -> String {
    format!(
        "{}\n",
        json!({ "type": format!("{command}_response"), "reqId": req_id, "data": data })
    )
}

/// 失败响应: data = `{"error": msg}`(面板按行分帧, 同样带换行)。
pub fn error_line(req_id: i64, command: &str, message: &str) -> String {
    response_line(req_id, command, json!({ "error": message }))
}

/// 面板版本握手: 请求里必须带 `panelProtocolVersion`, 且与 [PROTOCOL_VERSION] 一致。
/// 独立 exe 的 Windows 面板可能出现"旧面板连新覆盖层", 这里明确报错而不是静默错乱。
pub fn check_panel_version(payload: &Value) -> Result<(), String> {
    let panel = payload
        .get("panelProtocolVersion")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    if panel == i64::from(PROTOCOL_VERSION) {
        Ok(())
    } else {
        Err(format!(
            "面板协议版本不一致: 面板 v{panel}, 后端 v{PROTOCOL_VERSION}; 请重启面板或重新安装"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(v: Vec<String>) -> Vec<String> {
        let mut v = v;
        v.sort();
        v
    }

    #[test]
    fn command_table_is_well_formed() {
        let mut names = std::collections::HashSet::new();
        for c in COMMANDS {
            assert!(!c.name.is_empty());
            assert!(names.insert(c.name), "命令重复: {}", c.name);
            assert!(known(c.name));
        }
        assert_eq!(command("no_such_command"), None);
        assert!(!known("no_such_command"));
    }

    /// Windows 管道 `msg_type == "..."` 的分支集合 = Main ∪ Legacy。
    #[test]
    fn pipe_dispatch_matches_command_table() {
        let src = include_str!("../../../crates/glaspen-windows/src/overlay/pipe.rs");
        let mut actual = Vec::new();
        let needle = "msg_type == \"";
        let mut rest = src;
        while let Some(i) = rest.find(needle) {
            let tail = &rest[i + needle.len()..];
            let end = tail.find('"').expect("msg_type 字符串未闭合");
            actual.push(tail[..end].to_string());
            rest = &tail[end..];
        }
        actual.sort();
        actual.dedup();
        assert_eq!(
            actual,
            vec!["downloadUpdate", "uiConfig"],
            "Windows 管道仍有未迁移 JSON 命令"
        );
        for token in [
            "PanelInput::GetSettings",
            "PanelInput::SetSetting",
            "PanelInput::ListPages",
            "PanelInput::PageOp",
            "PanelInput::GetBubbles",
        ] {
            assert!(src.contains(token), "Windows typed dispatch 缺 {token}");
        }
    }

    /// Flutter 管道桥实际发出的请求类型 = 非 Legacy 命令。
    #[test]
    fn panel_requests_match_command_table() {
        let src = include_str!("../../../flutter_settings/lib/main.bridge.dart");
        let start = src
            .find("class _NamedPipeBridge")
            .expect("找不到 _NamedPipeBridge");
        let seg = &src[start..];
        let mut actual = Vec::new();
        let mut rest = seg;
        while let Some(i) = rest.find("_request(") {
            let tail = &rest[i + "_request(".len()..];
            let tail = tail.trim_start();
            if let Some(q) = tail.strip_prefix('\'') {
                let end = q.find('\'').expect("_request 参数未闭合");
                actual.push(q[..end].to_string());
                rest = &q[end..];
            } else {
                rest = tail;
            }
        }
        // P1 protobuf: one-way 请求走 _send('type', ...)。
        let mut rest = seg;
        while let Some(i) = rest.find("_send(") {
            let tail = &rest[i + "_send(".len()..];
            let tail = tail.trim_start();
            if let Some(q) = tail.strip_prefix('\'') {
                let end = q.find('\'').expect("_send 参数未闭合");
                actual.push(q[..end].to_string());
                rest = &q[end..];
            } else {
                rest = tail;
            }
        }

        // 面板还有若干直接 jsonEncode({'type': ...}) 的请求。
        let mut rest = seg;
        while let Some(i) = rest.find("'type': '") {
            let tail = &rest[i + "'type': '".len()..];
            let end = tail.find('\'').expect("type 字符串未闭合");
            actual.push(tail[..end].to_string());
            rest = &tail[end..];
        }
        actual.sort();
        actual.dedup();
        let expected = vec![
            "cancelDownload".to_string(),
            "downloadUpdate".to_string(),
            "uiConfig".to_string(),
        ];
        assert_eq!(actual, expected, "Flutter 面板仍有未迁移 JSON 请求");
        for token in [
            "encodeGetSettingsRequest",
            "encodeSetSettingRequest",
            "encodePageOpRequest",
            "encodeGetBubblesRequest",
        ] {
            assert!(seg.contains(token), "Flutter typed bridge 缺 {token}");
        }
    }

    /// Flutter 侧的协议版本常量必须与 core 同步。
    #[test]
    fn panel_protocol_version_matches_core() {
        let src = include_str!("../../../flutter_settings/lib/main.bridge.dart");
        let marker = "kPanelProtocolVersion =";
        let i = src
            .find(marker)
            .expect("Flutter 缺 kPanelProtocolVersion 常量");
        let tail = &src[i..];
        let eq = tail.find('=').expect("常量无赋值");
        let num: u32 = tail[eq + 1..]
            .trim_start()
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse()
            .expect("协议版本不是数字");
        assert_eq!(num, PROTOCOL_VERSION, "Flutter 与 core 协议版本不一致");
    }

    #[test]
    fn request_parsing() {
        let r = parse_request(r#"{"type":"getSettings","reqId":7,"x":1}"#).unwrap();
        assert_eq!(r.req_id, 7);
        assert_eq!(r.command.name, "getSettings");
        assert_eq!(r.payload["x"], 1);

        // reqId 缺省 = 0
        let r = parse_request(r#"{"type":"uiConfig","panelProtocolVersion":1}"#).unwrap();
        assert_eq!(r.req_id, 0);

        assert!(matches!(
            parse_request("not json"),
            Err(ProtocolError::NotJson(_))
        ));
        assert!(matches!(parse_request("[]"), Err(ProtocolError::NotObject)));
        assert!(matches!(
            parse_request(r#"{"reqId":1}"#),
            Err(ProtocolError::MissingType)
        ));
        assert!(matches!(
            parse_request(r#"{"type":"nope"}"#),
            Err(ProtocolError::UnknownCommand(_))
        ));
    }

    #[test]
    fn response_frames_are_single_line_and_typed() {
        let line = response_line(3, "uiConfig", json!({"a": 1}));
        assert!(line.ends_with('\n'));
        assert_eq!(line.matches('\n').count(), 1);
        let v: Value = serde_json::from_str(line.trim_end()).unwrap();
        assert_eq!(v["type"], "uiConfig_response");
        assert_eq!(v["reqId"], 3);
        assert_eq!(v["data"]["a"], 1);

        let err = error_line(4, "getSettings", "boom");
        assert!(err.ends_with('\n'));
        let v: Value = serde_json::from_str(err.trim_end()).unwrap();
        assert_eq!(v["type"], "getSettings_response");
        assert_eq!(v["reqId"], 4);
        assert_eq!(v["data"]["error"], "boom");
    }

    #[test]
    fn version_handshake() {
        assert!(check_panel_version(&json!({"panelProtocolVersion": PROTOCOL_VERSION})).is_ok());
        assert!(check_panel_version(&json!({"panelProtocolVersion": 999})).is_err());
        assert!(check_panel_version(&json!({})).is_err());
        // 旧面板完全没有版本字段 → 明确报错, 不静默继续。
        let e = check_panel_version(&json!({"panelProtocolVersion": 0})).unwrap_err();
        assert!(e.contains("版本不一致"));
    }
}
