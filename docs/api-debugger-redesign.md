# API 调试器插件 — 重新设计文档（从零实现）

> 目标读者：负责实现的 AI 模型 / 工程师。本文档是**唯一事实来源**，按本文档逐文件、逐步骤实现即可，不要参考旧 `view.rs` / `service.rs` 的代码风格（它们是被替换的对象）。
>
> 所属仓库：`qingqi`（Rust + GPUI 桌面应用，workspace 插件化）。
> 所属 crate：`crates/qingqi-feature-api-debugger`。
> 必须遵守：`/.claude/CLAUDE.md` 中的全部强制规则（分层、命名、高性能 UI 铁律、错误处理、语义 token）。

---

## 1. 背景与重设计动机

旧实现的问题（不要重蹈覆辙）：

| 问题 | 说明 | 本设计的对策 |
|------|------|--------------|
| 巨石文件 | `view.rs` 5228 行、`service.rs` 3680 行 | 拆分 `view/` 子模块，每文件单一职责 |
| 无 ViewModel | `render` 里 `clone` 整组数据 + `format!` 计算 | 引入 `ApiViewModel`，数据变化时算一次 |
| 树未虚拟化 | `collection_tree` 全量渲染 | 用 `uniform_list` 虚拟化扁平化后的树 |
| render 中 panic | `.expect("init")`、`.expect("api request should exist")` | 全部消除，render 只读 ViewModel |
| 窗口外壳不统一 | 自己 `pl(px(68.0))` 硬塞交通灯 | 自绘左右两列顶栏，竖线贯穿；macOS 原生交通灯仅用 `pl` 让位 |
| 弹窗状态混乱 | 多个 `show_xxx: bool` 标志位 | 单一 `overlay: Option<Overlay>` 状态机 |

**不变量保持不变**：插件不得依赖 `qingqi-core`/`qingqi-app`/其他 feature，只依赖 `qingqi-plugin` 与 `qingqi-ui`。

---

## 2. 界面规格（必须严格实现）

整体为**左右分栏**。左栏固定宽 `280px`，右栏 `flex_1`。**一条竖直分隔线从窗口最顶部一直贯穿到底部**，把整窗切成左右两半：顶栏左段是交通灯+标题+新建，顶栏右段是 tab 栏。**不使用 `popup_window_chrome`**——它是横贯全宽的标题栏，做不出这个被分隔线切开的顶栏。

```
┌───────────────────────┬─────────────────────────────────────────────────────┐
│  ● ● ●   API    [＋]   │             [tab1] [tab2] [tab3] [+]                 │ ← 顶栏 36px：左右各一段
│                       │                                                     │
│  接口树               │  ┌─ Tab Content（tabcontent）─────────────────────┐ │
│   ▸ 📁 用户模块        │  │  [GET ▾] [https://.../users         ] [发送]   │ │
│      • GET 列表        │  │  ─ Params│Headers│Body│Auth│前置│后置 ───────── │ │
│      • POST 创建       │  │  │  <编辑区>                                   │ │
│   ▸ 📁 订单模块        │  │  ──────────────────────────────────────────── │ │
│                       │  │  响应：200 OK  128ms  2.1KB                    │ │
│                       │  │  ─ Body│Headers│Cookies│cURL│日志│历史 ──────── │ │
│                       │  │  │  <响应区>                                   │ │
│                       │  └────────────────────────────────────────────────┘ │
│  ⚙ 设计               │                                                     │ ← 左下角「设计」按钮
└───────────────────────┴─────────────────────────────────────────────────────┘
   ↑ 左栏 280px（与顶栏左段同宽）  ↑ 竖线贯穿到顶   ↑ 右栏 flex_1
```

**布局骨架（关键）**：根容器 `flex_row`（左右两大列），每列内部再 `flex_col`。竖直分隔线就是两列之间的边框，天然从顶到底贯穿：

```
根 = flex_row
├─ 左列  w 280px, flex_col
│   ├─ 顶栏左段 (高 36px)：交通灯让位 + 「API」标题 + 「＋」新建
│   ├─ 接口树   (flex_1, min_h 0, uniform_list)
│   └─ 设计入口 (高 40px, 固定底部)
├─ 竖直分隔线 (border / 1px div, 全高)
└─ 右列  flex_1, flex_col
    ├─ 顶栏右段 (高 36px)：tab 栏 + [＋]
    └─ tabcontent (flex_1)：请求行 + 编辑 tab + 响应区
```

