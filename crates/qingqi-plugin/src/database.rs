use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;

use crate::storage::AppPaths;

pub type SqlitePool = Pool<SqliteConnectionManager>;
pub type PooledConnection = r2d2::PooledConnection<SqliteConnectionManager>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DatabaseLocation {
    App { filename: String },
    FeatureState { feature: String, filename: String },
    Path(PathBuf),
}

impl DatabaseLocation {
    fn resolve(&self, paths: &AppPaths) -> PathBuf {
        match self {
            Self::App { filename } => paths.database(filename),
            Self::FeatureState { feature, filename } => paths.feature_state(feature, filename),
            Self::Path(path) => path.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatabaseSpec {
    pub key: String,
    pub location: DatabaseLocation,
}

impl DatabaseSpec {
    pub fn app(key: impl Into<String>, filename: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            location: DatabaseLocation::App {
                filename: filename.into(),
            },
        }
    }

    pub fn feature(
        feature: impl Into<String>,
        name: impl Into<String>,
        filename: impl Into<String>,
    ) -> Self {
        let feature = feature.into();
        let name = name.into();
        Self {
            key: feature_database_key(&feature, &name),
            location: DatabaseLocation::FeatureState {
                feature,
                filename: filename.into(),
            },
        }
    }

    pub fn path(key: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            key: key.into(),
            location: DatabaseLocation::Path(path.into()),
        }
    }
}

pub fn feature_database_key(feature: &str, name: &str) -> String {
    format!("{feature}/{name}")
}

#[derive(Clone, Debug)]
pub struct DatabaseService {
    paths: AppPaths,
    pools: Arc<Mutex<HashMap<PathBuf, SqlitePool>>>,
    registrations: Arc<Mutex<HashMap<String, PathBuf>>>,
}

impl DatabaseService {
    pub fn new(paths: AppPaths) -> Self {
        Self {
            paths,
            pools: Arc::new(Mutex::new(HashMap::new())),
            registrations: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

    pub fn register_database(&self, spec: DatabaseSpec) -> Result<PathBuf> {
        let path = spec.location.resolve(&self.paths);
        let mut registrations = self
            .registrations
            .lock()
            .map_err(|_| anyhow::anyhow!("database registration registry lock poisoned"))?;
        if let Some(existing) = registrations.get(&spec.key) {
            if *existing != path {
                anyhow::bail!(
                    "database key {} already registered for {}",
                    spec.key,
                    existing.display()
                );
            }
        } else {
            registrations.insert(spec.key, path.clone());
        }
        drop(registrations);
        self.pool_for_path(path.clone())?;
        Ok(path)
    }

    pub fn register_databases<I>(&self, specs: I) -> Result<()>
    where
        I: IntoIterator<Item = DatabaseSpec>,
    {
        for spec in specs {
            self.register_database(spec)?;
        }
        Ok(())
    }

    pub fn path_for_key(&self, key: &str) -> Result<PathBuf> {
        self.registrations
            .lock()
            .map_err(|_| anyhow::anyhow!("database registration registry lock poisoned"))?
            .get(key)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("database key not registered: {key}"))
    }

    pub fn pool(&self, key: &str) -> Result<SqlitePool> {
        let path = self.path_for_key(key)?;
        self.pool_for_path(path)
    }

    pub fn connection(&self, key: &str) -> Result<PooledConnection> {
        let pool = self.pool(key)?;
        pool.get().context("cannot get sqlite pooled connection")
    }

    pub fn pool_for_database(&self, name: &str) -> Result<SqlitePool> {
        self.pool_for_path(self.paths.database(name))
    }

    pub fn pool_for_feature_state(&self, feature: &str, name: &str) -> Result<SqlitePool> {
        self.pool_for_path(self.paths.feature_state(feature, name))
    }

    pub fn pool_for_path(&self, path: impl Into<PathBuf>) -> Result<SqlitePool> {
        let path = path.into();
        let canonical_key = normalized_key(&path);

        if let Some(pool) = self
            .pools
            .lock()
            .map_err(|_| anyhow::anyhow!("database pool registry lock poisoned"))?
            .get(&canonical_key)
            .cloned()
        {
            return Ok(pool);
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("cannot create database dir {}", parent.display()))?;
        }

        let manager = SqliteConnectionManager::file(&path);
        let pool = Pool::builder()
            .max_size(8)
            .connection_customizer(Box::new(ConnectionCustomizer))
            .build(manager)
            .with_context(|| format!("cannot build sqlite pool {}", path.display()))?;

        let mut pools = self
            .pools
            .lock()
            .map_err(|_| anyhow::anyhow!("database pool registry lock poisoned"))?;
        let pool = pools.entry(canonical_key).or_insert_with(|| pool.clone());
        Ok(pool.clone())
    }

    pub fn connection_for_path(&self, path: impl Into<PathBuf>) -> Result<PooledConnection> {
        let pool = self.pool_for_path(path)?;
        pool.get().context("cannot get sqlite pooled connection")
    }

    pub fn with_connection<T, F>(&self, path: impl Into<PathBuf>, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let conn = self.connection_for_path(path)?;
        f(&conn)
    }

    pub fn with_registered_connection<T, F>(&self, key: &str, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let conn = self.connection(key)?;
        f(&conn)
    }

    pub fn shutdown(&self) {
        if let Ok(mut pools) = self.pools.lock() {
            pools.clear();
        }
        if let Ok(mut registrations) = self.registrations.lock() {
            registrations.clear();
        }
    }
}

fn normalized_key(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[derive(Debug)]
struct ConnectionCustomizer;

impl r2d2::CustomizeConnection<Connection, rusqlite::Error> for ConnectionCustomizer {
    fn on_acquire(&self, conn: &mut Connection) -> std::result::Result<(), rusqlite::Error> {
        configure_connection(conn)
    }
}

pub fn configure_connection(conn: &Connection) -> std::result::Result<(), rusqlite::Error> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", 1)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_paths(label: &str) -> AppPaths {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!("qingqi-db-service-{label}-{nanos}"));
        fs::create_dir_all(&dir).expect("temp dir");
        AppPaths::for_test(dir)
    }

    #[test]
    fn returns_same_pool_for_same_path() {
        let service = DatabaseService::new(temp_paths("same-pool"));
        let first = service.pool_for_database("demo.db").expect("first pool");
        let second = service.pool_for_database("demo.db").expect("second pool");
        assert_eq!(first.state().connections, second.state().connections);
        let first_conn = first.get().expect("first conn");
        let second_conn = second.get().expect("second conn");
        first_conn
            .execute("CREATE TABLE IF NOT EXISTS test_pool (id INTEGER)", [])
            .expect("create table");
        second_conn
            .execute("INSERT INTO test_pool (id) VALUES (1)", [])
            .expect("insert");
    }

    #[test]
    fn configures_new_connections() {
        let service = DatabaseService::new(temp_paths("pragmas"));
        let pool = service.pool_for_database("pragmas.db").expect("pool");
        let conn = pool.get().expect("conn");
        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .expect("journal_mode");
        let foreign_keys: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .expect("foreign_keys");
        assert_eq!(journal_mode.to_lowercase(), "wal");
        assert_eq!(foreign_keys, 1);
    }

    #[test]
    fn registers_and_resolves_database_by_key() {
        let service = DatabaseService::new(temp_paths("registered"));
        service
            .register_database(DatabaseSpec::feature("demo-plugin", "main", "demo.db"))
            .expect("register database");

        let conn = service.connection("demo-plugin/main").expect("connection");
        conn.execute("CREATE TABLE IF NOT EXISTS t (id INTEGER)", [])
            .expect("create table");
    }
}
