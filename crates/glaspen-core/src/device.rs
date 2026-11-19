//! 设备身份: 本机一个稳定 UUIDv7(时间有序 —— 未来跨设备消息按它排序/
//! 对齐, 左右分栏互不干扰)。首次使用时生成并存 `user_settings` 表
//! (键 `device_uuid`), 之后恒定; 进程内 OnceLock 缓存, 每个进程至多
//! 一次 DB 往返(chat_glue 的 chat_seq 同款模式)。

use std::sync::OnceLock;

static DEVICE: OnceLock<String> = OnceLock::new();

/// 本机设备 UUID(v7 字符串形如 `0192f3a4-...`)。
pub fn device_uuid() -> String {
    DEVICE
        .get_or_init(|| {
            if let Some(v) = crate::runtime().block_on(crate::db::load_setting("device_uuid")) {
                let v = v.trim().to_string();
                if !v.is_empty() {
                    return v;
                }
            }
            let id = uuid::Uuid::now_v7().to_string();
            crate::runtime().block_on(crate::db::save_setting("device_uuid", &id));
            tracing::info!(%id, "生成设备 UUID(v7)");
            id
        })
        .clone()
}

#[cfg(test)]
mod tests {
    #[test]
    fn uuid_is_v7_and_stable() {
        let a = super::device_uuid();
        let b = super::device_uuid();
        assert_eq!(a, b, "同进程两次取值恒定");
        assert_eq!(a.len(), 36, "UUID 标准格式: {a}");
        // 版本位: 第 14 个字符(0-indexed) = '7'
        assert_eq!(a.as_bytes()[14], b'7', "v7 版本位: {a}");
    }
}
