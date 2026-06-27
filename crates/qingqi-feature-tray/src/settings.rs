use std::time::Duration;

use anyhow::Result;
use qingqi_plugin::dict_store::PluginDictStore;

pub const SETTINGS_NAMESPACE: &str = "network-speed";

const KEY_VISIBLE: &str = "visible";
const KEY_DISPLAY_MODE: &str = "display_mode";
const KEY_TEXT_MODE: &str = "text_mode";
const KEY_UPDATE_INTERVAL_MS: &str = "update_interval_ms";
const KEY_POPUP_WIDTH: &str = "popup_width";
const KEY_POPUP_HEIGHT: &str = "popup_height";
const KEY_SHOW_TOTALS: &str = "show_totals";
const KEY_SHOW_INTERFACES: &str = "show_interfaces";
const KEY_MAX_INTERFACES: &str = "max_interfaces";

const DEFAULT_UPDATE_INTERVAL_MS: u64 = 1000;
const MIN_UPDATE_INTERVAL_MS: u64 = 500;
const MAX_UPDATE_INTERVAL_MS: u64 = 5000;
const DEFAULT_POPUP_WIDTH: u32 = 340;
const DEFAULT_POPUP_HEIGHT: u32 = 360;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkSpeedDisplayMode {
    TextOnly,
    IconOnly,
    IconAndText,
}

impl NetworkSpeedDisplayMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::TextOnly => "text_only",
            Self::IconOnly => "icon_only",
            Self::IconAndText => "icon_and_text",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "icon_only" => Self::IconOnly,
            "icon_and_text" => Self::IconAndText,
            _ => Self::TextOnly,
        }
    }
}

impl Default for NetworkSpeedDisplayMode {
    fn default() -> Self {
        Self::TextOnly
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkSpeedTextMode {
    Both,
    DownloadOnly,
    UploadOnly,
    Dominant,
}

impl NetworkSpeedTextMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Both => "both",
            Self::DownloadOnly => "download_only",
            Self::UploadOnly => "upload_only",
            Self::Dominant => "dominant",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "both" => Self::Both,
            "upload_only" => Self::UploadOnly,
            "dominant" => Self::Dominant,
            _ => Self::DownloadOnly,
        }
    }
}

