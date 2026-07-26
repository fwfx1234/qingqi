use std::{
    collections::HashSet,
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
};

use anyhow::{Context as _, ensure};
use plist::{Dictionary, Value};
use qingqi_platform::display::{self, DisplayModeKey};
use sha2::{Digest as _, Sha256};

use crate::{
    model::{
        HidpiPreset, ManagedDisplay, OptimizerSnapshot, OptimizerStatus, is_eligible_qhd_display,
    },
    store::{InstallRecord, OptimizerStore},
};

const OVERRIDES_ROOT: &str = "/Library/Displays/Contents/Resources/Overrides";

pub struct DisplayOptimizerService {
    store: OptimizerStore,
    mutation_lock: Mutex<()>,
}

impl DisplayOptimizerService {
    pub fn new(root: PathBuf) -> Self {
        Self {
            store: OptimizerStore::new(root),
            mutation_lock: Mutex::new(()),
        }
    }

    pub fn snapshot(&self) -> anyhow::Result<OptimizerSnapshot> {
        let state = self.store.load()?;
        let os_supported = is_supported_macos_version();
        let online = display::online_displays()?;
        let mut displays = Vec::new();
        let mut online_keys = HashSet::new();

        for descriptor in online {
            let key = identity_key(descriptor.vendor_id, descriptor.product_id);
            let record = state.installations.iter().find(|record| {
                record.vendor_id == descriptor.vendor_id
                    && record.product_id == descriptor.product_id
            });
            if !is_eligible_qhd_display(&descriptor) && record.is_none() {
                continue;
            }
            online_keys.insert(key);
            let modes = display::display_modes(descriptor.id).unwrap_or_else(|error| {
                tracing::warn!(
                    display_id = descriptor.id,
                    error = %error,
                    "enumerate display modes failed"
                );
                Vec::new()
            });
            let status = if !os_supported {
                OptimizerStatus::Unsupported
            } else if let Some(record) = record {
                managed_status(record, &modes)
            } else if HidpiPreset::ALL
                .iter()
                .all(|preset| modes.iter().copied().any(|mode| preset.matches(mode)))
            {
                OptimizerStatus::Active
            } else {
                OptimizerStatus::NotInstalled
            };
            displays.push(ManagedDisplay {
                name: descriptor.name.clone(),
                vendor_id: descriptor.vendor_id,
                product_id: descriptor.product_id,
                serial_number: descriptor.serial_number,
                status,
                modes,
                is_managed: record.is_some(),
                descriptor: Some(descriptor),
            });
        }

        for record in state
            .installations
            .iter()
            .filter(|record| !online_keys.contains(&record.key()))
        {
            displays.push(ManagedDisplay {
                descriptor: None,
                name: record.display_name.clone(),
                vendor_id: record.vendor_id,
                product_id: record.product_id,
                serial_number: record.serial_number,
                status: OptimizerStatus::Disconnected,
                modes: Vec::new(),
                is_managed: true,
            });
        }

        displays.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(OptimizerSnapshot {
            displays,
            os_supported,
        })
    }

    pub fn install(&self, display_id: u32) -> anyhow::Result<()> {
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| anyhow::anyhow!("外接屏优化操作锁已损坏"))?;
        ensure!(is_supported_macos_version(), "需要 macOS 12.4 或更高版本");
        let descriptor = display::display_descriptor(display_id)?;
        ensure!(
            is_eligible_qhd_display(&descriptor),
            "只支持原生 2560×1440 的外接显示器"
        );

        let mut state = self.store.load()?;
        ensure!(
            !state.installations.iter().any(|record| {
                record.vendor_id == descriptor.vendor_id
                    && record.product_id == descriptor.product_id
            }),
            "该显示器型号已经由 Qingqi 管理"
        );

