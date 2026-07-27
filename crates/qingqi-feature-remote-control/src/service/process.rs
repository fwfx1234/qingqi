use sysinfo::System;

use crate::platform::{get_process_info, ProcessActions};
use crate::protocol::responses::{ForegroundResponse, ProcessInfo, ProcessListResponse};

pub struct ProcessManager {
    system: System,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            system: System::new_all(),
        }
    }

    pub fn refresh(&mut self) {
        self.system.refresh_all();
    }

    pub fn list_processes(
        &mut self,
        search: Option<&str>,
        page: usize,
        page_size: usize,
    ) -> ProcessListResponse {
        self.refresh();

        let search_lower = search.map(|s| s.to_lowercase());
        let mut processes: Vec<ProcessInfo> = self
            .system
            .processes()
            .values()
            .map(get_process_info)
            .filter(|p| {
                search_lower.as_ref().map_or(true, |q| {
                    p.name.to_lowercase().contains(q)
                        || p.pid.to_string().contains(q)
                        || p.path.as_ref().map_or(false, |path| path.to_lowercase().contains(q))
                })
            })
            .collect();

        processes.sort_by(|a, b| a.name.cmp(&b.name));
        let total = processes.len();

        let start = page * page_size;
        let paged = if start < total {
            processes[start..(start + page_size).min(total)].to_vec()
        } else {
            Vec::new()
        };

        ProcessListResponse {
            processes: paged,
            total,
            page,
            page_size,
        }
    }

    pub fn get_foreground(&self) -> anyhow::Result<ForegroundResponse> {
        crate::platform::get_foreground_window_info()
    }

    pub fn kill_process(&self, pid: u32) -> anyhow::Result<()> {
        ProcessActions::kill(pid)
    }

    pub fn suspend_process(&self, pid: u32) -> anyhow::Result<()> {
        ProcessActions::suspend(pid)
    }

    pub fn resume_process(&self, pid: u32) -> anyhow::Result<()> {
        ProcessActions::resume(pid)
    }

    pub fn set_process_priority(&self, pid: u32, priority: &str) -> anyhow::Result<()> {
        crate::platform::set_priority(pid, priority)
    }
}
