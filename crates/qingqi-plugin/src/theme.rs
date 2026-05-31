#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum ThemeMode {
    #[serde(rename = "light", alias = "Light")]
    Light,
    #[serde(rename = "dark", alias = "Dark")]
    Dark,
    #[serde(rename = "system", alias = "System", alias = "auto", alias = "Auto")]
    #[default]
    System,
}

impl ThemeMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Light => "浅色模式",
            Self::Dark => "深色模式",
            Self::System => "跟随系统",
        }
    }

    pub fn persisted_value(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::System => "system",
        }
    }
}
