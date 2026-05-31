use std::sync::atomic::{AtomicBool, Ordering};

/// Global theme state - synchronized across the app
static DARK_MODE: AtomicBool = AtomicBool::new(false);

/// Returns the current theme mode (true = dark, false = light)
pub fn is_dark() -> bool {
    DARK_MODE.load(Ordering::Relaxed)
}

/// Set the theme mode explicitly
pub fn set_dark(dark: bool) {
    DARK_MODE.store(dark, Ordering::Relaxed);
}