        let target = override_path(descriptor.vendor_id, descriptor.product_id);
        validate_target_path(&target, descriptor.vendor_id, descriptor.product_id)?;
        let original = match fs::read(&target) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error).context("读取现有 Display Override 失败"),
        };
        let merged = build_override(
            original.as_deref(),
            descriptor.vendor_id,
            descriptor.product_id,
        )?;
        let key = identity_key(descriptor.vendor_id, descriptor.product_id);
        let staged = self.store.write_staged(&key, &merged)?;
        let backup = match original.as_deref() {
            Some(bytes) => Some(self.store.write_backup(&key, bytes)?),
            None => None,
        };
        let record = InstallRecord {
            vendor_id: descriptor.vendor_id,
            product_id: descriptor.product_id,
            serial_number: descriptor.serial_number,
            display_name: descriptor.name,
            original_existed: original.is_some(),
            backup_file: backup
                .as_ref()
                .map_or_else(String::new, |path| path.to_string_lossy().into_owned()),
            original_sha256: original.as_deref().map(sha256_bytes),
            installed_sha256: sha256_bytes(&merged),
        };
        state.installations.push(record.clone());
        self.store.save(&state)?;

        if let Err(error) = privileged_install(&staged, &target, true) {
            state.installations.retain(|candidate| candidate != &record);
            let _ = self.store.save(&state);
            let _ = fs::remove_file(&staged);
            if let Some(backup) = backup {
                let _ = fs::remove_file(backup);
            }
            return Err(error);
        }
        let _ = fs::remove_file(staged);
        Ok(())
    }

    pub fn uninstall(&self, vendor_id: u32, product_id: u32) -> anyhow::Result<()> {
        let _guard = self
            .mutation_lock
            .lock()
            .map_err(|_| anyhow::anyhow!("外接屏优化操作锁已损坏"))?;
        let mut state = self.store.load()?;
        let index = state
            .installations
            .iter()
            .position(|record| record.vendor_id == vendor_id && record.product_id == product_id)
            .context("没有找到该显示器的 Qingqi 安装记录")?;
        let record = state.installations[index].clone();
        let target = override_path(vendor_id, product_id);
        validate_target_path(&target, vendor_id, product_id)?;
        let installed = fs::read(&target).context("Display Override 已被删除或无法读取")?;
        ensure!(
            sha256_bytes(&installed) == record.installed_sha256,
            "Display Override 已被其他工具修改，已停止自动恢复"
        );

        if record.original_existed {
            let backup = PathBuf::from(&record.backup_file);
            let backup_bytes = fs::read(&backup).context("原始 Display Override 备份不存在")?;
            ensure!(
                record.original_sha256.as_deref() == Some(sha256_bytes(&backup_bytes).as_str()),
                "原始 Display Override 备份校验失败"
            );
            privileged_install(&backup, &target, false)?;
        } else {
            privileged_remove(&target)?;
        }

        state.installations.remove(index);
        self.store.save(&state)?;
        if !record.backup_file.is_empty() {
            let _ = fs::remove_file(record.backup_file);
        }
        Ok(())
    }

    pub fn apply_mode(&self, display_id: u32, mode: DisplayModeKey) -> anyhow::Result<()> {
        ensure!(
            mode.pixel_width == mode.width * 2 && mode.pixel_height == mode.height * 2,
            "只允许应用真实 HiDPI 模式"
        );
        display::set_display_mode(display_id, mode)
    }

    pub fn restore_mode(&self, display_id: u32, mode: DisplayModeKey) -> anyhow::Result<()> {
        display::set_display_mode(display_id, mode)
    }
}

fn managed_status(record: &InstallRecord, modes: &[display::DisplayMode]) -> OptimizerStatus {
    let target = override_path(record.vendor_id, record.product_id);
    let Ok(bytes) = fs::read(target) else {
        return OptimizerStatus::Conflict;
    };
    if sha256_bytes(&bytes) != record.installed_sha256 {
        return OptimizerStatus::Conflict;
    }
    if HidpiPreset::ALL
        .iter()
        .all(|preset| modes.iter().copied().any(|mode| preset.matches(mode)))
    {
        OptimizerStatus::Active
    } else {
        OptimizerStatus::PendingRestart
    }
}

