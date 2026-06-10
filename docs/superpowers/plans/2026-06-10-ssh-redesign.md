# SSH 远程管理插件实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 重写 SSH 远程管理插件 `qingqi-feature-ssh`，支持 SSH/SFTP/FTP/FTPS 四种协议，替代旧插件 `qingqi-feature-ftp-sftp-ssh-client`

**Architecture:** 采用 RemoteProtocol trait 抽象四协议差异，SshService 通过 ConnectionPool + ProtocolRegistry 管理连接，SshView 通过 View-Model 模式和 broadcast 事件订阅实现响应式 UI。分离式顶栏布局（左侧边栏 + 右侧 Tab 区域），传输面板嵌入 Tab 内容内。

**Tech Stack:** Rust + GPUI + russh (SSH/SFTP) + suppaftp (FTP/FTPS) + alacritty_terminal + rusqlite

---

## 文件清单

### 新建文件

```
crates/qingqi-feature-ssh/
├── Cargo.toml
└── src/
    ├── lib.rs                  # 公开导出 + databases() + build()
    ├── manifest.rs             # PLUGIN_ID="ssh", Manifest 元数据
    ├── plugin.rs               # SshPlugin: impl Plugin
    ├── model.rs                # 领域类型（纯数据）
    ├── store.rs                # ProfileStore（数据库 CRUD）
    ├── service.rs              # SshService（核心服务组装）
    ├── connection.rs           # ConnectionPool + ProtocolRegistry
    ├── protocol/
    │   ├── mod.rs              # RemoteProtocol trait 定义
    │   ├── ssh.rs              # SshProtocol: russh 实现
    │   └── ftp.rs              # FtpProtocol/FtpsProtocol: suppaftp 实现
    ├── terminal.rs             # TerminalEngine（PTY + 日志双模式）
    ├── transfer.rs             # TransferQueue（传输队列 + 详细日志）
    └── view/
        ├── mod.rs              # SshView + SshViewModel
        ├── sidebar.rs          # 左侧 Profile 边栏
        ├── session_tabs.rs     # Session Tab 栏
        ├── file_tree.rs        # 文件树面板
        ├── terminal_pane.rs    # 终端面板（SSH/FTP 透明）
        ├── transfer_panel.rs   # 传输记录面板
        └── settings_dialog.rs  # Profile 编辑弹窗
```

### 修改文件

- `Cargo.toml`（workspace）：添加 `qingqi-feature-ssh` 成员
- `crates/qingqi/Cargo.toml`：添加 `qingqi-feature-ssh` 依赖
- `crates/qingqi/src/features/registry.rs`：注册新插件，注释旧插件

---

## 阶段 0：项目骨架搭建

### Task 0.1: 创建 Cargo.toml 和工作区注册

**Files:**
- Create: `crates/qingqi-feature-ssh/Cargo.toml`
- Modify: `Cargo.toml`

- [ ] **Step 1: 创建 crate 目录**

```bash
mkdir -p crates/qingqi-feature-ssh/src/protocol
mkdir -p crates/qingqi-feature-ssh/src/view
```

- [ ] **Step 2: 编写 Cargo.toml**

创建 `crates/qingqi-feature-ssh/Cargo.toml`：

```toml
[package]
name = "qingqi-feature-ssh"
version = "0.1.0"
edition = "2024"

[dependencies]
gpui.workspace = true
qingqi-plugin.workspace = true
qingqi-ui.workspace = true
gpui-component.workspace = true

russh = "0.47"
russh-sftp = "2"
suppaftp = { version = "6", features = ["async-ssl"] }
alacritty_terminal = "0.24"
anyhow.workspace = true
tokio = { version = "1", features = ["full"] }
uuid = { version = "1", features = ["v4"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rusqlite = { version = "0.31", features = ["bundled"] }
async-trait = "0.1"
log = "0.4"
```

- [ ] **Step 3: 注册 workspace 成员**

在根 `Cargo.toml` 的 `[workspace]` 节 `members` 数组中添加：

```toml
"crates/qingqi-feature-ssh",
```

- [ ] **Step 4: 验证编译**

```bash
cargo check -p qingqi-feature-ssh
```

Expected: `error: couldn't read .../src/lib.rs: No such file or directory`（正常，下一步创建）

- [ ] **Step 5: Commit**

```bash
git add crates/qingqi-feature-ssh/Cargo.toml Cargo.toml
git commit -m "feat(ssh): 创建 qingqi-feature-ssh crate 骨架与依赖"
```

### Task 0.2: 创建基础源文件

**Files:**
- Create: `crates/qingqi-feature-ssh/src/lib.rs`
- Create: `crates/qingqi-feature-ssh/src/manifest.rs`
- Create: `crates/qingqi-feature-ssh/src/plugin.rs`
- Create: `crates/qingqi-feature-ssh/src/model.rs`
- Create: `crates/qingqi-feature-ssh/src/store.rs`
- Create: `crates/qingqi-feature-ssh/src/service.rs`
- Create: `crates/qingqi-feature-ssh/src/connection.rs`
- Create: `crates/qingqi-feature-ssh/src/protocol/mod.rs`
- Create: `crates/qingqi-feature-ssh/src/protocol/ssh.rs`
- Create: `crates/qingqi-feature-ssh/src/protocol/ftp.rs`
- Create: `crates/qingqi-feature-ssh/src/terminal.rs`
- Create: `crates/qingqi-feature-ssh/src/transfer.rs`
- Create: `crates/qingqi-feature-ssh/src/view/mod.rs`
- Create: `crates/qingqi-feature-ssh/src/view/sidebar.rs`
- Create: `crates/qingqi-feature-ssh/src/view/session_tabs.rs`
- Create: `crates/qingqi-feature-ssh/src/view/file_tree.rs`
- Create: `crates/qingqi-feature-ssh/src/view/terminal_pane.rs`
- Create: `crates/qingqi-feature-ssh/src/view/transfer_panel.rs`
- Create: `crates/qingqi-feature-ssh/src/view/settings_dialog.rs`

- [ ] **Step 1: 创建 lib.rs 最小骨架**

```rust
// crates/qingqi-feature-ssh/src/lib.rs
pub mod manifest;
pub mod model;
pub mod plugin;
pub mod store;
pub mod service;
pub mod connection;
pub mod protocol;
pub mod terminal;
pub mod transfer;
pub mod view;

use std::sync::Arc;

use anyhow::Result;
use qingqi_plugin::{
    database::{DatabaseService, DatabaseSpec},
    plugin::Plugin,
};

pub fn databases() -> Vec<DatabaseSpec> {
    vec![DatabaseSpec::feature("ssh", "profiles", "ssh_profiles.db")]
}

pub fn build(database: Arc<DatabaseService>, paths: std::sync::Arc<qingqi_plugin::storage::AppPaths>) -> Result<Box<dyn Plugin>> {
    let profile_db_path = database.path_for_key("ssh/profiles")?;
    let profile_store = Arc::new(store::ProfileStore::new(Arc::clone(&database), profile_db_path));
    let service = Arc::new(service::SshService::new(
        Arc::clone(&database),
        profile_store,
        paths.cache_dir().to_path_buf(),
    ));
    Ok(Box::new(plugin::SshPlugin { service }))
}
```

- [ ] **Step 2: 创建空模块文件（每个文件含一行 #![allow(unused)] 占位）**

为以下每个文件写入最小内容 `// 占位`：
`manifest.rs`, `plugin.rs`, `model.rs`, `store.rs`, `service.rs`, `connection.rs`, `protocol/mod.rs`, `protocol/ssh.rs`, `protocol/ftp.rs`, `terminal.rs`, `transfer.rs`, `view/mod.rs`, `view/sidebar.rs`, `view/session_tabs.rs`, `view/file_tree.rs`, `view/terminal_pane.rs`, `view/transfer_panel.rs`, `view/settings_dialog.rs`

```bash
for f in manifest plugin model store service connection terminal transfer \
  protocol/mod protocol/ssh protocol/ftp \
  view/mod view/sidebar view/session_tabs view/file_tree \
  view/terminal_pane view/transfer_panel view/settings_dialog; do
  echo "// 占位" > crates/qingqi-feature-ssh/src/${f}.rs
done
```

- [ ] **Step 3: 验证编译**

```bash
cargo check -p qingqi-feature-ssh 2>&1 | head -20
```

Expected: 编译错误（lib.rs 引用了尚不存在的类型），这是预期行为，下一步开始实现。

- [ ] **Step 4: Commit**

```bash
git add crates/qingqi-feature-ssh/src/
git commit -m "feat(ssh): 创建所有源文件骨架"
```

---

## 阶段 1：模型与存储层

### Task 1.1: 实现 model.rs（领域类型）

**Files:**
- Write: `crates/qingqi-feature-ssh/src/model.rs`

- [ ] **Step 1: 编写完整 model.rs**

