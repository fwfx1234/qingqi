use serde::{Deserialize, Serialize};

/// HTTP method for captured exchanges.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CaptureMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
    Other,
}

impl CaptureMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
            Self::Other => "OTHER",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_ascii_uppercase().as_str() {
            "GET" => Self::Get,
            "POST" => Self::Post,
            "PUT" => Self::Put,
            "PATCH" => Self::Patch,
            "DELETE" => Self::Delete,
            "HEAD" => Self::Head,
            "OPTIONS" => Self::Options,
            _ => Self::Other,
        }
    }

    pub fn color(&self) -> u32 {
        match self {
            Self::Get => 0x33aa66,
            Self::Post => 0x3388cc,
            Self::Put => 0x7b5fff,
            Self::Patch => 0xcc9933,
            Self::Delete => 0xcc4444,
            Self::Head => 0x888888,
            Self::Options => 0x999999,
            Self::Other => 0x888888,
        }
    }
}

impl std::fmt::Display for CaptureMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Proxy running state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProxyState {
    Stopped,
    Running { port: u16 },
}

impl ProxyState {
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running { .. })
    }

    pub fn port(&self) -> Option<u16> {
        match self {
            Self::Running { port } => Some(*port),
            Self::Stopped => None,
        }
    }
}

/// HTTPS certificate status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CertificateStatus {
    NotGenerated,
    Generated,
    Installed,
}

impl CertificateStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::NotGenerated => "未生成",
            Self::Generated => "已生成",
            Self::Installed => "已信任",
        }
    }

    pub fn ready_for_https(self) -> bool {
        !matches!(self, Self::NotGenerated)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureEndpoint {
    pub ip: String,
    pub port: u16,
}

impl CaptureEndpoint {
    pub fn proxy_url(&self) -> String {
        format!("{}:{}", self.ip, self.port)
    }

    pub fn http_proxy_url(&self) -> String {
        format!("http://{}", self.proxy_url())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureSetupInfo {
    pub proxy_state: ProxyState,
    pub certificate_status: CertificateStatus,
    pub local_endpoint: CaptureEndpoint,
    pub lan_endpoint: CaptureEndpoint,
    pub cert_path: String,
    pub mobile_cert_path: String,
    pub cert_download_url: String,
    pub ca_dir: String,
    pub install_command: Option<String>,
}

impl CaptureSetupInfo {
    pub fn is_running(&self) -> bool {
        self.proxy_state.is_running()
    }

    pub fn port(&self) -> u16 {
        self.proxy_state.port().unwrap_or(self.local_endpoint.port)
    }
}

/// Detail inspector tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DetailTab {
    Overview,
    RequestHeaders,
    RequestBody,
    ResponseHeaders,
    ResponseBody,
    Timing,
}

impl DetailTab {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Overview => "概览",
            Self::RequestHeaders => "请求头",
            Self::RequestBody => "请求体",
            Self::ResponseHeaders => "响应头",
            Self::ResponseBody => "响应体",
            Self::Timing => "计时",
        }
    }

    pub const ALL: [DetailTab; 6] = [
        DetailTab::Overview,
        DetailTab::RequestHeaders,
        DetailTab::RequestBody,
        DetailTab::ResponseHeaders,
        DetailTab::ResponseBody,
        DetailTab::Timing,
    ];
}

/// A single captured HTTP exchange.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapturedExchange {
    pub id: i64,
    pub method: String,
    pub url: String,
    pub host: String,
    pub status: i64,
    pub protocol: String,
    pub duration_ms: i64,
    pub request_size: i64,
    pub response_size: i64,
    pub request_headers_json: String,
    pub response_headers_json: String,
    pub request_body: String,
    pub response_body: String,
    pub timestamp: String,
    pub is_https: bool,
}

impl CapturedExchange {
    pub fn method_enum(&self) -> CaptureMethod {
        CaptureMethod::from_str(&self.method)
    }

    pub fn status_color(&self) -> u32 {
        match self.status {
            200..=299 => 0x33aa66,
            300..=399 => 0x3388cc,
            400..=499 => 0xcc9933,
            500..=599 => 0xcc4444,
            _ => 0x888888,
        }
    }

    pub fn formatted_size(&self) -> String {
        format_bytes(self.response_size)
    }

