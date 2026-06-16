# API 调试器重构实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use subagent-driven-development（推荐）或 executing-plans 逐个任务执行。步骤用 `- [ ]` 跟踪。

**目标：** 按 `docs/api-debugger-redesign.md` 重构 API 调试器，拆分巨石 view.rs、引入 ViewModel、虚拟化接口树、整理 service.rs 契约。

**架构概要：**
- 将 UI 枚举（EditorTab/ResponseTab/EnvDetailTab）从 service.rs 移到 view_model.rs
- ApiService 按 §4.2 整理，所有写操作带 `revision += 1`
- ApiViewModel 是 render-ready 数据，render 只读 vm，不 IO/不锁/不 panic
- 接口树扁平化为 Vec<TreeRowVm>，用 uniform_list 虚拟化渲染
- 弹窗用统一 `overlay: Option<Overlay>` 状态机

**技术栈：** Rust 2024, GPUI, gpui-component (TabBar/Tab/Button/IconName), reqwest, SQLite

---

## 文件变更清单

| 操作 | 文件 | 说明 |
|------|------|------|
| 创建 | `src/view_model.rs` | ApiViewModel + UI 枚举 + *Vm 类型 |
| 重写 | `src/service.rs` | 按 §4 契约整理，移出 UI 枚举 |
| 删除 | `src/view.rs` | 整个文件删除，被 view/ 目录取代 |
| 创建 | `src/view/mod.rs` | ApiDebuggerView 主结构 + render 编排 |
| 创建 | `src/view/sidebar.rs` | 左栏（标题行 / 接口树 / 设计入口） |
| 创建 | `src/view/workspace.rs` | 右栏（tab 栏 / 请求行 / 编辑 tab / 响应区） |
| 创建 | `src/view/overlay.rs` | 弹窗（设计 / 重命名 / 上下文菜单 / 导入） |
| 修改 | `src/plugin.rs` | 更新 view 路径引用 |
| 修改 | `src/lib.rs` | 新增 `pub mod view_model;` |

---

### Task 1: 创建 view_model.rs — ViewModel 核心

**文件：**
- 创建：`src/view_model.rs`

**依赖：** 阅读 `src/model.rs`（CollectionNode/NodeKind）、`src/service.rs`（ApiResponse/ApiEnvironment）

- [ ] **Step 1: 定义 UI 枚举（从 service.rs 移入）**

```rust
use gpui::SharedString;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorTab {
    Params,
    Path,
    Body,
    Headers,
    Cookies,
    Auth,
    PreOps,
    PostOps,
}

impl EditorTab {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Params => "Params",
            Self::Path => "Path",
            Self::Body => "Body",
            Self::Headers => "Headers",
            Self::Cookies => "Cookies",
            Self::Auth => "Auth",
            Self::PreOps => "Pre-ops",
            Self::PostOps => "Post-ops",
        }
    }
    pub fn all() -> [Self; 8] {
        [
            Self::Params,
            Self::Path,
            Self::Body,
            Self::Headers,
            Self::Cookies,
            Self::Auth,
            Self::PreOps,
            Self::PostOps,
        ]
    }
    pub fn index(&self) -> i64 {
        match self {
            Self::Params => 0,
            Self::Path => 1,
            Self::Body => 2,
            Self::Headers => 3,
            Self::Cookies => 4,
            Self::Auth => 5,
            Self::PreOps => 6,
            Self::PostOps => 7,
        }
    }
    pub fn from_index(index: i64) -> Option<Self> {
        Some(match index {
            0 => Self::Params,
            1 => Self::Path,
            2 => Self::Body,
            3 => Self::Headers,
            4 => Self::Cookies,
            5 => Self::Auth,
            6 => Self::PreOps,
            7 => Self::PostOps,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResponseTab {
    Body,
    Cookies,
    Headers,
    Request,
    Curl,
    Logs,
    History,
    Code,
}

impl ResponseTab {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Body => "Body",
            Self::Cookies => "Cookies",
            Self::Headers => "Headers",
            Self::Request => "Request",
            Self::Curl => "cURL",
            Self::Logs => "日志",
            Self::History => "历史",
            Self::Code => "代码",
        }
    }
    pub fn all() -> [Self; 8] {
        [
            Self::Body,
            Self::Cookies,
            Self::Headers,
            Self::Request,
            Self::Curl,
            Self::Logs,
            Self::History,
            Self::Code,
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvDetailTab {
    Variables,
    Headers,
}

impl EnvDetailTab {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Variables => "变量",
            Self::Headers => "公共 Headers",
        }
    }
}
```

