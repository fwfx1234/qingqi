//! 脚本管理服务
//!
//! 对 data_source 中 scripts 表的 CRUD 封装。
//! 支持列表、获取、保存、删除，以及 JSON 导入/导出。

use crate::data_source::ApiDebuggerDataSource;
use crate::model::{Script, ScriptCategory};
use anyhow::Result;
use uuid::Uuid;

/// 脚本管理服务
pub struct ScriptManager {
    store: ApiDebuggerDataSource,
}

impl ScriptManager {
    pub fn new(store: ApiDebuggerDataSource) -> Self {
        Self { store }
    }

    /// 列出所有脚本（可选按分类过滤）
    pub fn list(&self, category: Option<ScriptCategory>) -> Result<Vec<Script>> {
        self.store.list_scripts(category)
    }

    /// 获取单个脚本
    pub fn get(&self, id: &str) -> Result<Option<Script>> {
        self.store.get_script(id)
    }

    /// 保存脚本（新建或更新）
    pub fn save(&self, script: &Script) -> Result<()> {
        self.store.save_script(script)
    }

    /// 创建新脚本
    pub fn create(&self, name: &str, category: ScriptCategory, content: &str) -> Result<Script> {
        let id = Uuid::new_v4().to_string();
        let now = now_iso();
        let script = Script {
            id,
            name: name.to_string(),
            category,
            content: content.to_string(),
            sort_order: 0,
            created_at: now.clone(),
            updated_at: now,
        };
        self.store.save_script(&script)?;
        Ok(script)
    }

    /// 更新脚本
    pub fn update(
        &self,
        id: &str,
        name: &str,
        category: ScriptCategory,
        content: &str,
    ) -> Result<bool> {
        let existing = self.store.get_script(id)?;
        match existing {
            Some(mut script) => {
                script.name = name.to_string();
                script.category = category;
                script.content = content.to_string();
                script.updated_at = now_iso();
                self.store.save_script(&script)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// 删除脚本
    pub fn delete(&self, id: &str) -> Result<bool> {
        self.store.delete_script(id)
    }

    /// 导出所有脚本为 JSON 字符串
    pub fn export_json(&self) -> Result<String> {
        let scripts = self.list(None)?;
        serde_json::to_string_pretty(&scripts).map_err(Into::into)
    }

    /// 从 JSON 字符串导入脚本（追加，不覆盖已存在的同名脚本）
    pub fn import_json(&self, json: &str) -> Result<usize> {
        let scripts: Vec<Script> = serde_json::from_str(json)?;
        let mut count = 0;
        for script in &scripts {
            // 跳过已存在的
            if self.store.get_script(&script.id)?.is_some() {
                continue;
            }
            self.store.save_script(script)?;
            count += 1;
        }
        Ok(count)
    }
}

fn now_iso() -> String {
    use time::OffsetDateTime;
    OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| String::from("2025-01-01T00:00:00Z"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_script_export_import_json_roundtrip() {
        let scripts = vec![
            Script {
                id: "s1".into(),
                name: "脚本A".into(),
                category: ScriptCategory::PreRequest,
                content: "header X: 1".into(),
                sort_order: 0,
                created_at: "now".into(),
                updated_at: "now".into(),
            },
            Script {
                id: "s2".into(),
                name: "脚本B".into(),
                category: ScriptCategory::PostRequest,
                content: "status == 200".into(),
                sort_order: 1,
                created_at: "now".into(),
                updated_at: "now".into(),
            },
        ];

        let json = serde_json::to_string_pretty(&scripts).unwrap();
        assert!(json.contains("脚本A"));
        assert!(json.contains("脚本B"));

        let parsed: Vec<Script> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].name, "脚本A");
    }

    #[test]
    fn test_script_category_roundtrip() {
        for (cat, expected) in [
            (ScriptCategory::PreRequest, "pre"),
            (ScriptCategory::PostRequest, "post"),
            (ScriptCategory::Common, "common"),
        ] {
            assert_eq!(cat.as_str(), expected);
            assert_eq!(ScriptCategory::from_db(expected), cat);
        }
    }
}
