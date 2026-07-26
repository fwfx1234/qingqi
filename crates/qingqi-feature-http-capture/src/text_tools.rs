use base64::{Engine as _, engine::general_purpose};

/// URL encode a string
pub fn url_encode(input: &str) -> String {
    urlencoding::encode(input).into_owned()
}

/// URL decode a string
pub fn url_decode(input: &str) -> Result<String, String> {
    urlencoding::decode(input)
        .map(|s| s.into_owned())
        .map_err(|e| format!("URL 解码失败: {e}"))
}

/// Base64 encode
pub fn base64_encode(input: &str) -> String {
    general_purpose::STANDARD.encode(input.as_bytes())
}

/// Base64 decode
pub fn base64_decode(input: &str) -> Result<String, String> {
    general_purpose::STANDARD.decode(input)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .map_err(|e| format!("Base64 解码失败: {e}"))
}

/// Format JSON string with pretty print
pub fn json_format(input: &str) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(input)
        .map_err(|e| format!("JSON 解析失败: {e}"))?;
    serde_json::to_string_pretty(&value)
        .map_err(|e| format!("JSON 格式化失败: {e}"))
}

/// Minify JSON string
pub fn json_minify(input: &str) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(input)
        .map_err(|e| format!("JSON 解析失败: {e}"))?;
    serde_json::to_string(&value)
        .map_err(|e| format!("JSON 压缩失败: {e}"))
}

/// Decode JWT payload (without verification)
pub fn jwt_decode(input: &str) -> Result<String, String> {
    let parts: Vec<&str> = input.split('.').collect();
    if parts.len() != 3 {
        return Err("无效的 JWT 格式".to_string());
    }
    let payload = base64_decode(parts[1])?;
    json_format(&payload)
}

/// Compute MD5 hash
pub fn md5_hash(input: &str) -> String {
    format!("{:x}", md5::compute(input.as_bytes()))
}

/// Compute SHA1 hash
pub fn sha1_hash(input: &str) -> String {
    use sha1::{Sha1, Digest};
    let mut hasher = Sha1::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Compute SHA256 hash
pub fn sha256_hash(input: &str) -> String {
    use sha2::Sha256;
    use sha2::Digest;
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Hex encode bytes to string
pub fn hex_encode(input: &str) -> String {
    input.as_bytes().iter().map(|b| format!("{:02x}", b)).collect()
}

/// Hex decode string to bytes
pub fn hex_decode(input: &str) -> Result<String, String> {
    let bytes = (0..input.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&input[i..i+2], 16))
        .collect::<Result<Vec<u8>, _>>()
        .map_err(|e| format!("Hex 解码失败: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("UTF-8 解码失败: {e}"))
}