> 注意：顶栏左段与接口树同属左列，所以左段宽度自动等于左栏宽度（280px），竖线自然对齐贯穿——不需要单独再拼一行顶栏。两列高度均为 `size_full`，顶栏是各列 `flex_col` 的第一个子元素。

**交通灯**：本插件是 `blurred` 窗口（manifest `ratio_blurred`），macOS 下系统在窗口左上角**原生绘制交通灯**，无需也不要自己画。左列顶栏左段只需 `pl(px(72.0))` 给交通灯让出空间。Windows 下该路径无原生交通灯，可在左段最左放一个 `ui::window_close_button()`（次要平台，先按 macOS 实现）。

### 2.1 左列（`sidebar`）

`flex_col`，宽 `280px`，`size_full`（高度撑满），三段竖向布局：

1. **顶栏左段**（高 `36px`，与右列顶栏右段同高对齐）
   - macOS 由系统原生绘制交通灯，**不要自己画**。左段 `pl(px(72.0))` 给交通灯让位。
   - 标题文字「API」`text_size(px(13.0))` `SEMIBOLD`，用 `ui::text_primary()`。
   - 右侧「＋ 新建」按钮：`Button::new("api-sidebar-new").ghost().icon(IconName::Plus).xsmall()`。点击打开「新建」上下文菜单（新建文件夹 / 新建接口 / 从 cURL 导入 / 导入 OpenAPI / 导入 Postman）。

2. **接口树**（`flex_1`，`min_h(px(0.0))`，可滚动）
   - 用 `uniform_list` 虚拟化（**强制**，见 §7.3）。
   - 数据来自 ViewModel 的扁平化树 `Vec<TreeRowVm>`。
   - 每行：缩进（按 `depth`）+ 折叠箭头（仅 folder）+ 图标 + 名称 + method 徽章（仅 endpoint/case）。
   - 单击：folder 折叠/展开；endpoint/case 在右侧打开对应 tab。
   - 右键：弹出节点上下文菜单（重命名 / 删除 / 新建子项）。

3. **设计入口**（高 `40px`，固定底部）
   - 一个全宽按钮：`⚙ 设计`，点击打开「设计弹窗」（§5）。

> 左列与右列之间用 `border_r_1().border_color(glass::divider(dark))`（或在两列间插一个 `1px` 全高 `div`）形成贯穿顶部的竖直分隔线。

### 2.2 右列（`workspace`）

`flex_1`，`flex_col`，`size_full`，两段竖向布局：

1. **顶栏右段**（高 `36px`）
   - 展示所有打开的请求 tab，用 `gpui_component::tab::{TabBar, Tab}`（参考 FTP 插件 `titlebar_slot` 的 TabBar 用法）。
   - 每个 tab：method 颜色点 + 标题 + 关闭按钮 `✕`。
   - 末尾 `[＋]` 新建空白请求 tab。

2. **tabcontent**（`flex_1`）
   - **请求行**：method 下拉（`Select<Vec<HttpMethod>>`）+ URL 输入框（`TextInput`）+ `发送/取消` 按钮。
   - **请求编辑 Tab 组**：`Params / Path / Body / Headers / Cookies / Auth / 前置 / 后置`（枚举 `EditorTab`，沿用旧 model）。
   - **响应区**：状态行（status / 耗时 / 大小）+ 响应 Tab 组：`Body / Headers / Cookies / Request / cURL / 日志 / 历史 / 代码`（枚举 `ResponseTab`）。

---

## 3. 文件结构（重设计后）

```
crates/qingqi-feature-api-debugger/src/
  lib.rs              # 模块声明 + databases() + build()   （保留，几乎不动）
  manifest.rs         # 元数据，纯数据                       （保留不动）
  model.rs            # 领域类型，禁止 GPUI/IO               （保留，复用现有类型）
  store.rs            # 旧 JSON 工作区，逐步废弃             （保留兼容，新增逻辑勿用）
  data_source.rs      # SQLite 持久化，禁止 GPUI             （保留，复用现有 schema）
  service.rs          # 领域服务 ApiService：snapshot+revision（重写为更清晰的契约，见 §4）
  variable_service.rs # 变量解析（保留不动）
  script_service.rs   # 脚本/断言（保留不动）
  curl_parser.rs / import_openapi.rs / import_postman.rs / code_gen.rs  # 工具（保留不动）
  view_model.rs       # ★新增：ApiViewModel + 所有 *Vm 行结构（render-ready）
  view/
    mod.rs            # ★新增：ApiDebuggerView 主视图 + render 编排 + 实体持有
    sidebar.rs        # ★新增：左栏（标题行 / 接口树 / 设计入口）
    workspace.rs      # ★新增：右栏（tab 栏 + 请求行 + 编辑 tab + 响应区）
    overlay.rs        # ★新增：所有弹窗（设计 / 环境 / 变量 / 重命名 / 导入）
  plugin.rs           # 装配，impl Plugin                    （保留，仅改 view 路径）
```

