use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
    mpsc::{Receiver, Sender, channel},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppEventKind {
    FeatureChanged,
    CommandsChanged,
    JobsChanged,
    /// 托盘项被点击（provider_id）
    TrayClicked,
    /// 托盘弹窗已显示（provider_id）
    TrayPopupShown,
    /// 托盘弹窗已关闭（provider_id）
    TrayPopupClosed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppEvent {
    pub revision: u64,
    pub source: Arc<str>,
    pub kind: AppEventKind,
}

#[derive(Clone, Default)]
pub struct AppEventBus {
    revision: Arc<AtomicU64>,
    last_event: Arc<Mutex<Option<AppEvent>>>,
    subscribers: Arc<Mutex<Vec<Sender<AppEvent>>>>,
}

impl AppEventBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn publish(&self, source: impl Into<Arc<str>>, kind: AppEventKind) -> u64 {
        let revision = self.revision.fetch_add(1, Ordering::SeqCst) + 1;
        let event = AppEvent {
            revision,
            source: source.into(),
            kind,
        };
        if let Ok(mut last_event) = self.last_event.lock() {
            *last_event = Some(event.clone());
        }
        if let Ok(mut subscribers) = self.subscribers.lock() {
            subscribers.retain(|subscriber| subscriber.send(event.clone()).is_ok());
        }
        revision
    }

    pub fn subscribe(&self) -> Receiver<AppEvent> {
        let (sender, receiver) = channel();
        if let Ok(mut subscribers) = self.subscribers.lock() {
            subscribers.push(sender);
        }
        receiver
    }

    #[allow(dead_code)]
    pub fn last_event(&self) -> Option<AppEvent> {
        self.last_event.lock().ok().and_then(|event| event.clone())
    }
}