fn build_override(
    existing: Option<&[u8]>,
    vendor_id: u32,
    product_id: u32,
) -> anyhow::Result<Vec<u8>> {
    let mut document = match existing {
        Some(bytes) => Value::from_reader(Cursor::new(bytes))
            .context("现有 Display Override 不是有效 plist")?,
        None => Value::Dictionary(Dictionary::new()),
    };
    let dictionary = document
        .as_dictionary_mut()
        .context("现有 Display Override 顶层不是字典")?;
    dictionary.insert(
        "DisplayVendorID".into(),
        Value::Integer(u64::from(vendor_id).into()),
    );
    dictionary.insert(
        "DisplayProductID".into(),
        Value::Integer(u64::from(product_id).into()),
    );

    if !dictionary.contains_key("scale-resolutions") {
        dictionary.insert("scale-resolutions".into(), Value::Array(Vec::new()));
    }
    let resolutions = dictionary
        .get_mut("scale-resolutions")
        .context("无法读取 scale-resolutions")?
        .as_array_mut()
        .context("现有 scale-resolutions 不是数组")?;
    for preset in HidpiPreset::ALL {
        let value = Value::Data(preset.mode_data());
        if !resolutions.contains(&value) {
            resolutions.push(value);
        }
    }

    let mut output = Vec::new();
    document
        .to_writer_xml(&mut output)
        .context("编码 Display Override 失败")?;
    Ok(output)
}

fn identity_key(vendor_id: u32, product_id: u32) -> String {
    format!("{vendor_id:04x}-{product_id:04x}")
}

fn override_path(vendor_id: u32, product_id: u32) -> PathBuf {
    Path::new(OVERRIDES_ROOT)
        .join(format!("DisplayVendorID-{vendor_id:x}"))
        .join(format!("DisplayProductID-{product_id:x}"))
}

fn validate_target_path(path: &Path, vendor_id: u32, product_id: u32) -> anyhow::Result<()> {
    ensure!(
        path == override_path(vendor_id, product_id),
        "Display Override 目标路径校验失败"
    );
    ensure!(
        path.starts_with(OVERRIDES_ROOT),
        "Display Override 目标不在允许目录"
    );
    Ok(())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(target_os = "macos")]
fn privileged_install(source: &Path, target: &Path, enable_resolution: bool) -> anyhow::Result<()> {
    let target_dir = target.parent().context("Display Override 目标没有父目录")?;
    ensure!(source.is_file(), "待安装 Display Override 不存在");
    let script = r#"
on run argv
    set sourcePath to item 1 of argv
    set targetDir to item 2 of argv
    set targetPath to item 3 of argv
    set shouldEnable to item 4 of argv
    set temporaryPath to targetPath & ".qingqi.tmp"
    set shellCommand to "set -e; "
    if shouldEnable is "yes" then
        set shellCommand to shellCommand & "/usr/bin/defaults write /Library/Preferences/com.apple.windowserver DisplayResolutionEnabled -bool YES; "
    end if
    set shellCommand to shellCommand & "/bin/mkdir -p " & quoted form of targetDir & "; "
    set shellCommand to shellCommand & "/usr/sbin/chown root:wheel " & quoted form of targetDir & "; "
    set shellCommand to shellCommand & "/bin/chmod 0755 " & quoted form of targetDir & "; "
    set shellCommand to shellCommand & "/usr/bin/install -m 0644 -o root -g wheel " & quoted form of sourcePath & " " & quoted form of temporaryPath & "; "
    set shellCommand to shellCommand & "/bin/mv -f " & quoted form of temporaryPath & " " & quoted form of targetPath
    do shell script shellCommand with administrator privileges
end run
"#;
    run_osascript(
        script,
        &[
            source.to_string_lossy().as_ref(),
            target_dir.to_string_lossy().as_ref(),
            target.to_string_lossy().as_ref(),
            if enable_resolution { "yes" } else { "no" },
        ],
    )
}

#[cfg(not(target_os = "macos"))]
fn privileged_install(
    _source: &Path,
    _target: &Path,
    _enable_resolution: bool,
) -> anyhow::Result<()> {
    anyhow::bail!("Display Override 安装仅支持 macOS")
}