> 旧 `view.rs`（单文件）将被 `view/` 目录替换。实现时新建 `view/` 目录，最后删除旧 `view.rs`，并把 `lib.rs` 的 `pub mod view;` 指向目录模块（Rust 中 `view/mod.rs` 自动等价于 `view` 模块，无需改 `lib.rs` 的 `pub mod view;`）。新增 `pub mod view_model;`。

---

## 4. Service 契约（`service.rs`）

`ApiService` 是唯一事实来源，被 `Arc` 共享。视图通过 **snapshot + revision** 读数据，绝不直接持有 store/连接。

### 4.1 核心字段（保留旧实现思路）

```rust
pub struct ApiService {
    revision: AtomicU64,      // 任何数据变化 +1
    generation: AtomicU64,    // 请求代际，防过期回写
    state: Mutex<ApiServiceState>,   // in_flight / pending_response / pending_error / pending_notice
    data_source: ApiDebuggerDataSource,  // SQLite
}
```

### 4.2 必须暴露的方法（按功能分组，签名以现有实现为准）

读取（视图构建 snapshot 用）：
- `fn revision(&self) -> u64`
- `fn is_in_flight(&self) -> bool`
- `fn list_collection_nodes(&self) -> Vec<CollectionNode>`（返回全部节点，视图侧建树）
- `fn list_environments_ui(&self) -> Vec<ApiEnvironment>`
- `fn load_persisted_tabs(&self) -> Vec<HttpTab>`
- `fn load_persisted_tab_by_id(&self, id: &str) -> Option<HttpTab>`
- `fn list_history(&self, tab_id: &str, limit: i64) -> Result<Vec<HttpHistory>>`
- `fn take_pending_response(&self) -> Option<ApiResponse>`
- `fn take_pending_error(&self) -> Option<String>`
- `fn take_pending_notice(&self) -> Option<String>`

写入（全部提供 `_async` 版本，内部 `revision += 1`）：
- 节点：`create_folder_async` / `create_endpoint_async` / `create_case_async` / `rename_collection_item_async` / `delete_collection_item_async` / `persist_endpoint_snapshot`
- 环境：`create_environment_async` / `duplicate_environment_async` / `delete_environment_by_index_async` / `save_environment_fields_async`
- 导入：`import_from_curl_async` / `import_from_openapi_async` / `import_from_postman_async` / `import_environments_json_async`
- Tab：`save_tab_state_async` / `delete_persisted_tab_async`

请求执行：
- `fn send_request(self: &Arc<Self>, env, request, pre_ops, post_ops, tab_id) -> Result<()>`
  - 后台线程发请求；带 generation guard；完成后写 `pending_response`，`revision += 1`。
- `fn cancel_request(&self)`：推进 generation，丢弃在途结果。

> **规则**：所有 `_async` 方法用后台线程/executor，完成后 `revision += 1`。视图在 `render` 前调用 `sync_service_updates()` 比较 `revision`，变了就重建 ViewModel。**禁止视图直接调 `data_source`**。

---

## 5. 设计弹窗（左下角「设计」入口）

「设计」弹窗是一个集中管理工作区配置的 modal，分左侧菜单 + 右侧内容两栏。包含以下分页：

1. **环境管理**：环境列表（新增/复制/删除），右侧编辑 base_url、变量表、公共 headers。
2. **变量管理**：全局 / 环境 / 模块作用域变量的查看与编辑。
3. **脚本管理**：前置/后置/公共脚本的增删改（`scripts` 表）。
4. **导入导出**：从 cURL / OpenAPI / Postman 导入；导出集合为 OpenAPI、导出环境 JSON。

弹窗实现要点：
- 用统一 overlay 状态：`overlay: Option<Overlay>`，`enum Overlay { Design(DesignTab), Rename{ node_id }, ContextMenu{...}, ... }`。
- 背景遮罩用 `ui::overlay_backdrop()`，`Esc` 关闭（在主 `render` 的 `on_key_down` 统一处理）。
- 弹窗内的输入框实体（环境名、base_url、变量文本等）在 `ApiDebuggerView::new` 时**一次性创建**，弹窗只 `set_text` 复用（铁律5：实体只创建一次）。

