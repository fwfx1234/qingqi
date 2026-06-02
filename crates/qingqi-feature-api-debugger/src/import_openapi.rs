//! OpenAPI / Swagger 导入
//!
//! 支持 OpenAPI 3.x 和 Swagger 2.0 的 JSON/YAML 格式。
//! 解析结果为 `ImportedCollection`，包含模块树、端点和环境信息。

use crate::model::{KeyValueRow, RequestSnapshot};
use serde_json::Value;

/// 导入后的一条端点记录
#[derive(Clone, Debug)]
pub struct ImportedEndpoint {
    pub name: String,
    pub method: String,
    pub url: String,
    pub parent_folder: Option<String>,
    pub snapshot: RequestSnapshot,
}

/// 导入结果
#[derive(Clone, Debug, Default)]
pub struct ImportedCollection {
    pub endpoints: Vec<ImportedEndpoint>,
    pub base_urls: Vec<String>,
    pub title: String,
}

/// 解析 OpenAPI/Swagger 内容（自动检测 JSON 或 YAML）
pub fn parse_openapi(content: &str) -> Result<ImportedCollection, String> {
    let value: Value = if content.trim().starts_with('{') || content.trim().starts_with('[') {
        serde_json::from_str(content).map_err(|e| format!("JSON 解析失败: {e}"))?
    } else {
        serde_yaml::from_str(content).map_err(|e| format!("YAML 解析失败: {e}"))?
    };

    let mut result = ImportedCollection::default();

    // 标题
    result.title = value
        .get("info")
        .and_then(|i| i.get("title"))
        .and_then(|v| v.as_str())
        .unwrap_or("导入的 API")
        .to_string();

    // OpenAPI 3.x servers
    if let Some(servers) = value.get("servers").and_then(|v| v.as_array()) {
        for server in servers {
            if let Some(url) = server.get("url").and_then(|v| v.as_str()) {
                result.base_urls.push(url.to_string());
            }
        }
    }

    // Swagger 2.0: host + basePath + schemes
    if result.base_urls.is_empty() {
        let host = value
            .get("host")
            .and_then(|v| v.as_str())
            .unwrap_or("localhost");
        let base_path = value
            .get("basePath")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let schemes = value
            .get("schemes")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|s| s.as_str())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| vec!["http"]);
        let scheme = schemes.first().unwrap_or(&"http");
        result
            .base_urls
            .push(format!("{scheme}://{host}{base_path}"));
    }

    // 构建 tag → folder 映射
    let mut tag_order: Vec<String> = Vec::new();
    if let Some(tags) = value.get("tags").and_then(|v| v.as_array()) {
        for tag in tags {
            if let Some(name) = tag.get("name").and_then(|v| v.as_str()) {
                tag_order.push(name.to_string());
            }
        }
    }

    // 解析 paths
    let paths = value.get("paths");
    if paths.is_none() || !paths.unwrap().is_object() {
        return Ok(result);
    }

    let path_obj = paths.unwrap().as_object().unwrap();
    for (path, methods) in path_obj {
        let methods_obj = methods.as_object().ok_or("paths 格式错误")?;

        for (method, detail) in methods_obj {
            if method == "parameters" || method == "summary" || method == "description" {
                continue;
            }

            let mut snapshot = RequestSnapshot {
                method: method.to_uppercase(),
                url: path.clone(),
                ..Default::default()
            };

            // summary / operationId → 名称
            let name = detail
                .get("summary")
                .and_then(|v| v.as_str())
                .or_else(|| detail.get("operationId").and_then(|v| v.as_str()))
                .unwrap_or(&method)
                .to_string();

            // tags → parent_folder（只取第一个 tag）
            let folder = detail
                .get("tags")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            // 路径参数
            if let Some(params) = detail.get("parameters").and_then(|v| v.as_array()) {
                let mut query_pairs = Vec::new();
                let mut path_pairs = Vec::new();
                let mut header_pairs = Vec::new();
                for param in params {
                    let key = param.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let val = param
                        .get("example")
                        .or_else(|| param.get("default"))
                        .and_then(|v| {
                            if v.is_string() {
                                Some(v.as_str().unwrap().to_string())
                            } else {
                                Some(v.to_string())
                            }
                        })
                        .unwrap_or_default();
                    let location = param.get("in").and_then(|v| v.as_str()).unwrap_or("query");
                    match location {
                        "path" => path_pairs.push(KeyValueRow::new(key, val)),
                        "header" => header_pairs.push(KeyValueRow::new(key, val)),
                        _ => query_pairs.push(KeyValueRow::new(key, val)),
                    }
                }
                snapshot.params_text = format_rows(&query_pairs);
                snapshot.path_params_text = format_rows(&path_pairs);
                // 合并 headers
                let existing_hdrs = parse_rows(&snapshot.headers_text);
                let all_hdrs: Vec<_> = existing_hdrs
                    .into_iter()
                    .chain(header_pairs)
                    .collect();
                snapshot.headers_text = format_rows(&all_hdrs);
            }

            // requestBody (OpenAPI 3.x)
            if let Some(body) = detail.get("requestBody") {
                if let Some(content) = body.get("content") {
                    if let Some(json_media) = content.get("application/json") {
                        if let Some(example) = extract_example(json_media) {
                            snapshot.body_text = example;
                            snapshot.body_mode = "json".into();
                        }
                    } else if let Some(form_media) = content.get("multipart/form-data") {
                        snapshot.body_mode = "form-data".into();
                    }
                }
            }

            // Swagger 2.0: parameters 中的 body
            if snapshot.body_text.is_empty() {
                if let Some(params) = detail.get("parameters").and_then(|v| v.as_array()) {
                    for param in params {
                        if param.get("in").and_then(|v| v.as_str()) == Some("body") {
                            if let Some(schema) = param.get("schema") {
                                if let Some(example) = extract_example(schema) {
                                    snapshot.body_text = example;
                                    snapshot.body_mode = "json".into();
                                }
                            }
                        }
                    }
                }
            }

            // headers (从 security / produces)
            if let Some(produces) = value.get("produces").and_then(|v| v.as_array()) {
                if let Some(first) = produces.first().and_then(|v| v.as_str()) {
                    let existing = parse_rows(&snapshot.headers_text);
                    let has_ct = existing.iter().any(|r| r.key.eq_ignore_ascii_case("Content-Type"));
                    if !has_ct {
                        let mut new_hdrs = existing;
                        new_hdrs.push(KeyValueRow::new("Content-Type", first.to_string()));
                        snapshot.headers_text = format_rows(&new_hdrs);
                    }
                }
            }

            result.endpoints.push(ImportedEndpoint {
                name,
                method: method.to_uppercase(),
                url: path.clone(),
                parent_folder: folder,
                snapshot,
            });
        }
    }

    Ok(result)
}

