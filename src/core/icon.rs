use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// 图标引用：当前形态是指向 `assets/` 下资源的相对路径（如 `"icons/json.svg"`）。
/// owned + serde，便于将来作为第三方插件的线格式；clone 便宜（内部 `Arc<str>`）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IconRef(Arc<str>);

impl IconRef {
    /// 由资源相对路径构造，如 `IconRef::asset("icons/json.svg")`。
    pub fn asset(path: impl Into<Arc<str>>) -> Self {
        Self(path.into())
    }

    /// 取底层路径字符串，用于传给渲染层（`ui::icon_element` 收 `&str`）。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