---

## 6. View 主结构（`view/mod.rs`）

### 6.1 结构定义

```rust
pub struct ApiDebuggerView {
    service: Arc<ApiService>,

    // ── render-ready 数据（唯一被 render 读取的状态）──
    vm: ApiViewModel,
    last_revision: u64,

    // ── 交互选择态（轻量、Copy/小 String）──
    open_tabs: Vec<TabId>,        // 打开的 tab id 列表
    active_tab: Option<TabId>,
    editor_tab: EditorTab,
    response_tab: ResponseTab,
    overlay: Option<Overlay>,     // 统一弹窗状态机

    // ── 实体：构造时创建一次，禁止 render 中 cx.new ──
    url_input: Entity<TextInput>,
    body_input: Entity<TextInput>,
    method_select: Entity<SelectState<Vec<HttpMethod>>>,
    params_kv: KvEditor,
    headers_kv: KvEditor,
    cookies_kv: KvEditor,
    path_kv: KvEditor,
    auth_*_input: Entity<TextInput>,   // 沿用旧 auth 表单实体集合
    pre_ops_input: Entity<TextInput>,
    post_ops_input: Entity<TextInput>,
    // 设计弹窗用的实体（环境/变量/重命名/导入）
    env_name_input / env_base_url_input / env_variables_input / env_headers_input: Entity<TextInput>,
    rename_input: Entity<TextInput>,
    curl_import_input: Entity<TextInput>,
}
```

### 6.2 render 编排（顶层）

根容器是 `flex_row`（左右两列），**不使用 `popup_window_chrome`**。竖直分隔线是左列的右边框，从顶到底贯穿。

```rust
impl Render for ApiDebuggerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_service_updates(cx);   // 比较 revision，必要时重建 vm
        let dark = qingqi_ui::theme_mode::is_dark();

        div().relative().size_full()
            .bg(glass::bg(dark))
            .rounded(px(12.0)).overflow_hidden()
            .shadow(glass::shadow())
            .font_family(ui::font_ui())
            .text_color(theme::semantic().text_primary)
            .flex().flex_row()                       // ← 左右两列
            .on_key_down(/* Esc 关闭 overlay */)
            .child(
                // 左列：固定 280px，右边框即贯穿顶部的竖直分隔线
                div().w(px(280.0)).h_full()
                    .flex().flex_col()
                    .border_r_1().border_color(glass::divider(dark))
                    .child(sidebar::render(self, cx, dark))   // 顶栏左段 + 接口树 + 设计入口
            )
            .child(
                // 右列：flex_1
                div().flex_1().min_w(px(0.0)).h_full()
                    .flex().flex_col()
                    .child(workspace::render(self, window, cx, dark))  // 顶栏右段 + tabcontent
            )
            .children(self.overlay.as_ref().map(|o| overlay::render(self, o, cx, dark)))
    }
}
```

> **顶栏对齐**：左列与右列各自 `flex_col` 的第一个子元素都是高 `36px` 的顶栏段，二者顶部对齐，被竖线在中间切开——这就实现了「顶栏被分隔线一直切到最顶部」的效果。
>
> **交通灯让位**：左列顶栏左段（`sidebar::render` 内）`pl(px(72.0))`，给 macOS 系统原生交通灯让出空间。整窗根容器**不要** `pt`，让顶栏直接顶到窗口最上沿（交通灯就落在左段的 padding 区里）。


### 6.3 `sync_service_updates`

```rust
fn sync_service_updates(&mut self, cx: &mut Context<Self>) {
    // 1. 取在途结果
    if let Some(resp) = self.service.take_pending_response() { self.vm.response = ResponseVm::build(&resp); }
    if let Some(err)  = self.service.take_pending_error()    { self.vm.notice = err; }
    if let Some(note) = self.service.take_pending_notice()   { self.vm.notice = note; }
    // 2. revision 变化才重建数据型 vm（树/环境）
    let rev = self.service.revision();
    if rev != self.last_revision {
        self.last_revision = rev;
        let nodes = self.service.list_collection_nodes();
        let envs  = self.service.list_environments_ui();
        self.vm.rebuild_tree(&nodes, &self.open_tabs, self.active_tab.as_ref());
        self.vm.environments = envs.into_iter().map(EnvVm::build).collect();
        cx.notify();
    }
}
```

