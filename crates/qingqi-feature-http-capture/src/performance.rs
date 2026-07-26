use serde::{Deserialize, Serialize};

/// Performance statistics for captured sessions
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PerformanceStats {
    pub total_requests: usize,
    pub total_bytes: i64,
    pub avg_response_time_ms: f64,
    pub max_response_time_ms: i64,
    pub min_response_time_ms: i64,
    pub error_rate: f64,
    pub requests_per_second: f64,
    pub status_code_distribution: StatusDistribution,
    pub content_type_distribution: ContentTypeDistribution,
    pub slowest_endpoints: Vec<EndpointTiming>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StatusDistribution {
    pub ok_2xx: usize,
    pub redirect_3xx: usize,
    pub client_error_4xx: usize,
    pub server_error_5xx: usize,
    pub other: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ContentTypeDistribution {
    pub html: usize,
    pub json: usize,
    pub image: usize,
    pub css: usize,
    pub javascript: usize,
    pub other: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EndpointTiming {
    pub url: String,
    pub method: String,
    pub avg_duration_ms: f64,
    pub count: usize,
}

/// Calculate performance statistics from exchanges
pub fn calculate_stats(exchanges: &[crate::model::CapturedExchange]) -> PerformanceStats {
    if exchanges.is_empty() {
        return PerformanceStats::default();
    }

    let total_requests = exchanges.len();
    let total_bytes: i64 = exchanges.iter().map(|e| e.response_size).sum();
    let total_duration: i64 = exchanges.iter().map(|e| e.duration_ms).sum();
    let avg_response_time_ms = total_duration as f64 / total_requests as f64;
    let max_response_time_ms = exchanges.iter().map(|e| e.duration_ms).max().unwrap_or(0);
    let min_response_time_ms = exchanges.iter().map(|e| e.duration_ms).min().unwrap_or(0);
    let error_count = exchanges.iter().filter(|e| e.status >= 400).count();
    let error_rate = error_count as f64 / total_requests as f64;

    // Calculate time span for RPS
    // (simplified - in real impl would parse timestamps)
    let requests_per_second = if total_duration > 0 {
        total_requests as f64 / (total_duration as f64 / 1000.0)
    } else {
        0.0
    };

    // Status distribution
    let mut status_dist = StatusDistribution::default();
    for e in exchanges {
        match e.status {
            200..=299 => status_dist.ok_2xx += 1,
            300..=399 => status_dist.redirect_3xx += 1,
            400..=499 => status_dist.client_error_4xx += 1,
            500..=599 => status_dist.server_error_5xx += 1,
            _ => status_dist.other += 1,
        }
    }

    // Content type distribution
    let mut content_dist = ContentTypeDistribution::default();
    for e in exchanges {
        let headers: Vec<(String, String)> = serde_json::from_str(&e.response_headers_json)
            .unwrap_or_default();
        let ct = headers.iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
            .map(|(_, v)| v.to_lowercase())
            .unwrap_or_default();

        if ct.contains("html") { content_dist.html += 1; }
        else if ct.contains("json") { content_dist.json += 1; }
        else if ct.contains("image") { content_dist.image += 1; }
        else if ct.contains("css") { content_dist.css += 1; }
        else if ct.contains("javascript") || ct.contains("js") { content_dist.javascript += 1; }
        else { content_dist.other += 1; }
    }

    // Slowest endpoints (top 10)
    let mut endpoint_map: std::collections::HashMap<String, (i64, usize)> = std::collections::HashMap::new();
    for e in exchanges {
        let key = format!("{} {}", e.method, e.url);
        let entry = endpoint_map.entry(key).or_insert((0, 0));
        entry.0 += e.duration_ms;
        entry.1 += 1;
    }

    let mut slowest: Vec<EndpointTiming> = endpoint_map.into_iter()
        .map(|(key, (total_ms, count))| {
            let parts: Vec<&str> = key.splitn(2, ' ').collect();
            EndpointTiming {
                method: parts[0].to_string(),
                url: parts[1].to_string(),
                avg_duration_ms: total_ms as f64 / count as f64,
                count,
            }
        })
        .collect();
    slowest.sort_by(|a, b| b.avg_duration_ms.partial_cmp(&a.avg_duration_ms).unwrap());
    slowest.truncate(10);

    PerformanceStats {
        total_requests,
        total_bytes,
        avg_response_time_ms,
        max_response_time_ms,
        min_response_time_ms,
        error_rate,
        requests_per_second,
        status_code_distribution: status_dist,
        content_type_distribution: content_dist,
        slowest_endpoints: slowest,
    }
}