- [ ] **Step 2: 定义 ViewModel 数据结构**

```rust
use crate::model::{CollectionNode, NodeKind};

#[derive(Clone)]
pub struct TreeRowVm {
    pub node_id: String,
    pub kind: NodeKind,
    pub depth: u8,
    pub name: SharedString,
    pub method_label: Option<SharedString>,
    pub method_color: u32,
    pub expanded: bool,
    pub has_children: bool,
    pub selected: bool,
}

#[derive(Clone)]
pub struct EnvVm {
    pub name: SharedString,
    pub badge: SharedString,
    pub color: u32,
    pub base_url: SharedString,
}

#[derive(Clone, Default)]
pub struct ResponseVm {
    pub status_line: SharedString,
    pub status_code: u16,
    pub meta: SharedString,
    pub body: SharedString,
    pub headers: SharedString,
    pub cookies: SharedString,
    pub content_type: SharedString,
    pub request_dump: SharedString,
    pub curl: SharedString,
    pub logs: Vec<SharedString>,
    pub assertion_results: Vec<(String, bool)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesignTab {
    Environments,
    Variables,
    Scripts,
    ImportExport,
}

/// 统一弹窗状态机
#[derive(Clone, Debug)]
pub enum Overlay {
    Design(DesignTab),
    Rename { node_id: String },
    ContextMenu { title: SharedString, position: (f32, f32), node_id: String },
    EnvironmentDropdown,
    CurlImport,
}

#[derive(Clone, Default)]
pub struct ApiViewModel {
    pub tree_rows: Vec<TreeRowVm>,
    pub environments: Vec<EnvVm>,
    pub response: ResponseVm,
    pub notice: SharedString,
}
```

- [ ] **Step 3: 实现 build 函数**

```rust
use crate::model::CollectionNode;
use crate::service::{ApiEnvironment, ApiResponse};

impl ApiViewModel {
    pub fn rebuild_tree(
        &mut self,
        nodes: &[CollectionNode],
        expanded_ids: &[String],
        selected_node_id: Option<&str>,
    ) {
        let mut rows = Vec::new();
        // 收集子节点索引
        let mut children: std::collections::HashMap<Option<String>, Vec<&CollectionNode>> =
            std::collections::HashMap::new();
        for node in nodes {
            let parent = node.parent_id.clone();
            // 空字符串视为 None
            let parent = if parent.is_empty() { None } else { Some(parent) };
            children.entry(parent).or_default().push(node);
        }
        // 为每个父节点的子列表按 sort_order 排序
        for list in children.values_mut() {
            list.sort_by_key(|n| n.sort_order);
        }

        let mut stack: Vec<(u8, &CollectionNode)> = Vec::new();
        // 从 root（parent_id = None）开始
        for root in children.remove(&None).unwrap_or_default() {
            stack.push((0, root));
        }

        while let Some((depth, node)) = stack.pop() {
            let expanded = expanded_ids.iter().any(|id| id == &node.id) || node.expanded;
            let has_children = children.contains_key(&Some(node.id.clone()));
            let method_color = match node.kind {
                NodeKind::Endpoint | NodeKind::Case => {
                    // method_color 只对 endpoint/case 有意义
                    parse_method_color(&node.method)
                }
                NodeKind::Folder => 0,
            };
            let method_label = match node.kind {
                NodeKind::Endpoint | NodeKind::Case => Some(SharedString::from(node.method.as_str())),
                NodeKind::Folder => None,
            };
            rows.push(TreeRowVm {
                node_id: node.id.clone(),
                kind: node.kind,
                depth,
                name: node.name.clone().into(),
                method_label,
                method_color,
                expanded,
                has_children,
                selected: selected_node_id.map_or(false, |id| id == node.id),
            });
            // 如果已展开，将子节点逆序压栈（保持显示顺序）
            if expanded {
                if let Some(kids) = children.get(&Some(node.id.clone())) {
                    for child in kids.iter().rev() {
                        stack.push((depth + 1, child));
                    }
                }
            }
        }
        self.tree_rows = rows;
    }

    pub fn set_environments(&mut self, envs: &[ApiEnvironment]) {
        self.environments = envs.iter().map(EnvVm::build).collect();
    }

    pub fn update_response(&mut self, resp: &ApiResponse) {
        self.response = ResponseVm::build(resp);
    }
}

fn parse_method_color(method: &str) -> u32 {
    match method.to_uppercase().as_str() {
        "GET" => 0x338855,
        "POST" => 0x336699,
        "PUT" => 0x7b5fff,
        "PATCH" => 0x997733,
        "DELETE" => 0x994444,
        "HEAD" => 0x557788,
        "OPTIONS" => 0x6b5b95,
        _ => 0x338855,
    }
}

impl EnvVm {
    pub fn build(env: &ApiEnvironment) -> Self {
        let badge = env
            .name
            .chars()
            .next()
            .map(|c| c.to_string())
            .unwrap_or_default();
        Self {
            name: env.name.clone().into(),
            badge: badge.into(),
            color: env.color,
            base_url: env.base_url.clone().into(),
        }
    }
}

impl ResponseVm {
    pub fn build(resp: &ApiResponse) -> Self {
        let duration = resp.duration_ms;
        let size = resp.size_bytes;
        Self {
            status_line: resp.status_line.clone().into(),
            status_code: resp.status_code,
            meta: format!("{duration}ms · {size}B").into(),
            body: resp.body.clone().into(),
            headers: resp.headers.clone().into(),
            cookies: resp.cookies.clone().into(),
            content_type: resp.content_type.clone().into(),
            request_dump: resp.request_dump.clone().into(),
            curl: resp.curl.clone().into(),
            logs: resp.logs.iter().map(|l| SharedString::from(l.as_str())).collect(),
            assertion_results: resp.assertion_results.clone(),
        }
    }
}
```

