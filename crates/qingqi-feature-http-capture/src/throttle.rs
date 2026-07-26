use std::sync::Mutex;

/// Network speed preset
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThrottlePreset {
    Off,
    ThreeG, // ~400 Kbps
    FourG, // ~4 Mbps
    Custom, // user-defined
}

impl ThrottlePreset {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Off => "关闭",
            Self::ThreeG => "3G (400 Kbps)",
            Self::FourG => "4G (4 Mbps)",
            Self::Custom => "自定义",
        }
    }

    pub fn kbps(&self) -> u32 {
        match self {
            Self::Off => 0,
            Self::ThreeG => 400,
            Self::FourG => 4000,
            Self::Custom => 0, // uses custom_kbps
        }
    }
}

/// Throttle configuration
#[derive(Clone, Debug)]
pub struct ThrottleConfig {
    pub preset: ThrottlePreset,
    pub custom_kbps: u32,
    pub enabled: bool,
}

impl Default for ThrottleConfig {
    fn default() -> Self {
        Self {
            preset: ThrottlePreset::Off,
            custom_kbps: 1000,
            enabled: false,
        }
    }
}

/// Manages throttling state
pub struct ThrottleManager {
    config: Mutex<ThrottleConfig>,
}

impl ThrottleManager {
    pub fn new() -> Self {
        Self {
            config: Mutex::new(ThrottleConfig::default()),
        }
    }

    pub fn set_preset(&self, preset: ThrottlePreset) {
        let mut cfg = self.config.lock().unwrap();
        cfg.preset = preset;
        cfg.enabled = preset != ThrottlePreset::Off;
    }

    pub fn set_custom_kbps(&self, kbps: u32) {
        let mut cfg = self.config.lock().unwrap();
        cfg.custom_kbps = kbps;
    }

    pub fn config(&self) -> ThrottleConfig {
        self.config.lock().unwrap().clone()
    }

    /// Calculate delay for a given response size in bytes
    /// Returns the artificial delay needed to simulate the configured speed
    pub fn calculate_delay(&self, bytes: usize) -> Option<std::time::Duration> {
        let cfg = self.config.lock().unwrap();
        if !cfg.enabled {
            return None;
        }

        let kbps = if cfg.preset == ThrottlePreset::Custom {
            cfg.custom_kbps
        } else {
            cfg.preset.kbps()
        };

        if kbps == 0 {
            return None;
        }

        // Calculate time in ms: bytes * 8 / kbps = ms
        let delay_ms = (bytes as f64 * 8.0 / kbps as f64) as u64;
        Some(std::time::Duration::from_millis(delay_ms))
    }
}
