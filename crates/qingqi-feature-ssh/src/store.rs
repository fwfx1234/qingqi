//! Profile 持久化存储
//!
//! 基于 SQLite，使用 JSON 字段存储认证配置以支持多协议。

use std::collections::HashSet;
use std::sync::Arc;

use anyhow::{Context, Result};
use rusqlite::{OptionalExtension, params};

use crate::model::{
    AuthConfig, PathConfig, Profile, ProfileAdvanced, ProfileDraft, ProtocolType, SshAuthMethod,
};

pub struct ProfileStore {
    database: Arc<qingqi_plugin::database::DatabaseService>,
    db_path: std::path::PathBuf,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS remote_profiles_v3 (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL DEFAULT '',
    protocol TEXT NOT NULL DEFAULT 'ssh',
    host TEXT NOT NULL DEFAULT '',
    port INTEGER NOT NULL DEFAULT 22,
    auth_json TEXT NOT NULL DEFAULT '{}',
    remote_root TEXT NOT NULL DEFAULT '~',
    local_root TEXT NOT NULL DEFAULT '~/Downloads',
    note TEXT NOT NULL DEFAULT '',
    advanced_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
);

CREATE INDEX IF NOT EXISTS idx_profiles_v3_name
    ON remote_profiles_v3(name);

CREATE TABLE IF NOT EXISTS remote_profiles_v3_migration_log (
    old_id INTEGER PRIMARY KEY,
    new_id INTEGER NOT NULL,
    migrated_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
);
";

impl ProfileStore {
    pub fn new(
        database: Arc<qingqi_plugin::database::DatabaseService>,
        db_path: std::path::PathBuf,
    ) -> Self {
        Self { database, db_path }
    }

    pub fn init(&self) -> Result<()> {
        let conn = self.database.connection_for_path(&self.db_path)?;
        conn.execute_batch(SCHEMA)
            .with_context(|| "创建 remote_profiles_v3 表")?;
        let _ = conn.execute(
            "ALTER TABLE remote_profiles_v3 ADD COLUMN advanced_json TEXT NOT NULL DEFAULT '{}'",
            [],
        );
        Ok(())
    }

    /// 从旧表 remote_profiles_v2 迁移数据
    pub fn migrate_from_v2(&self) -> Result<usize> {
        let conn = self.database.connection_for_path(&self.db_path)?;

        let has_v2: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='remote_profiles_v2'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !has_v2 {
            return Ok(0);
        }

        let migrated: HashSet<i64> = {
            let mut stmt = conn.prepare("SELECT old_id FROM remote_profiles_v3_migration_log")?;
            let rows = stmt.query_map([], |row| row.get::<_, i64>(0))?;
            rows.filter_map(|r| r.ok()).collect()
        };

        let mut stmt = conn.prepare(
            "SELECT id, name, protocol, host, port, username, auth_method, password,
                    private_key_path, private_key_passphrase, remote_root, local_root, notes
             FROM remote_profiles_v2 ORDER BY id ASC",
        )?;

        type OldRow = (
            i64,
            String,
            String,
            String,
            u16,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
        );
        let old_rows: Vec<OldRow> = stmt
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                    row.get(11)?,
                    row.get(12)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut count = 0;
        for (
            old_id,
            name,
            protocol,
            host,
            port,
            _username,
            auth_method,
            password,
            private_key_path,
            private_key_passphrase,
            remote_root,
            local_root,
            note,
        ) in &old_rows
        {
            if migrated.contains(old_id) {
                continue;
            }

            let proto = match protocol.as_str() {
                "ftp" => ProtocolType::Ftp,
                "ftps" => ProtocolType::Ftps,
                _ => ProtocolType::Ssh,
            };

            let auth = if matches!(proto, ProtocolType::Ftp | ProtocolType::Ftps) {
                AuthConfig::Ftp {
                    username: _username.clone(),
                    password: password.clone(),
                }
            } else {
                match auth_method.as_str() {
                    "private_key" => AuthConfig::Ssh {
                        username: _username.clone(),
                        method: SshAuthMethod::PrivateKey {
                            path: private_key_path.clone(),
                            passphrase: private_key_passphrase.clone(),
                        },
                    },
                    "agent" => AuthConfig::Ssh {
                        username: _username.clone(),
                        method: SshAuthMethod::Agent,
                    },
                    _ => AuthConfig::Ssh {
                        username: _username.clone(),
                        method: SshAuthMethod::Password {
                            password: password.clone(),
                        },
                    },
                }
            };

            let paths = PathConfig {
                remote_root: if remote_root.is_empty() {
                    "~".into()
                } else {
                    remote_root.clone()
                },
                local_root: if local_root.is_empty() {
                    "~/Downloads".into()
                } else {
                    local_root.clone()
                },
            };

            let draft = ProfileDraft {
                name: name.clone(),
                protocol: proto,
                host: host.clone(),
                port: *port,
                auth,
                paths,
                advanced: ProfileAdvanced::default(),
                note: note.clone(),
            };

            if let Ok(profile) = self.create(&draft) {
                conn.execute(
                    "INSERT OR IGNORE INTO remote_profiles_v3_migration_log (old_id, new_id) VALUES (?1, ?2)",
                    params![old_id, profile.id],
                )?;
                count += 1;
            }
        }

        Ok(count)
    }

