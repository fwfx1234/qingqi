use serde::{Deserialize, Serialize};
use serde_json;

/// HAR 1.2 format structures
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HarFile {
    pub log: HarLog,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HarLog {
    pub version: String,
    pub creator: HarCreator,
    pub entries: Vec<HarEntry>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HarCreator {
    pub name: String,
    pub version: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HarEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_date_time: Option<String>,
    pub time: f64,
    pub request: HarRequest,
    pub response: HarResponse,
    pub timings: HarTimings,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HarRequest {
    pub method: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_version: Option<String>,
    pub headers: Vec<HarHeader>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_data: Option<HarPostData>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HarResponse {
    pub status: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_version: Option<String>,
    pub headers: Vec<HarHeader>,
    pub content: HarContent,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HarHeader {
    pub name: String,
    pub value: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HarContent {
    pub size: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HarPostData {
    pub mime_type: String,
    pub text: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HarTimings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub send: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receive: Option<f64>,
}

/// Convert captured exchanges to HAR format
pub fn exchanges_to_har(
    exchanges: &[crate::model::CapturedExchange],
) -> HarFile {
    let entries: Vec<HarEntry> = exchanges.iter().map(|e| {
        let req_headers: Vec<(String, String)> = serde_json::from_str(&e.request_headers_json)
            .unwrap_or_default();
        let resp_headers: Vec<(String, String)> = serde_json::from_str(&e.response_headers_json)
            .unwrap_or_default();

        let req_headers_mime = req_headers.iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| "application/octet-stream".to_string());

        let resp_content_mime = resp_headers.iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
            .map(|(_, v)| v.clone());

        HarEntry {
            started_date_time: Some(e.timestamp.clone()),
            time: e.duration_ms as f64,
            request: HarRequest {
                method: e.method.clone(),
                url: e.url.clone(),
                http_version: Some(e.protocol.clone()),
                headers: req_headers.into_iter().map(|(name, value)| HarHeader { name, value }).collect(),
                post_data: if e.request_body.is_empty() {
                    None
                } else {
                    Some(HarPostData {
                        mime_type: req_headers_mime,
                        text: e.request_body.clone(),
                    })
                },
            },
            response: HarResponse {
                status: e.status as i32,
                status_text: Some(status_text(e.status)),
                http_version: Some(e.protocol.clone()),
                headers: resp_headers.into_iter().map(|(name, value)| HarHeader { name, value }).collect(),
                content: HarContent {
                    size: e.response_size,
                    mime_type: resp_content_mime,
                    text: if e.response_body.is_empty() { None } else { Some(e.response_body.clone()) },
                },
            },
            timings: HarTimings {
                send: Some(0.0),
                wait: Some(e.duration_ms as f64),
                receive: Some(0.0),
            },
        }
    }).collect();

    HarFile {
        log: HarLog {
            version: "1.2".to_string(),
            creator: HarCreator {
                name: "Qingqi".to_string(),
                version: "1.0.0".to_string(),
            },
            entries,
        },
    }
}

/// Parse a HAR file and return the entries as JSON
pub fn har_to_json(har: &HarFile) -> serde_json::Result<String> {
    serde_json::to_string_pretty(har)
}

/// Serialize exchanges to HAR JSON string
pub fn export_exchanges_as_har(
    exchanges: &[crate::model::CapturedExchange],
) -> serde_json::Result<String> {
    let har = exchanges_to_har(exchanges);
    har_to_json(&har)
}

/// Parse HAR JSON and extract request info for import
pub fn import_har(json: &str) -> serde_json::Result<HarFile> {
    serde_json::from_str(json)
}

fn status_text(status: i64) -> String {
    match status {
        200 => "OK".to_string(),
        201 => "Created".to_string(),
        204 => "No Content".to_string(),
        301 => "Moved Permanently".to_string(),
        302 => "Found".to_string(),
        304 => "Not Modified".to_string(),
        400 => "Bad Request".to_string(),
        401 => "Unauthorized".to_string(),
        403 => "Forbidden".to_string(),
        404 => "Not Found".to_string(),
        500 => "Internal Server Error".to_string(),
        502 => "Bad Gateway".to_string(),
        503 => "Service Unavailable".to_string(),
        _ => "Unknown".to_string(),
    }
}
