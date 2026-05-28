use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use crate::{
    core::page::Page,
    core::storage::AppPaths,
    features::app_launcher::store::{
        AppIndexCache, AppIndexStore, AppLaunchUsage, query_terms, search_text,
    },
    platform::apps::{
        InstalledApp, clear_broken_icon_paths, open_application, populate_application_icons,
        scan_application_metadata, scan_application_paths,
    },
};
use time::{OffsetDateTime, format_description::FormatItem, macros::format_description};

static TIMESTAMP_FORMAT: &[FormatItem<'static>] =
    format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");

pub type AppEntry = InstalledApp;

#[derive(Clone, Debug)]
pub struct AppIndexSnapshot {
    pub apps: Vec<AppEntry>,
    pub scan_running: bool,
    pub icon_refresh_running: bool,
    pub last_scan: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug)]
struct AppIndexState {
    apps: Vec<AppEntry>,
    usage: HashMap<String, AppLaunchUsage>,
    scan_running: bool,
    icon_refresh_running: bool,
    last_scan: Option<String>,
    last_error: Option<String>,
    revision: u64,
    probe_running: bool,
    last_probe_at: u64,
}

pub struct AppIndexService {
    store: AppIndexStore,
    state: Mutex<AppIndexState>,
}

impl AppIndexService {
    pub const DEFAULT_PAGE_LIMIT: usize = 40;

    pub fn new(paths: AppPaths) -> Self {
        let store = AppIndexStore::new(paths.database("app_index.db"));
        let (mut cache, last_error) = match store.load() {
            Ok(cache) => (cache, None),
            Err(error) => (AppIndexCache::default(), Some(error.to_string())),
        };
        let usage = store.usage_map().unwrap_or_else(|error| {
            tracing::warn!(error = %error, "app launch usage cache load failed");
            HashMap::new()
        });

        clear_broken_icon_paths(&mut cache.apps);

        Self {
            store,
            state: Mutex::new(AppIndexState {
                apps: cache.apps,
                usage,
                scan_running: false,
                icon_refresh_running: false,
                last_scan: cache.last_scan,
                last_error,
                revision: 0,
                probe_running: false,
                last_probe_at: 0,
            }),
        }
    }