- [ ] **Step 4: 确认编译**

Run: `touch src/view_model.rs && cargo check -p qingqi-feature-api-debugger`
Expected: 成功（先只看语法，等接线后整体验证）

- [ ] **Step 5: 提交**

```bash
git add crates/qingqi-feature-api-debugger/src/view_model.rs
git commit -m "feat(api-debugger): 新增 view_model.rs — ViewModel 核心类型与 build 函数"
```

---

### Task 2: 重写 service.rs — 按 §4 契约整理

**文件：**
- 修改：`src/service.rs`（保留全部领域逻辑，移除 UI 枚举，添加 `list_collection_nodes` 方法）

**变更要点：**
1. 移除 `EditorTab`/`ResponseTab`/`EnvDetailTab` 枚举及转换函数
2. 移除 `load_workspace()`（不再被新视图使用，视图直接调 `list_collection_nodes` + `list_environments_ui`）
3. 移除 `save_workspace()`（旧方法，新视图用 `save_environment_fields_async`）
4. 保留 `ApiResponse`/`ApiServiceState`/`TabDraft`/`build_http_tab`/`restore_tab_draft` 等类型
5. 添加 `pub fn list_collection_nodes(&self) -> Vec<CollectionNode>`（直接转调 data_source）
6. 所有 `_async` 方法保持不变
7. 保留 `send_request`/`perform_request` 等核心逻辑
8. 重写 `Default impl` 去掉 expect

- [ ] **Step 1: 删除 UI 枚举块（约 100 行）**

```rust
// 删除以下全部内容：
// ── UI-specific enums (not persisted) ──
// EditorTab + impl  + editor_tab_index + index_to_editor_tab
// ResponseTab + impl
// EnvDetailTab + impl
```

Edit service.rs: 删除从 `// ── UI-specific enums` 到 `EnvDetailTab` 结束 + `env_detail_tab_index` + `index_to_env_detail_tab` 的全部代码。

替换后文件起始应当是：
```rust
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, atomic::{AtomicU64, Ordering}},
    thread, time::Instant,
};
use anyhow::{Result, anyhow, bail};
use serde_json::Value;
use uuid::Uuid;
use crate::{
    data_source::ApiDebuggerDataSource,
    model::{ApiVariable, CollectionNode, HttpTab, NodeKind, RequestSnapshot, VariableScope},
    script_service,
    store::ApiWorkspace,
    variable_service,
};
use qingqi_plugin::{database::DatabaseService, log_error, storage::AppPaths};

pub use crate::model::{
    ApiEnvironment, ApiGroup, ApiRequest, ApiScenario, AuthType, BodyMode, EnvHeader,
    EnvVariable, EnvironmentFull, HttpHistory, HttpMethod, KeyValueRow, ScenarioStatus,
};
```

