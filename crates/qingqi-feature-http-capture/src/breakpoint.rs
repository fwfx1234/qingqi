use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// When to trigger the breakpoint
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BreakpointPhase {
    BeforeRequest,
    AfterResponse,
}

impl BreakpointPhase {
    pub fn label(&self) -> &'static str {
        match self {
            Self::BeforeRequest => "请求前",
            Self::AfterResponse => "响应后",
        }
    }
}

/// Breakpoint rule for matching requests
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BreakpointRule {
    pub id: String,
    pub enabled: bool,
    pub phase: BreakpointPhase,
    pub url_pattern: String,
    pub method: String,
}

impl BreakpointRule {
    pub fn new(id: impl Into<String>, phase: BreakpointPhase, url_pattern: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            enabled: true,
            phase,
            url_pattern: url_pattern.into(),
            method: String::new(),
        }
    }

    pub fn matches(&self, method: &str, url: &str) -> bool {
        if !self.enabled { return false; }
        if !self.method.is_empty() && !self.method.eq_ignore_ascii_case(method) {
            return false;
        }
        Self::url_glob_match(&self.url_pattern, url)
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

/// State of an active breakpoint interception
#[derive(Clone, Debug)]
pub struct BreakpointState {
    pub exchange_id: i64,
    pub phase: BreakpointPhase,
    pub modified: bool,
}

/// Manages breakpoint rules and active interceptions
pub struct BreakpointManager {
    rules: Vec<BreakpointRule>,
    active_breakpoints: HashMap<i64, BreakpointState>,
}

impl BreakpointManager {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            active_breakpoints: HashMap::new(),
        }
    }

    pub fn add_rule(&mut self, rule: BreakpointRule) {
        self.rules.push(rule);
    }

    pub fn remove_rule(&mut self, id: &str) {
        self.rules.retain(|r| r.id != id);
    }

    pub fn list_rules(&self) -> &[BreakpointRule] {
        &self.rules
    }

    pub fn update_rule(&mut self, id: &str, f: impl FnOnce(&mut BreakpointRule)) {
        if let Some(rule) = self.rules.iter_mut().find(|r| r.id == id) {
            f(rule);
        }
    }

    pub fn check_request(&self, method: &str, url: &str) -> bool {
        self.rules.iter().any(|r| r.phase == BreakpointPhase::BeforeRequest && r.matches(method, url))
    }

    pub fn check_response(&self, method: &str, url: &str) -> bool {
        self.rules.iter().any(|r| r.phase == BreakpointPhase::AfterResponse && r.matches(method, url))
    }

    pub fn pause(&mut self, exchange_id: i64, phase: BreakpointPhase) {
        self.active_breakpoints.insert(exchange_id, BreakpointState {
            exchange_id,
            phase,
            modified: false,
        });
    }

    pub fn resume(&mut self, exchange_id: i64) {
        self.active_breakpoints.remove(&exchange_id);
    }

    pub fn is_paused(&self, exchange_id: i64) -> bool {
        self.active_breakpoints.contains_key(&exchange_id)
    }

    pub fn get_active(&self) -> &HashMap<i64, BreakpointState> {
        &self.active_breakpoints
    }

    pub fn clear_active(&mut self) {
        self.active_breakpoints.clear()
    }
}
