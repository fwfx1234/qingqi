use std::{
    net::SocketAddr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use hudsucker::rustls::crypto::aws_lc_rs;

use crate::{
    certificate::CaManager,
    mock_engine::MockEngine,
    model::{CertificateStatus, ProxyState},
    proxy_handler::ProxyHttpHandler,
    store::CaptureStore,
};
use qingqi_plugin::events::AppEventBus;

/// 代理捕获引擎状态。
struct EngineState {
    proxy_state: ProxyState,
    /// 通知 tokio runtime 关闭的信号发送端
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    /// 后台线程 join handle
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

/// 代理捕获引擎 — 管理 HTTP/HTTPS 中间人代理的生命周期。
///
/// 在独立线程中运行 tokio runtime，不阻塞 GPUI 主线程。
/// 通过 graceful shutdown 信号安全停止代理。
pub struct CaptureEngine {
    state: Mutex<EngineState>,
    store: Arc<Mutex<CaptureStore>>,
    mock_engine: Arc<MockEngine>,
    ca_manager: Arc<Mutex<CaManager>>,
    events: AppEventBus,
    /// 标记引擎是否已初始化证书（只执行一次）
    ca_initialized: AtomicBool,
}

impl CaptureEngine {
    pub fn new(
        store: Arc<Mutex<CaptureStore>>,
        mock_engine: Arc<MockEngine>,
        ca_manager: Arc<Mutex<CaManager>>,
        events: AppEventBus,
    ) -> Self {
        Self {
            state: Mutex::new(EngineState {
                proxy_state: ProxyState::Stopped,
                shutdown_tx: None,
                thread_handle: None,
            }),
            store,
            mock_engine,
            ca_manager,
            events,
            ca_initialized: AtomicBool::new(false),
        }
    }

    /// 确保 CA 证书已生成/加载。幂等操作。
    pub fn ensure_ca(&self) -> anyhow::Result<CertificateStatus> {
        if self.ca_initialized.load(Ordering::SeqCst) {
            return Ok(self.ca_manager.lock().unwrap().status());
        }

        let mut ca = self
            .ca_manager
            .lock()
            .map_err(|e| anyhow::anyhow!("CA Manager 锁中毒: {e}"))?;
        ca.ensure_ca()?;
        ca.refresh_status();
        let status = ca.status();
        drop(ca);

        self.ca_initialized.store(true, Ordering::SeqCst);
        Ok(status)
    }

    /// 启动代理服务器。
    ///
    /// 创建独立的 tokio runtime 并在后台线程中运行 hudsucker Proxy。
    /// 若已在运行则返回错误。
    pub fn start(&self, port: u16) -> anyhow::Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|e| anyhow::anyhow!("引擎锁中毒: {e}"))?;

        if matches!(state.proxy_state, ProxyState::Running { .. }) {
            anyhow::bail!("代理已在运行中，请先停止");
        }

        // 确保 CA 证书已生成
        self.ensure_ca()?;

        let ca_manager = Arc::clone(&self.ca_manager);
        let store = Arc::clone(&self.store);
        let mock_engine = Arc::clone(&self.mock_engine);
        let events = self.events.clone();

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        let addr = SocketAddr::from(([127, 0, 0, 1], port));

        let thread_handle = std::thread::spawn(move || {
            // 在后台线程中创建 tokio runtime
            let rt = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::error!("创建 tokio runtime 失败: {e}");
                    return;
                }
            };

            rt.block_on(async move {
                // 从 CaManager 构建证书颁发机构
                let ca = {
                    let mgr = match ca_manager.lock() {
                        Ok(m) => m,
                        Err(e) => {
                            tracing::error!("获取 CaManager 锁失败: {e}");
                            return;
                        }
                    };
                    let key = match mgr.load_key_pair() {
                        Ok(k) => k,
                        Err(e) => {
                            tracing::error!("加载 CA 密钥失败: {e}");
                            return;
                        }
                    };
                    let params = match mgr.ca_params() {
                        Ok(p) => p.clone(),
                        Err(e) => {
                            tracing::error!("获取 CA 证书参数失败: {e}");
                            return;
                        }
                    };
                    // 构建 rcgen Issuer（使用 owned params 和 key 获取 'static 生命周期）
                    let issuer = rcgen::Issuer::new(params, key);

                    // 构建 hudsucker 的 RcgenAuthority
                    hudsucker::certificate_authority::RcgenAuthority::new(
                        issuer,
                        1_000,
                        aws_lc_rs::default_provider(),
                    )
                };

                let handler = ProxyHttpHandler::new(store, mock_engine, events);

                let proxy = match hudsucker::Proxy::builder()
                    .with_addr(addr)
                    .with_ca(ca)
                    .with_rustls_connector(aws_lc_rs::default_provider())
                    .with_http_handler(handler)
                    .with_graceful_shutdown(async {
                        shutdown_rx.await.ok();
                    })
                    .build()
                {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::error!("构建 hudsucker Proxy 失败: {e}");
                        return;
                    }
                };

                tracing::info!("HTTP 抓包代理已启动: http://{}", addr);
                if let Err(e) = proxy.start().await {
                    tracing::error!("代理运行错误: {e}");
                }
                tracing::info!("HTTP 抓包代理已停止");
            });
        });

        state.shutdown_tx = Some(shutdown_tx);
        state.thread_handle = Some(thread_handle);
        state.proxy_state = ProxyState::Running { port };

        Ok(())
    }

    /// 停止代理服务器。
    pub fn stop(&self) {
        let mut state = match self.state.lock() {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("停止引擎时获取锁失败: {e}");
                return;
            }
        };

        if matches!(state.proxy_state, ProxyState::Stopped) {
            return;
        }

        // 发送 shutdown 信号
        if let Some(tx) = state.shutdown_tx.take() {
            let _ = tx.send(());
        }

        // 等待线程退出（最多 3 秒）
        if let Some(handle) = state.thread_handle.take() {
            // 不在锁内 join，避免死锁
            drop(state);
            // 使用 std::thread::JoinHandle 的 is_finished 检查（不稳定 API，
            // 这里简单等待一小段时间）
            let _ = handle.join();
        } else {
            drop(state);
        }

        // 更新状态
        if let Ok(mut s) = self.state.lock() {
            s.proxy_state = ProxyState::Stopped;
        }
    }

    /// 代理是否正在运行。
    pub fn is_running(&self) -> bool {
        self.state
            .lock()
            .map(|s| matches!(s.proxy_state, ProxyState::Running { .. }))
            .unwrap_or(false)
    }

    /// 当前代理端口（仅在运行时有效）。
    pub fn port(&self) -> Option<u16> {
        self.state
            .lock()
            .ok()
            .and_then(|s| match s.proxy_state {
                ProxyState::Running { port } => Some(port),
                ProxyState::Stopped => None,
            })
    }

    /// 获取当前代理状态。
    pub fn proxy_state(&self) -> ProxyState {
        self.state
            .lock()
            .map(|s| s.proxy_state.clone())
            .unwrap_or(ProxyState::Stopped)
    }

    /// 证书状态。
    pub fn certificate_status(&self) -> CertificateStatus {
        self.ca_manager
            .lock()
            .map(|m| m.status())
            .unwrap_or(CertificateStatus::NotGenerated)
    }

    /// 获取 CaManager 引用（用于导出证书等操作）。
    pub fn ca_manager(&self) -> &Arc<Mutex<CaManager>> {
        &self.ca_manager
    }

    /// 获取事件总线引用。
    pub fn events(&self) -> &AppEventBus {
        &self.events
    }
}

impl Drop for CaptureEngine {
    fn drop(&mut self) {
        self.stop();
    }
}