- [ ] **Step 2: 重构 re-export 行**

将 `EnvironmentExport` 后的整个 UI 枚举区域替换掉。保持 `ApiResponse` 和 `ApiServiceState` 结构体定义不变（它们属于领域类型）。

- [ ] **Step 3: 添加 list_collection_nodes 方法（约 3 行）**

在 `impl ApiService` 块内，`revision()` 方法之后：

```rust
pub fn list_collection_nodes(&self) -> Vec<CollectionNode> {
    self.data_source.list_collection_nodes().unwrap_or_default()
}
```

- [ ] **Step 4: 删除 load_workspace 方法**

删除：
```rust
pub fn load_workspace(&self) -> Result<ApiWorkspace> {
    let groups = self.build_collection_tree()?;
    let environments = self.list_environments_ui();
    Ok(ApiWorkspace::new(groups, environments))
}
```
和 `build_collection_tree` 私有方法（新视图不再需 ApiGroup 转换）。

- [ ] **Step 5: 删除 save_workspace 方法**

删除 `pub fn save_workspace` 和 `pub fn save_workspace_async` 两个方法。

- [ ] **Step 6: 重写 Default impl**

替换：
```rust
impl Default for ApiService {
    fn default() -> Self {
        let paths = AppPaths::resolve().expect("failed to resolve qingqi data path");
        let database = Arc::new(DatabaseService::new(paths.clone()));
        Self::new(database, paths)
    }
}
```
为：
```rust
impl Default for ApiService {
    fn default() -> Self {
        Self::new(
            Arc::new(DatabaseService::new(AppPaths::resolve().unwrap_or_default())),
            AppPaths::resolve().unwrap_or_default(),
        )
    }
}
```

- [ ] **Step 7: 保留所有其他方法**

以下方法**保留不动**（它们是领域逻辑核心）：
- `revision()`/`is_in_flight()`/`cancel_request()`
- `take_pending_response()`/`take_pending_error()`/`take_pending_notice()`/`publish_notice()`
- `list_environments_ui()`/`export_environments_json()`/`import_environments_json()`
- `persist_endpoint_snapshot()`/`get_collection_node()`
- 全部 CRUD 方法（create_endpoint/case/folder、delete/rename）
- 全部 _async 方法
- 全部 import/export 方法
- `send_request()` 完整实现
- 环境 CRUD（create/duplicate/delete/save_environment_fields）
- Tab 持久化（load/save/delete_persisted_tab）
- `list_history()`/`clear_history()`
- `build_http_tab()`/`restore_tab_draft()`/`format_auth_for_input()`
- 全部私有辅助函数（`perform_request`/`build_final_url`/`parse_kv_text` 等）
- `code_snippet()` 公开函数

保留 `TabDraft` struct（视图层用到）。

- [ ] **Step 8: 确认编译**

Run: `cargo check -p qingqi-feature-api-debugger`
Expected: 编译通过（注意旧 `view.rs` 可能还引用已移除的类型，下个任务会处理）

- [ ] **Step 9: 提交**

```bash
git add crates/qingqi-feature-api-debugger/src/service.rs
git commit -m "refactor(api-debugger): service.rs 按 §4 契约整理，移除 UI 枚举"
```

---

### Task 3: 创建 view/ 目录 — ApiDebuggerView 骨架

**依赖：** 确认 `src/service.rs` 中 `ApiResponse`、`TabDraft` 等类型仍在原位置

**文件：**
- 创建：`src/view/mod.rs`（主结构 + render 编排）
- 创建：`src/view/sidebar.rs`
- 创建：`src/view/workspace.rs`
- 创建：`src/view/overlay.rs`
- 删除：`src/view.rs`（旧巨石文件）
- 修改：`src/lib.rs`（确认 `pub mod view;` 保留，新增 `pub mod view_model;`）
- 修改：`src/plugin.rs`（更新时间 `view::` 引用）

