pub mod auth;
pub mod routes;
pub mod ws_handler;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use tokio::sync::broadcast;

use crate::protocol::requests::WsEvent;
use crate::service::RemoteControlService;
use crate::service::system::SystemService;
use crate::service::process::ProcessManager;
use crate::service::app_scanner::AppScanner;

pub type EventSender = broadcast::Sender<WsEvent>;

#[derive(Clone)]
pub struct AppState {
    pub service: Arc<RemoteControlService>,
    pub system_service: Arc<SystemService>,
    pub process_manager: Arc<std::sync::Mutex<ProcessManager>>,
    pub token_store: Arc<auth::TokenStore>,
    pub events: EventSender,
    pub app_scanner: Arc<AppScanner>,
}

impl AppState {
    pub fn new(service: RemoteControlService) -> Self {
        let (tx, _) = broadcast::channel(256);
        let scanner_config_path = service.paths().config("scanner_custom_dirs.json");
        Self {
            service: Arc::new(service),
            system_service: Arc::new(SystemService::new()),
            process_manager: Arc::new(std::sync::Mutex::new(ProcessManager::new())),
            token_store: Arc::new(auth::TokenStore::new()),
            events: tx,
            app_scanner: Arc::new(AppScanner::new(scanner_config_path)),
        }
    }
}

pub use auth::TokenStore;

pub struct RemoteServer;

impl RemoteServer {
    pub fn create_router(state: AppState) -> Router {
        use axum::Extension;

        let mut app = Router::new();

        // Auth routes
        app = app
            .route("/api/v1/auth/pair", axum::routing::post(routes::auth::pair))
            .route("/api/v1/auth/verify", axum::routing::post(routes::auth::verify))
            .route("/api/v1/auth", axum::routing::delete(routes::auth::revoke))
            .route("/api/v1/auth/devices", axum::routing::get(routes::auth::list_devices))
            .route("/api/v1/auth/devices/:device_name", axum::routing::delete(routes::auth::revoke_device));

        // System routes
        app = app
            .route("/api/v1/system/status", axum::routing::get(routes::system::status))
            .route("/api/v1/system/mac", axum::routing::get(routes::system::mac_address))
            .route("/api/v1/system/shutdown", axum::routing::post(routes::system::shutdown))
            .route("/api/v1/system/sleep", axum::routing::post(routes::system::sleep))
            .route("/api/v1/system/restart", axum::routing::post(routes::system::restart))
            .route("/api/v1/system/logoff", axum::routing::post(routes::system::logoff))
            .route("/api/v1/system/lock", axum::routing::post(routes::system::lock));

        // Process routes
        app = app
            .route("/api/v1/processes", axum::routing::get(routes::process::list))
            .route("/api/v1/processes/foreground", axum::routing::get(routes::process::foreground))
            .route("/api/v1/processes/:pid/kill", axum::routing::post(routes::process::kill))
            .route("/api/v1/processes/:pid/suspend", axum::routing::post(routes::process::suspend))
            .route("/api/v1/processes/:pid/resume", axum::routing::post(routes::process::resume));

        // App routes
        app = app
            .route("/api/v1/apps/launch", axum::routing::post(routes::app::launch))
            .route("/api/v1/apps/search", axum::routing::get(routes::app::search));

        // App Scanner routes
        app = app
            .route("/api/v1/scanner/apps", axum::routing::get(routes::scanner::scan_apps))
            .route("/api/v1/scanner/apps/rename", axum::routing::post(routes::scanner::rename_app))
            .route("/api/v1/scanner/apps/:id/launch", axum::routing::post(routes::scanner::launch_app))
            .route("/api/v1/scanner/refresh", axum::routing::post(routes::scanner::refresh_apps));

        // Steam routes
        app = app
            .route("/api/v1/scanner/steam", axum::routing::get(routes::steam::list_games))
            .route("/api/v1/scanner/steam/refresh", axum::routing::post(routes::steam::refresh))
            .route("/api/v1/scanner/steam/:app_id/launch", axum::routing::post(routes::steam::launch_game));

        // Custom directory routes
        app = app
            .route("/api/v1/scanner/custom-dirs", axum::routing::get(routes::custom_dir::list))
            .route("/api/v1/scanner/custom-dirs", axum::routing::post(routes::custom_dir::add))
            .route("/api/v1/scanner/custom-dirs/validate", axum::routing::post(routes::custom_dir::validate))
            .route("/api/v1/scanner/custom-dirs/:id", axum::routing::put(routes::custom_dir::update))
            .route("/api/v1/scanner/custom-dirs/:id", axum::routing::delete(routes::custom_dir::remove));

        // Task Manager routes
        app = app
            .route("/api/v1/tasks", axum::routing::get(routes::task::list_tasks))
            .route("/api/v1/tasks/:pid/kill", axum::routing::post(routes::task::kill_task))
            .route("/api/v1/tasks/:pid/priority", axum::routing::post(routes::task::set_priority))
            .route("/api/v1/tasks/stats", axum::routing::get(routes::task::system_stats));

        // Mobile Web Interface
        app = app
            .route("/", axum::routing::get(routes::web::index))
            .route("/api/v1/web/apps", axum::routing::get(routes::web::mobile_apps))
            .route("/api/v1/web/tasks", axum::routing::get(routes::web::mobile_tasks));

        // WebSocket
        app = app.route("/api/v1/events", axum::routing::get(ws_handler::ws_handler));

        // CORS
        app = app.layer(
            tower_http::cors::CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any),
        );

        // Request logging
        app = app.layer(tower_http::trace::TraceLayer::new_for_http()
            .make_span_with(|req: &axum::http::Request<_>| {
                tracing::info_span!(
                    "http",
                    method = %req.method(),
                    uri = %req.uri(),
                )
            })
            .on_response(|resp: &axum::http::Response<_>, latency: std::time::Duration, _span: &tracing::Span| {
                tracing::info!(
                    "[远程控制] <- {} in {:?}",
                    resp.status().as_u16(),
                    latency,
                );
            }));

        // Provide state via Extension
        app.layer(Extension(state))
    }

    pub async fn run(state: AppState, port: u16) -> anyhow::Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
        let app = Self::create_router(state);
        tracing::info!("[远程控制] 正在尝试绑定端口 {}...", port);
        let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
        let addr = listener.local_addr()?;
        tracing::info!("[远程控制] 服务器已绑定: {} (0.0.0.0:{})", addr, port);
        let handle = tokio::spawn(async move {
            tracing::info!("[远程控制] HTTP 服务开始监听...");
            let serve = axum::serve(listener, app);
            let _ = serve.into_future().await;
            tracing::warn!("[远程控制] HTTP 服务已停止");
        });
        Ok((addr, handle))
    }
}
