//! SSH 连接诊断日志辅助

/// 将字节预览为可打印字符串（控制字符转义），用于 debug 原始协议数据
pub fn bytes_preview(data: &[u8], max_len: usize) -> String {
    let slice = if data.len() > max_len {
        &data[..max_len]
    } else {
        data
    };
    let mut out = String::with_capacity(slice.len() + 8);
    for &b in slice {
        match b {
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            0x20..=0x7e => out.push(b as char),
            _ => out.push_str(&format!("\\x{b:02x}")),
        }
    }
    if data.len() > max_len {
        out.push_str("…");
    }
    out
}
