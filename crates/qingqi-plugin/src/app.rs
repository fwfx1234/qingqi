#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AppEntry {
    pub name: String,
    pub path: String,
    pub bundle_id: Option<String>,
    pub icon_path: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub icon_letter: String,
}

#[derive(Clone, Debug)]
pub struct AppIndexSnapshot {
    pub apps: Vec<AppEntry>,
    pub scan_running: bool,
    pub icon_refresh_running: bool,
    pub last_scan: Option<String>,
    pub last_error: Option<String>,
    pub scan_completed_once: bool,
}