/// 提取 JSON 示例（优先取 example，其次取 schema 生成的默认值）
fn extract_example(node: &Value) -> Option<String> {
    if let Some(example) = node.get("example") {
        if example.is_string() {
            return Some(example.as_str().unwrap().to_string());
        }
        return Some(serde_json::to_string_pretty(example).unwrap_or_default());
    }
    if let Some(examples) = node.get("examples") {
        if let Some(first) = examples.as_object().and_then(|o| o.values().next()) {
            if let Some(value) = first.get("value") {
                return Some(serde_json::to_string_pretty(value).unwrap_or_default());
            }
        }
    }
    // 从 schema 生成默认示例
    if let Some(schema) = node.get("schema") {
        return generate_example_from_schema(schema);
    }
    None
}

/// 从 JSON Schema 生成示例数据
fn generate_example_from_schema(schema: &Value) -> Option<String> {
    let obj = match schema.get("properties") {
        Some(props) => {
            let mut map = serde_json::Map::new();
            if let Some(props_obj) = props.as_object() {
                for (key, prop) in props_obj {
                    let example_val = match prop.get("type").and_then(|v| v.as_str()) {
                        Some("string") => {
                            let eg = prop.get("example").and_then(|v| v.as_str()).unwrap_or("");
                            Value::String(if eg.is_empty() { "string" } else { eg }.to_string())
                        }
                        Some("integer") => Value::Number(
                            prop.get("example")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0)
                                .into(),
                        ),
                        Some("number") => {
                            if let Some(n) = prop.get("example").and_then(|v| v.as_f64()) {
                                serde_json::Number::from_f64(n).map(Value::Number).unwrap_or(Value::Number(0.into()))
                            } else {
                                Value::Number(0.into())
                            }
                        }
                        Some("boolean") => {
                            Value::Bool(prop.get("example").and_then(|v| v.as_bool()).unwrap_or(false))
                        }
                        Some("array") => Value::Array(vec![]),
                        Some("object") => {
                            generate_example_from_schema(prop)
                                .and_then(|s| serde_json::from_str(&s).ok())
                                .unwrap_or(Value::Object(serde_json::Map::new()))
                        }
                        _ => Value::String("".into()),
                    };
                    map.insert(key.clone(), example_val);
                }
            }
            Value::Object(map)
        }
        None => {
            // 没有 properties，按类型给默认值
            match schema.get("type").and_then(|v| v.as_str()) {
                Some("object") => Value::Object(serde_json::Map::new()),
                Some("array") => Value::Array(vec![]),
                Some("string") => Value::String("".into()),
                _ => Value::Object(serde_json::Map::new()),
            }
        }
    };
    Some(serde_json::to_string_pretty(&obj).unwrap_or_else(|_| "{}".into()))
}

