use serde::{Deserialize, Serialize};

/// A composed request that can be sent
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComposedRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl ComposedRequest {
    pub fn new(method: &str, url: &str) -> Self {
        Self {
            method: method.to_string(),
            url: url.to_string(),
            headers: vec![
                ("User-Agent".to_string(), "Qingqi/1.0".to_string()),
                ("Accept".to_string(), "*/*".to_string()),
            ],
            body: String::new(),
        }
    }

    /// Create a ComposedRequest from a captured exchange
    pub fn from_captured(req_headers: &[(String, String)], method: &str, url: &str, body: &str) -> Self {
        Self {
            method: method.to_string(),
            url: url.to_string(),
            headers: req_headers.to_vec(),
            body: body.to_string(),
        }
    }
}

/// Result of sending a composed request
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComposedResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub duration_ms: u64,
}

/// The request composer - manages composed requests and their history
pub struct RequestComposer {
    history: Vec<(ComposedRequest, ComposedResponse)>,
    max_history: usize,
}

impl RequestComposer {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            max_history: 50,
        }
    }

    /// Send a composed request
    pub fn send_request(&mut self, request: &ComposedRequest) -> anyhow::Result<ComposedResponse> {
        let start = std::time::Instant::now();

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .danger_accept_invalid_certs(true)
            .build()?;

        let mut req = client.request(
            reqwest::Method::from_bytes(request.method.as_bytes())?,
            &request.url,
        );

        for (key, value) in &request.headers {
            req = req.header(key, value);
        }

        if !request.body.is_empty() {
            req = req.body(request.body.clone());
        }

        let resp = req.send()?;
        let status = resp.status().as_u16();
        let headers: Vec<(String, String)> = resp.headers()
            .iter()
            .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        let body = resp.text().unwrap_or_default();
        let duration_ms = start.elapsed().as_millis() as u64;

        let response = ComposedResponse {
            status,
            headers,
            body,
            duration_ms,
        };

        // Add to history
        self.history.push((request.clone(), response.clone()));
        if self.history.len() > self.max_history {
            self.history.remove(0);
        }

        Ok(response)
    }

    /// Get request history
    pub fn history(&self) -> &[(ComposedRequest, ComposedResponse)] {
        &self.history
    }

    /// Clear history
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// Clone a captured request for editing
    pub fn clone_from_capture(
        &self,
        method: &str,
        url: &str,
        req_headers: &[(String, String)],
        req_body: &str,
    ) -> ComposedRequest {
        ComposedRequest::from_captured(req_headers, method, url, req_body)
    }
}