> **注意**：`sync_service_updates` 在 render 头部调用，里面有 `list_*`（读 SQLite）。这是**有条件的**——只在 `revision` 变化时执行，稳态下 render 不触发 IO。这是项目里被接受的模式（FTP 插件同款）。稳态 render 只读 `self.vm`。

---

## 7. ViewModel（`view_model.rs`）★核心

`render` 只能读 `ApiViewModel`，所有排序/格式化/建树在 `build`/`rebuild_*` 里算一次。

```rust
#[derive(Default)]
pub struct ApiViewModel {
    pub tree_rows: Vec<TreeRowVm>,      // 扁平化后的接口树（虚拟列表数据源）
    pub environments: Vec<EnvVm>,
    pub response: ResponseVm,
    pub notice: String,
}

pub struct TreeRowVm {
    pub node_id: String,
    pub kind: NodeKind,
    pub depth: u8,
    pub name: SharedString,
    pub method_label: Option<SharedString>,  // endpoint/case 的 method
    pub method_color: u32,                    // 来自 HttpMethod::color()，仅此处用 0x..，render 不算颜色
    pub expanded: bool,
    pub has_children: bool,
    pub selected: bool,
}

#[derive(Default)]
pub struct ResponseVm {
    pub status_line: SharedString,
    pub status_color: u32,
    pub meta: SharedString,    // "128ms · 2.1KB"
    pub body: SharedString,
    pub headers: SharedString,
    pub cookies: SharedString,
    // ...
}
```

### 7.1 建树（扁平化）

`rebuild_tree`：把 `Vec<CollectionNode>`（含 `parent_id`、`sort_order`、`expanded`）转成**按显示顺序排好的扁平 `Vec<TreeRowVm>`**，折叠的 folder 不展开其子节点。算一次，render 直接虚拟化渲染。

### 7.2 颜色规则

- ViewModel `build` 阶段允许调用 `HttpMethod::color()`（返回 `u32`）写入 `method_color`，因为这是**领域语义色**，旧 model 已定义。
- **render 中禁止** `rgb(0x...)`。背景/边框/文字一律用 `ui::*` 与 `theme::semantic()`。method 徽章颜色读 `vm.method_color` 再 `rgb()`（数据来自 vm，不是 render 计算）。

### 7.3 接口树虚拟化（强制）

```rust
uniform_list(
    "api-tree",
    self.vm.tree_rows.len(),
    cx.processor(move |view, range: Range<usize>, _w, cx| {
        range.map(|i| sidebar::tree_row(&view.vm.tree_rows[i], i, cx, dark)).collect()
    }),
).size_full()
```

---

## 8. 编码规则清单（逐条核对，违反即返工）

render 路径（`render` + 所有 `render_*`/`tree_row`）必须：
- [ ] 不出现 `.lock()` / `.unwrap()` / `.expect()`
- [ ] 不读 SQLite / 文件 / 网络（`sync_service_updates` 里有条件的读除外）
- [ ] 不 `cx.new()` 创建实体
- [ ] 不排序 / 不建树 / 不大量 `format!`（这些在 ViewModel `build` 里做）
- [ ] 不出现硬编码 `rgb(0x...)`（method 色读 vm 字段除外）
- [ ] 接口树用 `uniform_list`

异步：
- [ ] `send_request` / 所有 `_async` 带 generation guard
- [ ] URL/搜索输入去抖 60–120ms（若加搜索）
- [ ] 不跨 `.await` 持锁，不 `std::thread::sleep`

命名：
- [ ] 主视图 `ApiDebuggerView`（不是 Panel）
- [ ] render 数据结构以 `*Vm` 结尾
- [ ] 服务 `ApiService`、持久化 `ApiDebuggerDataSource`

组件/样式：
- [ ] 优先 `gpui_component`（Button/Select/TabBar/Tab/Icon）> `qingqi_ui::ui` > `div()`
- [ ] 颜色用 `ui::bg_surface()` / `ui::text_primary()` / `theme::semantic()`
- [ ] 字体用 `ui::font_ui()` / `ui::font_mono()`
- [ ] 图标用 `IconName::*` 或 SVG，禁止新增 PNG

---

## 9. 实现步骤（按序执行，每步 `cargo check`）

