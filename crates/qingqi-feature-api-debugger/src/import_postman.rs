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

            let (normalized_url, query_rows) = crate::service::split_request_url(&url_str);
            let path_rows = crate::service::extract_path_parameter_names(&normalized_url)
                .into_iter()
                .map(|name| KeyValueRow::new(name, ""))
                .collect::<Vec<_>>();
            let mut snapshot = crate::model::RequestSnapshot {
                method: method.clone(),
                url: normalized_url,
                params_text: format_rows(&query_rows),
                path_params_text: format_rows(&path_rows),
                ..Default::default()
            };

            // Headers
            if let Some(headers) = request.get("header").and_then(|v| v.as_array()) {
                let mut rows = Vec::new();
                let mut cookies = Vec::new();
                for header in headers {
                    let key = header.get("key").and_then(|v| v.as_str()).unwrap_or("");
                    let value = header.get("value").and_then(|v| v.as_str()).unwrap_or("");
                    if key.eq_ignore_ascii_case("cookie") {
                        cookies.extend(value.split(';').filter_map(|pair| {
                            let (key, value) = pair.trim().split_once('=')?;
                            Some(format!("{}={}", key.trim(), value.trim()))
                        }));
                    } else if !key.is_empty() {
                        rows.push(KeyValueRow::new(key.to_string(), value.to_string()));
                    }
                }
                snapshot.headers_text = rows
                    .iter()
                    .map(|r| format!("{}={}", r.key, r.value))
                    .collect::<Vec<_>>()
                    .join("\n");
                snapshot.cookies_text = cookies.join("\n");
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
                                let rows = params
                                    .iter()
                                    .filter_map(|p| {
                                        let k = p.get("key").and_then(|v| v.as_str())?;
                                        let is_file = p.get("type").and_then(|v| v.as_str())
                                            == Some("file");
                                        let value = if is_file {
                                            p.get("src").and_then(|v| v.as_str()).unwrap_or("")
                                        } else {
                                            p.get("value").and_then(|v| v.as_str()).unwrap_or("")
                                        };
                                        let mut row = KeyValueRow::new(k, value);
                                        row.value_type = if is_file { "file" } else { "text" }.into();
                                        Some(row)
                                    })
                                    .collect::<Vec<_>>();
                                snapshot.body_text = format_rows(&rows);
                                snapshot.body_payloads.form_data = rows;
                            }
                        }
                        _ => {}
                    }
                }
            }
            if snapshot.body_payloads.is_empty() && !snapshot.body_text.is_empty() {
                snapshot.body_payloads = crate::model::RequestBody::migrate_legacy(
                    crate::model::BodyMode::from_db(&snapshot.body_mode),
                    &snapshot.body_text,
                );
            }

            // Auth
            if let Some(auth) = request.get("auth") {
                if let Some(auth_type) = auth.get("type").and_then(|v| v.as_str()) {
                    match auth_type {
                        "bearer" => {
                            if let Some(token) = find_auth_value(auth, "token") {
                                snapshot.auth_type = "bearer".into();
                                snapshot.auth_value = token;
                            }
                        }
                        "basic" => {
                            let user = find_auth_value(auth, "username").unwrap_or_default();
                            let pass = find_auth_value(auth, "password").unwrap_or_default();
                            if !user.is_empty() || !pass.is_empty() {
                                snapshot.auth_type = "basic".into();
                                snapshot.auth_value = crate::service::base64_encode(
                                    format!("{user}:{pass}").as_bytes(),
                                );
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

fn format_rows(rows: &[KeyValueRow]) -> String {
    rows.iter()
        .filter(|row| !row.key.is_empty())
        .map(|row| format!("{}={}", row.key, row.value))
        .collect::<Vec<_>>()
        .join("\n")
}

fn find_auth_value(auth: &Value, key: &str) -> Option<String> {
    if let Some(auth_type) = auth.get("type").and_then(|value| value.as_str())
        && let Some(values) = auth.get(auth_type).and_then(|value| value.as_array())
        && let Some(value) = values
            .iter()
            .find(|value| value.get("key").and_then(|value| value.as_str()) == Some(key))
    {
        return value
            .get("value")
            .and_then(|value| value.as_str())
            .map(str::to_string);
    }
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

    #[test]
    fn public_echo_fixture_covers_request_scenarios() {
        let content = include_str!("../examples/postman-echo-scenarios.postman_collection.json");
        let result = parse_postman(content).unwrap();

        assert_eq!(result.endpoints.len(), 14);
        assert!(result.endpoints.iter().all(|endpoint| {
            endpoint
                .parent_folder
                .as_deref()
                .is_some_and(|folder| folder == "基础请求" || folder == "请求体与方法")
        }));
        for method in ["GET", "POST", "PUT", "PATCH", "DELETE"] {
            assert!(
                result
                    .endpoints
                    .iter()
                    .any(|endpoint| endpoint.method == method)
            );
        }

        let basic_auth = result
            .endpoints
            .iter()
            .find(|endpoint| endpoint.name == "Basic Auth")
            .unwrap();
        assert_eq!(basic_auth.snapshot.auth_type, "basic");
        assert_eq!(basic_auth.snapshot.auth_value, "cG9zdG1hbjpwYXNzd29yZA==");

        let cookies = result
            .endpoints
            .iter()
            .find(|endpoint| endpoint.name == "Cookie")
            .unwrap();
        assert_eq!(
            cookies.snapshot.cookies_text,
            "session_id=qingqi-demo\ntheme=light"
        );

        let body_modes: Vec<&str> = result
            .endpoints
            .iter()
            .map(|endpoint| endpoint.snapshot.body_mode.as_str())
            .collect();
        assert!(body_modes.contains(&"json"));
        assert!(body_modes.contains(&"urlencoded"));
        assert!(body_modes.contains(&"form-data"));
    }
}