**view/mod.rs 骨架（含 OpenTab 类型）：**

```rust
// OpenTab 管理打开的请求/场景标签页
#[derive(Clone, Debug, PartialEq, Eq)]
enum OpenTab {
    Request { index: usize, tab_id: String, node_id: String },
    Scenario { request_index: usize, scenario_index: usize, tab_id: String, node_id: String },
}

impl OpenTab {
    fn tab_id(&self) -> &str {
        match self { Self::Request { tab_id, .. } | Self::Scenario { tab_id, .. } => tab_id }
    }
    fn node_id(&self) -> &str {
        match self { Self::Request { node_id, .. } | Self::Scenario { node_id, .. } => node_id }
    }
}

pub struct ApiDebuggerView {
    service: Arc<ApiService>,
    vm: ApiViewModel,
    last_revision: u64,
    open_tabs: Vec<OpenTab>,
    active_tab: usize,
    editor_tab: EditorTab,
    response_tab: ResponseTab,
    overlay: Option<Overlay>,
    show_env_dropdown: bool,
    // ...（后续步骤补充完整字段）
}
```

**view/sidebar.rs 骨架：**
```rust
use gpui::*;
use crate::view_model::ApiViewModel;

pub fn sidebar(
    vm: &ApiViewModel,
    dark: bool,
) -> impl IntoElement {
    div().size_full()
}
```

**view/workspace.rs 骨架：**
```rust
use gpui::*;
use crate::view_model::ApiViewModel;

pub fn workspace(
    vm: &ApiViewModel,
    dark: bool,
) -> impl IntoElement {
    div().size_full()
}
```

**view/overlay.rs 骨架：**
```rust
use gpui::*;

pub fn render(
    overlay: &Overlay,
    dark: bool,
) -> impl IntoElement {
    div()
}
```

- [ ] **Step 1: 创建 `src/view/mod.rs`** 写入完整 `ApiDebuggerView` 结构定义 + 初始字段 + `new` + `default`

- [ ] **Step 2: 创建 `src/view/sidebar.rs`** 写入 `pub fn titlebar_left()` + `pub fn tree()` + `pub fn design_entry()` 骨架

- [ ] **Step 3: 创建 `src/view/workspace.rs`** 写入 `pub fn titlebar_right()` + `pub fn request_bar()` + `pub fn editor_panel()` + `pub fn response_panel()` 骨架

- [ ] **Step 4: 创建 `src/view/overlay.rs`** 写入 `pub fn render()` 骨架

- [ ] **Step 5: 删除旧 `src/view.rs`**

Run: `rm src/view.rs`

- [ ] **Step 6: 更新 `src/plugin.rs`** 确认 import 路径从 `crate::view::ApiDebuggerView` 改用 `crate::view` 模块（Rust 搜索规则不变，view.rs → view/mod.rs 自动生效，`use crate::view` 保持不变）。

- [ ] **Step 7: 确认编译**

Run: `cargo check -p qingqi-feature-api-debugger`
Expected: 编译通过（视图骨架均为空 div，无业务逻辑）

- [ ] **Step 8: 提交**

```bash
git add crates/qingqi-feature-api-debugger/src/view/
git add -u crates/qingqi-feature-api-debugger/src/
git commit -m "feat(api-debugger): 创建 view/ 目录，旧 view.rs 删除，骨架编译通过"
```

---

### Task 4: 实现 view/mod.rs — ApiDebuggerView 完整结构 + render 编排

**文件：**
- 修改：`src/view/mod.rs`

- [ ] **Step 1: 实现 `sync_service_updates`**

