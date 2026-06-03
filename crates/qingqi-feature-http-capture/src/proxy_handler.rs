use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use http_body_util::BodyExt;
use hudsucker::{
    Body, HttpContext, HttpHandler, RequestOrResponse,
    hyper::{Request, Response},
};
use serde_json;

use crate::{mock_engine::MockEngine, store::CaptureStore};
use qingqi_plugin::events::{AppEventBus, AppEventKind};

/// 请求/响应体最大截取大小（1 MB）。
const MAX_BODY_SIZE: usize = 1_024 * 1_024;

/// hudsucker HTTP 处理器 — 捕获请求/响应数据并写入存储。
///
/// 在 `handle_request` 中先检查 Mock 规则，若匹配则直接返回模拟响应；
/// 否则转发到真实服务器。在 `handle_response` 中提取响应数据并持久化。
#[derive(Clone)]
pub struct ProxyHttpHandler {
    store: Arc<Mutex<CaptureStore>>,
    mock_engine: Arc<MockEngine>,
    events: AppEventBus,
}

impl ProxyHttpHandler {
    pub fn new(
        store: Arc<Mutex<CaptureStore>>,
        mock_engine: Arc<MockEngine>,
        events: AppEventBus,
    ) -> Self {
        Self {
            store,
            mock_engine,
            events,
        }
    }

    /// 将捕获的交换记录写入存储并通知 UI。
    fn capture_and_store(
        &self,
        method: &str,
        url: &str,
        host: &str,
        status: i64,
        protocol: &str,
        duration_ms: i64,
        request_size: i64,
        response_size: i64,
        request_headers_json: &str,
        response_headers_json: &str,
        request_body: &str,
        response_body: &str,
        is_https: bool,
    ) {
        if let Ok(store) = self.store.lock() {
            let _ = store.insert(
                method,
                url,
                host,
                status,
                protocol,
                duration_ms,
                request_size,
                response_size,
                request_headers_json,
                response_headers_json,
                request_body,
                response_body,
                is_https,
            );
        }

        // 通知 UI 刷新
        self.events
            .publish("http-capture", AppEventKind::FeatureChanged);
    }

    /// 将 hyper HeaderMap 转换为 JSON 字符串。
    fn headers_to_json(headers: &hyper::HeaderMap) -> String {
        let entries: Vec<(String, String)> = headers
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str().to_string(),
                    v.to_str().unwrap_or("[binary]").to_string(),
                )
            })
            .collect();
        serde_json::to_string(&entries).unwrap_or_else(|_| "[]".to_string())
    }

    /// 截断过大的 body，超过 MAX_BODY_SIZE 部分替换为提示。
    fn truncate_body(bytes: &[u8]) -> String {
        if bytes.len() <= MAX_BODY_SIZE {
            String::from_utf8_lossy(bytes).to_string()
        } else {
            let truncated = String::from_utf8_lossy(&bytes[..MAX_BODY_SIZE]).to_string();
            format!(
                "{truncated}\n\n[截断: 原始大小 {} MB]",
                bytes.len() as f64 / (1024.0 * 1024.0)
            )
        }
    }

    /// 从 URL 中提取 Host。
    fn extract_host(uri: &hyper::Uri) -> String {
        uri.host().unwrap_or("unknown").to_string()
    }
}

