use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsonMode {
    Format,
    Compact,
    Validate,
    Query,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonStats {
    pub char_count: usize,
    pub line_count: usize,
    pub kind: String,
    pub size: usize,
    pub depth: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonError {
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub phase: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonResult {
    pub output: String,
    pub status: String,
    pub error: Option<JsonError>,
    pub stats: Option<JsonStats>,
}

pub fn run(input: &str, query: &str, mode: JsonMode) -> JsonResult {
    let input = input.trim();
    if input.is_empty() {
        return JsonResult {
            output: String::new(),
            status: String::from("请输入 JSON"),
            error: None,
            stats: None,
        };
    }

    let value = match serde_json::from_str::<Value>(input) {
        Ok(value) => value,
        Err(error) => {
            return JsonResult {
                output: String::new(),
                status: String::from("格式无效"),
                error: Some(JsonError {
                    message: format!("JSON 解析错误: {error}"),
                    line: error.line(),
                    column: error.column(),
                    phase: "parse",
                }),
                stats: None,
            };
        }
    };

    match mode {
        JsonMode::Format => result_with_value(&value, &value, String::from("格式化完成"), true),
        JsonMode::Compact => result_with_value(&value, &value, String::from("压缩完成"), false),
        JsonMode::Validate => result_with_value(&value, &value, String::from("验证通过"), true),
        JsonMode::Query => query_value(&value, query),
    }
}

fn query_value(value: &Value, query: &str) -> JsonResult {
    let query = query.trim();
    if query.is_empty() {
        return result_with_value(value, value, String::from("格式化完成"), true);
    }

    let segments = match parse_query(query) {
        Ok(segments) => segments,
        Err(error) => {
            return JsonResult {
                output: String::new(),
                status: String::from("查询错误"),
                error: Some(JsonError {
                    message: format!("查询错误: {error}"),
                    line: 0,
                    column: 0,
                    phase: "query",
                }),
                stats: None,
            };
        }
    };

    match resolve_segments(value, &segments) {
        Ok(result) => result_with_value(result, result, String::from("查询完成"), true),
        Err(error) => JsonResult {
            output: String::new(),
            status: String::from("查询无结果"),
            error: Some(JsonError {
                message: format!("查询错误: {error}"),
                line: 0,
                column: 0,
                phase: "query",
            }),
            stats: None,
        },
    }
}

fn result_with_value(source: &Value, target: &Value, status: String, pretty: bool) -> JsonResult {
    let output = if pretty {
        serde_json::to_string_pretty(target).unwrap_or_else(|_| source.to_string())
    } else {
        serde_json::to_string(target).unwrap_or_else(|_| source.to_string())
    };

    JsonResult {
        stats: Some(JsonStats {
            char_count: output.chars().count(),
            line_count: output.lines().count(),
            kind: value_kind(target).to_string(),
            size: value_size(target),
            depth: value_depth(target),
        }),
        output,
        status,
        error: None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum QuerySegment {
    Key(String),
    Index(usize),
}

fn parse_query(query: &str) -> Result<Vec<QuerySegment>, String> {
    if query == "$" || query == "/" {
        return Ok(Vec::new());
    }

    if query.starts_with('/') {
        return Ok(query
            .split('/')
            .skip(1)
            .filter(|segment| !segment.is_empty())
            .map(parse_slash_segment)
            .collect());
    }

    let normalized = query
        .strip_prefix("$.")
        .or_else(|| query.strip_prefix('.'))
        .or_else(|| query.strip_prefix('$'))
        .unwrap_or(query);

    let mut chars = normalized.chars().peekable();
    let mut token = String::new();
    let mut segments = Vec::new();

    while let Some(ch) = chars.next() {
        match ch {
            '.' => {
                if !token.is_empty() {
                    segments.push(QuerySegment::Key(std::mem::take(&mut token)));
                }
            }
            '[' => {
                if !token.is_empty() {
                    segments.push(QuerySegment::Key(std::mem::take(&mut token)));
                }
                let mut raw_index = String::new();
                let mut closed = false;
                for next in chars.by_ref() {
                    if next == ']' {
                        closed = true;
                        break;
                    }
                    raw_index.push(next);
                }
                if !closed {
                    return Err(String::from("缺少 ]"));
                }
                let index = raw_index
                    .parse::<usize>()
                    .map_err(|_| format!("无效数组下标: {raw_index}"))?;
                segments.push(QuerySegment::Index(index));
            }
            ']' => return Err(String::from("多余的 ]")),
            _ => token.push(ch),
        }
    }

    if !token.is_empty() {
        segments.push(QuerySegment::Key(token));
    }

    if segments.is_empty() {
        return Err(String::from("查询语法不能为空"));
    }

    Ok(segments)
}

fn parse_slash_segment(segment: &str) -> QuerySegment {
    segment
        .parse::<usize>()
        .map(QuerySegment::Index)
        .unwrap_or_else(|_| QuerySegment::Key(segment.to_string()))
}

fn resolve_segments<'a>(value: &'a Value, segments: &[QuerySegment]) -> Result<&'a Value, String> {
    let mut current = value;
    for segment in segments {
        match segment {
            QuerySegment::Key(key) => {
                current = current
                    .get(key)
                    .ok_or_else(|| format!("对象字段不存在: {key}"))?;
            }
            QuerySegment::Index(index) => {
                current = current
                    .get(*index)
                    .ok_or_else(|| format!("数组下标越界: {index}"))?;
            }
        }
    }
    Ok(current)
}

fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Object(_) => "object",
        Value::Array(_) => "array",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Null => "null",
    }
}

fn value_size(value: &Value) -> usize {
    match value {
        Value::Object(map) => map.len(),
        Value::Array(list) => list.len(),
        Value::String(text) => text.chars().count(),
        _ => 0,
    }
}

fn value_depth(value: &Value) -> usize {
    match value {
        Value::Object(map) => 1 + map.values().map(value_depth).max().unwrap_or(0),
        Value::Array(list) => 1 + list.iter().map(value_depth).max().unwrap_or(0),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_json() {
        let result = run(r#"{"a":1}"#, "", JsonMode::Format);
        assert_eq!(result.status, "格式化完成");
        assert!(result.output.contains("\"a\": 1"));
        assert_eq!(
            result.stats.as_ref().map(|stats| stats.kind.as_str()),
            Some("object")
        );
    }

    #[test]
    fn compacts_json() {
        let result = run("{\n  \"a\": 1\n}", "", JsonMode::Compact);
        assert_eq!(result.output, r#"{"a":1}"#);
        assert_eq!(result.status, "压缩完成");
    }

    #[test]
    fn validate_returns_stats() {
        let result = run(r#"{"foo":[1,2]}"#, "", JsonMode::Validate);
        assert_eq!(result.status, "验证通过");
        assert!(result.error.is_none());
        assert_eq!(result.stats.as_ref().map(|stats| stats.size), Some(1));
    }

    #[test]
    fn queries_pointer_like_path() {
        let result = run(r#"{"foo":{"bar":2}}"#, ".foo.bar", JsonMode::Query);
        assert_eq!(result.output, "2");
        assert_eq!(result.status, "查询完成");
    }

    #[test]
    fn queries_bracket_path() {
        let result = run(
            r#"{"items":[{"name":"a"},{"name":"b"}]}"#,
            "$.items[1].name",
            JsonMode::Query,
        );
        assert_eq!(result.output, "\"b\"");
    }

    #[test]
    fn reports_parse_error_location() {
        let result = run(r#"{"a": }"#, "", JsonMode::Format);
        assert_eq!(result.status, "格式无效");
        assert_eq!(
            result.error.as_ref().map(|error| error.phase),
            Some("parse")
        );
        assert!(result.error.as_ref().map(|error| error.column).unwrap_or(0) > 0);
    }

    #[test]
    fn reports_query_error() {
        let result = run(r#"{"foo":1}"#, "$.bar", JsonMode::Query);
        assert_eq!(result.status, "查询无结果");
        assert_eq!(
            result.error.as_ref().map(|error| error.phase),
            Some("query")
        );
    }
}
