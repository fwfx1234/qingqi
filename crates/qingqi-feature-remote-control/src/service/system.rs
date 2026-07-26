use std::sync::Mutex;

use sysinfo::System;

use crate::platform::SystemController;
use crate::protocol::responses::SystemStatus;

pub struct SystemService {
    system: Mutex<System>,
}

impl SystemService {
    pub fn new() -> Self {
        Self {
            system: Mutex::new(System::new_all()),
        }
    }

    pub fn get_status(&self) -> SystemStatus {
        let mut system = self.system.lock().unwrap();
        system.refresh_all();

        let cpu_usage = system.global_cpu_usage();
        let memory_total = system.total_memory();
        let memory_used = system.used_memory();
        let memory_percent = if memory_total > 0 {
            (memory_used as f32 / memory_total as f32) * 100.0
        } else {
            0.0
        };
        let uptime_seconds = System::uptime();

        SystemStatus {
            cpu_usage,
            memory_total,
            memory_used,
            memory_percent,
            uptime_seconds,
        }
    }

    /// 获取本机 MAC 地址
    pub fn get_mac_address(&self) -> Option<String> {
        use local_ip_address::local_ip_address_list;

        let interfaces = local_ip_address_list();
        // 优先返回非回环接口的 MAC 地址
        interfaces
            .iter()
            .find(|iface| !iface.is_loopback() && iface.mac().is_some())
            .and_then(|iface| iface.mac())
    }

    pub fn shutdown(&self, force: bool, delay_secs: u64) -> anyhow::Result<()> {
        SystemController::shutdown(force, delay_secs)
    }

    pub fn sleep(&self, hibernate: bool) -> anyhow::Result<()> {
        SystemController::sleep(hibernate)
    }

    pub fn restart(&self, force: bool) -> anyhow::Result<()> {
        SystemController::restart(force)
    }

    pub fn logoff(&self) -> anyhow::Result<()> {
        SystemController::logoff()
    }

    pub fn lock(&self) -> anyhow::Result<()> {
        SystemController::lock()
    }
}