    pub fn snapshot(&self) -> AppIndexSnapshot {
        let state = self.state.lock().expect("app index lock poisoned");
        AppIndexSnapshot {
            apps: state.apps.clone(),
            scan_running: state.scan_running,
            icon_refresh_running: state.icon_refresh_running,
            last_scan: state.last_scan.clone(),
            last_error: state.last_error.clone(),
        }
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<AppEntry> {
        let page_limit = if limit == 0 {
            AppIndexService::DEFAULT_PAGE_LIMIT
        } else {
            limit
        };
        let mut apps = self.search_page(query, 0, page_limit).rows;

        if limit > 0 {
            apps.truncate(limit);
        }
        apps
    }

    pub fn search_page(&self, query: &str, offset: usize, limit: usize) -> Page<AppEntry> {
        let started = Instant::now();
        let apps = {
            let state = self.state.lock().expect("app index lock poisoned");
            (state.apps.clone(), state.usage.clone())
        };
        let page = search_apps_page(apps.0, &apps.1, query, offset, limit);
        log_slow_app_index_step(
            "app index memory search",
            started,
            &[("query", query), ("total", &page.total.to_string())],
        );
        page
    }

    pub fn revision(&self) -> u64 {
        self.state.lock().expect("app index lock poisoned").revision
    }

    pub fn open_app(&self, path: &str) -> Result<(), String> {
        open_application(path)
    }

    pub fn record_launch(&self, path: &str) -> anyhow::Result<()> {
        let result = self.store.record_launch(path);
        match self.store.usage_map() {
            Ok(usage) => {
                let mut state = self.state.lock().expect("app index lock poisoned");
                state.usage = usage;
                state.revision += 1;
            }
            Err(error) => {
                let mut state = self.state.lock().expect("app index lock poisoned");
                state.last_error = Some(format!("刷新应用启动记录失败: {error}"));
                state.revision += 1;
            }
        }
        result
    }

    pub fn request_scan(self: &Arc<Self>) -> bool {
        let started = Instant::now();
        {
            let mut state = self.state.lock().expect("app index lock poisoned");
            if state.scan_running {
                log_slow_app_index_step("app index request_scan skipped", started, &[]);
                return false;
            }
            state.scan_running = true;
            state.icon_refresh_running = false;
            state.last_error = None;
            state.revision += 1;
        }
        log_slow_app_index_step("app index request_scan", started, &[]);

        let service = Arc::clone(self);
        thread::spawn(move || service.refresh_index());
        true
    }

    pub fn request_probe_scan(self: &Arc<Self>) -> bool {
        let now = epoch_secs();
        {
            let mut state = self.state.lock().expect("app index lock poisoned");
            if state.scan_running || state.probe_running {
                return false;
            }
            // Coalesce frequent triggers while launcher is open.
            if now.saturating_sub(state.last_probe_at) < 2 {
                return false;
            }
            state.probe_running = true;
            state.last_probe_at = now;
        }
        let service = Arc::clone(self);
        thread::spawn(move || service.probe_and_maybe_scan());
        true
    }

    fn probe_and_maybe_scan(self: Arc<Self>) {
        let started = Instant::now();
        let current_paths = scan_application_paths();
        let cached_paths = {
            let state = self.state.lock().expect("app index lock poisoned");
            let mut paths = state
                .apps
                .iter()
                .map(|app| app.path.clone())
                .collect::<Vec<_>>();
            paths.sort();
            paths
        };
        let changed = current_paths != cached_paths;
        {
            let mut state = self.state.lock().expect("app index lock poisoned");
            state.probe_running = false;
        }
        log_slow_app_index_step(
            "app index probe scan",
            started,
            &[
                ("changed", if changed { "true" } else { "false" }),
                ("current", &current_paths.len().to_string()),
                ("cached", &cached_paths.len().to_string()),
            ],
        );
        if changed {
            let _ = self.request_scan();
        }
    }

    fn refresh_index(&self) {
        let started = Instant::now();
        tracing::debug!("app index scan started");
        let mut apps = scan_application_metadata();
        log_slow_app_index_step(
            "app index metadata scan",
            started,
            &[("apps", &apps.len().to_string())],
        );
        self.publish_metadata_pass(apps.clone());
        let icons_started = Instant::now();
        populate_application_icons(&mut apps);
        log_slow_app_index_step(
            "app index icon refresh",
            icons_started,
            &[("apps", &apps.len().to_string())],
        );
        clear_broken_icon_paths(&mut apps);
        let timestamp = now_label();
        let cache = AppIndexCache {
            apps: apps.clone(),
            last_scan: Some(timestamp.clone()),
        };

        let save_started = Instant::now();
        let cache_error = self
            .store
            .save(&cache)
            .err()
            .map(|error| format!("保存缓存失败: {error}"));
        log_slow_app_index_step(
            "app index full cache save",
            save_started,
            &[("apps", &apps.len().to_string())],
        );

        let mut state = self.state.lock().expect("app index lock poisoned");
        state.apps = apps;
        state.scan_running = false;
        state.icon_refresh_running = false;
        state.last_scan = Some(timestamp);
        state.last_error = cache_error;
        state.revision += 1;
        tracing::info!(
            duration_ms = started.elapsed().as_millis() as u64,
            "app index scan finished"
        );
    }

    fn publish_metadata_pass(&self, apps: Vec<AppEntry>) {
        let last_scan = {
            let mut state = self.state.lock().expect("app index lock poisoned");
            state.apps = apps.clone();
            state.icon_refresh_running = true;
            state.revision += 1;
            state.last_scan.clone()
        };

        let app_count = apps.len();
        let save_started = Instant::now();
        let save_result = self.store.save(&AppIndexCache { apps, last_scan });
        log_slow_app_index_step(
            "app index metadata cache save",
            save_started,
            &[("apps", &app_count.to_string())],
        );

        let mut state = self.state.lock().expect("app index lock poisoned");
        match save_result {
            Ok(()) => {
                state.revision += 1;
            }
            Err(error) => {
                state.last_error = Some(format!("保存应用元数据缓存失败: {error}"));
                state.revision += 1;
            }
        }
    }
}

fn epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn search_apps_page(
    apps: Vec<AppEntry>,
    usage: &HashMap<String, AppLaunchUsage>,
    query: &str,
    offset: usize,
    limit: usize,
) -> Page<AppEntry> {
    let terms = query_terms(query);
    let mut filtered = if terms.is_empty() {
        apps
    } else {
        apps.into_iter()
            .filter(|app| {
                let haystack = search_text(app);
                terms.iter().all(|term| haystack.contains(term))
            })
            .collect()
    };

    if terms.is_empty() {
        filtered.sort_by(|left, right| {
            let left_usage = app_usage(usage, &left.path);
            let right_usage = app_usage(usage, &right.path);
            right_usage
                .use_count
                .cmp(&left_usage.use_count)
                .then_with(|| right_usage.last_used_at.cmp(&left_usage.last_used_at))
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
                .then_with(|| left.name.cmp(&right.name))
        });
    } else {
        filtered.sort_by(|left, right| {
            left.name
                .to_lowercase()
                .cmp(&right.name.to_lowercase())
                .then_with(|| left.name.cmp(&right.name))
        });
    }

    let total = filtered.len();
    let rows = filtered
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect::<Vec<_>>();

    Page {
        rows,
        total,
        offset,
        limit,
    }
}

fn app_usage<'a>(usage: &'a HashMap<String, AppLaunchUsage>, path: &str) -> &'a AppLaunchUsage {
    static DEFAULT_USAGE: AppLaunchUsage = AppLaunchUsage {
        use_count: 0,
        last_used_at: 0,
    };

    usage.get(&format!("app:{path}")).unwrap_or(&DEFAULT_USAGE)
}