    pub fn formatted_duration(&self) -> String {
        if self.duration_ms < 1000 {
            format!("{}ms", self.duration_ms)
        } else {
            format!("{:.1}s", self.duration_ms as f64 / 1000.0)
        }
    }

    pub fn has_request_headers(&self) -> bool {
        !self.request_headers_json.is_empty() && self.request_headers_json != "[]"
    }

    pub fn has_response_headers(&self) -> bool {
        !self.response_headers_json.is_empty() && self.response_headers_json != "[]"
    }

    pub fn has_request_body(&self) -> bool {
        !self.request_body.is_empty()
    }

    pub fn has_response_body(&self) -> bool {
        !self.response_body.is_empty()
    }

    pub fn request_headers_entries(&self) -> Vec<HeaderEntry> {
        HeaderEntry::from_json(&self.request_headers_json)
    }

    pub fn response_headers_entries(&self) -> Vec<HeaderEntry> {
        HeaderEntry::from_json(&self.response_headers_json)
    }

    pub fn timing_summary(&self) -> String {
        format!(
            "请求大小: {}\n响应大小: {}\n耗时: {}\n协议: {}",
            format_bytes(self.request_size),
            format_bytes(self.response_size),
            self.formatted_duration(),
            self.protocol
        )
    }

    /// Structured timing/size pairs for the detail timing tab.
    pub fn timing_rows(&self) -> Vec<(&'static str, String)> {
        vec![
            ("协议", self.protocol.clone()),
            ("耗时", self.formatted_duration()),
            ("请求大小", format_bytes(self.request_size)),
            ("响应大小", format_bytes(self.response_size)),
            ("时间戳", self.timestamp.clone()),
        ]
    }

    /// Returns request body text, or an honest hint when the body string is
    /// empty but the captured request_size suggests there was content (binary
    /// upload, stream, or unrecorded body).
    pub fn request_body_display(&self) -> BodyDisplay {
        body_display(&self.request_body, self.request_size, BodyKind::Request)
    }

    /// Same as `request_body_display` for the response side.
    pub fn response_body_display(&self) -> BodyDisplay {
        body_display(&self.response_body, self.response_size, BodyKind::Response)
    }
}

/// What we can say about a request/response body in the inspector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BodyDisplay {
    /// No body and no size — render an empty-state notice.
    Empty,
    /// Body string is empty but `size > 0` — likely binary/streamed/unrecorded.
    Hinted(String),
    /// Plain text body to render line-by-line.
    Text(String),
}

#[derive(Clone, Copy)]
enum BodyKind {
    Request,
    Response,
}

fn body_display(body: &str, size: i64, kind: BodyKind) -> BodyDisplay {
    if !body.is_empty() {
        return BodyDisplay::Text(body.to_string());
    }
    if size > 0 {
        let label = match kind {
            BodyKind::Request => "请求正文",
            BodyKind::Response => "响应正文",
        };
        return BodyDisplay::Hinted(format!(
            "[{label}未按文本捕获 — 可能是二进制、流式上传或未记录正文，共 {}]",
            format_bytes(size)
        ));
    }
    BodyDisplay::Empty
}

/// Filter state for the capture list.
#[derive(Clone, Debug, Default)]
pub struct FilterState {
    pub method: String,
    pub host: String,
    pub status: String,
    pub search: String,
    pub https_only: bool,
    pub error_only: bool,
    pub hide_static: bool,
}

impl FilterState {
    pub fn matches(&self, exchange: &CapturedExchange) -> bool {
        if !self.method.is_empty()
            && self.method != "ALL"
            && exchange.method.to_uppercase() != self.method.to_uppercase()
        {
            return false;
        }
        if !self.host.is_empty()
            && !exchange
                .host
                .to_lowercase()
                .contains(&self.host.to_lowercase())
        {
            return false;
        }
        if !self.status.is_empty() {
            if let Ok(status_num) = self.status.parse::<i64>() {
                if exchange.status != status_num {
                    return false;
                }
            } else if !self.status.is_empty() {
                return false;
            }
        }
        if !self.search.is_empty()
            && !exchange
                .url
                .to_lowercase()
                .contains(&self.search.to_lowercase())
        {
            return false;
        }
        if self.https_only && !exchange.is_https {
            return false;
        }
        if self.error_only && exchange.status < 400 {
            return false;
        }
        if self.hide_static {
            let url_lower = exchange.url.to_lowercase();
            static EXTENSIONS: &[&str] = &[
                ".css", ".js", ".png", ".jpg", ".jpeg", ".gif", ".svg", ".ico", ".woff", ".woff2",
                ".ttf", ".eot", ".map", ".webp", ".avif",
            ];
            if EXTENSIONS.iter().any(|ext| url_lower.ends_with(ext)) {
                return false;
            }
        }
        true
    }
}