#[cfg(target_os = "macos")]
fn privileged_remove(target: &Path) -> anyhow::Result<()> {
    let target_dir = target.parent().context("Display Override 目标没有父目录")?;
    let script = r#"
on run argv
    set targetDir to item 1 of argv
    set targetPath to item 2 of argv
    set shellCommand to "set -e; /bin/rm -f " & quoted form of targetPath & "; /bin/rmdir " & quoted form of targetDir & " 2>/dev/null || true"
    do shell script shellCommand with administrator privileges
end run
"#;
    run_osascript(
        script,
        &[
            target_dir.to_string_lossy().as_ref(),
            target.to_string_lossy().as_ref(),
        ],
    )
}

#[cfg(not(target_os = "macos"))]
fn privileged_remove(_target: &Path) -> anyhow::Result<()> {
    anyhow::bail!("Display Override 移除仅支持 macOS")
}

#[cfg(target_os = "macos")]
fn run_osascript(script: &str, arguments: &[&str]) -> anyhow::Result<()> {
    let output = Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(script)
        .arg("--")
        .args(arguments)
        .output()
        .context("无法启动管理员授权")?;
    ensure!(
        output.status.success(),
        "管理员操作未完成: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(())
}

pub(crate) fn is_supported_version_string(version: &str) -> bool {
    let mut parts = version.trim().split('.');
    let major = parts.next().and_then(|part| part.parse::<u32>().ok());
    let minor = parts
        .next()
        .and_then(|part| part.parse::<u32>().ok())
        .unwrap_or(0);
    matches!(major, Some(major) if major > 12 || (major == 12 && minor >= 4))
}

#[cfg(target_os = "macos")]
fn is_supported_macos_version() -> bool {
    Command::new("/usr/bin/sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .is_some_and(|version| is_supported_version_string(&version))
}

#[cfg(not(target_os = "macos"))]
fn is_supported_macos_version() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_merge_preserves_fields_and_deduplicates_modes() {
        let first = HidpiPreset::Recommended.mode_data();
        let mut existing = Dictionary::new();
        existing.insert("CustomField".into(), Value::String("keep-me".into()));
        existing.insert(
            "scale-resolutions".into(),
            Value::Array(vec![Value::Data(first.clone())]),
        );
        let mut bytes = Vec::new();
        Value::Dictionary(existing)
            .to_writer_xml(&mut bytes)
            .expect("encode fixture");

        let merged = build_override(Some(&bytes), 0x5a63, 0x8432).expect("merge override");
        let value = Value::from_reader(Cursor::new(merged)).expect("parse merged override");
        let dictionary = value.as_dictionary().expect("dictionary");
        assert_eq!(
            dictionary.get("CustomField").and_then(Value::as_string),
            Some("keep-me")
        );
        let modes = dictionary
            .get("scale-resolutions")
            .and_then(Value::as_array)
            .expect("mode array");
        assert_eq!(modes.len(), 3);
        assert_eq!(
            modes
                .iter()
                .filter(|value| **value == Value::Data(first.clone()))
                .count(),
            1
        );
    }

    #[test]
    fn invalid_existing_override_is_rejected() {
        assert!(build_override(Some(b"not a plist"), 1, 2).is_err());
    }

    #[test]
    fn target_path_is_scoped_to_single_product() {
        let path = override_path(0x5a63, 0x8432);
        assert_eq!(
            path,
            PathBuf::from(
                "/Library/Displays/Contents/Resources/Overrides/DisplayVendorID-5a63/DisplayProductID-8432"
            )
        );
        validate_target_path(&path, 0x5a63, 0x8432).expect("valid path");
    }

    #[test]
    fn supports_macos_12_4_and_newer() {
        assert!(!is_supported_version_string("12.3.9"));
        assert!(is_supported_version_string("12.4"));
        assert!(is_supported_version_string("13.0"));
        assert!(is_supported_version_string("26.5.2"));
        assert!(!is_supported_version_string("invalid"));
    }

    #[test]
    fn changed_installed_hash_is_a_conflict() {
        let record = InstallRecord {
            vendor_id: 0,
            product_id: 0,
            serial_number: 0,
            display_name: "test".into(),
            original_existed: false,
            backup_file: String::new(),
            original_sha256: None,
            installed_sha256: "not-the-file-hash".into(),
        };
        assert_eq!(managed_status(&record, &[]), OptimizerStatus::Conflict);
    }
}