```rust
fn sync_service_updates(&mut self) {
    // 处理 pending 响应/错误/通知
    if let Some(resp) = self.service.take_pending_response() {
        self.vm.update_response(&resp);
        if self.response_tab == ResponseTab::History {
            self.refresh_history();
        }
    }
    if let Some(err) = self.service.take_pending_error() {
        self.vm.notice = format!("请求失败: {err}").into();
        let err_resp = ApiResponse {
            status_line: "请求失败".into(), status_code: 0,
            duration_ms: 0, size_bytes: 0,
            body: format!("{{\"error\":{:?}}}", err),
            headers: String::new(), cookies: String::new(),
            content_type: String::new(), request_dump: String::new(),
            curl: String::new(), logs: vec![format!("请求失败: {err}")],
            assertion_results: Vec::new(),
        };
        self.vm.update_response(&err_resp);
    }
    if let Some(note) = self.service.take_pending_notice() {
        self.vm.notice = note.into();
        self.vm.set_environments(&self.service.list_environments_ui());
    }
    // revision 变化时重建数据型 vm
    let rev = self.service.revision();
    if rev != self.last_revision {
        self.last_revision = rev;
        let nodes = self.service.list_collection_nodes();
        let envs = self.service.list_environments_ui();
        let selected_id = self.selected_node_id();
        self.vm.rebuild_tree(&nodes, &[], selected_id.as_deref());
        self.vm.set_environments(&envs);
    }
}
```

- [ ] **Step 2: 实现 `render` 编排（设计文档 §6.2）**

根容器是 `flex_row`（左右两列），**不使用** `popup_window_chrome`。竖直分隔线是左列的右边框。

```rust
impl Render for ApiDebuggerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_service_updates(window, cx);
        let dark = qingqi_ui::theme_mode::is_dark();
        let entity = cx.entity();

        div()
            .relative().size_full()
            .bg(glass::bg(dark))
            .rounded(px(12.0)).overflow_hidden()
            .font_family("Inter, PingFang SC")
            .text_color(theme::semantic().text_primary)
            .on_key_down(...) // Esc 关闭 overlay
            .flex().flex_row()
            // 左列：固定 280px，右边框为竖线
            .child(
                div()
                    .w(px(280.0)).h_full()
                    .flex().flex_col()
                    .border_r_1().border_color(glass::divider(dark))
                    .child(sidebar::sidebar(self, dark))
            )
            // 右列：flex_1
            .child(
                div()
                    .flex_1().min_w(px(0.0)).h_full()
                    .flex().flex_col()
                    .child(workspace::workspace(self, window, cx, dark))
            )
        // overlay 弹窗（稍后实现）
    }
}
```

- [ ] **Step 3: 实现所有实体创建（`new` 函数）**

构造 ApiDebuggerView 时，调用 `cx.new(|| ...)` 创建所有 TextInput 实体。

- [ ] **Step 4: 实现 tab 管理方法**

`select_request(node_id)`：设置 `active_tab`，从 service 加载对应 tab 的 draft 恢复输入，`cx.notify()`。
`select_open_tab(index)`：按 `open_tabs[index]` 切换 `active_tab`，恢复输入。
`close_open_tab(index)`：从 `open_tabs` 移除，`service.delete_persisted_tab_async()`，若关闭的是当前 tab 则切换到相邻 tab。
所有 tab 切换方法都调用 `persist_current_tab_state()` 保存当前编辑内容再切换。

- [ ] **Step 5: 实现 KvEditor 子模块（内联或独立）**

将旧 view.rs 的 `KvRow`/`KvEditor` 结构体移入 mod.rs（或作为私有子模块），保持所有方法（`new`/`set_rows`/`to_rows`/`add_row`/`remove_row`/`toggle`）。

- [ ] **Step 6: 实现 auth 表单方法**

`auth_rows(cx) -> Vec<KeyValueRow>`：根据 `self.auth_type` 从对应输入读值，组合成标准的 auth KV 行（Bearer 格式 `Authorization=Bearer <token>`；Basic 格式 `Authorization=Basic <base64>`；ApiKey 格式 `X-API-Key=<value>`，description 标记 header/query）。
`load_auth_form(cx, rows)`：反向操作，从 KV 行解析出 auth_type 和各输入框值，用 `derive_auth_form()` 辅助函数。
`derive_auth_form(rows) -> AuthFormValues`：从 auth KV 行解析出各字段的字符串值。

- [ ] **Step 7: 确认编译**

Run: `cargo check -p qingqi-feature-api-debugger`
Expected: 编译通过

- [ ] **Step 8: 提交**

```bash
git add crates/qingqi-feature-api-debugger/src/view/mod.rs
git commit -m "feat(api-debugger): view/mod.rs 完整 render 编排 + sync_service_updates"
```

---

### Task 5: 实现 sidebar.rs — 左栏

**文件：**
- 修改：`src/view/sidebar.rs`

