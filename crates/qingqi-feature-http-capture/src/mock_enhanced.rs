use serde::{Deserialize, Serialize};

/// Enhanced mock rule with file mapping and regex support
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnhancedMockRule {
    pub id: String,
    pub enabled: bool,
    pub name: String,
    pub url_pattern: String,           // glob pattern (simple)
    pub url_regex: Option<String>,     // optional regex pattern (takes priority)
    pub method: String,                // empty = match all
    pub action_status_code: u16,
    pub action_headers: Vec<(String, String)>,
    pub action_body: String,
    pub action_file_path: Option<String>, // map response to local file
    pub action_delay_ms: u64,
    pub sort_order: i64,
}

impl EnhancedMockRule {
    pub fn new(name: impl Into<String>, url_pattern: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            enabled: true,
            name: name.into(),
            url_pattern: url_pattern.into(),
            url_regex: None,
            method: String::new(),
            action_status_code: 200,
            action_headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            action_body: "{}".to_string(),
            action_file_path: None,
            action_delay_ms: 0,
            sort_order: 0,
        }
    }
}

/// Result of an enhanced mock match
#[derive(Clone, Debug)]
pub struct EnhancedMockResult {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub delay_ms: u64,
}

/// Enhanced mock engine
pub struct EnhancedMockEngine {
    rules: Vec<EnhancedMockRule>,
}

impl EnhancedMockEngine {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }
    
    pub fn add_rule(&mut self, rule: EnhancedMockRule) {
        self.rules.push(rule);
        self.rules.sort_by_key(|r| r.sort_order);
    }
    
    pub fn remove_rule(&mut self, id: &str) {
        self.rules.retain(|r| r.id != id);
    }
    
    pub fn list_rules(&self) -> &[EnhancedMockRule] {
        &self.rules
    }
    
    pub fn update_rule(&mut self, id: &str, f: impl FnOnce(&mut EnhancedMockRule)) {
        if let Some(rule) = self.rules.iter_mut().find(|r| r.id == id) {
            f(rule);
        }
    }
    
    pub fn match_request(&self, method: &str, url: &str) -> Option<EnhancedMockResult> {
        for rule in &self.rules {
            if !rule.enabled { continue; }
            if !rule.method.is_empty() && !rule.method.eq_ignore_ascii_case(method) {
                continue;
            }
            
            let url_matches = if let Some(ref regex) = rule.url_regex {
                regex::Regex::new(regex).ok().map(|re| re.is_match(url)).unwrap_or(false)
            } else {
                Self::glob_match(&rule.url_pattern, url)
            };
            
            if !url_matches { continue; }
            
            // If file mapping is configured, read file content
            let body = if let Some(ref path) = rule.action_file_path {
                std::fs::read_to_string(path).unwrap_or_else(|e| {
                    format!("[Mock] 文件读取失败: {e}")
                })
            } else {
                rule.action_body.clone()
            };
            
            return Some(EnhancedMockResult {
                status: rule.action_status_code,
                headers: rule.action_headers.clone(),
                body,
                delay_ms: rule.action_delay_ms,
            });
        }
        None
    }
    
    fn glob_match(pattern: &str, url: &str) -> bool {
        if pattern == "*" || pattern.is_empty() { return true; }
        let segments: Vec<&str> = pattern.split('*').collect();
        if segments.len() == 1 { return url.contains(pattern); }
        let mut remaining = url;
        for (i, seg) in segments.iter().enumerate() {
            if seg.is_empty() { continue; }
            match remaining.find(seg) {
                Some(pos) => {
                    if i == 0 && pos != 0 { return false; }
                    remaining = &remaining[pos + seg.len()..];
                }
                None => return false,
            }
        }
        true
    }
}