1. **新增 `view_model.rs`**：定义 `ApiViewModel` 及各 `*Vm`，实现 `rebuild_tree` / `EnvVm::build` / `ResponseVm::build`。先不接 UI，写单元测试验证扁平化建树。
2. **`service.rs` 补齐契约**：确认 §4.2 所有方法存在且带 `revision += 1`；缺 `list_collection_nodes` 就新增（读全部节点）。
3. **建 `view/` 目录**：先放 `mod.rs`，定义 `ApiDebuggerView` 结构 + `new`（创建所有实体一次）+ 空 `render`（先只画左右两个空 `div`，跑通编译）。
4. **`view/sidebar.rs`**：实现标题行（交通灯让位 + 新建按钮）、`uniform_list` 接口树、设计入口按钮。
5. **`view/workspace.rs`**：tab 栏（`TabBar`/`Tab`）+ 请求行（method select + url input + 发送）+ 编辑 tab 组 + 响应区。
6. **`view/overlay.rs`**：`Overlay` 状态机 + 设计弹窗（环境/变量/脚本/导入导出）+ 重命名 + 上下文菜单。
7. **接线**：`sync_service_updates`、各按钮 `on_click` 调 service `_async`、tab 持久化。
8. **删除旧 `view.rs`**，更新 `lib.rs`（新增 `pub mod view_model;`，`pub mod view;` 保持）。
9. **自检**：跑 §8 清单 + 下方验证命令。

---

## 10. 验证命令

```bash
cargo fmt --all
cargo check -p qingqi-feature-api-debugger
cargo clippy -p qingqi-feature-api-debugger --all-targets

# 架构不变量（必须为空）
cargo tree -p qingqi-feature-api-debugger | rg "qingqi-(core|app)"   # 不应出现 core/app

# render 路径不得有 panic / 锁（人工核对，辅助查找）
rg -n "unwrap\(\)|expect\(|lock\(\)" crates/qingqi-feature-api-debugger/src/view/

# 接口树虚拟化
rg -n "uniform_list" crates/qingqi-feature-api-debugger/src/view/sidebar.rs   # 必须命中

# render 中硬编码颜色（应仅在 view_model 出现 0x，view/ 下不应有 rgb(0x）
rg -n "rgb\(0x" crates/qingqi-feature-api-debugger/src/view/
```

---

## 11. 复用清单（这些现有文件**直接复用，不要重写**）

| 文件 | 复用内容 |
|------|----------|
| `model.rs` | `HttpMethod`/`BodyMode`/`AuthType`/`CollectionNode`/`ApiRequest`/`ApiEnvironment`/`KeyValueRow`/`NodeKind`/`HttpTab`/`HttpHistory`/`RequestSnapshot` 等全部领域类型 |
| `data_source.rs` | SQLite schema 与全部 CRUD（`collection_nodes`/`environments`/`http_tabs`/`http_history`/`api_variables`/`scripts`） |
| `variable_service.rs` | `{{var}}` 变量解析 |
| `script_service.rs` | `extract_variables` / `run_assertions` / `format_assertion_results` |
| `curl_parser.rs` / `import_openapi.rs` / `import_postman.rs` | 导入解析 |
| `code_gen.rs` | `CodeLanguage` 与 `code_snippet`（响应「代码」tab） |
| `manifest.rs` / `plugin.rs` / `lib.rs` | 元数据与装配（`plugin.rs` 仅改 `view::ApiDebuggerView::new` 引用路径） |

> `service.rs` 的领域逻辑（`send_request`/`perform_request`/各 `_async`）逻辑复用，但**对外契约按 §4 整理**；UI 相关枚举（`EditorTab`/`ResponseTab`/`EnvDetailTab`）移到 `view_model.rs` 或 `view/mod.rs`，不要再放 `service.rs`。

---

## 12. 参考实现

最佳参考是同仓库刚完成 macOS 风格重构的 FTP/SSH 插件：
`crates/qingqi-feature-ftp-sftp-ssh-client/src/view/mod.rs`

具体参考点：
- `left_sidebar` — 左列布局与交通灯让位（`pl` 给原生交通灯留空间）。注意 FTP 用了 chrome，本插件**不用**，但左列内部结构可参考。
- `uniform_list` 用法（`mod.rs:2007`）— 接口树虚拟化照抄。
- `TabBar` / `Tab` 用法（`mod.rs:1539` 的 `titlebar_slot`）— 顶栏右段 tab 栏照抄（但放进右列顶栏，不放进 chrome titlebar）。
- `glass::bg` / `glass::divider` / `glass::shadow`（`crates/qingqi-ui/src/ui/glass.rs`）— 背景、竖直分隔线、阴影。
- overlay 模式（`profile_editor_overlay` 等）— 设计弹窗照此组织。
