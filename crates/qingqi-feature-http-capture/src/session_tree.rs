use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

/// A node in the session tree
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SessionTreeNode {
    Domain(DomainNode),
    Request(RequestNode),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DomainNode {
    pub host: String,
    pub request_count: usize,
    pub total_bytes: i64,
    pub avg_duration_ms: f64,
    pub error_count: usize,
    pub children: Vec<RequestNode>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequestNode {
    pub id: i64,
    pub method: String,
    pub url: String,
    pub status: i64,
    pub duration_ms: i64,
    pub response_size: i64,
    pub timestamp: String,
}

/// Build a tree from flat exchange list
pub fn build_session_tree(exchanges: &[crate::model::CapturedExchange]) -> Vec<DomainNode> {
    let mut map: BTreeMap<String, Vec<&crate::model::CapturedExchange>> = BTreeMap::new();

    for exchange in exchanges {
        map.entry(exchange.host.clone()).or_default().push(exchange);
    }

    let mut domains: Vec<DomainNode> = map.into_iter().map(|(host, exchanges)| {
        let request_count = exchanges.len();
        let total_bytes: i64 = exchanges.iter().map(|e| e.response_size).sum();
        let total_duration: i64 = exchanges.iter().map(|e| e.duration_ms).sum();
        let avg_duration_ms = if request_count > 0 {
            total_duration as f64 / request_count as f64
        } else {
            0.0
        };
        let error_count = exchanges.iter().filter(|e| e.status >= 400).count();

        let children: Vec<RequestNode> = exchanges.into_iter().map(|e| RequestNode {
            id: e.id,
            method: e.method.clone(),
            url: e.url.clone(),
            status: e.status,
            duration_ms: e.duration_ms,
            response_size: e.response_size,
            timestamp: e.timestamp.clone(),
        }).collect();

        DomainNode {
            host,
            request_count,
            total_bytes,
            avg_duration_ms,
            error_count,
            children,
        }
    }).collect();

    // Sort by request count descending
    domains.sort_by(|a, b| b.request_count.cmp(&a.request_count));
    domains
}

/// Flatten tree for display (returns rows with indentation info)
#[derive(Clone, Debug)]
pub struct FlatRow {
    pub indent: usize,
    pub is_domain: bool,
    pub node_id: String,  // domain name or request id
    pub display: String,
    pub exchange_id: Option<i64>,
}

pub fn flatten_tree(domains: &[DomainNode]) -> Vec<FlatRow> {
    let mut rows = Vec::new();

    for domain in domains {
        rows.push(FlatRow {
            indent: 0,
            is_domain: true,
            node_id: domain.host.clone(),
            display: format!("{} ({} requests, {} errors)",
                domain.host, domain.request_count, domain.error_count),
            exchange_id: None,
        });

        for req in &domain.children {
            let status_icon = if req.status >= 400 { "❌" } else { "✅" };
            rows.push(FlatRow {
                indent: 1,
                is_domain: false,
                node_id: req.id.to_string(),
                display: format!("{} {} {} ({}ms, {})",
                    status_icon, req.method, req.url, req.duration_ms,
                    format_bytes(req.response_size)),
                exchange_id: Some(req.id),
            });
        }
    }

    rows
}

fn format_bytes(bytes: i64) -> String {
    if bytes < 1024 { format!("{bytes} B") }
    else if bytes < 1024 * 1024 { format!("{:.1} KB", bytes as f64 / 1024.0) }
    else { format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0)) }
}