```rust
// crates/qingqi-feature-ssh/src/model.rs
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============ 协议类型 ============

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtocolType {
    #[serde(rename = "ssh")]
    Ssh,
    #[serde(rename = "ftp")]
    Ftp,
    #[serde(rename = "ftps")]
    Ftps,
}

impl ProtocolType {
    pub fn default_port(&self) -> u16 {
        match self {
            Self::Ssh => 22,
            Self::Ftp => 21,
            Self::Ftps => 990,
        }
    }

    pub fn display(&self) -> &'static str {
        match self {
            Self::Ssh => "SSH",
            Self::Ftp => "FTP",
            Self::Ftps => "FTPS",
        }
    }

    pub fn supports_terminal(&self) -> TerminalKind {
        match self {
            Self::Ssh => TerminalKind::Shell,
            Self::Ftp | Self::Ftps => TerminalKind::Log,
        }
    }
}

// ============ 认证配置 ============

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthConfig {
    Ssh {
        method: SshAuthMethod,
    },
    Ftp {
        username: String,
        password: String,
    },
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self::Ssh {
            method: SshAuthMethod::Password {
                password: String::new(),
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SshAuthMethod {
    Password { password: String },
    PrivateKey { path: String, passphrase: String },
    Agent,
}

// ============ 路径配置 ============

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathConfig {
    pub remote_root: String,
    pub local_root: String,
}

impl Default for PathConfig {
    fn default() -> Self {
        Self {
            remote_root: "~".into(),
            local_root: "~/Downloads".into(),
        }
    }
}

// ============ Profile ============

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub id: i64,
    pub name: String,
    pub protocol: ProtocolType,
    pub host: String,
    pub port: u16,
    pub auth: AuthConfig,
    pub paths: PathConfig,
    pub note: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug)]
pub struct ProfileDraft {
    pub name: String,
    pub protocol: ProtocolType,
    pub host: String,
    pub port: u16,
    pub auth: AuthConfig,
    pub paths: PathConfig,
    pub note: String,
}

impl Default for ProfileDraft {
    fn default() -> Self {
        Self {
            name: String::new(),
            protocol: ProtocolType::Ssh,
            host: String::new(),
            port: 22,
            auth: AuthConfig::default(),
            paths: PathConfig::default(),
            note: String::new(),
        }
    }
}

// ============ Session ============

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SessionId(pub Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionStatus {
    Connecting,
    Connected,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerminalKind {
    Shell,
    Log,
}

#[derive(Clone, Debug)]
pub struct SessionSummary {
    pub session_id: SessionId,
    pub profile_id: i64,
    pub title: String,
    pub endpoint: String,
    pub protocol: ProtocolType,
    pub status: SessionStatus,
    pub terminal_kind: TerminalKind,
    pub has_terminal: bool,
    pub message: String,
}

// ============ 文件系统 ============

#[derive(Clone, Debug)]
pub struct RemoteEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified_at: String,
}

// ============ 传输 ============

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TransferId(pub Uuid);

impl TransferId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferDirection {
    Upload,
    Download,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug)]
pub struct TransferTask {
    pub id: TransferId,
    pub session_id: SessionId,
    pub direction: TransferDirection,
    pub status: TransferStatus,
    pub local_path: String,
    pub remote_path: String,
    pub transferred_bytes: u64,
    pub total_bytes: u64,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub message: String,
    pub logs: Vec<String>,
}

// ============ 服务层快照 ============

#[derive(Clone, Debug)]
pub struct SshSnapshot {
    pub profiles: Vec<Profile>,
    pub sessions: Vec<SessionSummary>,
    pub revision: u64,
}

#[derive(Clone, Debug)]
pub struct SessionSnapshot {
    pub summary: SessionSummary,
    pub entries: Vec<RemoteEntry>,
    pub remote_cwd: String,
}
```

- [ ] **Step 2: 验证编译**

```bash
cargo check -p qingqi-feature-ssh
```

Expected: 错误减少，只剩 store.rs / service.rs / plugin.rs 引用问题

- [ ] **Step 3: 确认 model.rs 不依赖 GPUI**

```bash
cargo tree -p qingqi-feature-ssh -e no-dev --invert gpui 2>&1 | grep "qingqi-feature-ssh"
```

Expected: 空（model.rs 不应触发 gpui 依赖）

- [ ] **Step 4: Commit**

```bash
git add crates/qingqi-feature-ssh/src/model.rs
git commit -m "feat(ssh): 实现 model.rs 领域类型（无 GPUI 依赖）"
```

### Task 1.2: 实现 store.rs（Profile 持久化）

**Files:**
- Write: `crates/qingqi-feature-ssh/src/store.rs`

- [ ] **Step 1: 编写 store.rs**

```rust
// crates/qingqi-feature-ssh/src/store.rs
use std::sync::Arc;

use anyhow::{Context, Result};
use rusqlite::{params, OptionalExtension};

use crate::model::{
    AuthConfig, PathConfig, Profile, ProfileDraft, ProtocolType, SshAuthMethod,
};

pub struct ProfileStore {
    database: Arc<qingqi_plugin::database::DatabaseService>,
    db_path: std::path::PathBuf,
}

/// 新 Profile 表 schema（使用 auth_json 替代旧版的多个认证字段）
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

    /// 初始化表结构（幂等）
    pub fn init(&self) -> Result<()> {
        let conn = self.database.connection_for_path(&self.db_path)?;
        conn.execute_batch(SCHEMA)
            .with_context(|| "创建 remote_profiles_v3 表")?;
        Ok(())
    }

    /// 尝试从旧表 remote_profiles_v2 迁移数据到 v3
    pub fn migrate_from_v2(&self) -> Result<usize> {
        let conn = self.database.connection_for_path(&self.db_path)?;

        // 检查旧表是否存在
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

        // 查询已迁移过的 old_id
        let migrated: std::collections::HashSet<i64> = {
            let mut stmt = conn
                .prepare("SELECT old_id FROM remote_profiles_v3_migration_log")?;
            let rows = stmt.query_map([], |row| row.get(0))?;
            let mut set = std::collections::HashSet::new();
            for r in rows {
                if let Ok(id) = r {
                    set.insert(id);
                }
            }
            set
        };

        // 读取旧表数据
        let mut stmt = conn.prepare(
            "SELECT id, name, protocol, host, port, username, auth_method, password,
                    private_key_path, private_key_passphrase, remote_root, local_root, notes
             FROM remote_profiles_v2
             ORDER BY id ASC"
        )?;
        let old_rows: Vec<(i64, String, String, String, u16, String, String, String, String, String, String, String, String)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?,
                    row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?,
                    row.get(8)?, row.get(9)?, row.get(10)?, row.get(11)?,
                    row.get(12)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut count = 0;
        for (old_id, name, protocol, host, port, username, auth_method, password, private_key_path, private_key_passphrase, remote_root, local_root, note) in &old_rows {
            if migrated.contains(old_id) {
                continue;
            }

            // 解析协议类型
            let proto = match protocol.as_str() {
                "ftp" => ProtocolType::Ftp,
                "ftps" => ProtocolType::Ftps,
                _ => ProtocolType::Ssh,
            };

            // 构建 AuthConfig
            let auth = match auth_method.as_str() {
                "private_key" => AuthConfig::Ssh {
                    method: SshAuthMethod::PrivateKey {
                        path: private_key_path.clone(),
                        passphrase: private_key_passphrase.clone(),
                    },
                },
                "agent" => AuthConfig::Ssh {
                    method: SshAuthMethod::Agent,
                },
                _ => AuthConfig::Ssh {
                    method: SshAuthMethod::Password {
                        password: password.clone(),
                    },
                },
            };

            let paths = PathConfig {
                remote_root: if remote_root.is_empty() { "~".into() } else { remote_root.clone() },
                local_root: if local_root.is_empty() { "~/Downloads".into() } else { local_root.clone() },
            };

            let draft = ProfileDraft {
                name: name.clone(),
                protocol: proto,
                host: host.clone(),
                port: *port,
                auth,
                paths,
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
            "SELECT id, name, protocol, host, port, auth_json, remote_root, local_root, note, created_at, updated_at
             FROM remote_profiles_v3
             ORDER BY updated_at DESC, id ASC"
        )?;

        let rows = stmt.query_map([], |row| {
            let auth_json: String = row.get(5)?;
            let auth: AuthConfig = serde_json::from_str(&auth_json).unwrap_or_default();
            Ok(Profile {
                id: row.get(0)?,
                name: row.get(1)?,
                protocol: Self::parse_protocol(&row.get::<_, String>(2)?),
                host: row.get(3)?,
                port: row.get(4)?,
                auth,
                paths: PathConfig {
                    remote_root: row.get(6)?,
                    local_root: row.get(7)?,
                },
                note: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?;

        let mut profiles = Vec::new();
        for r in rows {
            if let Ok(p) = r {
                profiles.push(p);
            }
        }
        Ok(profiles)
    }

    pub fn get(&self, id: i64) -> Result<Option<Profile>> {
        let conn = self.database.connection_for_path(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT id, name, protocol, host, port, auth_json, remote_root, local_root, note, created_at, updated_at
             FROM remote_profiles_v3 WHERE id = ?1"
        )?;

        let result = stmt.query_row(params![id], |row| {
            let auth_json: String = row.get(5)?;
            let auth: AuthConfig = serde_json::from_str(&auth_json).unwrap_or_default();
            Ok(Profile {
                id: row.get(0)?,
                name: row.get(1)?,
                protocol: Self::parse_protocol(&row.get::<_, String>(2)?),
                host: row.get(3)?,
                port: row.get(4)?,
                auth,
                paths: PathConfig {
                    remote_root: row.get(6)?,
                    local_root: row.get(7)?,
                },
                note: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        }).optional()?;

        Ok(result)
    }

    pub fn create(&self, draft: &ProfileDraft) -> Result<Profile> {
        let conn = self.database.connection_for_path(&self.db_path)?;
        let auth_json = serde_json::to_string(&draft.auth)?;

        conn.execute(
            "INSERT INTO remote_profiles_v3 (name, protocol, host, port, auth_json, remote_root, local_root, note)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                draft.name,
                Self::protocol_str(&draft.protocol),
                draft.host,
                draft.port,
                auth_json,
                draft.paths.remote_root,
                draft.paths.local_root,
                draft.note,
            ],
        )?;

        let id = conn.last_insert_rowid();
        self.get(id).map(|p| p.expect("刚创建的 Profile 必定存在"))
    }

    pub fn update(&self, id: i64, draft: &ProfileDraft) -> Result<Option<Profile>> {
        let conn = self.database.connection_for_path(&self.db_path)?;
        let auth_json = serde_json::to_string(&draft.auth)?;

        let affected = conn.execute(
            "UPDATE remote_profiles_v3
             SET name = ?1, protocol = ?2, host = ?3, port = ?4, auth_json = ?5,
                 remote_root = ?6, local_root = ?7, note = ?8,
                 updated_at = datetime('now','localtime')
             WHERE id = ?9",
            params![
                draft.name,
                Self::protocol_str(&draft.protocol),
                draft.host,
                draft.port,
                auth_json,
                draft.paths.remote_root,
                draft.paths.local_root,
                draft.note,
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn temp_store() -> ProfileStore {
        let dir = std::env::temp_dir().join(format!("ssh-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let paths = qingqi_plugin::storage::AppPaths::for_test(dir.clone());
        let database = Arc::new(qingqi_plugin::database::DatabaseService::new(paths));
        let db_path = dir.join("test.db");
        let store = ProfileStore::new(Arc::clone(&database), db_path.clone());
        database.register_database(qingqi_plugin::database::DatabaseSpec::path("ssh/profiles", db_path)).unwrap();
        store.init().unwrap();
        store
    }

    #[test]
    fn test_crud_profile() {
        let store = temp_store();

        // Create
        let draft = ProfileDraft {
            name: "test-server".into(),
            protocol: ProtocolType::Ssh,
            host: "192.168.1.1".into(),
            port: 22,
            auth: AuthConfig::Ssh {
                method: SshAuthMethod::Password {
                    password: "secret".into(),
                },
            },
            ..Default::default()
        };

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
        let mut update_draft = draft.clone();
        update_draft.name = "updated-server".into();
        let updated = store.update(profile.id, &update_draft).unwrap().unwrap();
        assert_eq!(updated.name, "updated-server");

        // Delete
        assert!(store.delete(profile.id).unwrap());
        assert!(store.get(profile.id).unwrap().is_none());
    }
}
```

