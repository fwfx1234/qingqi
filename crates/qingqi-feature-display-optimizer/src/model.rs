use qingqi_platform::display::{DisplayDescriptor, DisplayMode, DisplayModeKey};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HidpiPreset {
    #[default]
    Recommended,
    Comfortable,
    LargeText,
}

impl HidpiPreset {
    pub const ALL: [Self; 3] = [Self::Recommended, Self::Comfortable, Self::LargeText];

    pub const fn logical_size(self) -> (u32, u32) {
        match self {
            Self::Recommended => (2048, 1152),
            Self::Comfortable => (1920, 1080),
            Self::LargeText => (1680, 945),
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Recommended => "推荐",
            Self::Comfortable => "舒适",
            Self::LargeText => "大字号",
        }
    }

    pub fn mode_data(self) -> Vec<u8> {
        let (width, height) = self.logical_size();
        let mut data = Vec::with_capacity(16);
        data.extend_from_slice(&(width * 2).to_be_bytes());
        data.extend_from_slice(&(height * 2).to_be_bytes());
        data.extend_from_slice(&1_u32.to_be_bytes());
        data.extend_from_slice(&0x0020_0000_u32.to_be_bytes());
        data
    }

    pub fn matches(self, mode: DisplayMode) -> bool {
        let (width, height) = self.logical_size();
        mode.is_hidpi()
            && mode.key.width == width
            && mode.key.height == height
            && mode.key.pixel_width == width * 2
            && mode.key.pixel_height == height * 2
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptimizerStatus {
    NotInstalled,
    PendingRestart,
    Active,
    Conflict,
    Unsupported,
    Disconnected,
}

impl OptimizerStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::NotInstalled => "未安装",
            Self::PendingRestart => "等待重启",
            Self::Active => "已启用",
            Self::Conflict => "配置冲突",
            Self::Unsupported => "不支持",
            Self::Disconnected => "显示器已断开",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ManagedDisplay {
    pub descriptor: Option<DisplayDescriptor>,
    pub name: String,
    pub vendor_id: u32,
    pub product_id: u32,
    pub serial_number: u32,
    pub status: OptimizerStatus,
    pub modes: Vec<DisplayMode>,
    pub is_managed: bool,
}

impl ManagedDisplay {
    pub fn identity_key(&self) -> String {
        match self.descriptor.as_ref() {
            Some(display) => format!(
                "{:04x}-{:04x}-display-{}",
                self.vendor_id, self.product_id, display.id
            ),
            None => format!("{:04x}-{:04x}-offline", self.vendor_id, self.product_id),
        }
    }

    pub fn display_id(&self) -> Option<u32> {
        self.descriptor.as_ref().map(|display| display.id)
    }

    pub fn current_mode(&self) -> Option<DisplayMode> {
        self.descriptor
            .as_ref()
            .and_then(|display| display.current_mode)
    }

    pub fn mode_for_preset(&self, preset: HidpiPreset) -> Option<DisplayModeKey> {
        let current_refresh = self
            .current_mode()
            .map_or(0, |mode| mode.key.refresh_millihz);
        self.modes
            .iter()
            .copied()
            .filter(|mode| preset.matches(*mode))
            .min_by_key(|mode| mode.key.refresh_millihz.abs_diff(current_refresh))
            .map(|mode| mode.key)
    }
}

#[derive(Clone, Debug, Default)]
pub struct OptimizerSnapshot {
    pub displays: Vec<ManagedDisplay>,
    pub os_supported: bool,
}

pub fn is_eligible_qhd_display(display: &DisplayDescriptor) -> bool {
    !display.is_builtin && display.native_width == 2560 && display.native_height == 1440
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_framebuffers_are_double_sized() {
        let expected = [
            (HidpiPreset::Recommended, 4096_u32, 2304_u32),
            (HidpiPreset::Comfortable, 3840, 2160),
            (HidpiPreset::LargeText, 3360, 1890),
        ];
        for (preset, width, height) in expected {
            let data = preset.mode_data();
            assert_eq!(data.len(), 16);
            assert_eq!(
                u32::from_be_bytes(data[0..4].try_into().expect("framebuffer width bytes")),
                width
            );
            assert_eq!(
                u32::from_be_bytes(data[4..8].try_into().expect("framebuffer height bytes")),
                height
            );
            assert_eq!(
                u32::from_be_bytes(data[8..12].try_into().expect("HiDPI marker bytes")),
                1
            );
            assert_eq!(
                u32::from_be_bytes(data[12..16].try_into().expect("mode flag bytes")),
                0x0020_0000
            );
        }
    }

    #[test]
    fn eligibility_excludes_builtin_and_non_qhd_displays() {
        let qhd = DisplayDescriptor {
            id: 3,
            name: "QHD".into(),
            vendor_id: 1,
            product_id: 2,
            serial_number: 3,
            is_builtin: false,
            native_width: 2560,
            native_height: 1440,
            current_mode: None,
        };
        assert!(is_eligible_qhd_display(&qhd));
        assert!(!is_eligible_qhd_display(&DisplayDescriptor {
            is_builtin: true,
            ..qhd.clone()
        }));
        assert!(!is_eligible_qhd_display(&DisplayDescriptor {
            native_width: 3840,
            native_height: 2160,
            ..qhd
        }));
    }

    #[test]
    fn identical_models_have_distinct_online_selection_keys() {
        let display = |id| ManagedDisplay {
            descriptor: Some(DisplayDescriptor {
                id,
                name: "QHD".into(),
                vendor_id: 1,
                product_id: 2,
                serial_number: 0,
                is_builtin: false,
                native_width: 2560,
                native_height: 1440,
                current_mode: None,
            }),
            name: "QHD".into(),
            vendor_id: 1,
            product_id: 2,
            serial_number: 0,
            status: OptimizerStatus::NotInstalled,
            modes: Vec::new(),
            is_managed: false,
        };

        assert_ne!(display(3).identity_key(), display(4).identity_key());
    }

    #[test]
    fn preset_selection_requires_true_hidpi_and_keeps_refresh_rate() {
        let mode = |pixel_width, pixel_height, refresh_millihz| DisplayMode {
            key: DisplayModeKey {
                width: 1920,
                height: 1080,
                pixel_width,
                pixel_height,
                refresh_millihz,
            },
            mode_id: refresh_millihz as i32,
            io_flags: 0,
        };
        let display = ManagedDisplay {
            descriptor: Some(DisplayDescriptor {
                id: 3,
                name: "QHD".into(),
                vendor_id: 1,
                product_id: 2,
                serial_number: 0,
                is_builtin: false,
                native_width: 2560,
                native_height: 1440,
                current_mode: Some(mode(2560, 1440, 59_940)),
            }),
            name: "QHD".into(),
            vendor_id: 1,
            product_id: 2,
            serial_number: 0,
            status: OptimizerStatus::Active,
            modes: vec![
                mode(1920, 1080, 59_940),
                mode(3840, 2160, 60_000),
                mode(3840, 2160, 59_940),
            ],
            is_managed: true,
        };

        let selected = display
            .mode_for_preset(HidpiPreset::Comfortable)
            .expect("matching HiDPI mode");
        assert_eq!(selected.pixel_width, 3840);
        assert_eq!(selected.pixel_height, 2160);
        assert_eq!(selected.refresh_millihz, 59_940);
    }
}
