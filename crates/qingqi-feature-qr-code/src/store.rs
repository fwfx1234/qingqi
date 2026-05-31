use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::FormatItem, macros::format_description};

const MAX_HISTORY_ITEMS: usize = 200;
static TIMESTAMP_FORMAT: &[FormatItem<'static>] =
    format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QrHistoryKind {
    Save,
    Copy,
    Scan,
}

impl QrHistoryKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Save => "保存",
            Self::Copy => "复制",
            Self::Scan => "扫描",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QrHistoryRecord {
    pub id: String,
    pub kind: QrHistoryKind,
    pub content: String,
    pub source: String,
    pub created_at: String,
}

#[derive(Clone)]
pub struct QrHistoryStore {
    path: PathBuf,
}

impl QrHistoryStore {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("无法创建二维码历史目录 {}", parent.display()))?;
        }
        Ok(Self { path })
    }

    pub fn list(&self, query: &str) -> Result<Vec<QrHistoryRecord>> {
        let query = query.trim().to_lowercase();
        let records = self.read_records()?;
        if query.is_empty() {
            return Ok(records);
        }

        Ok(records
            .into_iter()
            .filter(|record| {
                record.content.to_lowercase().contains(&query)
                    || record.kind.label().to_lowercase().contains(&query)
                    || record.source.to_lowercase().contains(&query)
            })
            .collect())
    }

    pub fn push(
        &self,
        kind: QrHistoryKind,
        content: &str,
        source: impl Into<String>,
    ) -> Result<QrHistoryRecord> {
        let mut records = self.read_records()?;
        let record = QrHistoryRecord {
            id: record_id(kind, content),
            kind,
            content: content.to_string(),
            source: source.into(),
            created_at: now_label(),
        };
        records.insert(0, record.clone());
        records.truncate(MAX_HISTORY_ITEMS);
        self.write_records(&records)?;
        Ok(record)
    }

    pub fn clear(&self) -> Result<()> {
        self.write_records(&[])
    }

    pub fn remove(&self, id: &str) -> Result<bool> {
        let mut records = self.read_records()?;
        let before = records.len();
        records.retain(|record| record.id != id);
        let changed = records.len() != before;
        if changed {
            self.write_records(&records)?;
        }
        Ok(changed)
    }

    pub fn export(&self, target: &Path) -> Result<PathBuf> {
        let records = self.read_records()?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("无法创建导出目录 {}", parent.display()))?;
        }

        let mut lines = Vec::new();
        for record in &records {
            lines.push(format!("[{}] {}", record.kind.label(), record.created_at));
            lines.push(record.content.clone());
            if !record.source.is_empty() {
                lines.push(format!("来源: {}", record.source));
            }
            lines.push(String::new());
        }

        fs::write(target, lines.join("\n"))
            .with_context(|| format!("无法写入二维码历史导出文件 {}", target.display()))?;
        Ok(target.to_path_buf())
    }

    fn read_records(&self) -> Result<Vec<QrHistoryRecord>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let raw = fs::read_to_string(&self.path)
            .with_context(|| format!("无法读取二维码历史文件 {}", self.path.display()))?;
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }
        serde_json::from_str(trimmed)
            .with_context(|| format!("二维码历史文件已损坏 {}", self.path.display()))
    }

    fn write_records(&self, records: &[QrHistoryRecord]) -> Result<()> {
        let json = serde_json::to_string_pretty(records).context("无法编码二维码历史记录")?;
        fs::write(&self.path, json)
            .with_context(|| format!("无法写入二维码历史文件 {}", self.path.display()))
    }
}

fn record_id(kind: QrHistoryKind, content: &str) -> String {
    let mut hasher = DefaultHasher::new();
    kind.hash(&mut hasher);
    content.hash(&mut hasher);
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
        .hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn now_label() -> String {
    OffsetDateTime::now_local()
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .format(TIMESTAMP_FORMAT)
        .unwrap_or_else(|_| String::from("1970-01-01 00:00:00"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let dir = std::env::temp_dir().join(format!("qingqi-qr-store-{nanos}"));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join(name);
        let _ = fs::remove_file(&path);
        path
    }

    #[test]
    fn push_list_remove_and_clear_history() {
        let path = temp_file("history.json");
        let store = QrHistoryStore::open(&path).expect("store should open");

        let first = store
            .push(QrHistoryKind::Save, "https://openai.com", "/tmp/a.png")
            .expect("first record should be saved");
        let second = store
            .push(QrHistoryKind::Copy, "hello", "")
            .expect("second record should be saved");

        let all = store.list("").expect("records should load");
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].id, second.id);
        assert_eq!(all[1].id, first.id);

        let filtered = store.list("保存").expect("filter should work");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, first.id);

        assert!(store.remove(&first.id).expect("remove should succeed"));
        assert_eq!(store.list("").expect("records should load").len(), 1);

        store.clear().expect("clear should succeed");
        assert!(store.list("").expect("records should load").is_empty());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn exports_history_text() {
        let path = temp_file("history.json");
        let export = temp_file("history.txt");
        let store = QrHistoryStore::open(&path).expect("store should open");
        store
            .push(QrHistoryKind::Copy, "demo", "")
            .expect("record should be saved");

        let target = store.export(&export).expect("export should succeed");
        let raw = fs::read_to_string(target).expect("export file should exist");
        assert!(raw.contains("demo"));
        assert!(raw.contains("复制"));

        let _ = fs::remove_file(path);
        let _ = fs::remove_file(export);
    }
}