- [ ] **Step 2: 运行单元测试**

```bash
cargo test -p qingqi-feature-ssh -- store::tests::test_crud_profile
```

Expected: PASS

- [ ] **Step 3: 更新 lib.rs 以正确初始化 store**

修改 `crates/qingqi-feature-ssh/src/lib.rs` 中 build 函数：

```rust
pub fn build(
    database: Arc<DatabaseService>,
    paths: Arc<qingqi_plugin::storage::AppPaths>,
) -> Result<Box<dyn Plugin>> {
    let profile_db_path = database.path_for_key("ssh/profiles")?;
    let profile_store = Arc::new(store::ProfileStore::new(
        Arc::clone(&database),
        profile_db_path,
    ));
    profile_store.init()?;
    // 尝试迁移旧数据
    let migrated = profile_store.migrate_from_v2().unwrap_or(0);
    if migrated > 0 {
        log::info!("从旧版迁移了 {migrated} 个 Profile");
    }

    let service = Arc::new(service::SshService::new(
        Arc::clone(&database),
        profile_store,
        paths.cache_dir().to_path_buf(),
    ));
    Ok(Box::new(plugin::SshPlugin { service }))
}
```

- [ ] **Step 4: 验证编译**

```bash
cargo check -p qingqi-feature-ssh
```

Expected: store 相关错误消失，剩余 service/plugin 错误

- [ ] **Step 5: Commit**

```bash
git add crates/qingqi-feature-ssh/src/store.rs crates/qingqi-feature-ssh/src/lib.rs
git commit -m "feat(ssh): 实现 ProfileStore CRUD 与旧版数据迁移"
```

---

## 阶段 2：协议抽象层

### Task 2.1: 定义 RemoteProtocol trait

**Files:**
- Write: `crates/qingqi-feature-ssh/src/protocol/mod.rs`

- [ ] **Step 1: 编写 protocol/mod.rs**

```rust
// crates/qingqi-feature-ssh/src/protocol/mod.rs
use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::model::{Profile, ProtocolType, RemoteEntry};

// ============ 协议能力 ============

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProtocolCapability {
    InteractiveTerminal,  // SSH: 交互式 shell
    LogTerminal,          // FTP/FTPS: 命令响应日志
}

// ============ 终端输出 ============

#[derive(Clone, Debug)]
pub enum TerminalOutput {
    /// SSH: PTY 原始输出（含 ANSI 转义序列）
    PtyOutput(Vec<u8>),
    /// FTP: 日志行
    LogLine { level: LogLevel, text: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Info,    // 一般信息
    Sent,    // 发出的命令（对应 FTP >）
    Received,// 收到的响应（对应 FTP <）
    Error,   // 错误
}

// ============ 传输进度 ============

#[derive(Clone, Debug)]
pub struct TransferProgress {
    pub transferred_bytes: u64,
    pub total_bytes: u64,
    pub speed_bytes_per_sec: f64,
}

// ============ RemoteProtocol trait ============

#[async_trait]
pub trait RemoteProtocol: Send + Sync {
    /// 连接到远程服务器
    async fn connect(&self) -> Result<()>;

    /// 断开连接
    async fn disconnect(&self);

    /// 是否已连接
    fn is_connected(&self) -> bool;

    /// 返回协议能力列表
    fn capabilities(&self) -> Vec<ProtocolCapability>;

    /// 打开终端通道
    /// SSH → PTY stream，FTP → 日志 stream
    async fn open_terminal(&self) -> Result<mpsc::UnboundedReceiver<TerminalOutput>>;

    /// 发送终端输入
    /// SSH → 传给 PTY，FTP → 执行原生命令并产生日志
    async fn send_terminal_input(&self, data: &[u8]) -> Result<()>;

    /// 调整终端大小（仅 SSH）
    async fn resize_terminal(&self, _cols: u16, _rows: u16) -> Result<()> {
        Ok(())
    }

    /// 列出目录内容
    async fn list_directory(&self, path: &str) -> Result<Vec<RemoteEntry>>;

    /// 创建目录
    async fn create_directory(&self, path: &str) -> Result<()>;

    /// 重命名
    async fn rename_entry(&self, old_path: &str, new_path: &str) -> Result<()>;

    /// 删除文件
    async fn remove_file(&self, path: &str) -> Result<()>;

    /// 删除目录
    async fn remove_directory(&self, path: &str) -> Result<()>;

    /// 上传文件（进度通过 channel 回传）
    async fn upload_file(
        &self,
        local: &Path,
        remote: &str,
        progress_tx: mpsc::UnboundedSender<TransferProgress>,
    ) -> Result<()>;

    /// 下载文件（进度通过 channel 回传）
    async fn download_file(
        &self,
        remote: &str,
        local: &Path,
        progress_tx: mpsc::UnboundedSender<TransferProgress>,
    ) -> Result<()>;
}

// ============ ProtocolFactory（工厂注册表） ============

pub type ProtocolFactory = Box<dyn Fn(&Profile) -> Result<Box<dyn RemoteProtocol>> + Send + Sync>;

pub struct ProtocolRegistry {
    factories: std::collections::HashMap<ProtocolType, ProtocolFactory>,
}

impl ProtocolRegistry {
    pub fn new() -> Self {
        Self {
            factories: std::collections::HashMap::new(),
        }
    }

    pub fn register(&mut self, proto: ProtocolType, factory: ProtocolFactory) {
        self.factories.insert(proto, factory);
    }

    pub fn create(&self, profile: &Profile) -> Result<Box<dyn RemoteProtocol>> {
        let factory = self
            .factories
            .get(&profile.protocol)
            .ok_or_else(|| anyhow::anyhow!("不支持的协议: {:?}", profile.protocol))?;
        factory(profile)
    }
}

impl Default for ProtocolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 2: 编写 protocol/ssh.rs 骨架（后续任务填充实现）**

```rust
// crates/qingqi-feature-ssh/src/protocol/ssh.rs
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;

use super::{LogLevel, ProtocolCapability, RemoteProtocol, TerminalOutput, TransferProgress};
use crate::model::{Profile, RemoteEntry};

pub struct SshProtocol {}

impl SshProtocol {
    pub fn new(_profile: &Profile) -> Result<Self> {
        Ok(Self {})
    }
}

#[async_trait]
impl RemoteProtocol for SshProtocol {
    async fn connect(&self) -> Result<()> {
        // TODO: Task 2.2 实现
        Ok(())
    }

    async fn disconnect(&self) {}

    fn is_connected(&self) -> bool {
        false
    }

    fn capabilities(&self) -> Vec<ProtocolCapability> {
        vec![ProtocolCapability::InteractiveTerminal]
    }

    async fn open_terminal(&self) -> Result<mpsc::UnboundedReceiver<TerminalOutput>> {
        Err(anyhow::anyhow!("未实现"))
    }

    async fn send_terminal_input(&self, _data: &[u8]) -> Result<()> {
        Err(anyhow::anyhow!("未实现"))
    }

    async fn resize_terminal(&self, _cols: u16, _rows: u16) -> Result<()> {
        Ok(())
    }

    async fn list_directory(&self, _path: &str) -> Result<Vec<RemoteEntry>> {
        Err(anyhow::anyhow!("未实现"))
    }

    async fn create_directory(&self, _path: &str) -> Result<()> {
        Err(anyhow::anyhow!("未实现"))
    }

    async fn rename_entry(&self, _old_path: &str, _new_path: &str) -> Result<()> {
        Err(anyhow::anyhow!("未实现"))
    }

    async fn remove_file(&self, _path: &str) -> Result<()> {
        Err(anyhow::anyhow!("未实现"))
    }

    async fn remove_directory(&self, _path: &str) -> Result<()> {
        Err(anyhow::anyhow!("未实现"))
    }

    async fn upload_file(
        &self,
        _local: &Path,
        _remote: &str,
        _progress_tx: mpsc::UnboundedSender<TransferProgress>,
    ) -> Result<()> {
        Err(anyhow::anyhow!("未实现"))
    }

    async fn download_file(
        &self,
        _remote: &str,
        _local: &Path,
        _progress_tx: mpsc::UnboundedSender<TransferProgress>,
    ) -> Result<()> {
        Err(anyhow::anyhow!("未实现"))
    }
}
```

- [ ] **Step 3: 编写 protocol/ftp.rs 骨架**

```rust
// crates/qingqi-feature-ssh/src/protocol/ftp.rs
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;

use super::{ProtocolCapability, RemoteProtocol, TerminalOutput, TransferProgress};
use crate::model::{Profile, RemoteEntry};

pub struct FtpProtocol {}

impl FtpProtocol {
    pub fn new(_profile: &Profile) -> Result<Self> {
        Ok(FtpProtocol {})
    }
}

#[async_trait]
impl RemoteProtocol for FtpProtocol {
    async fn connect(&self) -> Result<()> { Ok(()) }
    async fn disconnect(&self) {}
    fn is_connected(&self) -> bool { false }
    fn capabilities(&self) -> Vec<ProtocolCapability> {
        vec![ProtocolCapability::LogTerminal]
    }
    async fn open_terminal(&self) -> Result<mpsc::UnboundedReceiver<TerminalOutput>> {
        Err(anyhow::anyhow!("未实现"))
    }
    async fn send_terminal_input(&self, _data: &[u8]) -> Result<()> {
        Err(anyhow::anyhow!("未实现"))
    }
    async fn list_directory(&self, _path: &str) -> Result<Vec<RemoteEntry>> {
        Err(anyhow::anyhow!("未实现"))
    }
    async fn create_directory(&self, _path: &str) -> Result<()> {
        Err(anyhow::anyhow!("未实现"))
    }
    async fn rename_entry(&self, _old_path: &str, _new_path: &str) -> Result<()> {
        Err(anyhow::anyhow!("未实现"))
    }
    async fn remove_file(&self, _path: &str) -> Result<()> {
        Err(anyhow::anyhow!("未实现"))
    }
    async fn remove_directory(&self, _path: &str) -> Result<()> {
        Err(anyhow::anyhow!("未实现"))
    }
    async fn upload_file(
        &self, _local: &Path, _remote: &str,
        _progress_tx: mpsc::UnboundedSender<TransferProgress>,
    ) -> Result<()> { Err(anyhow::anyhow!("未实现")) }
    async fn download_file(
        &self, _remote: &str, _local: &Path,
        _progress_tx: mpsc::UnboundedSender<TransferProgress>,
    ) -> Result<()> { Err(anyhow::anyhow!("未实现")) }
}
```

- [ ] **Step 4: 验证编译**

```bash
cargo check -p qingqi-feature-ssh
```

- [ ] **Step 5: Commit**

```bash
git add crates/qingqi-feature-ssh/src/protocol/
git commit -m "feat(ssh): 定义 RemoteProtocol trait 与协议骨架"
```

---

## 阶段 3：服务层核心

### Task 3.1: 实现 ConnectionPool + ProtocolRegistry 注册

**Files:**
- Write: `crates/qingqi-feature-ssh/src/connection.rs`

- [ ] **Step 1: 编写 connection.rs**

```rust
// crates/qingqi-feature-ssh/src/connection.rs
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use tokio::sync::Mutex as TokioMutex;