    // ========== CRUD ==========

    pub fn list(&self) -> Result<Vec<Profile>> {
        let conn = self.database.connection_for_path(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT id, name, protocol, host, port, auth_json, remote_root, local_root, note, advanced_json, created_at, updated_at
             FROM remote_profiles_v3 ORDER BY updated_at DESC, id ASC"
        )?;

        let rows = stmt.query_map([], |row| {
            let auth_json: String = row.get(5)?;
            let auth: AuthConfig = serde_json::from_str(&auth_json).unwrap_or_default();
            Self::row_to_profile(row, auth)
        })?;

        let profiles: Vec<_> = rows.filter_map(|r| r.ok()).collect();
        Ok(profiles)
    }

    pub fn get(&self, id: i64) -> Result<Option<Profile>> {
        let conn = self.database.connection_for_path(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT id, name, protocol, host, port, auth_json, remote_root, local_root, note, advanced_json, created_at, updated_at
             FROM remote_profiles_v3 WHERE id = ?1"
        )?;

        stmt.query_row(params![id], |row| {
            let auth_json: String = row.get(5)?;
            let auth: AuthConfig = serde_json::from_str(&auth_json).unwrap_or_default();
            Self::row_to_profile(row, auth)
        })
        .optional()
        .map_err(|e| anyhow::anyhow!("{e}"))
    }

    pub fn create(&self, draft: &ProfileDraft) -> Result<Profile> {
        let conn = self.database.connection_for_path(&self.db_path)?;
        let auth_json = serde_json::to_string(&draft.auth)?;

        let advanced_json = serde_json::to_string(&draft.advanced)?;
        conn.execute(
            "INSERT INTO remote_profiles_v3 (name, protocol, host, port, auth_json, remote_root, local_root, note, advanced_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                draft.name,
                Self::protocol_str(&draft.protocol),
                draft.host,
                draft.port,
                auth_json,
                draft.paths.remote_root,
                draft.paths.local_root,
                draft.note,
                advanced_json,
            ],
        )?;

        let id = conn.last_insert_rowid();
        self.get(id)
            .map(|opt| opt.expect("刚创建的 Profile 必定存在"))
    }

    pub fn update(&self, id: i64, draft: &ProfileDraft) -> Result<Option<Profile>> {
        let conn = self.database.connection_for_path(&self.db_path)?;
        let auth_json = serde_json::to_string(&draft.auth)?;

        let advanced_json = serde_json::to_string(&draft.advanced)?;
        let affected = conn.execute(
            "UPDATE remote_profiles_v3
             SET name = ?1, protocol = ?2, host = ?3, port = ?4, auth_json = ?5,
                 remote_root = ?6, local_root = ?7, note = ?8, advanced_json = ?9,
                 updated_at = datetime('now','localtime')
             WHERE id = ?10",
            params![
                draft.name,
                Self::protocol_str(&draft.protocol),
                draft.host,
                draft.port,
                auth_json,
                draft.paths.remote_root,
                draft.paths.local_root,
                draft.note,
                advanced_json,
                id,
            ],
        )?;

        if affected == 0 {
            return Ok(None);
        }
        self.get(id)
    }