fn log_slow_app_index_step(step: &'static str, started: Instant, fields: &[(&str, &str)]) {
    let duration_ms = started.elapsed().as_millis() as u64;
    if duration_ms < 50 {
        tracing::debug!(step, duration_ms, ?fields, "app index step");
    } else {
        tracing::warn!(step, duration_ms, ?fields, "slow app index step");
    }
}

fn now_label() -> String {
    OffsetDateTime::now_local()
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .format(TIMESTAMP_FORMAT)
        .unwrap_or_else(|_| String::from("1970-01-01 00:00:00"))
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn temp_db(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        std::env::temp_dir()
            .join(format!("qingqi-app-service-{nanos}"))
            .join(name)
    }

    fn sample_apps() -> Vec<AppEntry> {
        vec![
            AppEntry {
                name: String::from("Arc"),
                path: String::from("/Applications/Arc.app"),
                bundle_id: Some(String::from("company.thebrowser.Browser")),
                icon_path: None,
                aliases: vec![String::from("Arc Browser")],
                icon_letter: String::from("A"),
            },
            AppEntry {
                name: String::from("Safari"),
                path: String::from("/Applications/Safari.app"),
                bundle_id: Some(String::from("com.apple.Safari")),
                icon_path: None,
                aliases: vec![String::from("Browser")],
                icon_letter: String::from("S"),
            },
            AppEntry {
                name: String::from("Visual Studio Code"),
                path: String::from("/Applications/Visual Studio Code.app"),
                bundle_id: Some(String::from("com.microsoft.VSCode")),
                icon_path: None,
                aliases: vec![String::from("VS Code")],
                icon_letter: String::from("V"),
            },
        ]
    }

    #[test]
    fn search_page_returns_total_and_slice() {
        let store = AppIndexStore::new(temp_db("index.db"));
        store
            .save(&AppIndexCache {
                apps: sample_apps(),
                last_scan: None,
            })
            .expect("cache should save");
        let service = AppIndexService {
            store,
            state: Mutex::new(AppIndexState {
                apps: sample_apps(),
                usage: HashMap::new(),
                scan_running: false,
                icon_refresh_running: false,
                last_scan: None,
                last_error: None,
                revision: 0,
            }),
        };

        let page = service.search_page("browser", 1, 1);
        assert_eq!(page.total, 2);
        assert_eq!(page.offset, 1);
        assert_eq!(page.limit, 1);
        assert_eq!(page.rows.len(), 1);
        assert_eq!(page.rows[0].name, "Safari");
    }

    #[test]
    fn publish_metadata_pass_marks_icon_refresh_running() {
        let service = AppIndexService {
            store: AppIndexStore::new(temp_db("metadata.db")),
            state: Mutex::new(AppIndexState {
                apps: Vec::new(),
                usage: HashMap::new(),
                scan_running: true,
                icon_refresh_running: false,
                last_scan: None,
                last_error: None,
                revision: 7,
            }),
        };

        service.publish_metadata_pass(vec![AppEntry {
            name: String::from("Safari"),
            path: String::from("/Applications/Safari.app"),
            bundle_id: Some(String::from("com.apple.Safari")),
            icon_path: None,
            aliases: vec![String::from("Safari")],
            icon_letter: String::from("S"),
        }]);

        let snapshot = service.snapshot();
        assert_eq!(snapshot.apps.len(), 1);
        assert!(snapshot.scan_running);
        assert!(snapshot.icon_refresh_running);
        assert!(snapshot.apps[0].icon_path.is_none());
        assert_eq!(service.revision(), 9);

        let page = service.search_page("safari", 0, 10);
        assert_eq!(page.total, 1);
        assert_eq!(page.rows[0].name, "Safari");
    }

    #[test]
    fn empty_query_orders_by_launch_usage() {
        let service = AppIndexService {
            store: AppIndexStore::new(temp_db("usage.db")),
            state: Mutex::new(AppIndexState {
                apps: sample_apps(),
                usage: HashMap::from([
                    (
                        String::from("app:/Applications/Safari.app"),
                        AppLaunchUsage {
                            use_count: 5,
                            last_used_at: 20,
                        },
                    ),
                    (
                        String::from("app:/Applications/Arc.app"),
                        AppLaunchUsage {
                            use_count: 5,
                            last_used_at: 10,
                        },
                    ),
                ]),
                scan_running: false,
                icon_refresh_running: false,
                last_scan: None,
                last_error: None,
                revision: 0,
            }),
        };

        let page = service.search_page("", 0, 10);
        assert_eq!(page.rows[0].name, "Safari");
        assert_eq!(page.rows[1].name, "Arc");
        assert_eq!(page.rows[2].name, "Visual Studio Code");
    }
}