use crate::model::{Profile, ProtocolType};
use crate::protocol::{ProtocolRegistry, RemoteProtocol};

/// 连接池 — 按 profile_id 管理活跃连接
pub struct ConnectionPool {
    registry: ProtocolRegistry,
    connections: TokioMutex<HashMap<i64, Arc<dyn RemoteProtocol>>>,
}

impl ConnectionPool {
    pub fn new(registry: ProtocolRegistry) -> Self {
        Self {
            registry,
            connections: TokioMutex::new(HashMap::new()),
        }
    }

    /// 获取或创建连接（自动识别协议类型，已连接则复用）
    pub async fn get_or_connect(&self, profile: &Profile) -> Result<Arc<dyn RemoteProtocol>> {
        let mut conns = self.connections.lock().await;

        // 已有连接直接复用
        if let Some(existing) = conns.get(&profile.id) {
            if existing.is_connected() {
                return Ok(Arc::clone(existing));
            }
            // 连接已断开，移除
            conns.remove(&profile.id);
        }

        // 创建新连接
        let protocol = self.registry.create(profile)?;
        protocol.connect().await?;

        let arc_proto: Arc<dyn RemoteProtocol> = Arc::from(protocol);
        conns.insert(profile.id, Arc::clone(&arc_proto));
        Ok(arc_proto)
    }

    /// 断开指定 Profile 的连接
    pub async fn disconnect(&self, profile_id: i64) {
        let mut conns = self.connections.lock().await;
        if let Some(proto) = conns.remove(&profile_id) {
            proto.disconnect().await;
        }
    }

    /// 关闭所有连接
    pub async fn close_all(&self) {
        let conns = {
            let mut guard = self.connections.lock().await;
            std::mem::take(&mut *guard)
        };
        for (_, proto) in conns {
            proto.disconnect().await;
        }
    }
}

/// 构建默认的 ProtocolRegistry（注册 SSH + FTP + FTPS 工厂）
pub fn default_registry() -> ProtocolRegistry {
    let mut registry = ProtocolRegistry::new();

    registry.register(ProtocolType::Ssh, Box::new(|profile| {
        Ok(Box::new(crate::protocol::ssh::SshProtocol::new(profile)?))
    }));

    registry.register(ProtocolType::Ftp, Box::new(|profile| {
        Ok(Box::new(crate::protocol::ftp::FtpProtocol::new(profile)?))
    }));

    registry.register(ProtocolType::Ftps, Box::new(|profile| {
        // FTPS 复用 FtpProtocol，只是端口和 TLS 配置不同
        Ok(Box::new(crate::protocol::ftp::FtpProtocol::new(profile)?))
    }));

    registry
}
```

- [ ] **Step 2: 验证编译**

```bash
cargo check -p qingqi-feature-ssh
```

- [ ] **Step 3: Commit**

```bash
git add crates/qingqi-feature-ssh/src/connection.rs
git commit -m "feat(ssh): 实现 ConnectionPool + ProtocolRegistry 注册"
```

### Task 3.2: 实现 TerminalEngine

**Files:**
- Write: `crates/qingqi-feature-ssh/src/terminal.rs`

- [ ] **Step 1: 编写 terminal.rs**

```rust
// crates/qingqi-feature-ssh/src/terminal.rs
use std::sync::{Arc, Mutex};

use anyhow::Result;
use tokio::sync::mpsc;

use crate::model::TerminalKind;
use crate::protocol::{LogLevel, TerminalOutput};

/// 终端渲染帧（View 渲染用）
#[derive(Clone, Debug)]
pub struct TerminalFrame {
    pub lines: Vec<TerminalLine>,
    pub cursor_visible: bool,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub status_text: String,
    pub terminal_kind: TerminalKind,
}

#[derive(Clone, Debug)]
pub struct TerminalLine {
    pub text: String,
    pub fg_color: Option<[f32; 4]>,  // RGBA
    pub bg_color: Option<[f32; 4]>,
    pub bold: bool,
}

/// 终端输入指令
#[derive(Clone, Debug)]
pub enum TerminalInput {
    Key(String),
    Paste(String),
    Resize { cols: u16, rows: u16 },
}

/// 终端引擎 — 统一 SSH PTY 和 FTP 日志
pub struct TerminalEngine {
    kind: TerminalKind,
    lines: Mutex<Vec<TerminalLine>>,
    cursor_row: Mutex<usize>,
    cursor_col: Mutex<usize>,
    cursor_visible: Mutex<bool>,
    status_text: Mutex<String>,
    max_lines: usize,
}

impl TerminalEngine {
    pub fn new(kind: TerminalKind) -> Self {
        Self {
            kind,
            lines: Mutex::new(Vec::new()),
            cursor_row: Mutex::new(0),
            cursor_col: Mutex::new(0),
            cursor_visible: Mutex::new(true),
            status_text: Mutex::new(String::new()),
            max_lines: 5000,
        }
    }

    /// 启动输出处理循环
    /// SSH 模式：从 rx 接收 PtyOutput，解析 ANSI
    /// FTP 模式：从 rx 接收 LogLine，按级别着色
    pub fn start_processing(
        engine: Arc<Self>,
        mut rx: mpsc::UnboundedReceiver<TerminalOutput>,
    ) {
        tokio::spawn(async move {
            while let Some(output) = rx.recv().await {
                match output {
                    TerminalOutput::PtyOutput(data) => {
                        // SSH PTY 输出 — 简单按行分割（生产环境使用 alacritty_terminal 解析 ANSI）
                        let text = String::from_utf8_lossy(&data);
                        let mut lines = engine.lines.lock().unwrap_or_else(|e| e.into_inner());
                        for line in text.lines() {
                            lines.push(TerminalLine {
                                text: line.to_string(),
                                fg_color: None,
                                bg_color: None,
                                bold: false,
                            });
                        }
                        // 限制缓冲区
                        while lines.len() > engine.max_lines {
                            lines.remove(0);
                        }
                        if let Some(last) = lines.last() {
                            let mut row = engine.cursor_row.lock().unwrap_or_else(|e| e.into_inner());
                            *row = lines.len().saturating_sub(1);
                        }
                    }
                    TerminalOutput::LogLine { level, text } => {
                        let color = match level {
                            LogLevel::Sent => Some([0.0, 0.8, 1.0, 1.0]),       // 青色
                            LogLevel::Received => Some([0.55, 0.55, 0.55, 1.0]), // 灰色
                            LogLevel::Error => Some([1.0, 0.3, 0.3, 1.0]),      // 红色
                            LogLevel::Info => None,                              // 默认
                        };
                        let prefix = match level {
                            LogLevel::Sent => "> ",
                            LogLevel::Received => "< ",
                            LogLevel::Error => "! ",
                            LogLevel::Info => "  ",
                        };
                        let mut lines = engine.lines.lock().unwrap_or_else(|e| e.into_inner());
                        lines.push(TerminalLine {
                            text: format!("{prefix}{text}"),
                            fg_color: color,
                            bg_color: None,
                            bold: false,
                        });
                        while lines.len() > engine.max_lines {
                            lines.remove(0);
                        }
                    }
                }
            }
        });
    }

    /// 追加一行日志（FTP 模式外部调用）
    pub fn append_log(&self, level: LogLevel, text: &str) {
        let color = match level {
            LogLevel::Sent => Some([0.0, 0.8, 1.0, 1.0]),
            LogLevel::Received => Some([0.55, 0.55, 0.55, 1.0]),
            LogLevel::Error => Some([1.0, 0.3, 0.3, 1.0]),
            LogLevel::Info => None,
        };
        let prefix = match level {
            LogLevel::Sent => "> ",
            LogLevel::Received => "< ",
            LogLevel::Error => "! ",
            LogLevel::Info => "  ",
        };
        let mut lines = self.lines.lock().unwrap_or_else(|e| e.into_inner());
        lines.push(TerminalLine {
            text: format!("{prefix}{text}"),
            fg_color: color,
            bg_color: None,
            bold: false,
        });
        while lines.len() > self.max_lines {
            lines.remove(0);
        }
    }

    /// 生成渲染快照
    pub fn snapshot(&self) -> TerminalFrame {
        let lines = self.lines.lock().unwrap_or_else(|e| e.into_inner());
        TerminalFrame {
            lines: lines.clone(),
            cursor_visible: *self.cursor_visible.lock().unwrap_or_else(|e| e.into_inner()),
            cursor_row: *self.cursor_row.lock().unwrap_or_else(|e| e.into_inner()),
            cursor_col: *self.cursor_col.lock().unwrap_or_else(|e| e.into_inner()),
            status_text: self.status_text.lock().unwrap_or_else(|e| e.into_inner()).clone(),
            terminal_kind: self.kind.clone(),
        }
    }

    pub fn set_status(&self, text: &str) {
        let mut s = self.status_text.lock().unwrap_or_else(|e| e.into_inner());
        *s = text.to_string();
    }
}
```

- [ ] **Step 2: 验证编译**

```bash
cargo check -p qingqi-feature-ssh
```

- [ ] **Step 3: Commit**

```bash
git add crates/qingqi-feature-ssh/src/terminal.rs
git commit -m "feat(ssh): 实现 TerminalEngine（PTY + 日志双模式）"
```

### Task 3.3: 实现 TransferQueue

**Files:**
- Write: `crates/qingqi-feature-ssh/src/transfer.rs`

- [ ] **Step 1: 编写 transfer.rs**

```rust
// crates/qingqi-feature-ssh/src/transfer.rs
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use tokio::sync::mpsc;

use crate::model::{SessionId, TransferDirection, TransferId, TransferStatus, TransferTask};
use crate::protocol::TransferProgress;

const MAX_CONCURRENT: usize = 3;

/// 传输队列 — 每个 Session 独立一个实例
pub struct TransferQueue {
    session_id: SessionId,
    tasks: Mutex<Vec<TransferTask>>,
    max_concurrent: usize,
    event_tx: tokio::sync::broadcast::Sender<super::service::SshEvent>,
}