    pub fn delete(&self, id: i64) -> Result<bool> {
        let conn = self.database.connection_for_path(&self.db_path)?;
        let affected = conn.execute("DELETE FROM remote_profiles_v3 WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    // ========== 辅助 ==========

    fn protocol_str(proto: &ProtocolType) -> &'static str {
        match proto {
            ProtocolType::Ssh => "ssh",
            ProtocolType::Ftp => "ftp",
            ProtocolType::Ftps => "ftps",
        }
    }

    fn parse_protocol(s: &str) -> ProtocolType {
        match s {
            "ftp" => ProtocolType::Ftp,
            "ftps" => ProtocolType::Ftps,
            _ => ProtocolType::Ssh,
        }
    }

    fn normalize_auth(protocol: &ProtocolType, auth: AuthConfig) -> AuthConfig {
        match (protocol, &auth) {
            (
                ProtocolType::Ftp | ProtocolType::Ftps,
                AuthConfig::Ssh {
                    username,
                    method: SshAuthMethod::Password { password },
                },
            ) => AuthConfig::Ftp {
                username: username.clone(),
                password: password.clone(),
            },
            (ProtocolType::Ftp | ProtocolType::Ftps, AuthConfig::Ftp { .. }) => auth,
            (ProtocolType::Ftp | ProtocolType::Ftps, _) => AuthConfig::Ftp {
                username: String::new(),
                password: String::new(),
            },
            _ => auth,
        }
    }

    fn row_to_profile(row: &rusqlite::Row<'_>, auth: AuthConfig) -> rusqlite::Result<Profile> {
        let advanced_json: String = row.get(9).unwrap_or_else(|_| "{}".into());
        let advanced: ProfileAdvanced =
            serde_json::from_str(&advanced_json).unwrap_or_default();
        let protocol = Self::parse_protocol(&row.get::<_, String>(2)?);
        Ok(Profile {
            id: row.get(0)?,
            name: row.get(1)?,
            protocol: protocol.clone(),
            host: row.get(3)?,
            port: row.get(4)?,
            auth: Self::normalize_auth(&protocol, auth),
            paths: PathConfig {
                remote_root: row.get(6)?,
                local_root: row.get(7)?,
            },
            note: row.get(8)?,
            advanced,
            created_at: row.get(10)?,
            updated_at: row.get(11)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn temp_store() -> ProfileStore {
        let dir = std::env::temp_dir().join(format!("ssh-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let paths = qingqi_plugin::storage::AppPaths::for_test(dir.clone());
        let database = Arc::new(qingqi_plugin::database::DatabaseService::new(paths));
        let db_path = dir.join("test.db");
        let store = ProfileStore::new(Arc::clone(&database), db_path.clone());
        database
            .register_database(qingqi_plugin::database::DatabaseSpec::path(
                "ssh/profiles",
                db_path,
            ))
            .unwrap();
        store.init().unwrap();
        store
    }

    #[test]
    fn test_crud_profile() {
        let store = temp_store();

        let draft = ProfileDraft {
            name: "test-server".into(),
            protocol: ProtocolType::Ssh,
            host: "192.168.1.1".into(),
            port: 22,
            auth: AuthConfig::Ssh {
                username: "root".into(),
                method: SshAuthMethod::Password {
                    password: "secret".into(),
                },
            },
            ..Default::default()
        };

        // Create
        let profile = store.create(&draft).unwrap();
        assert_eq!(profile.name, "test-server");
        assert_eq!(profile.host, "192.168.1.1");

        // List
        let profiles = store.list().unwrap();
        assert_eq!(profiles.len(), 1);

        // Get
        let fetched = store.get(profile.id).unwrap().unwrap();
        assert_eq!(fetched.name, "test-server");

        // Update
        let mut update_draft = draft;
        update_draft.name = "updated-server".into();
        let updated = store.update(profile.id, &update_draft).unwrap().unwrap();
        assert_eq!(updated.name, "updated-server");

        // Delete
        assert!(store.delete(profile.id).unwrap());
        assert!(store.get(profile.id).unwrap().is_none());
    }

    #[test]
    fn test_delete_nonexistent() {
        let store = temp_store();
        assert!(!store.delete(999).unwrap());
    }

    #[test]
    fn test_list_empty() {
        let store = temp_store();
        let profiles = store.list().unwrap();
        assert!(profiles.is_empty());
    }

    #[test]
    fn test_ftp_profile_auth_roundtrip() {
        let store = temp_store();

        let draft = ProfileDraft {
            name: "ftp-test".into(),
            protocol: ProtocolType::Ftp,
            host: "ftp.example.com".into(),
            port: 21,
            auth: AuthConfig::Ftp {
                username: "anonymous".into(),
                password: "guest".into(),
            },
            ..Default::default()
        };

        let profile = store.create(&draft).unwrap();
        match &profile.auth {
            AuthConfig::Ftp { username, .. } => assert_eq!(username, "anonymous"),
            _ => panic!("应为 Ftp AuthConfig"),
        }
    }
}
