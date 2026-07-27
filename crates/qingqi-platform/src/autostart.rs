use std::path::PathBuf;

use anyhow::{Context, Result};

const AUTOSTART_KEY: &str = "Qingqi";

/// Get the current executable path.
fn exe_path() -> Result<PathBuf> {
    std::env::current_exe().context("cannot locate current executable")
}

// ── Windows ──

#[cfg(target_os = "windows")]
mod win {
    use super::*;
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStrExt;
    use ::windows::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
        RegSetValueExW, REG_SAM_FLAGS,
    };
    use ::windows::core::PCWSTR;

    const RUN_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

    fn to_wstr(s: &str) -> Vec<u16> {
        OsString::from(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    fn open_key(access: REG_SAM_FLAGS) -> Result<Option<HKEY>> {
        let subkey = to_wstr(RUN_PATH);
        let mut hkey = HKEY(std::ptr::null_mut());
        let result = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey.as_ptr()),
                Some(0),
                access,
                &mut hkey,
            )
        };
        if result.is_ok() {
            Ok(Some(hkey))
        } else {
            Ok(None)
        }
    }

    fn read_value() -> Result<Option<String>> {
        let hkey = match open_key(KEY_READ)? {
            Some(k) => k,
            None => return Ok(None),
        };

        let value_name = to_wstr(AUTOSTART_KEY);
        let mut buf = vec![0u16; 1024];
        let mut len = (buf.len() * 2) as u32;
        let mut kind = REG_SZ;

        let result = unsafe {
            RegQueryValueExW(
                hkey,
                PCWSTR(value_name.as_ptr()),
                None,
                Some(&mut kind),
                Some(buf.as_mut_ptr() as *mut u8),
                Some(&mut len),
            )
        };

        let _ = unsafe { RegCloseKey(hkey) };

        if result.is_ok() {
            let chars = (len as usize).saturating_sub(2) / 2;
            let s = String::from_utf16_lossy(&buf[..chars]);
            Ok(Some(s))
        } else {
            Ok(None)
        }
    }

    pub fn enable() -> Result<()> {
        let exe = exe_path()?;
        let quoted = format!("\"{}\"", exe.display());
        let subkey = to_wstr(RUN_PATH);
        let value_name = to_wstr(AUTOSTART_KEY);
        let data = to_wstr(&quoted);
        let mut hkey = HKEY(std::ptr::null_mut());

        unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey.as_ptr()),
                Some(0),
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                None,
                &mut hkey,
                None,
            )
            .ok()
            .context("failed to open registry key")?;

            let byte_len = data.len() * 2;
            RegSetValueExW(
                hkey,
                PCWSTR(value_name.as_ptr()),
                Some(0),
                REG_SZ,
                Some(std::slice::from_raw_parts(
                    data.as_ptr() as *const u8,
                    byte_len,
                )),
            )
            .ok()
            .context("failed to set registry value")?;

            let _ = RegCloseKey(hkey);
        }

        Ok(())
    }

    pub fn disable() -> Result<()> {
        let hkey = match open_key(KEY_SET_VALUE)? {
            Some(k) => k,
            None => return Ok(()),
        };

        let value_name = to_wstr(AUTOSTART_KEY);

        let result = unsafe { RegDeleteValueW(hkey, PCWSTR(value_name.as_ptr())) };

        let _ = unsafe { RegCloseKey(hkey) };

        result.ok().context("failed to delete registry value")?;
        Ok(())
    }

    pub fn is_enabled() -> Result<bool> {
        match read_value()? {
            Some(current) => {
                let exe = exe_path()?;
                let expected = format!("\"{}\"", exe.display());
                Ok(current == expected)
            }
            None => Ok(false),
        }
    }
}

// ── macOS ──

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn plist_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/"))
            .join("Library/LaunchAgents/com.qingqi.app.plist")
    }

    fn plist_content(exe_path: &Path) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.qingqi.app</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>ProcessType</key>
    <string>Interactive</string>
</dict>
</plist>"#,
            exe_path.display()
        )
    }

    pub fn enable() -> Result<()> {
        let exe = exe_path()?;
        let path = plist_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("cannot create directory {}", parent.display()))?;
        }
        fs::write(&path, plist_content(&exe))
            .with_context(|| format!("cannot write plist {}", path.display()))?;
        Ok(())
    }

    pub fn disable() -> Result<()> {
        let path = plist_path();
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("cannot remove plist {}", path.display()))?;
        }
        Ok(())
    }

    pub fn is_enabled() -> Result<bool> {
        let path = plist_path();
        if !path.exists() {
            return Ok(false);
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("cannot read plist {}", path.display()))?;
        let exe = exe_path()?;
        Ok(content.contains(&exe.display().to_string()))
    }
}

// ── Linux ──

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn desktop_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("/"))
                    .join(".config")
            })
            .join("autostart/qingqi.desktop")
    }

    fn desktop_content(exe_path: &Path) -> String {
        format!(
            r#"[Desktop Entry]
Type=Application
Name=Qingqi
Exec={}
Comment=Lightweight desktop toolbox
Terminal=false
X-GNOME-Autostart-enabled=true
"#,
            exe_path.display()
        )
    }

    pub fn enable() -> Result<()> {
        let exe = exe_path()?;
        let path = desktop_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("cannot create directory {}", parent.display()))?;
        }
        fs::write(&path, desktop_content(&exe))
            .with_context(|| format!("cannot write desktop file {}", path.display()))?;
        Ok(())
    }

    pub fn disable() -> Result<()> {
        let path = desktop_path();
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("cannot remove desktop file {}", path.display()))?;
        }
        Ok(())
    }

    pub fn is_enabled() -> Result<bool> {
        let path = desktop_path();
        if !path.exists() {
            return Ok(false);
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("cannot read desktop file {}", path.display()))?;
        let exe = exe_path()?;
        Ok(content.contains(&exe.display().to_string()))
    }
}

// ── Public API ──

pub fn enable_auto_start() -> Result<()> {
    #[cfg(target_os = "windows")]
    return win::enable();
    #[cfg(target_os = "macos")]
    return macos::enable();
    #[cfg(target_os = "linux")]
    return linux::enable();
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    Err(anyhow::anyhow!("auto-start not supported on this platform"))
}

pub fn disable_auto_start() -> Result<()> {
    #[cfg(target_os = "windows")]
    return win::disable();
    #[cfg(target_os = "macos")]
    return macos::disable();
    #[cfg(target_os = "linux")]
    return linux::disable();
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    Err(anyhow::anyhow!("auto-start not supported on this platform"))
}

pub fn is_auto_start_enabled() -> Result<bool> {
    #[cfg(target_os = "windows")]
    return win::is_enabled();
    #[cfg(target_os = "macos")]
    return macos::is_enabled();
    #[cfg(target_os = "linux")]
    return linux::is_enabled();
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    Ok(false)
}