impl TransferQueue {
    pub fn new(
        session_id: SessionId,
        event_tx: tokio::sync::broadcast::Sender<super::service::SshEvent>,
    ) -> Self {
        Self {
            session_id,
            tasks: Mutex::new(Vec::new()),
            max_concurrent: MAX_CONCURRENT,
            event_tx,
        }
    }

    /// 入队一个新传输任务，返回 TransferId
    pub fn enqueue(
        &self,
        direction: TransferDirection,
        local_path: String,
        remote_path: String,
        total_bytes: u64,
    ) -> TransferId {
        let id = TransferId::new();
        let task = TransferTask {
            id,
            session_id: self.session_id,
            direction,
            status: TransferStatus::Queued,
            local_path,
            remote_path,
            transferred_bytes: 0,
            total_bytes,
            started_at: None,
            finished_at: None,
            message: String::new(),
            logs: vec![format!(
                "{} [INFO] 加入传输队列",
                Self::now_str()
            )],
        };
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        tasks.push(task);
        drop(tasks);
        self.emit_changed(id);
        id
    }

    /// 开始执行传输（由 Service 调度）
    pub fn start_transfer(
        queue: Arc<Self>,
        id: TransferId,
        mut progress_rx: mpsc::UnboundedReceiver<TransferProgress>,
        cancel_rx: tokio::sync::watch::Receiver<bool>,
    ) {
        tokio::spawn(async move {
            // 标记为 Running
            {
                let mut tasks = queue.tasks.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(t) = tasks.iter_mut().find(|t| t.id == id) {
                    t.status = TransferStatus::Running;
                    t.started_at = Some(Self::now_str());
                    t.logs.push(format!("{} [INFO] 开始传输", Self::now_str()));
                }
            }
            queue.emit_changed(id);

            let start = std::time::Instant::now();
            loop {
                tokio::select! {
                    // 检查取消
                    _ = cancel_rx.changed() => {
                        if *cancel_rx.borrow() {
                            let mut tasks = queue.tasks.lock().unwrap_or_else(|e| e.into_inner());
                            if let Some(t) = tasks.iter_mut().find(|t| t.id == id) {
                                t.status = TransferStatus::Cancelled;
                                t.logs.push(format!("{} [WARN] 已取消", Self::now_str()));
                            }
                            queue.emit_changed(id);
                            return;
                        }
                    }
                    // 进度更新
                    progress = progress_rx.recv() => {
                        match progress {
                            Some(p) => {
                                let mut tasks = queue.tasks.lock().unwrap_or_else(|e| e.into_inner());
                                if let Some(t) = tasks.iter_mut().find(|t| t.id == id) {
                                    t.transferred_bytes = p.transferred_bytes;
                                    let pct = if p.total_bytes > 0 {
                                        (p.transferred_bytes as f64 / p.total_bytes as f64 * 100.0) as u32
                                    } else { 0 };
                                    let speed = format_speed(p.speed_bytes_per_sec);
                                    t.logs.push(format!(
                                        "{} [INFO] 已传输 {} ({pct}%)，速度 {speed}",
                                        Self::now_str(),
                                        format_size(p.transferred_bytes),
                                    ));
                                }
                                queue.emit_changed(id);
                            }
                            None => {
                                // channel 关闭，传输完成
                                let elapsed = start.elapsed();
                                let mut tasks = queue.tasks.lock().unwrap_or_else(|e| e.into_inner());
                                if let Some(t) = tasks.iter_mut().find(|t| t.id == id) {
                                    if matches!(t.status, TransferStatus::Running) {
                                        t.status = TransferStatus::Completed;
                                        t.finished_at = Some(Self::now_str());
                                        t.logs.push(format!(
                                            "{} [INFO] 完成，耗时 {:.1}s",
                                            Self::now_str(),
                                            elapsed.as_secs_f64()
                                        ));
                                    }
                                }
                                queue.emit_changed(id);
                                return;
                            }
                        }
                    }
                }
            }
        });
    }

