use serde::{Deserialize, Serialize};

/// Mock 规则 — 匹配请求并返回模拟响应。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MockRule {
    pub id: i64,
    /// 规则名称（用户可读）
    pub name: String,
    /// 是否启用
    pub enabled: bool,
    /// URL 匹配模式（支持 `*` 通配符）
    pub match_url_pattern: String,
    /// HTTP 方法（空 = 匹配所有）
    pub match_method: String,
    /// 请求头匹配条件（JSON: `[["key","value"],...]`）
    pub match_headers_json: String,
    /// 模拟响应状态码
    pub action_status_code: i64,
    /// 模拟响应头（JSON 格式）
    pub action_headers_json: String,
    /// 模拟响应体
    pub action_body: String,
    /// 模拟延迟（毫秒）
    pub action_delay_ms: i64,
    /// 排序（越小越优先匹配）
    pub sort_order: i64,
    /// 创建时间
    pub created_at: String,
}

impl MockRule {
    /// 新建一条默认启用的规则。
    pub fn new(name: impl Into<String>, url_pattern: impl Into<String>) -> Self {
        Self {
            id: 0,
            name: name.into(),
            enabled: true,
            match_url_pattern: url_pattern.into(),
            match_method: String::new(),
            match_headers_json: "[]".to_string(),
            action_status_code: 200,
            action_headers_json: r#"[["Content-Type","application/json"]]"#.to_string(),
            action_body: "{}".to_string(),
            action_delay_ms: 0,
            sort_order: 0,
            created_at: String::new(),
        }
    }

    /// 解析匹配头条件为键值对列表。
    pub fn match_headers_entries(&self) -> Vec<(String, String)> {
        serde_json::from_str::<Vec<(String, String)>>(&self.match_headers_json).unwrap_or_default()
    }

    /// 解析响应头条件为键值对列表。
    pub fn action_headers_entries(&self) -> Vec<(String, String)> {
        serde_json::from_str::<Vec<(String, String)>>(&self.action_headers_json)
            .unwrap_or_default()
    }
}

/// Mock 匹配结果 — 匹配成功后返回给客户端的模拟响应。
#[derive(Clone, Debug)]
pub struct MockMatchResult {
    pub status: i64,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub delay_ms: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_rule_new_defaults() {
        let rule = MockRule::new("测试规则", "*/api/*");
        assert_eq!(rule.name, "测试规则");
        assert_eq!(rule.match_url_pattern, "*/api/*");
        assert!(rule.enabled);
        assert_eq!(rule.action_status_code, 200);
        assert!(rule.action_body.contains("{}"));
    }

    #[test]
    fn match_headers_entries_parses_json() {
        let rule = MockRule {
            match_headers_json: r#"[["X-Test","abc"],["Content-Type","json"]]"#.to_string(),
            ..MockRule::new("test", "*")
        };
        let entries = rule.match_headers_entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0], ("X-Test".to_string(), "abc".to_string()));
    }

    #[test]
    fn action_headers_entries_parses_json() {
        let rule = MockRule {
            action_headers_json: r#"[["Server","mock"]]"#.to_string(),
            ..MockRule::new("test", "*")
        };
        let entries = rule.action_headers_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, "Server");
    }
}