impl Default for NetworkSpeedTextMode {
    fn default() -> Self {
        Self::DownloadOnly
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetworkSpeedSettings {
    pub network_speed_visible: bool,
    pub network_speed_display_mode: NetworkSpeedDisplayMode,
    pub network_speed_text_mode: NetworkSpeedTextMode,
    pub network_speed_update_interval_ms: u64,
    pub popup_width: u32,
    pub popup_height: u32,
    pub network_speed_show_totals: bool,
    pub network_speed_show_interfaces: bool,
    pub network_speed_max_interfaces: u8,
}

impl Default for NetworkSpeedSettings {
    fn default() -> Self {
        Self {
            network_speed_visible: true,
            network_speed_display_mode: NetworkSpeedDisplayMode::TextOnly,
            network_speed_text_mode: NetworkSpeedTextMode::DownloadOnly,
            network_speed_update_interval_ms: DEFAULT_UPDATE_INTERVAL_MS,
            popup_width: DEFAULT_POPUP_WIDTH,
            popup_height: DEFAULT_POPUP_HEIGHT,
            network_speed_show_totals: true,
            network_speed_show_interfaces: true,
            network_speed_max_interfaces: 5,
        }
    }
}

impl NetworkSpeedSettings {
    pub fn sanitized(mut self) -> Self {
        self.network_speed_update_interval_ms = self
            .network_speed_update_interval_ms
            .clamp(MIN_UPDATE_INTERVAL_MS, MAX_UPDATE_INTERVAL_MS);
        self.popup_width = self.popup_width.clamp(280, 520);
        self.popup_height = self.popup_height.clamp(240, 640);
        self.network_speed_max_interfaces = self.network_speed_max_interfaces.clamp(0, 10);
        self
    }

    pub fn effective_network_speed_show_icon(&self) -> bool {
        matches!(
            self.network_speed_display_mode,
            NetworkSpeedDisplayMode::IconOnly | NetworkSpeedDisplayMode::IconAndText
        )
    }

    pub fn effective_network_speed_show_text(&self) -> bool {
        !matches!(
            self.network_speed_display_mode,
            NetworkSpeedDisplayMode::IconOnly
        )
    }

    pub fn network_speed_update_interval(&self) -> Duration {
        Duration::from_millis(self.network_speed_update_interval_ms)
    }
}

#[derive(Clone)]
pub struct NetworkSpeedSettingsStore {
    dict: PluginDictStore,
}

impl NetworkSpeedSettingsStore {
    pub fn new(dict: PluginDictStore) -> Self {
        Self { dict }
    }

    pub fn settings(&self) -> NetworkSpeedSettings {
        self.load().unwrap_or_else(|error| {
            tracing::warn!(error = %error, "load network speed settings failed");
            NetworkSpeedSettings::default()
        })
    }

    pub fn load(&self) -> Result<NetworkSpeedSettings> {
        let defaults = NetworkSpeedSettings::default();
        Ok(NetworkSpeedSettings {
            network_speed_visible: self
                .dict
                .get_bool(SETTINGS_NAMESPACE, KEY_VISIBLE)?
                .unwrap_or(defaults.network_speed_visible),
            network_speed_display_mode: self
                .dict
                .get_string(SETTINGS_NAMESPACE, KEY_DISPLAY_MODE)?
                .map(|value| NetworkSpeedDisplayMode::parse(&value))
                .unwrap_or(defaults.network_speed_display_mode),
            network_speed_text_mode: self
                .dict
                .get_string(SETTINGS_NAMESPACE, KEY_TEXT_MODE)?
                .map(|value| NetworkSpeedTextMode::parse(&value))
                .unwrap_or(defaults.network_speed_text_mode),
            network_speed_update_interval_ms: self
                .dict
                .get_u64(SETTINGS_NAMESPACE, KEY_UPDATE_INTERVAL_MS)?
                .unwrap_or(defaults.network_speed_update_interval_ms),
            popup_width: self
                .dict
                .get_u64(SETTINGS_NAMESPACE, KEY_POPUP_WIDTH)?
                .and_then(|value| u32::try_from(value).ok())
                .unwrap_or(defaults.popup_width),
            popup_height: self
                .dict
                .get_u64(SETTINGS_NAMESPACE, KEY_POPUP_HEIGHT)?
                .and_then(|value| u32::try_from(value).ok())
                .unwrap_or(defaults.popup_height),
            network_speed_show_totals: self
                .dict
                .get_bool(SETTINGS_NAMESPACE, KEY_SHOW_TOTALS)?
                .unwrap_or(defaults.network_speed_show_totals),
            network_speed_show_interfaces: self
                .dict
                .get_bool(SETTINGS_NAMESPACE, KEY_SHOW_INTERFACES)?
                .unwrap_or(defaults.network_speed_show_interfaces),
            network_speed_max_interfaces: self
                .dict
                .get_u64(SETTINGS_NAMESPACE, KEY_MAX_INTERFACES)?
                .and_then(|value| u8::try_from(value).ok())
                .unwrap_or(defaults.network_speed_max_interfaces),
        }
        .sanitized())
    }

    pub fn save(&self, settings: &NetworkSpeedSettings) -> Result<NetworkSpeedSettings> {
        let settings = settings.clone().sanitized();
        self.dict.set_bool(
            SETTINGS_NAMESPACE,
            KEY_VISIBLE,
            settings.network_speed_visible,
        )?;
        self.dict.set_string(
            SETTINGS_NAMESPACE,
            KEY_DISPLAY_MODE,
            settings.network_speed_display_mode.as_str(),
        )?;
        self.dict.set_string(
            SETTINGS_NAMESPACE,
            KEY_TEXT_MODE,
            settings.network_speed_text_mode.as_str(),
        )?;
        self.dict.set_u64(
            SETTINGS_NAMESPACE,
            KEY_UPDATE_INTERVAL_MS,
            settings.network_speed_update_interval_ms,
        )?;
        self.dict.set_u64(
            SETTINGS_NAMESPACE,
            KEY_POPUP_WIDTH,
            settings.popup_width as u64,
        )?;
        self.dict.set_u64(
            SETTINGS_NAMESPACE,
            KEY_POPUP_HEIGHT,
            settings.popup_height as u64,
        )?;
        self.dict.set_bool(
            SETTINGS_NAMESPACE,
            KEY_SHOW_TOTALS,
            settings.network_speed_show_totals,
        )?;
        self.dict.set_bool(
            SETTINGS_NAMESPACE,
            KEY_SHOW_INTERFACES,
            settings.network_speed_show_interfaces,
        )?;
        self.dict.set_u64(
            SETTINGS_NAMESPACE,
            KEY_MAX_INTERFACES,
            settings.network_speed_max_interfaces as u64,
        )?;
        Ok(settings)
    }

    pub fn update(
        &self,
        apply: impl FnOnce(&mut NetworkSpeedSettings),
    ) -> Result<NetworkSpeedSettings> {
        let mut settings = self.load()?;
        apply(&mut settings);
        self.save(&settings)
    }

    pub fn set_network_speed_visible(&self, visible: bool) -> Result<NetworkSpeedSettings> {
        self.update(|settings| settings.network_speed_visible = visible)
    }

    pub fn set_network_speed_display_mode(
        &self,
        mode: NetworkSpeedDisplayMode,
    ) -> Result<NetworkSpeedSettings> {
        self.update(|settings| settings.network_speed_display_mode = mode)
    }

    pub fn set_network_speed_text_mode(
        &self,
        mode: NetworkSpeedTextMode,
    ) -> Result<NetworkSpeedSettings> {
        self.update(|settings| settings.network_speed_text_mode = mode)
    }

    pub fn set_network_speed_update_interval_ms(
        &self,
        interval_ms: u64,
    ) -> Result<NetworkSpeedSettings> {
        self.update(|settings| settings.network_speed_update_interval_ms = interval_ms)
    }

    pub fn set_popup_size(&self, width: u32, height: u32) -> Result<NetworkSpeedSettings> {
        self.update(|settings| {
            settings.popup_width = width;
            settings.popup_height = height;
        })
    }

    pub fn set_network_speed_show_totals(&self, show: bool) -> Result<NetworkSpeedSettings> {
        self.update(|settings| settings.network_speed_show_totals = show)
    }

    pub fn set_network_speed_show_interfaces(&self, show: bool) -> Result<NetworkSpeedSettings> {
        self.update(|settings| settings.network_speed_show_interfaces = show)
    }

    pub fn set_network_speed_max_interfaces(
        &self,
        max_interfaces: u8,
    ) -> Result<NetworkSpeedSettings> {
        self.update(|settings| settings.network_speed_max_interfaces = max_interfaces)
    }
}
