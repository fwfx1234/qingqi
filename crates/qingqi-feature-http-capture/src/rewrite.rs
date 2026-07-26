use serde::{Deserialize, Serialize};

/// What part of the request/response to rewrite
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RewriteTarget {
    RequestHeader,
    RequestBody,
    ResponseHeader,
    ResponseBody,
    Url,
}

impl RewriteTarget {
    pub fn label(&self) -> &'static str {
        match self {
            Self::RequestHeader => "请求头",
            Self::RequestBody => "请求体",
            Self::ResponseHeader => "响应头",
            Self::ResponseBody => "响应体",
            Self::Url => "URL",
        }
    }
}

/// Match condition for rewrite rule
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RewriteCondition {
    pub url_pattern: String,    // glob pattern
    pub method: String,         // empty = all
}

/// A single rewrite action
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RewriteAction {
    pub target: RewriteTarget,
    pub header_name: Option<String>,  // for header rewrites
    pub match_pattern: String,        // regex or literal
    pub replace_value: String,
    pub is_regex: bool,
}

/// Complete rewrite rule
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RewriteRule {
    pub id: String,
    pub enabled: bool,
    pub name: String,
    pub condition: RewriteCondition,
    pub actions: Vec<RewriteAction>,
}

/// Manages rewrite rules
pub struct RewriteEngine {
    rules: Vec<RewriteRule>,
}

impl RewriteEngine {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn add_rule(&mut self, rule: RewriteRule) {
        self.rules.push(rule);
    }

    pub fn remove_rule(&mut self, id: &str) {
        self.rules.retain(|r| r.id != id);
    }

    pub fn list_rules(&self) -> &[RewriteRule] {
        &self.rules
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Check if a URL+method matches any rule
    pub fn matches(&self, method: &str, url: &str) -> bool {
        self.rules.iter().any(|r| {
            if !r.enabled { return false; }
            if !r.condition.method.is_empty() && !r.condition.method.eq_ignore_ascii_case(method) {
                return false;
            }
            Self::url_glob_match(&r.condition.url_pattern, url)
        })
    }

    fn url_glob_match(pattern: &str, url: &str) -> bool {
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
