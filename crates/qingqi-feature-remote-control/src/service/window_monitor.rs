use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread;
use std::time::Duration;

use crate::platform;
use crate::protocol::requests::WsEvent;
use crate::server::EventSender;

pub struct WindowMonitor {
    is_gaming: Arc<AtomicBool>,
    watcher: Option<thread::JoinHandle<()>>,
    event_tx: Option<EventSender>,
    last_foreground: Arc<Mutex<Option<usize>>>,
    running: Arc<AtomicBool>,
}

impl WindowMonitor {
    pub fn new() -> Self {
        Self {
            is_gaming: Arc::new(AtomicBool::new(false)),
            watcher: None,
            event_tx: None,
            last_foreground: Arc::new(Mutex::new(None)),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn set_event_sender(&mut self, tx: EventSender) {
        self.event_tx = Some(tx);
    }

    pub fn start(&mut self) {
        if self.running.load(Ordering::Relaxed) {
            return;
        }
        self.running.store(true, Ordering::Relaxed);

        let is_gaming = Arc::clone(&self.is_gaming);
        let _last_foreground = Arc::clone(&self.last_foreground);
        let event_tx = self.event_tx.clone();
        let running = Arc::clone(&self.running);

        self.watcher = Some(
            thread::Builder::new()
                .name("window-monitor".into())
                .spawn(move || {
                    unsafe {
                        let handle = windows::Win32::System::Threading::GetCurrentThread();
                        let _ = windows::Win32::System::Threading::SetThreadPriority(
                            handle,
                            windows::Win32::System::Threading::THREAD_PRIORITY_LOWEST,
                        );
                    }

                    let mut debounce: Option<std::time::Instant> = None;
                    let mut last_hwnd: Option<usize> = None;

                    while running.load(Ordering::Relaxed) {
                        if is_gaming.load(Ordering::Relaxed) {
                            thread::sleep(Duration::from_secs(30));
                            debounce = None;
                            continue;
                        }

                        thread::sleep(Duration::from_secs(3));

                        if let Ok(info) = platform::get_foreground_window_info() {
                            let current_hwnd = info.pid as usize;
                            if last_hwnd.map_or(true, |h| h != current_hwnd) {
                                last_hwnd = Some(current_hwnd);
                                debounce = Some(std::time::Instant::now());
                            } else if let Some(t) = debounce {
                                if t.elapsed() > Duration::from_millis(500) {
                                    if let Some(ref tx) = event_tx {
                                        let _ = tx.send(WsEvent::ForegroundChanged {
                                            data: crate::protocol::requests::ForegroundInfo {
                                                pid: info.pid,
                                                title: info.title,
                                                path: info.path,
                                            },
                                        });
                                    }
                                    debounce = None;
                                }
                            }
                        }
                    }
                })
                .expect("failed to spawn window monitor"),
        );
    }

    pub fn set_gaming(&self, gaming: bool) {
        self.is_gaming.store(gaming, Ordering::Relaxed);
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

impl Drop for WindowMonitor {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.watcher.take() {
            let _ = handle.join();
        }
    }
}
