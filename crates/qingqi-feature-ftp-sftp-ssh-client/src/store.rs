use std::sync::Arc;

use anyhow::{Context, Result};
use rusqlite::{OptionalExtension, params};

use crate::model::{
    AuthConfig, ConnectionLimits, Profile, ProfileDraft, ProfilePaths, RemoteProtocol,
    SecurityPolicy, SshHostKeyPolicy, TlsVerifyPolicy,
};
use qingqi_plugin::database::{DatabaseService, PooledConnection, SqlitePool};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS remote_profiles_v2 (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL DEFAULT '',
    protocol TEXT NOT NULL DEFAULT 'ssh',
    host TEXT NOT NULL DEFAULT '',
    port INTEGER NOT NULL DEFAULT 22,
    username TEXT NOT NULL DEFAULT '',
    auth_method TEXT NOT NULL DEFAULT 'password',
    password TEXT NOT NULL DEFAULT '',
    private_key_path TEXT NOT NULL DEFAULT '',
    private_key_passphrase TEXT NOT NULL DEFAULT '',
    remote_root TEXT NOT NULL DEFAULT '/',
    local_root TEXT NOT NULL DEFAULT '~/Downloads',
    ssh_host_key_policy TEXT NOT NULL DEFAULT 'tofu',
    pinned_host_key TEXT NOT NULL DEFAULT '',
    tls_verify_policy TEXT NOT NULL DEFAULT 'system',
    pinned_tls_sha256 TEXT NOT NULL DEFAULT '',
    connect_timeout_secs INTEGER NOT NULL DEFAULT 15,
    transfer_concurrency INTEGER NOT NULL DEFAULT 3,
    passive_mode INTEGER NOT NULL DEFAULT 1,
    notes TEXT NOT NULL DEFAULT '',
    imported_from_legacy INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT '',
    updated_at TEXT NOT NULL DEFAULT '',
    last_used_at TEXT NOT NULL DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_remote_profiles_v2_order
    ON remote_profiles_v2(last_used_at DESC, id ASC);
";

const LIST_PROFILES: &str = "
SELECT id, name, protocol, host, port, username, auth_method, password,
       private_key_path, private_key_passphrase, remote_root, local_root,
       ssh_host_key_policy, pinned_host_key, tls_verify_policy, pinned_tls_sha256,
       connect_timeout_secs, transfer_concurrency, passive_mode, notes,
       created_at, updated_at, last_used_at
FROM remote_profiles_v2
ORDER BY last_used_at DESC, id ASC
";

const GET_PROFILE: &str = "
SELECT id, name, protocol, host, port, username, auth_method, password,
       private_key_path, private_key_passphrase, remote_root, local_root,
       ssh_host_key_policy, pinned_host_key, tls_verify_policy, pinned_tls_sha256,
       connect_timeout_secs, transfer_concurrency, passive_mode, notes,
       created_at, updated_at, last_used_at
FROM remote_profiles_v2
WHERE id = ?1
";

const INSERT_PROFILE: &str = "
INSERT INTO remote_profiles_v2
    (name, protocol, host, port, username, auth_method, password, private_key_path,
     private_key_passphrase, remote_root, local_root, ssh_host_key_policy, pinned_host_key,
     tls_verify_policy, pinned_tls_sha256, connect_timeout_secs, transfer_concurrency,
     passive_mode, notes, created_at, updated_at, last_used_at)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
        ?16, ?17, ?18, ?19,
        strftime('%Y-%m-%d %H:%M:%S', 'now', 'localtime'),
        strftime('%Y-%m-%d %H:%M:%S', 'now', 'localtime'),
        '')
";

const UPDATE_PROFILE: &str = "
UPDATE remote_profiles_v2
SET name = ?1,
    protocol = ?2,
    host = ?3,
    port = ?4,
    username = ?5,
    auth_method = ?6,
    password = ?7,
    private_key_path = ?8,
    private_key_passphrase = ?9,
    remote_root = ?10,
    local_root = ?11,
    ssh_host_key_policy = ?12,
    pinned_host_key = ?13,
    tls_verify_policy = ?14,
    pinned_tls_sha256 = ?15,
    connect_timeout_secs = ?16,
    transfer_concurrency = ?17,
    passive_mode = ?18,
    notes = ?19,
    updated_at = strftime('%Y-%m-%d %H:%M:%S', 'now', 'localtime')
