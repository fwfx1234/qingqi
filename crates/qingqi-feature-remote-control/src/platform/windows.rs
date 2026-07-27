// Windows-specific implementations using Win32 API

use std::mem;

use windows::Win32::System::Shutdown::{
    ExitWindowsEx, LockWorkStation, EWX_FORCE, EWX_FORCEIFHUNG, EWX_LOGOFF, EWX_REBOOT,
    EWX_SHUTDOWN, SHUTDOWN_REASON,
};
use windows::Win32::System::Power::SetSuspendState;
use windows::Win32::System::Threading::{
    OpenProcess, OpenProcessToken, PROCESS_QUERY_INFORMATION, PROCESS_TERMINATE,
    PROCESS_VM_READ,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId, SW_SHOWNORMAL,
};
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND};
use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
use windows::Win32::Security::{
    AdjustTokenPrivileges, LookupPrivilegeValueW, SE_PRIVILEGE_ENABLED,
    TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};

use crate::protocol::responses::{AppInfo, ForegroundResponse};

/// Enable a privilege for the current process token.
fn enable_privilege(name: windows::core::PCSTR) -> anyhow::Result<()> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(
            windows::Win32::System::Threading::GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token,
        )?;

        let mut luid = windows::Win32::Foundation::LUID::default();
        let name_wide: Vec<u16> = name.to_string().unwrap().encode_utf16().chain(Some(0)).collect();
        let name_ptr = windows::core::PCWSTR(name_wide.as_ptr());
        LookupPrivilegeValueW(None, name_ptr, &mut luid)?;

        let tp = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [windows::Win32::Security::LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };
        AdjustTokenPrivileges(token, false, Some(&tp), 0, None, None)?;
        let _ = CloseHandle(token);
    }
    Ok(())
}

pub fn suspend(pid: u32) -> anyhow::Result<()> {
    // Use NtSuspendProcess via undocumented API
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_INFORMATION, false, pid)?;
        if process.is_invalid() {
            anyhow::bail!("Failed to open process {pid}");
        }
        // Call NtSuspendProcess through ntdll
        let result = call_nt_suspend_process(process);
        CloseHandle(process)?;
        result
    }
}

pub fn resume(pid: u32) -> anyhow::Result<()> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_INFORMATION, false, pid)?;
        if process.is_invalid() {
            anyhow::bail!("Failed to open process {pid}");
        }
        let result = call_nt_resume_process(process);
        CloseHandle(process)?;
        result
    }
}

pub fn kill(pid: u32) -> anyhow::Result<()> {
    unsafe {
        let process = OpenProcess(PROCESS_TERMINATE, false, pid)?;
        if process.is_invalid() {
            anyhow::bail!("Failed to open process {pid}");
        }
        windows::Win32::System::Threading::TerminateProcess(process, 1)?;
        CloseHandle(process)?;
        Ok(())
    }
}

pub fn shutdown(force: bool, _delay_secs: u64) -> anyhow::Result<()> {
    enable_privilege(windows::core::s!("SeShutdownPrivilege"))?;
    let flags = if force {
        EWX_SHUTDOWN | EWX_FORCEIFHUNG
    } else {
        EWX_SHUTDOWN | EWX_FORCE
    };
    unsafe {
        ExitWindowsEx(flags, SHUTDOWN_REASON(0))?;
    }
    Ok(())
}

pub fn sleep(hibernate: bool) -> anyhow::Result<()> {
    unsafe {
        let result = SetSuspendState(hibernate, true, false);
        if !result {
            anyhow::bail!("Failed to set suspend state");
        }
    }
    Ok(())
}

pub fn restart(force: bool) -> anyhow::Result<()> {
    enable_privilege(windows::core::s!("SeShutdownPrivilege"))?;
    let flags = if force {
        EWX_REBOOT | EWX_FORCEIFHUNG
    } else {
        EWX_REBOOT | EWX_FORCE
    };
    unsafe {
        ExitWindowsEx(flags, SHUTDOWN_REASON(0))?;
    }
    Ok(())
}

pub fn logoff() -> anyhow::Result<()> {
    unsafe {
        ExitWindowsEx(EWX_LOGOFF | EWX_FORCEIFHUNG, SHUTDOWN_REASON(0))?;
    }
    Ok(())
}

pub fn lock() -> anyhow::Result<()> {
    unsafe {
        LockWorkStation()?;
    }
    Ok(())
}

