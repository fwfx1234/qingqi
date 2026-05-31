use std::collections::HashMap;

use serde_json::Value;

/// A lightweight line-oriented DSL interpreter for pre/post request operations.
/// No JavaScript runtime -- purely declarative text commands.

#[derive(Clone, Debug, Default)]
pub struct RequestDraft {
    pub method: String,
    pub url: String,
    pub params: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub body: String,
}

/// Process pre-ops text line by line.
///
/// Supported directives:
/// - `set key=value` -- adds to temporary vars (used during variable resolution)
/// - `header Key: Value` -- injects into draft headers
/// - `query key=value` -- injects into draft params
/// - `body.append <text>` -- appends to draft body
/// - Lines starting with `#` are comments; blank lines are skipped
pub fn apply_pre_ops(draft: &mut RequestDraft, pre_ops_text: &str) -> HashMap<String, String> {
    let mut temporary = HashMap::new();

    for line in pre_ops_text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(rest) = line.strip_prefix("set ") {
            if let Some((key, value)) = rest.trim().split_once('=') {
                temporary.insert(key.trim().to_string(), value.trim().to_string());
            }
        } else if let Some(rest) = line.strip_prefix("header ") {
            if let Some((key, value)) = rest.trim().split_once(':') {
                draft
                    .headers
                    .insert(key.trim().to_string(), value.trim().to_string());
            }
        } else if let Some(rest) = line.strip_prefix("query ") {
            if let Some((key, value)) = rest.trim().split_once('=') {
                draft
                    .params
                    .insert(key.trim().to_string(), value.trim().to_string());
            }
        } else if let Some(rest) = line.strip_prefix("body.append ") {
            if !draft.body.is_empty() {
                draft.body.push('\n');
            }
            draft.body.push_str(rest.trim());
        }
    }

    temporary
}

/// Run assertions against a response body and status code.
///
/// Supported assertion syntax:
/// - `status == 200` -- checks status code equality
/// - `body contains 'text'` -- substring check on response body
/// - `json $.code == 200` -- minimal JSONPath equality check
///
/// Returns a list of (assertion_text, passed) tuples.
pub fn run_assertions(
    assertions_text: &str,
    status_code: u16,
    response_body: &str,
) -> Vec<(String, bool)> {
    let mut results = Vec::new();

    for line in assertions_text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(rest) = line.strip_prefix("status") {
            let rest = rest.trim();
            if let Some(expected_str) = rest.strip_prefix("==") {
                let expected: u16 = match expected_str.trim().parse() {
                    Ok(v) => v,
                    Err(_) => {
                        results.push((line.to_string(), false));
                        continue;
                    }
                };
                results.push((line.to_string(), status_code == expected));
            } else if let Some(expected_str) = rest.strip_prefix("!=") {
                let expected: u16 = match expected_str.trim().parse() {
                    Ok(v) => v,
                    Err(_) => {
                        results.push((line.to_string(), true));
                        continue;
                    }
                };
                results.push((line.to_string(), status_code != expected));
            } else {
                results.push((line.to_string(), false));
            }
        } else if let Some(rest) = line.strip_prefix("body contains ") {
            let needle = rest.trim().trim_matches('\'').trim_matches('"');
            results.push((line.to_string(), response_body.contains(needle)));
        } else if let Some(rest) = line.strip_prefix("json ") {
            // Format: json $.path == expected
            let rest = rest.trim();
            if let Some((path_expr, comparison)) = rest.split_once("==") {
                let path = path_expr.trim();
                let expected = comparison.trim().trim_matches('\'').trim_matches('"');
                let actual = json_path_get(response_body, path);
                let passed = actual.as_deref() == Some(expected);
                results.push((line.to_string(), passed));
            } else if let Some((path_expr, comparison)) = rest.split_once("!=") {
                let path = path_expr.trim();
                let expected = comparison.trim().trim_matches('\'').trim_matches('"');
                let actual = json_path_get(response_body, path);
                let passed = actual.as_deref() != Some(expected);
                results.push((line.to_string(), passed));
            } else {
                // Just check if the path exists
                let path = rest.trim();
                let exists = json_path_get(response_body, path).is_some();
                results.push((line.to_string(), exists));
            }
        } else {
            results.push((format!("SKIP: {line}"), true));
        }
    }

    results
}

/// Extract variables from post-ops text.
///
/// Syntax: `extract token=$.data.token`
/// Uses minimal JSONPath ($.a.b.c, array indexes like $.items.0.id).
pub fn extract_variables(post_ops_text: &str, response_body: &str) -> HashMap<String, String> {
    let mut extracted = HashMap::new();

    for line in post_ops_text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(rest) = line.strip_prefix("extract ") {
            if let Some((var_name, path_expr)) = rest.trim().split_once('=') {
                let var_name = var_name.trim();
                let path = path_expr.trim();
                if let Some(value) = json_path_get(response_body, path) {
                    extracted.insert(var_name.to_string(), value);
                }
            }
        }
    }

    extracted
}

/// Minimal JSONPath traversal: $.key.subkey and $.array.index
fn json_path_get(body: &str, path: &str) -> Option<String> {
    let path = path.strip_prefix('$')?.trim_start_matches('.');
    if path.is_empty() {
        return Some(body.to_string());
    }

    let value: Value = serde_json::from_str(body).ok()?;
    let segments: Vec<&str> = path.split('.').collect();
    let mut current = &value;

    for segment in segments {
        if let Ok(index) = segment.parse::<usize>() {
            current = current.get(index)?;
        } else {
            current = current.get(segment)?;
        }
    }

    match current {
        Value::String(s) => Some(s.clone()),
        Value::Null => Some("null".to_string()),
        other => Some(other.to_string()),
    }
}