WHERE id = ?20
";

const DELETE_PROFILE: &str = "DELETE FROM remote_profiles_v2 WHERE id = ?1";

const UPDATE_LAST_USED: &str = "
UPDATE remote_profiles_v2
SET last_used_at = strftime('%Y-%m-%d %H:%M:%S', 'now', 'localtime'),
    updated_at = strftime('%Y-%m-%d %H:%M:%S', 'now', 'localtime')
WHERE id = ?1
";

const COUNT_V2: &str = "SELECT COUNT(*) FROM remote_profiles_v2";
const COUNT_LEGACY: &str =
    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='remote_file_profiles'";

const IMPORT_LEGACY: &str = "
INSERT INTO remote_profiles_v2
    (name, protocol, host, port, username, auth_method, password, private_key_path,
     private_key_passphrase, remote_root, local_root, ssh_host_key_policy, pinned_host_key,
     tls_verify_policy, pinned_tls_sha256, connect_timeout_secs, transfer_concurrency,
     passive_mode, notes, imported_from_legacy, created_at, updated_at, last_used_at)
SELECT
    COALESCE(NULLIF(name, ''), '导入连接'),
    CASE protocol
        WHEN 'ssh' THEN 'ssh'
        WHEN 'sftp' THEN 'sftp'
        WHEN 'ftp' THEN 'ftp'
        ELSE 'ssh'
    END,
    host,
    port,
    username,
    auth_method,
    password,
    private_key_path,
    private_key_passphrase,
    COALESCE(NULLIF(remote_dir, ''), '/'),
    COALESCE(NULLIF(local_dir, ''), '~/Downloads'),
    CASE
        WHEN protocol IN ('ssh', 'sftp') THEN 'tofu'
        ELSE 'system'
    END,
    '',
    CASE
        WHEN protocol = 'ftp' THEN 'system'
        ELSE 'system'
    END,
    '',
    COALESCE(NULLIF(connect_timeout_secs, 0), 15),
    3,
    COALESCE(passive_mode, 1),
    COALESCE(notes, ''),
    1,
    COALESCE(created_at, strftime('%Y-%m-%d %H:%M:%S', 'now', 'localtime')),
    COALESCE(updated_at, strftime('%Y-%m-%d %H:%M:%S', 'now', 'localtime')),
    COALESCE(last_used_at, '')
FROM remote_file_profiles
";

#[derive(Clone)]
pub struct ProfileStore {
    pool: SqlitePool,
}

impl ProfileStore {
    pub fn open(database: Arc<DatabaseService>, key: &str) -> Result<Self> {
        let pool = database.pool(key)?;
        let store = Self { pool };
        store.ensure_schema()?;
        store.import_legacy_if_needed()?;
        Ok(store)
    }