pub fn get_foreground_window_info() -> anyhow::Result<ForegroundResponse> {
    unsafe {
        let hwnd: HWND = GetForegroundWindow();
        if hwnd.is_invalid() {
            anyhow::bail!("No foreground window");
        }

        // Get window title
        let mut title_buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, &mut title_buf);
        let title = String::from_utf16_lossy(&title_buf[..len as usize]);

        // Get PID
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));

        // Get process path
        let path = if pid != 0 {
            let process = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid);
            if let Ok(handle) = process {
                if !handle.is_invalid() {
                    let mut path_buf = [0u16; 260];
                    let len = GetModuleFileNameExW(Some(handle), None, &mut path_buf);
                    let _ = CloseHandle(handle);
                    if len > 0 {
                        Some(String::from_utf16_lossy(&path_buf[..len as usize]))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let process_name = if let Some(ref p) = path {
            std::path::Path::new(p)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string()
        } else {
            String::new()
        };

        Ok(ForegroundResponse {
            pid,
            title,
            path,
            process_name,
        })
    }
}

pub fn launch_app(path: &str, args: &[String]) -> anyhow::Result<()> {
    use windows::Win32::UI::Shell::ShellExecuteW;

    let path_wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
    let args_wide: Vec<u16> = args.join(" ").encode_utf16().chain(Some(0)).collect();
    let operation: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();

    unsafe {
        let result = ShellExecuteW(
            None,
            windows::core::PCWSTR(operation.as_ptr()),
            windows::core::PCWSTR(path_wide.as_ptr()),
            windows::core::PCWSTR(if args.is_empty() {
                std::ptr::null()
            } else {
                args_wide.as_ptr()
            }),
            None,
            SW_SHOWNORMAL,
        );
        if result.is_invalid() || (result.0 as usize) <= 32 {
            anyhow::bail!("ShellExecuteW failed with code {}", result.0 as usize);
        }
    }
    Ok(())
}

pub fn search_installed_apps(query: &str) -> Vec<AppInfo> {
    use std::fs;

    let query_lower = query.to_lowercase();
    let mut results = Vec::new();

    let dirs = [
        format!(
            "{}\\Microsoft\\Windows\\Start Menu",
            std::env::var("ProgramData").unwrap_or_default()
        ),
        format!(
            "{}\\Microsoft\\Windows\\Start Menu",
            std::env::var("APPDATA").unwrap_or_default()
        ),
    ];

    for dir in &dirs {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_file() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if query.is_empty() || name.to_lowercase().contains(&query_lower) {
                            results.push(AppInfo {
                                name: name.trim_end_matches(".lnk").to_string(),
                                path: path.to_string_lossy().to_string(),
                            });
                        }
                    } else if metadata.is_dir() {
                        if let Ok(sub) = fs::read_dir(&path) {
                            for sub_entry in sub.flatten() {
                                if sub_entry.metadata().map(|m| m.is_file()).unwrap_or(false) {
                                    let name =
                                        sub_entry.file_name().to_string_lossy().to_string();
                                    if query.is_empty()
                                        || name.to_lowercase().contains(&query_lower)
                                    {
                                        results.push(AppInfo {
                                            name: name.trim_end_matches(".lnk").to_string(),
                                            path: sub_entry.path().to_string_lossy().to_string(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    results.truncate(100);
    results
}

// Helper: call NtSuspendProcess from ntdll.dll
unsafe fn call_nt_suspend_process(process: HANDLE) -> anyhow::Result<()> {
    let ntdll = unsafe {
        windows::Win32::System::LibraryLoader::GetModuleHandleW(windows::core::w!("ntdll.dll"))?
    };
    let proc_addr = unsafe {
        windows::Win32::System::LibraryLoader::GetProcAddress(ntdll, windows::core::s!("NtSuspendProcess"))
    };
    if let Some(func) = proc_addr {
        let f: unsafe extern "system" fn(HANDLE) -> i32 = unsafe { mem::transmute(func) };
        let status = unsafe { f(process) };
        if status < 0 {
            anyhow::bail!("NtSuspendProcess failed with status 0x{status:08X}");
        }
    } else {
        anyhow::bail!("NtSuspendProcess not found");
    }
    Ok(())
}

// Helper: call NtResumeProcess from ntdll.dll
unsafe fn call_nt_resume_process(process: HANDLE) -> anyhow::Result<()> {
    let ntdll = unsafe {
        windows::Win32::System::LibraryLoader::GetModuleHandleW(windows::core::w!("ntdll.dll"))?
    };
    let proc_addr = unsafe {
        windows::Win32::System::LibraryLoader::GetProcAddress(ntdll, windows::core::s!("NtResumeProcess"))
    };
    if let Some(func) = proc_addr {
        let f: unsafe extern "system" fn(HANDLE) -> i32 = unsafe { mem::transmute(func) };
        let status = unsafe { f(process) };
        if status < 0 {
            anyhow::bail!("NtResumeProcess failed with status 0x{status:08X}");
        }
    } else {
        anyhow::bail!("NtResumeProcess not found");
    }
    Ok(())
}

/// Set process priority (normal, high, low, etc.)
pub fn set_priority(pid: u32, priority: &str) -> anyhow::Result<()> {
    use windows::Win32::System::Threading::SetPriorityClass;

    unsafe {
        let process = OpenProcess(PROCESS_QUERY_INFORMATION, false, pid)?;
        if process.is_invalid() {
            anyhow::bail!("Failed to open process {pid}");
        }

        let priority_class = match priority {
            "low" | "below_normal" => windows::Win32::System::Threading::BELOW_NORMAL_PRIORITY_CLASS,
            "high" | "above_normal" => windows::Win32::System::Threading::ABOVE_NORMAL_PRIORITY_CLASS,
            "realtime" => windows::Win32::System::Threading::REALTIME_PRIORITY_CLASS,
            "normal" | _ => windows::Win32::System::Threading::NORMAL_PRIORITY_CLASS,
        };

        let _ = SetPriorityClass(process, priority_class);
        CloseHandle(process)?;
    }
    Ok(())
}