/// Format assertion results for display.
pub fn format_assertion_results(results: &[(String, bool)]) -> String {
    results
        .iter()
        .map(|(text, passed)| {
            if text.starts_with("SKIP:") {
                format!("SKIP  {text}")
            } else if *passed {
                format!("PASS  {text}")
            } else {
                format!("FAIL  {text}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pre_ops_set_creates_temporary() {
        let mut draft = RequestDraft::default();
        let temp = apply_pre_ops(&mut draft, "set token=abc123\nset user=admin");
        assert_eq!(temp.get("token").unwrap(), "abc123");
        assert_eq!(temp.get("user").unwrap(), "admin");
    }

    #[test]
    fn pre_ops_header_injects_headers() {
        let mut draft = RequestDraft::default();
        apply_pre_ops(
            &mut draft,
            "header Content-Type: application/json\nheader X-Custom: hello",
        );
        assert_eq!(
            draft.headers.get("Content-Type").unwrap(),
            "application/json"
        );
        assert_eq!(draft.headers.get("X-Custom").unwrap(), "hello");
    }

    #[test]
    fn pre_ops_query_injects_params() {
        let mut draft = RequestDraft::default();
        apply_pre_ops(&mut draft, "query page=1\nquery size=20");
        assert_eq!(draft.params.get("page").unwrap(), "1");
        assert_eq!(draft.params.get("size").unwrap(), "20");
    }

    #[test]
    fn pre_ops_body_append() {
        let mut draft = RequestDraft::default();
        apply_pre_ops(&mut draft, "body.append line1\nbody.append line2");
        assert_eq!(draft.body, "line1\nline2");
    }

    #[test]
    fn pre_ops_skips_comments_and_blank() {
        let mut draft = RequestDraft::default();
        let temp = apply_pre_ops(
            &mut draft,
            "# this is a comment\n\nset key=val\n# another comment",
        );
        assert_eq!(temp.len(), 1);
        assert_eq!(temp.get("key").unwrap(), "val");
    }

    #[test]
    fn assertions_status_equal() {
        let results = run_assertions("status == 200", 200, "{}");
        assert_eq!(results.len(), 1);
        assert!(results[0].1);
    }

    #[test]
    fn assertions_status_not_equal() {
        let results = run_assertions("status == 200", 404, "{}");
        assert_eq!(results.len(), 1);
        assert!(!results[0].1);
    }

    #[test]
    fn assertions_body_contains() {
        let results = run_assertions("body contains 'success'", 200, r#"{"msg":"success"}"#);
        assert_eq!(results.len(), 1);
        assert!(results[0].1);
    }

    #[test]
    fn assertions_json_path() {
        let body = r#"{"code":200,"data":{"id":42}}"#;
        let results = run_assertions("json $.code == 200", 200, body);
        assert_eq!(results.len(), 1);
        assert!(results[0].1);

        let results = run_assertions("json $.data.id == 42", 200, body);
        assert_eq!(results.len(), 1);
        assert!(results[0].1);
    }

    #[test]
    fn assertions_multiple() {
        let body = r#"{"code":200}"#;
        let results = run_assertions(
            "status == 200\njson $.code == 200\nbody contains 'code'",
            200,
            body,
        );
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|(_, passed)| *passed));
    }

    #[test]
    fn extract_simple_path() {
        let body = r#"{"data":{"token":"abc123"}}"#;
        let vars = extract_variables("extract token=$.data.token", body);
        assert_eq!(vars.get("token").unwrap(), "abc123");
    }

    #[test]
    fn extract_array_index() {
        let body = r#"{"items":[{"id":1},{"id":2}]}"#;
        let vars = extract_variables("extract first_id=$.items.0.id", body);
        assert_eq!(vars.get("first_id").unwrap(), "1");
    }

    #[test]
    fn extract_missing_path_no_var() {
        let body = r#"{"data":{}}"#;
        let vars = extract_variables("extract token=$.data.token", body);
        assert!(vars.is_empty());
    }

    #[test]
    fn json_path_get_string() {
        let body = r#"{"name":"test"}"#;
        assert_eq!(json_path_get(body, "$.name"), Some("test".into()));
    }

    #[test]
    fn json_path_get_number() {
        let body = r#"{"count":42}"#;
        assert_eq!(json_path_get(body, "$.count"), Some("42".into()));
    }

    #[test]
    fn json_path_get_nested() {
        let body = r#"{"a":{"b":{"c":"deep"}}}"#;
        assert_eq!(json_path_get(body, "$.a.b.c"), Some("deep".into()));
    }

    #[test]
    fn format_assertion_results_display() {
        let results = vec![
            ("status == 200".into(), true),
            ("json $.code == 200".into(), false),
            ("SKIP: unknown directive".into(), true),
        ];
        let formatted = format_assertion_results(&results);
        assert!(formatted.contains("PASS  status == 200"));
        assert!(formatted.contains("FAIL  json $.code == 200"));
        assert!(formatted.contains("SKIP  SKIP: unknown directive"));
    }
}
