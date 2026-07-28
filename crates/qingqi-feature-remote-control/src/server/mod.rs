pub mod auth;
pub mod log_store;
pub mod routes;
pub mod ws_handler;

use log_store::LogStore;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use qingqi_plugin::database::DatabaseService;
use tokio::sync::broadcast;

use crate::protocol::requests::WsEvent;
use crate::service::RemoteControlService;
use crate::service::system::SystemService;
use crate::service::process::ProcessManager;
use crate::service::app_scanner::AppScanner;
use crate::service::window_monitor::WindowMonitor;

pub type EventSender = broadcast::Sender<WsEvent>;

#[derive(Clone)]
pub struct AppState {
    pub service: Arc<RemoteControlService>,
    pub system_service: Arc<SystemService>,
    pub process_manager: Arc<std::sync::Mutex<ProcessManager>>,
    pub token_store: Arc<auth::TokenStore>,
    pub events: EventSender,
    pub app_scanner: Arc<AppScanner>,
    pub log_store: Arc<log_store::LogStore>,
    pub window_monitor: Arc<std::sync::Mutex<WindowMonitor>>,
}

impl AppState {
    pub fn new(service: RemoteControlService, database: Arc<DatabaseService>) -> Self {
        let (tx, _) = broadcast::channel(256);
        let scanner_config_path = service.paths().config("scanner_custom_dirs.json");
        let mut window_monitor = WindowMonitor::new();
        window_monitor.set_event_sender(tx.clone());
        Self {
            service: Arc::new(service),
            system_service: Arc::new(SystemService::new()),
            process_manager: Arc::new(std::sync::Mutex::new(ProcessManager::new())),
            token_store: Arc::new(auth::TokenStore::new(database.clone())),
            events: tx,
            app_scanner: Arc::new(AppScanner::new(scanner_config_path)),
            log_store: Arc::new(LogStore::new(database)),
            window_monitor: Arc::new(std::sync::Mutex::new(window_monitor)),
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

    // Window management routes
    app = app
        .route("/api/v1/windows", axum::routing::get(routes::windows::list))
        .route("/api/v1/windows/active", axum::routing::get(routes::windows::active))
        .route("/api/v1/windows/:id/focus", axum::routing::post(routes::windows::focus))
        .route("/api/v1/windows/:id/minimize", axum::routing::post(routes::windows::minimize))
        .route("/api/v1/windows/:id/maximize", axum::routing::post(routes::windows::maximize))
        .route("/api/v1/windows/:id/restore", axum::routing::post(routes::windows::restore))
        .route("/api/v1/windows/:id/close", axum::routing::post(routes::windows::close))
        .route("/api/v1/windows/:id/move", axum::routing::post(routes::windows::r#move))
        .route("/api/v1/windows/:id/always-on-top", axum::routing::post(routes::windows::always_on_top));

    // File management routes
    app = app
        .route("/api/v1/files/browse", axum::routing::get(routes::files::browse))
        .route("/api/v1/files/quick-access", axum::routing::get(routes::files::quick_access))
        .route("/api/v1/files/download", axum::routing::get(routes::files::download))
        .route("/api/v1/files/upload", axum::routing::post(routes::files::upload))
        .route("/api/v1/files", axum::routing::delete(routes::files::delete))
        .route("/api/v1/files/rename", axum::routing::put(routes::files::rename))
        .route("/api/v1/files/create-dir", axum::routing::post(routes::files::create_dir))
        .route("/api/v1/files/info", axum::routing::get(routes::files::info));

        // Mobile Web Interface
        app = app
            .route("/", axum::routing::get(routes::web::index))
            .route("/api/v1/web/apps", axum::routing::get(routes::web::mobile_apps))
            .route("/api/v1/web/tasks", axum::routing::get(routes::web::mobile_tasks));

        // Server management routes
        app = app
            .route("/api/v1/qrcode", axum::routing::get(routes::server_mgmt::qrcode))
            .route("/api/v1/server/logs", axum::routing::get(routes::server_mgmt::list_logs))
            .route("/api/v1/server/logs", axum::routing::delete(routes::server_mgmt::clear_logs))
            .route("/api/v1/server/settings", axum::routing::get(routes::server_mgmt::get_settings))
            .route("/api/v1/server/settings", axum::routing::put(routes::server_mgmt::update_settings));

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