**实现内容：**
1. **顶栏左段（高 36px）**：`pl(px(72.0))` 给交通灯让位 + 标题「API」+ 「＋ 新建」按钮
2. **接口树（flex_1）**：`uniform_list` 虚拟化渲染 `self.vm.tree_rows`
3. **设计入口（高 40px）**：全宽「⚙ 设计」按钮

接口树每行：
- 缩进 `depth * 16px`
- Folder：折叠箭头 + 📁 + 名称
- Endpoint：method 徽章（带颜色）+ 名称（点击打开 tab）
- Case：缩进 + 🔹 + 名称

- [ ] **Step 1: 实现 `sidebar()` 入口函数 + 顶栏左段**

```rust
pub fn sidebar(view: &mut ApiDebuggerView, dark: bool) -> impl IntoElement {
    div().flex().flex_col().size_full()
        .child(titlebar_left(view, dark))
        .child(tree(view, dark))
        .child(design_entry(view, dark))
}
```

- [ ] **Step 2: 实现接口树 `uniform_list`**

```rust
fn tree(view: &mut ApiDebuggerView, dark: bool) -> impl IntoElement {
    let rows = view.vm.tree_rows.clone();
    div()
        .flex_1().min_h(px(0.0))
        .child(
            uniform_list(
                "api-tree",
                rows.len(),
                move |_this, range, _window, _cx| {
                    range.map(|i| tree_row(&rows[i], i, dark)).collect()
                },
            )
            .size_full()
        )
}
```

- [ ] **Step 3: 实现 tree_row 函数**

- [ ] **Step 4: 实现 `design_entry` 按钮**

- [ ] **Step 5: 确认编译**

Run: `cargo check -p qingqi-feature-api-debugger`

- [ ] **Step 6: 提交**

```bash
git add crates/qingqi-feature-api-debugger/src/view/sidebar.rs
git commit -m "feat(api-debugger): sidebar 左栏 — 标题行 + uniform_list 接口树 + 设计入口"
```

---

### Task 6: 实现 workspace.rs — 右栏

**文件：**
- 修改：`src/view/workspace.rs`

**实现内容：**
1. **顶栏右段（高 36px）**：TabBar + 环境下拉（参考 open_tabs_bar）
2. **请求行**：method 下拉（`Select<HttpMethod>`）+ URL 输入框 + 发送/取消按钮
3. **编辑 tab 组**：`TabBar` + 各 tab 编辑区（Params/Path/Body/Headers/Cookies/Auth/PreOps/PostOps）
4. **响应区**：状态行 + 响应 tab 组 + 内容显示

- [ ] **Step 1: 实现 `workspace()` 入口 + 顶栏右段（TabBar）**

- [ ] **Step 2: 实现请求行（method Select + URL + 发送按钮）**

- [ ] **Step 3: 实现编辑 tab 组 + kv_editor_table + auth_form_panel**

- [ ] **Step 4: 实现响应区（状态行 + 响应 tab + body/headers/cookies/cURL/logs/history/code）**

- [ ] **Step 5: 确认编译**

Run: `cargo check -p qingqi-feature-api-debugger`

- [ ] **Step 6: 提交**

```bash
git add crates/qingqi-feature-api-debugger/src/view/workspace.rs
git commit -m "feat(api-debugger): workspace 右栏 — tab 栏 + 请求行 + 编辑区 + 响应区"
```

---

### Task 7: 实现 overlay.rs — 弹窗系统

**文件：**
- 修改：`src/view/overlay.rs`

**实现具体弹窗：**
1. **环境管理弹窗**（DesignTab::Environments）：环境列表 + 编辑 base_url/变量/headers
2. **上下文菜单**（右键接口树）：新建端点/分组、导入 cURL/OpenAPI/Postman、重命名、删除
3. **重命名弹窗**：单行输入 + 确认/取消
4. **cURL 导入弹窗**：多行输入 + 导入按钮
5. **环境下拉面板**（点击环境标签展开/收起）

- [ ] **Step 1: 实现主 dispatch 函数**

