use std::sync::{Arc, Mutex};

use crate::mock_model::{MockMatchResult};
use crate::mock_store::MockStore;

/// Mock 规则匹配引擎。
///
/// 遍历启用的规则（按 `sort_order` 升序），返回第一条匹配的模拟响应。
/// 匹配条件包括 URL 模式（glob `*` 通配符）、HTTP 方法和请求头。
pub struct MockEngine {
    store: Arc<Mutex<MockStore>>,
}

impl MockEngine {
    pub fn new(store: Arc<Mutex<MockStore>>) -> Self {
        Self { store }
    }

    /// 匹配请求并返回模拟响应。若无匹配则返回 `None`。
    ///
    /// - `method`: HTTP 方法（如 "GET"）
    /// - `url`: 完整 URL（如 "https://example.com/api/users/123"）
    /// - `headers`: 请求头键值对列表
    pub fn match_request(
        &self,
        method: &str,
        url: &str,
        headers: &[(String, String)],
    ) -> Option<MockMatchResult> {
        let rules = match self.store.lock().ok() {
            Some(store) => match store.list_enabled() {
                Ok(rules) => rules,
                Err(_) => return None,
            },
            None => return None,
        };

        for rule in rules {
            if !Self::url_matches(&rule.match_url_pattern, url) {
                continue;
            }
            if !Self::method_matches(&rule.match_method, method) {
                continue;
            }
            if !Self::headers_match(&rule.match_headers_json, headers) {
                continue;
            }

            // 匹配成功
            return Some(MockMatchResult {
                status: rule.action_status_code,
                headers: rule.action_headers_entries(),
                body: rule.action_body.clone(),
                delay_ms: rule.action_delay_ms,
            });
        }

        None
    }

    /// Glob 风格 URL 模式匹配。
    ///
    /// 支持 `*` 通配符，可出现在模式的任意位置：
    /// - `*` → 匹配所有 URL
    /// - `*/api/*` → 匹配包含 "/api/" 的 URL
    /// - `https://example.com/*` → 匹配以该前缀开头的 URL
    fn url_matches(pattern: &str, url: &str) -> bool {
        if pattern == "*" || pattern.is_empty() {
            return true;
        }

        // 将 pattern 分割为 segments，用 `*` 作为分隔符
        let segments: Vec<&str> = pattern.split('*').collect();

        // 没有通配符的情况：精确匹配
        if segments.len() == 1 {
            return url.contains(pattern);
        }

        let mut remaining = url;

        for (i, segment) in segments.iter().enumerate() {
            if segment.is_empty() {
                continue;
            }

            match remaining.find(segment) {
                Some(pos) => {
                    // 第一个 segment 必须从头匹配
                    if i == 0 && pos != 0 {
                        return false;
                    }
                    remaining = &remaining[pos + segment.len()..];
                }
                None => return false,
            }
        }

        true
    }

    /// HTTP 方法匹配（空 = 匹配所有）。
    fn method_matches(pattern: &str, method: &str) -> bool {
        if pattern.is_empty() {
            return true;
        }
        pattern.eq_ignore_ascii_case(method)
    }

