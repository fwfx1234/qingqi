//! Postman Collection 导入
//!
//! 支持 Postman Collection v2.1 JSON 格式。
//! 解析 items 层级为 CollectionNode 树，request 字段为 RequestSnapshot。

use crate::import_openapi::ImportedCollection;
use crate::model::KeyValueRow;
use serde_json::Value;

/// 解析 Postman Collection v2.1 JSON
pub fn parse_postman(content: &str) -> Result<ImportedCollection, String> {
    let root: Value = serde_json::from_str(content).map_err(|e| format!("JSON 解析失败: {e}"))?;

    let info = root.get("info").unwrap_or(&Value::Null);
    let title = info
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Postman 导入")
        .to_string();

    let mut result = ImportedCollection {
        title,
        ..Default::default()
    };

    // 递归解析 items 树
    if let Some(items) = root.get("item").and_then(|v| v.as_array()) {
        parse_items(items, None, "", &mut result);
    }

    Ok(result)
}

fn parse_items(
    items: &[Value],
    parent_folder: Option<&str>,
    folder_prefix: &str,
    result: &mut ImportedCollection,
) {
    for item in items {
        let name = item
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("未命名")
            .to_string();

        // 检查是否有子 item（即这是文件夹）
        if let Some(children) = item.get("item").and_then(|v| v.as_array()) {
            let folder_name = if folder_prefix.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", folder_prefix, name)
            };
            parse_items(children, Some(&folder_name), &folder_name, result);
            continue;
        }

        // 这是端点
        if let Some(request) = item.get("request") {
            let method = request
                .get("method")
                .and_then(|v| v.as_str())
                .unwrap_or("GET")
                .to_uppercase();

            let url_str = extract_postman_url(request);

            let mut snapshot = crate::model::RequestSnapshot {
                method: method.clone(),
                url: url_str.clone(),
                ..Default::default()
            };

            // Headers
            if let Some(headers) = request.get("header").and_then(|v| v.as_array()) {
                let rows: Vec<KeyValueRow> = headers
                    .iter()
                    .filter_map(|h| {
                        let key = h.get("key").and_then(|v| v.as_str()).unwrap_or("");
                        let val = h.get("value").and_then(|v| v.as_str()).unwrap_or("");
                        if key.is_empty() {
                            None
                        } else {
                            Some(KeyValueRow::new(key.to_string(), val.to_string()))
                        }
                    })
                    .collect();
                snapshot.headers_text = rows
                    .iter()
                    .map(|r| format!("{}={}", r.key, r.value))
                    .collect::<Vec<_>>()
                    .join("\n");
            }

            // Body
            if let Some(body) = request.get("body") {
                if let Some(mode) = body.get("mode").and_then(|v| v.as_str()) {
                    match mode {
                        "raw" => {
                            snapshot.body_text = body
                                .get("raw")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            // 检测 JSON
                            let trimmed = snapshot.body_text.trim();
                            if (trimmed.starts_with('{') && trimmed.ends_with('}'))
                                || (trimmed.starts_with('[') && trimmed.ends_with(']'))
                            {
                                snapshot.body_mode = "json".into();
                            } else {
                                snapshot.body_mode = "text".into();
                            }
                        }
                        "urlencoded" => {
                            snapshot.body_mode = "urlencoded".into();
                            if let Some(params) = body.get("urlencoded").and_then(|v| v.as_array())
                            {
                                snapshot.body_text = params
                                    .iter()
                                    .filter_map(|p| {
                                        let k = p.get("key").and_then(|v| v.as_str())?;
                                        let v =
                                            p.get("value").and_then(|v| v.as_str()).unwrap_or("");
                                        Some(format!("{k}={v}"))
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n");
                            }
                        }
                        "formdata" => {
                            snapshot.body_mode = "form-data".into();
                            if let Some(params) = body.get("formdata").and_then(|v| v.as_array()) {
                                snapshot.body_text = params
                                    .iter()
                                    .filter_map(|p| {
                                        let k = p.get("key").and_then(|v| v.as_str())?;
                                        let v =
                                            p.get("value").and_then(|v| v.as_str()).unwrap_or("");
                                        Some(format!("{k}={v}"))
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n");
                            }
                        }
                        _ => {}
                    }
                }
            }

            // Auth
            if let Some(auth) = request.get("auth") {
                if let Some(auth_type) = auth.get("type").and_then(|v| v.as_str()) {
                    match auth_type {
                        "bearer" => {
                            if let Some(token) = find_auth_value(auth, "token") {
                                let rows = parse_or_empty(&snapshot.headers_text);
                                let mut new_rows = rows;
                                let has_auth = new_rows
                                    .iter()
                                    .any(|r| r.key.eq_ignore_ascii_case("Authorization"));
                                if !has_auth {
                                    new_rows.push(KeyValueRow::new(
                                        "Authorization".to_string(),
                                        format!("Bearer {token}"),
                                    ));
                                    snapshot.headers_text = new_rows
                                        .iter()
                                        .map(|r| format!("{}={}", r.key, r.value))
                                        .collect::<Vec<_>>()
                                        .join("\n");
                                }
                            }
                        }
                        "basic" => {
                            let user = find_auth_value(auth, "username").unwrap_or_default();
                            let pass = find_auth_value(auth, "password").unwrap_or_default();
                            let rows = parse_or_empty(&snapshot.headers_text);
                            let has_auth = rows
                                .iter()
                                .any(|r| r.key.eq_ignore_ascii_case("Authorization"));
                            if !has_auth && (!user.is_empty() || !pass.is_empty()) {
                                let mut new_rows = rows;
                                new_rows.push(KeyValueRow::new(
                                    "Authorization".to_string(),
                                    format!("Basic {}:{}", user, pass),
                                ));
                                snapshot.headers_text = new_rows
                                    .iter()
                                    .map(|r| format!("{}={}", r.key, r.value))
                                    .collect::<Vec<_>>()
                                    .join("\n");
                            }
                        }
                        _ => {}
                    }
                }
            }

            result
                .endpoints
                .push(crate::import_openapi::ImportedEndpoint {
                    name,
                    method,
                    url: url_str,
                    parent_folder: parent_folder.map(|s| s.to_string()),
                    snapshot,
                });
        }
    }
}

fn extract_postman_url(request: &Value) -> String {
    if let Some(url) = request.get("url") {
        if let Some(raw) = url.get("raw").and_then(|v| v.as_str()) {
            return raw.to_string();
        }
        // 组合 url 对象
        let host = url
            .get("host")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(".")
            })
            .unwrap_or_default();
        let path = url
            .get("path")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_default();
        let protocol = url
            .get("protocol")
            .and_then(|v| v.as_str())
            .unwrap_or("https");
        let port = url.get("port").and_then(|v| v.as_str()).unwrap_or("");
        if !host.is_empty() {
            let port_str = if port.is_empty() {
                String::new()
            } else {
                format!(":{port}")
            };
            if path.is_empty() {
                format!("{protocol}://{host}{port_str}")
            } else {
                format!("{protocol}://{host}{port_str}/{path}")
            }
        } else {
            String::new()
        }
    } else {
        String::new()
    }
}

fn find_auth_value(auth: &Value, key: &str) -> Option<String> {
    if let Some(arr) = auth.get(key).and_then(|v| v.as_array()) {
        return arr
            .first()
            .and_then(|v| v.get("value"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
    }
    // 另一种格式: key 直接在 auth 对象上
    auth.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn parse_or_empty(text: &str) -> Vec<KeyValueRow> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| KeyValueRow::new(k.trim().to_string(), v.trim().to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_postman_basic() {
        let content = r#"{
          "info": { "name": "My API" },
          "item": [
            {
              "name": "用户模块",
              "item": [
                {
                  "name": "获取用户列表",
                  "request": {
                    "method": "GET",
                    "url": { "raw": "https://api.example.com/users?page=1" },
                    "header": [
                      { "key": "Content-Type", "value": "application/json" }
                    ]
                  }
                },
                {
                  "name": "创建用户",
                  "request": {
                    "method": "POST",
                    "url": { "raw": "https://api.example.com/users" },
                    "header": [
                      { "key": "Content-Type", "value": "application/json" }
                    ],
                    "body": {
                      "mode": "raw",
                      "raw": "{\"name\":\"test\"}"
                    }
                  }
                }
              ]
            },
            {
              "name": "健康检查",
              "request": {
                "method": "GET",
                "url": { "raw": "https://api.example.com/health" }
              }
            }
          ]
        }"#;

        let result = parse_postman(content).unwrap();
        assert_eq!(result.title, "My API");
        assert_eq!(result.endpoints.len(), 3);

        // 第一个端点应该在 "用户模块" 下
        let user_ep = result.endpoints.first().unwrap();
        assert_eq!(user_ep.parent_folder.as_deref(), Some("用户模块"));
        assert_eq!(user_ep.snapshot.method, "GET");

        // POST 端点应该有 body
        let post_ep = result.endpoints.get(1).unwrap();
        assert_eq!(post_ep.snapshot.method, "POST");
        assert!(!post_ep.snapshot.body_text.is_empty());
    }
}