    /// 获取所有传输任务快照
    pub fn snapshot(&self) -> Vec<TransferTask> {
        self.tasks.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// 取消指定传输
    pub fn cancel(&self, id: &TransferId) {
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(t) = tasks.iter_mut().find(|t| &t.id == id) {
            if matches!(t.status, TransferStatus::Queued | TransferStatus::Running) {
                t.status = TransferStatus::Cancelled;
                t.logs.push(format!("{} [WARN] 已取消", Self::now_str()));
            }
        }
        drop(tasks);
        self.emit_changed(*id);
    }

    fn emit_changed(&self, transfer_id: TransferId) {
        use super::service::SshEvent;
        let _ = self.event_tx.send(SshEvent::TransferChanged(self.session_id, transfer_id));
    }

    fn now_str() -> String {
        chrono::Local::now().format("%H:%M:%S").to_string()
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn format_speed(bytes_per_sec: f64) -> String {
    if bytes_per_sec < 1024.0 {
        format!("{bytes_per_sec:.0} B/s")
    } else if bytes_per_sec < 1024.0 * 1024.0 {
        format!("{:.1} KB/s", bytes_per_sec / 1024.0)
    } else {
        format!("{:.1} MB/s", bytes_per_sec / (1024.0 * 1024.0))
    }
}
```

- [ ] **Step 2: 在 Cargo.toml 添加 chrono 依赖**

编辑 `crates/qingqi-feature-ssh/Cargo.toml`，在 `[dependencies]` 节添加：

```toml
chrono = "0.4"
```

- [ ] **Step 3: 验证编译**

```bash
cargo check -p qingqi-feature-ssh
```

- [ ] **Step 4: Commit**

```bash
git add crates/qingqi-feature-ssh/src/transfer.rs crates/qingqi-feature-ssh/Cargo.toml
git commit -m "feat(ssh): 实现 TransferQueue（传输队列 + 详细日志）"
```

### Task 3.4: 实现 SshService

**Files:**
- Write: `crates/qingqi-feature-ssh/src/service.rs`

- [ ] **Step 1: 编写 service.rs**

```rust
// crates/qingqi-feature-ssh/src/service.rs
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;
use tokio::sync::broadcast;

use crate::connection::{ConnectionPool, default_registry};
use crate::model::{
    Profile, ProfileDraft, RemoteEntry, SessionId, SessionSnapshot,
    SessionStatus, SessionSummary, SshSnapshot, TerminalKind, TransferId,
    TransferDirection, TransferStatus, TransferTask,
};
use crate::protocol::RemoteProtocol;
use crate::store::ProfileStore;
use crate::terminal::{TerminalEngine, TerminalFrame, TerminalInput};
use crate::transfer::TransferQueue;

// ============ 事件 ============

#[derive(Clone, Debug)]
pub enum SshEvent {
    ProfileCreated(i64),
    ProfileUpdated(i64),
    ProfileDeleted(i64),
    SessionOpened(SessionId),
    SessionConnected(SessionId),
    SessionDataChanged(SessionId),
    SessionClosed(SessionId),
    TransferChanged(SessionId, TransferId),
}

// ============ Session 内部状态 ============

struct SessionState {
    profile_id: i64,
    protocol: crate::model::ProtocolType,
    summary: SessionSummary,
    terminal: Option<Arc<TerminalEngine>>,
    entries: Vec<RemoteEntry>,
    remote_cwd: String,
    transfer_queue: Arc<TransferQueue>,
    _protocol_handle: Arc<dyn RemoteProtocol>,
}

// ============ SshService ============

pub struct SshService {
    database: Arc<qingqi_plugin::database::DatabaseService>,
    profile_store: Arc<ProfileStore>,
    cache_dir: PathBuf,
    connection_pool: Arc<ConnectionPool>,
    sessions: Mutex<HashMap<SessionId, SessionState>>,
    event_tx: broadcast::Sender<SshEvent>,
    revision: AtomicU64,
}

impl SshService {
    pub fn new(
        database: Arc<qingqi_plugin::database::DatabaseService>,
        profile_store: Arc<ProfileStore>,
        cache_dir: PathBuf,
    ) -> Self {
        let registry = default_registry();
        let (event_tx, _) = broadcast::channel(256);

        Self {
            database,
            profile_store,
            cache_dir,
            connection_pool: Arc::new(ConnectionPool::new(registry)),
            sessions: Mutex::new(HashMap::new()),
            event_tx,
            revision: AtomicU64::new(0),
        }
    }

    fn bump(&self) {
        self.revision.fetch_add(1, Ordering::SeqCst);
    }

    fn emit(&self, event: SshEvent) {
        self.bump();
        let _ = self.event_tx.send(event);
    }

    // ========== Profile CRUD ==========

    pub fn list_profiles(&self) -> Result<Vec<Profile>> {
        self.profile_store.list()
    }

    pub fn get_profile(&self, id: i64) -> Result<Option<Profile>> {
        self.profile_store.get(id)
    }

    pub fn create_profile(&self, draft: ProfileDraft) -> Result<Profile> {
        let profile = self.profile_store.create(&draft)?;
        self.emit(SshEvent::ProfileCreated(profile.id));
        Ok(profile)
    }

    pub fn update_profile(&self, id: i64, draft: ProfileDraft) -> Result<Profile> {
        let profile = self
            .profile_store
            .update(id, &draft)?
            .ok_or_else(|| anyhow::anyhow!("Profile {id} 不存在"))?;
        self.emit(SshEvent::ProfileUpdated(id));
        Ok(profile)
    }

    pub fn delete_profile(&self, id: i64) -> Result<bool> {
        let deleted = self.profile_store.delete(id)?;
        if deleted {
            self.emit(SshEvent::ProfileDeleted(id));
        }
        Ok(deleted)
    }

    // ========== Session 管理 ==========

    pub fn open_session(&self, profile_id: i64) -> Result<SessionId> {
        let profile = self
            .get_profile(profile_id)?
            .ok_or_else(|| anyhow::anyhow!("Profile {profile_id} 不存在"))?;

        let session_id = SessionId::new();
        let terminal_kind = profile.protocol.supports_terminal();

        let summary = SessionSummary {
            session_id,
            profile_id,
            title: format!("{}@{}", if matches!(profile.auth, crate::model::AuthConfig::Ftp { .. }) { "ftp" } else { "user" }, profile.host),
            endpoint: format!("{}:{}", profile.host, profile.port),
            protocol: profile.protocol.clone(),
            status: SessionStatus::Connecting,
            terminal_kind: terminal_kind.clone(),
            has_terminal: true,
            message: "连接中...".into(),
        };

        let pool = Arc::clone(&self.connection_pool);
        let event_tx = self.event_tx.clone();
        let sid = session_id;
        let p = profile.clone();

        tokio::spawn(async move {
            match pool.get_or_connect(&p).await {
                Ok(proto) => {
                    let _ = event_tx.send(SshEvent::SessionConnected(sid));
                }
                Err(e) => {
                    let _ = event_tx.send(SshEvent::SessionDataChanged(sid));
                }
            }
        });

        // 临时注册 session（后续 SessionConnected 时更新）
        let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions.insert(session_id, SessionState {
            profile_id,
            protocol: profile.protocol.clone(),
            summary: summary.clone(),
            terminal: None,
            entries: Vec::new(),
            remote_cwd: profile.paths.remote_root.clone(),
            transfer_queue: Arc::new(TransferQueue::new(session_id, self.event_tx.clone())),
            _protocol_handle: Arc::new(DummyProtocol),  // 占位，连接成功后替换
        });
        drop(sessions);

        self.emit(SshEvent::SessionOpened(session_id));
        Ok(session_id)
    }

    pub fn close_session(&self, id: &SessionId) -> Result<()> {
        let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(state) = sessions.remove(id) {
            let pool = Arc::clone(&self.connection_pool);
            let profile_id = state.profile_id;
            tokio::spawn(async move {
                pool.disconnect(profile_id).await;
            });
        }
        drop(sessions);
        self.emit(SshEvent::SessionClosed(*id));
        Ok(())
    }

    pub fn session_summaries(&self) -> Vec<SessionSummary> {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions.values().map(|s| s.summary.clone()).collect()
    }

    pub fn session_snapshot(&self, id: &SessionId) -> Option<SessionSnapshot> {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions.get(id).map(|s| SessionSnapshot {
            summary: s.summary.clone(),
            entries: s.entries.clone(),
            remote_cwd: s.remote_cwd.clone(),
        })
    }

    pub fn snapshot(&self) -> SshSnapshot {
        let profiles = self.list_profiles().unwrap_or_default();
        let sessions = self.session_summaries();
        SshSnapshot {
            profiles,
            sessions,
            revision: self.revision.load(Ordering::SeqCst),
        }
    }

    // ========== 终端 ==========

    pub fn terminal_snapshot(&self, id: &SessionId) -> Option<TerminalFrame> {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions.get(id)?.terminal.as_ref().map(|t| t.snapshot())
    }

    // ========== 文件操作 ==========

    pub fn session_entries(&self, id: &SessionId) -> Vec<RemoteEntry> {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions.get(id).map(|s| s.entries.clone()).unwrap_or_default()
    }

    pub fn session_cwd(&self, id: &SessionId) -> String {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions.get(id).map(|s| s.remote_cwd.clone()).unwrap_or_default()
    }

    // ========== 传输 ==========

    pub fn transfer_snapshots(&self, id: &SessionId) -> Vec<TransferTask> {
        let sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions
            .get(id)
            .map(|s| s.transfer_queue.snapshot())
            .unwrap_or_default()
    }

    // ========== 事件订阅 ==========

    pub fn subscribe(&self) -> broadcast::Receiver<SshEvent> {
        self.event_tx.subscribe()
    }
}

// DummyProtocol 占位（open_session 时连接尚未建立）
use async_trait::async_trait;
struct DummyProtocol;
#[async_trait]
impl RemoteProtocol for DummyProtocol {
    async fn connect(&self) -> Result<()> { Ok(()) }
    async fn disconnect(&self) {}
    fn is_connected(&self) -> bool { false }
    fn capabilities(&self) -> Vec<crate::protocol::ProtocolCapability> { vec![] }
    async fn open_terminal(&self) -> Result<tokio::sync::mpsc::UnboundedReceiver<crate::protocol::TerminalOutput>> {
        Err(anyhow::anyhow!("未连接"))
    }
    async fn send_terminal_input(&self, _: &[u8]) -> Result<()> { Err(anyhow::anyhow!("未连接")) }
    async fn list_directory(&self, _: &str) -> Result<Vec<RemoteEntry>> { Err(anyhow::anyhow!("未连接")) }
    async fn create_directory(&self, _: &str) -> Result<()> { Err(anyhow::anyhow!("未连接")) }
    async fn rename_entry(&self, _: &str, _: &str) -> Result<()> { Err(anyhow::anyhow!("未连接")) }
    async fn remove_file(&self, _: &str) -> Result<()> { Err(anyhow::anyhow!("未连接")) }
    async fn remove_directory(&self, _: &str) -> Result<()> { Err(anyhow::anyhow!("未连接")) }
    async fn upload_file(&self, _: &Path, _: &str, _: tokio::sync::mpsc::UnboundedSender<crate::protocol::TransferProgress>) -> Result<()> { Err(anyhow::anyhow!("未连接")) }
    async fn download_file(&self, _: &str, _: &Path, _: tokio::sync::mpsc::UnboundedSender<crate::protocol::TransferProgress>) -> Result<()> { Err(anyhow::anyhow!("未连接")) }
}
```

- [ ] **Step 2: 更新 lib.rs 修复编译**

```rust
// lib.rs — 由于 service.rs 已实现，build 函数现在可编译
pub fn build(
    database: Arc<DatabaseService>,
    paths: Arc<qingqi_plugin::storage::AppPaths>,
) -> Result<Box<dyn Plugin>> {
    let profile_db_path = database.path_for_key("ssh/profiles")?;
    let profile_store = Arc::new(store::ProfileStore::new(
        Arc::clone(&database),
        profile_db_path,
    ));
    profile_store.init()?;

    let service = Arc::new(service::SshService::new(
        Arc::clone(&database),
        profile_store,
        paths.cache_dir().to_path_buf(),
    ));
    Ok(Box::new(plugin::SshPlugin { service }))
}
```

- [ ] **Step 3: 验证编译**

```bash
cargo check -p qingqi-feature-ssh
```

- [ ] **Step 4: Commit**

```bash
git add crates/qingqi-feature-ssh/src/service.rs crates/qingqi-feature-ssh/src/lib.rs
git commit -m "feat(ssh): 实现 SshService 核心服务（Profile/Session/终端/传输/事件）"
```

---

## 阶段 4：视图层

### Task 4.1: 编写 manifest.rs 和 plugin.rs

**Files:**
- Write: `crates/qingqi-feature-ssh/src/manifest.rs`
- Write: `crates/qingqi-feature-ssh/src/plugin.rs`

- [ ] **Step 1: 编写 manifest.rs**

```rust
// crates/qingqi-feature-ssh/src/manifest.rs
use qingqi_plugin::{
    icon::IconRef,
    plugin::Manifest,
    plugin_spec::{
        PluginAccent, PluginCategory, PluginStats, PluginStatus, PluginVisualSpec,
        PluginWindowMode, WindowSpec,
    },
};

pub const PLUGIN_ID: &str = "ssh";

pub fn manifest() -> Manifest {
    Manifest {
        id: PLUGIN_ID.into(),
        name: "远程管理".into(),
        description: "SSH/SFTP/FTP/FTPS 远程连接管理。多标签页终端与文件传输。".into(),
        keywords: ["ssh", "sftp", "ftp", "ftps", "远程", "终端", "文件", "传输", "服务器"]
            .into_iter()
            .map(Into::into)
            .collect(),
        icon: IconRef::asset("icons/folder-network.svg"),
        prefixes: vec!["ssh".into(), "sftp".into(), "ftp".into()],
        mode: PluginWindowMode::Window,
        window: WindowSpec::ratio_blurred(0.86, 0.82),
        category: PluginCategory::Tool,
        status: PluginStatus::Ready,
        background: false,
        dynamic_commands: false,
        visual: Some(PluginVisualSpec {
            icon: IconRef::asset("icons/folder-network.svg"),
            accent: PluginAccent::Cyan,
            category: PluginCategory::Tool,
            status: PluginStatus::Ready,
            mode: PluginWindowMode::Window,
            window: WindowSpec::ratio_blurred(0.86, 0.82),
        }),
        stats: Some(PluginStats {
            primary: "多会话标签页".into(),
            secondary: "终端 + 文件传输".into(),
            tertiary: "SSH/SFTP/FTP/FTPS".into(),
        }),
        command_hint: Some("SSH 终端、SFTP/FTP 文件浏览、上传下载".into()),
        command_prefixes: ["ssh", "sftp", "ftp"].into_iter().map(Into::into).collect(),
    }
}
```

- [ ] **Step 2: 编写 plugin.rs（先用最小实现保证编译）**

```rust
// crates/qingqi-feature-ssh/src/plugin.rs
use std::sync::Arc;

use anyhow::Result;
use gpui::{App, AppContext, Entity, Window};

use crate::manifest;
use crate::service::SshService;
use crate::view::SshView;
use qingqi_plugin::{
    command::Command,
    plugin::{Plugin, PluginCx, PluginId, PluginView, WindowView},
};

pub struct SshPlugin {
    pub service: Arc<SshService>,
}

impl Plugin for SshPlugin {
    fn manifest(&self) -> qingqi_plugin::plugin::Manifest {
        manifest::manifest()
    }

    fn commands(&self, _query: &str) -> Vec<Command> {
        let m = self.manifest();
        vec![Command::plugin_open(
            m.id.as_ref(),
            m.name.as_ref(),
            m.description.as_ref(),
            m.keywords.iter().map(|s| s.as_ref()),
            m.prefixes.iter().map(|s| s.as_ref()),
            m.icon.as_str(),
        )]
    }

    fn open(&mut self, cx: &mut PluginCx<'_>) -> Result<PluginView> {
        let view = cx.app.new(|cx| {
            SshView::new(Arc::clone(&self.service), cx)
        });

        Ok(PluginView::Window(Box::new(SshWindowView { view })))
    }

    fn start_background(&mut self, _events: qingqi_plugin::events::AppEventBus, _cx: &mut App) {}
}

struct SshWindowView {
    view: Entity<SshView>,
}

impl WindowView for SshWindowView {
    fn plugin_id(&self) -> PluginId {
        manifest::PLUGIN_ID.into()
    }

    fn title(&self) -> Arc<str> {
        "远程管理".into()
    }

    fn render(&mut self, _window: &mut Window, _cx: &mut App) -> gpui::AnyElement {
        self.view.clone().into_any_element()
    }
}
```

- [ ] **Step 3: 编写最小 view/mod.rs 桩代码**

```rust
// crates/qingqi-feature-ssh/src/view/mod.rs
use std::sync::Arc;

use gpui::*;

use crate::service::SshService;

pub struct SshView {
    service: Arc<SshService>,
    focus_handle: FocusHandle,
}

impl SshView {
    pub fn new(service: Arc<SshService>, cx: &mut Context<Self>) -> Self {
        Self {
            service,
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Render for SshView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(qingqi_ui::ui::bg_surface())
            .child("SSH 远程管理")
    }
}

impl Focusable for SshView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
```

- [ ] **Step 4: 验证编译**

```bash
cargo check -p qingqi-feature-ssh
```

Expected: SUCCESS（全 crate 可编译）

- [ ] **Step 5: Commit**

```bash
git add crates/qingqi-feature-ssh/src/manifest.rs crates/qingqi-feature-ssh/src/plugin.rs crates/qingqi-feature-ssh/src/view/mod.rs
git commit -m "feat(ssh): 实现 manifest、plugin 装配与最小 SshView"
```

### Task 4.2: 实现 SshView 完整结构与 ViewModel

**Files:**
- Rewrite: `crates/qingqi-feature-ssh/src/view/mod.rs`

- [ ] **Step 1: 编写完整 SshView + SshViewModel**

```rust
// crates/qingqi-feature-ssh/src/view/mod.rs
use std::sync::Arc;

use gpui::*;

use crate::model::{SessionId, SessionStatus, TerminalKind};
use crate::service::{SshEvent, SshService};
use qingqi_ui::ui;

// ========== ViewModel (render-ready 纯数据) ==========

#[derive(Clone, Debug)]
struct ProfileItem {
    id: i64,
    name: String,
    endpoint: String,
    protocol_badge: String,
    is_connected: bool,
    is_selected: bool,
}

#[derive(Clone, Debug)]
struct SessionTabItem {
    session_id: SessionId,
    title: String,
    is_selected: bool,
    status_color: Hsla,
    terminal_kind: TerminalKind,
}

#[derive(Clone, Debug)]
struct FileTreeViewModel {
    current_path: String,
    parent_path: Option<String>,
    entries: Vec<FileEntryRow>,
}

#[derive(Clone, Debug)]
struct FileEntryRow {
    path: String,
    name: String,
    icon_name: String,
    size_text: String,
    is_dir: bool,
    is_selected: bool,
}

#[derive(Clone, Debug)]
struct TerminalViewModel {
    status: String,
    lines: Vec<crate::terminal::TerminalLine>,
    cursor_visible: bool,
    terminal_kind: TerminalKind,
}

#[derive(Clone, Debug)]
struct TransferPanelViewModel {
    active_count: usize,
    completed_count: usize,
    failed_count: usize,
    rows: Vec<TransferRowViewModel>,
}

#[derive(Clone, Debug)]
struct TransferRowViewModel {
    id: crate::model::TransferId,
    direction_icon: &'static str,
    file_name: String,
    progress_percent: u8,
    status_text: String,
    status_color: Hsla,
    speed_text: String,
    logs: Vec<String>,
    expanded: bool,
}

#[derive(Clone, Debug)]
struct SshViewModel {
    profiles: Vec<ProfileItem>,
    sessions: Vec<SessionTabItem>,
    file_tree: FileTreeViewModel,
    terminal: TerminalViewModel,
    transfers: TransferPanelViewModel,
}

// ========== SshView ==========

pub struct SshView {
    service: Arc<SshService>,
    focus_handle: FocusHandle,

    // ViewModel
    vm: SshViewModel,

    // UI 状态
    selected_profile_id: Option<i64>,
    selected_session_id: Option<SessionId>,
    transfer_panel_expanded: bool,

    // 后台事件循环
    event_task: Option<Task<()>>,
    last_revision: u64,
    generation: u64,
}

impl SshView {
    pub fn new(service: Arc<SshService>, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            service: Arc::clone(&service),
            focus_handle: cx.focus_handle(),
            vm: SshViewModel::default(),
            selected_profile_id: None,
            selected_session_id: None,
            transfer_panel_expanded: false,
            event_task: None,
            last_revision: 0,
            generation: 0,
        };
        this.rebuild_view_model();
        this.start_event_loop(cx);
        this
    }

    fn start_event_loop(&mut self, cx: &mut Context<Self>) {
        let service = Arc::clone(&self.service);
        let mut rx = service.subscribe();

        self.generation = self.generation.wrapping_add(1);
        let gen = self.generation;

        self.event_task = Some(cx.spawn(async move |view, acx| {
            while let Ok(event) = rx.recv().await {
                let _ = view.update(acx, |view, cx| {
                    if view.generation != gen {
                        return;
                    }
                    view.on_service_event(&event, cx);
                });
            }
        }));
    }

    fn on_service_event(&mut self, _event: &SshEvent, cx: &mut Context<Self>) {
        self.rebuild_view_model();
        cx.notify();
    }

    fn rebuild_view_model(&mut self) {
        let snap = self.service.snapshot();
        if snap.revision == self.last_revision {
            return;
        }
        self.last_revision = snap.revision;

        self.vm = SshViewModel {
            profiles: Self::build_profiles(&snap.profiles, self.selected_profile_id),
            sessions: Self::build_sessions(&snap.sessions, self.selected_session_id),
            file_tree: Self::build_file_tree(self.selected_session_id.as_ref(), &self.service),
            terminal: Self::build_terminal(self.selected_session_id.as_ref(), &self.service),
            transfers: Self::build_transfers(self.selected_session_id.as_ref(), &self.service),
        };
    }

    fn build_profiles(
        profiles: &[crate::model::Profile],
        selected_id: Option<i64>,
    ) -> Vec<ProfileItem> {
        profiles
            .iter()
            .map(|p| ProfileItem {
                id: p.id,
                name: p.name.clone(),
                endpoint: format!("{}:{}", p.host, p.port),
                protocol_badge: p.protocol.display().to_string(),
                is_connected: false, // TODO: 从 session 状态判断
                is_selected: selected_id == Some(p.id),
            })
            .collect()
    }

    fn build_sessions(
        sessions: &[crate::model::SessionSummary],
        selected_id: Option<SessionId>,
    ) -> Vec<SessionTabItem> {
        sessions
            .iter()
            .map(|s| SessionTabItem {
                session_id: s.session_id,
                title: s.title.clone(),
                is_selected: selected_id == Some(s.session_id),
                status_color: match s.status {
                    SessionStatus::Connecting => hsla(0.14, 0.8, 0.5, 1.0),  // 黄色
                    SessionStatus::Connected => hsla(0.4, 0.8, 0.5, 1.0),   // 绿色
                    SessionStatus::Failed => hsla(0.0, 0.8, 0.5, 1.0),      // 红色
                },
                terminal_kind: s.terminal_kind.clone(),
            })
            .collect()
    }

    fn build_file_tree(
        session_id: Option<&SessionId>,
        service: &SshService,
    ) -> FileTreeViewModel {
        let (current_path, entries) = session_id
            .map(|id| {
                let cwd = service.session_cwd(id);
                let ents = service.session_entries(id);
                (cwd, ents)
            })
            .unwrap_or_default();

        let parent = if current_path == "/" || current_path.is_empty() {
            None
        } else {
            let p = std::path::Path::new(&current_path);
            p.parent().map(|p| p.to_string_lossy().to_string())
        };

        FileTreeViewModel {
            current_path,
            parent_path: parent,
            entries: entries
                .into_iter()
                .map(|e| FileEntryRow {
                    path: e.path.clone(),
                    name: if e.is_dir {
                        format!("{}/", e.name)
                    } else {
                        e.name.clone()
                    },
                    icon_name: if e.is_dir {
                        "folder".into()
                    } else {
                        "file".into()
                    },
                    size_text: if e.is_dir {
                        String::new()
                    } else {
                        format_size(e.size)
                    },
                    is_dir: e.is_dir,
                    is_selected: false,
                })
                .collect(),
        }
    }

    fn build_terminal(
        session_id: Option<&SessionId>,
        service: &SshService,
    ) -> TerminalViewModel {
        session_id
            .and_then(|id| service.terminal_snapshot(id))
            .map(|frame| TerminalViewModel {
                status: frame.status_text,
                lines: frame.lines,
                cursor_visible: frame.cursor_visible,
                terminal_kind: frame.terminal_kind,
            })
            .unwrap_or(TerminalViewModel {
                status: "未连接".into(),
                lines: Vec::new(),
                cursor_visible: false,
                terminal_kind: TerminalKind::Shell,
            })
    }

    fn build_transfers(
        session_id: Option<&SessionId>,
        service: &SshService,
    ) -> TransferPanelViewModel {
        let tasks = session_id
            .map(|id| service.transfer_snapshots(id))
            .unwrap_or_default();

        let (active, completed, failed) = tasks.iter().fold((0, 0, 0), |(a, c, f), t| {
            match t.status {
                crate::model::TransferStatus::Queued | crate::model::TransferStatus::Running => {
                    (a + 1, c, f)
                }
                crate::model::TransferStatus::Completed => (a, c + 1, f),
                crate::model::TransferStatus::Failed => (a, c, f + 1),
                _ => (a, c, f),
            }
        });

        TransferPanelViewModel {
            active_count: active,
            completed_count: completed,
            failed_count: failed,
            rows: tasks
                .into_iter()
                .map(|t| TransferRowViewModel {
                    id: t.id,
                    direction_icon: match t.direction {
                        crate::model::TransferDirection::Upload => "\u{2191}",
                        crate::model::TransferDirection::Download => "\u{2193}",
                    },
                    file_name: std::path::Path::new(&t.remote_path)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| t.remote_path.clone()),
                    progress_percent: if t.total_bytes > 0 {
                        ((t.transferred_bytes as f64 / t.total_bytes as f64) * 100.0) as u8
                    } else {
                        0
                    },
                    status_text: match t.status {
                        crate::model::TransferStatus::Queued => "排队中".into(),
                        crate::model::TransferStatus::Running => "传输中".into(),
                        crate::model::TransferStatus::Completed => "完成".into(),
                        crate::model::TransferStatus::Failed => "失败".into(),
                        crate::model::TransferStatus::Cancelled => "已取消".into(),
                    },
                    status_color: match t.status {
                        crate::model::TransferStatus::Queued => hsla(0.0, 0.0, 0.5, 1.0),
                        crate::model::TransferStatus::Running => hsla(0.55, 0.8, 0.5, 1.0),
                        crate::model::TransferStatus::Completed => hsla(0.4, 0.8, 0.5, 1.0),
                        crate::model::TransferStatus::Failed => hsla(0.0, 0.8, 0.5, 1.0),
                        crate::model::TransferStatus::Cancelled => hsla(0.12, 0.7, 0.5, 1.0),
                    },
                    speed_text: String::new(),
                    logs: t.logs,
                    expanded: false,
                })
                .collect(),
        }
    }
}

impl SshViewModel {
    fn default() -> Self {
        Self {
            profiles: Vec::new(),
            sessions: Vec::new(),
            file_tree: FileTreeViewModel {
                current_path: String::new(),
                parent_path: None,
                entries: Vec::new(),
            },
            terminal: TerminalViewModel {
                status: "未连接".into(),
                lines: Vec::new(),
                cursor_visible: false,
                terminal_kind: TerminalKind::Shell,
            },
            transfers: TransferPanelViewModel {
                active_count: 0,
                completed_count: 0,
                failed_count: 0,
                rows: Vec::new(),
            },
        }
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

// ========== Render ==========

impl Render for SshView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // 分离式顶栏：顶层水平 flex
        div()
            .size_full()
            .bg(ui::bg_base())
            .flex()
            .child(
                // 左侧列
                div()
                    .w(px(280.0))
                    .h_full()
                    .flex()
                    .flex_col()
                    .bg(ui::bg_surface())
                    .border_r_1()
                    .border_color(ui::border_light())
                    .child(render_sidebar_top(&self.vm.profiles))
                    .child(render_profile_list(&self.vm.profiles, self.selected_profile_id))
                    .child(render_sidebar_bottom()),
            )
            .child(
                // 右侧列
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(render_session_tabs(&self.vm.sessions))
                    .child(
                        // 内容区：文件树 + 终端 + 传输面板
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .min_h(px(0.0))
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .min_h(px(0.0))
                                    .child(render_file_tree(&self.vm.file_tree))
                                    .child(render_terminal(&self.vm.terminal)),
                            )
                            .child(render_transfer_panel(
                                &self.vm.transfers,
                                self.transfer_panel_expanded,
                            )),
                    ),
            )
    }
}

impl Focusable for SshView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

// ========== 子组件渲染桩代码（后续 Task 填充） ==========

fn render_sidebar_top(profiles: &[ProfileItem]) -> impl IntoElement {
    div()
        .h(px(52.0))
        .flex()
        .items_center()
        .px_3()
        .border_b_1()
        .border_color(ui::border_light())
        .child(mac_traffic_lights())
        .child(div().ml_2().text_size(px(15.0)).font_weight(FontWeight::SEMIBOLD).child("远程管理"))
        .child(div().flex_1())
        .child(div().child("+"))
}

fn mac_traffic_lights() -> impl IntoElement {
    div().flex().gap(px(8.0)).px(px(4.0))
        .child(div().size(px(12.0)).rounded_full().bg(rgb(0xED6A5E)))
        .child(div().size(px(12.0)).rounded_full().bg(rgb(0xF5BF4F)))
        .child(div().size(px(12.0)).rounded_full().bg(rgb(0x61C554)))
}

fn render_profile_list(
    profiles: &[ProfileItem],
    selected_id: Option<i64>,
) -> impl IntoElement {
    div().flex_1().overflow_y_scroll().p_2().children(
        profiles.iter().map(|p| render_profile_card(p, selected_id == Some(p.id)))
    )
}

fn render_profile_card(profile: &ProfileItem, is_selected: bool) -> impl IntoElement {
    div()
        .p_2()
        .mb_1()
        .rounded_md()
        .bg(if is_selected {
            hsla(0.55, 0.3, 0.5, 0.15)
        } else {
            hsla(0.0, 0.0, 0.0, 0.0)
        })
        .border_l_3()
        .border_color(if profile.is_connected {
            hsla(0.4, 0.8, 0.5, 1.0)
        } else {
            hsla(0.0, 0.0, 0.0, 0.0)
        })
        .child(
            div().flex().flex_col().gap(px(2.0))
                .child(div().text_size(px(13.0)).font_weight(FontWeight::MEDIUM).child(profile.name.clone()))
                .child(div().text_size(px(11.0)).text_color(ui::text_secondary()).fontFamily(".AppleSystemUIFontMonospaced").child(profile.endpoint.clone()))
        )
}

fn render_sidebar_bottom() -> impl IntoElement {
    div()
        .h(px(48.0))
        .flex()
        .items_center()
        .justify_center()
        .border_t_1()
        .border_color(ui::border_light())
        .child("设置")
}

fn render_session_tabs(sessions: &[SessionTabItem]) -> impl IntoElement {
    div()
        .h(px(44.0))
        .flex()
        .items_center()
        .px_2()
        .bg(ui::bg_surface())
        .border_b_1()
        .border_color(ui::border_light())
        .children(
            sessions.iter().map(|s| {
                div()
                    .px_3()
                    .py_1()
                    .mr_1()
                    .rounded_t_md()
                    .bg(if s.is_selected {
                        ui::bg_base()
                    } else {
                        hsla(0.0, 0.0, 0.0, 0.0)
                    })
                    .border_b_2()
                    .border_color(if s.is_selected {
                        s.status_color
                    } else {
                        hsla(0.0, 0.0, 0.0, 0.0)
                    })
                    .child(
                        div().flex().items_center().gap(px(6.0))
                            .child(div().size(px(8.0)).rounded_full().bg(s.status_color))
                            .child(div().text_size(px(12.0)).child(s.title.clone()))
                    )
            })
        )
        .child(div().ml_2().child("+"))
}

fn render_file_tree(tree: &FileTreeViewModel) -> impl IntoElement {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .border_r_1()
        .border_color(ui::border_light())
        .child(
            // 工具栏
            div().h(px(36.0)).flex().items_center().px_2().border_b_1()
                .border_color(ui::border_light())
                .child(div().text_size(px(12.0)).text_color(ui::text_secondary()).child(tree.current_path.clone()))
        )
        .child(
            // 文件列表
            div().flex_1().overflow_y_scroll().children(
                tree.entries.iter().map(|e| {
                    div().h(px(28.0)).flex().items_center().px_2().text_size(px(12.0))
                        .bg(if e.is_selected { hsla(0.55, 0.3, 0.5, 0.15) } else { hsla(0.0, 0.0, 0.0, 0.0) })
                        .child(e.name.clone())
                })
            )
        )
}

fn render_terminal(term: &TerminalViewModel) -> impl IntoElement {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .bg(ui::bg_base())
        .child(
            // 状态栏
            div().h(px(28.0)).flex().items_center().px_2().border_b_1()
                .border_color(ui::border_light())
                .child(div().text_size(px(11.0)).text_color(ui::text_secondary()).child(term.status.clone()))
        )
        .child(
            // 终端内容
            div().flex_1().overflow_y_scroll().p_2().fontFamily(".AppleSystemUIFontMonospaced")
                .children(term.lines.iter().map(|line| {
                    let mut el = div().text_size(px(12.0)).child(line.text.clone());
                    if let Some(color) = line.fg_color {
                        el = el.text_color(hsla(color[0], color[1], color[2], color[3]));
                    }
                    el
                }))
        )
}

fn render_transfer_panel(
    transfers: &TransferPanelViewModel,
    expanded: bool,
) -> impl IntoElement {
    div()
        .w_full()
        .border_t_1()
        .border_color(ui::border_light())
        .bg(ui::bg_surface())
        .child(
            // 控制栏
            div().h(px(36.0)).flex().items_center().px_3().justify_between()
                .child(div().text_size(px(11.0)).text_color(ui::text_secondary())
                    .child(format!(
                        "传输记录 ({} 进行中, {} 已完成, {} 失败)",
                        transfers.active_count,
                        transfers.completed_count,
                        transfers.failed_count,
                    )))
                .child(if expanded { "收起 ▲" } else { "展开 ▼" })
        )
        .when(expanded, |root| {
            root.child(
                div().h(px(200.0)).overflow_y_scroll().children(
                    transfers.rows.iter().map(|row| {
                        div().h(px(32.0)).flex().items_center().px_3().text_size(px(12.0))
                            .child(div().mr_2().child(row.direction_icon))
                            .child(div().flex_1().child(row.file_name.clone()))
                            .child(div().mr_2().text_color(row.status_color).child(row.status_text.clone()))
                    })
                )
            )
        })
}
```

- [ ] **Step 2: 验证编译**

```bash
cargo check -p qingqi-feature-ssh
```

Expected: SUCCESS

- [ ] **Step 3: Commit**

```bash
git add crates/qingqi-feature-ssh/src/view/mod.rs
git commit -m "feat(ssh): 实现 SshView 完整结构、ViewModel、分离式顶栏布局与子组件渲染"
```

---

由于计划内容较大，剩余 Task（4.3-4.7 视图子组件细化、阶段 5 集成、阶段 6 协议实现填充）将在 Part 2 延续。

当前进度总结：
- ✅ 阶段 0：项目骨架（Cargo.toml、源文件）
- ✅ 阶段 1：模型与存储层（model.rs、store.rs + 测试）
- ✅ 阶段 2：协议抽象（RemoteProtocol trait、工厂注册表）
- ✅ 阶段 3：服务层（ConnectionPool、TerminalEngine、TransferQueue、SshService）
- 🚧 阶段 4：视图层（SshView 主结构已完成，子组件待细化）
- ⬜ 阶段 5：集成与测试
- ⬜ 阶段 6：协议实现填充
```

---

### Task 4.3-4.7: 视图子组件细化 → 阶段 5: 集成 → 阶段 6: 协议实现

**后续步骤摘要：**

**Task 4.3: sidebar.rs 细化** — 从 view/mod.rs 中提取 `render_sidebar_top`、`render_profile_list`、`render_profile_card`、`mac_traffic_lights`、`render_sidebar_bottom` 到 `view/sidebar.rs`，添加右键菜单事件、双击连接处理、uniform_list 虚拟化

**Task 4.4: session_tabs.rs 细化** — 提取 `render_session_tabs`，添加 Tab 关闭按钮、选中下划线动画、快速新建按钮事件

**Task 4.5: file_tree.rs 细化** — 提取 `render_file_tree`，添加面包屑导航、上传/刷新/新建文件夹操作按钮、uniform_list 虚拟化、拖放支持、右键菜单

**Task 4.6: terminal_pane.rs 细化** — 提取 `render_terminal`，添加自动聚焦、键盘输入处理、光标闪烁、滚动历史

**Task 4.7: transfer_panel.rs + settings_dialog.rs 细化** — 传输面板展开/收起动画、详细日志展开、进度条渲染；设置弹窗 Overlay 布局、表单验证、协议切换联动认证方式

**阶段 5: 集成** — 在 `crates/qingqi/Cargo.toml` 添加依赖，在 `registry.rs` 注册插件，验证 `cargo build`

**阶段 6: 协议实现填充** — `protocol/ssh.rs` 实现 russh 连接/PTY/SFTP；`protocol/ftp.rs` 实现 suppaftp 连接/文件操作/日志终端
```

