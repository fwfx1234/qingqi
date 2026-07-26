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

pub type EventSender = broadcast::Sender<WsEvent>;

#[derive(Clone)]
pub struct AppState {
    pub service: Arc<RemoteControlService>,
    pub system_service: Arc<SystemService>,
    pub process_manager: Arc<std::sync::Mutex<ProcessManager>>,
    pub token_store: Arc<auth::TokenStore>,
    pub events: EventSender,
}

impl AppState {
    pub fn new(service: RemoteControlService) -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            service: Arc::new(service),
            system_service: Arc::new(SystemService::new()),
            process_manager: Arc::new(std::sync::Mutex::new(ProcessManager::new())),
            token_store: Arc::new(auth::TokenStore::new()),
            events: tx,
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
            .route("/api/v1/auth", axum::routing::delete(routes::auth::revoke));

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

        // WebSocket
        app = app.route("/api/v1/events", axum::routing::get(ws_handler::ws_handler));

        // CORS
        app = app.layer(
            tower_http::cors::CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any),
        );

        // Provide state via Extension
        app.layer(Extension(state))
    }

    pub async fn run(state: AppState, port: u16) -> anyhow::Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
        let app = Self::create_router(state);
        let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
        let addr = listener.local_addr()?;
        let handle = tokio::spawn(async move {
            let serve = axum::serve(listener, app);
            let _ = serve.into_future().await;
        });
        Ok((addr, handle))
    }
}