/// Computed statistics from captured exchanges.
#[derive(Clone, Debug, Default)]
pub struct CaptureStats {
    pub total: usize,
    pub visible: usize,
    pub errors: usize,
    pub https_count: usize,
    pub avg_duration_ms: f64,
    pub total_bytes: i64,
}

/// Header key-value pair for display.
#[derive(Clone, Debug)]
pub struct HeaderEntry {
    pub name: String,
    pub value: String,
}

impl HeaderEntry {
    pub fn from_json(json: &str) -> Vec<HeaderEntry> {
        serde_json::from_str::<Vec<(String, String)>>(json)
            .unwrap_or_default()
            .into_iter()
            .map(|(name, value)| HeaderEntry { name, value })
            .collect()
    }
}

pub fn format_bytes(bytes: i64) -> String {
    if bytes < 0 {
        return "0 B".to_string();
    }
    let bytes = bytes as f64;
    if bytes < 1024.0 {
        format!("{} B", bytes as i64)
    } else if bytes < 1024.0 * 1024.0 {
        format!("{:.1} KB", bytes / 1024.0)
    } else {
        format!("{:.1} MB", bytes / (1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_exchange(
        method: &str,
        url: &str,
        host: &str,
        status: i64,
        is_https: bool,
    ) -> CapturedExchange {
        CapturedExchange {
            id: 0,
            method: method.to_string(),
            url: url.to_string(),
            host: host.to_string(),
            status,
            protocol: "HTTP/1.1".to_string(),
            duration_ms: 50,
            request_size: 100,
            response_size: 2048,
            request_headers_json: "[]".to_string(),
            response_headers_json: "[]".to_string(),
            request_body: String::new(),
            response_body: String::new(),
            timestamp: "2025-01-01 00:00:00".to_string(),
            is_https,
        }
    }

    #[test]
    fn method_from_str() {
        assert_eq!(CaptureMethod::from_str("GET"), CaptureMethod::Get);
        assert_eq!(CaptureMethod::from_str("post"), CaptureMethod::Post);
        assert_eq!(CaptureMethod::from_str("DELETE"), CaptureMethod::Delete);
        assert_eq!(CaptureMethod::from_str("PATCH"), CaptureMethod::Patch);
        assert_eq!(CaptureMethod::from_str("XYZ"), CaptureMethod::Other);
    }

    #[test]
    fn method_display() {
        assert_eq!(CaptureMethod::Get.to_string(), "GET");
        assert_eq!(CaptureMethod::Post.to_string(), "POST");
    }

    #[test]
    fn filter_by_method() {
        let filter = FilterState {
            method: "GET".to_string(),
            ..Default::default()
        };
        let get_req = make_exchange("GET", "/api", "example.com", 200, false);
        let post_req = make_exchange("POST", "/api", "example.com", 200, false);
        assert!(filter.matches(&get_req));
        assert!(!filter.matches(&post_req));
    }

    #[test]
    fn filter_by_host() {
        let filter = FilterState {
            host: "example".to_string(),
            ..Default::default()
        };
        let matching = make_exchange("GET", "/", "api.example.com", 200, false);
        let not_matching = make_exchange("GET", "/", "other.com", 200, false);
        assert!(filter.matches(&matching));
        assert!(!filter.matches(&not_matching));
    }

    #[test]
    fn filter_by_search() {
        let filter = FilterState {
            search: "/users".to_string(),
            ..Default::default()
        };
        let matching = make_exchange("GET", "/api/users/123", "example.com", 200, false);
        let not_matching = make_exchange("GET", "/api/posts", "example.com", 200, false);
        assert!(filter.matches(&matching));
        assert!(!filter.matches(&not_matching));
    }

    #[test]
    fn filter_https_only() {
        let filter = FilterState {
            https_only: true,
            ..Default::default()
        };
        let https = make_exchange("GET", "/", "example.com", 200, true);
        let http = make_exchange("GET", "/", "example.com", 200, false);
        assert!(filter.matches(&https));
        assert!(!filter.matches(&http));
    }

    #[test]
    fn filter_error_only() {
        let filter = FilterState {
            error_only: true,
            ..Default::default()
        };
        let ok = make_exchange("GET", "/", "example.com", 200, false);
        let err = make_exchange("GET", "/", "example.com", 500, false);
        assert!(!filter.matches(&ok));
        assert!(filter.matches(&err));
    }

    #[test]
    fn filter_hide_static() {
        let filter = FilterState {
            hide_static: true,
            ..Default::default()
        };
        let html = make_exchange("GET", "/page.html", "example.com", 200, false);
        let css = make_exchange("GET", "/style.css", "example.com", 200, false);
        let js = make_exchange("GET", "/app.js", "example.com", 200, false);
        let png = make_exchange("GET", "/logo.png", "example.com", 200, false);
        assert!(filter.matches(&html));
        assert!(!filter.matches(&css));
        assert!(!filter.matches(&js));
        assert!(!filter.matches(&png));
    }

    #[test]
    fn filter_all_passes() {
        let filter = FilterState::default();
        let exchange = make_exchange("GET", "/api", "example.com", 200, false);
        assert!(filter.matches(&exchange));
    }

    #[test]
    fn status_color_codes() {
        let ok = make_exchange("GET", "/", "a.com", 200, false);
        let redirect = make_exchange("GET", "/", "a.com", 301, false);
        let client_err = make_exchange("GET", "/", "a.com", 404, false);
        let server_err = make_exchange("GET", "/", "a.com", 500, false);
        assert_eq!(ok.status_color(), 0x33aa66);
        assert_eq!(redirect.status_color(), 0x3388cc);
        assert_eq!(client_err.status_color(), 0xcc9933);
        assert_eq!(server_err.status_color(), 0xcc4444);
    }

    #[test]
    fn formatted_size() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1048576), "1.0 MB");
    }

    #[test]
    fn formatted_duration() {
        let fast = CapturedExchange {
            duration_ms: 42,
            ..make_exchange("GET", "/", "a.com", 200, false)
        };
        assert_eq!(fast.formatted_duration(), "42ms");

        let slow = CapturedExchange {
            duration_ms: 2340,
            ..make_exchange("GET", "/", "a.com", 200, false)
        };
        assert_eq!(slow.formatted_duration(), "2.3s");
    }

    #[test]
    fn detail_tab_labels() {
        assert_eq!(DetailTab::Overview.label(), "概览");
        assert_eq!(DetailTab::RequestHeaders.label(), "请求头");
        assert_eq!(DetailTab::Timing.label(), "计时");
    }

    #[test]
    fn detail_tab_all_has_six_variants() {
        assert_eq!(DetailTab::ALL.len(), 6);
        assert_eq!(DetailTab::ALL[0], DetailTab::Overview);
        assert_eq!(DetailTab::ALL[5], DetailTab::Timing);
    }

    #[test]
    fn header_entry_from_json() {
        let entries =
            HeaderEntry::from_json(r#"[["Content-Type","application/json"],["X-Foo","bar"]]"#);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "Content-Type");
        assert_eq!(entries[1].value, "bar");
    }

    #[test]
    fn header_entry_from_invalid_json() {
        let entries = HeaderEntry::from_json("not json");
        assert!(entries.is_empty());
    }

    #[test]
    fn has_headers_detects_content() {
        let exchange = CapturedExchange {
            request_headers_json: r#"[["Content-Type","text/html"]]"#.to_string(),
            response_headers_json: "[]".to_string(),
            ..make_exchange("GET", "/", "a.com", 200, false)
        };
        assert!(exchange.has_request_headers());
        assert!(!exchange.has_response_headers());
    }

    #[test]
    fn has_body_detects_content() {
        let exchange = CapturedExchange {
            request_body: "hello".to_string(),
            response_body: String::new(),
            ..make_exchange("GET", "/", "a.com", 200, false)
        };
        assert!(exchange.has_request_body());
        assert!(!exchange.has_response_body());
    }

    #[test]
    fn headers_entries_parses_json() {
        let exchange = CapturedExchange {
            request_headers_json: r#"[["Content-Type","application/json"],["X-Request-Id","abc"]]"#
                .to_string(),
            response_headers_json: r#"[["Server","nginx"]]"#.to_string(),
            ..make_exchange("GET", "/", "a.com", 200, false)
        };
        let req_h = exchange.request_headers_entries();
        assert_eq!(req_h.len(), 2);
        assert_eq!(req_h[0].name, "Content-Type");
        assert_eq!(req_h[1].value, "abc");

        let resp_h = exchange.response_headers_entries();
        assert_eq!(resp_h.len(), 1);
        assert_eq!(resp_h[0].name, "Server");
    }

    #[test]
    fn timing_summary_includes_sizes_and_duration() {
        let exchange = CapturedExchange {
            duration_ms: 42,
            request_size: 256,
            response_size: 2048,
            protocol: "HTTP/2".to_string(),
            ..make_exchange("GET", "/", "a.com", 200, false)
        };
        let summary = exchange.timing_summary();
        assert!(summary.contains("256 B"));
        assert!(summary.contains("2.0 KB"));
        assert!(summary.contains("42ms"));
        assert!(summary.contains("HTTP/2"));
    }

    #[test]
    fn empty_exchange_has_no_content() {
        let exchange = make_exchange("GET", "/", "a.com", 200, false);
        assert!(!exchange.has_request_headers());
        assert!(!exchange.has_response_headers());
        assert!(!exchange.has_request_body());
        assert!(!exchange.has_response_body());
    }

    #[test]
    fn timing_rows_includes_all_fields() {
        let exchange = CapturedExchange {
            duration_ms: 42,
            request_size: 256,
            response_size: 2048,
            protocol: "HTTP/2".to_string(),
            timestamp: "2025-01-01 12:34:56".to_string(),
            ..make_exchange("GET", "/", "a.com", 200, false)
        };
        let rows = exchange.timing_rows();
        let keys: Vec<&str> = rows.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec!["协议", "耗时", "请求大小", "响应大小", "时间戳"]);
        let lookup = |k: &str| -> String {
            rows.iter()
                .find(|(rk, _)| *rk == k)
                .map(|(_, v)| v.clone())
                .unwrap()
        };
        assert_eq!(lookup("协议"), "HTTP/2");
        assert_eq!(lookup("耗时"), "42ms");
        assert_eq!(lookup("请求大小"), "256 B");
        assert_eq!(lookup("响应大小"), "2.0 KB");
        assert_eq!(lookup("时间戳"), "2025-01-01 12:34:56");
    }

    #[test]
    fn body_display_text_when_present() {
        let exchange = CapturedExchange {
            request_body: "hello".to_string(),
            response_body: "world".to_string(),
            ..make_exchange("GET", "/", "a.com", 200, false)
        };
        assert_eq!(
            exchange.request_body_display(),
            BodyDisplay::Text("hello".to_string())
        );
        assert_eq!(
            exchange.response_body_display(),
            BodyDisplay::Text("world".to_string())
        );
    }

    #[test]
    fn body_display_empty_when_no_data_and_no_size() {
        let exchange = CapturedExchange {
            request_size: 0,
            response_size: 0,
            ..make_exchange("GET", "/", "a.com", 200, false)
        };
        assert_eq!(exchange.request_body_display(), BodyDisplay::Empty);
        assert_eq!(exchange.response_body_display(), BodyDisplay::Empty);
    }

    #[test]
    fn body_display_hinted_when_size_present_but_body_empty() {
        let exchange = CapturedExchange {
            request_body: String::new(),
            response_body: String::new(),
            request_size: 4096,
            response_size: 12345,
            ..make_exchange("GET", "/", "a.com", 200, false)
        };
        match exchange.request_body_display() {
            BodyDisplay::Hinted(msg) => {
                assert!(msg.contains("请求正文"));
                assert!(msg.contains("4.0 KB"));
            }
            other => panic!("expected Hinted, got {other:?}"),
        }
        match exchange.response_body_display() {
            BodyDisplay::Hinted(msg) => {
                assert!(msg.contains("响应正文"));
                assert!(msg.contains("12.1 KB"));
            }
            other => panic!("expected Hinted, got {other:?}"),
        }
    }
}
