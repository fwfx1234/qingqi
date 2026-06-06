//! cURL 命令解析器
//!
//! 将 cURL 命令字符串解析为结构化的请求参数。

use crate::model::{BodyMode, KeyValueRow};

/// cURL 解析结果
#[derive(Clone, Debug, Default)]
pub struct CurlParsed {
    pub method: String,
    pub url: String,
    pub headers: Vec<KeyValueRow>,
    pub body: String,
    pub body_mode: BodyMode,
    pub form_data: Vec<KeyValueRow>,
    pub auth_type: String, // "basic" / "bearer"
    pub auth_value: String,
    pub cookies: Vec<KeyValueRow>,
}

/// 解析 cURL 命令字符串
///
/// 支持的参数：
/// - `-X` / `--request`: HTTP 方法
/// - `-H` / `--header`: 请求头
/// - `-d` / `--data` / `--data-raw`: 请求体
/// - `--data-binary`: 二进制请求体
/// - `-F` / `--form`: form-data
/// - `-u` / `--user`: Basic Auth
/// - `-b` / `--cookie`: Cookie
/// - URL 作为位置参数
pub fn parse_curl(input: &str) -> Result<CurlParsed, String> {
    let tokens = tokenize(input);
    let mut result = CurlParsed::default();

    // 跳过 "curl" 命令本身
    let mut i = 0;
    while i < tokens.len() {
        let token = &tokens[i];

        match token.as_str() {
            // 跳过 curl 命令名
            t if i == 0 && (t == "curl" || t == "curl.exe") => {
                i += 1;
                continue;
            }

            // 方法
            "-X" | "--request" => {
                if i + 1 < tokens.len() {
                    result.method = tokens[i + 1].to_uppercase();
                    i += 2;
                } else {
                    i += 1;
                }
            }

            // 请求头
            "-H" | "--header" => {
                if i + 1 < tokens.len() {
                    if let Some(kv) = parse_header(&tokens[i + 1]) {
                        result.headers.push(kv);
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            }

            // 请求体 (--data / -d)
            "--data" | "--data-raw" | "-d" => {
                if i + 1 < tokens.len() {
                    result.body = tokens[i + 1].clone();
                    if result.body_mode == BodyMode::None {
                        result.body_mode = BodyMode::Text; // 默认 text，后续可能检测为 JSON
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            }

            // 二进制请求体
            "--data-binary" => {
                if i + 1 < tokens.len() {
                    let val = &tokens[i + 1];
                    if val.starts_with('@') {
                        // 文件路径
                        result.body = val[1..].to_string();
                        result.body_mode = BodyMode::Binary;
                    } else {
                        result.body = val.clone();
                        result.body_mode = BodyMode::Binary;
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            }

            // form-data
            "-F" | "--form" => {
                if i + 1 < tokens.len() {
                    if let Some(kv) = parse_form_field(&tokens[i + 1]) {
                        result.form_data.push(kv);
                    }
                    if result.body_mode == BodyMode::None {
                        result.body_mode = BodyMode::FormData;
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            }

            // Basic Auth
            "-u" | "--user" => {
                if i + 1 < tokens.len() {
                    result.auth_type = "basic".to_string();
                    result.auth_value = tokens[i + 1].clone();
                    i += 2;
                } else {
                    i += 1;
                }
            }

            // Cookie
            "-b" | "--cookie" => {
                if i + 1 < tokens.len() {
                    let cookie_str = &tokens[i + 1];
                    for part in cookie_str.split(';') {
                        let part = part.trim();
                        if let Some((k, v)) = part.split_once('=') {
                            result
                                .cookies
                                .push(KeyValueRow::new(k.trim().to_string(), v.trim().to_string()));
                        }
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            }

            // 压缩标志 (忽略)
            "--compressed" => {
                i += 1;
            }

            // 静默标志 (忽略)
            "-s" | "--silent" | "-S" | "--show-error" | "-sS" => {
                i += 1;
            }

            // URL (位置参数)
            // 不以 - 开头且不是前一个参数的值
            _ => {
                if !token.starts_with('-') && result.url.is_empty() {
                    // 检查是否可能是 URL
                    if token.starts_with("http://")
                        || token.starts_with("https://")
                        || token.starts_with("localhost")
                        || token.starts_with("127.")
                        || token.starts_with("::1")
                    {
                        result.url = token.clone();
                    }
                }
                i += 1;
            }
        }
    }

    // 如果方法为空，默认为 GET（如果有 body 则为 POST）
    if result.method.is_empty() {
        if result.body.is_empty() && result.form_data.is_empty() {
            result.method = "GET".to_string();
        } else {
            result.method = "POST".to_string();
        }
    }

    // 如果 body 非空且 body_mode 为 Text，尝试检测 JSON
    if result.body_mode == BodyMode::Text && !result.body.is_empty() {
        let trimmed = result.body.trim();
        if (trimmed.starts_with('{') && trimmed.ends_with('}'))
            || (trimmed.starts_with('[') && trimmed.ends_with(']'))
        {
            result.body_mode = BodyMode::Json;
        }
    }

    // 如果 body_mode 为 Text 但无内容，改为 None
    if result.body_mode == BodyMode::Text && result.body.is_empty() {
        result.body_mode = BodyMode::None;
    }

    Ok(result)
}

/// 将 cURL 命令字符串分割为 token 列表，处理引号
fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    let len = chars.len();

    while i < len {
        // 跳过空白
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }

        // 处理行继续符 \
        if chars[i] == '\\' && i + 1 < len && chars[i + 1] == '\n' {
            i += 2;
            continue;
        }
        if chars[i] == '\\' && i + 1 < len && chars[i + 1] == '\r' {
            i += 2;
            if i < len && chars[i] == '\n' {
                i += 1;
            }
            continue;
        }
        // 行尾的 \ (Windows 风格或行尾空格)
        if chars[i] == '\\' && (i + 1 == len || (i + 1 < len && chars[i + 1].is_whitespace())) {
            let mut j = i + 1;
            while j < len && chars[j].is_whitespace() && chars[j] != '\n' && chars[j] != '\r' {
                j += 1;
            }
            if j < len && (chars[j] == '\n' || chars[j] == '\r') {
                i = j + 1;
                if i < len && chars[i - 1] == '\r' && chars[i] == '\n' {
                    i += 1;
                }
                continue;
            }
        }

        // 引号字符串
        if chars[i] == '"' || chars[i] == '\'' {
            let quote = chars[i];
            i += 1;
            let mut s = String::new();
            while i < len && chars[i] != quote {
                if chars[i] == '\\' && i + 1 < len {
                    i += 1;
                    s.push(chars[i]);
                } else {
                    s.push(chars[i]);
                }
                i += 1;
            }
            if i < len {
                i += 1; // skip closing quote
            }
            tokens.push(s);
            continue;
        }

        // 普通 token (非空白、非引号)
        let mut s = String::new();
        while i < len && !chars[i].is_whitespace() && chars[i] != '"' && chars[i] != '\'' {
            s.push(chars[i]);
            i += 1;
        }
        tokens.push(s);
    }

    tokens
}

/// 解析 header: "Key: Value" 格式
fn parse_header(s: &str) -> Option<KeyValueRow> {
    let s = s.trim();
    if let Some(pos) = s.find(": ") {
        let key = s[..pos].trim().to_string();
        let value = s[pos + 2..].trim().to_string();
        Some(KeyValueRow::new(key, value))
    } else if let Some(pos) = s.find(':') {
        let key = s[..pos].trim().to_string();
        let value = s[pos + 1..].trim().to_string();
        Some(KeyValueRow::new(key, value))
    } else {
        None
    }
}

/// 解析 form field: "key=value" 或 "key=@/path/to/file" 格式
fn parse_form_field(s: &str) -> Option<KeyValueRow> {
    let s = s.trim();
    // 处理 "name=value" 或 "name=@filepath"
    if let Some(pos) = s.find('=') {
        let key = s[..pos].trim().to_string();
        let value = s[pos + 1..].trim().to_string();
        // 去掉可能的引号包裹
        let value = value.trim_matches('"').trim_matches('\'').to_string();
        Some(KeyValueRow::new(key, value))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_get() {
        let result = parse_curl("curl https://api.example.com/users").unwrap();
        assert_eq!(result.method, "GET");
        assert_eq!(result.url, "https://api.example.com/users");
    }

    #[test]
    fn test_parse_post_json() {
        let input = r#"curl -X POST https://api.example.com/users -H "Content-Type: application/json" -H "Authorization: Bearer token123" -d '{"name":"test","age":25}'"#;
        let result = parse_curl(input).unwrap();
        assert_eq!(result.method, "POST");
        assert_eq!(result.url, "https://api.example.com/users");
        assert_eq!(result.headers.len(), 2);
        assert_eq!(result.body, r#"{"name":"test","age":25}"#);
        assert_eq!(result.body_mode, BodyMode::Json);
    }

    #[test]
    fn test_parse_form_data() {
        let input = r#"curl -X POST https://api.example.com/upload -F "name=test" -F "file=@/tmp/data.csv""#;
        let result = parse_curl(input).unwrap();
        assert_eq!(result.method, "POST");
        assert_eq!(result.form_data.len(), 2);
        assert_eq!(result.body_mode, BodyMode::FormData);
        assert_eq!(result.form_data[0].key, "name");
        assert_eq!(result.form_data[0].value, "test");
        assert_eq!(result.form_data[1].key, "file");
        assert_eq!(result.form_data[1].value, "@/tmp/data.csv");
    }

    #[test]
    fn test_parse_basic_auth() {
        let result = parse_curl("curl -u admin:pass123 https://api.example.com/admin").unwrap();
        assert_eq!(result.auth_type, "basic");
        assert_eq!(result.auth_value, "admin:pass123");
    }

    #[test]
    fn test_parse_binary() {
        let result =
            parse_curl("curl --data-binary @/tmp/image.png https://api.example.com/upload")
                .unwrap();
        assert_eq!(result.body, "/tmp/image.png");
        assert_eq!(result.body_mode, BodyMode::Binary);
    }

    #[test]
    fn test_parse_cookies() {
        let result =
            parse_curl(r#"curl -b "session=abc123; token=xyz" https://api.example.com/me"#)
                .unwrap();
        assert_eq!(result.cookies.len(), 2);
        assert_eq!(result.cookies[0].key, "session");
        assert_eq!(result.cookies[1].key, "token");
    }

    #[test]
    fn test_parse_multiline_curl() {
        let input = r#"curl -X POST \
  https://api.example.com/users \
  -H "Content-Type: application/json" \
  -d '{"name":"test"}'"#;
        let result = parse_curl(input).unwrap();
        assert_eq!(result.method, "POST");
        assert_eq!(result.url, "https://api.example.com/users");
        assert_eq!(result.headers.len(), 1);
    }

    #[test]
    fn test_auto_detect_post_from_body() {
        // 没有显式 -X 但有 -d，应自动推断为 POST
        let result =
            parse_curl(r#"curl https://api.example.com/users -d '{"name":"test"}'"#).unwrap();
        assert_eq!(result.method, "POST");
    }
}