```rust
pub fn render(
    view: &mut ApiDebuggerView,
    overlay: &Overlay,
    window: &mut Window,
    cx: &mut Context<ApiDebuggerView>,
    dark: bool,
) -> impl IntoElement {
    match overlay {
        Overlay::Design(tab) => design_dialog(view, *tab, dark),
        Overlay::Rename { node_id } => rename_dialog(view, node_id.clone(), dark),
        Overlay::ContextMenu { title, position, node_id } => context_menu(view, title, *position, node_id, dark),
        Overlay::EnvironmentDropdown => env_dropdown(view, dark),
        Overlay::CurlImport => curl_import_dialog(view, dark),
    }
}
```

- [ ] **Step 2: 实现 `design_dialog`（环境管理弹窗，设计文档 §5）**

- [ ] **Step 3: 实现 `context_menu`（右键菜单）**

- [ ] **Step 4: 实现 `rename_dialog` / `curl_import_dialog`**

- [ ] **Step 5: 实现 `env_dropdown`（环境下拉面板）**

- [ ] **Step 6: 确认编译**

Run: `cargo check -p qingqi-feature-api-debugger`

- [ ] **Step 7: 提交**

```bash
git add crates/qingqi-feature-api-debugger/src/view/overlay.rs
git commit -m "feat(api-debugger): overlay 弹窗系统 — 设计/菜单/重命名/导入/环境下拉"
```

---

### Task 8: 接线 — 按钮事件、tab 持久化、send_request

**文件：**
- 修改：`src/view/mod.rs`
- 修改：`src/view/sidebar.rs`（补充 on_click 回调）
- 修改：`src/view/workspace.rs`（补充 on_click 回调）

**接线内容：**
1. sidebar「＋ 新建」→ `view.open_collection_menu()`
2. sidebar 接口树单击 → `view.select_request(node_id)`
3. sidebar 接口树右键 → `view.open_context_menu()`
4. sidebar 设计入口 → `view.overlay = Some(Overlay::Design(DesignTab::Environments))`
5. workspace 请求行发送 → `view.send_request(cx)`
6. workspace 编辑 tab 切换 → `view.editor_tab = tab`
7. workspace 响应 tab 切换 → `view.set_response_tab(tab)`
8. workspace tab 栏 × 关闭 → `view.close_open_tab(index, cx)`
9. tab 持久化 → `view.persist_current_tab_state(cx)`（切换/修改时调用）
10. overlay 各按钮 → 对应 `service._async()` 方法

- [ ] **Step 1: 实现 `send_request` + `cancel_request` 方法**

- [ ] **Step 2: 实现 `persist_current_tab_state` + `collect_tab_draft`**

- [ ] **Step 3: 将所有 on_click 回调接入 view 的方法**

- [ ] **Step 4: 确认编译**

Run: `cargo check -p qingqi-feature-api-debugger`

- [ ] **Step 5: 提交**

```bash
git add crates/qingqi-feature-api-debugger/src/view/
git commit -m "feat(api-debugger): 接线 — 按钮事件、tab 持久化、send_request"
```

---

### Task 9: 自检 — 按 §8 清单验证 + 运行检测命令

**文件：** 无代码修改

- [ ] **Step 1: 运行完整验证命令**

Run: 参见设计文档 §10

```bash
cargo fmt --all
cargo check -p qingqi-feature-api-debugger
cargo clippy -p qingqi-feature-api-debugger --all-targets

# 架构不变量（必须为空）
cargo tree -p qingqi-feature-api-debugger | rg "qingqi-(core|app)"

# render 路径不得有 panic / 锁（人工核对，辅助查找）
rg -n "unwrap\(\)|expect\(|lock\(\)" crates/qingqi-feature-api-debugger/src/view/

# 接口树虚拟化
rg -n "uniform_list" crates/qingqi-feature-api-debugger/src/view/sidebar.rs

# render 中硬编码颜色
rg -n "rgb\(0x" crates/qingqi-feature-api-debugger/src/view/
```

- [ ] **Step 2: 检查 §8 清单**

逐项核对：
- [ ] qingqi-ui 版本兼容
- [ ] render 路径无 `lock()`/`unwrap()`/`expect()`
- [ ] render 路径无 `cx.new()`
- [ ] 接口树用 `uniform_list`
- [ ] 颜色用 `ui::*`/`theme::semantic()` token
- [ ] 命名规范（`ApiDebuggerView`/`*Vm`/`ApiService`）

- [ ] **Step 3: 提交最终版本**

```bash
git commit -m "feat(api-debugger): 自检通过，重构完成"
```
