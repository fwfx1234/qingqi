use std::{fs, path::Path};

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};

use crate::features::ftp_sftp_ssh_client::model::{
    AuthMethod, RemoteProfile, RemoteProfileDraft, RemoteProtocol,
};

pub struct RemoteProfileStore {
    conn: Connection,
}

impl RemoteProfileStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("无法创建远程连接配置目录 {}", parent.display()))?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        let store = Self { conn };
        store.ensure_schema()?;
        Ok(store)
    }

    pub fn list_profiles(&self) -> Result<Vec<RemoteProfile>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT id, name, protocol, host, port, username, auth_method, password,
                   private_key_path, private_key_passphrase, remote_dir, local_dir,
                   encoding, passive_mode, connect_timeout_secs, jump_enabled, jump_host,
                   jump_port, jump_username, jump_password, jump_private_key_path,
                   jump_private_key_passphrase, pinned, notes, last_used_at, created_at,
                   updated_at
            FROM remote_file_profiles
            ORDER BY pinned DESC, last_used_at DESC, id ASC
            ",
        )?;
        let rows = stmt.query_map([], map_profile)?;
        let mut profiles = Vec::new();
        for row in rows {
            profiles.push(row?);
        }
        Ok(profiles)
    }

    pub fn get_profile(&self, id: i64) -> Result<Option<RemoteProfile>> {
        self.conn
            .query_row(
                "
                SELECT id, name, protocol, host, port, username, auth_method, password,
                       private_key_path, private_key_passphrase, remote_dir, local_dir,
                       encoding, passive_mode, connect_timeout_secs, jump_enabled, jump_host,
                       jump_port, jump_username, jump_password, jump_private_key_path,
                       jump_private_key_passphrase, pinned, notes, last_used_at, created_at,
                       updated_at
                FROM remote_file_profiles
                WHERE id = ?1
                ",
                params![id],
                map_profile,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn create_profile(&self, draft: &RemoteProfileDraft) -> Result<RemoteProfile> {
        let draft = draft.clone().normalize();
        self.conn.execute(
            "
            INSERT INTO remote_file_profiles
                (name, protocol, host, port, username, auth_method, password,
                 private_key_path, private_key_passphrase, remote_dir, local_dir,
                 encoding, passive_mode, connect_timeout_secs, jump_enabled, jump_host,
                 jump_port, jump_username, jump_password, jump_private_key_path,
                 jump_private_key_passphrase, pinned, notes, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                    ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23,
                    strftime('%Y-%m-%d %H:%M:%S', 'now', 'localtime'),
                    strftime('%Y-%m-%d %H:%M:%S', 'now', 'localtime'))
            ",
            params![
                draft.name,
                draft.protocol.as_str(),
                draft.host,
                i64::from(draft.port),
                draft.username,
                draft.auth_method.as_str(),
                draft.password,
                draft.private_key_path,
                draft.private_key_passphrase,
                draft.remote_dir,
                draft.local_dir,
                draft.encoding,
                if draft.passive_mode { 1 } else { 0 },
                i64::from(draft.connect_timeout_secs),
                if draft.jump_enabled { 1 } else { 0 },
                draft.jump_host,
                i64::from(draft.jump_port),
                draft.jump_username,
                draft.jump_password,
                draft.jump_private_key_path,
                draft.jump_private_key_passphrase,
                if draft.pinned { 1 } else { 0 },
                draft.notes,
            ],
        )?;
        let id = self.conn.last_insert_rowid();
        self.get_profile(id)?
            .context("创建连接配置后无法重新读取记录")
    }

    pub fn update_profile(
        &self,
        id: i64,
        draft: &RemoteProfileDraft,
    ) -> Result<Option<RemoteProfile>> {
        let draft = draft.clone().normalize();
        let affected = self.conn.execute(
            "
            UPDATE remote_file_profiles
            SET name = ?1,
                protocol = ?2,
                host = ?3,
                port = ?4,
                username = ?5,
                auth_method = ?6,
                password = ?7,
                private_key_path = ?8,
                private_key_passphrase = ?9,
                remote_dir = ?10,
                local_dir = ?11,
                encoding = ?12,
                passive_mode = ?13,
                connect_timeout_secs = ?14,
                jump_enabled = ?15,
                jump_host = ?16,
                jump_port = ?17,
                jump_username = ?18,
                jump_password = ?19,
                jump_private_key_path = ?20,
                jump_private_key_passphrase = ?21,
                pinned = ?22,
                notes = ?23,
                updated_at = strftime('%Y-%m-%d %H:%M:%S', 'now', 'localtime')
            WHERE id = ?24
            ",
            params![
                draft.name,
                draft.protocol.as_str(),
                draft.host,
                i64::from(draft.port),
                draft.username,
                draft.auth_method.as_str(),
                draft.password,
                draft.private_key_path,
                draft.private_key_passphrase,
                draft.remote_dir,
                draft.local_dir,
                draft.encoding,
                if draft.passive_mode { 1 } else { 0 },
                i64::from(draft.connect_timeout_secs),
                if draft.jump_enabled { 1 } else { 0 },
                draft.jump_host,
                i64::from(draft.jump_port),
                draft.jump_username,
                draft.jump_password,
                draft.jump_private_key_path,
                draft.jump_private_key_passphrase,
                if draft.pinned { 1 } else { 0 },
                draft.notes,
                id,
            ],
        )?;
        if affected == 0 {
            return Ok(None);
        }
        self.get_profile(id)
    }

    pub fn update_last_used(&self, id: i64) -> Result<()> {
        self.conn.execute(
            "
            UPDATE remote_file_profiles
            SET last_used_at = strftime('%Y-%m-%d %H:%M:%S', 'now', 'localtime'),
                updated_at = strftime('%Y-%m-%d %H:%M:%S', 'now', 'localtime')
            WHERE id = ?1
            ",
            params![id],
        )?;
        Ok(())
    }

    pub fn toggle_pinned(&self, id: i64) -> Result<Option<bool>> {
        let pinned = self
            .conn
            .query_row(
                "SELECT pinned FROM remote_file_profiles WHERE id = ?1",
                params![id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        let Some(pinned) = pinned else {
            return Ok(None);
        };
        let next = if pinned == 0 { 1 } else { 0 };
        self.conn.execute(
            "
            UPDATE remote_file_profiles
            SET pinned = ?1,
                updated_at = strftime('%Y-%m-%d %H:%M:%S', 'now', 'localtime')
            WHERE id = ?2
            ",
            params![next, id],
        )?;
        Ok(Some(next == 1))
    }

    pub fn delete_profile(&self, id: i64) -> Result<bool> {
        Ok(self.conn.execute(
            "DELETE FROM remote_file_profiles WHERE id = ?1",
            params![id],
        )? > 0)
    }

    pub fn seed_defaults(&self) -> Result<usize> {
        let count =
            self.conn
                .query_row("SELECT COUNT(*) FROM remote_file_profiles", [], |row| {
                    row.get::<_, i64>(0)
                })?;
        if count > 0 {
            return Ok(0);
        }
        for index in 0..3 {
            self.create_profile(&RemoteProfileDraft::demo(index))?;
        }
        Ok(3)
    }

    fn ensure_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS remote_file_profiles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL DEFAULT '',
                protocol TEXT NOT NULL DEFAULT 'sftp',
                host TEXT NOT NULL DEFAULT '',
                port INTEGER NOT NULL DEFAULT 22,
                username TEXT NOT NULL DEFAULT '',
                auth_method TEXT NOT NULL DEFAULT 'password',
                password TEXT NOT NULL DEFAULT '',
                private_key_path TEXT NOT NULL DEFAULT '',
                private_key_passphrase TEXT NOT NULL DEFAULT '',
                remote_dir TEXT NOT NULL DEFAULT '/',
                local_dir TEXT NOT NULL DEFAULT '',
                encoding TEXT NOT NULL DEFAULT 'utf-8',
                passive_mode INTEGER NOT NULL DEFAULT 1,
                connect_timeout_secs INTEGER NOT NULL DEFAULT 15,
                jump_enabled INTEGER NOT NULL DEFAULT 0,
                jump_host TEXT NOT NULL DEFAULT '',
                jump_port INTEGER NOT NULL DEFAULT 22,
                jump_username TEXT NOT NULL DEFAULT '',
                jump_password TEXT NOT NULL DEFAULT '',
                jump_private_key_path TEXT NOT NULL DEFAULT '',
                jump_private_key_passphrase TEXT NOT NULL DEFAULT '',
                pinned INTEGER NOT NULL DEFAULT 0,
                notes TEXT NOT NULL DEFAULT '',
                last_used_at TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT '',
                updated_at TEXT NOT NULL DEFAULT ''
            );

            CREATE INDEX IF NOT EXISTS idx_remote_file_profiles_order
                ON remote_file_profiles(pinned DESC, last_used_at DESC, id ASC);
            ",
        )?;
        for sql in [
            "ALTER TABLE remote_file_profiles ADD COLUMN private_key_passphrase TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE remote_file_profiles ADD COLUMN connect_timeout_secs INTEGER NOT NULL DEFAULT 15",
            "ALTER TABLE remote_file_profiles ADD COLUMN jump_enabled INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE remote_file_profiles ADD COLUMN jump_host TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE remote_file_profiles ADD COLUMN jump_port INTEGER NOT NULL DEFAULT 22",
            "ALTER TABLE remote_file_profiles ADD COLUMN jump_username TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE remote_file_profiles ADD COLUMN jump_password TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE remote_file_profiles ADD COLUMN jump_private_key_path TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE remote_file_profiles ADD COLUMN jump_private_key_passphrase TEXT NOT NULL DEFAULT ''",
        ] {
            let _ = self.conn.execute(sql, []);
        }
        Ok(())
    }
}