fn parse_rows(text: &str) -> Vec<KeyValueRow> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| KeyValueRow::new(k.trim().to_string(), v.trim().to_string()))
        .collect()
}

fn format_rows(rows: &[KeyValueRow]) -> String {
    rows.iter()
        .filter(|r| !r.key.is_empty())
        .map(|r| format!("{}={}", r.key, r.value))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_openapi_json() {
        let content = r#"{
          "openapi": "3.0.0",
          "info": { "title": "Test API", "version": "1.0" },
          "servers": [ { "url": "https://api.example.com/v1" } ],
          "paths": {
            "/users": {
              "get": {
                "summary": "获取用户列表",
                "tags": ["用户管理"],
                "parameters": [
                  { "name": "page", "in": "query", "example": "1" },
                  { "name": "size", "in": "query", "example": "20" }
                ]
              },
              "post": {
                "summary": "创建用户",
                "tags": ["用户管理"],
                "requestBody": {
                  "content": {
                    "application/json": {
                      "schema": {
                        "type": "object",
                        "properties": {
                          "name": { "type": "string" },
                          "email": { "type": "string" }
                        }
                      }
                    }
                  }
                }
              }
            },
            "/health": {
              "get": {
                "summary": "健康检查"
              }
            }
          }
        }"#;

        let result = parse_openapi(content).unwrap();
        assert_eq!(result.title, "Test API");
        assert_eq!(result.base_urls, vec!["https://api.example.com/v1"]);
        assert_eq!(result.endpoints.len(), 3);

        let get_users = result.endpoints.iter().find(|e| e.method == "GET" && e.url == "/users").unwrap();
        assert_eq!(get_users.name, "获取用户列表");
        assert_eq!(get_users.parent_folder.as_deref(), Some("用户管理"));
        assert!(get_users.snapshot.params_text.contains("page=1"));

        let post_users = result.endpoints.iter().find(|e| e.method == "POST").unwrap();
        assert!(!post_users.snapshot.body_text.is_empty());

        let health = result.endpoints.iter().find(|e| e.url == "/health").unwrap();
        assert!(health.parent_folder.is_none());
    }

    #[test]
    fn test_parse_swagger_20() {
        let content = r#"{
          "swagger": "2.0",
          "info": { "title": "Legacy API" },
          "host": "api.legacy.com",
          "basePath": "/v2",
          "schemes": ["https"],
          "paths": {
            "/items": {
              "get": {
                "operationId": "listItems",
                "parameters": [
                  { "name": "limit", "in": "query", "default": "10" }
                ]
              }
            }
          }
        }"#;

        let result = parse_openapi(content).unwrap();
        assert_eq!(result.title, "Legacy API");
        assert_eq!(result.base_urls, vec!["https://api.legacy.com/v2"]);
        assert_eq!(result.endpoints.len(), 1);
        assert_eq!(result.endpoints[0].name, "listItems");
    }
}
