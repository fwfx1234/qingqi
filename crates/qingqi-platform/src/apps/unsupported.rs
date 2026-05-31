use super::InstalledApp;

pub(super) fn scan_application_metadata() -> Vec<InstalledApp> {
    Vec::new()
}

pub(super) fn scan_application_paths() -> Vec<String> {
    Vec::new()
}

pub(super) fn populate_application_icons(_apps: &mut [InstalledApp]) {}

pub(super) fn open_application(_path: &str) -> Result<(), String> {
    Err(String::from(
        "application launching is not supported on this platform",
    ))
}