    pub fn list_profiles(&self) -> Result<Vec<Profile>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(LIST_PROFILES)?;
        let rows = stmt.query_map([], map_profile)?;
        let mut items = Vec::new();
        for row in rows {
            items.push(row?);
        }
        Ok(items)
    }

    pub fn get_profile(&self, id: i64) -> Result<Option<Profile>> {
        let conn = self.connection()?;
        conn.query_row(GET_PROFILE, params![id], map_profile)
            .optional()
            .map_err(Into::into)
    }

    pub fn create_profile(&self, draft: &ProfileDraft) -> Result<Profile> {
        let draft = draft.clone().normalize();
        let conn = self.connection()?;
        conn.execute(
            INSERT_PROFILE,
            params![
                draft.name,
                draft.protocol.as_str(),
                draft.host,
                i64::from(draft.port),
                draft.auth.username,
                draft.auth.method.as_str(),
                draft.auth.password,
                draft.auth.private_key_path,
                draft.auth.private_key_passphrase,
                draft.paths.remote_root,
                draft.paths.local_root,
                draft.security.ssh_host_key.as_str(),
                draft.security.pinned_host_key,
                draft.security.tls_verify.as_str(),
                draft.security.pinned_tls_sha256,
                i64::from(draft.limits.connect_timeout_secs),
                i64::from(draft.limits.transfer_concurrency),
                bool_to_int(draft.limits.passive_mode),
                draft.notes,
            ],
        )?;
        let id = conn.last_insert_rowid();
        drop(conn);
        self.get_profile(id)?.context("创建连接配置后无法读取记录")
    }

    pub fn update_profile(&self, id: i64, draft: &ProfileDraft) -> Result<Option<Profile>> {
        let draft = draft.clone().normalize();
        let conn = self.connection()?;
        let affected = conn.execute(
            UPDATE_PROFILE,
            params![
                draft.name,
                draft.protocol.as_str(),
                draft.host,
                i64::from(draft.port),
                draft.auth.username,
                draft.auth.method.as_str(),
                draft.auth.password,
                draft.auth.private_key_path,
                draft.auth.private_key_passphrase,
                draft.paths.remote_root,
                draft.paths.local_root,
                draft.security.ssh_host_key.as_str(),
                draft.security.pinned_host_key,
                draft.security.tls_verify.as_str(),
                draft.security.pinned_tls_sha256,
                i64::from(draft.limits.connect_timeout_secs),
                i64::from(draft.limits.transfer_concurrency),
                bool_to_int(draft.limits.passive_mode),
                draft.notes,
                id,
            ],
        )?;
        if affected == 0 {
            return Ok(None);
        }
        drop(conn);
        self.get_profile(id)
    }

    pub fn delete_profile(&self, id: i64) -> Result<bool> {
        let conn = self.connection()?;
        let affected = conn.execute(DELETE_PROFILE, params![id])?;
        Ok(affected > 0)
    }

    pub fn update_last_used(&self, id: i64) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(UPDATE_LAST_USED, params![id])?;
        Ok(())
    }

    pub fn seed_defaults(&self) -> Result<()> {
        if !self.list_profiles()?.is_empty() {
            return Ok(());
        }
        let drafts = [
            ProfileDraft {
                name: String::from("生产 SSH"),
                protocol: RemoteProtocol::Ssh,
                host: String::from("prod.example.com"),
                auth: AuthConfig {
                    username: String::from("deploy"),
                    method: crate::model::AuthMethod::PrivateKey,
                    private_key_path: String::from("~/.ssh/id_ed25519"),
                    ..AuthConfig::default()
                },
                ..ProfileDraft::default()
            },
            ProfileDraft {
                name: String::from("静态资源 SFTP"),
                protocol: RemoteProtocol::Sftp,
                host: String::from("assets.example.com"),
                port: 22,
                auth: AuthConfig {
                    username: String::from("assets"),
                    ..AuthConfig::default()
                },
                ..ProfileDraft::default()
            },
            ProfileDraft {
                name: String::from("旧系统 FTP"),
                protocol: RemoteProtocol::Ftp,
                host: String::from("legacy.example.com"),
                port: 21,
                auth: AuthConfig {
                    username: String::from("ops"),
                    ..AuthConfig::default()
                },
                ..ProfileDraft::default()
            },
        ];
        for draft in drafts {
            let _ = self.create_profile(&draft)?;
        }
        Ok(())
    }

    fn ensure_schema(&self) -> Result<()> {
        let conn = self.connection()?;
        conn.execute_batch(SCHEMA)?;
        Ok(())
    }

    fn import_legacy_if_needed(&self) -> Result<()> {
        let conn = self.connection()?;
        let count_v2: i64 = conn.query_row(COUNT_V2, [], |row| row.get(0))?;
        if count_v2 > 0 {
            return Ok(());
        }
        let has_legacy: i64 = conn.query_row(COUNT_LEGACY, [], |row| row.get(0))?;
        if has_legacy == 0 {
            return Ok(());
        }
        conn.execute_batch(IMPORT_LEGACY)?;
        Ok(())
    }

    fn connection(&self) -> Result<PooledConnection> {
        self.pool
            .get()
            .context("cannot get remote profile pooled connection")
    }
}