fn map_profile(row: &rusqlite::Row<'_>) -> rusqlite::Result<RemoteProfile> {
    let protocol: String = row.get(2)?;
    let auth_method: String = row.get(6)?;
    Ok(RemoteProfile {
        id: row.get(0)?,
        name: row.get(1)?,
        protocol: RemoteProtocol::from_db(&protocol),
        host: row.get(3)?,
        port: row.get::<_, i64>(4)?.clamp(1, 65535) as u16,
        username: row.get(5)?,
        auth_method: AuthMethod::from_db(&auth_method),
        password: row.get(7)?,
        private_key_path: row.get(8)?,
        private_key_passphrase: row.get(9)?,
        remote_dir: row.get(10)?,
        local_dir: row.get(11)?,
        encoding: row.get(12)?,
        passive_mode: row.get::<_, i64>(13)? != 0,
        connect_timeout_secs: row.get::<_, i64>(14)?.clamp(1, 600) as u16,
        jump_enabled: row.get::<_, i64>(15)? != 0,
        jump_host: row.get(16)?,
        jump_port: row.get::<_, i64>(17)?.clamp(1, 65535) as u16,
        jump_username: row.get(18)?,
        jump_password: row.get(19)?,
        jump_private_key_path: row.get(20)?,
        jump_private_key_passphrase: row.get(21)?,
        pinned: row.get::<_, i64>(22)? != 0,
        notes: row.get(23)?,
        last_used_at: row.get(24)?,
        created_at: row.get(25)?,
        updated_at: row.get(26)?,
    })
}