    /// 请求头匹配：规则中指定的每个头键值对必须存在于请求头中。
    fn headers_match(
        match_json: &str,
        request_headers: &[(String, String)],
    ) -> bool {
        let conditions: Vec<(String, String)> =
            serde_json::from_str(match_json).unwrap_or_default();

        if conditions.is_empty() {
            return true;
        }

        for (cond_name, cond_value) in &conditions {
            let found = request_headers.iter().any(|(name, value)| {
                name.eq_ignore_ascii_case(cond_name)
                    && value.to_lowercase().contains(&cond_value.to_lowercase())
            });
            if !found {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_model::MockRule;
    use qingqi_plugin::database::DatabaseService;

    fn setup_engine(rules: Vec<MockRule>) -> MockEngine {
        // 创建临时数据库
        let db_path = std::env::temp_dir().join(format!(
            "qingqi-mock-eng-{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let dir = db_path.parent().unwrap();
        let _ = std::fs::create_dir_all(dir);
        let paths = qingqi_plugin::storage::AppPaths::for_test(dir.to_path_buf());
        let database = Arc::new(DatabaseService::new(paths));
        let key = qingqi_plugin::database::feature_database_key("http-capture", "mock-engine-test");
        database
            .register_database(qingqi_plugin::database::DatabaseSpec::path(
                &key,
                db_path.clone(),
            ))
            .unwrap();
        let store = Arc::new(Mutex::new(MockStore::open(database, &key).unwrap()));

        for rule in &rules {
            store.lock().unwrap().insert(rule).unwrap();
        }

        MockEngine::new(store)
    }

    #[test]
    fn url_matches_wildcard() {
        assert!(MockEngine::url_matches("*", "https://example.com/anything"));
        assert!(MockEngine::url_matches("", "https://example.com/anything"));
    }

    #[test]
    fn url_matches_prefix() {
        assert!(MockEngine::url_matches(
            "https://example.com/*",
            "https://example.com/api/users"
        ));
        assert!(!MockEngine::url_matches(
            "https://example.com/*",
            "https://other.com/api"
        ));
    }

    #[test]
    fn url_matches_contains() {
        assert!(MockEngine::url_matches("*/api/*", "https://example.com/api/users/123"));
        assert!(MockEngine::url_matches("*/api/*", "http://localhost:8080/api/v2/health"));
        assert!(!MockEngine::url_matches("*/api/*", "https://example.com/home"));
    }

    #[test]
    fn url_matches_exact_substring() {
        // 无通配符时做 substring 包含匹配
        assert!(MockEngine::url_matches("api/users", "https://example.com/api/users/123"));
        assert!(!MockEngine::url_matches("api/posts", "https://example.com/api/users"));
    }

    #[test]
    fn method_matches_all() {
        assert!(MockEngine::method_matches("", "GET"));
        assert!(MockEngine::method_matches("", "POST"));
    }

    #[test]
    fn method_matches_exact() {
        assert!(MockEngine::method_matches("GET", "GET"));
        assert!(MockEngine::method_matches("get", "GET"));
        assert!(!MockEngine::method_matches("GET", "POST"));
    }

    #[test]
    fn headers_match_subset() {
        let conditions = r#"[["Content-Type","json"]]"#;
        let headers = vec![
            ("Host".to_string(), "example.com".to_string()),
            ("Content-Type".to_string(), "application/json".to_string()),
        ];
        assert!(MockEngine::headers_match(conditions, &headers));

        let missing_headers = vec![("Host".to_string(), "example.com".to_string())];
        assert!(!MockEngine::headers_match(conditions, &missing_headers));
    }

    #[test]
    fn headers_match_empty_conditions() {
        assert!(MockEngine::headers_match("[]", &[]));
        assert!(MockEngine::headers_match("[]", &[("X".to_string(), "y".to_string())]));
    }

    #[test]
    fn match_request_finds_rule() {
        let rule = MockRule::new("API Mock", "*/api/*");
        let engine = setup_engine(vec![rule]);

        let result = engine.match_request(
            "GET",
            "https://example.com/api/users",
            &[],
        );
        assert!(result.is_some());
        assert_eq!(result.unwrap().status, 200);
    }

    #[test]
    fn match_request_no_match() {
        let rule = MockRule::new("API Mock", "*/api/*");
        let engine = setup_engine(vec![rule]);

        let result = engine.match_request(
            "GET",
            "https://example.com/home",
            &[],
        );
        assert!(result.is_none());
    }

    #[test]
    fn match_request_method_filter() {
        let mut rule = MockRule::new("POST Only", "*/api/*");
        rule.match_method = "POST".to_string();
        let engine = setup_engine(vec![rule]);

        assert!(engine
            .match_request("POST", "https://x.com/api/create", &[])
            .is_some());
        assert!(engine
            .match_request("GET", "https://x.com/api/list", &[])
            .is_none());
    }
}
