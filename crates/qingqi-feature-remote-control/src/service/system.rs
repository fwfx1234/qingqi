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

    /// 获取本机 MAC 地址（支持 Wake-on-LAN 的物理网卡）
    pub fn get_mac_address(&self) -> Option<String> {
        // Use windows crate to get MAC address via GetAdaptersAddresses
        use windows::Win32::NetworkManagement::IpHelper::{
            GetAdaptersAddresses, GAA_FLAG_INCLUDE_PREFIX, IP_ADAPTER_ADDRESSES_LH,
        };
        use windows::Win32::Networking::WinSock::AF_UNSPEC;
        use std::alloc::{alloc, dealloc, Layout};

        // 虚拟网卡描述关键词（不区分大小写匹配）
        const VIRTUAL_ADAPTER_KEYWORDS: &[&str] = &[
            "virtual",
            "vmware",
            "hyper-v",
            "hyperv",
            "wsl",
            "vpn",
            "tap",
            "tun",
            "loopback",
            "bluetooth",
            "miniport",
            "ndis",
            "wan miniport",
            "microsoft kernel debug",
            "remote ndis",
            "virtualbox",
            "vbox",
            "ppp",
            "ras",
        ];

        // 检查是否为虚拟网卡
        fn is_virtual_adapter(description: &str) -> bool {
            let desc_lower = description.to_lowercase();
            VIRTUAL_ADAPTER_KEYWORDS.iter().any(|kw| desc_lower.contains(kw))
        }

        unsafe {
            let mut size: u32 = 0;
            let result = GetAdaptersAddresses(
                AF_UNSPEC.0 as u32,
                GAA_FLAG_INCLUDE_PREFIX,
                None,
                None,
                &mut size,
            );
            if result != 0x000000EA && size == 0 {
                // ERROR_BUFFER_OVERFLOW = 0x6F (111)
                return None;
            }

            let layout = Layout::from_size_align_unchecked(size as usize, 8);
            let buffer = alloc(layout);
            if buffer.is_null() {
                return None;
            }

            let result = GetAdaptersAddresses(
                AF_UNSPEC.0 as u32,
                GAA_FLAG_INCLUDE_PREFIX,
                None,
                Some(buffer as *mut IP_ADAPTER_ADDRESSES_LH),
                &mut size,
            );

            if result != 0 {
                dealloc(buffer, layout);
                return None;
            }

            // 第一遍：优先查找物理以太网适配器（IfType = 6，IF_TYPE_ETHERNET_CSMACD）
            let mut adapter = buffer as *mut IP_ADAPTER_ADDRESSES_LH;
            while !adapter.is_null() {
                let physical_address = (*adapter).PhysicalAddress;
                let physical_address_length = (*adapter).PhysicalAddressLength as usize;
                let if_type = (*adapter).IfType;
                
                // 只考虑有 MAC 地址的适配器
                if physical_address_length >= 6 {
                    // 获取适配器描述
                    let description = (*adapter).Description;
                    let desc_str = {
                        let mut len = 0;
                        while *description.0.add(len) != 0 {
                            len += 1;
                        }
                        let slice: &[u16] = std::slice::from_raw_parts(description.0 as *const u16, len);
                        String::from_utf16_lossy(slice)
                    };
                    
                    // 过滤掉虚拟网卡
                    if !is_virtual_adapter(&desc_str) {
                        // 优先选择以太网适配器（IfType = 6）
                        if if_type == 6 {
                            let mac = format!(
                                "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                                physical_address[0],
                                physical_address[1],
                                physical_address[2],
                                physical_address[3],
                                physical_address[4],
                                physical_address[5]
                            );
                            dealloc(buffer, layout);
                            return Some(mac);
                        }
                    }
                }
                adapter = (*adapter).Next;
            }

            // 第二遍：如果没有找到以太网适配器，找任何非虚拟的物理网卡
            adapter = buffer as *mut IP_ADAPTER_ADDRESSES_LH;
            while !adapter.is_null() {
                let physical_address = (*adapter).PhysicalAddress;
                let physical_address_length = (*adapter).PhysicalAddressLength as usize;
                
                if physical_address_length >= 6 {
                    let description = (*adapter).Description;
                    let desc_str = {
                        let mut len = 0;
                        while *description.0.add(len) != 0 {
                            len += 1;
                        }
                        let slice: &[u16] = std::slice::from_raw_parts(description.0 as *const u16, len);
                        String::from_utf16_lossy(slice)
                    };
                    
                    if !is_virtual_adapter(&desc_str) {
                        let mac = format!(
                            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                            physical_address[0],
                            physical_address[1],
                            physical_address[2],
                            physical_address[3],
                            physical_address[4],
                            physical_address[5]
                        );
                        dealloc(buffer, layout);
                        return Some(mac);
                    }
                }
                adapter = (*adapter).Next;
            }

            dealloc(buffer, layout);
            None
        }
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