impl HttpHandler for ProxyHttpHandler {
    async fn handle_request(
        &mut self,
        _ctx: &HttpContext,
        req: Request<Body>,
    ) -> RequestOrResponse {
        let start = Instant::now();

        let method = req.method().to_string();
        let url = req.uri().to_string();
        let host = Self::extract_host(req.uri());
        let is_https = req.uri().scheme_str() == Some("https");
        let protocol = format!("{:?}", req.version());

        // 提取请求头
        let req_headers_json = Self::headers_to_json(req.headers());
        let req_headers: Vec<(String, String)> = req
            .headers()
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str().to_string(),
                    v.to_str().unwrap_or("[binary]").to_string(),
                )
            })
            .collect();

        // 1. 检查 Mock 规则 — 匹配时直接返回模拟响应
        if let Some(mock_result) = self.mock_engine.match_request(&method, &url, &req_headers) {
            // 模拟延迟
            if mock_result.delay_ms > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(
                    mock_result.delay_ms as u64,
                ))
                .await;
            }

            let duration_ms = start.elapsed().as_millis() as i64;
            let body_bytes = mock_result.body.as_bytes();
            let response_headers_json =
                serde_json::to_string(&mock_result.headers).unwrap_or_else(|_| "[]".to_string());

            // 记录 Mock 交换
            self.capture_and_store(
                &method,
                &url,
                &host,
                mock_result.status,
                &protocol,
                duration_ms,
                0, // 请求大小（Mock 不记录真实请求体）
                body_bytes.len() as i64,
                &req_headers_json,
                &response_headers_json,
                "",
                &mock_result.body,
                is_https,
            );

            // 构建模拟响应
            let mut resp_builder = Response::builder().status(
                hyper::StatusCode::from_u16(mock_result.status as u16)
                    .unwrap_or(hyper::StatusCode::OK),
            );

            for (name, value) in &mock_result.headers {
                resp_builder = resp_builder.header(name.as_str(), value.as_str());
            }

            let resp = resp_builder.body(Body::from(mock_result.body)).unwrap();

            return RequestOrResponse::Response(resp);
        }

        // 2. 提取请求体
        let (parts, body) = req.into_parts();
        let collected = match body.collect().await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("读取请求体失败: {e}");
                return RequestOrResponse::Request(Request::from_parts(parts, Body::empty()));
            }
        };
        let body_bytes = collected.to_bytes();
        let request_size = body_bytes.len() as i64;
        let request_body = Self::truncate_body(&body_bytes);

        // 存储开始时间到 parts.extensions 中（通过 ctx 或自定义方式传递）
        // hudsucker 的 HttpContext 不直接支持携带自定义数据，
        // 我们使用 URL 作为关联键 + 时间戳记录的方式
        // 为简化实现，这里将请求信息存到局部变量，在 handle_response 中通过 URL 关联

        // 重建 Request 并返回
        let mut new_req = Request::from_parts(parts, Body::from(body_bytes));

        // 将捕获信息存入 extensions（供 handle_response 使用）
        new_req.extensions_mut().insert(CaptureContext {
            method: method.clone(),
            url: url.clone(),
            host: host.clone(),
            protocol: protocol.clone(),
            is_https,
            start,
            request_size,
            request_body,
            req_headers_json,
        });

        RequestOrResponse::Request(new_req)
    }

    async fn handle_response(&mut self, _ctx: &HttpContext, res: Response<Body>) -> Response<Body> {
        // 提取响应体
        let (parts, body) = res.into_parts();
        let collected = match body.collect().await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("读取响应体失败: {e}");
                return Response::from_parts(parts, Body::empty());
            }
        };
        let body_bytes = collected.to_bytes();
        let response_size = body_bytes.len() as i64;
        let response_body = Self::truncate_body(&body_bytes);
        let response_headers_json = Self::headers_to_json(&parts.headers);
        let status = parts.status.as_u16() as i64;

        // 尝试从 extensions 中提取捕获上下文
        let capture_ctx = parts.extensions.get::<CaptureContext>();

        match capture_ctx {
            Some(ctx) => {
                let duration_ms = ctx.start.elapsed().as_millis() as i64;

                self.capture_and_store(
                    &ctx.method,
                    &ctx.url,
                    &ctx.host,
                    status,
                    &ctx.protocol,
                    duration_ms,
                    ctx.request_size,
                    response_size,
                    &ctx.req_headers_json,
                    &response_headers_json,
                    &ctx.request_body,
                    &response_body,
                    ctx.is_https,
                );
            }
            None => {
                // 无法关联请求上下文（罕见情况），仍然记录响应
                tracing::debug!("响应无法关联到请求");
            }
        }

        // 重建 Response
        let mut new_res = Response::from_parts(parts, Body::from(body_bytes));
        // 移除我们注入的 extension
        new_res.extensions_mut().remove::<CaptureContext>();
        new_res
    }
}

/// 捕获上下文 — 通过 Request extensions 在 handle_request 和 handle_response 之间传递信息。
#[derive(Clone)]
struct CaptureContext {
    method: String,
    url: String,
    host: String,
    protocol: String,
    is_https: bool,
    start: Instant,
    request_size: i64,
    request_body: String,
    req_headers_json: String,
}