fn map_profile(row: &rusqlite::Row<'_>) -> rusqlite::Result<Profile> {
    let protocol = row.get::<_, String>(2)?;
    let auth_method = row.get::<_, String>(6)?;
    let ssh_policy = row.get::<_, String>(12)?;
    let tls_policy = row.get::<_, String>(14)?;
    Ok(Profile {
        id: row.get(0)?,
        name: row.get(1)?,
        protocol: RemoteProtocol::from_db(&protocol),
        host: row.get(3)?,
        port: row.get::<_, i64>(4)? as u16,
        auth: AuthConfig {
            username: row.get(5)?,
            method: crate::model::AuthMethod::from_db(&auth_method),
            password: row.get(7)?,
            private_key_path: row.get(8)?,
            private_key_passphrase: row.get(9)?,
        },
        paths: ProfilePaths {
            remote_root: row.get(10)?,
            local_root: row.get(11)?,
        },
        security: SecurityPolicy {
            ssh_host_key: SshHostKeyPolicy::from_db(&ssh_policy),
            pinned_host_key: row.get(13)?,
            tls_verify: TlsVerifyPolicy::from_db(&tls_policy),
            pinned_tls_sha256: row.get(15)?,
        },
        limits: ConnectionLimits {
            connect_timeout_secs: row.get::<_, i64>(16)? as u16,
            transfer_concurrency: row.get::<_, i64>(17)? as u16,
            passive_mode: row.get::<_, i64>(18)? != 0,
        },
        notes: row.get(19)?,
        created_at: row.get(20)?,
        updated_at: row.get(21)?,
        last_used_at: row.get(22)?,
    })
}

fn bool_to_int(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };

    use anyhow::Result;

    use super::ProfileStore;
    use crate::model::{ProfileDraft, RemoteProtocol};
    use qingqi_plugin::{
        database::{DatabaseService, feature_database_key},
        storage::AppPaths,
    };

    fn make_store(label: &str) -> Result<ProfileStore> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let root = std::env::temp_dir().join(format!("qingqi-remote-store-{label}-{nanos}"));
        fs::create_dir_all(&root)?;
        let paths = AppPaths::for_test(root);
        let database = Arc::new(DatabaseService::new(paths));
        database.register_databases(crate::databases())?;
        ProfileStore::open(
            database,
            &feature_database_key(crate::manifest::PLUGIN_ID, "profiles"),
        )
    }

    #[test]
    fn crud_roundtrip() -> Result<()> {
        let store = make_store("crud")?;
        let created = store.create_profile(&ProfileDraft {
            name: String::from("demo"),
            host: String::from("example.com"),
            protocol: RemoteProtocol::Ssh,
            ..ProfileDraft::default()
        })?;
        assert_eq!(created.host, "example.com");

        let loaded = store.get_profile(created.id)?.expect("profile exists");
        assert_eq!(loaded.name, "demo");

        let updated = store
            .update_profile(
                created.id,
                &ProfileDraft {
                    name: String::from("changed"),
                    host: String::from("changed.example.com"),
                    protocol: RemoteProtocol::FtpsExplicit,
                    ..ProfileDraft::default()
                },
            )?
            .expect("updated profile");
        assert_eq!(updated.protocol, RemoteProtocol::FtpsExplicit);

        assert!(store.delete_profile(created.id)?);
        assert!(store.get_profile(created.id)?.is_none());
        Ok(())
    }

    #[test]
    fn seed_defaults_populates_when_empty() -> Result<()> {
        let store = make_store("seed")?;
        store.seed_defaults()?;
        let profiles = store.list_profiles()?;
        assert!(!profiles.is_empty());
        Ok(())
    }
}
